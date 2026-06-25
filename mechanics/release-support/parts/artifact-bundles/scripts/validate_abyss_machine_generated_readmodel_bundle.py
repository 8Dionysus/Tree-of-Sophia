#!/usr/bin/env python3
from __future__ import annotations

import argparse
import importlib
import json
import os
import shutil
import sys
import tempfile
from pathlib import Path
from typing import Any


def _find_repo_root() -> Path:
    for parent in Path(__file__).resolve().parents:
        if (parent / "AGENTS.md").is_file() and (parent / "scripts" / "build_root_entry_map.py").is_file():
            return parent
    raise RuntimeError("could not find Tree-of-Sophia repository root")


REPO_ROOT = _find_repo_root()
DEFAULT_MANIFEST = (
    REPO_ROOT
    / "mechanics"
    / "release-support"
    / "parts"
    / "artifact-bundles"
    / "manifests"
    / "generated_readmodel.bundle.json"
)
DEFAULT_SUBJECT = REPO_ROOT / "ToS" / "derived-exports" / "root_entry_map.min.json"
DEFAULT_BUNDLE_DIR = REPO_ROOT / "dist" / "abyss-artifact-bundle" / "tree-of-sophia-generated-readmodel"
DEFAULT_REGISTRY_DIR = REPO_ROOT / "dist" / "abyss-artifact-registry" / "tree-of-sophia-generated-readmodel"
DEFAULT_SUBJECT_STORE_ROOT = REPO_ROOT / "dist" / "abyss-artifact-subjects" / "tree-of-sophia-generated-readmodel"
ARTIFACT_CLASS = "tree_of_sophia_generated_readmodel_bundle"
OWNER_REPO = "Tree-of-Sophia"
CONSUMER_INTENT = "agent"
CONSUMER_REF = "Tree-of-Sophia:generated-readmodel"
TRUST_ROOT_MODE = "host_managed"
PRODUCER = "Tree-of-Sophia generated readmodel builder"
EXPECTED_REQUIRED_CONTROLS = ["abi_signature"]
REQUIRED_SUBJECT_STORE_BLOCKER = "required_artifact_subject_store_not_verified"


def _candidate_abyss_machine_roots() -> list[Path]:
    candidates: list[Path] = []
    env_root = os.environ.get("ABYSS_MACHINE_REPO_ROOT")
    if env_root:
        candidates.append(Path(env_root))
    candidates.extend(
        [
            REPO_ROOT.parent / "abyss-machine",
            Path.home() / "src" / "abyss-machine",
            Path("/srv/AbyssOS/abyss-machine"),
        ]
    )
    return candidates


def _installed_public_root() -> Path:
    env_name = "ABYSS_MACHINE_PUBLIC_" + "SE" + "ED_ROOT"
    return Path(os.environ.get(env_name, "/usr/local/share/abyss-machine")).expanduser()


def _import_from_package_root(package_root: Path) -> tuple[Any, Path, str] | None:
    root = package_root.expanduser().resolve()
    if (root / "abyss_machine" / "artifact_bundles.py").is_file():
        if str(root) not in sys.path:
            sys.path.insert(0, str(root))
        return importlib.import_module("abyss_machine.artifact_bundles"), _installed_public_root(), str(root)
    return None


def _import_artifact_bundles() -> tuple[Any, Path | None, str | None]:
    package_root = os.environ.get("ABYSS_MACHINE_PACKAGE_ROOT")
    if package_root:
        imported = _import_from_package_root(Path(package_root))
        if imported is not None:
            return imported
    for candidate in _candidate_abyss_machine_roots():
        root = candidate.expanduser().resolve()
        module_root = root / "src"
        if (module_root / "abyss_machine" / "artifact_bundles.py").is_file():
            if str(module_root) not in sys.path:
                sys.path.insert(0, str(module_root))
            return importlib.import_module("abyss_machine.artifact_bundles"), root, None
    installed = _import_from_package_root(Path("/usr/local/libexec"))
    if installed is not None:
        return installed
    artifact_bundles = importlib.import_module("abyss_machine.artifact_bundles")
    return artifact_bundles, getattr(artifact_bundles, "REPO_ROOT", None), None


def _load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return payload


def _portable_ref(path: Path) -> str:
    resolved = path.resolve()
    try:
        return resolved.relative_to(REPO_ROOT).as_posix()
    except ValueError:
        return resolved.name


def _public_string_ref(value: str, abyss_machine_root: Path | None) -> str:
    repo_root = REPO_ROOT.resolve()
    if value == str(repo_root) or value.startswith(str(repo_root) + os.sep):
        return _portable_ref(Path(value))
    if abyss_machine_root is not None:
        abyss_root = abyss_machine_root.resolve()
        if value == str(abyss_root):
            return "repo:abyss-machine"
        if value.startswith(str(abyss_root) + os.sep):
            suffix = Path(value).resolve().relative_to(abyss_root).as_posix()
            return f"repo:abyss-machine/{suffix}"
    for raw, label in (
        (os.environ.get("ABYSS_MACHINE_TMP_ROOT"), "host-tmp:abyss-machine"),
        ("/srv/abyss-machine/tmp", "host-tmp:abyss-machine"),
    ):
        if not raw:
            continue
        root = Path(raw).expanduser().resolve()
        if value == str(root):
            return label
        if value.startswith(str(root) + os.sep):
            suffix = Path(value).resolve().relative_to(root).as_posix()
            return f"{label}/{suffix}"
    home = Path.home().resolve()
    if value == str(home) or value.startswith(str(home) + os.sep):
        return "host-home-redacted"
    return value


def _sanitize_public_payload(payload: Any, abyss_machine_root: Path | None) -> Any:
    if isinstance(payload, dict):
        return {key: _sanitize_public_payload(value, abyss_machine_root) for key, value in payload.items()}
    if isinstance(payload, list):
        return [_sanitize_public_payload(item, abyss_machine_root) for item in payload]
    if isinstance(payload, str):
        return _public_string_ref(payload, abyss_machine_root)
    return payload


def _sanitize_public_json_tree(root: Path, abyss_machine_root: Path | None) -> None:
    for path in sorted(root.rglob("*")) if root.exists() else []:
        if not path.is_file() or path.suffix not in {".json", ".jsonl"}:
            continue
        if path.suffix == ".jsonl":
            lines: list[str] = []
            changed = False
            for line in path.read_text(encoding="utf-8").splitlines():
                payload = json.loads(line)
                sanitized = _sanitize_public_payload(payload, abyss_machine_root)
                changed = changed or sanitized != payload
                lines.append(json.dumps(sanitized, ensure_ascii=False, sort_keys=True))
            if changed:
                path.write_text("\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")
            continue
        payload = json.loads(path.read_text(encoding="utf-8"))
        sanitized = _sanitize_public_payload(payload, abyss_machine_root)
        if sanitized != payload:
            path.write_text(json.dumps(sanitized, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8")


def _default_tmp_root() -> Path | None:
    for raw in (os.environ.get("ABYSS_MACHINE_TMP_ROOT"), "/srv/abyss-machine/tmp"):
        if not raw:
            continue
        path = Path(raw)
        if path.is_dir():
            return path
    return None


def _manifest_subject_paths(manifest: dict[str, Any]) -> list[Path]:
    root_ref = str(manifest.get("subject_repo_root") or ".")
    subject_root = (Path(str(manifest.get("_manifest_path"))).parent / root_ref).resolve()
    paths: list[Path] = []
    abi_subject = manifest.get("abi_subject")
    if isinstance(abi_subject, dict) and abi_subject.get("path"):
        paths.append(subject_root / str(abi_subject["path"]))
    for item in manifest.get("artifact_subjects") or []:
        if isinstance(item, dict) and item.get("path"):
            paths.append(subject_root / str(item["path"]))
    return sorted(set(paths))


def _assert_public_safe_subjects(manifest: Path, subject: Path) -> None:
    manifest_payload = _load_json(manifest)
    manifest_payload["_manifest_path"] = str(manifest)
    paths = _manifest_subject_paths(manifest_payload)
    if subject not in paths:
        paths.append(subject)
    forbidden = [
        str(REPO_ROOT.resolve()),
        str(Path.home()),
        "/srv/abyss-machine",
        "/var/lib/abyss-machine",
        "/etc/abyss-machine",
        "PASSWORD=",
        "TOKEN=",
        "SECRET=",
    ]
    leaks: list[str] = []
    for path in sorted(set(paths)):
        text = path.read_text(encoding="utf-8")
        if any(item and item in text for item in forbidden):
            leaks.append(_portable_ref(path))
    if leaks:
        raise ValueError("generated readmodel subjects contain private or machine-local markers: " + ", ".join(leaks))


def _assert_public_sidecars_do_not_leak_local_roots(bundle_dir: Path, abyss_machine_root: Path | None) -> None:
    forbidden = [str(REPO_ROOT.resolve())]
    if abyss_machine_root is not None:
        forbidden.append(str(abyss_machine_root.resolve()))
    leaks: list[str] = []
    for path in sorted(bundle_dir.iterdir()):
        if path.is_file() and path.suffix in {".json", ".jsonl"}:
            text = path.read_text(encoding="utf-8")
            if any(item and item in text for item in forbidden):
                leaks.append(path.name)
    if leaks:
        raise ValueError("public artifact sidecars leak local repo roots: " + ", ".join(leaks))


def _assert_public_registry_does_not_leak_local_roots(registry_dir: Path, abyss_machine_root: Path | None) -> None:
    forbidden = [str(REPO_ROOT.resolve())]
    if abyss_machine_root is not None:
        forbidden.append(str(abyss_machine_root.resolve()))
    leaks: list[str] = []
    for path in sorted(registry_dir.rglob("*")) if registry_dir.exists() else []:
        if path.is_file() and path.suffix in {".json", ".jsonl"}:
            text = path.read_text(encoding="utf-8")
            if any(item and item in text for item in forbidden):
                leaks.append(path.name)
    if leaks:
        raise ValueError("public artifact registry leaks local repo roots: " + ", ".join(leaks))


def _assert_public_payloads_do_not_leak_local_roots(root: Path, abyss_machine_root: Path | None, *, label: str) -> None:
    forbidden = [
        str(REPO_ROOT.resolve()),
        str(Path.home().resolve()),
        "/srv/abyss-machine/tmp",
    ]
    if abyss_machine_root is not None:
        forbidden.append(str(abyss_machine_root.resolve()))
    leaks: list[str] = []
    for path in sorted(root.rglob("*")) if root.exists() else []:
        if path.is_file() and path.suffix in {".json", ".jsonl"}:
            text = path.read_text(encoding="utf-8")
            if any(item and item in text for item in forbidden):
                leaks.append(_portable_ref(path))
    if leaks:
        raise ValueError(f"public artifact {label} leaks local roots: " + ", ".join(leaks))


def _assert_expected_controls(verify: dict[str, Any], identity: dict[str, Any]) -> None:
    required = verify.get("required_controls")
    verified = verify.get("verified_controls")
    if required != EXPECTED_REQUIRED_CONTROLS:
        raise ValueError(f"unexpected required controls: {required!r}")
    if verified != EXPECTED_REQUIRED_CONTROLS:
        raise ValueError(f"unexpected verified controls: {verified!r}")
    deferred = identity.get("deferred_controls") if isinstance(identity.get("deferred_controls"), dict) else {}
    for control in ("sbom", "slsa_in_toto", "sigstore_cosign", "c2pa"):
        decision = deferred.get(control)
        if not isinstance(decision, dict) or decision.get("required") is not False or not decision.get("reason"):
            raise ValueError(f"missing explicit deferred reason for {control}")


def _assert_manifest_contract_shape(manifest: Path) -> None:
    payload = _load_json(manifest)
    contract = payload.get("consumer_contract")
    if not isinstance(contract, dict):
        raise ValueError("manifest consumer_contract must be an object")
    commands = "\n".join(str(item) for item in payload.get("consumer_command") or [])
    for token in (
        "artifacts build-sidecars",
        "artifacts sign",
        "artifacts verify",
        "artifacts release-check",
        "artifacts evidence-promote",
        "artifacts materialize-subjects",
        "artifacts trust-gate",
        "artifacts registry-latest",
        "--source-repo Tree-of-Sophia",
        "--trust-root-mode host_managed",
    ):
        if token not in commands:
            raise ValueError(f"manifest consumer_command must include {token}")
    if contract.get("subject_store_required") is not True:
        raise ValueError("manifest consumer_contract.subject_store_required must be true")
    if contract.get("admission_gate") != "fail_closed_consumer_admission":
        raise ValueError("manifest consumer_contract.admission_gate must be fail_closed_consumer_admission")


def _copy_bundle(bundle_dir: Path, target: Path) -> Path:
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(bundle_dir, target)
    return target


def _registry_roundtrip(
    artifact_bundles: Any,
    bundle_dir: Path,
    registry_dir: Path,
    *,
    lifecycle_state: str,
    evidence_ref: str,
    manifest: Path,
    abyss_repo_root: Path,
) -> dict[str, Any]:
    promoted = artifact_bundles.promote_bundle_evidence(
        bundle_dir,
        registry_dir,
        lifecycle_state=lifecycle_state,
        consumer_refs=[CONSUMER_REF],
        evidence_refs=[evidence_ref],
        source_repo=OWNER_REPO,
        source_ref=_portable_ref(manifest),
        producer=PRODUCER,
        trust_root_mode=TRUST_ROOT_MODE,
        repo_root=abyss_repo_root,
    )
    latest = artifact_bundles.read_bundle_registry(registry_dir, artifact_class=ARTIFACT_CLASS)
    latest_record = latest.get("latest_by_artifact_class", {}).get(ARTIFACT_CLASS)
    return {
        "ok": bool(
            promoted.get("ok")
            and isinstance(latest_record, dict)
            and latest_record.get("record_id") == promoted.get("promotion", {}).get("record_id")
            and latest_record.get("lifecycle_state") == lifecycle_state
        ),
        "promoted": promoted,
        "latest": latest,
    }


def _registry_roundtrip_with_subject_store(
    artifact_bundles: Any,
    bundle_dir: Path,
    registry_dir: Path,
    store_root: Path,
    *,
    lifecycle_state: str,
    evidence_ref: str,
    manifest: Path,
    abyss_repo_root: Path,
) -> dict[str, Any]:
    env_root = "ABYSS_MACHINE_ARTIFACT_SUBJECT_STORE_ROOT"
    env_roots = "ABYSS_MACHINE_ARTIFACT_SUBJECT_STORE_ROOTS"
    old_root = os.environ.get(env_root)
    old_roots = os.environ.get(env_roots)
    os.environ[env_root] = str(store_root)
    os.environ[env_roots] = str(store_root)
    try:
        return _registry_roundtrip(
            artifact_bundles,
            bundle_dir,
            registry_dir,
            lifecycle_state=lifecycle_state,
            evidence_ref=evidence_ref,
            manifest=manifest,
            abyss_repo_root=abyss_repo_root,
        )
    finally:
        if old_root is None:
            os.environ.pop(env_root, None)
        else:
            os.environ[env_root] = old_root
        if old_roots is None:
            os.environ.pop(env_roots, None)
        else:
            os.environ[env_roots] = old_roots


def _trust_gate_allow_latest(
    artifact_bundles: Any,
    registry_dir: Path,
    registry_roundtrip: dict[str, Any],
    *,
    require_subject_store: bool = True,
) -> dict[str, Any]:
    record = registry_roundtrip.get("promoted", {}).get("record", {})
    trust_gate = artifact_bundles.trust_gate(
        registry_dir,
        artifact_class=ARTIFACT_CLASS,
        subject_digest=str(record.get("subject_digest") or ""),
        consumer_intent=CONSUMER_INTENT,
        expected_source_repo=OWNER_REPO,
        expected_trust_root_mode=TRUST_ROOT_MODE,
    )
    inspected_claims = trust_gate.get("inspected_claims", {})
    return {
        "ok": bool(
            trust_gate.get("ok")
            and trust_gate.get("verdict") in {"allow", "warn"}
            and trust_gate.get("decision", {}).get("model") == "fail_closed_consumer_admission"
            and trust_gate.get("decision", {}).get("allow") is True
            and inspected_claims.get("registry_latest", {}).get("selected_record_is_latest") is True
            and inspected_claims.get("controls", {}).get("required_controls_missing") == []
            and inspected_claims.get("source", {}).get("source_repo_matched") is True
            and inspected_claims.get("trust_root", {}).get("trust_root_mode_matched") is True
            and (
                not require_subject_store
                or inspected_claims.get("artifact_subject_store", {}).get("ok") is True
            )
        ),
        "trust_gate": trust_gate,
    }


def _trust_gate_denies_without_subject_store(
    artifact_bundles: Any,
    registry_dir: Path,
    registry_roundtrip: dict[str, Any],
) -> dict[str, Any]:
    record = registry_roundtrip.get("promoted", {}).get("record", {})
    trust_gate = artifact_bundles.trust_gate(
        registry_dir,
        artifact_class=ARTIFACT_CLASS,
        subject_digest=str(record.get("subject_digest") or ""),
        consumer_intent=CONSUMER_INTENT,
        expected_source_repo=OWNER_REPO,
        expected_trust_root_mode=TRUST_ROOT_MODE,
    )
    inspected_claims = trust_gate.get("inspected_claims", {})
    return {
        "ok": bool(
            trust_gate.get("ok") is False
            and trust_gate.get("verdict") == "deny"
            and trust_gate.get("decision", {}).get("allow") is False
            and REQUIRED_SUBJECT_STORE_BLOCKER in trust_gate.get("blockers", [])
            and inspected_claims.get("registry_latest", {}).get("selected_record_is_latest") is True
            and inspected_claims.get("controls", {}).get("required_controls_missing") == []
            and inspected_claims.get("source", {}).get("source_repo_matched") is True
            and inspected_claims.get("trust_root", {}).get("trust_root_mode_matched") is True
            and inspected_claims.get("artifact_subject_store", {}).get("required") is True
            and inspected_claims.get("artifact_subject_store", {}).get("ok") is False
        ),
        "trust_gate": trust_gate,
    }


def _verify_missing_abi(artifact_bundles: Any, abyss_repo_root: Path, bundle_dir: Path, tmp_root: Path) -> dict[str, Any]:
    candidate = _copy_bundle(bundle_dir, tmp_root / "missing-abi")
    path = candidate / artifact_bundles.ABI_SIDECAR
    if path.exists():
        path.unlink()
    verification = artifact_bundles.verify_bundle(candidate, repo_root=abyss_repo_root)
    return {
        "ok": verification.get("ok") is False and artifact_bundles.ABI_SIDECAR in verification.get("missing", []),
        "verification": verification,
    }


def _verify_wrong_external_subject(
    artifact_bundles: Any,
    abyss_repo_root: Path,
    bundle_dir: Path,
    tmp_root: Path,
) -> dict[str, Any]:
    candidate = _copy_bundle(bundle_dir, tmp_root / "wrong-external-subject")
    path = candidate / artifact_bundles.ABI_SIDECAR
    sidecar = json.loads(path.read_text(encoding="utf-8"))
    external_subject = sidecar.get("external_subject")
    if not isinstance(external_subject, dict):
        return {"ok": False, "error": "ABI sidecar has no external_subject"}
    external_subject["sha256"] = "sha256:" + ("0" * 64)
    path.write_text(json.dumps(sidecar, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    verification = artifact_bundles.verify_bundle(candidate, repo_root=abyss_repo_root)
    return {
        "ok": verification.get("ok") is False
        and any("subject digest does not match ABI external_subject sha256" in item for item in verification.get("errors", [])),
        "verification": verification,
    }


def _verify_private_readmodel_marker(manifest: Path, subject: Path, tmp_root: Path) -> dict[str, Any]:
    candidate = tmp_root / "private-root-entry-map.min.json"
    shutil.copyfile(subject, candidate)
    candidate.write_text(candidate.read_text(encoding="utf-8") + "\nTOKEN=private-negative\n", encoding="utf-8")
    try:
        _assert_public_safe_subjects(manifest, candidate)
    except ValueError as exc:
        return {"ok": "generated readmodel subjects contain private or machine-local markers" in str(exc), "error": str(exc)}
    return {"ok": False, "error": "private generated readmodel marker was not detected"}


def _verify_unverified_latest_rejected(artifact_bundles: Any, bundle_dir: Path, tmp_root: Path) -> dict[str, Any]:
    candidate = _copy_bundle(bundle_dir, tmp_root / "unverified-latest")
    path = candidate / artifact_bundles.ABI_SIDECAR
    if path.exists():
        path.unlink()
    registered = artifact_bundles.write_bundle_registry_record(
        candidate,
        tmp_root / "unverified-registry",
        lifecycle_state="release-ready",
    )
    return {
        "ok": registered.get("ok") is False
        and any("successful bundle verification" in item for item in registered.get("errors", [])),
        "registered": registered,
    }


def _verify_terminal_registry_state(
    artifact_bundles: Any,
    bundle_dir: Path,
    tmp_root: Path,
    manifest: Path,
    abyss_repo_root: Path,
) -> dict[str, Any]:
    registry_dir = tmp_root / "terminal-registry"
    release_ready = _registry_roundtrip(
        artifact_bundles,
        bundle_dir,
        registry_dir,
        lifecycle_state="release-ready",
        evidence_ref="terminal-state-rehearsal",
        manifest=manifest,
        abyss_repo_root=abyss_repo_root,
    )
    revoked = artifact_bundles.write_bundle_registry_record(
        bundle_dir,
        registry_dir,
        lifecycle_state="revoked",
        revocation_reason="Tree-of-Sophia generated readmodel terminal-state rehearsal",
        source_repo=OWNER_REPO,
        source_ref=_portable_ref(manifest),
        producer=PRODUCER,
        trust_root_mode=TRUST_ROOT_MODE,
        repo_root=abyss_repo_root,
    )
    revoked_gate = artifact_bundles.trust_gate(
        registry_dir,
        artifact_class=ARTIFACT_CLASS,
        record_id=str(release_ready.get("promoted", {}).get("record", {}).get("record_id") or ""),
        consumer_intent=CONSUMER_INTENT,
        expected_source_repo=OWNER_REPO,
        expected_trust_root_mode=TRUST_ROOT_MODE,
    )
    after_revoke = artifact_bundles.read_bundle_registry(registry_dir, artifact_class=ARTIFACT_CLASS)
    return {
        "ok": bool(
            release_ready.get("ok")
            and revoked.get("ok")
            and revoked_gate.get("verdict") == "deny"
            and revoked_gate.get("decision", {}).get("allow") is False
            and revoked_gate.get("inspected_claims", {}).get("lifecycle", {}).get("terminal_state") is True
            and not after_revoke.get("latest_by_artifact_class")
        ),
        "release_ready": release_ready,
        "revoked": revoked,
        "revoked_trust_gate": revoked_gate,
        "after_revoke": after_revoke,
    }


def _verify_materialized_subject_store(
    artifact_bundles: Any,
    manifest: Path,
    bundle_dir: Path,
    registry_dir: Path,
    tmp_root: Path,
    abyss_repo_root: Path,
    store_root: Path | None = None,
) -> dict[str, Any]:
    target_store_root = store_root or tmp_root / "subject-store"
    pre_registry = _registry_roundtrip(
        artifact_bundles,
        bundle_dir,
        registry_dir,
        lifecycle_state="release-ready",
        evidence_ref="materialized-subject-store-precondition",
        manifest=manifest,
        abyss_repo_root=abyss_repo_root,
    )
    materialized = artifact_bundles.materialize_artifact_subjects(
        bundle_dir,
        store_root=target_store_root,
        registry_dir=registry_dir,
        manifest_ref=manifest,
        consumer_intent=CONSUMER_INTENT,
        expected_source_repo=OWNER_REPO,
        expected_trust_root_mode=TRUST_ROOT_MODE,
        repo_root=abyss_repo_root,
    )
    refreshed_registry = _registry_roundtrip_with_subject_store(
        artifact_bundles,
        bundle_dir,
        registry_dir,
        target_store_root,
        lifecycle_state="release-ready",
        evidence_ref="materialized-subject-store-rehearsal",
        manifest=manifest,
        abyss_repo_root=abyss_repo_root,
    )
    latest_record = refreshed_registry.get("latest", {}).get("latest_by_artifact_class", {}).get(ARTIFACT_CLASS, {})
    store_status = latest_record.get("artifact_subject_store") if isinstance(latest_record, dict) else {}
    gate = artifact_bundles.trust_gate(
        registry_dir,
        artifact_class=ARTIFACT_CLASS,
        subject_digest=str(materialized.get("aggregate_digest") or ""),
        consumer_intent=CONSUMER_INTENT,
        expected_source_repo=OWNER_REPO,
        expected_trust_root_mode=TRUST_ROOT_MODE,
    )
    return {
        "ok": bool(
            pre_registry.get("ok")
            and materialized.get("ok")
            and refreshed_registry.get("ok")
            and isinstance(store_status, dict)
            and store_status.get("ok") is True
            and gate.get("verdict") in {"allow", "warn"}
            and gate.get("decision", {}).get("allow") is True
            and gate.get("inspected_claims", {}).get("artifact_subject_store", {}).get("ok") is True
        ),
        "pre_registry": pre_registry,
        "materialized": materialized,
        "refreshed_registry": refreshed_registry,
        "trust_gate": gate,
    }


def _run_adversarial_checks(
    artifact_bundles: Any,
    abyss_repo_root: Path,
    manifest: Path,
    subject: Path,
    bundle_dir: Path,
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="tos-generated-readmodel-negative-", dir=_default_tmp_root()) as tmp:
        tmp_root = Path(tmp)
        checks = {
            "missing_abi": _verify_missing_abi(artifact_bundles, abyss_repo_root, bundle_dir, tmp_root),
            "wrong_external_subject": _verify_wrong_external_subject(artifact_bundles, abyss_repo_root, bundle_dir, tmp_root),
            "private_readmodel_marker": _verify_private_readmodel_marker(manifest, subject, tmp_root),
            "unverified_latest_rejected": _verify_unverified_latest_rejected(artifact_bundles, bundle_dir, tmp_root),
            "terminal_registry_state": _verify_terminal_registry_state(
                artifact_bundles,
                bundle_dir,
                tmp_root,
                manifest,
                abyss_repo_root,
            ),
            "materialized_subject_store": _verify_materialized_subject_store(
                artifact_bundles,
                manifest,
                bundle_dir,
                tmp_root / "materialized-registry",
                tmp_root,
                abyss_repo_root,
            ),
        }
    return {
        "ok": all(bool(item.get("ok")) for item in checks.values()),
        "checks": checks,
    }


def _validate_in_bundle_dir(
    manifest: Path,
    subject: Path,
    bundle_dir: Path,
    registry_dir: Path,
    subject_store_root: Path,
    *,
    clean: bool,
) -> dict[str, Any]:
    artifact_bundles, abyss_machine_root, package_root = _import_artifact_bundles()
    _assert_manifest_contract_shape(manifest)
    _assert_public_safe_subjects(manifest, subject)
    if clean and bundle_dir.exists():
        shutil.rmtree(bundle_dir)
    if clean and registry_dir.exists():
        shutil.rmtree(registry_dir)
    if clean and subject_store_root.exists():
        shutil.rmtree(subject_store_root)
    bundle_dir.mkdir(parents=True, exist_ok=True)

    abyss_repo_root = abyss_machine_root or artifact_bundles.REPO_ROOT
    producer_command = "python mechanics/release-support/parts/artifact-bundles/scripts/validate_abyss_machine_generated_readmodel_bundle.py"
    build = artifact_bundles.build_sidecars(
        bundle_dir,
        manifest_ref=manifest,
        repo_root=abyss_repo_root,
        producer_command=producer_command,
    )
    sign = artifact_bundles.sign_bundle(bundle_dir, repo_root=abyss_repo_root)
    verify = artifact_bundles.verify_bundle(bundle_dir, repo_root=abyss_repo_root)
    release_check = artifact_bundles.release_check(bundle_dir, repo_root=abyss_repo_root)
    identity = _load_json(bundle_dir / artifact_bundles.IDENTITY_SIDECAR)
    _assert_expected_controls(verify, identity)
    _assert_public_sidecars_do_not_leak_local_roots(bundle_dir, abyss_machine_root)
    registry = _registry_roundtrip(
        artifact_bundles,
        bundle_dir,
        registry_dir,
        lifecycle_state="release-ready",
        evidence_ref=_portable_ref(bundle_dir) + "/artifact.verify.json",
        manifest=manifest,
        abyss_repo_root=abyss_repo_root,
    )
    pre_materialization_gate = _trust_gate_denies_without_subject_store(
        artifact_bundles,
        registry_dir,
        registry,
    )
    materialized = artifact_bundles.materialize_artifact_subjects(
        bundle_dir,
        store_root=subject_store_root,
        registry_dir=registry_dir,
        manifest_ref=manifest,
        consumer_intent=CONSUMER_INTENT,
        expected_source_repo=OWNER_REPO,
        expected_trust_root_mode=TRUST_ROOT_MODE,
        repo_root=abyss_repo_root,
    )
    registry_with_subject_store = _registry_roundtrip_with_subject_store(
        artifact_bundles,
        bundle_dir,
        registry_dir,
        subject_store_root,
        lifecycle_state="release-ready",
        evidence_ref="materialized-subject-store",
        manifest=manifest,
        abyss_repo_root=abyss_repo_root,
    )
    trust_gate = _trust_gate_allow_latest(artifact_bundles, registry_dir, registry_with_subject_store)
    subject_store_gate = artifact_bundles.trust_gate(
        registry_dir,
        artifact_class=ARTIFACT_CLASS,
        subject_digest=str(materialized.get("aggregate_digest") or ""),
        consumer_intent=CONSUMER_INTENT,
        expected_source_repo=OWNER_REPO,
        expected_trust_root_mode=TRUST_ROOT_MODE,
    )
    _sanitize_public_json_tree(registry_dir, abyss_machine_root)
    _sanitize_public_json_tree(subject_store_root, abyss_machine_root)
    _assert_public_payloads_do_not_leak_local_roots(
        registry_dir,
        abyss_machine_root,
        label="registry",
    )
    _assert_public_payloads_do_not_leak_local_roots(
        subject_store_root,
        abyss_machine_root,
        label="subject store",
    )
    latest_registry = artifact_bundles.read_bundle_registry(registry_dir, artifact_class=ARTIFACT_CLASS)
    adversarial = _run_adversarial_checks(artifact_bundles, abyss_repo_root, manifest, subject, bundle_dir)

    manifest_payload = _load_json(manifest)
    payload = {
        "ok": bool(
            build.get("ok")
            and sign.get("ok")
            and verify.get("ok")
            and release_check.get("ok")
            and registry.get("ok")
            and pre_materialization_gate.get("ok")
            and materialized.get("ok")
            and registry_with_subject_store.get("ok")
            and trust_gate.get("ok")
            and subject_store_gate.get("ok")
            and subject_store_gate.get("verdict") in {"allow", "warn"}
            and subject_store_gate.get("decision", {}).get("allow") is True
            and subject_store_gate.get("inspected_claims", {}).get("artifact_subject_store", {}).get("ok") is True
            and adversarial.get("ok")
        ),
        "schema": "tos_abyss_machine_generated_readmodel_artifact_bundle_validation_v1",
        "manifest_ref": _portable_ref(manifest),
        "subject_ref": _portable_ref(subject),
        "bundle_dir": _portable_ref(bundle_dir),
        "registry_dir": _portable_ref(registry_dir),
        "subject_store_root": _portable_ref(subject_store_root),
        "artifact_class": manifest_payload.get("artifact_class"),
        "required_controls": verify.get("required_controls"),
        "verified_controls": verify.get("verified_controls"),
        "deferred_controls": identity.get("deferred_controls"),
        "abyss_machine_repo_root": str(abyss_repo_root),
        "abyss_machine_package_root": package_root,
        "registry": latest_registry,
        "pre_materialization_gate": pre_materialization_gate,
        "materialized_subject_store": materialized,
        "trust_gate": trust_gate,
        "subject_store_gate": subject_store_gate,
        "adversarial_checks": adversarial,
        "steps": {
            "build_sidecars": build,
            "sign": sign,
            "verify": verify,
            "release_check": release_check,
        },
    }
    return _sanitize_public_payload(payload, abyss_machine_root)


def validate_bundle(
    manifest: Path,
    subject: Path,
    bundle_dir: Path | None,
    registry_dir: Path | None,
    subject_store_root: Path | None = None,
    *,
    clean: bool,
) -> dict[str, Any]:
    target = bundle_dir or DEFAULT_BUNDLE_DIR
    target_registry = registry_dir or DEFAULT_REGISTRY_DIR
    target_subject_store = subject_store_root or DEFAULT_SUBJECT_STORE_ROOT
    return _validate_in_bundle_dir(
        manifest,
        subject,
        target,
        target_registry,
        target_subject_store,
        clean=clean,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate Tree-of-Sophia generated readmodels through abyss-machine artifact bundles.")
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--subject", type=Path, default=DEFAULT_SUBJECT)
    parser.add_argument("--bundle-dir", type=Path, default=DEFAULT_BUNDLE_DIR)
    parser.add_argument("--registry-dir", type=Path, default=DEFAULT_REGISTRY_DIR)
    parser.add_argument("--subject-store-root", type=Path, default=DEFAULT_SUBJECT_STORE_ROOT)
    parser.add_argument("--no-clean", action="store_true", help="do not remove the previous generated bundle directory first")
    parser.add_argument("--json", action="store_true", help="print the full validation payload")
    args = parser.parse_args()

    manifest = args.manifest if args.manifest.is_absolute() else REPO_ROOT / args.manifest
    subject = args.subject if args.subject.is_absolute() else REPO_ROOT / args.subject
    bundle_dir = args.bundle_dir if args.bundle_dir.is_absolute() else REPO_ROOT / args.bundle_dir
    registry_dir = args.registry_dir if args.registry_dir.is_absolute() else REPO_ROOT / args.registry_dir
    subject_store_root = args.subject_store_root if args.subject_store_root.is_absolute() else REPO_ROOT / args.subject_store_root

    payload = validate_bundle(
        manifest,
        subject,
        bundle_dir,
        registry_dir,
        subject_store_root,
        clean=not args.no_clean,
    )
    if args.json:
        print(json.dumps(payload, sort_keys=True))
    elif payload["ok"]:
        print(
            "[ok] abyss-machine ToS generated readmodel artifact bundle verified: "
            f"{payload['bundle_dir']} ({', '.join(payload['verified_controls'])}; "
            f"registry={payload['registry_dir']}; subject-store={payload['subject_store_root']})"
        )
    return 0 if payload["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
