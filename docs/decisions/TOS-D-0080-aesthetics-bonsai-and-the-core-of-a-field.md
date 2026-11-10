# TOS-D-0080 The Aesthetics Bonsai and the Core of a Field

## Index Metadata

- Decision ID: TOS-D-0080
- Original date: 2026-11-09
- Surface classes: bonsai, access/web
- ToS layers: derived projection
- Tree classes: none
- Guard families: source-first authority, review separation, bounded downstream seams
- Posture: accepted by explicit operator instruction

## Context

The third bonsai is Aesthetics. Zarathustra is one work read in depth;
Antiquity is one epoch kept as a library by questions. Aesthetics is a
discipline cut across twenty-five centuries. Its material was prepared outside
the repository as research registers: thirty-six areas in four groups, each
with its question, and for the first batch 416 concepts, 810 readings of places
in 552 works and 462 links, made by four separate passes with their own
columns, status words and local IDs. A reading is a paraphrase with a locator.
A media pass over the registers found errors in them, among them four
co-authors of one article with invented given names.

The Tree's registry already names what such a discipline consists of: a
crosscutting concept, the situated conception that belongs to it, and the
relations `conception-of`, `conception-attributed-to` and
`conception-expressed-in`. The core of a work (`tos.bonsai.core.v1`) gives one
conception to each concept, the work's; its check is bound to one work's
witnesses and voices.

## Decision

The Aesthetics bonsai is planted at `bonsai/aesthetics/` with the core of a
field as its main module, `field.json` (`tos.bonsai.field.v1`), described in
`bonsai/CORE.md`, "A core across works". An area is a region of navigation
with its question; a concept is a subject any author can address; a conception
is one work's answer at a cited place of the original, in the voice that gives
it, with one thesis of ours beside the work's own words; a link joins concepts
or conceptions by a verb and rests on conceptions the field holds.

A research register proposes and a reader decides. The registers are kept
whole under `bonsai/review/aesthetics/`: every row as its workbook holds it,
and one layer in one shape with the same author and work joined and the media
pass's corrections applied and recorded. A reading enters the bonsai only
after a reader has opened its place in the work, cited the formula over whole
sentences and given a verdict; the stand's admission rests on that verdict
alone. Every record of the field names the research records it grew from.

All thirty-six areas and all concepts stand in the field at once, as the
skeleton. Depth grows area by area, beginning with the Beautiful and the
Sublime, whose readings all rest on works written before 1930. Links a
register marks as its compiler's comparison stay in the layer and are grown
with the operator. The bonsai is planted before its stand opens: the back
first, then the stand.

## Options Considered

Shaping Aesthetics as a collection like Antiquity would give it branches of
passages joined place to place. The registers' links join concepts, and one
concept gathers many works, which a branch of passages does not hold.

Extending the core of a work to many works would bind every check of voices,
chapters and occurrences to cases they were not made for. A sibling module
keeps both checks exact and shares the principles of a node.

Planting the registers' readings at once, as paraphrases to be replaced later,
would put retellings on a stand and unread claims into a bonsai.

## Rationale

The question belongs to the Tree and the answer to the work. A field is the
place where that principle carries the structure: conceptions of different
works meet on one question. Keeping the register's paraphrase out of the
bonsai and its key inside every record keeps both the rule that a stand shows
a work's own text and the path back to where a reading came from.

## Consequences

The first planting holds seven conceptions read in the Greek of the public
corpus — Plato's Symposium, Plotinus's Enneads I.6 and Aristotle's Poetics 7 —
and three links resting on them; they grew from nine of the registers'
readings and four of their links. Works outside the corpus enter with their
places and rights through the bonsai's places of other works. A chronology,
places, media and the stand follow; until the bonsai keeps a chronology it
exports nothing.

## Source Surfaces

- `bonsai/lib/field.mjs`, `bonsai/lib/graft.mjs`
- `bonsai/aesthetics/`
- `bonsai/review/aesthetics/`
- `bonsai/CORE.md`, `bonsai/README.md`
- `access/web/src/public-research/bonsai.mjs`

## Validation

`bonsai-field.test.mjs` covers the field's contract against the Tree's
registries, the refusals, the reading of every formula in the original where
the corpus is fetched, and the equality of the stored modules with what the
planting tool gives. `check_layer.py` reads the layer against the register
rows. `node bonsai/lib/cli.mjs check aesthetics` passes with the corpus
fetched.
