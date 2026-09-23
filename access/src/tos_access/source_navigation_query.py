from __future__ import annotations

import re
from collections import deque
from collections.abc import Callable, Iterable, Mapping
from typing import Any


_ROOT_RIGHTS_ID = re.compile(r"^tos\.rights\.[a-z0-9]+(?:[.-][a-z0-9]+)*$")
_LAYER_RIGHTS_ID = re.compile(
    r"^tos\.rights\.[a-z0-9]+(?:[.-][a-z0-9]+)*\.layer\.[a-z0-9]+(?:[.-][a-z0-9]+)*$"
)


def _aggregate_rights_records(records: Iterable[dict[str, Any]]) -> list[dict[str, Any]]:
    """Return only unambiguous whole-record assessments for openness decisions.

    New projections tag aggregate and layer rows explicitly. Historical rows
    have no tag, so accept their aggregate only when one rights ID in the exact
    source record matches the top-level rights schema but not the layer-ID
    pattern, and every other ID in that source group is layer-shaped. The
    top-level ID grammar overlaps that pattern, so ambiguous groups fail closed.
    """

    by_source: dict[str, list[dict[str, Any]]] = {}
    for record in records:
        source_ref = record.get("source_ref")
        if isinstance(source_ref, str) and source_ref:
            by_source.setdefault(source_ref, []).append(record)

    aggregate_records: list[dict[str, Any]] = []
    for source_records in by_source.values():
        if any("assessment_kind" in record for record in source_records):
            if any(record.get("assessment_kind") not in ("aggregate", "layer") for record in source_records):
                continue
            aggregates = [record for record in source_records if record.get("assessment_kind") == "aggregate"]
            if len(aggregates) == 1:
                aggregate_records.extend(aggregates)
            continue

        aggregate_candidates = [
            record for record in source_records
            if _ROOT_RIGHTS_ID.fullmatch(str(record.get("rights_id") or ""))
            and not _LAYER_RIGHTS_ID.fullmatch(str(record.get("rights_id") or ""))
        ]
        layer_candidates = [
            record for record in source_records
            if _LAYER_RIGHTS_ID.fullmatch(str(record.get("rights_id") or ""))
        ]
        if len(aggregate_candidates) == 1 and len(aggregate_candidates) + len(layer_candidates) == len(source_records):
            aggregate_records.extend(aggregate_candidates)
    return aggregate_records


def _filter_file_scoped_rights(
    rights: list[dict[str, Any]],
    component_ids: set[str],
    nodes_by_id: Mapping[str, dict[str, Any]],
    component_edges: Iterable[dict[str, Any]],
    incoming: Mapping[str, Iterable[dict[str, Any]]],
) -> list[dict[str, Any]]:
    """Keep File-scoped rights only through memberships in this dossier."""

    file_ids = {
        node_id for node_id in component_ids
        if nodes_by_id.get(node_id, {}).get("node_kind") == "file"
    }
    if not file_ids:
        return rights
    target_bibliographic_ids = {
        node_id for node_id in component_ids
        if nodes_by_id.get(node_id, {}).get("node_kind") in {"work", "expression", "edition"}
    }
    memberships_by_file: dict[str, list[dict[str, Any]]] = {}
    for edge in component_edges:
        if edge.get("edge_kind") != "authored_item_manifest" or edge.get("predicate_id") != "has_file":
            continue
        item_id = edge.get("from_id")
        file_id = edge.get("to_id")
        if (
            not isinstance(item_id, str) or item_id not in component_ids
            or nodes_by_id.get(item_id, {}).get("node_kind") != "item"
            or not isinstance(file_id, str) or file_id not in file_ids
        ):
            continue

        raw_source_refs = edge.get("source_refs")
        source_refs = {
            ref for ref in raw_source_refs
            if isinstance(raw_source_refs, list) and isinstance(ref, str) and ref
        }
        valid = (
            isinstance(raw_source_refs, list)
            and len(source_refs) == len(raw_source_refs)
            and bool(source_refs)
        )
        properties = edge.get("properties")
        properties = properties if isinstance(properties, dict) else {}
        rights_refs: set[str] = set()
        legacy = False
        if "item_file_contexts" not in properties:
            legacy = True
            valid = valid and len(source_refs) == 1
        else:
            raw_contexts = properties.get("item_file_contexts")
            context_manifest_refs: set[str] = set()
            if not isinstance(raw_contexts, list) or not raw_contexts:
                valid = False
            else:
                for context in raw_contexts:
                    if not isinstance(context, dict):
                        valid = False
                        continue
                    manifest_ref = context.get("manifest_ref")
                    if (
                        not isinstance(manifest_ref, str) or not manifest_ref
                        or manifest_ref not in source_refs or manifest_ref in context_manifest_refs
                    ):
                        valid = False
                    else:
                        context_manifest_refs.add(manifest_ref)
                    rights_ref = context.get("rights_ref")
                    if isinstance(rights_ref, str) and rights_ref:
                        rights_refs.add(rights_ref)
                    elif rights_ref is None or rights_ref == "":
                        legacy = True
                    else:
                        valid = False
                if context_manifest_refs != source_refs:
                    valid = False
        if len(rights_refs) > 1 or (rights_refs and legacy):
            valid = False
        memberships_by_file.setdefault(file_id, []).append({
            "item_id": item_id,
            "rights_refs": rights_refs,
            "legacy": legacy,
            "valid": valid,
        })
    for memberships in memberships_by_file.values():
        item_counts: dict[str, int] = {}
        for membership in memberships:
            item_counts[membership["item_id"]] = item_counts.get(membership["item_id"], 0) + 1
        for membership in memberships:
            if item_counts[membership["item_id"]] > 1:
                membership["valid"] = False

    def record_scopes(record: dict[str, Any]) -> set[str]:
        raw_scopes = record.get("scope_refs")
        if not isinstance(raw_scopes, list):
            return set()
        return {ref for ref in raw_scopes if isinstance(ref, str) and ref}

    def legacy_source_is_unique(item_id: str, file_id: str, source_ref: Any) -> bool:
        owner_edges = [
            edge for edge in incoming.get(file_id, [])
            if edge.get("edge_kind") == "authored_item_manifest"
            and edge.get("predicate_id") == "has_file"
        ]
        if len(owner_edges) != 1 or owner_edges[0].get("from_id") != item_id:
            return False
        matching = [
            record for record in rights
            if {item_id, file_id}.issubset(record_scopes(record))
        ]
        sources = {
            record.get("source_ref") for record in matching
            if isinstance(record.get("source_ref"), str) and record.get("source_ref")
        }
        return (
            len(sources) == 1
            and all(isinstance(record.get("source_ref"), str) and record.get("source_ref") for record in matching)
            and isinstance(source_ref, str)
            and source_ref in sources
        )

    filtered: list[dict[str, Any]] = []
    for record in rights:
        scopes = record_scopes(record)
        file_scopes = scopes & file_ids
        if not file_scopes or scopes & target_bibliographic_ids:
            filtered.append(record)
            continue
        source_ref = record.get("source_ref")
        bound_to_every_file = True
        for file_id in file_scopes:
            bound_to_file = False
            for membership in memberships_by_file.get(file_id, []):
                if not membership["valid"] or file_id not in scopes:
                    continue
                if membership["rights_refs"]:
                    if isinstance(source_ref, str) and source_ref in membership["rights_refs"]:
                        bound_to_file = True
                        break
                elif (
                    membership["legacy"]
                    and membership["item_id"] in scopes
                    and legacy_source_is_unique(membership["item_id"], file_id, source_ref)
                ):
                    bound_to_file = True
                    break
            if not bound_to_file:
                bound_to_every_file = False
                break
        if bound_to_every_file:
            filtered.append(record)
    return filtered


def source_descend_query(
    navigation: Mapping[str, Any],
    nodes_by_id: Mapping[str, dict[str, Any]],
    outgoing: Mapping[str, Iterable[dict[str, Any]]],
    node_id: str,
    *,
    max_depth: int,
    limit: int,
) -> dict[str, Any]:
    """Walk downward through the authored source-navigation projection."""

    if node_id not in nodes_by_id:
        raise KeyError(f"unknown ToS source-navigation node: {node_id}")

    queue: deque[tuple[str, int]] = deque([(node_id, 0)])
    depths = {node_id: 0}
    selected_edges: list[dict[str, Any]] = []
    truncated = False
    while queue:
        current, depth = queue.popleft()
        if depth >= max_depth:
            continue
        for edge in outgoing.get(current, []):
            target = str(edge.get("to_id") or "")
            if target not in nodes_by_id:
                continue
            if target not in depths and len(depths) >= limit:
                truncated = True
                continue
            if edge not in selected_edges:
                selected_edges.append(edge)
            if target not in depths:
                depths[target] = depth + 1
                queue.append((target, depth + 1))
    selected_nodes = [
        {**nodes_by_id[selected_id], "depth": depth}
        for selected_id, depth in sorted(depths.items(), key=lambda item: (item[1], item[0]))
    ]
    return {
        "schema": "tos_source_descent_v1",
        "root_id": node_id,
        "max_depth": max_depth,
        "limit": limit,
        "truncated": truncated,
        "counts": {"nodes": len(selected_nodes), "edges": len(selected_edges)},
        "nodes": selected_nodes,
        "edges": selected_edges,
        "authority_note": navigation.get("authority_boundary"),
    }


def source_dossier_query(
    navigation: Mapping[str, Any],
    nodes_by_id: Mapping[str, dict[str, Any]],
    incoming: Mapping[str, Iterable[dict[str, Any]]],
    semantic_outgoing: Mapping[str, Iterable[dict[str, Any]]],
    rights_for: Callable[[set[str]], Iterable[dict[str, Any]]],
    object_id: str,
    *,
    limit: int,
) -> dict[str, Any]:
    """Return compact human and agent-facing context for one source object."""

    selected = nodes_by_id.get(object_id)
    if selected is None:
        raise KeyError(f"unknown ToS dossier object: {object_id}")
    if selected.get("node_kind") not in {"work", "expression", "edition", "item", "file", "link"}:
        raise ValueError("dossiers are available for Work, Expression, Edition, Item, File, and Link objects")

    bibliographic_predicates = {"has_expression", "embodied_by", "exemplified_by"}
    link_predicates = {"described_by", "metadata_at", "downloadable_at", "rights_statement_at"}
    component_ids = {object_id}
    component_edges: dict[str, dict[str, Any]] = {}
    truncated = False

    def admit(node_id: str) -> bool:
        nonlocal truncated
        if node_id in component_ids:
            return True
        if node_id not in nodes_by_id:
            return False
        if len(component_ids) >= limit:
            truncated = True
            return False
        component_ids.add(node_id)
        return True

    # A Link dossier first climbs only its asserted bibliographic lineage
    # to the owning Work. A Work dossier already has its root and never
    # walks backward through a shared Item into neighboring Works.
    forward_roots = {object_id} if selected.get("node_kind") == "work" else set()
    if selected.get("node_kind") != "work":
        lineage_queue: deque[str] = deque([object_id])
        visited_lineage: set[str] = set()
        while lineage_queue:
            current = lineage_queue.popleft()
            if current in visited_lineage:
                continue
            visited_lineage.add(current)
            current_kind = nodes_by_id[current].get("node_kind")
            if current_kind == "work":
                forward_roots.add(current)
                continue
            allowed_predicates = link_predicates if current_kind == "link" else bibliographic_predicates
            for edge in incoming.get(current, []):
                structural_file_parent = current_kind == "file" and edge.get("edge_kind") == "authored_item_manifest"
                if not structural_file_parent and (
                    edge.get("edge_kind") != "evidence_claim"
                    or edge.get("predicate_id") not in allowed_predicates
                ):
                    continue
                parent = str(edge.get("from_id") or "")
                if not admit(parent):
                    continue
                component_edges[str(edge.get("edge_id") or "")] = edge
                lineage_queue.append(parent)
        if not forward_roots:
            forward_roots = {
                node_id
                for node_id in component_ids
                if nodes_by_id[node_id].get("node_kind") != "link"
            }

    forward_queue: deque[str] = deque(sorted(forward_roots))
    visited_forward: set[str] = set()
    while forward_queue:
        current = forward_queue.popleft()
        if current in visited_forward:
            continue
        visited_forward.add(current)
        for edge in semantic_outgoing.get(current, []):
            target = str(edge.get("to_id") or "")
            if not admit(target):
                continue
            component_edges[str(edge.get("edge_id") or "")] = edge
            forward_queue.append(target)

    # Add only the plantings that point to a Work in this component, then
    # walk their branch ancestors upward. This preserves complete tree
    # paths without opening a route sideways into unrelated dossiers.
    ancestor_queue: deque[str] = deque()
    work_ids = {
        node_id
        for node_id in component_ids
        if nodes_by_id[node_id].get("node_kind") == "work"
    }
    for work_id in sorted(work_ids):
        for edge in incoming.get(work_id, []):
            if edge.get("edge_kind") != "authored_source_planting":
                continue
            parent = str(edge.get("from_id") or "")
            if parent not in nodes_by_id:
                continue
            if parent not in component_ids and len(component_ids) >= limit:
                truncated = True
                continue
            component_ids.add(parent)
            component_edges[str(edge.get("edge_id") or "")] = edge
            ancestor_queue.append(parent)

    visited_ancestors: set[str] = set()
    while ancestor_queue:
        current = ancestor_queue.popleft()
        if current in visited_ancestors:
            continue
        visited_ancestors.add(current)
        current_kind = nodes_by_id[current].get("node_kind")
        for edge in incoming.get(current, []):
            is_branch_parent = edge.get("edge_kind") == "authored_branch_hierarchy"
            is_planting_parent = (
                current_kind == "source_planting"
                and edge.get("edge_kind") == "authored_source_planting"
                and edge.get("predicate_id") == "has_source_planting"
            )
            if not (is_branch_parent or is_planting_parent):
                continue
            parent = str(edge.get("from_id") or "")
            if parent not in nodes_by_id:
                continue
            if parent not in component_ids and len(component_ids) >= limit:
                truncated = True
                continue
            component_ids.add(parent)
            component_edges[str(edge.get("edge_id") or "")] = edge
            ancestor_queue.append(parent)

    component_nodes = [nodes_by_id[node_id] for node_id in sorted(component_ids)]
    grouped_chain = {
        kind: [node for node in component_nodes if node.get("node_kind") == kind]
        for kind in (
            "branch",
            "era",
            "region",
            "tradition",
            "source_planting",
            "work",
            "expression",
            "edition",
            "item",
            "file",
            "link",
        )
    }
    outgoing: dict[str, list[dict[str, Any]]] = {}
    for edge in component_edges.values():
        outgoing.setdefault(str(edge.get("from_id") or ""), []).append(edge)
    tree_paths: list[dict[str, Any]] = []
    for era in grouped_chain["era"]:
        era_id = str(era.get("node_id") or "")
        frontier: deque[tuple[str, list[str], list[str]]] = deque([(era_id, [era_id], [])])
        seen = {era_id}
        while frontier:
            current, node_path, edge_path = frontier.popleft()
            if current == object_id:
                tree_paths.append({"node_ids": node_path, "edge_ids": edge_path})
                break
            for edge in sorted(outgoing.get(current, []), key=lambda item: str(item.get("edge_id") or "")):
                target = str(edge.get("to_id") or "")
                if target and target not in seen:
                    seen.add(target)
                    frontier.append((target, [*node_path, target], [*edge_path, str(edge.get("edge_id") or "")]))
    rights = [
        record
        for record in rights_for(component_ids)
        if isinstance(record, dict)
        and set(str(item) for item in record.get("scope_refs", []) if isinstance(record.get("scope_refs"), list)) & component_ids
    ]
    if selected.get("node_kind") != "file":
        rights = _filter_file_scoped_rights(
            rights, component_ids, nodes_by_id, component_edges.values(), incoming,
        )
    decision_scope_ids = {object_id}
    file_membership_complete = False
    file_member_reviewed_positive: dict[str, bool] = {}
    file_membership_gap: str | None = None
    if selected.get("node_kind") == "link":
        decision_scope_ids = {
            str(edge.get("from_id"))
            for edge in component_edges.values()
            if edge.get("to_id") == object_id and edge.get("edge_kind") == "evidence_claim"
        }
    if selected.get("node_kind") == "file":
        membership_edges = [
            edge for edge in incoming.get(object_id, [])
            if edge.get("edge_kind") == "authored_item_manifest"
            and edge.get("predicate_id") == "has_file"
            and edge.get("to_id") == object_id
        ]
        memberships: dict[str, dict[str, Any]] = {}
        membership_bindings_valid = bool(membership_edges)
        for edge in membership_edges:
            item_id = str(edge.get("from_id") or "")
            if not item_id or nodes_by_id.get(item_id, {}).get("node_kind") != "item":
                membership_bindings_valid = False
                continue
            if item_id in memberships:
                membership_bindings_valid = False
            entry = memberships.setdefault(item_id, {"rights_refs": set(), "legacy": False})
            properties = edge.get("properties")
            properties = properties if isinstance(properties, dict) else {}
            raw_edge_source_refs = edge.get("source_refs")
            edge_source_refs = {
                ref for ref in raw_edge_source_refs
                if isinstance(raw_edge_source_refs, list) and isinstance(ref, str) and ref
            }
            if not isinstance(raw_edge_source_refs, list) or len(edge_source_refs) != len(raw_edge_source_refs):
                membership_bindings_valid = False
            if "item_file_contexts" not in properties:
                entry["legacy"] = True
                if len(edge_source_refs) != 1:
                    membership_bindings_valid = False
                continue
            raw_contexts = properties.get("item_file_contexts")
            if not isinstance(raw_contexts, list) or not raw_contexts:
                membership_bindings_valid = False
                continue
            contexts = raw_contexts
            context_manifest_refs: set[str] = set()
            for context in contexts:
                if not isinstance(context, dict):
                    membership_bindings_valid = False
                    continue
                manifest_ref = context.get("manifest_ref")
                rights_ref = context.get("rights_ref")
                if not isinstance(manifest_ref, str) or not manifest_ref or manifest_ref not in edge_source_refs:
                    membership_bindings_valid = False
                elif manifest_ref in context_manifest_refs:
                    membership_bindings_valid = False
                else:
                    context_manifest_refs.add(manifest_ref)
                if isinstance(rights_ref, str) and rights_ref:
                    entry["rights_refs"].add(rights_ref)
                else:
                    entry["legacy"] = True
            if context_manifest_refs != edge_source_refs:
                membership_bindings_valid = False
        member_ids = set(memberships)
        decision_scope_ids = member_ids | {object_id}
        file_membership_complete = bool(member_ids) and member_ids.issubset(component_ids)
        if not file_membership_complete:
            membership_bindings_valid = False
        # Older projections did not carry rights_ref on Item→File edges. A
        # single, complete membership remains safely resolvable from the
        # rights record's exact Item+File scope; shared legacy Files do not.
        legacy_single_owner = len(member_ids) == 1
        bound_rights: list[dict[str, Any]] = []
        rights_by_member: dict[str, list[dict[str, Any]]] = {item_id: [] for item_id in member_ids}
        for item_id, membership in memberships.items():
            rights_refs = membership["rights_refs"]
            if len(rights_refs) > 1:
                membership_bindings_valid = False
                continue
            if rights_refs and membership["legacy"]:
                membership_bindings_valid = False
                continue
            if not rights_refs and not (legacy_single_owner and membership["legacy"]):
                membership_bindings_valid = False
                continue
            member_rights = []
            for record in rights:
                scope_refs = {
                    str(ref) for ref in record.get("scope_refs", [])
                    if isinstance(record.get("scope_refs"), list) and isinstance(ref, str)
                }
                if rights_refs:
                    # The exact manifest rights_ref supplies the Item context.
                    # Keep rows scoped to that Item or this selected File;
                    # layered source records need not repeat both IDs in every
                    # assessment scope.
                    if record.get("source_ref") not in rights_refs:
                        continue
                    if not scope_refs.intersection({item_id, object_id}):
                        continue
                elif not {item_id, object_id}.issubset(scope_refs):
                    # Legacy snapshots lack an edge-level binding, so keep the
                    # stricter Item+File scope requirement.
                    continue
                member_rights.append(record)
            if not rights_refs:
                # A legacy single-owner File has no exact edge-level rights
                # ref. Preserve it only if its Item+File scope resolves to a
                # single source rights file; otherwise an any-positive record
                # could mask a conflicting unbound source record.
                legacy_sources = {
                    record.get("source_ref")
                    for record in member_rights
                    if isinstance(record.get("source_ref"), str) and record.get("source_ref")
                }
                if len(legacy_sources) != 1 or any(
                    not isinstance(record.get("source_ref"), str) or not record.get("source_ref")
                    for record in member_rights
                ):
                    membership_bindings_valid = False
                    continue
            rights_by_member[item_id] = member_rights
            bound_rights.extend(member_rights)
        decision_rights = sorted(
            {str(record.get("rights_id") or id(record)): record for record in bound_rights}.values(),
            key=lambda record: str(record.get("rights_id") or ""),
        )
        # A File packet is an aggregate over its explicit Item memberships;
        # do not attach a merely File-intersecting rights row to that content
        # identity when its source record cannot be bound to a member.
        rights = decision_rights
        for item_id, member_rights in rights_by_member.items():
            member_aggregate_rights = _aggregate_rights_records(member_rights)
            member_positive = [
                record for record in member_aggregate_rights
                if record.get("assessment_status") in {"licensed", "public_domain_reviewed"}
                and record.get("redistribution_posture") in {"authorized", "authorized_with_conditions"}
            ]
            member_reviewed_positive = [
                record for record in member_positive
                if record.get("review_status") in {"accepted", "accepted_with_limits"}
            ]
            file_member_reviewed_positive[item_id] = bool(member_reviewed_positive)
            if not member_rights:
                membership_bindings_valid = False
        file_membership_complete = file_membership_complete and membership_bindings_valid
        if not file_membership_complete:
            file_membership_gap = "File membership or its exact rights binding is incomplete"
    else:
        decision_rights = [
            record
            for record in rights
            if set(str(item) for item in record.get("scope_refs", []) if isinstance(record.get("scope_refs"), list)) & decision_scope_ids
        ]
        if selected.get("node_kind") == "item":
            # An Item packet is scoped to that acquisition; rights from a
            # neighboring Item can share its File but are not its context.
            rights = decision_rights

    dossier_links = [selected] if selected.get("node_kind") == "link" else grouped_chain["link"]
    link_statuses = {
        str(node.get("properties", {}).get("access_status") or "unknown")
        for node in dossier_links
    }
    if "open_download" in link_statuses:
        technical_access = "downloadable"
    elif "open_view" in link_statuses:
        technical_access = "viewable"
    elif "metadata_only" in link_statuses:
        technical_access = "metadata_only"
    elif link_statuses & {"restricted", "login_required", "unavailable"}:
        technical_access = "restricted_or_unavailable"
    else:
        technical_access = "unknown"

    positive_statuses = {"licensed", "public_domain_reviewed"}
    decision_aggregate_rights = _aggregate_rights_records(decision_rights)
    positive_rights = [
        record
        for record in decision_aggregate_rights
        if record.get("assessment_status") in positive_statuses
        and record.get("redistribution_posture") in {"authorized", "authorized_with_conditions"}
    ]
    reviewed_positive = [
        record
        for record in positive_rights
        if record.get("review_status") in {"accepted", "accepted_with_limits"}
    ]
    file_all_members_reviewed_positive = (
        selected.get("node_kind") == "file"
        and file_membership_complete
        and bool(file_member_reviewed_positive)
        and all(file_member_reviewed_positive.values())
    )
    if selected.get("node_kind") == "file":
        if file_all_members_reviewed_positive:
            rights_posture = "reviewed_reuse_route"
        elif file_membership_gap or any(file_member_reviewed_positive.values()):
            rights_posture = "membership_scoped_review_required"
        elif positive_rights:
            rights_posture = "candidate_requires_human_review"
        elif decision_rights:
            rights_posture = "not_cleared"
        else:
            rights_posture = "unknown"
    elif reviewed_positive:
        rights_posture = "reviewed_reuse_route"
    elif positive_rights:
        rights_posture = "candidate_requires_human_review"
    elif decision_rights:
        rights_posture = "not_cleared"
    else:
        rights_posture = "unknown"
    gaps: list[str] = []
    if not decision_rights:
        gaps.append("no associated public rights record")
    elif not decision_aggregate_rights:
        gaps.append("no unambiguous aggregate rights assessment")
    if positive_rights and not reviewed_positive:
        gaps.append("positive rights route exists but has no accepted human review")
    if file_membership_gap:
        gaps.append(file_membership_gap)
    elif selected.get("node_kind") == "file" and not file_all_members_reviewed_positive:
        gaps.append("not every exact Item membership has an accepted positive rights route")
    if not grouped_chain["link"]:
        gaps.append("no first-class associated Link record")

    source_refs = sorted(
        {
            str(ref)
            for node in component_nodes
            for ref in [node.get("source_ref")]
            if isinstance(ref, str) and ref
        }
        | {
            str(ref)
            for edge in component_edges.values()
            for ref in edge.get("source_refs", [])
            if isinstance(ref, str) and ref
        }
        | {
            str(record.get("source_ref"))
            for record in rights
            if isinstance(record.get("source_ref"), str)
        }
    )
    return {
        "schema": "tos_source_dossier_v1",
        "object_id": object_id,
        "object": selected,
        "agent_summary": {
            "technical_access": technical_access,
            "rights_posture": rights_posture,
            "human_review_required": not (file_all_members_reviewed_positive if selected.get("node_kind") == "file" else bool(reviewed_positive)),
            "can_conclude_legal_openness": file_all_members_reviewed_positive if selected.get("node_kind") == "file" else bool(reviewed_positive),
            "availability_is_license": False,
            "rights_scope_refs": sorted(decision_scope_ids),
            "gaps": gaps,
        },
        "chain": grouped_chain,
        "tree_paths": tree_paths,
        "relations": [component_edges[key] for key in sorted(component_edges)],
        "rights": sorted(rights, key=lambda record: str(record.get("rights_id") or "")),
        "source_refs": source_refs,
        "truncated": truncated,
        "authority_note": navigation.get("authority_boundary"),
    }
