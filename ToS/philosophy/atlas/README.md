# Philosophy Atlas

`atlas/` is the prepared navigation body for the whole ToS philosophy tree.

It holds the master-table row spine, prepared A-series dossier index, and aggregate pressure maps that tell the philosophy tree what must grow next.

## Shape

```text
atlas/
  master-tables/
    table-i/
    table-ii/
    table-iii/
  dossiers/
    index.jsonl
    graph-shape-summary.json
    source-anchor-backlog.jsonl
    term-index.jsonl
    transmission-backlog.jsonl
  multilingual/
    content-labels.json
    language-registry.json
    text-bearing-nodes.contract.json
```

The atlas is prepared navigation and growth pressure. Branch bodies live in `ToS/philosophy/eras/...`; pre-canon graph material lives in `ToS/philosophy/graph-workbench/`; authored canon relation packs live in the canon route.

`multilingual/` is a source-owned display companion for the atlas and generated graph projections. It preserves the route rule that planted works must carry their attested original form when available, plus Russian and English display labels for review and downstream visualization.

For works, corpora, inscriptions, source witnesses, translations, versions, and commentaries, `multilingual/text-bearing-nodes.contract.json` is the planting contract. It keeps original-language title posture, transliteration, Russian review text, English review/runtime text, witness posture, and graph relation pressure in one source-owned packet shape.
