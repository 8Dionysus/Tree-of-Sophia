# TDD Slice Shapes

Use this reference when the test-first slice is wider than a simple function or
method change. It is not a checklist. Pick the smallest shape that names one
observable behavior, one red check, one minimal green implementation, and one
refactor stop-line.

## How To Use

1. Name the behavior in terms a consumer or maintainer can observe.
2. Choose the narrowest slice shape below.
3. Write the red check before changing implementation.
4. Make the smallest green change.
5. Refactor only inside the declared slice after the check passes.

## Shape Set

| Shape | Red Check | Minimal Green | Refactor Boundary | Avoid | Verify |
|---|---|---|---|---|---|
| Pure module behavior | Unit or focused regression test for input, output, error, or state. | Change the smallest rule or branch. | The module or function under test. | Testing private helper choreography. | Focused unit tests plus relevant existing suite. |
| CLI or tool behavior | Command invocation, exit code, machine-readable report, or stable stderr/stdout contract. | Add the flag, parser branch, or report field. | Command handler and report shape only. | Freezing incidental log prose. | CLI test with success and failure path when useful. |
| Parser, validator, schema, or manifest | Valid and invalid fixture that names the accepted or rejected shape. | Add the narrow validation or parse rule. | Parser/validator and fixture set. | Treating current formatting as semantic behavior. | Fixture test plus schema or invalid-case failure. |
| Builder or generated/export path | Source fixture rebuilds one derived artifact or detects no-drift. | Update source-to-output mapping, not derived output by hand. | Builder and source fixture. | Making generated artifacts the source of truth. | Rebuild/no-drift check plus focused assertion. |
| Router or selection behavior | Equivalent inputs route the same; decisive signal changes route. | Add or adjust the small selection rule. | Router rule and its fixtures. | Encoding incidental ordering as law. | Permutation or tie-break test with claim limit. |
| Adapter or facade behavior | Fake adapter or facade test for one observable dependency interaction. | Add the narrow port call or mapping. | Facade/adapter seam already chosen. | Redesigning the dependency boundary mid-slice. | Fake plus one smoke if the real adapter is cheap and available. |
| Workflow or gate behavior | Minimal route trial, state transition, receipt field, or stop condition. | Add the narrow phase, gate, or receipt rule. | Workflow step and receipt/report fields. | Turning one slice into a whole process rewrite. | Focused workflow test plus negative skip case when relevant. |
| Regression repair | Failing test that reproduces the observed break. | Fix the smallest behavior that makes the regression pass. | Fault-local code and tests. | Broad cleanup before the regression is pinned. | Regression test stays in suite and explains the behavior. |

## Compact Slice Pass

| Field | Answer |
|---|---|
| Observable behavior |  |
| Shape selected |  |
| Red check |  |
| Minimal green change |  |
| Refactor boundary |  |
| Out of scope |  |
| Existing checks to rerun |  |
| Claim limit |  |

## Verification

- the first failing check constrains consumer-visible or maintainer-visible
  behavior
- the green change is smaller than a redesign
- generated/export assertions point back to source-owned builder behavior
- snapshots or goldens are treated as stable only when consumers rely on them
- the closeout names both covered behavior and untouched behavior
