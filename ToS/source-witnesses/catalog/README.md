# Source-Witness Catalog

This directory is the tracked, generated navigation index over authoritative
object and bibliographic claim records in the speaking `agents/`, `works/`,
and `collections/` trees.

| File | Record class |
| --- | --- |
| `agents.jsonl` | people and organizations |
| `works.jsonl` | intellectual works |
| `expressions.jsonl` | language/textual responsibility states |
| `editions.jsonl` | published/edited manifestations |
| `collections.jsonl` | aggregate publications |
| `items.jsonl` | acquired physical/digital copies or containers |
| `claims.jsonl` | source-returnable membership, responsibility, publication, and Work/Expression/Edition/Item topology claims |
| `catalog.manifest.json` | counts, paths, digest, and generation boundary |

Object records own identity; source claim packets own bibliographic
assertions. These JSONL files are rebuildable indexes. Object entries include
their source record path and canonical digest. Claim entries additionally
retain the exact source JSONL line, canonical claim digest, subject, predicate,
object, evidence, maker, provenance event, and review posture. The projection
does not accept or promote any claim. Only claims already marked `public` or
`public_metadata_only` may enter this tracked projection; local, restricted,
or permission-pending material requires a separately reviewed public-safe
derivative rather than silent copying.

The current topology contribution is 55 separately addressable packets: 20
`has_expression`, 20 `embodied_by`, and 15 `exemplified_by`. Their presence in
this generated catalog proves exact projection only. It does not make the
declared identity ladder true, accept any text, or turn embodiment into
textual equivalence.

The current bounded projection contains 65 object records and 92 claim records
(157 entries total). The responsibility slice contains seven Work authorship,
seven Expression translation, and three Edition role claims; all remain
source-returnable, model-made, public-metadata-only, and unreviewed.

Regeneration, parity checks, and source-foundation validation route through
`scripts/AGENTS.md`, which owns the catalog builder and evidence-spine
validator. A green result proves exact projection and source return, not the
truth of metadata or relations, rights, OCR, translation, review, or
semantics.
