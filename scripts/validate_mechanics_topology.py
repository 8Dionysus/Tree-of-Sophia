#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import TypeAlias
from urllib.parse import unquote


REPO_ROOT = Path(__file__).resolve().parents[1]
TOPOLOGY_PATH = Path("mechanics/topology.json")

Issue: TypeAlias = tuple[str, str]

PACKAGE_SLUG_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
ALLOWED_CLASSES = {"head-fed/local", "local"}
ALLOWED_STATUSES = {"active", "planted"}
IGNORED_MECHANICS_DIRS = {"__pycache__", "legacy"}
MARKDOWN_LINK_RE = re.compile(r"(?<!!)\[[^\]]+\]\(([^)\s]+)")
MARKDOWN_REFERENCE_USE_RE = re.compile(r"(?<!!)\[([^\]]+)\]\[([^\]]*)\]")
MARKDOWN_REFERENCE_DEFINITION_RE = re.compile(
    r"(?m)^[ \t]{0,3}\[([^\]]+)\]:\s*(?:<([^>\n]+)>|(\S+))"
)
SCRIPT_REFERENCE_RE = re.compile(
    r"(?<![A-Za-z0-9_.-])((?:\.\./)*(?:scripts|mechanics)/[A-Za-z0-9_./-]+\.(?:py|sh))"
)
SCRIPT_INVENTORY_PATH = Path("docs/validation/script_inventory.json")
ROUTE_MAP_MARKERS = (
    "## Task-to-owner map",
    "## Progressive disclosure",
    "topology.json",
    "validation_lanes.json",
    "docs/decisions/README.md",
)


def load_topology(repo_root: Path, issues: list[Issue]) -> dict[str, object] | None:
    path = repo_root / TOPOLOGY_PATH
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        issues.append((TOPOLOGY_PATH.as_posix(), "missing mechanics topology"))
        return None
    except json.JSONDecodeError as exc:
        issues.append((TOPOLOGY_PATH.as_posix(), f"invalid JSON: {exc}"))
        return None
    if not isinstance(payload, dict):
        issues.append((TOPOLOGY_PATH.as_posix(), "topology root must be a JSON object"))
        return None
    return payload


def require_file(repo_root: Path, issues: list[Issue], relative_path: str) -> None:
    if not (repo_root / relative_path).is_file():
        issues.append((relative_path, "missing required file"))


def require_absent(repo_root: Path, issues: list[Issue], relative_path: str, message: str) -> None:
    if (repo_root / relative_path).exists():
        issues.append((relative_path, message))


def read_text_if_file(repo_root: Path, relative_path: str) -> str | None:
    path = repo_root / relative_path
    if not path.is_file():
        return None
    return path.read_text(encoding="utf-8")


def current_mechanics_markdown(repo_root: Path) -> tuple[Path, ...]:
    mechanics_root = repo_root / "mechanics"
    if not mechanics_root.is_dir():
        return ()
    return tuple(
        sorted(
            path
            for path in mechanics_root.rglob("*.md")
            if path.name != "AGENTS.md" and "legacy" not in path.relative_to(repo_root).parts
        )
    )


def reference_parts(reference: str) -> tuple[str, str]:
    value = reference.strip("<>")
    target_fragment = value.split("#", 1)
    target = target_fragment[0].split("?", 1)[0]
    fragment = target_fragment[1] if len(target_fragment) == 2 else ""
    return target, fragment


def normalized_destination(reference: str) -> str:
    return reference_parts(reference)[0]


def rendered_markdown(text: str) -> str:
    text = re.sub(r"(?s)<!--.*?-->", "", text)
    return re.sub(r"(?ms)^[ \t]*(`{3,}|~{3,}).*?^[ \t]*\1[ \t]*$", "", text)


def markdown_destinations(text: str, *, rendered_only: bool = False) -> tuple[str, ...]:
    if rendered_only:
        text = rendered_markdown(text)
    destinations = list(MARKDOWN_LINK_RE.findall(text))
    definitions = {
        re.sub(r"\s+", " ", label.strip()).casefold(): (angle or bare)
        for label, angle, bare in MARKDOWN_REFERENCE_DEFINITION_RE.findall(text)
        if not label.strip().startswith("^")
    }
    destinations.extend(definitions.values())
    return tuple(destinations)


def heading_anchor(value: str) -> str:
    value = re.sub(r"\{#([^}]+)\}", "", value)
    value = re.sub(r"<[^>]+>", "", value)
    value = re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", value)
    value = re.sub(r"[^\w\s-]", "", value.casefold())
    return re.sub(r"[\s-]+", "-", value).strip("-")


def document_anchor_ids(path: Path) -> set[str]:
    text = path.read_text(encoding="utf-8")
    anchors = {
        unquote(value).casefold()
        for value in re.findall(r"(?:id|name)\s*=\s*[\"']([^\"']+)[\"']", text)
    }
    anchors.update(value.casefold() for value in re.findall(r"\{#([A-Za-z0-9][A-Za-z0-9_-]*)\}", text))
    counts: dict[str, int] = {}
    for heading in re.findall(r"(?m)^[ \t]{0,3}#{1,6}\s+(.+?)\s*#*\s*$", text):
        anchor = heading_anchor(heading)
        if not anchor:
            continue
        occurrence = counts.get(anchor, 0)
        anchors.add(anchor if occurrence == 0 else f"{anchor}-{occurrence}")
        counts[anchor] = occurrence + 1
    return anchors


def document_has_fragment(path: Path, fragment: str) -> bool:
    return not fragment or unquote(fragment).casefold() in document_anchor_ids(path)


def resolve_doc_reference(repo_root: Path, source_path: Path, reference: str) -> Path | None:
    target, fragment = reference_parts(reference)
    if not target and not fragment:
        return None
    if target.startswith("/") or "://" in target or target.startswith("mailto:"):
        return None

    resolved_root = repo_root.resolve()
    candidates = (source_path,) if not target else (source_path.parent / target, repo_root / target)
    for candidate in candidates:
        try:
            resolved = candidate.resolve()
            resolved.relative_to(resolved_root)
        except (OSError, ValueError):
            continue
        if resolved.exists():
            return resolved
    return None


def load_inventory_paths(repo_root: Path, issues: list[Issue]) -> set[str]:
    path = repo_root / SCRIPT_INVENTORY_PATH
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        issues.append((SCRIPT_INVENTORY_PATH.as_posix(), "missing script inventory for route references"))
        return set()
    except json.JSONDecodeError as exc:
        issues.append((SCRIPT_INVENTORY_PATH.as_posix(), f"invalid script inventory: {exc}"))
        return set()

    entries = payload.get("script_surfaces") if isinstance(payload, dict) else None
    if not isinstance(entries, list):
        issues.append((SCRIPT_INVENTORY_PATH.as_posix(), "script inventory must contain a script_surfaces list"))
        return set()
    paths: set[str] = set()
    for entry in entries:
        if isinstance(entry, dict) and isinstance(entry.get("path"), str):
            paths.add(entry["path"])
    return paths


def validate_route_map(
    repo_root: Path,
    issues: list[Issue],
    packages: dict[str, dict[str, object]],
    package_parts: dict[str, set[str]],
) -> None:
    root_text = read_text_if_file(repo_root, "mechanics/README.md")
    if root_text is None:
        return

    for marker in ROUTE_MAP_MARKERS:
        if marker not in root_text:
            issues.append(("mechanics/README.md", f"missing executable-architecture route marker: {marker}"))

    for slug in packages:
        package_ref = f"]({slug}/README.md)"
        if package_ref not in root_text:
            issues.append(("mechanics/README.md", f"package {slug} is missing from the human package map"))

        package_path = f"mechanics/{slug}/README.md"
        package_text = read_text_if_file(repo_root, package_path)
        if package_text is None:
            continue
        package_links = {
            normalized_destination(reference)
            for reference in markdown_destinations(package_text, rendered_only=True)
        }
        for companion in ("PARTS.md", "PROVENANCE.md", "ROADMAP.md"):
            if companion not in package_links:
                issues.append((package_path, f"package route does not link to {companion}"))

        parts_text = read_text_if_file(repo_root, f"mechanics/{slug}/PARTS.md")
        if parts_text is None:
            continue
        part_links = {
            normalized_destination(reference)
            for reference in markdown_destinations(parts_text, rendered_only=True)
        }
        for part in sorted(package_parts.get(slug, set())):
            part_ref = f"parts/{part}/README.md"
            if part_ref not in part_links:
                issues.append(
                    (
                        f"mechanics/{slug}/PARTS.md",
                        f"active part {part} is not present in the package selection map",
                    )
                )


def validate_documentation_references(repo_root: Path, issues: list[Issue]) -> None:
    inventory_paths = load_inventory_paths(repo_root, issues)
    for path in current_mechanics_markdown(repo_root):
        relative = path.relative_to(repo_root).as_posix()
        text = path.read_text(encoding="utf-8")
        references = list(markdown_destinations(text))
        definitions = {
            re.sub(r"\s+", " ", label.strip()).casefold(): (angle or bare)
            for label, angle, bare in MARKDOWN_REFERENCE_DEFINITION_RE.findall(text)
        }
        for label, explicit_label in MARKDOWN_REFERENCE_USE_RE.findall(text):
            key = re.sub(r"\s+", " ", (explicit_label or label).strip()).casefold()
            if key not in definitions:
                issues.append((relative, f"unresolved reference-style documentation route: {explicit_label or label}"))
            else:
                references.append(definitions[key])

        for reference in references:
            if reference.startswith(("http:", "https:", "mailto:")):
                continue
            resolved = resolve_doc_reference(repo_root, path, reference)
            if resolved is None:
                issues.append((relative, f"broken local documentation route: {reference}"))
            elif reference_parts(reference)[1] and not document_has_fragment(resolved, reference_parts(reference)[1]):
                issues.append((relative, f"broken local documentation fragment: {reference}"))

        for reference in SCRIPT_REFERENCE_RE.findall(text):
            resolved = resolve_doc_reference(repo_root, path, reference)
            if resolved is None:
                issues.append((relative, f"stale executable reference: {reference}"))
                continue
            resolved_relative = resolved.relative_to(repo_root).as_posix()
            if resolved_relative not in inventory_paths:
                issues.append(
                    (
                        relative,
                        f"executable reference is absent from script inventory: {resolved_relative}",
                    )
                )


def validate_context_budget(repo_root: Path, issues: list[Issue], topology: dict[str, object]) -> None:
    budget = topology.get("always_on_context_budget")
    if not isinstance(budget, dict):
        issues.append((TOPOLOGY_PATH.as_posix(), "always_on_context_budget must be an object"))
        return

    surface = budget.get("surface")
    metric = budget.get("metric")
    maximum = budget.get("max_tokens")
    if surface != "mechanics/README.md":
        issues.append((TOPOLOGY_PATH.as_posix(), "always_on_context_budget.surface must be mechanics/README.md"))
    if metric != "whitespace_tokens_v1":
        issues.append((TOPOLOGY_PATH.as_posix(), "always_on_context_budget.metric is unsupported"))
    if isinstance(maximum, bool) or not isinstance(maximum, int) or maximum <= 0:
        issues.append((TOPOLOGY_PATH.as_posix(), "always_on_context_budget.max_tokens must be a positive integer"))
        return

    text = read_text_if_file(repo_root, str(surface)) if isinstance(surface, str) else None
    if text is None:
        issues.append((str(surface), "always-on context surface is missing"))
        return
    measured = len(text.split())
    if measured > maximum:
        issues.append(
            (
                str(surface),
                f"always-on context exceeds {maximum} whitespace tokens: {measured}",
            )
        )


def discover_package_dirs(repo_root: Path) -> set[str]:
    mechanics_root = repo_root / "mechanics"
    if not mechanics_root.is_dir():
        return set()
    return {
        path.name
        for path in mechanics_root.iterdir()
        if path.is_dir() and path.name not in IGNORED_MECHANICS_DIRS and (path / "AGENTS.md").is_file()
    }


def validate_package(repo_root: Path, issues: list[Issue], slug: str, entry: dict[str, object]) -> tuple[str, ...]:
    if not PACKAGE_SLUG_RE.fullmatch(slug):
        issues.append((TOPOLOGY_PATH.as_posix(), f"package slug is not normalized: {slug}"))

    for filename in ("AGENTS.md", "README.md", "PARTS.md", "PROVENANCE.md", "ROADMAP.md"):
        require_file(repo_root, issues, f"mechanics/{slug}/{filename}")

    package_class = entry.get("class")
    if package_class not in ALLOWED_CLASSES:
        issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.class must be one of {sorted(ALLOWED_CLASSES)!r}"))

    status = entry.get("status")
    if status not in ALLOWED_STATUSES:
        issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.status must be one of {sorted(ALLOWED_STATUSES)!r}"))

    active_parts = entry.get("active_parts")
    if (
        not isinstance(active_parts, list)
        or not active_parts
        or not all(isinstance(part, str) and PACKAGE_SLUG_RE.fullmatch(part) for part in active_parts)
    ):
        issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.active_parts must be a non-empty normalized string list"))
        parts: tuple[str, ...] = ()
    else:
        parts = tuple(active_parts)
        if len(parts) != len(set(parts)):
            issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.active_parts contains duplicates"))

    for part in parts:
        require_file(repo_root, issues, f"mechanics/{slug}/parts/{part}/README.md")

    legacy_required = entry.get("legacy_required")
    if not isinstance(legacy_required, bool):
        issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.legacy_required must be boolean"))
        return parts
    if legacy_required:
        require_file(repo_root, issues, f"mechanics/{slug}/legacy/README.md")
        require_file(repo_root, issues, f"mechanics/{slug}/legacy/INDEX.md")
    else:
        require_absent(
            repo_root,
            issues,
            f"mechanics/{slug}/legacy",
            "package-local legacy is allowed only for moved-path or raw-receipt accounting",
        )

    return parts


def validate_moved_targets(
    repo_root: Path,
    issues: list[Issue],
    topology: dict[str, object],
    packages: dict[str, dict[str, object]],
    package_parts: dict[str, set[str]],
) -> None:
    moved_accounting = topology.get("moved_path_accounting")
    if not isinstance(moved_accounting, dict):
        issues.append((TOPOLOGY_PATH.as_posix(), "moved_path_accounting must be an object"))
        return

    moved_targets = topology.get("moved_path_targets")
    if not isinstance(moved_targets, dict):
        issues.append((TOPOLOGY_PATH.as_posix(), "moved_path_targets must be an object"))
        return

    accounted_old_paths: set[str] = set()
    packages_with_moved_accounting: set[str] = set()
    for package, parts in moved_accounting.items():
        if not isinstance(parts, dict):
            issues.append((TOPOLOGY_PATH.as_posix(), f"moved_path_accounting.{package} must be an object"))
            continue
        if package not in packages:
            issues.append((TOPOLOGY_PATH.as_posix(), f"moved_path_accounting.{package} is not a known package"))
            known_parts: set[str] = set()
        else:
            known_parts = package_parts.get(package, set())
        for part, old_paths in parts.items():
            if part not in known_parts:
                issues.append((TOPOLOGY_PATH.as_posix(), f"moved_path_accounting.{package}.{part} is not an active part"))
            if not isinstance(old_paths, list) or not all(isinstance(item, str) and item for item in old_paths):
                issues.append((TOPOLOGY_PATH.as_posix(), f"moved_path_accounting.{package}.{part} must be a string list"))
                continue
            if old_paths and package in packages:
                packages_with_moved_accounting.add(package)
            for old_path in old_paths:
                accounted_old_paths.add(old_path)
                new_path = moved_targets.get(old_path)
                if not isinstance(new_path, str) or not new_path:
                    issues.append((TOPOLOGY_PATH.as_posix(), f"{old_path} is missing a moved_path_targets entry"))
                    continue
                expected_prefix = f"mechanics/{package}/parts/{part}/"
                if part in known_parts and not new_path.startswith(expected_prefix):
                    issues.append((TOPOLOGY_PATH.as_posix(), f"{old_path} target must stay under {expected_prefix}"))
                require_absent(
                    repo_root,
                    issues,
                    old_path,
                    "mechanic-owned payload must stay in mechanics/, not the old ToS/root path",
                )
                require_file(repo_root, issues, new_path)
                legacy_index = read_text_if_file(repo_root, f"mechanics/{package}/legacy/INDEX.md")
                if legacy_index is not None:
                    if old_path not in legacy_index:
                        issues.append(
                            (
                                f"mechanics/{package}/legacy/INDEX.md",
                                f"missing former path from moved_path_accounting: {old_path}",
                            )
                        )
                    if new_path not in legacy_index:
                        issues.append(
                            (
                                f"mechanics/{package}/legacy/INDEX.md",
                                f"missing active target from moved_path_targets: {new_path}",
                            )
                        )

    for old_path, new_path in moved_targets.items():
        if not isinstance(old_path, str) or not isinstance(new_path, str):
            issues.append((TOPOLOGY_PATH.as_posix(), "moved_path_targets keys and values must be strings"))
            continue
        if old_path not in accounted_old_paths:
            issues.append((TOPOLOGY_PATH.as_posix(), f"{old_path} target is not listed in moved_path_accounting"))

    for package, entry in packages.items():
        if entry.get("legacy_required") is True and package not in packages_with_moved_accounting:
            issues.append(
                (
                    TOPOLOGY_PATH.as_posix(),
                    f"{package}.legacy_required true requires moved_path_accounting entries",
                )
            )


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    require_file(root, issues, "mechanics/AGENTS.md")
    require_file(root, issues, "mechanics/README.md")
    require_absent(root, issues, "mechanics/legacy", "root mechanics legacy is forbidden")

    topology = load_topology(root, issues)
    if topology is None:
        return issues

    if topology.get("schema_version") != "tos_mechanics_topology_v2":
        issues.append((TOPOLOGY_PATH.as_posix(), "schema_version must be tos_mechanics_topology_v2"))
    if topology.get("owner_repo") != "Tree-of-Sophia":
        issues.append((TOPOLOGY_PATH.as_posix(), "owner_repo must be Tree-of-Sophia"))
    if topology.get("root") != "mechanics/":
        issues.append((TOPOLOGY_PATH.as_posix(), "root must be mechanics/"))
    if (
        topology.get("legacy_policy")
        != "package-local-only-when-active-route-has-moved-path-or-raw-receipt-accounting"
    ):
        issues.append((TOPOLOGY_PATH.as_posix(), "legacy_policy drifted"))

    packages = topology.get("packages")
    if not isinstance(packages, list):
        issues.append((TOPOLOGY_PATH.as_posix(), "packages must be a list"))
        return issues

    seen: dict[str, dict[str, object]] = {}
    package_parts: dict[str, set[str]] = {}
    order: list[str] = []
    for entry in packages:
        if not isinstance(entry, dict):
            issues.append((TOPOLOGY_PATH.as_posix(), "each package entry must be an object"))
            continue
        slug = entry.get("slug")
        if not isinstance(slug, str) or not slug:
            issues.append((TOPOLOGY_PATH.as_posix(), "package slug must be a non-empty string"))
            continue
        if slug in seen:
            issues.append((TOPOLOGY_PATH.as_posix(), f"duplicate package slug {slug}"))
        seen[slug] = entry
        order.append(slug)
        package_parts[slug] = set(validate_package(root, issues, slug, entry))

    package_dirs = discover_package_dirs(root)
    for missing in sorted(package_dirs - set(seen)):
        issues.append((TOPOLOGY_PATH.as_posix(), f"mechanics package directory missing from topology: {missing}"))
    for extra in sorted(set(seen) - package_dirs):
        issues.append((TOPOLOGY_PATH.as_posix(), f"topology package missing mechanics directory: {extra}"))

    if len(order) != len(set(order)):
        issues.append((TOPOLOGY_PATH.as_posix(), "packages must be unique"))

    validate_context_budget(root, issues, topology)
    validate_route_map(root, issues, seen, package_parts)
    validate_documentation_references(root, issues)
    validate_moved_targets(root, issues, topology, seen, package_parts)

    return issues


def main() -> int:
    issues = run_validation(REPO_ROOT)
    if issues:
        print("Mechanics topology validation failed.", file=sys.stderr)
        for location, message in issues:
            print(f"- {location}: {message}", file=sys.stderr)
        return 1

    print("[ok] validated ToS mechanics topology")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
