# ToS Source Home

`ToS/` is the home organ for Tree of Sophia.

It gathers the old ToS-owned root layers into one tree-shaped topology so the
repository no longer has a separate root-level `tree/` competing with the ToS
home.

## Map

| Path | Use For | Stronger Than |
| --- | --- | --- |
| `doctrine/` | knowledge law, node contracts, route posture, source-first review law | review notes and generated readers |
| `source-witnesses/` | primary source-facing witness material | intake, canon summaries, exports |
| `philosophy/` | growing domain tree of philosophy: trunk, eras, regions, traditions, works, figures, concepts, transmissions, local graph workbenches | candidate intake and generated readers |
| `candidate-intake/` | provisional extraction and promotion ledgers | generated exports |
| `canon/` | canonical authored nodes, relations, and registries | public mirrors and derived exports |
| `public-compatibility/` | public-safe examples, mirrors, tiny-entry compatibility | derived exports |
| `derived-exports/` | generated downstream-facing read models | nothing source-owned |
| `contracts/` | schemas and public structural contracts | generated payloads |
| `review-ledger/` | dated review and inspection notes | generated readers only |

## First Contour

This pass is a structural migration, not final ontology.
The first contour makes the source-home symmetry real while preserving the
current bounded Zarathustra route:

- current public root: `README.md`
- current tiny-entry surface: `ToS/public-compatibility/tos_tiny_entry_route.example.json`
- current capsule: `ToS/doctrine/ZARATHUSTRA_TRILINGUAL_ENTRY.md`
- current source canon: `ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json`
- current bounded export: `ToS/derived-exports/kag_export.min.json`

The next contour grows `ToS/philosophy/` as the domain-shaped philosophy
branch. It is the authored philosophical body whose local branches may later
prepare nodes and relation packs for review and promotion into `ToS/canon/`.

## Operating Card

| Field | Route |
| --- | --- |
| role | source-home entrypoint for ToS-authored philosophical work |
| input | source witnesses, domain branches, candidate extraction, canon objects, public examples, generated exports, contracts, doctrine, and review evidence |
| output | branch-shaped ToS surface with a visible owner, source posture, and validation route |
| owner | `ToS/AGENTS.md` and `ToS/source_home.manifest.json` |
| next route | witness -> philosophy or candidate intake -> canon -> public compatibility -> derived export |
| tools | nearest nested `AGENTS.md`, source-home manifest, branch manifest, schema, generator, or validator |
| check | `python scripts/validate_tos_source_home.py` plus the owning branch validator |

## Boundary Routes

- ToS-owned active material routes into `ToS/`, not back into root-level active
  homes.
- Source-facing evidence routes to `source-witnesses/`; domain growth routes to
  `philosophy/`; provisional extraction routes to `candidate-intake/`; authored
  nodes and relation packs route to `canon/`.
- Public examples route to `public-compatibility/`; generated read models route
  to `derived-exports/`; schemas route to `contracts/`.
- Durable rationale routes to `docs/decisions/`; current ToS doctrine stays in
  `ToS/doctrine/`.
- AoA federation, runtime, SDK, memory, eval, and KAG-substrate authority stay
  with their owning repositories or layers.
