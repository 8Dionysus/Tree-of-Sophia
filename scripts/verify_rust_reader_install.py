#!/usr/bin/env python3
"""Install the native exact reader outside checkout and check retained bytes."""
from __future__ import annotations

import json
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/conformance/rust/corpus-v1"


def main() -> None:
    corpus = json.loads((FIXTURE / "fixture.json").read_text(encoding="utf-8"))
    with tempfile.TemporaryDirectory(prefix="tos-reader-install-") as install_root:
        install = Path(install_root)
        subprocess.run(
            ["cargo", "install", "--path", "rust/crates/tos-reader", "--root", str(install),
             "--locked", "--offline"],
            cwd=ROOT,
            check=True,
        )
        executable = install / "bin/tos-reader"
        capabilities = json.loads(subprocess.run(
            [str(executable), "--capabilities"], cwd=install,
            capture_output=True, check=True, text=True,
        ).stdout)
        required_platform = {
            "schema_version": "tos_reader_capabilities_v1",
            "store_format": "tos_corpus_snapshot_v1",
            "platform": "linux",
            "minimum_kernel": "5.6",
            "required_open_api": "openat2",
            "path_traversal": "beneath_no_symlinks",
            "unsafe_fallback": False,
        }
        if any(capabilities.get(key) != value for key, value in required_platform.items()):
            raise ValueError("installed reader platform capabilities differ")
        for case in corpus["selected_cases"]:
            if "expected_bytes_hex" not in case or "source_id" not in case:
                continue
            command = [
                str(executable), "--store", str((FIXTURE / "store").resolve()),
                "--revision", case["revision"], "--source-id", case["source_id"],
                "--stage-dir", str(install.resolve()),
                "--max-manifest-bytes", "1048576", "--max-manifest-entries", "1000",
                "--max-selected-object-bytes", "1048576", "--json-max-depth", "64",
                "--json-max-visits", "300000", "--json-max-integer-digits", "4300",
            ]
            result = subprocess.run(command, cwd=install, capture_output=True, check=True)
            expected = bytes.fromhex(case["expected_bytes_hex"])
            if result.stdout != expected:
                raise ValueError(f"installed reader bytes differ for {case['case_id']}")
    print("Installed native Rust reader returned exact old and current bytes outside checkout.")


if __name__ == "__main__":
    main()
