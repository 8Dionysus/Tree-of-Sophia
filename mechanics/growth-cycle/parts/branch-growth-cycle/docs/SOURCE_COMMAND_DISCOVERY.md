# Source-command discovery

Ask the existing source-command front door what it implements before selecting
any protected delegation or source target:

```sh
tos-native-owner-command source-commands --discover
tos-native-owner-command source-commands --discover --handler native-expression-responsibility
```

The Rust API is `tos_command::source_native_cli::discover_commands(handler)`.
An optional handler ID selects one exact handler from the complete catalogue.
Unknown selectors and additional CLI arguments refuse. Discovery ignores stdin
and takes no owner configuration or invocation. Execution uses the separate
`source-commands --invocation ABSOLUTE_PATH` entry and its protected inputs.
HTTP `GET /commands/catalog` uses the same function under the listener's
existing request authentication.

## Meaning of the result

`tos_source_command_discovery_v1` contains the connected handler IDs, owner
configuration schema tags, request schema tags, owner routes, definitions,
typed source schema/profile handles, preconditions and operation shapes.
`handler_count` counts the returned handlers, not currently authorized grants.

Each `request_shape` is an exact top-level JSON Schema: required fields,
constant envelope/operation tags and `additionalProperties: false`. Empty
property schemas delegate nested validation to the named handler and its typed
contracts. Profile handles point to the canonical type/relation registries.
Discovery exposes those handles; execution loads applicable contracts and
checks the independently issued writer grant.

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

The authored packaged descriptor is
`rust/crates/tos-command/src/source_command_catalog.json`. Rust CLI and HTTP
embed it directly. Each entry names the native implementation that owns
execution and nested validation; descriptor edits accompany a change to that
owner's grammar. Discovery describes the connected implementation and supplies
no delegation. The former Python descriptor builder is retired.

Currently connected families include public source/Claim forms;
historical, declared-profile and standalone native creation; Sign promotion;
public record and Claim correction; selected native correction/recovery;
private TextUnit, metadata-profile and Claim transport; native Work/Expression
growth; qualified Expression translator attachment; and separately delegated
[Expression/Edition growth](NATIVE_EXPRESSION_EDITION_GROWTH.md). Distinct owner-schema
versions retain their actual typed-value and recovery boundaries.

The separately selected [private native extraction and first-segmentation
route](NATIVE_TEXT_LAYER_CONSTRUCTION.md) exposes `owner-local-text-layer-create`
and adds the v2 layer-only input to `owner-local-text-unit-create`. Discovery
lists implemented grammar only, never an existing source, usable payload,
private selector/slot, current grant, rights decision or accepted TextLayer.
The same route's separate `owner-local-text-layer-derive` handler exposes
correction, Unicode normalization and recording supplied OCR/transcription.
One independent grant chooses one operation; reported upstream methods are not
provider execution receipts and new layers do not inherit prior quality.

The translation-alignment owner exposes its [native record
route](NATIVE_TRANSLATION_ALIGNMENT.md) through
`owner-local-native-translation-alignment`: exact private create/revise,
metadata-only version inspection and retained native recovery. Alignment
subject identity, descriptive record and Claim versions each retain their own
role. The operation captures supplied mappings; aligner execution and
translation assessment require their respective evidence. Packet-v1 review
semantics remain unchanged.

The distinct `owner-local-text-layer-record-owner-ocr` handler records one
authenticated signed `abyss-stack` OCR result under a separate protected grant.
It calls only the pinned owner's verification modes, never OCR execution;
copied signed metadata remains independently verifiable without reading text.
The distinct `owner-local-text-layer-record-owner-page-ocr` handler retains
the original PDF identity and separately pinned retained page image, authenticates
the owner's new execution/capture receipt and never claims a fresh render.
Neither handler grants image disclosure or textual quality.

Assessment-journal and semantic-registry evolution use their explicit owner
routes. Access CLI, HTTP, WebMCP and native MCP retain read-only access
contracts. Source commands execute through the separately configured
source-owner front door.

## Verification and limits

Discovery reads the packaged descriptor and performs no owner/configuration,
source-target, credential, clock or network reads and writes no source files.
Its output is deterministic for the loaded implementation, with no target
currentness or remote/runtime availability claim.

The native catalogue regression checks all handler selectors and the shared
authority boundary. Family conformance owns request validation, authority,
byte history, conflict, replay and recovery. Existing committed receipts keep
their historical request and serialization meaning; a preparation whose
implementation dependencies changed must be renewed.
