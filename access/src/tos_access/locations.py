"""Independent locations for installed software and explicitly selected data."""
from __future__ import annotations

import os
from pathlib import Path


PACKAGE_ROOT = Path(__file__).resolve().parent
def data_root(explicit: str | Path | None = None) -> Path:
    """Select data without discovering an unrelated checkout through cwd.

    TOS_DATA_ROOT selects a dataset; TOS_RELEASE_ROOT selects a managed pair.
    Neither selects executable code, web assets or the reader's API contracts.
    A missing explicit selection stays missing instead of falling back.
    """
    selected = explicit or os.environ.get("TOS_DATA_ROOT")
    if selected:
        root = Path(selected).expanduser().absolute()
        if root != root.resolve():
            raise ValueError("data root may not contain symlinks")
        if os.path.lexists(root / "manifest.json") and (root / "data").is_dir():
            return root / "data"
        return root
    return PACKAGE_ROOT / "runtime_data"


INDEX_RELATIVE_PATH = Path("ToS/derived-exports/tos_corpus_index.min.json")
PHILOSOPHY_PROJECTION_RELATIVE_PATH = Path("ToS/derived-exports/philosophy_graph_projection.min.json")
BIBLIOGRAPHIC_GRAPH_RELATIVE_PATH = Path(
    "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json"
)
ENTITY_TYPE_REGISTRY_RELATIVE_PATH = Path(
    "ToS/doctrine/semantic-interchange/entity-types.v1.json"
)
RELATION_TYPE_REGISTRY_RELATIVE_PATH = Path(
    "ToS/doctrine/semantic-interchange/relation-types.v1.json"
)
PHILOSOPHY_AUDIT_RELATIVE_PATH = Path("ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json")
EVIDENCE_PROJECTION_RELATIVE_PATH = Path("ToS/derived-exports/epistemic_evidence_projection.min.json")

_SOURCE_SELECTORS = (
    ('index_path', 'TOS_CORPUS_INDEX_PATH', INDEX_RELATIVE_PATH),
    ('philosophy_graph_projection_path', 'TOS_PHILOSOPHY_GRAPH_PROJECTION_PATH', PHILOSOPHY_PROJECTION_RELATIVE_PATH),
    ('bibliographic_graph_path', 'TOS_BIBLIOGRAPHIC_GRAPH_PATH', BIBLIOGRAPHIC_GRAPH_RELATIVE_PATH),
    ('entity_type_registry_path', 'TOS_ENTITY_TYPE_REGISTRY_PATH', ENTITY_TYPE_REGISTRY_RELATIVE_PATH),
    ('relation_type_registry_path', 'TOS_RELATION_TYPE_REGISTRY_PATH', RELATION_TYPE_REGISTRY_RELATIVE_PATH),
    ('philosophy_post_planting_audit_path', 'TOS_PHILOSOPHY_POST_PLANTING_AUDIT_PATH', PHILOSOPHY_AUDIT_RELATIVE_PATH),
    ('evidence_projection_path', 'TOS_EVIDENCE_PROJECTION_PATH', EVIDENCE_PROJECTION_RELATIVE_PATH),
)

def source_carrier_paths(root: Path, **explicit) -> dict[str, Path]:
    """Freeze the maintained explicit -> environment -> root-relative selection.

    This selects paths only. It grants no runtime quota or publication authority.
    """
    unknown = explicit.keys() - {name for name, _, _ in _SOURCE_SELECTORS}
    if unknown:
        raise TypeError("unknown source carrier selectors: " + ", ".join(sorted(unknown)))
    paths = {}
    for name, env, relative in _SOURCE_SELECTORS:
        path = Path(explicit.get(name) or os.environ.get(env) or root / relative).expanduser()
        if not path.is_absolute():
            path = root / path
        paths[name] = path.resolve()
    return paths

QUERY_STORE_RELATIVE_PATH = Path('ToS/derived-exports/runtime/knowledge.sqlite3')


def prepare_owned_source_discovery():
    """Prepare optional Linux OS mechanics before the setup receiving ledger.

    Source path selection stays here. The returned owner is not an admission,
    default switch, deadline or managed-release grant.
    """
    from .native_core_selection_paths import prepare_owned_discovery
    return prepare_owned_discovery()
