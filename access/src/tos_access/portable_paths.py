"""Portable projection path values shared by explicit offline producers."""
from pathlib import Path
from typing import Any


def normalize_paths(value: Any, root: Path) -> Any:
    root_text = root.resolve().as_posix()
    prefix = root_text + "/"

    def convert(item):
        if isinstance(item, str):
            if item == root_text:
                return "Tree-of-Sophia"
            return item.removeprefix(prefix) if item.startswith(prefix) else item
        if isinstance(item, list):
            return [convert(child) for child in item]
        if isinstance(item, dict):
            return {key: convert(child) for key, child in item.items()}
        return item

    return convert(value)
