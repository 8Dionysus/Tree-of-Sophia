#!/usr/bin/env python3
"""Measure each discovery channel with a monotonic HTTP probe.

The receipt measures transport through at most the first 16 KiB of response
bytes. It does not measure source interpretation, rights review, or human time.
"""

from __future__ import annotations

import argparse
import copy
import json
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


METHOD = "monotonic-http-request-v1"
CLOCK = "python.time.perf_counter_ns"
TIMING_SCOPE = "request-through-first-16384-response-bytes"
READ_LIMIT = 16_384
USER_AGENT = "Tree-of-Sophia-open-work-discovery/1.0 (+source-research)"


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def measure_url(url: str, *, timeout_seconds: float) -> dict[str, Any]:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    started_at = _utc_now()
    started_ns = time.perf_counter_ns()
    outcome = "success"
    http_status: int | None = None
    response_bytes = 0
    try:
        with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
            http_status = response.status
            response_bytes = len(response.read(READ_LIMIT))
    except urllib.error.HTTPError as exc:
        outcome = "http-error"
        http_status = exc.code
        response_bytes = len(exc.read(READ_LIMIT))
    except (OSError, urllib.error.URLError):
        outcome = "transport-error"
    ended_ns = time.perf_counter_ns()
    ended_at = _utc_now()
    elapsed_seconds = max(round((ended_ns - started_ns) / 1_000_000_000, 6), 0.000001)
    return {
        "method": METHOD,
        "clock": CLOCK,
        "timing_scope": TIMING_SCOPE,
        "probe_url": url,
        "started_at": started_at,
        "ended_at": ended_at,
        "elapsed_seconds": elapsed_seconds,
        "outcome": outcome,
        "http_status": http_status,
        "response_bytes_observed": response_bytes,
    }


def build_receipt(discovery_path: Path, *, timeout_seconds: float) -> dict[str, Any]:
    discovery = json.loads(discovery_path.read_text(encoding="utf-8"))
    channels = discovery.get("channels")
    if not isinstance(channels, list) or not channels:
        raise ValueError("discovery record must contain at least one channel")
    measurements = []
    for channel in channels:
        if not isinstance(channel, dict):
            raise ValueError("every discovery channel must be an object")
        channel_id = channel.get("channel_id")
        probe_url = channel.get("endpoint_url")
        if not isinstance(channel_id, str) or not channel_id:
            raise ValueError("every discovery channel must have a channel_id")
        if not isinstance(probe_url, str) or not probe_url:
            raise ValueError(f"{channel_id}: endpoint_url must be a non-empty URL")
        measurements.append(
            {
                "channel_id": channel_id,
                "measurement": measure_url(probe_url, timeout_seconds=timeout_seconds),
            }
        )
    discovery_id = discovery.get("discovery_id")
    if not isinstance(discovery_id, str) or not discovery_id.startswith("tos.discovery."):
        raise ValueError("discovery record must contain a valid discovery_id")
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/open-work-channel-timing-receipt.schema.json",
        "schema_version": "tos_open_work_channel_timing_receipt_v1",
        "timing_id": "open-work-channel-timing." + discovery_id.removeprefix("tos.discovery."),
        "discovery_ref": discovery_path.as_posix(),
        "discovery_id": discovery_id,
        "measured_at": _utc_now(),
        "measurements": measurements,
        "claim_limit": (
            "monotonic transport timing through the first 16384 response bytes only; "
            "not research, interpretation, rights-review, or human elapsed time"
        ),
        "record_version": 1,
    }


def build_instrumented_discovery(
    source: dict[str, Any],
    receipt: dict[str, Any],
) -> dict[str, Any]:
    output = copy.deepcopy(source)
    measurements = {
        entry["channel_id"]: entry["measurement"]
        for entry in receipt["measurements"]
    }
    for channel in output["channels"]:
        measurement = measurements[channel["channel_id"]]
        channel["queried_at"] = measurement["started_at"]
        channel["elapsed_seconds"] = measurement["elapsed_seconds"]
    for comparison in output["channel_comparison"]:
        measurement = measurements[comparison["channel_id"]]
        comparison["machine_seconds"] = measurement["elapsed_seconds"]
        comparison["notes"] = (
            comparison["notes"].split(" Timer sentinel", 1)[0]
            .split(" Timing sentinel", 1)[0]
            .split(" Per-channel timers", 1)[0]
            .split(" Zero timing", 1)[0]
            + " Machine time is the automatic monotonic HTTP measurement; "
            "human_minutes=0 means no real-human review was performed."
        )
    measurement_rows = list(measurements.values())
    output["started_at"] = measurement_rows[0]["started_at"]
    output["ended_at"] = measurement_rows[-1]["ended_at"]
    return output


def retarget_timing_receipt(
    receipt: dict[str, Any],
    *,
    discovery_id: str,
    discovery_ref: str,
) -> dict[str, Any]:
    """Bind a timing receipt to the superseding discovery it instruments."""
    output = copy.deepcopy(receipt)
    output["timing_id"] = "open-work-channel-timing." + discovery_id.removeprefix("tos.discovery.")
    output["discovery_id"] = discovery_id
    output["discovery_ref"] = discovery_ref
    return output


def build_superseding_discovery(
    source: dict[str, Any],
    receipt: dict[str, Any],
    *,
    discovery_id: str,
    supersedes_ref: str,
    provenance_event_ref: str,
) -> dict[str, Any]:
    source_discovery_id = source.get("discovery_id")
    if not isinstance(source_discovery_id, str) or not source_discovery_id:
        raise ValueError("input discovery must contain a non-empty discovery_id")
    if supersedes_ref != source_discovery_id:
        raise ValueError(
            "supersedes_ref must match the input discovery discovery_id "
            f"{source_discovery_id!r}"
        )
    output = build_instrumented_discovery(source, receipt)
    output["discovery_id"] = discovery_id
    output["provenance_event_refs"] = [provenance_event_ref]
    output["supersedes_discovery_ref"] = supersedes_ref
    output["record_version"] = int(source.get("record_version", 1)) + 1
    return output


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("discovery", type=Path)
    parser.add_argument("--timeout-seconds", type=float, default=20.0)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--superseding-output", type=Path)
    parser.add_argument("--instrumented-output", type=Path)
    parser.add_argument("--new-discovery-id")
    parser.add_argument("--supersedes-ref")
    parser.add_argument("--provenance-event-ref")
    args = parser.parse_args()
    if args.superseding_output is not None and args.instrumented_output is not None:
        parser.error("--superseding-output and --instrumented-output are mutually exclusive")
    receipt = build_receipt(args.discovery, timeout_seconds=args.timeout_seconds)
    if args.superseding_output is not None:
        required = {
            "--new-discovery-id": args.new_discovery_id,
            "--supersedes-ref": args.supersedes_ref,
            "--provenance-event-ref": args.provenance_event_ref,
        }
        missing = [name for name, value in required.items() if not value]
        if missing:
            parser.error(f"{', '.join(missing)} required with --superseding-output")
        receipt = retarget_timing_receipt(
            receipt,
            discovery_id=args.new_discovery_id,
            discovery_ref=args.superseding_output.as_posix(),
        )
    rendered = json.dumps(receipt, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    if args.output is None:
        print(rendered, end="")
    else:
        args.output.write_text(rendered, encoding="utf-8")
    if args.superseding_output is not None:
        source = json.loads(args.discovery.read_text(encoding="utf-8"))
        try:
            superseding = build_superseding_discovery(
                source,
                receipt,
                discovery_id=args.new_discovery_id,
                supersedes_ref=args.supersedes_ref,
                provenance_event_ref=args.provenance_event_ref,
            )
        except ValueError as exc:
            parser.error(str(exc))
        args.superseding_output.write_text(
            json.dumps(superseding, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
    if args.instrumented_output is not None:
        source = json.loads(args.discovery.read_text(encoding="utf-8"))
        instrumented = build_instrumented_discovery(source, receipt)
        args.instrumented_output.write_text(
            json.dumps(instrumented, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
