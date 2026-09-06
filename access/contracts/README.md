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
Structured `semantics.time` comparisons require declared Gregorian or
proleptic-Gregorian calendar and astronomical year numbering. Missing or
unsupported context, approximate/uncertain dates, invalid parts, conflicting
nested context and incomplete/reversed intervals preserve `raw` and `issues`
without `sort_start/sort_end`; no calendar conversion or uncertainty expansion
is implicit. Bare legacy `YYYY[-MM[-DD]]` strings retain proleptic-Gregorian,
astronomical shorthand. Relative and unknown dating assertions remain
addressable without absolute order keys. Numeric filters select proposed
values, not accepted historical facts; assertion contexts still govern their
reading. These fields are materialized by the Python producer and transported
unchanged to D1; existing read snapshots require regeneration to gain this
correction. Processor dependency digests invalidate affected normalization
cache entries, not source history.
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

Localized text and registry labels retain extensible language/script keys
(for example `grc-Grek`, `zh-Hant`, `fr-CA`, or private-use `x-research`).
`default` and `original` remain compatibility roles, not language declarations.
The key pattern is a transport envelope, not an IANA registration check or a
judgment of translation quality. An absent translation stays null: source prose
in any supported key outranks a synthetic endpoint sentence. The deterministic
fallback order is `default`, `ru`, `en`, `original`, then remaining keys sorted;
this order does not assert that fallback text has the requested UI language.
Malformed/unknown source structures remain in the lossless source record, not
silently interpreted as translations. Per-form source, script, translation
assessment and qualifier-aware prose remain foundation work; this legacy map
alone is not the complete Forms contract.

Lens language preferences accept those keys. Safe display filters, sorts and
groups accept `display.<field>.<language>` for the declared node or relation
display fields. The catalog's `human_languages` section lists observed fields
and nonempty availability counts without claiming semantic readiness. Local
Python and the Worker/D1 reader share this grammar; changing preferences never
alters corpus identity or imports new knowledge.

Lens results additionally return `display_selection` per carrier. Selection
tries an exact case-insensitive language key, then progressively less-specific
tags, then the compatibility fallback order above. It never translates. The
packet names the requested language, selected source key, actual language when
known, fallback reason and available alternatives. Conflicting case aliases
produce `ambiguous-language-key` with no selected text. `original` gets its
language only from applicable source metadata, never from the UI preference;
the title's source language is not applied to the summary. An unspecified
`default` language remains unknown. Source-unavailable description placeholders
have `content_available: false` even when their compatibility text is nonempty.
The same applies to titles supplied only by an ID/path fallback; an actual
source name stays available without implying that its quality was assessed.
Selections bind the normalized `content_revision` and source form pointer;
they do not change that revision. `essential_context_pointers` identify the
assertion contexts which must accompany any reading of the selected wording.
Search, inspection and resumable exploration still return the form maps, not
language-selected packets. Consumer rendering/assessment remains a separate
integration requirement; selection itself does not certify safe abbreviation.

Where the source supplies explicit Forms, lens carriers also return
`human_form_selection` in full and compact results. It is separate from the
legacy display map above. The seven roles are name, caption, hover, statement,
grounds, history and technical. Each reports a selected exact form and intact
materialization, a missing/unavailable state, ambiguity, or an exact reference
requiring inspection. Mandatory context is part of the selected packet, never
removed to meet a scene character limit. Consumers must respect
`standalone_reading: false` even when a legacy display string is also present.

Selection uses exact case-insensitive language, then less-specific tags. A
unique remaining form can be an observable fallback; multiple remaining forms
are ambiguous, not a source-order vote. `auto` does not adjudicate alternatives.
Explicit `original` selects only an intact source-bound `language_context`
declaring that relation. Multiple originals remain ambiguous; missing metadata
reports `original-role-not-declared`. Unknown language, a form ID and source-copy
derivation alone do not establish the relation. Linguistic metadata and source
wording for translation/transliteration/adaptation must remain in the packet's
mandatory context. This is a source declaration, not a judgment of historical
priority. The current bibliographic adapter has not yet supplied this metadata.
Source-snapshot assessments are transported, not
reevaluated for current policy or runtime authorization. Missing, restricted,
stale and needs-assessment input states are not silently replaced by accepted wording.

The initial adapter binds the exact bibliographic record ID, version and
source digest. Other source owners still need their own binding adapter.
Candidates retain their exact form refs and pointers into full inspection;
compact attributes stay empty. The role packet itself retains context,
dependencies and provenance. Inputs are capped at 32 source materializations;
delivery has a 16 KiB conservative JSON-byte budget. Oversized forms return a
ref, not a substring; an oversized candidate inventory requires full
inspection. Language preferences are bounded at 128 characters independently
of whether the current query finds Forms. This is a request budget, not a
closed language vocabulary. Local Python and Worker/D1 run the same selection
contract; actual UI consumption remains a separate integration requirement.

`semantics.assertion_contexts` preserves source-declared claim fields in both
full and compact carriers. Each value names its source JSON Pointer and the
digest of the exact supplied public source record. False, null, empty and
absent remain distinct. An embedded owner claim takes precedence over carrier
conveniences; contradictory conveniences remain explicit in `conflicts`.
Qualifier objects, competing claims, counterevidence, confidence meaning,
maker and separate epistemic/review states survive compact delivery. Unknown
qualifier members are preserved without claimed interpretation. Bibliographic
edges bind their exact referenced claim context; a review or evidence node that
merely cites a claim cannot supply its governing assertion. Multiple source
records for the same claim keep all differing contexts, without automatic
adjudication or blocking unrelated records; malformed reference extensions
remain preserved but are not used as join keys. These contexts are
not assessments or admissions, and a historical review status is not current
permission to use the claim. Human rendering must use the context rather than
reading an endpoint sentence as unconditional truth.

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
