# Test validation

Run the focused test for the changed owner first. Repository-wide tests use the
exact command recorded in the release sequence:

```bash
python -m unittest discover -s tests
```

Use `VALIDATION.md` at the repository root for release composition. Test
success is bounded evidence and does not establish semantic or owner
acceptance.
