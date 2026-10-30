# Bonsai

A bonsai is the mini-Tree of one public stand. It keeps everything the stand
reads — navigation, chronology, places, lens settings — in the shapes of the
Tree of Sophia, so that the stand can grow and experiment on its own while
every finding stays ready to hand to the Tree. The Tree itself grows
independently; a bonsai records where it was grafted from and reports how far
the Tree has moved since.

| Bonsai | Stand | Holds |
| --- | --- | --- |
| [`antiquity/`](antiquity/bonsai.json) | `/antiquity/` | the collection and its fifteen branches, the first six branches, the alignment of the corpus works they cite and the places of other works, the Tree's object types of its own words, the chronology of lives, phases and writings, places, the graft map |
| [`zarathustra/`](zarathustra/bonsai.json) | `/zarathustra/` | the book's witnesses (German first editions, Common's English, Antonovsky's Russian of 1913) aligned paragraph by paragraph and sentence by sentence, the places of other works its map cites, the alignment of the corpus works it cites, the Tree's object types of its own words, the chronology of the book's four parts, of the texts its map rests on and of the thought of eternal recurrence, their places, the graft map; the map itself is grafted from the [constructor atlas](../access/web/constructor/README.md) |
| [`shared/`](shared/coastlines.json) | both | Natural Earth coastlines under the map of places |

The stands under [`access/web/src/public-research/`](../access/web/src/public-research/)
read their bonsai through one adapter, `bonsai.mjs`. They present what a bonsai
holds and never write to it.

The display catalogs under [`locales/`](locales/) provide Spanish, German and
Simplified Chinese forms alongside the modules' Russian and English wording.
[`lib/locales.mjs`](lib/locales.mjs) resolves these interface languages and the
stand's display messages. The public adapter applies them after constructing
the existing library identity. A translated label does not change a source's
wording, dating, coordinates, identity, review status or export; quoted evidence
and the text of an edition retain their own languages.

## What a bonsai holds

Each `bonsai/<stand>/bonsai.json` is a manifest (`tos.bonsai.v1`): the stand
it serves, its posture, its graft, its modules and its lenses.

- **Graft.** The Tree revision and the digests of the Tree contracts and
  registries the bonsai was shaped by. A changed contract or registry is drift
  to review, not a silent change.
- **Modules.** Each names its file, role and format. A module may be
  `borrowed` from another owner; the manifest then names the files it rests on.
- **Lenses.** Each lens of the stand says whether a star's position on it
  *measures* something (years, longitude and latitude, a distance by a named
  criterion) or only *groups* (by branch, by kind). The stand marks each lens
  accordingly. A proximity lens names its criteria here, so a stand can try
  another criterion by editing its bonsai.
- **Depth.** On the lenses that group, the third dimension reads the step of
  interpretation the stand already names: a source lies deepest, a reading
  before it, a comparison of readings before that, a personal record nearest.
  `depth` names the steps with their depth and words, and the pose the space
  rests in so that its depth shows; the planes of the steps appear, named,
  while the space turns. The lenses that measure keep the space flat, since a
  turned scale would no longer measure. Depth orders and groups; it measures
  nothing, and a stand can read it otherwise by editing its bonsai.

### Chronology (`tos.bonsai.chronology.v1`)

A chronology holds *situations*, as the Tree models what a dating is about: a
phase of a life (`biographical-phase`) or a process such as the writing of a
work (`historical-process`). Each key is the Tree identity the bonsai proposes
for the situation, and each carries exactly one dating — a value of the Tree's
[`historicalDate`](../ToS/contracts/historical-claim.schema.json):

| Kind | Used for | Example |
| --- | --- | --- |
| `date-assertion` | one dated year, month or day | Cicero, *On Duties*: `"-0043"` (44 BCE); the thought of eternal recurrence: `"1881-08"` |
| `interval-assertion` | a span with both bounds, or one bound only | *Zarathustra* I: `"1883-01"`–`"1883-02"`; Deleuze, *Nietzsche and Philosophy*: written by `"1962"` |
| `relative-order` | written *during* another dated situation | the *Meno*: during Plato's dialogues between the early and the middle; the *Letter to Menoeceus*: during Epicurus' life; the *Outlines*: during Sextus' years of activity |
| `unknown-date` | a date the sources leave open | — |

"Within the author's life" is therefore not a display fallback: it is a stated
dating, `relative-order` `during` the author's lifetime phase, whose own
interval comes from a named source; the stand shows it as such, and no firmer
than that interval. A source that dates a
person only by activity ("flourished in the 3rd century") gives a phase of
activity, not a lifetime; a corpus of many hands has no life at all. Years are
astronomical (1 BCE = 0, 2 BCE = −1), as the Tree requires of comparable
values; dates are taken as the sources give them, and the bonsai asserts no
calendar. Each
situation also says what its source states (`statement`, in Russian and
English), which sources it rests on, how firmly it stands (`footing`: on a
study, on a summary of scholarship, on a source citing none) and whether it is
`reported` by a source or `inferred` by the bonsai. `lives` and
`compositions` map an author, a work or a passage to its situation.

### Places (`tos.bonsai.places.v1`)

Places carry their label and gazetteer identities — Pleiades for the ancient
world, Wikidata otherwise — with the representative point the gazetteer
publishes (longitude, latitude in WGS84). The point is for display; the Tree has
no admitted geometry contract yet, so the export carries the gazetteer identity,
not the point. Each situation's entry says on what basis it stands: the source
states where the work was written (`composition`) or where an event took place
(`event`), or only where the author lived and worked in the dated span
(`activity`), from which the place is inferred. Where the sources leave the
place open the entry says so (`unknown`) with what they do say, names no place
and exports nothing. Every entry has its statement, sources and footing.

### The work's own text instead of a retelling

A stand never retells a work in its own words. What a work says, narrates or
has a speaker say is shown as an exact fragment of it; the stand's own words
remain only for what a quotation cannot give — a reading, a reconstructed
argument, a distinction, a link between two places, a question, a method of
reading, historical or editorial context. An authored form is therefore
`{ru, en, cite}`: its own words in both languages, the places it rests on, or
both; a form whose words only retold a place keeps the place alone. A citation
names a place in the **original**:

- in a work the bonsai keeps as witnesses: `{"w": "also-sprach-zarathustra", "c":
  "III.02", "u": [first unit, last unit], "s": start, "e": end}` — the start in
  the first unit and the end in the last are optional and count characters of
  the unit's exact text;
- in a work of the public corpus: `{"w": "apology", "u": [first, last], "s",
  "e"}` with the corpus unit ids of the original-language version;
- in a work outside both: `{"w": "ethica", "loc": "III.P6"}`, its canonical
  locator, whose text `fragments.json` holds.

The place in a translation is never chosen by hand: the stand finds it through
the alignment of the witnesses, so a new translation needs an alignment, not
another round of hand-picked paragraphs.

### Witnesses (`tos.bonsai.witnesses.v1`)

`zarathustra/witnesses.json` and `zarathustra/witnesses/<chapter>.json` keep
the book chapter by chapter in three witnesses: the German first editions
(the pivot, in the Tree's technical-markup units of the Deutsches Textarchiv
transcription), Common's English (the public corpus units) and Antonovsky's
Russian of 1913 (the Wikisource transcription, with the bonsai's own unit
IDs). Each unit is `[id, kind, text, section]` exactly as its source gives it —
a correction belongs to the source, never to the module; the German units add
the public corpus units they map onto, so a place opens in the Tree's reader.
`rows` join the units of the three witnesses that translate one another, and
each row holds the sentence pairs inside it. The manifest records every
witness's edition, sources (URLs, digests, revisions), licence, attribution and
rights basis; the module is shared under CC BY-SA 4.0, like the DTA text and
the Wikisource transcription it holds. The censored edition of 1913 lacks three
chapters and the second half of *The Awakening*; the rows show those places as
left out.

### Alignment (`lib/align.mjs`)

The alignment is made by the bonsai's own mechanism and only by it.
Paragraphs are joined by length (Gale and Church) with small costs for what a
translation keeps — questions, exclamations, an opening mark, a name the work
repeats, verse — and then by words: an IBM Model 1 trained on the confident
one-to-one pairs of the first pass charges each candidate group for the words
it cannot account for (Moore). Every group keeps its shape (1:1, 1:2, 2:1, 2:2,
1:3, 3:1, up to 1:6 for verse, a unit without counterpart); a run of left-out
units opens once and extends more cheaply, so a long omission reads as one gap
rather than as false matches. Inside each group sentences are aligned the same
way. Greek and Latin end their sentences on their own marks: Greek on the full
stop and on its question mark ";", not on the raised dot "·", which joins
clauses inside a sentence and which a translation renders as a colon or a
semicolon. A third witness
checks the two pairings against each other: a pivot unit agrees when its
English and Russian counterparts meet in the direct English–Russian alignment.
The manifest records the receipt — group shapes, sentence pairs, the triangle
and the groups whose words the model cannot account for, to review first.

```sh
node bonsai/lib/cli.mjs align zarathustra          # align again, report what differs
node bonsai/lib/cli.mjs align zarathustra --write  # store the new rows and receipt
```

The stands' tests align the stored texts again and require the stored rows,
so a change to the mechanism is seen before it is stored.

### Places of other works (`tos.bonsai.fragments.v1`) and corpus alignment (`tos.bonsai.corpus-alignment.v1`)

`fragments.json` holds the exact text of each cited place of a work that is
neither a witness nor in the public corpus, in the original and in
translations, each witness with its edition, the URL its text was taken from,
licence, attribution and rights basis: public domain, public domain with a
licence of the digital copy, a short quotation of a protected work under the
quotation right, or the locator alone where no exact text can be shown
lawfully. The rights record names the law of each jurisdiction (`russia`,
`usa`, `eu`) and the reasoning. A place may link its own page in a witness
(`links`, by language) or a page where a work cited by locator alone can be
found (`url`); a work names the languages it has no witness in and why
(`absent`), and the module keeps how the texts were gathered and corrected
(`provenance`).

`alignment.json` gives, for each cited unit of a corpus original, its
counterparts in the corpus translations and the sentence pairs inside them:
identifiers and offsets only, since the texts stay in the corpus. It is made
by `lib/corpus-align.mjs` from the units the stand's content cites (its own
modules and the modules it borrows). A translation divided like its original
pairs by the shared reference, one divided more finely by the sections inside
the original's section, one divided more coarsely by the book that holds the
original's sections (they are aligned together against it), one that is a
single long text by aligning the sentences of the whole work. A unit that names no place of the work — an
edition's numbered chunk of a long chapter ("Unit 59") — is aligned with the
unit before it, on whichever side it stands, so a chapter one edition cuts
into pieces still meets its whole counterpart. Inside each run the sentences
are paired by `align.mjs` in two passes, by length and then by the words a
model trained on the confident pairs can account for. A Greek or Latin period
may meet up to six sentences of a modern translation, as a verse line may. An edition's notes, which the corpus
keeps at the end of a unit with their numbers in 〔 〕, are not its text and
take no part in the pairing. In each language a version whose notes run into
its sentences ("artsA less well attested reading…") comes after one whose notes
stand apart, so a stand shows the cleaner text first. The receipt names, for
each version, how it was paired, the sentences on both sides, the group shapes,
the training pairs and the share of words the model accounts for — the first
place to look when a pairing seems wrong. Where a translator reorders or
merges sentences of the original, a monotone alignment cannot follow, and the
counterpart shown may be the neighbouring sentence; the parallel reading of
the whole place shows the context.

```sh
node bonsai/lib/cli.mjs align-corpus antiquity          # align again, report whether the module matches
node bonsai/lib/cli.mjs align-corpus antiquity --write  # store the new module
```

### Objects of thought (`tos.bonsai.objects.v1`)

`zarathustra/objects.json` says what the stand's own words are in the Tree's
[entity registry](../ToS/doctrine/semantic-interchange/entity-types.v1.json):
a tension is a problem, articulated by its question, and the perspectives on it
are the positions that address it; a concept's perspectives are its aspects; a
reading is a conception, of the concept it interprets; a symbol or a character
is an image of thought that presents the readings it supports; an inquiry has
its argument and, where it has one, the objection to it; the foundation has its
distinctions and its method; every question of a material, a relation, a route
or a lens is a question. Each object names the form that holds its words —
never a copy of them — and the places of text it rests on. Relations come only
from the Tree's [relation registry](../ToS/doctrine/semantic-interchange/relation-types.v1.json),
each within its domain and range, and only where the stand's own structure
gives them, each with its basis in the stand: an edge that compares, questions
or develops does not say which relation of the registry it means, and stays
without one. The links between places of texts that the stand's words name —
one place repeats, varies, develops, contrasts with or echoes another — have no
relation in the registry yet and are kept as the stand's proposal with both
places; the stand shows both places beside the words that link them. Theses,
premises and the steps of an inference are not yet set apart in the Zarathustra
stand.

`antiquity/objects.json` does the same for the Antiquity collection, whose
arguments already name the role of each step: a step is a thesis (a premise,
claim, condition, definition, consequence, explanation, qualification, reply or
conclusion — premise and conclusion being roles of a thesis within a step, as
the Tree has it), an objection, a distinction, a problem or a question; where
premises support a conclusion the argument has an inference step with those
premises and that conclusion, and the argument is for its conclusions; an
objection that tests, answers or qualifies a thesis objects to it. A question
of the collection is a problem articulated by its question, with the positions
that address it; a term is a lexeme and each of its uses an occurrence of it.
The first branches repeat the collection's branches and are typed there.
Nothing here is admitted to the Tree.

### Graft map (`tos.bonsai.identities.v1`)

Modules use the bonsai's own IDs for works, agents and places. `identities.json`
maps each to the Tree identity it stands for, `resolved` where the Tree already
holds it and `to-mint` where the Tree's own route would have to create it.
When the Tree renames or mints differently, only this map changes.

## Check and export

```sh
node bonsai/lib/cli.mjs check
node bonsai/lib/cli.mjs export antiquity --out antiquity.export.json
```

`check` validates each module and checks the bonsai's export against the Tree
contracts in the current checkout (`ToS/contracts/`) and against the Tree's
relation registry (`ToS/doctrine/semantic-interchange/`): each claim's
predicate must take the claim's schema and assertion layer, and the kinds of
its subject and object must fall within the predicate's domain and range. It
then reports drift since the graft: changed contracts and registries, and —
where `ToS/source-witnesses/` is checked out — identities the bonsai calls
resolved that the Tree no longer holds, or proposes to mint that the Tree now
has. It fails only on a broken module, an export the Tree refuses, or a
relation registry it cannot read.

`export` writes a packet (`tos.bonsai.export.v1`) and never writes into
`ToS/`: provisional identity records for historical processes, events and
places, datings as `tos_source_temporal_claim_v1`, the work of a process as
`tos_source_relation_claim_v1`, the place of a situation and the person of a
life phase as `tos_historical_context_claim_v1` with their basis, scope and
time scope, and the list of every identity they refer to with its graft
status; phases, agents and works to mint are listed there, not recorded. Every
claim is `unreviewed` and `public_metadata_only`. Taking a candidate into the Tree is the Tree's own
[declared source-claim creation](../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#declared-source-claim-creation),
with its delegation, evidence and review.

The check also reads the witnesses (every chapter listed, each unit in
exactly one row, every sentence span inside its unit, the rights of each
witness), the places of other works (a rights basis for every witness, no text
for a work cited by locator only, short quotations short), the corpus
alignment (known works and well-formed pairs) and the objects of thought (every
type and relation in the Tree's registries, every relation within its domain and
range, every place an object rests on or a link joins found in the witnesses or
among the places of other works).

The library under [`lib/`](lib/) is plain JavaScript without dependencies:
`time.mjs` reads and checks the Tree's date grammar and words a dating for the
reader, `chronology.mjs` resolves situations to spans, `places.mjs` reads and
checks places, `schema.mjs` validates against the Tree's JSON Schemas,
`export.mjs` builds the packet and `graft.mjs` checks a bonsai on disk;
`align.mjs` aligns witnesses, `realign.mjs` aligns a stored witnesses module
again, `witnesses.mjs` reads it and finds a cited place in every witness,
`corpus-align.mjs` aligns the corpus works a stand cites, `fragments.mjs` checks
the places of other works and the corpus alignment, and `objects.mjs` checks the
objects of thought against the Tree's registries. The
stands' tests in `access/web` run the same check on every bonsai, so the
browser lane fails when a module or its export no longer fits the Tree.

## Growing a bonsai

- Never retell a work: cite the place in the original and let the stand show
  it. Write own words only for what the place cannot say by itself.
- Never edit a witness's text or pick a translation's place by hand. Correct
  a text at its source and bring it in again; improve the alignment through
  the mechanism, align again and review what changed.
- A new translation of a witnessed work needs only its units and the
  alignment: add the witness, align, check the triangle and the groups to
  review.
- Bring in a translation or a place of another work only with its rights
  basis recorded: never a text the law forbids, and never leave out a text it
  allows.

- Add or correct a dating or place in its module with its source, footing and
  statement; never store a fallback the display would invent.
- Keep a module's material IDs when it moves or changes; saved personal graphs
  of the stand depend on them.
- When the Tree gains an identity the bonsai proposed, mark it `resolved` in
  the graft map; when a Tree contract changes, run the check and record the
  new digest in the manifest once the bonsai fits it.
- A new stand gets its own `bonsai/<stand>/` with a manifest; its stand reads it
  through `bonsai.mjs`.
