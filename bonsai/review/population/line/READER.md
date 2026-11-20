# Reading the Population line's questions

You are one of two readers of a packet. The other reader answers the same
questions without seeing your file; only what you both agree on becomes a
decision. Read [AGENTS.md](AGENTS.md) first.

A packet holds questions of one type (`type` in `packet.json`):

- `identity` — are two members one bearer of responsibility?
- `kind` — what kind of entry is a register row?
- `latin` — what is a member's name in Latin script?
- `dates` and `living` — when did a member live, and does the member live now?

Read the questions with `node ../../kit.mjs show <packet>` (one question:
`show <packet> <item>`); `items.jsonl` holds the same, one JSON line each. Every
question gives its `item` and `sha`; copy both into your verdict.

## Every verdict

One JSON line per question in `reader-<n>.jsonl`. Whatever the type, a verdict
carries:

- `quotes` — the exact words (four characters or more) of a field of a row the
  question holds, each with its row and field (an identity question also names
  the side, `a` or `b`);
- `authority` — optional: identifiers you opened, each with a scheme from
  `orcid`, `isni`, `viaf`, `gnd`, `lccn`, `wikidata`, `bnf`, `dila` (and its
  side for an identity question);
- `sources` — optional: the URLs or citations of the other sources you opened;
- `basis` — what decided it:
  - `name` — the names agree;
  - `dates` — years of life or activity;
  - `works` — the works the rows list;
  - `authority` — an authority record;
  - `teacher` — a teacher, school or disciple;
  - `place` — a place of life or work;
  - `office` — an office or title;
  - `text` — a source text or study, cited by a row or opened by you;
  - `register` — a register's own disambiguation note or statement;
- `note` — one sentence in English or Russian saying what decided it.

`undecidable` answers any type when the evidence you can reach does not settle
it; it needs only the note.

When you are done, run `node ../../kit.mjs gate <packet> --reader <n>` and fix
your file until it names no problem. Never run the gate without `--reader`.

## Identity

Each question holds two members, `a` and `b`, each with every register row it
grew from: the row's ID, its parsed kind and the register's status, the typed
identifiers the resolution read (`ids`), the row's own fields
(`preferred_label`, `label_ru`, `variant_labels`, `life_dates`,
`external_identifiers`, `disambiguation_note`, `responsibility_summary`,
`notes`, `work_inventory_scope`, `source_refs`, `evidence_notes`) and `works`:
the works of the row's batch whose responsibility lines name it, with their
titles (twelve at most). `reason` says why the resolution asked:

- `strong-undated` — a full name shared, no comparable years;
- `short-only`, `short-partial` — a one-word or transcribed name, with no year
  or one year in common;
- `date-conflict`, `short-conflict` — the name agrees, a year does not;
- `contained` — a name standing alone and the same name with an origin
  ("Hierocles" and "Hierocles of Alexandria");
- `id-conflict`, `id-with-date-conflict` — the rows' identifiers (`ids`)
  disagree, or agree while a year does not;
- `kind` — a person and a traditional, attributed or anonymous identity;
- `disputed` — a register marks a row disputed;
- `blocked:…` — joining would put two conflicting rows into one member;
- `decision-conflict` — earlier decisions contradict each other.

`shared` gives the name keys both sides share; `hierocles …` marks a name one
side carries only with its origin.

How to decide:

1. Read both sides whole. The rows come from different research batches: one
   person often appears under different spellings and languages, and two people
   often share a name.
2. Compare what tells people apart: years of life and activity, the works each
   row lists, teachers and schools, places, offices, the register's own
   disambiguation note.
3. Where the rows do not settle it, open the sources they cite, an authority
   file, or a scholarly source (an edition, a catalogue, an encyclopedia, a
   specialist's study).
4. The rows' own words decide, not the parsed `kind`: a row that calls itself a
   tradition, or the historical person, is read as it says, and one row's words
   suffice when nothing in the other contradicts them. Say in the note where a
   row contradicts its kind.
5. Decide `verdict`:
   - `same` — one bearer of responsibility, also where one batch lists the
     same person twice; needs a basis beyond the name;
   - `different` — two;
   - `related` — two, bound by a `relation`: `pseudonym_of`, `attributed_to`,
     `conventional_for`, `part_of` (a person in a collective), `namesake_of`
     (sources or registers confuse the two; the line counts it as
     `different`);
   - `undecidable`.

```json
{"item":"q-…","sha":"…","verdict":"same","relation":null,
 "basis":["dates","works"],
 "quotes":[{"side":"a","row":"T3-06:T3-06-A004","field":"life_dates","text":"рождение; 01.07.1646"},
           {"side":"b","row":"T3-83:T3-83-A186","field":"external_identifiers","text":"n79081493"}],
 "authority":[{"side":"b","scheme":"lccn","value":"n79081493"}],
 "sources":["https://plato.stanford.edu/entries/leibniz/"],
 "note":"Both rows date Leibniz 1646–1716 and list the Monadology."}
```

## A member's questions

A `kind`, `latin`, `dates` or `living` question holds one member: its ID, its
names and kind as the bonsai plants them now, and every register row it grew
from, with the same fields as above.

### Kind

The question names one row (`row`) whose words hint at another kind than the
one the parser read (`signals`). Answer with the kind the row's own words give,
as `verdict`: `person`, `pseudonymous` (an author writing under another's name),
`conventional` (a traditional or conventional authorial identity),
`attributed-voice` (a voice a tradition attributes a text to),
`unnamed-hand` (an anonymous scribe or compiler), `collective` (a group), or
`undecidable`. Quote the words that decide it.

```json
{"item":"q-…","sha":"…","verdict":"conventional","basis":["register"],
 "quotes":[{"row":"T2-31:T2-31-A004","field":"notes","text":"традиционная авторская идентичность"}],
 "note":"The row calls the Nyāyasūtra's author a traditional identity."}
```

### Name in Latin script

No row gives the member's name in Latin script. Give it as scholarship writes
it: the form a row's variants or sources use, else a standard transliteration
(pinyin for Chinese, IAST for Sanskrit and Pali, ALA-LC for Russian, Arabic,
Persian and Hebrew), with its marks. `verdict` is `latin` with the name in
`latin`, or `undecidable`. Quote the row, or name the source you took the form
from. The line compares the two readers' forms without their marks and case,
and keeps the one with more marks.

```json
{"item":"q-…","sha":"…","verdict":"latin","latin":"Huìyuǎn","basis":["name"],
 "quotes":[{"row":"T2-44:T2-44-A023","field":"preferred_label","text":"慧遠"}],
 "note":"Pinyin of 慧遠."}
```

### Years and living

The member has no years the parser could read (`reason`: `life:unchecked`, the
register did not check them; `life:unread`, the register's words are not a form
the parser reads; `life:none`), its rows give different years
(`rows-differ`), or a row gives a birth from 1900 on with no death (`living`).
Read the rows' words, and where they do not give the years, the sources they
cite or a scholarly source; never years from memory alone.

`verdict` is `dates` with:

- `birth`, `death` — each `null` or `{"year": 428, "era": "BCE" or "CE",
  "certainty": "exact", "approximate" or "uncertain"}`;
- `activity` — `null` or the years of activity where no life is known:
  `{"from": {"year", "era"}, "to": {"year", "era"}, "certainty"}`; a century
  is its first and last year;
- `living` — `true` only for a person the sources show alive (born 1880 or
  later, no death); otherwise leave it out.

Give what the evidence holds and leave the rest `null`: a death without a birth
is an answer. Quote the row's words, or name the source in `sources` (and the
basis `text` or `authority`). The two readers agree when they give the same
bounds within a few years (more for approximate and ancient years); the line
keeps the first reader's years at the weaker certainty.

```json
{"item":"q-…","sha":"…","verdict":"dates",
 "birth":{"year":1275,"era":"CE","certainty":"approximate"},"death":{"year":1344,"era":"CE","certainty":"approximate"},
 "activity":null,"basis":["text"],"quotes":[],
 "sources":["https://plato.stanford.edu/entries/burley/"],
 "note":"The SEP article dates Walter Burley c. 1275 – c. 1344."}
```
