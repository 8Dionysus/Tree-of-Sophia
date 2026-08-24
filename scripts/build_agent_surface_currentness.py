#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = Path(".agents/agent-surface.manifest.json")
CURRENTNESS_PATH = Path(".agents/agent-surface.current.json")
SKILLS_ROOT = Path(".agents/skills")
COMPANION_FAMILIES = ("references", "examples", "checks", "scripts", "assets")
WORD_RE = re.compile(r"\S+")
FRONTMATTER_RE = re.compile(r"\A---\n(.*?)\n---\n", re.DOTALL)
FRONTMATTER_KEYS = (
    "name",
    "description",
    "license",
    "compatibility",
    "aoa_scope",
    "aoa_status",
    "aoa_invocation_mode",
    "aoa_source_skill_path",
    "aoa_source_repo",
    "aoa_portable_profile",
)
TOP_LEVEL_FRONTMATTER_KEYS = ("name", "description", "license", "compatibility")
METADATA_FRONTMATTER_KEYS = tuple(
    key for key in FRONTMATTER_KEYS if key not in TOP_LEVEL_FRONTMATTER_KEYS
)
OPENAI_KEYS = ("implicit_activation_policy", "allow_implicit_invocation")
IGNORED_PACKAGE_PARTS = {
    ".deps",
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    "__pycache__",
    "htmlcov",
}
IGNORED_PACKAGE_SUFFIXES = {".pyc", ".pyo"}


def read_text(root: Path, relative: str) -> str:
    return (root / relative).read_text(encoding="utf-8")


def digest_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def file_record(root: Path, path: Path) -> dict[str, Any]:
    payload = path.read_bytes()
    return {
        "path": path.relative_to(root).as_posix(),
        "bytes": len(payload),
        "sha256": digest_bytes(payload),
    }


def word_count(value: str) -> int:
    return len(WORD_RE.findall(value))


def quoted_scalar_is_open(value: str) -> bool:
    if not value or value[0] not in ("'", '"'):
        return False
    quote = value[0]
    if quote == "'":
        index = 1
        while index < len(value):
            if value[index] != "'":
                index += 1
                continue
            # YAML single-quoted scalars encode an apostrophe as a doubled
            # quote.  It is not the closing delimiter, including when the
            # pair is the final content on a physical line.
            if index + 1 < len(value) and value[index + 1] == "'":
                index += 2
                continue
            return False
        return True
    escaped = False
    for character in value[1:]:
        if escaped:
            escaped = False
        elif character == "\\":
            escaped = True
        elif character == '"':
            return False
    return True


def frontmatter_scalars(block: str, *, path: Path) -> dict[str, str | None]:
    """Read top-level fields and known fields from the metadata mapping only."""
    values: dict[str, str | None] = {key: None for key in FRONTMATTER_KEYS}
    metadata_active = False
    metadata_seen = False
    for line_number, line in enumerate(block.splitlines(), start=1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(line) - len(line.lstrip(" "))
        key, separator, value = stripped.partition(":")
        if indent == 0:
            if key == "metadata":
                if metadata_seen:
                    raise ValueError(f"{path}:{line_number}: duplicate metadata mapping")
                if not separator or value.strip():
                    raise ValueError(f"{path}:{line_number}: metadata must be a mapping")
                metadata_seen = True
                metadata_active = True
                continue
            metadata_active = False
            if key in TOP_LEVEL_FRONTMATTER_KEYS:
                if values[key] is not None:
                    raise ValueError(f"{path}:{line_number}: duplicate frontmatter field {key}")
                if not separator or not value.strip():
                    raise ValueError(f"{path}:{line_number}: frontmatter field {key} needs a scalar")
                scalar = value.strip()
                if key == "description" and (
                    re.fullmatch(r"[>|][0-9+-]*", scalar)
                    or quoted_scalar_is_open(scalar)
                ):
                    raise ValueError(
                        f"{path}:{line_number}: multiline description scalars are not supported"
                    )
                values[key] = scalar
                continue
            if key in METADATA_FRONTMATTER_KEYS:
                raise ValueError(
                    f"{path}:{line_number}: {key} must be an immediate child of metadata"
                )
            continue
        if key not in METADATA_FRONTMATTER_KEYS:
            continue
        if not metadata_active or indent != 2:
            raise ValueError(
                f"{path}:{line_number}: {key} must be an immediate child of metadata"
            )
        if values[key] is not None:
            raise ValueError(f"{path}:{line_number}: duplicate metadata field {key}")
        if not separator or not value.strip():
            raise ValueError(f"{path}:{line_number}: metadata field {key} needs a scalar")
        values[key] = value.strip()
    return values


def boolean_scalar(value: str | None, *, key: str, path: Path) -> bool | None:
    if value is None:
        return None
    if value == "true":
        return True
    if value == "false":
        return False
    raise ValueError(f"{path}: {key} must be the YAML boolean literal true or false")


def policy_scalars(text: str, *, path: Path) -> dict[str, str | None]:
    """Read activation fields only from the top-level YAML policy mapping."""
    values: dict[str, str | None] = {key: None for key in OPENAI_KEYS}
    policy_active = False
    policy_seen = False
    for line_number, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(line) - len(line.lstrip(" "))
        if stripped == "policy:":
            if indent != 0:
                raise ValueError(f"{path}:{line_number}: policy mapping must be top-level")
            if policy_seen:
                raise ValueError(f"{path}:{line_number}: duplicate top-level policy mapping")
            policy_seen = True
            policy_active = True
            continue
        key, separator, value = stripped.partition(":")
        if indent == 0:
            policy_active = False
            if key in OPENAI_KEYS:
                raise ValueError(
                    f"{path}:{line_number}: {key} must be an immediate child of policy"
                )
            continue
        if key not in OPENAI_KEYS:
            continue
        if not policy_active or indent != 2:
            raise ValueError(
                f"{path}:{line_number}: {key} must be an immediate child of policy"
            )
        if values[key] is not None:
            raise ValueError(f"{path}:{line_number}: duplicate policy field {key}")
        if not separator or not value.strip():
            raise ValueError(f"{path}:{line_number}: policy field {key} needs a scalar")
        values[key] = value.strip()
    return values


def _fallback_package_files(package_root: Path) -> list[Path]:
    return [
        child
        for child in sorted(package_root.rglob("*"))
        if child.is_file()
        and not IGNORED_PACKAGE_PARTS.intersection(child.relative_to(package_root).parts)
        and child.suffix not in IGNORED_PACKAGE_SUFFIXES
    ]


def package_files(root: Path, package_root: Path) -> list[Path]:
    """Read only tracked package files; use a filtered fallback outside Git."""
    try:
        relative_root = package_root.relative_to(root).as_posix()
        result = subprocess.run(
            ("git", "ls-files", "--cached", "--", relative_root),
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError, ValueError):
        return _fallback_package_files(package_root)
    tracked = [
        root / line
        for line in result.stdout.splitlines()
        if line and (root / line).is_file()
    ]
    return tracked or _fallback_package_files(package_root)


def parse_skill(skill_path: Path) -> dict[str, Any]:
    text = skill_path.read_text(encoding="utf-8")
    match = FRONTMATTER_RE.match(text)
    if not match:
        raise ValueError(f"{skill_path}: missing YAML frontmatter")
    frontmatter = match.group(1)
    body = text[match.end():]
    values = frontmatter_scalars(frontmatter, path=skill_path)
    if not values["name"] or not values["description"]:
        raise ValueError(f"{skill_path}: name and description are required")
    openai_path = skill_path.parent / "agents" / "openai.yaml"
    openai_text = openai_path.read_text(encoding="utf-8")
    activation = policy_scalars(openai_text, path=openai_path)
    activation["allow_implicit_invocation"] = boolean_scalar(
        activation["allow_implicit_invocation"],
        key="allow_implicit_invocation",
        path=openai_path,
    )
    files = package_files(REPO_ROOT, skill_path.parent)
    companion_counts = {
        family: sum(
            1
            for child in files
            if child.relative_to(skill_path.parent).parts[:1] == (family,)
        )
        for family in COMPANION_FAMILIES
    }
    return {
        "id": values["name"],
        "entrypoint": skill_path.relative_to(REPO_ROOT).as_posix(),
        "package_bytes": sum(path.stat().st_size for path in files),
        "package_file_count": len(files),
        "package_sha256": digest_bytes(
            b"".join(
                path.relative_to(skill_path.parent).as_posix().encode("utf-8")
                + b"\0"
                + path.read_bytes()
                + b"\0"
                for path in files
            )
        ),
        "companion_counts": companion_counts,
        "frontmatter": {
            "description_words": word_count(values["description"] or ""),
            "name": values["name"],
            "license": values["license"],
            "compatibility": values["compatibility"],
            "aoa_scope": values["aoa_scope"],
            "aoa_status": values["aoa_status"],
            "aoa_invocation_mode": values["aoa_invocation_mode"],
            "aoa_source_skill_path": values["aoa_source_skill_path"],
            "aoa_source_repo": values["aoa_source_repo"],
            "aoa_portable_profile": values["aoa_portable_profile"],
        },
        "triggered_body_words": word_count(body),
        "activation": activation,
        "on_demand_families_present": [
            family for family in COMPANION_FAMILIES
            if companion_counts[family]
        ],
    }


def load_manifest(root: Path = REPO_ROOT) -> dict[str, Any]:
    return json.loads((root / MANIFEST_PATH).read_text(encoding="utf-8"))


def collect_port_inputs(root: Path, manifest: dict[str, Any]) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    ports = manifest.get("owner_ports", {})
    if not isinstance(ports, dict):
        return records
    for port_id in sorted(ports):
        port = ports[port_id]
        if not isinstance(port, dict):
            continue
        inputs: list[dict[str, Any]] = []
        for relative in port.get("currentness_inputs", []):
            path = root / relative
            if path.is_file():
                inputs.append(file_record(root, path))
            else:
                inputs.append({"path": relative, "missing": True})
        records.append({
            "id": port_id,
            "manifest": port.get("manifest"),
            "inputs": inputs,
        })
    return records


def build_currentness(root: Path = REPO_ROOT) -> dict[str, Any]:
    manifest_path = root / MANIFEST_PATH
    manifest = load_manifest(root)
    package_records = []
    for skill_path in sorted((root / SKILLS_ROOT).glob("*/SKILL.md")):
        package_records.append(parse_skill(skill_path))
    package_inventory = {
        "skill_entrypoints": len(package_records),
        "references": sum(record["companion_counts"]["references"] for record in package_records),
        "examples": sum(record["companion_counts"]["examples"] for record in package_records),
        "checks": sum(record["companion_counts"]["checks"] for record in package_records),
        "scripts": sum(record["companion_counts"]["scripts"] for record in package_records),
        "assets": sum(record["companion_counts"]["assets"] for record in package_records),
        "agents_metadata": sum(
            1
            for record in package_records
            if (root / record["entrypoint"]).parent.joinpath("agents/openai.yaml").is_file()
        ),
    }
    probe_depths = {
        probe["id"]: probe["mandatory_reading_depth"]
        for probe in manifest.get("task_probes", [])
        if isinstance(probe, dict) and "id" in probe
    }
    return {
        "schema_version": "tos_agent_tool_owner_port_current_v1",
        "owner_repo": manifest.get("owner_repo"),
        "source_manifest": MANIFEST_PATH.as_posix(),
        "manifest_sha256": digest_bytes(manifest_path.read_bytes()),
        "builder": "scripts/build_agent_surface_currentness.py",
        "package_inventory": package_inventory,
        "packages": package_records,
        "task_probe_depths": probe_depths,
        "owner_ports": collect_port_inputs(root, manifest),
    }


def rendered_currentness(root: Path = REPO_ROOT) -> bytes:
    return (
        json.dumps(
            build_currentness(root),
            indent=2,
            sort_keys=True,
            ensure_ascii=False,
        )
        + "\n"
    ).encode("utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build the ToS agent surface currentness read model.")
    parser.add_argument("--check", action="store_true", help="check generated parity without writing")
    args = parser.parse_args(argv)
    output_path = REPO_ROOT / CURRENTNESS_PATH
    expected = rendered_currentness(REPO_ROOT)
    if args.check:
        if not output_path.is_file():
            print(f"[error] missing {CURRENTNESS_PATH}", file=sys.stderr)
            return 1
        if output_path.read_bytes() != expected:
            print(f"[error] stale {CURRENTNESS_PATH}; run the builder", file=sys.stderr)
            return 1
        print(f"[ok] currentness parity for {CURRENTNESS_PATH}")
        return 0
    output_path.write_bytes(expected)
    print(f"[ok] wrote {CURRENTNESS_PATH}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
