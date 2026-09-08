# Local KAG-provider validation

The operator ended the temporary freeze in TOS-D-0044. Validate the complete
provider, including source ownership, current file hashes and shard integrity:

```bash
python scripts/validation_lanes.py --run local_kag_provider
python scripts/validation_lanes.py --run public_entry
```

The manifest owns these sequences. `public_entry` protects the narrow ToS
export; the portable source family additionally needs the pinned `aoa-kag`
builder and owner-family parity check described in `docs/RELEASING.md`.
Regenerate changed source projections before claiming currentness. There is
no freeze-only alternative or automatic refreeze.

Shared KAG composition and runtime freshness remain with `aoa-kag` and its
consumers. Source validation does not deploy or activate a consumer, accept
authored meaning, or prove rights, canon, artifact admission or live health.
