# Example

## Scenario

A workflow catalog builder mixes source-owned bundle classification rules with
installed export path formatting, markdown report rendering, runtime router
hint assembly, and local filesystem discovery. Reviews keep getting stuck
because contributors cannot tell whether a change alters reusable catalog
semantics or only changes generated delivery surfaces.

## Why this skill fits

- the core problem is unclear responsibility between reusable catalog logic and export/report/runtime glue
- the surface mixes stable source-to-catalog rules with projection, rendering, and local delivery detail
- future changes will stay safer if the reusable center is separated before more generated surfaces depend on it

## Expected inputs

- the target builder or slice with mixed responsibilities
- the current classification, source-ref, and source-to-catalog rules that appear stable
- the surrounding export, rendering, runtime, path, and environment-specific code
- the pressure points that make reviews or tests muddy today

## Expected outputs

- a clearer statement of what belongs in the reusable catalog core
- notes on what should remain generated/export, report, runtime, or local-path glue
- a bounded refactor proposal or small implementation move
- a source-owner stop-line so generated surfaces do not become authored bundle truth
- a short verification summary about improved clarity

## Boundary notes

- this example is about deciding what belongs in the reusable center versus generated or runtime-facing edge surfaces
- it is not yet about introducing a port around a concrete dependency once the boundary is already clear
- it is not a broad catalog redesign or a promotion of generated output into source authority

## Verification notes

- verify that stable catalog rules are separated conceptually from export formatting and report rendering
- verify that generated, router, and local path details remain outside the reusable center
- verify that authored bundle meaning stays stronger than the generated consumer surfaces
- verify that the change improves review clarity without expanding into a broad rewrite
