# Explicit local source-command transport

`../scripts/source_command_http.py` exposes the source-command front door on
**127.0.0.1 only**. This separately started source-owner service dispatches
commands under existing delegations and their exact target, identity and
assessment requirements. Read-only access HTTP `/api`, WebMCP and native
access MCP keep their existing contracts.

The local operator selects one existing protected owner configuration, one
private credential file, one exact browser origin and one unused port:

```sh
python mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_http.py \
  --owner-config /absolute/protected/owner.json \
  --token-file /absolute/protected/transport-token \
  --browser-origin http://127.0.0.1:44257 --port 44259
```

Use the host's resource launcher where required. Starting this process is
separate from installing, activating or restarting any existing application.
The credential file contains 64 lowercase hexadecimal characters from 32
cryptographically random bytes, optionally followed by one newline. It must
belong to the process account and have no group/other permission bits (0600).
Do not put the token in URLs, logs, repository files or browser persistence.
The caller must obtain it through its owner's private local channel.

## Shared wire grammar

Both humans and agents use these endpoints with a per-request HMAC-SHA256
authorization proof. The private token itself is **never sent**:

- `GET /commands/catalog` returns the unchanged `discover_commands()` grammar.
  Implemented operations are not currently granted operations.
- `POST /commands`, `Content-Type: application/json`, takes the exact existing
  source command object, without a wrapper or a client-selected owner path.
  Its successful response is the unchanged owner result or receipt.

For example, the configured handler's context is requested with
`{"schema_version":"tos_local_source_command_v1","operation":"describe"}`.
Choose subsequent preparation and mutation shapes from the handler-owned
[discovery contract](SOURCE_COMMAND_DISCOVERY.md), and retain the exact
prepared expected versions and original command ID. The transport never
replaces a stale expectation, synthesizes a grant or automatically retries.
The configuration is reread and its delegation rechecked by the existing
handler for every command, including replay. Token authentication is only
transport access: possessing it cannot enlarge the selected delegation.

The wire header is `Authorization: ToS-HMAC-SHA256 T:N:D:S`, where `T` is the
decimal Unix timestamp in seconds, `N` a fresh random 32-byte lowercase hex
nonce, `D` the SHA-256 of the exact request body (empty for GET), and `S` the
HMAC-SHA256 using the decoded 32-byte token over the compact JSON array
`["tos-request-v1",method,path,T,N,D]`. All digest/proof values are lowercase
hex. Requests must be within 30 seconds of the server clock. The server keeps
at most 1024 still-valid nonces, rejects reuse and fails closed at capacity.
An explicit owner-command replay uses a fresh transport nonce but the original
command ID and body; transport freshness is not source idempotency.

Every authenticated response includes `X-ToS-Response-Signature`: the HMAC of
the compact JSON array `["tos-response-v1",N,status,SHA256(body)]`, with numeric
HTTP status and the exact response bytes. The browser verifies this proof before
trusting a result or error envelope. A port impostor cannot learn the secret,
alter a request or forge a receipt. Missing, stale or invalid authentication may
produce an unsigned denial, which the client treats as unconfirmed delivery.
This protocol provides authenticity, **not encryption**: request contents remain
visible to a hostile local endpoint. Use it only for the intended local public
metadata workflow; confidential command payloads require an independently
authenticated encrypted owner transport. A hostile process with the same UID
can read the credential file and is outside this local-account boundary.

The browser client at
`access/web/constructor/source-command-client.mjs` uses a separately supplied
owner origin and an in-memory credential. It never writes through the read-only
access API, follows redirects, sends ambient cookies, saves the credential or
queues concurrent commands. A 30-second client transport deadline and explicit
close abort network waiting, never source execution or an already committed
change; the original command remains available for reconciliation.
`source-form-session.mjs` connects one bounded
source-copy workflow: describe, choose an owner-declared field/form, prepare,
inspect and apply. The owner preparation includes `prepared_materialization`
from the same reader used after apply, without publishing the in-memory
successor. The panel refuses apply when that readable exact-form preview is
unavailable; existing materializations never substitute for it. It retains the
exact command ID and expectations for explicit replay after uncertain delivery.
This is the source-copy form route only, not all human growth, interpretation,
assessment or profile-evolution operations. Real browser consumption and the
remaining human growth routes have their own acceptance checks.

## Boundaries and failure

Every request requires the exact numeric loopback Host and, if an Origin is
present, the single selected browser origin. CORS preflight allows only the
two routes and their corresponding methods/headers; it does not authenticate
or execute a command. All actual reads and writes require the private token.
Missing Origin is permitted for an explicitly authenticated non-browser agent.
The token is reread each time, so replacement, removal or unsafe permissions
revoke transport access. Browser same-origin compromise remains outside this
local-account boundary. Only the numeric `127.0.0.1` owner endpoint is accepted
by the client; the separately configured browser origin may use `localhost`.

Command bodies are capped at 1 MiB and responses at 4 MiB. Duplicate JSON keys,
ambiguous Content-Length, transfer encoding, non-JSON bodies and route/query
selectors are rejected before dispatch. The server handles one command at a
time, with a four-connection listen backlog, five-second idle socket timeout and
one 30-second absolute deadline over the request line, headers and body. Trickle
traffic cannot restart this total read deadline. This is **not a command execution deadline**: the selected owner operation keeps
its own work, locking, version, transaction and recovery bounds.

Errors are generic and do not reveal source prose, token or owner paths.
`outcome: not-dispatched` means rejection occurred before calling the owner.
Once dispatched, failure or missing/oversized delivery is `unconfirmed`, not
proof of rollback. An owner commit may precede a disconnected browser. Retain
the original request and reconcile through its existing idempotency/history or
recovery operation; do not generate another command ID as an automatic retry.
Reader publication, semantic acceptance, CI and deployment each require
verification of their own resulting state.

## Verification

`mechanics/growth-cycle/tests/test_source_command_http.py` exercises real HTTP
describe/apply/replay/revocation against an isolated copy of owner metadata and
forms. It also covers token rotation, permissions, cross-origin and rebinding
protection, duplicate headers, framing/budgets and uncertain delivery. The copied record supplies a controlled mechanics fixture under an isolated
test delegation. Browser client tests live beside the client module and
cover request/response proofs, impostor responses, nonce binding, deadlines and
explicit close. UI close/unload protection must cover in-flight and uncertain
commands, not just successful replies.
