# Tree of Sophia Access

`access/` is the standalone, read-only product surface for Tree of Sophia.
It gives native MCP, HTTP, CLI, and the web application one query core and one
fingerprinted projection set. The next WebMCP implementation belongs here and
will call the same browser actions already used by the site.

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
- `contracts/web-actions.v1.json` is the browser action ABI for the site and
  future WebMCP registration.
- `profiles/standalone.v1.json` is the required no-AbyssOS profile.
- `profiles/abyssos.v1.json` declares optional ecosystem adapters.

The current `ToS` projection v1/v2 contracts remain source-owned. Their legacy
`runtime_owner` field describes the existing downstream deployment contract;
it does not override this product's standalone runtime manifest.
