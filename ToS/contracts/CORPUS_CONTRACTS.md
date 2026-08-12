# Corpus Foundation Contracts

These contracts make the corpus evidence spine mechanically exchangeable.
They do not make a bibliographic, textual, translation, semantic, or rights
judgment true.

## Contract family

| Contract | Owns |
| --- | --- |
| `corpus-record.schema.json` | persistent agent/work/expression/edition/collection/item identity plus exact outgoing Work→Expression, Expression→Edition, Edition→Item, and optional Expression-derivation claim closure refs |
| `source-item-manifest.schema.json` | immutable local payload inventory, digest, and tracked companion refs |
| `source-resource-inventory.schema.json` | text-free PDF or bundled-DjVu page, EPUB member/spine, TEI page-break/division, and provider DjVu/ABBYY OCR-page inventory with geometry, ordering, counts, member fixity, and one-way fingerprints |
| `lexical-index-plan.schema.json` | source-gated exact-form observation plan with explicit local source-bearing versus tracked hash-only outputs, field-by-field authority, rights routing, and a semantic non-effect boundary |
| `lexical-index-projection.schema.json` | rebuild receipt and non-sequential form-hash/count/page/division read model over exact local witnesses, with working local query probes but no accepted source, lemma, sign, context, or publication claim |
| `lexical-recurrence-plan.schema.json` | frozen exact-form recurrence question over the tracked lexical projection, preserving A frequency, B structural range, and C tupleized part-size-aware dispersion without a composite score or semantic effect |
| `lexical-recurrence-projection.schema.json` | deterministic hash-only frequency/range/DP observation tuples with exact source totals, integer rounding law, residue accounting, and explicit zero authority for source acceptance, linguistic analysis, signs, semantics, or human work |
| `lexical-usage-context-plan.schema.json` | one frozen question, preselected exact-form control, complete occurrence census, page-bounded context law, composite source selectors, ignored local-output route, tracked exposure ceiling, rights gate, and explicit non-semantic effect |
| `lexical-usage-context-row.schema.json` | private source-bearing exact usage row with deterministic context/occurrence identity, source state, structural and position selectors, exact page-bounded token window, clipping, and no semantic field |
| `lexical-usage-context-receipt.schema.json` | tracked source-withholding fixity/count/selector-closure receipt for the private context bundle, binding the frozen question, lexical/recurrence inputs, local database, generator, rights posture, and zero linguistic, semantic, graph, public, or human-work authority |
| `morphology-evaluation-plan.schema.json` | source-gated historical-German morphology question with an identity control, exhaustive direct-form A census, sequentially blocked contextual A/B/C follow-up, explicit layer/competence/rights boundaries, and zero automatic linguistic or semantic promotion |
| `morphology-input-receipt.schema.json` | text-free fixity and count receipt for the ignored exact-form morphology input packet, binding it to the exact lexical database, tracked projection, plan, and generator without tracking source strings |
| `morphology-contextual-episode-plan.schema.json` | additive one-question morphology follow-up that binds a concrete A ambiguity, complete source recurrence, output-blind first/median/last selection, B-only relevance, local context route, rights and competence gates, and zero semantic or human-backlog effect |
| `morphology-contextual-episode-row.schema.json` | private exact raw-TEI context row with one selected occurrence, composite source selectors, exact target offsets, unchanged historical input, and no accepted linguistic authority |
| `morphology-contextual-episode-receipt.schema.json` | tracked text- and position-free receipt for the private contextual packet, proving trigger, selection, source-return and variant-state closure while B remains unacquired and C question-inapplicable |
| `morphology-contextual-artifact-admission.schema.json` | additive source- and path-free record of exact private artifact acquisition, rights metadata gaps, owner resource cost, required/present/verified trust controls, fail-closed runtime admission, and zero execution or linguistic/semantic effect without rewriting the frozen pre-output plan |
| `witness-structure-correspondence.schema.json` | text-free named-division locator candidates between exact witness inventories, with transient matching metrics, monotonic routes, provenance, and an explicit non-identity ceiling |
| `witness-structure-anchor-set.schema.json` | stable proposed TEI, EPUB-member, and PDF-page addresses bound to a witness-structure correspondence without asserting an exact passage boundary or textual identity |
| `numbered-unit-page-map.schema.json` | text-free source-only numbered-unit start-page candidates bound to one exact scan package, its PDF/DjVu/ABBYY inventories, proposed whole-page anchors, explicit review basis, and no textual or critical-edition acceptance |
| `target-numbered-unit-page-map.schema.json` | text-free target-expression numbered-label start-page candidates bound to one exact translation scan, its work boundary and source-map asymmetry reference, while forbidding source-to-target alignment, equivalence, quality, or semantic claims |
| `hierarchical-target-numbered-unit-page-map.schema.json` | text-free target-expression numbering with independently resetting series such as a preface and multiple essays, preserving series-qualified unit identity, proposed page starts, machine/model review basis, and zero textual or transfer authority |
| `hierarchical-source-numbered-unit-page-map.schema.json` | text-free source-expression numbering with independently resetting series, exact address and navigation witness bindings, proposed page starts, bounded page-relation evidence, and zero accepted-text or alignment authority |
| `hierarchical-numbered-unit-label-correspondence.schema.json` | release-safe intersection of independently materialized source and target `series:unit` labels, binding maps and layered rights without comparing prose or asserting passage/translation alignment |
| `parallel-numbered-unit-label-map.schema.json` | release-safe intersection of independently materialized source and target number-label keys, binding both maps, anchors, rights records, and source-only asymmetries without comparing text or asserting passage/translation alignment |
| `transfer-candidate-structural-crosswalk.schema.json` | text-free narrowing of frozen whole-page transfer candidates through an already tracked target unit-start map and shared-label correspondence, preserving spill ambiguity, exact-next-start context, zero eligible units, and zero translation/semantic authority |
| `transfer-candidate-target-structural-crosswalk.schema.json` | target-only narrowing when a hierarchical target unit map exists but no source parallel map does, retaining spill ambiguity, an explicit zero source-route count, zero eligibility, and no alignment or translation authority |
| `transfer-candidate-source-structural-route.schema.json` | composition of a frozen target-only candidate crosswalk with shared hierarchical labels, materializing possible German structural routes while exact passage ends, alignment, accepted text, eligibility, gold, human and semantic effects remain false |
| `transfer-target-passage-candidate-set.schema.json` | private local materialization of complete target numbered-unit slices from an expected label to the next same-series label in one exact embedded-PDF bbox layer, with tracked text-free geometry/digests, preserved nonintersection negatives, and zero accepted text, alignment, eligibility, gold, human, semantic, publication, or canon authority |
| `transfer-source-passage-candidate-set.schema.json` | private local materialization of German numbered-unit slices only where an expected source label and the next same-series label resolve inside one named automatic layer, with tracked text-free geometry/digests, explicit unresolved boundaries and address/navigation witness relations, and zero accepted German, source-target alignment, eligibility, gold, human, semantic, publication, or canon authority |
| `private-transfer-source-visible-review-bundle.schema.json` | ignored mode-0600 model-source-visible diplomatic candidates over one exact source/target page pair, with exact local payloads, automatic candidates, page-render fixity, a separate critical-edition comparison layer, explicit model maker, and no human, acceptance, alignment, linguistic, semantic, publication, or canon authority |
| `transfer-source-visible-review-receipt.schema.json` | tracked text-free projection of one private review bundle: input and generator fixity, deterministic page-render reproduction, aggregate alphabetic-token discrepancy topology, separate historical/critical posture, material finding classes, and closed acceptance, alignment, eligibility, gold, human-debt, semantic, publication, and canon gates |
| `parallel-witness-structure-map.schema.json` | text-free parallel PDF division starts and division-level numbered-unit spans across an original-language expression and one translation expression, bound to any separate source-only numbered-unit map while preserving supplemental-unit asymmetries, zero exact target-unit pages, and no translation-equivalence claim |
| `collection-work-boundary-map.schema.json` | text-free complete or explicitly partial member-work representation, contiguous represented/non-member/unrepresented container-page coverage, exact Work/Expression/claim refs, optional evidence-bearing responsibility refs, source order and anchors, and an explicit bibliographic-only ceiling for aggregate items |
| `source-anchor.schema.json` | structural, quote, position, and page-region selectors tied to one file digest |
| `source-text-layer.schema.json` | immutable role-bearing source-text representation bound to exact Work/Expression/Edition/Item/File and source-anchor-v2 identity, with predecessor and edit/normalization lineage, uncertainty, editorial policy, explicit rights/publication-authority refs, and separate mechanical, review, language-competence, accepted-use, rights, and publication gates |
| `source-text-unit-packet-v1.schema.json` | additive frozen-layer unit and segmentation owner with opaque label-independent scheme/segmentation/unit identities, exact ordered anchor return, distinct layout/source-structure/orthographic/linguistic/model-input kinds, explicit coverage/gaps/overlap/whitespace/punctuation/line-break/hyphenation posture, reciprocal alternatives, scoped source-visible human review, status-preserving projections, and no model-subword-to-semantic promotion |
| `witness-text-collation-packet-v1.schema.json` | stand-off same-language witness comparison with exact witness, layer, unit, selector, digest, rights, method, normalized-view, and private-detail bindings; proposed/decided status and projection admission stay separate from preferred reading, textual equivalence, Expression derivation, translation, semantics, graph truth, canon, and publication |
| `authored-route-evidence-bridge-v1.schema.json` | text-free reconciliation of one pre-existing authored route with exact source anchor/layers and reciprocal source/authored segmentations; binds legacy witness, node, relation, and review inventories by digest while keeping modern human attestation, claim/evidence closure, source/translation acceptance, sign/concept promotion, graph admission, canon revision, publication, server transfer, and bulk migration at zero |
| `provenance-event.schema.json` | legacy v1 acquisition and transformation entity/activity/agent trail, preserved without reinterpretation |
| `provenance-event-v2.schema.json` | additive immutable execution receipt with exact input/output/byproduct entities, explicit derivation, terminal state, command/configuration/software/runtime/model capture, responsibility, manual changes, measurements, authentication, rights, review, and bounded replay posture |
| `lived-witness-packet.schema.json` | private-by-default first-person authored body, distinct experience/capture time, work or passage targets, raw/AI/transformation provenance, third-party posture, separate downstream permissions, author confirmation, revision/withdrawal, and explicit zero source/philology/semantic/canon authority |
| `rights-record.schema.json` | researched rights, permission, visibility, and redistribution posture |
| `material-discovery-record.schema.json` | exact ordered queries, result order, originating-record links, declared-rights evidence, acquisition/snapshot posture, and channel cost comparison |
| `access-request.schema.json` | public-safe request scope, institutional contact route, separate permission purposes, private-correspondence boundary, response/expiry state, and no-bypass law |
| `server-import-contract.schema.json` | future item/file manifest handoff, checksum and rights gates, access class, derivative matrix, operator approval, publication/takedown state, and server non-authority |
| `private-laboratory-evidence-handoff.schema.json` | exact private-raw custody boundary, a distinct active-goal authorization state before raw read, public-safe aggregate allowlist, reconstructive-detail denylist, governed destination, separate creation/publication effects, and human publication gate |
| `public-laboratory-evidence-derivative.schema.json` | actual aggregate derivative payload: opaque private return, minimum-cell suppression, method/outcome/error/cost summaries, explicit confounds, review state, and zero source/translation/semantic/canon authority |
| `manual-error-ledger-record.schema.json` | append-only historical ledger state plus one bounded aggregate source-visible human candidate-review episode, digest-bound to its governed handoff, public-safe derivative, and provenance while accepting no transcription, independent gold, method winner, content authority, or routine human backlog |
| `sign-annotation.schema.json` | occurrence-to-concept sign ladder without layer collapse, using distinct occurrence, lexeme, sign, and concept identities |
| `semantic-annotation-packet-v2.schema.json` | additive stand-off semantic packet with label-independent opaque identities for lexeme, lexical sense, sign, concept, annotation, claim, relation, and review; exact occurrence/source-anchor return; first-class competing claims; real-human competence and unassisted sign-promotion review; and accepted-claim-only downstream graph projection |
| `claim-packet.schema.json` | evidence-bearing assertion over a stable ToS subject, with alternatives, lineage, and human review state |
| `expression-derivation.schema.json` | typed qualifiers for a directed Expression→Expression source relation, including derivation kind, source directness, evidence/collation posture, same-Work scope, non-transitivity, asymmetry, irreflexivity, and an explicit no-equivalence boundary |
| `translation-alignment-packet-v1.schema.json` | additive stand-off source/target mapping packet binding exact Work/Expression/Edition/Item/File, frozen text-layer, segmentation/tokenization, and ordered anchor identities; separate alignment/claim/review/projection identities; explicit cardinality, omission, addition, reorder, uncertainty, competition, supersession, human competence, and most-restrictive visibility; machine proposals cannot accept themselves and TEI/Web Annotation/XLIFF/TMX/graph forms remain derived |
| `translation-packet.schema.json` | source-accepted lifecycle packet binding independent frozen analyses to human, AI, machine-alternative, and AI+human drafts before comparator reveal, comparison, change tracking, and human adjudication |
| `translation-laboratory-plan.schema.json` | exact source-first 17-stage translation order, real-human lane law, sealed comparator, model-candidate posture, and source-acceptance gate |
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
| `german-source-triangulation.schema.json` | text-free machine comparison of one local critical-edition candidate, one structured TEI witness, and one OCR witness, including source-aware normalization failure controls, transport/rights limits, and zero human/translation/semantic gate effects |
| `edition-reading-admission.schema.json` | edition-local admission of what one exact documented scholarly transcription reads, bound to Item/file fixity, source selector, editorial method, rights, exact text-free comparison evidence, and a distinct critical-edition witness; it permits later source-observational work while accepted German, linguistic correctness, Edition identity, author-finality, accepted translation, sign, semantics, graph, canon, transfer, publication, and routine human debt remain false |
| `bounded-translation-research-input.schema.json` | one DTA-derived ignored local source artifact admitted only to blind machine-method calibration, with exact selector, transformation, digest, corroboration, rights posture, explicit sealing of pre-existing authored translation surfaces, and zero accepted-German, translation, semantic, graph, or canon effects |
| `retrieval-query-plan.schema.json` | frozen query intents, languages, expected anchors, hard negatives, and local-only query-content digest |
| `visual-retrieval-plan.schema.json` | output-blind direct page-image retrieval challenger over the same frozen queries, digest-bound visual crosswalk and local renders, immutable completed text controls, exact model revision, triggered-only human review, and zero automatic promotion |
| `visual-retrieval-result-receipt.schema.json` | text-free return receipt for one exact owner-local direct-page-image run: frozen-input and private-artifact fixity, persisted normalization, source-anchor closure, measured resource cost, preserved audit-incomplete lineage, narrow declared review triggers, and closed relevance, adoption, promotion, publication, and routine-human-work gates |
| `graph-query-plan.schema.json` | frozen four-layer graph questions, allowed predicates, claim-set digest, and unreviewed expectations |
| `source-witness-bibliographic-graph.schema.json` | generated claim-reified bibliographic graph with exact source return, typed literal objects, evidence/maker/provenance/review closure, and no unqualified subject-object edge |

## Common laws

- ToS IDs persist across path changes and are never reused.
- Repo-relative refs and local payload paths do not contain absolute paths or
  parent traversal.
- Every source-bearing record cites an exact file digest or an anchor that does.
- Every automated action identifies software/model/configuration through a
  provenance event or receipt.
- Earlier transcription, correction, translation, annotation, and claim
  versions are superseded, not overwritten.
- Translation alignment is a versioned claim over two exact frozen sides, not
  an intrinsic property of either string. Unaligned members, reorder, and
  reciprocal competing maps remain explicit; interchange IDs and graph edges
  never replace ToS owner identity.
- Review states retain rejection, ambiguity, deferral, and counterevidence.
- Rights/visibility constraints travel into derivatives and projections.
- A tracked lexical projection may expose only the content posture authorized
  by its plan. A hash-only form row is a navigational fingerprint, not
  confidentiality, source acceptance, a lexeme, a lemma, or a sign.
- A recurrence projection may derive frequency, structural range, and
  part-size-aware dispersion only from a fixity-bound lexical projection.
  Those dimensions remain separate observations: they are not a rank,
  philosophical-importance score, motif, sign candidate, sign, or semantic
  promotion.
- A usage-context plan must name and freeze the exact question and selection
  law before source-bearing output. Exact context and occurrence positions
  remain ignored local evidence; a tracked receipt may expose only fixity,
  counts, source-state and selector closure, rights posture, and explicit
  non-authority. A concordance window is not a sentence, sense, lexeme, sign,
  semantic relation, publication object, or human-work schedule.
- A morphology input receipt may prove that every exact-form row was
  deterministically materialized into a private packet. It does not prove
  provider coverage, correctness, German competence, a lemma, or a lexeme.
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

It cannot establish that:

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

Those require source-visible human review and a reasoned decision record.

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
