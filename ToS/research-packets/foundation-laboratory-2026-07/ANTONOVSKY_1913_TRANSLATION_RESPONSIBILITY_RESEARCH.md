# Antonovsky 1913 Translation-Responsibility Research

Date of evidence refresh: 2026-08-01
Scope: one exact Russian Expression and its visible translator statement
Authority posture: model-made research; no human bibliographic or textual review

## Question

Can the existing 1913 Russian *Also sprach Zarathustra* Expression gain one
source-returnable `translated_by` claim without treating a later reprint as the
same Expression, resolving the translator's full personal identity beyond the
displayed initials, accepting the translation text, or publishing the local
PDF?

## 1. Exact source and official records

The fixity-verified local research copy has SHA-256
`687716bc25ebf2281b967ebb0c6cf16b043c2d40bd16833d57d6dcf260d3476b`.
Direct visual inspection of PDF page 7 shows the work title, the statement
`ПЕРЕВОДЪ СЪ НѢМЕЦКАГО`, the displayed responsibility `Ю. М. Антоновскаго`,
the place `С. ПЕТЕРБУРГЪ`, and the year `1913`. Only the translator statement
is admitted in this slice. No OCR string is used as evidence, and no page image
or source text is copied into Git.

The current [Wikimedia Commons API record](https://commons.wikimedia.org/w/api.php?action=query&format=json&formatversion=2&prop=imageinfo&iiprop=url%7Csize%7Csha1%7Cmime%7Cmediatype%7Ctimestamp%7Cextmetadata&titles=File%3A%D0%9D%D0%B8%D1%86%D1%88%D0%B5_%D0%A2%D0%B0%D0%BA_%D0%B3%D0%BE%D0%B2%D0%BE%D1%80%D0%B8%D0%BB_%D0%97%D0%B0%D1%80%D0%B0%D1%82%D1%83%D1%81%D1%82%D1%80%D0%B0_1913.pdf)
was fetched at `2026-08-01T02:08:28-06:00`; response SHA-256
`be1f73c305fe4d26d02f33499d60aead15099b1b9e24cadf6b1025d0c80aa2b7`.
It still resolves the exact digital object whose provider SHA-1 and local
acquisition fixity were reconciled in the 2026-07-28 discovery run.

The official [National Electronic Library record for the 1911 fourth edition](https://rusneb.ru/catalog/000199_000009_003693383/)
explicitly credits the German translation to `Ю. М. Антоновского`. Its current
response SHA-256 is
`2c91f51d963526f179104c883b0e1ccdfb96c61eb2d2a79b298292386b9d6a6c`.
This corroborates the responsibility label but represents a different Edition
and does not prove textual identity with the 1913 Expression.

The official [Russian State Library record for the 1981 Chalidze reprint](https://search.rsl.ru/ru/record/01000380884)
also credits `Ю. М. Антоновского`; current response SHA-256
`bf28b2b7d4eff77631c653ba0b0783955b1583c88e76e76c326b875d8839d9a1`.
It is later manifestation evidence only, not an equivalence route.

## 2. Established editorial evidence

The [National Electronic Library record for the 2007 scholarly collected edition](https://rusneb.ru/catalog/000199_000009_005395580/)
records the work with translation by `Ю. М. Антоновского`; current response
SHA-256
`856b82ce8ede2dfa2fb82965fcec2105a03ad215242c96780fe5b67267537090`.
The exact local 2007 witness already independently carries the same displayed
credit. Editorial reuse across 1911, 1913, 1981, 1996, and 2007 supports a
responsibility route, but it must not collapse these Expressions or settle
revision history.

The existing 1913 forensic report had already inspected PDF pages 7, 13, 15,
and 402 and warned that embedded ABBYY text is not reviewed text. This new pass
returns to the actual page image rather than trusting that OCR layer.

## 3. Fresh/current check, with general web last

Current official-library records for later 2001, 2014, and 2019 manifestations
continue to display Antonovsky translation responsibility. They are freshness
corroboration only. General-web searches were run last for the exact 1913
publisher string and the translator's expanded name; they produced retail,
reuse, and unrelated-person results but no stronger authority identity for the
person and no contradiction to the exact page statement.

Therefore the local Agent remains `tos.agent.yuri-antonovsky` with preferred
label `Ю. М. Антоновский`, no external identifier, and explicitly unresolved
full-name/authority reconciliation. This slice does not promote `Юрий
Михайлович Антоновский` as accepted identity.

## Admission decision

Admit exactly:

- one proposed whole-page anchor for PDF page 7, bound to the Item and file
  digest;
- one model-made, `public_metadata_only`, `unreviewed` bibliographic claim from
  the exact 1913 Russian Expression via `translated_by` to the existing
  provisional Antonovsky Agent;
- one digest-bound annotation event.

Do not admit:

- source text, OCR, transcription, page image, translation quality, or an
  accepted translation;
- equivalence among the 1911, 1913, 1981, 1996, 2007, or later Expressions;
- an expanded personal name, authority identifier, biography, or rights claim;
- the place, publisher, edition number, or chronology as part of this
  responsibility claim;
- human review, semantics, sign/concept relations, publication, or canon.

The claim records what the exact page visibly credits. It does not prove that
every byte or editorial intervention in the 1913 volume was produced by one
translator, and it does not convert bibliographic responsibility into textual
acceptance.
