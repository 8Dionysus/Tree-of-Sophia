# ToS export validation

Build the native ToS source-return export from an explicitly selected corpus,
then verify that artifact without a consumer checkout or interpreter:

```sh
tos-kag-release export-build --repo-root /path/to/tos-software --store /path/to/corpus-store --revision CORPUS_SHA256 --output /path/to/new-export
tos-kag-release export-verify --release /path/to/new-export
cargo test --locked -p tos-ops-mechanics-plan --no-default-features --features mechanics --test kag_publication_native
```

The manifest binds the exact corpus revision, full membership, hashes and
canonical source-return locator. Verification refuses missing, extra, linked
or changed files. Native unit checks in `kag_corpus_export.rs` additionally
cover the bounded producer and source-return rules.

The KAG consumer constructs and validates its indexes at its own boundary.
ToS never selects or invokes that consumer's source, Python runtime or validator.
A software release does not rebuild a production corpus or consumer index.
Mechanical verification grants no source, rights, canon or runtime admission.
