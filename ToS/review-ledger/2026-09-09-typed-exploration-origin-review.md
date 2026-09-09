# Exact node and relation exploration origins

Date: 2026-09-09 UTC. Scope: access changes after
`ffd919acf91af38fab4b4ef6e3ba7a7e6aa4f989`, not Foundation v1 acceptance.
The primary agent implemented the Python/contracts path and reviewed the
Worker counterpart. A separate helper reviewed the Python origin, index and
packaging boundaries; its own Worker implementation is not called an
independent review.

## Owner contract and boundary checklist

The [exploration contracts](../../access/contracts/README.md) add request/result
v2 to the existing read-only operation. V1 retains `focus_node_id`, alias
resolution and its result shape. V2 instead selects an exact normalized node
or relation ID with source and content revisions. Discovery returns both
contract pairs; HTTP, native MCP, standalone bundles and Worker static contract
exports share that grammar. No source command or new entity type is introduced.

- **Yes — identity and source return.** A relation origin is the selected
  relation, not a fabricated node or a rewritten Claim. Its exact endpoints
  are bound from the same snapshot and included in every page. Entity/native
  aliases are rejected by v2, and ambiguous exact IDs are not chosen silently.
- **Yes — scale and bounded progress.** Relation endpoints are two depth-zero
  roots, or one root for a self-loop. Direction and predicate filters govern
  subsequent traversal, not whether the explicitly selected relation exists.
  Depth zero returns only that closure. Mandatory roots and the seed relation
  are context; incremental node/relation budgets still permit new work on the
  first page. Context and newly emitted IDs are separately inspectable.
- **Yes — compact reading and plurality.** The selected relation remains
  visible even when it is normally a collapsed projection or Claim leg.
  An incident Claim is not folded away. Partial Claim context keeps its
  more specific incomplete-context reason; a complete selected Claim path
  uses `focus-relation`. No visual path becomes a new historical assertion.
- **Yes — exact continuation.** The resolved origin is in the immutable
  checkpoint state. Snapshot changes reject continuation; existing local
  process-lifetime expiry and D1 persistent replay semantics remain distinct.
  D1 retains its atomic epoch guard and bounded checkpoint store.
- **Yes — malformed data and authority.** Invalid requests use 400, unknown
  or ambiguous exact origins 404, revision conflict 409, expired/missing cursor
  410, and a structurally corrupt selected carrier or endpoint 503. A failed
  local index replacement does not publish a partial index. These guards do
  not replace full graph validation or judge unknown semantic contents.
- **Not applicable.** No source record, translation, admission, rights,
  consent, canon, deployment, counterpart or lived-witness transition occurs.

## Focused verification and findings

The Python tests compare multiroot traversal with an independent zero/one-cost
distance oracle across cycles, parallel edges, self-loops, direction, profile,
depth and small page sizes. They protect partition accounting, progress,
snapshot/revision changes, replay, expiry, corruption and legacy v1 behavior.
Fixture HTTP and native MCP tests also verify actual adapter responses and
the discoverable contract pairs. The final run passed 24 methods and 450
subtests in 5.10 seconds.

The Worker matrix compared complete Python/D1 pages and the independent oracle
across 24 relation-origin combinations and four node-origin controls. Five
new-origin tests passed in 135.08 seconds, peak 634.5 MiB, zero swap. The actual
Worker HTTP test includes isolate restart, eight concurrent exact retries,
source-revision ABA, expiry and version mismatch. A later focused HTTP/error
repeat passed after the cursor trailing-newline guard. Existing v1 D1 tests
passed in the preceding combined run; this is local Miniflare evidence,
not remote D1, published runtime or UI acceptance.

Independent review found one real portability defect: ECMAScript and Python
disagree on the whitespace class used by `\S`. The v2 ID schema now enumerates
the intended edge-whitespace set explicitly. Both runtimes reject 58 prefixed
or suffixed whitespace cases and preserve seven supported controls, including
U+FEFF, zero-width space, internal whitespace and a supplementary-plane
character. The additional JavaScript test executes the actual schema pattern,
so Python's JSON Schema implementation cannot hide this difference. It passed
in 6.09 ms; the combined Python/JavaScript run took 7.416 seconds, peak 143 MiB,
zero swap. Runtime ID meaning and legacy v1 were not changed by this fix.
The independent reviewer reread the exact pattern and controls and closed the
finding. Worker typecheck then passed in 2.141 seconds, peak 152.9 MiB, zero swap.

The first production access-contract run found a stale legacy bibliographic
navigation descriptor after the registry change. Owner catalog/graph/index
regeneration corrected currentness; the post-regeneration 44-test access
contract run passed. A green retry does not retroactively make the earlier
combined run green.

Reproducible focused routes:

```sh
python -m pytest access/tests/test_exploration.py access/tests/test_exploration_origin.py
node --experimental-strip-types --test access/deploy/cloudflare-worker/test/exploration-origin.test.ts
python -m pytest access/tests/test_access_contract.py
```

## Real corpus and transport canary

A new process used normalized source revision
`52ec53b461b50445ffc0bac156cfb003cd5a082e8a3abd03a2db98d79d8870a5`.
It located the [new electronic-Edition Claim](2026-09-09-native-expression-edition-review.md)
by semantic identity and selected its exact normalized `has_object` relation
ID from the graph. With one incremental node and relation per page, core,
loopback HTTP and native MCP tools returned six schema-valid semantically
identical pages. Disposable cursor values were compared only within their own
service; replayed pages had identical JSON values within each transport. Every
page retained the selected relation, both endpoint roles and the Claim.
Depth zero retained only the two endpoints and relation despite a filter that
excluded the relation's predicate.

Separate real depth-zero node origins succeeded for a crosscutting Concept,
Agent, Work, historical Event, Place, Claim and Edition. No exact
`tos.entity.occurrence` node was present in this projection; this run does not
claim a real Occurrence canary. The normalized graph and tests support arbitrary
typed origins, but a missing production example is not synthetic evidence.

Cold graph preparation took 32.362 seconds. The canary body completed in
36.21 seconds; total process runtime was 41.266 seconds, peak 1.2 GiB, zero
swap. This is not a frame-performance or corpus-scaling benchmark. Earlier
harness attempts confused semantic and native IDs and required a complete-
path retention reason on an incomplete small page; those assertions were
corrected to the actual contract, without changing production behavior.

The temporary HTTP listener used an allocated loopback port and was closed by
its own process. Existing browser sessions and unrelated services were not
targets of this canary. Native MCP tool execution here is not an external
client handshake. UI construction of typed origins, temporal comparison,
other source growth, integration CI/merge and deployment remain separately
owned work; none is inferred from these local checks.
