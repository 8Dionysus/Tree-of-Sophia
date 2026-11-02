# Growing the Zarathustra graph from the book's words

Tools and briefs for the second step of the Zarathustra rebuild: links grown from every place of a
concept's word and of a character's appearance, first in «Also sprach Zarathustra», later in
Nietzsche's other works.

- `prep.py <witnesses> <out>` — flat aligned text (de first editions, ru Antonovsky 1913, en Common 1909), one row per aligned paragraph.
- `kwic.py` — every place of a word with its neighbourhood; `refrains.py` — word sequences the book repeats across chapters; `checkcite.py` — checks cites and quotes against the German text.
- `seeds.json` — the concepts (Dionysus's list, then the stand's original concepts and images), the characters, and the question bank.
- `brief-concepts.md`, `brief-characters.md` — the two growing passes (GPT-6.1 Sol, max effort); `brief-affirm.md` — rewriting the strict rebuild's texts as affirmations.

What the passes find is unreviewed: every link carries its places and a status (text, reconstruction,
hypothesis), and only links the book itself makes at a named place go to the map.

## Reproducing this core

[`assembly-inputs/`](assembly-inputs/) holds the voice map, review cards,
verdicts and English revisions used by `assemble.py`. Those files are exact
copies. The concept and character files are compact derivatives of the maker's
files: recursively, `de`, `translations`, `sentence`, `sentence_neighbours`,
`context`, `rows` and `row` keys were omitted, except for the German word and
name lists under `lexicon.de` and `names.de`. This removes copied witness text
and some workbench metadata; it does not claim to be the complete original
research. [`provenance.json`](assembly-inputs/provenance.json) records each
original and derivative digest. The original inputs are preserved separately.

To replay without modifying those inputs, copy `assembly-inputs/` to a working
directory and generate the aligned rows from the checked-in witnesses with
`prep.py`. Run `assemble.py --work <working-copy> --corpus <rows-directory>
--tos <output-directory> --registry <checkout>/ToS/doctrine/semantic-interchange`.
The assembler writes its outputs under the output directory and records any
missing English revisions in the working copy. On the preserved originals and
the compact inputs, it produced identical core files. `control.json` is an
authored control file and is not emitted by this assembler.

[`assembly-inputs/corrections.json`](assembly-inputs/corrections.json) lists
the language-review ledgers in application order, with the original
[`language-corrections-2026-11-02.json`](assembly-inputs/language-corrections-2026-11-02.json)
first, followed by
[`term-corrections-2026-11-02.json`](assembly-inputs/term-corrections-2026-11-02.json).
Each entry replaces a whole reader-text wording in the assembled core;
paired Russian and English entries stay linked to their matching Russian text,
while an English-only entry applies to every exact matching English reader
text. The assembler applies these reviews after the verdicts, so the makers'
and reviewers' wordings stay in their inputs, and drops the generated filler of
proposed nodes. Counts in `core.json` retain the Russian totals and report each
language and ledger separately. `access/web/src/public-research/
bonsai-language.test.mjs` keeps reviewed-out words from coming back.


## Source-visible additions and stable addresses

`assembly-inputs/proposal-readings.json` preserves a source-grounded stand reading
of each of the 28 proposed objects. Each states a related existing object, the
places read, a reading and its limits. The independent source-visible reviews
are retained in `independent-content-review-2026-11-01.json` and its two
correction reviews. Earlier wording and the actual findings remain in the
revision history; retaining a proposal does not admit it to Tree canon.
`place-relations.json` keeps seven exact-place comparisons and their voice
attributions in the same proposal posture. The assembled copies are
`core/proposed.json` and `core/place-relations.json`.

`statement-identities.json` is the authored identity register for the existing
core's addressable statement fields and chain steps. A record's `statement_ids`
uses field paths; collection members carry their own `statement_ids` and retain
an existing `id` or receive a `statement_anchor`. `$` identifies the member
itself. Chain steps receive an `id` and the former zero-based
`core-chain:<chain-id>:<index>` address in `aliases`. IDs identify the claim
across language variants and do not depend on the position of a collection
member. The register initially binds members to a fingerprint of the assembled
content. If a member changes, the assembler fails rather than silently assigning
its old identity elsewhere: review the change and explicitly update the
fingerprint while retaining the ID if the claim is the same, or assign a new
ID when it is a new claim. This maintenance is separate from accepting a claim.

`figure-scopes.json` binds each existing figure proposal to a work-scoped
figure candidate and source-described appearance roles. Voice attribution
candidates cover the first exact unit of a segment only; unmapped voices
retain their original speaker labels and are not silently made into figures.
`link-families.json` records one broad subject-model family with its
source-grounded rationale for every link, separate from the exact registry
relation (which remains `tos.relation.unmapped` where no warranted fit is
established). The family is a stand reading, not a registry predicate. Each
voice segment also has an exact-scoped `voice_descriptor`. The primary narrator
segments share a work-local narrator role without asserting a person's identity;
other unbound voices keep their source designation and uncertainty. Neither a
figure candidate, an Occurrence candidate ID nor a family assignment creates
a native source record or accepts a semantic claim.

`route-growth.json` adds the gold and brothers proposals and three reviewed
routes. The blessing-and-giving route compares seven places in branches:
overflow → address → reception; overflow → departure → weariness;
address → things → sacrifice. Its transitions name both compared places;
the retired draft transitions remain in `retired_steps`. These comparisons
do not assert a chronological argument or identify the book's different
future groups. `step-revisions.json` corrects two earlier giving-route steps
while keeping their stable addresses and earlier wording.

`link-sense-revisions.json` records the later source-visible correction of four
link senses. The assembler binds each revision to the digest of the previous
sense and retains that sense and its assessment in `sense_revision_history`;
previous maker `proposed` fields and all places remain intact. The current
revision names the independent reviewer and gives its reason in both languages.
The family register similarly retains `family_proposed` and `family_review`
where an independent reading changed its initial classification.
