# Native translation-alignment source review

Review date: 2026-09-09. Implementation starts from the shared prerequisite
`b445d445207c6eba83eb02ee60c09321ee92822e`; the exact resulting source commit and session belong to the
ordinary checkpoint review, not to a fabricated self-referential hash here.

## Changed owner surfaces

The additive native record belongs to the existing translation-alignment
owner. Its schema references legacy v1 mapping/side/qualification/rights
definitions; the legacy packet, human-review format and public semantics are
unchanged. The source command front door gains one native owner handler.
It reuses the native source resolver, bounded identity walk, source receipt,
lock order and retained no-replace construction primitive. No registry,
language-specific builder, corpus Item, TextLayer, TextUnit, semantic entity,
translation review, accepted use or public graph is introduced by this change.

The contract and source review found:

- Yes: an Alignment subject is independent of its descriptive record and
  versioned Claim. Describe preserves identity, both native scopes and mapping;
  remap introduces a new Claim, and competition introduces a distinct
  Alignment with an exact alternative ref. The old alternative is not rewritten.
- Yes: predecessor record identity/version/raw bytes are checked, and Claim
  predecessor hashes are computed from the Claim inside those exact bytes.
  A stale sibling/future record blocks a fresh version fork. Historical exact
  replay is not mistaken for a fresh identity reservation.
- Yes: native packet, segmentation, unit and TextLayer versions, original File,
  anchors, code-point selectors, source-return locators and bytes remain
  distinct and source-returnable. Neither labels nor matching strings choose
  identity. Explicit granularity retains the legacy fine-granularity requirement
  for two frozen tokenization inputs.
- Yes: full legacy schema and mapping/evidence/rights checks remain in force.
  1:N, N:1, N:M, omission, addition, monotonic and explicitly reordered
  proposals use the same grammar. A proposed ordering is not a bilingual
  assessment or a preferred reading; no universal word/sentence aligner is
  inferred from the contract.
- Yes: both current rights gates and exact immutable history/scope are checked
  before content. Current grant/expiry checks surround resolver reads and a
  shared cooperative deadline bounds the command. Inspect is metadata-only,
  checks exact delegated identity/version/scope and withholds private source
  values. Preparation also enforces the common bounded Record ceiling.
- Yes: supplied mapping and attribution remain unassessed proposals. Retained
  provenance describes annotation/capture and exact source verification, not
  aligner, model, OCR or translation execution. Source-visible bilingual
  comparison, competent assessment, independent baseline, scoped admission,
  rights, publication and canon remain with their actual owner routes.
- Yes: source packages are private, flat and immutable. A retained exact plan
  permits retry after complete staged files; torn/foreign stage evidence is
  neither silently overwritten nor discarded. No original source is edited.

During source review, metadata-only predecessor inspection was tightened to
check the supplied record ID/version and native scope as well as its raw hash.
History validation was also moved entirely before representation reads: a
wrong predecessor Claim or a changed predecessor scope must not open source
bytes before a later failure. The tests now protect that no-content-read
boundary. Capture time/event alone is not treated as a descriptive change.

Counterpart, lived-witness, canon mirror, public release and durable-decision
transitions are not applicable. This review is of source contracts and code,
not of any historical text or translation.

## Validation and limits

The shared four-module regression passed 79 tests and 135 subtests in
259.37 seconds: native alignment, TextUnit construction, TextLayer construction
and source-command discovery. The exact transient unit peaked at 107.8 MiB
memory and 74.4 MiB swap. Script/test topology passed 16 tests and 741 subtests
(136.5 MiB memory, no swap), and both legacy v1 alignment tests passed.
The initial nine-test run had four wrong test constant references and one
invalid synthetic rights enum; those fixture errors were corrected without
weakening a production error or rights boundary.

The final alignment-only rerun after the source-review corrections passed all
15 tests and 22 subtests in 123.24 seconds. Its transient unit peaked at
97.9 MiB memory with no swap. This final-code result includes exact predecessor
inspection and refusal of a wrong predecessor Claim before content reads.

Source-home, validation-lane manifest, mechanics topology and nested AGENTS
checks passed. The AGENTS-route and documentation-family companions were
regenerated from their owners; the agent-surface carrier itself was unchanged.
Local documentation guards (without re-running neighboring owner gates) passed.

The full cross-corpus gate was **not green**: its initial AGENTS currentness
finding was repaired by the standard builder, but the KAG candidate budget
seal/file count/source epoch and inherited index-family content hash require
post-union regeneration. The observed inherited mismatch included
`access/web/tsconfig.json`, which this change does not edit. No KAG gate was
weakened, receipt admitted or index regenerated in this bounded branch.
The integration owner owns final union, KAG/index-budget/currentness rebuild,
full release/CI validation and landing. No CI, merge, deployment, installation,
real-corpus write, bilingual assessment or Foundation-wide completion is
claimed by this source review.

## Integration review correction, 2026-09-10 UTC

The parent source reviewer (`agent:codex-tos-foundation`, GPT-6 Astra/high,
session `01a06cc7-0452-77f2-b89a-fb77fb86c3bf`) inspected the complete native
handler and its contract at `54e390de55ac5fd81ec76717f37f55c84c1386a9`.
It found and reproduced an exact-reference cache defect: two competing refs
with the same path and digest but different asserted record ID or version
could reuse one cached validation. Both negative controls failed before the
fix. Byte identity did not validate the second ref's claimed record identity.

The cache now keys the whole immutable reference, including record ID and
version. Both forged refs are rejected before any representation content
read. The existing competition test retains its positive old-source
preservation check and adds these two negative controls. The full 15-test
native alignment module passed in 119.374 seconds after the fix; managed unit
`abyss-machine-generic-medium-1a088dcf789-357967` reported 31.1 MiB peak memory
and 7.5 MiB swap. This corrects a reusable reference invariant, not a
historical or translation judgment. Real source growth and bilingual
assessment remain separate outstanding work.
