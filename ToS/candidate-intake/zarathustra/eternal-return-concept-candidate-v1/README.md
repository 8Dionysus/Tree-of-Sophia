# Eternal return concept candidate v1

This pass turns “eternal return” into one stable, trackable annotation
candidate over the complete four-part German/Russian technical corpus. It is a
semantic dossier, not an accepted concept.

The evidence spine distinguishes four roles:

- `core`: explicit recurrence formulas in the declared central readings;
- `supporting`: eternity, ring, time, joy, life, and affirmation passages in
  those readings;
- `ambiguous`: lexical neighbors elsewhere in the work;
- `excluded`: declared controls where return means returning to humans or
  returning home.

Tracked evidence contains alignment and source-anchor references, exact-text
digests, signal codes, and candidate claim links, but no witness text. Exact
German and Russian passages, mutable formula labels, and concordance excerpts
remain in the ignored mode-`0600` private analysis referenced by the manifest.

`concept-candidate.v1.json` conforms to `sign-annotation.schema.json` with
`sign_layer: concept_candidate`. It carries an opaque `annotation_id`, mutable
labels, an inclusion law, candidate facets, and three coexisting interpretive
readings. It deliberately carries no `sign_id` or `concept_id`.
`interpretation-templates.v1.jsonl` does not materialize claims: each row is an
unreviewed template with `materialized_claim: false`. The graph projection is
explicitly empty.

`independent-agent-audit.v1.json` records a separate semantic-design challenger
and an exhaustive source challenger. The latter traversed all 3,423 mappings
and all 7,016 paragraph anchors, and its occurrence/passages denominator is
kept distinct from this dossier's aligned-unit denominator. Five alignment/OCR
gaps are retained explicitly; two central Russian III.13 formulas are visible
in the witness but missed by the direct detector, so no silent correction is
performed.

Build and verify:

```bash
python scripts/build_zarathustra_eternal_return_concept_candidate_v1.py --build --issue-identities
python scripts/build_zarathustra_eternal_return_concept_candidate_v1.py --check
python -m unittest tests.test_zarathustra_eternal_return_concept_candidate_v1
```
