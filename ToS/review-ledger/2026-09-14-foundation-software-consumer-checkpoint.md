# Foundation software consumer checkpoint · 2026-09-14

This is a bounded integration review, not Foundation v1 acceptance, corpus
admission, CI, merge, or deployment evidence.

Reviewed worktree: `codex/tos-foundation-final-20260914`, with first parent
`7f59dc9147690f767e332a41dcd1503c996bbed0` and incoming parent
`36de25a5018aa277f64cec547e6dd9941358e697`. The observations below were made
before the merge commit, against its edited software. Session:
`01a06cc7-0452-77f2-b89a-fb77fb86c3bf`. Exact commit checkpoint review follows
separately; neither parent alone contains the tested integration.

## Working capabilities and their limits

| Capability actually checked | Goal coverage | Remaining boundary |
| --- | --- | --- |
| On the real canonical departure event and Zarathustra support record, agent indexed search → human result selection → agent inspection preserves the selected identity and readable inspector. The same bounded dataset also passes HTTP/native MCP inspection, compact lens and continuation, with request-time full graph construction forbidden. | G5/G6/G7/G9: shared human/agent reading, stable identity, bounded query delivery. | Two real records and a real JGB date Claim are integration seams, not the complete two mandatory research routes or full-corpus UI acceptance. |
| The compiled query store directly serves indexed search with the same four-rank ordering and native records as the legacy route. Node and relation pages reach EOF without duplicates; malformed, expired, changed-query/filter/snapshot cursors fail closed. Scan-only stores and insufficient candidate/verification budgets do not silently rebuild an index or graph. | G6/G7: explicit query compilation, bounded work, snapshot-bound continuation. | The isolated installable candidate and CI remain separate checks. |
| Oversized native D1 rows retain complete source JSON and searchable content in versioned metadata chunks; digest verification and the ordinary read budget still apply. Full/delta/replay equivalence and missing/corrupt/over-budget refusal are checked. A failed large read does not block another node. | G7/G8: no silent field loss, atomic derived publication, bounded reads. | Producer retention up to 8 MiB does not grant delivery above the 1 MiB native read budget. The corrected complete D1 candidate has not yet been reimported and health-verified. |
| Optional knowledge-search pagination no longer prevents browser initialization when its control is absent. All four existing WebMCP browser scenarios pass after the fix. | G6/G9: selection, source-gap and agent browser operations remain connected. | This is not a redesign or a new global smoothness claim. |

The real-record browser run reported no page errors and zero full-graph calls.
It does not read private payloads or perform semantic admission. Public metadata
test snapshots retain their source paths and digests; they do not become new
canonical records or rights grants.

## Reproducible evidence

Durable task artifacts are identified by basename and SHA-256 below. They are
local observations, not publicly activated runtime receipts.

| Artifact | SHA-256 |
| --- | --- |
| `final-real-browser-cycle-r2.json` | `98637d648767135025660a488c8a5af459c117d738351de7006f6480ebc5d70a` |
| `indexed-query-store-focused-r1.log` | `03ec00144a15294c998a749f79fe2c978791b5f618cb6530c0db4363b4121b7d` |
| `final-native-overflow-seams-r2.log` | `699e7a749a2f111040b3d82d5a47136c60faf89790fb58f8122e93be8412e8f3` |
| `final-overflow-delta-r1.log` | `e9332dec30b0592eaa280d219e5480cd533c1af5c84c191b4c8f3d09f9b8a8ee` |

Durable regressions live in `access/tests/test_query_store.py`,
`test_published_read_model.py`, `test_incremental_runtime.py` and Worker
`test/knowledge.test.ts`. The browser scenarios are in `access/e2e/test_webmcp.py`.

## Source-first review and next owner

Source traceability, native-field retention, identifier stability, language
forms, uncertainty and authored/derived separation remain explicit: yes.
The changed readers do not accept Claims, manufacture human signatures, grant
private access, merge identities or publish canon: yes. No new interpretation
or assessment competence is claimed; those checklist items are not applicable
to this software checkpoint. Access remains read-only.

The master retains completion ownership. Remaining K1–K4 acceptance must use
their exact source/corpus and consumer evidence, not the number of green tests.
In particular, exact-text permission, complete corrected D1 health, the
installable software candidate, required CI and merge are not closed here.
The pre-existing complete D1 candidate with a header-count defect is retained
as diagnostic evidence, not presented as healthy. Its replacement is a
separate exact-target storage action, not implied cleanup permission.
