"""Imported native diagnostic report and mechanical terminal rendering."""
from __future__ import annotations

from pathlib import Path
from typing import Any


def doctor_report(*, tos_root: str | Path | None = None,
                  profile: str = 'standalone', require_mcp: bool = False,
                  native_prefix: str | Path | None = None) -> dict[str, Any]:
    """Return the existing installed Rust diagnostic, including not-ready reports.

    Native software is selected independently through native_prefix or
    TOS_NATIVE_PREFIX. Omitted data uses the native installed runtime-data route;
    the adapter never discovers another source repository or executes an oracle.
    """
    if profile not in {'standalone', 'abyssos'}:
        raise ValueError(f'unknown access profile: {profile}')
    if type(require_mcp) is not bool:
        raise TypeError('require_mcp must be a boolean')
    from .native_io import native_packets
    arguments = []
    if tos_root is not None:
        arguments += ['--root', str(Path(tos_root).expanduser().absolute())]
    arguments += ['doctor', '--json', '--profile', profile,
                  '--require-mcp', 'true' if require_mcp else 'false']
    packets = list(native_packets(arguments, prefix=native_prefix,
                                 frame_cap=65536, input_cap=1,
                                 valid_returncodes=(0, 1)))
    if len(packets) != 1 or type(packets[0]) is not dict:
        raise ValueError('native doctor did not return one full report')
    report = packets[0]
    if (report.get('schema_version') != 'tos_access_doctor_report_v1'
            or report.get('profile') != profile or type(report.get('ok')) is not bool
            or type(report.get('checks')) is not list
            or type(report.get('required_failures')) is not list):
        raise ValueError('native doctor report contract differs')
    return report


def reference_doctor_report(**arguments):
    """Explicit historical diagnostic oracle; never selected as fallback."""
    from .reference_doctor import reference_doctor_report as oracle
    return oracle(**arguments)


def web_root_for(core) -> Path | None:
    from .locations import web_root
    return web_root()


def render_doctor(report: dict[str, Any]) -> str:
    lines = [f"Tree of Sophia access: {'ready' if report['ok'] else 'not ready'} ({report['profile']})"]
    for item in report["checks"]:
        mark = "ok" if item["ok"] else ("optional" if not item["required"] else "fail")
        lines.append(f"[{mark}] {item['check_id']}")
    if report["required_failures"]:
        lines.append("Required failures: " + ", ".join(report["required_failures"]))
    return "\n".join(lines)
