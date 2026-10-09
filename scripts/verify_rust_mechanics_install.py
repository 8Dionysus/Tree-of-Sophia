#!/usr/bin/env python3
"""Check the installed native runner through the existing process-custody fixture."""
from __future__ import annotations

import argparse
from contextlib import nullcontext
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--command-entries-only", action="store_true")
    parser.add_argument("--installed-prefix", type=Path, help="consume an explicitly prepared native command prefix")
    args = parser.parse_args()
    if args.installed_prefix is not None and (not args.command_entries_only or not args.installed_prefix.is_absolute()):
        parser.error("--installed-prefix requires --command-entries-only and an absolute prefix")
    # Debug installation reuses the admitted debug profile. The fixture consumes
    # the installed artifact, so this checks installation without a second suite.
    prefix_context = (nullcontext(str(args.installed_prefix)) if args.installed_prefix is not None
                      else tempfile.TemporaryDirectory(prefix="tos-mechanics-install-"))
    with prefix_context as directory:
        install = Path(directory)
        installation = ["cargo", "install", "--debug", "--locked", "--offline", "--path",
                        "rust/crates/tos-ops-mechanics-plan", "--root", str(install)]
        if args.command_entries_only:
            # Preserve default full-package installation for the lifecycle route.
            installation.append("--no-default-features")
            for name in ("tos-validation-lanes", "tos-release-check", "tos-software-ci"):
                installation.extend(["--bin", name])
        if args.installed_prefix is None:
            subprocess.run(installation, cwd=ROOT, check=True)
        for name in ("tos-validation-lanes", "tos-release-check", "tos-software-ci"):
            product = install / "bin" / name
            with product.open("rb") as stream:
                digest = hashlib.file_digest(stream, "sha256").hexdigest()
            print(f"[installed] {name} bytes={product.stat().st_size} sha256={digest}", flush=True)
        environment = os.environ.copy()
        environment["TOS_MECHANICS_TEST_EXECUTABLE"] = str(install / "bin/tos-ops-mechanics-plan")
        if not args.command_entries_only:
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
        if not args.command_entries_only:
            subprocess.run([sys.executable, str(entrypoint)], cwd=fixture,
                           env=environment, check=True)
            if (fixture / "compatibility-executed").read_text(encoding="utf-8") != "yes":
                raise ValueError("installed compatibility entrypoint did not execute its selected test")
        # Cargo's maintained package install supplies these separate command
        # products too. Consume their real compatibility entries from the same
        # fresh prefix, rather than assuming availability from a build receipt.
        manifest = fixture / "docs/validation/validation_lanes.json"
        manifest.parent.mkdir(parents=True)
        steps = [
            {"label": "software contracts", "command": ["python", "scripts/record_phase.py", "checks"]},
            {"label": "run tests", "command": ["python", "scripts/record_phase.py", "tests"]},
        ]
        manifest.write_text(json.dumps({"command_sequences": {"release_check": steps}}), encoding="utf-8")
        (fixture / "scripts/record_phase.py").write_text(
            "from pathlib import Path\nimport sys\n"
            "Path(sys.argv[1] + '-executed').write_text(sys.executable)\n", encoding="utf-8"
        )
        selected = subprocess.run(
            [str(install / "bin/tos-validation-lanes"), "--repo-root", str(fixture), "--python", sys.executable, "--sequence", "release_check"],
            cwd=fixture, env=environment, check=True, capture_output=True, text=True,
        )
        expected = [step["label"] + ": " + " ".join([sys.executable, *step["command"][1:]]) for step in steps]
        if selected.stdout.splitlines() != expected:
            raise ValueError("installed validation entry did not select exact authored commands")
        for phase in ("checks", "tests"):
            subprocess.run(
                [str(install / "bin/tos-release-check"), "--repo-root", str(fixture), "--python", sys.executable, "--phase", phase],
                cwd=fixture, env=environment, check=True,
            )
            if (fixture / f"{phase}-executed").read_text(encoding="utf-8") != sys.executable:
                raise ValueError("installed release entry did not execute its exact phase")
            if phase == "checks" and (fixture / "tests-executed").exists():
                raise ValueError("installed checks phase executed the tests phase")
        def git(*arguments: str) -> str:
            return subprocess.check_output(["git", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", *arguments], cwd=fixture, text=True).strip()
        git("init", "--quiet")
        git("config", "user.name", "Installed command fixture")
        git("config", "user.email", "fixture@example.invalid")
        git("add", ".")
        git("commit", "--quiet", "-m", "installed command baseline")
        base = git("rev-parse", "HEAD")
        source = fixture / "rust/crates/tos-access/src/package.rs"
        source.parent.mkdir(parents=True)
        source.write_text("// tiny native package trigger\n", encoding="utf-8")
        git("add", ".")
        git("commit", "--quiet", "-m", "native package change")
        environment["GITHUB_OUTPUT"] = str(fixture / "selector-output")
        planned = subprocess.run(
            [str(install / "bin/tos-software-ci"), "plan", "--repo-root", str(fixture), "--base", base],
            cwd=fixture, env=environment, check=True, capture_output=True, text=True,
        )
        selection = json.loads(planned.stdout)
        if (selection["software_mode"], selection["rust"], selection["worker"]) != ("browser", True, False):
            raise ValueError("installed CI selector omitted native package consumer")
        needs = {"plan": {"result": "success", "outputs": {
            "software_mode": selection["software_mode"], "worker": str(selection["worker"]).lower(),
            "rust": str(selection["rust"]).lower()}},
            "software": {"result": "success"}, "worker": {"result": "skipped"},
            "rust": {"result": "success"}}
        environment["CI_NEEDS"] = json.dumps(needs)
        entry = [str(install / "bin/tos-software-ci"), "gate"]
        subprocess.run(entry, cwd=fixture, env=environment, check=True)
        needs["plan"]["result"] = "failure"
        environment["CI_NEEDS"] = json.dumps(needs)
        if subprocess.run(entry, cwd=fixture, env=environment, capture_output=True).returncode == 0:
            raise ValueError("installed CI gate accepted failed native preparation")
    if args.command_entries_only:
        print("Installed native validation, release and CI command entries passed.")
    else:
        print("Installed native mechanics lifecycle and command entries passed.")


if __name__ == "__main__":
    main()
