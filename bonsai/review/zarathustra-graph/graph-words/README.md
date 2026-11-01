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
