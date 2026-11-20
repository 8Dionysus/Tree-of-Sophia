# TOS-D-0082 The Population Bonsai and the Core of a Population

## Index Metadata

- Decision ID: TOS-D-0082
- Original date: 2026-11-20
- Surface classes: bonsai, access/web
- ToS layers: derived projection
- Tree classes: none
- Guard families: source-first authority, review separation, bounded downstream seams
- Posture: accepted by explicit operator instruction

## Context

The Tree's research registers of persons and works cover the whole
philosophical corpus batch by batch: 190 Identity workbooks of antiquity, the
middle ages and modern times, with 16 971 rows of people, pseudonymous authors,
traditional identities and collectives, 68 634 works, and Context and Events
registers that name the same people by their row IDs. The authors table of the
corpus lists 3 470 spellings of its authors. The registers were made by separate
research passes: one person recurs across batches under different spellings and
languages, namesakes share names, and the registers' own statuses are their
compilers' statements. The three bonsai stands hold 17 to 47 persons each.

## Decision

The fourth bonsai, Population, is planted at `bonsai/population/` with the core
of a population as its main module, `population.json`
(`tos.bonsai.population.v1`), described in `bonsai/CORE.md`, "A core of a
population". A member is a bearer of responsibility grown from the register rows
the identity resolution joined; it carries a kind typed by the Tree's entity
registry, its names, the registers' statuses beside the reader's verdict, its
life as biographical phases of the stand's chronology, its gaps, and the rows it
grew from. Its ID comes from the row it was founded on and is kept in a ledger
that never gives an ID twice.

The registers are kept whole and public: every row of each `agents` sheet, five
cells of every row of each `works` sheet and the authors table, in R2 under
`bonsai/review/population/layers.json`, by the operator's decision. Rows join
automatically only on a shared authority identifier, on a strong name with an
agreeing year of birth or death, or on a short name with both years; every other
pair that shares a key, and every name standing alone beside the same name
qualified by its origin, is a question for two independent readers, whose agreed
verdicts become decisions. The skeleton plants the members the authors table
binds through its works; translators, editors, works, places and events grow on
it as rings from the same registers.

## Options Considered

Extending the Aesthetics library would give each author a slug of the Latin
name and join records whose names agree. At this scale namesakes and missing
Latin forms make slugs collide, and joining by name alone merges different
people.

Planting every register row as its own member would keep one person in up to
thirty-seven places and give saved graphs IDs that later merges invalidate.

Resolving all identities by readers before planting would hold the skeleton
back for thousands of reading turns that a mass line performs faster once the
questions, the gate and the decisions format exist.

## Rationale

Identity comes before meaning: a wrong join of two people is worse than two
members for one person, so automatic joins are strict and the rest is read.
Stable IDs keep saved graphs and later rings valid across merges. Keeping the
rows whole and every member's `grew_from` keeps the path back to what each
register said.

## Consequences

The skeleton plants the corpus authors with their kinds, names and lives in the
registers' words; every member's verdict stays `undecided`. The identity
questions, and the queues of kinds, dates, Latin names and living status, go to
the reading line under `bonsai/review/population/line/`. The stand opens later;
until then the site carries the manifest alone, while merging publishes the
register layer to R2 and the site copy.

## Source Surfaces

- `bonsai/lib/population.mjs`, `bonsai/lib/graft.mjs`, `bonsai/lib/cli.mjs`
- `bonsai/population/`
- `bonsai/review/population/`
- `bonsai/CORE.md`, `bonsai/README.md`
- `access/web/src/public-research/bonsai.mjs`

## Validation

`bonsai-population.test.mjs` covers the module's contract against the Tree's
registries, the chronology and the graft map, the refusals, the resolution's
tiers, the control cases and the equality of the stored files with what the
tools give. `node bonsai/lib/cli.mjs check population` checks the module, its
export and the review folder: every register row in exactly one member or left
out with a reason, every member ID in the ledger, every control case held.
