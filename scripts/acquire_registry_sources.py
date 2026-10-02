#!/usr/bin/env python3
"""Thin compatibility adapter for native registry-source acquisition."""
from __future__ import annotations

import argparse
import base64
import json
from pathlib import Path
import subprocess
import sys
import xml.etree.ElementTree as ET

try:
    from acquisition_native import NativeAcquisitionError, invoke as _native_invoke
except ModuleNotFoundError as exc:  # pragma: no cover - package import compatibility
    if exc.name != "acquisition_native":
        raise
    from scripts.acquisition_native import NativeAcquisitionError, invoke as _native_invoke

ROOT = Path(__file__).resolve().parents[1]
SOURCE = "ToS/source-witnesses"
TOPOLOGY = f"{SOURCE}/relations/provenance.jsonl"
TOPOLOGY_EVENT = "tos.event.annotation.source-witness-bibliographic-topology.2026-07-31"
ALLOWED_REPOSITORIES = {
    "openscriptures/morphhb",
    "oraec/corpus_raw_data",
    "suttacentral/bilara-data",
    "PerseusDL/canonical-greekLit",
    "PerseusDL/canonical-latinLit",
}
NO_ACQUISITION = {
    "downloaded": False,
    "acquired_at": None,
    "byte_size": None,
    "sha256": None,
    "event_ref": None,
}


def _absolute(path: Path | str) -> str:
    return str(Path(path).absolute())


def _wire_bytes(body: bytes) -> str:
    if not isinstance(body, bytes):
        raise TypeError("native registry byte helpers require bytes")
    return base64.b64encode(body).decode("ascii")


def _call(name: str, *, fetcher=None, **args):
    return _native_invoke(
        {
            "family": "registry",
            "operation": f"registry.helper.{name}",
            "args": args,
        },
        fetcher=fetcher,
    )


def sha256(data: bytes) -> str:
    return _call("sha256", body=_wire_bytes(data))


def utcnow() -> str:
    return _call("utcnow")


def event(
    event_id: str,
    event_type: str,
    started: str,
    ended: str,
    inputs: list[dict],
    outputs: list[dict],
    *,
    name: str,
    configuration: dict,
    rights_ref: str,
    receipts: list[str],
) -> dict:
    return _call(
        "event",
        event_id=event_id,
        event_type=event_type,
        started=started,
        ended=ended,
        inputs=inputs,
        outputs=outputs,
        name=name,
        configuration=configuration,
        rights_ref=rights_ref,
        receipts=receipts,
    )


def json_bytes(value: object) -> bytes:
    return base64.b64decode(_call("json_bytes", value=value), validate=True)


def write_json(path: Path, value: object) -> None:
    _call("write_json", path=_absolute(path), value=value)


def append_jsonl(path: Path, value: dict) -> None:
    _call("append_jsonl", path=_absolute(path), value=value)


def write_jsonl_record(path: Path, value: dict) -> None:
    """Append one compact record through the native guarded JSONL writer."""
    append_jsonl(path, value)


def manifest_fixity(manifest: dict) -> str:
    return _call("manifest_fixity", manifest=manifest)


def rights_with_file_scopes(prepared_rights: dict, manifest: dict) -> dict:
    return _call("rights_with_file_scopes", rights=prepared_rights, manifest=manifest)


def safe_path(root: Path, ref: str) -> Path:
    return Path(_call("safe_path", root=_absolute(root), reference=ref))


def validate_json(value: dict, schema_name: str, root: Path) -> None:
    _call("validate_json", root=_absolute(root), value=value, schema_name=schema_name)


def check_file(body: bytes, entry: dict) -> str:
    return _call("check_file", body=_wire_bytes(body), entry=entry)


def strict_json(body: bytes) -> object:
    return _call("strict_json", body=_wire_bytes(body))


def load_preparation(root: Path, path: Path, *, allow_unbound: bool = False) -> tuple[dict, dict[str, dict]]:
    result = _call(
        "load_preparation",
        root=_absolute(root),
        path=_absolute(path),
        allow_unbound=allow_unbound,
    )
    return result["manifest"], result["packages"]


def operation_date(target: dict) -> str:
    return _call("operation_date", target=target)


def selected_metadata_observations(preparation: dict, target: dict) -> list[dict]:
    return _call("selected_metadata_observations", preparation=preparation, target=target)


def metadata_elapsed_seconds(observation: dict) -> float:
    return _call("metadata_elapsed_seconds", **observation)


def preflight_identities(root: Path, targets: list[dict], packages: dict[str, dict]) -> None:
    _call("preflight_identities", root=_absolute(root), targets=targets, packages=packages)


def validate_work_extension(root: Path, target: dict, package: dict) -> tuple[str, bytes] | None:
    result = _call(
        "validate_work_extension",
        root=_absolute(root),
        target=target,
        package=package,
    )
    if result is None:
        return None
    return result["path"], base64.b64decode(result["before_base64"], validate=True)


def check_preparation_receipt(root: Path, manifest_path: Path, receipt_path: Path) -> dict:
    return _call(
        "check_preparation_receipt",
        root=_absolute(root),
        manifest_path=_absolute(manifest_path),
        receipt_path=_absolute(receipt_path),
    )


def tei_division_addresses(
    edition: ET.Element,
    milestone_unit: str | None = None,
    repeated_milestones: list[dict] | None = None,
    repeated_divisions: list[dict] | None = None,
) -> list[str]:
    result = _call(
        "tei_division_addresses",
        xml=ET.tostring(edition, encoding="unicode"),
        milestone_unit=milestone_unit,
        collect_repeated_milestones=repeated_milestones is not None,
        collect_repeated_divisions=repeated_divisions is not None,
    )
    if repeated_milestones is not None:
        repeated_milestones.extend(_tuple_addresses(result["repeated_milestones"]))
    if repeated_divisions is not None:
        repeated_divisions.extend(_tuple_addresses(result["repeated_divisions"]))
    return result["addresses"]


def _tuple_addresses(rows: list[dict]) -> list[dict]:
    return [
        {
            "source_address": [tuple(part) for part in row["source_address"]],
            "occurrences": row["occurrences"],
        }
        for row in rows
    ]


def inspect_payloads(target: dict, bodies: list[tuple[dict, bytes]]) -> dict:
    return _call(
        "inspect_payloads",
        target=target,
        bodies=[{"entry": entry, "body": _wire_bytes(body)} for entry, body in bodies],
    )


def transfer(
    root: Path,
    target: dict,
    entry: dict,
    log: Path,
    *,
    payload_source_root: Path | None = None,
    fetcher=None,
) -> tuple[bytes, dict]:
    if fetcher is None:
        raise ValueError("registry transfer helper requires an explicit fixture fetcher")
    arguments = {
        "root": _absolute(root),
        "target": target,
        "entry": entry,
        "log": _absolute(log),
        "fetch_callback": True,
    }
    if payload_source_root is not None:
        arguments["payload_source_root"] = _absolute(payload_source_root)
    result = _call(
        "transfer",
        fetcher=fetcher,
        **arguments,
    )
    return base64.b64decode(result["body_base64"], validate=True), result["receipt"]


def install_target(
    root: Path,
    manifest_path: Path,
    preparation: dict,
    target: dict,
    package: dict,
    *,
    payload_source_root: Path | None = None,
    fetcher=None,
) -> dict:
    arguments = {
        "root": _absolute(root),
        "manifest_path": _absolute(manifest_path),
        "preparation": preparation,
        "target": target,
        "package": package,
        "fetch_callback": fetcher is not None,
    }
    if payload_source_root is not None:
        arguments["payload_source_root"] = _absolute(payload_source_root)
    return _call(
        "install_target",
        fetcher=fetcher,
        **arguments,
    )


def refresh_topology(root: Path, evidence_root: Path, ended: str) -> None:
    _call(
        "refresh_topology",
        root=_absolute(root),
        evidence_root=_absolute(evidence_root),
        ended=ended,
    )


def write_discovery(
    root: Path,
    manifest_path: Path,
    preparation: dict,
    target: dict,
    transfers: list[dict],
    acquisition: dict,
) -> None:
    _call(
        "write_discovery",
        root=_absolute(root),
        manifest_path=_absolute(manifest_path),
        preparation=preparation,
        target=target,
        transfers=transfers,
        acquisition=acquisition,
    )


def verify_target(
    root: Path,
    target: dict,
    *,
    payload_source_root: Path | None = None,
) -> dict:
    arguments = {"root": _absolute(root), "target": target}
    if payload_source_root is not None:
        arguments["payload_source_root"] = _absolute(payload_source_root)
    return _call("verify_target", **arguments)


def main(*, fetcher=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["verify-preparation", "acquire", "verify-local"])
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--preparation-receipt", type=Path)
    parser.add_argument("--target", action="append", default=[])
    parser.add_argument(
        "--payload-source-root",
        type=Path,
        help="explicit directory mirroring ToS/source-witnesses for payload bytes",
    )
    parser.add_argument(
        "--allow-unbound-registry",
        action="store_true",
        help="Preparation inspection only; never accepted by acquire",
    )
    args = parser.parse_args()
    manifest_path = args.manifest.absolute()
    try:
        manifest_path.relative_to(ROOT.absolute())
    except ValueError as error:
        raise ValueError("registry manifest must be inside the repository root") from error
    if args.command == "acquire" and args.payload_source_root is None:
        raise ValueError(
            "acquire requires an explicit --payload-source-root; refusing to write inside the metadata checkout"
        )
    operations = {
        "verify-preparation": "registry.verify_preparation",
        "acquire": "registry.acquire",
        "verify-local": "registry.verify_local",
    }
    request = {
        "family": "registry",
        "operation": operations[args.command],
        "root": str(ROOT.absolute()),
        "manifest_path": str(manifest_path),
        "target_slugs": args.target,
        "allow_unbound_registry": args.allow_unbound_registry,
    }
    if args.preparation_receipt is not None:
        request["preparation_receipt_path"] = _absolute(args.preparation_receipt)
    if args.payload_source_root is not None:
        request["payload_source_root"] = _absolute(args.payload_source_root)
    try:
        result = _native_invoke(request, fetcher=fetcher)
    except NativeAcquisitionError as error:
        print(f"registry acquisition stopped: {error.message}", file=sys.stderr)
        return 1
    except (ValueError, KeyError, OSError) as error:
        print(f"registry acquisition stopped: {error}", file=sys.stderr)
        return 1
    if args.command == "verify-preparation":
        print(json.dumps(result, ensure_ascii=False, allow_nan=False))
    else:
        for item in result["results"]:
            print(json.dumps(item, ensure_ascii=False, allow_nan=False), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
