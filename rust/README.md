# Rust workspace

This workspace owns the Rust build and installation boundary for Tree of Sophia.
`tos-foundation` owns shared codec, identity and schema-execution binding types;
`tos-source-store` owns bounded exact V1 and native V2 corpus reads, descriptor
codecs and physical source custody. Admission and publication remain in
`tos-command`. The read-only `tos-reader` depends on storage directly, so its
build does not include command, compiler, query, validation or mechanics crates.
The independent `tos-conformance` package consumes public APIs and synthetic
fixtures. `tos-reader` is a trusted-local, read-only adapter for an exact retained
revision and source ID; it stages and verifies selected bytes before stdout.
`tos-segment-store` is a separate Linux local byte-custody candidate with
bounded immutable segments, crash-recoverable pin journals and receipt-bound
selected reads. Its independent conformance target checks exact synthetic bytes.

`tos-query` defaults to shared query rules, with only `tos-foundation` and
`regex` as direct dependencies. Native model opening, SQLite adapters and the
compiler-backed query implementations require its explicit `native-models`
feature. Access, command and conformance select it; `tos-web-rules` uses shared
rules without it on native hosts and WASM. Native disclosure leases require
`Send` in both configurations. Run selected-model integration tests with
`cargo test -p tos-query --features test-fixture`; this enables compiler fixtures
and the gzip reader. Default tests cover portable rules without the compiler or
storage stack, and Cargo declares the native tests' `required-features`.

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
the schema-worker process boundary, and rules that consume `tos-source-store`
remain native-only. Crate additions follow the workspace and lockfile owner.

`tos-source-store` defaults to exact format, custody and read operations.
Its explicit `capture` feature enables Git capture, import, archive restore and
chunk transfer for writer/tool consumers. The standalone `tos-reader` and
ordinary native query do not select that preparation feature.

`tos-native-owner-command source-capture import-corpus` prepares a fresh,
unselected V1 store. It can combine a current Git capture with explicitly
selected disjoint retained subtrees, each using the
[`retained-corpus-binding`](../ToS/contracts/retained-corpus-binding.schema.json)
contract. The target preserves every original snapshot and receipt, shares one
content-addressed object pool with the composed snapshot, and writes a V2
custody receipt. Retained members consume the original aggregate limits;
overlapping or nested namespaces are refused. Selection order does not change
the result. No current pointer, identity/dependency
index, semantic admission or publication authority is created. The command's
`--help` owns its exact input and resource options.

For selected-record corpus admission, the protected
`tos_local_native_foundation_invocation_v1` may explicitly contain
`record_selection_snapshot` with exactly `absolute_path`, `raw_utf8` and
`sha256`. The absolute normalized address must match the CLI's
`--record-selection-manifest` under `selected-record-closure` or
`selected-generated-record-closure`; an unused or mismatched snapshot is
refused. The prefixed SHA-256 binds the exact decoded UTF-8 bytes, preserving
whitespace and line endings without parsing and reserializing the manifest.
The existing selection parser, limits, validator identity and receipt binding
consume those bytes. This is a frozen snapshot: the manifest address is not
opened, and its independent filesystem currentness is not claimed. Host-held
initial and final custody checks authenticate the whole protected invocation,
including the snapshot, through the existing one-invocation transport. The
1 MiB invocation cap remains in force; decoded snapshot buffers and parse
overlap consume the original state budget. Omitting the field preserves
ordinary held-file selection and currentness checks. No filesystem fallback
or additional custody protocol operation is selected.

Run the named `rust_workspace` lane from
[`docs/validation/validation_lanes.json`](../docs/validation/validation_lanes.json)
when the pinned toolchain, rustfmt and WASM target are available. Set
`CARGO_TARGET_DIR` to an owner-approved build-cache path outside the checkout.

The executor defaults to 300 seconds per command and 3,600 seconds per lane.
Rust CI selects a three-hour aggregate budget with
`--lane-timeout-ms 10800000`; use that budget for the full local lane too.
The validation manifest owns the separate command limits and test groups.
The executor caps each command by the remaining lane time.

The full route is `--run rust_workspace`. CI may select
`--run rust_workspace --rust-group conformance|workspace|cold-install`; every
group runs the common `prepare` prefix: formatting, both WASM checks and exact
schema-worker, owner CLI and Access builds. Each job builds its own products; only `conformance` prepares
its Cargo JSON-selected test images. The manifest owns ordered commands and
limits. Review them with `--sequence rust_workspace --rust-group <group>` and
add `--cargo-timings` for Cargo reports. Growth exclusions and assertion counts
derive from the complete lane, including isolated tests in other groups; an
incomplete partition fails closed.

Conformance preparation uses `--exclude tos-access` and
`tos-query/test-fixture`; the exact Access CLI is built first. Workspace tests
include Access's default, research and load-readiness features, exclude
`tos-conformance`, and recheck the current-lane Access path and digest. Inherited
environment values cannot supply the conformance case digest. The release-actions
fixture runs separately at the start of `cold-install`. A locked dependency fetch
precedes the offline mechanics install; its verifier stays offline. The manifest
retains the full route's order, product selection and budgets.

`tos-validation-lanes --run rust_workspace --growth-jobs N` supports one to four
source-derived Growth classes concurrently, each in its own supervisor process.
The default is one. Each class retains its exact prepared
image, declared assertion/ignored counts, single test thread, output cap,
timeout and descendant custody. The parent joins the complete group before
later commands can rebuild its shared Cargo products. The first observed
failure cancels the group and stops the lane. CI selects one, two or four from
the same observed CPU and memory profile as Cargo: four requires at least four
confirmed CPUs and 12 GiB of effective available memory across the visible
cgroup hierarchy. This changes concurrent resource use, so local callers should
choose a setting within their admitted resource budget.

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
