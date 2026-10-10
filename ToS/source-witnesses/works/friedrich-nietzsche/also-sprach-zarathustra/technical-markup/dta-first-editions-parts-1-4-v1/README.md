# Zarathustra DTA technical markup v1

This route materializes the non-semantic technical spine of the four German
first-publication parts represented by the exact local DTA TEI witnesses.

It records:

- one part-local document unit for each DTA item;
- TEI divisions and source-marked paratext containers as sections;
- paragraphs, verse groups, verse lines, headings, list items, quotes, page
  breaks, and source milestones;
- reciprocal parent/child membership;
- direct and recursive paragraph/verse counts for every container, plus the
  nearest major-structure binding and within-major ordinal for each unit;
- opaque unit identities issued independently of labels, text, ordinals,
  offsets, and the current citation display;
- a rebuildable text-free citation spine such as
  `Za-DE-III.s013.s002.p004`;
- the existing 82 machine-corroborated major-structure candidates, while
  retaining the Part-I speeches wrapper as a wrapper rather than silently
  counting it as another reading unit.

The attached `Dionysos-Dithyramben` sequence in the Part-IV source container
is outside this work-level route. Its division subtree and two immediately
preceding source-container page milestones are explicitly excluded by exact
tracked TEI resource IDs. They are not deleted or reclassified in the source
item.

## Authority ceiling

The TEI boundaries are recorded as `observed_source_structure`. This means
only that the selected DTA representation contains those elements. It does not
accept a critical German text, settle editorial hierarchy, create a sentence,
word, lemma, sign, concept, relation, translation alignment, graph fact, or
canon node.

The tracked artifacts contain locators, identities, digests, counts, and
status only. The sequential extracted German layers remain mode-0600 below the
already governed `gold-sets/foundation-pilot-v1/local-content/technical-markup-v1/`
storage boundary and are not authorized for publication. This storage reuse
does not make the technical packets a gold set or part of the semantic pilot.

Russian witnesses are intentionally absent from this German v1 packet. The
complete witness-local Antonovsky structural/paragraph route lives at
`../antonovsky-1911-structural-paragraph-v2/`; the older
`../antonovsky-1911-pdf-layout-v1/` remains its fixed layout-observation
predecessor. No German/Russian correspondence is created by the coexistence
of those technical spines.

## Files

- `plan.v1.json` is the authored source, exclusion, count, storage, and
  authority plan.
- `identity-issuance.v1.json` is the stable text-free mapping from exact source
  locators to already issued opaque IDs. Rebuilding never remints them.
- `part-*.source-text-unit.v1.json` are source-bound packets governed by
  `ToS/contracts/source-text-unit-packet-v1.schema.json`.
- `citation-spine.v1.jsonl` is a text-free navigational projection. Its
  citations are display addresses, not identity.
- `summary.v1.json` is a generated count and closure receipt.
- `provenance.jsonl` records the bounded deterministic segmentation event.
- `agent-verification.v1.json` records the completed agent-side hierarchy,
  ordinal, rebuild, and authority-boundary checks.

## Build and check

The installed Rust entry takes explicit absolute roots. Full historical parity
reads the exact private TEI inputs and existing extracted layers without writing:

```sh
tos dta-technical-markup --source-root /absolute/source \
  --local-input-root /absolute/private-input \
  --local-output-root /absolute/private-output --check
tos dta-technical-markup --source-root /absolute/source --validate-tracked
```

For a new segmentation event, provide a reviewed plan selecting fresh output
and issuance paths, a new event ID, and the admitted output quota:

```sh
tos dta-technical-markup --source-root /absolute/source \
  --local-input-root /absolute/private-input \
  --local-output-root /absolute/private-output \
  --plan ToS/source-witnesses/path/to/new-plan.json \
  --event-id tos.event.native-segmentation.example.2026-10-08 \
  --build --issue-identities --scratch-bytes 50331648
```

`--issue-identities` is the explicit initial issuance action. Replay uses the
same plan and event without this flag. Existing identical outputs are retained;
different outputs are refused before writing. Private layers require the
source-owned ignore boundary and mode 0600. Historical provenance continues
to name its original producer; fresh provenance names the Rust executor.

Focused software tests: `cargo test -p tos-compiler --lib dta_technical_markup`.

Any source-locator set change fails closed. Extending or superseding the
identity map requires a reviewed successor operation; the builder does not
silently assign existing IDs to a changed structure.
