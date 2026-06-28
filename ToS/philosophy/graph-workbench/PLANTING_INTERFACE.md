# Planting Interface

This interface names how prepared philosophy material becomes graph-workbench
matter before canon promotion.

It belongs to `Tree-of-Sophia` because planting changes ToS-owned meaning
surfaces: atlas rows, branch homes, proposed nodes, proposed relations, source
anchor backlogs, term and transmission indexes, branch fragments, review
packets, and generated derived exports.

`abyss-stack` reads the resulting derived projection for UI, Neo4j, MCP, cache,
and launch ergonomics. It does not choose node kinds, predicates, canon status,
epochs, source authority, or promotion results.

## Source Spine

Prepared planting starts from source-owned atlas material:

| Surface | Role |
| --- | --- |
| `ToS/philosophy/atlas/master-tables/*/rows.jsonl` | master row spine for planned philosophy growth |
| `ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json` | dossier-to-branch route map |
| operator-local prepared DOCX corpus | temporary extraction input for supported planting scripts |
| `ToS/philosophy/atlas/dossiers/index.jsonl` | dossier identity and graph pressure index |
| `ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl` | future real witness, edition, corpus, and risk-control anchors |
| `ToS/philosophy/atlas/dossiers/term-index.jsonl` | prepared term rows |
| `ToS/philosophy/atlas/dossiers/transmission-backlog.jsonl` | incoming and outgoing transmission rows |

Prepared research files are extraction input. Historical authority still routes
to witnesses, editions, translations, corpora, branch review, and canon
surfaces.

## Planting Packet

A plantable packet carries these records as one route:

| Record | Required Meaning |
| --- | --- |
| table row | `row_id`, table id, normalized route fields, dossier availability |
| dossier route | `dossier_id`, `branch_path`, `branch_role` |
| dossier index row | title, source document, master table, branch path, node/relation/source/term/transmission counts |
| proposed node row | `candidate_id`, node kind, label, period, priority, branch path, `canon_status: pre-canon`, source row/table indexes, source ref |
| proposed relation row | `candidate_id`, relation kind/label, source and target endpoint labels, resolved candidate ids when available, confidence, endpoint resolution, source ref |
| source anchor row | witness, corpus, edition, access, reliability, limitation, or source need |
| term row | term, language, transliteration, meaning, ToS role |
| transmission row | direction, transmitted matter, channel, confidence, next check |
| branch fragment | branch path, dossier id, local counts, source anchor count |
| promotion ledger entry | planted counts and next promotion route |

The packet is complete enough for graph review when every proposed node and
relation can point back to a ToS source ref and every unresolved endpoint is
visible as unresolved, not silently upgraded.

## Growth Route

```text
master table row
  -> prepared dossier route
    -> branch home
      -> source anchor backlog
      -> proposed nodes
      -> proposed relations
      -> branch fragment
        -> review packet
          -> derived graph projection
            -> abyss-stack read-only projection
              -> relation-weaving review
                -> canon promotion
```

Current supported entrypoint:

```bash
python scripts/plant_prepared_dossiers.py --readiness
python scripts/plant_prepared_dossiers.py --table table-i --plant
python scripts/build_philosophy_atlas_projection.py
python scripts/build_philosophy_graph_views.py
python scripts/build_philosophy_graph_projection.py
python scripts/build_philosophy_post_planting_audit.py
```

Future Table II and Table III planting should extend this route by adding their
source-owned dossier route maps and packet extraction support, then publishing
the same graph-workbench surfaces.

## Review Handoff

The first human review pass reads:

| Review Need | Surface |
| --- | --- |
| branch placement | `ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json` |
| graph row pressure | `ToS/philosophy/graph-workbench/proposed-nodes/` and `proposed-relations/` |
| source pressure | `ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl` |
| transmission pressure | `ToS/philosophy/atlas/dossiers/transmission-backlog.jsonl` |
| view switching | `ToS/philosophy/graph-workbench/views/view-contracts.json` |
| cluster reading | `ToS/philosophy/graph-workbench/clusters/cluster-contracts.json` |
| review packet | `ToS/philosophy/graph-workbench/review-packets/` |
| runtime projection | `ToS/derived-exports/philosophy_graph_projection.min.json` |

Runtime review through `tos-up` is a lens over these surfaces. Any correction
returns to the ToS source spine first, then derived exports are rebuilt.
