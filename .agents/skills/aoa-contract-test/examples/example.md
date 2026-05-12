# Example

## Scenario

A routing layer consumes generated catalogs from source-owner surfaces. The
catalogs must keep stable IDs, source references, object type, and route fields
so routing, SDK, and low-context readers can point back to owner truth without
copying the authored payload into a second canon.

## Why this skill fits

- the important risk is a boundary contract between source-owned producers and downstream consumers
- downstream assumptions need to become visible and reviewable
- validation should focus on the generated/export shape and source-owner limits, not only on builder internals

## Expected inputs

- the source-owned catalog producer and the consuming router or SDK surface
- the required generated/export fields and source references
- the current schema, fixture, smoke, or build-check surface for the boundary
- known downstream expectations that would break if the fields or claim limits drift

## Expected outputs

- explicit contract assumptions for the generated/export boundary
- schema, fixture, smoke, or build checks tied to the contract
- a short downstream impact note if the contract is changed or tightened
- a claim limit that says the generated surface summarizes owner truth rather than owning it

## Boundary notes

- this example is not a broad redesign of routing, SDK, or source-owner surfaces
- the point is to make the seam explicit, not to prove the whole federation correct
- if the owner or object meaning is unclear, use bounded-context mapping before contract testing

## Verification notes

- verify that the generated/export contract is visible in tests or structured checks
- verify that missing required fields fail in a reviewable way
- verify that source-owned meaning remains stronger than the generated consumer surface
- verify that the report names any unchanged weak spots in downstream coverage
