#!/usr/bin/env python3
"""Execute candidate builds without reading the sealed evaluation manifest."""

from __future__ import annotations

import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from lxml import etree


LAB_DIR = Path(__file__).resolve().parent
REPO_ROOT = LAB_DIR.parents[3]
MANIFEST_PATH = LAB_DIR / "input-manifest.json"
BUILDER_PATH = LAB_DIR / "build_candidate.py"
PUBLIC_ROOT = LAB_DIR / "public-synthetic"
OBSERVATIONS_PATH = LAB_DIR / "run-observations.json"
SOURCE_RECEIPT_PATH = LAB_DIR / "source-run-receipt.json"
METHOD_FREEZE_PATH = LAB_DIR / "freeze-receipt-v6.json"


def canonical_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_path(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def utc_timestamp() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="microseconds").replace("+00:00", "Z")


def output_binding(output: Path) -> tuple[str, str]:
    resolved = output.resolve()
    try:
        return "lab", resolved.relative_to(LAB_DIR.resolve()).as_posix()
    except ValueError:
        return "absolute", str(resolved)


def write_json(path: Path, value: Any, mode: int = 0o600) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_bytes(canonical_bytes(value))
    os.chmod(temporary, mode)
    temporary.replace(path)


def measured_build(
    *,
    candidate: str,
    selection_kind: str,
    selection_id: str,
    output: Path,
) -> dict[str, Any]:
    output.parent.mkdir(parents=True, exist_ok=True)
    if output.exists():
        output.unlink()
    with tempfile.TemporaryDirectory(prefix="tos-generic-xml-time-") as temp_dir:
        timing_path = Path(temp_dir) / "time.txt"
        command = [
            "/usr/bin/time",
            "-f",
            "%e\t%M",
            "-o",
            str(timing_path),
            sys.executable,
            str(BUILDER_PATH),
            "--manifest",
            str(MANIFEST_PATH),
            "--candidate",
            candidate,
            f"--{selection_kind}",
            selection_id,
            "--output",
            str(output),
        ]
        completed = subprocess.run(
            command,
            cwd=REPO_ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            env={**os.environ, "LC_ALL": "C.UTF-8", "TZ": "UTC"},
        )
        timing_text = timing_path.read_text(encoding="utf-8").strip()
        wall_seconds: float | None = None
        max_rss_kib: int | None = None
        if timing_text:
            wall_raw, rss_raw = timing_text.splitlines()[-1].split("\t", 1)
            wall_seconds = float(wall_raw)
            max_rss_kib = int(rss_raw)

    output_scope, output_ref = output_binding(output)

    observation: dict[str, Any] = {
        "candidate": candidate,
        "selection_kind": selection_kind,
        "selection_id": selection_id,
        "exit_code": completed.returncode,
        "wall_seconds": wall_seconds,
        "max_rss_kib": max_rss_kib,
        "stdout_sha256": sha256_bytes(completed.stdout),
        "stderr_sha256": sha256_bytes(completed.stderr),
        "output_created": output.is_file(),
        "output_scope": output_scope,
        "output_ref": output_ref,
    }
    if output.is_file():
        observation.update(
            {
                "output_sha256": sha256_path(output),
                "output_bytes": output.stat().st_size,
            }
        )
    return observation


def output_summary(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    candidate = payload["candidate"]
    if candidate == "A":
        resource_count = len(payload["resources"])
        details: dict[str, Any] = {
            "resource_count": resource_count,
            "element_return_supported": payload["element_return_supported"],
        }
    elif candidate == "B":
        details = {
            "resource_count": payload["summary"]["resource_count"],
            "attribute_count": payload["summary"]["attribute_count"],
            "ordered_topology_sha256": payload["summary"]["ordered_topology_sha256"],
            "unordered_element_shape_sha256": payload["summary"]["unordered_element_shape_sha256"],
        }
    elif candidate == "C":
        details = dict(payload["summary"])
        details["generic_xml_owner_claimed"] = payload["generic_xml_owner_claimed"]
    else:
        details = {
            "owner_resource_count": payload["owner"]["summary"]["resource_count"],
            "owner_attribute_count": payload["owner"]["summary"]["attribute_count"],
            "ordered_topology_sha256": payload["owner"]["summary"]["ordered_topology_sha256"],
            "unordered_element_shape_sha256": payload["owner"]["summary"]["unordered_element_shape_sha256"],
            "projection_resource_count": payload["projection"]["summary"]["resource_count"],
            "verse_count": payload["projection"]["summary"]["verse_count"],
            "word_count": payload["projection"]["summary"]["word_count"],
            "word_counts_by_verse": payload["projection"]["summary"]["word_counts_by_verse"],
        }
    return {
        "candidate": candidate,
        "file_sha256": (
            payload["file_binding"]["sha256"]
            if candidate != "BC"
            else payload["owner"]["file_binding"]["sha256"]
        ),
        "output_sha256": sha256_path(path),
        "output_bytes": path.stat().st_size,
        **details,
    }


def main() -> int:
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    method_freeze = json.loads(METHOD_FREEZE_PATH.read_text(encoding="utf-8"))
    method_freeze_sha256 = sha256_path(METHOD_FREEZE_PATH)
    method_freeze_ref = METHOD_FREEZE_PATH.relative_to(REPO_ROOT).as_posix()
    if method_freeze.get("self_sha256") not in (None, method_freeze_sha256):
        raise RuntimeError("method freeze self digest is not stable")
    output_run_started_at = utc_timestamp()
    fixture_ids = [fixture["id"] for fixture in manifest["fixtures"]]
    security_ids = [fixture["id"] for fixture in manifest["security_fixtures"]]
    observations: list[dict[str, Any]] = []

    for run_number in (1, 2):
        for fixture_id in fixture_ids:
            candidates = ["A", "B"]
            if fixture_id == "PC1-uxlc-shape":
                candidates.extend(["C", "BC"])
            for candidate in candidates:
                output = (
                    PUBLIC_ROOT
                    / f"run-{run_number}"
                    / candidate.lower()
                    / f"{fixture_id}.json"
                )
                observations.append(
                    measured_build(
                        candidate=candidate,
                        selection_kind="fixture",
                        selection_id=fixture_id,
                        output=output,
                    )
                )

    for candidate in ("A", "B", "C", "BC"):
        for security_id in security_ids:
            output = PUBLIC_ROOT / "negative" / candidate.lower() / f"{security_id}.json"
            observations.append(
                measured_build(
                    candidate=candidate,
                    selection_kind="fixture",
                    selection_id=security_id,
                    output=output,
                )
            )

    observations.append(
        measured_build(
            candidate="C",
            selection_kind="fixture",
            selection_id="P1-no-namespace",
            output=PUBLIC_ROOT / "negative" / "c" / "N5-genericity-P1.json",
        )
    )

    private_root = Path(manifest["private_output_root"])
    source_ids = [
        source_id
        for source_id, source in manifest["exact_sources"].items()
        if Path(source["path"]).is_file()
    ]
    source_observations: list[dict[str, Any]] = []
    for source_id in source_ids:
        for run_number in (1, 2):
            for candidate in ("A", "B", "C", "BC"):
                output = private_root / "outputs" / source_id / f"run-{run_number}" / f"{candidate.lower()}.json"
                observation = measured_build(
                    candidate=candidate,
                    selection_kind="source",
                    selection_id=source_id,
                    output=output,
                )
                observations.append(observation)
                source_observations.append(observation)

    source_summaries: dict[str, Any] = {}
    for source_id in source_ids:
        source_path = Path(manifest["exact_sources"][source_id]["path"])
        outputs = {
            candidate: output_summary(
                private_root / "outputs" / source_id / "run-1" / f"{candidate.lower()}.json"
            )
            for candidate in ("A", "B", "C", "BC")
        }
        source_summaries[source_id] = {
            "source_sha256": sha256_path(source_path),
            "source_bytes": source_path.stat().st_size,
            "source_mode": f"{source_path.stat().st_mode & 0o777:04o}",
            "candidates": outputs,
        }

    observations_payload = {
        "schema_version": "tos_generic_xml_resource_inventory_lab_run_observations_v1",
        "lab_id": manifest["lab_id"],
        "environment": {
            "python": platform.python_version(),
            "lxml": etree.LXML_VERSION,
            "libxml2_compiled": etree.LIBXML_COMPILED_VERSION,
            "libxml2_runtime": etree.LIBXML_VERSION,
            "platform": platform.platform(),
            "locale": "C.UTF-8",
            "timezone": "UTC",
        },
        "process_count": len(observations),
        "output_run_started_at": output_run_started_at,
        "method_freeze_ref": method_freeze_ref,
        "method_freeze_sha256": method_freeze_sha256,
        "processes": observations,
        "authority_boundary": "timing, memory, exit and output-fixity observations only; no content or acceptance authority",
    }
    write_json(OBSERVATIONS_PATH, observations_payload, mode=0o644)

    source_receipt = {
        "schema_version": "tos_generic_xml_resource_inventory_source_run_receipt_v1",
        "lab_id": manifest["lab_id"],
        "source_payloads_copied_to_tracked_lab": False,
        "source_candidate_outputs_tracked": False,
        "source_text_included": False,
        "sources": source_summaries,
        "source_process_count": len(source_observations),
        "direct_monetary_cost": 0,
        "authority_boundary": "text-free source-visible mechanics only; no source-text, language, translation, semantic, graph, canon, rights or publication authority",
    }
    write_json(SOURCE_RECEIPT_PATH, source_receipt, mode=0o644)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
