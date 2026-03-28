#!/usr/bin/env python3
from __future__ import annotations

import json
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]

TREE_SOURCE_NODE_REL = Path(
    "tree/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json"
)
TREE_BOUNDED_HOP_REL = Path("tree/concept/becoming/node.json")
TREE_WORKED_PRINCIPLE_REL = Path(
    "tree/principle/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/solitude-as-ripening/node.json"
)
TREE_WORKED_LINEAGE_REL = Path(
    "tree/lineage/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/becoming-to-overcoming/node.json"
)
TREE_WORKED_EVENT_REL = Path(
    "tree/event/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/departure-from-origin/node.json"
)
TREE_WORKED_STATE_REL = Path(
    "tree/state/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/solitary-ripening-10y/node.json"
)
TREE_WORKED_SUPPORT_REL = Path(
    "tree/support/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/zarathustra/node.json"
)

EXAMPLE_SOURCE_NODE_REL = Path("examples/source_node.example.json")
EXAMPLE_BOUNDED_HOP_REL = Path("examples/concept_node.example.json")
EXAMPLE_WORKED_PRINCIPLE_REL = Path("examples/principle_node.example.json")
EXAMPLE_WORKED_LINEAGE_REL = Path("examples/lineage_node.example.json")
EXAMPLE_WORKED_EVENT_REL = Path("examples/event_node.example.json")
EXAMPLE_WORKED_STATE_REL = Path("examples/state_node.example.json")
EXAMPLE_WORKED_SUPPORT_REL = Path("examples/support_node.example.json")


def read_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def encode_json(payload: object) -> str:
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def mirror_pairs(repo_root: Path | None = None) -> list[tuple[Path, Path]]:
    root = repo_root or REPO_ROOT
    return [
        (root / TREE_SOURCE_NODE_REL, root / EXAMPLE_SOURCE_NODE_REL),
        (root / TREE_BOUNDED_HOP_REL, root / EXAMPLE_BOUNDED_HOP_REL),
        (root / TREE_WORKED_PRINCIPLE_REL, root / EXAMPLE_WORKED_PRINCIPLE_REL),
        (root / TREE_WORKED_LINEAGE_REL, root / EXAMPLE_WORKED_LINEAGE_REL),
        (root / TREE_WORKED_EVENT_REL, root / EXAMPLE_WORKED_EVENT_REL),
        (root / TREE_WORKED_STATE_REL, root / EXAMPLE_WORKED_STATE_REL),
        (root / TREE_WORKED_SUPPORT_REL, root / EXAMPLE_WORKED_SUPPORT_REL),
    ]


def build_expected_example_payloads(
    repo_root: Path | None = None,
) -> list[tuple[Path, object]]:
    payloads: list[tuple[Path, object]] = []
    for tree_path, example_path in mirror_pairs(repo_root):
        payloads.append((example_path, read_json(tree_path)))
    return payloads


def write_examples(repo_root: Path | None = None) -> list[Path]:
    written: list[Path] = []
    for example_path, payload in build_expected_example_payloads(repo_root):
        example_path.write_text(encode_json(payload), encoding="utf-8")
        written.append(example_path)
    return written


def main() -> int:
    for path in write_examples():
        print(f"[ok] wrote {path.relative_to(REPO_ROOT).as_posix()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
