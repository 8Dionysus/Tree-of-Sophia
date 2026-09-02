# Tree of Sophia Access

`access/` is the standalone, source-read-only product surface for Tree of
Sophia. It gives native MCP, HTTP, CLI, the web application, and WebMCP one
query core and one fingerprinted projection set. Page commands may change the
browser presentation, but never ToS source, review, rights, or canon state.

Authored meaning stays under `ToS/`. This package reads allowlisted derived
exports and always returns their `source_ref` routes; it never promotes canon,
changes review state, or writes to the corpus.

## Development install

Create a local environment with `python -m venv .venv`, install the product
with `.venv/bin/python -m pip install -e 'access[mcp,dev]'`, then use
`.venv/bin/tos doctor`, `.venv/bin/tos serve`, or `.venv/bin/tos mcp`.

`tos serve` is loopback-only by default and serves the checked-in production
web assets plus the JSON API. `tos mcp` uses stdio unless a loopback-only
streamable HTTP transport is selected with `TOS_MCP_TRANSPORT`.

## Standalone archive

Build a release candidate with
`python access/packaging/build_standalone_bundle.py --output dist/tos-standalone.zip`
and validate it with
`python access/packaging/validate_standalone.py --bundle dist/tos-standalone.zip`.
Validation requires the adjacent external `.zip.manifest.json` digest sidecar;
use `--manifest` when the sidecar is stored under another path.

The archive contains the installable `access/` package, prebuilt web assets,
and only the runtime data allowlist. It contains no Git metadata, sibling
repository, restricted source payload, lexical projection, Neo4j database, or
AbyssOS runtime dependency.

After extraction, install the full standalone profile from any location:
`python -m pip install '/path/to/tree-of-sophia-standalone/access[mcp]'`. Then
run `tos verify --profile standalone`, `tos serve`, or `tos mcp`.

## Contracts

- `contracts/runtime-manifest.v1.json` defines dual runtime posture.
- `contracts/runtime-data.v1.json` is the publication/bundle allowlist.
- `contracts/query-operations.v1.json` owns transport-neutral read operations.
- `contracts/epistemic-packet.v1.schema.json` defines the projection-bounded
  epistemic result and its fail-closed authority boundary.
- `contracts/page-commands.v1.json` owns revisioned browser context and shared
  human/WebMCP actuation.
- `contracts/web-actions.v1.json` is retained only as the v1 migration marker.
- `profiles/standalone.v1.json` is the required no-AbyssOS profile.
- `profiles/abyssos.v1.json` declares optional ecosystem adapters.

`tos verify --profile abyssos` additionally requires `TOS_ABYSSOS_ROOT` to
point at an AbyssOS root containing `abyss-stack`; this setting is never
required by the standalone profile.

The current `ToS` projection v1/v2 contracts remain source-owned. Their legacy
`runtime_owner` field describes the existing downstream deployment contract;
it does not override this product's standalone runtime manifest.

## WebMCP posture

The site feature-detects the experimental `document.modelContext` API and
remains fully usable when it is absent. Stable tools expose view, search,
selection, focus, cancellation, and page-context commands. Selection-dependent
tools are registered only for the current node or edge and bind the captured
context revision, so a delayed reference to “this edge” fails closed after the
human changes selection. Tool execution forwards the browser-provided
`AbortSignal` through page commands to HTTP queries.

The first epistemic reading command exposes source-return routes, candidate or
canon posture, and projected `contested_by`, `uncertain_relation`, and
`polemicizes_with` signals around the current selection. It reports partial
coverage explicitly: graph challenge signals are review leads, not adjudicated
counterevidence, rights clearance, or semantic truth.

The implementation follows the WebMCP Community Group draft shape current at
`webmachinelearning/webmcp@41d12f057167ccf5954dbcf49d99502cb6c84491`:
`document.modelContext.registerTool()`, registration lifecycle by
`AbortSignal`, and execution cancellation through callback options. This is an
experimental browser surface, not a ToS authority or availability guarantee.
