# ToS Tiny Entry Route

This document defines the first public tiny-entry seam for Tree of Sophia.

The seam is meant to help humans and smaller models enter a bounded authored route without mistaking orientation for authority.

## Current public root

At the current wave, the public `tos-root` is `README.md`.

No separate root file is introduced here.
The root stays human-readable, public, and tree-first.
`generated/root_entry_map.min.json` is the additive machine-facing companion for that same root.

## Tree-first chain

The current tiny-entry chain is:

`README.md -> node kind -> capsule surface -> authority surface -> one bounded concept hop`

For the first public route, that means:

- `README.md` as the public root
- `source_node` as the node-kind decision
- `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md` as the capsule surface
- `tree/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json` as the canonical authored source node
- `examples/source_node.example.json` as the current public compatibility authority surface
- `tree/concept/becoming/node.json` as the canonical authored bounded hop
- `examples/concept_node.example.json` as one bounded public compatibility hop
- `docs/KNOWLEDGE_MODEL.md` as the in-repo fallback orientation surface

This is a tree-first route, not a graph-first entry contract.

## Orientation and authority

ToS needs both orientation surfaces and authority surfaces, but they should not collapse into one layer.

- orientation surfaces help a reader or smaller model enter the right authored path quickly
- authority surfaces hold the authored node contract or the closest published source-backed compatibility surface

In this wave:

- `README.md` and this note are orientation surfaces
- `generated/root_entry_map.min.json` is the compact root-entry capsule for machine-facing entry
- `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md` is the worked capsule that explains the bounded route
- `tree/` holds the canonical authored node surfaces
- `examples/source_node.example.json` remains the public compatibility authority surface for the first published tiny-entry example

The capsule is allowed to summarize the route.
It is not allowed to replace the canonical tree node or the source-facing compatibility authority surface.

## First worked route

The first public tiny-entry route is anchored in the Zarathustra prologue path:

- `node_id`: `tos.source.thus-spoke-zarathustra.prologue`
- capsule surface: `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md`
- canonical source node: `tree/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json`
- public compatibility authority surface: `examples/source_node.example.json`
- canonical bounded hop: `tree/concept/becoming/node.json`
- bounded hop: `examples/concept_node.example.json`
- fallback: `docs/KNOWLEDGE_MODEL.md`

This first route stays deliberately narrow:

- one real authored source node
- one worked capsule
- one bounded concept hop
- no separate `context_node` expansion yet

## Hop field posture

The current public hop field for this route type is `bounded_hop`.

The older `lineage_or_context_hop` label may remain as a legacy compatibility alias during transition where a downstream consumer still expects it.

The public example should treat `bounded_hop` as primary.
If both fields are present during transition, they should point to the same in-repo surface.

## Downstream boundary

`aoa-kag` and `aoa-routing` now consume this tiny-entry seam as downstream orientation or derived knowledge input.

That current downstream use stays bounded:

- `aoa-kag` derives a federation-readiness spine entry from the public tiny-entry seam
- `aoa-routing` hands `tos-root` into the source-owned tiny-entry route and the ToS-specific derived `kag_view`

They do not become ToS authority surfaces.
They do not replace authored node law.
They do not become the public root of this route.

## Source-first re-entry

If a downstream consumer loses ToS boundary and needs to restore the current bounded route from `tos-root`, the source-first re-entry should stay:

`README.md -> examples/tos_tiny_entry_route.example.json -> examples/source_node.example.json`

`CHARTER.md` remains the root authority note for ToS posture, but the worked route should re-enter through the source-owned tiny-entry example before any derived `kag_view` or adjunct.
`aoa-routing` may restore this re-entry hop as bounded navigation, but it must not replace Tree-of-Sophia authority or jump directly to downstream derived surfaces.

## Anti-collapse rule

A tiny-entry route is an orientation aid inside Tree of Sophia.

It must never:

- replace ToS-authored source authority
- flatten capsule and authority into one interchangeable surface
- turn a bounded route into an unbounded graph walk
- cite downstream repositories as ToS authority
- pretend that one worked route already solves wider corpus entry

## Current public type

The authored type for this seam is `tos_tiny_entry_route`.

Its current public example is [examples/tos_tiny_entry_route.example.json](../examples/tos_tiny_entry_route.example.json).

The additive machine-facing root capsule is `generated/root_entry_map.min.json`.

See [docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md](ZARATHUSTRA_TRILINGUAL_ENTRY.md), `python scripts/build_root_entry_map.py --check`, `python scripts/validate_root_entry_map.py`, `python scripts/validate_tiny_entry_route.py`, `python scripts/validate_kag_export.py`, and [docs/REVIEW_CHECKLIST.md](REVIEW_CHECKLIST.md) for the current validator and manual-review route for this bounded seam.
