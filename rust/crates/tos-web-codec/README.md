# WEB.1 codec binding

`tos-web-codec` is a small `wasm-bindgen` adapter over the shared
`tos-foundation` source. `codec_v1` accepts a raw `Uint8Array`, an operation,
and an explicit versioned profile advertised by the foundation. The profiles
include strict and request JSON, corpus snapshot with final LF, source record
without LF, and source command input without LF and with strict duplicate
rejection. It returns a structured result with exact output
bytes or a stable error code and optional original-input byte offset. The
host may decode successful bytes for display but must not parse input JSON
before Rust. `codec_capabilities_v1` declares the ABI and the exact supported
foundation profiles; unknown operations and profiles fail closed.

The binding has no filesystem, D1, Fetch, DOM, rights, source admission or
publication access. It is an E1 codec/host feasibility result, not a query
port or a public Worker artifact. Browser and Worker bundling need their own
installed-artifact checks. Independent vectors live in
`tests/conformance/rust/foundation.jsonl` and
`canonical-profiles-v1.jsonl`; a separately reviewed bounded float sample
is held in execution evidence.

To run the host check, OPS registers the crate in the root Cargo workspace,
builds `--target wasm32-unknown-unknown --features wasm --release`, then
runs the matching version of `wasm-bindgen --target web` against the cdylib.
Place generated JS/WASM in a temporary package marked `type: module`, then
pass the generated JS, generated WASM, both repository fixture paths and the
float evidence path to `tests/wasm-host.mjs`. Node's WebAssembly runtime executes the same Rust
semantic code as native; the script measures artifact bytes, cold startup and
small-vector process memory. These measurements are an initial host envelope,
not a production capacity profile.

When an existing local Miniflare/workerd installation is available,
`tests/worker-host.mjs` can load the same generated glue and compiled WASM
module binding in an ephemeral module worker. It checks one raw-byte canonical
response and one strict duplicate error without D1 or remote deployment.
