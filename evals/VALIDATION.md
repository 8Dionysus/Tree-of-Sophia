# Local eval-port validation

This port is advisory. `aoa-evals` owns central proof and verdict semantics.
Point `AOA_EVALS_ROOT` at the exact intended owner checkout before running:

```bash
python "$AOA_EVALS_ROOT/scripts/validate_local_eval_port.py" --target-root .
```

Local validation does not admit a bundle or create a central verdict.
