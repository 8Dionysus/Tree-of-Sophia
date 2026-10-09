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

The historical root `tests/` suite mixes source-snapshot acceptance and code
regressions. It is not the ordinary software test route; run it only for an
intentionally materialized source snapshot:

```sh
python -m pytest -q -p no:cacheprovider --durations=20 tests
```

Select browser behavior through `software_browser` in root `VALIDATION.md`.
Test success establishes its declared mechanics, not source meaning, rights,
review, canon, deployment or ecosystem admission.
