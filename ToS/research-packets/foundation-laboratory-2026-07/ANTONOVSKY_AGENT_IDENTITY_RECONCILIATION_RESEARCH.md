# Antonovsky Agent Identity Reconciliation Research

Date of evidence refresh: 2026-08-01
Scope: the one stable Agent already targeted by three Antonovsky responsibility claims
Authority posture: model-made identity reconciliation; no human bibliographic or textual review

## Question

Can `Ю. М. Антоновский` be resolved to a full personal identity and stable
external authority without changing the exact source credits, rewriting the
three existing responsibility claims, equating Russian Expressions, accepting
translation text, or turning a misleading legacy slug into a name assertion?

## Ordered method

The pass followed the repository discovery order:

1. exact source-visible initials and official authority/catalog records;
2. established historical reference and peer-reviewed Nietzsche scholarship;
3. fresh current publisher corroboration;
4. general web search last, used only to locate or challenge stronger records.

The exact 1913 title-page claim was not reopened. Its page-7 anchor and
responsibility packet already prove only the displayed credit
`Ю. М. Антоновскаго`. This pass concerns the identity of the Agent at the
object end of that unchanged claim.

## 1. Official authority and national-library records

### DNB GND

The current DNB SRU authority query for `Antonovskij` returns GND
[`123235553`](https://d-nb.info/gnd/123235553). The response was fetched on
2026-08-01 and had SHA-256
`7514b1c4e3660e0d1c6d17320e55727f1139552e0cd09991e5ef6fd9c7250cf6`.
The exact authority record contains:

- preferred form `Antonovskij, Julij M.`;
- variants `Antonovskij, Ju.`, `Antonovskij, Ju. M.`, and
  `Antonovskij, Julij Michajlovič`;
- original-script form `Антоновский, Ю. М.`;
- profession `Übersetzer`;
- publication context `Übers. von: Tak govoril Zaratustra. - 1996`.

The DNB JSON-LD representation had SHA-256
`5e6e645a7cc55a97a62067bb2be824223c33b518ed866f3db1399a583b37c1f3`,
declares CC0 for the authority metadata, and reports the authority modified at
`2025-09-24T15:27:55.000`. The record's `1996` activity field is not treated as
a life date or the beginning of the translation lineage.

### Russian State Library

The current [RSL Zarathustra record](https://search.rsl.ru/ru/record/01004322945)
uses the displayed responsibility `Ю. М. Антоновского` and expands MARC 700 to
`Антоновский, Юлий Михайлович, 1857-1913`, role `пер.`. The current HTML
response had SHA-256
`b2b7b4d17dcbc66bfa714b807cc66a8e36358ae1a91250f884acc8fccf628d94`.
This is the needed Cyrillic full-name bridge in the same Nietzsche-work
context. It is later-edition metadata and does not establish textual identity
with the 1913 Expression.

The [National Electronic Library record for the 1911 *Ecce homo*](https://rusneb.ru/catalog/000199_000009_003782420/)
independently expands `Ю.М. Антоновского` to `Антоновский, Юлий Михайлович,
1857-1913` and identifies editorial and preface responsibility. It supports
the person identity across another Nietzsche responsibility but does not make
editing, preface authorship, and translation interchangeable roles.

Together, GND and RSL close the chain:

```text
exact source credit Ю. М. Антоновский
  -> GND original-script initials + Julij Michajlovič + translator + Zarathustra
  -> RSL same initials + Юлий Михайлович + 1857-1913 + translator role
```

## 2. Established reference and scholarship

The 1911 *New Encyclopedic Dictionary* entry, available with its source
reference through [Wikisource](https://ru.wikisource.org/wiki/НЭС/Антоновский,_Юлий_Михайлович),
names `Антоновский, Юлий Михайлович`, calls him a writer and translator, gives
his birth year as 1857, and specifically reports that his *Так говорил
Заратустра* translation reached a fourth edition in 1911. The current page
response had SHA-256
`34f63a39591edfb988162b5ca52d320bd5853c710745a1c1196ef7e0adb2a3e8`.
This is contemporary reference evidence rather than a fresh biographical
retelling.

The peer-reviewed 2020 Institute of Philosophy RAS publication
[*S. L. Frank in the F. Nietzsche archives: A 1932 lecture in Weimar*](https://pj.iphras.ru/issue/download/307/144)
identifies `Юлий Михайлович Антоновский (1857-1913)` and names his translations
of *Так говорил Заратустра*, *Происхождение трагедии*, and *Ecce homo*. The
served journal PDF had SHA-256
`340a0f02e483e6b2158273529bca58bc5198fbfcabf735a7e4c63de4d135b6dc`.
This is established scholarly corroboration, not the external-identifier
authority.

## 3. Fresh/current check, with general web last

General web search was run only after the authority and established routes.
Current publisher and reuse pages consistently expose `Юлий Михайлович
Антоновский` as the translator. In particular, the current
[AST *Так говорил Заратустра* record](https://ast.ru/book/tak-govoril-zaratustra-882344/)
does so directly; the response fetched on 2026-08-01 had SHA-256
`08d3b66d7f44adb6deb0ab272012adaa8fda188b7124c82b8ed87e94bebfe7d7`.
This is freshness corroboration only. Retail/catalog reuse does not outrank
GND, RSL, the historical reference, or scholarship and cannot establish
edition or textual equivalence.

The search also exposed the repository's earlier English expansion `Yuri M.
Antonovsky` as an error. The full authority chain supports `Julij` / `Юлий`,
not `Yuri` / `Юрий`. No competing `Юрий Михайлович Антоновский` translator
authority was found.

## Stable-ID and speaking-path decision

The existing ToS ref `tos.agent.yuri-antonovsky` is already the object of three
source-owned claims. Stable ToS IDs protect lineage and do not silently change
when labels improve. Replacing this ID would rewrite claim objects and digests
without changing the person they target.

The filesystem path is human navigation and may improve through an explicit
migration. Therefore:

- retain `tos.agent.yuri-antonovsky` as a legacy stable ref only;
- move the Agent record from `agents/yuri-antonovsky/` to the accurate speaking
  route `agents/yuliy-antonovsky/`;
- set the preferred label to `Юлий Михайлович Антоновский`;
- retain `Ю. М. Антоновский` and DNB transliterations as verified variants;
- mark `Yuri M. Antonovsky` rejected;
- add GND `123235553` as the external authority;
- explain in the record that `yuri` is a legacy slug, not a forename claim.

This is a path and label correction around one stable identity, not creation
of a second Agent, same-as assertion, or silent history rewrite.

## Admission decision

Admit exactly:

- the full preferred label `Юлий Михайлович Антоновский`;
- GND `123235553` with the RSL full-name bridge;
- verified initial and transliterated variants;
- rejection of the prior `Yuri M. Antonovsky` expansion;
- an explicit speaking-path migration with the stable ToS ref retained;
- one ordered discovery record and one identity-reconciliation provenance
  event.

Do not admit:

- a new Agent or replacement claim ID;
- changes to the three existing `translated_by` claim objects or claim counts;
- equivalence among the 1911, 1913, 1996, 2007, or later Expressions;
- source text, OCR, accepted translation, translation quality, semantic claims,
  signs, concepts, human review, publication rights, site payloads, or canon;
- DNB's 1996 activity value as a biographical date;
- current publisher metadata as authority over the official and established
  chain.

The earlier `ANTONOVSKY_1913_TRANSLATION_RESPONSIBILITY_RESEARCH.md` remains
the evidence packet for the exact page-7 responsibility claim. Its cautious
identity conclusion is superseded only by this later independent Agent pass;
the original file is left unchanged so its recorded digest and claim
provenance remain honest.
