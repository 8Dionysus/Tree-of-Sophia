# Public prototype reading journeys

These six journeys are provisional reading questions for the two public
profiles. They are not canon, translation, or semantic acceptance. Each
`readingHypothesis` is paired with a text-grounded `strongAlternative` so the
reader can test the proposed route rather than inherit its conclusion.

## Source places read

- Zarathustra: the source-language German Project Gutenberg transcription
  `gutenberg-7205-de`, in the source-visible paragraphs of **Von der
  schenkenden Tugend**, **Das Nachtlied**, **Das Tanzlied**, **Vom Gesicht und
  Räthsel**, and **Der Genesende**. The passages include the gold/giving
  comparison, the disciples' wish to become gifts, the night speaker's desire
  and self-light, Zarathustra's address to the ship's company, the reported
  laughter, the animals' interruption and account of eternal return, and
  Zarathustra's final silence.
- Plato, **Meno**, `perseus-lamb-1924-en`: the opening question, Menon's
  report of perplexity, Socrates' account of true opinion and knowledge, and
  the conditional closing exchange. The generated public page rows carry
  Stephanus references: the route uses 70, 80, 97, and 98 as separate
  locations, while the final exchange is reference 100. The visible `Men.`,
  `Soc.`, and `An.` labels were used to keep the speakers distinct. Unit IDs
  are opaque; use each generated page row's `reference` and `text` rather than
  inferring Stephanus locations from unit order.
- Aristotle, **Nicomachean Ethics**, `perseus-rackham-1934-en`: VIII.1 on
  friendship, virtue, and life; VIII.3 on the lovable, utility, pleasure, and
  virtue friendship; IX.9 on friendship and happiness; and II.1 on acquiring
  character virtue through action. Rackham's parenthetical notes remain part
  of the supplied text layer, so the prompts paraphrase rather than quote.
- Plato, **Apology**, `perseus-fowler-1914-en`: the source paragraph at 38,
  read as part of Socrates' courtroom response. Its relation to the *Meno* is
  framed as a comparison of distinct dialogues and rhetorical situations.

All route targets are existing document/version/unit triples in the generated
public snapshot and corresponding source-text-unit packets. The three
Zarathustra selection starts are section headings in the generated pages:
**Von der schenkenden Tugend** (unit 830), **Das Nachtlied** (unit 1207), and
**Der Genesende** (unit 2585). The steps use matching public-page rows for
actual textual passages, including the later **Das Tanzlied** (unit 1239)
as a counterexample to inevitable isolation. Several source units begin or
end mid-sentence; prompts use adjacent context where needed and do not treat
segmentation boundaries as semantic boundaries. The Apology 38 prompt follows
the source's sense that Socrates says silence would be disobedience to the god.

Interpretations are hypotheses only. No direct source passage has been quoted
or translated here; prompts paraphrase the supplied texts. No claim is made
that the routes settle a philosophical reading.

The separate [reader witness-difference note](reader-textual-variants.v1.json)
records a source-visible agent comparison of one phrase in Adams' *Sacred
Disease* against the 1886 printed witness. The reader shows both readings
without changing the retained digital layer, its revision, or existing
selectors. This provisional note is not a complete collation or canon admission.
