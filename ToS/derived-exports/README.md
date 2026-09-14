# ToS Generated

This directory holds derived export surfaces for bounded downstream consumption.

Generated files here do not replace ToS-authored authority.
Canonical authority remains in `../canon/`, while the public entry mirrors remain in `../public-compatibility/`.

## Current role

Use `ToS/derived-exports/` when you need:

- a compact downstream-safe export of the current bounded route
- a reviewable derived payload for KAG-oriented consumers
- a checked whole-corpus index for runtime graph, UI, and MCP access planes

Do not treat `ToS/derived-exports/` as:

- the primary authored home of the route
- a replacement for the canonical tree node
- an excuse to skip the source-owned capsule and tiny-entry docs
- a runtime projection store or graph UI authority

## Current bounded exports

The current generated export surfaces are:

- `kag_export.json`
- `kag_export.min.json`
- `root_entry_map.min.json`
- `tos_corpus_index.min.json`
- `philosophy_atlas_projection.min.json`
- `philosophy_graph_views.min.json`
- `philosophy_graph_projection.min.json`
- `epistemic_evidence_projection.min.json`
- `graph/source-witness-bibliographic-claims.min.json`
- `lexical-search/zarathustra-dta-first-editions-parts-1-4-v1.min.json`

Most summarize the current Zarathustra route for downstream consumers while
pointing back to ToS-owned authority and compatibility surfaces. The
source-witness bibliographic graph instead spans the bounded current Nietzsche
object/claim catalog without claiming corpus completeness.
The root entry map is the machine-facing entry capsule for consumers that need
schema-checked root-route orientation before touching downstream exports.
The corpus index covers the whole `ToS/` home as a derived resource map so
`abyss-stack` can project and visualize the corpus without owning ToS meaning.
Its canonical and candidate CSV relation carriers retain the complete parsed
row in `properties.source_record`, including unknown columns, with
`source_file_sha256` and the one-based `source_row` data-record ordinal.
The ordinal excludes the header and is not a physical line number; a quoted
cell can span lines. Null missing cells differ from empty strings, and the
file hash binds the original bytes rather than a reserialized CSV. The shared
reader returns this metadata in relation `attributes` and returns the source
path through the relation pack. It does not interpret unknown fields, turn
an intake `promoted` marker into current canon admission, reinterpret source
confidence, or replace a Claim's evidence/assessment model. Older snapshots
may lack this optional binding; new snapshots must travel with their matching
corpus schema and pass source-backed parity. Ambiguous duplicate/empty headers
and unnamed surplus cells are rejected rather than silently dropped.

The owner library `scripts/tos_corpus_index_common.py` exposes
`read_exact_edge_row(path, source_file_sha256=..., source_row=...,
source_record=...)` for an explicitly selected index row. It verifies the
unchanged regular file and every parsed cell, returning the original CSV record
(including its delimiter), byte offset, row byte count and raw-record SHA-256.
Multiline cells do not change logical row numbering. Default file/record read
budgets are 8 MiB/1 MiB; stale hashes, wrong rows, malformed CSV and exceeded
budgets refuse without rewriting source or index. This helper does **not**
admit a caller path, establish selected-index membership or grant publication
rights. `scripts/authored_corpus_source_read.py` supplies the optional addressed
adapter: explicit bootstrap verifies all rows of each selected tracked pack
against the existing corpus index and writes an unselected immutable root.
Selecting that root as `authored-corpus` in the source vector enables exact
`authored_csv_record` handles through the common source-read contract. This
changes the vector revision; old prepared readers cannot acquire the root by
silent attachment. Source files and canon/intake posture remain unchanged.
Serving looks up one selected pack/edge member and then rechecks its exact CSV
bytes, without a corpus scan or accepting a caller path. Bootstrap is bounded
to 4,096 packs, 65,536 rows and 64 MiB of selected input; a larger bootstrap
requires an explicitly designed owner route, not raised serving budgets.

The philosophy atlas projection turns `ToS/philosophy/atlas/` into a first
reviewable tree/graph read model for visualization and graph switching.
The philosophy graph view catalog turns source-owned view cards and
`view-contracts.json` into downstream-readable lens filters for `abyss-stack`.
The philosophy graph projection materializes the atlas projection once as a
source-ref-preserving node/edge set. Each graph view carries stable node/edge
ID membership over that set, avoiding a second full copy of the same records
inside every lens while keeping runtime access subordinate to ToS authority.

The atlas/graph readers retain complete public authored atlas manifests,
master rows, dossier index rows, proposed nodes and proposed relations in
`properties.source_record`. `source_record_ref` and `source_file_sha256`
bind the exact parsed file; `source_record_sha256` separately binds the
canonical JSON object. JSONL `source_row` counts nonblank records, while
`source_line` is the physical line. Whole JSON manifests use `source_pointer`
with the empty root pointer and invent no row number. Original DOCX table/row
indexes remain inside the unchanged source object, not those JSONL locators.
Unknown nested fields, null, false, empty and absent values stay distinct.
Applied endpoint aliases also retain their complete owner packet, claim limit
and selected alias pointers. These envelopes are not native Corpus Record
identity, source assessment, publication clearance or canon admission.

Every existing authored candidate node/relation remains globally inspectable,
including candidates selected by no current lens and their existing endpoint
closure. Such records have `view_ids: []`; source-owned view filters, view
membership and cluster denominators do not expand. Global fingerprints bind
unlensed bodies as well as IDs.

Existing `atlas-dossier:{dossier_id}` nodes also expose `properties.source_backlogs`:
`source_anchor_backlog`, `term_index` and `transmission_backlog`. Each family
returns its authored manifest's `source_ref`, exact `source_file_sha256`,
`record_count` and full `records` array of the same JSONL source envelopes.
All records remain under their source-declared dossier; all three families
remain present when their record arrays are empty. A missing source file is
an error, not an empty family. Raw source-local IDs, original DOCX coordinates,
status, constraints and unknown fields are retained without semantic mapping.

These backlogs have no global stable record IDs. Address an occurrence by the
existing dossier ID, family source ref, exact file digest and `source_row` /
`source_line`; this is a snapshot locator, not a minted corpus or graph identity.
Identical rows on distinct source lines remain distinct occurrences. Branch
anchor mirrors are not counted a second time. Reviewed discovery leads keep
their separate, limited source selectors and do not confer acceptance on the
backlog. No new nodes, relations, lens membership or canon status are created.
Ordinary philosophy and knowledge node inspection returns these arrays intact;
its relation limit does not truncate node attributes. The separate human-Form
selection byte budget does not apply to full raw-record inspection.

The epistemic evidence projection joins two bounded, public-safe research
scenes to explicit source, review, canon, claim, and rights return routes. It
does not copy source text or infer closure: the Zarathustra scene distinguishes
retained canon from still-open modern claim/evidence work, while the Archaic
Tribute scene keeps a contested pre-canon reading separate from verified
bibliographic identity.
The source-witness bibliographic graph separately projects the public-safe
object/claim catalog into a claim-reified graph. Every edge returns to the
claim packet, evidence, maker, provenance event, review posture, exact source
line, and digest; it contains no unqualified subject-to-object edge and does
not widen the atlas projection into a bibliographic owner. The local
`scripts/query_source_witness_bibliographic_graph.py` reader verifies a fresh
source-backed rebuild before answering exact claim, subject, identity-object,
predicate, review-status, or visibility selectors. It returns deterministic
stdout JSON with the exact source claim and full trace, writes no state, and
fails rather than truncate an over-limit match set.
The lexical projection is a source-withholding, non-sequential companion over
four local DTA TEIs: form hashes, counts, and TEI page/division refs only. Its
source-bearing SQLite/FTS5 sibling remains gitignored. The hashes are
dictionary-recoverable, so the projection is not cleared for publication; both
outputs remain subordinate to source, rights, linguistic, semantic, and
runtime owners.
Only the subjects explicitly listed in
`mechanics/release-support/parts/artifact-bundles/manifests/generated_readmodel.bundle.json`
enter the current ABI-only OS Abyss artifact bundle. The lexical projection
has its own source-gated validator and does not enter that bundle.
The source-witness bibliographic graph is release-validated in ToS but does not
enter the artifact bundle until a downstream consumer contract explicitly
admits it.

## How to verify

Use:

- `../../mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`
- `../zarathustra/public-entry/TINY_ENTRY_ROUTE.md`
- `../public-compatibility/source_node.example.json`
- `python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py`
- `python scripts/build_root_entry_map.py --check`
- `python scripts/validate_root_entry_map.py`
- `python scripts/build_tos_corpus_index.py --check`
- `python scripts/validate_tos_corpus_index.py`
- `python scripts/build_zarathustra_lexical_index.py --payload-source-root /srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses --local-output-root /srv/AbyssOS/Tree-of-Sophia --check`
- `python scripts/validate_zarathustra_lexical_index.py --local-output-root /srv/AbyssOS/Tree-of-Sophia`
- `python scripts/build_philosophy_atlas_projection.py --check`
- `python scripts/validate_philosophy_atlas_projection.py`
- `python scripts/build_philosophy_graph_views.py --check`
- `python scripts/validate_philosophy_graph_views.py`
- `python scripts/build_philosophy_graph_projection.py --check`
- `python scripts/validate_philosophy_graph_projection.py`
- `python scripts/build_epistemic_evidence_projection.py --check`
- `python scripts/validate_epistemic_evidence_projection.py`
- `python scripts/build_source_witness_bibliographic_graph.py --check`
- `python scripts/validate_source_witness_bibliographic_graph.py`
- `python mechanics/release-support/parts/artifact-bundles/scripts/validate_abyss_machine_generated_readmodel_bundle.py`
