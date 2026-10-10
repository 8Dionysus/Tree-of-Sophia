# Rust workspace

This is the initial build and installation boundary for the Tree of Sophia
Rust migration. `tos-foundation` owns shared codec and identity types;
`tos-source-store` owns bounded exact v1 corpus reads. The independent
`tos-conformance` package consumes their public APIs and tiny synthetic
fixtures. The `tos-reader` binary is a trusted-local, read-only adapter for an
exact retained revision and source ID. It stages and verifies selected bytes
before writing them to stdout. It grants no public access or current-use right.
`tos-segment-store` is a separate Linux local byte-custody candidate with
bounded immutable segments, crash-recoverable pin journals and receipt-bound
selected reads. Its independent conformance target checks exact synthetic
segment bytes. A custody receipt grants neither source admission nor rights.

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
persisted receipt spooling, the schema-worker process boundary, and rules that
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

Run the named `rust_workspace` lane from
[`docs/validation/validation_lanes.json`](../docs/validation/validation_lanes.json)
when the pinned toolchain, rustfmt and WASM target are available. Set
`CARGO_TARGET_DIR` to an owner-approved build-cache path outside the
checkout. Passing this lane proves only the checked Rust contracts, the portable
validation-backend WASM compilation, native reader installation,
installed mechanics executor lifecycle parity
and generated WEB.1 codec execution in Node WebAssembly against tiny synthetic
vectors. The WEB.1 route requires the matching `wasm-bindgen` CLI 0.2.128 and
Node. It does not prove a released public adapter, browser/Worker bundle
integration or production-scale runtime.

The native lane defaults each command to 300 seconds and caps the full lane at
3,600 seconds. The conformance suite runs in disjoint source and named family
commands so output and deadlines stay attributable to those test groups. Each
of the four grouped family commands has a 900-second command deadline. The
native executor keeps one shared 3,600-second lane deadline and caps every
command by the remaining lane time. Other steps retain the 300-second command
default.

The PostgreSQL integration target requires the explicit `postgres-lab` feature
and a dedicated ephemeral database. The ordinary workspace lane excludes this
target; it does not establish PostgreSQL execution. CI runs the durable
target with its PostgreSQL service:

```sh
: "${TOS_CMD_POSTGRES_URL:?dedicated PostgreSQL connection is required}"
cargo test -p tos-command --features postgres-lab --test postgres_durable_lab --locked -- --nocapture
```

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
