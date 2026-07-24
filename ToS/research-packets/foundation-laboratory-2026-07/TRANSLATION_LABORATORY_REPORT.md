# Translation Laboratory Report: Foundation Pilot v1

Status: source-review laboratory prepared and mechanically exercised;
translation A/B/C not started
Experiment: `tos-translation-foundation-v1`
Report date: 2026-07-23

## Result first

The laboratory now has a reproducible, source-first route for comparing
human-only, AI-only, multiple AI alternatives, AI+human collaboration, and a
recognized translation witness without mixing their evidence. It does not yet
have accepted German input, a translation draft, a comparison score, or a
winner.

The first automatic source selector failed source-visible inspection and was
rejected. The successor v2 review interface was then frozen, materialized, and
checked against the exact source scan. It contains all 30 blind review units
and keeps the recognized Antonovsky witness sealed, but every human field is
blank. The stack gate therefore exits `2` and keeps every translation lane
blocked.

This is the correct current translation result: the method and human work
surface exist; translation evidence does not.

## Evidence sequence

### 1. Frozen v1 source candidates

Thirty German candidate fragments were selected before translation output.
They were bound to exact container members and proposed anchors. Automatic
EPUB OCR was declared a derivative, never source truth, and the recognized
Russian witness was sealed.

An exhaustive source-visible model inspection then classified all 30
candidates:

| Advisory decision | Units | Meaning |
| --- | ---: | --- |
| `accept-with-limits` | 7 | structurally plausible only after source correction or boundary work |
| `reject` | 22 | heading, page-start tail, or visible automatic-text contamination |
| `uncertain` | 1 | boundary could not be settled from the available context |
| real-human source acceptance | 0 | no human pass occurred |

The v1 selector method was rejected. Its candidate strings remain preserved
as negative evidence and are forbidden as v2 input.

### 2. Blind v2 source-review interface

The successor plan freezes 30 new review-unit identities and 89 unique
previous/current/next source-scan pages. It does not emit the v1 candidate,
automatic German, or comparator text.

| Evidence | Value |
| --- | --- |
| plan SHA-256 | `ba04adba9787c91ffca017128db0250f5565663084ba7f60a609e227b88eaf25` |
| packet status | `awaiting-real-human-source-review` |
| packet files / bytes | 94 / 45,034,156 |
| review units | 30 |
| unique rendered source pages | 89 |
| packet manifest SHA-256 | `8db6f88131355e858b861696899ae5edc37b238ec31493d898c559153010850c` |
| blank review JSONL SHA-256 | `cebb91dde2ae3c796d173734ea7d5bc095332bba15cacc72193700755a6b582a` |
| human pass 1 / pass 2 / accepted units | 0 / 0 / 0 |

Independent checks established exact page-set closure, no v1 complete
candidate strings, no comparator identifiers, and no hidden human
attestation. Three distant source pages were opened and inspected visually for
correct routing. These checks were performed by `model:codex`; they verify the
interface, not the German transcription.

### 3. Reference and pre-draft soil

`translation-reference-register.v1.json` contains 14 candidates covering all
nine required categories: dictionaries, corpora, critical-edition resources,
Nietzsche-specific lexical resources, and recognized translation witnesses.
It keeps scholarly role, access, rights, citation, contact, and content
admission separate.

Current admission counts are deliberately zero:

- admitted reference content: 0;
- human bibliographic reviews: 0;
- human rights reviews: 0;
- permission requests sent: 0.

The pre-draft contract fixes nine independent stages before any translation:
morphology, syntax, polysemy/unusual forms/repetition, historical German
senses, sourced etymology, *Zarathustra* parallels, Nietzsche-corpus
parallels, evidenced allusions/idioms/cultural links, and a literal
interlinear. Etymology requires cited external evidence; model memory is not a
source.

No real pre-draft packet exists because accepted German is 0 of 30.

## Frozen translation lifecycle

`translation-laboratory-plan.v1.json` has SHA-256
`9a260fb4a21c44a67b56d2887ebba4d01d82c49c5d3008b39ed44d4779145ee4`.
It freezes 17 stages and 19 evaluation axes. The lifecycle is:

```text
two-pass accepted German source
  -> independent human / AI / AI-alternative pre-draft analyses
    -> blind human-only draft
    -> blind AI-only draft
    -> at least two blind AI alternatives
      -> frozen human + AI inputs permit AI+human draft
        -> all five blind drafts freeze
          -> recognized comparator reveal
            -> comparison and post-reveal change ledger
              -> human adjudication with rejected alternatives preserved
```

The real-human-only lane forbids AI assistance and machine-authored findings.
Model lanes forbid hidden human editing and require model, prompt, runtime, and
provenance receipts. The AI+human lane cannot begin until independent human
and AI work is frozen. The recognized translation remains a witness rather
than ground truth. Personal read-aloud interpretation remains a separate
authored layer and cannot substitute for philological source acceptance.

## Gate executions

The stack-owned gate was exercised against the actual private v2 interface:

| Input | Records | Human pass 1 | Human pass 2 | Accepted German | Decision / exit |
| --- | ---: | ---: | ---: | ---: | --- |
| no review output | 0 | 0 | 0 | 0 | `blocked` / `2` |
| immutable blank template | 30 | 0 | 0 | 0 | `blocked` / `2` |
| blank template plus validated reference register | 30 | 0 | 0 | 0 | `blocked` / `2` |

The third run additionally reports the three pre-draft analysis lanes
(`human_pre_draft_analysis`, `ai_pre_draft_analysis`, and
`ai_alternative_pre_draft_analysis`) as present but blocked. Thirty valid blank
rows therefore do not become thirty human decisions merely because the packet
and schema are green.

## Quality, speed, and cost

### Quality

No semantic-fidelity, adequacy, omission/addition, etymology, rhythm,
pronounceability, or comparator-influence score exists. No recognized
translation comparison exists. No translation draft exists.

### Speed

The only measured translation-related speed is interface materialization, not
translation speed: the owner service ran for 40.706 seconds to create and
verify the v2 packet. Translation tokens per second and human translation time
are not measured.

### Machine and storage cost

The v2 materialization was admitted by `abyss-machine` as `medium/indexing`.
It observed 142.961 MiB peak memory, zero swap, and produced 45,034,156 bytes
across 94 files. The learned next-run estimate is 178.701 MiB; the original
96-MiB estimate must not be reused as observed demand.

No translation model download or new translation runtime was admitted because
the source gate is closed. The resident-first model ordering remains a future
plan, not an execution claim.

### Human cost

Human source-review, pre-draft, translation, comparison, correction, and
adjudication minutes are all unmeasured because no human review has begun.
Missing human time is not zero cost.

## Promotion and rejection decisions

- Reject v1 automatic source selection as translation input.
- Preserve all v1 failures and the first failed manual set-diff query as
  negative evidence.
- Promote the v2 review interface, source gate, independent-lane contract,
  comparator seal, reference register, and lifecycle as laboratory method
  only.
- Block every pre-draft and draft lane until real pass 1 and independent pass
  2 accept all required German units.
- Block recognized-witness reveal until every blind draft is frozen.
- Block all translation quality, speed, cost, winner, and promotion claims.
- Do not treat a validator, model inspection, automatic EPUB text, recognized
  translation, or user's personal reading as source acceptance.

## Next admissible action

A real human copies the blank review JSONL to a new output, completes pass 1
against the visible source pages, and then performs an independent pass 2.
Each acceptance must carry human identity, distinct timestamps, uncertainty,
source boundaries, diplomatic German, and a receipt. Only after 30 final
acceptances may the independent pre-draft lanes begin.

Until then, the honest laboratory conclusion is narrow: Tree of Sophia has a
translation method capable of preserving source, alternatives, etymological
evidence, and comparator influence, but it has not yet translated or accepted
a single unit.
