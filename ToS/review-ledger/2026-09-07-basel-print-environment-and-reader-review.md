# Basel print environment: implementation review

Reviewed on 2026-09-07 by `agent:codex-tos-foundation`, against the change
following `f5657ebcbeb5d965583e6e2992be84cf3754880c`.
This is source/reader integration evidence, not independent historical
assessment, admission, publication or Foundation v1 completion.

## Source and operation

One HistoricalEnvironment under
`ToS/source-witnesses/history/basel-print-research/` carries cultural,
religious and scientific-technological accounts. One native Agent under
`ToS/source-witnesses/agents/erasmus-of-rotterdam/` and two qualified Claims
under `ToS/source-witnesses/relations/basel-print-research/` connect the
environment to Erasmus and the existing Basel Place. No new type, predicate,
Python branch, source registry or UI screen was required.

The [frozen reading note](2026-09-07-basel-print-environment-source-reading.md)
records the inspected catalog sources and limits. The two historical
environments in Basel have distinct scope and identities. A shared location
does not establish contemporaneity. Context does not establish personal
religious adherence, use of technology, employment or causal influence.
This is not a transcription or an acquired edition; scholarly summaries
and names remain unverified.

The existing separately delegated `source.create`, `claims.create` and form
operations produced the packages. Every creation was prepared against the
current dependencies and immediately replayed exactly with the same receipt.
The observed combined prepare/create/retry times were 7.186 and 6.956 seconds
for the subjects and 5.971 seconds for the two-Claim batch. Seven subject
forms and two Russian Claim statements materialized as ready but unadmitted.
Ready source-copy materialization does not mean accepted wording.

## Real consumer observation

The ordinary `ToSAccessCore` preserved exact records in both source carriers,
Russian names and hover descriptions, English names, the German Erasmus name,
and complete source-bound Claim statements and qualification contexts.
All carriers retained derived authority; Claims stayed `unreviewed` and
forms had no admission. No time keys were inferred from descriptive years.

Depth-2 focus (150 nodes/300 relations maximum) verified Erasmus ↔ environment
↔ Basel and Basel ↔ nineteenth-century appointment environment. Results were
5 nodes/4 relations, 7/7, 12/13 and 9/10 respectively; the selected subject
had one scene vertex. Calls took 0.264, 0.243, 0.251 and 1.174 seconds.
The six discovered environment property IDs selected their actual records:
political/economic/educational in the appointment account and cultural/
religious/scientific-technological in the printing account. Missing fields
were not filled from the other environment.

One reproducible query, after using the source builders, is:

```python
from tos_access.core import ToSAccessCore
core = ToSAccessCore.discover('.')
result = core.compile_knowledge_lens({
    'schema_version': 'tos_lens_spec_v1', 'lens_id': 'religious-environments',
    'node_query': {'filters': [{
        'property_id': 'tos.property.historical-environment-religious-account',
        'op': 'exists', 'value': True}]},
    'relation_query': {'enabled': False}, 'detail': 'compact', 'explain': True})
```

Run with `access/src` on the Python path. Discover property definitions from
`core.knowledge_catalog()['semantic_registries']['properties']`; inspect
the result's source refs for the account's qualifications. `exists` tests
recorded coverage, not historical truth or the absence of religious life.

Cold construction was 22.406 seconds and peak RSS 1,276,084 KiB for source
snapshot `e474814ab42eec854b26aa1c017fea799bb1a0f14ac6a6df33ee58cbc9285c7c`.
These are single local observations, not p95 or accepted performance budgets.
Subsequent documentation rebuilding may change the snapshot.

## Verification and boundary review

Source-foundation and source-claim graph validators passed. The existing
historical-context positive/negative contract test passed in 0.442 seconds;
it already covers all six environment dimensions and rejects missing accounts,
language or scope. No permanent test for incidental historical counts was
added. The real probe is source-preservation and query evidence, not a semantic
quality score.

Yes: source return, identity/Claim/form separation, attributed language,
historical scope, unknown coverage, stable shared Place and no admission.
Not applicable: canon, counterpart, lived witness, calibration, rights
clearance, private payload, deployment and UI mutation. H05 remains partial:
all six domain accounts now have real connected source records, but
independent substantive assessment and broader research behavior remain open.

The next owner is ToS source review, with relevant source-language competence
and independently authorized assessment. No qualified reviewer was launched
in this change. Rollback of generated readers must leave the source packages,
forms and creation history intact. Corrected accounts use versioned owner
commands, not hand-edited prior receipts. Source scans, full graph construction,
Worker/D1, actual UI interaction, CI, merge and deployment are not resolved
by these two new subjects.
