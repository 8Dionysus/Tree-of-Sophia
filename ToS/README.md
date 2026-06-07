# ToS Source Home

`ToS/` is the home organ for Tree of Sophia.

It gathers the old ToS-owned root layers into one tree-shaped topology so the
repository no longer has a separate root-level `tree/` competing with the ToS
home.

## Map

| Path | Use For | Stronger Than |
| --- | --- | --- |
| `doctrine/` | knowledge law, node contracts, templates, interpretation ladder, and authored route doctrine | review notes and generated readers |
| `source-witnesses/` | primary source-facing witness material | intake, canon summaries, exports |
| `zarathustra/` | golden route capsule and public-entry orientation for the bounded Zarathustra prologue path | generic orientation and generated readers |
| `research-packets/` | non-authoritative research scaffolds and capture metadata | philosophy branch review |
| `philosophy/` | growing domain tree of philosophy: trunk, eras, regions, traditions, works, figures, concepts, transmissions, local graph workbenches | candidate intake and generated readers |
| `candidate-intake/` | provisional extraction and promotion ledgers | generated exports |
| `canon/` | canonical authored nodes, relations, and registries | public mirrors and derived exports |
| `public-compatibility/` | ToS-owned public-safe examples, mirrors, and tiny-entry compatibility | derived exports |
| `derived-exports/` | ToS-owned generated downstream-facing read models | nothing source-owned |
| `contracts/` | ToS-owned schemas and public structural contracts | generated payloads |
| `review-ledger/` | dated review and inspection notes | generated readers only |

## First Contour

This pass is a structural migration, not final ontology.
The first contour makes the source-home symmetry real while preserving the
current bounded Zarathustra route:

- current public root: `README.md`
- current tiny-entry surface: `ToS/public-compatibility/tos_tiny_entry_route.example.json`
- current capsule: `ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md`
- current source canon: `ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json`
- current bounded export: `ToS/derived-exports/kag_export.min.json`

The next contour grows `ToS/philosophy/` as the domain-shaped philosophy
branch. It is the authored philosophical body whose local branches may later
prepare nodes and relation packs for review and promotion into `ToS/canon/`.

## Operating Card

| Field | Route |
| --- | --- |
| role | source-home entrypoint for ToS-authored philosophical work |
| input | source witnesses, Zarathustra route surfaces, research packets, domain branches, candidate extraction, canon objects, ToS-owned public examples, generated exports, contracts, doctrine, and review evidence |
| output | branch-shaped ToS surface with a visible owner, source posture, and validation route |
| owner | `ToS/AGENTS.md` and `ToS/source_home.manifest.json` |
| next route | witness or research packet -> zarathustra, philosophy, or candidate intake -> canon -> public compatibility -> derived export |
| tools | nearest nested `AGENTS.md`, source-home manifest, branch manifest, schema, generator, or validator |
| check | `python scripts/validate_tos_source_home.py` plus the owning branch validator |

## Boundary Routes

- ToS-owned active material routes into `ToS/`, not back into root-level active
  homes.
- Source-facing evidence routes to `source-witnesses/`; bounded Zarathustra
  route orientation routes to `zarathustra/`; non-authoritative research
  scaffolds route to `research-packets/`; domain growth routes to
  `philosophy/`; provisional extraction routes to `candidate-intake/`;
  authored nodes and relation packs route to `canon/`.
- Public examples route to `public-compatibility/`; generated read models route
  to `derived-exports/`; schemas route to `contracts/`.
- Current ToS knowledge law stays in `ToS/doctrine/`; operational process
  documents stay in `mechanics/`.
- AoA federation, runtime, SDK, memory, eval, and KAG-substrate authority stay
  with their owning repositories or layers.
