#!/usr/bin/env python3
"""Verify the initial Rust binary installs and runs outside the checkout."""
from __future__ import annotations

import os
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
PROBE = "tos-workspace-probe"
EXPECTED = "tos-rust-workspace-scaffold-v1"


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="tos-rust-install-") as install_root:
        subprocess.run(
            ["cargo", "install", "--path", "rust/crates/tos-workspace-probe",
             "--root", install_root, "--locked", "--offline"],
            cwd=ROOT,
            check=True,
        )
        executable = Path(install_root) / "bin" / (PROBE + (".exe" if os.name == "nt" else ""))
        actual = subprocess.check_output([str(executable)], cwd=install_root, text=True)
        if actual != EXPECTED + "\n":
            raise ValueError(f"unexpected installed probe output: {actual!r}")
    print("Installed Rust workspace probe ran outside checkout.")


if __name__ == "__main__":
    main()
