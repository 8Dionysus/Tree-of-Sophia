# Source-command discovery

Ask the existing source-command front door what it implements before selecting
any protected delegation or source target:

```bash
python mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py --discover
python mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py --discover --handler native-expression-responsibility
```

The JSON API is `discover_commands()` or
`run_local_command(None, request)` with:

```json
{"schema_version":"tos_source_command_discovery_request_v1","operation":"discover"}
```

An optional `handler_id` selects one exact handler returned by the complete
catalogue. The same request may be supplied on stdin without `--owner-config`.
Unknown selectors, operations, schema versions and extra top-level fields fail
closed. `--discover` ignores stdin and cannot be combined with `--owner-config`;
`--handler` requires `--discover`. Existing `--owner-config PATH` invocations and
their request/receipt envelopes remain unchanged.

## Meaning of the result

`tos_source_command_discovery_v1` contains the connected handler IDs, owner
configuration schema tags, request schema tags, owner routes, definitions,
typed source schema/profile handles, preconditions and operation shapes.
`handler_count` counts the returned handlers, not currently authorized grants.

Each `request_shape` is an exact top-level JSON Schema: required fields,
constant envelope/operation tags and `additionalProperties: false`. Empty
property schemas deliberately leave nested values to the named handler and
its typed contracts; they are not claims that arbitrary records or fields are
valid. Profile handles point back to the canonical type/relation registries.
Discovery does not load those registries or turn their read profiles into
writer operations. A profile schema being readable never grants writing.

`mutation` names the operation's possible source effect under an independently
valid grant. `none` means the command does not publish source changes; the
ordinary delegated `describe`/preparation/version commands may still read
protected inputs and require configuration. `delegated_operation_names` names
the grant vocabulary that the handler checks, not a set of permissions held
by the current caller. Form batches check each actual form operation.

The result always says `implemented_capabilities_not_authorized_now`,
`authorization_status: not_evaluated` and `grants_admission: false`. It does not
check or reveal account authority, grants, expiry, private contexts, targets,
source text, current versions, identity availability, rights or admission.
Execution still requires its separate current owner configuration and the
handler's full scope, version, dependency, idempotency and recovery checks.
For `sign.promote`, existing independent assessment admission remains a
precondition; discovery and serialization do not grant it.

## One implementation grammar

`source_command_contracts.py` supplies pure descriptors. Each connected
handler owns its descriptors beside its implementation. The front door's
fixed implementation imports collect those descriptors and use them for
configuration dispatch, operation dispatch and exact request-key validation.
The JSON catalogue is their projection, not a separately maintained operation
registry, an import path from caller data, or an executable schema extension.

Currently connected families include public source/Claim forms;
historical, declared-profile and standalone native creation; Sign promotion;
public record and Claim correction; selected native correction/recovery;
private TextUnit, metadata-profile and Claim transport; native Work/Expression
growth; qualified Expression translator attachment; and separately delegated
[Expression/Edition growth](NATIVE_EXPRESSION_EDITION_GROWTH.md). Distinct owner-schema
versions retain their actual typed-value and recovery boundaries.

Assessment-journal and semantic-registry evolution remain explicit owner
handoffs, not operations dispatched by this front door. Access CLI, HTTP,
WebMCP and native MCP remain read-only access surfaces; this catalogue does not
connect them to a writer or expose confidential configurations.

## Verification and limits

Discovery imports known implementation code but performs no owner/configuration,
source-target, credential, clock or network reads and writes no source files.
Its output is deterministic for the loaded implementation, with no target
currentness or remote/runtime availability claim.

Focused tests cover the complete connected grammar, shared descriptor dispatch,
missing/extra/unknown request keys, grant-free and direct-handler failure
boundaries, and fresh-process CLI/API parity. Existing handler tests own nested
schema, authority, byte-history, conflict, replay and recovery behavior. Old
committed receipts retain their historical request and serialization meaning;
new preparation includes the shared grammar implementation in its dependency
closure. A preparation from before an implementation change must be renewed.
