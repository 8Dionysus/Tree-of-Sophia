# Private native TextLayer and first TextUnit construction

This source-owner route implements two separately delegated steps:
`text-layer.create` extracts a bounded representation from an already acquired
exact EPUB member; `text-unit.create` with its v2 owner configuration constructs
the first explicit interval partition of one real TextLayer. Neither step needs
an older TextUnit. Neither invents a bootstrap packet, text assessment, accepted
reading, semantic Description, Occurrence, Lexeme or publication permission.

Existing source-text-layer v1, source-anchor v2, source-text-unit packet v1 and
provenance-event v2 contracts own the output grammar. The new
[`native-text-layer-binding` contract](../../../../../ToS/contracts/native-text-layer-binding.schema.json)
is a selection handle, not a replacement source record or assessment subject.
Existing v1 unit-construction and public Occurrence gates keep their contracts.

## Separately selected extraction grant

The common `source_commands.py` front door dispatches the protected mode-0600
`tos_local_text_layer_create_owner_v1` configuration to
`source_text_layer_commands.py`. Its exact top-level fields are:

- `schema_version`, current local `uid`, `principal_id`, `authority_ref`,
  `expires_at`, and `allowed_operations: ["text-layer.create"]`;
- `source_context_ref`: an independently selected absolute protected context
  configuration; `source_path`: a new private-prefix package ending in
  `source-text-layer.v1.json`; the containing collection exists, but the new
  layer package directory must not already exist;
- `source_scope`: `work_ref`, `expression_ref`, `edition_ref`, `item_ref`,
  `file_ref`, `file_sha256`; File identity is `tos.file.sha256.<raw SHA-256>`;
- `source_record_refs` and `source_record_sha256`: exact maps for `work`,
  `expression`, `edition`, `item`; refs select authored corpus records, and
  `manifest_sha256` separately pins the Item's manifest bytes;
- `source_access`: `read_scope: "exact_acquired_file"`, `access_allowed: true`,
  absolute `payload_root`, exact positive `byte_size`, its own `authority_ref`
  and `expires_at`;
- `derivation_access`: `derivation_allowed: true`,
  `operation: "structural_extraction"`, `rights_record_refs: [{ref, sha256}]`,
  `content_visibility: "local_only"`, its own `authority_ref` and `expires_at`;
- `member: {member_path, member_sha256}`, exact `selector: {type, scheme, value}`,
  and the complete supported versioned `policy` object;
- `identities`: opaque independently delegated `layer_id`, `anchor_id`,
  `passage_id`, `provenance_event_id`, each `tos.<kind>.sid-<32 lowercase hex>`;
- `maker: {maker_type, agent_ref, method, version}` with `maker_type: "software"`
  and `agent_ref` equal to the principal; `language`; and
  `limits: {max_output_bytes, max_seconds}`.

The context still has exactly its existing public/source-contract and private
roots. The explicitly granted payload root is a separate input adapter, never
a fallback namespace. It must be disjoint from the private output root. The
original physical file is selected as `payload_root / <Item metadata parent
relative to ToS/source-witnesses> / <manifest payload relative_path>`. No search,
download, source acquisition, directory creation for missing parents, alias
resolution or implicit choice of payload root occurs.

Metadata topology, raw metadata/manifest hashes, the local-only EPUB manifest
entry, separate current reading/derivation grants, exact Item/File rights and
an exact decision for the **new layer identity** are checked before original
payload bytes are opened. Item-only permission is not sufficient for this new
content-bearing output. Supported derivative posture is `local_research_only`
or unconditional `allowed`; denied, conflicting, inactive, conditional and
permission-required decisions fail closed. This is enforcement of already
selected owner rights, not a legal assessment or a way to create those rights.

## Narrow, versioned extraction profile

`scripts/source_text_layer_proposal.py` owns the pure implementation and exact
`DEFAULT_POLICY` object, tagged `tos_xhtml_text_extraction_policy_v1`. Passing
a different policy object is unsupported; the retained policy's raw bytes and
digest are bound by the layer. The command selects no caller-supplied code.

The input is one strict UTF-8 XHTML document in the XHTML namespace. Supported
selectors have `type: "structural"` and either:

- `scheme: "tos.xhtml.element-ordinal.v1"`, `value: "p:2"` for a one-based,
  document-order ordinal of an exact allowed XHTML local element name;
- `scheme: "tos.xhtml.element-id.v1"`, an exact unique `id` or XML `id` value.

Zero or multiple matches fail. The selected element may contain supported
transparent inline markup and `br`, which contributes one LF. Text nodes are
concatenated in document order without the selected root's tail, trimming,
space collapse, inserted block separators or Unicode normalization. NFD,
existing LF and spaces survive. Bounded comments are omitted; CDATA contributes
literal character data. Unknown markup inside the selected element fails.

This first profile deliberately rejects raw CR, **all ampersands** (including
predefined/numeric references), DTDs and processing instructions other than an
XML 1.0 declaration with optional UTF-8 encoding. It does not repair unsupported
input. It is neither a general EPUB renderer nor OCR/correction/normalization.

Resource ceilings are explicit: original File 512 MiB; ordinary non-ZIP64,
single-disk ZIP with at most 2,048 members, 1 MiB central directory, 1,024-byte
names, 16 MiB per expanded ZIP member and 64 MiB declared aggregate expansion.
Actual directory counts are checked before `ZipInfo` allocation. Selected
stored/deflated member data is streamed with bounded expansion, local/central
name checks, exact size, CRC and raw SHA-256; encrypted, duplicate, escaping or
symlink members fail. The stricter XHTML profile accepts at most 8 MiB, 65,536
elements, depth 64, 16 KiB markup tokens and 128 attributes per start tag,
including namespace declarations. Token checks precede parser allocation.
Output is at most 8 MiB, further narrowed by the grant. The command's monotonic
budget is at most 60 seconds; bounded parser work is checked before/after, not
an operating-system hard-timeout guarantee.

## Request and immutable output

Use the common `tos_local_source_command_v1` envelope. `describe` and
`prepare-create` accept no source or extraction fields from the request.
`describe` opens no payload. Preparation computes the exact dependency token.
`text-layer.create` adds only `command_id`, `expected_configuration`,
`expected_dependencies`, `expected_source: null`, `expected_revision: null`.
The first two tokens come from preparation. Extra request fields fail.

The new mode-0700 package contains mode-0600 files:

- `source-text-layer.v1.json`, `source-anchor.v2.json`, `content.txt`,
  `extraction-policy.json`;
- retained `source-create-owner-configuration.json` and
  `source-create-inputs.json`;
- `source-create-request.json`, `source-create-environment.json`,
  `source-create-provenance.jsonl`, `source-create-receipt.json`.

The layer is unreviewed `machine_transcription` from `structural_extraction`,
with no predecessor layer or invented human review. Its anchor returns through
the exact original File to the selected member and structural selector. The
event is `native_extraction`, distinct from later `segmentation`. Whole-text
SHA-256, exact UTF-8 scope, policy/configuration, maker, input/output provenance
and recorded rights remain private. No extraction quality judgment is implied.

Public command responses expose only operation/status information, opaque
configuration/dependency/receipt digests and `grants_admission: false` with
`content_disclosure: "withheld"`; no path, selector, content, native identity,
span or raw private receipt is returned. Grant-free discovery reads no context,
grant, rights, payload, selector or private target.

## First segmentation from the real layer

`tos_local_text_unit_create_owner_v2` retains the v1 unit grant's exact top-level
shape, declared source-reading authority, IDs, explicit scope, method and
bounded spans/gaps. Only `source_binding` selects the new layer-only schema:
`schema_version: "tos_native_text_layer_binding_v1"`,
`text_layer: {record_ref, record_sha256, layer_id, layer_version}` and
`source_record_refs: {work, expression, edition, item}`. No packet, unit or
segmentation identity is supplied as a predecessor.

`NativeTextBindingResolver.resolve_layer` validates the actual layer, lineage,
policy/configuration, source anchors, corpus/manifest and recorded rights.
Exact private reading is independently explicit; rights precede representation
I/O. It never opens the original payload and never claims public availability
or applied assessment. The delegated interval must lie inside the real layer's
declared scope, not a fabricated unit. New packet fields are derived directly
from that verified layer and binding, with ordinary explicit span/gap partition.

V2 responses withhold slots, scope, paths, private receipt and prepared file
hashes. The ordinary private packet has an additional `source-create-inputs.json`
binding consumed raw metadata, representation, context, implementation and
runtime bytes. V1 replay remains historical-request/current-validation; V2 and
layer creation require their exact pinned construction inputs still to match.
An unrelated new identity elsewhere is not historical source-byte drift, but
collision discovery still runs and source/right/context changes are refused.

## No-replace commit, replay and retained recovery

Both new routes use existing source-owner locks and atomic no-replace directory
publication. A deterministic private `.native-construction-<digest>.pending`
control per exact target/command retains a mode-0600 bounded `plan.json` before
staged output writes. It records the complete exact byte set, request digest and
target. Package limits are 12 files/12 MiB; the base64 recovery plan is at most
18 MiB. The confidential plan remains outside the final source package, even
after completion; it is not a second accepted source or public registry.

Retry verifies the plan, original request/receipt, every retained input/output,
current rights/grants and directory identities. It fills only absent staged
files and can resume after any fully written file. A torn file, absent/torn
plan, changed control/ancestor, foreign residue or occupied destination fails
closed and remains for explicit owner review. Nothing is overwritten or
automatically erased. Repeating the same command does not multiply stages.
Operators should reuse that exact command after recoverable interruption, not
create an unbounded series of new command IDs. Changed implementation/input
requires a new explicitly chosen route, not rewriting committed evidence.

Exact retry preserves the committed package and returns the same receipt
digest. The issuer still owns stability against noncooperating same-UID edits;
path/digest rechecks are not isolation or a cross-filesystem transaction.

## Verification boundary

Focused synthetic tests cover bounded parser/resource refusal, exact text,
separate authority before payload I/O, layer-only bootstrap, stable input
binding, redacted results, no-replace replay, interruption/torn recovery and
the existing private Occurrence consumer/public-denial boundary. These tests
do not adopt any real witness, rights decision, source text or assessment, and
do not prove CI, landing, publication, runtime installation or semantic quality.
