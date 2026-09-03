# Mechanics validation

Select the changed package or part before running checks. Exact internal
sequences remain in `docs/validation/validation_lanes.json`.

```bash
python scripts/validation_lanes.py --run mechanics_topology
python scripts/validation_lanes.py --run mechanics_local
```

Use `experience_contracts`, `questbook_surface`, `public_entry`, or
`artifact_bundles` when that named package surface changed. Mechanics checks
prove operation shape only; they do not author ToS meaning or runtime policy.
