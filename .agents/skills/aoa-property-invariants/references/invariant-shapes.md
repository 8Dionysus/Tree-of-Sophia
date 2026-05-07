# Invariant Shapes

Use this reference when the invariant is wider than one ordinary data rule. It
is not a checklist. Pick the smallest shape that names one stable truth, one
variation space, one property check, and one claim limit.

## How To Use

1. Name the stable truth in plain language.
2. Name the input space, artifact family, state transition, or transformation.
3. Choose the narrowest invariant shape below.
4. State valid inputs and exclusions before writing the property.
5. Keep a few concrete examples when they make failures easier to understand.

## Shape Set

| Shape | Protected Truth | Variation Space | Property | Avoid | Useful Check |
|---|---|---|---|---|---|
| Conservation or accounting | Totals, balances, references, or counts remain conserved. | Operation sequences, grouped artifacts, filtered subsets. | Sum, count, or reference relation stays equal under valid operations. | Counting examples while missing state sequences. | Stateful property plus boundary examples. |
| Monotonicity or ordering | Progress, version, priority, timestamp, or maturity only moves in allowed directions. | Status transitions, append-only logs, sorted lists, version bumps. | Movement never reverses or violates ordering rules. | Assuming all newer data is better or authoritative. | Random transition sequence with invalid-reversal cases. |
| Idempotency or repeatability | Repeating a safe operation does not change meaning after the first run. | Rebuilds, imports, normalization, refreshes, retries. | Same source and options produce equivalent result. | Ignoring timestamps, nondeterministic IDs, or environment effects. | Repeated-run property with stable fields and explicit exclusions. |
| Round trip or normalization | Parse, serialize, import, export, or normalize preserves intended meaning. | Encodings, field order, optional values, aliases. | `decode(encode(x))` or normalized variants preserve semantic fields. | Treating formatting bytes as meaning unless required. | Round-trip fixtures plus generated cases for optional fields. |
| Structural or schema relationship | Required relationships between fields or nodes stay valid. | Manifests, trees, graphs, references, nested objects. | IDs are unique, refs resolve, parent/child or source/output links hold. | Schema validity without relationship checks. | Generated graph/manifest variants with invalid-ref negative cases. |
| Lifecycle or state machine | Objects move only through allowed states. | Candidate/evaluated/canonical, pending/active/closed, draft/released/deprecated. | Every transition is allowed and required evidence appears at gates. | Treating roadmap or provenance as current state. | Transition generator plus forbidden-transition examples. |
| Source to generated/export | Derived surfaces preserve source identity and stay subordinate. | Builders, compact indexes, installed copies, exports, reports. | Eligible sources appear once, refs are preserved, excluded sources stay out. | Generated freshness as proof of source meaning. | Rebuild/idempotency property plus source-ref and exclusion checks. |
| Routing or selection | Selection stays stable under irrelevant variation and changes under relevant signals. | Dispatch hints, filters, priority rules, compatibility choices. | Equivalent inputs route the same; changed decisive signal changes result. | Encoding today's incidental ordering as law. | Permutation/irrelevant-field property plus tie-break examples. |
| Authorization or risk boundary | Unsafe actions require explicit authority or safer fallback. | Mutation surfaces, share targets, infra actions, risk tiers. | Risky route cannot proceed without required approval or gate output. | Using property language to bypass human approval. | Generated risk cases with required denial and allow cases. |
| Provenance or memory/recall | Retrieved, stored, or derived evidence keeps source and freshness limits clear. | Recall objects, writebacks, capsules, summaries, retention windows. | Source refs, timestamps, retention bounds, and claim limits survive transformations. | Treating recall output as live proof. | Transformation property plus stale/missing-source negative case. |

## Compact Invariant Pass

| Field | Answer |
|---|---|
| Stable truth |  |
| Shape selected |  |
| Variation space |  |
| Valid inputs |  |
| Exclusions |  |
| Property statement |  |
| Generator or repeated-check plan |  |
| Concrete examples kept |  |
| Claim limit |  |

## Verification

- the property protects a named stable truth
- generated or repeated cases vary the meaningful dimension
- exclusions are explicit enough for review
- failure output points back to the invariant, not only generated data
- broad coverage does not claim total quality beyond the named property
