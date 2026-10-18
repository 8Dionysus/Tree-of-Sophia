# ToS source-return export

ToS builds and verifies its own bounded export from an explicitly selected
immutable corpus revision. The output preserves canonical source bytes, exact
membership, hashes and source-return identities. It contains no consumer index
family, runtime installation or neighbor-repository configuration.

```sh
tos-kag-release export-build --repo-root /path/to/tos-software \
  --store /path/to/corpus-store --revision CORPUS_SHA256 --output /path/to/new-export
tos-kag-release export-verify --release /path/to/new-export
```

The native implementation is
[`kag_corpus_export.rs`](../rust/crates/tos-ops-mechanics-plan/src/kag_corpus_export.rs).
The export has an 8 MiB bound and binds all declared source members plus the
bounded KAG capsule. Missing, undeclared, linked or changed files are refused.
The selected corpus supplies source bytes; the software checkout supplies the
versioned producer. No neighboring checkout or Python interpreter is selected.

A KAG consumer imports this artifact, constructs its indexes and validates its
own provider, schemas, shards and admission in its own repository. The former
ToS commands for consumer build/status/verification, Python program identity
and provider-template materialization have been retired. Existing consumer
artifacts remain the consuming owner's responsibility.

Software, corpus and export identities stay separate. Successful byte
verification does not accept philosophical meaning, rights, review or canon.
See [validation](VALIDATION.md) and the
[source export contract](../mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md).
