# Test validation

Software behavior runs on bounded fixtures:

```sh
python scripts/release_check.py --phase tests
```

For a changed source builder or validator, run the affected owner test module.
Tests marked `data_release` require an explicitly selected `TOS_DATA_ROOT` and
belong to that data release. They are not part of the software command.

The historical full `tests/` suite mixes source-snapshot acceptance and code
regressions. Run it only for an intentionally materialized source snapshot:

```sh
python -m pytest -q -p no:cacheprovider --durations=20 tests
```

Select browser behavior through `software_browser` in root `VALIDATION.md`.
Test success establishes its declared mechanics, not source meaning, rights,
review, canon, deployment or ecosystem admission.
