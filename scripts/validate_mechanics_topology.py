#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import TypeAlias


REPO_ROOT = Path(__file__).resolve().parents[1]
TOPOLOGY_PATH = Path("mechanics/topology.json")

Issue: TypeAlias = tuple[str, str]

EXPECTED_PACKAGES: dict[str, dict[str, object]] = {
    "agon": {
        "class": "head-fed/local",
        "status": "active",
        "parts": ("threshold-intake", "canon-restraint", "threshold-registry", "landing-handoff"),
        "legacy_required": True,
    },
    "antifragility": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("source-resilience",),
        "legacy_required": False,
    },
    "audit": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("review-ledger-route",),
        "legacy_required": True,
    },
    "boundary-bridge": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("derived-kag-seam",),
        "legacy_required": True,
    },
    "canon-formation": {
        "class": "local",
        "status": "planted",
        "parts": ("promotion-gate",),
        "legacy_required": False,
    },
    "checkpoint": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("review-return",),
        "legacy_required": False,
    },
    "distillation": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("source-compost",),
        "legacy_required": True,
    },
    "experience": {
        "class": "head-fed/local",
        "status": "active",
        "parts": (
            "candidate-review",
            "adoption-boundary",
            "governance-boundary",
            "installation-boundary",
            "service-office-boundary",
            "pattern-review",
            "write-guards",
        ),
        "legacy_required": True,
    },
    "growth-cycle": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("branch-growth-cycle",),
        "legacy_required": True,
    },
    "method-growth": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("node-method-spine",),
        "legacy_required": False,
    },
    "questbook": {
        "class": "head-fed/local",
        "status": "active",
        "parts": ("obligation-boundary", "dispatch-contracts"),
        "legacy_required": True,
    },
    "recurrence": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("calibration-return",),
        "legacy_required": False,
    },
    "relation-weaving": {
        "class": "local",
        "status": "planted",
        "parts": ("graph-promotion",),
        "legacy_required": False,
    },
    "release-support": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("source-release-gate",),
        "legacy_required": False,
    },
    "rpg": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("reading-progression",),
        "legacy_required": False,
    },
    "source-witnessing": {
        "class": "local",
        "status": "planted",
        "parts": ("witness-route",),
        "legacy_required": True,
    },
}

EXPECTED_ORDER = tuple(EXPECTED_PACKAGES)

MOVED_TARGETS: dict[str, dict[str, tuple[tuple[str, str], ...]]] = {
    "agon": {
        "threshold-intake": (
            (
                "ToS/doctrine/TOS_AGON_THRESHOLD_INTAKE.md",
                "mechanics/agon/parts/threshold-intake/docs/TOS_AGON_THRESHOLD_INTAKE.md",
            ),
            (
                "ToS/contracts/tos-agon-threshold-intake.schema.json",
                "mechanics/agon/parts/threshold-intake/schemas/tos-agon-threshold-intake.schema.json",
            ),
        ),
        "canon-restraint": (
            (
                "ToS/doctrine/TOS_REJECTION_AND_BRANCHING.md",
                "mechanics/agon/parts/canon-restraint/docs/TOS_REJECTION_AND_BRANCHING.md",
            ),
            (
                "ToS/doctrine/TOS_CANON_RESTRAINT.md",
                "mechanics/agon/parts/canon-restraint/docs/TOS_CANON_RESTRAINT.md",
            ),
            (
                "ToS/doctrine/TOS_THRESHOLD_REVIEW_BOUNDARY.md",
                "mechanics/agon/parts/canon-restraint/docs/TOS_THRESHOLD_REVIEW_BOUNDARY.md",
            ),
        ),
        "threshold-registry": (
            (
                "ToS/contracts/tos-agon-threshold-intake-registry.schema.json",
                "mechanics/agon/parts/threshold-registry/schemas/tos-agon-threshold-intake-registry.schema.json",
            ),
            (
                "ToS/public-compatibility/tos_agon_threshold_intake_registry.example.json",
                "mechanics/agon/parts/threshold-registry/examples/tos_agon_threshold_intake_registry.example.json",
            ),
            (
                "ToS/derived-exports/tos_agon_threshold_intake_registry.min.json",
                "mechanics/agon/parts/threshold-registry/generated/tos_agon_threshold_intake_registry.min.json",
            ),
            (
                "config/tos_agon_threshold_intakes.config.json",
                "mechanics/agon/parts/threshold-registry/config/tos_agon_threshold_intakes.config.json",
            ),
        ),
        "landing-handoff": (
            (
                "ToS/doctrine/SOPHIAN_THRESHOLD_TOS_LANDING.md",
                "mechanics/agon/parts/landing-handoff/docs/SOPHIAN_THRESHOLD_TOS_LANDING.md",
            ),
        ),
    },
    "audit": {
        "review-ledger-route": (
            (
                "ToS/doctrine/REVIEW_CHECKLIST.md",
                "mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md",
            ),
        ),
    },
    "boundary-bridge": {
        "derived-kag-seam": (
            (
                "ToS/doctrine/KAG_EXPORT.md",
                "mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md",
            ),
        ),
    },
    "distillation": {
        "source-compost": (
            (
                "ToS/doctrine/CONTEXT_COMPOST.md",
                "mechanics/distillation/parts/source-compost/docs/CONTEXT_COMPOST.md",
            ),
        ),
    },
    "experience": {
        "candidate-review": (
            (
                "ToS/doctrine/AOA_EXPERIENCE_CANDIDATE_INTAKE_BOUNDARY.md",
                "mechanics/experience/parts/candidate-review/docs/AOA_EXPERIENCE_CANDIDATE_INTAKE_BOUNDARY.md",
            ),
            (
                "ToS/doctrine/TOS_CANDIDATE_DOSSIER_REVIEW.md",
                "mechanics/experience/parts/candidate-review/docs/TOS_CANDIDATE_DOSSIER_REVIEW.md",
            ),
            (
                "ToS/contracts/aoa_experience_candidate_dossier_v1.json",
                "mechanics/experience/parts/candidate-review/schemas/aoa_experience_candidate_dossier_v1.json",
            ),
            (
                "ToS/contracts/tos_intake_boundary_decision_v1.json",
                "mechanics/experience/parts/candidate-review/schemas/tos_intake_boundary_decision_v1.json",
            ),
            (
                "ToS/public-compatibility/aoa_experience_candidate_dossier.example.json",
                "mechanics/experience/parts/candidate-review/examples/aoa_experience_candidate_dossier.example.json",
            ),
            (
                "ToS/public-compatibility/tos_intake_boundary_decision.example.json",
                "mechanics/experience/parts/candidate-review/examples/tos_intake_boundary_decision.example.json",
            ),
        ),
        "adoption-boundary": (
            (
                "ToS/doctrine/TOS_ADOPTION_BOUNDARY_DOSSIER.md",
                "mechanics/experience/parts/adoption-boundary/docs/TOS_ADOPTION_BOUNDARY_DOSSIER.md",
            ),
            (
                "ToS/doctrine/NO_RUNTIME_ADOPTION_FROM_TOS.md",
                "mechanics/experience/parts/adoption-boundary/docs/NO_RUNTIME_ADOPTION_FROM_TOS.md",
            ),
            (
                "ToS/contracts/tos_adoption_boundary_dossier_v1.json",
                "mechanics/experience/parts/adoption-boundary/schemas/tos_adoption_boundary_dossier_v1.json",
            ),
            (
                "ToS/contracts/tos_no_runtime_adoption_guard_v1.json",
                "mechanics/experience/parts/adoption-boundary/schemas/tos_no_runtime_adoption_guard_v1.json",
            ),
            (
                "ToS/public-compatibility/tos_adoption_boundary_dossier.example.json",
                "mechanics/experience/parts/adoption-boundary/examples/tos_adoption_boundary_dossier.example.json",
            ),
            (
                "ToS/public-compatibility/tos_no_runtime_adoption_guard.example.json",
                "mechanics/experience/parts/adoption-boundary/examples/tos_no_runtime_adoption_guard.example.json",
            ),
        ),
        "governance-boundary": (
            (
                "ToS/doctrine/GOVERNANCE_DOSSIER_INTAKE_BOUNDARY.md",
                "mechanics/experience/parts/governance-boundary/docs/GOVERNANCE_DOSSIER_INTAKE_BOUNDARY.md",
            ),
            (
                "ToS/doctrine/GOVERNANCE_PRECEDENT_DOSSIER_BOUNDARY.md",
                "mechanics/experience/parts/governance-boundary/docs/GOVERNANCE_PRECEDENT_DOSSIER_BOUNDARY.md",
            ),
            (
                "ToS/doctrine/TOS_GOVERNANCE_REVIEW_NOTE.md",
                "mechanics/experience/parts/governance-boundary/docs/TOS_GOVERNANCE_REVIEW_NOTE.md",
            ),
            (
                "ToS/doctrine/NO_DIRECT_POLIS_GOVERNANCE_WRITE.md",
                "mechanics/experience/parts/governance-boundary/docs/NO_DIRECT_POLIS_GOVERNANCE_WRITE.md",
            ),
            (
                "ToS/contracts/tos_governance_dossier_boundary_v1.json",
                "mechanics/experience/parts/governance-boundary/schemas/tos_governance_dossier_boundary_v1.json",
            ),
            (
                "ToS/contracts/tos_governance_review_note_v1.json",
                "mechanics/experience/parts/governance-boundary/schemas/tos_governance_review_note_v1.json",
            ),
            (
                "ToS/public-compatibility/tos_governance_dossier_boundary_v1.example.json",
                "mechanics/experience/parts/governance-boundary/examples/tos_governance_dossier_boundary_v1.example.json",
            ),
            (
                "ToS/public-compatibility/tos_governance_review_note.example.json",
                "mechanics/experience/parts/governance-boundary/examples/tos_governance_review_note.example.json",
            ),
        ),
        "installation-boundary": (
            (
                "ToS/doctrine/INSTALLATION_DOSSIER_BOUNDARY.md",
                "mechanics/experience/parts/installation-boundary/docs/INSTALLATION_DOSSIER_BOUNDARY.md",
            ),
            (
                "ToS/doctrine/NO_DIRECT_EXPERIENCE_INSTALL_WRITE.md",
                "mechanics/experience/parts/installation-boundary/docs/NO_DIRECT_EXPERIENCE_INSTALL_WRITE.md",
            ),
            (
                "ToS/contracts/tos_installation_dossier_boundary_v1.json",
                "mechanics/experience/parts/installation-boundary/schemas/tos_installation_dossier_boundary_v1.json",
            ),
            (
                "ToS/public-compatibility/tos_installation_dossier_boundary_v1.example.json",
                "mechanics/experience/parts/installation-boundary/examples/tos_installation_dossier_boundary_v1.example.json",
            ),
        ),
        "service-office-boundary": (
            (
                "ToS/doctrine/SERVICE_DOSSIER_BOUNDARY.md",
                "mechanics/experience/parts/service-office-boundary/docs/SERVICE_DOSSIER_BOUNDARY.md",
            ),
            (
                "ToS/doctrine/NO_RUNTIME_OFFICE_WRITE.md",
                "mechanics/experience/parts/service-office-boundary/docs/NO_RUNTIME_OFFICE_WRITE.md",
            ),
            (
                "ToS/contracts/tos_service_dossier_boundary_v1.json",
                "mechanics/experience/parts/service-office-boundary/schemas/tos_service_dossier_boundary_v1.json",
            ),
            (
                "ToS/contracts/tos_no_runtime_office_write_guard_v1.json",
                "mechanics/experience/parts/service-office-boundary/schemas/tos_no_runtime_office_write_guard_v1.json",
            ),
            (
                "ToS/public-compatibility/tos_service_dossier_boundary_v1.example.json",
                "mechanics/experience/parts/service-office-boundary/examples/tos_service_dossier_boundary_v1.example.json",
            ),
            (
                "ToS/public-compatibility/tos_no_runtime_office_write_guard_v1.example.json",
                "mechanics/experience/parts/service-office-boundary/examples/tos_no_runtime_office_write_guard_v1.example.json",
            ),
        ),
        "pattern-review": (
            (
                "ToS/doctrine/TOS_PATTERN_REVIEW_NOTE.md",
                "mechanics/experience/parts/pattern-review/docs/TOS_PATTERN_REVIEW_NOTE.md",
            ),
            (
                "ToS/contracts/tos_pattern_review_note_v1.json",
                "mechanics/experience/parts/pattern-review/schemas/tos_pattern_review_note_v1.json",
            ),
            (
                "ToS/public-compatibility/tos_pattern_review_note.example.json",
                "mechanics/experience/parts/pattern-review/examples/tos_pattern_review_note.example.json",
            ),
        ),
        "write-guards": (
            (
                "ToS/doctrine/NO_DIRECT_ARENA_OR_EXPERIENCE_WRITE.md",
                "mechanics/experience/parts/write-guards/docs/NO_DIRECT_ARENA_OR_EXPERIENCE_WRITE.md",
            ),
            (
                "ToS/doctrine/NO_DIRECT_CONSTITUTION_RUNTIME_WRITE.md",
                "mechanics/experience/parts/write-guards/docs/NO_DIRECT_CONSTITUTION_RUNTIME_WRITE.md",
            ),
            (
                "ToS/contracts/tos_no_direct_write_guard_v1.json",
                "mechanics/experience/parts/write-guards/schemas/tos_no_direct_write_guard_v1.json",
            ),
            (
                "ToS/public-compatibility/tos_no_direct_write_guard.example.json",
                "mechanics/experience/parts/write-guards/examples/tos_no_direct_write_guard.example.json",
            ),
        ),
    },
    "growth-cycle": {
        "branch-growth-cycle": (
            (
                "ToS/doctrine/GROWTH_STRUCTURE.md",
                "mechanics/growth-cycle/parts/branch-growth-cycle/docs/GROWTH_STRUCTURE.md",
            ),
            (
                "ToS/doctrine/HUMAN_CURATED_EXPANSION.md",
                "mechanics/growth-cycle/parts/branch-growth-cycle/docs/HUMAN_CURATED_EXPANSION.md",
            ),
            (
                "ToS/doctrine/PRE_EXPANSION_SOIL.md",
                "mechanics/growth-cycle/parts/branch-growth-cycle/docs/PRE_EXPANSION_SOIL.md",
            ),
        ),
    },
    "questbook": {
        "obligation-boundary": (
            (
                "ToS/doctrine/QUESTBOOK_TOS_INTEGRATION.md",
                "mechanics/questbook/parts/obligation-boundary/docs/QUESTBOOK_TOS_INTEGRATION.md",
            ),
        ),
        "dispatch-contracts": (
            (
                "ToS/contracts/quest.schema.json",
                "mechanics/questbook/parts/dispatch-contracts/schemas/quest.schema.json",
            ),
            (
                "ToS/contracts/quest_dispatch.schema.json",
                "mechanics/questbook/parts/dispatch-contracts/schemas/quest_dispatch.schema.json",
            ),
            (
                "ToS/public-compatibility/quest_catalog.min.example.json",
                "mechanics/questbook/parts/dispatch-contracts/examples/quest_catalog.min.example.json",
            ),
            (
                "ToS/public-compatibility/quest_dispatch.min.example.json",
                "mechanics/questbook/parts/dispatch-contracts/examples/quest_dispatch.min.example.json",
            ),
        ),
    },
    "source-witnessing": {
        "witness-route": (
            (
                "ToS/doctrine/MANUAL_CORPUS_ENTRY_GATE.md",
                "mechanics/source-witnessing/parts/witness-route/docs/MANUAL_CORPUS_ENTRY_GATE.md",
            ),
        ),
    },
}


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


def validate_package(repo_root: Path, issues: list[Issue], slug: str, expected: dict[str, object]) -> None:
    for filename in ("AGENTS.md", "README.md", "PARTS.md", "PROVENANCE.md", "ROADMAP.md"):
        require_file(repo_root, issues, f"mechanics/{slug}/{filename}")

    parts = expected["parts"]
    assert isinstance(parts, tuple)
    for part in parts:
        require_file(repo_root, issues, f"mechanics/{slug}/parts/{part}/README.md")

    legacy_required = expected["legacy_required"]
    assert isinstance(legacy_required, bool)
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


def validate_moved_targets(repo_root: Path, issues: list[Issue], topology: dict[str, object]) -> None:
    moved_accounting = topology.get("moved_path_accounting")
    if not isinstance(moved_accounting, dict):
        issues.append((TOPOLOGY_PATH.as_posix(), "moved_path_accounting must be an object"))
        return

    for package, parts in MOVED_TARGETS.items():
        package_accounting = moved_accounting.get(package)
        if not isinstance(package_accounting, dict):
            issues.append((TOPOLOGY_PATH.as_posix(), f"missing moved_path_accounting.{package}"))
            continue
        for part, moves in parts.items():
            old_paths = package_accounting.get(part)
            if old_paths != [old for old, _ in moves]:
                issues.append((TOPOLOGY_PATH.as_posix(), f"{package}.{part} moved path accounting drifted"))
            for old_path, new_path in moves:
                require_absent(
                    repo_root,
                    issues,
                    old_path,
                    "mechanic-owned payload must stay in mechanics/, not the old ToS/root path",
                )
                require_file(repo_root, issues, new_path)


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

        expected = EXPECTED_PACKAGES.get(slug)
        if expected is None:
            issues.append((TOPOLOGY_PATH.as_posix(), f"unexpected top-level mechanic {slug}"))
            continue
        if entry.get("class") != expected["class"]:
            issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.class must be {expected['class']}"))
        if entry.get("status") != expected["status"]:
            issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.status must be {expected['status']}"))
        if entry.get("active_parts") != list(expected["parts"]):
            issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.active_parts must be {list(expected['parts'])!r}"))
        if entry.get("legacy_required") != expected["legacy_required"]:
            issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.legacy_required drifted"))
        validate_package(root, issues, slug, expected)

    if tuple(order) != EXPECTED_ORDER:
        issues.append((TOPOLOGY_PATH.as_posix(), "package order must stay shared-first and ToS-local-aware"))
    for missing in sorted(set(EXPECTED_PACKAGES) - set(seen)):
        issues.append((TOPOLOGY_PATH.as_posix(), f"missing package {missing}"))

    validate_moved_targets(root, issues, topology)

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
