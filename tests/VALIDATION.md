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
`.py.txt` files under `tests/historical/`. Their maintained behavior runs through
the native owner tests above. They are not a second validation route.

Select browser behavior through `software_browser` in root `VALIDATION.md`.
Test success establishes its declared mechanics, not source meaning, rights,
review, canon, deployment or ecosystem admission.
