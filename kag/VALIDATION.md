# Local KAG-provider validation

Under TOS-D-0062, these checks belong to a separately selected KAG integration
artifact. Select its exact ToS source/data revision and provider revision:

```sh
python scripts/validation_lanes.py --run local_kag_provider
python scripts/validation_lanes.py --run public_entry
```

Run with that revision materialized and its declared export inputs available.
Validate source ownership, file hashes, shard integrity and canonical parity
before publishing the selected artifact. An older KAG artifact must report its
actual source revision and staleness; it cannot claim latest-source currentness.
A failed or lagging KAG integration does not block standalone ToS software.

The former requirement to regenerate the provider in every source PR is
superseded. This changes release boundaries, not the integrity requirements for
a KAG artifact. External composition, runtime freshness and consumer admission
remain with aoa-kag and the consuming owner. No validation here deploys a service
or accepts source meaning, rights, canon or a runtime artifact.
