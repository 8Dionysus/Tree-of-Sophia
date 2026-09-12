"""Explicit full source bootstrap. No implicit reader selection or delta build."""
from __future__ import annotations

import argparse
from dataclasses import asdict
import json
import os
from pathlib import Path
import sys

from . import core as source
from .portable_paths import normalize_paths
from .prepared_publication import PublicationLimits, publish_prepared_rows
from .compressed_search_bootstrap import BulkBootstrapLimits

SCHEMA = "tos_offline_prepared_bootstrap_receipt_v1"


def _sync_directory(path: Path) -> None:
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def _exclusive_json(path: Path, value: dict) -> None:
    # Private new directory, no replacement: publish only a complete fsynced
    # JSON inode. An interrupted temporary file is not a completion marker.
    temporary = path.with_name("." + path.name + ".partial")
    fd = os.open(temporary, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8") as stream:
        json.dump(value, stream, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    created = temporary.stat()
    linked = False
    try:
        os.link(temporary, path)
        linked = True
        temporary.unlink()
        _sync_directory(path.parent)
    except BaseException:
        if linked:
            try:
                current = path.lstat()
                if (current.st_dev, current.st_ino) == (created.st_dev, created.st_ino):
                    path.unlink()
            except FileNotFoundError:
                pass
        raise


def prepare(source_root: str | Path, output_dir: str | Path, *,
            limits: PublicationLimits | None = None,
            search_scratch_limits: BulkBootstrapLimits | None = None) -> dict:
    """Create a fresh private output directory; retain incomplete attempts.

    The completed marker is the sole success signal for this directory ABI.
    Source coherence is checked at build time, not granted for future reads.
    """
    root = Path(source_root).expanduser().resolve(strict=True)
    if not root.is_dir():
        raise ValueError("source root must be a directory")
    output = Path(output_dir).expanduser().absolute()
    limits = limits or PublicationLimits()
    if search_scratch_limits is not None:
        if not isinstance(search_scratch_limits, BulkBootstrapLimits):
            raise ValueError("explicit BulkBootstrapLimits required")
        search_scratch_limits.validate()
    # No parents=True: the caller must select an existing output parent.
    output.mkdir(mode=0o700)
    _sync_directory(output.parent)
    core = source.ToSAccessCore.discover(
        tos_root=root,
        index_path=root / source.INDEX_RELATIVE_PATH,
        philosophy_graph_projection_path=root / source.PHILOSOPHY_PROJECTION_RELATIVE_PATH,
        bibliographic_graph_path=root / source.BIBLIOGRAPHIC_GRAPH_RELATIVE_PATH,
        entity_type_registry_path=root / source.ENTITY_TYPE_REGISTRY_RELATIVE_PATH,
        relation_type_registry_path=root / source.RELATION_TYPE_REGISTRY_RELATIVE_PATH,
        philosophy_post_planting_audit_path=root / source.PHILOSOPHY_AUDIT_RELATIVE_PATH,
        evidence_projection_path=root / source.EVIDENCE_PROJECTION_RELATIVE_PATH,
        search_read_model_path=output / ".unused-source-search.sqlite",
        search_read_model_max_bytes=source.SEARCH_READ_MODEL_DEFAULT_MAX_BYTES,
    )
    snapshot = core.knowledge_snapshot_once()
    state = snapshot["source_state"]
    if core._knowledge_input_state() != state:
        raise RuntimeError("source changed after coherent snapshot selection")
    graph = snapshot["graph"]
    header = normalize_paths({key: value for key, value in graph.items()
                              if key not in ("nodes", "relations")}, root)
    catalog = normalize_paths(snapshot["catalog"], root)
    path = output / "snapshot.sqlite"
    binding = publish_prepared_rows(path, source_header=header, catalog=catalog,
        row_factory=lambda kind: (normalize_paths(item, root) for item in graph[kind + "s"]), limits=limits,
        search_scratch_path=output / ".search-sort.sqlite" if search_scratch_limits is not None else None,
        search_scratch_limits=search_scratch_limits)
    if core._knowledge_input_state() != state:
        raise RuntimeError("source changed during offline publication")
    receipt = {
        "schema": SCHEMA, "status": "completed", "mode": "full_bootstrap",
        "source_root": root.as_posix(), "output_dir": output.as_posix(),
        "source_revision": graph["source_revision"],
        "normalization_binding": graph["normalization_binding"],
        "source_state_checked": True, "ongoing_currentness_granted": False,
        "normalization_cache": "disabled", "consumer_switched": False,
        "snapshot": "snapshot.sqlite", "binding_file": "binding.json",
        "binding": binding, "publication_limits": asdict(limits),
        "search_bootstrap": "bulk" if search_scratch_limits is not None else "buffered",
        "search_scratch_limits": asdict(search_scratch_limits) if search_scratch_limits is not None else None,
        "build_counts": {"nodes": len(graph["nodes"]), "relations": len(graph["relations"]),
                         "snapshot_bytes": path.stat().st_size},
    }
    _exclusive_json(output / "binding.json", binding)
    _exclusive_json(output / "completed.json", receipt)
    return receipt


def main(argv: list[str] | None = None) -> int:
    def positive_integer(value: str) -> int:
        try:
            result = int(value)
        except ValueError as error:
            raise argparse.ArgumentTypeError("must be a positive integer") from error
        if result < 1:
            raise argparse.ArgumentTypeError("must be a positive integer")
        return result

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", required=True)
    parser.add_argument("--output-dir", required=True, help="fresh directory in an existing parent")
    defaults = PublicationLimits()
    parser.add_argument("--max-bytes", type=positive_integer, default=defaults.max_bytes,
                        help="explicit SQLite file byte cap; caller must reserve disk/journal capacity")
    parser.add_argument("--max-mutations", type=positive_integer, default=defaults.max_mutations,
                        help="explicit total SQL mutation cap; does not authorize unbounded processing")
    parser.add_argument("--bulk-search-scratch-bytes", type=positive_integer,
                        help="explicit bulk search scratch byte cap; requires scratch mutation cap and host reservation")
    parser.add_argument("--bulk-search-scratch-mutations", type=positive_integer,
                        help="explicit bulk search scratch mutation cap; also charged to --max-mutations")
    args = parser.parse_args(argv)
    if (args.bulk_search_scratch_bytes is None) != (args.bulk_search_scratch_mutations is None):
        parser.error("bulk search requires both scratch byte and mutation caps")
    try:
        scratch_limits = (BulkBootstrapLimits(args.bulk_search_scratch_bytes, args.bulk_search_scratch_mutations)
                          if args.bulk_search_scratch_bytes is not None else None)
        receipt = prepare(args.source_root, args.output_dir, limits=PublicationLimits(
            max_bytes=args.max_bytes, max_mutations=args.max_mutations), search_scratch_limits=scratch_limits)
    except Exception as error:
        # Exception text may contain source payloads/paths: report a bounded
        # class only, with explicit non-success; leave partial output untouched.
        print(json.dumps({"schema": SCHEMA, "status": "failed",
                          "error_type": type(error).__name__}), file=sys.stderr)
        return 1
    print(json.dumps(receipt, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
