"""Owner-bound exact source record delivery.

This module is the small access-plane ABI between a source owner and a human
or agent consumer.  It deliberately does not discover paths, open arbitrary
files, or manufacture a source revision.  The source owner supplies already
bound readers (the metadata, Claim, and addressed source-slot readers); this
module only validates their exact result and transports it through a bounded
typed handle.

The owner readers remain authoritative for source publication currentness,
catalog membership, source bytes, history, and rights.  A handle is therefore
an opaque, transport-safe assertion of one exact owner selection, not a bearer
permission or a replacement for those checks.
"""
from __future__ import annotations

import copy
import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Callable


HANDLE_SCHEMA = "tos_source_read_handle_v1"
DISCOVERY_SCHEMA = "tos_source_handle_discovery_v1"
RESULT_SCHEMA = "tos_source_read_result_v1"
CAPABILITIES_SCHEMA = "tos_source_read_capabilities_v1"
ISSUER = "Tree-of-Sophia/source-witnesses"
AUTHORED_ISSUER = "Tree-of-Sophia/authored-corpus"
PUBLICATION_PROTOCOL = "tos_selected_source_metadata_v1"

MAX_SAFE_INTEGER = 9_007_199_254_740_991
MAX_HANDLE_BYTES = 16 * 1024
MAX_REQUEST_BYTES = 64 * 1024
MAX_RECORD_BYTES = 1 * 1024 * 1024
MAX_RESPONSE_BYTES = 2 * 1024 * 1024

_HEX = re.compile(r"[a-f0-9]{64}\Z")
_SHA = re.compile(r"sha256:[a-f0-9]{64}\Z")
_METADATA_ID = re.compile(r"tos\.(?!claim\.)[a-z0-9]+(?:[.-][a-z0-9]+)*\Z")
_CLAIM_ID = re.compile(r"tos\.claim\.[a-z0-9]+(?:[.-][a-z0-9]+)*\Z")
_RECORD_TYPE = re.compile(r"[a-z][a-z0-9-]{0,63}\Z")
_SLOT_ID = re.compile(r"tos\.[a-z0-9]+(?:[.-][a-z0-9]+)*\Z")
_NAMESPACE = re.compile(r"[a-z][a-z0-9_.-]{0,127}\Z")

STATUSES = frozenset(
    {"available", "missing", "stale", "corrupt", "access-restricted", "over-budget", "unsupported"}
)
METADATA_LAYER = "metadata_record"
CLAIM_LAYER = "claim_record"
SLOT_LAYER = "source_slot"
CSV_LAYER = "authored_csv_record"
SUPPORTED_LAYERS = frozenset({METADATA_LAYER, CLAIM_LAYER, SLOT_LAYER, CSV_LAYER})
SLOT_KINDS = frozenset({"claim", "provenance_event", "anchor"})
PUBLIC_VISIBILITIES = frozenset({"public", "public_metadata_only"})


class SourceReadError(ValueError):
    """The supplied ABI value is structurally invalid."""


class SourceReadBudgetExceeded(SourceReadError):
    """The selected exact record would exceed a delivery budget."""


def _canonical_bytes(value: Any) -> bytes:
    try:
        return json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    except (TypeError, ValueError, OverflowError, RecursionError) as error:
        raise SourceReadError("source read value is not canonical JSON") from error


def _canonical_digest(value: Any) -> str:
    return hashlib.sha256(_canonical_bytes(value)).hexdigest()


def _copy(value: Any) -> Any:
    """Detach owner output before it crosses the access boundary."""
    return json.loads(_canonical_bytes(value).decode("utf-8"))


def _bare_digest(value: Any, *, field: str) -> str:
    if type(value) is not str or _HEX.fullmatch(value) is None:
        raise SourceReadError(f"{field} must be an exact lowercase SHA-256")
    return value


def _sha_digest(value: Any, *, field: str) -> str:
    if type(value) is not str or _SHA.fullmatch(value) is None:
        raise SourceReadError(f"{field} must be an exact sha256-prefixed digest")
    return value


def _exact_ref(value: Any, *, claim: bool) -> dict[str, Any]:
    if type(value) is not dict or set(value) != {"id", "version", "digest"}:
        raise SourceReadError("an exact id/version/digest record reference is required")
    identifier = value.get("id")
    if type(identifier) is not str:
        raise SourceReadError("record reference identity must be a string")
    valid_id = _CLAIM_ID.fullmatch(identifier) if claim else _METADATA_ID.fullmatch(identifier)
    if valid_id is None:
        raise SourceReadError("record reference belongs to another source family")
    version = value.get("version")
    if type(version) is not int or not 1 <= version <= MAX_SAFE_INTEGER:
        raise SourceReadError("record reference version must be an exact safe integer")
    digest = _sha_digest(value.get("digest"), field="record reference digest")
    return {"id": identifier, "version": version, "digest": digest}


def _publication(value: Any) -> dict[str, Any]:
    if type(value) is not dict or set(value) != {"protocol", "token", "generation"}:
        raise SourceReadError("an exact source publication binding is required")
    if value.get("protocol") != PUBLICATION_PROTOCOL:
        raise SourceReadError("source publication protocol is not supported")
    token = _sha_digest(value.get("token"), field="source publication token")
    generation = value.get("generation")
    if type(generation) is not int or not 0 <= generation <= MAX_SAFE_INTEGER:
        raise SourceReadError("source publication generation must be an exact safe integer")
    return {"protocol": PUBLICATION_PROTOCOL, "token": token, "generation": generation}


def _selector(value: Any) -> dict[str, Any]:
    """Validate a producer-facing catalog selector before owner lookup.

    Selectors intentionally contain only a typed identity.  The catalog owner
    supplies the version, content digest, and (for slots) row digest.  This is
    the only discovery shortcut in the adapter; it is not an ID-to-path
    heuristic and it never searches a second catalog or a current fallback.
    """
    if type(value) is not dict or "layer" not in value:
        raise SourceReadError("source selector requires one exact typed layer")
    layer = value.get("layer")
    if layer == CSV_LAYER:
        if set(value) != {'layer', 'pack_id', 'edge_id'}:
            raise SourceReadError('authored CSV selector has an invalid closed envelope')
        pack, edge = _csv_identity(value.get('pack_id'), value.get('edge_id'))
        return {'layer': CSV_LAYER, 'pack_id': pack, 'edge_id': edge}
    if layer == METADATA_LAYER:
        if set(value) != {"layer", "record_type", "record_id"}:
            raise SourceReadError("metadata selector has an invalid closed envelope")
        record_type = value.get("record_type")
        record_id = value.get("record_id")
        if type(record_type) is not str or _RECORD_TYPE.fullmatch(record_type) is None:
            raise SourceReadError("metadata selector requires an owner-declared record type")
        if type(record_id) is not str or _METADATA_ID.fullmatch(record_id) is None:
            raise SourceReadError("metadata selector requires an exact metadata identity")
        return {"layer": METADATA_LAYER, "record_type": record_type, "record_id": record_id}
    if layer == CLAIM_LAYER:
        if set(value) != {"layer", "claim_id"}:
            raise SourceReadError("Claim selector has an invalid closed envelope")
        claim_id = value.get("claim_id")
        if type(claim_id) is not str or _CLAIM_ID.fullmatch(claim_id) is None:
            raise SourceReadError("Claim selector requires an exact Claim identity")
        return {"layer": CLAIM_LAYER, "claim_id": claim_id}
    if layer == SLOT_LAYER:
        if set(value) != {"layer", "slot_kind", "identity"}:
            raise SourceReadError("source-slot selector has an invalid closed envelope")
        slot_kind, identity = value.get("slot_kind"), value.get("identity")
        if type(slot_kind) is not str or slot_kind not in SLOT_KINDS or type(identity) is not str or _SLOT_ID.fullmatch(identity) is None:
            raise SourceReadError("source-slot selector requires one exact typed identity")
        if slot_kind == "claim" and _CLAIM_ID.fullmatch(identity) is None:
            raise SourceReadError("Claim source-slot selector requires an exact Claim identity")
        return {"layer": SLOT_LAYER, "slot_kind": slot_kind, "identity": identity}
    raise SourceReadError("source selector layer is unsupported")


def _csv_identity(pack, edge):
    if (type(pack) is not str or len(pack.encode('utf-8')) > 2048
            or not pack.startswith(('canon/relations/', 'candidate-intake/'))
            or any(part in {'', '.', '..', 'payload'} or part.startswith('.') for part in pack.split('/'))
            or any(char in pack for char in ('\\', '\x00'))
            or type(edge) is not str or not edge or len(edge.encode('utf-8')) > 2048 or '\x00' in edge):
        raise SourceReadError('authored CSV requires exact bounded pack and edge identities')
    return pack, edge


def exact_csv_target(payload):
    """Pure projection of a complete retained corpus-index row, not admission."""
    try:
        if type(payload) is not dict or type(payload.get('properties')) is not dict:
            return None
        fields = payload['properties']
        record = fields.get('source_record')
        if (type(record) is not dict or any(type(k) is not str or (v is not None and type(v) is not str) for k,v in record.items())):
            return None
        return _target({'layer': CSV_LAYER, 'pack_id': payload.get('pack_id'), 'edge_id': payload.get('edge_id'),
                        'source_row': fields.get('source_row'), 'source_file_sha256': fields.get('source_file_sha256'),
                        'content_revision': 'sha256:' + _canonical_digest(record)})
    except (SourceReadError, UnicodeError, TypeError, ValueError, RecursionError):
        return None


@dataclass(frozen=True)
class SourceEpoch:
    """One source-owner publication/vector selection.

    ``source_revision`` is supplied by the owner (for example the prepared
    source vector revision).  It is intentionally never computed from a path,
    record ID, or the selected content digest.
    """

    source_revision: str
    catalog_root_sha256: str
    catalog_namespace: str
    source_publication: dict[str, Any]

    def __post_init__(self) -> None:
        _bare_digest(self.source_revision, field="source revision")
        _bare_digest(self.catalog_root_sha256, field="catalog root digest")
        if type(self.catalog_namespace) is not str or _NAMESPACE.fullmatch(self.catalog_namespace) is None:
            raise SourceReadError("catalog namespace must be a bounded owner namespace")
        publication = _publication(self.source_publication)
        object.__setattr__(self, "source_publication", publication)

    @classmethod
    def from_value(cls, value: Any) -> "SourceEpoch":
        if type(value) is not dict or set(value) != {
            "source_revision", "catalog_root_sha256", "catalog_namespace", "source_publication"
        }:
            raise SourceReadError("source epoch has an invalid closed envelope")
        return cls(
            source_revision=value["source_revision"],
            catalog_root_sha256=value["catalog_root_sha256"],
            catalog_namespace=value["catalog_namespace"],
            source_publication=value["source_publication"],
        )

    def value(self) -> dict[str, Any]:
        return {
            "source_revision": self.source_revision,
            "catalog_root_sha256": self.catalog_root_sha256,
            "catalog_namespace": self.catalog_namespace,
            "source_publication": _copy(self.source_publication),
        }


def _reader_snapshot(reader: Any) -> Any | None:
    """Return the reader's addressed catalog snapshot, if it exposes one."""
    snapshot = getattr(reader, "catalog_snapshot", None)
    if snapshot is None:
        snapshot = getattr(reader, "_catalog_snapshot", None)
    return snapshot


def _reader_binding(reader: Any) -> SourceEpoch:
    """Read an owner-issued binding from a reader that has no catalog adapter."""
    issue = getattr(reader, "source_read_binding", None)
    if not callable(issue):
        raise SourceReadError("owner reader has no mandatory source-read binding")
    try:
        value = issue()
    except Exception as error:  # owner failures must not become a guessed epoch
        raise SourceReadError("owner reader could not issue a source-read binding") from error
    return value if isinstance(value, SourceEpoch) else SourceEpoch.from_value(value)


def _verify_reader(reader: Any) -> None:
    verify = getattr(reader, "verify_current", None)
    if not callable(verify):
        raise SourceReadError("owner reader has no mandatory currentness verifier")
    try:
        verify()
    except SourceReadError:
        raise
    except Exception as error:  # do not stamp an unverified reader as current
        raise SourceReadError("owner reader currentness verification failed") from error


@dataclass(frozen=True)
class SourceOwnerBinding:
    """Owner-issued source epoch plus the checks that make it usable.

    A caller cannot select an arbitrary ``SourceEpoch`` and attach a reader to
    it.  ``from_prepared_source`` derives the epoch from an immutable prepared
    source vector and an addressed catalog snapshot, then requires every
    selected reader to verify that same snapshot/publication.  The generic
    ``from_owner_readers`` path is for an owner adapter which can issue and
    re-verify the complete epoch itself; the adapter's binding is mandatory,
    never an optional attribute comparison.
    """

    epoch: SourceEpoch
    metadata_reader: Any | None = None
    claim_reader: Any | None = None
    slot_reader: Any | None = None
    authored_reader: Any | None = None
    catalog_snapshot: Any | None = None
    source_inputs: Any | None = None
    binding_kind: str = "owner-issued-reader"

    def __post_init__(self) -> None:
        if type(self.epoch) is not SourceEpoch:
            raise TypeError("owner binding requires a validated SourceEpoch")
        if not any(reader is not None for reader in (self.metadata_reader, self.claim_reader, self.slot_reader, self.authored_reader)):
            raise SourceReadError("owner binding requires at least one reader")
        if type(self.binding_kind) is not str or self.binding_kind not in {"owner-issued-reader", "prepared-source-vector"}:
            raise SourceReadError("owner binding kind is not supported")

    @classmethod
    def from_owner_readers(
        cls,
        *,
        metadata_reader: Any | None = None,
        claim_reader: Any | None = None,
        slot_reader: Any | None = None,
    ) -> "SourceOwnerBinding":
        readers = tuple(reader for reader in (metadata_reader, claim_reader, slot_reader) if reader is not None)
        if not readers:
            raise SourceReadError("at least one owner reader is required")
        epochs = []
        for reader in readers:
            epochs.append(_reader_binding(reader))
            _verify_reader(reader)
        if any(value != epochs[0] for value in epochs[1:]):
            raise SourceReadError("owner readers issue different source epochs")
        return cls(
            epoch=epochs[0], metadata_reader=metadata_reader, claim_reader=claim_reader,
            slot_reader=slot_reader, binding_kind="owner-issued-reader",
        )

    @classmethod
    def from_prepared_source(
        cls,
        source_inputs: Any,
        *,
        catalog_snapshot: Any,
        metadata_reader: Any | None = None,
        claim_reader: Any | None = None,
        slot_reader: Any | None = None,
    ) -> "SourceOwnerBinding":
        """Derive a binding from the existing prepared vector and catalog owner.

        The access plane intentionally does not reconstruct a source vector.
        The selected source assembler verifies the complete revision vector;
        constructing ``PreparedSourceInputs`` alone is not that verification.
        The assembler must be available on this explicit owner route. Readers
        must be addressed to that exact snapshot and expose their own bounded
        ``verify_current`` check before a handle can be issued.
        """
        try:
            from .prepared_source_binding import PreparedSourceInputs

            if type(source_inputs) is not PreparedSourceInputs:
                raise SourceReadError("an owner-issued prepared source vector is required")
            from source_agent_publication import _require_vector

            _require_vector(source_inputs)
            value = source_inputs.value()
            roots = source_inputs.roots()
        except SourceReadError:
            raise
        except Exception as error:
            raise SourceReadError("prepared source vector could not be verified") from error
        if type(catalog_snapshot) is type(None):
            raise SourceReadError("an addressed source catalog snapshot is required")
        selected = roots.get("source-catalog")
        if selected is None:
            raise SourceReadError("prepared source vector has no source-catalog root")
        selected_root = getattr(catalog_snapshot, "root_sha256", None)
        if type(selected_root) is not str or selected.snapshot_digest != selected_root:
            raise SourceReadError("prepared source vector and catalog snapshot differ")
        selected_path = getattr(selected, "namespace_path", None)
        catalog_path = getattr(getattr(catalog_snapshot, "view", None), "namespace_path", None)
        if selected_path != catalog_path:
            raise SourceReadError("prepared source vector and catalog namespace differ")
        header = getattr(catalog_snapshot, "header", None)
        if not isinstance(header, dict):
            raise SourceReadError("addressed catalog snapshot has no owner header")
        publication = header.get("source_publication")
        if not isinstance(publication, dict):
            raise SourceReadError("addressed catalog has no source publication")
        if value.get("source_publication") != publication.get("token"):
            raise SourceReadError("prepared source vector and catalog publication differ")
        if not isinstance(value.get("source_publication"), str):
            raise SourceReadError("prepared source vector has no exact source publication")
        epoch = SourceEpoch.from_value({
            "source_revision": value.get("source_revision"),
            "catalog_root_sha256": selected_root,
            "catalog_namespace": header.get("catalog_namespace"),
            "source_publication": publication,
        })
        readers = tuple(reader for reader in (metadata_reader, claim_reader, slot_reader) if reader is not None)
        if not readers:
            raise SourceReadError("at least one owner reader is required")
        for reader in readers:
            snapshot = _reader_snapshot(reader)
            if snapshot is None or getattr(snapshot, "root_sha256", None) != selected_root:
                raise SourceReadError("owner reader is not addressed to the selected catalog snapshot")
            reader_header = getattr(snapshot, "header", None)
            if reader_header != header:
                raise SourceReadError("owner reader catalog header differs from the selected epoch")
            _verify_reader(reader)
        binding = cls(
            epoch=epoch, metadata_reader=metadata_reader, claim_reader=claim_reader,
            slot_reader=slot_reader, catalog_snapshot=catalog_snapshot,
            source_inputs=source_inputs, binding_kind="prepared-source-vector",
        )
        binding.verify()
        return binding

    def verify(self) -> None:
        """Re-verify the owner epoch immediately before each read."""
        if self.source_inputs is not None:
            try:
                from .prepared_source_binding import PreparedSourceInputs

                if type(self.source_inputs) is not PreparedSourceInputs:
                    raise SourceReadError("source binding lost its prepared vector type")
                retained = PreparedSourceInputs.parse(self.source_inputs.raw)
                from source_agent_publication import _require_vector

                _require_vector(retained)
                if retained != self.source_inputs:
                    raise SourceReadError("prepared source vector changed during read")
                roots = retained.roots()
                selected = roots.get("source-catalog")
                if selected is None or selected.snapshot_digest != self.epoch.catalog_root_sha256:
                    raise SourceReadError("prepared source vector no longer binds the selected catalog")
                value = retained.value()
                snapshot = self.catalog_snapshot
                if selected.namespace_path != getattr(getattr(snapshot, "view", None), "namespace_path", None):
                    raise SourceReadError("prepared source namespace binding changed")
                header = getattr(snapshot, "header", None)
                publication = header.get("source_publication") if isinstance(header, dict) else None
                if (value.get("source_revision") != self.epoch.source_revision
                        or value.get("source_publication") != self.epoch.source_publication.get("token")
                        or selected.snapshot_digest != getattr(snapshot, "root_sha256", None)
                        or not isinstance(publication, dict)
                        or publication != self.epoch.source_publication):
                    raise SourceReadError("prepared source epoch is no longer current")
            except SourceReadError:
                raise
            except Exception as error:
                raise SourceReadError("prepared source vector verification failed") from error
        for reader in (self.metadata_reader, self.claim_reader, self.slot_reader):
            if reader is None:
                continue
            snapshot = _reader_snapshot(reader)
            if self.catalog_snapshot is not None:
                if snapshot is None or getattr(snapshot, "root_sha256", None) != self.epoch.catalog_root_sha256:
                    raise SourceReadError("owner reader catalog binding changed")
                if getattr(snapshot, "header", None) != getattr(self.catalog_snapshot, "header", None):
                    raise SourceReadError("owner reader publication binding changed")
            else:
                issued = _reader_binding(reader)
                if issued != self.epoch:
                    raise SourceReadError("owner reader issued another source epoch")
            _verify_reader(reader)
        if self.authored_reader is not None:
            if self.source_inputs is None:
                raise SourceReadError('authored reader requires an explicit prepared source vector')
            selected = self.source_inputs.roots().get('authored-corpus')
            view = getattr(self.authored_reader, 'view', None)
            if (selected is None or view != selected or _reader_binding(self.authored_reader) != self.epoch):
                raise SourceReadError('authored reader differs from selected source vector')
            _verify_reader(self.authored_reader)


class SourceCatalogTargetIssuer:
    """Issue exact targets from one already-selected public catalog snapshot.

    This adapter is deliberately catalog-only: it reads one addressed row by
    typed identity and copies the owner-recorded version/content digest into a
    target.  It never accepts a path, digest supplied by the caller, range, or
    ``latest`` flag.  The surrounding service checks that this issuer is bound
    to the same snapshot as the owner readers.
    """

    def __init__(self, catalog_snapshot: Any, *, authored_reader: Any | None = None):
        if catalog_snapshot is None:
            raise TypeError("an addressed source catalog snapshot is required")
        root = getattr(catalog_snapshot, "root_sha256", None)
        header = getattr(catalog_snapshot, "header", None)
        if type(root) is not str or _HEX.fullmatch(root) is None or not isinstance(header, dict):
            raise SourceReadError("target issuer requires an addressed owner catalog")
        self.catalog_snapshot = catalog_snapshot
        self.authored_reader = authored_reader
        self._root_sha256 = root
        self._header = _copy(header)

    def verify_current(self) -> None:
        """Ensure the issuer still points at the same immutable catalog root."""
        if (getattr(self.catalog_snapshot, "root_sha256", None) != self._root_sha256
                or getattr(self.catalog_snapshot, "header", None) != self._header):
            raise SourceReadError("owner target catalog binding changed")

    def issue(self, selector: dict[str, Any]) -> dict[str, Any]:
        """Resolve one typed selector into the owner's full exact target."""
        self.verify_current()
        selector = _selector(selector)
        try:
            if selector['layer'] == CSV_LAYER:
                if self.authored_reader is None:
                    raise _OwnerUnavailable('unsupported', 'authored-corpus-owner-unconfigured')
                target = self.authored_reader.issue(selector['pack_id'], selector['edge_id'])
                if target is None:
                    raise _OwnerUnavailable('missing', 'authored-corpus-target-missing')
                return _target(target)
            if selector["layer"] == METADATA_LAYER:
                row = self.catalog_snapshot.lookup(selector["record_id"])
                if row is None:
                    raise _OwnerUnavailable("missing", "owner-catalog-target-missing")
                entry, source = row.entry, row.source
                if entry.get("record_type") != selector["record_type"]:
                    raise SourceReadError("owner catalog target type differs")
                reference = source.get("record_ref")
                return _target({
                    "layer": METADATA_LAYER,
                    "record_type": selector["record_type"],
                    "record_ref": reference,
                    "content_revision": reference.get("digest") if isinstance(reference, dict) else None,
                })
            if selector["layer"] == CLAIM_LAYER:
                row = self.catalog_snapshot.lookup_claim(selector["claim_id"])
                if row is None:
                    raise _OwnerUnavailable("missing", "owner-catalog-claim-target-missing")
                reference = row.claim_ref
                return _target({
                    "layer": CLAIM_LAYER,
                    "record_ref": reference,
                    "content_revision": reference.get("digest") if isinstance(reference, dict) else None,
                })
            row = self.catalog_snapshot.lookup_slot(selector["slot_kind"], selector["identity"])
            if row is None:
                raise _OwnerUnavailable("missing", "owner-catalog-source-slot-target-missing")
            source = row.source
            return _target({
                "layer": SLOT_LAYER,
                "slot_kind": selector["slot_kind"],
                "identity": selector["identity"],
                "row_sha256": row.row_sha256,
                "content_revision": "sha256:" + source.get("canonical_sha256", ""),
            })
        except SourceReadError:
            raise
        except FileNotFoundError as error:
            raise _OwnerUnavailable("missing", "owner-catalog-part-missing") from error
        except PermissionError as error:
            raise _OwnerUnavailable("access-restricted", "owner-catalog-access-restricted") from error
        except OSError as error:
            raise _OwnerUnavailable("corrupt", "owner-catalog-read-failed") from error
        except (TypeError, KeyError, AttributeError, ValueError) as error:
            if error.__class__.__name__ == "SourceCatalogRequiresBootstrap":
                raise SourceReadError("owner catalog target requires explicit source bootstrap") from error
            raise SourceReadError("owner catalog target is corrupt") from error


class SourceCatalogClaimReader:
    """Expose current catalog-addressed Claims through the common resolver.

    ``SourceCatalogSourceReader.read_claim`` is the existing source owner for
    current Claim rows.  This adapter only translates its exact slot result to
    the metadata/Claim reader envelope; it has no historical or text fallback.
    Its addressed snapshot is mandatory so the parent binding can pair it with
    the prepared source vector.
    """

    def __init__(self, source_reader: Any):
        snapshot = _reader_snapshot(source_reader)
        if snapshot is None or not callable(getattr(source_reader, "read_claim", None)):
            raise TypeError("an addressed current Claim source reader is required")
        self.source_reader = source_reader
        self.catalog_snapshot = snapshot
        self._root_sha256 = getattr(snapshot, "root_sha256", None)
        self._header = _copy(getattr(snapshot, "header", {}))
        if type(self._root_sha256) is not str or _HEX.fullmatch(self._root_sha256) is None:
            raise SourceReadError("Claim reader requires an addressed owner catalog")

    def verify_current(self) -> None:
        if (getattr(self.catalog_snapshot, "root_sha256", None) != self._root_sha256
                or getattr(self.catalog_snapshot, "header", None) != self._header):
            raise SourceReadError("owner Claim catalog binding changed")
        self.source_reader.verify_current()

    def resolve(self, exact_ref: dict[str, Any]) -> dict[str, Any]:
        result = {
            "status": None,
            "reason": None,
            "exact_ref": _copy(exact_ref),
            "version_status": "current",
            "record": None,
            "record_digest": None,
            "provenance": None,
            "grants_current_use": False,
            "performs_assessment": False,
            "writes_to_source": False,
        }
        try:
            self.verify_current()
            row = self.catalog_snapshot.lookup_claim(exact_ref["id"])
            if row is None:
                return {**result, "status": "missing", "reason": "claim-not-in-public-catalog"}
            if row.claim_ref != exact_ref:
                return {**result, "status": "stale", "reason": "exact-version-digest-mismatch"}
            selected = self.source_reader.read_claim(
                exact_ref["id"], expected_row_sha256=row.row_sha256
            )
            record = selected.payload
            if not isinstance(record, dict):
                return {**result, "status": "corrupt", "reason": "claim-source-row-is-not-an-object"}
            if (record.get("claim_id") != exact_ref["id"]
                    or type(record.get("claim_version")) is not int
                    or "sha256:" + _canonical_digest(record) != exact_ref["digest"]):
                return {**result, "status": "stale", "reason": "claim-source-row-digest-mismatch"}
            return {
                **result,
                "status": "available",
                "reason": "exact-current-version",
                "exact_ref": copy.deepcopy(exact_ref),
                "record": copy.deepcopy(record),
                "record_digest": exact_ref["digest"],
                "provenance": copy.deepcopy(selected.provenance),
            }
        except SourceReadError:
            raise
        except PermissionError:
            return {**result, "status": "access-restricted", "reason": "claim-source-path-restricted"}
        except FileNotFoundError:
            return {**result, "status": "missing", "reason": "claim-source-row-missing"}
        except ValueError as error:
            if error.__class__.__name__ == "SourceCatalogRequiresBootstrap":
                return {**result, "status": "unsupported", "reason": "claim-source-bootstrap-required"}
            return {**result, "status": "corrupt", "reason": "claim-source-catalog-invalid"}
        except OSError:
            return {**result, "status": "stale", "reason": "claim-source-changed-during-read"}


@dataclass(frozen=True)
class SourceReadLimits:
    """Delivery limits; they do not enlarge an owner reader's own limits."""

    max_handle_bytes: int = MAX_HANDLE_BYTES
    max_request_bytes: int = MAX_REQUEST_BYTES
    max_record_bytes: int = MAX_RECORD_BYTES
    max_response_bytes: int = MAX_RESPONSE_BYTES

    def __post_init__(self) -> None:
        if any(type(value) is not int or value < 0 for value in vars(self).values()):
            raise SourceReadError("source read limits must be nonnegative integers")


def _access(scope: str, visibility: str, *, visibility_verified: bool) -> dict[str, Any]:
    if type(scope) is not str or scope not in {
        "public-metadata-record", "public-claim-record", "public-source-slot-metadata", "public-authored-csv-record"
    }:
        raise SourceReadError("source access scope is not supported")
    if type(visibility) is not str or visibility not in PUBLIC_VISIBILITIES:
        raise SourceReadError("source visibility is not public metadata")
    if visibility_verified is not True:
        raise SourceReadError("source visibility must be explicitly verified by its owner")
    return {
        "scope": scope,
        "visibility": visibility,
        "visibility_verified": True,
        # Public metadata disclosure is not a usage-rights or licence grant.
        "rights_revalidated": False,
        "rights_scope": "metadata-disclosure-only",
        "authority": "source-owner-public-metadata-contract",
    }


def _target(value: Any) -> dict[str, Any]:
    if type(value) is not dict or "layer" not in value:
        raise SourceReadError("source target requires one exact layer")
    layer = value.get("layer")
    if layer == CSV_LAYER:
        if set(value) != {'layer', 'pack_id', 'edge_id', 'source_row', 'source_file_sha256', 'content_revision'}:
            raise SourceReadError('authored CSV target has an invalid closed envelope')
        pack, edge = _csv_identity(value.get('pack_id'), value.get('edge_id'))
        ordinal = value.get('source_row')
        if type(ordinal) is not int or not 1 <= ordinal <= MAX_SAFE_INTEGER:
            raise SourceReadError('authored CSV target requires an exact logical row ordinal')
        return {'layer': CSV_LAYER, 'pack_id': pack, 'edge_id': edge, 'source_row': ordinal,
                'source_file_sha256': _bare_digest(value.get('source_file_sha256'), field='authored file digest'),
                'content_revision': _sha_digest(value.get('content_revision'), field='authored record digest')}
    if layer == METADATA_LAYER:
        if set(value) != {"layer", "record_type", "record_ref", "content_revision"}:
            raise SourceReadError("metadata target has an invalid closed envelope")
        record_type = value.get("record_type")
        if type(record_type) is not str or _RECORD_TYPE.fullmatch(record_type) is None:
            raise SourceReadError("metadata target requires an owner-declared record type")
        reference = _exact_ref(value.get("record_ref"), claim=False)
        content_revision = _sha_digest(value.get("content_revision"), field="content revision")
        if content_revision != reference["digest"]:
            raise SourceReadError("metadata content revision differs from its exact record digest")
        return {
            "layer": METADATA_LAYER,
            "record_type": record_type,
            "record_ref": reference,
            "content_revision": content_revision,
        }
    if layer == CLAIM_LAYER:
        if set(value) != {"layer", "record_ref", "content_revision"}:
            raise SourceReadError("Claim target has an invalid closed envelope")
        reference = _exact_ref(value.get("record_ref"), claim=True)
        content_revision = _sha_digest(value.get("content_revision"), field="content revision")
        if content_revision != reference["digest"]:
            raise SourceReadError("Claim content revision differs from its exact record digest")
        return {"layer": CLAIM_LAYER, "record_ref": reference, "content_revision": content_revision}
    if layer == SLOT_LAYER:
        if set(value) != {"layer", "slot_kind", "identity", "row_sha256", "content_revision"}:
            raise SourceReadError("source-slot target has an invalid closed envelope")
        slot_kind, identity = value.get("slot_kind"), value.get("identity")
        if type(slot_kind) is not str or slot_kind not in SLOT_KINDS or type(identity) is not str or _SLOT_ID.fullmatch(identity) is None:
            raise SourceReadError("source-slot target requires one exact typed identity")
        if slot_kind == "claim" and _CLAIM_ID.fullmatch(identity) is None:
            raise SourceReadError("Claim source-slot identity is not an exact Claim ID")
        row_sha256 = _bare_digest(value.get("row_sha256"), field="source-slot catalog row digest")
        content_revision = _sha_digest(value.get("content_revision"), field="content revision")
        return {
            "layer": SLOT_LAYER,
            "slot_kind": slot_kind,
            "identity": identity,
            "row_sha256": row_sha256,
            "content_revision": content_revision,
        }
    raise SourceReadError("source target layer is unsupported")


def _metadata_identity(record: dict[str, Any]) -> tuple[str, str] | None:
    native = {
        "tos_scholarly_composite_witness_v1": ("composite", "composite_id"),
        "tos_artifact_source_witness_v1": ("artifact", "artifact_id"),
        "tos_artifact_source_witness_v2": ("artifact", "artifact_id"),
    }
    schema = record.get("schema_version")
    if isinstance(schema, (dict, list)):
        return None
    if isinstance(schema, str) and schema in native:
        kind, field = native[schema]
        if "record_id" in record or "record_type" in record:
            return None
        identifier = record.get(field)
        return (kind, field) if isinstance(identifier, str) and identifier.startswith("tos." + kind + ".") else None
    kind = record.get("record_type")
    return (kind, "record_id") if isinstance(kind, str) else None


def exact_target_from_record(record: Any, *, layer: str) -> dict[str, Any] | None:
    """Project one complete raw owner record into the existing target ABI.

    This is a pure projection used by graph inspection.  It does not consult a
    catalog, source path, or current owner binding; the owner reader remains
    responsible for rechecking the returned target before issuing a handle.
    Invalid or unsupported raw records are deliberately omitted rather than
    turned into an inferred selector or an availability claim.
    """
    if layer not in (METADATA_LAYER, CLAIM_LAYER) or type(record) is not dict:
        return None
    try:
        if layer == CLAIM_LAYER:
            identifier = record.get("claim_id")
            version = record.get("claim_version")
            if type(identifier) is not str or type(version) is not int:
                return None
            reference = {
                "id": identifier,
                "version": version,
                "digest": "sha256:" + _canonical_digest(record),
            }
            return _target({
                "layer": CLAIM_LAYER,
                "record_ref": reference,
                "content_revision": reference["digest"],
            })

        identity = _metadata_identity(record)
        if identity is None:
            return None
        record_type, identity_field = identity
        identifier = record.get(identity_field)
        version = record.get("record_version")
        if (
            type(identifier) is not str
            or type(record_type) is not str
            or type(version) is not int
        ):
            return None
        reference = {
            "id": identifier,
            "version": version,
            "digest": "sha256:" + _canonical_digest(record),
        }
        return _target({
            "layer": METADATA_LAYER,
            "record_type": record_type,
            "record_ref": reference,
            "content_revision": reference["digest"],
        })
    except (SourceReadError, TypeError, ValueError, OverflowError, RecursionError, UnicodeError):
        return None


def _handle_access(value: Any, target: dict[str, Any]) -> dict[str, Any]:
    if type(value) is not dict or set(value) != {
        "scope", "visibility", "visibility_verified", "rights_revalidated", "rights_scope", "authority"
    }:
        raise SourceReadError("source handle access has an invalid closed envelope")
    if value.get("visibility_verified") is not True:
        raise SourceReadError("source handle must carry an owner visibility check")
    if value.get("rights_revalidated") is not False or value.get("rights_scope") != "metadata-disclosure-only":
        raise SourceReadError("source handle must not imply usage rights")
    expected_scope = {
        METADATA_LAYER: "public-metadata-record",
        CLAIM_LAYER: "public-claim-record",
        SLOT_LAYER: "public-source-slot-metadata",
        CSV_LAYER: "public-authored-csv-record",
    }[target["layer"]]
    if value.get("scope") != expected_scope or value.get("authority") != "source-owner-public-metadata-contract":
        raise SourceReadError("source handle access scope does not match its typed layer")
    if type(value.get("visibility")) is not str or value["visibility"] not in PUBLIC_VISIBILITIES:
        raise SourceReadError("source handle visibility is not public metadata")
    return _access(value["scope"], value["visibility"], visibility_verified=True)


def _handle_core(value: Any) -> dict[str, Any]:
    if type(value) is not dict or set(value) != {
        "schema_version", "issuer", "epoch", "target", "access", "handle_digest"
    }:
        raise SourceReadError("source handle has an invalid closed envelope")
    if value.get("schema_version") != HANDLE_SCHEMA:
        raise SourceReadError("source handle issuer or schema is not supported")
    epoch = SourceEpoch.from_value(value.get("epoch"))
    target = _target(value.get("target"))
    issuer = AUTHORED_ISSUER if target['layer'] == CSV_LAYER else ISSUER
    if value.get('issuer') != issuer:
        raise SourceReadError('source handle issuer differs from its owner layer')
    access = _handle_access(value.get("access"), target)
    digest = _sha_digest(value.get("handle_digest"), field="source handle digest")
    unsigned = {key: _copy(value[key]) for key in ("schema_version", "issuer", "epoch", "target", "access")}
    expected = "sha256:" + hashlib.sha256(_canonical_bytes(unsigned)).hexdigest()
    if digest != expected:
        raise SourceReadError("source handle digest does not bind its exact target and epoch")
    return {
        "schema_version": HANDLE_SCHEMA,
        "issuer": issuer,
        # Keep the validated wire form here.  Internal SourceEpoch objects
        # must never leak into budget/error serialization paths.
        "epoch": epoch.value(),
        "target": target,
        "access": access,
        "handle_digest": digest,
    }


def validate_handle(value: Any, *, limits: SourceReadLimits | None = None) -> dict[str, Any]:
    """Validate and detach one opaque owner-issued handle."""
    limits = SourceReadLimits() if limits is None else limits
    if not isinstance(limits, SourceReadLimits):
        raise TypeError("source read limits must be SourceReadLimits")
    raw = _canonical_bytes(value)
    if len(raw) > limits.max_handle_bytes:
        raise SourceReadBudgetExceeded("source handle exceeds its byte budget")
    return _handle_core(value)


def _make_handle(epoch: SourceEpoch, target: dict[str, Any], access: dict[str, Any], limits: SourceReadLimits) -> dict[str, Any]:
    unsigned = {
        "schema_version": HANDLE_SCHEMA,
        "issuer": AUTHORED_ISSUER if target['layer'] == CSV_LAYER else ISSUER,
        "epoch": epoch.value(),
        "target": _copy(target),
        "access": _copy(access),
    }
    handle = {**unsigned, "handle_digest": "sha256:" + hashlib.sha256(_canonical_bytes(unsigned)).hexdigest()}
    validate_handle(handle, limits=limits)
    return handle


def _status(value: Any) -> tuple[str, str]:
    if not isinstance(value, dict):
        return "corrupt", "owner-reader-returned-no-envelope"
    status, reason = value.get("status"), value.get("reason")
    if type(status) is not str or status not in STATUSES:
        return "corrupt", "owner-reader-returned-unknown-status"
    if type(reason) is not str or not 1 <= len(reason) <= 256:
        return "corrupt", "owner-reader-returned-invalid-reason"
    return status, reason


def _visibility(record: dict[str, Any], descriptor: dict[str, Any] | None = None) -> str:
    has_record_visibility = "visibility" in record
    value = record.get("visibility")
    if has_record_visibility and (type(value) is not str or value not in PUBLIC_VISIBILITIES):
        raise SourceReadError("owner reader returned a non-public metadata record")
    if isinstance(descriptor, dict) and "source_scope" in descriptor:
        if type(descriptor["source_scope"]) is not str or descriptor["source_scope"] not in PUBLIC_VISIBILITIES:
            raise SourceReadError("owner metadata descriptor has a non-public source scope")
        if has_record_visibility and value != descriptor["source_scope"]:
            raise SourceReadError("owner metadata descriptor and record visibility differ")
        return descriptor["source_scope"]
    if type(value) is str and value in PUBLIC_VISIBILITIES:
        return value
    # Native Corpus metadata has no visibility field; its owner reader's
    # public-corpus adapter is the explicit visibility declaration.
    if isinstance(descriptor, dict) and descriptor.get("adapter") == "native-corpus":
        return "public_metadata_only"
    raise SourceReadError("owner reader returned a non-public metadata record")


def _record_size(record: Any, limits: SourceReadLimits) -> None:
    size = len(_canonical_bytes(record))
    if size > limits.max_record_bytes:
        raise SourceReadBudgetExceeded("exact source record exceeds its byte budget")


def _request_size(request: Any, limits: SourceReadLimits) -> None:
    """Apply the service request cap for non-HTTP callers too.

    HTTP enforces the same bound before parsing, but MCP and direct owner use
    must not get a larger implicit request budget merely by bypassing that
    transport.  Canonicalization also rejects NaN/non-JSON values before any
    owner lookup.
    """
    if len(_canonical_bytes(request)) > limits.max_request_bytes:
        raise SourceReadBudgetExceeded("source request exceeds its byte budget")


def _provenance(value: Any, limits: SourceReadLimits) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise SourceReadError("owner provenance is not an object")
    detached = _copy(value)
    if len(_canonical_bytes(detached)) > limits.max_response_bytes:
        raise SourceReadBudgetExceeded("exact source provenance exceeds its byte budget")
    return detached


def _base_discovery(epoch: SourceEpoch, target: dict[str, Any] | None) -> dict[str, Any]:
    return {
        "schema_version": DISCOVERY_SCHEMA,
        "status": "unsupported",
        "reason": "owner-reader-not-configured",
        "target": _copy(target),
        "handle": None,
        "source_revision": epoch.source_revision,
        "content_revision": target["content_revision"] if target is not None else None,
        "provenance": None,
        "access": None,
        "grants_current_use": False,
        "performs_assessment": False,
        "writes_to_source": False,
    }


def _base_result(epoch: SourceEpoch, handle: dict[str, Any]) -> dict[str, Any]:
    target = handle["target"]
    return {
        "schema_version": RESULT_SCHEMA,
        "status": "unsupported",
        "reason": "owner-reader-not-configured",
        "handle": _copy(handle),
        "source_revision": epoch.source_revision,
        "content_revision": target["content_revision"],
        "layer": target["layer"],
        "record_kind": "metadata" if target["layer"] == METADATA_LAYER else
        "claim" if target["layer"] == CLAIM_LAYER else "authored_csv" if target['layer'] == CSV_LAYER else "source_slot",
        "record_ref": _copy(target.get("record_ref")) if "record_ref" in target else None,
        "record": None,
        "provenance": None,
        "access": _copy(handle["access"]),
        "grants_current_use": False,
        "performs_assessment": False,
        "writes_to_source": False,
    }


class SourceReadService:
    """Compose source-owner readers behind the exact access ABI.

    ``metadata_reader`` is expected to expose ``resolve_typed(exact_ref)``;
    ``claim_reader`` exposes ``resolve(exact_ref)``; and ``slot_reader``
    exposes ``read_slot(kind, identity, expected_row_sha256=...)`` plus an
    addressed catalog snapshot.  These are deliberately duck-typed so the
    access package does not import mechanics scripts as a second source truth,
    but the surrounding ``SourceOwnerBinding`` is not optional: it is the
    owner-issued proof that the readers share one verified epoch.  An optional
    ``target_issuer`` must carry the same binding and can turn a typed catalog
    selector into a full owner target; it never accepts a path or caller digest.
    """

    def __init__(
        self,
        binding: SourceOwnerBinding,
        *,
        metadata_reader: Any | None = None,
        claim_reader: Any | None = None,
        slot_reader: Any | None = None,
        target_issuer: Any | None = None,
        metadata_record_types: set[str] | frozenset[str] | tuple[str, ...] = (),
        limits: SourceReadLimits | None = None,
        slot_descriptor: Callable[[str, str], dict[str, Any]] | None = None,
    ) -> None:
        if type(binding) is not SourceOwnerBinding:
            raise TypeError("an owner-issued SourceOwnerBinding is required")
        supplied = (metadata_reader, claim_reader, slot_reader)
        bound = (binding.metadata_reader, binding.claim_reader, binding.slot_reader)
        if any(value is not None and value is not selected for value, selected in zip(supplied, bound)):
            raise SourceReadError("source reader differs from the owner-issued binding")
        if target_issuer is not None:
            if not callable(getattr(target_issuer, "issue", None)) or not callable(
                getattr(target_issuer, "verify_current", None)
            ):
                raise SourceReadError("owner target issuer has no mandatory binding verifier")
            issuer_snapshot = _reader_snapshot(target_issuer)
            if binding.catalog_snapshot is not None:
                if issuer_snapshot is not binding.catalog_snapshot:
                    raise SourceReadError("owner target issuer differs from the selected catalog snapshot")
            else:
                issuer_binding = getattr(target_issuer, "source_read_binding", None)
                if not callable(issuer_binding) or _reader_binding(target_issuer) != binding.epoch:
                    raise SourceReadError("owner target issuer is not bound to the selected source epoch")
        self.binding = binding
        self.epoch = binding.epoch
        self.metadata_reader = binding.metadata_reader
        self.claim_reader = binding.claim_reader
        self.slot_reader = binding.slot_reader
        self.authored_reader = binding.authored_reader
        if target_issuer is not None and getattr(target_issuer, 'authored_reader', None) is not binding.authored_reader:
            raise SourceReadError('authored target issuer differs from selected owner reader')
        self.target_issuer = target_issuer
        self.limits = SourceReadLimits() if limits is None else limits
        if not isinstance(self.limits, SourceReadLimits):
            raise TypeError("source read limits must be SourceReadLimits")
        if type(metadata_record_types) not in (set, frozenset, tuple):
            raise TypeError("metadata record types must be a bounded collection")
        if any(type(value) is not str or _RECORD_TYPE.fullmatch(value) is None for value in metadata_record_types):
            raise SourceReadError("metadata record types must be owner-declared identifiers")
        self.metadata_record_types = frozenset(metadata_record_types)
        self.slot_descriptor = slot_descriptor
        self.binding.verify()
        self._verify_target_issuer()

    def _verify_target_issuer(self) -> None:
        if self.authored_reader is not self.binding.authored_reader:
            raise SourceReadError('authored reader changed after owner binding')
        if self.target_issuer is not None:
            if getattr(self.target_issuer, 'authored_reader', None) is not self.authored_reader:
                raise SourceReadError('authored issuer changed after owner binding')
            try:
                self.target_issuer.verify_current()
            except SourceReadError:
                raise
            except Exception as error:
                raise SourceReadError("owner target issuer currentness verification failed") from error

    def capabilities(self) -> dict[str, Any]:
        return {
            "schema_version": CAPABILITIES_SCHEMA,
            "available": bool(self.metadata_reader or self.claim_reader or self.slot_reader or self.authored_reader),
            "issuer": ISSUER,
            "layers": {
                METADATA_LAYER: sorted(self.metadata_record_types) if self.metadata_reader else [],
                CLAIM_LAYER: bool(self.claim_reader),
                SLOT_LAYER: bool(self.slot_reader),
                CSV_LAYER: bool(self.authored_reader),
            },
            "limits": {
                "max_handle_bytes": self.limits.max_handle_bytes,
                "max_request_bytes": self.limits.max_request_bytes,
                "max_record_bytes": self.limits.max_record_bytes,
                "max_response_bytes": self.limits.max_response_bytes,
            },
            "source_epoch": self.epoch.value(),
            "binding": {
                "kind": self.binding.binding_kind,
                "catalog_root_sha256": self.epoch.catalog_root_sha256,
                "source_inputs_sha256": (
                    self.binding.source_inputs.digest if self.binding.source_inputs is not None else None
                ),
            },
            "authority": {
                "is_source": False,
                "writes_to_source": False,
                "grants_current_use": False,
                "native_text_payload": False,
                "note": "Owner readers retain source, rights, and currentness authority; this adapter discloses only owner-declared public metadata and grants no use rights.",
            },
        }

    def _metadata_result(self, target: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
        if self.metadata_reader is None:
            raise SourceReadError("metadata-owner-reader-unconfigured")
        if target["record_type"] not in self.metadata_record_types:
            raise SourceReadError("metadata-type-not-owner-declared")
        result = self.metadata_reader.resolve_typed(target["record_ref"])
        status, reason = _status(result)
        if status != "available":
            raise _OwnerUnavailable(status, reason)
        record = result.get("record")
        if not isinstance(record, dict):
            raise SourceReadError("owner metadata reader returned no exact record")
        descriptor = result.get("descriptor")
        if not isinstance(descriptor, dict) or descriptor.get("record_type") != target["record_type"]:
            raise SourceReadError("owner metadata descriptor does not bind the requested record type")
        owner_ref = result.get("exact_ref")
        if owner_ref != target["record_ref"]:
            raise SourceReadError("owner metadata reader did not return the requested exact reference")
        identity = _metadata_identity(record)
        if identity is None or identity[0] != target["record_type"]:
            raise SourceReadError("owner metadata record type differs")
        identity_field = identity[1]
        if identity_field != "record_id" and (descriptor.get("adapter") != "native-witness"
                or descriptor.get("identity_field") != identity_field):
            raise SourceReadError("owner native metadata descriptor identity differs")
        if record.get(identity_field) != target["record_ref"]["id"] or record.get("record_version") != target["record_ref"]["version"]:
            raise SourceReadError("owner metadata record identity or version differs")
        if "sha256:" + _canonical_digest(record) != target["content_revision"]:
            raise SourceReadError("owner metadata content digest differs")
        _record_size(record, self.limits)
        visibility = _visibility(record, descriptor)
        provenance = _provenance(result.get("provenance"), self.limits)
        return record, provenance, _access("public-metadata-record", visibility, visibility_verified=True)

    def _claim_result(self, target: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
        if self.claim_reader is None:
            raise SourceReadError("claim-owner-reader-unconfigured")
        result = self.claim_reader.resolve(target["record_ref"])
        status, reason = _status(result)
        if status != "available":
            raise _OwnerUnavailable(status, reason)
        record = result.get("record")
        if not isinstance(record, dict):
            raise SourceReadError("owner Claim reader returned no exact record")
        owner_ref = result.get("exact_ref")
        if owner_ref != target["record_ref"]:
            raise SourceReadError("owner Claim reader did not return the requested exact reference")
        if record.get("claim_id") != target["record_ref"]["id"] or record.get("claim_version") != target["record_ref"]["version"]:
            raise SourceReadError("owner Claim record identity or version differs")
        if "sha256:" + _canonical_digest(record) != target["content_revision"]:
            raise SourceReadError("owner Claim content digest differs")
        _record_size(record, self.limits)
        visibility = _visibility(record)
        provenance = _provenance(result.get("provenance"), self.limits)
        return record, provenance, _access("public-claim-record", visibility, visibility_verified=True)

    def _slot_descriptor(self, kind: str, identity: str) -> dict[str, Any]:
        if self.slot_descriptor is not None:
            value = self.slot_descriptor(kind, identity)
        else:
            snapshot = _reader_snapshot(self.slot_reader)
            if snapshot is None:
                raise SourceReadError("addressed source-slot catalog is not configured")
            value = snapshot.get_slot(kind, identity)
            value = {
                "row_sha256": value.row_sha256,
                "canonical_sha256": value.source.get("canonical_sha256"),
                "visibility": value.source.get("visibility"),
                "provenance": value.provenance,
            }
        if not isinstance(value, dict):
            raise SourceReadError("source-slot owner returned no descriptor")
        row_sha256 = _bare_digest(value.get("row_sha256"), field="source-slot catalog row digest")
        canonical_sha256 = _bare_digest(value.get("canonical_sha256"), field="source-slot content digest")
        visibility = value.get("visibility")
        if type(visibility) is not str or visibility not in PUBLIC_VISIBILITIES:
            raise SourceReadError("source-slot owner must declare public visibility")
        return {
            "row_sha256": row_sha256,
            "canonical_sha256": canonical_sha256,
            "visibility": visibility,
            "provenance": _provenance(value.get("provenance") or {}, self.limits),
        }

    @staticmethod
    def _slot_identity_field(kind: str) -> str:
        return {"claim": "claim_id", "provenance_event": "event_id", "anchor": "anchor_id"}[kind]

    def _slot_result(self, target: dict[str, Any], descriptor: dict[str, Any] | None = None) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
        if self.slot_reader is None:
            raise SourceReadError("source-slot-owner-reader-unconfigured")
        descriptor = (
            self._slot_descriptor(target["slot_kind"], target["identity"])
            if descriptor is None else descriptor
        )
        if descriptor["row_sha256"] != target["row_sha256"] or "sha256:" + descriptor["canonical_sha256"] != target["content_revision"]:
            raise _OwnerUnavailable("stale", "source-slot-descriptor-differs")
        selected = self.slot_reader.read_slot(
            target["slot_kind"], target["identity"], expected_row_sha256=target["row_sha256"]
        )
        selected_row = getattr(selected, "row_sha256", None)
        if selected_row is None:
            selected_slot = getattr(selected, "slot", None)
            selected_row = getattr(selected_slot, "row_sha256", None)
        if selected_row != target["row_sha256"]:
            raise SourceReadError("owner source-slot row digest differs")
        payload = getattr(selected, "payload", None)
        if not isinstance(payload, dict):
            raise SourceReadError("owner source-slot reader returned no exact payload")
        if payload.get(self._slot_identity_field(target["slot_kind"])) != target["identity"]:
            raise SourceReadError("owner source-slot identity differs")
        if "sha256:" + _canonical_digest(payload) != target["content_revision"]:
            raise SourceReadError("owner source-slot content digest differs")
        raw_bytes = getattr(selected, "raw_bytes", None)
        if isinstance(raw_bytes, bytes) and len(raw_bytes) > self.limits.max_record_bytes:
            raise SourceReadBudgetExceeded("exact source-slot row exceeds its byte budget")
        _record_size(payload, self.limits)
        visibility = descriptor["visibility"]
        payload_visibility = payload.get("visibility")
        if payload_visibility is not None and (type(payload_visibility) is not str or payload_visibility not in PUBLIC_VISIBILITIES):
            raise SourceReadError("source-slot payload has restricted visibility")
        if payload_visibility is not None and payload_visibility != visibility:
            raise SourceReadError("source-slot descriptor and payload visibility differ")
        provenance = _provenance(getattr(selected, "provenance", None) or descriptor["provenance"], self.limits)
        return payload, provenance, _access("public-source-slot-metadata", visibility, visibility_verified=True)

    def _owner_target(self, target: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
        if target['layer'] == CSV_LAYER:
            if self.authored_reader is None:
                raise SourceReadError('authored-corpus-owner-unconfigured')
            try:
                returned = self.authored_reader.read(target)
            except SourceReadBudgetExceeded:
                raise
            except FileNotFoundError:
                raise
            except ValueError as error:
                raise SourceReadError('authored-csv-source-binding-not-verified') from error
            if returned is None:
                raise _OwnerUnavailable('missing', 'authored-corpus-target-missing')
            record, provenance = returned
            _record_size(record, self.limits)
            if 'sha256:' + _canonical_digest(record) != target['content_revision']:
                raise SourceReadError('authored CSV content digest differs')
            self.binding.verify()
            return record, _provenance(provenance, self.limits), _access('public-authored-csv-record', 'public_metadata_only', visibility_verified=True)
        if target["layer"] == METADATA_LAYER:
            return self._metadata_result(target)
        if target["layer"] == CLAIM_LAYER:
            # A prepared-bound owner may supply current or retained historical
            # exact refs through this dispatch.  The concrete catalog adapter
            # below exposes current Claims only; ClaimVersionReader history
            # still needs its own explicit epoch-bound adapter.
            return self._claim_result(target)
        if target["layer"] == SLOT_LAYER:
            return self._slot_result(target)
        # Review-ledger and native/text layers intentionally have no reader
        # here.  Adding one must register a new typed target and owner proof,
        # rather than falling through to a source-slot or metadata adapter.
        raise SourceReadError("source target layer is unsupported")

    def discover(self, request: dict[str, Any]) -> dict[str, Any]:
        """Issue one handle after an exact owner lookup; never enumerate IDs."""
        _request_size(request, self.limits)
        if type(request) is not dict or set(request) not in ({"target"}, {"selector"}):
            raise SourceReadError("source handle discovery requires one target or typed selector")
        self.binding.verify()
        self._verify_target_issuer()
        if "selector" in request:
            if self.target_issuer is None:
                raise SourceReadError("owner target resolver is not configured")
            selector = _selector(request["selector"])
            try:
                target = _target(self.target_issuer.issue(selector))
            except _OwnerUnavailable as error:
                return {**_base_discovery(self.epoch, None), "selector": selector,
                        "status": error.status, "reason": error.reason}
        else:
            target = _target(request["target"])
        result = _base_discovery(self.epoch, target)
        try:
            record, provenance, access = self._owner_target(target)
            handle = _make_handle(self.epoch, target, access, self.limits)
            if target['layer'] == CSV_LAYER:
                provenance = {key: value for key, value in provenance.items() if key != 'raw_record'}
            response = {
                **result,
                "status": "available",
                "reason": "owner-issued-exact-source-handle",
                "handle": handle,
                "provenance": provenance,
                "access": access,
            }
            if len(_canonical_bytes(response)) > self.limits.max_response_bytes:
                raise SourceReadBudgetExceeded("source handle discovery exceeds its byte budget")
            # Keep the owner result alive only for validation; no record is
            # returned by discovery, preventing accidental source disclosure.
            del record
            return response
        except _OwnerUnavailable as error:
            return {**result, "status": error.status, "reason": error.reason}
        except SourceReadBudgetExceeded:
            return {**result, "status": "over-budget", "reason": "source-read-budget"}
        except SourceReadError as error:
            reason = str(error)
            if reason.endswith("-unconfigured") or reason in {"metadata-type-not-owner-declared"}:
                return {**result, "status": "unsupported", "reason": reason[:256]}
            return {**result, "status": "corrupt", "reason": reason[:256]}
        except PermissionError:
            return {**result, "status": "access-restricted", "reason": "owner-source-path-restricted"}
        except FileNotFoundError:
            return {**result, "status": "missing", "reason": "owner-source-record-missing"}
        except OSError:
            return {**result, "status": "corrupt", "reason": "owner-source-io-failed"}

    def read(self, request: dict[str, Any]) -> dict[str, Any]:
        """Read one exact owner-selected JSON record, never a byte range."""
        _request_size(request, self.limits)
        if type(request) is not dict or set(request) != {"handle", "representation"}:
            raise SourceReadError("source read requires handle and representation only")
        if request.get("representation") not in ("record", "native_public_unit", "native_local_unit"):
            raise SourceReadError("only exact record or owner-gated native unit representations are supported")
        handle = validate_handle(request["handle"], limits=self.limits)
        self.binding.verify()
        self._verify_target_issuer()
        result = _base_result(self.epoch, {
            "schema_version": HANDLE_SCHEMA,
            "issuer": handle['issuer'],
            "epoch": _copy(handle["epoch"]),
            "target": handle["target"],
            "access": handle["access"],
            "handle_digest": handle["handle_digest"],
        })
        if request["representation"] in ("native_public_unit", "native_local_unit"):
            return {**result, "schema_version": "tos_source_native_unit_read_result_v1",
                    "native_unit": None, "text_access": None,
                    "status": "stale" if handle["epoch"] != self.epoch.value() else "unsupported",
                    "reason": "source-epoch-differs" if handle["epoch"] != self.epoch.value() else "native-unit-owner-unconfigured"}
        if handle["epoch"] != self.epoch.value():
            return {**result, "status": "stale", "reason": "source-epoch-differs"}
        try:
            record, provenance, access = self._owner_target(handle["target"])
            result = {
                **result,
                "status": "available",
                "reason": "exact-owner-source-record",
                "record": _copy(record),
                "provenance": provenance,
                "access": access,
            }
            if len(_canonical_bytes(result)) > self.limits.max_response_bytes:
                return {**_base_result(self.epoch, handle), "status": "over-budget", "reason": "source-response-byte-budget"}
            return result
        except _OwnerUnavailable as error:
            return {**result, "status": error.status, "reason": error.reason, "access": None}
        except SourceReadBudgetExceeded:
            return {**_base_result(self.epoch, handle), "status": "over-budget", "reason": "source-read-budget"}
        except SourceReadError as error:
            reason = str(error)
            if reason.endswith("-unconfigured") or reason == "metadata-type-not-owner-declared":
                return {**result, "status": "unsupported", "reason": reason[:256], "access": None}
            return {**result, "status": "corrupt", "reason": reason[:256], "access": None}
        except PermissionError:
            return {**result, "status": "access-restricted", "reason": "owner-source-path-restricted", "access": None}
        except FileNotFoundError:
            return {**result, "status": "missing", "reason": "owner-source-record-missing", "access": None}
        except OSError:
            return {**result, "status": "corrupt", "reason": "owner-source-io-failed", "access": None}


class _OwnerUnavailable(Exception):
    def __init__(self, status: str, reason: str):
        self.status, self.reason = status, reason


def unavailable_capabilities() -> dict[str, Any]:
    """Explicit capability state when no owner binding was selected."""
    return {
        "schema_version": CAPABILITIES_SCHEMA,
        "available": False,
        "issuer": ISSUER,
        "layers": {METADATA_LAYER: [], CLAIM_LAYER: False, SLOT_LAYER: False, CSV_LAYER: False},
        "limits": {
            "max_handle_bytes": MAX_HANDLE_BYTES,
            "max_request_bytes": MAX_REQUEST_BYTES,
            "max_record_bytes": MAX_RECORD_BYTES,
            "max_response_bytes": MAX_RESPONSE_BYTES,
        },
        "source_epoch": None,
        "authority": {
            "is_source": False,
            "writes_to_source": False,
            "grants_current_use": False,
            "native_text_payload": False,
            "note": "Select an explicit source-owner binding; no path or latest fallback is available.",
        },
        "status": "unsupported",
        "reason": "source-owner-reader-not-configured",
    }


def contract_summary() -> dict[str, Any]:
    """Small stable descriptor for callers that cannot read repository files."""
    return {
        "schema_version": "tos_source_read_contract_descriptor_v1",
        "handle_schema": HANDLE_SCHEMA,
        "discovery_schema": DISCOVERY_SCHEMA,
        "result_schema": RESULT_SCHEMA,
        "supported_layers": sorted(SUPPORTED_LAYERS),
        "representations": ["record", "native_public_unit", "native_local_unit"],
        "native_unit_result_schema": "tos_source_native_unit_read_result_v1",
        "native_unit_owner_required": True,
        "discovery_inputs": ["owner_target", "typed_catalog_selector"],
        "statuses": sorted(STATUSES),
        "authority": {
            "is_source": False,
            "writes_to_source": False,
            "grants_current_use": False,
            "native_text_payload": False,
            "path_or_range_requests": False,
            "latest_fallback": False,
            "target_origin": "owner-card-or-typed-catalog-resolver",
            "rights_scope": "metadata-disclosure-only",
            "native_unit_rights_scope": "separate-unconditional-public-owner-gate",
            "native_local_unit_rights_scope": "separate-current-owner-condition-selection-with-notices",
        },
    }
