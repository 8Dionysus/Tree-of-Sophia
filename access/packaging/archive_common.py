"""Deterministic archive bytes, independent of data compilation."""
from __future__ import annotations
import hashlib
from pathlib import Path
import shutil
import zipfile

FIXED_ZIP_TIME = (2020, 1, 1, 0, 0, 0)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def _write_deterministic_zip(stage_root: Path, output: Path) -> None:
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(item for item in stage_root.rglob("*") if item.is_file()):
            relative = path.relative_to(stage_root).as_posix()
            info = zipfile.ZipInfo(relative, FIXED_ZIP_TIME)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = (0o755 if path.name.endswith(".py") else 0o644) << 16
            info.file_size = path.stat().st_size
            info._compresslevel = 9
            with path.open("rb") as source, archive.open(info, "w", force_zip64=info.file_size >= zipfile.ZIP64_LIMIT) as target:
                shutil.copyfileobj(source, target, length=1024 * 1024)
