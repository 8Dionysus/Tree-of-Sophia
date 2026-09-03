# Local stats-port validation

Point `AOA_STATS_ROOT` at the exact compatible `aoa-stats` owner checkout, then
run the local port checks:

```bash
python scripts/validate_local_stats_port.py
python -m unittest tests.test_local_stats_port
```

The reference packet is not live state, and a green validator does not make a
measurement authoritative beyond its declared population and evidence ref.
