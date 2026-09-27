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
helper selects no production policy; the native binary keeps its existing
1 MiB packet and frame caps and remains without a public owner binding.

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
