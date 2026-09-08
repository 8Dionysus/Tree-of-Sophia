"""Synthetic private transport checks, not source or publication acceptance.

Only the existing synthetic native fixture and public authored schemas are
read. No retained private text or durable owner-local store is opened.
"""
from __future__ import annotations

import copy
import json
from pathlib import Path
import tempfile
import unittest

from test_native_text_binding import (
    NativeTextBindingError,
    NativeTextBindingFixture,
    NativeTextBindingResolver,
    REPO_ROOT,
    digest,
)
from source_owner_context import OwnerLocalSourceContext


SCHEMA_REF = "ToS/contracts/owner-local-source-context.schema.json"
STORE_ID = "sid-" + "1" * 32
OTHER_STORE_ID = "sid-" + "2" * 32
PREFIX = "ToS/source-witnesses/owner-local/" + STORE_ID + "/"
REF = PREFIX + "native/source-text-unit.synthetic.v1.json"
REJECTED = (ValueError, PermissionError, OSError)


class SourceOwnerContextTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tos-owner-context-test-")
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.public = self.base / "public"
        self.private = self.base / "private"
        self.public.mkdir(mode=0o755)
        self.private.mkdir(mode=0o700)
        self.fixture = NativeTextBindingFixture(self.public)
        self.fixture.write_bytes(SCHEMA_REF, (REPO_ROOT / SCHEMA_REF).read_bytes())
        old_packet = self.public / self.fixture.packet_ref
        self.packet_bytes = old_packet.read_bytes()
        self.packet = self.private / REF
        self.write_private(self.packet, self.packet_bytes)
        old_packet.unlink()
        self.binding = copy.deepcopy(self.fixture.binding)
        self.binding["packet_ref"] = REF
        self.config_path = self.base / "owner-context.json"
        self.config = {
            "schema_version": "tos_owner_local_source_context_v1",
            "store_id": STORE_ID,
            "public_root": str(self.public),
            "private_root": str(self.private),
            "private_prefix": PREFIX,
        }
        self.write_config()

    def write_private(self, path, raw):
        path.parent.mkdir(parents=True, exist_ok=True)
        current = path.parent
        while current.is_relative_to(self.private):
            current.chmod(0o700)
            if current == self.private:
                break
            current = current.parent
        path.write_bytes(raw)
        path.chmod(0o600)

    def write_config(self, config=None):
        self.config_path.write_bytes(
            (json.dumps(self.config if config is None else config, sort_keys=True) + "\n").encode()
        )
        self.config_path.chmod(0o600)

    def load(self):
        return OwnerLocalSourceContext.load(self.config_path)

    def resolver(self, context=None, **kwargs):
        return NativeTextBindingResolver(
            self.public, owner_context=context or self.load(), **kwargs
        )

    def resolve(self, context=None, **kwargs):
        return self.resolver(context).resolve(self.binding, **kwargs)

    def test_exact_private_resolution_preserves_native_bytes_and_public_dependencies(self):
        context = self.load()
        self.assertEqual(context.public_root, self.public)
        self.assertEqual(context.private_root, self.private)
        self.assertEqual(context.store_id, STORE_ID)
        self.assertEqual(context.private_prefix, PREFIX)
        self.assertEqual(context.path(REF), self.packet)
        self.assertEqual(context.path(self.fixture.layer_ref), self.public / self.fixture.layer_ref)
        self.assertEqual(context.path(SCHEMA_REF), self.public / SCHEMA_REF)
        self.assertEqual(context.read_bytes(self.packet, len(self.packet_bytes)), self.packet_bytes)
        self.assertEqual(self.binding["packet_sha256"], digest(self.packet_bytes))
        self.assertFalse((self.public / REF).exists())
        self.assertFalse((self.public / self.fixture.original_ref).exists())
        observed = []

        def read(path, limit):
            observed.append(path)
            with path.open("rb") as stream:
                return stream.read(limit + 1)

        resolver = self.resolver(context, read_bytes=read)
        summary = resolver.resolve(self.binding, verify_content=True, allow_private_content=True)
        self.assertTrue(summary["metadata_verified"])
        self.assertTrue(summary["content_verified"])
        self.assertFalse(summary["public_content_declared"])
        self.assertFalse(summary["assessment_applied"])
        self.assertIn(self.packet, observed)
        self.assertIn(self.public / self.fixture.content_ref, observed)
        self.assertTrue(all(path.is_relative_to(self.public) or path == self.packet for path in observed))
        self.assertEqual((self.public / self.fixture.content_ref).read_bytes(), self.fixture.content)
        self.assertIn(b"\r\n", self.fixture.content)
        self.assertIn("cafe\u0301".encode(), self.fixture.content)
        self.assertRegex(resolver.snapshot(), r"^sha256:[a-f0-9]{64}$")

    def test_context_transport_does_not_grant_private_content_read(self):
        summary = self.resolve()
        self.assertTrue(summary["metadata_verified"])
        self.assertFalse(summary["content_verified"])
        with self.assertRaises(NativeTextBindingError):
            self.resolve(verify_content=True)

    def test_private_packet_transport_caps_otherwise_public_source_visibility(self):
        self.fixture.make_public()
        public_packet = self.public / self.fixture.packet_ref
        self.packet_bytes = public_packet.read_bytes()
        self.write_private(self.packet, self.packet_bytes)
        public_packet.unlink()
        self.binding = copy.deepcopy(self.fixture.binding)
        self.binding["packet_ref"] = REF
        summary = self.resolve()
        self.assertTrue(summary["owner_local_transport"])
        self.assertEqual(summary["effective_visibility"], "local_only")
        self.assertFalse(summary["public_content_declared"])
        with self.assertRaises(NativeTextBindingError):
            self.resolve(verify_content=True)
        summary = self.resolve(verify_content=True, allow_private_content=True)
        self.assertTrue(summary["content_verified"])
        self.assertFalse(summary["public_content_available"])

    def test_private_content_route_is_checked_before_first_private_byte_read(self):
        self.fixture.make_public()
        private_content_ref = PREFIX + "content/synthetic.txt"
        private_content = self.private / private_content_ref
        self.write_private(private_content, self.fixture.content)
        (self.public / self.fixture.content_ref).unlink()
        self.fixture.layer["representation"]["content_ref"] = private_content_ref
        for anchor in self.fixture.packet["anchors"]:
            anchor["source_return"]["locator_ref"] = private_content_ref
        self.fixture.refresh()
        observed = []

        def read(path, limit):
            observed.append(path)
            return path.read_bytes()

        resolver = self.resolver(read_bytes=read)
        with self.assertRaises(NativeTextBindingError):
            resolver.resolve(self.fixture.binding, verify_content=True)
        self.assertNotIn(private_content, observed)
        summary = self.resolver().resolve(
            self.fixture.binding, verify_content=True, allow_private_content=True
        )
        self.assertTrue(summary["content_verified"])
        self.assertTrue(summary["owner_local_transport"])
        self.assertFalse(summary["public_content_available"])
        self.assertEqual(summary["effective_visibility"], "local_only")
        # Metadata-only public reads must not advertise a reserved private
        # locator as a publicly resolvable source just because no bytes opened.
        with self.assertRaises(NativeTextBindingError):
            NativeTextBindingResolver(self.public).resolve(self.fixture.binding)

    def test_default_public_resolver_refuses_reserved_prefix_even_when_file_exists(self):
        self.fixture.write_bytes(REF, self.packet_bytes)
        observed = []

        def read(path, limit):
            observed.append(path)
            return path.read_bytes()

        resolver = NativeTextBindingResolver(self.public, read_bytes=read)
        with self.assertRaises(NativeTextBindingError):
            resolver.resolve(self.binding)
        self.assertNotIn(self.public / REF, observed)

    def test_private_ref_never_falls_back_to_public_copy(self):
        context = self.load()
        self.packet.unlink()
        self.fixture.write_bytes(REF, self.packet_bytes)
        observed = []

        def read(path, limit):
            observed.append(path)
            return path.read_bytes()

        with self.assertRaises(REJECTED):
            context.read_bytes(self.packet, len(self.packet_bytes), read_bytes=read)
        self.assertNotIn(self.public / REF, observed)
        with self.assertRaises(REJECTED):
            context.snapshot()
        with self.assertRaises(REJECTED):
            self.load()

    def test_private_packet_missing_without_public_copy_is_not_relocated(self):
        context = self.load()
        self.packet.unlink()
        self.fixture.write_bytes(self.fixture.packet_ref, self.packet_bytes)
        with self.assertRaises(REJECTED):
            self.resolve(context)
        self.assertEqual(context.path(REF), self.packet)

    def test_collision_is_rechecked_after_initial_context_load(self):
        context = self.load()
        resolver = self.resolver(context)
        resolver.resolve(self.binding, verify_content=True, allow_private_content=True)
        self.fixture.write_bytes(REF, self.packet_bytes)
        with self.assertRaises(REJECTED):
            context.snapshot()
        with self.assertRaises(REJECTED):
            resolver.snapshot()

    def test_foreign_store_and_noncanonical_refs_fail_closed(self):
        context = self.load()
        for ref in (
            REF.replace(STORE_ID, OTHER_STORE_ID),
            "ToS/source-witnesses/owner-local/unconfigured/source.json",
            PREFIX + "../source.json",
            PREFIX + "./source.json",
            PREFIX + "nested//source.json",
            str(self.packet),
            PREFIX + "nested\\source.json",
        ):
            with self.subTest(ref=ref), self.assertRaises(REJECTED):
                context.path(ref)
        with self.assertRaises(REJECTED):
            context.read_bytes(self.public / REF, len(self.packet_bytes))

    def test_context_and_native_snapshots_bind_exact_config_and_schema_bytes(self):
        for path in (self.config_path, self.public / SCHEMA_REF):
            with self.subTest(kind=path.name):
                original = path.read_bytes()
                context = self.load()
                resolver = self.resolver(context)
                resolver.resolve(self.binding)
                path.write_bytes(original + b"\n")
                try:
                    with self.assertRaises(REJECTED):
                        context.snapshot()
                    with self.assertRaises(REJECTED):
                        resolver.snapshot()
                finally:
                    path.write_bytes(original)

    def test_native_snapshot_rejects_private_packet_dependency_drift(self):
        resolver = self.resolver()
        resolver.resolve(self.binding, verify_content=True, allow_private_content=True)
        self.packet.write_bytes(self.packet_bytes + b"\n")
        with self.assertRaises(REJECTED):
            resolver.snapshot()

    def test_root_identity_change_is_not_hidden_by_same_path_and_bytes(self):
        context = self.load()
        resolver = self.resolver(context)
        resolver.resolve(self.binding)
        previous = self.base / "previous-private"
        self.private.rename(previous)
        self.private.mkdir(mode=0o700)
        self.write_private(self.packet, self.packet_bytes)
        with self.assertRaises(REJECTED):
            context.snapshot()
        with self.assertRaises(REJECTED):
            resolver.snapshot()

    def test_private_permissions_are_stricter_than_other_user_write_protection(self):
        for path, unsafe, original in (
            (self.config_path, 0o644, 0o600),
            (self.private, 0o755, 0o700),
            (self.packet.parent, 0o755, 0o700),
            (self.packet, 0o644, 0o600),
        ):
            with self.subTest(kind=path.name):
                path.chmod(unsafe)
                try:
                    with self.assertRaises(REJECTED):
                        self.resolve(verify_content=True, allow_private_content=True)
                finally:
                    path.chmod(original)

    def test_custom_reader_cannot_bypass_pre_or_post_read_private_protection(self):
        context = self.load()
        called = []

        def read(path, limit):
            called.append(path)
            return self.packet_bytes

        self.packet.chmod(0o644)
        with self.assertRaises(REJECTED):
            context.read_bytes(self.packet, len(self.packet_bytes), read_bytes=read)
        self.assertEqual(called, [])
        self.packet.chmod(0o600)

        def expose_after_read(path, limit):
            raw = path.read_bytes()
            path.chmod(0o644)
            return raw

        with self.assertRaises(REJECTED):
            context.read_bytes(self.packet, len(self.packet_bytes), read_bytes=expose_after_read)
        self.packet.chmod(0o600)

    def test_symlinked_config_roots_packet_and_public_schema_are_refused(self):
        config_link = self.base / "context-link.json"
        config_link.symlink_to(self.config_path)
        with self.assertRaises(REJECTED):
            OwnerLocalSourceContext.load(config_link)
        for field in ("private_root", "public_root"):
            with self.subTest(kind=field):
                target = Path(self.config[field])
                link = self.base / (field + "-link")
                link.symlink_to(target, target_is_directory=True)
                self.write_config({**self.config, field: str(link)})
                with self.assertRaises(REJECTED):
                    self.load()
        self.write_config()
        for path in (self.packet, self.public / SCHEMA_REF):
            with self.subTest(kind=path.name):
                retained = path.with_name(path.name + ".retained")
                path.rename(retained)
                path.symlink_to(retained)
                try:
                    with self.assertRaises(REJECTED):
                        self.resolve()
                finally:
                    path.unlink()
                    retained.rename(path)

    def test_overlapping_roots_and_mismatched_resolver_root_are_refused(self):
        public_child = self.public / "private-child"
        private_child = self.private / "public-child"
        public_child.mkdir(mode=0o700)
        private_child.mkdir(mode=0o700)
        for updates in (
            {"private_root": str(self.public)},
            {"private_root": str(public_child)},
            {"public_root": str(private_child)},
        ):
            with self.subTest(updates=updates):
                self.write_config({**self.config, **updates})
                with self.assertRaises(REJECTED):
                    self.load()
        self.write_config()
        context = self.load()
        with self.assertRaises(REJECTED):
            NativeTextBindingResolver(self.private, owner_context=context)

    def test_config_rejects_store_prefix_mismatch_and_unrecognized_authority_fields(self):
        for updates in (
            {"private_prefix": PREFIX.replace(STORE_ID, OTHER_STORE_ID)},
            {"private_prefix": PREFIX.rstrip("/")},
            {"private_prefix": "ToS/source-witnesses/"},
            {"store_id": "synthetic-store"},
            {"fallback_public": True},
            {"allow_private_content": True},
        ):
            with self.subTest(updates=updates):
                self.write_config({**self.config, **updates})
                with self.assertRaises(REJECTED):
                    self.load()

    def test_private_schema_copy_cannot_shadow_public_grammar(self):
        context = self.load()
        grammar_ref = "ToS/contracts/native-text-unit-binding.schema.json"
        public_schema = self.public / grammar_ref
        original = public_schema.read_bytes()
        self.write_private(self.private / grammar_ref, b"{\"not\": {}}\n")
        self.assertEqual(context.path(grammar_ref), public_schema)
        self.assertTrue(self.resolve(context)["metadata_verified"])
        self.write_private(self.private / grammar_ref, original)
        public_schema.write_bytes(b"{\"not\": {}}\n")
        with self.assertRaises(REJECTED):
            self.resolve()

    def test_public_snapshots_remain_unchanged_and_private_snapshots_are_opaque(self):
        self.fixture.write_bytes(self.fixture.packet_ref, self.packet_bytes)
        public_resolver = NativeTextBindingResolver(self.public)
        public_resolver.resolve(self.fixture.binding)
        public_snapshot = public_resolver.snapshot()
        context = self.load()
        resolver = self.resolver(context)
        summary = resolver.resolve(self.binding, verify_content=True, allow_private_content=True)
        private_snapshot, context_snapshot = resolver.snapshot(), context.snapshot()
        for value in (private_snapshot, context_snapshot):
            self.assertRegex(value, r"^sha256:[a-f0-9]{64}$")
        rendered = json.dumps([summary, private_snapshot, context_snapshot], ensure_ascii=False)
        for private_value in (
            str(self.private), str(self.config_path), REF, self.fixture.content_ref,
            self.fixture.text[3:8], digest(self.fixture.text[3:8].encode()),
        ):
            self.assertNotIn(private_value, rendered)
        self.config_path.write_bytes(self.config_path.read_bytes() + b"\n")
        self.assertEqual(public_resolver.snapshot(), public_snapshot)


if __name__ == "__main__":
    unittest.main()
