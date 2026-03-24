# 2026-03-23 Tree-First Tiny-Entry Review

## What changed

- added `docs/TINY_ENTRY_ROUTE.md` as the first public doctrine note for the tree-first tiny-entry seam
- added `schemas/tos-tiny-entry-route.schema.json` and `examples/tos_tiny_entry_route.example.json`
- reframed `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md` as the first worked capsule for the tiny-entry seam
- updated `README.md`, `ROADMAP.md`, and `docs/KNOWLEDGE_MODEL.md` so the tiny-entry route is visible in the public entry path
- updated `docs/REVIEW_CHECKLIST.md` with tiny-entry-specific manual review items

## Most at-risk checklist items

- `README.md` staying the current public `tos-root` rather than being displaced by a derived entry surface
- capsule and authority collapsing into one interchangeable surface
- fallback drifting out of ToS into `aoa-kag` or `aoa-routing`
- the route widening beyond one real authored node plus one bounded hop
- the tiny-entry seam being mistaken for a graph-first or KAG-first entry contract

## Review result

- `yes` the route remains tree-first and starts from `README.md` as the current public `tos-root`
- `yes` `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md` now acts as a worked capsule without replacing `examples/source_node.example.json` as the authority surface
- `yes` the fallback stays inside ToS through `docs/KNOWLEDGE_MODEL.md`
- `yes` no field in the new public example points to `aoa-kag` or `aoa-routing`
- `yes` the route stays bounded to one real authored source node plus one bounded concept hop
- `yes` no separate `context_node` example was introduced in this wave

## What remains deferred

- separate `context_node` tiny-entry expansion
- any public validator script for the tiny-entry route
- downstream `aoa-kag` or `aoa-routing` sync work that consumes the new ToS seam at the time of this review
- any broader graph-first or multi-hop entry contract

## Later update

- 2026-03-24: the downstream `aoa-kag` and `aoa-routing` syncs for this public seam have since landed; see `docs/reviews/2026-03-24-tiny-entry-downstream-consumption-review.md`

## Boundary note

The tiny-entry seam is an orientation aid inside Tree of Sophia.

It does not replace ToS-authored authority.
It does not delegate authority to downstream repositories.
It does not turn one bounded Zarathustra route into a general solution for wider corpus entry.
