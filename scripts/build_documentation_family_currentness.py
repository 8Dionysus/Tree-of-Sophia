#!/usr/bin/env python3
"""Build the source-derived documentation family currentness read model."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
MAP_PATH = Path("docs/validation/documentation_family_map.json")
CURRENTNESS_PATH = Path("docs/validation/documentation-family.current.json")
HUMAN_EXTENSIONS = {".md", ".txt", ".yaml", ".yml"}


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def load_map(repo_root: Path = REPO_ROOT) -> tuple[Path, dict[str, Any]]:
    path = repo_root / MAP_PATH
    return path, json.loads(path.read_text(encoding="utf-8"))


def tracked_paths(repo_root: Path = REPO_ROOT) -> list[Path]:
    result = subprocess.run(
        ("git", "ls-files", "-z"),
        cwd=repo_root,
        check=True,
        capture_output=True,
    )
    return [
        Path(raw.decode("utf-8"))
        for raw in result.stdout.split(b"\0")
        if raw
    ]


def relative_path(repo_root: Path, path: Path) -> str:
    return path.relative_to(repo_root).as_posix()


def included_surface(path: Path, surface_rules: dict[str, Any]) -> bool:
    value = path.as_posix()
    if value in set(surface_rules.get("exclude_paths", [])):
        return False
    if any(value.startswith(prefix) for prefix in surface_rules.get("exclude_prefixes", [])):
        return False
    extensions = surface_rules.get("include_extensions", [])
    return path.suffix in extensions


def projection_tracked_paths(paths: list[Path], surface_rules: dict[str, Any]) -> list[Path]:
    excluded_paths = set(surface_rules.get("exclude_paths", []))
    excluded_prefixes = tuple(surface_rules.get("exclude_prefixes", []))
    return [
        path
        for path in paths
        if path.as_posix() not in excluded_paths
        and not any(path.as_posix().startswith(prefix) for prefix in excluded_prefixes)
    ]


def family_for(path: Path, families: list[dict[str, Any]]) -> str | None:
    value = path.as_posix()
    for family in families:
        match = family.get("match", {})
        if match.get("kind") == "root_files" and len(path.parts) == 1:
            return family.get("id")
        prefix = match.get("prefix")
        if isinstance(prefix, str) and value.startswith(prefix):
            return family.get("id")
    return None


def surface_kind(path: Path) -> str:
    if path.suffix in HUMAN_EXTENSIONS:
        return "human"
    if path.suffix == ".json":
        return "structured"
    if path.suffix in {".py", ".sh"}:
        return "executable"
    return "other"


def in_human_scope(path: Path) -> bool:
    return path.suffix in HUMAN_EXTENSIONS and not (
        path.as_posix().startswith(".agents/skills/")
        and path.parts[-2:] == ("agents", "openai.yaml")
    )


def human_scope(repo_root: Path, paths: list[Path], families: list[dict[str, Any]]) -> dict[str, Any]:
    human = [path for path in paths if in_human_scope(path)]
    by_family: Counter[str] = Counter()
    bytes_by_family: Counter[str] = Counter()
    lines_by_family: Counter[str] = Counter()
    for path in human:
        family_id = family_for(path, families) or "unhandled"
        payload = (repo_root / path).read_bytes()
        by_family[family_id] += 1
        bytes_by_family[family_id] += len(payload)
        lines_by_family[family_id] += payload.count(b"\n")
    return {
        "count": len(human),
        "bytes": sum(bytes_by_family.values()),
        "lines": sum(lines_by_family.values()),
        "family_counts": {
            family_id: {
                "count": by_family[family_id],
                "bytes": bytes_by_family[family_id],
                "lines": lines_by_family[family_id],
            }
            for family_id in sorted(by_family)
        },
        "excluded_skill_launch_metadata": sum(
            path.suffix in HUMAN_EXTENSIONS
            and path.as_posix().startswith(".agents/skills/")
            and path.parts[-2:] == ("agents", "openai.yaml")
            for path in paths
        ),
    }


def build_currentness(repo_root: Path = REPO_ROOT) -> dict[str, Any]:
    map_path, source_map = load_map(repo_root)
    families = source_map["families"]
    surface_rules = source_map["surface_rules"]
    tracked = tracked_paths(repo_root)
    projection_tracked = projection_tracked_paths(tracked, surface_rules)
    surface_paths = [path for path in projection_tracked if included_surface(path, surface_rules)]
    records: list[dict[str, Any]] = []
    unhandled: list[str] = []
    family_records: defaultdict[str, list[dict[str, Any]]] = defaultdict(list)
    kind_counts: Counter[str] = Counter()

    for path in surface_paths:
        family_id = family_for(path, families)
        relative = path.as_posix()
        if family_id is None:
            unhandled.append(relative)
            family_id = "unhandled"
        payload = (repo_root / path).read_bytes()
        kind = surface_kind(path)
        kind_counts[kind] += 1
        record = {
            "path": relative,
            "family_id": family_id,
            "surface_kind": kind,
            "bytes": len(payload),
            "sha256": sha256_bytes(payload),
            "currentness": "tracked_source_snapshot",
        }
        records.append(record)
        family_records[family_id].append(record)

    records.sort(key=lambda item: item["path"])
    digest_input = b"".join(
        f"{item['path']}\0{item['sha256']}\0".encode("utf-8")
        for item in records
    )
    source_snapshot_digest = sha256_bytes(digest_input)
    summaries = []
    for family in families:
        family_id = family["id"]
        members = family_records.get(family_id, [])
        summaries.append(
            {
                "family_id": family_id,
                "tracked_surface_count": len(members),
                "tracked_surface_bytes": sum(item["bytes"] for item in members),
                "surface_kind_counts": dict(sorted(Counter(item["surface_kind"] for item in members).items())),
            }
        )

    coverage = {
        "tracked_path_count": len(projection_tracked),
        "tracked_surface_count": len(records),
        "excluded_tracked_path_count": len(projection_tracked) - len(surface_paths),
        "excluded_generated_carrier_rules": len(set(surface_rules.get("exclude_paths", []))) + len(set(surface_rules.get("exclude_prefixes", []))),
        "unhandled_family_count": len(unhandled),
        "unhandled_paths": sorted(unhandled),
        "surface_kind_counts": dict(sorted(kind_counts.items())),
        "human_scope": human_scope(repo_root, tracked, families),
    }
    context_summary = {
        "metric": "whitespace_tokens_v1",
        "posture": "summary_first_records_on_demand",
        "tracked_surface_count": len(records),
        "family_count": len(families),
        "unhandled_family_count": len(unhandled),
        "record_loading": "machine readers may select records by family_id or path; do not load the full carrier into an always-on prompt",
    }
    return {
        "schema_version": "tos_documentation_family_currentness_v1",
        "source_map": relative_path(repo_root, map_path),
        "source_map_sha256": sha256_bytes(map_path.read_bytes()),
        "generated_by": "scripts/build_documentation_family_currentness.py",
        "tracked_source": "git ls-files -z",
        "tracked_surface_digest": source_snapshot_digest,
        "atlas_method": source_map["atlas_method"],
        "coverage": coverage,
        "family_summaries": summaries,
        "context_summary": context_summary,
        "tracked_surfaces": records,
    }


def render_currentness(data: dict[str, Any]) -> str:
    return json.dumps(data, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def resolve_output(repo_root: Path, output: Path | None) -> Path:
    if output is None:
        return repo_root / CURRENTNESS_PATH
    return output if output.is_absolute() else repo_root / output


def display_path(repo_root: Path, path: Path) -> str:
    try:
        return path.relative_to(repo_root).as_posix()
    except ValueError:
        return path.as_posix()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="check the generated projection without writing")
    parser.add_argument("--output", type=Path, default=None, help="override the generated output path")
    args = parser.parse_args()
    output = resolve_output(REPO_ROOT, args.output)
    rendered = render_currentness(build_currentness(REPO_ROOT))
    if args.check:
        if not output.is_file() or output.read_text(encoding="utf-8") != rendered:
            print(f"documentation family currentness is stale or missing: {display_path(REPO_ROOT, output)}")
            return 1
        print(f"documentation family currentness is current: {display_path(REPO_ROOT, output)}")
        return 0
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(rendered, encoding="utf-8")
    print(f"wrote {display_path(REPO_ROOT, output)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
