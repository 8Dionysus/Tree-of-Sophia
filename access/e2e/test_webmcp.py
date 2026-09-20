from __future__ import annotations

from contextlib import contextmanager
import copy
import hashlib
import json
import os
import shutil
import socket
import subprocess
import tempfile
import threading
import time
import urllib.request
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
import sys

sys.path.insert(0, str(REPO_ROOT / "access/src"))
sys.path.insert(0, str(REPO_ROOT / "access/tests"))

playwright = pytest.importorskip("playwright.sync_api")
from playwright.sync_api import Browser, BrowserContext, Page, sync_playwright  # noqa: E402

from tos_access.core import ToSAccessCore  # noqa: E402
from tos_access.http_server import make_server  # noqa: E402
from tos_access.source_read import (  # noqa: E402
    PUBLICATION_PROTOCOL,
    SourceOwnerBinding,
    SourceReadService,
    _canonical_digest,
)
from fixture_support import write_fixture  # noqa: E402

CHROMIUM = os.environ.get("TOS_E2E_CHROMIUM") or shutil.which("chromium-browser") or shutil.which("chromium")
SOURCE_RECORD_RELATIVE = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json"
)
SOURCE_RECORD_PATH = REPO_ROOT / "access/tests/fixtures/source-assembly" / SOURCE_RECORD_RELATIVE


class _E2EMetadataOwner:
    """Small owner-bound reader for one exact public metadata fixture.

    The browser fixture carries the real frozen source record as a normalized
    source target.  The reader below models the source owner that must issue
    the matching epoch and re-check that exact id/version/digest; it does not
    infer a path or expose native text.
    """

    def __init__(self, record: dict, source_revision: str, source_ref: Path = SOURCE_RECORD_RELATIVE):
        self.record = copy.deepcopy(record)
        self.source_revision = source_revision
        self.source_ref = source_ref

    def source_read_binding(self) -> dict:
        return {
            "source_revision": self.source_revision,
            "catalog_root_sha256": "b" * 64,
            "catalog_namespace": "tos.catalog.e2e.fixture",
            "source_publication": {
                "protocol": PUBLICATION_PROTOCOL,
                "token": "sha256:" + "c" * 64,
                "generation": 1,
            },
        }

    def verify_current(self) -> None:
        return None

    def resolve_typed(self, exact_ref: dict) -> dict:
        expected_ref = {
            "id": self.record["record_id"],
            "version": self.record["record_version"],
            "digest": "sha256:" + _canonical_digest(self.record),
        }
        if exact_ref != expected_ref:
            return {
                "status": "stale",
                "reason": "exact-version-digest-mismatch",
                "exact_ref": copy.deepcopy(exact_ref),
                "record": None,
                "record_digest": None,
                "provenance": None,
            }
        return {
            "status": "available",
            "reason": "exact-current-version",
            "exact_ref": copy.deepcopy(exact_ref),
            "record": copy.deepcopy(self.record),
            "record_digest": exact_ref["digest"],
            "provenance": {
                "catalog": {
                    "record_key": self.record["record_id"],
                    "row_sha256": "d" * 64,
                },
                "source": {"source_ref": self.source_ref.as_posix()},
            },
            "descriptor": {
                "adapter": "native-corpus",
                "record_type": self.record["record_type"],
                "source_scope": "public_metadata_only",
            },
        }

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
def access_base_url(tmp_path_factory: pytest.TempPathFactory, request: pytest.FixtureRequest) -> str:
    root = tmp_path_factory.mktemp("access-e2e")
    write_fixture(root)
    projection_path = root / "ToS/derived-exports/philosophy_graph_projection.min.json"
    projection_bytes = projection_path.read_bytes()
    projection = json.loads(projection_bytes)
    source_record = None
    source_ref = SOURCE_RECORD_RELATIVE
    source_mode = getattr(request, "param", None)
    if source_mode in ("source", "source-native-metadata"):
        if source_mode == "source":
            source_record = json.loads(SOURCE_RECORD_PATH.read_text(encoding="utf-8"))
        else:
            # Synthetic metadata only: the binding prompts optional transport
            # discovery, while this owner never delivers native wording.
            source_ref = Path("ToS/synthetic/native-metadata.json")
            source_record = {
                "record_type": "text-unit", "record_id": "tos.text-unit.browser-fixture",
                "record_version": 1, "preferred_label": "Synthetic native metadata",
                "notes": "Available exact metadata remains readable.",
                "native_text_binding": {
                    "unit_id": "tos.text-unit.browser-fixture", "unit_version": 1,
                    "segmentation_id": "tos.text-segmentation.browser-fixture", "segmentation_version": 1,
                    "packet_id": "tos.source-text-unit-packet.browser-fixture", "packet_version": 1,
                    "packet_sha256": "d" * 64,
                    "text_layer": {"layer_id": "tos.text-layer.browser-fixture", "layer_version": 1, "record_sha256": "e" * 64},
                    "ordered_anchor_refs": ["tos.anchor.browser-fixture"],
                },
            }
        for node in projection.get("nodes", []):
            if node.get("node_id") == "a":
                properties = node.setdefault("properties", {})
                properties["source_record"] = copy.deepcopy(source_record)
                node["source_ref"] = source_ref.as_posix()
                break
        else:
            raise AssertionError("synthetic source fixture node a is missing")
        projection_path.write_text(json.dumps(projection), encoding="utf-8")
    projection_bytes = projection_path.read_bytes()
    projection = json.loads(projection_bytes)
    projection_digest = hashlib.sha256(projection_bytes).hexdigest()
    count_digest = hashlib.sha256(
        json.dumps(projection.get("counts", {}), sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    view_fingerprints = []
    for view in projection.get("views", []):
        view_id = str(view.get("view_id") or "synthetic")
        view_fingerprints.append(
            {
                "view_id": view_id,
                "fingerprint": hashlib.sha256(
                    json.dumps(view, sort_keys=True, separators=(",", ":")).encode("utf-8")
                ).hexdigest(),
                "node_count": len(view.get("node_ids", [])),
                "edge_count": len(view.get("edge_ids", [])),
                "cluster_count": sum(view_id in cluster.get("view_ids", []) for cluster in projection.get("clusters", [])),
                "source_ref_count": len(view.get("source_refs", [])),
            }
        )
    projection["snapshot_review"] = {
        "snapshot_schema_version": "tos_philosophy_graph_projection_snapshot_v1",
        "current_snapshot": {
            # Synthetic pre-review fixture identity only; this is not a production fingerprint.
            "projection_fingerprint": projection_digest,
            "count_fingerprint": count_digest,
            "view_fingerprints": view_fingerprints,
        },
        "diff_route": {
            "mode": "fingerprint-ready",
            "changed_subgraph_available": False,
            "previous_snapshot_ref": None,
            "next_route": "synthetic browser fixture only",
        },
    }
    projection_path.write_text(json.dumps(projection), encoding="utf-8")

    ledger = root / "ToS/source-witnesses/access-requests/public-ledger"
    ledger.mkdir(parents=True, exist_ok=True)
    permission_states = {
        "local_access": "unknown",
        "ocr_or_transcription": "unknown",
        "indexing": "unknown",
        "embeddings": "unknown",
        "quotation": "unknown",
        "metadata_publication": "unknown",
        "derivative_publication": "unknown",
        "server_processing": "unknown",
        "source_redistribution": "unknown",
    }
    for request_id, title in (
        ("fixture.nietzsche.lexicon", "Nietzsche-Wörterbuch (synthetic test fixture)"),
        ("fixture.nietzsche.edition", "Nietzsche edition (synthetic test fixture)"),
    ):
        record = {
            "$schema": "https://tree-of-sophia.local/ToS/contracts/access-request.schema.json",
            "schema_version": "tos_access_request_v1",
            "request_id": request_id,
            "material": {
                "title": title,
                "responsibility": "synthetic browser fixture",
                "edition_or_resource": "synthetic browser fixture",
                "requested_portion": "synthetic browser fixture",
                "identifiers": [],
                "tos_refs": ["a"],
                "discovery_refs": ["ToS/canon/a.json"],
            },
            "rights_holder_or_institution": {
                "name": "Synthetic fixture institution",
                "role": "unknown",
                "identification_evidence_urls": ["https://example.test/synthetic-fixture"],
                "identity_status": "unknown",
            },
            "contact_route": {
                "channel_type": "unknown",
                "public_institutional_url": "https://example.test/synthetic-fixture",
                "refreshed_at": "2026-09-14T00:00:00Z",
                "personal_contact_committed": False,
            },
            "project_description_ref": "synthetic browser fixture",
            "research_purpose": "synthetic browser fixture",
            "requested_format": "synthetic browser fixture",
            "requested_permissions": permission_states,
            "local_storage_conditions": {
                "access_controlled": True,
                "source_payload_gitignored": True,
                "location_class": "operator-controlled-local-ToS-storage",
                "retention_posture": "synthetic browser fixture only",
                "removal_supported": True,
            },
            "non_redistribution_without_permission": True,
            "technical_access_bypass_used": False,
            "access_status": "unknown",
            "request_status": "draft",
            "human_send_approval": False,
            "sent_at": None,
            "response": {
                "state": "none",
                "received_at": None,
                "permission_expires_at": None,
                "conditions": [],
                "safe_evidence_refs": [],
            },
            "private_correspondence_ref": None,
            "redacted_public_receipt_ref": None,
            "personal_or_confidential_data_committed": False,
            "rights_record_refs": [],
            "provenance_event_refs": ["tos.event.fixture.synthetic"],
            "record_version": 1,
        }
        (ledger / f"{request_id}.access-request.json").write_text(json.dumps(record), encoding="utf-8")
    core = ToSAccessCore.discover(tos_root=root)
    if source_record is not None:
        # The graph projection owns the selected snapshot revision.  Pin the
        # fixture reader to that revision before exposing the HTTP route.
        source_revision = core.knowledge_graph()["source_revision"]
        owner = _E2EMetadataOwner(source_record, source_revision, source_ref)
        source_read_service = SourceReadService(
            SourceOwnerBinding.from_owner_readers(metadata_reader=owner),
            metadata_record_types={source_record["record_type"]},
        )
        core = ToSAccessCore.discover(tos_root=root, source_read_service=source_read_service)
    if getattr(request, "param", None) == "prepared":
        from tos_access.prepared_publication import publish_prepared
        snapshot = core.knowledge_snapshot()
        publication = root / "browser-prepared.sqlite"
        binding = publish_prepared(publication, graph=snapshot["graph"], catalog=snapshot["catalog"])
        core = ToSAccessCore.discover(tos_root=root, published_read_model_path=publication,
                                      published_read_model_expected=binding)
    server = make_server(core, port=0)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}"
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


@contextmanager
def short_chromium_tmp():
    with tempfile.TemporaryDirectory(prefix="tos-e2e-", dir="/tmp") as browser_tmp:
        previous_tmpdir = os.environ.get("TMPDIR")
        previous_tempdir = tempfile.tempdir
        os.environ["TMPDIR"] = browser_tmp
        tempfile.tempdir = browser_tmp
        try:
            yield
        finally:
            tempfile.tempdir = previous_tempdir
            if previous_tmpdir is None:
                os.environ.pop("TMPDIR", None)
            else:
                os.environ["TMPDIR"] = previous_tmpdir


@pytest.fixture()
def webmcp_page(access_base_url: str):
    with short_chromium_tmp():
        with sync_playwright() as p:
            launch_options = {"headless": True, "args": ["--no-sandbox"]}
            if CHROMIUM:
                launch_options["executable_path"] = CHROMIUM
            browser: Browser = p.chromium.launch(**launch_options)
            context: BrowserContext = browser.new_context(locale="en-US")
            context.add_init_script(MODEL_CONTEXT_INIT)
            page = context.new_page()
            page.goto(f"{access_base_url}/?mode=philosophy&view=chronology&graph=nodes&ui=en", wait_until="domcontentloaded", timeout=30_000)
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


def idb_readings(page: Page, db_name: str) -> list[dict]:
    return page.evaluate(
        """async dbName => {
          const db = await new Promise((resolve, reject) => {
            const request = indexedDB.open(dbName);
            request.onsuccess = () => resolve(request.result);
            request.onerror = () => reject(request.error);
          });
          try {
            return await new Promise((resolve, reject) => {
              const request = db.transaction('readings', 'readonly').objectStore('readings').getAll();
              request.onsuccess = () => resolve(request.result);
              request.onerror = () => reject(request.error);
            });
          } finally {
            db.close();
          }
        }""",
        db_name,
    )


def wait_for_idb_reading(page: Page, db_name: str, predicate, timeout: float = 15.0) -> list[dict]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        readings = idb_readings(page, db_name)
        if any(predicate(item) for item in readings):
            return readings
        page.wait_for_timeout(100)
    raise AssertionError(f"IndexedDB reading did not settle for {db_name}")


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
    assert "tos.zarathustra.word-analysis.prepare" in names
    assert page.locator("#agent-surface").get_attribute("data-webmcp-state") == "connected"
    assert "Codex WebMCP ready" in page.locator("#agent-surface").inner_text()

    word_analysis_result = invoke(
        page,
        "tos.zarathustra.word-analysis.prepare",
        {"query": "судьбы", "language": "ru", "rank": 1},
    )
    assert len(word_analysis_result["content"][0]["text"]) < 1500
    word_analysis = command_value(word_analysis_result)
    assert word_analysis["available"] is False
    assert word_analysis["publication_posture"] == "excluded_from_public_bundle"
    assert word_analysis["page_updated"] is True
    assert "Source-bound word analysis" in page.locator("#detail-list").inner_text()

    edge_id, node_id = first_edge_and_node(page)
    invoke(page, "tos.page.select", {"item_id": edge_id})
    wait_for(page, "window.__TOS_E2E.names().includes('tos.page.add-session-hypothesis')")
    assert "tos.page.inspect-selection" in page.evaluate("window.__TOS_E2E.names()")
    assert "tos.page.add-note-to-selection" in page.evaluate("window.__TOS_E2E.names()")
    assert "tos.page.compare-readings" in page.evaluate("window.__TOS_E2E.names()")
    assert "tos.page.stage-proposal" in page.evaluate("window.__TOS_E2E.names()")
    assert "tos.page.exclude-selected-edge" in page.evaluate("window.__TOS_E2E.names()")
    assert "tos.page.save-route-comparison" in page.evaluate("window.__TOS_E2E.names()")

    selection_result = invoke(page, "tos.page.inspect-selection")
    assert len(selection_result["content"][0]["text"]) < 1500
    selection = command_value(selection_result)
    assert selection["selection"]["id"] == edge_id
    assert selection["selection"]["semantic_kind"] == "relation"
    comparison_result = invoke(page, "tos.page.compare-readings", {"limit": 20})
    assert len(comparison_result["content"][0]["text"]) < 1500
    comparison = command_value(comparison_result)
    assert comparison["schema"] == "tos_interpretation_comparison_v1"
    assert comparison["page_updated"] is True
    assert "Interpretation comparison" in page.locator("#detail-list").inner_text()
    wait_for(page, "window.__TOS_E2E.names().includes('tos.page.stage-proposal')")
    proposal_result = invoke(page, "tos.page.stage-proposal", {
        "kind": "interpretation",
        "statement": "Treat this route as a reviewable alternative.",
        "source_refs": ["ToS/philosophy/atlas/atlas.manifest.json"],
        "evidence_refs": [edge_id],
        "confidence": "low",
    })
    assert len(proposal_result["content"][0]["text"]) < 1500
    proposal = command_value(proposal_result)
    assert proposal["proposal"]["status"] == "pending_review"
    assert proposal["authority"] == {"source": False, "reviewed": False, "canon": False}
    assert "pending_review" in (page.locator("#research-workspace-body").text_content() or "")

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


def test_real_browser_source_gap_research_stages_reviewable_route(webmcp_page: Page) -> None:
    page = webmcp_page
    assert "tos.page.find-source-gaps" in page.evaluate("window.__TOS_E2E.names()")
    found = text_result(invoke(page, "tos.page.find-source-gaps", {"query": "Nietzsche", "limit": 20}))
    assert found["result_count"] >= 2
    assert "corpus-completeness" in found["authority_note"]
    lexical = next(gap for gap in found["gaps"] if "Nietzsche-Wörterbuch" in gap["label"])
    assert "Nietzsche-Wörterbuch" in page.locator("#detail-list").inner_text()

    invoke(page, "tos.page.select", {"item_id": lexical["id"]})
    wait_for(page, "window.__TOS_E2E.names().includes('tos.page.stage-proposal')")
    assert "tos.page.inspect-epistemic" not in page.evaluate("window.__TOS_E2E.names()")
    staged = command_value(invoke(page, "tos.page.stage-proposal", {
        "kind": "source_route",
        "statement": "Verify a lawful current institutional route to the relevant Nietzsche-Wörterbuch article before source use.",
        "confidence": "unknown",
    }))
    assert staged["proposal"]["status"] == "pending_review"
    assert staged["authority"] == {"source": False, "reviewed": False, "canon": False}


@pytest.mark.parametrize("access_base_url", ["source"], indirect=True)
def test_real_browser_sources_panel_reads_frozen_metadata_record(
    webmcp_page: Page, access_base_url: str
) -> None:
    page = webmcp_page
    edge_id, node_id = first_edge_and_node(page)
    del edge_id
    normalized_id = "philosophy:" + node_id
    page.goto(f"{access_base_url}/?focus={normalized_id}&ui=en", wait_until="domcontentloaded", timeout=30_000)
    wait_for(page, "window.__TOS_E2E.names().includes('tos.page.context')")
    wait_for(page, "window.__TOS_E2E.names().includes('tos.page.select')")
    wait_for(page, "document.querySelector('#sophia-gestures')?.dataset.dataState === 'ready'")
    invoke(page, "tos.page.select", {"item_id": normalized_id})
    wait_for(page, "window.__TOS_E2E.names().includes('tos.page.inspect-selection')")

    node_packet = page.evaluate(
        """async id => (await (await fetch('/api/knowledge/nodes/' + encodeURIComponent(id) + '?relation_limit=0')).json())""",
        normalized_id,
    )
    source_target = node_packet["source_read_targets"][normalized_id]["target"]
    source_record = json.loads(SOURCE_RECORD_PATH.read_text(encoding="utf-8"))
    expected_digest = "sha256:" + _canonical_digest(source_record)
    assert source_target["layer"] == "metadata_record"
    assert source_target["record_type"] == source_record["record_type"] == "work"
    assert source_target["record_ref"] == {
        "id": source_record["record_id"],
        "version": source_record["record_version"],
        "digest": expected_digest,
    }
    assert source_target["content_revision"] == expected_digest

    page.get_by_role("button", name="Открыть источники", exact=True).click()
    with page.expect_response(lambda response: response.url.endswith('/api/source/read')) as delivery:
        page.get_by_role("button", name="Открыть исходную запись", exact=True).click()
    wait_for(page, "document.querySelector('.sc-exact-source')?.dataset.sourceReadStatus === 'available'")

    exact = page.locator(".sc-exact-source")
    for index in range(exact.locator("details").count()):
        exact.locator("details").nth(index).locator("summary").click()
    exact_text = exact.inner_text()
    assert source_record["record_id"] not in exact_text
    assert source_record["preferred_label"] in exact_text
    assert SOURCE_RECORD_RELATIVE.as_posix() not in exact_text
    assert exact.locator('pre').count() == 0
    with page.expect_download() as downloaded:
        exact.get_by_role('button', name='Скачать запись', exact=True).click()
    exported = json.loads(Path(downloaded.value.path()).read_text(encoding='utf-8'))
    envelope = delivery.value.json()
    envelope.pop('handle')
    assert exported.pop('selection') == {
        'kind': 'node', 'id': normalized_id,
        'source_revision': node_packet['source_revision'],
        'content_revision': node_packet['matches'][0]['content_revision'],
    }
    assert exported == envelope
    assert exported['record'] == source_record



@pytest.mark.parametrize("access_base_url", ["source-native-metadata"], indirect=True)
@pytest.mark.parametrize("discovery_failure", ["network", "malformed"])
def test_source_metadata_remains_when_native_discovery_fails(
    webmcp_page: Page, access_base_url: str, discovery_failure: str
) -> None:
    from urllib.parse import quote
    page = webmcp_page
    _, node_id = first_edge_and_node(page)
    page.goto(f"{access_base_url}/static/research.html?focus={quote('philosophy:' + node_id, safe='')}")
    page.locator('#tree[data-ready="true"] .reading [data-source-record-id]').wait_for(state="attached")
    page.locator('.reading details').filter(has=page.locator('[data-source-record-id]')).locator(':scope > summary').click()
    pending = []
    capability_calls = 0

    def capabilities(route):
        nonlocal capability_calls
        capability_calls += 1
        if capability_calls == 1:
            route.continue_()  # The exact metadata read still checks its owner.
        else:
            pending.append(route)  # Hold optional discovery while inspecting metadata.

    page.route("**/api/source/capabilities", capabilities)
    page.locator('.reading [data-source-record-id]').click()
    record = page.locator('dialog[data-kind="source-record"] .dialog-content')
    record.get_by_text('Synthetic native metadata', exact=True).wait_for(state="visible")
    record.locator('details > summary').click()
    assert "Available exact metadata remains readable." in record.inner_text()
    assert len(pending) == 1
    assert record.get_attribute('data-source-read-status') == 'available'
    if discovery_failure == 'network':
        pending[0].abort('failed')
    else:
        pending[0].fulfill(status=200, content_type='application/json', body='{')
    record.get_by_text('Способы чтения текста сейчас недоступны.', exact=True).wait_for(state="visible")
    assert record.get_attribute('data-source-read-status') == 'available'
    assert record.get_by_text('Synthetic native metadata', exact=True).is_visible()
    assert "Available exact metadata remains readable." in record.inner_text()
    assert record.locator('.source-native-actions button').count() == 0
    assert 'tos.text-unit.browser-fixture' not in record.inner_text()
    assert 'ToS/synthetic/native-metadata.json' not in record.inner_text()
    assert record.locator('pre').count() == 0
    with page.expect_download() as downloaded:
        record.get_by_role('button', name='Скачать данные', exact=True).click()
    exported = json.loads(Path(downloaded.value.path()).read_text(encoding='utf-8'))
    assert exported['record']['record_id'] == 'tos.text-unit.browser-fixture'
    assert 'handle' not in exported
    assert 'ToS/synthetic/native-metadata.json' in json.dumps(exported)


@pytest.mark.parametrize("access_base_url", ["source"], indirect=True)
def test_research_seeded_locale_and_panel_sequence(webmcp_page: Page, access_base_url: str) -> None:
    import random
    from urllib.parse import quote

    page = webmcp_page
    _, node_id = first_edge_and_node(page)
    page.goto(f"{access_base_url}/static/research.html?focus={quote('philosophy:' + node_id, safe='')}")
    page.locator('#tree[data-ready="true"] .reading h1').wait_for()
    names = {
        'ru': ['Собрать линзу', 'Читать произведения и издания', 'Моя полка', 'Об этой области', 'Настроить связи', 'Закрыть'],
        'en': ['Build a lens', 'Read works and editions', 'My shelf', 'About this area', 'Choose relations', 'Close'],
    }
    language = 'ru'
    actions = ['en', 'ru', 'sources', 'scope', 'conditions', 'shelf', 'library'] * 3
    random.Random(915236).shuffle(actions)
    for step, action in enumerate(actions):
        if action in names:
            page.locator(f'[data-live-lang="{action}"]').click()
            language = action
        elif action == 'sources':
            disclosure = page.locator('.reading details').filter(has=page.locator('[data-source-record-id]')).locator(':scope > summary')
            disclosure.click()
            assert page.locator('.reading [data-source-record-id]').is_visible(), (step, action)
            assert page.locator('.reading pre').count() == 0
            disclosure.click()
        elif action == 'shelf':
            page.get_by_role('button', name=names[language][2], exact=True).click()
            shelf = page.locator('.research-shelf')
            labels = ['Исследовательская полка', 'Тип', 'Закрыть полку'] if language == 'ru' else ['Research shelf', 'Type', 'Close shelf']
            assert shelf.get_by_role('heading', name=labels[0], exact=True).is_visible(), (step, action)
            assert shelf.get_by_label(labels[1], exact=True).is_visible(), (step, action)
            assert shelf.locator('pre').count() == 0
            shelf.get_by_role('button', name=labels[2], exact=True).click()
        elif action == 'library':
            page.get_by_role('button', name=names[language][1], exact=True).click()
            reader = page.locator('.corpus-reader')
            message = 'Источник полных текстов пока не подключён.' if language == 'ru' else 'The full-text source is not connected yet.'
            reader.get_by_text(message, exact=True).wait_for(state='visible')
            assert reader.get_by_text(message, exact=True).count() == 1
            assert reader.locator('.cr-library,.cr-save').count() == 0
            reader.get_by_role('button', name='Вернуться к Древу' if language == 'ru' else 'Return to the tree', exact=True).click()
        else:
            page.get_by_role('button', name=names[language][3 if action == 'scope' else 4], exact=True).click()
            dialog = page.get_by_role('dialog')
            assert dialog.is_visible(), (step, action)
            assert dialog.locator('pre').count() == 0
            dialog.get_by_role('button', name=names[language][5], exact=True).click()
        for name in names[language][:3]:
            assert page.get_by_role('button', name=name, exact=True).is_visible(), (step, action, name)
    assert page.locator('.reading h1').is_visible()


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


@pytest.mark.parametrize("access_base_url", ["prepared"], indirect=True)
def test_real_browser_prepared_search_preserves_engine_and_selection(webmcp_page: Page) -> None:
    page = webmcp_page
    first = command_value(invoke(page, "tos.page.knowledge-search", {"query": "philosophy", "limit": 1}))
    assert first["schema"] == "tos_knowledge_search_compressed_v3"
    assert first["search_mode"] == "compressed"
    assert len(first["nodes"]) == 1
    page.locator("#detail-list [data-result]").first.click()
    selected = command_value(invoke(page, "tos.page.inspect-selection"))
    assert selected["selection"]["id"] == first["nodes"][0]["id"]
    assert page.locator("#detail-list").inner_text().strip()
    with pytest.raises(playwright.Error, match="mode unavailable: indexed"):
        invoke(page, "tos.page.knowledge-search", {"query": "philosophy", "search_mode": "indexed"})
    assert command_value(invoke(page, "tos.page.inspect-selection"))["selection"] == selected["selection"]
    assert first["next_cursor"]
    second = command_value(invoke(page, "tos.page.knowledge-search", {
        "query": "philosophy", "cursor": first["next_cursor"],
        "search_mode": first["search_mode"], "limit": 1,
    }))
    assert second["schema"] == first["schema"]
    assert second["search_mode"] == first["search_mode"]
    assert second["source_revision"] == first["source_revision"]
    assert not ({node["id"] for node in first["nodes"]} & {node["id"] for node in second["nodes"]})


@pytest.mark.parametrize("access_base_url", ["prepared"], indirect=True)
def test_observatory_prepared_human_agent_search_and_continuation(webmcp_page: Page, access_base_url: str) -> None:
    from urllib.parse import quote
    page = webmcp_page
    initial = command_value(invoke(page, "tos.page.knowledge-search", {"query": "fixture", "limit": 1}))
    page.goto(f"{access_base_url}/?focus={quote(initial['nodes'][0]['id'], safe='')}")
    wait_for(page, "window.__TOS_E2E.names().includes('tos.page.knowledge-search')")
    first = command_value(invoke(page, "tos.page.knowledge-search", {"query": "fixture", "limit": 1}))
    assert first["search_mode"] == "compressed"
    assert first["nodes"][0]["label"]
    page.locator(".sc-search-results .sc-result").first.click()
    wait_for(page, "window.__TOS_E2E.names().includes('tos.page.inspect-selection')")
    assert command_value(invoke(page, "tos.page.inspect-selection"))["selection"]["id"] == first["nodes"][0]["id"]
    assert first["next_cursor"]
    second = command_value(invoke(page, "tos.page.knowledge-search", {
        "query": "fixture", "limit": 1, "search_mode": first["search_mode"], "cursor": first["next_cursor"],
    }))
    assert second["source_revision"] == first["source_revision"]
    assert not ({node["id"] for node in first["nodes"]} & {node["id"] for node in second["nodes"]})
    # Ordinary human typing uses the same client, but owns its own cursor history.
    page.locator("#sc-query").fill("fixture ")
    page.locator(".sc-search-pager").get_by_role("button", name="Далее", exact=True).wait_for(state="visible")
    first_page = page.locator(".sc-search-results .sc-result").all_text_contents()
    page.locator(".sc-search-pager").get_by_role("button", name="Далее", exact=True).click()
    page.locator(".sc-search-pager").get_by_role("button", name="Ранее", exact=True).click()
    page.locator(".sc-search-results .sc-result").first.wait_for(state="visible")
    assert page.locator(".sc-search-results .sc-result").all_text_contents() == first_page


def test_research_visibility_refreshes_links_and_preserves_source_disclosure(webmcp_page: Page, access_base_url: str) -> None:
    """A changed view refreshes navigable links without replacing the reading."""
    from urllib.parse import quote
    page = webmcp_page
    hits = command_value(invoke(page, "tos.page.knowledge-search", {"query": "Alpha", "limit": 10}))
    material_id = next(row['id'] for row in hits['nodes'] if row['id'] == 'philosophy:a')

    def service_catalog(route):
        response = route.fetch()
        catalog = response.json()
        changed = 0
        # Only this synthetic fixture declares its neighboring types service
        # objects. No production corpus or source registry is changed.
        for entry in catalog['semantic_registries']['entity_types']['entries']:
            if any(mapping.get('source_graph') == 'philosophy' and mapping.get('source_kind_id') in ('work', 'source')
                   for mapping in entry.get('source_mappings', [])):
                entry['object_role'] = 'projection'
                changed += 1
        assert changed
        route.fulfill(response=response, json=catalog)

    page.route('**/api/knowledge/catalog', service_catalog)
    page.goto(f"{access_base_url}/static/research.html?focus={quote(material_id, safe='')}")
    reading = page.locator('#tree[data-ready="true"] .reading')
    reading.locator('h1').wait_for(state='visible')
    reading.get_by_text('Источники', exact=True).click()
    opened_source = reading.locator('details[open]').first
    assert opened_source.is_visible()
    page.locator('[data-show-service-objects]').click()
    reading.locator('.related-materials [data-related-node-id]').first.wait_for(state='visible')
    assert opened_source.is_visible()
    assert not page.get_by_text('Не удалось выполнить действие. Повторите попытку.', exact=True).is_visible()
    reading.locator('.related-relation').first.click()
    reading.locator('.relation-participants').wait_for(state='visible')
    selection = page.locator('#tree').get_attribute('data-selection')
    page.locator('[data-live-projection="compact"]').click()
    assert page.locator('#tree').get_attribute('data-selection') == selection
    assert reading.locator('.relation-participants [data-related-node-id]').count() == 2
    reading.locator('.relation-participants [data-related-node-id]').first.click()
    reading.locator('h1').wait_for(state='visible')
    page.locator('[data-live-projection="grouped"]').click()
    reading.locator('.related-materials [data-related-node-id]').first.wait_for(state='visible')


def test_built_research_entry_persists_exact_shelf_and_camera(webmcp_page: Page, access_base_url: str) -> None:
    """Use the shipped entry and default browser storage, with a small dataset."""
    from urllib.parse import quote
    page = webmcp_page
    hits = command_value(invoke(page, "tos.page.knowledge-search", {"query": "fixture", "limit": 1}))
    material_id = hits["nodes"][0]["id"]
    page.goto(f"{access_base_url}/static/research.html?focus={quote(material_id, safe='')}")
    reading = page.locator('#tree[data-ready="true"] .reading')
    reading.get_by_role("button", name="Сохранить материал", exact=True).click()
    # A named neighbor is a navigation action; the predicate has its own action.
    neighbor = reading.locator('.related-materials [data-related-node-id]').first
    neighbor.wait_for(state='visible')
    neighbor_id = neighbor.get_attribute('data-related-node-id')
    neighbor.click()
    wait_for(page, f"document.querySelector('#tree').dataset.selection === {json.dumps(neighbor_id)}")
    reading.locator('.related-relation').first.click()
    endpoint = reading.locator('.relation-participants [data-related-node-id]').first
    endpoint.wait_for(state='visible')
    endpoint_id = endpoint.get_attribute('data-related-node-id')
    endpoint.click()
    wait_for(page, f"document.querySelector('#tree').dataset.selection === {json.dumps(endpoint_id)}")
    reading.get_by_role('button', name='Сравнить', exact=True).click()
    assert reading.get_by_role('button', name='Добавлено к сравнению', exact=True).is_disabled()
    assert page.locator('[data-live-action="compare"]').inner_text() == 'Сопоставить (1/2)'
    # Dismissing an overlay preserves the selected material underneath it.
    selected_title = reading.locator('h1').inner_text()
    page.locator('[data-live-action="compare"]').click()
    page.locator('dialog[open]').wait_for(state='visible')
    page.keyboard.press('Escape')
    page.locator('dialog[open]').wait_for(state='hidden')
    assert reading.is_visible()
    assert reading.locator('h1').inner_text() == selected_title
    page.locator('.search-trigger').click()
    page.locator('.search').fill('fixture')
    page.keyboard.press('Escape')
    assert not page.locator('.materials-panel').is_visible()
    assert reading.is_visible()
    assert reading.locator('h1').inner_text() == selected_title

    page.locator('[data-research-shelf="true"]').click()
    card = page.locator('.research-shelf-card').first
    card.wait_for(state="visible")
    record_id = card.get_attribute('data-record-id')
    assert record_id
    assert "работает в памяти" not in page.locator('.research-shelf').inner_text()
    page.reload()
    page.locator('[data-research-shelf="true"]').click()
    page.locator(f'.research-shelf-card[data-record-id="{record_id}"]').wait_for(state="visible")
    page.get_by_role('button', name='Закрыть полку', exact=True).click()
    page.locator('.reading').get_by_role('button', name='Закрыть', exact=True).click()
    page.locator('[data-live-action="motion"]').click()
    before = page.locator('#tree').get_attribute('data-camera')
    page.mouse.move(600, 380)
    page.mouse.wheel(0, 220)
    wait_for(page, f"document.querySelector('#tree').dataset.camera !== {json.dumps(before)}")
    # View writes are deliberately debounced; this waits for the visible input
    # gesture to be persisted, not for a mocked storage adapter.
    page.wait_for_timeout(750)
    pose = page.evaluate("""async () => {
      const db = await new Promise((resolve, reject) => {
        const request = indexedDB.open('tos-real-ui-view-v1');
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
      });
      const saved = await new Promise((resolve, reject) => {
        const tx = db.transaction('views', 'readonly');
        const request = tx.objectStore('views').get('constructor-live');
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
      });
      db.close();return saved.presentation.pose;
    }""")
    assert pose['zoom'] != float(before.split(',')[2])
    page.reload()
    page.locator('#tree[data-ready="true"] .reading').get_by_role("button", name="Сохранить материал", exact=True).wait_for(state="visible")
    restored_pose = page.locator('#tree').get_attribute('data-camera')
    assert [float(part) for part in restored_pose.split(',')] == pytest.approx(
        [pose['yaw'], pose['pitch'], pose['zoom'], *pose['pan']], abs=0.002
    )
    page.set_viewport_size({"width": 390, "height": 844})
    page.locator('.research-shelf-narrow').click()
    assert page.evaluate('document.documentElement.scrollWidth <= innerWidth')
    page.locator(f'.research-shelf-card[data-record-id="{record_id}"]').wait_for(state="visible")
    page.get_by_role('button', name='Закрыть полку', exact=True).click()
    assert page.locator('.research-shelf-narrow').evaluate('(node) => node === document.activeElement')


def test_research_catalog_limit_explains_failed_connection_and_can_retry(
    webmcp_page: Page, access_base_url: str
) -> None:
    page = webmcp_page
    unavailable = {'value': True}
    def catalog_response(route):
        if unavailable['value']:
            route.fulfill(status=413, content_type='application/json', body='{}')
        else:
            route.continue_()
    page.route('**/api/knowledge/catalog', catalog_response)
    page.goto(access_base_url + '/static/research.html', wait_until='domcontentloaded')
    connection = page.locator('dialog[open][data-kind="connection"]')
    connection.get_by_text('Словарь данных слишком велик для загрузки. Обратитесь к оператору сервиса.', exact=True).wait_for(state='visible')
    assert 'Выберите более узкий центр' not in connection.inner_text()
    unavailable['value'] = False
    connection.get_by_role('button', name='Повторить подключение', exact=True).click()
    page.locator('#tree[data-ready="true"]').wait_for(state='visible')
    page.locator('dialog[data-kind="connection"]').wait_for(state='hidden')


def test_observatory_research_and_saved_route_links_use_packaged_entry(
    webmcp_page: Page, access_base_url: str
) -> None:
    """Both ordinary entry links must use the installed HTTP static route."""
    from urllib.parse import parse_qs, quote, urlparse
    page = webmcp_page
    hits = command_value(invoke(page, "tos.page.knowledge-search", {"query": "fixture", "limit": 1}))
    material_id = hits["nodes"][0]["id"]
    observatory_url = f"{access_base_url}/?focus={quote(material_id, safe='')}"
    page.goto(observatory_url, wait_until='domcontentloaded')
    page.locator('#sophia-gestures[data-data-state="ready"]').wait_for(state='visible')
    with page.expect_navigation(wait_until='domcontentloaded') as entry:
        page.get_by_role('link', name='Пространство исследования', exact=True).click()
    assert entry.value.status == 200
    assert urlparse(entry.value.url).path == '/static/research.html'
    page.locator('#tree[data-ready="true"]').wait_for(state='visible')

    # Save an actual bounded area through the constructor, then reopen its
    # IndexedDB shelf entry from Observatory at the ordinary packaged root.
    page.goto(f"{access_base_url}/static/research.html?focus={quote(material_id, safe='')}")
    page.locator('#tree[data-ready="true"] .reading [data-source-record-id]').wait_for(state='attached')
    page.locator('[data-live-action="scope"]').click()
    page.get_by_role('button', name='Сохранить эту область', exact=True).click()
    page.get_by_text('Область сохранена на вашей полке.', exact=True).wait_for(state='visible')
    page.locator('dialog[data-kind="scope"]').get_by_role('button', name='Закрыть', exact=True).click()
    page.locator('[data-research-shelf="true"]').click()
    card = page.locator('.research-shelf-card').first
    card.wait_for(state='visible')
    record_id = card.get_attribute('data-record-id')
    assert record_id
    page.get_by_role('button', name='Закрыть полку', exact=True).click()

    page.goto(observatory_url, wait_until='domcontentloaded')
    page.locator('#sophia-gestures[data-data-state="ready"]').wait_for(state='visible')
    page.locator('[data-research-shelf="true"]').click()
    stored = page.locator(f'.research-shelf-card[data-record-id="{record_id}"]')
    with page.expect_navigation(wait_until='domcontentloaded') as route:
        stored.get_by_role('button', name='Открыть', exact=True).click()
    assert route.value.status == 200
    assert urlparse(route.value.url).path == '/static/research.html'
    assert parse_qs(urlparse(route.value.url).query)['shelfRoute'] == [record_id]
    page.locator('#tree[data-ready="true"] .reading [data-source-record-id]').wait_for(state='attached')
    assert page.locator('#tree').get_attribute('data-selection') == material_id


def test_research_lens_preview_save_apply_and_return(webmcp_page: Page, access_base_url: str) -> None:
    """A saved query reopens through the real host and never borrows a cursor."""
    from urllib.parse import quote, urlparse
    page = webmcp_page
    hits = command_value(invoke(page, "tos.page.knowledge-search", {"query": "fixture", "limit": 1}))
    material_id = hits["nodes"][0]["id"]
    discovery_requests = []
    page.on('request', lambda request: discovery_requests.append(urlparse(request.url).path)
            if urlparse(request.url).path in ('/api/knowledge/catalog', '/api/knowledge/contracts') else None)
    page.goto(f"{access_base_url}/static/research.html?focus={quote(material_id, safe='')}")
    tree = page.locator('#tree[data-ready="true"][data-loading="false"]')
    tree.wait_for(state="visible")
    wait_for(page, f"document.querySelector('#tree').dataset.selection === {json.dumps(material_id)}")
    before_selection = tree.get_attribute('data-selection')
    before_camera = tree.get_attribute('data-camera')
    assert discovery_requests == ['/api/knowledge/catalog']
    page.get_by_role('button', name='Собрать линзу', exact=True).click()
    builder = page.locator('.lens-builder')
    page.locator('.lens-builder[data-state="ready"]').wait_for(state='visible')
    # Every ordinary reopening uses the same immutable session vocabulary and
    # validated schema. A page reload below must establish a new binding.
    for _ in range(2):
        builder.locator('.lens-builder-header .lens-builder-close').click()
        page.get_by_role('button', name='Собрать линзу', exact=True).click()
        page.locator('.lens-builder[data-state="ready"]').wait_for(state='visible')
    assert discovery_requests == ['/api/knowledge/catalog', '/api/knowledge/contracts']
    builder.get_by_label('Название линзы', exact=True).fill('Fixture saved lens')
    builder.get_by_role('button', name='Предпросмотр', exact=True).click()
    page.locator('.lens-builder[data-state="preview"]').wait_for(state="visible")
    assert builder.get_by_role('button', name='Открыть область', exact=True).is_enabled()
    assert tree.get_attribute('data-selection') == before_selection
    assert tree.get_attribute('data-camera') == before_camera
    builder.get_by_role('button', name='Сохранить линзу', exact=True).click()
    builder.locator('[data-state="saved"]').wait_for(state="visible")
    builder.get_by_role('button', name='Открыть область', exact=True).click()
    page.locator('#tree[data-history="1"]').wait_for(state="visible")
    page.locator('[data-live-action="back"]').click()
    page.locator('#tree[data-history="0"][data-loading="false"]').wait_for(state="visible")
    assert tree.get_attribute('data-selection') == before_selection
    assert tree.get_attribute('data-camera') == before_camera
    page.reload()
    page.locator('#tree[data-ready="true"][data-loading="false"]').wait_for(state="visible")
    assert tree.get_attribute('data-selection') == before_selection
    page.get_by_role('button', name='Моя полка', exact=True).click()
    card = page.locator('.research-shelf-card').filter(has_text='Fixture saved lens')
    card.get_by_role('button', name='Открыть', exact=True).click()
    page.locator('.lens-builder[data-state="ready"]').wait_for(state="visible")
    assert discovery_requests == ['/api/knowledge/catalog', '/api/knowledge/contracts'] * 2
    assert builder.get_by_label('Название линзы', exact=True).input_value() == 'Fixture saved lens'
    assert not builder.get_by_role('button', name='Открыть область', exact=True).is_enabled()
    builder.get_by_role('button', name='Предпросмотр', exact=True).click()
    page.locator('.lens-builder[data-state="preview"]').wait_for(state="visible")
    assert builder.get_by_role('button', name='Открыть область', exact=True).is_enabled()


def test_real_browser_graceful_without_webmcp(access_base_url: str) -> None:
    with short_chromium_tmp():
        with sync_playwright() as p:
            launch_options = {"headless": True, "args": ["--no-sandbox"]}
            if CHROMIUM:
                launch_options["executable_path"] = CHROMIUM
            browser = p.chromium.launch(**launch_options)
            context = browser.new_context(locale="en-US")
            page = context.new_page()
            page.goto(f"{access_base_url}/?mode=philosophy&view=chronology&ui=en", wait_until="domcontentloaded", timeout=30_000)
            wait_for(page, "Boolean(document.getElementById('current-view-title')?.textContent)")
            page.wait_for_timeout(500)
            assert page.evaluate("document.modelContext === undefined")
            assert page.locator("#app").count() == 1
            assert page.locator("#agent-surface").get_attribute("data-webmcp-state") == "unavailable"
            fallback = page.locator("#agent-surface").inner_text()
            assert "Codex WebMCP unavailable" in fallback
            assert "no API key or model API is required" in fallback
            assert "Optional off-page access" in fallback
            assert "Tree of Sophia" in page.locator("body").inner_text()
            context.close()
            browser.close()


def test_corpus_note_exit_recovers_exact_draft():
    """Real IDB: teardown flushes, and an interrupted write survives reload."""
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        port = probe.getsockname()[1]
    web = REPO_ROOT / 'access/web'
    server = subprocess.Popen(
        [str(web / 'node_modules/.bin/vite'), '--host', '127.0.0.1',
         '--port', str(port), '--strictPort'], cwd=web,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    url = f'http://127.0.0.1:{port}/static/fixtures/corpus-reader.html'
    read_notes = """async () => {
      const db=await new Promise((resolve,reject)=>{
        const request=indexedDB.open('tos-corpus-reader-fixture-v1');
        request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);
      });
      try{return await new Promise((resolve,reject)=>{
        const request=db.transaction('notes','readonly').objectStore('notes').getAll();
        request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);
      });}finally{db.close();}
    }"""
    try:
        deadline = time.monotonic() + 15
        while True:
            try:
                with urllib.request.urlopen(url, timeout=1) as response:
                    assert response.status == 200
                break
            except (OSError, AssertionError):
                if server.poll() is not None or time.monotonic() >= deadline:
                    raise
                time.sleep(0.1)
        with short_chromium_tmp(), sync_playwright() as p:
            options = {'headless': True, 'args': ['--no-sandbox']}
            if CHROMIUM:
                options['executable_path'] = CHROMIUM
            browser = p.chromium.launch(**options)
            for interrupted in (False, True):
                context = browser.new_context(locale='ru-RU')
                page = context.new_page()
                page.on('dialog', lambda dialog: dialog.accept())
                page.goto(url, wait_until='domcontentloaded')
                page.wait_for_selector('#tree[data-fixture-ready="true"]')
                page.locator('[data-corpus-open]').click()
                page.locator('.cr-unit:not(.cr-heading) .cr-unit-marker').first.click()
                page.locator('.cr-note-editor').wait_for()
                unit_id = page.locator('.cr-unit:not(.cr-heading)').first.get_attribute('data-unit-id')
                text = 'Незавершённая заметка 😀 α — ' + str(interrupted)
                # One JS task guarantees exit before the 450 ms debounce.
                if interrupted:
                    with page.expect_navigation(wait_until='domcontentloaded'):
                        page.evaluate("""text => {
                          const transaction=IDBDatabase.prototype.transaction;
                          IDBDatabase.prototype.transaction=function(names,mode,...rest){
                            if(mode==='readwrite')throw new DOMException('Interrupted test write','AbortError');
                            return transaction.call(this,names,mode,...rest);
                          };
                          const editor=document.querySelector('.cr-note-editor');
                          editor.value=text;editor.dispatchEvent(new Event('input',{bubbles:true}));
                          location.reload();
                        }""", text)
                else:
                    page.evaluate("""text => {
                      const editor=document.querySelector('.cr-note-editor');
                      editor.value=text;editor.dispatchEvent(new Event('input',{bubbles:true}));
                      window.dispatchEvent(new PageTransitionEvent('pagehide',{persisted:false}));
                    }""", text)
                page.wait_for_function(
                    'async text => (await (' + read_notes + ')()).some(note=>note.text===text)',
                    arg=text, timeout=15_000,
                )
                notes = page.evaluate(read_notes)
                assert len(notes) == 1
                assert notes[0]['reference']['unitId'] == unit_id
                if interrupted:
                    assert notes[0]['id'].startswith('recovered-')
                    page.reload(wait_until='domcontentloaded')
                    page.wait_for_selector('#tree[data-fixture-ready="true"]')
                    assert len(page.evaluate(read_notes)) == 1
                    assert page.evaluate(
                        "sessionStorage.getItem('tos.corpus.note-draft.v1:tos-corpus-reader-fixture-v1')"
                    ) is None
                context.close()
            browser.close()
    finally:
        server.terminate()
        try:
            server.wait(timeout=5)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait(timeout=5)


@pytest.fixture(scope='session')
def reader_fixture_base_url():
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        port = probe.getsockname()[1]
    web = REPO_ROOT / 'access/web'
    server = subprocess.Popen(
        [str(web / 'node_modules/.bin/vite'), '--host', '127.0.0.1', '--port', str(port), '--strictPort'],
        cwd=web, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    base_url = f'http://127.0.0.1:{port}'
    try:
        deadline = time.monotonic() + 15
        while True:
            try:
                with urllib.request.urlopen(base_url + '/static/fixtures/research-shelf.html', timeout=1) as response:
                    assert response.status == 200
                break
            except (OSError, AssertionError):
                if server.poll() is not None or time.monotonic() >= deadline:
                    raise
                time.sleep(0.1)
        yield base_url
    finally:
        server.terminate()
        try:
            server.wait(timeout=5)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait(timeout=5)


def test_research_shelf_collection_delete_is_atomic(reader_fixture_base_url):
    """Real IDB: two-connection CAS and rollback of a partially visited collection."""
    with short_chromium_tmp(), sync_playwright() as p:
        options = {'headless': True, 'args': ['--no-sandbox']}
        if CHROMIUM:
            options['executable_path'] = CHROMIUM
        browser = p.chromium.launch(**options)
        page = browser.new_page(locale='ru-RU')
        page.goto(reader_fixture_base_url + '/static/fixtures/research-shelf.html', wait_until='domcontentloaded')
        proof = page.locator('#research-shelf-proof[data-state]')
        proof.wait_for(state='visible')
        value = json.loads(proof.inner_text())
        assert proof.get_attribute('data-state') == 'passed', value
        assert value['tests']['collectionDetach']['ok'], value
        assert value['tests']['collectionDeleteRollback']['ok'], value
        assert all(test['ok'] for test in value['tests'].values()), value
        browser.close()


def test_lens_large_relation_area_requires_scope_confirmation(reader_fixture_base_url):
    """A sole all-tree option stays explicit and can actually be confirmed."""
    with short_chromium_tmp(), sync_playwright() as p:
        options = {'headless': True, 'args': ['--no-sandbox']}
        if CHROMIUM:
            options['executable_path'] = CHROMIUM
        browser = p.chromium.launch(**options)
        page = browser.new_page(locale='ru-RU')
        page.route('**/lens-scope-check', lambda route: route.fulfill(
            status=200, content_type='text/html',
            body='<html lang="ru"><body><main id="scope-check"></main></body></html>',
        ))
        page.goto(reader_fixture_base_url + '/lens-scope-check')
        page.evaluate("""async () => {
          const {mountLensBuilder}=await import('/static/src/observatory/lens-builder.mjs');
          const {lensContext,boundary}=await import('/static/fixtures/lens-scenarios.mjs');
          const context=lensContext(),nodes=Array.from({length:41},(_,i)=>({id:'fixture:'+i}));
          const selection={kind:'relation',id:'fixture:relation'};
          const packet={schema:'tos_browser_exploration_view_v1',source_revision:context.catalog.source_revision,
            authority_boundary:boundary,nodes,relations:[{id:selection.id,from_id:nodes[0].id,to_id:nodes[1].id}],selection};
          window.scopeCheck={packet,compiles:0};
          window.scopeCheck.builder=mountLensBuilder({host:document.querySelector('#scope-check'),
            getArea:()=>({packet,selection}),client:{
              request:async path=>path==='/catalog'?context.catalog:{schema:'tos_knowledge_contract_bundle_v1',
                authority_boundary:boundary,contracts:{lens_spec:context.schema}},
              compile:async()=>{window.scopeCheck.compiles++;throw Error('Preview must stay explicit');}
            }});
          await window.scopeCheck.builder.open();
        }""")
        builder = page.locator('#scope-check .lens-builder')
        choice = builder.locator('.lens-builder-area-choice')
        assert choice.locator('option').count() == 1
        assert choice.locator('select').input_value() == 'all'
        assert page.evaluate('scopeCheck.builder.state().areaChoice.required')
        assert builder.get_by_role('button', name='Предпросмотр', exact=True).is_disabled()
        choice.get_by_role('button', name='Открыть область', exact=True).click()
        choice.wait_for(state='hidden')
        assert not page.evaluate('scopeCheck.builder.state().areaChoice.required')
        assert page.evaluate('scopeCheck.compiles') == 0
        assert page.evaluate('scopeCheck.packet.nodes.length') == 41
        assert builder.get_by_role('button', name='Предпросмотр', exact=True).is_enabled()
        assert builder.get_by_role('button', name='Сохранить линзу', exact=True).is_enabled()
        # Reopen the mounted control around a selected node. The label must
        # resolve from the supplied area without depending on a scene object.
        page.evaluate("""async () => {
          scopeCheck.builder.close();
          scopeCheck.packet.selection.kind='node';
          scopeCheck.packet.selection.id='fixture:0';
          scopeCheck.packet.nodes[0].display={title:{ru:'Выбранный центр'}};
          await scopeCheck.builder.open();
        }""")
        assert choice.locator('select').input_value() == 'focus'
        choice.get_by_role('button', name='Открыть область', exact=True).click()
        assert 'Выбранный центр' in builder.inner_text()
        assert builder.get_by_role('button', name='Предпросмотр', exact=True).is_enabled()
        # In a normal bounded area, choosing focus uses the current selection
        # immediately; it must not require a second corrective click.
        page.evaluate('''async () => {
          scopeCheck.builder.close();scopeCheck.packet.nodes.splice(2);
          await scopeCheck.builder.open();
        }''')
        builder.locator('.lens-builder-field').filter(has=page.get_by_text('Отправная точка', exact=True)).locator('select').select_option('focus')
        assert 'Выбранный центр' in builder.locator('.lens-builder-focus').inner_text()
        assert builder.get_by_role('button', name='Взять выбранную звезду', exact=True).count() == 0
        browser.close()


def test_native_delivery_export_preserves_identity_and_respects_expiry(reader_fixture_base_url):
    """The real download carries exact text identity and dies with its delivery."""
    with short_chromium_tmp(), sync_playwright() as p:
        options = {'headless': True, 'args': ['--no-sandbox']}
        if CHROMIUM:
            options['executable_path'] = CHROMIUM
        browser = p.chromium.launch(**options)
        page = browser.new_page(locale='ru-RU')
        page.goto(reader_fixture_base_url + '/static/fixtures/native-reader.html')
        page.get_by_role('button', name='Открыть текст', exact=True).click()
        reader = page.locator('.native-reader[data-native-state="available"]')
        reader.wait_for()
        reader.get_by_text('Об источнике', exact=True).click()
        with page.expect_download() as download:
            reader.get_by_role('button', name='Скачать данные', exact=True).click()
        exported = json.loads(Path(download.value.path()).read_text(encoding='utf-8'))
        assert 'handle' not in exported
        assert exported['record_ref']['id'] == 'tos.text-unit.fixture'
        assert exported['native_unit']['packet']['version'] == 1
        assert exported['native_unit']['spans'][0]['text'].startswith('A😀e\u0301')
        assert exported['text_access']['scope'] == 'public-native-unit'
        assert exported['selection']['id'] == 'source-claims:identity:tos.text-unit.fixture'
        page.evaluate("""() => {
          window.oldExport=document.querySelector('.nr-exact .sc-data-download');
          window.exportAttempts=0;
          URL.createObjectURL=()=>{exportAttempts++;return 'blob:blocked';};
        }""")
        reader.get_by_role('button', name='К Древу', exact=True).click()
        page.locator('.native-reader').wait_for(state='hidden')
        assert page.evaluate('oldExport.click();exportAttempts') == 0
        page.get_by_role('button', name='Локальные условия', exact=True).click()
        page.clock.install()
        page.get_by_role('button', name='Открыть текст', exact=True).click()
        reader.wait_for()
        assert page.evaluate('oldExport.click();exportAttempts') == 0
        page.evaluate("window.localExport=document.querySelector('.nr-exact .sc-data-download')")
        # Move wall time without firing the expiry timer: click-time validity
        # must reject the delivery even when the browser delays its timer.
        page.clock.set_system_time(page.evaluate('Date.now()') + 91_000)
        assert page.evaluate('localExport.click();exportAttempts') == 0
        page.clock.fast_forward(91_000)
        page.locator('.native-reader[data-native-state="expired"]').wait_for()
        assert page.locator('.nr-exact .sc-data-download').count() == 0
        assert page.evaluate('localExport.click();exportAttempts') == 0
        browser.close()


def test_reader_positions_survive_pagehide_before_debounce():
    """A pagehide flush captures corpus and native positions before teardown."""
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        port = probe.getsockname()[1]
    web = REPO_ROOT / 'access/web'
    server = subprocess.Popen(
        [str(web / 'node_modules/.bin/vite'), '--host', '127.0.0.1',
         '--port', str(port), '--strictPort'], cwd=web,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    corpus_url = f'http://127.0.0.1:{port}/static/fixtures/corpus-reader.html'
    native_url = f'http://127.0.0.1:{port}/static/fixtures/native-reader.html'
    corpus_db = 'tos-corpus-reader-fixture-v1'
    native_db = 'tos-native-reader-validation-v1'
    try:
        deadline = time.monotonic() + 15
        while True:
            try:
                with urllib.request.urlopen(corpus_url, timeout=1) as response:
                    assert response.status == 200
                break
            except (OSError, AssertionError):
                if server.poll() is not None or time.monotonic() >= deadline:
                    raise
                time.sleep(0.1)
        with short_chromium_tmp(), sync_playwright() as p:
            options = {'headless': True, 'args': ['--no-sandbox']}
            if CHROMIUM:
                options['executable_path'] = CHROMIUM
            browser = p.chromium.launch(**options)
            context = browser.new_context(locale='ru-RU')

            corpus = context.new_page()
            corpus.goto(corpus_url, wait_until='domcontentloaded')
            corpus.wait_for_selector('#tree[data-fixture-ready="true"]')
            corpus.locator('[data-corpus-open]').click()
            corpus.locator('.cr-pane').wait_for()
            # Scroll and pagehide share one browser task, so the 550 ms
            # debounce cannot run between the observed position and teardown.
            corpus_position = corpus.evaluate("""() => {
              const pane = document.querySelector('.cr-pane');
              const maximum = pane.scrollHeight - pane.clientHeight;
              const requested = Math.min(maximum, Math.max(640, Math.floor(maximum * .45)));
              pane.scrollTop = requested;
              pane.dispatchEvent(new Event('scroll'));
              const paneRect = pane.getBoundingClientRect();
              const row = [...pane.querySelectorAll('.cr-unit')].find(item =>
                item.getBoundingClientRect().bottom > paneRect.top + 18);
              window.dispatchEvent(new PageTransitionEvent('pagehide', {persisted: false}));
              history.replaceState(history.state, '', location.pathname + location.search);
              return {top: pane.scrollTop, unitId: row?.dataset.unitId, versionId: pane.dataset.versionId};
            }""")
            assert corpus_position['top'] > 0
            assert corpus_position['unitId']
            corpus_readings = wait_for_idb_reading(
                corpus, corpus_db,
                lambda item: item.get('reference', {}).get('unitId') == corpus_position['unitId'],
            )
            corpus_saved = next(
                item for item in corpus_readings
                if item.get('reference', {}).get('unitId') == corpus_position['unitId']
            )
            assert corpus_saved['versionId'] == corpus_position['versionId']
            assert corpus_saved['reference']['target']['workId']

            # Reopen without the old route hash. The exact persisted unit is
            # used as the resume address and is brought to the pane start.
            corpus.reload(wait_until='domcontentloaded')
            corpus.wait_for_selector('#tree[data-fixture-ready="true"]')
            corpus.locator('[data-corpus-open]').click()
            corpus.locator('.cr-pane').wait_for()
            corpus.wait_for_function(
                """unitId => {
                  const pane = document.querySelector('.cr-pane');
                  const row = pane && pane.querySelector(`.cr-unit[data-unit-id="${CSS.escape(unitId)}"]`);
                  if (!row || !pane) return false;
                  const paneRect = pane.getBoundingClientRect();
                  const rowRect = row.getBoundingClientRect();
                  return rowRect.bottom > paneRect.top + 18 && rowRect.top <= paneRect.top + 28;
                }""", arg=corpus_position['unitId'],
            )

            native = context.new_page()
            native.goto(native_url, wait_until='domcontentloaded')
            native.get_by_role('button', name='Открыть текст', exact=True).click()
            native.locator('.native-reader:not([hidden])').wait_for()
            native.locator('.nr-article').wait_for()
            # The native 350 ms debounce also must not run between scroll and
            # the synthetic pagehide that asks the reader to flush.
            native_position = native.evaluate("""() => {
              const article = document.querySelector('.nr-article');
              article.scrollTop = Math.min(420, article.scrollHeight - article.clientHeight);
              article.dispatchEvent(new Event('scroll'));
              window.dispatchEvent(new PageTransitionEvent('pagehide', {persisted: false}));
              return {top: article.scrollTop};
            }""")
            assert native_position['top'] > 0
            native_readings = wait_for_idb_reading(
                native, native_db,
                lambda item: item.get('offset') == native_position['top'],
            )
            native_saved = next(item for item in native_readings if item.get('offset') == native_position['top'])
            assert native_saved['reference']['schemaVersion'] == 'tos.corpus.reader.native-reference.v1'

            native.evaluate("history.replaceState(history.state, '', location.pathname + location.search)")
            native.reload(wait_until='domcontentloaded')
            native.get_by_role('button', name='Открыть текст', exact=True).click()
            native.locator('.native-reader[data-native-state="available"]').wait_for()
            native.wait_for_function(
                """expected => Math.abs(document.querySelector('.nr-article').scrollTop - expected) < 1""",
                arg=native_position['top'],
            )
            context.close()
            browser.close()
    finally:
        server.terminate()
        try:
            server.wait(timeout=5)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait(timeout=5)
