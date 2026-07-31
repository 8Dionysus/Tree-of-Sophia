# AGENTS.md

This file applies to derived export artifacts under `ToS/derived-exports/`.

## Role

`ToS/derived-exports/` publishes generated downstream-facing read models for
Tree of Sophia. These files are consumer companions that return to authored
ToS authority.

## Operating Card

| Field | Route |
| --- | --- |
| role | generated downstream-facing read model surface |
| input | owned canon, compatibility examples, contracts, and generator logic |
| output | generated export payloads and compact read models |
| owner | `ToS/derived-exports/AGENTS.md` for route law; generator scripts for payload construction |
| next route | source-owned input or generator -> regenerate -> validate export |
| tools | KAG export generator/validator, corpus-index builder/validator, lexical-index builder/validator, philosophy-atlas projection builder/validator, philosophy graph-view catalog builder/validator, philosophy graph-projection builder/validator, source-witness bibliographic graph builder/validator, source validators |
| check | generated parity and export validation |

## Boundary Routes

- Change source-owned inputs or generator logic before changing exports.
- Keep generated fields pointing back to the source that gives them meaning.
- Keep runtime graph, UI, MCP, Neo4j, and service behavior in `abyss-stack`.
- Keep KAG, routing, and control-plane authority with owning downstream
  repositories.
- Let the corpus index cover `ToS/` as an index that returns to authored
  meaning.
- Let a tracked lexical projection expose only the source-withholding fields
  authorized by its source-owned plan; dictionary-recoverable form hashes are
  not a publication or confidentiality clearance.
- Let the philosophy atlas projection expose atlas rows, prepared dossier
  pressure, and graph-view routes as a derived read model for review.
- Let the philosophy graph-view catalog expose source-owned lens filters for
  downstream switching while keeping runtime behavior in `abyss-stack`.
- Let the philosophy graph projection materialize those atlas and view surfaces
  into source-ref-preserving node, edge, and view packets for downstream access.
- Let `graph/source-witness-bibliographic-claims.min.json` read the generated
  public-safe source-witness catalog without merging it into the atlas graph.
  Keep the claim as the relation center, retain literal objects as literals,
  and require every edge to return to claim, evidence, maker, provenance, and
  review posture.

## Validation

Use the relevant builder `--check` and validator for the export touched:
KAG export, root-entry map, corpus index, lexical index, philosophy atlas
projection, philosophy graph-view catalog, philosophy graph projection, public
mirror sync, source-witness bibliographic graph, or tiny-entry route.
For release-facing generated readmodels, also run the OS Abyss artifact bundle
validator under `mechanics/release-support/parts/artifact-bundles/scripts/`.
