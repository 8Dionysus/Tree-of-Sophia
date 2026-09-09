#!/usr/bin/env python3
"""Validate the ToS segmented KAG provider through an exact aoa-kag pin.

This adapter owns only the ToS consumer boundary.  The segmented-family
schema, digest rules, and bounded reader remain in aoa-kag; ToS selects the
provider checkout by immutable commit and refuses an unbound or dirty source.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import subprocess
import sys
import types
from pathlib import Path
from typing import Any, Mapping


REPO_ROOT = Path(__file__).resolve().parents[1]
PIN_PATH = REPO_ROOT / "kag" / "provider_pin.json"
MANIFEST_PATH = REPO_ROOT / "kag" / "indexes" / "index_family.manifest.json"
SEGMENTED_SCHEMA = "aoa-repo-local-kag-segmented-family-v1"
PIN_SCHEMA = "tos-aoa-kag-segmented-provider-pin-v1"


class ProviderValidationError(RuntimeError):
    """Raised when the pinned segmented consumer cannot be admitted."""


def _read_object(path: Path, *, label: str) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ProviderValidationError(f"{label} is unreadable: {path}") from exc
    if not isinstance(payload, dict):
        raise ProviderValidationError(f"{label} must be a JSON object: {path}")
    return payload


def read_pin(path: Path = PIN_PATH) -> dict[str, Any]:
    pin = _read_object(path, label="ToS aoa-kag provider pin")
    if pin.get("schema_version") != PIN_SCHEMA:
        raise ProviderValidationError("ToS aoa-kag provider pin schema is invalid")
    provider = pin.get("provider")
    if not isinstance(provider, Mapping):
        raise ProviderValidationError("ToS aoa-kag provider pin needs provider")
    if provider.get("repository") != "8Dionysus/aoa-kag":
        raise ProviderValidationError("ToS aoa-kag provider repository is not pinned")
    revision = provider.get("revision")
    if not isinstance(revision, str) or len(revision) != 40 or any(
        char not in "0123456789abcdef" for char in revision
    ):
        raise ProviderValidationError("ToS aoa-kag provider revision must be a 40-char lowercase commit")
    if provider.get("family_schema") != SEGMENTED_SCHEMA:
        raise ProviderValidationError("ToS aoa-kag provider pin must select segmented family v1")
    for field in ("builder", "validator", "reader"):
        if not isinstance(provider.get(field), str) or not provider[field].startswith(
            "scripts/"
        ):
            raise ProviderValidationError(f"ToS aoa-kag provider {field} route is invalid")
    rollback = pin.get("rollback")
    if not isinstance(rollback, Mapping):
        raise ProviderValidationError("ToS aoa-kag provider pin needs rollback posture")
    if rollback.get("selection") != "family_digest" or rollback.get("mode") != "explicit_dual_read":
        raise ProviderValidationError("ToS aoa-kag rollback must remain explicit and digest-selected")
    return pin


def _candidate_roots(repo_root: Path, explicit: str | None) -> tuple[Path, ...]:
    if explicit:
        return (Path(explicit).expanduser(),)
    return (
        repo_root / ".deps" / "aoa-kag",
        repo_root.parent / "aoa-kag",
    )


def _git_head(root: Path) -> str:
    result = subprocess.run(
        ("git", "-C", str(root), "rev-parse", "--verify", "HEAD"),
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise ProviderValidationError(f"aoa-kag checkout is not a readable Git repository: {root}")
    return result.stdout.strip()


def resolve_provider_root(
    pin: Mapping[str, Any],
    *,
    repo_root: Path,
    explicit_root: str | None = None,
) -> tuple[Path, str]:
    provider = pin["provider"]
    revision = str(provider["revision"])
    requested = explicit_root or os.environ.get("AOA_KAG_ROOT")
    candidates = _candidate_roots(repo_root, requested)
    checked: list[str] = []
    for candidate in candidates:
        candidate = candidate.resolve()
        checked.append(str(candidate))
        if not candidate.is_dir():
            continue
        try:
            head = _git_head(candidate)
        except ProviderValidationError:
            continue
        if head != revision:
            continue
        dirty = subprocess.run(
            ("git", "-C", str(candidate), "diff", "--quiet"),
            check=False,
        ).returncode != 0 or subprocess.run(
            ("git", "-C", str(candidate), "diff", "--cached", "--quiet"),
            check=False,
        ).returncode != 0
        if dirty:
            raise ProviderValidationError(
                f"aoa-kag checkout at pinned revision is dirty: {candidate}"
            )
        return candidate, revision
    raise ProviderValidationError(
        "aoa-kag pinned checkout unavailable or revision-mismatched; checked: "
        + ", ".join(checked)
    )


def _load_segmented_module(provider_root: Path, revision: str) -> types.ModuleType:
    """Load the provider reader without importing an unpinned local module."""
    package_name = f"_tos_pinned_aoa_kag_{revision[:12]}"
    package = types.ModuleType(package_name)
    package.__path__ = [str(provider_root / "scripts" / "repo_local")]  # type: ignore[attr-defined]
    sys.modules[package_name] = package
    for module_name in ("portable_family", "segmented_family"):
        full_name = f"{package_name}.{module_name}"
        spec = importlib.util.spec_from_file_location(
            full_name,
            provider_root / "scripts" / "repo_local" / f"{module_name}.py",
        )
        if spec is None or spec.loader is None:
            raise ProviderValidationError(f"cannot load pinned aoa-kag reader module: {module_name}")
        module = importlib.util.module_from_spec(spec)
        sys.modules[full_name] = module
        spec.loader.exec_module(module)
    return sys.modules[f"{package_name}.segmented_family"]


def validate_pinned_segmented_family(
    repo_root: Path = REPO_ROOT,
    *,
    pin_path: Path = PIN_PATH,
    manifest_path: Path = MANIFEST_PATH,
    provider_root: str | None = None,
) -> dict[str, Any]:
    pin = read_pin(pin_path)
    root, revision = resolve_provider_root(
        pin,
        repo_root=repo_root,
        explicit_root=provider_root,
    )
    reader = _load_segmented_module(root, revision)
    manifest = _read_object(manifest_path, label="segmented KAG family manifest")
    if manifest.get("schema_version") != SEGMENTED_SCHEMA:
        raise ProviderValidationError(
            "ToS segmented provider is not selected; v3/v4 family consumers remain fail-closed"
        )
    provider = pin["provider"]
    if manifest.get("family_identity", {}).get("schema_ref") != provider.get("schema_ref"):
        raise ProviderValidationError("segmented family schema reference does not match the ToS pin")
    if manifest.get("producer_identity", {}).get("route") != provider.get("producer"):
        raise ProviderValidationError("segmented family producer route does not match the ToS pin")
    reader.validate_segmented_manifest(manifest)
    counts = reader.validate_segmented_segments(repo_root, manifest)
    descriptors = manifest.get("segments")
    if not isinstance(descriptors, list) or not descriptors:
        raise ProviderValidationError("segmented family has no bounded descriptors")
    descriptor = descriptors[0]
    rows = reader.read_segment(
        repo_root,
        manifest,
        descriptor,
        request_bytes_max=manifest["budgets"]["request_bytes_max"],
    )
    if len(rows) != descriptor.get("records"):
        raise ProviderValidationError("bounded reader returned an unexpected record count")
    return {
        "provider_revision": revision,
        "provider_root": str(root),
        "family_digest": manifest["family_identity"]["content_digest"],
        "source_snapshot": manifest["family_identity"]["source_snapshot"],
        "schema_version": manifest["schema_version"],
        **counts,
        "bounded_probe": {
            "path": descriptor["path"],
            "records": len(rows),
            "bytes": descriptor["bytes"],
        },
        "full_materialization": "not_performed",
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument("--provider-root")
    parser.add_argument("--pin", type=Path, default=PIN_PATH)
    parser.add_argument("--manifest", type=Path, default=MANIFEST_PATH)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        result = validate_pinned_segmented_family(
            args.repo_root.resolve(),
            pin_path=args.pin if args.pin.is_absolute() else args.repo_root / args.pin,
            manifest_path=(
                args.manifest
                if args.manifest.is_absolute()
                else args.repo_root / args.manifest
            ),
            provider_root=args.provider_root,
        )
    except (OSError, ProviderValidationError, ValueError) as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1
    print(
        "[ok] validated ToS segmented KAG provider "
        f"revision={result['provider_revision']} "
        f"family={result['family_digest']} "
        f"segments={result['segments']} records={result['records']} "
        f"bytes={result['bytes']} bounded_probe={result['bounded_probe']['path']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
