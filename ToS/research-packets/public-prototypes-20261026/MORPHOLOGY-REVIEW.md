# Bounded review of retained morphology candidates

Date: 2026-10-26. Reviewer: Codex root, source-visible agent review. This is a
review of 30 selected forms, not human linguistic admission, a corpus-wide
accuracy measure or canon. The source versions and full tool snapshots are
selected in `access/public-corpus/selection.v1.json`.

The sample deliberately includes inflection, ambiguity, Greek polytonic text,
punctuation and historical spelling. The German source is `gutenberg-7205-de`;
the Greek source is `perseus-burnet-1903-grc` of *Meno*. Exact forms remain
unchanged. Candidate alternatives were inspected rather than treating the
first returned candidate as the answer.

| Exact form | Observed candidate behavior and limit |
| --- | --- |
| Gold | German neuter noun; multiple case readings remain |
| Kreis | Noun `Kreis` and verb `kreisen`; context must decide |
| Schicksal | Neuter noun; inflection candidates remain |
| Brüder | `Bruder`, plural, with case alternatives |
| Kindern | `Kind`, dative plural |
| schenken | Verb alternatives include person/mood ambiguity |
| höheren | `hoch`, comparative inflection alternatives |
| Geist | Masculine noun candidates |
| Mensch | Noun; nominative singular among the returned readings |
| Erde | Feminine noun, several case readings |
| gieng | Unrecognized historical spelling; no replacement with `ging` |
| Noth | Unrecognized historical spelling; no replacement with `Not` |
| thun | Unrecognized historical spelling; no replacement with `tun` |
| giebt | Unrecognized historical spelling; no replacement with `gibt` |
| Werthe | Unrecognized historical spelling; no replacement with `Werte` |
| ἔχεις | Includes `ἔχις`, `ἔχω`, `χάω`, `χέω`; in the opening question `ἔχω` fits, while the first noun candidate does not |
| μοι | `ἐγώ`, dative pronoun |
| εἰπεῖν | `εἶπον`, infinitive candidate |
| ὦ | Verb/subjunctive and exclamation alternatives; the address to Socrates needs the latter |
| Σώκρατες | Vocative candidate |
| ἆρα | Returned noun/`αἴρω` readings do not adequately represent the interrogative particle here; a known provider quality gap |
| διδακτὸν | Adjective candidates with case/gender alternatives |
| ἡ | Article candidates |
| ἀρετή | Feminine noun; nominative/vocative alternatives |
| ἢ | Conjunction/exclamation alternatives |
| οὐ | Adverb/negative candidate |
| ἀλλʼ | Processed but unrecognized; punctuation is preserved |
| ἀσκητόν | Adjective alternatives |
| οὔτε | Adverb/negative candidate |
| ἀσκητὸν | Accent-distinct exact form retained separately; adjective alternatives |

Decision: these finite candidate sets are useful as a source-linked aid when
their alternatives and unavailability states stay visible. They do not support
automatic contextual disambiguation, historical spelling normalization or
universal language quality claims. In particular, neither candidate order nor
successful HTTP acquisition establishes the right reading. Unknown Greek
forms outside the retained sample must say "not processed", not "unrecognized".
The reader preserves dictionary navigation and exact concordance in either
case. A future contextual provider can add a separately versioned assessment;
it must not overwrite the acquired candidate layer.
