# Script validation

Select the source or generated consumer before the script. Internal sequences
are loaded from `docs/validation/validation_lanes.json`.

```bash
python scripts/validation_lanes.py --run validation_authority
```

The repository route-docs and release sequences are owned by
[root `VALIDATION.md`](../VALIDATION.md#run). The default release route remains
the complete ordered sequence. CI may run its complementary `--phase checks`
and `--phase tests` partitions in parallel; both remain required before a
standalone candidate is built.

Run the narrowest affected lane first. A builder may mutate only its declared
generated outputs; validator success does not create source or runtime truth.

## Local active-naming feedback

The blocking `active_naming` lane intentionally remains the ordinary uncached
command:

```bash
python scripts/validate_active_naming.py
```

For repeated local edits, the same entrypoint accepts an explicit external
SQLite cache for the pure content-result check:

```bash
python scripts/validate_active_naming.py \
  --feedback-cache /srv/abyss-machine/cache/tree-of-sophia/active-naming.sqlite
```

This is a local feedback hint only. The cache stores content digests, the
resulting pure-check value, and a validator/Python policy key; it must remain
outside the repository. It does not cache discovery, path checks, file reads,
or route-scoped experience markers. Malformed, locked, or unavailable cache
state is recomputed uncached. Prefer one cache path per worktree/task when
running concurrent local feedback. Do not pass this option from CI, release,
or gate commands; cache rows are not authenticated evidence and a plausible
local row tamper remains outside the release trust boundary.

If the final cache commit/close fails, the run's validation result may contain
both cache hits and fresh checks; only cache persistence is incomplete. The
run remains local feedback, not release evidence.
