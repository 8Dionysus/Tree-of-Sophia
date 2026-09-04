from __future__ import annotations

import json
import os
import shutil
import threading
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
import sys

sys.path.insert(0, str(REPO_ROOT / "access/src"))

playwright = pytest.importorskip("playwright.sync_api")
from playwright.sync_api import Browser, BrowserContext, Page, sync_playwright  # noqa: E402

from tos_access.core import ToSAccessCore  # noqa: E402
from tos_access.http_server import make_server  # noqa: E402

CHROMIUM = os.environ.get("TOS_E2E_CHROMIUM") or shutil.which("chromium-browser") or shutil.which("chromium")

MODEL_CONTEXT_INIT = r"""
(() => {
  const active = new Map();
  const all = [];
  const names = () => [...active.keys()].sort();
  const first = (name) => all.find((tool) => tool.name === name);
  const invoke = async (name, input = {}) => {
    const tool = active.get(name);
    if (!tool) throw new Error(`missing WebMCP tool: ${name}`);
    return tool.execute(input, { signal: new AbortController().signal });
  };
  const invokeFirst = async (name, input = {}) => {
    const tool = first(name);
    if (!tool) throw new Error(`missing historical WebMCP tool: ${name}`);
    try {
      return { ok: true, value: await tool.execute(input, { signal: new AbortController().signal }) };
    } catch (error) {
      return { ok: false, name: error?.name, message: error?.message };
    }
  };
  const invokeAbort = async (name, input = {}) => {
    const tool = active.get(name);
    if (!tool) throw new Error(`missing WebMCP tool: ${name}`);
    const controller = new AbortController();
    const pending = tool.execute(input, { signal: controller.signal });
    setTimeout(() => controller.abort(new DOMException("e2e cancellation", "AbortError")), 0);
    try {
      return { ok: true, value: await pending };
    } catch (error) {
      return { ok: false, name: error?.name, message: error?.message };
    }
  };
  window.__TOS_E2E = { names, invoke, invokeFirst, invokeAbort, all };
  Object.defineProperty(document, "modelContext", {
    configurable: true,
    value: {
      registerTool: async (tool, options = {}) => {
        active.set(tool.name, tool);
        all.push(tool);
        options.signal?.addEventListener("abort", () => {
          if (active.get(tool.name) === tool) active.delete(tool.name);
        }, { once: true });
      },
    },
  });
})();
"""


@pytest.fixture(scope="session")
def access_base_url() -> str:
    server = make_server(ToSAccessCore.discover(tos_root=REPO_ROOT), port=0)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}"
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


@pytest.fixture()
def webmcp_page(access_base_url: str):
    with sync_playwright() as p:
        launch_options = {"headless": True, "args": ["--no-sandbox"]}
        if CHROMIUM:
            launch_options["executable_path"] = CHROMIUM
        browser: Browser = p.chromium.launch(**launch_options)
        context: BrowserContext = browser.new_context()
        context.add_init_script(MODEL_CONTEXT_INIT)
        page = context.new_page()
        page.goto(f"{access_base_url}/?mode=philosophy&view=chronology&graph=nodes", wait_until="domcontentloaded", timeout=30_000)
        wait_for(page, "window.__TOS_E2E.names().includes('tos.page.context')")
        wait_for(page, "Boolean(document.getElementById('current-view-title')?.textContent)")
        yield page
        context.close()
        browser.close()


def invoke(page: Page, name: str, input: dict | None = None) -> dict:
    return page.evaluate("([name, input]) => window.__TOS_E2E.invoke(name, input || {})", [name, input or {}])


def text_result(result: dict) -> dict:
    return json.loads(result["content"][0]["text"])


def command_value(result: dict) -> dict:
    packet = text_result(result)
    return packet.get("value", packet)


def wait_for(page: Page, expression: str, timeout: float = 30.0) -> None:
    import time

    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if page.evaluate(expression):
            return
        page.wait_for_timeout(100)
    raise AssertionError(f"browser condition timed out: {expression}")


def first_edge_and_node(page: Page) -> tuple[str, str]:
    payload = page.evaluate("""async () => {
      const response = await fetch('/api/philosophy/views/chronology?limit=1000');
      return response.json();
    }""")
    edge = next(item for item in payload.get("edges", []) if item.get("edge_id") and item.get("from_id") and item.get("to_id"))
    return str(edge["edge_id"]), str(edge["from_id"])


def test_real_browser_webmcp_loop_and_stale_deixis(webmcp_page: Page) -> None:
    page = webmcp_page
    names = page.evaluate("window.__TOS_E2E.names()")
    assert "tos.page.context" in names
    assert "tos.page.research-workspace" in names
    assert page.locator("#agent-surface").get_attribute("data-webmcp-state") == "connected"

    edge_id, node_id = first_edge_and_node(page)
    invoke(page, "tos.page.select", {"item_id": edge_id})
    wait_for(page, "window.__TOS_E2E.names().includes('tos.page.add-session-hypothesis')")
    assert "tos.page.exclude-selected-edge" in page.evaluate("window.__TOS_E2E.names()")
    assert "tos.page.save-route-comparison" in page.evaluate("window.__TOS_E2E.names()")

    old_context_tool = page.evaluate("window.__TOS_E2E.invokeFirst('tos.page.context')")
    assert command_value(old_context_tool["value"])["selected"]["id"] == edge_id
    page.locator("#research-workspace-panel > summary").click()
    page.locator("#workspace-note-input").fill("Human note")
    page.locator("#workspace-add-note").click()
    wait_for(page, "document.getElementById('research-workspace-summary')?.textContent?.includes('1N')")
    agent_note = command_value(invoke(page, "tos.page.add-research-note", {"text": "Agent note"}))
    assert agent_note["research_workspace"]["note_count"] == 2
    assert "Agent note" in page.locator("#research-workspace-body").inner_text()

    hypothesis = command_value(invoke(page, "tos.page.add-session-hypothesis", {"statement": "Keep this as a local possibility."}))
    assert hypothesis["authority"] == {"source": False, "reviewed": False, "canon": False}
    assert hypothesis["hypothesis"]["posture"] == {"session_hypothesis": True, "source": False, "reviewed": False, "canon": False}
    assert "not source" in page.locator("#research-workspace-body").inner_text().lower()

    invoke(page, "tos.page.save-route-comparison", {"label": "Direct"})
    invoke(page, "tos.page.save-route-comparison", {"label": "Alternative"})
    workspace_read = command_value(invoke(page, "tos.page.research-workspace"))
    assert workspace_read["research_workspace"]["comparison_count"] == 2
    assert len(workspace_read["route_preview"]) == 2

    invoke(page, "tos.page.select", {"item_id": node_id})
    stale = page.evaluate("window.__TOS_E2E.invokeFirst('tos.page.add-session-hypothesis', {statement: 'stale'})")
    # The old selection-bound tool is retained by the harness even after its
    # browser registration is aborted, so this exercises revision rejection.
    assert stale["ok"] is False
    assert "stale page context revision" in stale["message"]


def test_real_browser_cancellation_reload_and_deep_link(webmcp_page: Page) -> None:
    page = webmcp_page
    edge_id, node_id = first_edge_and_node(page)
    invoke(page, "tos.page.select", {"item_id": edge_id})
    wait_for(page, "!window.__TOS_E2E.names().includes('tos.page.show-neighborhood')")
    # Edge selection exposes epistemic and workspace tools; select a node for
    # the cancellable neighborhood command.
    invoke(page, "tos.page.select", {"item_id": node_id})
    wait_for(page, "window.__TOS_E2E.names().includes('tos.page.show-neighborhood')")
    cancelled = page.evaluate("window.__TOS_E2E.invokeAbort('tos.page.show-neighborhood', {})")
    assert cancelled["ok"] is False
    assert cancelled["name"] == "AbortError"

    invoke(page, "tos.page.add-research-note", {"text": "Survives reload"})
    page.reload(wait_until="domcontentloaded", timeout=30_000)
    wait_for(page, "Boolean(document.getElementById('current-view-title')?.textContent)")
    wait_for(page, "document.getElementById('research-workspace-body')?.textContent?.includes('Survives reload')")
    assert "Survives reload" in (page.locator("#research-workspace-body").text_content() or "")
    assert "mode=philosophy" in page.url and "view=chronology" in page.url and "graph=nodes" in page.url


def test_real_browser_graceful_without_webmcp(access_base_url: str) -> None:
    with sync_playwright() as p:
        launch_options = {"headless": True, "args": ["--no-sandbox"]}
        if CHROMIUM:
            launch_options["executable_path"] = CHROMIUM
        browser = p.chromium.launch(**launch_options)
        context = browser.new_context()
        page = context.new_page()
        page.goto(f"{access_base_url}/?mode=philosophy&view=chronology", wait_until="domcontentloaded", timeout=30_000)
        wait_for(page, "Boolean(document.getElementById('current-view-title')?.textContent)")
        page.wait_for_timeout(500)
        assert page.evaluate("document.modelContext === undefined")
        assert page.locator("#app").count() == 1
        assert page.locator("#agent-surface").get_attribute("data-webmcp-state") == "unavailable"
        assert "Tree of Sophia" in page.locator("body").inner_text()
        context.close()
        browser.close()
