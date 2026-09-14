"""Exact indexed neighborhoods for offline source-to-projection assembly.

The selected publication proves existing carrier incidence, not source change
authority, future membership, claim/reducer closure or semantic acceptance.
No source load, normalization, scan fallback, auxiliary index or write occurs.
"""
from dataclasses import dataclass

from .prepared_publication import SCHEMA
from .published_read_metadata import PublishedReadModelError
from .published_read_model import PublishedKnowledgeReadModel, PublishedReadBudgetExceeded


@dataclass(frozen=True)
class NeighborhoodLimits:
    max_seed_nodes: int = 128
    max_nodes: int = 1024
    max_relations: int = 1024

    def __post_init__(self):
        if any(type(value) is not int or value < 1 for value in vars(self).values()):
            raise ValueError('neighborhood limits must be positive integers')


def _identifier(value):
    if (type(value) is not str or not value or len(value) > 4096
            or len(value.encode('utf-8')) > 4096):
        raise ValueError('neighborhood requires exact nonempty bounded identifiers')
    return value


def capture_prepared_neighborhood(reader, *, node_ids=(), entity_id=None, limits=None):
    """Capture full rows in ONE selected read snapshot, refusing partial output.

    Supply either exact node IDs or one exact semantic entity ID. Entity lookup
    includes every existing representation, without collapsing the rows. Every
    incoming/outgoing relation of the resulting seeds and every opposite
    endpoint is returned once. Incidence of opposite endpoints is NOT expanded.
    Source-order tokens are captured for later compare-and-swap publication.

    This operation owns no transaction beyond the reader's ordinary read-only
    transaction. A later writer must still guard the returned exact binding.
    All ordinary reader VM/row/byte budgets and post-read ABA checks apply in
    addition to the neighborhood limits. There is no truncation or retry.
    """
    if not isinstance(reader, PublishedKnowledgeReadModel):
        raise ValueError('exact selected PublishedKnowledgeReadModel required')
    limits = limits or NeighborhoodLimits()
    if not isinstance(limits, NeighborhoodLimits):
        raise ValueError('explicit NeighborhoodLimits required')
    if type(node_ids) not in (tuple, list) or len(node_ids) > limits.max_seed_nodes:
        raise ValueError('bounded exact node ID list required')
    requested = tuple(_identifier(value) for value in node_ids)
    if len(set(requested)) != len(requested):
        raise ValueError('duplicate neighborhood seed')
    if entity_id is not None:
        _identifier(entity_id)
    if bool(requested) == (entity_id is not None):
        raise ValueError('select either exact node IDs or one entity ID')

    def operation(read, top):
        if top['read_model_schema'] != SCHEMA:
            raise PublishedReadModelError('offline neighborhood requires local prepared profile')
        # Named indexes must really have the required covering prefix. Merely
        # finding their names is insufficient to establish addressed work.
        indexes = {
            'knowledge_nodes_identity_seek': ('entity_id', 'id'),
            'knowledge_relations_from_seek': ('from_id', 'id'),
            'knowledge_relations_to_seek': ('to_id', 'id'),
        }
        for name, columns in indexes.items():
            table = 'knowledge_nodes' if name.startswith('knowledge_nodes_') else 'knowledge_relations'
            owners = read.query(f'PRAGMA index_list({table})')
            actual = [row for row in read.query(f'PRAGMA index_xinfo({name})') if row['key']]
            if (not any(row['name'] == name and row['partial'] == 0 for row in owners)
                    or tuple(row['name'] for row in actual) != columns
                    or any(row['coll'] != 'BINARY' or row['desc'] != 0 for row in actual)):
                raise PublishedReadModelError('neighborhood index layout requires explicit bootstrap')
        if entity_id is None:
            seeds = sorted(requested)
        else:
            rows = read.query('SELECT id FROM knowledge_nodes INDEXED BY knowledge_nodes_identity_seek '
                'WHERE entity_id=? ORDER BY id LIMIT ?', (entity_id, limits.max_seed_nodes + 1))
            if len(rows) > limits.max_seed_nodes:
                raise PublishedReadBudgetExceeded('neighborhood representation budget exceeded')
            seeds = [row['id'] for row in rows]
        if not seeds:
            raise ValueError('selected entity has no prepared representation')
        if len(seeds) > limits.max_nodes:
            raise PublishedReadBudgetExceeded('neighborhood node budget exceeded')
        nodes, relations, order = {}, {}, {'node': {}, 'relation': {}}

        def body(kind, identifier):
            values = read.items(kind, 'id=?', (identifier,), 1)
            if len(values) != 1:
                raise PublishedReadModelError('neighborhood selected carrier/endpoint is missing')
            address = read.query('SELECT doc_id,source_order FROM prepared_documents '
                                 'WHERE kind=? AND id=?', (kind, identifier))
            if (len(address) != 1 or any(type(address[0][field]) is not int
                    or not 0 <= address[0][field] <= 9_007_199_254_740_991
                    for field in ('doc_id', 'source_order'))):
                raise PublishedReadModelError('neighborhood prepared address/order is invalid')
            order[kind][identifier] = address[0]['source_order']
            return values[0]

        for identifier in seeds:
            nodes[identifier] = body('node', identifier)
            if entity_id is not None and nodes[identifier]['entity_id'] != entity_id:
                raise PublishedReadModelError('neighborhood representation identity differs')
        for identifier in seeds:
            for direction in ('from', 'to'):
                rows = read.query(f'SELECT id FROM knowledge_relations INDEXED BY '
                    f'knowledge_relations_{direction}_seek WHERE {direction}_id=? ORDER BY id LIMIT ?',
                    (identifier, limits.max_relations + 1))
                if len(rows) > limits.max_relations:
                    raise PublishedReadBudgetExceeded('neighborhood relation budget exceeded')
                for row in rows:
                    relation_id = row['id']
                    if relation_id in relations:
                        if relations[relation_id][direction + '_id'] != identifier:
                            raise PublishedReadModelError('neighborhood incidence differs from carrier')
                        continue
                    if len(relations) >= limits.max_relations:
                        raise PublishedReadBudgetExceeded('neighborhood relation union budget exceeded')
                    relation = body('relation', relation_id)
                    if relation[direction + '_id'] != identifier:
                        raise PublishedReadModelError('neighborhood incidence differs from carrier')
                    relations[relation_id] = relation
                    for endpoint in (relation['from_id'], relation['to_id']):
                        if endpoint not in nodes:
                            if len(nodes) >= limits.max_nodes:
                                raise PublishedReadBudgetExceeded('neighborhood endpoint budget exceeded')
                            nodes[endpoint] = body('node', endpoint)
        return {
            'schema': 'tos_prepared_neighborhood_v1', 'binding': reader.snapshot_binding,
            'seed_node_ids': seeds, 'entity_id': entity_id,
            'nodes': [nodes[key] for key in sorted(nodes)],
            'relations': [relations[key] for key in sorted(relations)],
            'source_order': order,
            'scope': {'complete_existing_seed_incidence_verified': True,
                      'returned_endpoint_closure_verified': True,
                      'opposite_endpoint_incidence_expanded': False,
                      'source_transition_verified': False,
                      'new_source_membership_verified': False,
                      'claim_and_reducer_closure_verified': False,
                      'semantic_acceptance': False, 'consumer_switched': False},
        }

    return reader._read(operation)
