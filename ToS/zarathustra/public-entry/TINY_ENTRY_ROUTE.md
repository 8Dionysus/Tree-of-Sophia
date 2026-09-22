# ToS Tiny Entry Route

This document defines the first public tiny-entry seam for Tree of Sophia.

The seam is meant to help humans and smaller models enter a bounded authored route without mistaking orientation for authority.

This tiny-entry seam is the public entrance to the Zarathustra golden growth
kernel. The kernel preserves observation and proposal layers, rejected and
unresolved readings, review rationale, version lineage and the method-transfer
scope defined in `ToS/zarathustra/GOLDEN_GROWTH_KERNEL.md`.

## Current public root

At the current phase, the public `tos-root` is `README.md`.

No separate root file is introduced here.
The root stays human-readable, public, and tree-first.
`ToS/derived-exports/root_entry_map.min.json` is the additive machine-facing companion for that same root.

## Tree-first chain

The current tiny-entry chain is:

`README.md -> node kind -> capsule surface -> authority surface -> one bounded concept hop`

For the first public route, that means:

- `README.md` as the public root
- `source_node` as the node-kind decision
- `ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md` as the capsule surface
- `ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json` as the canonical authored source node
- `ToS/public-compatibility/source_node.example.json` as the current public compatibility authority surface
- `ToS/canon/concept/becoming/node.json` as the canonical authored bounded hop
- `ToS/public-compatibility/concept_node.example.json` as one bounded public compatibility hop
- `ToS/doctrine/KNOWLEDGE_MODEL.md` as the in-repo fallback orientation surface

The entry follows this authored tree path.

## Orientation and authority

Orientation surfaces guide readers to the authored surfaces that own the
material.

- orientation surfaces help a reader or smaller model enter the right authored path quickly
- authority surfaces hold the authored node contract or the closest published source-backed compatibility surface

In this phase:

- `README.md` and this note are orientation surfaces
- `ToS/derived-exports/root_entry_map.min.json` is the compact root-entry capsule for machine-facing entry
- `ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md` is the worked capsule that explains the bounded route
- `ToS/canon/` holds the canonical authored node surfaces
- `ToS/public-compatibility/source_node.example.json` remains the public compatibility authority surface for the first published tiny-entry example

The capsule summarizes the route and returns the reader to the canonical tree
node or source-facing compatibility authority surface.

## First worked route

The first public tiny-entry route is anchored in the Zarathustra prologue path:

- `node_id`: `tos.source.thus-spoke-zarathustra.prologue`
- capsule surface: `ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md`
- canonical source node: `ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json`
- public compatibility authority surface: `ToS/public-compatibility/source_node.example.json`
- canonical bounded hop: `ToS/canon/concept/becoming/node.json`
- bounded hop: `ToS/public-compatibility/concept_node.example.json`
- fallback: `ToS/doctrine/KNOWLEDGE_MODEL.md`

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

`aoa-kag` and the `aoa-sdk` routing control plane now consume this tiny-entry
seam as downstream orientation or derived knowledge input.

That current downstream use stays bounded:

- `aoa-kag` derives a federation-readiness spine entry from the public tiny-entry seam
- the `aoa-sdk` routing control plane hands `tos-root` into the source-owned
  tiny-entry route and the ToS-specific derived `kag_view`, while preserving
  `aoa-routing` only as the stable compatibility namespace

ToS retains authored node law and its public root; these consumers preserve
the source-return route.

## Source-first re-entry

If a downstream consumer loses ToS boundary and needs to restore the current bounded route from `tos-root`, the source-first re-entry should stay:

`README.md -> ToS/public-compatibility/tos_tiny_entry_route.example.json -> ToS/public-compatibility/source_node.example.json`

`CHARTER.md` remains the root authority note for ToS posture, but the worked route should re-enter through the source-owned tiny-entry example before any derived `kag_view` or adjunct.
The `aoa-sdk` routing control plane may restore this re-entry hop as bounded
navigation under the stable `aoa-routing` compatibility namespace. Re-entry
passes through Tree-of-Sophia's authored authority before downstream derived
surfaces.

## Entry contract

A tiny-entry route guides the reader through a declared scope inside Tree of Sophia.

- Keep the authored source as the authority for its material.
- Let the capsule summarize and link to the owning node.
- Declare the permitted hops.
- Return downstream readers to ToS sources.
- State the coverage of each worked route; wider corpus entry grows through additional reviewed routes.

## Current public type

The authored type for this seam is `tos_tiny_entry_route`.

Its current public example is
[ToS/public-compatibility/tos_tiny_entry_route.example.json](../../public-compatibility/tos_tiny_entry_route.example.json).

The additive machine-facing root capsule is `ToS/derived-exports/root_entry_map.min.json`.

See
[ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md](../prologue-1/TRILINGUAL_ENTRY.md),
`python scripts/build_root_entry_map.py --check`,
`python scripts/validate_root_entry_map.py`,
`python scripts/validate_tiny_entry_route.py`,
[the selected KAG export validation route](../../../kag/VALIDATION.md), and
[mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md](../../../mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md)
for the current validator and manual-review route for this bounded seam.
