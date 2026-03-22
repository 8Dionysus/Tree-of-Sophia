# ToS Review Checklist

Use this checklist when `Tree-of-Sophia` changes but no public validator exists yet.

The goal is not to imitate a machine check.
The goal is to make source-first review repeatable and explicit.

## When to use it

Use this checklist for changes to:

- `README.md`
- `BOUNDARIES.md`
- `ROADMAP.md`
- `docs/KNOWLEDGE_MODEL.md`
- `docs/NODE_CONTRACT.md`
- `docs/PRACTICE_BRANCH.md`
- `docs/COUNTERPART_POLICY.md`
- `docs/CONTEXT_COMPOST.md`
- new authored architecture notes

## Review route

Walk the change in this order:

1. Compare the change against `README.md` and the owning architecture note it touches.
2. Check that authored meaning remains distinguishable from derived structure.
3. Check that source, extraction, interpretation, and synthesis layers remain legible.
4. Check that lineage and context did not disappear where they matter.
5. Check that downstream AoA operational detail did not quietly move into ToS.
6. Check that counterpart mapping, when present, stays optional and non-identity.
7. Check that compost routes, when present, preserve source refs, review state, and decay or demotion posture.
8. Check that uncertainty, ambiguity, or contestability did not get flattened into false certainty.

## Checklist

Answer each item with `yes`, `no`, or `not-applicable`.

- Does the changed surface keep a visible path back to source or authoritative meaning?
- Are node layers still distinguishable rather than collapsed into one summary voice?
- Are lineage relations still explicit where the subject needs them?
- Are temporal, spatial, civilizational, or interpretive contexts still attached where they matter?
- Is ToS still clearly authored truth rather than a restatement of a derived KAG or operational AoA surface?
- If practice lineage is mentioned, does it stay conceptual rather than absorbing operational ownership?
- If counterpart mapping is mentioned, does it stay derived, optional, and explicitly non-identity?
- If context compost is mentioned, do source refs, review state, and decay or demotion posture remain visible?
- Does the change preserve plurality rather than forcing every branch through one interpretive flattening?
- Are any new abstractions reversible and narrower than the material they summarize?
- Does the change name uncertainty honestly where the source or interpretation remains contested?

## Stop conditions

Pause and revise before merge if any answer is `no` for:

- source traceability
- authored versus derived distinction
- node layering
- lineage preservation
- ToS versus AoA ownership boundary

## Review note

A short review note should record:

- what changed
- which checklist items were most at risk
- what remained interpretive or unresolved
- whether follow-up belongs in `aoa-kag`, `Agents-of-Abyss`, or another neighboring repository
