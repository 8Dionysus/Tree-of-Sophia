# Initial historical source creation — 2026-09-07

## Scope and source ownership

This is a local implementation review of `historical.create` in the existing
Growth Cycle source-command entrypoint. The baseline is
`fc089499e7035ade125c6c25b0566d285f1417a7`. The operator's Foundation v1 goal
requires executable creation of related source knowledge, not only form
editing. `CORPUS_FOUNDATION.md` owns identity and evidence law; the historical
record/claim schemas and semantic registries retain their meanings. The
command contract lives in
`mechanics/growth-cycle/parts/branch-growth-cycle/README.md`.

The implementation creates one new provisional historical event, process or
state with initial separately identified unreviewed claims, source-copy forms
and a source-write receipt. It does not implement atomic revision of an
existing subject, all-profile creation, a new database or a new source envelope.
No actual historical record, existing corpus source, generated catalog or graph
was written in this implementation pass. Synthetic creation fixtures refer to
unchanged copies of real bibliographic identities; the invented episodes and
associations are explicitly not historical evidence.

## Change and manual review

- Separate independently selected creation configuration: exact local account,
  principal and maker kind, source path, record/claim/form IDs, operations,
  authority reference and expiry. A form-only grant cannot create a subject.
  Source prose and request fields cannot choose these trusted inputs.
- `describe` and record `prepare` discover the scope, schemas and semantic
  metadata fields. `prepare-create` validates the complete packet and returns
  exact output byte specifications and a dependency digest without writing.
  `historical.create` checks the expected absent target, configuration and
  dependencies. It rechecks the dependencies and configuration before commit.
- The existing authored record/claim collectors check identity collisions;
  the existing historical graph contract checks schemas, registered endpoints
  and date anchors. Evidence, counterevidence, provenance and alternative-claim
  references must resolve. Form IDs cannot collide with existing adjacent
  current or retained forms. Existing source schemas are not broadened.
- The new record, historical JSONL claims, adjacent form set and creation
  receipt are staged outside `source-witnesses`. Fsync plus Linux no-replace
  directory rename publishes them together without replacing an existing
  directory, including an empty one. One global create lock coordinates these
  command writers; ordinary editors must remain quiescent.
- The receipt binds exact initial output bytes, request and dependencies, not
  historical truth or admission. Current operation, maker and identity
  revocations apply to replay. A retry does not overwrite subsequent source
  changes or turn its old receipt into a current assessment.
- Ordinary exceptions remove only that invocation's private unpublished
  staging. Abrupt process loss leaves invisible staging; retry does not delete
  it. No committed source or history is deleted. A derived-reader rollback
  cannot erase source creation. Existing-subject corrections need their own
  version-preserving operation, which remains unfinished.

The source/derived distinction, source traceability, distinct subject/claim/form
identities, explicit uncertainty and bounded owner authority were reviewed as
preserved. Unknown JSON extensions and claim qualifiers survive unchanged.
No human signature, agent competence, semantic acceptance, rights clearance,
canon or publication is fabricated by the command. Existing assessment and
read-only access boundaries are unchanged. There is no reusable independent
proof/memory/progression candidate from the synthetic run; the remaining
pressure stays with Growth and the existing Foundation coverage map.

## Verification

Commands run from the repository root:

```bash
python -m unittest discover -s mechanics/growth-cycle/tests -q
python -m unittest discover -s tests -p 'test_source_witness_bibliographic_graph.py' -v
python -m unittest discover -s tests -p 'test_*topology.py' -q
python scripts/validation_lanes.py --run source_home
python scripts/build_source_witness_catalog.py --check
git diff --check
```

The focused source-command suite contains 22 tests, including seven new
creation tests. Its full transaction reaches the existing catalog, bibliographic
graph, normalized access identity, hover-form selection and two-hop focus.
The new source body and all claims survive unchanged, including synthetic
negation, uncertainty and unknown fields. Output bytes agree with preview and
the retained creation receipt. A fresh CLI process replays without writes.

Negative controls cover malformed or out-of-scope subjects, identity/version
reuse, private visibility, impersonated maker/kind, invalid endpoint and
provenance/evidence refs, traversal, invented acceptance, duplicate claims,
unsupported/undelegated forms, absent name form, stale configuration/revision/
dependencies, and request-supplied authority. Additional controls cover record
and form identity collisions, symlinks, concurrent create callers, an empty
competing destination, dependency drift and revocation during staging, ordinary
pre-commit failure, real abrupt subprocess loss before commit, response loss
after commit, restart and current scope revocation on replay.

Local results: 87 Growth tests, 46 bibliographic graph tests and 26 topology
tests passed; source-home, current catalog parity and `git diff --check` passed.
The final focused rerun also includes the executing source/renderer/schema
identity in the preview's dependency digest. No checks ran against a deployed
consumer.

## Limits and next owner

Creation currently scans authored metadata for global collision and dependency
closure, including adjacent forms. No latency, growing-corpus throughput or
incremental-write budget is established by fixture timings. A graph/catalog
builder that traverses across concurrent creation can require a retry; this
operation does not make separate generated catalog files a coherent snapshot.
No new source-content storage route is assumed from an existing form-only
reservation. Actual corpus materialization must use its truthful owner route.

The real historical route, existing-subject atomic revisions, new evidence
intake, broader profile growth, competent real-agent assessment and production
consumer integration remain open. The legacy private full-corpus validation
blocker is not repaired or worked around here. Full release/CI, merge,
deployment, Worker/D1 and actual UI interaction are not claimed. Foundation v1
is not complete.

## Serialization provenance extension

Follow-up baseline: `d45a865a4739dc9a543091f7b4d260116b417a64`.
The v1 command's requirement for a pre-existing provenance event cannot
honestly record its own new materialization. A separately selected v2 owner
configuration therefore delegates one new event ID; it does not borrow an
unrelated earlier research event or alter legacy records.

The same atomic directory now includes the canonical request, path-free runtime
description and a v2 serialization event. The event describes completed buffer
serialization before staging/commit, not future publication. Buffer digests
are explicit, stored-byte fixity remains unattested, and the receipt externally
binds the exact event bytes. Its own software executor and script/runtime
digests do not impersonate the caller's research/model provenance. Actual
process argv is digest-only to avoid disclosing private configuration paths.
No model is called or substantive decision made. Review, rights and admission
boundaries remain unchanged.

The graph preserves the complete original v2 record and uses its activity
fields only as execution metadata. Existing v1 behavior remains unchanged.
Schema and existing provenance cross-field checks reject private visibility,
malformed events, inverted execution time and missing output derivations.
Derivation means technical serialization from the request, not historical
influence or proof of the supplied claims.

The synthetic v2 command test verifies failure-before-publication, successful
atomic creation, exact request/environment/input/output hashes, schema and
cross-field validity, original event delivery through the existing graph,
byte-identical replay and current event-ID revocation. No real historical
source was materialized in this extension. Upstream research capture, actual
historical-route completion, incremental indexing and production consumption
remain open with the same owners.

Local verification: 88 Growth tests passed, the 47-test bibliographic graph
suite passed, and the final focused v2 graph test passed after adding the
cross-field negatives. Source-home validation, current source catalog parity
and diff whitespace checks passed. The schema check is not a signed execution
attestation or substantive review. No CI/merge/deployment claim is made.

## First real source episode and consumer verification

The v2 command then created
`tos.historical-event.friedrich-nietzsche.jenseits-1886-commission` at
`ToS/source-witnesses/history/friedrich-nietzsche/jenseits-1886-commission/`.
The published evidence route and its limits are recorded in
`ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_HISTORICAL_EPISODE_RESEARCH.md`.
This is an attributed report from Sommer 2016, p. 6, not an inspected original
letter. Three claims separately bind the reported commissioner, Work and date;
the provisional subject and claims do not grant admission. No place or
Naumann person/organization equivalence was invented.

The actual seven-file package is 23,631 bytes. Its receipt binds six outputs;
all byte counts/hashes, source/claim schemas, provenance cross-fields and two
source-copy forms were checked against the published local files. A second
CLI process returned `replayed: true` and the original creation timestamp.
The human-form language fields remain null: the current legacy source record
does not supply an independently typed language field, and an ID suffix does
not establish language. This remains a Forms/profile gap, not claimed completion.

The existing catalog and bibliographic graph were rebuilt through their
builders. Exact-target storage reservations were acquired and released after
terminal writes through the installed generic capacity-only accounting route;
no new per-operation host policy entry, cleanup or deployment was needed.
The generated graph contains 172 identities and 196 reified claims, including
this new episode; these are coverage facts, not evidence of historical truth.

Real materialization exposed two obsolete final-validator assumptions:
bibliographic-only layers and v1-only method naming. Both now follow the
already-owned schema/reader contracts. The regression tests exercise the
actual final validator and reject an invented layer. Corpus-total assertions
now check exact source-catalog claim coverage and unique identities instead
of requiring test-code edits for every new source row.

The normal read-only CLI resolves the historical ID to one object with three
incident lines. A fresh `ToSAccessCore.discover` process returned the complete
source record and all three claims unchanged, selected the exact source-note
hover without admission, and reached the Work and Nietzsche through two-hop
focus (10 nodes, 15 relations). Its source revision was
`c34e0e076e25f796c6e222a9d8d27e5c445aa923576a30e1c670b750a9832fb0`.
One local run measured cold inspection 20.069 s, focus 0.198 s, warm inspection
0.000071 s, user/system CPU 19.556/0.688 s and maximum RSS 1,192,716 KiB.
These single-process observations are not p95, production or growth-budget
proof; cold cost remains a real scaling gap.

Current source catalog parity, final graph validation and source-home checks
passed. The final graph module passed all 48 tests in 52.978 seconds, including
v1/v2 provenance and historical-layer final-validator checks. The artifact-bundle lane reported its existing frozen external
admission, not a consumer trust verdict. The letter carrier, Naumann identity,
environment/reception route, substantive assessment, all-profile growth,
UI/Worker/D1 integration and full release/CI remain open.
