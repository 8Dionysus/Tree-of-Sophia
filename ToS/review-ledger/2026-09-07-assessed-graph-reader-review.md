# Local assessed graph reader review

Date: 2026-09-07. Implementation/source-boundary reviewer:
`agent:codex-tos-foundation`. This is a local integration review, not independent
linguistic assessment, artifact admission, public release or Foundation v1
acceptance. It continues the [source command review](2026-09-07-assessed-form-command-review.md).

## Changed owner seam

The existing bibliographic and corpus-navigation builders now accept the same
explicit `AssessedFormSnapshot`. It resolves selected freeform IDs through the
protected source/journal command, verifies exact source/form refs and paths,
and checks observed configuration, source and committed journal drift before
return. No private configuration or journal path enters the result. The source
body, wording, mandatory qualifications and assessment observation stay bound.

Both existing carriers feed the unchanged common graph/focus API. Mixing
ordinary and assessed carriers, different source bodies, different selected
wording or different journal observations is refused. Python and Worker reject
malformed annotations and fake publication/runtime flags. Selection is read-only
and performs no model call or new admission. A transported observation is not
a fresh grant; double collection is not a cross-subject transaction or lease.

The additive optional annotation is documented in the source materialization
schema. Older closed-schema consumers must update, not strip it. Ordinary
metadata-only builders and source-parity readers retain their public route.
The [explicit local CLI](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#local-assessed-graph-builds)
creates only a separate new mode-0600 JSON candidate using exclusive staging,
fsync and an atomic no-replace link. A failed build never replaces the prior
reader or changes source/journal history. Separate CLI files are not an atomic
reader switch. Public-safety and artifact-consumer clearance remain required
before a candidate can be published or connected to runtime.

## Verification

```bash
PYTHONPATH=mechanics/growth-cycle/tests python -m unittest test_knowledge_assessment
PYTHONPATH=access/src python -m unittest discover -s access/tests -p test_knowledge_contract.py
PYTHONPATH=scripts:tests python -m unittest test_source_witness_bibliographic_graph test_tos_corpus_index
# From access/deploy/cloudflare-worker:
npm run typecheck
node --experimental-strip-types --test test/knowledge.test.ts
```

The final assessment suite passed 61 tests in 12.485 s; the common-reader suite
passed 61 in 35.772 s. Worker typecheck passed; all 12 tests passed in 25.711 s,
including Miniflare D1 agreement with the Worker and Python paths. Synthetic
checks cover pending -> admitted output, revoked grants/withdrawal, source and
owner drift, expiry, mismatched source/form paths, missing selections, output
limits, mixed carrier snapshots, malformed annotations, input nonmutation,
atomic no-replace output, injected link failure and changed access before write.
Schema checks validate ready and pending packets and reject publication flags.
These synthetic admissions do not prove model competence or actual review.

The complete bibliographic-graph and corpus-index suites passed together:
86 tests / 282.455 s. The ordinary bibliographic graph remained byte-current;
both projection validators passed. Corpus, agent-route and documentation-family
companions were regenerated from their owners. Documentation currentness and
mechanics topology passed after correcting the stale derived agent-route index.
The validation lane manifest passed. Full release/CI validation was not run for
this bounded checkpoint.

## Real source and unresolved work

An in-process probe builds both complete existing projections with the same
owner-selected snapshot for the two real RU/EN Hammurapi hover proposals, then
constructs the common graph and focuses that subject. Both carriers retain
`needs-assessment`, null wording, `unreviewed`, no journal head and zero batches.
The original source-copy hover remains usable. No historical source, standard
export or assessment journal is changed by that probe. It is deliberately not
an assessed artifact publication or a real model invocation.

The final probe observed claims construction 7.790 s, corpus construction
31.256 s, common normalization 30.185 s and focus 0.358 s; peak RSS was
1,249,696 KiB and maximum selected-form delivery 12,738 bytes. Claims snapshot:
`5268f8e3ae3ad1e5d3d8bac156621fcde1ca1bc37c8c582d3dac329a33e635b7`.
Common-reader snapshot before this note's final measurement update:
`3956cba5cadf74bcc7c1f6b80aec5ade2ef383a21b32b04c27e283d75083d794`.
These are single local observations, not global latency acceptance. The real
probe helper is task-local; the maintained suites above reproduce the guarded
interface on explicit fixtures without depending on private configuration.

The ToS assessment owner still needs qualified, source-visible linguistic
review and execution evidence. The existing proposals remain assigned to that
duty; this integration cannot self-issue competence or substitute fixture
reviews. Public candidate clearance, coherent runtime publication/switch and
actual UI acceptance remain separate work. No CI, merge or deployment is
claimed for this checkpoint.

## Boundary review and rollback

Yes: exact source return, whole-subject qualifications, stable identity,
source/form/assessment separation, retained previous versions, explicit
uncertainty, bounded delivery and read-only access. Yes: an unreviewed item
does not stop unrelated source copies. No rights, consent, canon, runtime or
publication authority is acquired. Counterpart, compost, lived-witness and
gold-promotion checklist items are not applicable to this transport change.

Rollback is to omit the optional assessed input and keep the ordinary reader;
neither proposals nor journal history is erased. The source owner controls
reassessment and the access/artifact owners control subsequent delivery. No new
decision record or parallel corpus registry was needed: the existing boundary
law is implemented, not replaced.
