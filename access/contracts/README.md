# Access contracts

These files define the portable consumer seam. They do not redefine the
meaning of the ToS projections they carry.

`runtime-manifest.v1.json` names component ownership and profiles;
`runtime-data.v1.json` is the exact standalone data allowlist;
`query-operations.v1.json` owns transport-neutral read operations; and
`page-commands.v1.json` owns revisioned browser context plus shared human and
WebMCP actuation. `epistemic-packet.v1.schema.json` keeps the selected item's
posture separate from the surrounding graph field and makes partial coverage
and non-authority machine-readable. `web-actions.v1.json` remains only as a
migration marker for the former combined ABI.

`knowledge-api.v1.json` is the backend-defined construction seam shared by
HTTP, native MCP, and CLI. `knowledge-graph.v1.schema.json` normalizes every
public node and relation into stable identity, display, epistemic, provenance,
source-reference, and lossless attribute envelopes. `lens-spec.v1.schema.json`
defines the safe declarative composition grammar; `lens-result.v1.schema.json`
defines its endpoint-closed result. A LensSpec is a read query, not executable
code or a mutation. A synthesized description is explicitly marked and never
presented as authored ToS meaning. The live catalog also publishes observed
attribute fields and types, filter-value contracts, facets, bounds, and stored
lenses, so clients do not need to hard-code the current corpus vocabulary.
The ToS-owned entity and relation registries under
`ToS/doctrine/semantic-interchange/` add stable machine type IDs, hierarchy,
source crosswalks, localized definitions, relation domain/range, direction,
cardinality, and evidence/review posture. Raw source kinds and predicates stay
present beside those stable IDs; unknown vocabulary stays visibly unmapped.
Bibliographic claims remain reified, and exact declared references — never
name similarity — create cross-layer grounding routes.
`seed.focus_node_id` and `tos.knowledge.focus` make the selected center
machine-readable in both request and result. Resolution is exact normalized
ID first, then stable entity ID, then unique native ID; ambiguity is rejected. Catalog entity routes connect
common human terms such as concept, author, work, and word to currently
available source-derived kinds without manufacturing absent entities.
`availability` reports whether trustworthy node kinds exist, while
`role_readiness` distinguishes a relation-confirmed contextual role from a
kind-only candidate. Lens counts separately expose relation-query matches,
relations eligible around the selected nodes, returned relations, and actual
truncation; a focused neighborhood therefore does not report unrelated global
edges as omitted neighbors.
`tos.knowledge.contracts` exposes the operation map, executable JSON Schemas,
registry schemas, and registry data as one read-only packet over HTTP, MCP,
and CLI. Contract file
paths remain provenance; clients use the packet instead of assuming repository
filesystem access.

The `tos-lens-execution-v2` capability revision adds optional `path_query`,
`explain`, and `pagination` fields to the existing v1 request family. Existing
requests are accepted; consumers should discover the current schemas rather
than pinning old response-key sets. Normalized requests include their defaults.
Fingerprints include the execution version and exclude delivery pagination;
they must not be treated as permanent entity identity.

Path conditions join at node-selector roots and stay within `sources` at every
step. Inclusion records are execution witnesses, never philosophical proof.
Delivery cursors partition a bounded lens result with explicit repeated
endpoint/focus context, not an unlimited corpus walk. The public contract is
stateless for the listed lens operations: changing the query or underlying result invalidates the cursor;
expired history is not silently substituted. See the access README's
constructor boundaries for exact count scopes and outstanding scheduler work.

The optional **resumable exploration extension** has separate
`exploration-request.v1.schema.json` and `exploration-result.v1.schema.json`
contracts. It retains disposable execution checkpoints, not graph writes.
Discovery is `/api/knowledge/explore/capabilities`; continuation is cursor-only
at `/api/knowledge/explore`. It is not a LensSpec. Local HTTP/native MCP use
process memory; the Worker uses shared D1 checkpoints after migration and a
compatible read-model build. Discover availability on the actual target.
410 means lost/expired checkpoint; 409 means changed snapshot. The access README
defines replay, work budgets, count scopes and context-node upsert semantics.
D1 may pause earlier for its SQL-query budget, so page partitions and snapshot
digests need not match Python. Ordered discoveries and relation emission agree;
cursors are backend-specific. D1 413 requires narrowing the request, and 503
means the migration or compatible data metadata is missing.

`tos.zarathustra.word-analysis.prepare` is a local-full-Tree operation. It
returns a ToS-owned exact-source task when the provider is present and an
explicit unavailable envelope in the public standalone bundle. Access does
not generate, persist, review, or accept the agent's linguistic analysis.
