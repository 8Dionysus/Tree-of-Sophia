#!/usr/bin/env python3
"""Coordinate cross-family documentation currentness and disclosure guards."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

from build_documentation_family_currentness import (
    CURRENTNESS_PATH,
    MAP_PATH,
    build_currentness,
    family_for,
    included_surface,
    load_map,
    render_currentness,
    tracked_paths,
)
import validate_agent_surface
import validate_decision_records
import validate_mechanics_topology
import validate_nested_agents
import validate_tiny_entry_route


REPO_ROOT = Path(__file__).resolve().parents[1]
Issue = tuple[str, str]
EXPECTED_FAMILY_IDS = {
    "root",
    "agents-skills",
    "agents-other",
    "github",
    "tos",
    "docs",
    "evals",
    "stats",
    "kag",
    "mechanics",
    "scripts",
    "tests",
    "manifests",
    "memo",
    "quests",
}
ROLE_KEYS = {"authored", "generated", "executable", "runtime", "tool", "receipt"}
MARKDOWN_SUFFIXES = {".md", ".txt", ".yaml", ".yml"}
COMMAND_REFERENCE_RE = re.compile(
    r"(?<![A-Za-z0-9_.-])((?:\.\./)*(?:scripts|mechanics)/[A-Za-z0-9_./-]+\.(?:py|sh))"
)
EXTERNAL_OWNER_MARKERS = (
    "aoa-kag",
    "aoa-evals",
    "aoa-memo",
    "aoa_kag_root",
    "aoa_evals_root",
    "aoa_memo_root",
    "external owner",
    "stronger owner",
)
CONTEXT_MEASURES = {"agents_route_max_inherited", "generated_summary", "sum", "max"}
EXPECTED_SURFACE_EXTENSIONS = {".md", ".txt", ".yaml", ".yml", ".json", ".py", ".sh"}
EXPECTED_CONTEXT_PROBE_IDS = {
    "root_entry",
    "inherited_agents_stacks",
    "mechanics_entry",
    "skill_discovery",
    "public_entry",
    "machine_projection_summary",
}
COMMAND_CARRIER_SPLIT_RE = re.compile(r"(?:;|&&|\|\||,|(?<=[.!?])\s+)")


def _issue(issues: list[Issue], location: str, message: str) -> None:
    issues.append((location, message))


def load_source_map(repo_root: Path) -> dict[str, Any] | None:
    try:
        _, payload = load_map(repo_root)
    except (OSError, TypeError, ValueError, json.JSONDecodeError):
        return None
    return payload


def validate_source_map(repo_root: Path, payload: dict[str, Any], issues: list[Issue]) -> None:
    required = {
        "schema_version",
        "owner_repo",
        "owner_surface",
        "schema_ref",
        "currentness_schema_ref",
        "generated_currentness",
        "builder",
        "validator",
        "atlas_method",
        "surface_rules",
        "families",
        "authority_declarations",
        "public_authored_surfaces",
        "public_forbidden_markers",
        "context_probes",
        "guard_families",
        "projection_decision",
    }
    missing = sorted(required - set(payload))
    if missing:
        _issue(issues, MAP_PATH.as_posix(), f"missing required fields: {', '.join(missing)}")
        return
    if payload.get("schema_version") != "tos_documentation_family_map_v1":
        _issue(issues, MAP_PATH.as_posix(), "unsupported schema_version")
    if payload.get("owner_repo") != "Tree-of-Sophia":
        _issue(issues, MAP_PATH.as_posix(), "owner_repo must be Tree-of-Sophia")
    for key in ("owner_surface", "schema_ref", "currentness_schema_ref", "generated_currentness", "builder", "validator"):
        value = payload.get(key)
        if not isinstance(value, str) or not value:
            _issue(issues, MAP_PATH.as_posix(), f"{key} must be a non-empty path")
        elif value.startswith("external:"):
            continue
        elif not (repo_root / value).exists():
            _issue(issues, MAP_PATH.as_posix(), f"{key} points to missing path: {value}")

    families = payload.get("families")
    if not isinstance(families, list):
        _issue(issues, MAP_PATH.as_posix(), "families must be a list")
        return
    ids: list[str] = []
    for family in families:
        if not isinstance(family, dict):
            _issue(issues, MAP_PATH.as_posix(), "each family must be an object")
            continue
        family_id = family.get("id")
        if not isinstance(family_id, str) or not family_id:
            _issue(issues, MAP_PATH.as_posix(), "family id must be a non-empty string")
            continue
        ids.append(family_id)
        for field in ("label", "match", "consumer", "load_moment", "owner", "roles", "currentness_inputs", "next_organ", "context", "public_safety", "validation_lane", "authority_keys"):
            if field not in family:
                _issue(issues, f"{MAP_PATH.as_posix()}#{family_id}", f"missing family field: {field}")
        roles = family.get("roles")
        if not isinstance(roles, dict) or set(roles) != ROLE_KEYS:
            _issue(issues, f"{MAP_PATH.as_posix()}#{family_id}", "roles must name authored/generated/executable/runtime/tool/receipt exactly")
        context = family.get("context")
        if not isinstance(context, dict) or context.get("posture") not in {"always_on", "always_on_entry_then_on_demand", "triggered_or_on_demand", "deep_on_demand", "on_demand", "tool_on_demand", "internal_port"}:
            _issue(issues, f"{MAP_PATH.as_posix()}#{family_id}", "context posture is invalid")
        elif not isinstance(context.get("max_tokens"), int) or context["max_tokens"] <= 0:
            _issue(issues, f"{MAP_PATH.as_posix()}#{family_id}", "context max_tokens must be positive")
        safety = family.get("public_safety")
        if not isinstance(safety, dict) or not isinstance(safety.get("posture"), str) or not isinstance(safety.get("forbidden"), list):
            _issue(issues, f"{MAP_PATH.as_posix()}#{family_id}", "public_safety must declare posture and forbidden markers")
        for value in family.get("currentness_inputs", []):
            if not isinstance(value, str) or not value:
                _issue(issues, f"{MAP_PATH.as_posix()}#{family_id}", "currentness_inputs must contain strings")
                continue
            if value.startswith("external:"):
                continue
            if any(token in value for token in ("*", "?")):
                if not list(repo_root.glob(value)):
                    _issue(issues, f"{MAP_PATH.as_posix()}#{family_id}", f"currentness glob has no match: {value}")
            elif not (repo_root / value).exists():
                _issue(issues, f"{MAP_PATH.as_posix()}#{family_id}", f"currentness input is missing: {value}")
    if len(ids) != len(set(ids)):
        _issue(issues, MAP_PATH.as_posix(), "family ids must be unique")
    if set(ids) != EXPECTED_FAMILY_IDS:
        _issue(issues, MAP_PATH.as_posix(), f"family ids must cover {sorted(EXPECTED_FAMILY_IDS)}")

    authority = payload.get("authority_declarations")
    if not isinstance(authority, list) or not authority:
        _issue(issues, MAP_PATH.as_posix(), "authority_declarations must be a non-empty list")
    for field in ("public_authored_surfaces", "public_forbidden_markers"):
        value = payload.get(field)
        if not isinstance(value, list) or not value or not all(isinstance(item, str) and item for item in value):
            _issue(issues, MAP_PATH.as_posix(), f"{field} must be a non-empty list of strings")
    guard_families = payload.get("guard_families")
    if not isinstance(guard_families, list) or {item.get("id") for item in guard_families if isinstance(item, dict)} != {
        "broken_markdown_route", "stale_executable_route", "authority_conflict", "generated_projection_mismatch", "missing_family_coverage", "public_safety", "context_budget"
    }:
        _issue(issues, MAP_PATH.as_posix(), "guard_families must cover all seven cross-corpus guards")
    decision = payload.get("projection_decision")
    if not isinstance(decision, dict) or decision.get("llms_txt") != "not_added":
        _issue(issues, MAP_PATH.as_posix(), "projection_decision must preserve the evidence-backed llms.txt no-add decision")
    for path_key in ("schema_ref", "builder", "validator", "generated_currentness"):
        value = payload.get(path_key)
        if isinstance(value, str) and not value.startswith("external:") and not (repo_root / value).exists():
            _issue(issues, MAP_PATH.as_posix(), f"source map route is missing: {value}")


def validate_surface_rules(surface_rules: Any, issues: list[Issue]) -> None:
    location = f"{MAP_PATH.as_posix()}#surface_rules"
    if not isinstance(surface_rules, dict):
        _issue(issues, location, "surface_rules must be an object")
        return
    extensions = surface_rules.get("include_extensions")
    if (
        not isinstance(extensions, list)
        or not extensions
        or not all(isinstance(item, str) and item.startswith(".") for item in extensions)
        or len(extensions) != len(set(extensions))
        or set(extensions) != EXPECTED_SURFACE_EXTENSIONS
    ):
        _issue(issues, location, f"include_extensions must be the explicit set {sorted(EXPECTED_SURFACE_EXTENSIONS)}")
    for field in ("exclude_paths", "exclude_prefixes"):
        values = surface_rules.get(field)
        if not isinstance(values, list) or not all(isinstance(item, str) and item for item in values):
            _issue(issues, location, f"{field} must be a list of non-empty strings")
            continue
        for item in values:
            path = Path(item)
            if path.is_absolute() or ".." in path.parts or "//" in item:
                _issue(issues, location, f"{field} contains an unsafe path: {item}")
            if field == "exclude_prefixes" and not item.endswith("/"):
                _issue(issues, location, f"exclude prefix must end with '/': {item}")


def validate_context_probe_configuration(probes: Any, issues: list[Issue]) -> None:
    location = f"{MAP_PATH.as_posix()}#context_probes"
    if not isinstance(probes, list) or not probes:
        _issue(issues, location, "context_probes must be a non-empty list")
        return
    ids: list[str] = []
    for index, probe in enumerate(probes):
        if not isinstance(probe, dict):
            _issue(issues, location, f"probe {index} must be an object")
            continue
        probe_id = probe.get("id")
        if not isinstance(probe_id, str) or not probe_id:
            _issue(issues, location, f"probe {index} needs a non-empty id")
        else:
            ids.append(probe_id)
        surfaces = probe.get("surfaces")
        if not isinstance(surfaces, list) or not surfaces or not all(isinstance(item, str) and item for item in surfaces):
            _issue(issues, f"{location}#{probe_id or index}", "surfaces must be a non-empty list of strings")
        if probe.get("measure") not in CONTEXT_MEASURES:
            _issue(issues, f"{location}#{probe_id or index}", f"unsupported context measure: {probe.get('measure')!r}")
    if len(ids) != len(set(ids)):
        _issue(issues, location, "context probe ids must be unique")
    missing = sorted(EXPECTED_CONTEXT_PROBE_IDS - set(ids))
    if missing:
        _issue(issues, location, f"missing required context probes: {', '.join(missing)}")


def validate_family_match_rules(families: Any, issues: list[Issue]) -> None:
    location = f"{MAP_PATH.as_posix()}#families"
    if not isinstance(families, list):
        return
    prefix_rules: list[tuple[str, str, dict[str, Any]]] = []
    root_count = 0
    for family in families:
        if not isinstance(family, dict):
            continue
        family_id = family.get("id", "<unknown>")
        match = family.get("match")
        if not isinstance(match, dict):
            _issue(issues, f"{location}#{family_id}", "match must be an object")
            continue
        if match.get("kind") == "root_files":
            root_count += 1
            continue
        prefix = match.get("prefix")
        if not isinstance(prefix, str) or not prefix or not prefix.endswith("/") or prefix.startswith("/") or ".." in Path(prefix).parts or "//" in prefix:
            _issue(issues, f"{location}#{family_id}", "prefix must be a non-empty relative directory boundary")
            continue
        exclusions = match.get("exclude_prefixes", [])
        if not isinstance(exclusions, list) or not all(isinstance(item, str) and item.endswith("/") and not item.startswith("/") and ".." not in Path(item).parts for item in exclusions):
            _issue(issues, f"{location}#{family_id}", "exclude_prefixes must contain safe directory boundaries")
        prefix_rules.append((prefix, str(family_id), match))
    if root_count != 1:
        _issue(issues, location, "families must declare exactly one root_files match")
    for index, (left, left_id, left_match) in enumerate(prefix_rules):
        for right, right_id, right_match in prefix_rules[index + 1 :]:
            if left == right:
                _issue(issues, location, f"families have duplicate match prefix: {left_id}, {right_id}")
                continue
            if left.startswith(right):
                broad, broad_id, broad_match, narrow = right, right_id, right_match, left
            elif right.startswith(left):
                broad, broad_id, broad_match, narrow = left, left_id, left_match, right
            else:
                continue
            exclusions = broad_match.get("exclude_prefixes", [])
            if not any(narrow.startswith(exclusion) for exclusion in exclusions):
                _issue(issues, location, f"overlapping family prefixes require an explicit exclusion: {broad_id} and {left_id if broad_id == right_id else right_id}")


def validate_authority_declarations(payload: list[dict[str, Any]], issues: list[Issue], location: str = "authority_declarations") -> None:
    seen: dict[str, tuple[str, str]] = {}
    for index, declaration in enumerate(payload):
        if not isinstance(declaration, dict):
            _issue(issues, location, f"declaration {index} must be an object")
            continue
        claim_id = declaration.get("id")
        owner = declaration.get("owner")
        strength = declaration.get("strength")
        if not all(isinstance(value, str) and value for value in (claim_id, owner, strength)):
            _issue(issues, location, f"declaration {index} needs id, owner, and strength")
            continue
        family_id = declaration.get("family_id")
        family_owner = declaration.get("family_owner")
        if (family_id is None) != (family_owner is None) or (
            family_id is not None
            and (not isinstance(family_id, str) or not family_id or not isinstance(family_owner, str) or not family_owner)
        ):
            _issue(issues, location, f"declaration {claim_id} needs a complete family_id/family_owner binding")
        current = (owner, strength)
        if claim_id in seen:
            if seen[claim_id] == current:
                _issue(issues, location, f"duplicate authority declaration: {claim_id}")
            else:
                _issue(issues, location, f"conflicting authority declaration: {claim_id}")
        else:
            seen[claim_id] = current


def validate_family_authority_claims(
    families: list[dict[str, Any]],
    declarations: list[dict[str, Any]],
    issues: list[Issue],
) -> None:
    declarations_by_id = {
        declaration.get("id"): declaration
        for declaration in declarations
        if isinstance(declaration, dict) and isinstance(declaration.get("id"), str)
    }
    for family in families:
        if not isinstance(family, dict):
            continue
        family_id = family.get("id", "<unknown>")
        family_owner = family.get("owner")
        authority_keys = family.get("authority_keys")
        if not isinstance(authority_keys, list):
            _issue(issues, f"{MAP_PATH.as_posix()}#{family_id}", "authority_keys must be a list")
            continue
        for authority_key in authority_keys:
            declaration = declarations_by_id.get(authority_key)
            location = f"{MAP_PATH.as_posix()}#{family_id}"
            if declaration is None:
                _issue(issues, location, f"authority key has no declaration: {authority_key}")
                continue
            if declaration.get("family_id") != family_id:
                _issue(issues, location, f"authority key {authority_key} is bound to family {declaration.get('family_id')!r}, not {family_id!r}")
            if declaration.get("family_owner") != family_owner:
                _issue(issues, location, f"family owner conflicts with declaration {authority_key}")


def _tracked_markdown_paths(repo_root: Path) -> list[Path]:
    return [
        repo_root / path
        for path in tracked_paths(repo_root)
        if path.suffix == ".md" and (repo_root / path).is_file()
    ]


def validate_markdown_routes(repo_root: Path, issues: list[Issue]) -> None:
    for path in _tracked_markdown_paths(repo_root):
        relative = path.relative_to(repo_root).as_posix()
        text = path.read_text(encoding="utf-8")
        for reference in validate_mechanics_topology.markdown_destinations(text, rendered_only=True):
            if reference.startswith(("http:", "https:", "mailto:")):
                continue
            resolved = validate_mechanics_topology.resolve_doc_reference(repo_root, path, reference)
            if resolved is None:
                _issue(issues, relative, f"broken local documentation route: {reference}")
                continue
            target, fragment = validate_mechanics_topology.reference_parts(reference)
            if fragment and not validate_mechanics_topology.document_has_fragment(resolved, fragment):
                _issue(issues, relative, f"broken local documentation fragment: {reference}")


def _script_inventory_paths(repo_root: Path) -> set[str]:
    path = repo_root / "docs/validation/script_inventory.json"
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return set()
    return {
        entry["path"]
        for entry in payload.get("script_surfaces", [])
        if isinstance(entry, dict) and isinstance(entry.get("path"), str)
    }


def _external_reference_allowed(line: str, start: int, end: int) -> bool:
    """Allow only a command in a single-marker, command-local carrier."""
    offset = 0
    for carrier in COMMAND_CARRIER_SPLIT_RE.split(line):
        carrier_start = line.find(carrier, offset)
        offset = carrier_start + len(carrier)
        if start < 0 or carrier_start < 0:
            continue
        carrier_end = carrier_start + len(carrier)
        if not carrier_start <= start < carrier_end:
            continue
        references = list(COMMAND_REFERENCE_RE.finditer(carrier))
        if len(references) != 1:
            return False
        marker_text = carrier.lower()
        return any(marker in marker_text for marker in EXTERNAL_OWNER_MARKERS)
    return False


def validate_executable_routes(repo_root: Path, issues: list[Issue]) -> None:
    inventory = _script_inventory_paths(repo_root)
    source_paths = [
        repo_root / path
        for path in tracked_paths(repo_root)
        if path.suffix in MARKDOWN_SUFFIXES
    ]
    for path in source_paths:
        if not path.is_file():
            continue
        relative = path.relative_to(repo_root).as_posix()
        # These are deliberately carrier or historical surfaces. Their route
        # names may point to a stronger owner, an archived experiment, or a
        # descriptive inventory entry rather than a local active executable.
        if (
            path.name == "CHANGELOG.md"
            or relative.startswith(("ToS/research-packets/", "ToS/review-ledger/"))
            or relative.startswith("kag/receipts/")
            or relative in {"evals/AGENTS.md", "memo/AGENTS.md", "kag/AGENTS.md"}
        ):
            continue
        text = path.read_text(encoding="utf-8")
        for line in text.splitlines():
            for match in COMMAND_REFERENCE_RE.finditer(line):
                reference = match.group(1)
                resolved = validate_mechanics_topology.resolve_doc_reference(repo_root, path, reference)
                if resolved is None or not resolved.is_file():
                    if _external_reference_allowed(line, match.start(1), match.end(1)):
                        continue
                    _issue(issues, relative, f"stale executable reference: {reference}")
                    continue
                resolved_relative = resolved.relative_to(repo_root).as_posix()
                if (resolved_relative.startswith("scripts/") or "/scripts/" in resolved_relative) and resolved_relative not in inventory:
                    _issue(issues, relative, f"executable reference is absent from script inventory: {resolved_relative}")
    cards = validate_nested_agents.discover_route_cards(repo_root)
    validate_nested_agents.validate_local_script_references(repo_root, cards, issues)


def validate_family_coverage(repo_root: Path, payload: dict[str, Any], issues: list[Issue]) -> None:
    try:
        current = build_currentness(repo_root)
    except (OSError, subprocess.SubprocessError, TypeError, ValueError) as exc:
        _issue(issues, MAP_PATH.as_posix(), f"cannot build family coverage: {exc}")
        return
    coverage = current.get("coverage", {})
    if coverage.get("unhandled_family_count") != 0:
        _issue(issues, MAP_PATH.as_posix(), f"unhandled family count is {coverage.get('unhandled_family_count')}: {coverage.get('unhandled_paths', [])[:5]}")
    expected = {family["id"] for family in payload.get("families", []) if isinstance(family, dict)}
    actual = {summary.get("family_id") for summary in current.get("family_summaries", [])}
    if expected != actual:
        _issue(issues, MAP_PATH.as_posix(), f"generated family summaries differ from authored families: {sorted(expected ^ actual)}")


def validate_generated_projection(repo_root: Path, issues: list[Issue]) -> None:
    path = repo_root / CURRENTNESS_PATH
    try:
        expected = render_currentness(build_currentness(repo_root))
        actual = path.read_text(encoding="utf-8")
    except (OSError, subprocess.SubprocessError, TypeError, ValueError) as exc:
        _issue(issues, CURRENTNESS_PATH.as_posix(), f"cannot load generated projection: {exc}")
        return
    if actual != expected:
        _issue(issues, CURRENTNESS_PATH.as_posix(), "generated documentation family projection is stale")


def _context_tokens(text: str) -> int:
    return len(text.split())


def validate_context_probes(repo_root: Path, payload: dict[str, Any], issues: list[Issue]) -> dict[str, int]:
    measured: dict[str, int] = {}
    generated: dict[str, Any] | None = None
    for probe in payload.get("context_probes", []):
        if not isinstance(probe, dict):
            _issue(issues, "context_probes", "probe must be an object")
            continue
        probe_id = probe.get("id", "<unknown>")
        surfaces = probe.get("surfaces", [])
        measure = probe.get("measure")
        if not isinstance(surfaces, list) or not surfaces or not all(isinstance(item, str) and item for item in surfaces):
            _issue(issues, f"context_probes#{probe_id}", "surfaces must be a non-empty list of strings")
            continue
        if measure not in CONTEXT_MEASURES:
            _issue(issues, f"context_probes#{probe_id}", f"unsupported context measure: {measure!r}")
            continue
        try:
            if measure == "agents_route_max_inherited":
                route_payload = json.loads((repo_root / surfaces[0]).read_text(encoding="utf-8"))
                values = [
                    route.get("inherited_context_tokens")
                    for route in route_payload.get("task_routes", [])
                    if isinstance(route, dict) and isinstance(route.get("inherited_context_tokens"), int)
                ]
                value = max(values, default=0)
            elif measure == "generated_summary":
                if generated is None:
                    generated = json.loads((repo_root / CURRENTNESS_PATH).read_text(encoding="utf-8"))
                value = _context_tokens(json.dumps(generated.get("context_summary", {}), ensure_ascii=False, sort_keys=True))
            else:
                values = [_context_tokens((repo_root / surface).read_text(encoding="utf-8")) for surface in surfaces]
                value = sum(values) if measure == "sum" else max(values, default=0)
        except (IndexError, OSError, TypeError, ValueError, json.JSONDecodeError) as exc:
            _issue(issues, f"context_probes#{probe_id}", f"cannot measure probe: {exc}")
            continue
        measured[str(probe_id)] = value
        maximum = probe.get("max_tokens")
        if not isinstance(maximum, int) or maximum <= 0:
            _issue(issues, f"context_probes#{probe_id}", "max_tokens must be positive")
        elif value > maximum:
            _issue(issues, f"context_probes#{probe_id}", f"context budget exceeded: {value}>{maximum}")
    return measured


def validate_public_safety(repo_root: Path, payload: dict[str, Any], issues: list[Issue]) -> None:
    markers = payload.get("public_forbidden_markers", [])
    for relative in payload.get("public_authored_surfaces", []):
        path = repo_root / relative
        if not path.is_file():
            _issue(issues, relative, "public authored surface is missing")
            continue
        text = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker in text:
                _issue(issues, relative, f"contains forbidden public-safety marker {marker!r}")


def _run_existing_command(repo_root: Path, script: str) -> tuple[str, str] | None:
    completed = subprocess.run(
        [sys.executable, script],
        cwd=repo_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode == 0:
        return None
    detail = (completed.stdout + completed.stderr).strip().splitlines()
    return script, " ".join(detail[-4:]) or f"exit code {completed.returncode}"


def validate_existing_owner_contracts(repo_root: Path, issues: list[Issue]) -> None:
    for location, message in validate_nested_agents.run_validation(repo_root):
        _issue(issues, location, f"AGENTS-route validator: {message}")
    for location, message in validate_mechanics_topology.run_validation(repo_root):
        _issue(issues, location, f"mechanics topology validator: {message}")
    for location, message in validate_decision_records.validate_decision_records(repo_root):
        _issue(issues, location, f"decision validator: {message}")
    for location, message in validate_tiny_entry_route.run_validation(repo_root):
        _issue(issues, location, f"public-entry validator: {message}")
    for location, message in validate_agent_surface.validate_manifest(repo_root, fetch_missing_budget_base=False):
        _issue(issues, location, f"agent-surface validator: {message}")
    for script in ("scripts/validate_root_entry_map.py", "scripts/validate_local_kag_provider.py"):
        result = _run_existing_command(repo_root, script)
        if result is not None:
            location, message = result
            _issue(issues, location, f"owner validator: {message}")


def run_validation(repo_root: Path = REPO_ROOT, *, reuse_existing: bool = True) -> list[Issue]:
    issues: list[Issue] = []
    payload = load_source_map(repo_root)
    if payload is None:
        return [(MAP_PATH.as_posix(), "source family map is missing or invalid JSON")]
    validate_source_map(repo_root, payload, issues)
    validate_surface_rules(payload.get("surface_rules"), issues)
    validate_context_probe_configuration(payload.get("context_probes"), issues)
    validate_family_match_rules(payload.get("families"), issues)
    validate_authority_declarations(payload.get("authority_declarations", []), issues)
    validate_family_authority_claims(
        payload.get("families", []),
        payload.get("authority_declarations", []),
        issues,
    )
    validate_markdown_routes(repo_root, issues)
    validate_executable_routes(repo_root, issues)
    validate_family_coverage(repo_root, payload, issues)
    validate_generated_projection(repo_root, issues)
    validate_public_safety(repo_root, payload, issues)
    validate_context_probes(repo_root, payload, issues)
    if reuse_existing:
        validate_existing_owner_contracts(repo_root, issues)
    return issues


def main() -> int:
    issues = run_validation(REPO_ROOT)
    if issues:
        print("Cross-corpus documentation validation failed.")
        for location, message in issues:
            print(f"- {location}: {message}")
        return 1
    print("[ok] validated cross-corpus documentation currentness and context guards")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
