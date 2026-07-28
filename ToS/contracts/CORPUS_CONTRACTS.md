# Corpus Foundation Contracts

These contracts make the corpus evidence spine mechanically exchangeable.
They do not make a bibliographic, textual, translation, semantic, or rights
judgment true.

## Contract family

| Contract | Owns |
| --- | --- |
| `corpus-record.schema.json` | persistent agent/work/expression/edition/collection/item catalog identity |
| `source-item-manifest.schema.json` | immutable local payload inventory, digest, and tracked companion refs |
| `source-anchor.schema.json` | structural, quote, position, and page-region selectors tied to one file digest |
| `provenance-event.schema.json` | acquisition and transformation entity/activity/agent trail |
| `rights-record.schema.json` | researched rights, permission, visibility, and redistribution posture |
| `material-discovery-record.schema.json` | exact ordered queries, result order, originating-record links, declared-rights evidence, acquisition/snapshot posture, and channel cost comparison |
| `access-request.schema.json` | public-safe request scope, institutional contact route, separate permission purposes, private-correspondence boundary, response/expiry state, and no-bypass law |
| `server-import-contract.schema.json` | future item/file manifest handoff, checksum and rights gates, access class, derivative matrix, operator approval, publication/takedown state, and server non-authority |
| `private-laboratory-evidence-handoff.schema.json` | exact private-raw custody boundary, public-safe aggregate allowlist, reconstructive-detail denylist, governed destination, separate creation/publication effects, and human publication gate |
| `sign-annotation.schema.json` | occurrence-to-concept sign ladder without layer collapse |
| `claim-packet.schema.json` | evidence-bearing assertion, alternatives, lineage, and human review state |
| `translation-packet.schema.json` | source-accepted lifecycle packet binding independent frozen analyses to human, AI, machine-alternative, and AI+human drafts before comparator reveal, comparison, change tracking, and human adjudication |
| `translation-laboratory-plan.schema.json` | exact source-first 17-stage translation order, real-human lane law, sealed comparator, model-candidate posture, and source-acceptance gate |
| `translation-reference-register.schema.json` | dated dictionaries, corpora, critical editions, lexical resources, and translation witnesses with separate scholarly, access, rights, citation, and admission posture |
| `translation-pre-draft-analysis.schema.json` | source-accepted, comparator-blind morphology-to-interlinear evidence packets kept independent for real-human, AI-only, and machine-alternative lanes |
| `semantic-ladder-packet.schema.json` | exact source-accepted form-to-graph sequence with honest empty blocked stages, translation evidence posture, a real-human sign decision, competing readings, and a disposable projection boundary |
| `golden-kernel-transfer-plan.schema.json` | source-gated cross-work A/B/C plan, title-page scouting boundary, content-bearing target-gold requirements, contamination controls, transfer metrics, and an honest non-run result |
| `source-gated-evaluation-plan.schema.json` | fail-closed semantic-annotation and LLM-assistance task plans requiring accepted source, real-human gold, 10 random plus 10 hard content tasks, blind variants, and proposal-only model authority |
| `laboratory-sample-plan.schema.json` | source-balanced frozen sample units, strata, anchors, and gold candidates |
| `ocr-visual-sample-plan.schema.json` | output-blind 3x12 visual OCR projection, shared render law, sealed reference witnesses, and an explicit human-gold gate |
| `manual-gold-status.schema.json` | model-draft and two-pass source-visible human gold state without relabeling |
| `manual-gold-assurance.schema.json` | additive solo+AI assurance ladder, trigger-gated sparse calibration, zero-human-debt closure, delayed same-human stability check, language-competence boundary, and digest-bound legacy lineage |
| `translation-sample-plan.schema.json` | thirty frozen German fragments, sealed comparator, staged lanes, and etymology law |
| `translation-source-review-plan.schema.json` | v2 page-triplet source review routing after selector failure, without reusing rejected automatic text |
| `german-assisted-source-review.schema.json` | solo+AI evidence lanes, visual-only competence boundary, critical-edition witness route, triggered 1-3 unit scheduling, and fail-closed translation consequences |
| `critical-edition-witness-admission.schema.json` | one exact critical-edition locator, reference identity, provenance, content non-capture, rights review, and fail-closed effects before citation-witness admission |
| `retrieval-query-plan.schema.json` | frozen query intents, languages, expected anchors, hard negatives, and local-only query-content digest |
| `graph-query-plan.schema.json` | frozen four-layer graph questions, allowed predicates, claim-set digest, and unreviewed expectations |

## Common laws

- ToS IDs persist across path changes and are never reused.
- Repo-relative refs and local payload paths do not contain absolute paths or
  parent traversal.
- Every source-bearing record cites an exact file digest or an anchor that does.
- Every automated action identifies software/model/configuration through a
  provenance event or receipt.
- Earlier transcription, correction, translation, annotation, and claim
  versions are superseded, not overwritten.
- Review states retain rejection, ambiguity, deferral, and counterevidence.
- Rights/visibility constraints travel into derivatives and projections.
- Search availability never settles rights; discovery, request, acquisition,
  server import, and publication remain separate events.
- Search and graph exports never satisfy the source-evidence fields by
  pointing back only to themselves.

## Validator restraint

A schema validator can establish that:

- required identities and references are present;
- a digest has the expected syntax;
- an anchor offers declared selectors;
- a transformation names inputs and outputs;
- a claim includes layer, evidence, maker, and review posture;
- a translation packet permits honest preparation without fabricated final
  evidence, binds every frozen draft to its own blind pre-draft packet, requires
  at least two machine alternatives, freezes human-only and AI-only work before
  AI+human collaboration, seals recognized witnesses until all blind drafts are
  frozen, and preserves post-reveal changes, rejected alternatives, and
  real-human adjudication as separate records;
- a sample plan contains the declared number of source-balanced units and gold
  candidates frozen before outputs;
- an OCR projection resolves every visual page to an exact item/file/anchor,
  keeps its one pre-output nonvisual replacement explicit, and does not claim
  human gold or formal quality metrics;
- a legacy gold-status packet keeps model drafts separate from both human
  passes;
- a manual-gold assurance packet keeps single-human, delayed same-human, and
  independent multi-human evidence distinct, binds its method and frozen
  inputs by digest, prevents visual-only language review from authorizing
  textual or semantic claims, and partitions every frozen unit into selected
  calibration or explicitly unscheduled work without turning packet size into
  human debt;
- a source-review v2 plan closes each page triplet over the rejected v1
  evidence while keeping candidate reuse, comparator visibility, and human
  acceptance false;
- a German assisted-review plan keeps visual human evidence, critical-edition
  evidence, independent machine candidates, AI explanations, and
  language-competent acceptance distinct; machine agreement cannot supply
  competence or create a human-only translation;
- a translation-laboratory plan preserves the exact workflow order, keeps all
  four draft lanes blocked before source acceptance, and forbids comparator
  consultation while sealed;
- a translation-reference register covers every required source category,
  resolves local witness identities, prepares contact routes for restricted
  resources, and keeps all reference content unadmitted before human
  bibliographic and rights review;
- a blind pre-draft packet requires two real-human source-acceptance passes,
  preserves the nine morphology-to-interlinear stages in order, rejects AI
  assistance or machine-authored findings in the human-only lane, rejects
  human editing in model lanes, keeps comparators and other lanes hidden, and
  requires cited external evidence for etymology;
- a semantic-ladder packet cannot begin without accepted source evidence and
  keeps relations, concepts, counterreadings, and graph projection blocked
  until its sign candidate receives an attested real-human decision whose
  status agrees with accept, reject, ambiguity, or deferral; a graph-projected
  packet cannot skip competing readings or claim its projection as authority;
- a golden-kernel transfer plan keeps title-page scouts ineligible for
  semantic evaluation, requires real-human kernel and target gold before a
  ready state, preserves exact A/B/C isolation, and cannot report runs,
  metrics, a winner, or promotion while `blocked-not-run`;
- a source-gated evaluation plan cannot materialize semantic or LLM tasks,
  runs, metrics, a winner, or promotion while accepted source and
  double-checked human gold remain absent;
- a discovery record preserves exact queries and ranked results while keeping
  declared rights as evidence rather than a ToS conclusion;
- an access-request record cannot label a draft as sent or granted without
  real-human send approval and the corresponding private/redacted evidence
  boundary;
- a server-import plan resolves exact manifest, file, and rights digests,
  blocks payload transfer for deny/metadata-only access, and requires verified
  bytes, reviewed rights, real-human operator approval, and a receipt before
  `imported`;
- a private-evidence handoff freezes its audience, exact destination,
  aggregation threshold, prohibited disclosure classes, raw-preservation law,
  and publication gate before private evidence is opened;
- a graph plan binds ten pre-output questions to one exact claim-set digest and
  keeps all four logical layers explicit.

It cannot establish that:

- two catalog records describe the same historical object;
- metadata, OCR, segmentation, lemma, etymology, or alignment is correct;
- a translation is faithful or philosophically adequate;
- a rights determination is legally sufficient;
- a semantic claim or relation should enter canon.
- a same-human delayed recheck is independent of memory or equivalent to
  agreement among different reviewers;
- a declared language competence is actually sufficient for the reviewed
  material.

Those require source-visible human review and a reasoned decision record.

## Versioning

The first family uses `v1`. A backward-incompatible change receives a new
schema version and migration evidence. Tightening a validator around an
unstated assumption is a contract change even if the JSON filename stays the
same.
