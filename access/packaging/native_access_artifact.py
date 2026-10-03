"""The exact build-owned native member of the existing software-only layout.

Integrity/provenance checks never execute the supplied artifact. They do not
admit a release or prove the truth of an untrusted build receipt.
"""
from __future__ import annotations
import json
import re
import stat
import tomllib
from pathlib import Path

NATIVE_MEMBER = "access/src/tos_access/tos-access"
NATIVE_SCHEMA = "tos_native_access_build_v1"
MAX_STATIC_BYTES = 16 * 1024 * 1024
MAX_MANIFEST_BYTES = 1_048_576
NATIVE_TARGET = "x86_64-unknown-linux-gnu"
NATIVE_PROOF_MEMBERS = ("Cargo.lock", "rust-toolchain.toml")
_KEYS = {"schema_version", "sha256", "size_bytes", "target", "source_commit", "source_tree", "lock_sha256", "toolchain", "profile"}


def validate_native_proof(proof: dict, source_ref: str) -> None:
    if not isinstance(proof, dict) or set(proof) != _KEYS:
        raise RuntimeError("native build receipt fields differ")
    if proof["schema_version"] != NATIVE_SCHEMA or proof["target"] != NATIVE_TARGET:
        raise RuntimeError("unsupported native build profile")
    if proof["source_commit"] != source_ref or proof["profile"] not in {"debug", "release"}:
        raise RuntimeError("native build source/profile binding differs")
    for key in ("sha256", "lock_sha256"):
        if not isinstance(proof[key], str) or re.fullmatch(r"[0-9a-f]{64}", proof[key]) is None:
            raise RuntimeError("native build digest invalid")
    for key in ("source_commit", "source_tree"):
        if not isinstance(proof[key], str) or re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", proof[key]) is None:
            raise RuntimeError("native build Git identity invalid")
    if type(proof["size_bytes"]) is not int or proof["size_bytes"] < 64:
        raise RuntimeError("native build size invalid")
    if not isinstance(proof["toolchain"], str) or re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", proof["toolchain"]) is None:
        raise RuntimeError("native toolchain identity invalid")


def validate_native_header(header: bytes) -> None:
    if len(header) != 64 or header[:7] != b"\x7fELF\x02\x01\x01" or header[18:20] != b"\x3e\x00":
        raise RuntimeError("native artifact is not Linux x86_64 ELF64")


def load_native_handoff(binary: Path, receipt: Path, repo: Path, source_ref: str, source_tree: str, sha256_file) -> dict:
    if receipt.is_symlink() or not receipt.is_file() or receipt.stat().st_size > 65536:
        raise RuntimeError("native build receipt unavailable or oversized")
    proof = json.loads(receipt.read_bytes())
    validate_native_proof(proof, source_ref)
    if binary.is_symlink() or not binary.is_file() or not stat.S_IMODE(binary.stat().st_mode) & 0o111:
        raise RuntimeError("native build artifact is not an executable regular file")
    before = binary.stat()
    with binary.open("rb") as stream:
        validate_native_header(stream.read(64))
    if before.st_size != proof["size_bytes"] or sha256_file(binary) != proof["sha256"]:
        raise RuntimeError("native artifact identity differs from build receipt")
    after = binary.stat()
    if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns):
        raise RuntimeError("native artifact changed during handoff")
    if proof["source_tree"] != source_tree or proof["lock_sha256"] != sha256_file(repo / "Cargo.lock"):
        raise RuntimeError("native build tree/lock binding differs")
    pin = repo / "rust-toolchain.toml"
    if pin.is_symlink() or not pin.is_file() or pin.stat().st_size > 8192:
        raise RuntimeError("native software toolchain pin unavailable")
    if tomllib.loads(pin.read_text())["toolchain"]["channel"] != proof["toolchain"]:
        raise RuntimeError("native build toolchain binding differs")
    return proof
