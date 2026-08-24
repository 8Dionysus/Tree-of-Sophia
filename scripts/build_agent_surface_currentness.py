#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
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
OPENAI_KEYS = ("implicit_activation_policy", "allow_implicit_invocation")


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


def scalar_from_block(block: str, key: str) -> str | None:
    match = re.search(rf"^(?:\s*){re.escape(key)}:\s*(.*?)\s*$", block, re.MULTILINE)
    return match.group(1).strip() if match else None


def parse_skill(skill_path: Path) -> dict[str, Any]:
    text = skill_path.read_text(encoding="utf-8")
    match = FRONTMATTER_RE.match(text)
    if not match:
        raise ValueError(f"{skill_path}: missing YAML frontmatter")
    frontmatter = match.group(1)
    body = text[match.end():]
    values = {key: scalar_from_block(frontmatter, key) for key in FRONTMATTER_KEYS}
    if not values["name"] or not values["description"]:
        raise ValueError(f"{skill_path}: name and description are required")
    openai_path = skill_path.parent / "agents" / "openai.yaml"
    openai_text = openai_path.read_text(encoding="utf-8")
    activation = {
        key: scalar_from_block(openai_text, key)
        for key in OPENAI_KEYS
    }
    activation["allow_implicit_invocation"] = (
        activation["allow_implicit_invocation"] == "true"
        if activation["allow_implicit_invocation"] is not None
        else None
    )
    files = [
        child
        for child in sorted(skill_path.parent.rglob("*"))
        if child.is_file()
    ]
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
