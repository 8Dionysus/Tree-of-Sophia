# TOS-D-0042 Repository-driven Cloudflare edge

## Index Metadata

- Decision ID: TOS-D-0042
- Original date: 2026-09-03
- Surface classes: access/deployment, github/workflow, docs/operations
- ToS layers: access, derived-exports, docs, ports
- Tree classes: public edge, generated read model, deployment route, owner handoff
- Guard families: source-first authority, contract parity, credential boundary, runtime independence
- Posture: accepted

## Context

The first public `treeofsophia.com` route used Cloudflare Tunnel to expose the
loopback-only standalone access server from an operator machine. That proved
the hostname and application could work together, but availability still
depended on that machine remaining powered, connected, and healthy.

The product requirement is stronger: a merge to the ToS repository's `main`
branch must be sufficient to build and deploy the public site on permanent
Cloudflare infrastructure. Domain ownership, hosting, and source authority
must stay distinct, and the accelerated landing must not reopen or regenerate
the deliberately frozen KAG family.

## Decision

Use one Cloudflare Worker as the production application boundary. Workers
Static Assets serve the built web application and precomputed high-volume API
packets. D1 serves a generated read-only model for bounded search, node,
neighborhood, path, Evidence Lens, and streaming scale-export queries. The
edge build reads the same four allowlisted inputs as the standalone access
product and compares representative responses with `ToSAccessCore`.

Cloudflare Workers Builds watches `main`, builds from
`access/deploy/cloudflare-worker/`, compares the generated source revision with
the deployed D1 revision, refreshes D1 only when they differ, and deploys the
Worker and assets. An operator may additionally configure a statement-count
ceiling for a bounded environment. The apex custom domain belongs to this
profile; `www` redirects to the apex. Cloudflare Tunnel remains a deliberately
activated recovery or local preview profile and is not a competing production
owner.

## Options Considered

- Keep Cloudflare Tunnel as production. Rejected because public availability
  would continue to depend on an operator machine.
- Publish only static UI assets and keep the Python API on another origin.
  Rejected because it splits deployment ownership and retains an external
  always-on dependency.
- Load the complete philosophy projection into each Worker invocation.
  Rejected because the large projection would turn a bounded edge request into
  a high-memory whole-corpus load.
- Use Static Assets plus a D1 generated read model behind one Worker. Chosen
  because UI, API, custom domains, and repository-driven deployment remain one
  bounded Cloudflare profile while source authority remains in ToS.

## Rationale

The Worker is transport and query glue, not a second philosophical core. The
Python core remains the stable contract and produces the edge inputs;
precomputed packets preserve exact high-volume responses, while D1 avoids
loading the full graph for ordinary bounded queries. Contract comparison makes
drift visible without assigning semantic authority to Cloudflare.

Keeping build and deploy commands in the repository makes the production path
reviewable and reproducible. Keeping credentials only in local Wrangler OAuth
or Cloudflare's encrypted build-token store prevents Git history and generated
artifacts from becoming secret stores.

The D1 refresh is content-addressed at deployment time. The digest binds both
the ToS inputs and an explicit read-model schema version, so a schema change
cannot reuse structurally stale D1 state. Code-only merges, retries, and
rebuilds of the same inputs and schema perform a bounded metadata read and zero
D1 writes instead of consuming the write allowance again.

## Consequences

A merged revision can deploy while every operator machine is offline. Static
packets are rebuilt on every relevant revision; the D1 read model is refreshed
only when its source revision changes and is otherwise reused exactly. Failed
source generation or required D1 preparation blocks deployment and leaves the
prior live read model available.

The edge may report deployment and data revisions, but it cannot promote
canon, accept a claim, clear rights, or turn a generated projection into
authored meaning. KAG is neither regenerated nor consulted by this deployment
profile. Future `api`, `assets`, or `docs` hostnames need their own explicit
owner decision before assignment.

## Source Surfaces

- `access/AGENTS.md`
- `access/README.md`
- `access/contracts/runtime-data.v1.json`
- `access/contracts/query-operations.v1.json`
- `access/src/tos_access/core.py`
- `access/deploy/cloudflare-worker/README.md`
- `access/deploy/cloudflare-worker/wrangler.jsonc`
- `access/deploy/cloudflare-tunnel/README.md`
- `.github/workflows/cloudflare-edge.yml`

## Validation

Regenerate and validate decision indexes through the canonical decision lane.
Run the Cloudflare edge build, generated binding types, TypeScript checks,
unit tests, local D1 import, core-versus-Worker contract comparison, and a
Wrangler dry run. Landing additionally requires green GitHub checks, a
successful Workers Build from `main`, and public apex plus `www` acceptance.
