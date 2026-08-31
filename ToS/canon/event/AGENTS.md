# AGENTS.md

This file applies to canonical authored event nodes under `ToS/canon/event/`.

## What lives here

`ToS/canon/event/` is the home of canonical authored event nodes.

It holds:

- route-local event surfaces that make bounded movement legible
- review-gated dynamic nodes promoted from candidate intake
- one `node.json` per stabilized event surface

## Editing posture

Treat these files as canonical authored tree law; raw candidate tables route to
`ToS/candidate-intake/`.
Keep each `node.json` bounded to one event surface.
Keep the source route explicit in `source_anchor`.
Keep event nodes source-linked, route-local, and dynamic; distilled claims
route to principle nodes after review.
Keep `ToS/public-compatibility/event_node.example.json` aligned with the worked canonical
mirror as a compatibility surface.

## Validation

Select the `canon_contracts` route from [`ToS/VALIDATION.md`](../../VALIDATION.md)
after the authored node class and affected companion are known. If a public
mirror or derived export is touched, select the corresponding `public_entry` or
`generated_parity` route; their owner docs retain procedure.
