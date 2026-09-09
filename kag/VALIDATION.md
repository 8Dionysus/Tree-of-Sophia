# Local KAG-provider validation

The operator ended the temporary freeze in TOS-D-0044. Validate the complete
provider, including source ownership, current file hashes and segmented-family
integrity:

```bash
python scripts/validation_lanes.py --run local_kag_provider
python scripts/validation_lanes.py --run public_entry
```

The manifest owns these sequences. `public_entry` protects the narrow ToS
export; the local provider route additionally requires the clean exact
`aoa-kag` checkout selected by `kag/provider_pin.json`, full segment validation,
and a bounded reader probe. The owner-family parity check is described in
`docs/RELEASING.md`. Regenerate changed source projections before claiming
currentness. There is no freeze-only alternative or automatic refreeze.

The v3/v4 shard and distribution routes remain explicit rollback carriers.
They are never selected implicitly for a v5 manifest, and a missing or dirty
pinned provider fails closed.

Shared KAG composition and runtime freshness remain with `aoa-kag` and its
consumers. Source validation does not deploy or activate a consumer, accept
authored meaning, or prove rights, canon, artifact admission or live health.
