# Manifest validation

The current manifests describe mechanics-owned posture. Run the corresponding
mechanics routes from the repository root:

```bash
python scripts/validation_lanes.py --run mechanics_local
python scripts/validation_lanes.py --run mechanics_topology
```

Passing checks do not activate a hook or create runtime authority.
