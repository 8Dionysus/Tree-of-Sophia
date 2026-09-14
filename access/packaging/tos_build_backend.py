"""Setuptools build hooks with bounded-memory wheel package-data writes.

The standalone SQLite snapshot is already compiled. Packaging only streams its
bytes and records their wheel hash; it never opens or rebuilds the database.
The command-local substitution ends when the build hook returns or raises.
"""
from __future__ import annotations

import base64
from contextlib import contextmanager
import importlib
import os
import stat
from zipfile import ZipFile, ZipInfo

from setuptools import build_meta as _setuptools

CHUNK_BYTES = 1024 * 1024
_BACKEND_PATH = "packaging/tos_build_backend.py"


def _wheel_command():
    try:
        return importlib.import_module("setuptools.command.bdist_wheel")
    except ModuleNotFoundError as error:
        if error.name != "setuptools.command.bdist_wheel":
            raise
        # Setuptools 69 obtains wheel through its normal dynamic build
        # requirements; newer releases own the command themselves.
        return importlib.import_module("wheel.bdist_wheel")


def _streaming_wheel_type(base):
    wheel_module = importlib.import_module(base.__module__)

    class StreamingWheelFile(base):
        def write(self, filename, arcname=None, compress_type=None):
            with open(filename, "rb") as source:
                source_stat = os.fstat(source.fileno())
                info = ZipInfo(
                    os.fspath(arcname or filename),
                    date_time=wheel_module.get_zipinfo_datetime(source_stat.st_mtime),
                )
                info.external_attr = (
                    stat.S_IMODE(source_stat.st_mode) | stat.S_IFMT(source_stat.st_mode)
                ) << 16
                info.compress_type = self.compression if compress_type is None else compress_type
                info.file_size = source_stat.st_size
                digest = self._default_algorithm()
                size = 0
                # Supply file_size before opening so ZipFile selects ZIP64
                # correctly for multi-GB members. Bypass WheelFile.open's read
                # verification wrapper, retaining the standard ZIP writer.
                with ZipFile.open(self, info, "w") as target:
                    while chunk := source.read(CHUNK_BYTES):
                        target.write(chunk)
                        digest.update(chunk)
                        size += len(chunk)
                if size != source_stat.st_size:
                    raise RuntimeError(f"wheel input changed size while packaging: {filename}")
            if info.filename != self.record_path:
                self._file_hashes[info.filename] = (
                    digest.name,
                    base64.urlsafe_b64encode(digest.digest()).rstrip(b"=").decode("ascii"),
                )
                self._file_sizes[info.filename] = size

    return StreamingWheelFile


@contextmanager
def _streaming_wheels():
    command = _wheel_command()
    original = command.WheelFile
    command.WheelFile = _streaming_wheel_type(original)
    try:
        yield
    finally:
        command.WheelFile = original


def build_wheel(wheel_directory, config_settings=None, metadata_directory=None):
    with _streaming_wheels():
        return _setuptools.build_wheel(wheel_directory, config_settings, metadata_directory)


def build_editable(wheel_directory, config_settings=None, metadata_directory=None):
    with _streaming_wheels():
        return _setuptools.build_editable(wheel_directory, config_settings, metadata_directory)


def build_sdist(sdist_directory, config_settings=None):
    # An in-tree PEP 517 backend must travel in the sdist as well as in the
    # standalone source archive. Add it through setuptools' own file list.
    from setuptools.command.egg_info import manifest_maker

    original = manifest_maker.add_defaults

    def add_defaults(command):
        original(command)
        command.filelist.append(_BACKEND_PATH)

    manifest_maker.add_defaults = add_defaults
    try:
        return _setuptools.build_sdist(sdist_directory, config_settings)
    finally:
        manifest_maker.add_defaults = original


get_requires_for_build_wheel = _setuptools.get_requires_for_build_wheel
get_requires_for_build_sdist = _setuptools.get_requires_for_build_sdist
prepare_metadata_for_build_wheel = _setuptools.prepare_metadata_for_build_wheel
get_requires_for_build_editable = _setuptools.get_requires_for_build_editable
prepare_metadata_for_build_editable = _setuptools.prepare_metadata_for_build_editable
