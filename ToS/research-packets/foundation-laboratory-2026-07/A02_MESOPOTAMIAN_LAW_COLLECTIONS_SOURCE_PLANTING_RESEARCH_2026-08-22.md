# A02 — Mesopotamian law collections source planting research

Date of research cut: 2026-08-22
Scope: backlog line 16, `law collections Ur-Namma / Lipit-Ishtar / Hammurapi`
Authority: research scaffold only; not source text, legal history canon, translation authority, or a claim that the collections were positive law.

## Result first

Plant three ancient Works and three provider-independent modern critical composites, not one invented aggregate Work called “Mesopotamian law codes.” Plant one exact physical witness for Ur-Namma and one for Lipit-Ištar, and reuse the already tracked Louvre `SB 8` witness for Hammurapi.

The foundation therefore separates:

```text
ancient composition Work
  ≠ king or royal speaker as modern author
  ≠ one physical tablet or stela
  ≠ mutable CDLI record or composite
  ≠ modern critical edition and line numbering
  ≠ recognized or experimental translation
  ≠ interpretation as statute, code, justice, ethics, or philosophy
```

No inscription, transliteration, translation, photograph, scan, critical-edition body, or model output is acquired or published in this slice.

## 1. Official and classical documentation

### Current source and genre controls

The [CDLI Law Collection genre history](https://cdli.earth/genres/history/30), updated and approved on 2026-01-01, prefers “law collections” while acknowledging “law codes” as a common label. It says that the relationship to everyday legal practice remains debated while the place of the material in a scholarly tradition does not; it also identifies casuistic form, expansion, and scribal imagination. This directly controls the A02 risk at backlog line 6: ToS may speak of legal-normative writing, royal justice, royal inscription, and scholarly transmission, but may not silently promote the compositions into modern positive legislation.

The current exact CDLI provider state observed on 2026-08-22 was:

| composition | CDLI composite | current inscription revision | live witnesses | dated period distribution |
|---|---|---:|---:|---|
| Laws of Ur-Namma | `Q000947` / `P432130` | `2269337` | 9 | 8 Old Babylonian; 1 Ur III |
| Laws of Lipit-Ištar | `Q000613` / `P464355` | `2269336` | 13 | 13 Early Old Babylonian |
| Laws of Hammurapi | `Q006387` / `P464358` | `2337705` | 27 | 4 Old Babylonian; 2 Middle Assyrian; 21 Neo-Assyrian |

These counts are current provider observations, not stable composition identities or proof of completeness. The provider pages show older record-update dates in places; those dates must not be confused with the current inscription revisions or the 2026 access event. A later exact-byte retry from the host timed out, so no false composite fingerprint or snapshot was fabricated.

### Exact physical witnesses

1. Ur-Namma: [CDLI P226580](https://cdli.earth/artifacts/226580), `Ist Ni 03191`, RIME `3/2.01.01.20, ex. 01`, Old Babylonian clay tablet, reported Nippur provenience, Istanbul Archaeological Museums. The live API reported direct relation `150014` to `Q000947`. Its JSON response was streamed but not retained: 18,305 bytes, SHA-256 `a319dbcc365284f2b019e63fbe101c51e806609cc771cf92a5794e85a81f864f`.

2. Lipit-Ištar: [Louvre AO 05473](https://collections.louvre.fr/en/ark:/53355/cl010166109), also CDLI `P345378`, RIME `4.01.05.add10, ex. 01`, Early Old Babylonian Sumerian clay tablet, 11.4 × 6.4 × 3.2 cm. CDLI reports uncertain provenience and direct relation `365423` to `Q000613`; ToS therefore uses `uncertain/` in the physical topology and does not infer Nippur. The Louvre page reports acquisition in 1910 and was updated 2025-01-10. Page and JSON were fingerprinted without retention: `93c6d6b0fca73c6873beb546f28fd88a392872af75db2ecdc3492a9a7210f39a` and `caa5b5a9acc95d0edd8f658fadd796fee96a857ba1ca7d4d35f691c3f5c17311`.

3. Hammurapi: reuse [Louvre SB 8](https://collections.louvre.fr/ark:/53355/cl010174436) / CDLI `P249253`, already tracked as `tos.artifact.old-babylonian.susa.hammurabi-stele-sb-8`. The current composite relation is `198588`. Reuse prevents two IDs for one physical monument and preserves the earlier artifact-level research and rights boundary.

Alternative witnesses remain valuable without being planted now:

- Ur-Namma `P250820` / Schøyen `MS 2064` is an Ur III clay cylinder and chronologically important, but its private custody and uncertain provenience require a separate provenance and rights inquiry.
- Lipit-Ištar `P256663` is a joined Nippur tablet in the Penn Museum and offers a strong institutional route, but one representative physical witness is sufficient for this bounded slice.

### Classical publication lineage

- Samuel N. Kramer, [“Ur-Nammu Law Code”](https://cdli.earth/publications/1669158), *Orientalia* NS 23.1 (1954), 40–51, is the classical publication route for `P226580`.
- Francis Rue Steele, [“The Code of Lipit-Ishtar”](https://www.journals.uchicago.edu/doi/10.2307/500438), *American Journal of Archaeology* 52.3 (1948), 425–450, is the classical edition route for the Lipit-Ištar collection.
- The existing Hammurapi witness packet retains Scheil 1902 and the later RIME catalog route; this packet does not duplicate that work.

## 2. Established field-shaping scholarship

- Miguel Civil, [“The law collection of Ur-Namma”](https://cdli.earth/publications/1749484), CUSAS 17 (2011), 221–286, is a major reconstruction control.
- Claus Wilcke, *Der Kodex Urnamma (CU): Versuch einer Rekonstruktion* (2002), and [“Gesetze in sumerischer Sprache”](https://cdli.earth/publications/1749485) (2014), provide reconstruction and direct Ur-Namma/Lipit-Ištar controls.
- Martha T. Roth, [*Law Collections from Mesopotamia and Asia Minor*](https://cart.sbl-site.org/books/061506E), 2nd ed. (1997), is the standard cross-corpus translation and reference control. It remains a modern scholarly Work with its own rights, not an ancient source or ground truth.
- Raymond Westbrook, “Cuneiform Law Codes and the Origins of Legislation,” *ZA* 79.2 (1989), DOI [`10.1515/za-1989-790231`](https://doi.org/10.1515/za-1989-790231), directly controls the legislation question.
- Pamela Barmash, [*The Laws of Hammurabi: At the Confluence of Royal and Scribal Traditions*](https://academic.oup.com/book/31901) (2020), supports treating royal inscription and scribal tradition together without collapsing them.

The practical conclusion is narrow: “law collection” is the preferred source label; “code” may be retained as an attributed historical title variant. Neither label proves direct enforcement, uniform legal force, or a single function across the three compositions.

## 3. Freshest directly relevant controls

Freshness was checked after the classical and established route, not used as a substitute for it.

- The 2026-01-01 [CDLI genre update](https://cdli.earth/genres/history/30) is the freshest direct semantic control and is more useful here than calendar-new popular summaries.
- The exact CDLI composite and relation state observed on 2026-08-22 is the freshest operational source state. It governs current provider coordinates only.
- Martha T. Roth, [“On Researching and Teaching Mesopotamian Law”](https://www.journals.uchicago.edu/doi/10.1086/739844), *JNES* 85.1 (2026), 173–183, was found as the newest directly relevant scholarly lead. The body was not accessible in this research route, and the publisher reserves text/data-mining and AI rights; only bibliographic metadata is used. No substantive conclusion is attributed to the inaccessible article.
- Jana Matuszak, [“Law, Morality, and Subversion in Sumerian Prose Miniatures”](https://edizionicafoscari.unive.it/en/edizioni4/libri/978-88-6969-776-0/law-morality-and-subversion-in-sumerian-prose-mini/) (2024), is a recent open peer-reviewed control for the border between law, morality, wisdom, and scribal literary work. Its analysis may later inform claims, but it is not source authority.
- The Louvre AO 05473 record, updated 2025-01-10, is the freshest exact holding-institution control in this bounded physical-witness set.

General web pages and popular “oldest law code” summaries were searched only last and rejected as authority because they flatten composition, witness, edition, translation, and legislation status.

## Rights and publication boundary

The [CDLI terms](https://cdli.earth/terms-of-use) describe conditional scholarly reuse of text, transliterations, and translations with attribution; photographs, line art, holding-institution objects, and third-party publications remain separately governed. That statement is not projected onto Louvre media, museum records, Roth, Wilcke, Steele, Kramer, Barmash, or any translation.

For each planted object, rights are assessed separately for:

- bounded public metadata;
- transliteration or modern editorial reconstruction;
- translation;
- photograph, line art, scan, 3D model, or publication body.

Only the first is admitted into Git. Open viewing and ancient date do not create publication permission. Local research remains possible where law and access terms allow it, but local availability never becomes server or public authority automatically.

## A/B/C laboratory route prepared, not run

The source foundation supports later manual experiments without pre-accepting text:

- A — inspect one physical witness and its exact catalog metadata alone;
- B — inspect a modern critical composite with witness boundaries and lacunae visible;
- C — compare composite, a modern critical edition, and current provider revision while preserving disagreements.

Manual checks must ask whether the method:

- keeps tablet, stela, Work, composite, edition, translation, and interpretation separate;
- preserves lacunae and period differences rather than synthesizing a fluent false original;
- exposes royal voice without inventing modern authorship;
- retains prologue, provisions, and epilogue as structure without treating editorial numbering as ancient stable identity;
- distinguishes legal-normative and royal-justice language from a claim of positive law;
- records every translation decision, alternative, etymological source, model, prompt, and human judgment;
- reduces human correction time without hiding errors behind a validator.

No laboratory result, semantic node, graph edge, or canon promotion is produced by this planting.

## Decision and remaining controls

Decision: plant the three exact composite routes and their ancient Work identities. Reuse the Hammurapi stela; add P226580 and AO 05473 as representative physical witnesses. Preserve Roth and Wilcke as separate upcoming control anchors at backlog lines 24 and 26 rather than consuming them silently inside line 16.

Remaining controls:

- composition date, manuscript date, royal reign, redaction stage, copy date, and provider revision date remain different fields;
- king, speaker, promulgator, commissioner, scribe, copyist, excavator, editor, and translator remain different responsibilities;
- the three compositions are not asserted to form one ancient collection;
- `law collection`, `code`, `legislation`, `justice`, `ethics`, and `philosophy` are not synonyms;
- witness counts and period distributions must be rechecked before any content experiment;
- the current composite bodies were not byte-captured because a retry timed out; member-level fingerprints and exact provider coordinates remain the available audit trail;
- no source text, translation, image, scan, critical-edition body, semantic claim, graph relation, canon state, access request, server transfer, or human task is created.
