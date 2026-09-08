#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sqlite3
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
TEXT_SUFFIXES = {
    ".csv",
    ".json",
    ".md",
    ".py",
    ".txt",
    ".yaml",
    ".yml",
}
EXCLUDED_PARTS = {
    ".git",
    ".agents",
    ".pytest_cache",
    "__pycache__",
    "legacy",
    "node_modules",
}
EXCLUDED_FILES = {
    "CHANGELOG.md",
    # The cross-corpus currentness carrier enumerates exact historical and
    # generated paths; it is a projection, not an active naming authority.
    "docs/validation/documentation-family.current.json",
    "kag/indexes/index_family.manifest.json",
    # The npm lock is exact generated dependency provenance. Authored package
    # identity remains checked in access/web/package.json and web sources.
    "access/web/package-lock.json",
    "scripts/validate_active_naming.py",
}
GENERATED_KAG_PREFIXES = (
    Path("kag/indexes/shards"),
    Path("kag/receipts/index_family_budget"),
)
MECHANICS_TOPOLOGY_ROUTE = "mechanics/topology.json"
RETIRED_TOKENS = (
    "w" + "ave",
    "w" + "aves",
    "s" + "eed",
    "s" + "eeds",
    "s" + "eeded",
    "s" + "eed-pack",
    "s" + "eed_pack",
)
RETIRED_TOKEN_PATTERN = r"(?:" + "|".join(re.escape(token) for token in RETIRED_TOKENS) + r")"
# Preserve Unicode IGNORECASE's ASCII-letter equivalents without changing
# string offsets (str.lower/casefold can expand characters such as U+0130).
# Literal search on the translated text avoids case-folding every word in
# large generated surfaces. The original text remains the diagnostic source.
TOKEN_CASE_TRANSLATION = str.maketrans(
    "ABCDEFGHIJKLMNOPQRSTUVWXYZİıſK", "abcdefghijklmnopqrstuvwxyziisk"
)
FOLDED_RETIRED_TOKEN_PATTERN = re.compile(RETIRED_TOKEN_PATTERN)
PATH_REFERENCE_MARKER_PATTERN = r"(?:[-_/]|\d|\.(?=[A-Za-z0-9]))"
PATH_TOKEN_PATTERN = re.compile(
    r"(?<![A-Za-z0-9])(?:"
    + RETIRED_TOKEN_PATTERN
    + r")(?![A-Za-z0-9])",
    re.IGNORECASE,
)
# Keep the broad path alphabet and IGNORECASE behavior of the legacy matcher,
# but inspect each maximal run once instead of asking a greedy lookahead to
# restart at every path separator.  The extra boundary character matters for
# the legacy marker's ``\d`` branch: Python's ``\d`` also accepts a Unicode
# decimal digit immediately after an ASCII path run.
PATH_RUN_PATTERN = re.compile(r"[A-Za-z0-9._/-]+", re.IGNORECASE)
PATH_RUN_CHARACTERS = frozenset(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._/-İıſK"
)
PATH_MARKER_SEARCH_PATTERN = re.compile(PATH_REFERENCE_MARKER_PATTERN, re.IGNORECASE)
ACTIVE_REFERENCE_PATTERN = re.compile(
    r"(?<![A-Za-z0-9])"
    r"(?=[A-Za-z0-9._/-]*" + PATH_REFERENCE_MARKER_PATTERN + r")"
    r"[A-Za-z0-9._/-]*"
    + RETIRED_TOKEN_PATTERN
    + r"[A-Za-z0-9._/-]*"
    r"(?![A-Za-z0-9])",
    re.IGNORECASE,
)
# These exact content identifiers belong to the current corpus laboratory, not
# to the retired route vocabulary. The exception is intentionally content
# only: the same tokens remain forbidden in active filesystem paths, and every
# near miss continues through the retired-name guard.
ALLOWED_ACTIVE_CONTENT_REFERENCES = frozenset(
    {
        "first-wave",
        "first-wave-resident",
        "may_seed_drafts",
        "may_seed_gold",
        "seed_claim_ref",
        # LensSpec selection grammar and its catalog pointer are API fields,
        # not the retired tree-route vocabulary. Filesystem checks stay strict.
        "seed.focus_node_id",
        "seed.node_ids",
        "seed.text_query",
        "seed_field",
    }
)
# Exact names of external artifacts may contain retired route vocabulary even
# though the repository only quotes them as immutable provenance.  Keep these
# exceptions content-only and exact: they do not authorize a repository path,
# a shortened token, or a renamed near miss.
QUOTED_EXTERNAL_ARTIFACT_IDENTITIES = frozenset(
    {
        "ToS Deep Research_ A48 — Океания _ khipu _ rongorongo as frontier seed.docx",
    }
)
# This exact sentence is captured DOCX provenance prose.  It is retained as a
# content-only quote in generated coverage; it does not authorize the retired
# token in an active path or in any altered/shortened wording.
QUOTED_CAPTURE_PROVENANCE_FRAGMENTS = frozenset(
    {
        "Bentham включён как заданный master-seed и как пороговая фигура: его ранние тексты до 1820 года учитываются только как генеалогический вход, тогда как ядро документа остаётся в пределах 1820–1900."
    }
)
OLD_ROUTE_PREFIX = "z" + "v"
RETIRED_ROUTE_LABEL_PATTERN = re.compile(
    r"(?<![A-Za-z0-9])" + OLD_ROUTE_PREFIX + r"\d+(?:[-_][A-Za-z0-9]+)+",
    re.IGNORECASE,
)
RETIRED_VERSION_PASS_PATTERN = re.compile(
    r"(?<![A-Za-z0-9])" + "v" + r"0\.[6-9](?:\.\d+)?(?![A-Za-z0-9])",
    re.IGNORECASE,
)
RETIRED_EXPERIENCE_VERSION_REF_PATTERN = re.compile(
    r"(?<![A-Za-z0-9])experience\." + "v" + r"0\.",
    re.IGNORECASE,
)
EXPERIENCE_ROUTE_PREFIX = "mechanics/experience/"
RETIRED_NORMALIZED_LABELS = (
    "deployment" + "-" + "watchtower",
    "federation" + "-" + "harvest",
    "adoption" + "-" + "forge",
    "constitution" + "-" + "runtime",
)
NORMALIZED_SEPARATOR_PATTERN = re.compile(r"[_\s]+")
_FEEDBACK_CACHE_MISS = object()


class FeedbackContentCache:
    """Best-effort local cache for the pure content-result check.

    This cache is intentionally not part of the validator's source of truth.
    The caller opts in with an external path for repeated local feedback; the
    release lane never supplies one. Cache rows are only trusted as local
    performance hints, while malformed storage falls back to recomputation.
    """

    def __init__(self, path: Path) -> None:
        self.path = path
        connection: sqlite3.Connection | None = None
        try:
            # Feedback is optional; lock contention should immediately fall
            # back to the uncached validator instead of delaying a local run.
            connection = sqlite3.connect(path, timeout=0.0)
            connection.execute(
                """
                CREATE TABLE IF NOT EXISTS content_results (
                    policy TEXT NOT NULL,
                    content_digest TEXT NOT NULL,
                    result_json TEXT NOT NULL,
                    PRIMARY KEY (policy, content_digest)
                )
                """
            )
            connection.commit()
            source_bytes = Path(__file__).resolve().read_bytes()
            policy = hashlib.sha256(
                source_bytes + sys.version.encode("utf-8")
            ).hexdigest()
            # A validator-source or Python-version change makes every prior
            # row stale. Keep only the active policy so an edit-heavy local
            # cache does not grow one full content corpus per source revision.
            connection.execute(
                "DELETE FROM content_results WHERE policy != ?",
                (policy,),
            )
            connection.commit()
        except BaseException:
            if connection is not None:
                try:
                    connection.close()
                except Exception:
                    pass
            raise
        self.connection = connection
        self.policy = policy
        self.hits = 0
        self.misses = 0

    @staticmethod
    def content_digest(text: str) -> str:
        return hashlib.sha256(text.encode("utf-8")).hexdigest()

    def lookup(self, text: str) -> object:
        digest = self.content_digest(text)
        row = self.connection.execute(
            "SELECT result_json FROM content_results WHERE policy=? AND content_digest=?",
            (self.policy, digest),
        ).fetchone()
        if row is None:
            self.misses += 1
            return _FEEDBACK_CACHE_MISS
        try:
            result = json.loads(row[0])
        except (TypeError, UnicodeDecodeError, json.JSONDecodeError):
            self.connection.execute(
                "DELETE FROM content_results WHERE policy=? AND content_digest=?",
                (self.policy, digest),
            )
            self.misses += 1
            return _FEEDBACK_CACHE_MISS
        if result is not None and not isinstance(result, str):
            self.connection.execute(
                "DELETE FROM content_results WHERE policy=? AND content_digest=?",
                (self.policy, digest),
            )
            self.misses += 1
            return _FEEDBACK_CACHE_MISS
        self.hits += 1
        return result

    def store(self, text: str, result: str | None) -> None:
        self.connection.execute(
            "INSERT OR REPLACE INTO content_results(policy, content_digest, result_json) VALUES (?, ?, ?)",
            (
                self.policy,
                self.content_digest(text),
                json.dumps(result, ensure_ascii=False),
            ),
        )

    def close(self) -> None:
        try:
            self.connection.commit()
        finally:
            self.connection.close()


def relative(path: Path) -> str:
    return path.relative_to(REPO_ROOT).as_posix()


def is_excluded(path: Path) -> bool:
    return _is_excluded_relative(path.relative_to(REPO_ROOT))


def _is_excluded_relative(rel: Path) -> bool:
    return (
        rel.as_posix() in EXCLUDED_FILES
        or any(prefix == rel or prefix in rel.parents for prefix in GENERATED_KAG_PREFIXES)
        or any(part in EXCLUDED_PARTS for part in rel.parts)
    )


def normalize_label_surface(value: str) -> str:
    return NORMALIZED_SEPARATOR_PATTERN.sub("-", value).lower()


def retired_normalized_label(value: str) -> str | None:
    normalized = normalize_label_surface(value)
    for label in RETIRED_NORMALIZED_LABELS:
        if label in normalized:
            return label
    return None


def retired_path_issue(value: str) -> str | None:
    match = PATH_TOKEN_PATTERN.search(value)
    if match:
        return match.group(0)
    match = RETIRED_ROUTE_LABEL_PATTERN.search(value)
    if match:
        return match.group(0)
    marker = retired_normalized_label(value)
    if marker:
        return marker
    return None


def active_reference_issue(text: str) -> str | None:
    """Return the first non-allowlisted retired path-like content reference."""
    consumed = 0
    for token in FOLDED_RETIRED_TOKEN_PATTERN.finditer(text.translate(TOKEN_CASE_TRANSLATION)):
        if token.start() < consumed:
            continue
        # Inspect only runs containing a retired token, once per run, rather
        # than applying the token regex to millions of unrelated words.
        start = token.start()
        while start and text[start - 1] in PATH_RUN_CHARACTERS:
            start -= 1
        end = PATH_RUN_PATTERN.match(text, token.start()).end()
        consumed = end
        # Search through one character beyond the run without copying the
        # string.  This preserves ACTIVE_REFERENCE_PATTERN's Unicode ``\d``
        # marker behavior at a path-run boundary.
        if PATH_MARKER_SEARCH_PATTERN.search(text, start, end + 1) is None:
            continue
        reference = text[start:end]
        if reference.lower() not in ALLOWED_ACTIVE_CONTENT_REFERENCES:
            return reference
    return None


def retired_content_issue(text: str) -> str | None:
    for artifact_identity in QUOTED_EXTERNAL_ARTIFACT_IDENTITIES:
        text = text.replace(artifact_identity, "[quoted-external-artifact-identity]")
    for capture_fragment in QUOTED_CAPTURE_PROVENANCE_FRAGMENTS:
        text = text.replace(capture_fragment, "[quoted-capture-provenance-fragment]")
    # Keep the legacy path-shaped matcher as a bounded test oracle; the live
    # search examines only runs containing a retired token.
    reference = active_reference_issue(text)
    if reference is not None:
        return reference
    match = RETIRED_ROUTE_LABEL_PATTERN.search(text)
    if match:
        return match.group(0)
    match = RETIRED_EXPERIENCE_VERSION_REF_PATTERN.search(text)
    if match:
        return match.group(0)
    marker = retired_normalized_label(text)
    if marker:
        return marker
    return None


def retired_experience_pass_issue(value: str) -> str | None:
    match = RETIRED_VERSION_PASS_PATTERN.search(value)
    return match.group(0) if match else None


def scalar_fragments(value: object) -> list[str]:
    if isinstance(value, str):
        return [value]
    if isinstance(value, bool):
        return [str(value).lower()]
    if isinstance(value, (int, float)):
        return [str(value)]
    if isinstance(value, list):
        fragments: list[str] = []
        for item in value:
            fragments.extend(scalar_fragments(item))
        return fragments
    return []


def mechanics_topology_active_text(text: str) -> str:
    try:
        payload = json.loads(text)
    except json.JSONDecodeError:
        return text
    if not isinstance(payload, dict):
        return text

    fragments: list[str] = []
    for key in ("schema_version", "owner_repo", "root", "legacy_policy"):
        fragments.extend(scalar_fragments(payload.get(key)))

    packages = payload.get("packages")
    if isinstance(packages, list):
        for package in packages:
            if not isinstance(package, dict):
                continue
            for key in ("slug", "class", "status", "active_parts", "legacy_required"):
                fragments.extend(scalar_fragments(package.get(key)))

    moved_targets = payload.get("moved_path_targets")
    if isinstance(moved_targets, dict):
        for active_target in moved_targets.values():
            fragments.extend(scalar_fragments(active_target))

    return "\n".join(fragments)


def active_content_text(rel: str, text: str) -> str:
    if rel == MECHANICS_TOPOLOGY_ROUTE:
        return mechanics_topology_active_text(text)
    return text


def _path_is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def feedback_cache_path(path: Path) -> Path:
    """Resolve and reject a feedback cache path inside the repository."""
    repository_root = REPO_ROOT.resolve()
    requested = path.expanduser()
    absolute = requested.absolute()
    resolved = requested.resolve()
    if _path_is_within(absolute, repository_root) or _path_is_within(
        resolved, repository_root
    ):
        raise ValueError("feedback cache must be outside the repository")
    return resolved


def validate(*, feedback_cache: Path | None = None) -> list[str]:
    issues: list[str] = []
    cache: FeedbackContentCache | None = None
    cache_stats: tuple[Path, int, int] | None = None
    cache_warning_reported = False
    if feedback_cache is not None:
        cache_path = feedback_cache_path(feedback_cache)
        try:
            cache_path.parent.mkdir(parents=True, exist_ok=True)
            cache = FeedbackContentCache(cache_path)
        except (OSError, sqlite3.Error) as exc:
            print(
                f"[feedback-cache] unavailable; recomputing uncached: {exc}",
                file=sys.stderr,
            )

    def check_content(text: str) -> str | None:
        nonlocal cache, cache_stats, cache_warning_reported
        if cache is None:
            return retired_content_issue(text)
        try:
            cached = cache.lookup(text)
            if cached is not _FEEDBACK_CACHE_MISS:
                return cached  # type: ignore[return-value]
            result = retired_content_issue(text)
            cache.store(text, result)
            return result
        except (OSError, sqlite3.Error) as exc:
            if not cache_warning_reported:
                print(
                    f"[feedback-cache] read/write failure; recomputing uncached: {exc}",
                    file=sys.stderr,
                )
                cache_warning_reported = True
            cache_stats = (cache.path, cache.hits, cache.misses)
            try:
                cache.close()
            except (OSError, sqlite3.Error):
                pass
            cache = None
            return retired_content_issue(text)

    # Walk top-down so excluded trees are pruned before their entries are
    # materialized. Path.rglob() enumerated those trees first and only then
    # discarded them in is_excluded(), which was needlessly expensive for a
    # repository with generated/read-model directories.
    try:
        for dirpath, dirnames, filenames in os.walk(REPO_ROOT, topdown=True, followlinks=False):
            rel_root = Path(dirpath).relative_to(REPO_ROOT)
            active_dirnames: list[str] = []
            for dirname in sorted(dirnames):
                rel_path = rel_root / dirname
                if _is_excluded_relative(rel_path):
                    continue
                rel = rel_path.as_posix()
                path_issue = retired_path_issue(rel)
                if path_issue:
                    issues.append(f"{rel}: retired active name in path: {path_issue}")
                if rel.startswith(EXPERIENCE_ROUTE_PREFIX):
                    path_issue = retired_experience_pass_issue(rel)
                    if path_issue:
                        issues.append(f"{rel}: retired experience pass marker in path: {path_issue}")
                active_dirnames.append(dirname)
            dirnames[:] = active_dirnames

            for filename in sorted(filenames):
                rel_path = rel_root / filename
                if _is_excluded_relative(rel_path):
                    continue
                rel = rel_path.as_posix()
                path = Path(dirpath) / filename
                path_issue = retired_path_issue(rel)
                if path_issue:
                    issues.append(f"{rel}: retired active name in path: {path_issue}")
                if rel.startswith(EXPERIENCE_ROUTE_PREFIX):
                    path_issue = retired_experience_pass_issue(rel)
                    if path_issue:
                        issues.append(f"{rel}: retired experience pass marker in path: {path_issue}")
                if not path.is_file() or path.suffix not in TEXT_SUFFIXES:
                    continue
                try:
                    text = path.read_text(encoding="utf-8")
                except UnicodeDecodeError:
                    continue
                text = active_content_text(rel, text)
                content_issue = check_content(text)
                if content_issue:
                    issues.append(f"{rel}: retired active path/id reference in content: {content_issue}")
                if rel.startswith(EXPERIENCE_ROUTE_PREFIX):
                    content_issue = retired_experience_pass_issue(text)
                    if content_issue:
                        issues.append(f"{rel}: retired experience pass marker in content: {content_issue}")
    finally:
        if cache is not None:
            cache_stats = (cache.path, cache.hits, cache.misses)
            try:
                cache.close()
            except (OSError, sqlite3.Error) as exc:
                if not cache_warning_reported:
                    print(
                        f"[feedback-cache] close failure; validation result may include cache hits and cache writes may be incomplete: {exc}",
                        file=sys.stderr,
                    )
        if cache_stats is not None:
            path, hits, misses = cache_stats
            print(
                f"[feedback-cache] local-only path={path} hits={hits} misses={misses}",
                file=sys.stderr,
            )
    return issues


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate active naming paths and content."
    )
    parser.add_argument(
        "--feedback-cache",
        type=Path,
        metavar="PATH",
        help=(
            "opt-in local SQLite cache for pure content checks; PATH must be "
            "outside the repository and is never used by release/CI lanes"
        ),
    )
    args = parser.parse_args(argv)
    try:
        issues = validate(feedback_cache=args.feedback_cache)
    except ValueError as exc:
        print(f"Active naming validation configuration failed: {exc}", file=sys.stderr)
        return 2
    if issues:
        print("Active naming validation failed.", file=sys.stderr)
        for issue in issues:
            print(f"- {issue}", file=sys.stderr)
        return 1
    print("[ok] validated active naming")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
