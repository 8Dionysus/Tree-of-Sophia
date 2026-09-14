"""Explicit full source bootstrap. No implicit reader selection or delta build."""
from __future__ import annotations

import argparse
from dataclasses import asdict, dataclass, field, replace
import json
import os
from pathlib import Path
import sys
import sqlite3

from . import core as source
from .portable_paths import normalize_paths
from .prepared_publication import PublicationLimits, publish_prepared_rows, _metadata
from .compressed_search_bootstrap import BulkBootstrapLimits
from .catalog_index import CatalogLimits
from .catalog_semantics import CatalogInputs, catalog_digest
from .semantic_index import SemanticIndexLimits
from .prepared_semantics import bootstrap_prepared_maintenance_transaction
from .published_read_metadata import TOP_KEY, _compact, emitted_row_digest, published_snapshot_binding

SCHEMA = "tos_offline_prepared_bootstrap_receipt_v1"


@dataclass(frozen=True)
class MaintenanceAttachmentLimits:
    """Explicit allowance for the separate auxiliary-index transaction.

    Publication and attachment do not share a SQL connection/counter. Their
    named mutation caps add to an upper bound, never an observed actual total.
    All byte caps still constrain the same whole SQLite file.
    """
    max_mutations: int = 2_000_000
    catalog_limits: CatalogLimits = field(default_factory=CatalogLimits)
    semantic_limits: SemanticIndexLimits = field(default_factory=SemanticIndexLimits)

    def __post_init__(self):
        PublicationLimits(max_mutations=self.max_mutations)
        if not isinstance(self.catalog_limits, CatalogLimits) or not isinstance(self.semantic_limits, SemanticIndexLimits):
            raise ValueError("explicit catalog and semantic maintenance limits required")


def _maintenance_caps(publication, maintenance):
    whole_file = min(publication.max_bytes, maintenance.catalog_limits.max_index_bytes,
                     maintenance.semantic_limits.max_bytes)
    return (replace(publication, max_bytes=whole_file, max_mutations=maintenance.max_mutations),
            replace(maintenance.catalog_limits, max_index_bytes=whole_file),
            replace(maintenance.semantic_limits, max_bytes=whole_file,
                    max_writes=min(maintenance.semantic_limits.max_writes, maintenance.max_mutations)))


def _selected_binding(db):
    top = _metadata(db, TOP_KEY)
    clock = db.execute("SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1").fetchone()
    if clock is None or _metadata(db, "data_revision") != {"sha256": top.get("data_revision")}:
        raise ValueError("maintenance selected publication metadata differs")
    return published_snapshot_binding(top, clock[0])


def _attach_maintenance(path, *, core, state, binding, inputs, row_factory, publication, maintenance):
    limits, catalog_limits, semantic_limits = _maintenance_caps(publication, maintenance)
    # mode=rw never creates a replacement when the just-published file is gone.
    db = sqlite3.connect(path.as_uri() + "?mode=rw", uri=True, isolation_level=None)
    try:
        db.execute("BEGIN IMMEDIATE")
        if core._knowledge_input_state() != state:
            raise RuntimeError("source changed before maintenance attachment")
        if _selected_binding(db) != binding:
            raise ValueError("selected publication changed before maintenance attachment")
        start = db.total_changes
        result = bootstrap_prepared_maintenance_transaction(db, expected_binding=binding,
            inputs=inputs, limits=limits, catalog_limits=catalog_limits,
            semantic_limits=semantic_limits, ordered_rows=row_factory)
        mutations = db.total_changes - start
        if (result.get("binding") != binding or _selected_binding(db) != binding
                or result.get("publication_changed") is not False
                or result.get("consumer_switched") is not False
                or result.get("sql_mutations") != mutations or mutations > limits.max_mutations):
            raise ValueError("maintenance changed the selected publication or exceeded its write budget")
        if core._knowledge_input_state() != state:
            raise RuntimeError("source changed during maintenance attachment")
        receipt = {
            "schema": "tos_prepared_maintenance_attachment_receipt_v1",
            "status": "attached", "mode": "catalog_semantic_indexes",
            "binding": binding.copy(), "catalog_digest": result["catalog_digest"],
            "semantic_report_sha256": emitted_row_digest(_compact(result["semantic_report"]))["sha256"],
            "sql_mutations": mutations, "declared_limits": asdict(maintenance),
            "effective_limits": {"publication": asdict(limits), "catalog": asdict(catalog_limits),
                                 "semantic": asdict(semantic_limits)},
            "mutation_budget_upper_bound": publication.max_mutations + maintenance.max_mutations,
            "publication_changed": False, "consumer_switched": False,
            "source_transition_verified": False, "semantic_acceptance": False,
        }
        db.execute("COMMIT")
        return receipt
    except BaseException:
        if db.in_transaction:
            db.execute("ROLLBACK")
        raise
    finally:
        db.close()


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
            search_scratch_limits: BulkBootstrapLimits | None = None,
            maintenance: MaintenanceAttachmentLimits | None = None) -> dict:
    """Create a fresh private output directory; retain incomplete attempts.

    The completed marker is the sole success signal for this directory ABI.
    Source coherence is checked at build time, not granted for future reads.
    """
    root = Path(source_root).expanduser().resolve(strict=True)
    if not root.is_dir():
        raise ValueError("source root must be a directory")
    output = Path(output_dir).expanduser().absolute()
    limits = limits or PublicationLimits()
    if maintenance is not None:
        if not isinstance(maintenance, MaintenanceAttachmentLimits):
            raise ValueError("explicit MaintenanceAttachmentLimits required")
        _maintenance_caps(limits, maintenance)
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
    snapshot = (core.knowledge_snapshot_once() if maintenance is None else
                core.knowledge_snapshot_once(include_catalog_inputs=True))
    state = snapshot["source_state"]
    if core._knowledge_input_state() != state:
        raise RuntimeError("source changed after coherent snapshot selection")
    graph = snapshot["graph"]
    header = normalize_paths({key: value for key, value in graph.items()
                              if key not in ("nodes", "relations")}, root)
    catalog = normalize_paths(snapshot["catalog"], root)
    inputs = None
    if maintenance is not None:
        captured = snapshot.get("catalog_inputs")
        if (not isinstance(captured, CatalogInputs)
                or catalog_digest(normalize_paths(captured.header, root)) != catalog_digest(header)):
            raise ValueError("exact coherent snapshot CatalogInputs required for maintenance")
        # Registry bytes own normalization identity: do not path-rewrite or
        # reconstruct them from their weaker catalog projection. If portable
        # rows/header/lenses cannot reproduce the exact selected catalog and
        # semantic report, attachment refuses instead of rebinding source truth.
        inputs = CatalogInputs(header, captured.entity_type_registry, captured.relation_type_registry,
            normalize_paths(captured.lenses, root), source_order_profile=captured.source_order_profile)
    path = output / "snapshot.sqlite"
    row_factory = lambda kind: (normalize_paths(item, root) for item in graph[kind + "s"])
    binding = publish_prepared_rows(path, source_header=header, catalog=catalog,
        row_factory=row_factory, limits=limits,
        search_scratch_path=output / ".search-sort.sqlite" if search_scratch_limits is not None else None,
        search_scratch_limits=search_scratch_limits)
    if core._knowledge_input_state() != state:
        raise RuntimeError("source changed during offline publication")
    attached = None
    if maintenance is not None:
        attached = _attach_maintenance(path, core=core, state=state, binding=binding, inputs=inputs,
            row_factory=row_factory, publication=limits, maintenance=maintenance)
        if core._knowledge_input_state() != state:
            raise RuntimeError("source changed after maintenance attachment")
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
    if attached is not None:
        receipt["maintenance"] = attached
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
                        help="explicit base-publication SQL mutation cap, including bulk scratch writes")
    parser.add_argument("--bulk-search-scratch-bytes", type=positive_integer,
                        help="explicit bulk search scratch byte cap; requires scratch mutation cap and host reservation")
    parser.add_argument("--bulk-search-scratch-mutations", type=positive_integer,
                        help="explicit bulk search scratch mutation cap; also charged to --max-mutations")
    parser.add_argument("--attach-maintenance", action="store_true",
                        help="explicitly attach catalog and semantic maintenance indexes before completion")
    parser.add_argument("--maintenance-max-mutations", type=positive_integer,
                        help="separate attachment transaction write cap; requires --attach-maintenance")
    args = parser.parse_args(argv)
    if (args.bulk_search_scratch_bytes is None) != (args.bulk_search_scratch_mutations is None):
        parser.error("bulk search requires both scratch byte and mutation caps")
    if args.maintenance_max_mutations is not None and not args.attach_maintenance:
        parser.error("maintenance mutation allowance requires --attach-maintenance")
    try:
        scratch_limits = (BulkBootstrapLimits(args.bulk_search_scratch_bytes, args.bulk_search_scratch_mutations)
                          if args.bulk_search_scratch_bytes is not None else None)
        maintenance = (MaintenanceAttachmentLimits(max_mutations=args.maintenance_max_mutations
                       if args.maintenance_max_mutations is not None else 2_000_000)
                       if args.attach_maintenance else None)
        receipt = prepare(args.source_root, args.output_dir, limits=PublicationLimits(
            max_bytes=args.max_bytes, max_mutations=args.max_mutations), search_scratch_limits=scratch_limits,
            maintenance=maintenance)
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
