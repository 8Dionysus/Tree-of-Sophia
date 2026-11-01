# The philosophical core of a bonsai

| | |
| --- | --- |
| Version | 1, 2026-11-01 |
| Instance | [`zarathustra/core/`](zarathustra/core/) — *Also sprach Zarathustra* |
| Posture | a stand's proposal; read by the stand, exported as candidates, never written into `ToS/` |
| Shaped by | [Knowledge model](../ToS/doctrine/KNOWLEDGE_MODEL.md), [Knowledge assessment](../ToS/doctrine/KNOWLEDGE_ASSESSMENT.md), [Common core contract](../ToS/doctrine/COMMON_CORE_CONTRACT.md), [Golden growth kernel](../ToS/zarathustra/GOLDEN_GROWTH_KERNEL.md), the entity and relation registries of [`semantic-interchange/`](../ToS/doctrine/semantic-interchange/) |

The core measures the thought of one work. For every thought it holds the place
where the work thinks it, the voice in which it is thought, the other thoughts it
holds together with, how it changes from one part of the work to the next, and
what the strongest counter-reading changes. The work's own sentence stands at the
centre of each thought; own words are written only for what a place cannot say by
itself.

The core grows from the invariants of the work towards its contested readings.
What the text settles — who speaks, to whom, in which scene, with which words —
comes first and is checked first. Concepts are drawn out of the thoughts the work
states; no thought is fitted to a concept prepared in advance. Once the stable
layer is gathered, the differences between readings become visible, and they too
form a finite set of kinds.

## Principles of a node

Confirmed by Dionysus on 2026-10-31.

1. **The work's formula is the centre.** A node is carried by the strongest
   sentence the work gives it, quoted with its place and its speaker. Beside it
   stands one line of ours: the thesis that sentence affirms.
2. **The question belongs to the Tree, the answer to the work.** A node asks a
   question about human life that can be put to any author; the answer is the
   work's, with its places. The question is the continuity criterion by which
   other works' conceptions join the same crosscutting concept.
3. **Dimensions grow out of the work.** A node's aspects are those the work opens
   (the earth, the will, the future for the overman), never a template of facets.
4. **Links are named by verbs.** A link says what happens at its place: *meets,
   recognises, mocks, saves, follows, becomes, teaches, objects, blesses*. The
   verb is the work's predicate; a registry relation is added only where it fits
   exactly.
5. **The antithesis is spoken by a voice.** A thought against the node is held as
   a position in the voice of the figure who speaks it, with the work's reply.

A transformation is a **chain**: steps with a verb and a place at every step.

**Affirmative writing.** A thought is stated as it is affirmed. A negation is
written where the work itself negates, and then in the work's words (the lion's
*heiliges Nein* in I.02, Zarathustra's rebuke of the animals' *Leier-Lied* in
III.13).

## Layers

| Layer | Holds | Settled by |
| --- | --- | --- |
| 0. Places | the witnesses, their units and alignment ([`witnesses`](zarathustra/witnesses.json)) | the source |
| 1. The work's world | voices, figures, scenes, occurrences, refrains | the text, read paragraph by paragraph; disputes concern boundaries |
| 2. Thought | crosscutting concepts and the work's conceptions, formulas and theses, aspects, positions, images, problems and questions, motifs | reading, with places |
| 3. Links | claims between the objects of layers 1–2, and chains | reading, with places on both ends |
| 4. Assessment | the maker's proposal, the independent verdict, the stand's admission | an independent reviewer and the stand |
| 5. Views | node tabs, routes, lenses, export | derived; carries no significance of its own |

Size, brightness and position on screen add nothing to a claim's weight. A
nuance is made visible in proportion to what it changes in understanding, a
decision or the next step.

## Objects

Every object names its places as citations `{w, c, u:[first, last], q?}`: units
of one chapter in sequence, `q` the exact words of the German witness where a
reader needs them. The core holds no copy of the work's text; the stand shows the
witness at the cited place.

| Object | Minimal data | Invariants | Tree type |
| --- | --- | --- | --- |
| Voice segment | chapter, first and last unit, speaker, addressee, mode (`speech`, `song`, `narration`, `dream`, `quoted`), layer (`primary`, `embedded` with `via`) | every unit of a chapter lies in exactly one primary segment; embedded speech names the voice that carries it | attribution basis |
| Figure | id, names with forms and counts, existence (`work-world`), appearances with role (`speaks`, `acts`, `spoken-of`, `addressed`) | a name is a designation; the figure, the author and any historical bearer of the name are distinct referents | thought-image (open question 1) |
| Occurrence | place, form, speaker, use group, a reading line where one was written | the form is found at the place; the use group is a reading, revisable | occurrence |
| Motif | 2–8 occurrences or places, signification, grouping basis, scope, contrast, limits | the contrast is stated: a repeated scene may end otherwise | motif-proposal |
| Crosscutting concept | id, title, question | the question names a subject any author can address | crosscutting-concept, question |
| Conception | formula (place, speaker, mode), thesis, answer, dimensions, development (emergence, movement, transformation), function | the formula is the work's sentence in the voice that says it; the thesis is ours and says so | conception, thesis, aspect |
| Position | speaker, formula, the concept it stands against, the work's reply with places | attributed to its speaker; the work's reply is cited, never supplied | position, objection |
| Image | id, places, the conceptions it presents | an image presents; what it presents is a reading | thought-image, metaphor |
| Question | text, the work's answer with places, status | a question the work asks itself keeps its own words | question |
| Link | from, verb, to, relation?, sense, grounds, order, path, status, counter-reading?, question? | grounds show both ends; order 1 at one place, 2 through a neighbouring place or repetition, 3 through an intermediate object named in `path` | relation claim |
| Chain | title, steps `{from, verb, to, cite}`, status | a place at every step; a chain composes only through objects it names | path of claims |
| Assessment | target, maker, reviewer, verdict, reason, revised fields, counter-reading, attention | the reviewer reads the places, not the maker's reasoning | assessment |

## Subject, claim and representation

- **The place** is what the work says: the witness's text at a stable address.
- **The claim** is what the core says about it: a line with its status, maker and
  assessment.
- **The representation** is how the stand shows it: tabs, routes, lenses.

A formula is a place; a thesis, a sense, an answer and a reading line are claims;
a route is a representation. A claim never stands in for its place.

## Statuses

| Status | Values | Owner |
| --- | --- | --- |
| claimed | `text` — the work itself joins the two at the named place; `reconstruction` — the link holds through several places, named; `hypothesis` — offered for testing | the maker |
| verdict | `confirmed`, `reconstruction`, `revise` (with the corrected verb, sense or grounds), `rejected` (with what the places say instead), `undecided` | the independent reviewer |
| admission | `map` — shown as a link of the work; `held` — kept with its reason, shown on request; `rejected` — kept with its reason | the stand |
| attention | `high` — changes the understanding of a node or leads further through the work; `normal` | the reviewer |

An absent verdict is `undecided` and is shown as such. A `revise` verdict admits
the revised claim, and the maker's version stays in the record. Rejected and held
claims remain part of the core: a gold example keeps accepted, rejected,
ambiguous and counter-read cases together with their reasons.

## Admissible inference

| From | Allowed | Requires more | Never |
| --- | --- | --- | --- |
| the same word at two places | an occurrence pair; a motif proposal with a contrast | a link, with a reading of both places | one teaching (Zarathustra's return to men in the Vorrede and the eternal return) |
| two neighbouring places | a link of order 2 | a link of order 1 | influence or cause |
| a sentence in a figure's voice | a position of that figure | the work's endorsement, from the work's reply | the work's thesis (the dwarf's *die Zeit selber ist ein Kreis*, III.02) |
| embedded speech | a position of the inner speaker | its adoption by the carrier | the carrier's own thesis (the animals' *Ewig rollt das Rad des Seins*, III.13) |
| a later place changing an earlier thought | `conception-expands`, `-redefines`, `-narrows` with both places | the author's rejection of the earlier thought | a silent replacement of the earlier claim |
| a repeated scene | a motif with both outcomes | an identity of the figures | the outcomes merged (the shepherd's bite and laughter, III.02; Zarathustra's bite and exhaustion, III.13) |
| a chain | a path through named places | the composed claim of its ends | a composition through an unnamed intermediate |
| a verb of the work | the link's predicate | a registry relation, where its definition fits | a meta-relation by use alone |
| a negation | the work's negation in its words | — | a negation of ours standing for a thought |

## Operations

| Operation | Who | In | Out | Refusals |
| --- | --- | --- | --- | --- |
| **map voices** | maker | the aligned text, chapter by chapter | voice segments | a unit in no segment; a speaker named by guess instead of by the text's cue |
| **grow** | maker | voices, a seed list, the word index | node files | a claim with no place; a retelling instead of a citation; a citation that does not resolve |
| **verify** | independent reviewer, in batches per node | cards without the maker's reasoning | verdicts | a verdict without a reason at a place |
| **apply** | the stand | proposals and verdicts | core modules with admission | a `confirmed` shown for an undecided claim |
| **check** | `bonsai/lib/core.mjs` | the core | pass or the failing claims | listed under *Checks* |
| **show** | the stand | the core | tabs, routes, lenses | significance carried by size or brightness |
| **export** | `bonsai/lib/export.mjs` | admitted claims | a candidate packet, `unreviewed` | a write into `ToS/` |

The maker and the reviewer are recorded with model, effort, brief and date. A
reviewer's competence is shown by reading: its verdicts cite places, and its
revisions pass the citation check.

## Checks

The check (`node bonsai/lib/cli.mjs check`) reads the core and fails when:

- a citation does not resolve in the witnesses, or its units are out of sequence;
- a `q` is not found in the cited German units;
- a link lacks a verb, a status or grounds; an order-1 link's grounds do not cover
  a place of each end; a `path` names an unknown object;
- a chain step lacks a place;
- a registry relation is unknown or outside its domain and range;
- a voice segment overlaps another primary segment of its chapter, or a figure
  speaks where the voice map gives another speaker;
- an admitted claim has no verdict, or a `map` claim's verdict is `rejected`;
- a control vector fails.

## Control vectors

[`zarathustra/core/control.json`](zarathustra/core/control.json) holds cases the
core must keep: an input, the expected state and what is forbidden. Each case
names its places by chapter and exact words, so that it can be read without the
core. The check runs every case.

| Case | Expected | Forbidden |
| --- | --- | --- |
| *die Zeit selber ist ein Kreis* (III.02) | a position of the dwarf against the recurrence, with Zarathustra's rebuke *mache dir es nicht zu leicht* | a formula of the recurrence in Zarathustra's voice |
| *Ewig rollt das Rad des Seins* (III.13) | the animals' song, embedded, with Zarathustra's reply *Oh ihr Schalks-Narren und Drehorgeln* | Zarathustra's thesis |
| the shepherd's bite (III.02) and *biss ihm den Kopf ab* (III.13) | a motif with two outcomes | an identity of the shepherd and Zarathustra stated as text |
| *heiliges Nein* and *heiliges Ja-sagen* (I.02) | the lion's No wins freedom; the child's Yes begins creating | the child's Yes standing without the lion's No before it |
| *Alles ist leer, Alles ist gleich, Alles war!* (II.19) | a position of the soothsayer against affirmation | a thesis of the work |
| *Zarathustra will wieder Mensch werden* (I.01) and the recurrence | a comparison of two returns | one teaching |

## Acceptance

The core is accepted for the stand when:

1. **Routes.** A reader can walk, on admitted links only:
   - *who are Zarathustra's brothers* — disciples, *Brüder im Kriege* (I.11),
     children, the higher men — and on to the gift, gold and giving back (*wie
     kam Gold zum höchsten Werthe?*, I.23);
   - the network of blessing — overflow, address, reception, the capacity to be
     touched, the receiver's own way, the giver's fatigue (*Oh Unseligkeit aller
     Schenkenden*, II.09), the blessing of things (*segnendes Ja-sagen*, III.04),
     the sacrifice called a ruse (IV.01) — without collapsing it into exchange;
   - the shepherd and Zarathustra — one scene, two outcomes.
2. **Depth.** For each `high` link a reader can say why these places are joined,
   what distinction the link makes, and what the strongest counter-reading
   changes.
3. **Coverage.** Every node has a formula with its voice, a thesis, a question and
   the work's answer; every link has a verdict; every figure has its appearances
   in the voice map.
4. **Check.** The check passes and every control vector holds.

## Stages

| Stage | State on 2026-11-01 |
| --- | --- |
| voice map | 81 chapters, 781 segments (maker: GPT-6.1 Sol Max) |
| figures | 33 (Sol Max) |
| concepts | 30 (Sol Max): 188 links, 60 chains, 30 formulas |
| verification | independent (GPT-6.1 Terra xhigh), batched per node; in progress |
| assembly | `bonsai/review/zarathustra-graph/core.mjs`, from proposals and verdicts |
| stand | node tabs read the core; routes walk admitted links |
| other works | Nietzsche's other works after *Zarathustra* |

## Open questions

1. **A figure of a work's world.** The registry has no type for a figure that
   exists within a work (`figure` anchors historical philosophers). The core
   types it as an image of thought and records its existence as `work-world`.
2. **Attribution within the work.** `thought-attributed-to` ranges over agents and
   organisations; a position in the dwarf's voice has no registry relation. The
   core keeps it as `voiced-by`, a proposal.
3. **Links between places.** One place repeats, varies, answers or echoes another;
   the registry has no relation for these, and the core keeps them as the work's
   verbs with both places.
4. **Families of verbs.** Each verb of the work belongs to one of the families of
   meta-relations of the subject model (identity and designation, attribution and
   role, event and change, similarity and contrast, grounds and conflict, …). The
   assignment is a separate pass.
5. **Occurrences in the Tree.** A motif proposal of the Tree needs Occurrence
   records; the core's occurrences are places of a form and become Occurrences
   through the Tree's own route.
6. **Use groups.** The maker's use group of an occurrence (teaching, image, song,
   question, narration) follows the voice mode; a reading of the use proper is a
   further pass.
