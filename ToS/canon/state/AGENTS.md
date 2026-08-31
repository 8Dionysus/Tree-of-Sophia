# AGENTS.md

This file applies to canonical authored state nodes under `ToS/canon/state/`.

## What lives here

`ToS/canon/state/` is the home of canonical authored state nodes.

It holds:

- route-local state surfaces that make bounded conditions legible
- review-gated dynamic nodes promoted from candidate intake
- one `node.json` per stabilized state surface

## Editing posture

Treat these files as canonical authored tree law; raw candidate tables route to
`ToS/candidate-intake/`.
Keep each `node.json` bounded to one state surface.
Keep the source route explicit in `source_anchor`.
Keep state nodes source-linked, route-local, and condition-bearing; commentary
doctrine routes to `ToS/doctrine/`.
Keep `ToS/public-compatibility/state_node.example.json` aligned with the worked canonical
mirror as a compatibility surface.

## Validation

Select the `canon_contracts` route from [`ToS/VALIDATION.md`](../../VALIDATION.md)
after the authored node class and affected companion are known. If a public
mirror or derived export is touched, select the corresponding `public_entry` or
`generated_parity` route; their owner docs retain procedure.
