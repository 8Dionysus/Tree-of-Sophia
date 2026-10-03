#!/usr/bin/env python3
"""Compare public D1 Worker packets with the independent ToS Python core."""

from __future__ import annotations

import hashlib
import json
import os
import re
import signal
import socket
import stat
import subprocess
import tempfile
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Callable


WORKER_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = Path(__file__).resolve().parents[4]
SELECTED_SOURCE_ROOT = Path(os.environ.get("TOS_VERIFY_SOURCE_ROOT", REPO_ROOT.as_posix()))
SOURCE_ROOT = SELECTED_SOURCE_ROOT.resolve()
ACCESS_SRC = REPO_ROOT / "access" / "src"
if ACCESS_SRC.as_posix() not in sys.path:
    sys.path.insert(0, ACCESS_SRC.as_posix())

from tos_access.core import ReferenceToSAccessCore as ToSAccessCore  # noqa: E402
from tos_access.projection_store import ProjectionReader, is_partitioned  # noqa: E402


# The fixed public-build closure; only the ledger directory contributes
# variable names. Optional inputs must appear with explicit null bindings.
SOURCE_INPUTS = (
    "ToS/derived-exports/tos_corpus_index.min.json",
    "ToS/derived-exports/philosophy_graph_projection.min.json",
    "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json",
    "access/contracts/knowledge-api.v1.json",
    "access/contracts/knowledge-graph.v1.schema.json",
    "access/contracts/knowledge-search-indexed.v2.schema.json",
    "access/contracts/readable-context.v1.schema.json",
    "access/contracts/lens-spec.v1.schema.json",
    "access/contracts/lens-result.v1.schema.json",
    "access/contracts/temporal-comparison-request.v1.schema.json",
    "access/contracts/temporal-comparison-result.v1.schema.json",
    "access/contracts/source-read.v1.schema.json",
    "access/contracts/exploration-request.v1.schema.json",
    "access/contracts/exploration-result.v1.schema.json",
    "access/contracts/exploration-request.v2.schema.json",
    "access/contracts/exploration-result.v2.schema.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
)
OPTIONAL_INPUTS = (
    "ToS/derived-exports/epistemic_evidence_projection.min.json",
    "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json",
)
LEDGER_PREFIX = "ToS/source-witnesses/access-requests/public-ledger/"
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
MAX_MANIFEST_BYTES = 2 * 1024 * 1024 + 1
# Each of the two verification passes uses the public build's existing work cap.
MAX_SOURCE_PASS_BYTES = 16 * 1024 * 1024 * 1024
MAX_PART_FILE_BYTES = 8 * 1024 * 1024 + 65536
# This verifier holds one ordered path list. ProjectionReader also retains its
# seen paths and up to 64 ancestor child maps during descriptor traversal;
# each index is format-bounded to 128 KiB before parsing. No graph is loaded.
MAX_PART_PATHS = 32768
MAX_PART_PATH_BYTES = 16 * 1024 * 1024
VERIFY_PROFILES = ("production", "representative")


def remaining(deadline: float) -> float:
    seconds = deadline - time.monotonic()
    if seconds <= 0:
        raise TimeoutError("public D1 verification deadline exceeded")
    return seconds


def completion_pair() -> tuple[dict[str, Any], bytes]:
    markers = (WORKER_ROOT / "runtime/manifest.json", WORKER_ROOT / "dist/__edge/build-manifest.json")
    packets = []
    for marker in markers:
        if marker.is_symlink() or not marker.is_file() or marker.stat().st_size > MAX_MANIFEST_BYTES:
            raise AssertionError(f"public D1 completion marker missing or oversized: {marker}")
        with marker.open("rb") as stream:
            packets.append(stream.read(MAX_MANIFEST_BYTES + 1))
    if packets[0] != packets[1] or len(packets[0]) > MAX_MANIFEST_BYTES:
        raise AssertionError("public D1 runtime/static completion markers differ")
    manifest = json.loads(packets[0])
    if (not isinstance(manifest, dict) or manifest.get("schema") != "tos_cloudflare_edge_build_v1"
            or manifest.get("read_model_schema") != "tos_cloudflare_edge_read_model_v9"
            or not isinstance(manifest.get("data_revision"), str)
            or not SHA256.fullmatch(manifest["data_revision"])):
        raise AssertionError("public D1 v9 completion marker is invalid")
    for name, field in (("read-model.sql", "sql_bytes"), ("read-model.rows.json", "baseline_bytes")):
        path = WORKER_ROOT / "runtime" / name
        size = manifest.get(field)
        if (path.is_symlink() or not path.is_file() or type(size) is not int
                or size < 1 or path.stat().st_size != size):
            raise AssertionError(f"public D1 {name} differs from completion marker")
    with (WORKER_ROOT / "runtime/read-model.rows.json").open("rb") as stream:
        prefix = stream.read(160)
    expected_prefix = (b'{"schema":"tos_cloudflare_edge_read_model_v9","revision":"'
                       + manifest["data_revision"].encode("ascii") + b'"')
    if not prefix.startswith(expected_prefix):
        raise AssertionError("public D1 row baseline revision differs from completion marker")
    return manifest, packets[0]


def source_path(label: str) -> Path:
    root = SOURCE_ROOT
    path = root / label
    if (not path.resolve(strict=False).is_relative_to(root)
            or any(parent.is_symlink() for parent in (path, *path.parents) if parent != root and parent.is_relative_to(root))):
        raise AssertionError(f"public D1 source path escaped or became a symlink: {label}")
    return path


def source_digest(path: Path, size: int | None, work: list[int], deadline: float,
                  max_size: int | None = None) -> tuple[int, str]:
    remaining(deadline)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        before = os.fstat(fd)
        if (not stat.S_ISREG(before.st_mode) or (size is not None and before.st_size != size)
                or (max_size is not None and before.st_size > max_size)):
            raise AssertionError(f"public D1 source type or size changed: {path}")
        # Read at most the selected length plus one sentinel byte. Admit that
        # whole physical read before allocating or hashing any part of it.
        work[0] += before.st_size + 1
        if work[0] > MAX_SOURCE_PASS_BYTES:
            raise AssertionError("public D1 source verification work exceeded bound")
        digest = hashlib.sha256()
        read = 0
        with os.fdopen(fd, "rb", buffering=0, closefd=False) as stream:
            while chunk := stream.read(min(1024 * 1024, before.st_size - read + 1)):
                remaining(deadline)
                read += len(chunk)
                if read > before.st_size:
                    raise AssertionError("public D1 source verification work exceeded bound")
                digest.update(chunk)
        after = os.fstat(fd)
        current = path.stat()
        identity = lambda value: (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns, value.st_ctime_ns)
        if read != before.st_size or identity(before) != identity(after) or identity(after) != identity(current):
            raise AssertionError(f"public D1 source changed during verification: {path}")
        return read, digest.hexdigest()
    finally:
        os.close(fd)


def binding_root(paths: list[str], domain: bytes) -> str:
    digest = hashlib.sha256(domain)
    for label in paths:
        encoded = label.encode("utf-8")
        digest.update(len(encoded).to_bytes(8, "big"))
        digest.update(encoded)
    return digest.hexdigest()


def verify_inputs(manifest: dict[str, Any], deadline: float) -> None:
    binding = manifest.get("public_input_binding")
    if (not isinstance(binding, dict)
            or set(binding) != {"schema", "sources", "public_ledger", "partitioned", "partition_parts"}
            or binding["schema"] != "tos_public_input_binding_v1"
            or not isinstance(binding["sources"], list)):
        raise AssertionError("public D1 measured input binding is missing")
    work = [0]
    directory = source_path(LEDGER_PREFIX[:-1])
    if directory.exists():
        if directory.is_symlink() or not directory.is_dir():
            raise AssertionError("public D1 ledger directory changed")
        ledger = []
        entries = 0
        with os.scandir(directory) as children:
            for child in children:
                remaining(deadline)
                entries += 1
                work[0] += len(child.name.encode("utf-8"))
                if entries > 4096 or work[0] > MAX_SOURCE_PASS_BYTES:
                    raise AssertionError("public D1 ledger enumeration exceeds builder bound")
                if child.name.endswith(".access-request.json"):
                    ledger.append(LEDGER_PREFIX + child.name)
        ledger.sort()
    else:
        ledger = []
    sources = binding["sources"]
    labels = [row.get("path") if isinstance(row, dict) else None for row in sources]
    if labels != sorted((*SOURCE_INPUTS, *OPTIONAL_INPUTS, *ledger)):
        raise AssertionError("public D1 source closure differs from fixed builder inputs")
    present = set()
    for row in sources:
        if not isinstance(row, dict) or set(row) != {"path", "size_bytes", "sha256"}:
            raise AssertionError("public D1 input entry is invalid")
        label, size, sha = row["path"], row["size_bytes"], row["sha256"]
        path = source_path(label)
        if label in OPTIONAL_INPUTS and size is None and sha is None:
            if path.exists() or path.is_symlink():
                raise AssertionError(f"public D1 optional input appeared: {label}")
            continue
        if (type(size) is not int or size < 0 or not isinstance(sha, str) or not SHA256.fullmatch(sha)):
            raise AssertionError(f"public D1 input binding is incomplete: {label}")
        if source_digest(path, size, work, deadline) != (size, sha):
            raise AssertionError(f"public D1 input changed since build: {label}")
        present.add(label)
    source_paths = manifest.get("source_paths")
    if (not isinstance(source_paths, list) or any(not isinstance(item, str) for item in source_paths)
            or len(source_paths) != len(set(source_paths)) or set(source_paths) != present):
        raise AssertionError("public D1 present source set differs from completion marker")
    ledger_binding = binding["public_ledger"]
    if (not isinstance(ledger_binding, dict) or set(ledger_binding) != {"count", "paths_sha256"}
            or type(ledger_binding["count"]) is not int or ledger_binding["count"] != len(ledger)
            or ledger_binding["paths_sha256"] != binding_root(ledger, b"tos-public-ledger-membership-v1\0")):
        raise AssertionError("public D1 ledger membership differs from build")
    partitioned = []
    part_binding = binding["partition_parts"]
    if (not isinstance(part_binding, dict) or set(part_binding) != {"count", "root_sha256"}
            or type(part_binding["count"]) is not int or part_binding["count"] < 0
            or part_binding["count"] > MAX_PART_PATHS
            or not isinstance(part_binding["root_sha256"], str)
            or not SHA256.fullmatch(part_binding["root_sha256"])):
        raise AssertionError("public D1 partition closure exceeds verifier profile")
    paths: list[str] = []
    retained_path_bytes = 0
    def charge_projection_read(stored: int, decoded: int) -> None:
        remaining(deadline)
        work[0] += stored + decoded
        if work[0] > MAX_SOURCE_PASS_BYTES:
            raise AssertionError("public D1 descriptor read exceeds verifier work bound")
    for label in SOURCE_INPUTS[:3]:
        path = source_path(label)
        selected = is_partitioned(path, before_read=charge_projection_read)
        partitioned.append(selected)
        if selected:
            reader = ProjectionReader(path, cache_bytes=0, before_read=charge_projection_read)
            for part in reader.closure_paths(verify_data=False):
                remaining(deadline)
                if part != path:
                    relative = part.relative_to(SOURCE_ROOT).as_posix()
                    encoded_bytes = len(relative.encode("utf-8"))
                    retained_path_bytes += encoded_bytes
                    work[0] += encoded_bytes
                    if (len(paths) >= part_binding["count"] or len(paths) >= MAX_PART_PATHS
                            or retained_path_bytes > MAX_PART_PATH_BYTES
                            or work[0] > MAX_SOURCE_PASS_BYTES):
                        raise AssertionError("public D1 partition closure exceeds verifier profile")
                    paths.append(relative)
    if partitioned[0] != partitioned[2] or type(binding["partitioned"]) is not bool or binding["partitioned"] != partitioned[0]:
        raise AssertionError("public D1 projection storage mode differs from build")
    paths.sort()
    if len(paths) != part_binding["count"] or any(left == right for left, right in zip(paths, paths[1:])):
        raise AssertionError("public D1 partition closure differs from build")
    root = hashlib.sha256(b"tos-public-part-closure-v1\0")
    for label in paths:
        size, sha = source_digest(source_path(label), None, work, deadline, MAX_PART_FILE_BYTES)
        encoded = label.encode("utf-8")
        root.update(len(encoded).to_bytes(8, "big"))
        root.update(encoded)
        root.update(size.to_bytes(8, "big"))
        root.update(bytes.fromhex(sha))
    if part_binding["root_sha256"] != root.hexdigest():
        raise AssertionError("public D1 partition closure differs from build")


def free_port() -> int:
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


def normalize_paths(value: Any) -> Any:
    prefix = SOURCE_ROOT.as_posix() + "/"
    if isinstance(value, str):
        if value == SOURCE_ROOT.as_posix():
            return "Tree-of-Sophia"
        if value.startswith(prefix):
            return value.removeprefix(prefix)
        return value
    if isinstance(value, list):
        return [normalize_paths(item) for item in value]
    if isinstance(value, dict):
        return {key: normalize_paths(item) for key, item in value.items()}
    return value


def fetch_json(base: str, path: str) -> dict[str, Any]:
    with urllib.request.urlopen(base + path, timeout=20) as response:
        payload = json.load(response)
    if not isinstance(payload, dict):
        raise AssertionError(f"{path} did not return a JSON object")
    return payload


def post_json(base: str, path: str, payload: dict[str, Any]) -> dict[str, Any]:
    request = urllib.request.Request(
        base + path,
        data=json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8"),
        method="POST",
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        result = json.load(response)
    if not isinstance(result, dict):
        raise AssertionError(f"{path} did not return a JSON object")
    return result


def fetch_jsonl(base: str, path: str) -> list[dict[str, Any]]:
    with urllib.request.urlopen(base + path, timeout=30) as response:
        rows = [json.loads(line) for line in response if line.strip()]
    if not all(isinstance(row, dict) for row in rows):
        raise AssertionError(f"{path} did not return JSON objects")
    return rows


def wait_ready(base: str, process: subprocess.Popen[str], deadline: float) -> None:
    ready_deadline = min(time.monotonic() + 30, deadline)
    while time.monotonic() < ready_deadline:
        if process.poll() is not None:
            output = process.stdout.read() if process.stdout else ""
            raise RuntimeError(f"wrangler dev exited before readiness:\n{output}")
        try:
            if fetch_json(base, "/health").get("ok") is True:
                return
        except (OSError, ValueError):
            time.sleep(0.2)
    raise TimeoutError("wrangler dev did not become healthy within 30 seconds")


def main() -> int:
    if "TOS_QUERY_STORE_PATH" in os.environ or "TOS_RELEASE_ROOT" in os.environ:
        raise AssertionError("ambient query store or managed release cannot select the independent oracle")
    profile = os.environ.get("TOS_VERIFY_PROFILE", "production")
    if profile not in VERIFY_PROFILES:
        raise ValueError("TOS_VERIFY_PROFILE must be production or representative")
    if (profile == "representative") != ("TOS_VERIFY_SOURCE_ROOT" in os.environ):
        raise ValueError("representative verification requires its explicit source root")
    if (not SELECTED_SOURCE_ROOT.is_absolute() or not SOURCE_ROOT.is_dir()
            or SELECTED_SOURCE_ROOT.is_symlink()
            or (profile == "representative" and SOURCE_ROOT == REPO_ROOT.resolve())):
        raise AssertionError("public D1 selected source root is missing or unsafe")
    raw_seconds = os.environ.get("TOS_VERIFY_MAX_SECONDS", "")
    if not raw_seconds.isascii() or not raw_seconds.isdecimal() or int(raw_seconds) < 1:
        raise ValueError("TOS_VERIFY_MAX_SECONDS must be a positive whole-operation deadline")
    deadline = time.monotonic() + int(raw_seconds)
    def expired(_signal, _frame):
        raise RuntimeError("public D1 local verification deadline exceeded")
    previous_handler = signal.signal(signal.SIGALRM, expired)
    try:
        signal.setitimer(signal.ITIMER_REAL, remaining(deadline))
        manifest, marker = completion_pair()
        expected_base = os.environ.get("TOS_VERIFY_EXPECT_DELTA_FROM")
        if expected_base is not None:
            delta = manifest.get("counts", {}).get("delta", {})
            if (profile != "representative" or not SHA256.fullmatch(expected_base)
                    or not isinstance(delta, dict) or delta.get("available") is not True
                    or delta.get("base_revision") != expected_base
                    or delta.get("target_revision") != manifest["data_revision"]):
                raise AssertionError("representative successor delta does not bind its predecessor")
        verify_inputs(manifest, deadline)
        # All three public projections can be partitioned independently of the
        # corpus-mode flag. Compile one fresh, explicit QueryStore even for a
        # legacy corpus; never permit the Python core's ambient default store.
        temporary = tempfile.TemporaryDirectory(prefix="tos-public-oracle-")
        try:
            query_store = Path(temporary.name) / "knowledge.sqlite3"
            program = ("import json,sys; from tos_access.knowledge_compile import compile_knowledge_store; "
                       "print(json.dumps(compile_knowledge_store(sys.argv[1],sys.argv[2],allow_legacy=True)))")
            environment = {**os.environ, "PYTHONPATH": ACCESS_SRC.as_posix(), "PYTHONDONTWRITEBYTECODE": "1"}
            environment.pop("TOS_QUERY_STORE_PATH", None)
            # subprocess.run owns its child timeout and kill/wait. Suspend the
            # parent's signal only for that call; the same absolute deadline
            # covers the initial closure, compile, oracle and HTTP comparisons.
            signal.setitimer(signal.ITIMER_REAL, 0)
            result = subprocess.run(
                [sys.executable, "-B", "-c", program, SOURCE_ROOT.as_posix(), query_store.as_posix()],
                env=environment, capture_output=True, text=True, check=True, timeout=remaining(deadline),
            )
            signal.setitimer(signal.ITIMER_REAL, remaining(deadline))
            if len(result.stdout) > MAX_MANIFEST_BYTES:
                raise AssertionError("independent Python oracle receipt exceeds verifier profile")
            compiled = json.loads(result.stdout)
            if compiled.get("output") != query_store.as_posix() or not query_store.is_file():
                raise AssertionError("independent Python oracle did not complete its selected store")
            os.environ["TOS_QUERY_STORE_PATH"] = query_store.as_posix()
            core = ToSAccessCore.discover(
                SOURCE_ROOT,
                index_path=source_path(SOURCE_INPUTS[0]),
                philosophy_graph_projection_path=source_path(SOURCE_INPUTS[1]),
                bibliographic_graph_path=source_path(SOURCE_INPUTS[2]),
                entity_type_registry_path=source_path(SOURCE_INPUTS[3]),
                relation_type_registry_path=source_path(SOURCE_INPUTS[4]),
                evidence_projection_path=source_path(OPTIONAL_INPUTS[0]),
                philosophy_post_planting_audit_path=source_path(OPTIONAL_INPUTS[1]),
                search_read_model_path=query_store.parent / "no-ambient-search-model",
            )
            if compiled.get("source_revision") != core.knowledge_header().get("source_revision"):
                raise AssertionError("independent Python oracle source revision changed")
            result_code = compare_packets(core, manifest, marker, deadline, profile)
        finally:
            # Cleanup must complete even when the whole-call timer fires.
            # It is charged against the same absolute deadline on return.
            signal.setitimer(signal.ITIMER_REAL, 0)
            temporary.cleanup()
        remaining(deadline)
        return result_code
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous_handler)


def compare_representative_packets(core: ToSAccessCore, base: str) -> None:
    """The existing access test fixture's finite public read surface.

    This is a software gate over representative inputs, not the stronger
    production/large-payload/scale profile below.
    """
    quote = lambda value: urllib.parse.quote(value, safe="")
    knowledge_node_id = "philosophy:a"
    node_packet = core.knowledge_node(knowledge_node_id, 20)
    if node_packet.get("source_revision") != core.knowledge_header().get("source_revision"):
        raise AssertionError("representative oracle knowledge revision changed")
    relations = node_packet.get("related_relations", [])
    if not relations:
        raise AssertionError("representative knowledge node has no relation")
    relation_id = str(relations[0]["id"])
    cases: list[tuple[str, Callable[[], dict[str, Any]], str]] = [
        ("corpus status", core.status, "/api/corpus/status"),
        ("corpus summary", core.summary, "/api/corpus/summary"),
        ("philosophy status", core.philosophy_status, "/api/philosophy/status"),
        ("philosophy views", core.philosophy_views, "/api/philosophy/views"),
        ("knowledge catalog", core.knowledge_catalog, "/api/knowledge/catalog"),
        ("knowledge contracts", core.knowledge_contracts, "/api/knowledge/contracts"),
        ("knowledge search", lambda: core.knowledge_search("Alpha", sources=["philosophy"], limit=5),
         "/api/knowledge/search?query=Alpha&sources=philosophy&limit=5"),
        ("knowledge node", lambda: node_packet,
         f"/api/knowledge/nodes/{quote(knowledge_node_id)}?relation_limit=20"),
        ("knowledge relation", lambda: core.knowledge_relation(relation_id),
         f"/api/knowledge/relations/{quote(relation_id)}"),
        ("knowledge focus", lambda: core.knowledge_focus(knowledge_node_id, depth=1),
         f"/api/knowledge/focus/{quote(knowledge_node_id)}?depth=1"),
        ("stored knowledge lens", lambda: core.stored_knowledge_lens("chronology"),
         "/api/knowledge/lenses/chronology"),
        ("philosophy view", lambda: core.philosophy_view("chronology", 100),
         "/api/philosophy/views/chronology?limit=100"),
        ("corpus view", lambda: core.graph_view("corpus-topology", 37),
         "/api/corpus/graph-views/corpus-topology?limit=37"),
        ("corpus search", lambda: core.search("Alpha", 5),
         "/api/corpus/search?query=Alpha&limit=5"),
        ("philosophy search", lambda: core.philosophy_search("Alpha", 5),
         "/api/philosophy/search?query=Alpha&limit=5"),
        ("source descent", lambda: core.source_descend("philosophy.eras.fixture", 8, 20),
         "/api/source/navigation/philosophy.eras.fixture?max_depth=8&limit=20"),
        ("source dossier", lambda: core.source_dossier("tos.link.fixture.download", 20),
         "/api/source/dossiers/tos.link.fixture.download?limit=20"),
        ("philosophy node", lambda: core.philosophy_node("a"),
         "/api/philosophy/nodes/a"),
        ("philosophy neighborhood", lambda: core.philosophy_neighborhood("a", 1, [], [], 10),
         "/api/philosophy/neighborhood/a?depth=1&limit=10"),
        ("philosophy path", lambda: core.philosophy_path_between("a", "b", [], [], 2, "outgoing", None, [], 2),
         "/api/philosophy/paths?from=a&to=b&max_depth=2&direction=outgoing&alternatives=2"),
        ("philosophy evidence", lambda: core.evidence_lens_packet("philosophy", "a", "direct-only", 10),
         "/api/philosophy/query/epistemic/a?view_id=direct-only&limit=10"),
    ]
    for label, expected, path in cases:
        if fetch_json(base, path) != normalize_paths(expected()):
            raise AssertionError(f"representative Cloudflare contract drift for {label}")
        print(f"ok representative: {label}")

    spec = {"schema_version": "tos_lens_spec_v1", "lens_id": "relations-first",
            "sources": ["philosophy"],
            "node_query": {"enabled": False, "match": "all", "filters": []},
            "relation_query": {"match": "all", "filters": [
                {"field": "predicate_id", "op": "eq", "value": "relates"}]},
            "composition": {"endpoint_policy": "independent"},
            "limits": {"nodes": 10, "relations": 10, "groups": 10}}
    if post_json(base, "/api/knowledge/lenses/compile", spec) != normalize_paths(core.compile_knowledge_lens(spec)):
        raise AssertionError("representative Cloudflare lens drift")
    print("ok representative: compiled knowledge lens")


def compare_packets(core: ToSAccessCore, manifest: dict[str, Any], marker: bytes,
                    deadline: float, profile: str) -> int:
    port = free_port()
    base = f"http://127.0.0.1:{port}"
    # A never-drained PIPE can block Wrangler logging and internal asset reads.
    logs = tempfile.TemporaryFile(mode='w+t')
    process = subprocess.Popen(
        ["npx", "wrangler", "dev", "--local", "--ip", "127.0.0.1", "--port", str(port)],
        cwd=WORKER_ROOT,
        stdout=logs,
        stderr=subprocess.STDOUT,
        text=True,
        env={key: value for key, value in os.environ.items()
             if key not in {"TOS_QUERY_STORE_PATH", "TOS_VERIFY_SOURCE_ROOT",
                            "TOS_VERIFY_PROFILE", "TOS_VERIFY_EXPECT_DELTA_FROM"}},
    )
    try:
        wait_ready(base, process, deadline)
        health = fetch_json(base, "/health")
        if health.get("data_revision") != manifest["data_revision"]:
            raise AssertionError("local D1 imported revision differs from the completed Rust build")
        if profile == "representative":
            compare_representative_packets(core, base)
            verify_inputs(manifest, deadline)
            if completion_pair()[1] != marker:
                raise AssertionError("public D1 completion changed during verification")
            print("representative software profile only; production and scale coverage remains outstanding")
            return 0
        node_id = "candidate-node:table-i-a01-node-016"
        target_id = "candidate-node:table-i-a01-node-014"
        source_work_id = (
            "tos.work.egyptian-scholarship."
            "on-four-songs-contained-in-an-egyptian-papyrus-in-the-british-museum"
        )
        knowledge_node_id = "philosophy:philosophy.atlas"
        knowledge_author_id = "source-navigation:tos.agent.friedrich-nietzsche"
        knowledge_work_id = "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
        large_knowledge_node_id = "canon:tos.source.thus-spoke-zarathustra.prologue"
        knowledge_node_packet = core.knowledge_node(knowledge_node_id, 20)
        if knowledge_node_packet.get("source_revision") != core.knowledge_header().get("source_revision"):
            raise AssertionError("independent oracle knowledge revision changed")
        related_relations = knowledge_node_packet.get("related_relations", [])
        if not related_relations:
            raise AssertionError(f"knowledge parity node has no relation: {knowledge_node_id}")
        knowledge_relation_id = str(related_relations[0]["id"])
        quote = lambda value: urllib.parse.quote(value, safe="")
        cases: list[tuple[str, Callable[[], dict[str, Any]], str]] = [
            ("corpus status", core.status, "/api/corpus/status"),
            ("corpus summary", core.summary, "/api/corpus/summary"),
            ("philosophy status", core.philosophy_status, "/api/philosophy/status"),
            ("philosophy views", core.philosophy_views, "/api/philosophy/views"),
            ("knowledge catalog", core.knowledge_catalog, "/api/knowledge/catalog"),
            ("knowledge contracts", core.knowledge_contracts, "/api/knowledge/contracts"),
            (
                "knowledge search",
                lambda: core.knowledge_search("Zarathustra", sources=["philosophy"], limit=5),
                "/api/knowledge/search?query=Zarathustra&sources=philosophy&limit=5",
            ),
            (
                "Unicode knowledge search",
                lambda: core.knowledge_search("Заратустра", sources=["philosophy"], limit=5),
                f"/api/knowledge/search?query={quote('Заратустра')}&sources=philosophy&limit=5",
            ),
            (
                "knowledge node",
                lambda: knowledge_node_packet,
                f"/api/knowledge/nodes/{quote(knowledge_node_id)}?relation_limit=20",
            ),
            (
                "lossless large knowledge node",
                lambda: core.knowledge_node(large_knowledge_node_id, 20),
                f"/api/knowledge/nodes/{quote(large_knowledge_node_id)}?relation_limit=20",
            ),
            (
                "knowledge relation",
                lambda: core.knowledge_relation(knowledge_relation_id),
                f"/api/knowledge/relations/{quote(knowledge_relation_id)}",
            ),
            (
                "focused knowledge neighborhood",
                lambda: core.knowledge_focus(
                    knowledge_node_id,
                    sources=["philosophy"],
                    depth=1,
                    direction="either",
                    node_limit=40,
                    relation_limit=40,
                ),
                f"/api/knowledge/focus/{quote(knowledge_node_id)}?sources=philosophy&depth=1&direction=either&node_limit=40&relation_limit=40",
            ),
            (
                "author-to-works knowledge neighborhood",
                lambda: core.knowledge_focus(
                    knowledge_author_id,
                    sources=["source-navigation"],
                    depth=1,
                    direction="either",
                    node_limit=40,
                    relation_limit=40,
                ),
                f"/api/knowledge/focus/{quote(knowledge_author_id)}?sources=source-navigation&depth=1&direction=either&node_limit=40&relation_limit=40",
            ),
            (
                "cross-layer work knowledge neighborhood",
                lambda: core.knowledge_focus(
                    knowledge_work_id,
                    sources=["canon", "source-navigation", "source-claims", "semantic-interchange"],
                    depth=5,
                    direction="either",
                    predicate_ids=[
                        "authored_by",
                        "has_expression",
                        "embodied_by",
                        "has_subject",
                        "has_object",
                        "has_normalized_place",
                        "projects",
                        "grounded_in",
                        "commentary-on",
                    ],
                    node_limit=400,
                    relation_limit=800,
                ),
                f"/api/knowledge/focus/{quote(knowledge_work_id)}?"
                "sources=canon,source-navigation,source-claims,semantic-interchange&depth=5&direction=either&"
                "predicates=authored_by,has_expression,embodied_by,has_subject,has_object,has_normalized_place,projects,grounded_in,commentary-on&"
                "node_limit=400&relation_limit=800",
            ),
            (
                "stored knowledge lens",
                lambda: core.stored_knowledge_lens("corpus-topology"),
                "/api/knowledge/lenses/corpus-topology",
            ),
            (
                "Zarathustra word-analysis capability",
                core.zarathustra_word_analysis_public_capability,
                "/api/zarathustra/word-analysis?query=Geist&language=de&rank=2&include_semantic_neighbors=true",
            ),
            ("chronology view", lambda: core.philosophy_view("chronology", 1000), "/api/philosophy/views/chronology?limit=1000"),
            ("dynamic corpus view", lambda: core.graph_view("route-graph", 37), "/api/corpus/graph-views/route-graph?limit=37"),
            ("corpus search", lambda: core.search("zarathustra", 5), "/api/corpus/search?query=zarathustra&limit=5"),
            ("philosophy search", lambda: core.philosophy_search("Gilgamesh", 5), "/api/philosophy/search?query=Gilgamesh&limit=5"),
            (
                "source descent",
                lambda: core.source_descend(source_work_id, 3, 40),
                f"/api/source/navigation/{quote(source_work_id)}?max_depth=3&limit=40",
            ),
            (
                "source dossier",
                lambda: core.source_dossier(source_work_id, 300),
                f"/api/source/dossiers/{quote(source_work_id)}?limit=300",
            ),
            ("node packet", lambda: core.philosophy_node(node_id), f"/api/philosophy/nodes/{quote(node_id)}"),
            (
                "neighborhood",
                lambda: core.philosophy_neighborhood(node_id, 1, [], [], 10),
                f"/api/philosophy/neighborhood/{quote(node_id)}?depth=1&limit=10",
            ),
            (
                "path",
                lambda: core.philosophy_path_between(node_id, target_id, [], [], 2, "outgoing", None, [], 2),
                f"/api/philosophy/paths?from={quote(node_id)}&to={quote(target_id)}&max_depth=2&direction=outgoing&alternatives=2",
            ),
            (
                "philosophy evidence lens",
                lambda: core.evidence_lens_packet("philosophy", node_id, None, 10),
                f"/api/philosophy/query/epistemic/{quote(node_id)}?limit=10",
            ),
            (
                "corpus evidence lens",
                lambda: core.evidence_lens_packet("corpus", "m113", "route-graph", 10),
                "/api/corpus/query/epistemic/m113?view_id=route-graph&limit=10",
            ),
        ]
        for label, expected, path in cases:
            actual = fetch_json(base, path)
            reference = normalize_paths(expected())
            if actual != reference:
                raise AssertionError(f"Cloudflare contract drift for {label}")
            print(f"ok: {label}")

        arbitrary_lens = {
            "schema_version": "tos_lens_spec_v1",
            "lens_id": "edge-contract-smoke",
            "sources": ["philosophy"],
            "node_query": {"enabled": False},
            "relation_query": {
                "filters": [{"field": "predicate_id", "op": "eq", "value": "uses_script"}]
            },
            "composition": {"endpoint_policy": "independent"},
            "limits": {"nodes": 20, "relations": 10, "groups": 10},
        }
        actual_lens = post_json(base, "/api/knowledge/lenses/compile", arbitrary_lens)
        expected_lens = normalize_paths(core.compile_knowledge_lens(arbitrary_lens))
        if actual_lens != expected_lens:
            raise AssertionError("Cloudflare contract drift for arbitrary knowledge lens")
        print("ok: arbitrary knowledge lens")
        compact_lens = {**arbitrary_lens, 'detail': 'compact'}
        if post_json(base, '/api/knowledge/lenses/compile', compact_lens) != normalize_paths(core.compile_knowledge_lens(compact_lens)):
            raise AssertionError('Cloudflare contract drift for compact knowledge carrier')
        print('ok: compact knowledge carrier')
        scoped_lens = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'source-scope-parity',
                       'sources': ['source-claims', 'semantic-interchange'],
                       'seed': {'focus_node_id': knowledge_work_id}, 'node_query': {'enabled': False},
                       'traversal': {'depth': 2, 'profile': 'all'}}
        if post_json(base, '/api/knowledge/lenses/compile', scoped_lens) != normalize_paths(core.compile_knowledge_lens(scoped_lens)):
            raise AssertionError('Cloudflare traversal escaped the selected source scope')
        print('ok: cross-layer traversal preserves source scope')

        null_lens = {
            "schema_version": "tos_lens_spec_v1",
            "lens_id": "edge-null-filter-smoke",
            "sources": ["philosophy"],
            "node_query": {
                "filters": [
                    {"field": "attributes.missing_contract_probe", "op": "in", "value": [None]}
                ]
            },
            "relation_query": {"enabled": False},
            "limits": {"nodes": 3, "relations": 0, "groups": 3},
        }
        actual_null_lens = post_json(base, "/api/knowledge/lenses/compile", null_lens)
        expected_null_lens = normalize_paths(core.compile_knowledge_lens(null_lens))
        if actual_null_lens != expected_null_lens:
            raise AssertionError("Cloudflare contract drift for null-valued knowledge filter")
        print("ok: null-valued knowledge filter")

        scale_layers = ["evidence-relation", "historical-relation"]
        scale_query = urllib.parse.urlencode({"view_id": "chronology", "layers": ",".join(scale_layers)})
        actual_manifest = fetch_json(base, f"/api/philosophy/scale-export/manifest?{scale_query}")
        expected_manifest = normalize_paths(core.philosophy_scale_manifest("chronology", scale_layers))
        if actual_manifest != expected_manifest:
            raise AssertionError("Cloudflare contract drift for scale manifest")
        print("ok: scale manifest")

        for table in (
            "nodes",
            "edges",
            "clusters",
            "cluster-node-memberships",
            "cluster-edge-memberships",
        ):
            actual_rows = fetch_jsonl(
                base,
                f"/api/philosophy/scale-export/{table}.jsonl?{scale_query}",
            )
            expected_rows = normalize_paths(core.philosophy_scale_rows(table, "chronology", scale_layers))
            if actual_rows != expected_rows:
                raise AssertionError(f"Cloudflare contract drift for scale export {table}")
            print(f"ok: scale export {table}")

        empty_manifest = fetch_json(
            base,
            "/api/philosophy/scale-export/manifest?view_id=chronology&layers=__tos_none__",
        )
        if any(descriptor["row_count"] for descriptor in empty_manifest["tables"].values()):
            raise AssertionError("Cloudflare scale export did not preserve the explicit empty layer filter")
        with urllib.request.urlopen(
            base + f"/api/philosophy/scale-export/nodes.csv?{scale_query}",
            timeout=30,
        ) as response:
            if response.headers.get_content_type() != "text/csv" or not response.readline().strip():
                raise AssertionError("Cloudflare CSV scale export is not downloadable")
        print("ok: scale export CSV and empty filter")
        verify_inputs(manifest, deadline)
        if completion_pair()[1] != marker:
            raise AssertionError("public D1 completion changed during verification")
    except Exception:
        logs.seek(0)
        print(logs.read()[-6000:], file=sys.stderr)
        raise
    finally:
        # Shutdown must complete even if the packet deadline fires. The
        # caller checks the same absolute deadline after both cleanups.
        signal.setitimer(signal.ITIMER_REAL, 0)
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
        logs.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
