# Human-only Translation Pre-draft Worksheet

This is a blank, tracked interface for a future real-human pass. It is not a
review receipt, accepted German, philological analysis, or translation draft.
Completed source text and answers belong in the ignored `local-content/`
route and must later be represented by a schema-valid
`translation-pre-draft-analysis` packet.

## Entry gate

Do not begin until the selected unit has:

- two distinct real-human, source-visible transcription passes;
- an explicit `accept` decision;
- one accepted diplomatic SHA-256 and exact source anchors;
- an admitted rights posture for every source that will be quoted.

Record locally:

- review unit ID:
- accepted source SHA-256:
- source anchor IDs:
- human reviewer ID:
- start time in UTC:

## Blindness attestation

The human completing this worksheet must attest locally that, for this pass:

- no LLM, machine-generated morphology, machine gloss, machine etymology,
  machine interlinear, or machine translation was consulted;
- no AI-only or AI-alternative packet was visible;
- no recognized translation or comparator was visible or consulted;
- no translation draft was written before all nine sections below froze.

If any statement is false, stop. Route the work to a disclosed assisted lane;
never relabel it human-only.

## Nine ordered sections

Complete the sections in this order. Every claim must return to an accepted
source anchor. External claims must cite an admitted entry from
`translation-reference-register.v1.json` with an exact locator, access date,
rights posture, and local evidence note.

### 1. Morphology

- exact form and source anchor:
- proposed lemma:
- inflection and grammatical features:
- uncertainty or alternatives:
- source-return check:

### 2. Syntax

- clause and dependency account:
- punctuation and word-order effects:
- defensible alternatives:
- source-return check:

### 3. Polysemy, unusual forms, and repetition

- ambiguous or unusual forms:
- possible senses without premature selection:
- repeated forms and immediate concordance:
- source-return check:

### 4. Historical German senses

- historical dictionary or corpus reference entry IDs:
- exact locators and dated sense evidence:
- fit, mismatch, or unresolved status for this context:
- source-return check:

### 5. Sourced etymology

- etymological reference entry IDs:
- exact locators and evidence notes:
- proposed relevance to this occurrence:
- rejected attractive but unsupported derivations:
- source-return check:

No etymological statement may be frozen from memory alone.

### 6. Parallels inside *Also sprach Zarathustra*

- occurrence and anchor IDs:
- same form, lemma, image, or proposed sign:
- contextual similarity and difference:
- source-return check:

### 7. Parallels across Nietzsche's corpus

- work, expression, occurrence, and anchor IDs:
- edition or critical-resource reference IDs:
- similarity, difference, and uncertainty:
- source-return check:

### 8. Evidenced allusions, idioms, and cultural links

- proposed link:
- external evidence and exact locator:
- competing explanation:
- status: evidenced, disputed, or unresolved:
- source-return check:

Do not record an attractive association as an allusion without evidence.

### 9. Literal interlinear

- source token or phrase:
- literal Russian rendering:
- grammatical information preserved or lost:
- ambiguity deliberately retained:
- source-return check:

This section is a control layer, not a literary translation draft.

## Freeze check

Before creating a human-only translation draft, record locally:

- all nine sections completed in order;
- every finding's human maker and provenance event;
- every etymology finding's external reference and citation;
- no comparator, other-lane analysis, or draft exposure;
- packet SHA-256 and freeze time;
- final state `frozen-ready-for-lane-draft`.

The user's separate read-aloud and lived-experience layer is recorded only
afterward through its own review route. It must not be used to certify
philological correctness.
