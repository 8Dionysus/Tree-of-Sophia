# Dry-run limitations

Use this file when the preview exists but does not prove the whole change is safe.

## Record at least these boundaries

- what the preview exercised
- what the preview did not exercise
- which real mutation still remains
- what could still fail after a clean preview

## Good language

- "This preview confirms the render shape but not post-apply runtime health."
- "This inspection shows the target files, not whether the write will succeed."
- "This plan reduces uncertainty but does not prove rollback safety."
