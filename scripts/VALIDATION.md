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
