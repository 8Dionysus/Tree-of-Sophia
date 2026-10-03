#!/usr/bin/env python3
"""Thin compatibility facade for native bounded source acquisition."""
from __future__ import annotations
import argparse
import base64
from dataclasses import dataclass
import json
from pathlib import Path
import sys
from typing import Any, Callable
try:
    from acquisition_native import invoke, NativeAcquisitionError
except ModuleNotFoundError:
    from scripts.acquisition_native import invoke, NativeAcquisitionError
REPO_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_SCHEMA = Path("ToS/contracts/acquisition-batch.schema.json")
MAX_PAYLOAD_BYTES = 300 * 1024 * 1024
class AcquisitionBatchError(ValueError):
    """The native acquisition owner refused the request."""
class SourceFetchError(AcquisitionBatchError):
    pass
class SourceIntegrityError(AcquisitionBatchError):
    pass
@dataclass(frozen=True)
class BatchContext:
    repo_root: Path
    manifest_path: Path
    manifest_ref: str
    manifest_sha256: str
    raw_manifest: bytes
    manifest: dict[str, Any]
@dataclass(frozen=True)
class PayloadSelection:
    selection: dict[str, Any]
    payload: dict[str, Any]
    @property
    def item_ref(self): return self.payload["item_ref"]
    @property
    def file_ref(self): return self.payload["file_ref"]
    @property
    def destination_ref(self): return f"{self.payload['item_root_ref']}/{self.payload['relative_path']}"
    @property
    def custody_key(self): return self.item_ref, self.file_ref, self.destination_ref
Fetcher = Callable[[dict[str, Any]], bytes]
def _request(operation, *, repo_root=REPO_ROOT, fetcher=None, **fields):
    request = {"family":"batch", "operation":operation, "repo_root":str(Path(repo_root).expanduser())}
    for key, value in fields.items():
        request[key] = str(value.expanduser()) if isinstance(value, Path) else value
    try:
        return invoke(request, fetcher=fetcher)
    except NativeAcquisitionError as exc:
        error = {"SourceFetchError":SourceFetchError, "SourceIntegrityError":SourceIntegrityError}.get(exc.error, AcquisitionBatchError)
        raise error(exc.message) from exc

def load_manifest(manifest_path, *, repo_root=REPO_ROOT, expected_sha256=None):
    path = Path(manifest_path).expanduser()
    if not path.is_absolute(): path = Path(repo_root) / path
    value = _request("load_manifest", repo_root=repo_root, manifest_path=path, expected_manifest_sha256=expected_sha256)
    # Preserve the exact bytes already checked by the native owner.
    return BatchContext(Path(value["repo_root"]), Path(value["manifest_path"]), value["manifest_ref"], value["manifest_sha256"], base64.b64decode(value["raw_manifest_base64"], validate=True), value["manifest"])

def prepare_batch(*, manifest_path, metadata_root, output_root, repo_root=REPO_ROOT, expected_manifest_sha256=None):
    return _request("prepare", repo_root=repo_root, manifest_path=Path(manifest_path), metadata_root=Path(metadata_root), output_root=Path(output_root), expected_manifest_sha256=expected_manifest_sha256)

def acquire_batch(*, manifest_path, metadata_root, output_root, repo_root=REPO_ROOT, expected_manifest_sha256=None, fetcher=None, max_attempts=2):
    return _request("acquire", repo_root=repo_root, manifest_path=Path(manifest_path), metadata_root=Path(metadata_root), output_root=Path(output_root), expected_manifest_sha256=expected_manifest_sha256, fetcher=fetcher, max_attempts=max_attempts)

def verify_local(*, output_root, repo_root=REPO_ROOT):
    return _request("verify_local", repo_root=repo_root, output_root=Path(output_root))

def measure_storage(output_root):
    return _request("measure_storage", output_root=Path(output_root))

def _payloads(context):
    return sorted((PayloadSelection(s,p) for s in context.manifest["selection"] for p in s["payload_files"]), key=lambda p:p.custody_key)

def _records(context):
    rows = {}
    for s in context.manifest["selection"]:
        for r in s["records"]: rows.setdefault(r["ref"], (s,r))
    return [rows[key] for key in sorted(rows)]

def _payload_custody_key(row):
    values = tuple(row.get(k) for k in ("item_ref","file_ref","destination_ref"))
    return values if all(isinstance(v,str) for v in values) else None

def _canonical(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)+"\n").encode()

def _verify_prepared_output(context, output, *, require_private_roots=True):
    _request("verify_prepared_output", repo_root=context.repo_root, manifest_path=context.manifest_path, output_root=Path(output), expected_manifest_sha256=context.manifest_sha256, require_private_roots=require_private_roots)

def _common_parser(parser: argparse.ArgumentParser, *, output_required: bool = True) -> None:
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--metadata-root", type=Path, required=output_required)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument("--expected-manifest-sha256", required=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    prepare_parser = subparsers.add_parser("prepare")
    _common_parser(prepare_parser)
    acquire_parser = subparsers.add_parser("acquire")
    _common_parser(acquire_parser)
    acquire_parser.add_argument("--max-attempts", type=int, default=2)
    verify_parser = subparsers.add_parser("verify-local")
    verify_parser.add_argument("--output-root", type=Path, required=True)
    verify_parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    args = parser.parse_args(argv)
    try:
        if args.command == "prepare":
            result = prepare_batch(
                manifest_path=args.manifest,
                metadata_root=args.metadata_root,
                output_root=args.output_root,
                repo_root=args.repo_root,
                expected_manifest_sha256=args.expected_manifest_sha256,
            )
        elif args.command == "acquire":
            result = acquire_batch(
                manifest_path=args.manifest,
                metadata_root=args.metadata_root,
                output_root=args.output_root,
                repo_root=args.repo_root,
                expected_manifest_sha256=args.expected_manifest_sha256,
                max_attempts=args.max_attempts,
            )
        else:
            result = verify_local(output_root=args.output_root, repo_root=args.repo_root)
    except AcquisitionBatchError as exc:
        print(f"acquisition-batch: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    if args.command == "acquire" and result.get("status") != "acquired-not-admitted":
        return 1
    if args.command == "verify-local" and result.get("status") != "verified":
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
