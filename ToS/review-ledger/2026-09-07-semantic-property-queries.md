# Snapshot-bound semantic property queries, 2026-09-07

## Scope and manual review

Partial Foundation Q02 work based on `e06e5644a3c9a0b9e8d13eca4b9a0780a95f37ee`.
Current authority remains the semantic property registry and the access
LensSpec/knowledge contracts, not this review. No new source claim, assessment,
admission, actor authority, UI design or deployment is introduced.

- Yes: the discovered property ID, declared value type, allowed operators and
  applicable type hierarchy are executed, not merely displayed in a catalog.
  A graph carries compact bindings from its own registry snapshot; D1 reads
  these bindings under the existing before/after publication revision guard.
- Yes: callers choose exactly one technical field or semantic property ID.
  Unknown IDs, wrong value types, undeclared operators, malformed IDs and
  unsafe paths fail. A request cannot inject a mapping or internal binding.
- Yes: inapplicable types do not match even `exists: false`. Missing/null
  values do not establish inequality. Property strings use exact code points;
  no translation, case folding, Unicode normalization or unit/calendar
  conversion is inferred. Existing technical-field semantics remain intact.
- Yes: node selectors and node conditions at path steps use the same compiler.
  Public results retain the semantic request, not private compiled fields.
  Compact delivery filters the complete record before omitting its attributes.
- Yes: the execution fingerprint version advances to v6 in Python, Worker,
  API and discovery. D1 data revision now includes serving property bindings;
  an older binding-less snapshot rejects the new selector. New metadata is
  staged through the existing publication transaction, not a separate store.
- Not applicable: substantive historical assessment, rights clearance, canon,
  new corpus admission, model competence, live UI or production activation.

## Verification

The new test first rejected `property_id` as an unknown schema field. After
implementation, the complete access suite passed: 147 tests in 146.862 seconds.
The standalone source-profile validator also passed. Focused property tests
additionally check malformed CR/LF IDs, unsafe bindings and duplicate property
identity after the complete run.

```bash
python -m unittest discover -s access/tests
python access/packaging/validate_standalone.py
python -m unittest discover -s access/tests -p test_knowledge_contract.py -k property_ids
python -m unittest discover -s access/tests -p test_access_contract.py -k edge_data_revision
cd access/deploy/cloudflare-worker
npm run typecheck
npm test
node --experimental-strip-types --test test/knowledge.test.ts
```

All 29 Worker tests passed in 44.798 seconds before the final string-contract
and negative-ID additions. The affected ten-test knowledge suite was rerun for
those additions. Its parity cases compare full Python/TypeScript/D1 results,
including fingerprints and semantic selectors, direct/path filters, inherited
type scope, missing values, booleans, arrays and Russian/Greek/emoji strings.
These are synthetic execution cases, not historical evidence.

A read-only `ToSAccessCore.compile_knowledge_lens` request over the existing
39,771-node/59,205-relation corpus used `tos.property.time-role` with
`exists: true`. It selected the source-described historical dating of the
Nietzsche commission letter, retaining `03. 06.1886`, its calendar and exact
`ToS/source-witnesses/history/friedrich-nietzsche/jenseits-1886-commission/historical-claims.jsonl`
return path. An initial `historical_time` value matched nothing; the source
actually declares `historical-time`. No vocabulary correction or historical
inference was synthesized to hide that mismatch.

The sampled cold preparation took 21.178 seconds and the subsequent property
query 0.209 seconds in the same local Python process during concurrent checks.
These are observations, not p95, hosted latency, or a scaling proof. The
source date remains an existing unreviewed assertion, not accepted by querying.

## Limits and next owner work

The local lens engine still scans the selected graph, and cold preparation
remains costly. This change does not prove bounded indexed execution on a
growing corpus. Relation properties, semantic sorting/grouping, broader
comparison/neighborhood rules and actual UI consumption remain unfinished.
CI, merge, release, remote D1 publication and deployment were not performed.
Rollback keeps source records and can restore the old reader; v6 cursors and
property selectors must not be silently interpreted by an older binding-less
snapshot. The full Foundation goal remains open.
