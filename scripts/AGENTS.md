# AGENTS.md

This file applies to the generator and validator tools under `scripts/`.

## Read first

Before editing tools here, read:
1. the repository root `AGENTS.md`
2. `docs/validation/validation_lanes.json` for command authority
3. `docs/validation/SCRIPT_TOPOLOGY.md` and `docs/validation/script_inventory.json`
4. `ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md`
5. `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`
6. `mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md`
7. `docs/decisions/AGENTS.md` when decision-index tooling is touched
8. the schema, example, intake, decision, or tree surfaces the script actually touches
9. `ToS/doctrine/CORPUS_FOUNDATION.md` and `ToS/source-witnesses/README.md` for source-witness catalog or validation work

## Local role

`scripts/` owns the bounded validator and generation machinery for the current ToS route.

These tools should:
- stay local
- stay deterministic
- stay source-owned
- make review easier rather than more magical

`validate_philosophy_topology.py` protects the domain-shaped `ToS/philosophy/`
branch so it cannot collapse into a flat import folder or inherit source UI
labels as repository topology.

`build_tos_corpus_index.py` and `validate_tos_corpus_index.py` publish the
checked whole-corpus index for graph review. They index the whole `ToS/` home
as a derived resource map; they do not move runtime projection or visualization
authority into Tree of Sophia.

`build_source_witness_catalog.py` and
`validate_source_witness_foundation.py` protect the corpus evidence spine and
tracked object/claim catalog. The generated claim projection covers only
tracked membership, responsibility, publication, provision-activity, and
identity-ladder packets,
including separately reified Work→Expression, Expression→Edition, and
Edition→Item directions. The current Nietzsche slice must also retain one
explicit Work-owned `authored_by` packet per Work, resolved to the Nietzsche
Agent and digest-bound to its documentary provenance; an author-named path is
not evidence. Each of those Works must separately retain one
`first_publication_chronology` packet under the source-owned chronology route;
the validator checks the temporal-object contract, Work/Edition closure,
sequence boundaries, evidence digests, and the prohibition on an untyped
single Work year. The tools preserve exact source-line return and canonical claim
digests, and remain subordinate to
those authored packets and their provenance events. It must fail closed rather
than copy a nonpublic claim into the tracked catalog. The release lane permits
gitignored payload bytes to be absent from a public clone; an operator may add
`--require-local-payloads` for the local laboratory. Passing this lane never
implies that a human accepted the bibliography, relation, text, rights,
translation, review, or semantics.

The catalog and foundation validator also admit the bounded Edition-owned
`provision_activity` family. Catalog v3 adds Place and Organization record
projections; the validator checks exact Edition claim-ref closure, activity
and role coherence, Gregorian precision, literal-versus-normalized identity,
normalized record type, evidence, and digest-bound annotation provenance. A
statement year is not a public-release date, a publisher is not a printer or
modern successor, and neither a green contract nor a normalized authority ID
accepts the bibliography.

The source-foundation validator also owns the public-synthetic
`tos_source_anchor_v2` laboratory. `--source-anchor-v2-lab-only` resolves the
frozen JSON, Unicode-text, and container/XHTML A/B/C and reports the actual
selected strings plus negative-control outcomes. It reads no corpus payload,
migrates no v1 anchor, and cannot establish source text, review, translation,
semantics, rights, graph truth, or canon. The ordinary source-witness lane runs
the same mechanics as one bounded subcheck.

The validator also owns the public-synthetic `tos_source_text_layer_v1`
laboratory. `--source-text-layer-lab-only` reads the three tracked UTF-8
artifacts, verifies their digests and exact source-anchor-v2 return, replays
the two declared Unicode-code-point transitions, checks normalization and
predecessor closure, and exercises fail-closed review/competence/use/rights
controls. It reads no private corpus payload, accepts no transcription, and
cannot supply human or language competence, translation, semantics,
publication, graph truth, canon, or bulk migration authority.

`build_source_text_unit_v1_lab.py` owns only the deterministic public-synthetic
source-text-unit and segmentation A/B/C. It writes one invented ASCII source,
A's exact physical-line and newline units, B's reciprocal sentence and
hyphen-token alternatives, and C's deliberately invalid model-shaped
acceptance with a hidden final-newline gap. It must never read corpus payload,
invoke a tokenizer or model, fabricate language competence or human review,
accept a real boundary, or create lexical, semantic, graph, publication, or
canon authority. Schema, research, builder, source, plan, or variant changes
require an intentional full rebuild because the manifest binds those bytes.

`build_zarathustra_source_text_foundation.py` owns one real, question-scoped
continuation only: the first paragraph of `Za-I-Vorrede-1` in the exact DTA
Part-I TEI Item. It converts each declared TEI `lb` to one line feed, fails on
unexpected child markup, writes the 353-byte result below ignored owner-local
`local-content/source-text-foundation/` with mode `0600`, and writes four
tracked text-free records: source-anchor v2, source-text-layer v1,
source-text-unit v1, and provenance-event v2. It must not expand to another
paragraph, normalize spelling, infer sentence/word boundaries, schedule human
work, or open translation/semantic/canon authority without a new concrete
question and its own plan.

`build_zarathustra_target_text_foundation.py` owns the paired but still
non-aligned target-side continuation: the six visible opening-paragraph lines
on page 6 of the exact Antonovsky/Prometey 1911 RSL/RuNEB PDF Item. It pins
Poppler `pdftotext` 26.01.0, the page geometry, block and word selectors, the
17-warning diagnostic surface, the 30,714-byte bbox byproduct, and the
633-byte/349-code-point embedded-text result. Both private outputs stay below
ignored owner-local `local-content/target-text-foundation/` with mode `0600`;
four tracked records remain text-free. The source-visible check found that the
PDF text layer exposes one visibly joined historical word as four tokens, so
two alternative digests remain withheld and uncertainty stays unresolved.
This builder must not silently join the tokens, call the result diplomatic or
accepted Russian, infer sentence/word boundaries, create a bilingual pair or
alignment, schedule human work, or open translation, semantic, graph,
publication, or canon authority.

`build_zarathustra_opening_sentence_alignment.py` owns one later concrete
comparison question across those two exact private paragraph layers. It may
freeze only the first `U+002E`-inclusive code-point span per side, exact
excluded remainders, two proposed orthographic sentence-unit packets, one
software-made proposed one-to-one claim, and its text-free provenance. It must
not copy source text into Git, normalize either layer, resolve the Antonovsky
spacing uncertainty, tokenize, translate, accept a boundary or alignment,
fabricate competence or review, create a projection/human backlog, or open
lexical, semantic, graph, canon, redistribution, or publication authority.

`build_antonovsky_2007_1911_collation.py` owns a different, same-language
witness question. It reuses the preserved but unattested p011 Workbench
observation exactly once, writes its text and reconstructive comparison detail
only below ignored owner-local `local-content/witness-text-collation/` with
mode `0600`, and writes five tracked text-free anchor/layer/unit/collation/
provenance records. It may expose exact selectors, digests, normalized-shadow
metrics, and opcode aggregates, but must not relabel the observation as human
review or gold, choose a preferred reading, infer textual equivalence, Edition
genealogy, Expression derivation, translation, lexical or semantic identity,
create a projection or human backlog, or authorize graph, canon, redistribution,
or publication effects.

`build_zarathustra_authored_canon_evidence_bridge.py` owns the non-migrating
return from the pre-existing authored `prologue-1` route to the complete exact
DTA `Za-I-Vorrede-1` section. It writes raw, declared-normalized,
normalization-operation, and comparison-detail artifacts only below ignored
owner-local `local-content/authored-canon-evidence-bridge/` with mode `0600`;
Git receives one anchor, raw/normalized layer records, a source/authored
competing-segmentation packet, a text-free bridge inventory, and provenance.
It must preserve the twelve TEI paragraph boundaries and twelve authored
segment boundaries separately, bind all existing witness/node/relation/review
surfaces by digest, and leave the old canon unchanged. It must not reinterpret
`canonical_source`, `working_translation`, `bridge_translation`, or legacy
review notes as modern acceptance or Human Gold; create claim/evidence closure,
signs, concepts, graph admission, migration, canon revision, human backlog,
publication, or server-transfer authority; or expose exact text in Git.

The source-foundation validator's `--source-text-unit-v1-lab-only` route checks
schema and reference closure, exact code-point ranges and digests, unit
ownership and coverage, declared gaps and overlap, reciprocal parent/child and
competing-segmentation relations, supersession, source-visible scoped review,
accepted-unit closure, projection admission, and most-restrictive visibility.
Its negative controls must fail closed. Green proves synthetic mechanics only;
it supplies no correct boundary, tokenization, German competence, review,
translation, semantic identity, graph fact, migration, or publication right.

`build_provenance_event_v2_lab.py` owns only the public-synthetic execution-
receipt A/B/C. Run `--prepare`, then variants `A`, `B`, and `C` as separate
commands, accepting C's declared exit code 7, and finish with `--finalize`.
The builder must preserve exact input/output/byproduct bytes, argv,
configuration, software/runtime identity, terminal state, and manifest fixity.
Never relabel a failed run's diagnostic as authoritative output. Changing the
builder or contract invalidates their bound digests and requires an intentional
full laboratory rebuild.

The source-foundation validator's `--provenance-v2-lab-only` route validates
the additive `tos_provenance_event_v2` schema, exact record/entity/command
closure, real copy and normalization bytes, honest failed-output boundary, and
negative controls. Legacy v1 records are not migrated. A green unsigned receipt
does not prove the execution occurred, authenticate its producer, establish
content quality, supply human or language competence, or authorize rights,
publication, semantics, graph truth, or canon.

`build_semantic_annotation_v2_lab.py` owns only the deterministic public-
synthetic semantic identity/annotation A/B/C. It creates two exact occurrences,
then two reciprocal model-shaped sign proposals, then one deliberately invalid
promotion attempt. It must never read a corpus payload, invoke a model,
fabricate human review, or produce an accepted sign, concept, relation, graph
edge, or canon effect. Changing the schema, research basis, builder, source, or
variant packets requires an intentional full rebuild so the manifest digests
remain exact.

The source-foundation validator's `--semantic-annotation-v2-lab-only` route
checks the additive stand-off contract, exact public source spans, opaque
label-independent higher IDs, reference closure, reciprocal competing claims,
real-human promotion boundary, accepted-claim-only graph admission, and twelve
negative controls. Its green result proves only synthetic contract mechanics.

`build_translation_alignment_v1_lab.py` owns only the deterministic
public-synthetic translation-alignment identity A/B/C. It writes two invented
private-use-language text sides, frozen synthetic segmentation/tokenization,
exact anchors, A one-to-one, B reciprocal one-to-one/one-to-many proposals,
and C's deliberately invalid acceptance. It must never read a corpus payload,
invoke an aligner/model, fabricate a real translation or human review, accept
an alignment, or emit a projection/canon effect. Schema, research, builder,
input, analysis, plan, and variant changes require a full rebuild because the
manifest binds all bytes.

The validator's `--translation-alignment-v1-lab-only` route checks both exact
text-layer returns, frozen analysis bindings, opaque alignment/claim IDs,
cardinality, reciprocal competition, review and projection admission,
most-restrictive visibility, and seventeen negative controls. Green proves
synthetic mechanics only; it supplies no translation, aligner/model execution,
competence, accepted mapping, lexical equivalence, semantics, graph, or canon.

`build_source_resource_inventories.py` is the focused local companion for
fixity-verified payloads. It writes tracked, text-free PDF or bundled-DjVu page,
EPUB resource, TEI structure, and provider OCR page inventories from an
explicit payload root. It is not a release-time acquisition step and its
one-way fingerprints are not accepted text or alignment evidence.

`build_zarathustra_lexical_index.py` is the explicit-local companion over the
four DTA first-edition part witnesses. It materializes exact forms, source
order, opaque occurrence locators, page context, and FTS5 query fields only in
the pilot's ignored `local-content/` database. Its tracked companion contains
only exact/normalized form hashes, aggregate counts, and TEI page/division
resource refs. Low-entropy hashes are not confidentiality; the tracked route
remains local-only and public-blocked while rights review is open.
`validate_zarathustra_lexical_index.py` checks release-safe source, count,
resource, provenance, blocker, and no-source-string closure without requiring
the ignored database. Neither script accepts German or creates a lexeme,
lemma, translation, sign candidate, sign, semantic claim, graph edge, canon,
or runtime search authority.

`build_zarathustra_recurrence_projection.py` is the release-safe next layer
over that tracked lexical companion. It reads no payload and no private
database. It preserves absolute frequency, part/division/page range,
part-size-aware `DP`, maximum part concentration, and declared residue as a
tuple for every exact-form hash. `--check` proves deterministic parity and the
focused tests independently recalculate the arithmetic. It deliberately
creates no rank, composite importance score, exact string, morphology, lemma,
lexeme, sign candidate, semantic packet, public route, or human task.

`build_zarathustra_usage_context_bundle.py` materializes one complete,
preselected exact-form usage census from the ignored lexical database. Every
private row keeps a fixed 24-token page-bounded window, exact source state,
structural and position selectors, and deterministic occurrence/context IDs;
the mode-0600 JSONL remains under ignored `local-content/`. Its tracked
companion contains only plan and artifact fixity, counts, selector closure,
rights posture, and an explicit semantic ceiling. The control is a known Work
identity check rather than a recurrence winner or sign-like sample. It accepts
no German or sentence boundary, schedules no challenger or human task, and
creates no morphology, lemma, lexeme, translation, sign, concept, claim,
relation, graph, canon, or public route. The release-safe lexical validator
checks the tracked plan/receipt/provenance without requiring the private
bundle; the builder's `--check` owns exact local parity.

`build_semantic_source_recurrence_bundle.py` binds one already selected
packet-local exact-form hash to exactly one existing four-part recurrence row,
then returns the complete occurrence census to raw TEI character-data offsets
with `xmllint --nonet`. The exact form and all occurrence positions remain in
one ignored mode-0600 local bundle. The tracked receipt contains only hashes,
aggregate tuple, per-part counts, fixity, and verification totals. It does not
reopen selection or change the semantic packet, and it creates no German,
morphology, lemma, sense, motif, sign, human, semantic, graph, canon, transfer,
or publication authority. `--check` owns exact private/tracked parity; the
release-safe source-witness validator checks the tracked closure without
requiring private bytes.

`build_zarathustra_morphology_input.py` projects every exact form from that
private lexical database into a deterministic ignored JSONL packet for the
admitted DWDSmor A census. Its tracked receipt contains only digests and
aggregates. It runs no morphology provider and cannot create a lemma, lexeme,
sign, semantic claim, or human backlog.

`record_zarathustra_morphology_census_result.py` is the return route from one
exact owner-local DWDSmor A run. It independently parses every private raw
row, recomputes denominators, distributions, unknown-residue shape,
determinism, performance, byte counts, and artifact fixity, then writes only a
text-free aggregate receipt. It retains the private run by opaque owner
reference. Mechanical residue is not reviewed residue: the recorder cannot
accept German, morphology, a lemma or lexeme, open B/C or a human task, clear
rights, authorize publication, or create semantic and graph authority.

`build_zarathustra_morphology_context_packet.py` is the additive bridge from
one already frozen A ambiguity and the complete selected-form recurrence to a
bounded B question. It selects recurrence ranks 1, inclusive-median, and last
without seeing B/C output, returns those occurrences to exact raw TEI
paragraph or verse-group character data, and writes the three source-bearing
rows only under ignored mode-0600 `local-content/`. Its tracked receipt and
provenance retain digests, rank roles, counts, variant state, and zero-effect
gates. The builder acquires or runs no model, admits no C normalization for an
unrelated POS question, and creates no accepted German, morphology, lemma,
lexeme, sign, semantic claim, publication route, or human backlog.

`record_zarathustra_visual_retrieval_result.py` is the return route from one
exact owner-local direct-page-image retrieval C run. It independently
reconstructs frozen-input and artifact fixity, normalization evidence,
repeat-ranking stability, source-anchor resolution, advisory A/B/C aggregates,
resource cost, and whether a declared rare-review trigger opened. The tracked
receipt contains no source or query strings, page images, vectors, or absolute
owner paths. The recorder cannot accept relevance, transcription, translation,
a sign or semantic relation, choose a winner, adopt or promote the method,
authorize publication, create a routine human backlog, or materialize another
review interface.

`build_witness_structure_correspondence.py` uses those inventories plus
explicit local DTA/EPUB payloads to produce a tracked, text-free map of named
division-start candidates and stable proposed TEI, EPUB-member, and PDF-page
addresses for each candidate. `validate_witness_structure_correspondence.py`
also checks the model-reviewed parallel-PDF division map for the Naumann 1886
and Polilov/Mysl 1996 Jenseits witnesses: inventory closure, proposed page
addresses, contiguous division-level number spans, explicit absence of exact
per-unit page claims, provenance, monotonicity, and the authority ceiling. The
addresses are reusable IDs, not verified passage boundaries. Neither script
accepts textual identity, original-language text, translation, semantics,
rights, or canon.

`build_jenseits_numbered_unit_structure.py` is the exact local companion for
the Naumann 1886 scan package. It combines ordered ABBYY numeral candidates
with an explicit bounded set of model-visible scan-review results, then emits
only 299 proposed start-page addresses for §§1–296 plus 65a, 73a, and 237a.
The release-safe structure validator checks the tracked result without needing
the local payloads. Neither a complete address set nor a green validator
accepts the OCR, the German text, exact line boundaries, a critical-edition
equivalence, a translation, or semantics.

`build_jenseits_polilov_numbered_unit_structure.py` is the separate target-only
companion for the Polilov/Mysl 1996 expression. It compares plain text,
unordered bbox candidates, and ordered bbox candidates plus bounded visible
gap review over the exact local PDF, then emits 298 proposed target-label
starts for 1–296, 65a, and 73a. It preserves source-only 237a as an explicit
nonmaterialized asymmetry instead of manufacturing a Russian label or
source-to-target alignment. Its green rebuild and validator prove only
deterministic address closure, not accepted Russian, translation equivalence
or quality, semantics, rights, or canon.

`build_nietzsche_transfer_source_structure.py` is the explicit-local German
companion for the already frozen *Genealogie* and *Antichrist* transfer frame.
It keeps the 1892 PDF and the 1906 Commons DjVu / separate Internet Archive
DjVuXML Item boundaries visible, emits 140 proposed series-qualified source
starts and anchors, and records machine versus bounded source-visible gap
basis. `build_nietzsche_transfer_source_routes.py` then reads only tracked maps
to pair the same `series:unit` labels and give the twelve frozen target pages
twenty possible German structural routes. The release-safe
`validate_nietzsche_transfer_source_routes.py` closes inventories, rights,
anchors, digests, provenance, pairings, and routes. None of these routes accepts
German or Russian text, creates exact passage ends or passage/translation
alignment, or opens eligibility, gold, semantic, canon, or human work.

`build_golden_kernel_transfer_target_passages.py` returns from those conservative
routes to the exact ignored Mysl PDF through explicit payload and output roots.
It keeps the frozen twenty-page frame unchanged, slices all thirty-five routes
from one numbered label to the next same-series label, writes private mode-0600
content under ignored `local-content/`, and emits only text-free geometry,
digests, proposed/rejected anchors, and provenance to Git. The companion
validator preserves thirty-two page intersections and three rejected
nonintersections. Neither route creates diplomatic or accepted Russian, a
source passage or source-to-target alignment, eligibility, gold, semantics,
canon, publication authority, or human work.

`build_golden_kernel_transfer_source_passages.py` is the explicit-local German
companion over the same thirty-five routes. It slices from one expected
numbered label to the next only when both boundaries resolve inside one named
ABBYY, DjVuXML, or PDF-bbox layer, or through an exact source-visible JP2
marker bound by same-Item scandata to the first following DjVuXML line, or an
exact embedded PDF image-mask marker bound to the first following Poppler bbox
record in that same fixity-bound PDF. It writes all thirty-five mode-0600
candidates under ignored `local-content/`. The validator fixes the two PDF
marker returns and four affected JP2-marker routes, keeps content versus
address witnesses distinct, and preserves the bounded two-page Antichrist
navigation relation that asserts no textual identity. It creates no diplomatic
or accepted German, source-to-target passage alignment, eligibility, gold,
human work, semantics, publication authority, or canon effect.

`build_golden_kernel_transfer_route_readiness.py` is a tracked-only derived
projection over the source and target candidate sets. It reads no private
content. It closes all thirty-five candidate identities, reports thirty-two
frozen-page intersections plus three target nonintersections, and verifies that
each of the twenty frozen pages still has at least one dual mechanically
available intersecting route. Its companion validator treats co-availability
as navigation only: it must never become a bilingual passage pair,
source-to-target alignment, eligibility, gold, human task, semantic opening,
publication grant, or canon effect.

`build_jenseits_numbered_unit_label_correspondence.py` is the release-safe
companion over the two independently generated Jenseits numbered-unit maps.
It reads no payload or text: it intersects exact materialized `unit_key`
values, resolves both proposed anchors, emits 298 proposed shared-label
pairings, and keeps source-only 237a unpaired. The output is reusable
structural navigation, not a source-to-target passage alignment, translation
correspondence, equivalence, quality judgment, or semantic relation.

`build_mysl_transfer_target_structure.py` is the explicit-local target-only
companion for the frozen *Genealogie* and *Antichrist* transfer pages in the
exact Mysl collection Item. It preserves independently resetting number series,
combines ordered embedded-PDF bbox candidates with a fixed model-visible gap
review, and emits only series-qualified proposed whole-page starts, anchors,
target-only crosswalks, and provenance. Its `--check` route requires the exact
ignored PDF; the release-safe source-foundation validator closes tracked
schema, digest, transfer-plan, count, and zero-authority invariants without the
payload. Neither route creates a German parallel unit map, passage alignment,
accepted Russian, translation relation, eligible transfer unit, target gold,
semantics, rights clearance, or human work.

`build_zarathustra_german_source_triangulation.py` is the explicit-local
companion for one German source question. It reads one ignored eKGWB HTTP
response, the exact DTA part-I TEI, and two exact Naumann automatic-EPUB
members from a caller-supplied payload root. It emits a reconciled discovery
record, a text-free comparison packet, and provenance. Its source-aware
normalization and aggregate agreement do not authenticate HTTP transport,
admit a critical edition, accept German, clear rights, open translation, or
create semantic/canon authority.

`build_zarathustra_bounded_translation_input.py` is the next, narrower
explicit-local companion. It deterministically derives the first complete
sentence from the exact DTA part-I provider transcription, requires exact
eKGWB and first-twenty-token Naumann corroboration, writes the source string
only to an explicit ignored local-output root, and emits a tracked text-free
admission packet plus provenance. Its sole authority is local blind
model/prompt/runtime calibration. It does not alter the frozen accepted-
translation plan, accept German, reveal a comparator, clear rights, authorize
redistribution, or open semantic, graph, or canon work. The builder also binds
the older authored canon node and compatibility mirror so their translations
can be explicitly sealed from blind candidates; it does not revalidate them.

`experimental-translation-episode.schema.json` and the source-witness
validator receive repeated single-candidate returns from the owner-local
`abyss-stack` laboratory. They keep a pre-candidate infrastructure failure
distinct from a candidate-bearing AI Russian-surface triage, bind admission,
source fixity, research, profile, runner, model, runtime, private receipts,
measurements, and provenance, and prohibit tracked source/candidate text or
private paths. A green episode proves neither German correctness, translation
fidelity, etymology, semantics, human review, publication, graph truth, nor
canon authority.

`build_philosophy_atlas_projection.py` and
`validate_philosophy_atlas_projection.py` publish the first atlas-shaped
tree/graph read model from `ToS/philosophy/atlas/` for review and downstream
visualization.

`build_philosophy_graph_views.py` and `validate_philosophy_graph_views.py`
publish the graph-view switching catalog from source-owned view cards and
`view-contracts.json`.

`build_philosophy_graph_projection.py` and
`validate_philosophy_graph_projection.py` materialize the atlas projection and
graph-view catalog into source-ref-preserving graph packets for downstream UI,
MCP, and Neo4j access planes. They do not create runtime authority inside ToS.

`build_source_witness_bibliographic_graph.py` and
`validate_source_witness_bibliographic_graph.py` form a separate derived route
over the public-safe source-witness object/claim catalog. They reify every
bibliographic claim, preserve identity-versus-literal typing, and require each
edge to carry source-line/digest, evidence, maker, provenance time/method, and
review return. They emit no direct subject-to-object fact edge, do not accept a
claim, do not merge with the atlas graph, and do not own runtime Neo4j,
Oxigraph, MCP, UI, or service behavior.

For `provision_activity`, the graph may add claim-originating
`has_normalized_place` and `has_normalized_agent` edges to the exact cataloged
Place, Agent, or Organization identity. These convenience edges remain part of
the claim trace and may never bypass the claim node.

`query_source_witness_bibliographic_graph.py` is the read-only local companion.
It validates the graph and requires an exact source-backed canonical rebuild
before answering explicit claim, subject, identity-object, predicate,
review-status, or visibility selectors. Combined selectors are ANDed. Every
match includes the exact source JSONL object and complete graph trace; the
reader also accepts an exact normalized identity selector, writes no files,
rejects selector-free requests, and fails rather than
silently truncate an over-limit result. Its stdout is navigation, not claim
truth, human review, runtime API, MCP, UI, backend, or canon authority.

`build_philosophy_post_planting_audit.py` and
`validate_philosophy_post_planting_audit.py` publish the compact Table I
post-planting review packet from the planted atlas, branch, workbench, and
graph projection surfaces. They check review readiness; they do not promote
prepared dossier material to canon.

`plant_prepared_dossiers.py` is the prepared-dossier planting entrypoint. It
reports package readiness by default and delegates supported planting to the
package implementation. `plant_table_i_prepared_dossiers.py` remains the Table I
implementation until Table II or Table III receives its own dossier route map.

## Operating Card

| Field | Route |
| --- | --- |
| role | deterministic generator and validator lane for current ToS surfaces |
| input | schema, example, intake, canon, decision, topology, or export surface |
| output | generated payload, generated index, or pass/fail validation signal |
| owner | `scripts/AGENTS.md` and the exact script being changed |
| next route | source surface -> script update -> generated artifact when needed -> validator |
| tools | local Python scripts, unittest, schema files, generated parity checks |
| check | affected script plus the release lane in `docs/validation/validation_lanes.json` for broad changes |

## Editing posture

Keep each generator or validator narrow to its declared source route.

Prefer:
- explicit file dependencies
- clear exit conditions
- reproducible transforms
- visible validation boundaries

Avoid:
- network calls
- hidden state
- broad corpus automation without an explicit derived-export contract
- discovery magic that blurs ownership
- turning validators into a runtime or orchestration control plane

The scripts should serve the source-first route, not become a kingdom of their own.

`scripts/release_check.py` is a runner for the `release_check` command sequence
declared in `docs/validation/validation_lanes.json`. Keep command composition
there, not as a hidden list inside Python code.

Every active file under `*/scripts/*` must stay represented in
`docs/validation/script_inventory.json`. That inventory describes owner,
source truth, side effects, lane posture, CI inclusion, and focused test target;
it does not execute commands or promote advisory skill helpers into release
authority.

## Boundary Routes

- Canonical source for authored meaning routes to `ToS/canon/` and
  `ToS/source-witnesses/`; compatibility examples remain mirrors.
- Generation logic follows contracts and owning source surfaces.
- Scope widening routes through a visible source, contract, decision, and
  validator change.
- Adjunct quest or progression behavior stays compatibility-only unless the
  owning ToS doctrine and contracts change.

## Validation

Run the affected script directly. For broad or release-visible changes, run the
repo gate:

```bash
python scripts/release_check.py
```

Local owner routes:

| Pressure | Route |
| --- | --- |
| public tiny entry | `python scripts/validate_tiny_entry_route.py` |
| bounded KAG export | `python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py` when inputs move, then `python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py` |
| questbook surface | `python mechanics/questbook/scripts/validate_questbook_surface.py` |
| corpus index | `python scripts/build_tos_corpus_index.py --check` and `python scripts/validate_tos_corpus_index.py` |
| philosophy atlas projection | `python scripts/build_philosophy_atlas_projection.py --check` and `python scripts/validate_philosophy_atlas_projection.py` |
| philosophy graph views | `python scripts/build_philosophy_graph_views.py --check` and `python scripts/validate_philosophy_graph_views.py` |
| philosophy graph projection | `python scripts/build_philosophy_graph_projection.py --check` and `python scripts/validate_philosophy_graph_projection.py` |
| source-witness bibliographic graph query | `python scripts/query_source_witness_bibliographic_graph.py --claim-ref <exact-claim-id>` after graph parity and validation |
| philosophy post-planting audit | `python scripts/build_philosophy_post_planting_audit.py --check` and `python scripts/validate_philosophy_post_planting_audit.py` |
| decision indexes | `python scripts/generate_decision_indexes.py --check` and `python scripts/validate_decision_records.py` |
| source-home or branch topology | `python scripts/validate_tos_source_home.py` and `python scripts/validate_philosophy_topology.py` |
| source-witness evidence spine | `python scripts/build_source_witness_catalog.py --check` and `python scripts/validate_source_witness_foundation.py`; add `--require-local-payloads` only for a machine expected to hold the local corpus |
| public synthetic source-text-unit and segmentation A/B/C | `python scripts/build_source_text_unit_v1_lab.py --build`, then `python scripts/validate_source_witness_foundation.py --source-text-unit-v1-lab-only`; rebuilding is intentional because the manifest binds the invented text, research, contract, builder, plan, and variants |
| real bounded `Za-I-Vorrede-1` source-text foundation | `python scripts/build_zarathustra_source_text_foundation.py --check --local-input-root /srv/AbyssOS/Tree-of-Sophia --local-output-root /srv/AbyssOS/Tree-of-Sophia`; use `--build` only to intentionally recreate the ignored mode-0600 paragraph plus the four tracked text-free records; green closure establishes no accepted German, linguistic boundary, translation, semantics, publication, or canon effect |
| real bounded Antonovsky 1911 target-text foundation | `python scripts/build_zarathustra_target_text_foundation.py --check --local-input-root /srv/AbyssOS/Tree-of-Sophia --local-output-root /srv/AbyssOS/Tree-of-Sophia`; use `--build` only to intentionally recreate the ignored mode-0600 embedded-text paragraph and bbox byproduct plus four tracked text-free records; green closure preserves the observed spacing conflict and establishes no accepted Russian, bilingual pair, alignment, translation, semantics, publication, or canon effect |
| real bounded `Za-I-Vorrede-1` opening-sentence alignment proposal | `python scripts/build_zarathustra_opening_sentence_alignment.py --check --local-input-root /srv/AbyssOS/Tree-of-Sophia`; use `--build` only to intentionally recreate the four tracked text-free sentence-unit/alignment/provenance records from the two existing ignored mode-0600 paragraph layers; green closure establishes only exact selectors, digests, source return, and one proposed ordinal correspondence—not accepted boundaries, German/Russian, translation fidelity, lexical equivalence, review, projection, semantics, publication, graph, or canon effect |
| local Antonovsky 2007/1911 witness-text collation proposal | `python scripts/build_antonovsky_2007_1911_collation.py --check --repo-root . --owner-root /srv/AbyssOS/Tree-of-Sophia --artifact-root /srv/abyss-machine/storage/artifacts`; use `--build` only to intentionally recreate the ignored mode-0600 p011 observation/detail and five tracked text-free records; green closure establishes deterministic reuse, selector/digest/metric/fixity closure, and one proposed same-language correspondence—not human review, gold, accepted text or boundaries, preferred reading, equivalence, genealogy/derivation, translation, semantics, projection, publication, graph, or canon effect |
| authored Zarathustra route evidence bridge | `python scripts/build_zarathustra_authored_canon_evidence_bridge.py --check --local-input-root /srv/AbyssOS/Tree-of-Sophia --local-output-root /srv/AbyssOS/Tree-of-Sophia`, then `python scripts/validate_source_witness_foundation.py`; use `--build` only to intentionally recreate four ignored mode-0600 exact/comparison artifacts and six tracked text-free records; green closure proves the complete 12/12 normalized representation crosswalk and current 92-node/125-relation inventory, not accepted German/translation, modern human review, claim/evidence or semantic closure, graph/canon promotion, publication, transfer, or migration authority |
| public synthetic semantic identity/annotation A/B/C | `python scripts/build_semantic_annotation_v2_lab.py --build`, then `python scripts/validate_source_witness_foundation.py --semantic-annotation-v2-lab-only`; rebuilding is intentional because the manifest binds all fixture bytes |
| public synthetic translation-alignment identity A/B/C | `python scripts/build_translation_alignment_v1_lab.py --build`, then `python scripts/validate_source_witness_foundation.py --translation-alignment-v1-lab-only`; rebuilding is intentional because the manifest binds both invented text sides, analysis artifacts, plan, research, contract, builder, and variants |
| local source resource inventories | `python scripts/build_source_resource_inventories.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check`; omit `--check` only for intentional regeneration from fixity-verified local bytes |
| local Zarathustra lexical observation index | `python scripts/build_zarathustra_lexical_index.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --local-output-root /srv/AbyssOS/Tree-of-Sophia --check`, then `python scripts/validate_zarathustra_lexical_index.py --local-output-root /srv/AbyssOS/Tree-of-Sophia`; omit `--check` only for an intentional deterministic rebuild of the tracked hash/resource projection and ignored SQLite/FTS5 database |
| tracked Zarathustra exact-form recurrence observations | `python scripts/build_zarathustra_recurrence_projection.py --check`; omit `--check` only to intentionally rebuild the tracked hash-only frequency/range/DP tuple and provenance from the frozen plan and tracked lexical projection |
| private question-scoped Zarathustra usage context | `python scripts/build_zarathustra_usage_context_bundle.py --local-input-root /srv/AbyssOS/Tree-of-Sophia --local-output-root /srv/AbyssOS/Tree-of-Sophia --check`, then `python scripts/validate_zarathustra_lexical_index.py`; omit `--check` only to intentionally rebuild the ignored mode-0600 exact-context JSONL plus its tracked string-free receipt and provenance from the frozen question |
| private selected-form Zarathustra raw-witness recurrence | `python scripts/build_semantic_source_recurrence_bundle.py --local-input-root /srv/AbyssOS/Tree-of-Sophia --local-output-root /srv/AbyssOS/Tree-of-Sophia --check`, then `python scripts/validate_source_witness_foundation.py`; omit `--check` only to intentionally rebuild the ignored mode-0600 complete occurrence-return bundle plus its tracked string/position-free receipt and provenance from the frozen selected hash |
| local Zarathustra morphology census input | `python scripts/build_zarathustra_morphology_input.py --local-input-root /srv/AbyssOS/Tree-of-Sophia --local-output-root /srv/AbyssOS/Tree-of-Sophia --check`; omit `--check` only to intentionally rebuild the ignored exact-form JSONL and its tracked text-free receipt before any morphology output |
| private Zarathustra morphology A result | `python scripts/record_zarathustra_morphology_census_result.py --run-root /srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/tos-historical-german-morphology-v1/<run-id>/variant-A --check`; omit `--check` only to intentionally record an exact owner-local run as the tracked text-free aggregate receipt |
| private Zarathustra direct-visual retrieval C result | `python scripts/record_zarathustra_visual_retrieval_result.py --run-root /srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/tos-visual-retrieval-foundation-v1/<run-id>/variant-C --prior-run-root /srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/tos-visual-retrieval-foundation-v1/<prior-run-id>/variant-C --local-query-content /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/retrieval/queries.v1.json --check`; omit `--check` only to intentionally record the exact owner-local run as a tracked text-free mechanical and trigger receipt |
| local witness structure correspondence | `python scripts/build_witness_structure_correspondence.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check` for local regeneration parity, then `python scripts/validate_witness_structure_correspondence.py` for tracked release-safe closure |
| local Jenseits numbered-unit structure | `python scripts/build_jenseits_numbered_unit_structure.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check`, then `python scripts/validate_witness_structure_correspondence.py` for tracked release-safe closure |
| local Jenseits target numbered-unit structure | `python scripts/build_jenseits_polilov_numbered_unit_structure.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check`, then `python scripts/validate_witness_structure_correspondence.py` for tracked release-safe closure |
| Jenseits shared number-label candidates | `python scripts/build_jenseits_numbered_unit_label_correspondence.py --check`, then `python scripts/validate_witness_structure_correspondence.py`; this route reads tracked maps only |
| local Mysl Genealogie/Antichrist target structure | `python scripts/build_mysl_transfer_target_structure.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check`, then `python scripts/validate_source_witness_foundation.py`; the local route reads only the exact ignored target PDF, while release validation uses tracked text-free outputs |
| local Genealogie/Antichrist German source structure | `python scripts/build_nietzsche_transfer_source_structure.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check`, then `python scripts/validate_nietzsche_transfer_source_routes.py`; local payloads remain ignored and the tracked result is text-free |
| Genealogie/Antichrist shared labels and frozen-candidate source routes | `python scripts/build_nietzsche_transfer_source_routes.py --check`, then `python scripts/validate_nietzsche_transfer_source_routes.py`; this route reads tracked maps only and preserves zero text/alignment/eligibility/gold/human/semantic effects |
| private golden-kernel target numbered-unit passage candidates | `python scripts/build_golden_kernel_transfer_target_passages.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --local-output-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check`, then `python scripts/validate_golden_kernel_transfer_target_passages.py --local-output-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses`; tracked release validation omits the private root and preserves the 32-intersection/3-rejection boundary |
| private golden-kernel German source passage candidates | `python scripts/build_golden_kernel_transfer_source_passages.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --local-output-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check`, then `python scripts/validate_golden_kernel_transfer_source_passages.py --local-output-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses`; tracked release validation omits the private root and preserves the 32-materialized/3-unresolved boundary, including exact text-free JP2/scandata marker returns, without accepting German or alignment |
| local Zarathustra German source triangulation | `python scripts/build_zarathustra_german_source_triangulation.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check`, then `python scripts/validate_source_witness_foundation.py`; the release lane validates tracked closure without requiring the ignored inputs |
| bounded local Zarathustra translation calibration input | `python scripts/build_zarathustra_bounded_translation_input.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --local-output-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check`, then `python scripts/validate_source_witness_foundation.py`; the two explicit roots respectively supply exact inputs and own the ignored source-bearing output |
| canon/example contracts | `python scripts/validate_tree_node_contracts.py`, `python mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py`, or `python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py` |
