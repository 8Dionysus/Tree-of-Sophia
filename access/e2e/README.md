# Standalone browser E2E

`test_webmcp.py` runs the checked-in access server against a real Chromium
browser. It covers the WebMCP registration seam with a deterministic
`document.modelContext` test double because WebMCP remains an experimental
browser feature and is not enabled in every Chromium build. The rest of the
path is real: HTTP responses, CSP and Permissions Policy, built web assets,
page commands, selection-bound registration, cancellation, local persistence,
reload, and the no-WebMCP fallback.

Install the optional dependencies and browser once:

```bash
python -m pip install -e 'access[e2e]'
playwright install chromium
npm --prefix access/web ci
npm --prefix access/web run build
```

Run the product-shell checks with:

```bash
PYTHONPATH=access/src pytest -q access/tests/test_http_security.py access/e2e/test_webmcp.py
```

Set `TOS_E2E_CHROMIUM` when Chromium is installed at a non-standard path.
The harness does not prove a browser vendor's native WebMCP implementation or
agent model tool-choice quality; those remain separate compatibility and eval
surfaces.
