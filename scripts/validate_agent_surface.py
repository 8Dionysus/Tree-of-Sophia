#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

from build_agent_surface_currentness import (
    COMPANION_FAMILIES,
    CURRENTNESS_PATH,
    MANIFEST_PATH,
    REPO_ROOT,
    SKILLS_ROOT,
    build_currentness,
    parse_skill,
    rendered_currentness,
)


Issue = tuple[str, str]
EXPECTED_SKILLS = {
    "aoa-adr-write",
    "aoa-approval-gate-check",
    "aoa-automation-opportunity-scan",
    "aoa-bounded-context-map",
    "aoa-change-protocol",
    "aoa-checkpoint-closeout-bridge",
    "aoa-commit-growth-seam",
    "aoa-contract-test",
    "aoa-core-logic-boundary",
    "aoa-dry-run-first",
    "aoa-invariant-coverage-audit",
    "aoa-local-stack-bringup",
    "aoa-port-adapter-refactor",
    "aoa-property-invariants",
    "aoa-quest-harvest",
    "aoa-safe-infra-change",
    "aoa-sanitized-share",
    "aoa-session-donor-harvest",
    "aoa-session-progression-lift",
    "aoa-session-route-forks",
    "aoa-session-self-diagnose",
    "aoa-session-self-repair",
    "aoa-source-of-truth-check",
    "aoa-summon",
    "aoa-tdd-slice",
}
EXPECTED_PORTS = {"eval_port", "stats_port", "kag_provider", "memo_port"}
EXPECTED_PROBES = {
    "source_authority",
    "bounded_context_mapping",
    "repo_local_change",
    "session_diagnosis_repair",
    "approval_or_dry_run",
    "eval_intake",
    "owner_local_stats",
    "kag_currentness",
    "memo_candidate_routing",
}
PATH_REFERENCE_RE = re.compile(r"\b(?:references|examples|checks|scripts|assets)/[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*\.[A-Za-z0-9_-]+\b")
PUBLIC_FORBIDDEN = (
    "/home/",
    "/srv/",
    "/tmp/",
    "BEGIN PRIVATE KEY",
    "OPENAI_API_KEY",
    "AWS_SECRET_ACCESS_KEY",
    "file://",
)


def activation_policy_issues(
    invocation_mode: str | None,
    implicit_policy: str | None,
    allow_implicit: bool | None,
    skill_id: str | None = None,
) -> list[str]:
    expected: tuple[str, bool] | None = {
        "explicit-only": ("manual", False),
        "explicit-preferred": ("invoke", True),
    }.get(invocation_mode or "")
    if skill_id == "aoa-checkpoint-closeout-bridge" and invocation_mode == "explicit-preferred":
        expected = ("suggest", False)
    if expected is None:
        return [f"unknown aoa_invocation_mode {invocation_mode!r}"]
    expected_policy, expected_allow = expected
    issues = []
    if implicit_policy != expected_policy:
        issues.append(f"implicit_activation_policy={implicit_policy!r}, expected {expected_policy!r}")
    if allow_implicit != expected_allow:
        issues.append(f"allow_implicit_invocation={allow_implicit!r}, expected {expected_allow!r}")
    return issues


def public_safety_issues(root: Path = REPO_ROOT) -> list[Issue]:
    issues: list[Issue] = []
    for relative in (".agents/README.md", MANIFEST_PATH.as_posix()):
        path = root / relative
        if not path.is_file():
            issues.append((relative, "public authored surface is missing"))
            continue
        text = path.read_text(encoding="utf-8")
        for forbidden in PUBLIC_FORBIDDEN:
            if forbidden in text:
                issues.append((relative, f"contains forbidden public-safety marker {forbidden!r}"))
    return issues


def generated_family_issues(root: Path, port: dict[str, Any]) -> list[Issue]:
    """Check the KAG generated carrier without coupling it into currentness."""
    issues: list[Issue] = []
    family = port.get("generated_family")
    if not isinstance(family, dict):
        return [("kag_provider", "generated_family carrier declaration is required")]
    manifest_text = family.get("manifest")
    shards_text = family.get("shards")
    receipt_root_text = family.get("receipt_root")
    for label, relative in (
        ("manifest", manifest_text),
        ("shards", shards_text),
        ("receipt_root", receipt_root_text),
    ):
        if not isinstance(relative, str) or not relative:
            issues.append(("kag_provider", f"generated_family.{label} is required"))
            continue
        path = root / relative
        if label == "manifest" and not path.is_file():
            issues.append((relative, "generated KAG family manifest is missing"))
        if label != "manifest" and not path.is_dir():
            issues.append((relative, f"generated KAG family {label} route is missing"))
    builder = family.get("builder")
    validator = family.get("validator")
    if not isinstance(builder, str) or not builder.startswith("aoa-kag:"):
        issues.append(("kag_provider", "generated_family.builder must keep the aoa-kag owner handle"))
    if not isinstance(validator, str) or not validator.startswith("aoa-kag:"):
        issues.append(("kag_provider", "generated_family.validator must keep the aoa-kag owner handle"))
    if not isinstance(manifest_text, str) or not isinstance(receipt_root_text, str):
        return issues
    manifest_path = root / manifest_text
    if not manifest_path.is_file():
        return issues
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        digest = manifest["family_identity"]["content_digest"]
    except (OSError, json.JSONDecodeError, KeyError, TypeError):
        issues.append((manifest_text, "generated KAG family manifest lacks family_identity.content_digest"))
        return issues
    if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
        issues.append((manifest_text, "generated KAG family digest must be 64 lowercase hex characters"))
        return issues
    receipt_path = root / receipt_root_text / f"{digest}.json"
    if not receipt_path.is_file():
        issues.append((receipt_path.relative_to(root).as_posix(), "matching generated KAG budget receipt is missing"))
        return issues
    try:
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        issues.append((receipt_path.relative_to(root).as_posix(), "matching generated KAG budget receipt is invalid"))
        return issues
    if receipt.get("head_family_digest") != digest:
        issues.append((receipt_path.relative_to(root).as_posix(), "budget receipt is not bound to the current family digest"))
    return issues


def currentness_parity_issues(root: Path = REPO_ROOT) -> list[Issue]:
    path = root / CURRENTNESS_PATH
    if not path.is_file():
        return [(CURRENTNESS_PATH.as_posix(), "generated currentness is missing")]
    if path.read_bytes() != rendered_currentness(root):
        return [(CURRENTNESS_PATH.as_posix(), "generated currentness is stale")]
    return []


def _check_local_reference_routes(root: Path, package_path: Path) -> list[Issue]:
    issues: list[Issue] = []
    for relative_file in ("SKILL.md", "agents/openai.yaml"):
        path = package_path / relative_file
        text = path.read_text(encoding="utf-8")
        for match_object in PATH_REFERENCE_RE.finditer(text):
            match = match_object.group(0)
            if "repo:" in text[max(0, match_object.start() - 20):match_object.start()]:
                continue
            target = package_path / match
            if not target.is_file():
                issues.append((path.relative_to(root).as_posix(), f"broken local companion route {match}"))
        for match in re.findall(r"\./(assets/[A-Za-z0-9_.-]+)", text):
            target = package_path / match
            if not target.is_file():
                issues.append((path.relative_to(root).as_posix(), f"broken asset route {match}"))
    return issues


def validate_manifest(root: Path = REPO_ROOT) -> list[Issue]:
    issues: list[Issue] = []
    manifest_path = root / MANIFEST_PATH
    try:
        import json
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return [(MANIFEST_PATH.as_posix(), "missing agent-surface manifest")]
    except Exception as exc:
        return [(MANIFEST_PATH.as_posix(), f"invalid JSON: {exc}")]

    if not isinstance(manifest, dict):
        return [(MANIFEST_PATH.as_posix(), "manifest root must be an object")]
    required = {
        "schema_version": "tos_agent_tool_owner_port_documentation_v1",
        "owner_repo": "Tree-of-Sophia",
        "owner_surface": ".agents/AGENTS.md",
        "human_entrypoint": ".agents/README.md",
        "generated_currentness": CURRENTNESS_PATH.as_posix(),
        "builder": "scripts/build_agent_surface_currentness.py",
        "validator": "scripts/validate_agent_surface.py",
    }
    for key, expected in required.items():
        if manifest.get(key) != expected:
            issues.append((MANIFEST_PATH.as_posix(), f"{key} must be {expected!r}"))
    for key in ("owner_surface", "human_entrypoint", "builder", "validator"):
        value = manifest.get(key)
        if isinstance(value, str) and not (root / value).is_file():
            issues.append((value, "declared route is missing"))

    budget = manifest.get("context_budget")
    if not isinstance(budget, dict):
        issues.append((MANIFEST_PATH.as_posix(), "context_budget must be an object"))
        budget = {}
    for key in (
        "discovery_description_max_words",
        "triggered_body_max_words",
        "mandatory_task_probe_depth_max",
    ):
        if not isinstance(budget.get(key), int) or budget[key] <= 0:
            issues.append((MANIFEST_PATH.as_posix(), f"context_budget.{key} must be positive"))

    packages = manifest.get("skills")
    package_by_id: dict[str, dict[str, Any]] = {}
    if not isinstance(packages, list):
        issues.append((MANIFEST_PATH.as_posix(), "skills must be a list"))
        packages = []
    for package in packages:
        if not isinstance(package, dict):
            issues.append((MANIFEST_PATH.as_posix(), "each skill record must be an object"))
            continue
        skill_id = package.get("id")
        if not isinstance(skill_id, str) or not skill_id:
            issues.append((MANIFEST_PATH.as_posix(), "skill record needs a non-empty id"))
            continue
        if skill_id in package_by_id:
            issues.append((MANIFEST_PATH.as_posix(), f"duplicate skill record {skill_id}"))
        package_by_id[skill_id] = package
        if not isinstance(package.get("family"), str) or not package["family"]:
            issues.append((MANIFEST_PATH.as_posix(), f"{skill_id} needs a family"))
        entrypoint = package.get("entrypoint")
        if not isinstance(entrypoint, str):
            issues.append((MANIFEST_PATH.as_posix(), f"{skill_id} needs an entrypoint"))
            continue
        entrypoint_path = root / entrypoint
        if not entrypoint_path.is_file():
            issues.append((entrypoint, f"{skill_id} entrypoint is missing"))
            continue
        package_path = entrypoint_path.parent
        try:
            current = parse_skill(entrypoint_path)
        except Exception as exc:
            issues.append((entrypoint, str(exc)))
            continue
        if current["id"] != skill_id:
            issues.append((entrypoint, f"frontmatter name {current['id']!r} does not match {skill_id!r}"))
        frontmatter = current["frontmatter"]
        if frontmatter["aoa_source_repo"] != "8Dionysus/aoa-skills":
            issues.append((entrypoint, "aoa_source_repo must be 8Dionysus/aoa-skills"))
        source_path = frontmatter["aoa_source_skill_path"]
        if not isinstance(source_path, str) or not source_path.startswith("skills/"):
            issues.append((entrypoint, "aoa_source_skill_path must identify an aoa-skills source path"))
        if current["frontmatter"]["description_words"] > budget.get("discovery_description_max_words", 0):
            issues.append((entrypoint, "discovery description exceeds context budget"))
        if current["triggered_body_words"] > budget.get("triggered_body_max_words", 0):
            issues.append((entrypoint, "triggered SKILL.md body exceeds context budget"))
        issues.extend(
            (location, f"{skill_id}: {message}")
            for location, message in _check_local_reference_routes(root, package_path)
        )
        issues.extend(
            (entrypoint, f"{skill_id}: {message}")
            for message in activation_policy_issues(
                frontmatter["aoa_invocation_mode"],
                current["activation"]["implicit_activation_policy"],
                current["activation"]["allow_implicit_invocation"],
                skill_id,
            )
        )

    discovered = {
        path.parent.name
        for path in sorted((root / SKILLS_ROOT).glob("*/SKILL.md"))
    }
    declared = set(package_by_id)
    if discovered != EXPECTED_SKILLS:
        issues.append((SKILLS_ROOT.as_posix(), f"discovered skills differ from expected 25: {sorted(discovered)}"))
    if declared != EXPECTED_SKILLS:
        issues.append((MANIFEST_PATH.as_posix(), f"declared skills differ from expected 25: {sorted(declared)}"))
    if discovered != declared:
        issues.append((MANIFEST_PATH.as_posix(), "manifest skill ids do not match local package discovery"))

    families = manifest.get("skill_families")
    if not isinstance(families, dict):
        issues.append((MANIFEST_PATH.as_posix(), "skill_families must be an object"))
        families = {}
    family_members: set[str] = set()
    for family_id, family in families.items():
        if not isinstance(family, dict):
            issues.append((MANIFEST_PATH.as_posix(), f"{family_id} family must be an object"))
            continue
        members = family.get("skills")
        if not isinstance(members, list) or not members:
            issues.append((MANIFEST_PATH.as_posix(), f"{family_id}.skills must be non-empty"))
            continue
        family_members.update(members)
        for required_field in ("consumer", "load_moment", "canonical_owner", "freshness", "next_organ", "negative_controls"):
            if not family.get(required_field):
                issues.append((MANIFEST_PATH.as_posix(), f"{family_id}.{required_field} is required"))
        for member in members:
            if member not in declared:
                issues.append((MANIFEST_PATH.as_posix(), f"{family_id} names unknown skill {member}"))
            elif package_by_id[member].get("family") != family_id:
                issues.append((MANIFEST_PATH.as_posix(), f"{member} family assignment disagrees"))
    if family_members != declared:
        issues.append((MANIFEST_PATH.as_posix(), "skill families do not cover exactly the declared skills"))

    ports = manifest.get("owner_ports")
    if not isinstance(ports, dict):
        issues.append((MANIFEST_PATH.as_posix(), "owner_ports must be an object"))
        ports = {}
    if set(ports) != EXPECTED_PORTS:
        issues.append((MANIFEST_PATH.as_posix(), f"owner ports must be {sorted(EXPECTED_PORTS)}"))
    for port_id, port in ports.items():
        if not isinstance(port, dict):
            issues.append((MANIFEST_PATH.as_posix(), f"{port_id} must be an object"))
            continue
        for field in ("local_owner", "manifest", "consumer", "load_moment", "canonical_owner", "freshness", "next_organ", "currentness_inputs", "carrier_classes"):
            if not port.get(field):
                issues.append((MANIFEST_PATH.as_posix(), f"{port_id}.{field} is required"))
        for route in (port.get("local_owner"), port.get("manifest")):
            if isinstance(route, str) and not (root / route).is_file():
                issues.append((route, f"{port_id} route is missing"))
        for relative in port.get("currentness_inputs", []):
            if not isinstance(relative, str) or not (root / relative).is_file():
                issues.append((str(relative), f"{port_id} currentness input is missing"))
        if port_id == "kag_provider":
            issues.extend(generated_family_issues(root, port))

    probes = manifest.get("task_probes")
    if not isinstance(probes, list):
        issues.append((MANIFEST_PATH.as_posix(), "task_probes must be a list"))
        probes = []
    probe_ids = set()
    maximum_depth = budget.get("mandatory_task_probe_depth_max", 0)
    for probe in probes:
        if not isinstance(probe, dict):
            issues.append((MANIFEST_PATH.as_posix(), "each task probe must be an object"))
            continue
        probe_id = probe.get("id")
        if not isinstance(probe_id, str) or not probe_id:
            issues.append((MANIFEST_PATH.as_posix(), "task probe needs an id"))
            continue
        probe_ids.add(probe_id)
        if not probe.get("first_route") or not probe.get("required_chain"):
            issues.append((MANIFEST_PATH.as_posix(), f"{probe_id} needs first_route and required_chain"))
        if not isinstance(probe.get("negative_controls"), list) or not probe["negative_controls"]:
            issues.append((MANIFEST_PATH.as_posix(), f"{probe_id} needs negative controls"))
        depth = probe.get("mandatory_reading_depth")
        if not isinstance(depth, int) or depth <= 0 or depth > maximum_depth:
            issues.append((MANIFEST_PATH.as_posix(), f"{probe_id} exceeds mandatory reading depth budget"))
        for skill_id in probe.get("skill_ids", []):
            if skill_id not in declared:
                issues.append((MANIFEST_PATH.as_posix(), f"{probe_id} names unknown skill {skill_id}"))
        port_id = probe.get("port_id")
        if port_id is not None and port_id not in ports:
            issues.append((MANIFEST_PATH.as_posix(), f"{probe_id} names unknown port {port_id}"))
    if probe_ids != EXPECTED_PROBES:
        issues.append((MANIFEST_PATH.as_posix(), f"task probes must be {sorted(EXPECTED_PROBES)}"))

    inventory = manifest.get("package_inventory")
    if not isinstance(inventory, dict):
        issues.append((MANIFEST_PATH.as_posix(), "package_inventory must be an object"))

    try:
        issues.extend(currentness_parity_issues(root))
        current = build_currentness(root)
        if isinstance(inventory, dict) and current["package_inventory"] != inventory:
            issues.append((MANIFEST_PATH.as_posix(), "package_inventory counts disagree with discovered packages"))
        current_ports = {record["id"] for record in current["owner_ports"]}
        if current_ports != set(ports):
            issues.append((CURRENTNESS_PATH.as_posix(), "owner-port currentness ids disagree with manifest"))
    except Exception as exc:
        issues.append((CURRENTNESS_PATH.as_posix(), f"cannot build currentness: {exc}"))

    issues.extend(public_safety_issues(root))
    return issues


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate the ToS model-facing skill and owner-port surface.")
    parser.add_argument("--check", action="store_true", help="validate the authored and generated surface")
    args = parser.parse_args(argv)
    if not args.check:
        parser.print_help()
        return 0
    issues = validate_manifest(REPO_ROOT)
    if issues:
        print("Agent surface validation failed.", file=sys.stderr)
        for location, message in issues:
            print(f"- {location}: {message}", file=sys.stderr)
        return 1
    print("[ok] validated ToS agent surface")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
