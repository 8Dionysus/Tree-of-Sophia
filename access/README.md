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
- `contracts/evidence-lens-packet.v1.schema.json` defines the joined page/API
  result and its compact WebMCP `agent_summary`.
- `contracts/page-commands.v1.json` owns revisioned browser context and shared
  human/WebMCP actuation.
- `contracts/research-workspace.v1.schema.json` defines the portable local
  session packet for hypotheses, exclusions, route comparisons, notes, and its
  action journal.
- `contracts/web-actions.v1.json` is retained only as the v1 migration marker.
- `profiles/standalone.v1.json` is the required no-AbyssOS profile.
- `profiles/abyssos.v1.json` declares optional ecosystem adapters; the profile
  is currently paused by the ToS integration posture until an explicit owner
  command reopens it.

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

The Evidence Lens command works on philosophy projection selections and on
canonical relations in the corpus `route-graph`. Its first two curated scenes
contrast a retained Zarathustra canon relation with open modern claim/evidence
closure against the contested pre-canon Archaic Tribute reading. The page
receives the full route packet; WebMCP receives a compact summary below 1,500
characters with posture, bounded conclusion, route counts, gaps, and next
actions. Projected `contested_by`, `uncertain_relation`, and
`polemicizes_with` relations remain review leads rather than adjudicated
counterevidence.

The local research workspace lets the human and agent work on the same
temporary investigation: exclude a relation, save the direct and alternative
routes, add notes, or draw a working hypothesis as a visibly distinct edge.
Every hypothesis is structurally fixed as `session_hypothesis: true`,
`source: false`, `reviewed: false`, and `canon: false`. Undo/redo, browser-local
persistence, and a validated JSON export/import packet are supported. None of
these actions writes to candidate intake, review ledgers, authored ToS source,
or canon; any future proposal/writeback flow requires a separate explicit
contract and user confirmation.

The product shell makes this shared surface visible instead of assuming the
browser integration worked. Its header panel reports WebMCP availability,
registered and selection-bound tool counts, the current context revision, and
any registration failure. Without WebMCP it gives a graceful native `tos mcp`
fallback while the atlas remains usable. The same panel contains three
bilingual, copyable prompts for the core demonstration loop: inspect evidence,
reroute around a disputed edge, and preserve an interpretation as a local-only
hypothesis.

Every successful `Repo Validation` run builds and validates
`tree-of-sophia-standalone.zip`, then uploads the archive and its external
digest manifest as a downloadable workflow artifact. This is a source-bound
release candidate, not an official ToS release or an AbyssOS artifact-admission
verdict; official publication still follows `docs/RELEASING.md`.

The implementation follows the WebMCP Community Group draft shape current at
`webmachinelearning/webmcp@41d12f057167ccf5954dbcf49d99502cb6c84491`:
`document.modelContext.registerTool()`, registration lifecycle by
`AbortSignal`, and execution cancellation through callback options. This is an
experimental browser surface, not a ToS authority or availability guarantee.
