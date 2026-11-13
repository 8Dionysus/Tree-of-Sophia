# Browser E2E

`test_webmcp.mjs` runs the shared reader and public research component cases
against a local Vite host and Chromium. The fixtures use artificial data.
Run the `software_browser` lane in
[`docs/validation/validation_lanes.json`](../../docs/validation/validation_lanes.json).
Set `TOS_E2E_CHROMIUM` when Chromium is installed at a non-standard path and
`TOS_E2E_NODE_MODULES` to the Playwright installation when needed.
These checks do not establish production deployment or source admission.
