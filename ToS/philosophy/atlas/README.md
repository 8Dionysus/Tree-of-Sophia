# Philosophy Atlas

`atlas/` is the prepared navigation body for the whole ToS philosophy tree.

It holds the master-table row spine and the index of prepared A-series dossier
documents. It is the project-prepared atlas that tells the philosophy tree
what must grow.

Historical source witnesses, editions, translations, and branch-level source
anchors still belong inside later branch work. Authored relation packs still
belong in canon after branch work gives them form.

## Shape

```text
atlas/
  master-tables/
    table-i/
      rows.jsonl
      table.manifest.json
    table-ii/
      rows.jsonl
      table.manifest.json
    table-iii/
      rows.jsonl
      table.manifest.json
  dossiers/
    index.jsonl
    graph-shape-summary.json
```

`master-tables/` holds all row entries from the three prepared ToS master
tables: `48 + 58 + 84 = 190` rows.

`dossiers/` indexes the prepared A-series Deep Research documents already
present on this machine. It records document shape, node-table counts,
relation-table counts, and vocabulary pressure without expanding those rows
into canon objects.

## Growth Route

```text
master-table row
  -> philosophy atlas
    -> era / region / tradition branch
      -> local graph workbench
        -> authored canon relation pack
          -> derived graph/export
```

Future patches to the master tables should update the matching row file and
table manifest directly, with the meaning of the patch reviewed in the owning
philosophy branch.
