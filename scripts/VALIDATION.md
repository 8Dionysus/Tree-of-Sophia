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

## Local changed-path feedback

For a normal local edit, the release entrypoint accepts one or more explicit
repository-relative paths:

```bash
python scripts/release_check.py \
  --feedback \
  --changed-path access/src/tos_access/core.py
```

The entrypoint maps only reviewed implementation paths to existing focused
test files and unions the targets for a multi-file edit. The currently narrow
routes are the portable access product/deploy, active-naming validator,
bibliographic-graph implementation scripts, and corpus-index implementation
scripts. The access contract test is an explicit special case because its
helper is imported by the other access tests. Other test files are not selected
by filename prefix. ToS data, schemas, generated outputs, canon, and other
shared or unreviewed paths remain deliberately unresolved and fall back to the
complete release oracle. It is intentionally not a second validation manifest
or test inventory: `docs/validation/validation_lanes.json` remains command
authority, while `tests/test_inventory.json` remains descriptive coverage
metadata.

An unsupported, shared, or topology-drifted path falls back to the complete
manifest-owned release sequence. A malformed path is rejected with exit 2;
it is not treated as a fallback request. The feedback path reuses the release
pytest flags and only replaces the final `tests` operand with existing focused
targets. It is local edit feedback; it does not alter the release command,
release phase split, CI, or acceptance boundary.

Focused feedback invokes pytest with third-party plugin autoload disabled by
default (`PYTEST_DISABLE_PLUGIN_AUTOLOAD=1`) to keep an unrelated user-site
plugin from changing the selected check. A caller-provided value, including
`0` or an empty value, is preserved. This environment adjustment is limited to
the selected focused step: unsupported-path fallback, ordinary release runs,
and CI retain their caller environments. The development requirements keep
pytest unpinned and do not add a plugin dependency, so this is not a blanket
plugin suppression policy. Pytest interprets any non-empty value, including
`0`, as “disable autoload”; an empty value enables its normal autoload behavior.

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
