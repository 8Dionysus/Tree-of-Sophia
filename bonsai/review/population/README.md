# Population: the persons registers and their resolution

The Population bonsai grows from the Tree's research registers of persons and
works, made outside the repository batch by batch: antiquity (`A01`–`A48`), the
middle ages (`T2-01`–`T2-58`) and modern times (`T3-01`–`T3-85`). Each batch has
an Identity workbook whose `agents` sheet lists the people, pseudonymous authors,
traditional identities and collectives its research met. This folder keeps those
rows, the tools that bring them into one shape and resolve them into members, and
the decisions of the readers. The bonsai itself is
[`bonsai/population/`](../../population/bonsai.json).

## What is kept

| Path | Holds | Where |
| --- | --- | --- |
| `registry/<batch>/agents.jsonl` | every row of the batch's `agents` sheet as the workbook holds it; the first line names the sheet and its columns | R2 |
| `registry/<batch>/works-responsibility.jsonl` | five cells of every row of its `works` sheet: `record_id`, `preferred_label`, `label_ru`, `work_dates`, `responsibility_statements` | R2 |
| `registry/authors-table/` | the authors table of the philosophical corpus: the sheets «Имена» (3 470 spellings) and «Произведения» (17 141 units) | R2 |
| `layer/agents/<batch>.jsonl` | the same agent rows parsed: name forms with their scripts, kind, identifiers, life dates, gaps | R2 |
| `provenance.json` | the workbooks read: path in the registers mirror, bytes, SHA-256; the workbooks themselves are not kept | git |
| `keys.jsonl` | every register row: the member it belongs to and on what basis, or why it was left out | git |
| `ids.jsonl` | the ledger of member IDs: minted, merged into another (a redirect), retired | git |
| `core.json` | the authors table's spellings bound to the register rows they stand for | git |
| `decisions.jsonl` | the readers' decisions: `same`, `different`, `related`, `held`, `left_out`, `kind` | git |
| `queue/` | the questions for readers: `identity.jsonl` (resolve.mjs), `kind`, `dates`, `latin`, `living` (line/kit.mjs) | git |
| `control.json` | the cases the resolution must keep whatever readers decide | git |
| `layers.json` | the digest manifest of `registry/` and `layer/` | git |

`node bonsai/lib/layers.mjs fetch bonsai/review/population` brings the R2 files
into a checkout.

## Rebuilding

```sh
python3 -I bonsai/review/population/intake.py --mirror "<registers mirror>"   # registry/, provenance.json
python3 -I bonsai/review/population/parse.py                                  # layer/
node bonsai/lib/layers.mjs manifest bonsai/review/population                  # layers.json
node bonsai/review/population/resolve.mjs                                     # keys, ledger, core, queue
node bonsai/review/population/plant.mjs                                       # bonsai/population/
node bonsai/review/population/line/kit.mjs queues                             # the other queues
node bonsai/lib/cli.mjs check population
```

The mirror holds `MANIFEST_SHA256.csv` and `Реестры/Реестр персон и
произведений/**/ToS_Identity_<batch>_registry.xlsx`; the authors table
`ToS_Авторы_для_бонсая.xlsx` lies beside it. Every workbook must match the
digest of the mirror's manifest and of the authors table, or the intake stops.
Every tool takes `--check`: it builds everything in memory and says whether the
stored files are what it gives.

## Parsing

`parse.py` reads only the row's own words. Names: the preferred label, the
Russian label without the register's wrappers (`Персона: …`, a verse number
` — THAG1.26`) and every variant form. A kind other than person needs an
explicit phrase of the register, quoted under `kind_basis`; a weaker phrase is a
`signal` for a reader. Life dates are read from the birth, death, life and
activity lines alone, in four forms: a year (`около 429 до н. э.`), a date
(`08.11.1848`), a range (`497–425 гг. до н. э.`) and a century (`XIII–XIV вв.`);
a Hijri pair (`866/1462`) where the conversion agrees within a year. A date that
equals the row's own check date, a year below 1000 in an antiquity batch without
an era, and every other form stay gaps with their lines. Years are astronomical.

## Resolution

`resolve.mjs` joins rows of different batches strictly: on a shared authority
identifier (ORCID, ISNI, GND, LCCN, VIAF, Wikidata, BnF, DILA, a Tree agent ID);
on a shared strong name key with an agreeing year of birth or death; on a short
key with both years agreeing. A key is a fold of a name form by its script; it is
strong with two name words besides particles (three Han characters), short
otherwise, and every transcription of a Han, kana or Hangul name is short. Rows of
one batch, a pseudonymous author with anyone else and a collective with a person
never join. Two years of one birth or death agree within one year from 1500 on
and two before it when both are exact, within five and ten when either is
approximate or uncertain (`yearTolerance`); a wider gap is a date conflict. An
automatic union that would put two rows of one batch, two rows a reader called
different, or two conflicting dates into one member is refused. A reader's
`same` may join rows of one batch, since a register can list one person twice;
only a reader's `different` or `related` refuses it, and that contradiction
goes back to the readers as a `decision-conflict`. Every other pair of members
that shares a key is a question in `queue/identity.jsonl`, and so is a name
qualified by its origin beside the same name standing alone ("Hierocles of
Alexandria" and "Hierocles", reason `contained`).

A member's ID is `m-` and ten base32 characters of the SHA-256 of the row it was
founded on. `keys.jsonl` is read back on the next run, so a row keeps its member;
the ledger never gives an ID twice, a merge leaves a redirect and a split keeps
the ID with its founding row.

The authors table is bound through its works: a spelling's units name works, a
work's responsibility lines name agent rows, and the row bearing the spelling —
else sharing a key with it, else the work's single author — is the spelling's
row. Only a spelling whose works give no row is looked for by its form.

## The reading line

[`line/`](line/AGENTS.md) cuts questions into packets for two independent
readers, checks their verdicts with a gate and turns the agreed ones into
decisions. An identity question asks whether two members are one bearer
(`resolve.mjs` reads the decision); a planted member's question asks a row's
kind, the member's name in Latin script, or its years and whether it lives
(`plant.mjs` plants the answer: a kind, a name, years with the readers' source
in place of the registers' gap). [`line/READER.md`](line/READER.md) is the
reader's route.

## What this folder does not claim

Nothing here is admitted to the bonsai's verdicts or to the Tree. A register's
`verified` is its own statement; a member stays `undecided` until readers have
read its rows. The layer keeps every row; corrections come as new versions of
the workbooks, never as edits of a row.
