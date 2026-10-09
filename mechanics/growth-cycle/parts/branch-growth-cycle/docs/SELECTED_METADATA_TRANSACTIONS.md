# Selected metadata publication and recovery

The native `tos-command` owner maintains selected metadata publication and
recovery. `source_metadata_publication.rs` owns the transaction lifecycle,
`source_work_transaction.rs` owns the selected publication epoch, and
`source_revision_observation.rs` owns read-only proof for a committed selected
source correction. These Rust paths are the maintained implementation.

The committed-record observer retains the exact selected source and archive
bindings, requires a current committed transaction, and rechecks the publication
epoch while the caller reads. It does not replay the command, acquire a derived
publication lease, complete graph dependencies, or grant semantic, rights,
publication, or canon authority. A downstream publisher must retain its own
source-owner lock, check unrelated inputs, and reverify before committing a
derived output.

See [native behavior coverage](NATIVE_BEHAVIOR_COVERAGE.md) for the maintained
assertion and consumer routes. The former Python snapshot and transition
adapters are retired; historical conformance provenance remains evidence of
the prior implementation and is not an active runtime dependency.
