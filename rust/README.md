# Rust workspace

This is the initial build and installation boundary for the Tree of Sophia
Rust migration. The `tos-foundation` crate is a buildable scaffold for FND;
it does not yet implement a public codec or ABI. The `tos-workspace-probe`
binary proves that a native crate can be linked and installed outside the
repository. It is not a Tree of Sophia reader or release artifact.

OPS owns the root workspace, lockfile, toolchain, CI selection and package
route. FND owns `tos-foundation` source and crate dependencies. Other owners
add crates through an OPS-reviewed workspace and lockfile update. This
workspace contains no corpus data and imports no sibling repository.

Run the named `rust_workspace` lane from
[`docs/validation/validation_lanes.json`](../docs/validation/validation_lanes.json)
when the pinned toolchain, rustfmt and WASM target are available. Set
`CARGO_TARGET_DIR` to an owner-approved build-cache path outside the
checkout. Passing this lane proves only the scaffold's native/WASM build and
native probe installation.
