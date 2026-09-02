# Reviewed Open-Work Candidate Loop

## Index Metadata

- Decision ID: TOS-D-0039
- Original date: 2026-08-29
- Surface classes: source-witness, contracts, scripts/validation, docs/route-law
- ToS layers: source-witnesses, contracts, philosophy, docs
- Tree classes: work, corpus, source
- Guard families: source-first authority, chronology boundary, rights boundary, generated parity, terminal receipt, per-channel timing, artifact representation, fixity
- Posture: accepted

## Context

The philosophy master tables, admitted dossier index, source-anchor backlogs,
source-witness Work catalog, and protocol-native discovery runs contain enough
material to search for openly usable works. They do not themselves answer one
workflow question: which exact Work-like target is next, why it precedes its
neighbors, and whether a previous loop iteration reached a terminal outcome.

Deriving that answer only inside an agent prompt would make ordering and
completion non-reproducible. Putting it into a generated catalog or graph would
let a weaker read model become the hidden author of candidate eligibility.

The first Ptahhotep pass also exposed two durable pressures. Its channel times
were explicit zero/unknown sentinels, so terminal completion could hide an
uninstrumented run. It also found an exact CC0 photograph of a physical papyrus
artifact, while the existing artifact metadata contract had no File-backed
visual representation surface.

Selected decision lenses: owner/source-of-truth, evidence state, risk and
approval, and handoff/scale. These lenses matter because the loop crosses
authored queue state, generated navigation, live transport measurements,
project-owned acquisition, immutable bytes, rights evidence, and stronger
source-witness authority without allowing one layer to silently replace the
others.

## Decision

Keep reviewed queue eligibility, chronology ordering, source selectors, and
the frozen pre-run target in the authored
`ToS/source-witnesses/discovery/candidates/reviewed-candidates.jsonl` ledger.
Record each completed iteration as an immutable terminal receipt under
`candidates/receipts/`. Build `candidates/queue.current.json`
deterministically from those records plus the current master-table, dossier,
backlog, Work-catalog, and discovery-run snapshot.

Select only reviewed `work` or `work-like-corpus` candidates. Preserve an
earlier non-Work scene as an explicit exclusion rather than silently skipping
it. A terminal receipt binds the exact candidate digest, the queue snapshot,
and one material-discovery run; rebuilding then advances to the next eligible
candidate without rewriting the completed candidate. The queue validator
replays receipt history, reconstructs the queue immediately before each
iteration when the current tree permits it, and otherwise requires an
independent frozen-snapshot witness. A superseding receipt retains the same
snapshot and selected target binding.

Require the latest terminal receipt for each candidate to bind one external
per-channel timing receipt under `discovery/timings/`. Every active channel
must have a positive monotonic `python.time.perf_counter_ns` measurement and
the discovery's `elapsed_seconds` and comparison `machine_seconds` must match
it. The measurement owns HTTP transport through the first 16 KiB only; it does
not claim research, interpretation, rights-review, or human elapsed time.
Historical runs and receipts remain immutable and may retain explicit unknown
sentinels when a measured superseding run becomes active.

An acquired terminal outcome is closed only when its representation, File,
artifact/composite/Item identity, acquisition provenance event, and planting
records resolve and agree. The validator checks this closure on every receipt;
the active timing check remains scoped to the latest receipt for each
candidate. These are mechanical record checks, not identity, rights, textual,
semantic, or canon acceptance.

Planting closure additionally binds the candidate atlas row, exact source-witness
record, receipt-visible discovery target, and provenance event input/output. A
pre-existing planting is admissible only when its identity and route are explicit
in the receipt target or operational acquisition relations; a planting ID alone
cannot smuggle an unrelated source into a terminal outcome.

Represent an acquired physical-artifact image as a separate content-addressed
File under `artifacts/.../representations/.../payload/`, governed by
`artifact-visual-representation.schema.json`. The visual record binds the
physical artifact, originating and aggregator records, exact acquisition,
fixity, rights, discovery, and provenance while keeping embedded text and all
semantic/publication effects false. Artifact witness v2 permits an exact
source artifact with no honest philosophy planting; queue/discovery/File/event
relations must not be replaced by a fabricated backlog anchor.

## Options Considered

- Put the queue entirely in the loop-goal prompt. Rejected because source
  coverage, tie-breaking, and completed states would not be reviewable or
  rebuildable.
- Generate candidates directly from dossier strings and treat the output as
  authoritative. Rejected because mechanical extraction cannot decide Work
  boundaries, chronology, identity, or queue eligibility.
- Add scheduling state to each discovery run. Rejected because a run owns one
  frozen search episode, not the population-level choice that precedes it.
- Store timing instrumentation inline in the shared historical material-
  discovery contract. Rejected because an additive loop metric would drift
  old provenance bindings and encourage rewriting historical runs.
- Treat a zero as a real measured duration. Rejected because it erases the
  distinction between unknown timing and an observed fast request.
- Plant CGT 54014 against the Prisse-specific backlog row. Rejected because a
  related Ptahhotep witness is not the Papyrus Prisse and the relation would be
  false.
- Route the CC0 JPEG into the host cache after the automation preflight deny.
  Rejected because ToS source evidence belongs to the project source tree.
- Use an authored candidate ledger, immutable receipts, and a generated queue
  read model, plus external timing receipts and a File-backed artifact visual
  contract. Accepted.

## Rationale

This split preserves the existing owner chain. Philosophy records provide
pre-canon pressure; candidate review makes only a bounded workflow choice;
the discovery run preserves ordered search evidence; exact source-witness
records own identity, fixity, provenance, and rights; human review owns textual
and semantic judgment. The generated queue can expose current work cheaply
without becoming a new source of philosophical or legal authority.

The storage preflight deny remains evidence about machine-owned host
automation on an `abyss_os_project` path. It is not an acquisition allow and
was not bypassed or rewritten. The operator's explicit instruction supplies
project-owner authorization for this bounded repository source write; the
representation record preserves both scopes so future automation cannot infer
general write authority from this episode.

## Consequences

Queue selection is now deterministic, source-returnable, and fail-closed on a
missing selector, stale generated file, changed candidate digest, or unknown
discovery receipt. A latest terminal receipt also fails closed on missing,
zero, mismatched, duplicate, or partial channel timing. Zero-result and
metadata-only outcomes can advance the loop without pretending that bytes or
rights were acquired.

The first active Ptahhotep route now supersedes its historical pass with 12
measured channels and one exact CC0 JPEG acquisition. The physical CGT 54014
artifact, photograph, File, rights record, and acquisition event remain
separate. The queue advances to Pyramid Texts without executing it.

The initial reviewed frontier is intentionally partial. It proves the earliest
current choice from A01, A04, and A05; later source pressure remains visible in
the generated source snapshot but is not called reviewed. Expanding the queue
requires new authored candidate review, not a looser parser.

This decision does not accept a Work identity, historical chronology,
translation, source text, rights conclusion, publication package, semantic
relation, canon node, deployment, or human judgment. A concrete acquisition
still requires exact owner authorization, storage-boundary review, and a
layer-specific rights route.

## Source Surfaces

- `ToS/source-witnesses/discovery/candidates/README.md`
- `ToS/source-witnesses/discovery/DISCOVERY_PROTOCOL.md`
- `ToS/source-witnesses/README.md`
- `ToS/doctrine/CORPUS_FOUNDATION.md`
- `ToS/philosophy/atlas/master-tables/`
- `ToS/philosophy/atlas/dossiers/index.jsonl`
- `ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl`
- `ToS/source-witnesses/catalog/works.jsonl`
- `ToS/source-witnesses/discovery/runs/`
- `ToS/source-witnesses/discovery/timings/`
- `ToS/source-witnesses/artifacts/`
- `ToS/contracts/open-work-candidate.schema.json`
- `ToS/contracts/open-work-candidate-receipt.schema.json`
- `ToS/contracts/open-work-candidate-queue.schema.json`
- `ToS/contracts/open-work-channel-timing-receipt.schema.json`
- `ToS/contracts/artifact-source-witness-v2.schema.json`
- `ToS/contracts/artifact-visual-representation.schema.json`

## Validation

Run `python scripts/build_open_work_candidate_queue.py --check`,
`python scripts/validate_open_work_candidate_queue.py`,
`python -m unittest tests.test_open_work_candidate_queue`,
`python scripts/validate_source_witness_foundation.py`, the focused artifact
visual-representation test in `tests.test_source_witness_foundation`,
`python scripts/generate_decision_indexes.py --check`, and
`python scripts/validate_decision_records.py`.

Also run the source-witness foundation lane and the broad repository gate when
the complete change is ready for review.
