# Example

## Scenario

A catalog export workflow directly reads local profile paths and writes installed artifacts from inside reusable source-to-output mapping logic. Tests are brittle because they need workspace-specific setup, and adding a second export profile would repeat the same path and write mechanics again.

## Why this skill fits

- the main problem is concrete filesystem and export-profile dependency leaking into logic that should stay reusable
- the module needs a clearer seam before further change
- tests and future export profiles benefit from a narrower boundary

## Expected inputs

- the target builder or workflow slice and the concrete path/write dependency currently embedded in it
- the source-to-output behavior the reusable logic actually needs from the dependency
- the desired scope of the refactor
- the validation path that will confirm behavior did not drift

## Expected outputs

- a narrower port that reflects what the mapping logic truly needs
- an adapter or equivalent seam around path discovery and artifact writing
- a clearer logic-versus-export-delivery boundary
- a short verification summary confirming generated output behavior stayed stable

## Boundary notes

- this example assumes the logic-versus-edge boundary is already clear enough to act on
- the focus is introducing a useful seam, not renaming packages or creating abstraction for ceremony alone
- generated/export contract validation remains a separate follow-up if downstream consumers rely on stable artifact shape

## Verification notes

- verify that the new port is narrower than direct filesystem/profile access
- verify that path discovery and artifact writing moved behind the adapter boundary
- verify that the refactor reduces coupling without changing the intended export behavior
