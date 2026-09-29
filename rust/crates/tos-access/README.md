# Native access response budgets

`AccessProfile::max_response_bytes` limits the raw query packet shared by CLI,
HTTP and MCP. MCP tool results contain that packet as both escaped JSON text
and raw `structuredContent`; the JSON-RPC envelope, encoded request ID and
newline add transport bytes. Raw packet admission therefore does not imply
MCP frame admission.

`AccessProfile::new(request, response, line)` preserves the existing default:
its complete outgoing MCP frame cap equals the raw response cap. A legal packet whose
complete frame exceeds that cap receives JSON-RPC error `-32603`,
`MCP response frame exceeds byte budget`, without packet disclosure. Exact
escaped frame length is checked before constructing the duplicated carrier.
CLI and HTTP packet limits are unchanged.

A caller can explicitly select a separate frame allowance with
`with_mcp_frame_budget`. `mcp::tool_result_frame_byte_bound` derives a
conservative finite allowance from declared packet and request byte caps,
including worst-case JSON escaping of packet text and canonical request ID,
the structured copy, fixed envelope and newline. It returns `None` on arithmetic
overflow. Pass the smaller of request and line caps for the request bound. This
helper selects no production policy. The explicitly selected managed-local
binary profile derives a separate complete frame allowance from its existing
1 MiB packet and 64 KiB request caps; library defaults keep equal caps.

The maintained selected-wire fixture explicitly uses this derived allowance
so every packet within its declared packet cap can traverse MCP. Its parser
uses the same frame cap, and its assertions retain exact CLI, HTTP, MCP text
and structured packet bytes plus the selected lease through final flush.
The existing framing refusal case continues to check the default equal caps.
Cancellation and current-authority disclosure fences apply independently of
these byte allowances.

The selected-family wire case also reuses the maintained native producer
fixture for temporal, lens compile, focus, stored lens and exploration replay.
It derives route selectors from the native descriptor and stored LensSpec from
the digest-bound selected catalog. The fixture authority is still a test-only
current-binding model, never an issuer or public grant. Native HTTP
`write_response` is shared by the socket path and supplied writers; its final
fence precedes bytes and its hold lasts through flush. Selected family cases
observe that hold and refuse withdrawal/cancellation before disclosure. Public
session issuance and Worker Response/body delivery require their own owners.

The maintained caller compatibility seam accepts `--option=value`, preserves
last scalar and `--sources` occurrence semantics, and retains appended
predicates. `serve --host/--port` (including equals spelling and IPv6 host
formatting) uses the existing loopback listener; no owner binding is inferred.
HTTP POST media rejection uses 415 `unsupported_media_type`; an
unknown POST route is selected from the descriptor and refused before body
parsing. GET/HEAD share the first repeated scalar/default/clamp rules, and
HEAD suppresses successful and error bodies while keeping their Content-Length.
Every outgoing MCP frame, including metadata and errors, is checked with ID
and newline before output. If even the correlated refusal cannot fit, stdio
returns an error without writing an oversized frame. Initialize is accepted
only after its response is flushed. Tool advertisement includes only actual
executor-ready operations and their exact descriptor schemas.

These caller checks do not close the full maintained 60-parser/50-HTTP/50-MCP
inventory. Global prepared/source selection, doctor/verify/profile/site
assembly, remaining domain engines and authentic owner binding remain
requirements of the existing API2 inventory. The native binary is an additive
closed candidate, not a replacement claiming these missing functions. Its
bounded compact JSON and typed native error/exit profile remain explicit:
0 successful flush, 1 query/disclosure/output failure, 2 request/file/usage
failure, 3 unavailable selected capability. Maintained Python pretty JSON and
FastMCP serialization are not silently substituted for this declared profile.

Maintained knowledge search now defaults to the selected legacy v1 engine,
including the optional empty query, offset, filters and explicit legacy mode.
Indexed mode retains its own cursor validation and QRY engine; compressed mode
refuses without an explicitly prepared publication. The transport does not
translate offsets into cursors or choose a fallback engine. The selected search
capabilities packet is available through CLI, GET/HEAD and MCP under its own
held operation scope. Its engine-selection-only readiness does not issue a
public grant. CLI arguments are bounded before option expansion; structured
files, HTTP targets, MCP input and output retain their separate declared caps.
Legacy ranking, complete counts, normalization and substring semantics remain
in QRY, with its actual Python differential evidence; the consumer case checks
complete selected bytes and final-flush custody through the existing wires.

Selected knowledge contracts has an explicit native composition helper taking
both borrowed original registry carriers. QRY validates their selected identity
and invokes the existing authority's default-deny registry callback; its one
disclosure hold must cover both grants through final transport flush. The
actual fixture executor supplies compiler-retained originals for CLI, GET/HEAD
and MCP checks. Production without that owner carrier holder still refuses:
the generic dispatcher and unselected NoOwner binary do not look up ambient ToS files,
create registry grants or advertise contracts readiness.

The explicit managed-local native composition follows
[the versioned consumer contract](../../../access/contracts/native-managed-release.v1.md).
`--release-root` or `TOS_RELEASE_ROOT` selects the existing local ReleaseStore
holder and an independently persisted producer companion. Missing shared lock,
current selection, kernel fs-verity custody or actual process enforcement refuses
admission. The factory retains the admitted model and exact registry carriers;
its real release lease lasts through final flush. Dossier uses the original
component only for existing projection fields; exact source read/text remains
a separate owner service. This source candidate does not activate a release.

Selected philosophy delivery uses the producer-retained ABI4 original component
for the eleven maintained GET/HEAD and MCP reads. HTTP query aliases and MCP
options construct the shared typed QRY request; no traversal, view selection or
packet projection is duplicated in transport. Original header/node/edge grants
share one final-flush hold even when no normalized graph carrier is consulted.
The existing producer fixture and QRY differential own domain parity; the native
wire case checks thirteen complete packets, HEAD lengths and final withdrawal,
cancellation and deadline refusal. These source checks do not establish installed
managed execution while the named host fs-verity prerequisite remains unavailable.

The maintained exploration-contracts GET/HEAD and MCP tool disclose the four
packaged v1/v2 request/result schemas independently of source selection. Data
roots and request parameters cannot override those compiled software bytes.
The maintained `/api/knowledge/explore/capabilities` GET/HEAD returns the same
current capability value without embedding the four schemas.
Unselected `NoOwner` reports exploration unavailable with zero configured
checkpoint/work/session limits. The managed selected executor reports its real
process checkpoint and QRY budgets only while its current release is available;
it never claims restart survival. This capability snapshot issues no source
grant and does not close a selected exploration execution or public activation.

Corpus reads reuse the selected ABI5 original component and the real managed release's declared captured index/member closure. The maintained six HTTP GET/HEAD routes and eight MCP tools share the QRY kernel, with resources/packet MCP-only. Availability requires the original component plus verified raw source members under the release holder; status paths identify those actual selected members. No corpus CLI or raw source/payload grant is added.

`tos-access --root SOURCE_DIRECTORY doctor [--json]` and `verify
[--profile standalone|abyssos] [--json]` inspect the source-backed profile before
managed-owner admission. `TOS_DATA_ROOT` and the maintained projection path
selectors choose data; prepared reader/binding or a managed release selector is
refused for this diagnostic. Default missing data remains beside the installed
software's `runtime_data`; no checkout is discovered through the working
directory. Commands never compile data, launch servers, import Python or
activate integration. A failed required check prints the report and exits 1;
invalid options/prepared selection exit 2. Text rendering preserves the
maintained readiness/check/failure lines; JSON preserves the report schema and
fields using the existing native compact JSON profile plus LF.

Source JSON is limited to 4 MiB per regular retained file, depth 64 and 200,000
JSON visits, with file identity checked across each read. The shared QRY
View-only diagnostic materializes the first maintained philosophy view with
100,000 total base/inline rows, 1,000,000 logical work steps and a 1 MiB packet
bound; it parses that finite raw document again and grants no selected/current
policy authority. Missing/invalid/oversized sources are failed checks. A legacy
configured/present query store or partitioned source requires an explicit
failed `query-store` check: this native diagnostic does not claim support for
the Python compiled-store backend or rebuild it. The report is a mechanics
snapshot, not full standalone cutover or source/rights admission.

Runtime contracts are compiled software bytes. Web assets must be readable,
nonempty `web_dist/assets/tos-graph.js` companions in the native executable's
own directory, following the existing installed software web_dist layout;
selected data cannot replace them. Current native build products without that
assembled companion correctly fail `web-assets`. Native MCP is built-in Rust
stdio, so its dependency check validates the packaged operation descriptor
instead of importing FastMCP. `verify` requires that check; `doctor` retains
its optional posture. AbyssOS configuration reports only the selected
`TOS_ABYSSOS_ROOT/abyss-stack` directory, and the packaged paused integration
posture still blocks the abyssos profile. No directory/configuration check
proves running integration or authorizes its activation.
