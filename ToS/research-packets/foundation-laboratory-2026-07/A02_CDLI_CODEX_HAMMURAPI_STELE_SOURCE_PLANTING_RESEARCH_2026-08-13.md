# A02 — CDLI posting “Codex Hammurapi”: exact stela source planting

Checked: 2026-08-13

Scope: A02 source-backlog line 13, `CDLI posting Codex Hammurapi`

Result: plant one physical artifact witness, the Louvre stela `SB 8` / CDLI `P249253`; retain the posting, inscription, composite, images, 3D model, editions, and interpretations as distinct layers.

## Outcome first

The backlog anchor is best grounded by the surviving physical stela, not by a modern web posting and not by the CDLI scholarly composite. The repository identity is therefore:

`tos.artifact.old-babylonian.susa.hammurabi-stele-sb-8`

Its stable path follows the holding-institution identity `Louvre SB 8`, while the mutable CDLI record `P249253` supplies an exact machine-readable catalog route. The CDLI posting is a concise discovery and orientation surface. Its “See artifact” link resolves to the modern composite `P464358` / `Q006387`, not to the physical stela. The first witness of that composite is the physical stela `P249253`.

No inscription text, transliteration, translation, image, line art, 3D model, semantic claim, graph relation, or canon judgment is admitted by this planting.

## Layer map

```text
physical stela Louvre SB 8
  └─ current CDLI artifact record P249253
       ├─ linked inscription representation (not acquired)
       ├─ linked photographs and line art (not acquired)
       └─ witness-of scholarly composite Q006387 / P464358

CDLI posting 185 “Codex Hammurapi”
  └─ concise modern profile; links to the composite, not the physical object

Scheil 1902; Frayne 1990; Roth 1995/1997; Barmash 2020;
Oelsner 2022; Zhiltsova 2023
  └─ publication, interpretation, and critical-control layers
```

The physical object is not the catalog record. The catalog record is not the inscription. The physical witness is not the multi-witness composite. A photograph, drawing, or 3D model is not the artifact. A modern critical edition is not the ancient object or an automatic source-text authority for ToS.

## 1. Official and classical documentation

### CDLI exact route

- The current [CDLI posting](https://cdli.earth/postings/185) describes the Louvre stela, its Susa find context, basalt material, relief, prologue and epilogue, and the conventional 282 legal provisions. Its body is credited to Moudhy al-Rashid. The page is mutable and its generated citation uses the access date; that date is not treated as an authored publication year.
- The posting’s artifact link resolves to [P464358](https://cdli.earth/artifacts/464358), `RIME 4.03.06.add21 (Laws of Hammurapi) composite`, composite number `Q006387`. This is a modern scholarly composite assembled from multiple witnesses.
- The physical stela is [P249253](https://cdli.earth/artifacts/249253), `RIME 4.03.06.add21, ex. 01`, object type `Official or display stela`, museum number `Sb 00008`, held by the Louvre. CDLI reports basalt, Akkadian, Old Babylonian, Susa find context, and height 2250 mm. The current JSON surface is [artifacts/249253/json](https://cdli.earth/artifacts/249253/json).
- The P249253 record links the object to composite `Q006387` / `P464358`. It also shows that metadata and transliteration revisions occurred at different times. This confirms that the provider record is mutable and that exact access-time provenance matters.
- The current [CDLI genre page](https://cdli.earth/genres/30) prefers “law collection”, while noting the established but debated “law code” terminology and the debated relationship between these compilations and everyday legal practice.
- [CDLI Terms of Use](https://cdli.earth/terms-of-use) distinguish catalog metadata, photographs, line art, transliterations, and translations. Open viewing is not a blanket redistribution license.

Current response fingerprints, streamed and not retained:

| Surface | Bytes | SHA-256 |
|---|---:|---|
| `https://cdli.earth/artifacts/249253/json` | 97,830 | `43527b5684b7f6f746f627d76bc24f97cc89c43996ef7a203b405b5ed4da9e33` |
| `https://cdli.earth/artifacts/249253` | 206,264 | `e23a3f46fc96760f8d6036e2da3506f273b50a0032c1f75ba85e533829efc2f8` |

### Louvre originating collection record

The [Louvre collection notice](https://collections.louvre.fr/ark:/53355/cl010174436) identifies the object as `Code de Hammurabi`, denomination `stèle`, inventory `SB 8`, with the additional identifiers `AS 6064` and `PEZARD ET POTTIER 8`. It reports basalt; height 225 cm, width 79 cm, thickness 47 cm; cuneiform; Old Babylonian language; Hammurabi dynasty, 1792–1750 BCE; discovery at Susa in 1901–1902; and current custody in the Département des Antiquités orientales. The notice itself warns that it may not reflect the latest knowledge and records a 2023-10-31 update.

The Louvre’s public [feature page](https://www.louvre.fr/le-code-de-hammurabi) cautions that the monument is not a “code” in the modern sense and presents the 282 numbered sections as a collection of jurisprudential decisions. The collection notice also links a separate Sketchfab 3D representation. Neither page grants ToS publication authority over the object’s visual payloads.

Current Louvre response fingerprints, streamed and not retained:

| Surface | Bytes | SHA-256 |
|---|---:|---|
| `https://collections.louvre.fr/ark:/53355/cl010174436.json` | 55,582 | `8a29b8f00bd68d53ab31c50d560963b296fbde8e0ce7d74f468dbc29a783d3a9` |
| `https://collections.louvre.fr/ark:/53355/cl010174436` | 294,920 | `02bcfc36cbbc36fb2e2d462c12f416c12a1d019b10ff0eb1f8b674a106c385f2` |

### Classical publication route

- Vincent Scheil’s 1902 publication, *Textes élamites-sémitiques*, deuxième série, contains the initial publication of the laws. Its bibliographic identity is controlled by the [BnF catalogue](https://catalogue.bnf.fr/ark:/12148/cb313123921); the Louvre cites the [Internet Archive volume](https://archive.org/details/mmoires04franuoft). No scan or publication body was acquired.
- Douglas Frayne’s *Old Babylonian Period (2003–1595 BC)*, RIME 4 (1990), is the exact primary catalog publication attached by CDLI to `RIME 4.03.06.add21, ex. 01`. It stabilizes the modern scholarly identifier, not the physical object’s identity by itself.

## 2. Established top scholarship

- Martha T. Roth, [“Mesopotamian Legal Traditions and the Laws of Hammurabi”](https://scholarship.kentlaw.iit.edu/cklawreview/vol71/iss1/3/) (1995), is an established methodological control on treating the monument within Mesopotamian legal traditions rather than projecting a modern statute-book model onto it.
- Roth’s *Law Collections from Mesopotamia and Asia Minor*, 2nd ed. (1997), [SBL WAW 6](https://cart.sbl-site.org/books/061506E), with [JSTOR stable record](https://www.jstor.org/stable/jj.25577265), is a standard comparative translation and source-list route. It is not copied into this planting.
- Pamela Barmash, [*The Laws of Hammurabi: At the Confluence of Royal and Scribal Traditions*](https://academic.oup.com/book/31901) (2020), DOI `10.1093/oso/9780197525401.001.0001`, supplies an established control against collapsing royal inscription, scribal transmission, and later legal function into one layer.

## 3. Fresh and currently relevant scholarship

- Joachim Oelsner, [*Der Kodex Ḫammu-rāpi. Textkritische Ausgabe und Übersetzung*](https://www.zaphon.de/kodex-hammu-rapi) (2022), dubsar 4, is the strongest current critical-edition route found for a future content experiment. It presents individual witnesses synoptically and exposes variants and divergences. The full edition was not acquired.
- Liubov Zhiltsova, [NABU 2023/62](https://sepoa.fr/wp/wp-content/uploads/2023/10/NABU-2023-3_LITE.pdf), pp. 122–123, provides a compact current structural map of the stela: columns I–V are the prologue; section 1 begins at V:26; the front preserves sixteen readable columns followed by a seven-column lacuna; the reverse resumes in the tail of section 100; section 282 ends in column XXIII; the epilogue occupies columns XXIV–XXVIII.
- Martha Roth’s 2026 review essay, [“On Researching and Teaching Mesopotamian Law”](https://www.journals.uchicago.edu/doi/abs/10.1086/739844), is current but paywalled. Only bibliographic metadata was available, so it contributes no substantive claim here. No access request is necessary for this narrow artifact planting; the route is recorded for later research if its full argument becomes necessary.

Newer work on machine recognition of cuneiform signs was screened out of this planting. Recency alone cannot replace exact relevance: the present task is a reliable artifact and structure profile, not OCR, sign recognition, translation, or semantic extraction.

## Bounded factual profile

| Field | Admitted statement | Limit |
|---|---|---|
| identity | Louvre stela `SB 8`; CDLI physical record `P249253` | Provider-independent artifact identity follows repository inventory, not a URL |
| form | official/display stela bearing a cuneiform inscription and relief | Object form does not determine legal or philosophical meaning |
| material and size | basalt; 2250 × 790 × 470 mm | Height agrees across CDLI and Louvre; width/thickness come from Louvre |
| dating | Hammurabi dynasty/reign, reported 1792–1750 BCE | Reported catalog dating, not exact manufacture date |
| find context | discovered at Susa, 1901–1902 | Findspot is not composition place; provider narrative about ancient removal remains attributed |
| custody | Musée du Louvre, Département des Antiquités orientales | Current catalog state, checked 2026-08-13 |
| language/script | Akkadian, Old Babylonian; cuneiform | Reported catalog classification |
| composition | prologue, conventionally numbered legal sections, epilogue | Numbering is modern editorial structure, not carved modern section numbers |
| physical gap | seven-column lacuna; reverse resumes near section 100 | Missing stela text must not be silently supplied from the composite |
| composite | `Q006387` / `P464358` | Modern multi-witness scholarly construct, not the stela |

## Claims explicitly not admitted

- that `P464358` is the physical stela;
- that Susa is the place of composition or original display;
- that uncertain `Sippar ?` metadata is established provenance;
- that the stela preserves all conventionally numbered sections continuously;
- that “282 laws” means a complete modern code enacted and applied like current legislation;
- that CDLI’s access date is the posting’s publication date;
- that any available transliteration, translation, photograph, drawing, or 3D model is automatically publishable;
- that an artifact profile establishes philosophical status, semantics, concepts, signs, graph relations, or canon membership.

## Rights and publication boundary

Only a bounded attributed metadata derivative is tracked. CDLI’s terms describe conditional academic reuse for transliterations and translations but require layer-specific attribution and review; no text was acquired. Photographs and line art remain subject to holding-institution and author/publication rights and are not authorized for ToS publication. The Louvre’s visual and 3D surfaces were inspected only as catalog links; no reuse license was established. The ancient inscription’s age does not erase rights and provenance requirements for modern transcription, edition, translation, image, or database layers.

Future ToS publication must repeat a source- and layer-specific rights check. Local research availability is not public redistribution authority.

## Laboratory route after this metadata planting

This planting prepares three manually inspected experiments without claiming that validators prove textual quality:

### A — physical-witness baseline

Use only Louvre and CDLI `P249253` catalog facts. Measure identity accuracy, field coverage, source disagreement handling, time, and cost. Manually check every field against the visible originating records.

### B — witness/composite crosswalk

Add the CDLI `Q006387` / `P464358` composite as a distinct scholarly layer. Test whether tooling preserves witness boundaries, lacunae, column/line coordinates, and provenance rather than filling the stela’s missing span invisibly. Manually inspect selected transitions before and after the lacuna.

### C — critical multi-witness route

Use Oelsner 2022 or another rights-cleared critical edition, with exact edition/version provenance, to compare individual witnesses, editorial reconstruction, transliteration, and translation. If access is not available for the required research use, prepare a written request to the publisher or rights holder before acquisition or publication.

For every arm, green schema or pipeline output proves only mechanics. Acceptance requires direct human reading of sampled source evidence, adversarial boundary cases, explicit error notes, and comparison of quality, cost, and speed. Translation, etymology, semantics, and comparison with recognized translations remain separate later branches.

## Remaining controls

1. Do not acquire or publish a source payload from this metadata planting.
2. Do not reconstruct the physical lacuna from the composite without an explicit editorial relation and source coordinates.
3. Before any content experiment, record exact edition, version, access time, rights layer, and witness/composite relationship.
4. Preserve the difference between ancient inscription sequence, modern section numbering, critical reconstruction, transliteration, translation, and interpretation.
5. Use the access-request channel only when a specific unavailable source is necessary; do not create performative requests when sufficient evidence already exists.
6. Do not promote semantics, signs, concepts, graph relations, or canon status from this artifact profile.

## Planting decision

Plant the physical Louvre `SB 8` stela as a public-metadata-only artifact witness for A02 backlog line 13. Keep the CDLI posting as discovery context, `P249253` as the exact current provider record, `Q006387` / `P464358` as a linked composite, and the scholarship sequence as explicit controls. This satisfies the requested concise but reliable profile without pretending that a modern web page, a composite text, or a green validator is the source itself.
