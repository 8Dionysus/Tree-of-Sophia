# Source-described concepts and conceptions, 2026-09-07

## Scope and manual review

Partial Foundation T01/R04/C01 work based on
`05f06bdf5dfcc87ef7dc8f18bc83e884978b4a1e`. Current meaning lives in the
semantic-interchange registries and README, `semantic-description-record` and
`semantic-relation-claim` schemas; rationale is TOS-D-0053. No real semantic
record or historical attribution is admitted by this change.

- Yes: existing Concept IDs, canon/philosophy mappings and scoped authored
  meanings remain unchanged. CrosscuttingConcept is a new specific subtype;
  Conception is a separate semantic subject, not a version of Concept.
- Yes: an explicit semantic reader mode reuses the source metadata pipeline
  without placing semantic subjects under bibliographic/historical Identity.
  The old modes retain their family guards. Unsupported modes, role/family
  substitutions, broad endpoint roots and silent reader reassignment fail.
- Yes: complete descriptions, language declarations and semantic scope are
  required for the two profiles. Scope and continuity criteria remain
  contestable research commitments, not universal definitions. A nonempty
  string check does not establish substantive adequacy.
- Yes: names and source notes carry exact scope and identity posture as
  mandatory human-form context. Unknown extension values survive unchanged.
  Correcting a description preserves ID and scope, advances record version,
  retains the prior source package and rebinds all current forms. The ordinary
  correction operation cannot change the referent criterion or admission.
- Yes: membership, attribution, expression and eight distinct transformations
  are reified Claims with exact source records, statement, relation basis,
  evidence, maker, uncertainty and separate assessment. Competing/negative
  Claims remain possible. No unconditional direct edge, transitive influence,
  automatic chronology, shared-definition claim or one-author ceiling is added.
- Yes: source.create, record.revise and claims.create use their existing
  independently scoped configurations, expected versions/dependencies,
  transaction and replay routes. A changed source scope invalidates a prepared
  Claim. Describing or validating a profile grants no new write authority.
- Not applicable: historical truth assessment, competence certification,
  canon, rights clearance, new agent authority, UI source, deployment or
  publication. These are not inferred from structural success.

## Verification

The first new contract test failed on absent CrosscuttingConcept. The next
boundary failure exposed the older graph trace schema's two-layer enum; its
declared trace now accepts the same eight assertion layers as source profiles,
while each profile still enforces its own narrower layer list. A full graph
run then rejected stale saved graph input digests. The normal source catalog
and graph builders refreshed the derived companion; the same complete suite
subsequently passed.

```bash
python -m unittest tests.test_source_witness_bibliographic_graph
PYTHONPATH=mechanics/growth-cycle/tests python -m unittest discover -s mechanics/growth-cycle/tests
python -m unittest discover -s access/tests -p test_knowledge_contract.py
```

- 61 graph tests passed in 60.945 seconds, including both source readers,
  ordinary knowledge composition, preserved complete source data, focus in
  both directions, mandatory scope context and all eight transformations.
- 143 growth tests passed in 79.628 seconds, including semantic subject
  creation/correction and semantic Claim creation through the shared writers.
- 57 knowledge-contract tests passed in 23.082 seconds.
- The source catalog/graph rebuild, graph parity and graph validator passed.

These are concurrent local samples, not hosted timings or p95. Synthetic
subjects and assertions remain tests, not historical evidence. This review
does not claim deployment, CI, UI interaction or Foundation completion.

## Limits and next source work

Real source-grounded concept/conception examples, precise occurrences,
arguments/objections, comparison operations and substantive assessment remain
to be carried through their source owners. Philosophical adequacy and a
same-referent prose correction require content assessment beyond these tests.
The broader Foundation map retains its remaining profile, scaling, migration,
assessment and UI requirements. Existing graph, query, transaction and payload
bounds remain; a new semantic profile does not remove them.

An old reader must reject new modes without destroying their source records.
Rollback of the reader or derived snapshot is not permission to delete source
history, and existing scoped Concept nodes need no migration for this additive
subtype. Publication and actual scoped admission keep their own owners.
