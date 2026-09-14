# Scoped member structure: source growth and reader review

Date: 2026-09-10 UTC. Reviewer: `agent:codex-tos-foundation` (model agent,
GPT-6 Astra, high reasoning). Base: `ec3a7970970defdfd377646780f8f2197b79e1dc`.
Session: `01a06cc7-0452-77f2-b89a-fb77fb86c3bf`.

## Owner change and assessment boundary

The [shared contract](../contracts/scoped-member-structure.schema.json) gives
intellectual part composition and research-corpus membership separate concrete
predicates, domains and value types. A persistent research corpus is not a
publication Collection. One versioned Claim contains an attributed member set,
scope, coverage judgment, membership basis, order and limitations. Array order
has no semantic authority. The fixed adapter checks at most 128 members and
8128 edges; it rejects self membership, missing endpoints, cycles, unordered
precedence and an incomplete total order. Competing Claims remain separate.
Unknown extensions are retained without becoming executable ordering rules.

The existing public source create/revise operations, permissions, exact refs,
idempotency, forms, catalog and graph carry these profiles without a new writer.
Private Claim command bounds remain separate and are not widened by this work.
Physical parts and the order of publication Collections remain unimplemented
here; a corpus membership Claim must not be substituted for either meaning.

The [frozen selection note](2026-09-10-scoped-research-corpus-selection.md)
records the source-visible research judgment. Its five linked source
descriptions were read in full. This is a newly authored itinerary, not another
historical reading of Nietzsche, Parmenides or Burnet, nor an accepted
translation or proof of exhaustive membership. No historical source bytes,
rights, canon or semantic admission were changed. The operation receipts
describe software serialization; the upstream agent judgment and its limits
are recorded here and in that note, not misrepresented as deterministic output.

## Actual source operation

- Corpus: `tos.research-corpus.sid-8304245ab3a045c79eeefad18ea266ee`,
  [source](../source-witnesses/research-corpora/foundation-source-routes/research-corpus.json).
- Membership Claim: `tos.claim.sid-1d1ea31c7f4f442a807e36afedd6fa18`,
  [source and operation companions](../source-witnesses/relations/foundation-source-route-membership/source-claims.jsonl).
- Qualified statement form:
  `tos.form.sid-3a637a957a6e4d9fa8c84c0c77d33dfe.statement`.

Corpus create, Claim create and statement-form create each returned the same
receipt on exact replay. Corpus name/note and Claim statement are source-copy
forms. The statement retains the whole Claim as mandatory context,
`standalone_reading: false`, and no admission. The five prior source files and
frozen input note remained byte-identical across application and replay.
The Claim remains `unreviewed`, `public_metadata_only`, with no assessment refs.
That state is not a pending requirement for a human signature: construction
and any future semantic admission are distinct operations.

## Checks and actual reader

Focused public Claim create/revise/replay and member-focus test: 1 test passed
(25.659 s). Research-corpus creation/revision and the source bibliography module:
23 tests passed (10.417 s). Existing reference authorization, command, private
Claim and private record regressions: 57 tests passed (209.124 s). These tests
include synthetic invalid orders and exact-history round trips; they do not
establish historical truth. Registry transition against the exact base passed
with no violations (158 types, 186 relations, 234 entity mappings and 321
relation mappings). Source catalog, bibliography graph and corpus index were
rebuilt from authored inputs.

Source-witness foundation, source-home and documentation-family currentness
checks passed after regeneration. The cross-corpus documentation guard did
not pass in this source worktree: AGENTS-route and agent-surface currentness
were stale, the matching generated KAG budget receipt was absent, and its
pinned KAG checkout was unavailable here. These are an explicit combined
integration regeneration/check obligation, not a waived green gate.

The actual full union reader used source revision
`464d0b63fd7e8859163f78a2e18c777ade87029d1d4e2d9e0577a1fb2d3ca9b4`
(42,265 nodes, 62,254 relations). It returned the exact Claim and qualified RU
form, retained the whole five-member value with a distinct literal identity,
and exposed the Claim from the corpus and all five members at depth two.
Each response stayed within 80 nodes / 100 relations; the Jenseits focus
reached that bound. Named property filters selected partial coverage and total
ordering. The two corpus carriers retained the same entity identity.

Observed timings in seconds: cold union 38.880; warm union 0.000371; Claim
inspection 0.359; six focuses 0.406 / 0.461 / 1.903 / 0.441 / 0.557 / 2.246;
property lens 0.626; corpus inspection 0.000904. The managed process reported
1.4 GiB peak memory and 431.5 MiB swap. This was a shared-host observation,
not an isolated performance benchmark or latency-budget pass.

## Handoff limits

This review covers source/schema semantics and the actual local Python reader.
The integration owner must regenerate the combined KAG family and verify
Worker/D1 and actual UI consumption on the resulting exact union. No CI,
merge, deployment, production health, independent historical assessment or
Foundation v1 completion is claimed by this note.
