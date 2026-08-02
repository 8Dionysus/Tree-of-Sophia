# Question-scoped usage-context foundation

Status: ordered method research and pre-output decision; not accepted German,
linguistic analysis, sign nomination, semantic annotation, graph truth, or a
human-work schedule

Research snapshot: 2026-08-02

Owner route: `ToS/research-packets/AGENTS.md`

## Result first

The next layer after exact-form recurrence should be a **question-scoped,
source-returnable usage-context bundle**, not a partially populated sign
packet.

For one preselected exact-form method control, Tree of Sophia should preserve
every occurrence in source order together with:

- immutable source-database and Item fixity;
- the existing opaque occurrence ID;
- Item, part, page, nearest TEI division, text-node path, and local character
  offsets;
- an exact, page-bounded window of 24 tokens on each side;
- explicit clipping at page boundaries and editorial/unsectioned residue.

The source-bearing rows belong only under the ignored `local-content/` route
with mode `0600`. The tracked companion may keep only hashes, digests, counts,
structural totals, clipping aggregates, and closed authority flags. It must
not contain an exact source string, context sequence, occurrence position,
sentence, quotation, or model output.

This layer answers one mechanical question: **can ToS construct a complete,
replayable concordance substrate for a bounded form without leaking source
content or manufacturing meaning?** It does not answer whether the form is a
lemma, lexeme, sign, motif, concept, or semantically stable entity.

## Exact pressure in the current foundation

The whole-work lexical database already contains exact source order and
opaque occurrence IDs for 86,287 token occurrences. Its tracked projection
withholds sequence and context. The recurrence projection then makes
frequency, structural range, and part-size-aware dispersion queryable for all
11,352 exact-form hashes while preserving the same withholding boundary.

The semantic ladder correctly remains blocked: the initial sign packet still
has no accepted German source unit, language-competence evidence, lexeme,
candidate sign, or human decision. Opening that packet merely to gain
concordance context would collapse two distinct layers:

1. an addressable occurrence in one exact witness;
2. a linguistic or philosophical interpretation of that occurrence.

The needed bridge is therefore a separate local evidence bundle below the
semantic ladder. It can later support sentence-aware extraction, morphology,
translation comparison, Word-in-Context judgments, counterreadings, or sign
research, but none of those effects follows from materialization.

## I. Classical standards and official documentation

### TEI P5 stand-off structures

TEI P5 4.11.0, updated 18 February 2026, defines `standOff` as a container for
annotation-like structures including `annotation`, `span`, `spanGrp`, `link`,
`linkGrp`, and `seg`:

<https://www.tei-c.org/release/doc/tei-p5-doc/en/html/ref-model.standOffPart.html>

The useful principle for ToS is separation: annotations and later
interpretations can address a source without rewriting the source layer.
This first context bundle does not claim TEI conformance and does not inject
markup into the DTA payloads. It retains the existing TEI resource IDs and
paths as selectors in a separate derived local record.

### W3C Web Annotation

The stable W3C Web Annotation Recommendation models an annotation as a
relation between resources and provides selectors for exact text,
positions, structure, ranges, and resource state:

<https://www.w3.org/TR/annotation-model/>

Three details directly constrain this route:

- multiple selectors may describe the same segment so later consumers can
  recover it through more than one path;
- `TextQuoteSelector` copies exact/prefix/suffix text and is explicitly risky
  when rights restrict redistribution;
- `TextPositionSelector` avoids copying text but is brittle under change, so
  the specification recommends binding the intended source state as well.

ToS therefore keeps the quote-like surface only in the ignored local bundle.
Its durable selector is composite rather than offset-only: source database
digest + Item/file digest + occurrence ID + page/division resource + TEI
text-node path + local offsets. The tracked receipt records only the state
digests and aggregate selector closure.

### Concordance as a source-reading instrument

The established concordancing literature treats a concordance as a target
form presented in a declared amount of surrounding text; the context may be a
fixed character/token window, sentence, or larger structural unit. A fixed
KWIC window is a transparent baseline because its inclusion law can be
replayed exactly:

<https://doi.org/10.1002/9781119180180.ch5>

The baseline should not pretend to discover a natural sentence boundary that
the current lexical database does not yet own. Page-bounded 24+target+24 token
windows are therefore chosen as an explicit mechanical view, with clipping
reported rather than hidden.

## II. Established annotation and semantic methods

### DURel: usage relatedness is an annotation, not a token property

DURel asks annotators to judge semantic relatedness between contextualized
uses and keeps diachronic comparison separate from the raw occurrences. Its
German test set used five annotators over 1,320 usage pairs:

<https://aclanthology.org/N18-2027/>

This supports preserving occurrences and contexts before assigning a sense.
It does not authorize ToS to import DURel labels or ask the solo operator to
perform routine pairwise German judgments without competence evidence.

### DWUG: graph edges are judgments between usage nodes

DWUG operationalizes individual usages as graph nodes and graded human
semantic-proximity judgments as edges. Its four-language resource contains
about 100,000 judgments and uses a multi-round annotation process before
clustering usages into senses:

<https://aclanthology.org/2021.emnlp-main.567/>

For ToS, the immediate lesson is architectural: a usage node must remain
source-returnable and distinguishable from a later edge, cluster, or sense
label. This context bundle creates only the local substrate that a future
usage node could cite. It creates no edge and no cluster.

### SemEval 2020: evaluation needs independent gold

The SemEval-2020 lexical-semantic-change task introduced manually annotated
benchmarks for English, German, Latin, and Swedish because evaluation without
gold standards was the pressing problem:

<https://aclanthology.org/2020.semeval-1.1/>

A green extraction validator is therefore not semantic evidence. The first
bundle is evaluated for deterministic closure, source return, leakage, cost,
and speed only. Semantic quality remains unmeasured.

## III. Freshest relevant work

### CoMeDi 2025: disagreement is evidence

The 2025 CoMeDi shared task treats Word-in-Context judgments as ordinal and
makes annotator disagreement an explicit prediction target rather than noise
to discard:

<https://aclanthology.org/2025.comedi-1.4/>

This is relevant later, when real contextual judgments exist. It argues
against prematurely collapsing variant readings into one automatic label.
The current bundle records no judgment at all.

### Embedding-assisted concordancing remains a challenger

Anthony's 2025 work on AI-assisted concordancing explores embeddings for
fuzzy search, grouping, and ordering of concordance lines:

<https://doi.org/10.1016/j.acorp.2025.100164>

That is a useful future B/C method, especially for surfacing non-identical
usages, but it adds model-dependent grouping before ToS has a gold criterion
for philosophical usefulness. It must be compared against a deterministic
exact-form floor rather than replace that floor.

### Instance-level embeddings do not replace source identity

Kishino and colleagues' ACL 2025 method uses contextual embeddings and
unbalanced optimal transport to estimate change at individual usage
instances:

<https://aclanthology.org/2025.acl-long.774/>

Its instance-level emphasis reinforces the need for stable occurrence
identity. Its semantic-shift metrics require diachronic corpora and model
representations that the current single-work control does not provide.

### July 2026 LSCD benchmark: keep the stages modular

The freshest directly applicable primary result found in this refresh is the
July 2026 LSCD Benchmark. It explicitly decomposes lexical semantic change
into usage-level Word-in-Context labels, a graph, word-sense induction, and
only then lemma-level change labels. Its benchmark standardizes components so
increasingly complex methods can be evaluated separately:

<https://aclanthology.org/2026.starsem-conference.10/>

ToS should preserve the same epistemic modularity without importing the
diachronic task: context materialization is a usage-evidence stage; a graph,
sense induction, and philosophical sign interpretation remain later owners.

### July 2026 survey: current models still require caution

The 10 July 2026 Cambridge survey reports that modern benchmarks commonly
sample usages and only partially annotate their quadratic usage-pair graphs.
It also records three current risks relevant to ToS:

- clustering can be unstable under sparse annotation and random choices;
- pretrained models can project modern senses onto historical usages;
- LLM performance is prompt-, scale-, and language-dependent, with weaker
  results outside the best-supported languages.

<https://doi.org/10.1017/nlp.2026.10032>

For a 527-occurrence method control, ToS can avoid sampling at this stage and
retain the complete occurrence census. Embeddings, LLM annotations, pairwise
graphs, and clusters remain explicitly deferred challengers.

The freshness search through 2026-08-02 found no newer primary method that
justifies skipping deterministic source-returnable contexts or treating an
LLM cluster as a stable philosophical entity.

## Frozen method-control selection

The selected exact-form SHA-256 is:

`9f1d4250b0115ee2a8bbc876f90f2048159bafd860d61a4fd1de3df44ee0aa55`

It was already present as a named identity control in the independent manual
recalculation of the recurrence projection before this bundle was proposed.
The frozen recurrence tuple is:

- 527 exact occurrences;
- all four source parts;
- 110 TEI divisions;
- 256 source pages;
- `DP = 144983` millionths;
- 3 unsectioned occurrences;
- 0 source-editorial occurrences.

The selection is **not** a recurrence winner, sign-likeness judgment, or claim
that the surface has one linguistic or philosophical identity. It is chosen
because it is a pre-output, work-level identity control with broad structural
coverage and a still-manageable complete census. The tracked plan retains the
hash, not a source quotation.

## Frozen context law

For each of the 527 target occurrences:

1. preserve the source database's existing occurrence ID and source order;
2. select up to 24 preceding and 24 following exact tokens from the same Item
   and source page;
3. never cross a page or Item boundary;
4. preserve the target's page, nearest division, TEI text-node path, offsets,
   editorial status, and source Item/file state;
5. record left/right clipping when fewer than 24 tokens exist on that page;
6. mutate or normalize no source string;
7. serialize one canonical JSON object per line in source order;
8. keep the packet ignored, local-only, and mode `0600`;
9. expose only digest/count/structure aggregates in the tracked receipt.

This is a page-bounded KWIC baseline. A later sentence-aware or TEI-structural
view must be a separate versioned method, because its segmentation law and
error modes differ.

## Future A/B/C route, not yet scheduled

The current materialization is a method control, not an A/B/C competition.
If a concrete semantic, translation, morphology, or retrieval question later
opens, the same frozen occurrence set can support:

| Candidate | Possible method | Required evidence before use |
| --- | --- | --- |
| A | exact page-bounded 24-token KWIC | current deterministic control |
| B | source-aware sentence/TEI structural context | a separately tested segmentation contract |
| C | contextual embedding or LLM-assisted grouping | frozen model/runtime/prompt plus task-specific competent evaluation |

No candidate may see another candidate's output before its own packet is
frozen. No model grouping is a sign, sense, or graph edge by default. Human
work opens only for a small, question-necessary decision set, never because
527 rows exist.

## Admission and stop lines

The usage-context bundle may be materialized because it uses a previously
frozen exact-form control and writes source strings only into ignored local
storage. The tracked receipt must remain source-withholding and public-blocked.

Materialization does not:

- accept, correct, normalize, or publish German;
- establish sentence boundaries or a critical text;
- create an authoritative occurrence beyond the exact witness observation;
- create morphology, lemma, lexeme, translation correspondence, sign
  candidate, sign, concept, claim, relation, graph, or canon state;
- open or modify `initial-sign-packet.v3.json`;
- create routine human work;
- authorize reuse of local source payloads on a future site.

The honest next question after this bundle is not “what does the form mean?”
It is whether the generated contexts close exactly over the source database,
remain source-returnable under independent checks, and provide a useful,
bounded substrate for one later research question.
