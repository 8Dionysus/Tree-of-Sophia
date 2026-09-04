# Zarathustra DE/RU morphology and theme candidates v1

This intake pass is the first content-facing layer above the complete technical
DE/RU lexical and paragraph-alignment foundation. It proposes overlapping form
families, bilingual recurrence neighborhoods, and typed relations. It does not
author a lemma, lexeme, sense, sign, concept, semantic relation, graph edge, or
canon object.

The tracked JSONL files withhold exact German and Russian strings. Readable
forms, provisional display hints, competing analyses, and agent-audit examples
remain in the ignored mode-`0600` private analysis named by `manifest.v1.json`.
Opaque candidate IDs are issued independently of mutable forms, stems, labels,
translations, and cluster hints; the issuance bindings are mechanical closure
keys, not identities or linguistic judgments.

Candidate status has a narrow meaning:

- `proposed`: the declared mechanical gates passed;
- `ambiguous`: a useful but competing or cross-alignment challenger;
- `deferred`: the family or neighborhood is too broad or weak for positive use.

`proposed` still means unreviewed candidate. All rows have empty review refs,
`accepted: false`, and no graph or canon effect.

`independent-agent-audit.v1.json` records two whole-corpus challenger runs. Its
German casing controls and Russian dictionary/quality controls were integrated
before the final build; the audit is explicitly not a human linguistic or
semantic review.

The implementation and focused validation are owned by
`scripts/build_zarathustra_morphology_theme_candidates_v1.py` and
`tests/test_zarathustra_morphology_theme_candidates_v1.py`; execute them
through the [ToS validation routes](../../../VALIDATION.md).
