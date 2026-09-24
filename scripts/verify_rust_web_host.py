#!/usr/bin/env python3
"""Build the WEB.1 codec and run independent vectors in a real Node WASM host."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
CLI = os.environ.get("WASM_BINDGEN_CLI", "wasm-bindgen")
TARGET = Path(os.environ["CARGO_TARGET_DIR"]) if "CARGO_TARGET_DIR" in os.environ else ROOT / "target"


def run(*command: str | Path) -> subprocess.CompletedProcess[str]:
    result = subprocess.run([str(part) for part in command], cwd=ROOT, text=True,
                            capture_output=True, check=False)
    if result.returncode:
        sys.stderr.write(result.stdout)
        sys.stderr.write(result.stderr)
        result.check_returncode()
    return result


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    version = run(CLI, "--version").stdout.strip()
    if version != "wasm-bindgen 0.2.128":
        raise ValueError(f"WEB.1 requires wasm-bindgen CLI 0.2.128, got {version!r}")
    run("cargo", "build", "--locked", "-p", "tos-web-codec", "--target",
        "wasm32-unknown-unknown", "--features", "wasm", "--release")
    wasm = TARGET / "wasm32-unknown-unknown/release/tos_web_codec.wasm"
    with tempfile.TemporaryDirectory(prefix="tos-web-host-") as temporary:
        output = Path(temporary)
        run(CLI, "--target", "web", "--out-dir", output, wasm)
        (output / "package.json").write_text('{"type":"module"}\n', encoding="utf-8")
        js = output / "tos_web_codec.js"
        generated_wasm = output / "tos_web_codec_bg.wasm"
        profiles = ROOT / "tests/conformance/rust/canonical-profiles-v1.jsonl"
        float_oracle = os.environ.get("TOS_WEB_FLOAT_ORACLE")
        float_args: list[str] = []
        if float_oracle:
            expected_sha = os.environ.get("TOS_WEB_FLOAT_ORACLE_SHA256")
            if not expected_sha or digest(Path(float_oracle)) != expected_sha:
                raise ValueError("WEB float oracle SHA-256 missing or mismatched")
            float_args.append(float_oracle)
        host = run("node", ROOT / "rust/crates/tos-web-codec/tests/wasm-host.mjs",
                   js, generated_wasm, ROOT / "tests/conformance/rust/foundation.jsonl",
                   profiles, *float_args)
        result = json.loads(host.stdout)
        if (result.get("status") != "pass" or result.get("foundation_vectors") != 18
                or result.get("profile_vectors") != 51
                or result.get("float_vectors") != (8258 if float_oracle else 0)):
            raise ValueError("WEB.1 host runner returned incomplete vector result")
        worker_package = os.environ.get("TOS_WEB_MINIFLARE_PACKAGE")
        worker_result = None
        if worker_package:
            worker = run("node", ROOT / "rust/crates/tos-web-codec/tests/worker-host.mjs",
                         worker_package, js, generated_wasm,
                         ROOT / "access/deploy/cloudflare-worker/wrangler.jsonc")
            worker_result = json.loads(worker.stdout)
            if worker_result.get("status") != "pass" or worker_result.get("cases") != 2:
                raise ValueError("WEB.1 Worker host returned incomplete result")
        print(json.dumps({"schema_version": "tos_web_host_verification_v1",
                          "result": result, "worker_result": worker_result,
                          "float_oracle_sha256": expected_sha if float_oracle else None,
                          "js_sha256": digest(js),
                          "wasm_sha256": digest(generated_wasm)}, sort_keys=True))


if __name__ == "__main__":
    main()
