# Tree of Sophia stats port

Tree of Sophia owns measurement questions and source populations; `aoa-stats`
owns shared grammar and composition. [port.manifest.json](port.manifest.json)
declares measurements and packets.

The reference measurement `Tree-of-Sophia/table-i-prepared-dossier-route-ratio`
counts unique Table I atlas rows with an explicit route to an existing
philosophy branch: 48 / 48 at source revision
`159d57402dcfc79c65af93fae7ce1346798dc392`. An empty set of routes over a valid
population yields zero. Missing, malformed, duplicate, unsupported or
out-of-population inputs yield unknown. The packet preserves its original
observation identity, time and evidence revision; a new observation requires a
new census.

Validate, publish and inspect the owner-local export:

```sh
tos-stats-release validate --port /path/to/selected-source/stats/port.manifest.json
tos-stats-release build --source-root /path/to/selected-source --release-root /path/to/stats-releases
tos-stats-release status --release-root /path/to/stats-releases --expected-revision PORT_EXPORT_SHA256
tos-stats-release verify --release /path/to/stats-releases/releases/EXPORT_SHA256
cargo test --locked -p tos-ops-mechanics-plan --no-default-features --features mechanics --test stats_release_native
```

The export binds port metadata and declared packets to an immutable identity.
The [embedded format](../rust/crates/tos-ops-mechanics-plan/schemas/README.md)
defines supported census values, missing states and evidence references.
`aoa-stats` performs consumer compatibility and composition checks.
