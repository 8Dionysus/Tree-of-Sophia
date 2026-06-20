# Tabular Base Contract

This document defines the current tabular base contract for the bounded
`thus-spoke-zarathustra/prologue-1` intake route.

It treats the current route-local workbook carrier at
`ToS/candidate-intake/thus-spoke-zarathustra/prologue-1/carriers/tos_kag_project_scheme.current.xlsx`
as a staging artifact.
It does not treat that workbook itself as ToS canon.

Only one live workbook carrier should remain per bounded route directory at a
time.
For this route, that means keeping a single live workbook under
`ToS/candidate-intake/thus-spoke-zarathustra/prologue-1/carriers/`.
When a new carrier supersedes the current one for the same route, replace the
file rather than accumulating versioned siblings in that active route-local
path.
Future chapters should keep their own route-local carrier files under their
own intake directories rather than widening one workbook across the whole book.

## Role

The tabular base contract exists to let ToS carry a reviewable graph-bearing
candidate pack without collapsing:

- source files in `ToS/source-witnesses/`
- candidate base tables in `ToS/candidate-intake/`
- authored source-node law in `ToS/canon/`
- public compatibility mirrors in `ToS/public-compatibility/`

The current route therefore keeps two different canonical surfaces:

- `ToS/canon/source/.../node.json` as the authored source-node canon
- `ToS/candidate-intake/.../mode-b/*.csv` as the candidate tabular base pack

These surfaces may agree and reinforce one another.
They do not replace one another.

## Current package

The current bounded tabular base pack consists of nine CSV files:

- `corpus_map.csv`
- `witnesses.csv`
- `segments.csv`
- `nodes.csv`
- `event_state_nodes.csv`
- `edges.csv`
- `translation_tensions.csv`
- `witness_glosses.csv`
- `principles.csv`

This pack stays candidate-only.
It may candidate later authored tree work, but it is not promoted by presence alone.

## Segment spine

The current machine spine for this route is:

`seg.1.1.1.n`

That spine is the only active machine-facing `segment_id` family for the
bounded route. Human-facing `working_name` values stay descriptive and do not
carry retired route labels.

The paragraph handle still remains visible as `[1]...[12]`, and the source
anchor still remains visible as `source_secondary = 1,1,1,n`.

## Current headers

```text
corpus_map:
corpus_row_id,work_id,part_no,chapter_no,subchapter_no,paragraph_no,source_secondary,sort_key,title_ru,title_en,note

witnesses:
witness_id,language,witness_role,authority_level,author_or_translator,edition_or_source,publication_year,based_on,normalization_note,active

segments:
segment_id,source_secondary,paragraph_anchor,sort_key,witness_scope,line_span,cluster_id,working_name,note

nodes:
node_id,label_ru,node_class,layer,first_segment_id,first_source_secondary,canonical_label,label_en,status,note

event_state_nodes:
es_id,kind,label_ru,anchor_mode,anchor_start_secondary,anchor_end_secondary,anchor_segment_ids,subject_hint,es_class,repeatable,status,note

edges:
edge_id,edge_kind,from_id,predicate_id,to_id,layer,anchor_mode,anchor_start_secondary,anchor_end_secondary,anchor_segment_ids,witness_scope,connectivity_role,confidence,note,status

translation_tensions:
tension_id,normalized_core,anchor_mode,anchor_start_secondary,anchor_end_secondary,anchor_segment_ids,witness_ids,why_load_bearing,decision_status,preferred_handling,note

witness_glosses:
gloss_id,witness_id,segment_id,source_secondary,token_or_phrase,normalized_core,tension_id,gloss_note,status

principles:
principle_id,layer,formula_ru,anchor_mode,anchor_start_secondary,anchor_end_secondary,anchor_segment_ids,status,note
```

## Current dynamic review ledger

`event_state_nodes.csv` is now also an explicit review ledger for the bounded
route.

Its required split is:

- 18 `event` rows with `status = promoted`
- 9 `state` rows with `status = promoted`
- 1 `analogy` row with `status = promoted_to_analogy`

That split lets ToS carry the reviewed dynamic canon in `ToS/canon/` while keeping
the original analogy row visible in `ToS/candidate-intake/` as a cross-family promotion
ledger entry rather than a second canon.

## Current principle review ledger

`principles.csv` is now also an explicit review ledger for the bounded route.

Its required split is:

- 13 `distilled_principle` rows with `status = promoted`
- 1 `authored_commentary` row with `status = promoted_to_synthesis`

That split lets ToS keep the original commentary-like row visible in
`ToS/candidate-intake/` while moving its canonical home to `ToS/canon/synthesis/`.

## Current support review ledger

`nodes.csv` is now also an explicit review ledger for the bounded `n.*` layer.

Its required split is:

- 46 non-literal rows with `status = promoted`
- 2 literal helper rows with `status = deferred_literal`

That split lets ToS carry one bounded `ToS/canon/support/` family for the full
non-literal local `n.*` layer without prematurely opening many semantic
families.

## Current edge review ledger

`edges.csv` is now also an explicit review ledger for the bounded route.

Its required split is:

- 125 rows with `status = promoted`
- 3 rows with `status = deferred_literal`

That split lets ToS carry one route-local canonical relation pack under
`ToS/canon/relations/.../edges.csv` while keeping only literal helper residue
visibly deferred in `ToS/candidate-intake/`.

## Registries

The current tabular graph layer also keeps vocabulary governance surfaces under
`ToS/canon/registries/`:

- `predicates.csv`
- `classes.csv`

These registries are repo-owned governance surfaces.
They are not candidate rows in `ToS/candidate-intake/`.
Their counts should always be regenerated from the current `edges.csv`.

## Validation

Run:

```bash
python scripts/validate_intake_pack.py
python mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py
python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py
```
