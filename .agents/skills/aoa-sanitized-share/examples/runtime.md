# Runtime Example

## Scenario
You want to share a support summary that includes a failing command, but the raw output contains hostnames, private paths, and an internal ticket reference.

## Why this skill fits
The task is to make the material shareable without exposing sensitive detail. The skill should preserve the lesson while removing or generalizing unsafe identifiers.

## Expected inputs
- the raw material to share
- the intended audience
- any known sensitive surfaces
- the minimum level of detail needed for the audience to understand the point

## Expected outputs
- a sanitized, shareable version of the material
- a short note about what was removed or generalized
- any remaining sensitivity warning that still matters

## Boundary notes
- If the material is already public-safe, do not over-sanitize it.
- If the real task is the underlying operational change, use a different skill first.
- Prefer summaries over raw excerpts when secrets or topology could leak.

## Verification notes
- Confirm sensitive surfaces were checked deliberately.
- Confirm the result still teaches the intended lesson.
- Confirm raw paths, tokens, and internal identifiers were not preserved by accident.
- Confirm any remaining uncertainty was called out plainly.
