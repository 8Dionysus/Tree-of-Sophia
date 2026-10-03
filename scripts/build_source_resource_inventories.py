#!/usr/bin/env python3
"""Wire facade for the native source-resource inventory owner.

The native owner reads exact Item bindings, profiles payloads, validates the
inventory schema, and performs bounded build/check writes. This module keeps
the historical Python call shapes without a parser or fallback implementation.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
from typing import Any

try:
    from acquisition_native import invoke as _native_invoke, NativeAcquisitionError
except ModuleNotFoundError:  # pragma: no cover - package import
    from scripts.acquisition_native import invoke as _native_invoke, NativeAcquisitionError


DEFAULT_EVENT_DATE = "2026-07-28"
DEFAULT_PLAIN_TEXT_PROFILE = "plain_utf8_file_v1"
SOURCE_ROOT = Path("ToS/source-witnesses")


class InventoryBuildError(RuntimeError):
    """The native inventory owner refused a payload, manifest, or operation."""


def _invoke_inventory(operation: str, **fields: Any) -> Any:
    request = {"family": "inventory", "operation": operation, **fields}
    try:
        return _native_invoke(request)
    except NativeAcquisitionError as exc:
        raise InventoryBuildError(exc.message) from exc


def inventory_metadata() -> dict[str, Any]:
    value = _invoke_inventory("metadata")
    if not isinstance(value, dict):
        raise InventoryBuildError("native inventory metadata response is invalid")
    return value


def inventory_authority_boundary(version: str) -> str:
    try:
        return inventory_metadata()["authority_boundaries"][str(version)]
    except (KeyError, TypeError) as exc:
        raise ValueError("unsupported resource inventory generator version") from exc


def build_file_inventory(
    payload_path: Path,
    payload_entry: dict[str, Any],
    *,
    plain_text_profile: str = DEFAULT_PLAIN_TEXT_PROFILE,
) -> dict[str, Any]:
    """Return one native file profile for an exact path and manifest entry."""
    value = _invoke_inventory(
        "file",
        payload_path=str(Path(payload_path).expanduser()),
        payload_entry=payload_entry,
        plain_text_profile=plain_text_profile,
    )
    if not isinstance(value, dict):
        raise InventoryBuildError("native file inventory response is invalid")
    return value


def build_inventory(
    *,
    repo_root: Path,
    manifest_path: Path,
    payload_source_root: Path,
    event_date: str,
    plain_text_profile: str = DEFAULT_PLAIN_TEXT_PROFILE,
) -> dict[str, Any] | None:
    """Preserve the imported per-Item API while delegating all work to Rust."""
    repo = Path(repo_root).expanduser().absolute()
    manifest = Path(manifest_path).expanduser().absolute()
    payloads = Path(payload_source_root).expanduser().absolute()
    try:
        manifest_ref = manifest.relative_to(repo).as_posix()
    except ValueError as exc:
        raise InventoryBuildError("Item manifest is outside the selected repository") from exc
    value = _invoke_inventory(
        "item",
        repo_root=str(repo),
        item_manifest_ref=manifest_ref,
        payload_source_root=str(payloads),
        event_date=event_date,
        plain_text_profile=plain_text_profile,
    )
    if value is None:
        return None
    if not isinstance(value, dict):
        raise InventoryBuildError("native Item inventory response is invalid")
    return value


def render_inventory(payload: dict[str, Any]) -> str:
    """Compatibility renderer; the production CLI writes from the native owner."""
    return json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=False) + "\n"


def main() -> int:
    repo_default = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(
        description="Build text-free resource inventories from local source payloads."
    )
    parser.add_argument("--repo-root", type=Path, default=repo_default)
    parser.add_argument(
        "--payload-source-root",
        type=Path,
        help=(
            "Source-witness root containing local item payloads; defaults to the "
            "current checkout's ToS/source-witnesses."
        ),
    )
    parser.add_argument(
        "--event-date",
        default=DEFAULT_EVENT_DATE,
        help="Date suffix for new provenance event refs (YYYY-MM-DD).",
    )
    parser.add_argument(
        "--plain-text-profile",
        choices=("plain_utf8_file_v1", "plain_text_v1"),
        default=DEFAULT_PLAIN_TEXT_PROFILE,
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if len(args.event_date) != 10 or any(
        not (char.isdigit() if index not in (4, 7) else char == "-")
        for index, char in enumerate(args.event_date)
    ):
        parser.error("--event-date must use YYYY-MM-DD")

    repo_root = args.repo_root.expanduser().resolve()
    payload_source_root = (
        args.payload_source_root.expanduser().resolve()
        if args.payload_source_root
        else (repo_root / SOURCE_ROOT).resolve()
    )
    try:
        result = _invoke_inventory(
            "build",
            repo_root=str(repo_root),
            payload_source_root=str(payload_source_root),
            event_date=args.event_date,
            plain_text_profile=args.plain_text_profile,
            check=args.check,
        )
    except InventoryBuildError as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 2

    processed = result["processed"]
    skipped = result["skipped"]
    if processed == 0:
        print(
            "[skip] no local source payloads were available for resource inventory",
            file=sys.stderr,
        )
        return 0
    drift = result["drift"]
    if drift:
        for path in drift:
            print(f"[drift] {path}", file=sys.stderr)
        return 1
    verb = "verified" if args.check else "wrote"
    print(
        f"[ok] {verb} {processed} source resource inventories"
        + (f"; skipped {skipped} absent local payload set(s)" if skipped else "")
    )
    print(
        "[scope] Text-free resource metadata, counts, geometry and one-way navigation fingerprints."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
