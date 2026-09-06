# Test validation

Run the focused test for the changed owner first. Repository-wide tests use the
exact command recorded in the release sequence:

```bash
python -m pytest -q -p no:cacheprovider --durations=20 tests
```

The pytest collection also runs the root `unittest.TestCase` classes, while
including top-level pytest functions that unittest discovery cannot collect.
The release command prints the slowest twenty tests without adding a second
full-suite invocation.

Use `VALIDATION.md` at the repository root for release composition. Test
success is bounded evidence and does not establish semantic or owner
acceptance.
