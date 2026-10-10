# ToS Generated

This directory holds derived export surfaces for bounded downstream consumption.

Generated exports return to their authored sources. Canonical authority
remains in `../canon/`, while public entry mirrors live in
`../public-compatibility/`.

## Current role

Use `ToS/derived-exports/` when you need:

- a compact downstream-safe export of the current bounded route
- a reviewable derived payload for KAG-oriented consumers
- a checked whole-corpus index for runtime graph, UI, and MCP access planes

Follow an export back to its authored route, canonical node, source-owned
capsule and tiny-entry documentation. Runtime storage and presentation follow
their own consumer contracts.

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

Authored relation CSV provenance is exposed only through the selected
`authored-corpus` native source-read route. Exact `authored_csv_record` targets
bind pack/edge identity, logical source row, source-file SHA-256, and canonical
parsed-cell content revision; the retained native source model preserves
original string/null cells and raw-row provenance. Native inspection, source
handles, and the human/MCP readers use the same selected-source operations.
Availability depends on the captured/published source vector and managed
native release; old vectors without the selected corpus route remain
unsupported. This access preserves source and canon/intake posture and grants
neither semantic assessment nor publication rights.

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
and selected alias pointers. These envelopes preserve authored atlas provenance. Native Corpus Record
identity, source assessment, publication and canon use their corresponding
owner contracts.

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
The native `corpus-projection-query` owner verifies source-backed parity
before answering exact claim, subject, identity-object, normalized identity,
predicate, review-status, or visibility selectors. It returns deterministic
JSON with the exact source claim and full trace, writes no state, and refuses
an over-limit match set. The native request route is documented in
[`Data and corpus operations`](../../docs/RELEASING.md#data-and-corpus-operations).
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
- `tos-ops-mechanics-plan --kag-source-export-verify --kag-export EXPORT`
- `tos-ops-mechanics-plan --repo-root ABS --root-entry-map-build --check --kag-export EXPORT`
- `tos-ops-mechanics-plan --repo-root ABS --root-entry-map-validate --kag-export EXPORT`
- Native tracked corpus-index and bibliographic-graph parity: see [`Data and corpus operations`](../../docs/RELEASING.md#data-and-corpus-operations).
- `tos lexical-index validate-tracked --source-root /srv/AbyssOS/Tree-of-Sophia --local-output-root /srv/AbyssOS/Tree-of-Sophia`
- `tos-ops-mechanics-plan --philosophy-product atlas --source-root "$PWD" --output-root "$PWD" --mode check`
- `tos-ops-mechanics-plan --philosophy-product atlas --source-root "$PWD" --output-root "$PWD" --mode validate`
- `tos-ops-mechanics-plan --philosophy-product views --source-root "$PWD" --output-root "$PWD" --mode check`
- `tos-ops-mechanics-plan --philosophy-product views --source-root "$PWD" --output-root "$PWD" --mode validate`
- `tos-ops-mechanics-plan --philosophy-product graph --source-root "$PWD" --output-root "$PWD" --mode check`
- `tos-ops-mechanics-plan --philosophy-product graph --source-root "$PWD" --output-root "$PWD" --mode validate`
- `tos-ops-mechanics-plan --philosophy-product audit --source-root "$PWD" --output-root "$PWD" --mode check`
- `tos-ops-mechanics-plan --philosophy-product audit --source-root "$PWD" --output-root "$PWD" --mode validate`
- `tos evidence-projection check --source-root "$PWD"`
- `tos evidence-projection validate --source-root "$PWD"`

The lexical maintainer is the native `tos lexical-index` command. `build` creates a fresh private candidate from an explicit source cut; it does not overwrite the retained projection or local database. `validate-tracked` checks the read-only source closure and optional local database fixity. Historical provenance keeps its original [historical lexical builder](https://github.com/8Dionysus/Tree-of-Sophia/blob/c04257b4f2270587856ac94d3fb28a5a9d5afa25/scripts/build_zarathustra_lexical_index.py) reference; exact generator bytes are retained under `ToS/research-packets/retained-builder-inputs/build_zarathustra_lexical_index/` and are not executable fallbacks.

Select a private, already reserved scratch directory through
`TOS_EVIDENCE_STAGING_PARENT` and its remaining byte quota through
`TOS_EVIDENCE_SCRATCH_BYTES` (or `--staging-parent` / `--scratch-bytes`). Each
command owns and removes one fresh SQLite directory. The explicit
`--staging FRESH_ABSOLUTE_PATH` form retains the selected capture for its caller.
`tos evidence-projection build --source-root "$PWD" --replace` updates the
companion atomically only while its captured prior bytes remain current. An
explicit `--output ABSOLUTE_PATH` chooses another generated destination.
The Rust compiler is the only implementation; native failure has no fallback.
- The corpus-projection owner recomposes and compares the tracked graph as part of its source-backed parity check.
- `tos-ops-mechanics-plan --artifact-bundle --repo-root ROOT`
