# Local memo-port validation

`aoa-memo` owns the validator and durable memory contract. Point
`AOA_MEMO_ROOT` at the exact intended owner checkout before running:

```bash
python "$AOA_MEMO_ROOT/scripts/memory/validate_local_memo_port.py" --path memo
python "$AOA_MEMO_ROOT/scripts/memory/build_local_memo_port_index.py" --path memo --check
```

Index rebuilding changes generated local read models; perform it only through
the owner builder after source packets move. A green check does not land
durable memory.
