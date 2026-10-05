#!/usr/bin/env python3
from __future__ import annotations

# Executable compatibility only: installed native code owns production behavior.
# Importable functions below remain explicit comparison APIs for retained tests.
if __name__ == "__main__":
    import argparse as _argparse
    import os as _os
    from pathlib import Path as _Path
    import shutil as _shutil
    import sys as _sys
    _parser = _argparse.ArgumentParser()
    _args = _parser.parse_args()
    _selected = _os.environ.get("TOS_OPS_MECHANICS_EXECUTOR")
    _executable = _selected if _selected is not None else _shutil.which("tos-ops-mechanics-plan")
    if not _executable:
        print("[error] install tos-ops-mechanics-plan or set TOS_OPS_MECHANICS_EXECUTOR", file=_sys.stderr)
        raise SystemExit(1)
    _argv = [_executable, "--repo-root", str(_Path(__file__).resolve().parents[5]), "--public-mirror-sync"]
    try:
        _os.execv(_executable, _argv)
    except OSError as _error:
        print(f"[error] cannot execute native public-mirror-sync: {_error}", file=_sys.stderr)
        raise SystemExit(1)
    raise SystemExit(1)


from tree_example_sync import main


if __name__ == "__main__":
    raise SystemExit(main())
