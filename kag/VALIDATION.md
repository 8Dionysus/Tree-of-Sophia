# Local KAG-provider validation

Under TOS-D-0062, select one accepted corpus revision and build its bounded
source-return export outside the software checkout:

```sh
python scripts/build_kag_export.py build --store /path/to/corpus-store --revision CORPUS_SHA256 --output /path/to/new-export
python scripts/validate_local_kag_provider.py --export /path/to/new-export
```

The export contains a bounded canonical source, its public entry and supporting
documents. Its manifest binds the exact corpus revision, complete file hashes
and canonical source-return locator. It contains no KAG index family. Verification
rejects undeclared, missing, changed or linked files and mismatched source identity.

Build the independent local KAG artifact with an explicitly selected consumer:

```sh
python scripts/publish_kag_release.py build --store /path/to/corpus-store --revision CORPUS_SHA256 --kag-root /path/to/aoa-kag --release-root /path/to/kag-releases
python scripts/publish_kag_release.py status --release-root /path/to/kag-releases --expected-revision CORPUS_SHA256
```

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
