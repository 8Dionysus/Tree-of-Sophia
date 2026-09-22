# Native translation-alignment records

This confidential native command route belongs to the existing
[`translation-alignment`
owner](../../../../../ToS/contracts/translation-alignment-packet-v1.schema.json).
Its additive [`native record
contract`](../../../../../ToS/contracts/native-translation-alignment-record-v1.schema.json)
reuses the v1 mapping, side, maker, evidence, granularity, rights and
authority definitions. The earlier packet retains its human-review and
public-use semantics; the native record carries an unassessed proposal.

## Identity and exact history

An Alignment is a stable opaque subject, independent of its text labels and
current mapping. It has `alignment_id` and no independent version counter.
`record_id` identifies its descriptive record lineage; `record_version`
versions the whole immutable description. `claim_id` identifies one proposed
correspondence; `claim_version` versions its descriptive qualifications.

| Change kind | Record | Alignment subject | Claim |
| --- | --- | --- | --- |
| `initial` | New ID, version 1, no predecessor | New ID | New ID, version 1 |
| `describe` | Same ID, next version | Same ID | Same ID, next version; mapping unchanged |
| `remap` | Same ID, next version | Same ID | New ID, version 1; mapping actually differs |
| `competing` | New ID, version 1 | New ID | New ID, version 1; explicit alternative record refs |

Every descriptive successor has an exact predecessor
`{record_ref, record_id, record_version, sha256}`. The digest addresses the raw
record file, including its serialization. Its Claim predecessor is
`{claim_id, claim_version, sha256}`, with the digest computed over the
source-command canonical JSON serialization of the Claim **inside that exact
predecessor record**. An inline supplied predecessor Claim is never trusted.

Describe cannot change either native source selection, source side,
granularity, inherited rights, competing references or mapping. It must change
a descriptive qualification; a capture timestamp/event alone is not a delta.
Remap also preserves the two exact source scopes. A source-scope change is not
an ordinary descriptive revision: select a distinct source-bound proposal and
obtain the appropriate owner judgment rather than silently repointing history.

Competing links are one-way exact source refs in the newly authored record;
the old immutable alternatives retain their bytes. A reverse edge can be
derived from explicitly supplied records. Global reverse indexing has its own
projection route. Competition and supersession record relationships among
versions; preference requires source-visible judgment, and multiple
alternatives may remain unresolved.

The existing bounded source identity walk checks both selected public and
private metadata homes. A new version may reuse only IDs in its exact
predecessor chain; an already present sibling/later version prevents stale
forking. There is no separate identity catalog or mutable alignment-head file.
Verified historical replay can read its already-owned version after a later
version exists; it cannot use that exemption for a fresh write.

## Source and proposal boundary

Each side is an ordered selection of existing native TextUnits from one exact
packet and segmentation. Every binding retains packet ID/version/raw digest,
segmentation ID/version, unit ID/version, ordered anchors, exact TextLayer
record ID/version/raw digest, and Work/Expression/Edition/Item owner refs.
The original File, UTF-8 representation, all selectors and anchor digests are
resolved through those existing native contracts. Source and target are
distinct Expressions, not merely two layer versions of one Expression.

The owner explicitly declares granularity and whether each selected frozen
segmentation also supplies tokenization. Phrase/token/morpheme/mixed
granularity requires both tokenization bindings. Alignment granularity,
accepted tokenization and linguistic quality retain separate declarations or
assessments. Unit selection supplies bounded context for one mapping;
omission/addition records identify its unaligned members.

Both side metadata and current recorded local-derivation rights are checked
before either representation is read. Current independent read/proposal grants
are checked again around each resolver read. UTF-8 bytes retain their exact
newlines and Unicode form. No tokenizer, aligner, model, network, OCR engine or
translation generator is called. Both source closures are retained as exact
input bindings, not reconstructed from labels or strings.

Mapping shape/cardinality, omissions/additions, reference and evidence closure,
technique combinations and rights use the complete existing v1 schema and
mechanical validator through a temporary, non-published validation view. V1's
ID-only lineage and in-packet reciprocal competition are not fabricated in
that view: the native wrapper checks exact immutable history separately.
Monotonic mappings preserve each native side's anchor order. `reordered`
retains the explicit proposed ordering; it is not an independently assessed
cross-record translation-order verdict. Rights are the strictest side or local
packet posture; all outputs remain private and publication unauthorized.

Supplied techniques, uncertainty, rationale and maker attribution retain
proposal status. The wrapper is `proposed` and
`unassessed_translation_proposal`. Provenance uses an annotation/capture event
and records supplied mapping alongside verified source bytes. Upstream aligner
execution requires its own evidence. An authorized competent reviewer assesses
translation through bilingual source-visible comparison, the required
independent baseline and a scoped assessment route. Quality, semantic use,
publication and canon each require the corresponding owner decision.

## Independent configuration and requests

The common source front door selects
`tos_local_native_alignment_owner_v1` through
`source_alignment_commands.py`; discovery advertises
`owner-local-native-translation-alignment` without reading any grant or source.
The protected mode-0600 configuration contains exactly:

- `schema_version`, current local `uid`, `principal_id`, `authority_ref`,
  `expires_at`, the independently selected absolute `source_context_ref`, and
  private logical `source_path` ending in a **new** package directory followed
  by `native-translation-alignment.v1.json`;
- `allowed_operations: ["alignment.create"]` for `initial`/`competing`, or
  `["alignment.revise"]` for `describe`/`remap`; the request cannot choose a
  different change kind;
- separate `source_access: {read_scope: "exact_owner_local", access_allowed:
  true, authority_ref, expires_at}` and `alignment_access:
  {derivation_allowed: true, authority_ref, expires_at}`;
- opaque `record_id`, `alignment_id`, `claim_id`, a fresh `provenance_event_id`,
  `change_kind`, exact `predecessor` or null, and `competing_records`;
- `native_bindings: {source: [...], target: [...]}`, `granularity`, and explicit
  `tokenization: {source: boolean, target: boolean}`;
- v1 `maker`, with `maker_kind: "imported_source"`, principal-bound
  `agent_ref`, supplied `made_at`/method, the delegated event ref and
  `method_output_posture: "proposal_not_truth"`. This attributes supplied
  material without impersonating its alleged upstream producer.

The existing mode-0700 private parent and source context are prerequisites.
The request uses the existing `tos_local_source_command_v1` envelope:

```python
prepared = run_local_command(owner_config, {
    "schema_version": "tos_local_source_command_v1",
    "operation": "prepare-create",  # prepare-revise for an exact successor
    "mapping": mapping,
    "qualifications": qualifications,
})
result = run_local_command(owner_config, {
    "schema_version": "tos_local_source_command_v1",
    "operation": "alignment.create",  # alignment.revise for a successor
    "mapping": mapping,
    "qualifications": qualifications,
    "command_id": command_id,
    "expected_configuration": prepared["owner_configuration"],
    "expected_dependencies": prepared["expected_dependencies"],
    "expected_source": None,
    "expected_revision": None,
})
```

The absent source/revision precondition describes the **new immutable version
package**, not the absence of a predecessor. That predecessor is separately
fixed by the protected configuration and the output's exact history.
`mapping` contains the five existing direction/shape/order/source-anchor/target-
anchor fields. `qualifications` contains existing `translation_techniques`,
`epistemic_status`, `certainty`, `status_reason` and `evidence`; request fields
cannot supply a maker, rights, source selection, identity, executable or review.

## Inspection, replay and recovery

`describe` reads contracts and protected configuration. `inspect` checks the
exact delegated target, retained receipt and current metadata/source closure.
`inspect-version` takes `source` in the exact native record-ref shape and
accepts only a version in that selected history. Both return metadata-only,
redacted verification/version summaries; source strings, locators, selectors,
private IDs and short-span hashes remain withheld. Source-visible comparison
uses the separate assessment route.

An identical apply request replays the exact immutable package. Request,
configuration, source inputs, implementation/runtime pins, source receipt and
stored bytes remain checked. Changed source bytes, scope, current rights or
implementation cannot be accepted merely because an old receipt exists.

The existing shared/public-then-private writer lock order and retained native
construction primitive publish a flat mode-0700 package with mode-0600 files.
Exclusive writes and atomic no-replace directory publication preserve prior
versions. A durable exact plan precedes output writes. `inspect-recovery` with
the original `command_id` checks the retained plan; retrying the exact original
apply request fills only absent staged files. Torn files, a missing/torn plan,
foreign residue or changed authority fail closed and remain for owner review;
this route does not delete or overwrite recovery evidence.

Bounds include 256 units per side, 32 explicit competing records, at most 64
logical history steps, the shared native resolver's 128-input/byte budgets and
a 1 MiB canonical record ceiling and 60-second cooperative command deadline. The file/dependency budget may reject
a history before the logical depth bound. These are fail-closed limits, not a
complete-corpus mode or a preemptive filesystem/parser timeout.

The focused synthetic tests belong to `mechanics/growth-cycle/tests/` and the
existing `mechanics_local` lane. No real corpus bytes, bilingual assessment,
external acquisition, KAG projection, release or runtime installation is
established by their success.
