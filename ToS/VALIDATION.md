# ToS source-home validation

Choose the lane after identifying the authored branch and changed layer.
Exact internal sequences remain in `docs/validation/validation_lanes.json`.

```bash
python scripts/validation_lanes.py --run source_home
python scripts/validation_lanes.py --run source_witness_foundation
python scripts/validation_lanes.py --run philosophy_topology
python scripts/validation_lanes.py --run canon_contracts
python scripts/validation_lanes.py --run intake_contracts
python scripts/validation_lanes.py --run generated_parity
```

Run only the lanes relevant to the touched source. Validators do not accept
text, translation, rights, interpretation, review, or canon judgments.
