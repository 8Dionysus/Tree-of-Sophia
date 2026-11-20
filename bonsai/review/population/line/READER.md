# Reading an identity question

You are one of two readers of a packet. The other reader answers the same
questions without seeing your file; only what you both agree on becomes a
decision. Read [AGENTS.md](AGENTS.md) first.

## The question

Read the questions with `node ../../kit.mjs show <packet>` (one question:
`show <packet> <item>`); `items.jsonl` holds the same, one JSON line each:

- `item` and `sha` — copy both into your verdict;
- `reason` — why the resolution asked:
  - `strong-undated` — a full name shared, no comparable years;
  - `short-only`, `short-partial` — a one-word or transcribed name, with no
    year or one year in common;
  - `date-conflict`, `short-conflict` — the name agrees, a year does not;
  - `contained` — a name standing alone and the same name with an origin
    ("Hierocles" and "Hierocles of Alexandria");
  - `id-conflict`, `id-with-date-conflict` — the rows' identifiers (`ids`)
    disagree, or agree while a year does not;
  - `kind` — a person and a traditional, attributed or anonymous identity;
  - `disputed` — a register marks a row disputed;
  - `blocked:…` — joining would put two conflicting rows into one member;
  - `decision-conflict` — earlier decisions contradict each other: one joined
    rows of these members, another separated them;
- `shared` — the name keys both sides share; `hierocles …` marks a name one
  side carries only with its origin;
- `a` and `b` — the two members, each with every register row it grew from:
  the row's ID, its parsed kind and the register's status, the typed
  identifiers the resolution read (`ids`), the row's own fields
  (`preferred_label`, `label_ru`, `variant_labels`, `life_dates`,
  `external_identifiers`, `disambiguation_note`, `responsibility_summary`,
  `notes`, `work_inventory_scope`, `source_refs`, `evidence_notes`) and
  `works`: the works of the row's batch whose responsibility lines name it,
  with their titles (twelve at most).

## How to decide

1. Read both sides whole. The rows come from different research batches: one
   person often appears under different spellings and languages, and two people
   often share a name.
2. Compare what tells people apart: years of life and activity, the works each
   row lists, teachers and schools, places, offices, the register's own
   disambiguation note.
3. Where the rows do not settle it, open the sources they cite, an authority
   file, or a scholarly source (an edition, a catalogue, an encyclopedia, a
   specialist's study). Name what you opened in `authority` or `sources`.
4. The rows' own words decide, not the parsed `kind`: a row that calls itself a
   tradition, or the historical person, is read as it says, and one row's words
   suffice when nothing in the other contradicts them. Say in the note where a
   row contradicts its kind.
5. Decide:
   - `same` — one bearer of responsibility, also where one batch lists the
     same person twice; needs a basis beyond the name;
   - `different` — two;
   - `related` — two, bound by a relation: `pseudonym_of`, `attributed_to`,
     `conventional_for`, `part_of` (a person in a collective), `namesake_of`
     (sources or registers confuse the two; the line counts it as
     `different`);
   - `undecidable` — the evidence you can reach does not settle it.

## The verdict

One JSON line per question in `reader-<n>.jsonl`:

```json
{"item":"q-…","sha":"…","verdict":"same","relation":null,
 "basis":["dates","works"],
 "quotes":[{"side":"a","row":"T3-06:T3-06-A004","field":"life_dates","text":"рождение; 01.07.1646"},
           {"side":"b","row":"T3-83:T3-83-A186","field":"external_identifiers","text":"n79081493"}],
 "authority":[{"side":"b","scheme":"lccn","value":"n79081493"}],
 "sources":["https://plato.stanford.edu/entries/leibniz/"],
 "note":"Both rows date Leibniz 1646–1716 and list the Monadology."}
```

- `basis` — what decided it; `same` needs one beyond `name`:
  - `name` — the names agree;
  - `dates` — years of life or activity;
  - `works` — the works the rows list;
  - `authority` — an authority record;
  - `teacher` — a teacher, school or disciple;
  - `place` — a place of life or work;
  - `office` — an office or title;
  - `text` — a source text or study, cited by a row or opened by you;
  - `register` — a register's own disambiguation note or statement.
- `quotes` — at least one for every verdict but `undecidable`: the exact words
  (four characters or more) of a field of a row the question holds, with its
  side, row and field.
- `authority` — optional: identifiers you opened, each with its side and a
  scheme from `orcid`, `isni`, `viaf`, `gnd`, `lccn`, `wikidata`, `bnf`,
  `dila`.
- `sources` — optional: the URLs or citations of the other sources you opened.
- `note` — one sentence in English or Russian saying what decided it.

When you are done, run `node ../../kit.mjs gate <packet> --reader <n>` and fix
your file until it names no problem. Never run the gate without `--reader`.
