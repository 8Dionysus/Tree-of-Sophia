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
