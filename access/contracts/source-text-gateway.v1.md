# Source-text gateway v1

The Cloudflare Worker can carry `tos.source-text.search` requests to the
existing native HTTP owner. The native process still selects one durable
PostgreSQL consumer at startup and owns its currentness, candidate completeness,
rights/disclosure fence and response. The Worker has no source-text table in
D1 and does not choose a source, profile, database, or consumer.

To select the transport, configure all five Worker environment bindings:

- `TOS_SOURCE_TEXT_GATEWAY_URL`: an HTTPS origin only, such as
  an operator-provisioned native source hostname; no path, credentials, query, fragment,
  or non-default port is accepted;
- `TOS_SOURCE_TEXT_GATEWAY_ACCESS_CLIENT_ID`;
- `TOS_SOURCE_TEXT_GATEWAY_ACCESS_CLIENT_SECRET`;
- `TOS_SOURCE_TEXT_GATEWAY_CALLER_ACCESS_ISSUER`: the exact HTTPS Cloudflare
  Access team issuer, for example `https://team.cloudflareaccess.com`;
- `TOS_SOURCE_TEXT_GATEWAY_CALLER_ACCESS_AUDIENCE`: the exact audience of the
  Access application that protects this caller route.

The two outbound Access service-token bindings are required whenever a gateway
origin is configured. Store the secret as a Worker secret. There is no default
origin.
The Worker always sends `POST /api/source-text/search` to that fixed path and
does not forward caller cookies, `Authorization`, the inbound JWT, or
destination information. Before configuring the inbound issuer and audience,
the source owner must map the Access application's audience and its Allow
policy to the principal set admitted for the selected native private
source-text delivery scope. Keep one native PostgreSQL consumer selected for
that scope. The audience is an application identifier; its Access policy
defines admitted callers. This mapping does not grant source rights, consent,
canon, or public publication.
The configured native origin must be separately protected by Cloudflare Access
using that service-token pair and must bind its own PostgreSQL selection through
`TOS_SOURCE_TEXT_SEARCH_SELECTION` and `TOS_SOURCE_PG_URL`.

For each request, the Worker requires the `Cf-Access-Jwt-Assertion` header and
verifies its RS256 signature with WebCrypto against the fixed issuer's
`/cdn-cgi/access/certs` JWKS endpoint. It requires an exact `iss`, configured
`aud` membership, `type: app`, an unexpired `exp`, a sane `iat`, and a valid
`nbf` when present. It does not follow JWKS redirects or use token-supplied key
URLs; the JWKS response is limited to 16 KiB and eight RSA signing keys. Missing
or invalid caller tokens are refused before the native request. A JWKS
availability or integrity failure is also fail-closed. The request-local JWKS
lookup uses the same deadline as body reading and both network exchanges; keys
are not cached across requests. The incoming assertion and the separate
outbound service-token credentials are never substituted for one another.

Request and response bodies are streamed under the shared operation ceilings:
64 KiB for the request and 1 MiB for the native page. Only JSON is accepted.
The Worker refuses redirects, non-JSON, malformed JSON, and oversized native
responses. For a valid native response it preserves the body bytes and HTTP
status, applies `no-store` and the standard Worker security headers, and
uses one 65-second deadline from caller token/JWKS validation and request-body
reading through native response headers and body. This is one end-to-end cap,
not a 60-second native-query allowance plus five seconds of transport: caller
authentication and request-body time consume the same 65 seconds, so the
Worker may terminate the native exchange before its independent 60-second
query cap. Expiry aborts the active fetch or stream reader and returns a sanitized 504;
caller cancellation aborts the same active work. Gateway failures are
sanitized before returning to the caller.

`GET /api/source/capabilities` includes the
`tos_source_text_gateway_capability_v1` envelope. `selected` means only that
the origin, service-token bindings, caller issuer, and caller audience passed
local configuration checks;
`runtime_readiness` remains `not-probed`. The Worker never probes the native
origin from the capability route. The endpoint origin and credentials are not
returned, and the caller audience is not returned. D1's source-read capability
remains unavailable even when the separate native gateway is configured.
