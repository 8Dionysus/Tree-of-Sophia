# Aesthetics: the research batches and their layer

The Aesthetics bonsai grows from research registers made outside the repository.
This folder keeps the first batch as records, the tool that brings its four
registers into one shape, and the hand-made decisions that tool reads. The
bonsai itself is [`bonsai/aesthetics/`](../../aesthetics/bonsai.json).

## The first batch

Four registers of 6–7 November 2026, one for each group of areas, and a media
pass over each. Each register was made by a separate research pass with its own
columns, status words and local IDs.

| Register | Areas | Concepts | Readings | Links | Works | Authors | Sources |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A | 1–12, the categories | 211 | 178 | 185 | 209 | 190 | 92 |
| B | 13–20 | 63 | 229 | 90 | 139 | 199 | 143 |
| C | 21–28 | 74 | 179 | 124 | 79 | 76 | 87 |
| D | 29–36 | 68 | 224 | 63 | 125 | 125 | 125 |

A reading is a register's account of one place of a work: the work, the place
by its locator, the thesis in the register's words and what to keep apart from
it. A register's words are a paraphrase, never the work's text.

## What is kept

- [`registry/`](registry/) holds every row of every content sheet as it stands
  in its workbook, one JSON object per line under the sheet's own column names;
  the first line names the sheet and its columns. Cover sheets hold counts only
  and are left out.
- [`layer/`](layer/) holds the same content in one shape. It is what the bonsai
  is grown from and what an independent reading of a place starts from.
- [`inputs/`](inputs/) holds the decisions made by hand: the 36 areas with their
  groups and questions as the assignment states them and their English titles
  and questions, the English title of every concept, the author records that
  are one person under two spellings, the work records that are one work under
  two titles, and the names and IDs of authors and works whose registers give
  none in Latin script.
- [`readings/`](readings/) holds the readers' records: one file for each pass
  in which a reader opened places in the works themselves. A record names its
  reader, the works it read with their Tree identities, and for each place the
  conception it bears out: the concepts, the locator, the words that open and
  close the formula, the thesis, the verdict and the registers' readings it
  grew from.
- [`provenance.json`](provenance.json) names the workbooks read, with their
  sizes and SHA-256. The workbooks are not kept here.

## The layer

| File | Holds |
| --- | --- |
| `areas.json` | the 36 areas: group A–D, title, question; scope and boundary where register A states them |
| `concepts.json` | 407 concepts: the registers' 416 records, nine of them the same concept named alike in two registers; each register's own account of the concept stays under `said` |
| `agents.json` | 501 authors from 590 records |
| `works.json` | 499 works from 552 records |
| `readings.json` | the 810 readings |
| `links.json` | the 462 links between concepts (432) and between readings (30) |
| `sources.json` | the 447 points of access the registers read from |
| `distinctions.json`, `materials.json`, `editions.json` | what register A alone keeps |
| `historical-nodes.json` | what register C alone keeps |
| `corrections.json` | what the media pass found wrong in the registers, and whether the layer applies it |
| `keys.json` | each register ID of an author, a work or a concept, and the layer record it belongs to |

Keys stay unique across registers: a register's own ID after the letter of its
register (`A:E021`, `C:A-I001`). The registers reuse IDs: 132 reading IDs name
different readings in different registers.

An author's ID is a slug of the name in Latin script, a work's ID the author's
ID and a slug of the original title (`immanuel-kant.kritik-der-urteilskraft`),
in the Tree's habit for German (`ue`, `oe`, `ae`).

**Joining.** Author records are one person when their original or Russian name
agrees. Work records of different registers are one work when the first author
agrees and so does a form of the original title, or the Russian title and the
year. Two records of one register stay two works unless the register joins them
itself. Concepts are joined only when their names agree exactly; `inputs/`
holds the rest. *Kritik der Urteilskraft* stands in all four registers and
carries 38 readings.

**Basis of a reading** (`basis`): `primary` — the register read the chosen
place in the work or a translation of it (683); `secondary` — in a study (96);
`abstract` (19); `bibliography` (7); `hypothesis` (5). The basis says where the
register read, not whether the reading holds.

**Grounding of a link** (`grounding`): `work` — the register says the relation
stands in the named work (267); `compiler` — the compiler's own comparison
(186); `study` — on a study of the work (8); `route` — a route of the register
(1). `mode` keeps the register's own word.

**Corrections.** The media pass opened sources again and found errors in the
registers. The layer applies the 14 corrections of register B that the pass
confirmed against the publication: ten author names (among them four co-authors
of one article with invented given names, and one article attributed to another
author), one title, and the attribution inside one thesis and two links. The
register's own value stays beside the corrected one (`register_names`,
`register_title`, `register_thesis`, `register_locus`). Ten remarks on register
A are attached to their records as `remarks` and change nothing. Three records
of register C were rewritten in the media workbook; the layer takes the later
reading. For register B the pass also recorded, reading by reading, whether it
reached the digital place of the text (`place_check`): 122 of 229 reached.

## Planting

```sh
node bonsai/lib/layers.mjs fetch access/public-corpus     # the corpus texts the formulas are found in
node bonsai/review/aesthetics/plant.mjs                   # write bonsai/aesthetics/{field,library,identities}.json
node bonsai/lib/cli.mjs align-corpus aesthetics --write   # the cited places in the corpus translations
node bonsai/lib/cli.mjs check aesthetics
```

`plant.mjs` writes the areas and concepts from the layer and `inputs/`, and a
conception from a record under `readings/` alone. It finds each formula in its
corpus unit by the words that open and close it and refuses a span that opens
or closes inside a sentence. `--check` says whether the stored modules are
what the records give.

A reader's pass over places: read the registers' readings of a place in
`layer/readings.json`, open the place in the work, decide what the place
itself says, and write the record. Readings of one place from different
registers meet in one conception. The verdict is `confirmed` where the
register's reading stands as it is, `revise` where the place bears out a
different placing, verb or concept, with a note of what changed.

## Rebuilding the layer

```sh
pip install openpyxl
python3 bonsai/review/aesthetics/unify.py --batch "<folder of the first batch>"
python3 bonsai/review/aesthetics/check_layer.py
```

The folder holds the four `Estetika_*_Reestr.xlsx` workbooks and `media/` with
the four `m_*.xlsx`. The tool stops on a sheet it does not know and on an
author or a work it cannot name. The check reads the layer against the
register rows: every row is in the layer once and every reference resolves.

## What the layer does not claim

Nothing here is admitted to the bonsai or to the Tree. A reading enters
`bonsai/aesthetics/` only after its place has been read independently in the
work itself and stands as a citation of that place; the registers' paraphrases
then remain here as the record of where the reading came from. The media
workbooks' own content — works of art, reproductions, bindings — is not in the
layer; it is read when the bonsai's media are planted.
