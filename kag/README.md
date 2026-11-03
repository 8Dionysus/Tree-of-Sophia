# ToS source-return export

Build a source-return export from an explicitly selected immutable corpus
revision. It preserves source bytes, membership, hashes and source-return
identities; `aoa-kag` imports it, builds indexes and checks compatibility.

```sh
tos-kag-release export-build --repo-root /path/to/tos-software \
  --store /path/to/corpus-store --revision CORPUS_SHA256 --output /path/to/new-export
tos-kag-release export-verify --release /path/to/new-export
cargo test --locked -p tos-ops-mechanics-plan --no-default-features --features mechanics --test kag_publication_native
```

The [native producer](../rust/crates/tos-ops-mechanics-plan/src/kag_corpus_export.rs)
binds the selected revision and checks exact membership and file fixity. See the
[source export contract](../mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md)
for the artifact and source-return rules.
