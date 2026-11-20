# Reading an identity question

You are one of two readers of a packet. The other reader answers the same
questions without seeing your file; only what you both agree on becomes a
decision. Read [AGENTS.md](AGENTS.md) first.

## The question

`items.jsonl` holds one question per line:

- `item` and `sha` — copy both into your verdict;
- `reason` — why the resolution asked: `strong-undated` (a full name shared,
  no comparable years), `short-only` or `short-partial` (a one-word or
  transcribed name, no years or one), `date-conflict` (the name agrees, a year
  does not), `kind` (a person and a traditional or attributed identity),
  `disputed` (a register marks a row disputed), `id-conflict`, `blocked:…`
  (joining would put two conflicting rows into one member), `contained` (a
  name standing alone and the same name with an origin, "Hierocles" and
  "Hierocles of Alexandria");
- `shared` — the name keys both sides share; `hierocles …` marks a name one
  side carries only with its origin;
- `a` and `b` — the two members, each with every register row it grew from:
  the row's ID, its kind and the register's status, and the row's own fields
  (`preferred_label`, `label_ru`, `variant_labels`, `life_dates`,
  `external_identifiers`, `disambiguation_note`, `responsibility_summary`,
  `notes`, `work_inventory_scope`, `source_refs`, `evidence_notes`).

## How to decide

1. Read both sides whole. The rows come from different research batches: one
   person often appears under different spellings and languages, and two people
   often share a name.
2. Compare what tells people apart: years of life and activity, the works each
   row lists, teachers and schools, places, offices, the register's own
   disambiguation note.
3. Where the rows do not settle it, open the sources they cite or an authority
   file. Record what you opened in `authority`.
4. Decide:
   - `same` — one bearer of responsibility; needs a basis beyond the name;
   - `different` — two;
   - `related` — two, bound by a relation: `pseudonym_of`, `attributed_to`,
     `conventional_for`, `variant_row` (one register row repeats another),
     `part_of` (a person in a collective), `namesake_of` (worth recording);
   - `undecidable` — the evidence you can reach does not settle it.

## The verdict

One JSON line per question in `reader-<n>.jsonl`:

```json
{"item":"q-…","sha":"…","verdict":"same","relation":null,
 "basis":["dates","works"],
 "quotes":[{"side":"a","row":"T3-06:T3-06-A004","field":"life_dates","text":"рождение; 01.07.1646"},
           {"side":"b","row":"T3-83:T3-83-A186","field":"external_identifiers","text":"n79081493"}],
 "authority":[{"side":"b","scheme":"lccn","value":"n79081493"}],
 "note":"Both rows date Leibniz 1646–1716 and list the Monadology."}
```

- `basis` — from `name`, `dates`, `works`, `authority`, `teacher`, `place`,
  `office`, `text`, `register`; `same` needs one beyond `name`.
- `quotes` — at least one for every verdict but `undecidable`: the exact words
  (four characters or more) of a field of a row the question holds, with its
  side, row and field.
- `authority` — optional: identifiers you opened, each with its side and a
  scheme from `orcid`, `isni`, `viaf`, `gnd`, `lccn`, `wikidata`, `bnf`,
  `dila`.
- `note` — one sentence in English or Russian saying what decided it.

Run `node ../../kit.mjs gate <packet>` when you are done, and fix your file
until no line of the gate names your reader number.
