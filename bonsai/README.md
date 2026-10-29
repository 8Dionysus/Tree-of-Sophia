# Bonsai

A bonsai is the mini-Tree of one public stand. It keeps everything the stand
reads — navigation, chronology, places, lens settings — in the shapes of the
Tree of Sophia, so that the stand can grow and experiment on its own while
every finding stays ready to hand to the Tree. The Tree itself grows
independently; a bonsai records where it was grafted from and reports how far
the Tree has moved since.

| Bonsai | Stand | Holds |
| --- | --- | --- |
| [`antiquity/`](antiquity/bonsai.json) | `/antiquity/` | the collection and its fifteen branches, the first six branches, the chronology of lives, phases and writings, places, the graft map |
| [`zarathustra/`](zarathustra/bonsai.json) | `/zarathustra/` | the chronology of the book's four parts, of the texts its map rests on and of the thought of eternal recurrence, their places, the graft map; the map itself is grafted from the [constructor atlas](../access/web/constructor/README.md) |
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

The library under [`lib/`](lib/) is plain JavaScript without dependencies:
`time.mjs` reads and checks the Tree's date grammar and words a dating for the
reader, `chronology.mjs` resolves situations to spans, `places.mjs` reads and
checks places, `schema.mjs` validates against the Tree's JSON Schemas,
`export.mjs` builds the packet and `graft.mjs` checks a bonsai on disk. The
stands' tests in `access/web` run the same check on every bonsai, so the
browser lane fails when a module or its export no longer fits the Tree.

## Growing a bonsai

- Add or correct a dating or place in its module with its source, footing and
  statement; never store a fallback the display would invent.
- Keep a module's material IDs when it moves or changes; saved personal graphs
  of the stand depend on them.
- When the Tree gains an identity the bonsai proposed, mark it `resolved` in
  the graft map; when a Tree contract changes, run the check and record the
  new digest in the manifest once the bonsai fits it.
- A new stand gets its own `bonsai/<stand>/` with a manifest; its stand reads it
  through `bonsai.mjs`.
