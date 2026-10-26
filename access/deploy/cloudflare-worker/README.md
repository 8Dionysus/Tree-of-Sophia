# Cloudflare Worker deployment

This package owns the repository's Cloudflare delivery route for Tree of
Sophia. The Worker serves the browser application and bounded, read-only
projection queries over D1. The Rust native product owns public data
generation, offline SQLite import, and the selected local verifier. Cloudflare
credentials, production publication, ToS meaning, review, rights and canon have
their own owners.

The public route is configured in [`wrangler.jsonc`](wrangler.jsonc); that file
does not prove a current remote deployment or its dashboard build settings.
The Worker can serve the browser and projections without an always-on origin
machine. The older [Tunnel profile](../cloudflare-tunnel/README.md) is for local
preview or recovery.

## Select the operation

From this directory, the maintained package commands are:

| Command | Effect |
| --- | --- |
| `npm ci` | Install the pinned Worker dependencies |
| `npm run ci:web` | Install the web client dependencies |
| `npm run build:ci` | Install web dependencies, build web assets and generate D1 data |
| `npm run check` | Build, regenerate Wrangler types, typecheck and run Worker tests |
| `npm run load:local` | Import generated data into local D1 |
| `npm run verify:local` | Run the native verifier with an explicit owner selection |
| `npm run check:local` | Run local build/check/import and the selected native verifier |
| `npm run deploy:edge:plan` | Compare revision and show the remote deployment plan without mutation |
| `npm run load:remote` | Import the generated full read model to remote D1 |
| `npm run deploy:edge` | Run revision-aware import/deploy flow |

Node handles package scripts, HTTP transport and Wrangler process invocation.
The package does not install or run Python. `npm run build:data` invokes the
installed native `tos build-data`; set `TOS_ACCESS_BIN` when that executable is
outside `PATH`. The import helper also uses the installed native
`edge-sql-stream` and `edge-import-local` commands; it does not invoke Cargo.
Stop a local Worker before the local full bootstrap.

The build reads only the allowlisted ToS projections, registries, and review
packet declared by the source tree. Those authored and derived sources remain
with ToS; the generated read model is a disposable delivery carrier. For
source identity, transfer and admission, follow the
[source-witness owner route](../../../ToS/source-witnesses/README.md).

## Native source-text gateway

`POST /api/source-text/search` is optional. It forwards to a separately
selected native PostgreSQL source-text consumer only when the operator configures
an explicit HTTPS origin, an outbound Cloudflare Access service token, and the
inbound Access issuer and application audience. The Worker validates the
caller's Access JWT against the issuer JWKS. It does not forward the caller
token, use the outbound token as caller authentication, or read source text
from D1.

Bind the inbound audience and Access Allow policy to the principal set approved
for the native private-delivery scope. Keep the origin on a distinct TLS
hostname behind a policy that accepts the Worker service token. The capability
packet reports configuration with `runtime_readiness: not-probed`; it does not
probe origin health, choose the native consumer, or grant source rights. See the
[gateway contract](../../contracts/source-text-gateway.v1.md) for exact
bindings and request limits.

The Worker shares one 65-second deadline across JWT/JWKS validation, request
body reading, native response headers and body. Preflight time reduces the
remaining native-query time; the native 60-second cap is not added on top. This
gateway does not grant publication authority or change source admission.

## Production flow

Configure Cloudflare Workers Builds for the intended branch and this package's
directory, using `npm run build:ci` as the build command and
`npm run deploy:edge` as the deployment command. These settings live in the
Cloudflare dashboard; this repository cannot prove their current remote value.

The deploy script compares the generated `data_revision` with D1 metadata. An
unchanged revision skips the row import; a changed source, row, capability,
catalog or read-model schema imports the generated model before Worker/static
asset deployment. Use `npm run deploy:edge:plan` to inspect the planned action
without import or deployment. `npm run load:remote` is the explicit full-import
recovery route.

Local and remote publication are separate. `npm run load:local` uses local D1;
the full bootstrap uses the native one-transaction SQLite importer when a known
local store is selected. Remote imports use Wrangler and require the appropriate
credentials and publication authority. Chunked imports use a positive
`TOS_D1_SQL_STREAM_MAX_SECONDS` whole-operation limit. The selected consumer
checks the source descriptor and pathname around reads; temporary owned files
are removed after a confirmed child close. If child close cannot be observed,
the helper reports the unreaped PID and retains the exact scratch directory.

The generated model is not a native-current read model and does not grant
publication authority. A D1 migration is not applied by an HTTP request. Apply
[`0001-exploration.sql`](migrations/0001-exploration.sql) before serving the
knowledge routes that depend on its publication clock and indexes. Readers
bracket their bounded work with revision/epoch checks and fail closed on a
changed or malformed publication. The [native semantics guide](../../shared/NATIVE_SEMANTICS.md)
owns current endpoint behavior, carrier compatibility and response limits.

## Local validation and acceptance

`npm run check` validates the declared Worker build, types and bounded fixtures.
It does not import corpus data, deploy the Worker, or establish semantic
acceptance. `npm run check:local` additionally imports generated data locally
and invokes the selected native verifier. That verifier needs caller-supplied
profile, exact Worker/native paths, pinned request inputs, resource bounds and
one monotonic deadline; neither `npm run check` nor a Cloudflare workflow
invents that owner selection. The exact fixture provenance route is
[`test/fixtures/python-oracles/README.md`](test/fixtures/python-oracles/README.md).

Green local checks, CI or a successful Workers Build are separate evidence.
Public acceptance requires external checks of the apex health response, HTML
and static assets, representative corpus and philosophy queries, and the `www`
redirect. A dry run is not a deployment, and a deployed Worker does not prove
semantic acceptance.

## Resumable exploration storage

`POST /api/knowledge/explore` resumes a bounded BFS frontier in disposable D1
checkpoints. `GET /api/knowledge/explore/capabilities` reports migration and
metadata availability, not full-row validation. The contract route returns
the shared request/result schemas and target-specific limits. The additive
`0001-exploration.sql` migration installs checkpoint tables, a publication
clock and adjacency indexes; deployment applies it on full, delta and
unchanged-data paths.

Each page has at most 24 adjacency queries and 512 graph work units. The store
has a 15-minute fixed TTL, at most 128 records and 32 MiB total, and a 1 MiB
per-state/replay response cap; oversized requests refuse before checkpoint
admission. Cursors are Worker-publication specific
and cannot be transferred to local native or HTTP consumers. Expired or evicted
state requires starting from focus again. Checkpoint rows are query state, not
source history or a saved user workspace; eviction never deletes knowledge
rows. The [exploration decision](../../../docs/decisions/TOS-D-0047-shared-d1-exploration-checkpoints.md)
records the rationale.

The same publication guard protects bounded knowledge reads. It rejects an
epoch or revision change while a query runs, including a change away and back
to the same digest. This guard does not establish completeness of a damaged
index. Exact query semantics, current execution versions and response limits
are maintained in [`NATIVE_SEMANTICS.md`](../../shared/NATIVE_SEMANTICS.md).

## Native offline prepared-pair capture

The installed native access product provides
`PREFIX/bin/tos edge-offline-capture --request ABS.json`. It creates an
unapplied, bounded SQL capture from explicitly selected inputs. It does not
mutate D1, switch consumers, deploy, admit source meaning or rights, or claim
semantic acceptance. Request limits have no defaults. Inputs and outputs are
absolute paths; SQLite inputs must be regular, non-symlink files held with
identity checks through capture. This is offline custody, not a live selection
lease.

The request is limited to 10 MiB. Prepared bindings are limited to 1 MiB;
catch-up source-input JSON is limited to 1 MiB before escaping; each retained
projection root is limited to 256 KiB. Strict preflight rejects duplicate JSON
members and bounds parser depth, visits, integer digits and logical state.
These are admitted input/work bounds, not process-memory guarantees.

`operation` selects one of five native routes:

- `prepared-delta`: exact predecessor/successor prepared snapshots and D1 revision.
- `prepared-catchup`: exact predecessor D1/source-input state plus a successor snapshot.
- `source-navigation-delta`: an admitted prepared transition for selected navigation rows.
- `source-navigation-bootstrap`: a complete product only when native navigation tables are absent; requires exact node/edge roots and a separately selected rights root.
- `source-navigation-integrity`: a held D1 product compared with selected roots; full mode audits rows and payloads, while `header_only` checks header policy and inventory counts without claiming a row audit.

Transition captures reuse the maintained D1 row, knowledge, lens and metadata
producers with auxiliary-store guards, predecessor checks and reversible closure;
the [native capture owner](../../../rust/crates/tos-access/src/edge_offline_capture.rs)
defines those checks.

Every request names the expected predecessor `expected_d1_revision`, distinct fresh
paths for `forward_sql`, optional `rollback_sql`, and `manifest_json`, and the
operation's positive finite resource limits. Delta and catch-up may omit
rollback; bootstrap and integrity require it. The result uses
`tos_edge_offline_capture_result_v1`; receipts use
`tos_edge_native_prepared_delta_receipt_v1`,
`tos_edge_native_prepared_catchup_receipt_v1`,
`tos_edge_native_source_navigation_delta_receipt_v1`,
`tos_edge_native_source_navigation_bootstrap_receipt_v1`, or
`tos_edge_native_source_navigation_integrity_receipt_v1`. Outputs include
actual hashes, bytes, accounting and lineage. Admission, currentness, rights, semantic
acceptance, applied D1 state and consumer switch remain false.

Example command:

```sh
/absolute/prefix/bin/tos edge-offline-capture --request /absolute/scratch/request.json
```

An addressed prepared delta request has this shape. Binding objects must be the
complete exact values from the selected prepared snapshots; placeholders below
are explanatory and must be replaced:

```json
{
  "schema": "tos_edge_offline_capture_request_v1",
  "operation": "prepared-delta",
  "d1_database": "/absolute/scratch/selected-d1.sqlite",
  "before_prepared_database": "/absolute/scratch/before.sqlite",
  "after_prepared_database": "/absolute/scratch/after.sqlite",
  "expected_d1_revision": "<64 lowercase hex characters>",
  "before_binding": { "replace_with": "exact predecessor binding" },
  "after_binding": { "replace_with": "exact successor binding" },
  "before_source_inputs_json": null,
  "rights_root": null,
  "forward_sql": "/absolute/scratch/capture/forward.sql",
  "rollback_sql": "/absolute/scratch/capture/rollback.sql",
  "manifest_json": "/absolute/scratch/capture/manifest.json",
  "limits": {
    "prepared": {
      "max_changes": 10000,
      "max_row_bytes": 2097152,
      "max_metadata_bytes": 2097152,
      "max_read_bytes": 67108864,
      "max_rows": 10000,
      "max_retained_bytes": 33554432,
      "max_sql_bytes": 67108864,
      "max_postings": 100000,
      "max_manifest_rows": 10000
    }
  }
}
```

Output parents must remain exclusively owned through capture. A failed or
incomplete capture is not publication evidence; the caller separately admits
runtime memory and storage. Remote publication is a separate authorized step.

## Web Analytics

`wrangler.jsonc` declares the public domains and `WEB_ANALYTICS_TOKEN` binding.
The Worker inserts the configured beacon into successful production HTML only;
local/off-domain previews do not. HTML is private/no-store because beacon
presence varies by geography, while static assets retain their declared cache
policy. Verify beacon load and delivery against the deployed site after a
production change; repository configuration alone is not live analytics proof.
