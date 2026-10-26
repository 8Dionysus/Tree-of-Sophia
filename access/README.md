# Tree of Sophia Access

`access/` is ToS's standalone, source-read-only delivery product: native CLI,
HTTP and MCP, the browser application, and WebMCP. Rust owns runtime query and
transport behavior. The optional imported Python SDK is a thin caller of the
installed native product; it does not ship a second query engine or a Python
runtime fallback.

Authored meaning, source admission, rights, review and canon remain with their
ToS owners. Access reads explicitly selected published data and returns
`source_ref` routes. It does not write corpus state or turn a projection,
capability packet, or successful validator into semantic acceptance.

## Choose a route

| Need | Route |
| --- | --- |
| Install or select native software | [Native fresh-prefix installation](#native-fresh-prefix-installation) and [release guidance](../docs/RELEASING.md) |
| Run local CLI, HTTP or MCP | [Native runtime](#native-runtime) and the [Rust access guide](../rust/crates/tos-access/README.md) |
| Select an offline prepared knowledge reader | [Explicit prepared local reader](#explicit-prepared-local-reader) |
| Understand read packets and compatibility | [Contracts](contracts/README.md) and the [native semantics guide](shared/NATIVE_SEMANTICS.md) |
| Use the browser and WebMCP | [Web client](web/README.md) |
| Build or operate the public Cloudflare route | [Worker deployment guide](deploy/cloudflare-worker/README.md) |
| Operate private source navigation or restore captures | [source-owner operations](#source-owner-operations) and [ToS source witnesses](../ToS/source-witnesses/README.md) |
| Validate this product | Root [validation routes](../VALIDATION.md); the `standalone_access` lane owns command order |

## Native runtime

Install and select software separately from data. For an installed command:

```sh
tos --help
tos --root /absolute/selected-data doctor
tos --root /absolute/selected-data verify --profile standalone
tos --root /absolute/selected-data serve 127.0.0.1:8765
tos --root /absolute/selected-data mcp
```

HTTP binds to loopback by default. MCP uses stdio unless a loopback-only
Streamable HTTP transport is selected explicitly. `--root` or `TOS_DATA_ROOT`
selects data; neither changes the installed software, browser assets, or API
schemas. No command searches a parent directory or neighboring checkout for
another dataset. `--release-root` selects an already admitted managed release;
it does not install software or build data.

`doctor` and `verify` inspect the selected source-backed profile. They do not
compile data, start a server, grant source rights, or prove semantic acceptance.
Prepared-reader flags belong to the explicit prepared route below and are
refused by source-backed diagnostics. The installed product is the native
software archive and its `bin/tos` entrypoint; there is no maintained Python
wheel or CLI.

### Native fresh-prefix installation

The native software archive installs without Python. Begin with an archive and
installer admitted through the release route, and pass finite positive budgets:

```sh
/path/to/admitted/tos-access software install --archive /absolute/software.zip \
  --prefix /absolute/fresh-user-prefix \
  --max-total-bytes ADMITTED_EXPANDED_BYTES --max-archive-bytes ADMITTED_ZIP_BYTES \
  --max-members ADMITTED_MEMBER_COUNT --max-metadata-bytes ADMITTED_METADATA_BYTES
/absolute/fresh-user-prefix/bin/tos --help
/absolute/fresh-user-prefix/bin/tos --version
```

The installer refuses an existing prefix and never starts the installed
program. A failed extraction may leave an incomplete candidate; do not select
it on `PATH`. Switching versions is an explicit choice of entrypoint, so a
retained prior prefix remains available for rollback. Installation does not
modify data releases, shell configuration, services, or publication state.
Managed data is selected separately with `--release-root`.

### Native software and managed releases

Use `tos-access software build`, `software verify`, `software extract`, and
`software install` for the code-only archive. The archive contains the native
entrypoint and its software-owned browser assets, not corpus data. Build inputs,
budgets, archive verification and official release steps are in
[`docs/RELEASING.md`](../docs/RELEASING.md).

Managed release preparation, status, promotion, rollback and revocation use
the native `native-release-*` commands and the
[`native-managed-release.v1.md`](contracts/native-managed-release.v1.md)
contract. A release binds an exact native software archive to one data
snapshot. Readers recheck the selected pair; rollback cannot revive a revoked
component. Data snapshot production and admission remain with their explicit
source/data owners.

## Read products and compatibility

Access exposes bounded projection search, knowledge catalog/search/inspection,
stored and compiled lenses, neighborhood exploration, source navigation,
and selected local reading operations. Their transport-neutral operations are
listed in [`query-operations.v1.json`](contracts/query-operations.v1.json),
[`knowledge-api.v1.json`](contracts/knowledge-api.v1.json), and the schemas
linked from the [contract index](contracts/README.md).

Selection is explicit. Source-backed readers use `--root` or an admitted
`--release-root`; an offline prepared knowledge reader requires both a prepared
SQLite path and an owner-selected binding. A reader never creates a missing
product, infers a binding from database contents, or falls back to a different
dataset after a selected product fails. Prepared knowledge is a bounded
projection, not source, rights, review, or publication authority.

Projection search retains its declared engine-specific behavior: legacy and
indexed modes keep their own pagination and filters, while compressed search is
available only for an explicitly selected prepared profile. The native
consumer does not translate offsets into cursors or silently select another
engine. See the [compressed-search contract](COMPRESSED_SEARCH_V3.md) and
[native semantics](shared/NATIVE_SEMANTICS.md) for engine compatibility and
work limits.

Source navigation provides `tos.source.descend` and `tos.dossier.inspect` on
the native source-navigation product (or the explicitly supported legacy
carrier). A dossier can include the connected bibliographic chain, observed
links, scoped rights records, gaps and source references. A downloadable URL
is technical access only; it does not establish legal openness. A prepared
reader without its declared navigation and rights companions is unavailable
for dossiers rather than permission to consult another index.

The native adapters expose:

- `tos.source.descend` (`GET /api/source/navigation/{node_id}`) walks from a
  selected navigation node toward bibliographic objects and links.
- `tos.dossier.inspect` (`GET /api/source/dossiers/{object_id}`) returns a
  bounded dossier with connected records, observed links, scoped rights,
  source refs and gaps.

### Local reading and the imported SDK

Zarathustra concept and word-analysis tools use the selected native installation
and local source root:

```sh
tos --root /absolute/source-root concept-search --query 'судьбы' --language ru --limit 20
tos --root /absolute/source-root word-analysis --query 'судьбы' --language ru --rank 1
tos --root /absolute/source-root word-analysis --validate-candidate /absolute/candidate.json
```

Candidate validation is read-only. The native `tos_zarathustra_prepare_word_analysis`
tool returns a candidate-only task; a selected provider refusal is not replaced
by a Python implementation. Reading/search capability availability describes
the selected local data, not source admission or publication.

Imported Python callers may frame the same installed native operations:

```python
from tos_access import NativeAccessCore

core = NativeAccessCore.discover(
    native_prefix="/absolute/installed/software",
    release_root="/absolute/owner-selected/release",
)
packet = core.knowledge_node("an-exact-owned-node-id")
```

The bridge retains the native packet and typed refusal; it has no independent
query/server implementation or fallback. The [Rust access guide](../rust/crates/tos-access/README.md)
owns the installed runtime and validation route.

`tos.source-text.search` requires an explicitly selected durable PostgreSQL
consumer. The optional Worker gateway forwards it only when its native HTTPS
origin and both outbound/inbound Cloudflare Access credentials are configured.
D1 is not a source-text reader. See the
[source-text gateway contract](contracts/source-text-gateway.v1.md).

### Backend-defined knowledge construction

Knowledge packets preserve stable ToS IDs, source-native identity, provenance,
source references, authority posture, and the recorded wording available in
the selected projection. Deterministic display synthesis is labeled as
synthesis; it is not authored ToS meaning. Missing translations are not
invented, and identifier fallbacks do not become source titles. A normalized
packet or successful transport check cannot assess truth, translation, rights,
review, or canon.

For the exact normalized packet shape, vocabulary, and language fallback rules,
use [`knowledge-graph.v1.schema.json`](contracts/knowledge-graph.v1.schema.json),
the [contract index](contracts/README.md), and the ToS-owned semantic registry
under [`ToS/doctrine/semantic-interchange/`](../ToS/doctrine/semantic-interchange/).
The browser's full-material context reader verifies the declared digest and
field coverage before presentation; this proves transport binding, not the
quality of the owner's assessment. See the
[human-form delivery contract](contracts/human-form-delivery.md) for the
separate rendering boundary.

### Resumable neighborhood exploration

Exploration is a bounded query over the selected snapshot. By default its
checkpoint is process-local. A caller may explicitly select a separate private
checkpoint store for restart survival; the store is not a source database and
does not grant source or publication authority. Replays recheck the selected
binding and execution configuration. Incompatible or corrupt checkpoint state
fails closed without automatic reset or migration.

For the local prepared reader, provide the checkpoint path with
`--exploration-checkpoints /absolute/private/continuations.sqlite`; the parent
directory must already exist. Native limits, storage bounds, and transport
fences are maintained by the Rust access owner and summarized in its
[runtime guide](../rust/crates/tos-access/README.md). The selected capability
reports configured availability; it does not prove real-corpus performance or
deployment.

## Explicit prepared local reader

Use this route only when an owner has already prepared and selected a local
knowledge publication with its binding. A minimal catalog query is:

```sh
tos --root /absolute/runtime-data \
  --prepared-read-model /absolute/selected.sqlite \
  --prepared-binding /absolute/owner-selected-binding.json \
  knowledge catalog
```

The same paired selectors apply to CLI, HTTP and MCP. For compressed search,
use the explicit `knowledge search QUERY --mode compressed` operation; see
[`COMPRESSED_SEARCH_V3.md`](COMPRESSED_SEARCH_V3.md#local-adapters) for its
cursor, capability and bounded-work rules. A prepared selection cannot be
combined with a managed release. It does not acquire managed-release custody
or source-rights authority.

The selector pins an immutable publication and checks its binding at each
disclosure boundary. Health reports only the selected header, binding and
required indices; it does not scan every source row. `doctor` and `verify`
remain source-backed diagnostics and do not claim prepared-reader health.
Native and managed consumers retain their distinct release and request guards.

## Source-owner operations

### Selected PostgreSQL source navigation

This is a separate, private source-owner product. Use the protected
`tos_local_source_navigation_owner_v1` selection and keep
`TOS_SOURCE_PG_URL` in the private process environment. The owner selection
binds the selected CMD domain, source proof, consumer/profile, finite model and
event limits, and policy receipt. Neither a prepared model nor its receipt
grants source rights, QRY disclosure, publication admission or canon.
The current PostgreSQL anchor is `tos_knowledge_managed_navigation_v2`; it
retains authenticated Navigation Original carriers and source proof, while
PostgreSQL owns the published graph. It does not contain offline compressed
search tables, so that search mode is unavailable on this anchor. Changing
producer profile requires an explicit derived rebuild; source versions, rights
and command history remain with their existing owners.

On a new installation or schema upgrade, initialize the derived tables first
with the same native `source-projection` entry and a protected
`tos_source_projection_selection_v1` selection for that database:

```sh
/absolute/prefix/bin/tos-native-owner-command source-projection \
  --selection /absolute/protected/source-navigation-owner.json \
  < /absolute/requests/init-navigation.json \
  > /absolute/receipts/navigation-init.json
```

The init request is `{"operation":"init"}`. One-time model preparation is a
separate local-owner operation. Its protected
`tos_local_source_navigation_model_prepare_owner_v1` fixes the selected model,
producer, policy template and finite resource bounds. Invoke it through the
native source-command entry:

```sh
/absolute/prefix/bin/tos-native-owner-command source-commands \
  --invocation /absolute/protected/navigation-model-prepare-invocation.json \
  < /absolute/requests/prepare-initial.json \
  > /absolute/receipts/navigation-model-prepare.json
```

The request is
`{"schema_version":"tos_local_source_navigation_model_prepare_request_v1","operation":"prepare-initial"}`.
The protected invocation selects the installed native images, finite export and
model budgets, private stage and delegated process limits; reserve model, stage
and temporary storage through the host owner. The parent retains its PostgreSQL
connection, while only the selected compiler child enters the private stage.
Do not target another owner's live database or reuse a failed partial output.
Only after preparation, bootstrap the complete selected current model before
advancing events:

```sh
/absolute/prefix/bin/tos-native-owner-command source-projection \
  --selection /absolute/protected/source-navigation-owner.json \
  < /absolute/requests/bootstrap-navigation.json \
  > /absolute/receipts/navigation-bootstrap.json
```

The bootstrap request is `{"operation":"bootstrap-navigation"}` with
optional `max_batches` from 1 to 256 (default 256). Repeat the same request
until both `completed` and `cleanup_complete` are true. It publishes a private
derived projection only; then advance bounded events with
`{"operation":"advance-navigation","max_events":1}`. Unsupported events
remain pending. Retain the returned receipts and their selected source/revision
bindings. If a response is lost after publication, repeating the exact
bootstrap request is safe.

Policy install and revoke use their own protected invocation and owner
configuration. They are separate local CLI operations; HTTP, MCP and
`private-text-read` do not expose policy mutation. Revoke withdraws only this
navigation policy; it does not change CMD rights. Do not treat a model, policy
template or bootstrap receipt as a grant.

The reader verifies the selected publication and resolves each disclosed
node/edge against the current source-owner metadata and rights. Changed
dependencies or a publication that is behind the requested sequence produce
`publication_pending`; the client may retry after source processing advances.
Use `source descend NODE_ID --at-least-commit-seq SEQUENCE` (HTTP query field
`at_least_commit_seq`, or the matching MCP argument) with the receipt's
canonical unsigned 64-bit decimal string. The D1 export has no CMD sequence
and refuses this parameter. Neither the check nor retry forces processing or
grants rights. Complete model preparation, source review, rights and reader
selection remain separate owner routes.

### Restoring a selected source capture

`tos capture-restore` restores an existing `tos_corpus_capture_v1` or `v2`
capture into a new directory. Obtain the expected commit, tree and manifest
digest from the retained owner record, not from the archive being restored:

```sh
tos capture-restore --capture /absolute/capture --output /absolute/new-root \
  --source-commit EXPECTED_COMMIT --source-tree EXPECTED_TREE \
  --manifest-sha256 EXPECTED_MANIFEST_SHA256 \
  --max-archive-bytes COMPRESSED_CAP --max-decoded-bytes TAR_STREAM_CAP \
  --max-source-bytes FILE_BYTES_CAP --max-metadata-bytes INDEX_CAP \
  --max-members MEMBER_CAP --max-seconds WHOLE_DEADLINE
```

Every cap is a positive integer. The decoded cap includes tar headers,
extensions and padding. Keep capture and destination under exclusive owner
control and reserve disk allocation separately. Restore verifies archive and
member digests, Git blob identities, paths and modes. It refuses an existing
output directory; failure may leave partial output for caller-owned cleanup.
Restore does not admit sources, select data for readers, change rights, or
publish a dataset. Production, verification and corpus import belong to the
[source-witness owner route](../ToS/source-witnesses/README.md).

Offline edge capture is also explicit and emits unapplied local artifacts:
`tos edge-offline-capture --request /absolute/request.json`. The Worker guide
owns its operation-specific request schema, finite limits, custody requirements
and outputs; capture does not import to D1, switch consumers, or publish
remotely. See [native offline prepared-pair capture](deploy/cloudflare-worker/README.md#native-offline-prepared-pair-capture).

#### Supervised Linux capture

The outer envelope uses `schema: "tos_edge_selected_capture_request_v1"`
and embeds the original ordered `tos_edge_offline_capture_request_v1` request
under `capture`; use the [Worker capture example](deploy/cloudflare-worker/README.md#native-offline-prepared-pair-capture)
for that inner object. Its other required fields are `selected_prefix`,
`scratch`, `transport_limits`, `installed_limits`, `work_timeout_ms`,
`cleanup_timeout_ms`, `supervisor_unit` and `supervisor_total_ms`.
`transport_limits` contains `frame_bytes`, `schema_bytes`, `request_state_bytes`,
`result_state_bytes`, `stream_bytes`, `metadata_bytes`, `state_bytes`, `io_bytes`,
`held_fds` and `census_entries`; `installed_limits` contains `max_image_bytes`,
`max_metadata_bytes`, `max_state_bytes`, `max_io_bytes` and `max_held_fds`.
The [selected-entry owner](../rust/crates/tos-access/src/edge_offline_capture/selected_entry.rs)
defines exact parsing, custody and accounting.

The `--selected-request` entry is only for a supervised Linux host. Its
request explicitly binds a selected prefix, scratch path, positive
`transport_limits` and `installed_limits`, work/cleanup clocks, and one finite
user service. The command verifies that service's actual invocation and
process/cgroup custody. Prefix and scratch are existing absolute paths; limits
have no defaults. Work is at most 1,200,000 ms, cleanup at most 10,000 ms, and
the outer service at most 1,210,000 ms from the original entry clock; neither
deadline is renewed. Run it under the selected
outer service, with `RuntimeMaxSec` no greater than the request's supervisor
total and `TimeoutStopSec` no greater than its cleanup interval:

```sh
systemd-run --user --wait --collect --pipe --unit UNIT.service \
  -p MemoryMax=MEMORY_BYTES -p MemorySwapMax=0 \
  -p RuntimeMaxSec=ORIGINAL_TOTAL_SECONDS -p TimeoutStopSec=ORIGINAL_CLEANUP_SECONDS \
  -p KillMode=control-group -- \
  /absolute/prefix/bin/tos edge-offline-capture --selected-request /absolute/request.json
```

If native child release is not proved within the original cleanup clock, the
CLI exits 125 without success or adoption and retains frames and failure
evidence for owner-directed cleanup. The selected outer service owns residual
cleanup; process death closes this CLI's owned read transactions. A library
caller retains its separate borrowed-custody contract. This is not a public
publication or authorization broker.

## Browser application and WebMCP

The web client is the human presentation of the same selected, bounded read
products. It supports source-linked search, evidence and route inspection,
bounded lens construction, and a browser-local research workspace. Workspace
hypotheses and proposals remain local, visibly unreviewed, and outside ToS
source, review ledger, candidate intake and canon. Export is the handoff.
Feature-specific behavior and development commands are in the
[web guide](web/README.md).

In supported browsers, WebMCP exposes page view/search/selection actions and
the currently available research tools to an agent on the same page. The site
does not embed a model or require an API key. It remains usable when
`document.modelContext` is absent; WebMCP support is experimental and does not
confer ToS authority or guarantee availability. The optional native `tos mcp`
server is an off-page route, not a prerequisite for browser use.

## Cloudflare delivery

The repository's public deployment route is the
[Cloudflare Worker package](deploy/cloudflare-worker/README.md). Static assets
serve the web client, while generated read-only packets and D1 provide bounded
projection access. Offline build/import steps and the production publication
boundary live with that route. The older
[Cloudflare Tunnel profile](deploy/cloudflare-tunnel/README.md) is for local
preview or recovery, not the intended availability architecture.

## Development

Use the root [`standalone_access` validation route](../VALIDATION.md) after a
specific contract or adapter changes; the repository lane manifest owns exact
command order. Browser assets can be built locally with:

```sh
npm ci --prefix access/web
npm run build --prefix access/web
```

Those commands build frontend assets only. They do not compile or admit corpus
data, create a Cloudflare publication, install software, or establish runtime
acceptance. Rust build profiles and the native validation entrypoint are
documented in the [Rust access guide](../rust/crates/tos-access/README.md).
