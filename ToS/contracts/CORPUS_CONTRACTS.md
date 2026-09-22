# Corpus Foundation Contracts

These contracts make the corpus evidence spine mechanically exchangeable.
Source-visible assessment evaluates bibliographic, textual, translation and
semantic judgments; rights and publication retain their actual owner
decisions. The table states each contract’s purpose. Exact status, permission
and admission requirements live in the referenced schema and its owner route.

## Contract family

| Contract | Owns |
| --- | --- |
| `artifact-source-witness.schema.json` / `artifact-source-witness-v2.schema.json` | provider-independent physical-artifact metadata and layer separation; v2 permits an exact unplanted artifact without manufacturing a philosophy-backlog relation |
| `artifact-visual-representation.schema.json` | one exact File-backed visual representation with provider records, payload fixity, rights, acquisition and storage scope |
| `open-work-channel-timing-receipt.schema.json` | external positive per-channel monotonic HTTP transport measurements for the active reviewed open-Work loop, explicitly excluding research, interpretation, rights-review, and human time |
| `corpus-record.schema.json` | persistent agent/work/expression/edition/collection/item identity plus exact outgoing Work→Expression, Expression→Edition, Edition→Item, and optional Expression-derivation claim closure refs |
| `research-corpus-record.schema.json` | persistent research selection, substantive purpose, selection criterion, coverage account and continuity; exact members remain Claims |
| `scoped-member-structure.schema.json` / `source-member-structure-claim.schema.json` | shared bounded typed member set, source scope, explicit coverage and local partial/total precedence, used by distinct intellectual-part, physical-part, research-corpus and Collection-order predicates; Collection order binds exact existing Collection/membership versions without creating membership; no global hierarchy or automatic completeness |
| `semantic-entity-type-registry.schema.json` | versioned stable `tos.entity.*` hierarchy and source-kind crosswalk with explicit lifecycle, owner, human labels, and an unmapped fallback |
| `semantic-relation-type-registry.schema.json` | versioned stable `tos.relation.*` hierarchy and source-predicate crosswalk with domain/range, directionality, cardinality, evidence/review posture, and an unmapped fallback |
| `human-form.schema.json` / `human-form-template.schema.json` | exact subject/source-bound human representation and finite owner-admitted template, distinct from subject identity; mandatory context, source-copy, deterministic rendering and current assessed freeform use follow `ToS/doctrine/HUMAN_FORMS.md` |
| `human-form-set.schema.json` | adjacent exact-subject collection of current human forms and retained predecessors; discovery and metadata-only adapter limits follow `HUMAN_FORMS.md` |
| `source-item-manifest.schema.json` | immutable local payload inventory, digest, and tracked companion refs |
| `source-resource-inventory.schema.json` | text-free PDF or bundled-DjVu page, EPUB member/spine, TEI page-break/division, and provider DjVu/ABBYY OCR-page inventory with geometry, ordering, counts, member fixity, and one-way fingerprints |
| `lexical-index-plan.schema.json` | exact-form observation plan declaring local source-bearing and tracked hash-only outputs, field scope and rights conditions |
| `lexical-index-projection.schema.json` | rebuild receipt and form-hash/count/page/division read model over exact local witnesses, with bounded local query probes |
| `lexical-recurrence-plan.schema.json` | frozen exact-form recurrence question preserving separate A frequency, B structural range and C part-size-aware dispersion observations |
| `lexical-recurrence-projection.schema.json` | deterministic hash-only frequency/range/DP tuples with exact source totals, integer rounding law and residue accounting |
| `lexical-usage-context-plan.schema.json` | one frozen question, preselected exact-form control, complete occurrence census, page-bounded context, composite selectors, local output and tracked disclosure scope |
| `lexical-usage-context-row.schema.json` | private exact usage row with deterministic context/occurrence identity, source state, structural and positional selectors, page-bounded token window and clipping |
| `lexical-usage-context-receipt.schema.json` | text-free fixity/count/selector receipt binding the private context bundle to its question, lexical inputs, local database, generator and rights posture |
| `morphology-evaluation-plan.schema.json` | source-gated historical-German morphology question with an identity control, exhaustive direct-form A census, staged contextual A/B/C follow-up and explicit competence and rights requirements |
| `morphology-input-receipt.schema.json` | text-free fixity and count receipt for the ignored exact-form morphology input packet, binding it to the exact lexical database, tracked projection, plan, and generator without tracking source strings |
| `morphology-contextual-episode-plan.schema.json` | one-question morphology follow-up binding a concrete A ambiguity, complete source recurrence, output-blind selection, B relevance, local context and assessment conditions |
| `morphology-contextual-episode-row.schema.json` | private exact raw-TEI context row with one selected occurrence, composite selectors, target offsets and unchanged historical input |
| `morphology-contextual-episode-receipt.schema.json` | tracked text- and position-free receipt for the private contextual packet, proving trigger, selection, source-return and variant-state closure while B remains unacquired and C question-inapplicable |
| `morphology-contextual-artifact-admission.schema.json` | source- and path-free artifact record preserving private acquisition, rights gaps, resource cost, trust controls and runtime admission, linked to the frozen pre-output plan |
| `morphology-contextual-result-receipt.schema.json` | text-free result of one private contextual B run, binding the question, retained admission history, runtime subject, trust checks, repeatability, provider-label aggregates, cost and local rights posture |
| `witness-structure-correspondence.schema.json` | text-free named-division locator candidates between exact witness inventories, with matching metrics, monotonic routes and provenance |
| `witness-structure-anchor-set.schema.json` | stable proposed TEI, EPUB-member and PDF-page addresses bound to a witness-structure correspondence |
| `numbered-unit-page-map.schema.json` | text-free source-only numbered-unit start-page candidates bound to one exact scan, resource inventories, proposed whole-page anchors and explicit review basis |
| `target-numbered-unit-page-map.schema.json` | text-free target numbered-label start-page candidates bound to one exact translation scan, its work boundary and source-map asymmetries |
| `hierarchical-target-numbered-unit-page-map.schema.json` | target numbering with independently resetting series, series-qualified identity, proposed page starts and machine/model review basis |
| `hierarchical-source-numbered-unit-page-map.schema.json` | source numbering with independently resetting series, exact address/navigation witnesses, proposed page starts and bounded page-relation evidence |
| `hierarchical-numbered-unit-label-correspondence.schema.json` | intersection of independently materialized source and target series:unit labels, binding both maps and their layered rights |
| `parallel-numbered-unit-label-map.schema.json` | intersection of independently materialized number-label keys, retaining both maps, anchors, rights and source-only asymmetries |
| `transfer-candidate-structural-crosswalk.schema.json` | text-free narrowing of frozen whole-page candidates through a target unit-start map and shared-label correspondence, preserving spill ambiguity and exact next-start context |
| `transfer-candidate-target-structural-crosswalk.schema.json` | target-only narrowing through a hierarchical target map, retaining spill ambiguity and explicitly recording the absent source parallel route |
| `transfer-candidate-source-structural-route.schema.json` | composition of target-only candidates with shared hierarchical labels to identify possible German structural routes and their unresolved passage boundaries |
| `transfer-target-passage-candidate-set.schema.json` | private target numbered-unit slices within one exact PDF-bbox layer, with text-free tracked geometry/digests and preserved proposed/rejected intersections |
| `transfer-source-passage-candidate-set.schema.json` | private German numbered-unit slices within named automatic layers, with text-free geometry/digests, unresolved boundaries and explicit address/navigation witness relations |
| `private-transfer-source-visible-review-bundle.schema.json` | private mode-0600 model-source-visible diplomatic candidates over an exact page pair, with payload and render fixity, automatic candidates, critical comparison and explicit maker |
| `transfer-source-visible-review-receipt.schema.json` | text-free return from one private review bundle: input/generator fixity, page-render reproduction, aggregate discrepancy topology, historical/critical context and finding classes |
| `parallel-witness-structure-map.schema.json` | text-free division starts and numbered-unit spans across source and translation PDF witnesses, preserving supplemental-unit asymmetries and the available target-address granularity |
| `collection-work-boundary-map.schema.json` | complete or explicitly partial member-work representation over exact container pages, with Work/Expression/Claim refs, optional responsibility Claims, source order and proposed anchors |
| `source-anchor.schema.json` | structural, quote, position, and page-region selectors tied to one file digest |
| `source-text-layer.schema.json` | immutable role-bearing source-text representation bound to exact Work/Expression/Edition/Item/File and source-anchor-v2 identity, with predecessor and edit/normalization lineage, uncertainty, editorial policy, explicit rights/publication-authority refs, and separate mechanical, review, language-competence, accepted-use, rights, and publication gates |
| `source-text-unit-packet-v1.schema.json` | additive frozen-layer unit and segmentation owner with opaque label-independent scheme/segmentation/unit identities, exact ordered anchor return, distinct layout/source-structure/orthographic/linguistic/model-input kinds, explicit coverage/gaps/overlap/whitespace/punctuation/line-break/hyphenation posture, reciprocal alternatives, scoped source-visible human review, status-preserving projections, and no model-subword-to-semantic promotion |
| `witness-text-collation-packet-v1.schema.json` | stand-off same-language witness comparison with exact witness, layer, unit, selector, digest, rights, method, normalized-view, and private-detail bindings; proposed/decided status and projection admission stay separate from preferred reading, textual equivalence, Expression derivation, translation, semantics, graph truth, canon, and publication |
| `authored-route-evidence-bridge-v1.schema.json` | text-free reconciliation of an authored route with exact source anchors/layers and reciprocal segmentations, preserving digest-bound legacy witness, node, relation and review inventories |
| `provenance-event.schema.json` | legacy v1 acquisition and transformation entity/activity/agent trail, preserved without reinterpretation |
| `provenance-event-v2.schema.json` | additive immutable execution receipt with exact input/output/byproduct entities, explicit derivation, terminal state, command/configuration/software/runtime/model capture, responsibility, manual changes, measurements, authentication, rights, review, and bounded replay posture |
| `lived-witness-packet.schema.json` | private-by-default first-person testimony with experience/capture time, Work/passage targets, raw and transformed provenance, third-party context, downstream permissions, author confirmation and revision/withdrawal |
| `rights-record.schema.json` | researched rights, permission, visibility, and redistribution posture |
| `material-discovery-record.schema.json` | exact ordered queries, result order, originating-record links, declared-rights evidence, acquisition/snapshot posture, and channel cost comparison |
| `access-request.schema.json` | public-safe request scope, institutional contact route, separate permission purposes, private-correspondence boundary, response/expiry state, and no-bypass law |
| `server-import-contract.schema.json` | item/file manifest handoff with checksum and rights checks, access class, derivative scope, operator approval and publication/takedown state |
| `private-laboratory-evidence-handoff.schema.json` | exact private-raw custody boundary, a distinct active-goal authorization state before raw read, public-safe aggregate allowlist, reconstructive-detail denylist, governed destination, separate creation/publication effects, and human publication gate |
| `public-laboratory-evidence-derivative.schema.json` | aggregate derivative with opaque private source return, minimum-cell suppression, method/outcome/error/cost summaries, explicit confounds and review state |
| `manual-error-ledger-record.schema.json` | append-only historical ledger plus one bounded aggregate source-visible review episode, bound to its governed handoff, public-safe derivative and provenance |
| `sign-annotation.schema.json` | occurrence-to-concept sign ladder without layer collapse, using distinct occurrence, lexeme, sign, and concept identities |
| `semantic-annotation-packet-v2.schema.json` | additive stand-off semantic packet with label-independent opaque identities for lexeme, lexical sense, sign, concept, annotation, claim, relation, and review; exact occurrence/source-anchor return; first-class competing claims; real-human competence and unassisted sign-promotion review; and accepted-claim-only downstream graph projection |
| `claim-packet.schema.json` | evidence-bearing assertion over a stable ToS subject, with alternatives, lineage, and human review state |
| `expression-derivation.schema.json` | typed qualifiers for a directed Expression→Expression source relation, including derivation kind, source directness, evidence/collation posture, same-Work scope, non-transitivity, asymmetry, irreflexivity, and an explicit no-equivalence boundary |
| `translation-alignment-packet-v1.schema.json` | additive stand-off source/target mapping packet binding exact Work/Expression/Edition/Item/File, frozen text-layer, segmentation/tokenization, and ordered anchor identities; separate alignment/claim/review/projection identities; explicit cardinality, omission, addition, reorder, uncertainty, competition, supersession, human competence, and most-restrictive visibility; machine proposals cannot accept themselves and TEI/Web Annotation/XLIFF/TMX/graph forms remain derived |
| `translation-packet.schema.json` | source-accepted lifecycle packet binding independent frozen analyses to human, AI, machine-alternative, and AI+human drafts before comparator reveal, comparison, change tracking, and human adjudication |
| `translation-laboratory-plan.schema.json` | exact source-first 17-stage translation order, real-human lane law, sealed comparator, model-candidate posture, and source-acceptance gate |
| `translation-exposure-aware-plan.schema.json` | additive solo+AI correction that keeps the historical plan frozen while scoping comparator exposure per actor/event, distinguishing operational model-context isolation from unknown training exposure, prohibiting a false blind-human baseline, and keeping German competence, accepted translation, publication, and routine human work closed |
| `translation-reference-register.schema.json` | dated dictionaries, corpora, critical editions, lexical resources, and translation witnesses with separate scholarly, access, rights, citation, and admission posture |
| `translation-pre-draft-analysis.schema.json` | source-accepted, comparator-blind morphology-to-interlinear evidence packets kept independent for real-human, AI-only, and machine-alternative lanes |
| `semantic-ladder-packet.schema.json` | v4 task-specific form-to-graph sequence separating an attested Edition reading from language competence: exact-form, frequency, context, and typed model proposals may advance without becoming reviewed German; promotion still requires competence-appropriate and real-human evidence, distinct occurrence/lexeme/sign/concept/claim/relation identities, competing readings, and a disposable non-authoritative projection; no prepared occurrence creates routine human work |
| `golden-kernel-transfer-plan.schema.json` | source-gated cross-work A/B/C plan, title-page scouting boundary, separate private/ineligible target-candidate soil, content-bearing target-gold requirements, contamination controls, transfer metrics, and an honest non-run result |
| `source-gated-evaluation-plan.schema.json` | preserved v1 semantic/LLM readiness law and its historical universal source/gold gate; no current plan uses it for scheduling or execution |
| `source-gated-semantic-evaluation-plan.schema.json` | task-specific semantic A/B/C readiness: accepted evidence per selected task, unassisted baselines only for materialized interpretive tasks, language-competence routing, separate labels and rationales, and exceptional rather than routine second-human review |
| `source-gated-llm-evaluation-plan.schema.json` | task-specific LLM readiness: twenty accepted anchored source units, a later unassisted baseline only for those materialized subjective tasks, non-authoritative 30/15 history, and fail-closed A/B/C execution |
| `laboratory-sample-plan.schema.json` | source-balanced frozen sample units, strata, anchors, and gold candidates |
| `ocr-visual-sample-plan.schema.json` | output-blind 3x12 visual OCR projection, shared render law, sealed reference witnesses, and an explicit human-gold gate |
| `manual-gold-status.schema.json` | model-draft and two-pass source-visible human gold state without relabeling |
| `manual-gold-assurance.schema.json` | additive solo+AI assurance ladder, trigger-gated sparse calibration, zero-human-debt closure, delayed same-human stability check, language-competence boundary, and digest-bound legacy lineage |
| `translation-sample-plan.schema.json` | thirty frozen German fragments, sealed comparator, staged lanes, and etymology law |
| `translation-source-review-plan.schema.json` | v2 page-triplet source review routing after selector failure, without reusing rejected automatic text |
| `german-assisted-source-review.schema.json` | solo+AI evidence lanes, visual-only competence boundary, critical-edition witness route, triggered 1-3 unit scheduling, and fail-closed translation consequences |
| `critical-edition-witness-admission.schema.json` | one exact critical-edition locator, reference identity, local witness structural context distinct from exact critical-text comparison, provenance, content non-capture, rights review, and fail-closed effects before citation-witness admission |
| `german-source-triangulation.schema.json` | text-free comparison of one critical-edition candidate, structured TEI witness and OCR witness, with normalization controls, transport observations and rights conditions |
| `edition-reading-admission.schema.json` | edition-local admission of a documented scholarly transcription reading, bound to exact Item/File, selector, editorial method, rights and comparison evidence; subsequent linguistic and semantic uses retain their own assessment requirements |
| `bounded-translation-research-input.schema.json` | one private DTA-derived artifact selected for blind machine-method calibration, with exact selector, transformation, digest, corroboration, rights and sealed authored translation comparators |
| `retrieval-query-plan.schema.json` | frozen query intents, languages, expected anchors, hard negatives, and local-only query-content digest |
| `visual-retrieval-plan.schema.json` | output-blind direct page-image retrieval challenger over the same frozen queries, digest-bound visual crosswalk and local renders, immutable completed text controls, exact model revision, triggered-only human review, and zero automatic promotion |
| `visual-retrieval-result-receipt.schema.json` | text-free return of a private direct-page-image run, retaining frozen inputs, artifact fixity, normalization, source-anchor closure, cost, audit history and declared review triggers |
| `graph-query-plan.schema.json` | frozen four-layer graph questions, allowed predicates, claim-set digest, and unreviewed expectations |
| `source-witness-bibliographic-graph.schema.json` | derived bibliographic graph with reified Claims, exact source return, typed literal values and evidence/maker/provenance/review context |

## Common laws

- ToS IDs persist across path changes and are never reused.
- Repo-relative refs and local payload paths do not contain absolute paths or
  parent traversal.
- Every source-bearing record cites an exact file digest or an anchor that does.
- Every automated action identifies software/model/configuration through a
  provenance event or receipt.
- Successor records retain the exact previous transcription, correction,
translation, annotation and Claim versions.
- Translation alignment is a separately versioned Claim over two exact frozen
sides. Unaligned members, reorder, and
  reciprocal competing maps remain explicit; interchange IDs and graph edges
  never replace ToS owner identity.
- Review states retain rejection, ambiguity, deferral, and counterevidence.
- Rights/visibility constraints travel into derivatives and projections.
- A tracked lexical projection may expose only the content posture authorized
  by its plan. A hash-only form row supplies a dictionary-recoverable navigation fingerprint
and retains the source’s access restrictions.
- A recurrence projection may derive frequency, structural range, and
  part-size-aware dispersion only from a fixity-bound lexical projection.
  Each dimension retains its own observational meaning; motif or Sign proposals
require separately grounded interpretation.
- A usage-context plan must name and freeze the exact question and selection
  law before source-bearing output. Exact context and occurrence positions
  remain ignored local evidence; a tracked receipt may expose only fixity,
  counts, source-state and selector closure, rights posture, and explicit
  non-authority. The window supplies exact context for a selected occurrence; linguistic
segmentation and semantic interpretation follow their own source routes.
- A morphology input receipt may prove that every exact-form row was
  deterministically materialized into a private packet. Provider coverage, correctness and linguistic interpretation require their own
assessment evidence.
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
- the frozen historical translation packet permits honest preparation without fabricated final
  evidence, binds every frozen draft to its own blind pre-draft packet, requires
  at least two machine alternatives, freezes human-only and AI-only work before
  AI+human collaboration, seals recognized witnesses until all blind drafts are
  frozen, and preserves post-reveal changes, rejected alternatives, and
  real-human adjudication as separate records;
- the active exposure-aware overlay preserves that historical packet while
  requiring actor/event exposure snapshots, prohibiting the current operator's
  false blind-human baseline, limiting AI independence to current-context
  isolation, keeping model-training exposure unknown, and admitting no
  accepted translation or routine human work;
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
- a German source-triangulation packet closes exact input fixity, selectors,
  aggregate token/paragraph counts, one-way fingerprints, and normalization
  controls while forcing unencrypted transport, rights review, German
  acceptance, translation, semantics, and promotion to remain unresolved;
- a bounded translation research-input packet may prove that one exact
  ignored source string was deterministically derived and machine-corroborated
  for one named local calibration purpose while keeping the accepted
  translation plan, recognized comparator and older authored translation
  surfaces, rights, language competence, semantics, graph, and canon gates
  closed;
- a visual-retrieval plan binds the same twenty query bytes to all thirty-six
  frozen page images through source and visual sample identities, preserves
  completed text variants A and B as immutable controls, fixes challenger C to
  one exact upstream model revision before download or output, and keeps
  relevance, human metrics, winner selection, publication, and promotion
  unavailable until their separate gates are actually satisfied;
- the historical translation-laboratory plan preserves its exact workflow order, keeps all
  four draft lanes blocked before source acceptance, and forbids comparator
  consultation while sealed;
- a translation-reference register covers every required source category,
  resolves local witness identities, prepares contact routes for restricted
  resources, and keeps all reference content unadmitted before human
  bibliographic and rights review;
- a legacy blind pre-draft packet requires two real-human source-acceptance passes,
  preserves the nine morphology-to-interlinear stages in order, rejects AI
  assistance or machine-authored findings in the human-only lane, rejects
  human editing in model lanes, keeps comparators and other lanes hidden, and
  requires cited external evidence for etymology;
- a semantic-ladder packet may bind one exact `edition_reading_attested`
  source unit while still carrying no selected form, occurrence, lexeme, sign
  candidate, human task, claim, relation, concept, or projection; exact-form,
  frequency, context, and explicitly typed model proposals may materialize
  from that Edition reading without asserting accepted German;
  competence-appropriate review remains independent, and relations, concepts,
  counterreadings, and graph projection remain blocked until the concrete sign
  candidate receives an attested real-human decision over a frozen unassisted baseline;
  accepted signs, concepts, claims, and relations use distinct stable IDs, and
  graph projection cannot skip competing readings or claim authority; this is
  a promotion checkpoint for one sign packet, not routine human work for every
  occurrence, form, concordance row, or machine proposal;
- a semantic-annotation v2 packet keeps occurrence, lexeme, lexical sense,
  sign, concept, claim, relation, review, and graph-edge identities separate;
  mutable labels, glosses, translations, and current concept names never seed
  semantic IDs; every claim and relation closes to source anchors and typed
  evidence; competing readings remain reciprocal first-class claims; model
  proposals cannot become accepted signs without a competence-appropriate
  real-human promotion review over a source-visible unassisted baseline; and
  graph projection admits only accepted claims and relations while preserving
  source return;
- a source-text-unit v1 packet keeps physical layout, source-observed
  structure, orthographic, linguistic, model-input, and non-surface analytic
  units separate over one exact frozen text layer; gives schemes,
  segmentations, units, reviews, and projections opaque identities independent
  of text, labels, ordinals, offsets, or mutable analysis; requires exact
  anchor return and explicit coverage, gaps, overlap, whitespace, punctuation,
  line-break, hyphenation, parent/child, competition, supersession, review
  scope, and visibility closure; blocks machine/model/synthetic self-acceptance
  and model-subword semantic promotion; and leaves the legacy
  `tos-local-sentence-segmentation-v1` string uninterpreted until a bounded
  migration is justified;
- a witness-text-collation v1 packet compares two or more exact source-bound
  witness spans without collapsing their Expression identities or mutating
  their text layers; keeps each correspondence as a separately reviewable
  claim; records exact, normalized-shadow, aggregate opcode, and withheld
  reconstructive-detail evidence without treating similarity as identity;
  requires a real source-visible human review before any decided status and
  admits projections only from accepted claims; and never infers a preferred
  reading, textual equivalence, edition genealogy, Expression derivation,
  translation relation, lexical or semantic identity, graph truth, canon, or
  publication authority;
- an authored-route evidence bridge preserves a living legacy route while
  returning it to exact source layers: source-attested TEI paragraphs and
  imported authored segments remain reciprocal competing segmentations, every
  source/authored/review surface is digest-bound, and the route's German,
  Russian, and English roles remain visible; the bridge is an inventory and
  representation crosswalk only, so old review notes do not become modern
  human attestations and old nodes or relations do not become claim/evidence-
  closed graph facts, accepted source, accepted translation, signs, concepts,
  publication permission, server-transfer authority, or bulk migration input;
- a translation-alignment v1 packet keeps exact source and target witness,
  text-layer, segmentation/tokenization, anchor, alignment, claim, review, and
  projection identities separate; checks correspondence-shape cardinality,
  anchor closure, reciprocal competing maps, supersession, and visibility
  inheritance; blocks machine/model/imported or synthetic proposals from
  accepting themselves; requires a matching source-and-target-visible
  real-human decision with declared source-language, target-language, and
  translation competence before acceptance; and permits TEI, Web Annotation,
  XLIFF, TMX, or graph output only as a non-authoritative projection of an
  accepted mapping;
- a golden-kernel transfer plan keeps title-page scouts ineligible for
  semantic evaluation, may preserve exactly twenty pre-output private
  page-level candidates without calling them gold or opening human debt,
  requires real-human kernel and target gold before a ready state, preserves
  exact A/B/C isolation, and cannot report runs, metrics, a winner, or
  promotion while `blocked-not-run`;
- a v1 source-gated semantic plan cannot materialize tasks, runs, metrics, a
  winner, or promotion while its accepted-source and double-checked-gold gates
  remain absent, but that preserved v1 contract has no current scheduling or
  execution authority;
- semantic v2 readiness comes only from twenty selected tasks whose exact
  anchors, accepted source digests, source-review events, and local-content
  digests close independently; the historical 30/15 packet counts schedule no
  work; unassisted human baselines open only for materialized sign and
  competing-reading tasks, morphology/lemma execution requires
  language-competence evidence, and a missing competent or second review
  leaves the claim unresolved rather than converting preparation into human
  debt;
- a blocked LLM v2 plan may freeze exactly one bounded proposal question,
  output boundary, 10-random/10-hard shape, and runtime-profile reference, but
  it must carry zero source anchors and task instances, schedule no human work,
  and require the runtime profile to be refreshed whenever the plan changes;
- LLM v2 readiness comes only from twenty task-specific accepted source units
  and twenty unassisted baselines for those same subjective tasks; the
  digest-bound historical 30-source/15-gold snapshot has neither scheduling
  nor execution authority;
- a discovery record preserves exact queries and ranked results while keeping
  declared rights as evidence rather than a ToS conclusion;
- a source-resource inventory resolves to one exact item manifest and payload
  digest, enumerates every declared PDF page, EPUB member, TEI structural
  resource, or provider OCR page, and carries no source-text field or content
  authority;
- a witness-structure map closes every cited division, EPUB member, and PDF
  page over exact resource inventories, preserves monotonic part routes, and
  emits no source text;
- a source-only numbered-unit page map closes all declared units and proposed
  whole-page starts over one exact scan inventory, distinguishes ordered OCR
  candidates from explicit source-visible review, and emits no OCR string or
  accepted text;
- a target-expression numbered-unit page map closes all labels actually
  materialized in one exact translation scan over its PDF inventory and work
  boundary, preserves source-only labels as explicit nonmaterialized
  asymmetries, and emits neither target text nor cross-lingual alignment;
- a hierarchical target map keeps every independently resetting numbered
  series in its own identity scope, closes machine/model start-page evidence
  and proposed anchors over exact inventory and work-boundary digests, and
  cannot create text, alignment, eligibility, gold, or human review;
- a parallel numbered-label map can intersect independently materialized
  structural keys, resolve both sides to proposed anchors, and retain unpaired
  keys without turning shared numbering into passage or translation alignment;
- a target-only transfer crosswalk closes the frozen work quota against one
  hierarchical target map, preserves spill ambiguity and exact next-start
  context, and requires zero source routes, eligibility, target gold, and human
  work while no German parallel map exists;
- a parallel-witness structure map closes each proposed division start and
  whole-page anchor over the exact original-language and translation
  inventories, verifies its separate source-only map binding, keeps numbered
  spans contiguous, and leaves exact target numbered-unit pages explicitly
  unmaterialized;
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
  keeps all four logical layers explicit;
- a recurrence plan and projection can bind every hash-only exact-form row to
  the same source totals, preserve A/B/C observation fields, and reproduce
  integer-rounded `DP` and maximum-part-share values without source payloads;
- a question-scoped usage-context plan can freeze one exact-form control and
  a complete-census law before output; its private rows can preserve exact
  source context and composite selectors while its tracked receipt proves
  plan/input/generator fixity, aggregate closure, local-only visibility, and
  zero sentence, linguistic, semantic, graph, public, or human-work effect;
- a morphology plan can freeze an exhaustive type/token coverage question
  before provider output, keep the exact surface unchanged, and block the
  contextual A/B/C stage until one shared occurrence set is frozen;
- a morphology input receipt can bind a private JSONL packet to the exact
  local lexical database and tracked source-withholding projection while
  exposing only aggregate counts and digests in Git.
- a later morphology artifact-admission receipt must remain additive: it may
  prove exact private acquisition and preserve a denied trust verdict, but it
  cannot rewrite the pre-output plan, turn an unverified candidate into a
  runtime, consume source content after denial, or claim quality from download
  cost.

Source-visible assessment must separately determine:

- two catalog records describe the same historical object;
- metadata, OCR, segmentation, lemma, etymology, or alignment is correct;
- a resource fingerprint or structural correspondence proves textual identity;
- a candidate locator makes two editions equivalent or accepts their German;
- matching division order, numbered spans, or page addresses makes a
  translation equivalent, faithful, or semantically aligned;
- a translation is faithful or philosophically adequate;
- a rights determination is legally sufficient;
- a semantic claim or relation should enter canon.
- a same-human delayed recheck is independent of memory or equivalent to
  agreement among different reviewers;
- a declared language competence is actually sufficient for the reviewed
  material.

Use authorized, competent source-visible review and a reasoned record under
`ToS/doctrine/KNOWLEDGE_ASSESSMENT.md`. Legacy human-only formats retain
their explicit historical review requirements until connected through an
authorized adapter.

## Versioning

The first family uses `v1`. A backward-incompatible change receives a new
schema version and migration evidence. Tightening a validator around an
unstated assumption is a contract change even if the JSON filename stays the
same.

`tos_golden_kernel_transfer_plan_v2` supersedes the v1 plan shape because the
strict v1 `target_units` array could represent only fully human-double-checked
evaluation units. V2 adds a separate, fail-closed
`candidate_target_units`/`candidate_preparation` layer for private pre-output
sampling soil while leaving the ready-state target-gold gate unchanged. The
v3 transfer provenance event records the migration, exact builder, source
digest, candidate anchors, and local-content digests.

`tos_semantic_annotation_packet_v2` is additive. It does not reinterpret or
bulk-migrate `tos_sign_annotation_v1`, `tos_claim_packet_v1`, the frozen
initial-sign packet, or semantic-ladder v4/v5. A real packet is created only
for one concrete source-grounded question.
