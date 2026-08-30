# A02 — Instructions of Šuruppag Open-Work Acquisition

## Result

The reviewed candidate `open-work-candidate.instructions-of-shuruppag` has a
positive exact-object route. Oxford Text Archive record `ota:2518` distributes
the revised Electronic Text Corpus of Sumerian Literature under CC BY-NC-SA
3.0 and contains the exact members:

- `etcsl/transliterations/c.5.6.1.xml` — the modern composite
  transliteration;
- `etcsl/translations/t.5.6.1.xml` — the modern English prose translation.

The official ZIP was downloaded over HTTPS after the Operator's explicit
acquisition directive. The container was used only to derive the two exact
members and was not retained. Each member has a separate content-addressed
File, representation record, rights record, and provenance edge beneath the
existing provider-independent scholarly composite.

## Identity and layer boundary

The existing ancient Work, the physical Abu Salabikh and Adab artifacts, the
provider-independent modern composite, the OTA container, the composite
transliteration File, and the English translation File remain distinct. The
licensed XML Files are evidence-bearing source witnesses; they do not become
an ancient original, an accepted text, a reviewed translation, a semantic
claim, a graph fact, or canon.

The live ETCSL pages continue to anchor composition title and editor
responsibility. CDLI Q000782 remains the freshest current composite and
witness-roster route. Neither live provider surface is treated as byte- or
license-equivalent to the OTA deposit.

## Rights result

The full OTA record states that the University of Oxford distributes the
corpus under Creative Commons Attribution-NonCommercial-ShareAlike 3.0
Unported. The license is positive for retaining and conditionally reusing the
two exact deposit members: attribution is required, commercial use is not
licensed, adaptations must use the same license, and moral rights remain
unaffected.

That result does not spread to:

- live ETCSL HTML by identity alone;
- CDLI score, ATF, JSON, images, or drawings;
- ISAC OIP 99, whose electronic-publication terms permit one personal copy but
  require distribution only from the ISAC site;
- the unlicensed Internet Archive user upload;
- Alster, Civil, or later publication bodies for which only metadata routes
  were established.

## Fixity

| Object | Bytes | SHA-256 |
| --- | ---: | --- |
| OTA `etcsl.zip` transport container, not retained | 4,910,212 | `d1a35b396399216deaeb483d5954ae603662e73c4e77f23e39f2e7b58466962b` |
| `c.5.6.1.xml` | 131,437 | `b412b8eb37035049b6b64ff3a06a6f2a09856af5c3b1793021fd3c32d56a701d` |
| `t.5.6.1.xml` | 24,878 | `546b7498864b4c798964a44f7eec69c98dc776766606bc248e49226a7a20bea8` |

## Discovery and timing

The run follows the repository discovery order and records the exact Library
of Congress, OTA, ETCSL, CDLI, ORACC, ISAC, identifier, open-library, and
general-web-last queries. Its timing receipt uses a monotonic clock for every
channel. All machine measurements are strictly positive; HTTP and TLS failures
remain outcomes rather than missing timing values. `human_minutes: 0` means no
real-human review was performed.

## Operational relations

The following relations are usable now without semantic promotion:

```text
tos.work.sumerian-literature.instructions-of-shuruppag
  <- existing source planting ->
tos.composite.sumerian.instructions-of-shuruppag
  <- represented by ->
tos.composite-representation.sumerian.instructions-of-shuruppag-ota-2518-c561
  -> tos.file.sha256.b412b8eb37035049b6b64ff3a06a6f2a09856af5c3b1793021fd3c32d56a701d

tos.composite.sumerian.instructions-of-shuruppag
  <- represented by ->
tos.composite-representation.sumerian.instructions-of-shuruppag-ota-2518-t561
  -> tos.file.sha256.546b7498864b4c798964a44f7eec69c98dc776766606bc248e49226a7a20bea8
```

The existing artifact observations remain unreviewed reported-witness
relations. No accepted source-text, translation-alignment, semantic, graph, or
canon relation is created in this iteration.

## Sources

- Oxford Text Archive record and files:
  <https://ota.bodleian.ox.ac.uk/repository/xmlui/handle/20.500.12024/2518>
- Full OTA metadata and rights:
  <https://ota.bodleian.ox.ac.uk/repository/xmlui/handle/20.500.12024/2518?show=full>
- Exact ETCSL composite: <https://etcsl.orinst.ox.ac.uk/section5/c561.htm>
- Exact ETCSL translation: <https://etcsl.orinst.ox.ac.uk/section5/tr561.htm>
- ETCSL credits: <https://etcsl.orinst.ox.ac.uk/credits.htm>
- CDLI score and terms:
  <https://cdli.earth/artifacts/composites-score/Q000782>,
  <https://cdli.earth/terms-of-use>
- ORACC license corroboration:
  <https://oracc.museum.upenn.edu/compass/downloads/2_4_Data_Acquisition_ETCSL.html>
- ISAC OIP 99 and electronic-publication terms:
  <https://isac.uchicago.edu/research/publications/oip/inscriptions-tell-abu-salabikh>,
  <https://isac.uchicago.edu/research/electronic-publications-initiative-institute-study-ancient-cultures>
- License: <https://creativecommons.org/licenses/by-nc-sa/3.0/>

## Authority boundary

This packet is a source-linked research summary. The source records own their
claims; the representation records own exact File identity; the rights records
own the bounded license posture; validators prove mechanics only; human review
owns text, translation, semantic, rights, and canon judgments.
