# Cloudflare Tunnel deployment

This is a temporary recovery and local-preview route for the ToS-owned
standalone access product. It is not the permanent production architecture.
`tos_access` remains the application server and binds to loopback; Cloudflare
Tunnel supplies outbound-only ingress and TLS while this fallback is active.

The route is intentionally small:

- application code, web assets, and allowlisted projections come from one
  `Tree-of-Sophia` checkout;
- `abyss-stack` is not a runtime dependency;
- the tunnel token stays outside Git;
- the Cloudflare account owns only the tunnel, DNS, and edge policy.

## Host configuration

Create `%h/.config/tree-of-sophia/site.env` from `site.env.example` and place
the remotely managed tunnel token in
`%h/.config/tree-of-sophia/cloudflare-tunnel.token` with mode `0600`.

Link the two unit files into `%h/.config/systemd/user/`, reload the user
manager, then enable `tos-cloudflare-tunnel.service`. The tunnel unit requires
and starts `tos-access-origin.service` first.

The configured checkout must pass the standalone access tests and browser
build checks before it is made public. A healthy local origin is observable at
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
