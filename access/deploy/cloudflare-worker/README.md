# Public research Worker

The public site uses `wrangler.public-research.jsonc` and serves the home,
Zarathustra and Antiquity stands and the `/__research/` assets. Unmatched
paths, including the retired atlas API, receive the site's 404 page. The
Worker has no D1, R2 or secret bindings.

Build the site with `npm run build:public-research` from this directory. Run
`npm run typecheck` and `npm test` for the Worker and shared contracts. The
shared tests use the matching generated `tos-web-rules` WASM; prepare it as
specified in [RELEASING](../../../docs/RELEASING.md). Inspect a local upload
with `npm run deploy:public-research:plan`. Deployment follows that release
route and is a separate operator action.

The old access Worker and its D1 database are retired from this codebase. The
operator owns D1 export, custom-domain transfer, and cloud removal after code
merge. The current route configuration is unchanged by that retirement.
