#!/usr/bin/env python3
"""Check the installed native runner through the existing process-custody fixture."""
from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    # Debug installation reuses the admitted debug profile. The fixture consumes
    # the installed artifact, so this checks installation without a second suite.
    with tempfile.TemporaryDirectory(prefix="tos-mechanics-install-") as directory:
        install = Path(directory)
        subprocess.run(
            ["cargo", "install", "--debug", "--locked", "--offline", "--path",
             "rust/crates/tos-ops-mechanics-plan", "--root", str(install)],
            cwd=ROOT, check=True,
        )
        environment = os.environ.copy()
        environment["TOS_MECHANICS_TEST_EXECUTABLE"] = str(install / "bin/tos-ops-mechanics-plan")
        subprocess.run(
            ["cargo", "test", "--locked", "--offline", "-p", "tos-ops-mechanics-plan",
             "--test", "executor_native"],
            cwd=ROOT, env=environment, check=True,
        )
        # Execute the exact compatibility entrypoint in a tiny standalone tree:
        # its resolved root and interpreter must reach the installed supervisor.
        fixture = install / "compatibility-fixture"
        (fixture / "scripts").mkdir(parents=True)
        entrypoint = fixture / "scripts/run_mechanics_local_tests.py"
        shutil.copyfile(ROOT / "scripts/run_mechanics_local_tests.py", entrypoint)
        for relative in [
            "mechanics/fixture/tests/test_unit.py",
            "mechanics/fixture/scripts/build_unit.py",
            "mechanics/fixture/scripts/validate_unit.py",
        ]:
            path = fixture / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("", encoding="utf-8")
        (fixture / "mechanics/fixture/tests/test_unit.py").write_text(
            "import unittest\nfrom pathlib import Path\n"
            "class InstalledRunnerFixture(unittest.TestCase):\n"
            "    def test_execution(self):\n"
            "        Path('compatibility-executed').write_text('yes')\n",
            encoding="utf-8",
        )
        environment["TOS_OPS_MECHANICS_EXECUTOR"] = str(install / "bin/tos-ops-mechanics-plan")
        subprocess.run([sys.executable, str(entrypoint)], cwd=fixture,
                       env=environment, check=True)
        if (fixture / "compatibility-executed").read_text(encoding="utf-8") != "yes":
            raise ValueError("installed compatibility entrypoint did not execute its selected test")
    print("Installed native mechanics runner passed its existing lifecycle fixture.")


if __name__ == "__main__":
    main()
