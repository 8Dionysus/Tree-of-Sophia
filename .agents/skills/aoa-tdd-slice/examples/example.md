# Example

## Scenario

A catalog builder needs a new behavior: when a source bundle declares an optional support reference, the generated compact export should include that reference once with its source path, while existing bundles without support references should continue to export unchanged.

## Why this skill fits

- the desired behavior is clear before implementation starts
- the change is small enough to stay inside one bounded slice
- confidence matters because downstream readers rely on the generated export shape
- the test can drive the source-to-output mapping without hand-editing the generated artifact

## Expected inputs

- the desired source-to-export behavior and the exact fixture shape
- the builder or command that produces the compact export
- the existing test surface for builder behavior
- non-goals such as redesigning catalog schema, rewriting routing, or changing unrelated export fields

## Expected outputs

- a failing test that expresses the new support-reference mapping before implementation
- the smallest implementation change that makes the test pass
- a short verification summary naming the relevant passing tests

## Boundary notes

- this example is about a bounded behavior slice, not a broader rewrite of the validation framework
- unrelated cleanup such as renaming modules, changing generated sort order, or rewriting export packaging stays out of scope
- it is not a source-of-truth decision because the source owner is already clear; the task is to specify one observable builder behavior

## Verification notes

- add or update the builder fixture test first
- run the focused test suite that covers source bundles with and without support references
- report the covered behavior and note that unrelated export fields were left untouched
