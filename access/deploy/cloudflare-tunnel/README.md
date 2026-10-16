# Cloudflare Tunnel deployment

This is a temporary recovery and local-preview route for the ToS-owned
standalone access product. It is not the permanent production architecture.
The installed native `tos` entrypoint binds to loopback; Cloudflare Tunnel
supplies outbound-only ingress and TLS while this fallback is active.

The route is intentionally small:

- an optional selected ToS source/data tree comes from one
  `Tree-of-Sophia` checkout;
- an optional durable PostgreSQL source-text consumer is selected by an
  absolute server-side selection file and can run without the ToS checkout;
- native application code and web assets come from the explicit installed
  prefix selected by `TOS_ACCESS_BIN`;
- `abyss-stack` is not a runtime dependency;
- the tunnel token stays outside Git;
- the Cloudflare account owns only the tunnel, DNS, and edge policy.

## Host configuration

Install the admitted native software archive into a fresh prefix as described
in [native fresh-prefix installation](../../README.md#native-fresh-prefix-installation).
Set `TOS_ACCESS_BIN` in `site.env` to that prefix's absolute `bin/tos` path.
For the existing Site route, keep `TOS_SITE_ROOT` set to the selected ToS
checkout. To serve only the durable PostgreSQL source-text route, omit
`TOS_SITE_ROOT` and set `TOS_SOURCE_TEXT_SEARCH_SELECTION` to its absolute
owner-selection JSON path. The native origin passes this selection directly
to `tos`; the request cannot choose its database, domain, consumer or profile.
Set `TOS_SOURCE_PG_URL` privately in `site.env` for that consumer. Keep this
environment file mode `0600` because it may contain database credentials. The
tunnel token remains in its separate credential file.

## Optional Worker source-text origin

The Worker profile can use this native PostgreSQL endpoint through a separately
configured HTTPS hostname and Cloudflare Access service-token policy. That
hostname must be distinct from `treeofsophia.com`, which this temporary profile
routes to the native site; the Worker must never target its own public route.
Configure the Worker-side origin and outbound Access client ID/secret only
after the native origin has its selected `TOS_SOURCE_TEXT_SEARCH_SELECTION` and
private `TOS_SOURCE_PG_URL`. Also configure the inbound Access team issuer and
application audience. The source owner must align that application's Allow
policy with the principal set admitted for the selected native private
source-text delivery scope. The Worker verifies the JWT audience and never
forwards its assertion or identity headers. These settings do not grant source
rights or public publication. No gateway hostname is assigned by this
repository, and the Worker capability route reports configuration only, not
origin health.

The systemd origin still locates the maintained helper through
`TOS_SITE_ROOT` for backward compatibility. When the helper is kept separately
from Site data, set `TOS_ACCESS_ORIGIN_LAUNCHER` to its absolute `run-origin.sh`
path; this decouples launcher code from the selected data root. The native
software archive still contains neither this helper nor the systemd units, so
keep this deployment directory available separately. Create
`%h/.config/tree-of-sophia/site.env` from `site.env.example` and place the
remotely managed tunnel token in
`%h/.config/tree-of-sophia/cloudflare-tunnel.token` with mode `0600`.

Link the two unit files into `%h/.config/systemd/user/`, reload the user
manager, then enable `tos-cloudflare-tunnel.service`. The tunnel unit requires
and starts `tos-access-origin.service` first.

When `TOS_SITE_ROOT` is set, the configured source checkout must contain the
selected runtime projections. A PostgreSQL-only origin requires no ToS
checkout or source projections. The installed prefix supplies the executable
and static web assets in either mode. A healthy local origin is observable at
`http://127.0.0.1:${TOS_SITE_PORT}/health`; tunnel health is observable only
after `cloudflared` reports connected and the public hostname returns that
same read-only health packet.

## Temporary Cloudflare configuration

Use a remotely managed tunnel with one ingress rule for the public hostname:

```text
treeofsophia.com -> http://127.0.0.1:5439
*                 -> http_status:404
```

When this fallback is deliberately activated, the apex DNS record is a proxied
CNAME to `<tunnel-id>.cfargotunnel.com`. The normal production owner is the
Worker profile in `../cloudflare-worker/`; do not run both profiles as
competing owners of the apex hostname. `www.treeofsophia.com` redirects to the
apex and must not become a second application runtime.

## Claim boundary

A healthy tunnel proves public transport to the selected ToS checkout. It does
not prove that unmerged source work, later generated projections, or optional
AbyssOS integrations are deployed. The host must remain powered and online;
that dependency is why this profile cannot satisfy permanent production
availability.
