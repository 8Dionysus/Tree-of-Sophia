# A02 — «Наставления Шуруппака»: Work, witnesses, composite, edition

Checked: 2026-08-22

Scope: A02 source-backlog lines 15 and 27, `Instructions of Šuruppag` and `Bendt Alster, исследования по Šuruppag`

Result: plant one ancient Work identity, two exact Early Dynastic physical witnesses, one provider-independent modern scholarly composite, and Bendt Alster's 1974 edition Work. Do not merge the composition, tablets, catalog records, transliterations, translations, editions, or interpretations.

## Outcome first

The source is neither one tablet nor one web page. It is a historically stratified composition known through materially distinct witnesses and modern editorial reconstruction:

```text
ancient composition Work: Instructions of Šuruppag
  -> surviving physical witnesses: P010233, P222243, and others
  -> modern scholarly composite: Q000782 / P478969 / ETCSL 5.6.1
  -> mutable current representations: CDLI score, revision, catalog; ETCSL composite and translation
  -> critical publication Works: Alster 1974, Civil 1984, later corrections
  -> interpretations: redaction, social norm, gender, wisdom tradition
```

The arrows express the source route, not identity. The physical artifacts are not the ancient Work. The modern composite is not an ancient complete original. A provider page is not the composite identity. A translation is not a transcription, and neither is automatically an accepted reading or semantic fact.

No cuneiform, transliteration, translation, scan, photograph, line art, semantic claim, graph relation, or canon judgment is admitted in this planting.

## 1. Official and classical documentation

### ETCSL 5.6.1

The Oxford [ETCSL composite text](https://etcsl.orinst.ox.ac.uk/section5/c561.htm), [translation](https://etcsl.orinst.ox.ac.uk/section5/tr561.htm), [bibliography](https://etcsl.orinst.ox.ac.uk/section5/b561.htm), and [catalogue](https://etcsl.orinst.ox.ac.uk/catalogue/catalogue5.htm) provide the classical digital route. ETCSL presents a composite Sumerian transliteration and an English translation with numbered lines, variants, lacunae, repeated framing formulas, and explicit source variation. The catalogue places 5.6.1 under the project's modern category “Wisdom literature”. This category is retained as an attributed editorial classification, not an ancient genre label or philosophy verdict.

ETCSL's [site documentation](https://etcsl.orinst.ox.ac.uk/site.htm) states that modern English titles are used for works otherwise identified through incipits. Therefore `Instructions of Šuruppag` is a stable modern label, not an attested ancient title string. No original-language title is invented from the transliteration.

ETCSL's [credits and copyright page](https://etcsl.orinst.ox.ac.uk/credits.htm) gives the required citation and copyright statement. It does not supply an open-content license for a ToS republication. The present route retains metadata and fingerprints only.

Current response fingerprints, streamed and not retained:

| Surface | Bytes | SHA-256 |
|---|---:|---|
| ETCSL composite 5.6.1 | 36,489 | `feba7f6adeff8afa2155793d89b5213568139346fec12b284b76a9f088200015` |
| ETCSL translation 5.6.1 | 24,534 | `577cff6444b6a3f2176d608fc61868b2654084e49b938edff2af5e0ce3789d7c` |
| ETCSL bibliography 5.6.1 | 6,305 | `207c0df7f0a7b011c37c7fbdae737cc9c284128295a8a1823c701c8523249f25` |
| ETCSL credits | 5,967 | `56130b894f6a0dd83a35e60f5c2b21ebcceac153d437573d0b2d91bad0a26c75` |

### CDLI composition and current digital states

CDLI currently exposes the same scholarly composition through several non-identical surfaces:

- [Q000782 score](https://cdli.earth/artifacts/composites-score/Q000782), cited by CDLI as updated 21 August 2026, aligns composite lines with named witnesses.
- [revision 2252483](https://cdli.earth/inscriptions/2252483), cited by CDLI as updated 20 August 2026, is a particular current inscription revision.
- [P478969](https://cdli.earth/artifacts/478969) is the older composite-artifact registry identity; its JSON supplies the full current witness array.

The P478969 JSON reports 88 witnesses: 85 Old Babylonian, two Early Dynastic IIIa, and one Neo-Assyrian. Sixty-two are cataloged from Nippur, six from Ur, ten with uncertain provenience, and the rest across Adab, Abu Salabikh, Assur, Kish, Kutalla, and Susa. These are dated provider observations, not a permanent or complete ontology of the tradition.

The score and revision pages changed immediately before this research cut-off. This is the freshest exact evidence found and demonstrates why a stable composite identity must remain separate from mutable provider representations.

Current response fingerprints, streamed and not retained:

| Surface | Bytes | SHA-256 |
|---|---:|---|
| CDLI Q000782 score | 953,906 | `6580ff5362fe467d51694bf804ff005d366cf476bda17b3d6261ef9cc427bddf` |
| CDLI revision 2252483 | 45,411 | `a560e304c805ce5d17be79c6392ffceeb044d873416ae0bdb8495f440c357e36` |
| CDLI P478969 HTML | 168,101 | `2736df77663a2693b33d1dd72b478fd585d8da8d081a9d5e01037c5963ab2c6d` |
| CDLI P478969 JSON | 186,998 | `dedc1b80b142ee1eeeb60d6e9771b751bc79aea7ecdf1d110eaca0baa4d58d81` |

### Early Dynastic physical witnesses

Two exact early witnesses are planted because they materially prevent the Old Babylonian composite from swallowing the surviving-object layer.

#### Abu Salabikh: P010233

The current [CDLI P010233 record](https://cdli.earth/artifacts/10233) identifies `CDLI Literary 000782, ex. 023`, excavation ensemble `AbS-T 0393 + AbS-T 0305`, museum number `IM 070204 +`, clay tablet, Sumerian, ED IIIa (ca. 2600–2500 BC), held by the National Museum of Iraq. The provider reports the provenience as `uncertain (mod. Abu Salabikh)`; ToS preserves that uncertainty instead of converting the site hint into a certain findspot. CDLI relation 15492 links the object to Q000782.

Its bibliography includes Biggs's *Inscriptions from Tell Abū Ṣalābīkh* (OIP 99, 1974, nos. 256 + 323), Alster 1974, Civil 1984, and Alster's 1991 Early Dynastic study. The official [OIP 99 PDF](https://isac.uchicago.edu/sites/default/files/uploads/shared/docs/oip99.pdf), pp. 57–62, publishes the Abu Salabikh version and compares the Adab witness and then-unpublished Old Babylonian reconstructions. The PDF was read remotely and not retained.

Current fingerprints: HTML 92,173 bytes, SHA-256 `ca0a0e0607c2296659eede3544c20b01ec7b7aca19659c144e8e472ddc69875c`; JSON 34,000 bytes, SHA-256 `392111f91be36a3c4ae508f32521fd9088abf9abd3502ca17924a63585519507`.

#### Adab: P222243

The current [CDLI P222243 record](https://cdli.earth/artifacts/222243) identifies `CDLI Literary 000782, ex. 003`, museum ensemble `OIM A00645 + OIM A00649a–i`, clay tablet, Sumerian, ED IIIa (ca. 2600–2500 BC), provenience Adab (modern Bismaya), held by the Institute for the Study of Ancient Cultures Museum. CDLI relation 141941 links the object to Q000782. The object is fragmentary; neither the composite nor modern joins erase that physical state.

Its classical publication route is OIP 14, *Inscriptions from Adab* (1930), nos. 55 and 56; OIP 138, *Bismaya: Recovering the Lost City of Adab* (2012), p. 134, is the modern archaeological control.

Current fingerprints: HTML 84,247 bytes, SHA-256 `f6569e4e3939043c801a010b0566c3bcb09909d5a1c2ff5526f8c169fd70c2ab`; JSON 9,119 bytes, SHA-256 `d65ea682c976dd6c9cf8661423927d8cb723052105b93f1ece85cbb1dc487e0e`.

### ORACC/ePSD2 supporting route

The [ePSD2 early-literature index](https://oracc.museum.upenn.edu/epsd2/earlylit/withatf) currently exposes Early Dynastic literary witnesses by period and provenience. It is a useful current query surface and independent route back to individual objects. It is not used as a new identity for the composition, and no ATF payload was acquired.

## 2. Established and field-shaping scholarship

- Bendt Alster, [*The Instructions of Šurruppak: A Sumerian Proverb Collection*](https://cdli.earth/publications/1685524), Mesopotamia 2 (Copenhagen: Akademisk Forlag, 1974), is the foundational modern edition Work. The [CiNii record](https://cir.nii.ac.jp/crid/1971149384813357314) confirms authorship, publication year, publisher, ISBN `8750015001`, the reconstruction and translation of multiple versions, and the modern alternative title “Instructions of Shuruppak to his son Ziusudra”. No book body was acquired.
- Miguel Civil, [“Notes on the ‘Instructions of Šuruppak’”](https://doi.org/10.1086/373090), *Journal of Near Eastern Studies* 43(4) (1984): 281–298, is the major critical correction and comparison route.
- Bendt Alster, [“Shuruppak's Instructions—Additional Lines to the Adu Manuscript and Notes on the Ur III Fragment”](https://doi.org/10.1515/zava.1990.80.1-2.15), *Zeitschrift für Assyriologie* 80 (1990): 15–19, preserves the incremental nature of reconstruction.
- Bendt Alster, “Early Dynastic Proverbs and Other Contributions to the Study of Literary Texts from Abū Ṣalābīḫ”, *Archiv für Orientforschung* 38/39 (1991/92): 1–51, is the direct established control for the early witness and proverb context.

Alster 1974 is planted as a modern Work for line 27. It is not the ancient Work, the full witness set, or the current composite. The compact backlog phrase “исследования по Šuruppag” is broader than one publication, but Alster 1974 is the exact central edition around which the later corrections are organized.

## 3. Freshest directly relevant scholarship and source state

- Nili Samet, [“Redaction Patterns in Biblical Wisdom Literature in Light of the Instructions of Shuruppak”](https://doi.org/10.1515/zaw-2021-2005), *Zeitschrift für die alttestamentliche Wissenschaft* 133(2) (2021): 208–224, uses collection growth, opening and closing formulas, and religiously oriented redaction as empirical controls. It supports a stratified redaction model, not a license to infer every stage from the composite alone.
- Nili Samet, [“Instructions of Shruppak: The World's Oldest Instruction Collection”](https://cris.biu.ac.il/en/publications/instructions-of-shruppak-the-worlds-oldest-instruction-collection/) (source spelling retained), in *Human Interaction with the Natural World in Wisdom Literature and Beyond* (2023), pp. 216–229, is a current synthetic control. “World's oldest” remains an attributed title claim and is not promoted into ToS authority.
- Özlem Albayrak, [“Eski Mezopotamya Bilgelik Edebiyatında İdeal Erkeklik: Šuruppak’ın Talimatları Üzerine Bir İnceleme”](https://doi.org/10.46931/aran.1742808), *Archivum Anatolicum* 19(2) (17 December 2025): 291–323, is the freshest directly relevant peer-reviewed interpretive article found. It studies continuity and variation in social roles and ideals across versions. Its gender and norm conclusions remain interpretation claims outside this source planting.
- CDLI's Q000782 score and revision were updated on 21 and 20 August 2026. These are newer and more directly source-relevant than calendar-new popular translations or summaries. They prove current provider activity, not final philological correctness.

The 2025 article and August 2026 CDLI updates are added to the existing research line because “fresh” means epistemically close to the exact object and question, not merely recently published.

## General web last

General search was run only after the official, classical, established, and current scholarly routes. A 2026 church-library translation claiming the “oldest wisdom text”, a recent Reddit discussion, Wikipedia, World History Encyclopedia, and generic downloadable PDFs were rejected as authorities. They add no exact witness identity, critical apparatus, stable provenance, or stronger rights evidence. Their recency does not outrank CDLI, ETCSL, the physical records, or peer-reviewed research.

## Rights and publication boundary

The [CDLI terms](https://cdli.earth/terms-of-use) state that text, transliterations, and translations may be copied, aggregated, and reused under fair academic practice with appropriate attribution for considerable reuse. Photographs generally belong to holding institutions, and line art to named authors or publication rights holders; publication requires separate permission. The current terms response was 29,361 bytes, SHA-256 `f4f6ec04c1cbb2d521ef6a679da641cd89291f99fe3bb07647a950e342f3d65e`.

ETCSL identifies copyright and citation requirements but no open license. Alster, Civil, Samet, and most modern publications remain restricted or metadata-visible. Open viewing, ancient date, and provider access do not authorize ToS to publish modern transcriptions, translations, scans, photographs, line art, or editions.

The tracked route therefore remains `public_metadata_only`; all source-bearing content is absent. A later exact-content experiment must select a version, assess rights per layer, preserve attribution, test philological quality, and request permission when required.

## Laboratory route

### A — representation-only comparison

Compare ETCSL composite/translation, CDLI Q000782 score, revision 2252483, and P478969 witness array without storing text. Measure retrieval time, provider drift, line-coordinate differences, witness coverage, missing fields, and human correction. Manually inspect exact pages and artifact records; a green schema is not acceptance.

### B — early-witness stratification

Use P010233 and P222243 as a preregistered physical-witness pair, then sample Old Babylonian Nippur and Ur copies. Test whether the method preserves artifact identity, provenience, custody, period, fragmentation, inscription state, composite membership, edition use, and interpretation separately. Record cost, time, source-return success, and every mistaken merger.

### C — critical-edition and current-revision comparison

Compare Alster 1974, Civil 1984, later Alster corrections, ETCSL, and the current CDLI score/revision. Evaluate apparatus visibility, lacuna handling, variant preservation, translation mediation, citation precision, and semantic overreach. Any original/AI/human translation branch must add etymology, morphology, semantic range, recognized-translation comparison, uncertainty, and foreign-language expert review rather than treating fluent output as correctness.

## Planting decision

Plant:

1. `tos.work.sumerian-literature.instructions-of-shuruppag` as the ancient composition identity with no ancient authorship or exact composition date asserted;
2. `tos.artifact.sumerian.uncertain.im-070204-plus` and `tos.artifact.sumerian.adab.oim-a00645-plus-a00649a-i` as exact physical witnesses;
3. `tos.composite.sumerian.instructions-of-shuruppag` as a provider-independent modern critical reconstruction with current CDLI and classical ETCSL representations;
4. `tos.work.sumerian-scholarship.the-instructions-of-shurruppak-a-sumerian-proverb-collection` and Bendt Alster's source-reported authorship claim;
5. separate A02 plantings for backlog lines 15 and 27.

Do not create an ancient author, fixed original-language title, exact composition date, source text, translation, semantic node, graph relation, canon state, public payload, access request, external message, or human task.
