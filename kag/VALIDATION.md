# Local KAG-provider validation

Use the internal manifest-backed routes after the affected source export is
known:

```bash
python scripts/validation_lanes.py --run local_kag_provider
python scripts/validation_lanes.py --run public_entry
```

These checks preserve local source return and generated parity. Shared KAG
composition and runtime freshness remain with `aoa-kag` and its consumers.
The current ToS KAG family is frozen by `kag/indexes/hot_profile.json`; a
family or source-snapshot drift is a blocking local-provider failure until an
explicit ToS operator unfreeze command.
