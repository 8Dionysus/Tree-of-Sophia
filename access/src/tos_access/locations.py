"""Independent locations for installed software and explicitly selected data."""
from __future__ import annotations

import os
from pathlib import Path


PACKAGE_ROOT = Path(__file__).resolve().parent
ACCESS_ROOT = PACKAGE_ROOT.parents[1]


def _source_checkout() -> bool:
    return PACKAGE_ROOT == ACCESS_ROOT / "src/tos_access" and (ACCESS_ROOT / "pyproject.toml").is_file()


def data_root(explicit: str | Path | None = None) -> Path:
    """Select data without discovering an unrelated checkout through cwd.

    TOS_ROOT/AOA_TOS_ROOT remain explicit compatibility inputs. They do not
    select executable code, web assets or the reader's API contracts.
    A missing explicit selection stays missing instead of falling back.
    """
    selected = explicit or os.environ.get("TOS_DATA_ROOT") or os.environ.get("TOS_ROOT") or os.environ.get("AOA_TOS_ROOT")
    return Path(selected).expanduser().resolve() if selected else PACKAGE_ROOT / "runtime_data"


def program_path(relative: str | Path) -> Path:
    """Resolve a versioned software subject, never a data-root override."""
    relative = Path(relative)
    if relative.is_absolute() or ".." in relative.parts:
        raise ValueError("program subject must be repository-relative")
    packaged = PACKAGE_ROOT / "runtime_data" / relative
    if packaged.is_file():
        return packaged
    # Source/editable development reads only this package's own checkout.
    # It never searches the current directory or the selected corpus root.
    source = ACCESS_ROOT.parent / relative
    if _source_checkout() and source.is_file():
        return source
    return packaged


def web_root() -> Path | None:
    """Executable browser assets travel with software, not with a corpus."""
    candidates = [PACKAGE_ROOT / "web_dist"]
    if _source_checkout():
        candidates.append(ACCESS_ROOT / "web/dist")
    return next((root for root in candidates if (root / "assets/tos-graph.js").is_file()), None)
