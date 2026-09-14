# Thought-practice profiles and real JGB reader review, 2026-09-07

## Owner changes and boundaries

Entity registry v12 adds ThoughtMethod, ThoughtOperation, ThoughtMove,
ThoughtExperiment, ThoughtImage, RhetoricalFigure, Metaphor, Value, Ideal and
OntologicalCommitment as source-described semantic profiles. The new
`thought-practice-record` schema composes the existing metadata and semantic
scope contract. Shared content language/script fields were extracted into
`semantic-description-record` without changing the accepted shapes of existing
reasoning or inquiry records. Twenty-two new content properties execute by
semantic property ID, including array membership. Metaphor inherits the
RhetoricalFigure account property without a duplicate subtype property.

Relation registry v11 adds thirteen concrete, nontransitive reified predicates.
They distinguish conceptual method/operation use, hypothetical assumptions and
examined theses, scoped ontological commitments, expressive images/figures,
normative values and an image's possibly criticized conception. Both reading
directions and explicit statement/grounds remain mandatory. The existing
thought-expression and attribution relations include the ten new profiles;
Conception retains its own existing expression/attribution predicates.

No per-kind Python reader, writer or UI branch was added. Existing atlas
Method/Figure and canon Analogy/Principle identities were not retyped. The
source-described profile rationale remains
[TOS-D-0053](../../docs/decisions/TOS-D-0053-source-described-conceptions.md).
Source notes are research descriptions; their normative language cannot grant
tool, policy or admission authority. Schema checks do not assess philosophy.

## Real source-connected data

The separate immutable [source reading](2026-09-07-jgb-practice-source-reading.md)
records the complete German JGB 21, 36 and 211, exact retained representation,
method and interpretive limits. Shared source-owner operations created:

- fourteen provisional subjects under `ToS/source-witnesses/semantic-descriptions/`:
  the ten new profiles, a second Value, two Theses and one Conception;
- thirty-one uncertain, unreviewed Claims in
  `ToS/source-witnesses/relations/jgb-practice-research/source-claims.jsonl`;
- seventy-three source-bound forms: 42 subject names/notes and 31 Claim statements.

The §36 scenario preserves the given-to-us supposition, hypothetical
will-causality trial and the additional successful reductions required for its
final conditional. Partial assumption coverage is explicit. The §211 ideal is
not an actual person; its existence questions remain unresolved. The §21 image
connects back to the existing freedom conception and reconstructed argument,
without becoming a new proven premise. Attribution concerns the specified
passages, not a timeless complete doctrine or approval by the researcher.

Russian notes, Russian/English names and English semantic accounts retain
their actual wording languages. Complete semantic scope/content or complete
Claim context accompanies selected forms. Every creation result leaves
admission false; every inspected form has no admission. Prior source bytes,
rights, historical decisions and the reading note were not rewritten.

## Verification and observed correction

- Registry compatibility with the exact parent registries passed: no previous
  type, identity, schema route or relation meaning was repurposed.
- New practice positive/negative contract passed in 1.001 s: mandatory
  accounts, typed arrays, unknown fields, language/script, explicit modal
  postures, concrete endpoints, inherited Metaphor, inverse wording and basis.
  An atlas Method is refused as an experiment's inquiry method.
- The focused shared creation/correction test passed across all 24 semantic
  profiles in 100.501 s. It checks semantic-ID property queries, full source
  and unknown-field preservation, source-bound forms, unchanged scope/ID,
  exact previous bytes, prohibited changes and idempotent replay.
- Full source-command suite: 36 tests passed in 97.969 s.
- Access knowledge suite: 58 tests passed in 27.159 s.
- Source graph contract suite: 65 tests passed in 152.581 s, with generation
  quiescent throughout the run.
- Source-foundation validation, including present-byte fixity, passed;
  source graph generation, exact parity and graph validation passed.

Preparation rejected two initially misselected generic thought predicates
for the Conception. Read-only endpoint validation identified the exact two
Claims; they were changed to the existing `conception_expressed_in` and
`conception_attributed_to` before publication. No partial Claim package was
created by that refusal and no domain/range rule was weakened. Synthetic tests
are not historical evidence or a substantive quality measurement.

## Actual complete-reader probe

`ToSAccessCore.discover('.')` read the full local corpus at normalized snapshot
`a0c28b5b518bd3d7ea1a14b480c0c37e0045c2417c8ed717359b02459fc83449`
(40,276 carrier nodes / 59,987 relations). All fourteen subjects survived
exactly in both source-claims and source-navigation carriers, preserving one
`entity_id` per referent rather than creating language or carrier identities.
All 31 Claims survived exactly. Russian source name/hover/statement selection
retained complete context and no admission. All new content properties selected
their actual subjects; array membership and the inherited Metaphor property
worked. Compact results omitted full source attributes.

Depth-two focus, bounded to 100 nodes / 200 relations:

| Focus | Required reach checked | Nodes / relations | Local seconds |
| --- | --- | --- | --- |
| `tos.thought-experiment.conditional-will-causality-jgb36` | method, opening assumption, conditional consequence, commitment | 12 / 16 | 0.295 |
| `tos.ideal.value-creating-philosopher-jgb211` | creative valuation, metaphor, role enumeration | 12 / 16 | 0.301 |
| `tos.thought-image.self-hair-pull-jgb21` | existing anti-self-origination argument and criticized conception | 8 / 10 | 0.287 |

Python 3.14.7, Linux 7.1.13-200.fc44.x86_64, x86_64. First graph call in the
process: 20.697 s; complete probe wall 29.481 s, user CPU 28.706 s, system CPU
0.689 s, peak RSS 1,240,232 KiB. Other local test processes ran concurrently.
This is one process-cold local observation with existing OS caches, not an
isolated benchmark, p95, hosted result, scaling proof or comparison with earlier
timings. The cold full-graph cost is still substantial. Adding this review to
the repository index changes the snapshot; the measured identity names the
actual probe, not a claim of an immutable final repository snapshot.

Reproduce source-preserving focus without writes:

```bash
PYTHONPATH=access/src python - <<'PY'
from tos_access.core import ToSAccessCore
c = ToSAccessCore.discover('.')
for identity in ('tos.thought-experiment.conditional-will-causality-jgb36',
                 'tos.ideal.value-creating-philosopher-jgb211',
                 'tos.thought-image.self-hair-pull-jgb21'):
    result = c.knowledge_focus(identity, depth=2, node_limit=100, relation_limit=200)
    print(identity, [(n['entity_id'], n['type_id']) for n in result['nodes']])
PY
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_commands.py
python -m unittest discover -s tests -p test_source_witness_bibliographic_graph.py
python -m unittest discover -s access/tests -p test_knowledge_contract.py
```

## Limits and continuation

Competence-backed content assessment, scoped admission, exact lexical
occurrences, complete bilingual descriptions and actual UI interaction remain
unproved for the new records. No source copy or green check substitutes for
those tasks. No new mandatory per-record human queue is introduced. Existing
atlas/canon mappings stay unchanged; the profile extension is not an automatic
migration or admission of their contents.

Reader rollback preserves source records, creation receipts, forms and all
predecessors; it does not delete research. CI, merge, release, deployment and
complete Foundation v1 are not claimed. Source/assessment and access owners
retain the remaining work under the full Foundation coverage map.
