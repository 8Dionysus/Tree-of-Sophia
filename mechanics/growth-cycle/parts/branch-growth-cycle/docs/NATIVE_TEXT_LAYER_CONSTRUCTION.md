# Private native TextLayer and first TextUnit construction

The initial source-owner route implements two separately delegated steps:
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

The separate additive derivation grant below now implements correction,
Unicode normalization and recording of supplied source-bound OCR/transcription
results. Schema enum support alone is not an execution route: the original
extraction grant cannot authorize any of these operations.

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

## Additive correction, normalization and supplied-result recording

`owner-local-text-layer-derive` is a separate discoverable handler in the same
`source_text_layer_commands.py` module. Its protected mode-0600 grant is
`tos_local_text_layer_derive_owner_v1`. It selects exactly one operation:

- `text-layer.correct`: apply explicitly supplied ordered code-point edit
  proposals to one exact predecessor, preserving everything outside them;
- `text-layer.normalize`: actually execute one declared NFC, NFD, NFKC or NFKD
  transformation under the named Python Unicode database version;
- `text-layer.record-transcription`: record supplied manual or model
  transcription bytes, without executing the reported upstream transcription;
- `text-layer.record-ocr`: record supplied OCR bytes, without launching an OCR
  engine or producing an OCR execution receipt.

All four create a new private `source-text-layer.v1.json`, never an update to
the input. Correction/normalization has one exact predecessor, a new opaque
layer identity, version `previous.layer_version + 1`, and `supersedes_layer_ref`
equal to the predecessor identity. The raw predecessor record/content and
original acquired File remain unchanged. Supplied OCR/transcription instead
starts a new version-1 layer from an already acquired File and existing exact
source anchor; it does not fabricate a predecessor transcription.

### Derivation grant and fixed policy

The common top-level fields retain the extraction grant's exact account,
principal/authority/expiry, source context/path, source scope, four metadata
refs/digests, manifest digest, language, maker and limits. The differences are:

- `allowed_operations` is exactly one of the operations above; `identities`
  contains only independently selected `layer_id` and `provenance_event_id`.
  Existing source anchors keep their identities and exact bytes.
- `derivation_access` keeps separate current authority, expiry, visibility and
  exact rights bindings, with `operation` equal to `correction`,
  `unicode_normalization`, `manual_transcription`, `model_transcription` or
  `ocr` as selected by the fixed policy. Both Item/File and the **new layer**
  need their own applicable existing derivation-rights basis.
- `policy` must equal `source_text_layer_proposal.derivation_policy(operation,
  unicode_form=...)`. Normalization requires an explicit supported form; other
  operations use `none`. For `record-transcription`, the optional
  `transcription_method` selects `manual_transcription` (default) or
  `model_transcription`. This helper returns inert versioned rules, not code
  selected by the caller. Normalization maker must be software, with method
  `tos.unicode.normalize.v1` and version equal to the policy's actual Unicode
  database version.
- `member` and `selector` do not occur in this grant. `input` and `material`
  have the exact operation-specific shapes below.

For correction/normalization, `input` is `{kind: "text_layer", binding: ...}`,
where the binding uses existing `tos_native_text_layer_binding_v1` and agrees
with the independently pinned metadata refs. `source_access` contains
`read_scope: "exact_text_layer"`, `access_allowed: true`, exact positive
`byte_size`, `authority_ref` and `expires_at`, with no payload root. The input
must be the whole exact UTF-8 representation with scope `[0, length)` and the
same source/language as the new layer. Its own recorded rights are checked
before its content is opened. No original payload is opened by these methods.

Correction `material` is `{edits: [...]}`. Each edit has exactly `start`, `end`,
`input_exact`, `input_sha256`, `output_exact`, `reason`, and `confidence` in
`[0, 1]`. Spans are ordered, nonoverlapping, half-open Unicode code points;
the input bytes/digest must match the predecessor. Insert/delete/replace are
derived from the supplied spans and strings, never from an implicit diff.
Every emitted edit retains input/output coordinates, exact text and hashes,
the configured responsible maker and original anchor refs, with `proposed`
status. A correction does not create human review merely because its supplier
is human. Source-near correction of an already normalized layer is unsupported;
it cannot silently erase that predecessor's normalization posture.

Normalization `material` is `{}`. The executor records one explicit whole-text
Unicode operation, including when the result bytes happen to be unchanged.
That means a declared normalization was applied, not that a textual error was
found. The new layer is `normalized_text`; it receives no source-fidelity or
diplomatic authority. No trimming, whitespace collapse or unrelated editorial
rewrite is performed.

For supplied OCR/transcription, `input` is `{kind: "acquired_file", anchor:
{anchor_id, record_ref, record_sha256}}`. The existing v2 anchor must bind the
same Item/File/digest and manifest media type. Supported exact Files are EPUB,
PDF, PNG, JPEG, TIFF and WebP; the command verifies original File fixity but
does not execute the anchor selector, render a page or inspect glyphs. Its
`source_access` has the extraction grant's exact acquired-File fields,
including the explicit disjoint payload root and byte size.

Here `material` contains exactly `content_ref`, `content_sha256`, `byte_size`,
`access_allowed: true`, its independent `authority_ref`/`expires_at`,
`provider_execution: "not_observed"`, and `reported_maker:
{maker_type, agent_ref, method, version}`. The content ref selects an existing
private file outside the new package. Its exact strict UTF-8 bytes are copied
without newline or Unicode rewriting. Manual transcription requires a reported
human maker; model transcription a reported model; OCR a reported software,
model or mixed producer. These are **supplied declarations**, distinct from
the command principal that records them. This route does not authenticate their
identity, method execution, competence, completeness or fidelity to the anchor.
It cannot substitute a declared provider name for observed execution evidence.

### Bounds, retained evidence and downstream route

Input and output text are each at most 128 KiB, further narrowed by the grant;
correction has at most 128 edits, with per-reason length bounded at 2,048
characters. This also bounds explicit input/output strings in the layer record.
No quadratic edit search runs. Original acquired Files retain the 512 MiB
streamed-fixity ceiling; no image/PDF decompression or provider process is run.
Existing metadata-count, lineage-depth and cumulative resolver budgets remain;
the derivation writer allows two bounded text representations for independent
immediate-predecessor delta verification. The 60-second cooperative deadline,
12-file/12-MiB package and 18-MiB retained-plan ceilings still apply.

The package contains the new layer, `content.txt`, `derivation-policy.json`,
and the existing retained configuration/inputs/request/environment/provenance/
receipt files. It references existing source anchors instead of manufacturing
new ones. Request grammar, expected configuration/dependencies, absent initial
target, protected locks, exact idempotent retry and interruption recovery are
the same construction route described below. An expired or changed grant,
predecessor, supplied result, rights, implementation or third-state file blocks
retry; evidence is retained for explicit owner action, not erased or reissued.

Provenance distinguishes actual correction/normalization from supplied-result
capture. The latter uses `annotation`, has no model invocation, and explicitly
says that upstream OCR/transcription was not executed or authenticated. Its
layer method describes the reported representation origin, not an execution
receipt. All outputs remain unreviewed, with no accepted use, competence,
promotion or publication authority; predecessor uncertainty stays unresolved.

`NativeTextBindingResolver` verifies exact predecessor record/content/version,
source scope and retained policy/configuration. Exact reads independently replay
the explicit edits; they do not replay a historical OCR provider or use the
current Unicode implementation as evidence of historical execution. Existing
first segmentation can consume the real new layer without inheriting quality.

The current `NativeLayerAssessmentSources._metadata` adapter has a separate
exact derived-layer comparison for the bounded EPUB/XHTML
source-view profile, documented in [native assessment](NATIVE_TEXT_LAYER_ASSESSMENT.md).
Images/PDFs and unsupported selectors still need their own source renderer.
A prior extraction assessment is not a quality basis for its successor.
The source-visible reviewer must assess the exact new layer against its
predecessors and original source, then use the existing assessment journal for
a separate current purpose-scoped quality basis. Construction and comparison
do not themselves establish real-source acceptance or Foundation-wide quality.

## Authenticated OCR of a retained PDF page

The distinct `tos_local_text_layer_record_owner_page_ocr_v1` protected profile
selects only `text-layer.record-owner-page-ocr`. Its input kind is
`retained_pdf_page`; original `source_scope` and whole-page anchor still name
the exact acquired PDF, never the derived PNG. It retains all independently
issued source, derivation, output and authenticated-evidence grants of the
owner OCR route, plus exact `material.input_representation` binding to the
retained image, one-based page, frozen render manifest/sample plan and unsigned
historical receipt. The fixed `abyss-stack` retained-page verifier authenticates
the new OCR execution and honest current input-verification capture. ToS never
runs a renderer or OCR provider through this recording operation.

The immutable 12-file package includes copied signed receipt, signature and
public key. Metadata-only source resolution authenticates these three files
without opening the original PDF, PNG or OCR text. Exact content reads bind
the signed output digest. Recording/recovery cannot reexecute OCR, sign an old
rendering event retroactively or mark raw OCR reviewed. The separate
[image/OCR assessment v6](NATIVE_IMAGE_OCR_ASSESSMENT.md) owns current
source-visible comparison and disclosure bounds; old supplied `record-ocr`
continues to mean supplied unverified origin, not observed execution.

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
