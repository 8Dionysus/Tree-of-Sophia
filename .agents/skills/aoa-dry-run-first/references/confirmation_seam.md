# Confirmation seam

The confirmation step should name the exact mutating action.

## Good confirmation shape

- command or operation name
- touched surface
- one-sentence risk note

## Example

Confirm the exact mutating step: switch symlink -> `ln -sfn previous current`

## Anti-patterns

- "Proceed?" with no mutation named
- reusing the dry-run command as the confirmation target
- confirming a batch of extra mutations that were not previewed
