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
tracked catalog. The release lane permits gitignored payload bytes to be absent
from a public clone; an operator may add `--require-local-payloads` for the
local laboratory. Passing this lane never implies that a human accepted the
bibliography, text, rights, translation, or semantics.

`build_source_resource_inventories.py` is the focused local companion for
fixity-verified payloads. It writes tracked, text-free PDF page, EPUB resource,
TEI structure, and provider OCR page inventories from an explicit payload
root. It is not a release-time acquisition step and its one-way fingerprints
are not accepted text or alignment evidence.

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

`build_jenseits_numbered_unit_label_correspondence.py` is the release-safe
companion over the two independently generated Jenseits numbered-unit maps.
It reads no payload or text: it intersects exact materialized `unit_key`
values, resolves both proposed anchors, emits 298 proposed shared-label
pairings, and keeps source-only 237a unpaired. The output is reusable
structural navigation, not a source-to-target passage alignment, translation
correspondence, equivalence, quality judgment, or semantic relation.

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
| philosophy post-planting audit | `python scripts/build_philosophy_post_planting_audit.py --check` and `python scripts/validate_philosophy_post_planting_audit.py` |
| decision indexes | `python scripts/generate_decision_indexes.py --check` and `python scripts/validate_decision_records.py` |
| source-home or branch topology | `python scripts/validate_tos_source_home.py` and `python scripts/validate_philosophy_topology.py` |
| source-witness evidence spine | `python scripts/build_source_witness_catalog.py --check` and `python scripts/validate_source_witness_foundation.py`; add `--require-local-payloads` only for a machine expected to hold the local corpus |
| local source resource inventories | `python scripts/build_source_resource_inventories.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check`; omit `--check` only for intentional regeneration from fixity-verified local bytes |
| local Zarathustra lexical observation index | `python scripts/build_zarathustra_lexical_index.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --local-output-root /srv/AbyssOS/Tree-of-Sophia --check`, then `python scripts/validate_zarathustra_lexical_index.py --local-output-root /srv/AbyssOS/Tree-of-Sophia`; omit `--check` only for an intentional deterministic rebuild of the tracked hash/resource projection and ignored SQLite/FTS5 database |
| local Zarathustra morphology census input | `python scripts/build_zarathustra_morphology_input.py --local-input-root /srv/AbyssOS/Tree-of-Sophia --local-output-root /srv/AbyssOS/Tree-of-Sophia --check`; omit `--check` only to intentionally rebuild the ignored exact-form JSONL and its tracked text-free receipt before any morphology output |
| private Zarathustra morphology A result | `python scripts/record_zarathustra_morphology_census_result.py --run-root /srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/tos-historical-german-morphology-v1/<run-id>/variant-A --check`; omit `--check` only to intentionally record an exact owner-local run as the tracked text-free aggregate receipt |
| local witness structure correspondence | `python scripts/build_witness_structure_correspondence.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check` for local regeneration parity, then `python scripts/validate_witness_structure_correspondence.py` for tracked release-safe closure |
| local Jenseits numbered-unit structure | `python scripts/build_jenseits_numbered_unit_structure.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check`, then `python scripts/validate_witness_structure_correspondence.py` for tracked release-safe closure |
| local Jenseits target numbered-unit structure | `python scripts/build_jenseits_polilov_numbered_unit_structure.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check`, then `python scripts/validate_witness_structure_correspondence.py` for tracked release-safe closure |
| Jenseits shared number-label candidates | `python scripts/build_jenseits_numbered_unit_label_correspondence.py --check`, then `python scripts/validate_witness_structure_correspondence.py`; this route reads tracked maps only |
| local Zarathustra German source triangulation | `python scripts/build_zarathustra_german_source_triangulation.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check`, then `python scripts/validate_source_witness_foundation.py`; the release lane validates tracked closure without requiring the ignored inputs |
| bounded local Zarathustra translation calibration input | `python scripts/build_zarathustra_bounded_translation_input.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --local-output-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --check`, then `python scripts/validate_source_witness_foundation.py`; the two explicit roots respectively supply exact inputs and own the ignored source-bearing output |
| canon/example contracts | `python scripts/validate_tree_node_contracts.py`, `python mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py`, or `python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py` |
