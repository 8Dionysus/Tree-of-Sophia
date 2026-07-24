# Golden-Kernel Transfer Report: Foundation Pilot v1

Status: `blocked-not-run`; no A/B/C output, metric, winner, benefit claim, or
harm claim exists
Experiment: `tos-golden-kernel-transfer-v1`
Frozen plan: `transfer-samples.json`
Decision date: 2026-07-23

## Result first

The current laboratory cannot yet answer whether a reviewed *Zarathustra*
golden kernel makes annotation of other Nietzsche works faster, more accurate,
or more distorted.

Running A/B/C now would create a false comparison. Variant C is defined by
human-accepted *Zarathustra* sign and translation packets. None exists. Giving
C model proposals, empty templates, general contracts, or the recognized
translation instead would silently collapse C into B or contaminate it with an
unreviewed interpretation. The resulting numbers could be reproducible and
still answer the wrong question.

The valid result is therefore a stopped result before execution:

- retain the exact A/B/C method;
- retain three out-of-kernel title-page boundaries as scouting evidence only;
- reject those pages as semantic-transfer evaluation units;
- admit no sign, translation, concept, relation, or ontology from
  *Zarathustra* into another work;
- wait for real source-visible human gold on both the kernel and the target
  texts.

This is not evidence that the golden-kernel idea succeeds or fails. It is
evidence that the current foundation can distinguish a prepared experiment
from an experiment that has actually become epistemically runnable.

## Entry-gate evidence

| Gate | Required before transfer | Current evidence | Decision |
| --- | ---: | ---: | --- |
| accepted German source-review units | 30 | 0 / 30 | blocked |
| double-source-visible kernel gold units | 15 | 0 / 15 | blocked |
| real-human-accepted kernel sign packets | at least 1 | 0 | blocked |
| real-human-accepted kernel translation packets | at least 1 | 0 | blocked |
| double-source-visible content-bearing target gold | 20 | 0 / 20 | blocked |
| target excerpts frozen before variant output | 20, including 10 random and 10 hard | 0 | blocked |
| real-human transfer review receipts | required | 0 | blocked |

The 30-unit source count comes from the frozen translation gate. All four
translation lanes remain `blocked-on-source-acceptance`, and the recognized
Antonovsky witness remains sealed. `gold-status.json` separately contains 15
candidate pages and zero `human_double_checked` units. A validator cannot
convert either zero into evidence.

## Frozen plan and provenance

The Tree-owned plan is:

```text
ToS/source-witnesses/works/friedrich-nietzsche/
  also-sprach-zarathustra/gold-sets/foundation-pilot-v1/
    transfer-samples.json
```

Its SHA-256 is
`26725f80cfd469c45393f956505a1bbf43401cbe2767d5fee84e40759334f7fa`.
The separate `transfer-provenance.jsonl` event binds that plan to the frozen
general sample plan, current manual-gold status, and current translation gate.

The stack-owned A/B/C specification remains at
`mechanics/inference-pilots/parts/tos-foundation-lab/examples/tos-foundation-suite.v1.json`.
At this decision point its SHA-256 is
`fd82bbd29afc1e398f83bdd56b0310043add342d6ee7da4bd6976c5d95db7bad`.
It now explicitly requires a satisfied kernel gate, content-bearing target
units, identical runtime conditions, target-gold sealing, and
human-accepted—not model-only—kernel packets.

## What the current cross-work samples actually are

| Sample | Page | Provisional work boundary | What it can support | What it cannot support |
| --- | ---: | --- | --- | --- |
| `tos-sample-mysl-p238` | 238 | *Beyond Good and Evil* | locate a title-page boundary in the acquired collection item | sign, semantic, relation, or transfer judgment |
| `tos-sample-mysl-p407` | 407 | *On the Genealogy of Morality* | locate a second title-page boundary | sign, semantic, relation, or transfer judgment |
| `tos-sample-mysl-p631` | 631 | *The Antichrist* | locate a distant third title-page boundary | sign, semantic, relation, or transfer judgment |

All three units are `model_source_visible`, not human accepted. They are full
title pages, not philosophical passages, and the three named works have not
yet been promoted into standalone ToS Work records. Their tracked labels are
therefore provisional routing labels, not a completed bibliographic catalog.

The executed retrieval baseline found literal other-work title pages at rank
1. Its own report states the limit directly: it supplied no work-membership or
relation reasoning. That observation proves only that an exact title can be
retrieved. It says nothing about whether a *Zarathustra* sign transfers to the
other text. Structure and OCR runs likewise provide page and text-layer
mechanics only.

## Frozen A/B/C comparison

| Variant | Inputs | Critical exclusion |
| --- | --- | --- |
| A — no kernel | task instruction plus the exact target excerpt | no ToS contracts, examples, or *Zarathustra* packets |
| B — method only | A inputs plus general contracts and content-free neutral format examples | no *Zarathustra* signs, translations, claims, category names, or semantic examples |
| C — reviewed kernel | B inputs plus independently human-accepted *Zarathustra* sign and translation packets and explicit non-transfer warnings | no unreviewed/model-only packet and no recognized translation used as truth |

All variants must receive the same target excerpt, task, model revision,
runtime, prompt shell, decoding, timeout, and resource class. Variant ordering
is randomized behind blind labels. The target gold and target-specific error
rubric stay sealed until every output and digest freezes.

The test must permit abstention, rejection of every inherited category, and
creation of a new target-specific sign. Otherwise C is rewarded for imposing
the kernel even when the target text resists it.

## Target sample required before execution

The three title pages remain scouting units. A future evaluation sample must
freeze at least 20 content-bearing passages from *Beyond Good and Evil*, *On
the Genealogy of Morality*, and *The Antichrist* before any variant output:

- 10 randomly selected content passages across the three works;
- 10 hard passages selected for ambiguity, rhetoric, page boundaries,
  difficult relation claims, and plausible but unsupported resemblance to a
  *Zarathustra* sign;
- ordinary prose, dense or dialogic prose, rhetorical or metaphorical prose,
  page-boundary cases, and hard negatives for kernel signs;
- at least one passage in which a target-specific category is more adequate
  than any existing kernel sign;
- two independent source-visible human passes for transcription, boundaries,
  target annotation, and the error rubric.

Selection must not prefer passages merely because they resemble
*Zarathustra*. Replacing a difficult passage after seeing output invalidates
the comparison.

## Measurements and current result

| Required measure | Measurement rule | Current value |
| --- | --- | --- |
| annotation speed | end-to-end tasks per minute under equal runtime conditions | not measured |
| annotation accuracy | blind comparison against double-checked target gold | not measured |
| manual correction | real-human repair/rejection minutes per unit | not measured |
| traceability | accepted proposals resolving to exact target anchors and evidence | not measured |
| hallucinated relations | unsupported proposed relations divided by all proposed relations | not measured |
| reusable-sign utility | accepted reused signs that reduce correction without suppressing target-specific signs | not measured |
| ontology imposition | unsupported imported *Zarathustra* categories divided by imported categories | not measured |
| machine cost | prompt tokens, wall time, peak memory, swap, energy receipt where available, and derived bytes | not measured |

No zero in this table may be substituted for “not measured.” Zero would be a
quality result; absence is an evidence state.

## Ontology-imposition audit

The manual ledger for each target unit must distinguish:

1. a supported reuse of an accepted kernel sign;
2. a lexical resemblance with a different target sense;
3. an unsupported imported sign;
4. an unsupported relation between otherwise valid signs;
5. a forced mapping where the target requires a new sign;
6. a target-specific sign omitted because the kernel context crowded it out;
7. a defensible abstention;
8. a genuinely useful method transfer—IDs, anchors, provenance, review
   states—without semantic category transfer.

Report aggregate improvement and damage separately for structure, retrieval,
sign proposal, relation proposal, and translation assistance. A gain in
format compliance cannot compensate for semantic overfit, and raw speed cannot
compensate for added human correction.

## Promotion decision

- **Promote method only:** the A/B/C boundary, source gates, blind review law,
  metrics, non-transfer warnings, and the right of the target text to produce
  new categories are durable laboratory method.
- **Retain with limits:** the three title pages remain source-addressable
  scouting boundaries for structure and retrieval.
- **Block:** all semantic transfer, sign reuse, translation reuse, relation
  reuse, benefit claims, harm claims, and backend winner claims.
- **Reject as invalid:** any run that uses the current title pages as semantic
  targets, uses model-only packets as C, treats the recognized translation as
  gold, or reports missing human measurements as zero.

## Re-entry condition and honest conclusion

The experiment may move from `blocked-not-run` to `ready` only when all entry
counts in `transfer-samples.json` are satisfied by resolvable receipts and the
20 target passages are frozen. The stack preflight and `abyss-machine`
resource/thermal owner must then admit each sequential run; this plan grants
no permission to bypass either owner.

As of this report, the usefulness and danger of *Zarathustra* as a golden
kernel are both **unknown**. What is established is the route by which either
a positive or a negative answer can later become credible without forcing
Nietzsche's categories onto the rest of philosophy.
