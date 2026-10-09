# Local KAG-provider validation

Under TOS-D-0062, select one accepted corpus revision and build its bounded
source-return export outside the software checkout:

The installed `tos-kag-release` command owns source-export construction,
verification, immutable publication and status. Its Rust owner is
`rust/crates/tos-ops-mechanics-plan/src/kag_release.rs`; source-return rules live
in `kag_corpus_export.rs`. Native tests in that source and
`rust/crates/tos-ops-mechanics-plan/tests/provider_controls_native.rs` cover
exact identities, repeat publication, failure preservation, complete membership,
changed consumer programs, historical program bindings and tamper refusal.

```sh
tos-kag-release export-build --repo-root /path/to/tos-software --store /path/to/corpus-store --revision CORPUS_SHA256 --output /path/to/new-export
tos-kag-release export-verify --release /path/to/new-export
```

The export contains a bounded canonical source, its public entry and supporting
documents. Its manifest binds the exact corpus revision, complete file hashes
and canonical source-return locator. It contains no KAG index family. Verification
rejects undeclared, missing, changed or linked files and mismatched source identity.

Build the independent local KAG artifact with an explicitly selected consumer:

```sh
tos-kag-release build --repo-root /path/to/tos-software --python /path/to/aoa-kag-python --store /path/to/corpus-store --revision CORPUS_SHA256 --kag-root /path/to/aoa-kag --release-root /path/to/kag-releases
tos-kag-release status --release-root /path/to/kag-releases --expected-revision CORPUS_SHA256
```

The explicitly selected interpreter belongs to the external aoa-kag consumer.
ToS export, verification and status require no Python. The selected aoa-kag
version must expose `scripts/validate_repo_local_kag_family.py --probe-source`:
it validates both the complete family and provider home before returning the
exact source identity. Published v1 releases with the historical four-program
binding remain readable; new publications also bind this owner CLI.

The consumer receives a private copy of the export and owns its full index,
shard, schema and family checks. A fresh consumer process verifies the produced
family and exact canonical source return before local promotion. Failed attempts
retain the previous successful artifact. Status exposes the selected corpus
revision, success identity/time, latest attempt/error and source-revision lag.
This operation does not install or activate an AbyssOS consumer, federation or MCP
service. A failed or lagging integration does not block standalone ToS software.

Releases use their own `integration_revision`, so a new consumer can publish a
new result from the same corpus. The provider template is materialized only in
the external release. The actual aoa-kag provider-home reader also checks that
home, with its cold shards present there. Select
`releases/INTEGRATION_SHA256/provider/Tree-of-Sophia` as `TREE_OF_SOPHIA_ROOT`
for existing explicit provider-root configuration. The separate query route is:

```sh
python /path/to/aoa-kag/scripts/query_repo_local_kag.py --repo-root /path/to/kag-releases/releases/INTEGRATION_SHA256/provider/Tree-of-Sophia --artifact-root /path/to/kag-releases/releases/INTEGRATION_SHA256/artifacts --no-shadow-git --mode exact --limit 1 ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json
```

The former requirement to regenerate the provider in every source PR is
superseded. This changes release boundaries, not the integrity requirements for
a KAG artifact. External composition, runtime freshness and consumer admission
remain with aoa-kag and the consuming owner. No validation here deploys a service
or accepts source meaning, rights, canon or a runtime artifact.
