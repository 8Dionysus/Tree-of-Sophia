# Local KAG-provider validation

Use the internal routes after the affected source export is known. While the
temporary ToS freeze is active, the KAG route performs only the exact frozen
family binding check:

```bash
python scripts/validation_lanes.py --run local_kag_provider
python scripts/validation_lanes.py --run public_entry
```

The public-entry route still checks the ToS-owned tiny entry. Full local KAG
record, regeneration, and source-currentness validation is intentionally
paused; shared KAG composition and runtime freshness remain with `aoa-kag` and
its consumers. The freeze-only route remains blocking: it verifies the frozen
manifest binding plus every shard's bytes, digest, and record count. Changes in
the live ToS sources are expected during the freeze and do not invalidate the
frozen snapshot. Changing the frozen family itself requires an explicit ToS
operator unfreeze command.
