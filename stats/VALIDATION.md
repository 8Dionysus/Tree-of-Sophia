# Local stats export validation

ToS validates its declaration, packet references, measurement identities,
privacy posture and observed ratios with its own native implementation:

```sh
tos-stats-release validate --port /path/to/selected-source/stats/port.manifest.json
cargo test --locked -p tos-ops-mechanics-plan --no-default-features --features mechanics --test stats_release_native
```

The current export supports census ratios with matching `kind` and unit.
`observed` requires the complete consistent numerator, positive denominator
and number. Declared `missing`, `unknown` and `stale` values require a reason;
missing or unknown cannot assert a number. Stale may retain a complete checked
ratio. Unsupported statuses, kinds and partial values refuse publication.
The port's `reference_only` capability requires a packet `reference` posture,
matching measurement capability and privacy, and supported freshness. A stale
value must declare stale freshness. These checks never promote a reference
observation to live state.

No consumer repository, interpreter or schema checkout is selected. Shared
format identifiers and contract references are portable compatibility metadata;
ToS does not fetch or execute their external owners. Consumers perform their
additional grammar, composition and admission checks on their side.

The native validator embeds the complete frozen v1 port, measurement and packet
structure from the [portable format snapshot](../rust/crates/tos-ops-mechanics-plan/schemas/README.md).
All schema references resolve inside that software product, in offline mode.
Required fields and types are checked before the ToS-owned question, census,
dimension, ratio, export-posture and observation bindings. This is a versioned
format dependency, with no consumer source or runtime dependency.

Publish and inspect the exact ToS export:

```sh
tos-stats-release build --source-root /path/to/selected-source --release-root /path/to/stats-releases
tos-stats-release status --release-root /path/to/stats-releases --expected-revision PORT_EXPORT_SHA256
tos-stats-release verify --release /path/to/stats-releases/releases/EXPORT_SHA256
```

The source revision binds the four port metadata files and every declared packet.
The declaration owns packet paths; adding or renaming a packet needs no program
edit. `export.json` binds the
immutable export identity, full membership, hashes, evidence posture and original
observations, each with its packet ref. Validation and publication require the
same bounded source closure. Verification rejects changed, missing, extra or linked files. A
failed preparation does not replace the previous successful export. Status checks
that successful artifact before reporting it.

A reference packet is not live state. Its observation ID, time and source revision
remain historical. A new observation requires the exact population and derivation
owned by ToS; do not rewrite an old observation when the current atlas changes.
Validation does not turn route presence into source adequacy, philosophical
quality, canon, live state or downstream admission.
