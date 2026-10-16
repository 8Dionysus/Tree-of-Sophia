# ToS source-home validation

Choose the lane after identifying the authored branch and changed layer.
Exact internal sequences remain in `docs/validation/validation_lanes.json`.

```bash
tos-validation-lanes --repo-root "$PWD" --run source_home
tos-validation-lanes --repo-root "$PWD" --run source_witness_foundation
tos-validation-lanes --repo-root "$PWD" --run philosophy_topology
tos-validation-lanes --repo-root "$PWD" --run canon_contracts
tos-validation-lanes --repo-root "$PWD" --run intake_contracts
tos-validation-lanes --repo-root "$PWD" --run generated_parity
```

Run only the lanes relevant to the touched source. Validators do not accept
text, translation, rights, interpretation, review, or canon judgments.
