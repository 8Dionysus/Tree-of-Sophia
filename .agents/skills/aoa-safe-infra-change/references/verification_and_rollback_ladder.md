# Verification and rollback ladder

Use stronger checks for stronger operational risk.

## Verification ladder

1. syntax / lint / render
2. dry-run / plan / diff
3. bounded rollout or startup status
4. health or smoke check on the changed surface

## Rollback rule

Have one concrete recovery idea before the mutation:

- undo command
- previous config reference
- previous release identifier
- restore or revert path
