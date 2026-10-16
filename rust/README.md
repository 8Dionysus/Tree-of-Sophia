# Rust workspace

This workspace owns the Rust build and installation boundary for Tree of Sophia.
`tos-foundation` owns shared codec, identity and schema-execution binding types;
`tos-source-store` owns bounded exact V1 and native V2 corpus reads, descriptor
codecs and physical source custody. Admission and publication remain in
`tos-command`. The read-only `tos-reader` depends on storage directly, so its
build does not include command, compiler, query, validation or mechanics crates.
The independent
`tos-conformance` package consumes their public APIs and tiny synthetic
fixtures. The `tos-reader` binary is a trusted-local, read-only adapter for an
exact retained revision and source ID. It stages and verifies selected bytes
before writing them to stdout. It grants no public access or current-use right.
`tos-segment-store` is a separate Linux local byte-custody candidate with
bounded immutable segments, crash-recoverable pin journals and receipt-bound
selected reads. Its independent conformance target checks exact synthetic
segment bytes. A custody receipt grants neither source admission nor rights.

`tos-query` defaults to shared query rules, with only `tos-foundation` and
`regex` as direct dependencies. Native model opening, SQLite adapters and the
compiler-backed query implementations require its explicit `native-models`
feature. Access, command and conformance consumers select that feature;
`tos-web-rules` uses the shared rules without it on native hosts as well as WASM.
Native disclosure leases still require `Send` in both configurations.
Run selected-model integration tests with `cargo test -p tos-query --features
test-fixture`. That profile explicitly enables the compiler fixtures and gzip
reader needed by those tests. Default query tests exercise the portable rules
without building the compiler or storage stack. Native model tests are declared
with `required-features` in Cargo; the full validation lanes select `test-fixture`.

`tos-query::source_text_search` owns the portable `tos_source_text_search_v1`
ranking, Unicode-16 query normalization, cursor binding and response budget over
one complete bounded candidate set. The adapter still owns source selection,
currentness, rights and candidate completeness; this operation does not replace
the indexed-v2 or compressed-v3 search profiles.

`tos-validation` provides typed candidate validation interfaces and an offline,
locally supplied Draft 2020-12 schema probe. Its observed legacy format profile
is comparison evidence; its prospective four-format profile is not an admitted
source rule.

`tos-validation` defaults to its `native` profile. The separate `wasm` profile
is selected with `--no-default-features --features wasm`; it retains the typed
candidate, trace, and outcome interfaces, `SchemaResource`, `FormatProfile`,
`SchemaBackendProbe`, and the source-agnostic rule kernels in `item_rules`,
`layer_family_rules`, `provenance_rules`, `relation_rules`,
`semantic_registry_rules`, `source_copy`, `source_forms`,
`text_metadata_rules`, and `text_rules`. `assessment`, source-cut composition,
the schema-worker process boundary, and rules that
consume `tos-source-store` remain native-only. The WASM check establishes
compilation feasibility for the portable validation/backend surface; it does
not establish native-adapter availability, source admission, rule completeness,
or validation execution inside a WASM runtime.

OPS owns the root workspace, lockfile, toolchain, CI selection and package
route. FND owns `tos-foundation`; STO owns `tos-source-store`; ASS owns
conformance vectors and runner. Other owners add crates through an
OPS-reviewed workspace and lockfile update. This workspace contains only
synthetic test bytes, no production corpus data, and imports no sibling
repository.

`tos-source-store` defaults to exact format, custody and read operations.
Its explicit `capture` feature enables Git capture, import, archive restore and
chunk transfer for writer/tool consumers. The standalone `tos-reader` and
ordinary native query do not select that preparation feature.

`tos-native-owner-command source-capture import-corpus` prepares a fresh,
unselected V1 store. It can combine a current Git capture with an explicitly
selected retained subtree, using the
[`retained-corpus-binding`](../ToS/contracts/retained-corpus-binding.schema.json)
contract. The target preserves the original snapshot and receipt, shares one
content-addressed object pool with the composed snapshot, and writes a V2
custody receipt. Retained members consume the original aggregate limits;
overlapping namespaces are refused. No current pointer, identity/dependency
index, semantic admission or publication authority is created. The command's
`--help` owns its exact input and resource options.

Run the named `rust_workspace` lane from
[`docs/validation/validation_lanes.json`](../docs/validation/validation_lanes.json)
when the pinned toolchain, rustfmt and WASM target are available. Set
`CARGO_TARGET_DIR` to an owner-approved build-cache path outside the
checkout. Passing this lane proves only the checked Rust contracts, the portable
validation-backend WASM compilation, native reader installation,
and installed mechanics executor lifecycle parity against tiny synthetic
inputs. It does not prove a released public adapter, browser/Worker bundle
integration or production-scale runtime.

The executor defaults to 300 seconds per command and 3,600 seconds per lane.
Rust CI selects a three-hour aggregate budget with
`--lane-timeout-ms 10800000`; use that budget for the full local lane too.
The validation manifest owns the separate command limits and test groups.
The executor caps each command by the remaining lane time.

The PostgreSQL integration target requires the explicit `postgres-lab` feature
and a dedicated ephemeral database. The ordinary workspace lane excludes this
target; it does not establish PostgreSQL execution. CI runs the durable
target with its PostgreSQL service:

```sh
: "${TOS_CMD_POSTGRES_URL:?dedicated PostgreSQL connection is required}"
cargo test -p tos-command --features postgres-lab --test postgres_durable_lab --locked -- --nocapture
```

The addressed backup/reselection case in this target also needs two separate
empty databases on that same PostgreSQL 16 server, exposed as
`TOS_CMD2_V2_RESTORE_PG_URL` and
`TOS_CMD2_V2_V1_WIRE_RESTORE_PG_URL`. Set `TOS_CMD2_PG_DUMP_PATH` and
`TOS_CMD2_PG_RESTORE_PATH` to the matching PostgreSQL 16 client tools, and set
their `TOS_CMD2_PG_DUMP_SHA256` and `TOS_CMD2_PG_RESTORE_SHA256` values to the
corresponding binary hashes. The [`Repo Validation workflow`](../.github/workflows/repo-validation.yml)
shows the isolated-service setup; its restore database names include the
workflow run and attempt so concurrent runs cannot share a target.
The current-writer leg verifies the V8 source-navigation census, including the
range-coverage table; the separate V1-wire leg projects only the historical
nine-field receipt envelope over the same current dump, cut and store bytes.
That projection exercises wire compatibility and is not an authentic old dump.

Ignored restore and child-process probes retain their separate prerequisites and
are not selected by this command.

The synthetic CMD1 coordinator and its separate test target are retired. The
managed durable creation case checks real registered source-home and form
predicate generation, completeness and definition changes before publication.
Mutable external authored trees remain `FullOnly`; current v1 source selection
still performs complete O(N) manifest and retained-chain work outside commit.

For an explicitly selected local `--root`, Concept search and Word task/candidate validation accept a paired `--concept-max-file-bytes N --concept-max-total-file-bytes N` profile before the command. Both are positive byte counts and the file bound must not exceed the total. These options affect only Concept/Word file fixity; Reading uses its own selectors. Defaults, SQL VM/row/materialization/work/output limits and authority remain unchanged. A prepared or explicit release selection refuses these options. For the retained 160,477,184-byte private Concept DB, the proposed bounded profile is 201326592 bytes per file and 402653184 bytes total; execution still requires normal resource admission.

The installed `tos-ops-mechanics-plan --repo-root /absolute/Tree-of-Sophia
--tree-node-validate` checks canonical node schemas, exact numeric input,
identity uniqueness and shared language-witness spines. Its cross-field rules
are shared with the canon compiler. The selected schema remains source-owned;
structural success does not grant canon admission.
