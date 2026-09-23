# Rust workspace

This is the initial build and installation boundary for the Tree of Sophia
Rust migration. `tos-foundation` owns shared codec and identity types;
`tos-source-store` owns bounded exact v1 corpus reads. The independent
`tos-conformance` package consumes their public APIs and tiny synthetic
fixtures. The `tos-reader` binary is a trusted-local, read-only adapter for an
exact retained revision and source ID. It stages and verifies selected bytes
before writing them to stdout. It grants no public access or current-use right.

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
checkout. Passing this lane proves only the checked Rust contracts, WASM
compilation and native reader installation against a tiny synthetic store. It
does not prove a released public adapter or production-scale runtime.
