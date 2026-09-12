"""Portable projection path values shared by explicit offline producers."""
from pathlib import Path
from typing import Any


def normalize_paths(value: Any, root: Path) -> Any:
    prefix = root.resolve().as_posix() + "/"
    if isinstance(value, str):
        if value == root.resolve().as_posix():
            return "Tree-of-Sophia"
        if value.startswith(prefix):
            return value.removeprefix(prefix)
        return value
    if isinstance(value, list):
        return [normalize_paths(item, root) for item in value]
    if isinstance(value, dict):
        return {key: normalize_paths(item, root) for key, item in value.items()}
    return value
