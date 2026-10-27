# Meno 70: two contextual word correspondences

Status: model-proposed, exact-layer-bound; bounded root source review completed for prototype use. These are two
local source-to-translation correspondences in the opening question of Plato's
*Meno*, not universal lexical translations, identity claims about concepts, or
accepted translation alignments. Their separate source and target references
must not be substituted across editions.

## Exact reading evidence

The selected Greek version is Burnet's Perseus `perseus-burnet-1903-grc`; its
text layer SHA-256 is
`318789bd9af1fdf6d162d06f302c894b1dcc6bad05d2d9a224cd530f564c2e95`. In the
Meno 70 unit `tos.text-unit.sid-f6f82156c79b42feb6d28fa2ace969f8`, the exact
opening reads:

> ΜΕΝ. ἔχεις μοι εἰπεῖν, ὦ Σώκρατες, ἆρα διδακτὸν ἡ ἀρετή; ἢ οὐ διδακτὸν ἀλλʼ ἀσκητόν; ἢ οὔτε ἀσκητὸν οὔτε μαθητόν, ἀλλὰ φύσει παραγίγνεται τοῖς ἀνθρώποις ἢ ἄλλῳ τινὶ τρόπῳ;

The selected English version is Lamb's Perseus `perseus-lamb-1924-en`; its
text layer SHA-256 is
`0c30e70af0c77aee04d8be6074ddf44c4c303ca70ee93cc65657329eb468d3c3`. In the
Meno 70 unit `tos.text-unit.sid-189f5af96a1249ec9d958c9bfa1fbf09`, the exact
opening reads:

> Men. Can you tell me, Socrates, whether virtue can be taught, or is acquired by practice, not teaching? Or if neither by practice nor by learning, whether it comes to mankind by nature or in some other way?

The selected spans, measured in Unicode code points from each frozen text
layer, are:

| Correspondence | Source span | Target span | Basis and limit |
| --- | --- | --- | --- |
| `ἀρετή` ↔ `virtue` | Greek `[50,55)` | Lamb `[40,46)` | Local nominal correspondence in the same question; this does not equate the Greek philosophical concept with every use of “virtue.” |
| `διδακτὸν` ↔ `can be taught` | Greek `[39,47)` | Lamb `[47,60)` | Local predicate correspondence in this question. Lamb renders the Greek adjective with an English verbal phrase; it is a phrase mapping, not a same-form claim. |

These spans occur in reversed word order across the sentence: Greek places
`διδακτὸν` before `ἀρετή`; Lamb places `virtue` before `can be taught`. The
packet records the exact relation and selector, without enabling synchronized
scrolling or transferring source identity to the target.

## Model assessment procedure

Assessment instruction (the digest recorded in each proposal): “Compare only
the exact Meno 70 source and Lamb target spans requested. Use their local
opening-question context; preserve the order difference; propose only the
context-bound correspondence supported by the supplied sentence. Do not infer
universal lexical translation, concept identity, or accepted alignment. Keep
the exact source and target revisions, code-point selectors, and quotations.”

The recorded digest covers the exact UTF-8 bytes of that assessment instruction
only. The hidden effective prompt and runtime context were unavailable. This is
a contextual proposal authored with the `gpt-6-luna` model alias from
source-visible text; the exact hosted model revision was not exposed and is
recorded as unavailable rather than guessed. The formal alignment packet has
`proposed` status and no human review record. Formal alignment admission remains pending; a
deterministic builder validates bytes and spans only and cannot accept either
claim.

Machine-readable source: `MENO-70-WORD-MAPPINGS.v1.json`, following
[`translation-alignment-packet-v1`](../../contracts/translation-alignment-packet-v1.schema.json).
The Greek and English selected-token spans are separately recorded in
`MENO-70-SOURCE-TOKENIZATION.v1.json` and
`MENO-70-TARGET-TOKENIZATION.v1.json`; each binding hashes exact complete file
bytes, including the final newline. The assessment event and its exact
instruction digest are in `MENO-70-WORD-MAPPINGS-PROVENANCE.v1.json`.
The public snapshot contains only a bounded projection of the two exact
proposals, with both durable references and exact quotes.


## Bounded root research-use review, 2026-10-26

Root personally read the exact Greek opening and Lamb sentence above. In this
question, nominative `ἀρετή` corresponds to Lamb's noun `virtue`; predicative
`διδακτὸν` corresponds to the whole phrase `can be taught`, rather than a
claim that the Greek adjective has the same grammatical form as an English
participle. The reversed order is visible in the two sentences. Both pairs
are supported for these exact occurrences and this local question. Repeated
occurrences elsewhere, other translations, and philosophical identity remain
outside the review.

This source-visible agent check supports displaying the proposals as a research
aid in the requested prototype. It does not insert a human review, accept a
legacy alignment, or change canon. The packet retains its proposed status and
empty legacy review fields. Runtime acceptance must additionally demonstrate
that each selected occurrence opens its own exact target and refuses stale or
unmapped occurrences. The Greek and English excerpts retain their source
attribution and CC BY-SA 4.0 terms through the exact side rights records and
[publication review](PUBLICATION-REVIEW.md).
