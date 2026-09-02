from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
from typing import Any

from .core import ToSAccessCore

SUPPORTED_INDEX_SCHEMAS = {"tos_corpus_index_v1"}
SUPPORTED_GRAPH_SCHEMAS = {
    "tos_philosophy_graph_projection_v1",
    "tos_philosophy_graph_projection_v2",
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def web_root_for(core: ToSAccessCore) -> Path | None:
    candidates = [
        core.tos_root / "access/web/dist",
        Path(__file__).resolve().parent / "web_dist",
    ]
    return next((path for path in candidates if (path / "assets/tos-graph.js").is_file()), None)


def contract_root_for(core: ToSAccessCore) -> Path | None:
    candidates = [
        core.tos_root / "access/contracts",
        Path(__file__).resolve().parent / "runtime_data/access/contracts",
    ]
    return next((path for path in candidates if (path / "runtime-manifest.v1.json").is_file()), None)


def _check(check_id: str, ok: bool, *, required: bool = True, **details: Any) -> dict[str, Any]:
    return {"check_id": check_id, "ok": ok, "required": required, **details}


def doctor_report(
    *,
    tos_root: str | Path | None = None,
    profile: str = "standalone",
    require_mcp: bool = False,
) -> dict[str, Any]:
    if profile not in {"standalone", "abyssos"}:
        raise ValueError(f"unknown access profile: {profile}")
    core = ToSAccessCore.discover(tos_root=tos_root)
    checks: list[dict[str, Any]] = []

    checks.append(_check("corpus-index-present", core.index_exists(), path=core.index_path.as_posix()))
    if core.index_exists():
        index = core.index()
        checks.append(
            _check(
                "corpus-index-schema",
                index.get("schema_version") in SUPPORTED_INDEX_SCHEMAS,
                schema_version=index.get("schema_version"),
                sha256=sha256_file(core.index_path),
            )
        )

    checks.append(
        _check(
            "philosophy-graph-present",
            core.philosophy_projection_exists(),
            path=core.philosophy_graph_projection_path.as_posix(),
        )
    )
    if core.philosophy_projection_exists():
        graph = core.philosophy_projection()
        checks.append(
            _check(
                "philosophy-graph-schema",
                graph.get("schema_version") in SUPPORTED_GRAPH_SCHEMAS,
                schema_version=graph.get("schema_version"),
                sha256=sha256_file(core.philosophy_graph_projection_path),
            )
        )
        view_packet = core.philosophy_view(str(graph.get("views", [{}])[0].get("view_id") or ""))
        checks.append(
            _check(
                "graph-view-materialization",
                view_packet.get("node_count", 0) > 0 and view_packet.get("edge_count", 0) > 0,
                view_id=view_packet.get("view", {}).get("view_id"),
                node_count=view_packet.get("node_count"),
                edge_count=view_packet.get("edge_count"),
            )
        )

    contract_root = contract_root_for(core)
    checks.append(
        _check(
            "runtime-contracts",
            contract_root is not None
            and all(
                (contract_root / name).is_file()
                for name in ("runtime-manifest.v1.json", "runtime-data.v1.json", "web-actions.v1.json")
            ),
            path=contract_root.as_posix() if contract_root else None,
        )
    )

    web_root = web_root_for(core)
    checks.append(_check("web-assets", web_root is not None, path=web_root.as_posix() if web_root else None))

    mcp_available = importlib.util.find_spec("mcp") is not None
    checks.append(
        _check(
            "native-mcp-dependency",
            mcp_available,
            required=require_mcp,
            install_hint="python -m pip install 'tree-of-sophia-access[mcp]'" if not mcp_available else None,
        )
    )

    abyssos_root_value = os.environ.get("TOS_ABYSSOS_ROOT", "").strip()
    abyssos_root = Path(abyssos_root_value).expanduser().resolve() if abyssos_root_value else None
    abyssos_available = bool(abyssos_root and (abyssos_root / "abyss-stack").is_dir())
    checks.append(
        _check(
            "abyssos-integration",
            abyssos_available,
            required=profile == "abyssos",
            available=abyssos_available,
            configured_root=abyssos_root.as_posix() if abyssos_root else None,
            install_hint="set TOS_ABYSSOS_ROOT to an AbyssOS root containing abyss-stack" if not abyssos_available else None,
            posture="optional-adapter",
        )
    )

    required_failures = [item["check_id"] for item in checks if item["required"] and not item["ok"]]
    return {
        "schema_version": "tos_access_doctor_report_v1",
        "profile": profile,
        "ok": not required_failures,
        "tos_root": core.tos_root.as_posix(),
        "checks": checks,
        "required_failures": required_failures,
        "authority_limit": "This report proves local access mechanics only; ToS sources own meaning and review.",
    }


def render_doctor(report: dict[str, Any]) -> str:
    lines = [f"Tree of Sophia access: {'ready' if report['ok'] else 'not ready'} ({report['profile']})"]
    for item in report["checks"]:
        mark = "ok" if item["ok"] else ("optional" if not item["required"] else "fail")
        lines.append(f"[{mark}] {item['check_id']}")
    if report["required_failures"]:
        lines.append("Required failures: " + ", ".join(report["required_failures"]))
    return "\n".join(lines)
