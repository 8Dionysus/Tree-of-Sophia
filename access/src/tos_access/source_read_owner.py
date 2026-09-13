"""Explicit local source-owner selection with request-local reader lifetimes.

Portable access imports no source mechanics until this route is selected.
The retained input vector is immutable; each request gets new bounded source
and catalog readers and must verify the same epoch. No scan, build or latest
selection is performed by this adapter.
"""
from __future__ import annotations

import importlib
import importlib.util
from pathlib import Path
import sys
from threading import BoundedSemaphore

from .prepared_source_binding import MAX_STATE_BYTES, PreparedSourceInputs
from .source_read import (
    SourceCatalogClaimReader, SourceCatalogTargetIssuer, SourceOwnerBinding,
    SourceReadError, SourceReadService, _canonical_bytes, _request_size,
)


def _module_at(name, expected):
    try:
        spec = importlib.util.find_spec(name)
    except (ImportError, ValueError) as error:
        raise SourceReadError("source owner module has no verifiable origin: " + name) from error
    if (spec is None or not isinstance(spec.origin, str)
            or Path(spec.origin).resolve() != expected.resolve()):
        raise SourceReadError("source owner module origin differs from this access checkout: " + name)
    module = importlib.import_module(name)
    if Path(module.__file__).resolve() != expected.resolve():
        raise SourceReadError("loaded source owner module origin changed: " + name)
    return module


def _owner_modules():
    # Load installed implementation code, never Python files from --root or
    # from a path supplied by the source vector. A schema-only portable bundle
    # has no such owner and cannot activate this optional route.
    execution_root = Path(__file__).resolve().parents[3]
    scripts = execution_root / "scripts"
    expected = scripts / "source_catalog_projection.py"
    if not expected.is_file():
        raise SourceReadError("local source owner implementation is unavailable")
    if str(scripts) not in sys.path:
        sys.path.insert(0, str(scripts))
    catalog = _module_at("source_catalog_projection", expected)
    expected_metadata = execution_root / "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/metadata_version_reader.py"
    # A cached catalog import does not repeat its sys.path setup. The owner
    # loader must bind its own implementation dependency explicitly.
    if str(expected_metadata.parent) not in sys.path:
        sys.path.insert(0, str(expected_metadata.parent))
    metadata = _module_at("metadata_version_reader", expected_metadata)
    profiles = _module_at("source_record_profiles", scripts / "source_record_profiles.py")
    _module_at("source_agent_publication", scripts / "source_agent_publication.py")
    _module_at("native_text_binding", scripts / "native_text_binding.py")
    _module_at("native_text_return", scripts / "native_text_return.py")
    return catalog, metadata, profiles


class SelectedSourceReadService:
    """One explicitly selected epoch, isolated readers for each operation.

At most two source operations execute concurrently. Saturation fails closed
without adding an unbounded queue. A handle is reproducible across requests
but never retains reader state or authorizes disclosure on its own.
    """

    def __init__(self, source_root: Path, inputs_path: Path, *, expected_revision: str):
        self.source_root = Path(source_root)
        if (not self.source_root.is_absolute() or ".." in self.source_root.parts
                or not self.source_root.is_dir()):
            raise SourceReadError("an explicit absolute source owner root is required")
        with Path(inputs_path).open("rb") as stream:
            raw = stream.read(MAX_STATE_BYTES + 1)
        self.inputs = PreparedSourceInputs.parse(raw)
        if self.inputs.value()["source_revision"] != expected_revision:
            raise SourceReadError("source vector and selected prepared reader revisions differ")
        self._modules = _owner_modules()
        self._slots = BoundedSemaphore(2)
        initial = self._new_session()
        self.epoch = initial.epoch
        self.limits = initial.limits

    def _new_session(self):
        catalog, metadata, profiles = self._modules
        view = self.inputs.roots().get("source-catalog")
        if view is None:
            raise SourceReadError("selected source vector has no addressed catalog")
        snapshot = catalog.SourceCatalogSnapshot(view,
            expected_root_sha256=view.snapshot_digest,
            trusted_baseline_sha256=view.snapshot_digest)
        metadata_reader = metadata.MetadataVersionReader(self.source_root, catalog_snapshot=snapshot)
        claim_reader = SourceCatalogClaimReader(
            catalog.SourceCatalogSourceReader(self.source_root, catalog_snapshot=snapshot))
        binding = SourceOwnerBinding.from_prepared_source(self.inputs,
            catalog_snapshot=snapshot, metadata_reader=metadata_reader, claim_reader=claim_reader)
        declared = profiles.SourceRecordProfiles(self.source_root)
        kinds = set(metadata.NATIVE_CATALOGS) | {"link", "artifact", "composite"} | set(declared.profiles)
        service = SourceReadService(binding, target_issuer=SourceCatalogTargetIssuer(snapshot),
                                    metadata_record_types=kinds)
        if hasattr(self, "epoch") and service.epoch != self.epoch:
            raise SourceReadError("source owner epoch changed; explicit reselection required")
        return service

    def _call(self, operation, *args):
        if not self._slots.acquire(blocking=False):
            raise SourceReadError("source reader concurrency budget exhausted")
        try:
            return getattr(self._new_session(), operation)(*args)
        finally:
            self._slots.release()

    def capabilities(self):
        result = self._call("capabilities")
        result["representations"] = ["record", "native_public_unit"]
        result["authority"]["native_text_payload"] = True
        result["authority"]["note"] = "Metadata handles select records, not text permissions. Explicit native_public_unit separately verifies exact native closure and unconditional public rights; no private or conditional text is delivered."
        return result

    def discover(self, request):
        return self._call("discover", request)

    def read(self, request):
        if isinstance(request, dict) and request.get("representation") == "native_public_unit":
            _request_size(request, self.limits)
            if set(request) != {"handle", "representation"}:
                raise SourceReadError("source read requires handle and representation only")
            if not self._slots.acquire(blocking=False):
                raise SourceReadError("source reader concurrency budget exhausted")
            try:
                return self._read_native_unit(request)
            finally:
                self._slots.release()
        return self._call("read", request)

    def _read_native_unit(self, request):
        session = self._new_session()
        selected = session.read({"handle": request["handle"], "representation": "record"})
        result = {**selected, "schema_version": "tos_source_native_unit_read_result_v1",
                  "record": None, "native_unit": None, "text_access": None}
        if selected["status"] != "available":
            return result
        record = selected["record"]
        native_binding = record.get("native_text_binding")
        if selected["layer"] != "metadata_record" or not isinstance(native_binding, dict):
            return {**result, "status": "unsupported", "reason": "no-exact-native-unit-binding"}
        module = _module_at("native_text_binding", Path(__file__).resolve().parents[3] / "scripts/native_text_binding.py")
        resolver = module.NativeTextBindingResolver(self.source_root)
        delivery = _module_at("native_text_return", Path(__file__).resolve().parents[3] / "scripts/native_text_return.py")
        try:
            unit = delivery.read_public_unit(resolver, native_binding)
            # Recheck the source publication as well as the native closure.
            # The metadata handle never grants disclosure of its text.
            session.binding.verify()
            resolver.snapshot()
            result.update(status="available", reason="exact-owner-public-native-unit", native_unit=unit,
                          text_access={"scope": "public-native-unit", "recorded_rights_verified": True,
                                       "conditional_rights": False, "grants_current_use": False})
            if len(_canonical_bytes(result)) > self.limits.max_response_bytes:
                return {**result, "status": "over-budget", "reason": "source-response-byte-budget",
                        "native_unit": None, "text_access": None}
            return result
        except module.NativeTextBindingError as error:
            # Return closed public reason codes, never paths or private inputs.
            reason = str(error)
            if "requires public content authority" in reason or "requires unconditional recorded rights" in reason:
                status, code = "access-restricted", "native-unit-public-rights-not-satisfied"
            elif "budget" in reason:
                status, code = "over-budget", "native-unit-read-budget"
            else:
                status, code = "corrupt", "native-unit-closure-not-verified"
            return {**result, "status": status, "reason": code}
