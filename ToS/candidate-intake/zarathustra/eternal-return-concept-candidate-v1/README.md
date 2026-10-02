# Eternal return concept candidate v1

This pass turns “eternal return” into one stable, trackable annotation
candidate over the complete four-part German/Russian technical corpus. The semantic dossier retains candidate status for source-visible concept
assessment.

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

The implementation and focused validation are owned by
`scripts/build_zarathustra_eternal_return_concept_candidate_v1.py` and
`tests/test_zarathustra_eternal_return_concept_candidate_v1.py`; execute them
through the [ToS validation routes](../../../VALIDATION.md).

The native command `tos zarathustra-eternal-return-concept-candidate-v1`
uses the original `plan.v1.json` by default. A reviewed technical input profile
can be selected with `--plan-ref <repository-relative-ref>` in `--build`,
`--check`, or `--preview`. The selected source root must be owned mode `0700`,
and the profile must be an owned regular mode-`0600` file, at most 64 KiB.
The original plan and its historical outputs remain separate and unchanged.

A profile follows source → proposal → review before use in a fresh carrier.
It names the exact original plan digest and a distinct versioned identity.
Only the SHA256 pins for `parallel_lexical_manifest` and
`morphology_theme_manifest` may change; input references and membership,
`frozen_at`, source scope, selection law, output route, authority boundaries,
and all other pins must match the original plan. The profile retains the
existing opaque identity issuance and must pass its complete binding check,
including in preview. `--issue-identities` is rejected for a custom profile.
Selected plan references and digests appear in provenance and the manifest.

This route authenticates upstream manifest membership. It does not read the
Morphology private analysis or accept morphology, translation, concept,
semantic, graph, or canon judgments. A successful build/check remains a
mechanical result; source-visible review and scoped admission retain their
existing owners. Fresh profile validation is independent of the original
plan's retained-input validation.

The maintained producer entry dispatches to the native `tos` command. Select
an exact installed executable with `TOS_NATIVE_PREPARED_CONSUMER_BIN` and pass
`--source-root` for a separately owned source/carrier root. `--build` produces
candidate files; `--check` regenerates them and compares the full product.
Use `--validate-tracked` to validate existing retained products without
regeneration, provider execution, or writes. This mode checks the original
default plan, input pins, manifest membership/digests, tracked/private modes,
existing opaque bindings, private exact return, counts and declared ceilings.
It reports receipt mechanics, not algorithm equivalence or semantic admission.

The original Python rendering recipes remain independent cold oracle/history
material with their exact Git identities. Both active Eternal entries are
native-only; the former Review-to-Concept Python hydration import retires with
the coordinated family cutover. Historical expected output identities remain
separate from actual native/oracle equivalence; no prior artifact is retagged.
