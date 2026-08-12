# Witness-text collation foundation research

Date: 2026-08-12

Status: ordered method research and one bounded contract decision; no accepted
text, gold, edition lineage, textual equivalence, preferred reading,
translation judgment, semantic relation, graph fact, canon effect, or
publication permission

## Question

How should Tree of Sophia compare exact passages from two witnesses of the
same Work without treating string similarity as a translation alignment,
edition genealogy, critical reading, or textual truth?

The immediate pressure is concrete. One operator-local 2007 Antonovsky page
contains a preserved, human-entered Russian transcription observation from the
closed sparse-calibration Workbench. It has no completion attestation and no
gold authority. The exact Antonovsky/Prometey 1911 Item independently supplies
one private raw embedded-text sentence proposal. Their surface similarity is
useful evidence for a later collation question, but it cannot by itself prove
that the 2007 reading was copied from 1911, that either string is correct, or
that the Expressions are equivalent.

## I. Classical and current primary documentation

### TEI P5 critical apparatus

The current [TEI P5 critical-apparatus chapter](https://tei-c.org/release/doc/tei-p5-doc/en/html/TC.html)
keeps witnesses, readings, apparatus entries, and the mechanism linking an
apparatus to its text explicit. Location-reference, double-end-point, and
parallel-segmentation encodings serve different reconstruction needs. For two
texts in the same language and script, parallel segmentation can represent
matching segments directly, while stand-off apparatus remains preferable when
competing or overlapping interpretations must coexist.

ToS consequence: each compared reading must retain its own exact
Work/Expression/Edition/Item/File, immutable text-layer, selector, digest, and
source-return route. A collation record is stand-off and may project to TEI;
TEI is not the owner truth and no witness becomes an implicit lemma.

### CollateX documentation

The official [CollateX documentation](https://collatex.net/doc/) separates
tokenization, normalization, alignment, analysis/feedback, and output. Its
variant graph merges tokens only under an explicit comparator; divergent
segments branch while each witness remains recoverable. The documentation
also warns that progressive alignment can depend on witness order, approximate
matching is heuristic, transpositions remain difficult, and computational
findings still require interpretation. JSON, TEI, GraphML, and Graphviz are
outputs of one collation process, not four independent truths.

ToS consequence: preserve original strings and record every comparison view
separately. Exact equality, whitespace-normalized similarity, character-level
similarity, token alignment, and a future variant graph are distinct method
results. The first bounded packet needs no token graph: it records only
digest-bound pairwise metrics and keeps its detailed edit script local.

### W3C Web Annotation and PROV

The [Web Annotation Data Model](https://www.w3.org/TR/annotation-model/)
separates bodies, targets, selectors, source states, creators, generators, and
motivation. [PROV-O](https://www.w3.org/TR/prov-o/) separately models entities,
activities, agents, use, generation, and derivation.

ToS consequence: a human-entered source observation, the software comparison,
and a later human review are different acts. The comparison points to exact
source-bound anchors and provenance. It neither upgrades the observation into
attested gold nor invents a derivation edge between Expressions.

## II. Established scholarly and computational foundations

Haentjens Dekker, Van Hulle, Middell, Neyt, and Van Zundert,
[Computer-supported collation of modern manuscripts](https://doi.org/10.1093/llc/fqu007)
(2015), remains the central CollateX account. It treats tool architecture,
heuristics, interoperability, and editorial use together: users choose textual
units and assess whether the resulting collation serves an archive, genetic
dossier, or edition.

Birnbaum and Eckhoff,
[Reassessing the locus of normalization in machine-assisted collation](https://www.digitalhumanities.org/dhq/vol/14/3/000489/000489.html)
(2020), makes the Gothenburg pipeline explicit and shows why normalization
must be purpose-bound and non-destructive. The witnessed token and its
normalized shadow must both survive; manual correction of an intermediate
alignment also creates a new derived state that needs provenance.

Bleeker and colleagues,
[Layers of Variation](https://dhq.digitalhumanities.org/vol/16/1/000583/000583.html)
(2022), shows that revisions and other nonlinear text exceed a flat sequence.
Variant graphs and hypergraphs are useful representations, but the scholarly
model of what counts as a revision or correspondence remains an interpretive
choice.

ToS consequence: the durable object is not a single diff. It is a packet that
binds witness states, exact scopes, method configurations, alternative
comparison views, uncertainty, review, rights, and authority limits. Detailed
graphs or apparatus are replaceable projections from that packet.

## III. Fresh edge checked for current relevance

Freshness was checked after the standards and established work, and only
relevant developments were retained.

- Dähne, Ritter, and Molitor,
  [Improving text collations by local text resegmentation](https://doi.org/10.1093/llc/fqaf033)
  (2025), demonstrates that fixed paragraph segmentation can create poor
  alignments when revisions split, join, insert, or move material. This is
  directly relevant to ToS: a sentence selector is a versioned proposal, not
  a permanent natural boundary.
- Bleeker, Spadini, Nava, Oostveen, and Haentjens Dekker,
  [Here is strangeness: A Collaborative Approach to Visualising Textual Variation](https://doi.org/10.5281/zenodo.15387538)
  (2025), identifies shared vocabulary, interchange, and visualization as
  continuing open problems. A ToS packet therefore must not confuse its local
  vocabulary or one rendering with a universal standard.
- Alrahabi and Wainstain,
  [Versus: an automatic text comparison tool for the digital humanities](https://aclanthology.org/2025.lm4dh-1.3/)
  (2025), combines multigranular comparison, interactive visualization, and
  critical traceability. Its relevance is the traceability requirement, not a
  license to let vector similarity assert textual history or semantics.

No 2026 paper found in this pass displaced these foundations for the bounded
same-language, two-witness question. Recency alone would be a weaker selection
criterion than exact methodological fit.

## IV. Contract decision

ToS needs an additive `tos_witness_text_collation_packet_v1` distinct from
both `tos_translation_alignment_packet_v1` and Expression derivation claims.
The contract must:

1. bind at least two independently identified witness states through exact
   Work/Expression/Edition/Item/File, immutable text-layer, frozen unit packet,
   source anchor, selector, digest, rights, and source return;
2. keep comparison identity opaque and independent of current strings,
   offsets, labels, score, or chosen algorithm;
3. represent pairwise correspondence as a claim with proposed, ambiguous,
   accepted, rejected, deferred, or superseded state;
4. record raw and declared normalized comparison views independently, with
   method, lengths, equality, score, operation summaries, and an opaque
   edit-script digest;
5. keep reconstructive detail in ignored local storage when any input is not
   public;
6. require a source-visible real-human review before acceptance while allowing
   software or a model to propose;
7. keep preferred reading, equivalence, Expression derivation, translation,
   lexical identity, semantics, graph, canon, and publication separate and
   false unless later owner evidence explicitly establishes them;
8. apply the most restrictive visibility and rights posture of every witness,
   packet, detail artifact, and destination;
9. treat TEI apparatus, alignment tables, and variant graphs as derived
   projections admitted only from reviewed claims and independently governed
   for rights.

## V. Bounded Antonovsky control

The first real packet may reuse exactly one existing observation without
asking the solo operator to repeat it:

- source observation: `tos-sample-antonovsky-p011` from the fixed Workbench
  autosave;
- observation posture: human-entered, `attestation_status: not_collected`, no
  pass receipt, no source acceptance, no gold, no method ranking;
- 2007 sentence selector: first non-whitespace span after the page's first
  `U+002E` terminator through the next `U+002E`, fixed as `[67,168)` in the
  exact 1,503-code-point observation layer;
- 1911 sentence selector: the existing `[0,107)` unreviewed proposal in the
  exact private raw embedded-text layer;
- comparison: exact, NFC, whitespace-collapsed, and alphanumeric-casefold
  character views using Python `difflib.SequenceMatcher` with `autojunk=false`;
- result posture: same-Work, same-language, one-to-one correspondence proposal
  suitable only for local collation research.

The score is screening evidence. It does not prove source fidelity, sentence
correctness, textual identity, revision history, or Expression equivalence.
The 2007 PDF, transcription, comparison detail, and all source-bearing
derivatives remain operator-local and are forbidden from the future site under
the current rights record.

## Stop line

The first packet ends after exact local materialization, text-free tracked
metadata, reproducible metrics, negative controls, and provenance. It creates
no second human pass, no Workbench task, no gold update, no accepted source,
no `is_derivative_of` claim, no translation comparison, no semantic or sign
packet, no graph projection, and no publication route.
