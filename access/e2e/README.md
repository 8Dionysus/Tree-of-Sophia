# Standalone browser E2E

`test_webmcp.mjs` runs the installed native access site through the production
Rust HTTP handler, with a disposable Rust fixture executor, against a real
Chromium browser. The five static reader cases use the checked-in Vite host.
It covers the WebMCP registration seam with a deterministic
`document.modelContext` test double because WebMCP remains an experimental
browser feature and is not enabled in every Chromium build. The rest of the
path is real: HTTP responses, CSP and Permissions Policy, built web assets,
page commands, selection-bound registration, cancellation, local persistence,
reload, and the no-WebMCP fallback.

Run the browser behavior through the `software_browser` lane in
[`docs/validation/validation_lanes.json`](../../docs/validation/validation_lanes.json)
or the matching CI step in
[`.github/workflows/repo-validation.yml`](../../.github/workflows/repo-validation.yml).
The lane owns the exact install and execution commands, including the pinned
Node Playwright runtime, installed native site, and test-only fixture host.
This fixture host exercises the production Rust HTTP and installed-site
handlers; its synthetic API packets remain test fixtures and do not claim
owner-selected source authority.

Set `TOS_E2E_CHROMIUM` when Chromium is installed at a non-standard path.
The harness does not prove a browser vendor's native WebMCP implementation or
agent model tool-choice quality; those remain separate compatibility and eval
surfaces.
