# Standalone browser E2E

`test_webmcp.py` runs the checked-in access server against a real Chromium
browser. It covers the WebMCP registration seam with a deterministic
`document.modelContext` test double because WebMCP remains an experimental
browser feature and is not enabled in every Chromium build. The rest of the
path is real: HTTP responses, CSP and Permissions Policy, built web assets,
page commands, selection-bound registration, cancellation, local persistence,
reload, and the no-WebMCP fallback.

Install the optional dependencies and browser, then run the product-shell
checks through the `standalone_access` lane in
[`docs/validation/validation_lanes.json`](../../docs/validation/validation_lanes.json)
or the matching CI step in
[`.github/workflows/repo-validation.yml`](../../.github/workflows/repo-validation.yml).
The lane owns the exact install and execution commands so this orientation
page does not become a second command authority.

Set `TOS_E2E_CHROMIUM` when Chromium is installed at a non-standard path.
The harness does not prove a browser vendor's native WebMCP implementation or
agent model tool-choice quality; those remain separate compatibility and eval
surfaces.
