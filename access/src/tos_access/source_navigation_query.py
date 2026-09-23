from __future__ import annotations

from collections import deque
from collections.abc import Callable, Iterable, Mapping
from typing import Any


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
            edge_source_refs = {
                ref for ref in edge.get("source_refs", [])
                if isinstance(edge.get("source_refs"), list) and isinstance(ref, str) and ref
            }
            if "item_file_contexts" not in properties:
                entry["legacy"] = True
                continue
            raw_contexts = properties.get("item_file_contexts")
            if not isinstance(raw_contexts, list) or not raw_contexts:
                membership_bindings_valid = False
                continue
            contexts = raw_contexts
            for context in contexts:
                if not isinstance(context, dict):
                    membership_bindings_valid = False
                    continue
                manifest_ref = context.get("manifest_ref")
                rights_ref = context.get("rights_ref")
                if not isinstance(manifest_ref, str) or not manifest_ref or manifest_ref not in edge_source_refs:
                    membership_bindings_valid = False
                if isinstance(rights_ref, str) and rights_ref:
                    entry["rights_refs"].add(rights_ref)
                else:
                    entry["legacy"] = True
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
            for record in rights:
                scope_refs = {
                    str(ref) for ref in record.get("scope_refs", [])
                    if isinstance(record.get("scope_refs"), list) and isinstance(ref, str)
                }
                if not {item_id, object_id}.issubset(scope_refs):
                    continue
                if rights_refs and record.get("source_ref") not in rights_refs:
                    continue
                rights_by_member[item_id].append(record)
                bound_rights.append(record)
        decision_rights = sorted(
            {str(record.get("rights_id") or id(record)): record for record in bound_rights}.values(),
            key=lambda record: str(record.get("rights_id") or ""),
        )
        # A File packet is an aggregate over its explicit Item memberships;
        # do not attach a merely File-intersecting rights row to that content
        # identity when its source record cannot be bound to a member.
        rights = decision_rights
        for item_id, member_rights in rights_by_member.items():
            member_positive = [
                record for record in member_rights
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
    positive_rights = [
        record
        for record in decision_rights
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
