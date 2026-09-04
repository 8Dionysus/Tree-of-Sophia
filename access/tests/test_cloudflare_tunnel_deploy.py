from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEPLOY_ROOT = REPO_ROOT / "access" / "deploy" / "cloudflare-tunnel"


class CloudflareTunnelDeployTests(unittest.TestCase):
    def test_origin_launcher_is_posix_shell_and_tos_owned(self) -> None:
        launcher = DEPLOY_ROOT / "run-origin.sh"
        source = launcher.read_text(encoding="utf-8")
        subprocess.run(["sh", "-n", launcher], check=True)
        self.assertIn("python3", source)
        self.assertIn("-m tos_access", source)
        self.assertIn("--host 127.0.0.1", source)
        self.assertNotIn("abyss-stack", source)

    def test_origin_launcher_fails_closed_for_non_tos_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(
                ["sh", DEPLOY_ROOT / "run-origin.sh"],
                env={
                    **os.environ,
                    "TOS_SITE_ROOT": directory,
                    "TOS_PYTHON": os.environ.get("PYTHON", "python3"),
                },
                capture_output=True,
                text=True,
            )
        self.assertNotEqual(result.returncode, 0)

    def test_tunnel_token_is_external_to_git_and_not_an_argument(self) -> None:
        unit = (DEPLOY_ROOT / "systemd" / "tos-cloudflare-tunnel.service").read_text(
            encoding="utf-8"
        )
        self.assertIn("LoadCredential=cloudflare-tunnel-token:", unit)
        self.assertIn("--token-file %d/cloudflare-tunnel-token", unit)
        self.assertNotIn("--token ", unit)
        self.assertNotIn("TUNNEL_TOKEN=", unit)

    def test_tunnel_requires_the_tos_origin(self) -> None:
        unit = (DEPLOY_ROOT / "systemd" / "tos-cloudflare-tunnel.service").read_text(
            encoding="utf-8"
        )
        self.assertIn("Requires=tos-access-origin.service", unit)
        self.assertIn("After=network-online.target tos-access-origin.service", unit)


if __name__ == "__main__":
    unittest.main()
