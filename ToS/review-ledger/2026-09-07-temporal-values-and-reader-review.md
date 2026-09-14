# Temporal values through the shared source constructor

Date: 2026-09-07 UTC. Baseline: `9ceae4d0772a0ab5e85155d403aa606f43945681`.
Owners: ToS historical value/profile contracts; Growth source commands;
deterministic catalog/graph; portable read-only access.

## Changed boundary

`historical-temporal-v1` executes the existing `historicalDate` grammar in
declared `source-claims.jsonl`. Date, interval, relative order and explicit
unknown remain Claim-scoped values, not independent historical identities.
The source reader enforces a historical-situation domain and a temporal-value
range separately, even if an invalid registry repeats one type on both sides.
A permissive extension schema cannot weaken the shared value grammar.

Create/revise owner configurations v2 explicitly allowlist exact values;
relative anchors also need independent identity scope. The v1 grants do not
gain object-value creation or correction. Correction cannot retarget an
identity edge or turn a value into an identity; it retains original attribution,
complete predecessor bytes, current form rebindings and shared history.
Source bindings distinguish identities from exact value/type/digest tuples.
The assessment adapter requires selected historical anchor sources rather than
crawling the corpus or accepting inline identity shadows.

The graph uses the declared temporal reader for relative-anchor edges and
access classification. A new synthetic predicate passes all four value kinds
through the same source/assessment/graph/access code with registry data only.
`historical_work` now has its existing concrete domain/range bound to the
shared identity-relation reader; it remains a topical association, not a
generic assertion of production, publication or influence. Legacy historical
Claim files keep their separate schema and reader unchanged.

## Actual source use

The [source reading](2026-09-07-jgb-production-dating-source-reading.md) supports
one new provisional HistoricalProcess for Sommer's reported JGB production
activity. `source.create` retained its record, three source-copy forms,
serialization inputs and receipt. A mixed `claims.create` v2 transaction added
the month-level historical interval and association with the existing Work.
Both Claims received explicit Russian source-copy statements through the
separate form operation. The initial commissioning episode and date remain
unchanged; no false historical disagreement or fabricated correction was
introduced merely to exercise the command.

An initial helper used `form.create` at the outer command boundary instead of
the documented `apply` envelope and was rejected before form writes. The helper
was corrected to consume `prepared_change`; both earlier creation operations
replayed their exact retained requests and the forms then completed. This is
a caller correction, not a hidden source-file repair or a weakened API.

Fresh `ToSAccessCore.discover(root)` inspection preserved all source bytes as
record/Claim payloads, both subject carriers, exact Russian forms, full Claim
context and null admission. The date value retains null calendar/numbering,
the declared month interval and explicit `calendar-not-comparable` and
`year-numbering-not-comparable` issues; neither sort bound is invented.
Process → Work and Work → process both return through ordinary two-hop focus.
One process measured cold graph 21.330 s; process focus 7 nodes/7 relations
0.277 s; Work focus 127/190, 0.502 s; peak RSS 1,265,160 KiB. Python 3.14.7,
Linux 7.1.13-200.fc44.x86_64, while other checks were active. These are observed
single-process timings, not p95 budgets or an incremental-writer result.

## Verification and limits

Reproducible repository checks:

```bash
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_claim_commands.py
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_commands.py
python -m unittest discover -s mechanics/growth-cycle/tests -p test_knowledge_assessment.py
PYTHONPATH=access/src python -m unittest discover -s access/tests -p test_knowledge_contract.py
PYTHONPATH=access/src python -m unittest discover -s tests -p test_source_witness_bibliographic_graph.py
python scripts/build_source_witness_catalog.py --check
python scripts/validate_source_witness_foundation.py
python scripts/build_source_witness_bibliographic_graph.py --check
python scripts/validate_tos_corpus_index.py
```

The new command controls cover v1/v2 scope separation, revoked values/anchors
even on replay, exact source-contract drift, all four value kinds, unknown
extensions, invalid/missing anchor families, source-selected assessment,
identity-retarget refusal, exact archived correction and original-create
replay. Existing interruption/concurrency/receipt-corruption controls continue
to exercise the shared writer. Final command suites passed 23 Claim tests in
130.060 s and 37 source-command tests in 138.986 s. Graph tests passed 68 cases
in 173.900 s; access 58 cases in 27.836 s; assessment 49 cases in 6.733 s.
An initial graph run before rebuilding its changed
source projection correctly failed parity; the rebuilt quiescent run passed.

Review checklist: source traceability, layer separation, retained identity,
unknown calendar posture, explicit writer authority and read-only access are
preserved. No rights, publication, canon or research admission is inferred.
This was source-visible review of the bounded scholarly description, not a
calibrated assessment or independent primary-production-document inspection.
No model runtime, host service, UI composition or external deployment changed.
Wider biographical/environment profiles, numerical temporal algebra, as-of
reconstruction, addressable writer scaling, D1/UI acceptance and full Foundation
completion remain separate required work.
