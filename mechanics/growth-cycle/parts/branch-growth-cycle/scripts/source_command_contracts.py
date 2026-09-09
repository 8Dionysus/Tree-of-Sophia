"""Pure handler-owned command grammar; no source, configuration or credential IO.

These descriptors are executable dispatch/shape inputs, not grants or a second
type registry. Nested values still belong to each handler's source validators.
"""
from dataclasses import dataclass
from typing import Callable

MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py'
REQUEST = 'tos_local_source_command_v1'
BASE_KEYS = frozenset({'schema_version', 'operation'})
COMMIT_KEYS = frozenset({'command_id', 'expected_configuration', 'expected_source',
                        'expected_revision', 'expected_dependencies'})
RECOVERY_KEYS = frozenset({'transaction_id', 'decision', 'expected_configuration'})
FORM_HANDLES = ('ToS/contracts/human-form.schema.json', 'ToS/contracts/human-form-set.schema.json')
RECORD_HANDLES = ('ToS/doctrine/semantic-interchange/entity-types.v1.json',
                  'ToS/contracts/semantic-entity-type-registry.schema.json')
CLAIM_HANDLES = ('ToS/doctrine/semantic-interchange/relation-types.v1.json',
                 'ToS/contracts/source-claim-record.schema.json')
OWNER_ROUTE = 'mechanics/growth-cycle/parts/branch-growth-cycle/README.md'
COMMON_PRECONDITIONS = (
    'Implemented capability is not authorized-now: select an independent current protected owner configuration.',
    'The handler rechecks its exact account, scope, source versions, dependencies and conflict/replay rules.',
    'A read-only source_record_profile or source_claim_profile grants neither writing nor source access.',
)


@dataclass(frozen=True)
class Operation:
    name: str
    keys: frozenset[str]
    definition: str
    mutation: str = 'none'
    delegated_operations: tuple[str, ...] = ()

    def shape(self, request_schema):
        return {'type': 'object', 'required': sorted(BASE_KEYS | self.keys),
                'properties': {key: ({'const': request_schema} if key == 'schema_version' else
                                     {'const': self.name} if key == 'operation' else {})
                               for key in sorted(BASE_KEYS | self.keys)},
                'additionalProperties': False}


def operation(name, keys=(), *, definition, mutation='none', grants=()):
    return Operation(name, frozenset(keys), definition, mutation, tuple(grants))


def describe():
    return operation('describe', definition='Inspect the independently delegated target and current command context.')


def inspect_version(*, claim=False):
    return operation('inspect-version', {'source', *({'claim_id'} if claim else ())},
                     definition='Resolve an exact retained source version through its continuous owner history.')


def recovery(name):
    return operation(name, RECOVERY_KEYS, definition='Resume or roll back the exact pending selected-file transaction.',
                     mutation='selected_transaction_recovery', grants=(name,))


@dataclass(frozen=True)
class Handler:
    handler_id: str
    owner_schemas: tuple[str, ...]
    operations: tuple[Operation, ...]
    run: Callable
    definition: str
    configure: Callable | None = None  # None means the front door's existing built-in parser.
    request_schema: str = REQUEST
    owner_route: str = OWNER_ROUTE
    typed_handles: tuple[str, ...] = ()
    profile_selection: str = 'The handler validates the independently selected exact source contract.'
    preconditions: tuple[str, ...] = ()
    manages_publication: bool = False

    def __post_init__(self):
        if (not self.owner_schemas or len(set(self.owner_schemas)) != len(self.owner_schemas)
                or not self.operations or len({op.name for op in self.operations}) != len(self.operations)
                or any(BASE_KEYS & op.keys for op in self.operations)):
            raise ValueError('handler descriptor has duplicate or invalid command grammar')

    def validate_request(self, request):
        if not isinstance(request, dict) or not isinstance(request.get('operation'), str):
            raise ValueError('source command must name an implemented operation')
        selected = next((op for op in self.operations if op.name == request['operation']), None)
        if (selected is None or request.get('schema_version') != self.request_schema
                or set(request) != BASE_KEYS | selected.keys):
            raise ValueError('source command fields, operation or schema do not match this handler')
        return selected

    def public(self):
        return {'handler_id': self.handler_id, 'implementation_ref':
                    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/' +
                    ('source_commands' if self.run.__module__ == '__main__' else self.run.__module__) + '.py',
                'owner_schema_versions': list(self.owner_schemas), 'request_schema_version': self.request_schema,
                'definition': self.definition, 'owner_route': self.owner_route,
                'typed_contract_handles': list(self.typed_handles), 'profile_selection': self.profile_selection,
                'preconditions': [*COMMON_PRECONDITIONS, *self.preconditions],
                'authorization_status': 'not_evaluated', 'grants_admission': False,
                'operations': [{'operation': op.name, 'definition': op.definition,
                    'request_shape': op.shape(self.request_schema),
                    'shape_scope': 'exact_top_level_keys_and_tags; nested_values_validated_by_handler',
                    'mutation': op.mutation, 'delegated_operation_names': list(op.delegated_operations),
                    'grants_admission': False} for op in self.operations]}
