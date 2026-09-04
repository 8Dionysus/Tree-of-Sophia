# Local KAG-provider validation

Use the internal routes after the affected source export is known. While the
temporary ToS freeze is active, the KAG route performs only the exact frozen
family binding check:

```bash
python scripts/validation_lanes.py --run local_kag_provider
python scripts/validation_lanes.py --run public_entry
```

The public-entry route still checks the ToS-owned tiny entry. Full local KAG
record, provider, and generated-family validation is intentionally paused;
shared KAG composition and runtime freshness remain with `aoa-kag` and its
consumers.
The current ToS KAG family is frozen by `kag/indexes/hot_profile.json`; a
family or source-snapshot drift is a blocking local-provider failure until an
explicit ToS operator unfreeze command.
