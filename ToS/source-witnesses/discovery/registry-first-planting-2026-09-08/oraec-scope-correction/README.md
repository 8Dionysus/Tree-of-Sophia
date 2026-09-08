# ORAEC observed scope correction

The two acquired raw JSON files exposed a narrower representation than the
broader ORAEC platform description used during preparation. They contain
Egyptian `written_form` transliteration, German sentence `translation` and
token `cotext_translation` glosses, and lexical/morphological annotations.
No hieroglyphic representation field was observed in these exact bytes.
The complete supplied files, including gaps and editorial signs, are retained.

`current.json` is the current scope statement, keyed by target slug in its
`targets` object. Consumers use `corrected_version_description` and `limits`
when describing these acquired versions. The frozen preparation remains
historical evidence and must not be silently restated as the observed content.

Observed field counts:

| Target | Egyptian written forms | German sentence translations | German token glosses |
| --- | ---: | ---: | ---: |
| immortality-of-writers / ORAEC 1016 | 323 | 43 | 319 |
| be-a-scribe / ORAEC 3091 | 145 | 21 | 142 |

Both source titles identify the expected sections of pChester Beatty IV /
British Museum EA 10684: verso 2.5–3.11 and 3.11–4.6. Their local byte digests
and complete field selectors are bound in the current statement and each
Item's `component-witnesses.json`. German translation and gloss selectors
belong to the German Expression; they do not become Egyptian source text.

The two Egyptian Expressions, two German Expressions, and two Editions have
version 2 descriptions with the current correction as evidence. Exact prior
acquired records and inspection reports are retained under `before/`, mapped
by original path and byte digest in `current.json`. Prior Item provenance bytes
and the inspector before its field-recognition repair are also retained there.
`topology-before/` retains the exact relation provenance preimage before the
owner's current input hashes were refreshed. Original preparation, license
snapshots, rights records, Item manifests and payload bytes were not changed.
The Item provenance streams each append a separate correction event; original
acquisition events remain intact.

`apply-correction.py` records the bounded execution. It refuses a second run
once `current.json` exists. This is execution evidence rather than a general
acquisition command or a replacement owner.

The source-visible review found and resolved two issues: omitted German gloss
selectors and an unsupported hieroglyphic layer assertion. Traceability,
version lineage, separation of languages, local storage and owner boundaries
remain explicit. The focused acquisition tests pass, and post-correction
inspection resolves every added selector and checks event input/output hashes.
No philological, translation, semantic, canon or publication acceptance follows.
