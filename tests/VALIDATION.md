# Test validation

Software behavior runs on bounded fixtures:

```sh
tos-release-check --repo-root "$PWD" --phase tests
```

For a changed source builder or validator, run the affected native owner test
module or command. Ordinary ToS source and command behavior is covered by Rust
owner tests and the `rust_workspace` route. Python's default test discovery is
limited to the standalone SDK tests under `access/tests`. Tests marked
`data_release` require an explicitly selected `TOS_DATA_ROOT` and belong to
that data release; they are not part of the software command.

With that complete source snapshot explicitly selected, run the native data
checks separately. Missing required private layers fail the selected private-data
check; they do not produce a passing skip.

```sh
export TOS_DATA_ROOT=/absolute/materialized-source-snapshot
cargo test --locked -p tos-compiler tracked_current_route_preserves_census_and_authority_ceiling -- --ignored
cargo test --locked -p tos-compiler present_private_layers_rebuild_exactly -- --ignored
cargo test --locked -p tos-ops-mechanics-plan table_one_and_two_language_packets_cover_selected_text_corpora -- --ignored
cargo test --locked -p tos-ops-mechanics-plan --test source_routes_native current_source_routes_run_natively_and_preserve_authority_bounds -- --ignored
```

Prior Python assertions and acquisition references are retained as nonexecutable
`.py.snapshot` files under `tests/historical/`. Their maintained behavior runs through
the native owner tests above. They are not a second validation route.

The ignored `actual_native_corpus_managed_installed_consumer` case requires
explicit installed native products and host resource admission. Its producer
may inherit the host's private-stage ticket or enter that stage through the
absolute `TOS_NATIVE_CORPUS_STAGE_LAUNCHER` executable. The launcher receives
the bounded native command as arguments and issues the real sealed ticket
before execution. `TOS_NATIVE_CORPUS_STAGE_CONFIG` selects its bounded JSON
resource description: `quota_bytes`, `inode_limit`, `working_ram_bytes` and
the absolute `persistent_store`. OPS binds this description to the launcher's
actual admission. This per-producer launch keeps the test's fs-verity files
and HTTP host on their separately admitted filesystem and network. The native
producer always verifies the ticket, limits and kernel isolation itself.

The same case supports `TOS_NATIVE_CORPUS_PHASE=prepare`, `producer-prepare`, `mcp`, `http`,
`consumer`, `producer`, `check`, or `producer-read`. Omission selects `all`,
the complete fresh acceptance pass. `prepare` runs the original source and
cold-query assertions and writes a completed checkpoint selected explicitly
by absolute `TOS_NATIVE_CORPUS_CHECKPOINT`. It retains the exact source cut,
software capture and sealed model only after preparation succeeds. The log
prints its SHA-256. A later phase requires that path and
`TOS_NATIVE_CORPUS_CHECKPOINT_SHA256`; all retained inputs are checked before
and after use. Incomplete preparation cannot be resumed as a ready dataset.

The consumer recipe retains its small canon/Claims/navigation cut and frozen
query oracle. The six-product producer uses a separately identified cut that
preserves those members and adds the complete authored `ToS/philosophy/`
branch. Its preparation runs before catalog/index construction. To upgrade an
older consumer-only checkpoint without repeating that construction, select
`producer-prepare`: it creates a sibling `producer-prepared-<digest>.json`, prints its
digest and keeps the original checkpoint intact. Select the new checkpoint
for `producer`, `check` and `producer-read`. This operation adds real authored
inputs; it does not admit an incomplete candidate or synthesize graph rows.
Preparation also captures and restores a new exact Git fixture containing the
expanded source set and the unchanged original software companions. The source
cut and repository inventory therefore name the same selected files; the old
consumer capture remains intact.
The writer's working RAM must cover the complete philosophy producer; the
small consumer's process allowance is independent. The source-file ceiling
and cold-reader row ceiling also describe different resources.

`mcp` and `http` replay their existing assertions on fresh disposable copies of
the prepared snapshot. `consumer` also exercises provenance refusals and
revocation. `producer` builds the native candidate once and saves its completed
receipt; `check` compares its six products and exercises the damaged-product
refusal; `producer-read` admits that same candidate through the managed reader.
Source or producer changes require the corresponding producer result to be
rebuilt. A test-runner or transport change can reuse an otherwise applicable
prepared dataset. A checkpoint never saves or renews host authority: each
producer invocation still needs a current private-stage ticket. These focused
passes are evidence for their named phase; final acceptance also runs `all`
from fresh inputs.

Producer preparation selects the current compiled corpus-owner source in its
new software capture and records both its old and new hashes. Authored members,
the retained consumer pair and all other software companions remain unchanged.
Corpus schema checks retain the complete header and validate every growing
array in bounded portions through its exact array selector. The schema's
type/items-only decomposition, ordered coverage, invalid-tail refusal and
maximum-width scalar case have a focused Rust regression; receipt, byte and
worker limits remain unchanged.

Before corpus preparation, the case checks the supplied executables and uses
the existing small fixture to test fs-verity custody on the selected filesystem.
It then calls `tos-native-owner-command corpus-build --preflight` with the
same declared cold/process limits as the writer. This option shares request,
private-stage and cgroup admission with actual execution, opens no source and
creates no candidate. It reports independent request and host failures together.
Actual execution repeats the admission checks and binds the real source cut.

Select browser behavior through `software_browser` in root `VALIDATION.md`.
Test success establishes its declared mechanics, not source meaning, rights,
review, canon, deployment or ecosystem admission.

For the full retained relation cut, the existing inherited-view module also
has an ignored diagnostic test:
`retained_complete_relation_seek_matches_full_order_without_resorts`.
Select `TOS_INHERITED_RELATION_CUT` (absolute SQLite path, at most 512 MiB),
`TOS_INHERITED_RELATION_CUT_SHA256`, `TOS_INHERITED_RELATION_ROOT` and
`TOS_INHERITED_RELATION_COUNT`. The cut retains every complete relation row's
identity, endpoints, source order, payload length and digest from one producer
snapshot. It is a relation-query fixture, not a ready corpus or data release.
The test uses the production seek SQL and bundled SQLite, compares every
returned row with the complete source/id-ordered reference, and reports query
plan, row count, sorts, full-scan steps and VM steps. It requires zero sorts in
the paged traversal and a linear VM-step bound. Full producer acceptance remains
separate; a diagnostic pass does not replace it.
