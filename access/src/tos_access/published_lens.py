"""Native-v7 lens semantics over an explicitly pinned, published v9 read model.

Default focus reads owner-built histograms and ordered local incidence. General
selectors/sorts use bounded native callbacks over keyset candidate streams;
they never acquire a graph provider or turn a budget refusal into a partial
success. Each page re-executes the exact bounded LensResult, as native v7 does.
"""
from __future__ import annotations

import heapq
from collections import OrderedDict
from dataclasses import dataclass

from . import knowledge as k
from .published_exploration import _Rows
from .published_read_metadata import (
    LENS_EXECUTION_VERSION, LENS_META_KEY, LENS_METADATA_MAX_BYTES, LENS_READER_SCHEMA,
    _compact, emitted_row_digest, lens_order_row, validate_lens_metadata,
)
from .published_read_model import PublishedReadBudgetExceeded, PublishedReadModelError

_DIMENSIONS = {"node": ("source_graph", "kind_id", "type_id"),
               "relation": ("source_graph", "predicate_id", "relation_type_id")}
_INDEXES = {"knowledge_lens_order_sort", "knowledge_lens_order_from",
            "knowledge_lens_order_to", "knowledge_lens_order_pair"}
_DEFAULT_SORT = [{"field": "id", "direction": "asc"}]


@dataclass(frozen=True)
class PublishedLensLimits:
    max_candidates: int = 2048
    max_callback_calls: int = 32768
    max_decoded_bytes: int = 16 * 1024 * 1024
    max_sort_bytes: int = 4 * 1024 * 1024
    max_cache_bytes: int = 2 * 1024 * 1024
    max_cache_entries: int = 64
    max_path_steps: int = 100000
    block_size: int = 16

    def __post_init__(self):
        if any(type(value) is not int or value < 1 for value in vars(self).values()) or self.block_size > 64:
            raise ValueError("prepared lens limits must be positive integers; block_size is at most 64")


class _Budget:
    def __init__(self, limits):
        self.limits = limits
        self.calls = self.decoded = self.sort_bytes = self.candidates = self.path_steps = 0

    def decode(self, raw):
        size = len(raw.encode("utf-8"))
        self.decoded += size
        if self.decoded > self.limits.max_decoded_bytes:
            raise PublishedReadBudgetExceeded("prepared lens native decoding exceeds its byte budget")
        return size

    def call(self, function, *args):
        self.calls += 1
        if self.calls > self.limits.max_callback_calls:
            raise PublishedReadBudgetExceeded("prepared lens native callbacks exceed their call budget")
        return function(*args)

    def candidate(self):
        self.candidates += 1
        if self.candidates > self.limits.max_candidates:
            raise PublishedReadBudgetExceeded("prepared lens selector exceeds its candidate budget")

    def path(self):
        self.path_steps += 1
        if self.path_steps > self.limits.max_path_steps:
            raise PublishedReadBudgetExceeded("prepared lens path query exceeds its work budget")

    def sort_key(self, item, rules):
        result = []
        for rule in rules:
            value = self.call(lambda: str(k._field(item, rule["field"]) or "").lower())
            self.sort_bytes += len(value.encode("utf-8"))
            if self.sort_bytes > self.limits.max_sort_bytes:
                raise PublishedReadBudgetExceeded("prepared lens sort keys exceed their byte budget")
            result.append(_Descending(value) if rule["direction"] == "desc" else value)
        result.append(str(item.get("id") or ""))
        return tuple(result)


class _Descending(str):
    def __lt__(self, other):
        return str.__gt__(self, other)

    def __gt__(self, other):
        return str.__lt__(self, other)


class _Worst:
    def __init__(self, key):
        self.key = key

    def __lt__(self, other):
        return self.key > other.key


class _Payloads:
    def __init__(self, read, budget):
        self.read, self.budget = read, budget
        self.cache, self.cache_bytes = OrderedDict(), 0

    def load(self, kind, ids):
        identifiers = sorted(set(ids))
        result = {}
        missing = []
        for identifier in identifiers:
            key = kind, identifier
            if key in self.cache:
                item, _ = self.cache[key]
                self.cache.move_to_end(key)
                result[identifier] = item
            else:
                missing.append(identifier)
        for start in range(0, len(missing), self.budget.limits.block_size):
            page = missing[start:start + self.budget.limits.block_size]
            sizes = []
            items = self.read.items(kind, "id IN (SELECT value FROM json_each(?))", (_compact(page),), len(page),
                                   before_parse=lambda raw: sizes.append(self.budget.decode(raw)))
            if len(items) != len(page):
                raise PublishedReadModelError("prepared lens selected row or endpoint is missing")
            ordering = {row["id"]: tuple(row) for row in self.read.query(
                "SELECT kind,id,sort_key,from_id,to_id FROM knowledge_lens_order "
                "WHERE kind=? AND id IN (SELECT value FROM json_each(?)) LIMIT ?",
                (kind, _compact(page), len(page) + 1))}
            for item, size in zip(items, sizes):
                identifier = item["id"]
                if ordering.get(identifier) != lens_order_row(kind, item):
                    raise PublishedReadModelError("prepared lens order carrier differs from its full row")
                result[identifier] = item
                key = kind, identifier
                if size <= self.budget.limits.max_cache_bytes:
                    while self.cache and (len(self.cache) >= self.budget.limits.max_cache_entries
                                          or self.cache_bytes + size > self.budget.limits.max_cache_bytes):
                        _, (_, retired) = self.cache.popitem(last=False)
                        self.cache_bytes -= retired
                    self.cache[key] = item, size
                    self.cache_bytes += size
        return result

    def get(self, kind, identifier):
        return self.load(kind, [identifier])[identifier]


def _source_where(alias, sources):
    return f"{alias}.source_graph IN (SELECT value FROM json_each(?))", [_compact(sources)]


class _Plan:
    def __init__(self, read, top, metadata, spec, limits):
        self.read, self.top, self.metadata, self.spec = read, top, metadata, spec
        self.budget = _Budget(limits)
        self.payloads = _Payloads(read, self.budget)
        self.block = limits.block_size
        self.sources = set(spec["sources"])
        self.generic_relations = None
        self.matched_nodes = 0
        self.matched_relations = 0

    def scope_cells(self, kind):
        return [cell for cell in self.metadata[kind + "_counts"] if cell[0] in self.sources]

    def dimensional(self, kind, group):
        return all(not rule.get("_property_binding") and rule.get("field") in _DIMENSIONS[kind]
                   for rule in group["filters"])

    def allowed_cells(self, kind, group):
        if not group["enabled"]:
            return []
        return [cell for cell in self.scope_cells(kind)
                if self.budget.call(k._matches_group, dict(zip(_DIMENSIONS[kind], cell[:3])), group)
                and (kind != "relation" or self._relation_regime(dict(zip(_DIMENSIONS[kind], cell[:3]))))]

    def _relation_regime(self, item):
        traversal = self.spec["traversal"]
        return (not traversal["predicate_ids"] or item["predicate_id"] in traversal["predicate_ids"]) and (
            traversal["profile"] != "overview" or
            (item["predicate_id"] not in k.OVERVIEW_EXCLUDED_PREDICATES
             and item["relation_type_id"] not in k.OVERVIEW_EXCLUDED_RELATION_TYPES))

    def where(self, kind, alias):
        group = self.spec[kind + "_query"]
        where, args = _source_where(alias, self.spec["sources"])
        if not group["enabled"]:
            return "0", []
        if self.dimensional(kind, group) and group["filters"]:
            cells = [cell[:3] for cell in self.allowed_cells(kind, group)]
            where += " AND EXISTS (SELECT 1 FROM json_each(?) cell WHERE " + " AND ".join(
                f"{alias}.{field}=json_extract(cell.value,'$[{index}]')" for index, field in enumerate(_DIMENSIONS[kind])) + ")"
            args.append(_compact(cells))
        if kind == "relation":
            traversal = self.spec["traversal"]
            if traversal["predicate_ids"]:
                where += f" AND {alias}.predicate_id IN (SELECT value FROM json_each(?))"
                args.append(_compact(traversal["predicate_ids"]))
            if traversal["profile"] == "overview":
                where += f" AND {alias}.predicate_id NOT IN (SELECT value FROM json_each(?)) AND {alias}.relation_type_id NOT IN (SELECT value FROM json_each(?))"
                args.extend((_compact(sorted(k.OVERVIEW_EXCLUDED_PREDICATES)), _compact(sorted(k.OVERVIEW_EXCLUDED_RELATION_TYPES))))
        return where, args

    def scan(self, kind):
        # General native evaluation has an explicit finite candidate ceiling;
        # it is not a hidden materialized graph or an approximate selector.
        after = ""
        where, args = _source_where("r", self.spec["sources"])
        while True:
            rows = self.read.query(f"SELECT r.id FROM knowledge_{kind}s r WHERE {where} AND r.id>? ORDER BY r.id LIMIT ?",
                                   (*args, after, self.block))
            if not rows:
                return
            items = self.payloads.load(kind, [row["id"] for row in rows])
            for row in rows:
                self.budget.candidate()
                yield items[row["id"]]
            after = rows[-1]["id"]

    def focus(self):
        identifier = self.spec["seed"]["focus_node_id"]
        if identifier is None:
            return None
        rows = _Rows(self.read, 1)
        item = rows.focus({"focus_node_id": identifier, "sources": self.spec["sources"]})
        # The selected row already went through reader integrity; count its
        # native decoded packet as well even though focus resolves just once.
        self.budget.decode(_compact(item))
        # Focus can be the only selected row (depth zero); it must check its
        # ordered carrier too, not bypass that closure through _Rows.focus.
        return self.payloads.get("node", item["id"])

    def _ordered(self, kind, where, args, *, endpoint=None, extra="", extra_args=()):
        after, first = ("", ""), True
        alias = "n" if kind == "node" else "r"
        index = "knowledge_lens_order_sort" if endpoint is None else "knowledge_lens_order_" + endpoint[0]
        endpoint_sql = "" if endpoint is None else f" AND l.{endpoint[0]}_id=?"
        endpoint_args = [] if endpoint is None else [endpoint[1]]
        while True:
            size = 1 if first and endpoint is not None else self.block
            rows = self.read.query(
                f"SELECT l.id,l.sort_key" + (",r.from_id,r.to_id" if kind == "relation" else "") +
                f" FROM knowledge_lens_order l INDEXED BY {index} CROSS JOIN knowledge_{kind}s {alias} ON {alias}.id=l.id"
                f" WHERE l.kind=?{endpoint_sql} AND (l.sort_key,l.id)>(?,?) AND {where}{extra}"
                " ORDER BY l.sort_key,l.id LIMIT ?",
                (kind, *endpoint_args, *after, *args, *extra_args, size))
            if not rows:
                return
            for row in rows:
                if row["sort_key"] != row["id"].lower():
                    raise PublishedReadModelError("prepared lens order key differs from native ordering")
                yield dict(row)
            after = rows[-1]["sort_key"], rows[-1]["id"]
            first = False

    def select_nodes(self, focus):
        group, seed = self.spec["node_query"], self.spec["seed"]
        if not group["enabled"]:
            return [], {}
        simple = self.dimensional("node", group) and not seed["node_ids"] and not seed["text_query"] and not self.spec["path_query"]
        limit = self.spec["limits"]["nodes"]
        if simple and self.spec["composition"]["sort_nodes"] == _DEFAULT_SORT:
            cells = self.allowed_cells("node", group)
            self.matched_nodes = sum(cell[3] for cell in cells)
            if not self.matched_nodes:
                return [], {}
            where, args = self.where("node", "n")
            ids = []
            for row in self._ordered("node", where, args):
                ids.append(row["id"])
                if len(ids) >= limit:
                    break
            if len(ids) != min(limit, self.matched_nodes):
                raise PublishedReadModelError("prepared lens selector/count/order closure is incomplete")
            items = self.payloads.load("node", ids)
            focus_matches = focus is not None and self.budget.call(k._matches_group, focus, group)
            return [items[identifier] for identifier in ids], ({focus["id"]: []} if focus_matches else {})
        heap = []
        focus_proof = {}
        selected_ids, text = set(seed["node_ids"]), seed["text_query"].lower()
        for node in self.scan("node"):
            if selected_ids and not selected_ids.intersection({str(node[key]) for key in ("id", "native_id", "entity_id")}):
                continue
            if text and text not in self.budget.call(k._searchable, node):
                continue
            if not self.budget.call(k._matches_group, node, group):
                continue
            proofs = []
            for condition in self.spec["path_query"]:
                witness = self.path_witness(node["id"], condition)
                if (witness is not None) != (condition["quantifier"] == "exists"):
                    break
                proofs.append(witness or {"path_id": condition["path_id"], "absence_in_scope": True})
            else:
                self.matched_nodes += 1
                if focus is not None and node["id"] == focus["id"]:
                    focus_proof[node["id"]] = proofs
                key = self.budget.sort_key(node, self.spec["composition"]["sort_nodes"])
                entry = (_Worst(key), node["id"], node, proofs)
                if len(heap) < limit:
                    heapq.heappush(heap, entry)
                elif key < heap[0][0].key:
                    heapq.heapreplace(heap, entry)
        ordered = sorted(heap, key=lambda entry: entry[0].key)
        return [entry[2] for entry in ordered], {**focus_proof, **{entry[1]: entry[3] for entry in ordered}}

    def prepare_relations(self):
        group = self.spec["relation_query"]
        if not group["enabled"]:
            self.generic_relations = []
        elif self.dimensional("relation", group) and self.spec["composition"]["sort_relations"] == _DEFAULT_SORT:
            self.matched_relations = sum(cell[3] for cell in self.allowed_cells("relation", group))
        else:
            values = []
            for relation in self.scan("relation"):
                if self._relation_regime(relation) and self.budget.call(k._matches_group, relation, group):
                    values.append({**{key: relation[key] for key in ("id", "from_id", "to_id")},
                                   "key": self.budget.sort_key(relation, self.spec["composition"]["sort_relations"])})
            self.generic_relations = sorted(values, key=lambda item: item["key"])
            self.matched_relations = len(values)

    def relations(self, frontier=None, direction="either"):
        if self.generic_relations is not None:
            return iter(item for item in self.generic_relations if frontier is None or
                        (direction != "incoming" and item["from_id"] in frontier) or
                        (direction != "outgoing" and item["to_id"] in frontier))
        where, args = self.where("relation", "r")
        if frontier is None:
            return self._ordered("relation", where, args)
        streams = [self._ordered("relation", where, args, endpoint=(side, identifier))
                   for identifier in sorted(frontier) for side in ("from", "to")
                   if (side == "from" and direction != "incoming") or (side == "to" and direction != "outgoing")]
        def merged():
            last = None
            for item in heapq.merge(*streams, key=lambda item: (item["sort_key"], item["id"])):
                if item["id"] != last:
                    last = item["id"]
                    yield item
        return merged()

    def aliases(self, frontier, selected, remaining):
        origins = {}
        for identifier, node in sorted(self.payloads.load("node", frontier).items()):
            entity = node.get("entity_id")
            if isinstance(entity, str) and entity.startswith("tos."):
                origins.setdefault(entity, identifier)
        if not origins:
            return [], False, origins
        where, args = _source_where("n", self.spec["sources"])
        rows = self.read.query("SELECT n.id FROM knowledge_nodes n INDEXED BY knowledge_nodes_identity_seek "
            f"WHERE n.entity_id IN (SELECT value FROM json_each(?)) AND {where} "
            "AND n.id NOT IN (SELECT value FROM json_each(?)) ORDER BY n.id LIMIT ?",
            (_compact(list(origins)), *args, _compact(list(selected)), remaining + 1))
        return [row["id"] for row in rows[:remaining]], len(rows) > remaining, origins

    def node_sources(self, identifiers):
        if not identifiers:
            return {}
        return {row["id"]: row["source_graph"] for row in self.read.query(
            "SELECT id,source_graph FROM knowledge_nodes WHERE id IN (SELECT value FROM json_each(?)) LIMIT ?",
            (_compact(sorted(set(identifiers))), len(set(identifiers)) + 1))}

    def path_witness(self, start, condition):
        def adjacent(identifier):
            after = ""
            while True:
                packets = [self.read.query(
                    f"SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_{side}_seek "
                    f"WHERE {side}_id=? AND id>? ORDER BY id LIMIT ?", (identifier, after, self.block))
                    for side in ("from", "to")]
                identifiers = sorted({row["id"] for packet in packets for row in packet})[:self.block]
                if not identifiers:
                    return
                items = self.payloads.load("relation", identifiers)
                for relation_id in identifiers:
                    relation = items[relation_id]
                    if relation["source_graph"] in self.sources:
                        yield relation
                after = identifiers[-1]
        def walk(current, depth, nodes, relations):
            if depth == len(condition["steps"]):
                return {"path_id": condition["path_id"], "node_ids": nodes, "relation_ids": relations}
            step = condition["steps"][depth]
            for relation in adjacent(current):
                self.budget.path()
                if not step["relation_query"]["enabled"] or not self.budget.call(k._matches_group, relation, step["relation_query"]):
                    continue
                for neighbor in k._relation_neighbors(relation, current, step["direction"]):
                    if self.node_sources([neighbor]).get(neighbor) not in self.sources or not step["node_query"]["enabled"]:
                        continue
                    node = self.payloads.get("node", neighbor)
                    if not self.budget.call(k._matches_group, node, step["node_query"]):
                        continue
                    found = walk(neighbor, depth + 1, [*nodes, neighbor], [*relations, relation["id"]])
                    if found is not None:
                        return found
            return None
        return walk(start, 0, [start], [])

    def _eligible(self, item, basis, traversed):
        policy = self.spec["composition"]["endpoint_policy"]
        return (policy == "independent" or item["id"] in traversed
                or (policy == "both" and item["from_id"] in basis and item["to_id"] in basis)
                or (policy == "either" and (item["from_id"] in basis or item["to_id"] in basis)))

    def eligible(self, basis, traversed):
        if self.generic_relations is not None:
            items = [item for item in self.generic_relations if self._eligible(item, basis, traversed)]
            return len(items), iter(items)
        policy = self.spec["composition"]["endpoint_policy"]
        if policy == "independent":
            return self.matched_relations, self.relations()
        where, args = self.where("relation", "r")
        basis_json, traversed_json = _compact(sorted(basis)), _compact(sorted(traversed))
        if policy == "both":
            # SQLite can use only the from prefix for two IN predicates and
            # scan an arbitrarily large external degree. Fixed equality probes
            # keep work dependent on this bounded selected basis and eligible
            # pairs, not external edges. Large local bases can still exhaust
            # the reader VM budget; that is an explicit refusal, not a recount.
            ids = ("SELECT l.id FROM json_each(?) a CROSS JOIN json_each(?) b "
                   "CROSS JOIN knowledge_lens_order l INDEXED BY knowledge_lens_order_pair "
                   "WHERE l.kind='relation' AND l.from_id=a.value AND l.to_id=b.value "
                   "UNION SELECT value AS id FROM json_each(?)")
            id_args = [basis_json, basis_json, traversed_json]
        else:
            ids = ("SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from_seek WHERE from_id IN (SELECT value FROM json_each(?)) "
                   "UNION SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to_seek WHERE to_id IN (SELECT value FROM json_each(?)) "
                   "UNION SELECT value AS id FROM json_each(?)")
            id_args = [basis_json, basis_json, traversed_json]
        count = self.read.query(f"WITH eligible AS ({ids}) SELECT count(*) AS total FROM eligible e CROSS JOIN knowledge_relations r ON r.id=e.id WHERE {where}",
                                (*id_args, *args))[0]["total"]
        def window():
            after = "", ""
            while True:
                rows = self.read.query(f"WITH eligible AS ({ids}) "
                    "SELECT r.id,r.from_id,r.to_id,l.sort_key FROM eligible e "
                    "CROSS JOIN knowledge_lens_order l INDEXED BY sqlite_autoindex_knowledge_lens_order_1 "
                    "ON l.kind='relation' AND l.id=e.id "
                    "CROSS JOIN knowledge_relations r ON r.id=e.id "
                    f"WHERE {where} AND (l.sort_key,l.id)>(?,?) ORDER BY l.sort_key,l.id LIMIT ?",
                    (*id_args, *args, *after, self.block))
                if not rows:
                    return
                for row in rows:
                    if row["sort_key"] != row["id"].lower():
                        raise PublishedReadModelError("prepared lens order key differs from native ordering")
                    yield dict(row)
                after = rows[-1]["sort_key"], rows[-1]["id"]
        return count, window()


class PublishedLensService:
    def __init__(self, reader, *, limits=None):
        self.reader = reader
        self.limits = limits or PublishedLensLimits()

    def capability(self):
        available = self.reader.snapshot_binding['read_model_schema'] == 'tos_cloudflare_edge_read_model_v9'
        return {"available": available, **({"reason": "published-v9-required"} if not available else {}),
                "execution_version": LENS_EXECUTION_VERSION,
                "read_model_schema": "tos_cloudflare_edge_read_model_v9",
                "pagination": "stateless-reexecution-of-complete-bounded-lens-result",
                "default_counts": "exact-owner-built-source-kind-type-predicate-histograms",
                "generic_queries": "exact-within-explicit-budgets-or-refusal",
                "native_limits": dict(vars(self.limits)), "writes_to_tree": False}

    def focus(self, node_id, **options):
        return self.execute(k.focus_lens_spec(node_id, **options))

    def execute(self, value):
        public = k.normalize_lens_spec(value)
        def operation(read, top):
            if top["read_model_schema"] != "tos_cloudflare_edge_read_model_v9" or top["schema"] != LENS_READER_SCHEMA:
                raise PublishedReadModelError("prepared lens requires an owner-published v9 snapshot")
            raw, metadata = read.metadata(LENS_META_KEY, LENS_METADATA_MAX_BYTES)
            if emitted_row_digest(raw)["sha256"] != top.get("lens_sha256"):
                raise PublishedReadModelError("prepared lens metadata checksum differs")
            validate_lens_metadata(metadata, top["source_revision"])
            indexes = read.query("SELECT name FROM sqlite_master WHERE type='index' AND name IN (SELECT value FROM json_each(?))",
                                 (_compact(sorted(_INDEXES)),))
            if {row["name"] for row in indexes} != _INDEXES:
                raise PublishedReadModelError("prepared lens ordered-index migration is unavailable")
            bound = k.bind_lens_query_properties(metadata["query_properties"], public)
            plan = _Plan(read, top, metadata, bound, self.limits)
            return self._execute(plan, public)
        return self.reader._read(operation)

    @staticmethod
    def _execute(plan, public):
        spec = plan.spec
        focus = plan.focus()
        candidates, proofs = plan.select_nodes(focus)
        selected, inclusion = {}, {}
        if focus is not None:
            selected[focus["id"]] = focus
            inclusion[focus["id"]] = {"kind": "focus"}
        for item in candidates:
            if len(selected) >= spec["limits"]["nodes"]:
                break
            selected.setdefault(item["id"], item)
            inclusion.setdefault(item["id"], {"kind": "selector", "path_witnesses": proofs.get(item["id"], [])})
        matched_nodes = plan.matched_nodes + int(focus is not None and focus["id"] not in proofs)
        plan.prepare_relations()
        frontier, traversed, identity_limited = list(selected), set(), False
        for depth in range(spec["traversal"]["depth"]):
            if spec["traversal"]["profile"] == "overview":
                aliases, limited, origins = plan.aliases(frontier, selected, spec["limits"]["nodes"] - len(selected))
                identity_limited |= limited
                for identifier, item in plan.payloads.load("node", aliases).items():
                    selected[identifier] = item
                    inclusion[identifier] = {"kind": "identity-carrier", "via_node_id": origins[item["entity_id"]],
                                             "entity_id": item["entity_id"], "depth": depth}
                    frontier.append(identifier)
            next_frontier = []
            frontier_order = {identifier: index for index, identifier in enumerate(frontier)}
            for relation in plan.relations(frontier_order, spec["traversal"]["direction"]):
                if len(selected) >= spec["limits"]["nodes"] and len(traversed) >= spec["limits"]["relations"]:
                    break
                touched = False
                origins = sorted({relation["from_id"], relation["to_id"]} & frontier_order.keys(), key=frontier_order.__getitem__)
                for origin in origins:
                    for neighbor in k._relation_neighbors(relation, origin, spec["traversal"]["direction"]):
                        touched = True
                        if (neighbor not in selected and len(selected) < spec["limits"]["nodes"]
                                and plan.node_sources([neighbor]).get(neighbor) in plan.sources):
                            selected[neighbor] = plan.payloads.get("node", neighbor)
                            inclusion[neighbor] = {"kind": "traversal", "via_node_id": origin,
                                                   "via_relation_id": relation["id"], "depth": depth + 1}
                            next_frontier.append(neighbor)
                if touched and len(traversed) < spec["limits"]["relations"]:
                    traversed.add(relation["id"])
            frontier = list(dict.fromkeys(next_frontier))
            if not frontier:
                break
        eligible_count, eligible = plan.eligible(set(selected), traversed)
        relation_ids = []
        examined, exhausted = 0, True
        for relation in eligible:
            if len(relation_ids) >= spec["limits"]["relations"]:
                exhausted = False
                break
            examined += 1
            missing = list(dict.fromkeys(identifier for identifier in (relation["from_id"], relation["to_id"]) if identifier not in selected))
            if len(selected) + len(missing) > spec["limits"]["nodes"]:
                continue
            allowed = plan.node_sources(missing)
            for identifier in missing:
                if allowed.get(identifier) in plan.sources:
                    selected[identifier] = plan.payloads.get("node", identifier)
                    inclusion[identifier] = {"kind": "endpoint", "via_relation_id": relation["id"]}
            if relation["from_id"] in selected and relation["to_id"] in selected:
                relation_ids.append(relation["id"])
        if exhausted and examined != eligible_count:
            raise PublishedReadModelError("prepared lens eligible/count/order closure is incomplete")
        relations = plan.payloads.load("relation", relation_ids)
        return k.finalize_knowledge_lens(public, selected.values(), [relations[identifier] for identifier in relation_ids],
            source_revision=plan.top["source_revision"], authority_boundary=plan.top["authority_boundary"],
            execution_counts={"available_nodes": sum(cell[3] for cell in plan.scope_cells("node")),
                              "available_relations": sum(cell[3] for cell in plan.scope_cells("relation")),
                              "matched_nodes": matched_nodes, "matched_relations": plan.matched_relations,
                              "eligible_relations": eligible_count, "identity_expansion_limited": identity_limited},
            focus_node=focus, inclusion=inclusion, traversed_relation_ids=traversed)
