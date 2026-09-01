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
| `ToS/research-packets/deep-research/philosophy/dossiers/table-i-docx-intake.manifest.json` | tracked fixity and capture-posture record for the untracked DOCX bytes |
| `ToS/research-packets/deep-research/philosophy/dossiers/table-i-docx-extraction-coverage.json` | explicit extracted/metadata/deferred row accounting and prose-only diagnostics |
| `ToS/research-packets/deep-research/philosophy/dossiers/table-ii-docx-intake.manifest.json` | tracked Table II fixity plus admitted/quarantined artifact posture |
| `ToS/research-packets/deep-research/philosophy/dossiers/table-ii-docx-extraction-coverage.json` | Table II structured/deferred/quarantined row accounting |
| `ToS/research-packets/deep-research/philosophy/dossiers/table-iii-docx-intake.manifest.json` | tracked fixity and admission posture for all 84 Table III artifacts |
| `ToS/research-packets/deep-research/philosophy/dossiers/table-iii-docx-extraction-coverage.json` | Table III structured/deferred row accounting with prose-only and missing-risk-table diagnostics kept explicit |
| `ToS/philosophy/atlas/dossiers/index.jsonl` | dossier identity and graph pressure index |
| `ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl` | future real witness, edition, corpus, and risk-control anchors |
| `ToS/philosophy/atlas/dossiers/term-index.jsonl` | prepared term rows |
| `ToS/philosophy/atlas/dossiers/transmission-backlog.jsonl` | incoming and outgoing transmission rows |

Prepared research files are extraction input. Historical authority still routes
to witnesses, editions, translations, corpora, branch review, and canon
surfaces.

Planting is bounded, not a full dossier transfer. Context rows that lack an
owned structured destination remain counted as deferred, and prose is not
silently converted into structured risk claims.

## Planting Packet

A plantable packet carries these records as one route:

| Record | Required Meaning |
| --- | --- |
| table row | `row_id`, table id, normalized route fields, dossier availability |
| dossier route | `dossier_id`, `branch_path`, `branch_role` |
| dossier index row | title, source document, master table, branch path, node/relation/source/term/transmission counts |
| proposed node row | `candidate_id`, node kind, label, period, priority, branch path, `canon_status: pre-canon`, source row/table indexes, source ref |
| proposed relation row | `candidate_id`, relation kind/label, source and target endpoint labels, resolved candidate ids when available, confidence, endpoint resolution, source ref |
| reviewed endpoint alias | exact origin dossier, endpoint role, endpoint label, admitted target dossier, target candidate id and label, pre-canon projection review status, explicit claim limit |
| source anchor row | witness, corpus, edition, access, reliability, limitation, or source need |
| term row | term, language, transliteration, meaning, ToS role |
| transmission row | direction, transmitted matter, channel, confidence, next check |
| text-bearing language packet | original title posture, language, script, transliteration, Russian label, English label, witness posture, translation/version relation pressure |
| branch fragment | branch path, dossier id, local counts, source anchor count |
| promotion ledger entry | planted counts and next promotion route |

The packet is complete enough for graph review when every proposed node and
relation can point back to a ToS source ref and every unresolved endpoint is
visible as unresolved, not silently upgraded.

Projection reuses tracked identity before creating a placeholder. An exact
admitted dossier id resolves to its `atlas-dossier:*` node with
`projection_endpoint_resolution: exact_admitted_dossier`; a leading known
master-row id whose dossier is quarantined or not supplied resolves to the
existing `atlas-row:*` node; only an otherwise unresolved label becomes an
origin-scoped `candidate-endpoint:*`. Reviewed aliases remain the explicit
route to a candidate inside another admitted dossier: a qualified label is
still bound to its exact origin and endpoint role, while an unqualified label
must use that origin-role key and is never resolved by a global label match.

Text-bearing packets are governed by
`ToS/philosophy/atlas/multilingual/text-bearing-nodes.contract.json`. They are
used for works, corpora, inscriptions, source witnesses, translations, versions,
and commentaries. The original slot preserves only attested, traditional,
normalized, or explicitly reconstructed source-language form; Russian and
English slots carry review/runtime labels with their own status.

## Growth Route

```text
master table row
  -> prepared dossier route
    -> branch home
      -> source anchor backlog
      -> text-bearing language packet
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

`scripts/plant_prepared_dossiers.py` owns readiness and planting orchestration.
Readiness may be limited with `--table`; planting is explicitly aggregate-only
and runs as `python scripts/plant_prepared_dossiers.py --plant` because the
atlas indexes, graph workbench, language packets, and branch manifests combine
all supported packages. The compatibility implementation entrypoint invokes
the same aggregate gate and cannot bypass it. Readiness requires an exact
unique master-row spine whose `row_id`, `table_id`, and normalized row identity
agree, an exact unique filename inventory, and a read-only parse of every
supplied DOCX through the same content and identity checks used by planting.
It also checks a Table I title id when row metadata is absent and reads package
provenance metadata, including optional custom-property XML, before any write.
Missing, duplicate, unexpected, corrupt, or identity-drifted inputs fail
closed before planting changes a companion.
The projection, corpus-index, and post-planting builders own their generated
outputs; `docs/validation/validation_lanes.json` owns checked verification
order. Use `scripts/AGENTS.md` for the operator route.

The complete Table I, Table II, and Table III packages use this route now.
Table III admits all 84 master-aligned artifacts; `T3-57` produces no
structured semantic candidates because its supplied artifact is prose-only
and explicitly insufficient, while `T3-76` remains an undeciphered-script
frontier rather than an era or deciphered tradition claim. T2-56 is routed
through an information-system frontier so khipu evidence is not projected as
a readable philosophical corpus.

## Review Handoff

The first human review pass reads:

| Review Need | Surface |
| --- | --- |
| branch placement | `ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json` |
| graph row pressure | `ToS/philosophy/graph-workbench/proposed-nodes/` and `proposed-relations/` |
| reviewed cross-dossier endpoint routing | `ToS/philosophy/graph-workbench/proposed-relations/reviewed-endpoint-aliases.json` |
| source pressure | `ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl` |
| transmission pressure | `ToS/philosophy/atlas/dossiers/transmission-backlog.jsonl` |
| view switching | `ToS/philosophy/graph-workbench/views/view-contracts.json` |
| cluster reading | `ToS/philosophy/graph-workbench/clusters/cluster-contracts.json` |
| review packet | `ToS/philosophy/graph-workbench/review-packets/` |
| runtime projection | `ToS/derived-exports/philosophy_graph_projection.min.json` |

Runtime review through `tos-up` is a lens over these surfaces. Any correction
returns to the ToS source spine first, then derived exports are rebuilt.
