# Graph Views

`views/` holds graph-view route cards and the source-owned switching contract.

Each view names a future visualization lens. It does not hold graph data, and
it does not replace source witnesses, branch review, proposed nodes, proposed
relations, canon relation packs, or derived exports.

`view-contracts.json` maps those route cards to graph layers, current atlas
projection filters, and future branch filters. It is source-owned input for the
generated `ToS/derived-exports/philosophy_graph_views.min.json` catalog.

## Views

| View | Lens |
| --- | --- |
| `chronology.graph.md` | formation, attestation, fixation, canonization, preservation, reception |
| `transmission.graph.md` | translation, copying, commentary, school, archive, and reception corridors |
| `source-evidence.graph.md` | source witnesses, evidence status, disputes, and preservation posture |
| `concept-lineage.graph.md` | concepts, semantic fields, problems, contrasts, and analogies |
| `institution-media.graph.md` | schools, temples, courts, libraries, universities, presses, journals, archives, and media |
| `script-decipherment.graph.md` | scripts, artifacts, partial readings, decipherment routes, and uncertainty |
| `imperial-multilingualism.graph.md` | parallel versions, administrative languages, translation regimes, and empire-scale writing |
| `ritual-law.graph.md` | treaty, oath, law, decree, ritual, legitimacy, and state action |
| `epigraphic-network.graph.md` | cities, inscriptions, carriers, alphabetic handoffs, and distributed public writing |
| `lost-corpus.graph.md` | lost or fragmentary corpora attested through indirect witnesses and mediation |
| `canon-promotion.graph.md` | candidate objects, reviewed relation packs, canon, and derived exports |
