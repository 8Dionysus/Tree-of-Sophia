# JGB freedom research: source-to-reader review, 2026-09-07

## Changed owner surfaces

The source-owned descriptions under `ToS/source-witnesses/semantic-descriptions/`
add one crosscutting Freedom subject, three passage-bound conceptions, four
theses, one argument, two inference steps and one objection. The separate
`ToS/source-witnesses/relations/jgb-freedom-research/source-claims.jsonl` supplies
28 explicitly uncertain, unreviewed Claims. Creation used the shared owner
commands with exact source/configuration bindings, not test fixture writers.
The adjacent requests, environment locks, provenance and receipts preserve
that creation history. These receipts expressly do not grant admission.

The 36 subject forms (Russian and English names, Russian hover descriptions)
and 28 Russian Claim statements are source-bound copies. A Claim statement
retains the complete Claim as mandatory context. An object's name/hover
retains its identity posture and semantic scope; thought descriptions also
retain semantic content. No new type, reader branch or special-purpose screen
was needed for this real data addition.

The separate [source-reading note](2026-09-07-jgb-freedom-source-reading.md)
records the primary material, counter-reading and limits. It is an immutable
input of the creation events, not a place to append later test results.

## Boundary review

- **Yes:** objects, researcher Claims, author attribution, evidence and generated
  records stay distinct. Graph connections remain reified Claim paths.
- **Yes:** the metaphysical and mechanistic accounts are criticism targets,
  not beliefs silently assigned to Nietzsche. The questionable reversal step
  is explicitly hypothetical, not attributed to an invented historical reader.
- **Yes:** concept membership cites a continuity criterion and a separate
  basis. Shared spelling is not the test of identity. Different section numbers
  are not asserted to be historical stages of the author's changing belief.
- **Yes:** metadata and interpretations remain provisional. A source-copy form
  is mechanically ready but has no assessment/admission. Reader visibility is
  not scholarly endorsement, canon, rights or publication approval.
- **Yes:** original source bytes and existing rights restrictions are unchanged;
  only original analytical metadata is added to public source surfaces.
- **Not applicable:** new doctrine, identity merge/split, counterpart mapping,
  private lived testimony or stronger-owner runtime changes.

## Verification

At parent `257c16b283c6a4f56e0dd9abf89e86e7f09612d6` plus this source delta:

- Source-witness foundation validation passed, including present-byte fixity.
- Source catalog and bibliographic graph regenerated; graph `--check` and
  validator passed. Corpus index regenerated and validated.
- Existing bibliographic graph contract battery: 63 tests passed, 101.767 s.
  Existing access knowledge contract battery: 58 tests passed, 20.420 s.
  These retain synthetic negative controls; they do not establish the truth
  of the new philosophical reconstruction.
- A separate read-only probe used `ToSAccessCore.discover('.')` and the actual
  complete `knowledge_graph()`, not a synthetic graph. All 12 source records
  and 28 Claims were preserved exactly in the corresponding normalized nodes.
  Russian name/hover/statement selections returned source wording and required
  context; no selected packet carried admission.
- Actual depth-two focus from `tos.crosscutting-concept.freedom` reached all
  three conceptions (8 returned nodes / 10 relations). Focus from
  `tos.objection.against-reversal-jgb21` reached the challenged step and thesis
  (12 nodes / 16 relations). Limits were 100 nodes / 200 relations.

The local Python process measured 18.468 s for its first complete graph call;
the two focus calls were 0.238 s and 0.230 s. Peak process RSS was 1,215,348 KiB.
This is one process-cold run on the current local corpus, not a filesystem-cold
experiment, hosted result, scaling test or p95. It exposes remaining cold-path
cost rather than proving the Foundation latency budget.

Reproduce the bounded consumer check without any writes:

```bash
PYTHONPATH=access/src python - <<'PY'
from tos_access.core import ToSAccessCore
from tos_access.knowledge import select_human_forms
c = ToSAccessCore.discover('.')
g = c.knowledge_graph()
by_id = {n['entity_id']: n for n in g['nodes'] if n['source_graph'] != 'repository'}
for identity in ('tos.crosscutting-concept.freedom', 'tos.objection.against-reversal-jgb21'):
    result = c.knowledge_focus(identity, depth=2, node_limit=100, relation_limit=200)
    print(identity, [(n['entity_id'], n['type_id']) for n in result['nodes']])
    print(select_human_forms(by_id[identity], 'ru'))
PY
python -m unittest discover -s tests -p test_source_witness_bibliographic_graph.py
python -m unittest discover -s access/tests -p test_knowledge_contract.py
```

## Remaining owner work

Exact text-occurrence links, complete parallel-language descriptions,
competence-backed substantive assessment and scoped admission remain ToS
source/assessment work. Existing start-page anchors are not newly verified
occurrences. The legacy `display.title.ru` field is not populated by these
source forms; the typed human-form selector does return the Russian name.
Actual UI consumption remains a separate consumer integration check, not
proven by this Python probe. CI, push, merge, deployment and full Foundation v1
completion are not claimed by this review.
