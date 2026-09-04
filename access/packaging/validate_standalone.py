#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path
from typing import Any

BLOCKED_CODE_MARKERS = (b"/srv/" + b"AbyssOS", b"/srv/" + b"abyss-machine")
EXPECTED_QUERY_OPERATIONS = {
    "tos.status",
    "tos.search",
    "tos.view.open",
    "tos.node.inspect",
    "tos.neighborhood",
    "tos.epistemic.inspect",
    "tos.path.find",
}
EXPECTED_PAGE_COMMANDS = {
    "tos.page.context",
    "tos.page.open-view",
    "tos.page.search",
    "tos.page.select",
    "tos.page.show-neighborhood",
    "tos.page.start-path",
    "tos.page.find-path",
    "tos.page.reroute-without-selection",
    "tos.page.inspect-epistemic",
    "tos.page.clear-focus",
    "tos.page.cancel",
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _validate_contracts(repo_root: Path) -> None:
    contract_root = repo_root / "access/contracts"
    runtime = json.loads((contract_root / "runtime-manifest.v1.json").read_text(encoding="utf-8"))
    if runtime.get("authority_owner") != "Tree-of-Sophia":
        raise RuntimeError("runtime authority owner must be Tree-of-Sophia")
    profiles = {item.get("profile_id"): item for item in runtime.get("runtime_profiles", [])}
    if profiles.get("standalone", {}).get("requires_abyssos") is not False:
        raise RuntimeError("standalone profile must not require AbyssOS")
    posture = runtime.get("integration_posture")
    if posture != {
        "state": "paused",
        "scope": ["abyssos"],
        "default_profile": "standalone",
        "external_activation": "disabled",
        "unfreeze_requires": "explicit ToS operator command",
    }:
        raise RuntimeError("AbyssOS integration posture must remain explicitly paused")
    abyssos_profile = json.loads(
        (repo_root / "access/profiles/abyssos.v1.json").read_text(encoding="utf-8")
    )
    if abyssos_profile.get("availability") != "paused":
        raise RuntimeError("AbyssOS access profile must remain paused")
    migration = json.loads((contract_root / "web-actions.v1.json").read_text(encoding="utf-8"))
    expected_successors = {"query-operations.v1.json", "page-commands.v1.json"}
    if migration.get("status") != "superseded" or set(migration.get("superseded_by", [])) != expected_successors:
        raise RuntimeError("web action migration marker must route to the split contracts")
    query = json.loads((contract_root / "query-operations.v1.json").read_text(encoding="utf-8"))
    operation_ids = {item.get("operation_id") for item in query.get("operations", [])}
    if operation_ids != EXPECTED_QUERY_OPERATIONS:
        raise RuntimeError(f"query operation contract drift: {sorted(operation_ids)}")
    epistemic = json.loads((contract_root / "epistemic-packet.v1.schema.json").read_text(encoding="utf-8"))
    if epistemic.get("properties", {}).get("schema", {}).get("const") != "tos_philosophy_epistemic_packet_v1":
        raise RuntimeError("epistemic packet schema identity drift")
    authority = epistemic.get("properties", {}).get("authority_boundary", {}).get("properties", {})
    if {key: value.get("const") for key, value in authority.items()} != {
        "is_source": False,
        "is_canon": False,
        "is_semantic_truth": False,
        "is_rights_clearance": False,
    }:
        raise RuntimeError("epistemic packet authority boundary must fail closed")
    evidence_schema = json.loads(
        (repo_root / "ToS/contracts/epistemic-evidence-projection.schema.json").read_text(encoding="utf-8")
    )
    if evidence_schema.get("properties", {}).get("schema_version", {}).get("const") != "tos_epistemic_evidence_projection_v1":
        raise RuntimeError("Evidence Lens projection schema identity drift")
    evidence_packet = json.loads(
        (contract_root / "evidence-lens-packet.v1.schema.json").read_text(encoding="utf-8")
    )
    if evidence_packet.get("properties", {}).get("schema", {}).get("const") != "tos_evidence_lens_packet_v1":
        raise RuntimeError("Evidence Lens packet schema identity drift")
    page = json.loads((contract_root / "page-commands.v1.json").read_text(encoding="utf-8"))
    command_ids = {item.get("command_id") for item in page.get("commands", [])}
    if command_ids != EXPECTED_PAGE_COMMANDS:
        raise RuntimeError(f"page command contract drift: {sorted(command_ids)}")
    allowlist = json.loads((contract_root / "runtime-data.v1.json").read_text(encoding="utf-8"))
    paths = {item.get("source_path") for item in allowlist.get("subjects", [])}
    if "ToS/derived-exports/epistemic_evidence_projection.min.json" not in paths:
        raise RuntimeError("runtime allowlist must include the Evidence Lens projection")
    if any("lexical-search" in str(path) or "/payload/" in str(path) for path in paths):
        raise RuntimeError("runtime allowlist admits an explicitly excluded subject")


def _scan_source(access_root: Path) -> None:
    suffixes = {".py", ".json", ".toml", ".ts", ".js", ".html"}
    for path in access_root.rglob("*"):
        if not path.is_file() or path.suffix not in suffixes or "runtime_data" in path.parts:
            continue
        payload = path.read_bytes()
        if any(marker in payload for marker in BLOCKED_CODE_MARKERS):
            raise RuntimeError(f"hard-coded host path: {path.relative_to(access_root)}")


def validate_source(repo_root: Path) -> dict[str, Any]:
    _validate_contracts(repo_root)
    _scan_source(repo_root / "access")
    env = os.environ.copy()
    env["PYTHONPATH"] = (repo_root / "access/src").as_posix()
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "tos_access",
            "--root",
            repo_root.as_posix(),
            "verify",
            "--profile",
            "standalone",
            "--json",
        ],
        cwd=repo_root,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode:
        raise RuntimeError(f"source doctor failed: {result.stdout}\n{result.stderr}")
    report = json.loads(result.stdout)
    return {
        "schema_version": "tos_standalone_validation_v1",
        "ok": True,
        "mode": "source",
        "doctor": report,
    }


def _verify_external_manifest(bundle: Path, manifest_path: Path | None = None) -> tuple[Path, dict[str, Any]]:
    sidecar = manifest_path or bundle.with_suffix(bundle.suffix + ".manifest.json")
    if not sidecar.is_file():
        raise RuntimeError(f"missing external bundle manifest: {sidecar}")
    manifest = json.loads(sidecar.read_text(encoding="utf-8"))
    expected_digest = manifest.get("archive_sha256")
    expected_size = manifest.get("archive_size_bytes")
    if not isinstance(expected_digest, str) or not expected_digest:
        raise RuntimeError("external bundle manifest has no archive_sha256")
    if sha256_file(bundle) != expected_digest:
        raise RuntimeError("archive digest does not match external bundle manifest")
    if not isinstance(expected_size, int) or bundle.stat().st_size != expected_size:
        raise RuntimeError("archive size does not match external bundle manifest")
    return sidecar, manifest


def validate_bundle(
    bundle: Path,
    *,
    manifest_path: Path | None = None,
    with_mcp: bool = False,
) -> dict[str, Any]:
    sidecar, external_manifest = _verify_external_manifest(bundle, manifest_path)
    with tempfile.TemporaryDirectory(prefix="tos-standalone-validate-") as raw_temp:
        temp = Path(raw_temp)
        extracted = temp / "bundle"
        outside = temp / "outside"
        outside.mkdir()
        with zipfile.ZipFile(bundle) as archive:
            archive.extractall(extracted)
        if any(path.name == ".git" for path in extracted.rglob("*")):
            raise RuntimeError("standalone archive contains Git metadata")
        embedded_manifest_path = extracted / "bundle.manifest.json"
        manifest = json.loads(embedded_manifest_path.read_text(encoding="utf-8"))
        if manifest.get("schema_version") != "tos_standalone_bundle_manifest_v1":
            raise RuntimeError("unknown standalone bundle manifest schema")
        if any(external_manifest.get(key) != value for key, value in manifest.items()):
            raise RuntimeError("embedded bundle manifest does not match the external manifest")
        for subject in manifest.get("subjects", []):
            path = extracted / str(subject["bundle_path"])
            if (
                not path.is_file()
                or sha256_file(path) != subject.get("sha256")
                or path.stat().st_size != subject.get("size_bytes")
            ):
                raise RuntimeError(f"bundle subject integrity failure: {subject.get('subject_id')}")
        package_src = extracted / "access/src"
        env = {
            key: value
            for key, value in os.environ.items()
            if key not in {"TOS_ROOT", "AOA_TOS_ROOT", "PYTHONPATH"}
        }
        env["PYTHONPATH"] = package_src.as_posix()
        probe = """
import json
import threading
import urllib.request
from tos_access.core import ToSAccessCore
from tos_access.doctor import doctor_report
from tos_access.http_server import make_server

core = ToSAccessCore.discover()
report = doctor_report(require_mcp=False)
assert report["ok"], report
view_id = core.philosophy_views()["views"][0]["view_id"]
packet = core.philosophy_view(view_id)
assert packet["node_count"] > 0 and packet["edge_count"] > 0
evidence = core.evidence_projection()
assert len(evidence["scenes"]) >= 2
server = make_server(core, port=0)
thread = threading.Thread(target=server.serve_forever, daemon=True)
thread.start()
try:
    base = f"http://127.0.0.1:{server.server_port}"
    health = json.load(urllib.request.urlopen(base + "/health"))
    status = json.load(urllib.request.urlopen(base + "/api/philosophy/status"))
    assert health["ok"] and status["projection_exists"]
finally:
    server.shutdown()
    server.server_close()
    thread.join(timeout=5)
print(json.dumps({"ok": True, "doctor": report, "view_id": view_id}))
"""
        result = subprocess.run(
            [sys.executable, "-c", probe],
            cwd=outside,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if result.returncode:
            raise RuntimeError(f"archive smoke failed: {result.stdout}\n{result.stderr}")
        smoke = json.loads(result.stdout)

        venv_root = temp / "venv"
        create_venv = subprocess.run(
            [sys.executable, "-m", "venv", venv_root.as_posix()],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if create_venv.returncode:
            raise RuntimeError(f"venv creation failed: {create_venv.stdout}\n{create_venv.stderr}")
        venv_python = venv_root / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        install_target = (extracted / "access").as_posix() + ("[mcp]" if with_mcp else "")
        install_command = [venv_python.as_posix(), "-m", "pip", "install"]
        if not with_mcp:
            install_command.append("--no-deps")
        install_command.append(install_target)
        install = subprocess.run(
            install_command,
            cwd=outside,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if install.returncode:
            raise RuntimeError(f"standalone package install failed: {install.stdout}\n{install.stderr}")
        installed_env = env.copy()
        installed_env.pop("PYTHONPATH", None)
        installed_command = [venv_python.as_posix(), "-m", "tos_access"]
        installed_command.extend(["verify", "--profile", "standalone", "--json"] if with_mcp else ["doctor", "--json"])
        installed_doctor = subprocess.run(
            installed_command,
            cwd=outside,
            env=installed_env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if installed_doctor.returncode:
            raise RuntimeError(f"installed doctor failed: {installed_doctor.stdout}\n{installed_doctor.stderr}")
        installed_report = json.loads(installed_doctor.stdout)
        mcp_smoke = None
        if with_mcp:
            mcp_probe = subprocess.run(
                [
                    venv_python.as_posix(),
                    "-c",
                    "from tos_access.core import ToSAccessCore; from tos_access.mcp_server import build_server; core=ToSAccessCore.discover(); assert build_server(tos_root=core.tos_root) is not None; print('ok')",
                ],
                cwd=outside,
                env=installed_env,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            if mcp_probe.returncode:
                raise RuntimeError(f"installed MCP smoke failed: {mcp_probe.stdout}\n{mcp_probe.stderr}")
            mcp_smoke = "registered"
        return {
            "schema_version": "tos_standalone_validation_v1",
            "ok": True,
            "mode": "bundle",
            "archive_sha256": sha256_file(bundle),
            "external_manifest": sidecar.as_posix(),
            "source_ref": manifest.get("source_ref"),
            "smoke": smoke,
            "installed_doctor": installed_report,
            "installed_mcp_smoke": mcp_smoke,
            "claim_limit": "Archive integrity and standalone read paths only; no publication or consumer admission.",
        }


def main() -> None:
    parser = argparse.ArgumentParser(description="Validate Tree of Sophia standalone access")
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--bundle", type=Path)
    parser.add_argument("--manifest", type=Path, help="external bundle manifest; defaults to <bundle>.manifest.json")
    parser.add_argument("--with-mcp", action="store_true", help="install the MCP extra and verify native MCP registration")
    args = parser.parse_args()
    result = (
        validate_bundle(
            args.bundle.resolve(),
            manifest_path=args.manifest.resolve() if args.manifest else None,
            with_mcp=args.with_mcp,
        )
        if args.bundle
        else validate_source(args.repo_root.resolve())
    )
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
