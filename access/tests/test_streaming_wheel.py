"""Portable wheel transport protects bounded reads and PEP 427 integrity."""
from __future__ import annotations

import base64
import csv
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile


ACCESS = Path(__file__).resolve().parents[1]
BACKEND = ACCESS / "packaging/tos_build_backend.py"
spec = importlib.util.spec_from_file_location("tos_streaming_wheel_test_backend", BACKEND)
backend = importlib.util.module_from_spec(spec)
spec.loader.exec_module(backend)


class StreamingWheelTests(unittest.TestCase):
    def test_bounded_reads_zip64_and_record(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "knowledge.sqlite3"
            payload = b"compiled snapshot bytes\0" * 100_000
            source.write_bytes(payload)
            wheel = root / "fixture-0.1-py3-none-any.whl"
            base = backend._wheel_command().WheelFile
            wheel_type = backend._streaming_wheel_type(base)
            real_open = open
            reads = []

            class BoundedSource:
                def __init__(self, stream):
                    self.stream = stream

                def __enter__(self):
                    return self

                def __exit__(self, *args):
                    self.stream.close()

                def fileno(self):
                    return self.stream.fileno()

                def read(self, size=-1):
                    self_test.assertGreater(size, 0)
                    self_test.assertLessEqual(size, backend.CHUNK_BYTES)
                    reads.append(size)
                    return self.stream.read(size)

            self_test = self

            def guarded_open(filename, *args, **kwargs):
                stream = real_open(filename, *args, **kwargs)
                return BoundedSource(stream) if Path(filename) == source else stream

            # Lower the stdlib threshold to exercise large-member ZIP64 without
            # allocating a multi-GB fixture.
            with patch("builtins.open", guarded_open), patch("zipfile.ZIP64_LIMIT", 1024):
                with wheel_type(wheel, "w") as archive:
                    archive.write(source, "fixture/runtime_data/knowledge.sqlite3")
                    archive.writestr("fixture-0.1.dist-info/WHEEL", "Wheel-Version: 1.0\n")
            self.assertGreaterEqual(len(reads), 3)
            with zipfile.ZipFile(wheel) as archive:
                name = "fixture/runtime_data/knowledge.sqlite3"
                self.assertEqual(archive.read(name), payload)
                self.assertEqual(archive.getinfo(name).extract_version, 45)
                records = {row[0]: row[1:] for row in csv.reader(io.StringIO(
                    archive.read("fixture-0.1.dist-info/RECORD").decode()
                ))}
                expected = base64.urlsafe_b64encode(hashlib.sha256(payload).digest()).rstrip(b"=").decode()
                self.assertEqual(records[name], ["sha256=" + expected, str(len(payload))])
                self.assertEqual(records["fixture-0.1.dist-info/RECORD"], ["", ""])
            # The ordinary WheelFile reader independently checks the RECORD hash.
            with base(wheel) as archive:
                self.assertEqual(archive.read(name), payload)

    def test_command_restored_on_failure(self):
        command = backend._wheel_command()
        original = command.WheelFile
        with patch.object(backend._setuptools, "build_wheel", side_effect=RuntimeError("fixture failure")):
            with self.assertRaisesRegex(RuntimeError, "fixture failure"):
                backend.build_wheel("unused")
        self.assertIs(command.WheelFile, original)

    def test_source_archive_wheel_and_installed_payload(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            project = root / "source"
            (project / "packaging").mkdir(parents=True)
            shutil.copy2(BACKEND, project / "packaging/tos_build_backend.py")
            package = project / "src/tos_wheel_fixture"
            (package / "runtime_data").mkdir(parents=True)
            (package / "__init__.py").write_text("VALUE = 'installed'\n")
            payload = b"offline compiled fixture\n" * 10_000
            (package / "runtime_data/knowledge.sqlite3").write_bytes(payload)
            (project / "pyproject.toml").write_text('''
[build-system]
requires = ["setuptools>=69"]
build-backend = "tos_build_backend"
backend-path = ["packaging"]
[project]
name = "tos-wheel-fixture"
version = "0.1"
[tool.setuptools]
package-dir = {"" = "src"}
[tool.setuptools.packages.find]
where = ["src"]
[tool.setuptools.package-data]
tos_wheel_fixture = ["runtime_data/**/*"]
''')
            env = {key: value for key, value in os.environ.items() if key != "PYTHONPATH"}
            env["PYTHONDONTWRITEBYTECODE"] = "1"

            def run(*arguments, cwd=project):
                result = subprocess.run([sys.executable, *arguments], cwd=cwd, env=env,
                                        text=True, capture_output=True, timeout=90)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                return result.stdout

            run("-c", "import sys; sys.path.insert(0,'packaging'); import tos_build_backend as b; b.build_sdist('dist')")
            sdist = next((project / "dist").glob("*.tar.gz"))
            unpacked = root / "unpacked"
            with tarfile.open(sdist) as archive:
                self.assertTrue(any(name.endswith("/packaging/tos_build_backend.py") for name in archive.getnames()))
                archive.extractall(unpacked, filter="data")
            rebuilt = next(unpacked.iterdir())
            wheels = root / "wheels"
            run("-m", "pip", "wheel", "--no-deps", "--no-build-isolation", "--no-cache-dir",
                "--wheel-dir", str(wheels), str(rebuilt))
            wheel = next(wheels.glob("*.whl"))
            installed = root / "installed"
            run("-m", "pip", "install", "--no-deps", "--no-compile", "--no-cache-dir",
                "--target", str(installed), str(wheel))
            output = run("-c", "import sys,json; sys.path.insert(0,sys.argv[1]); "
                         "import tos_wheel_fixture as f; from importlib.resources import files; "
                         "print(json.dumps([f.VALUE,files(f).joinpath('runtime_data/knowledge.sqlite3').read_bytes().hex()]))",
                         str(installed), cwd=root)
            value, actual = json.loads(output)
            self.assertEqual(value, "installed")
            self.assertEqual(bytes.fromhex(actual), payload)


if __name__ == "__main__":
    unittest.main()
