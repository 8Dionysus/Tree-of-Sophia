# Mysl Translator Identity Reconciliation Research

Date of evidence refresh: 2026-08-01
Scope: three translator Agents already named by the exact 1996 Mysl volume
Authority posture: model-made identity research; no human bibliographic or textual review

## Question

Can the existing translator Agents for the exact local *Works in Two Volumes*,
volume 2 (Mysl, 1996), be made more source-returnable without changing any
translation-responsibility claim, accepting any translation text, or collapsing
different Expressions and Editions?

The candidates are the displayed labels `К. А. Свасьян`, `Н. Полилов`, and
`В. А. Флёрова`. The required result is deliberately asymmetric: an identity
may be verified only when the initials, full name, authority record, and
Nietzsche context form a coherent chain. An unresolved label must remain
unresolved.

## 1. Official authority and national-library evidence

The DNB GND SRU authority service was queried first on 2026-08-01. No response
payload was retained, but exact response digests were recorded:

- GND `120452367`, response SHA-256
  `31377a0ef8486d5a844504f1fa595709a523a42cedaf19589e5bb54e04f1fa69`,
  identifies `Swassjan, Karen (1948-2024)`, includes the Cyrillic variant
  `Свасьян, К. А.`, classifies him as a translator, and describes him as a
  Russian translator and editor of works by Nietzsche and others. The MARC
  authority record was updated on 2026-06-05.
- GND `1012315509`, response SHA-256
  `ef8586009b528d0268f3a18c15f14eb4589037d5e0ca12a5a303f78a862e8833`,
  identifies `Polilov, N.` with Cyrillic variant `Полилов, Н.`, classifies the
  person as a translator, and names the translated work *Po tu storonu dobra i
  zla*. This is the same Nietzsche Work represented by the existing Mysl
  translation-responsibility claim, not merely a surname match.
- A bounded GND search for `Flerova`, response SHA-256
  `d5585ddc32493cb87be87fd837e0ab47a59c6678b67a0d017a68acf565b013c8`,
  returned nine records but no `В. А. Флёрова` translator identity. The nearby
  `Flërova, V. E.` record is an archaeology-related person with different
  initials and is rejected.

The current [Russian State Library Nietzsche record from 1993](https://search.rsl.ru/ru/record/01001677376)
bridges the displayed `К. А. Свасьян` to `Свасьян, Карен Араевич`; response
SHA-256
`105317d589e34822320668f7f82d0b688f1096a496a2c87c042024629c090e18`.
Together with the exact GND Cyrillic variant and Nietzsche/editor context, this
supports the preferred label `Карен Араевич Свасьян` and verified GND identity.

The current [RSL 1909 Kuno Fischer record](https://search.rsl.ru/ru/record/01003967636)
expands `Н. Н. Полилов` to `Полилов, Николай Николаевич` in an explicit
translator role; response SHA-256
`99a0c880e275bc2614afd3eaccfc6dba8783c2d0625d76ecb3bb53fbf5b39996`.
The corresponding [National Electronic Library record](https://rusneb.ru/catalog/000199_000009_003967636/)
preserves the same expansion; response SHA-256
`0561b417c9c958c53440d4246f8017a44b50e82a3241548171c023537852ce71`.
These records supply the full-name bridge; the exact Nietzsche-work bridge
comes independently from GND and the established edition below.

The current [RSL record for the 1907 *Antichristian*](https://search.rsl.ru/ru/record/01003742587)
explicitly credits `В. А. Флеровой` and MARC field 700 records only
`Флерова, В.А.`; response SHA-256
`3553ba7246a7d218a4fd643402bd2b77b29eda7da951b216bf94977817d756ff`.
Other RSL and NEL records for her Bergson translations likewise retain only
initials. They corroborate one historical translator label but do not disclose
a full personal identity.

## 2. Established editorial evidence

The 2012 Institute of Philosophy critical collected-edition volume,
[volume 5, *Beyond Good and Evil; On the Genealogy of Morality; The Case of Wagner*](https://www.nietzsche.ru/userfiles/pdf/genealogia.pdf),
explicitly credits `Н. Н. Полилов, К. А. Свасьян` and states that the familiar
translations were checked and scientifically edited. The current response
SHA-256 is
`1cde59a0171fa9e90ef42e5dfe995fd3c9b5c9cdbf4373693560c7bbb76dfdf7`.
This is established Nietzsche-specific editorial evidence for both initial
forms. It supports identity reconciliation only; it does not make the 2012 and
1996 Expressions equivalent or accept either translation.

The [Big Russian Encyclopedia bibliographic record](https://bigenc.ru/b/polnoe-sobranie-sochinenii-1afb25),
published online in 2024 and checked again now, independently describes the
same volume and credits `Н. Н. Полилов, К. А. Свасьян`; response SHA-256
`d274229406662238c6f6e0c1064d91a2ceca272505d069f0598e49a6a6e7b10f`.
It is corroboration, not the authority owner.

## 3. Fresh/current and general-web check last

Only after the authority and established-edition passes, current general-web
results were checked. The current [AST catalog](https://ast.ru/book/genealogiya-morali-kazus-vagner-865881/)
lists `Полилов Николай Николаевич`; response SHA-256
`7cace31cb35b64088f5eb510cece2ab61bb657732bce219d578e4abedcf2638b`.
It is useful freshness corroboration but is not substituted for GND, RSL, or
the critical edition.

General searches for an expanded `В. А. Флёрова` identity surfaced `Вера
Александровна Флёрова`, a geologist born in 1913. She cannot be the translator
credited in a 1907 book. This is retained as a negative collision and not as an
identity candidate. No current source supplied a stronger full-name route for
the translator.

The evidence hierarchy therefore changes two Agents and protects the third:

- `tos.agent.k-a-svasyan` becomes `Карен Араевич Свасьян`, verified through
  GND `120452367`, while retaining the exact displayed initials as a variant;
- `tos.agent.n-polilov` becomes `Николай Николаевич Полилов`, verified through
  GND `1012315509`, while retaining `Н. Полилов` and `Н. Н. Полилов` as
  source-bound variants;
- `tos.agent.v-a-flerova` remains provisional as `В. А. Флёрова`, with no
  external identifier and an explicit prohibition against expansion to the
  chronologically impossible geologist.

## Admission decision

Admit exactly:

- two source-returnable preferred-name expansions and two verified GND
  identifiers;
- source-bound initial variants for the two resolved Agents;
- one refreshed provisional record for `В. А. Флёрова` that preserves the
  negative result and the boundary of current knowledge;
- one ordered discovery run and one digest-bound provenance event.

Do not admit:

- any new or changed translation-responsibility claim;
- equivalence among the 1907, 1996, 2012, or current publishing Expressions;
- source text, transcription, OCR, translation quality, accepted translation,
  or semantic annotation;
- a full name or authority identifier for `В. А. Флёрова`;
- rights clearance, payload publication, human review, canon, or promotion.

The object and claim counts must remain unchanged. Only Agent identity labels,
their evidence, and the derived indexes may change.
