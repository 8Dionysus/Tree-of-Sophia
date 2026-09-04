# Cloudflare edge deployment

This directory is the permanent, repository-driven production profile for
`treeofsophia.com`. A Cloudflare Worker serves the checked-in web application,
uses Workers Static Assets for precomputed high-volume packets, and uses D1 for
bounded search and graph queries. The existing scale-export routes stream
CSV/JSONL from normalized D1 rows so the browser's download controls do not
depend on the former Python origin.

The edge is a generated read model. It does not own philosophical meaning,
review state, rights, or canon. `scripts/build_runtime.py` reads only the four
standalone access inputs already allowlisted by `Tree-of-Sophia`:

- `ToS/derived-exports/tos_corpus_index.min.json`
- `ToS/derived-exports/philosophy_graph_projection.min.json`
- `ToS/derived-exports/epistemic_evidence_projection.min.json`
- `ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json`

KAG is not a build input or runtime dependency. The edge build must not
regenerate or silently strengthen any KAG surface.

## Local verification

From this directory, install the Worker dependencies with `npm ci` and the web
dependencies with `npm run ci:web`. `npm run check:local` then rebuilds the web
assets and generated edge data, regenerates Cloudflare binding types,
type-checks the Worker, runs pure unit tests, imports the generated read model into a
local D1 instance, and compares representative HTTP packets against the Python
`ToSAccessCore` contract.

Generated `dist/`, `runtime/`, local D1 state, and dependencies are ignored.
Only source, configuration, lockfiles, tests, and generated binding types are
tracked.

## Production flow

Cloudflare Workers Builds watches the repository's `main` branch with this
directory as its root. Its production build command is `npm run build:ci`, and
its production deploy command is `npm run deploy:edge`.

The deploy command first compares the generated `data_revision` with the live
D1 metadata. That digest covers both the allowlisted inputs and the explicit
read-model schema version. Ordinary code-only rebuilds and retries skip the
64k-row import when that revision is already current. A changed source or
read-model schema revision imports the newly generated D1 read model before
deploying the Worker and static assets.
`npm run deploy:edge:plan` performs the read-only revision check without
importing or deploying, while `npm run load:remote` is the explicit recovery
command. Operators can set `TOS_D1_MAX_SYNC_STATEMENTS` when an environment
needs an additional write-size ceiling; paid D1 is not artificially capped by
the repository default.

The SQL build uses `_next` tables and only swaps them into the live names after
every data row has been prepared. A failed data build therefore leaves the
previous live tables in place. A Cloudflare revision check reads at most one
metadata row per query and a matching revision writes zero rows.

The Worker owns the apex custom domain and redirects `www.treeofsophia.com` to
the apex. `api`, `assets`, and `docs` remain unassigned for later bounded
profiles.

No Cloudflare credential belongs in this repository. Local Wrangler OAuth and
Cloudflare's encrypted Workers Builds token are operational credentials owned
by Cloudflare and the operator account.

## Acceptance boundary

Green local checks prove source-to-edge contract parity for the sampled API
surface. Green GitHub checks prove the pushed revision builds. A successful
Workers Build proves deployment from that revision. Public acceptance requires
the apex health packet, HTML and static assets, representative corpus and
philosophy queries, and the `www` redirect to succeed from outside the origin
host.

The former Cloudflare Tunnel profile remains a temporary recovery and local
preview route only. It is not the production availability architecture because
it requires a continuously powered origin machine.
