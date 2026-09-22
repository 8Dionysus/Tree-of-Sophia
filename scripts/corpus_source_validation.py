"""Program-owned source admission adapter over the existing ToS validators.

The first corpus migration deliberately keeps a conservative full source audit
inside the data transaction. A changed-path list is not a substitute for source
closure. Software validation never calls this adapter. No semantic/review or
rights status is synthesized by its mechanical admission result.
"""
from __future__ import annotations

import ast
import hashlib
import importlib.metadata
import json
from pathlib import Path
import re
import shutil
import sys

from corpus_store import (
    CorpusCandidate,
    CorpusStoreError,
    ValidationIndex,
    canonical,
    digest_file,
    stage_timing,
)

SOFTWARE_ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = 'ToS/source-witnesses/'
CATALOG_PREFIX = SOURCE_ROOT + 'catalog/'
# These namespaces have an explicit owner-local lifecycle and are not admitted
# through this public/source-corpus route. Existing local custody is untouched.
FORBIDDEN_PARTS = {'.git', 'payload', 'owner-local'}


def is_source_member(relative: str) -> bool:
    """Keep authored route cards while excluding owned generated read models."""
    from corpus_store import relative_path
    relative_path(relative)
    if not relative.startswith('ToS/') or set(Path(relative).parts) & FORBIDDEN_PARTS:
        return False
    if relative.startswith(('ToS/derived-exports/', CATALOG_PREFIX)):
        return relative.endswith('.md')
    return True


def validator_identity(source_root: Path) -> str:
    """Bind actual validator code and actual selected schema/registry bytes.

    Bind only this adapter and its transitive local Python imports. Unrelated
    packaging, UI, KAG or developer tools cannot invalidate source admission.
    """
    bindings = {}
    def local_program(module):
        for parent in ('scripts', 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'):
            path = SOFTWARE_ROOT / parent / (module + '.py')
            if path.exists() or path.is_symlink():
                return path
        return None
    pending = ['corpus_source_validation', 'validate_source_witness_foundation',
               'build_source_witness_catalog']
    visited = set()
    while pending:
        module = pending.pop()
        if module in visited:
            continue
        visited.add(module)
        path = local_program(module)
        if path is None or path.is_symlink() or path.resolve() != path.absolute() or not path.is_file():
            raise CorpusStoreError('source validator program dependency is missing or linked')
        raw = path.read_bytes()
        bindings['software:' + path.relative_to(SOFTWARE_ROOT).as_posix()] = hashlib.sha256(raw).hexdigest()
        for node in ast.walk(ast.parse(raw)):
            names = ([alias.name for alias in node.names] if isinstance(node, ast.Import)
                     else [node.module] if isinstance(node, ast.ImportFrom) and node.module else [])
            for name in names:
                local = name.split('.')[0]
                if local_program(local) is not None:
                    pending.append(local)
    bindings['runtime:python'] = list(sys.version_info[:3])
    for package in ('jsonschema', 'referencing'):
        bindings['runtime:' + package] = importlib.metadata.version(package)
    for prefix, pattern in [('ToS/contracts', '*.json'), ('ToS/doctrine/semantic-interchange', '*.json')]:
        for path in sorted((source_root / prefix).rglob(pattern)):
            if path.is_symlink() or path.resolve() != path.absolute():
                raise CorpusStoreError('source grammar may not use symlinks')
            bindings['source:' + path.relative_to(source_root).as_posix()] = digest_file(path)
    if not any(key.startswith('source:ToS/contracts/') for key in bindings):
        raise CorpusStoreError('source snapshot has no schema contracts')
    return hashlib.sha256(canonical(bindings)).hexdigest()


def _strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for child in value.values():
            yield from _strings(child)
    elif isinstance(value, list):
        for child in value:
            yield from _strings(child)


def _structured_rows(path: Path):
    # The semantic validators own field interpretation. This conservative
    # dependency index only finds additional *existing* source dependencies;
    # arbitrary wording never becomes an accepted relation or an ID owner.
    if path.suffix == '.jsonl':
        with path.open('rb') as stream:
            for line in stream:
                if line.strip():
                    try:
                        yield json.loads(line)
                    except (ValueError, UnicodeError):
                        continue  # actual admissibility is checked by owner validators
    elif path.suffix == '.json' and path.stat().st_size <= 16 * 1024 * 1024:
        try:
            yield json.loads(path.read_bytes())
        except (ValueError, UnicodeError):
            return


def _catalog_rows(catalog_outputs: dict, relative: str):
    """Read rows from one freshly rendered, disposable catalog output."""
    expected = Path(relative)
    text = catalog_outputs.get(expected)
    if text is None:
        raise CorpusStoreError(f'fresh source catalog is missing {relative}')
    for line_number, line in enumerate(text.splitlines(), start=1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except (ValueError, UnicodeError) as error:
            raise CorpusStoreError(
                f'fresh source catalog has invalid JSON at {relative}:{line_number}'
            ) from error
        if not isinstance(row, dict):
            raise CorpusStoreError(f'fresh source catalog row is not an object: {relative}:{line_number}')
        yield row


def source_index(
    root: Path,
    paths: set[str],
    *,
    base: dict | None = None,
    catalog_outputs: dict | None = None,
) -> ValidationIndex:
    """Index IDs from owner catalogs, with incoming evidence/reference paths.

    The native adapter permits several versions of one native semantic ID.
    Keep its existing owner anchor while that exact path remains in the source
    snapshot; additional version paths are dependencies, not duplicate records.
    """
    from build_source_witness_catalog import collect_records, collect_claims
    from source_record_profiles import SourceRecordProfiles
    profiles = SourceRecordProfiles(root)
    identities = {}
    def bind(identity, path):
        if path not in paths:
            raise CorpusStoreError('source identity is outside the admitted member set')
        if identity in identities and identities[identity] != path:
            raise CorpusStoreError('duplicate source identity')
        identities[identity] = path
    if catalog_outputs is None:
        record_groups = collect_records(root, profiles=profiles).values()
        claim_rows = collect_claims(root)
    else:
        from build_source_witness_catalog import MANIFEST_PATH
        try:
            manifest_raw = catalog_outputs[MANIFEST_PATH]
            manifest = json.loads(manifest_raw)
            record_files = manifest['record_files'].values()
            record_groups = [
                _catalog_rows(catalog_outputs, relative)
                for relative in record_files
            ]
            claim_rows = _catalog_rows(catalog_outputs, manifest['claim_file'])
        except (KeyError, TypeError, ValueError, UnicodeError) as error:
            raise CorpusStoreError('fresh source catalog manifest is invalid') from error
    for entries in record_groups:
        for row in entries:
            bind(row['record_id'], row['source_record_ref'])
    for row in claim_rows:
        bind(row['claim_id'], row['source_claim_file_ref'])
    native = profiles.native_semantic_identities()
    for identity, refs in native.items():
        previous = (base or {}).get('identities', {}).get(identity)
        bind(identity, previous if previous in refs else sorted(refs)[0])
    for relative in sorted(paths):
        if relative.startswith(SOURCE_ROOT + 'retirements/') and relative.endswith('.json'):
            from corpus_source_retirement import MAX_EVENT_BYTES, SCHEMA_REF, _json
            from jsonschema import Draft202012Validator, FormatChecker
            with (root / relative).open('rb') as stream:
                raw = stream.read(MAX_EVENT_BYTES + 1)
            if len(raw) > MAX_EVENT_BYTES:
                raise CorpusStoreError('source retirement record exceeds its bounded size')
            row = _json(raw)
            schema = _json((root / SCHEMA_REF).read_bytes())
            if (not Draft202012Validator(schema, format_checker=FormatChecker()).is_valid(row)
                    or row.get('schema_version') != 'tos_provenance_event_v1'
                    or row.get('event_type') != 'migration'
                    or row.get('method', {}).get('name') != 'corpus-source-retirement'
                    or not row.get('event_id', '').startswith('tos.event.')):
                raise CorpusStoreError('source retirement record has an invalid operation or ID')
            bind(row['event_id'], relative)
    dependencies = {path: set() for path in paths}
    # Grammar changes require the full audit through validator identity. Avoid
    # duplicating every schema into every row of a mass dependency manifest.
    for relative in sorted(paths):
        if relative.startswith(('ToS/contracts/', 'ToS/doctrine/semantic-interchange/')):
            continue
        for row in _structured_rows(root / relative):
            for value in _strings(row):
                target = identities.get(value)
                if target and target != relative:
                    dependencies[relative].add(target)
                if value.startswith('ToS/'):
                    candidate = value.split('#', 1)[0]
                    candidate = re.sub(r':\d+$', '', candidate)
                    if candidate in paths and candidate != relative:
                        dependencies[relative].add(candidate)
    for identity, refs in native.items():
        anchor = identities[identity]
        dependencies[anchor].update(ref for ref in refs if ref != anchor)
    return ValidationIndex(identities, {path: sorted(targets) for path, targets in dependencies.items() if targets})


class SourceValidator:
    def __init__(self, source_grammar_root: Path, *, payload_source_root: Path | None = None,
                 historical_capture: Path | list[Path] | None = None,
                 historical_root: Path | list[Path] | None = None):
        self.grammar_root = Path(source_grammar_root).absolute()
        self.grammar_sha256 = validator_identity(self.grammar_root)
        self.payload_source_root = payload_source_root
        self.evidence = []
        self._evidence_roots = {}
        if (historical_capture is None) != (historical_root is None):
            raise CorpusStoreError('historical evidence needs both an exact capture and restored root')
        if historical_capture is not None:
            from corpus_archive import verify_capture
            captures = historical_capture if isinstance(historical_capture, list) else [historical_capture]
            roots = historical_root if isinstance(historical_root, list) else [historical_root]
            if not captures or len(captures) != len(roots):
                raise CorpusStoreError('historical evidence needs both capture and restored root for every pack')
            for capture, restored in zip(captures, roots):
                verify_capture(capture)
                restored = Path(restored).absolute()
                if restored != restored.resolve() or not restored.is_dir():
                    raise CorpusStoreError('historical evidence root must be a regular directory')
                # Exact old projections, image bytes and surrounding authored
                # decisions can be provenance dependencies. They are data for
                # validation, never executable software or a public snapshot.
                with (capture / 'members.jsonl').open('rb') as stream:
                    for line in stream:
                        row = json.loads(line)
                        relative = row['path']
                        if ('owner-local' not in Path(relative).parts
                                and not is_source_member(relative)
                                and (not relative.startswith('ToS/')
                                     or relative.startswith('ToS/derived-exports/')
                                     or 'payload' in Path(relative).parts)):
                            if relative in self._evidence_roots:
                                raise CorpusStoreError('historical evidence packs overlap')
                            self.evidence.append(row)
                            self._evidence_roots[relative] = restored
            self.evidence.sort(key=lambda row: row['path'])
        self.sha256 = (hashlib.sha256(canonical({'grammar_validator': self.grammar_sha256,
                        'historical_evidence': self.evidence})).hexdigest()
                       if self.evidence else self.grammar_sha256)

    def _supply_evidence(self, root: Path):
        for entry in self.evidence:
            source = self._evidence_roots[entry['path']] / entry['path']
            target = root / entry['path']
            if (target.exists() or target.is_symlink() or source.is_symlink()
                    or source.resolve() != source.absolute()
                    or not source.is_file() or source.stat().st_size != entry['size_bytes']
                    or digest_file(source) != entry['sha256']):
                raise CorpusStoreError('historical validation evidence is missing, changed or overlaps source')
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            if digest_file(target) != entry['sha256']:
                raise CorpusStoreError('historical evidence changed during materialization')

    def __call__(self, candidate: CorpusCandidate, base: dict | None, affected: frozenset[str]) -> ValidationIndex:
        from corpus_source_retirement import membership_transition, validate_retirements
        if any(not is_source_member(path) for path in candidate.paths):
            raise CorpusStoreError('source member is outside the corpus admission boundary')
        retirement_ids = validate_retirements(candidate, base)
        if base is not None and base['validator_sha256'] == self.sha256:
            transition = membership_transition(candidate, base, retirement_ids)
            if transition is not None:
                with stage_timing(
                    'source_materialize',
                    members=sum(
                        path.startswith(('ToS/contracts/', 'ToS/doctrine/semantic-interchange/'))
                        for path in candidate.paths
                    ),
                    mode='retirement_fastpath',
                ):
                    grammar = candidate.materialize(path for path in candidate.paths if path.startswith(
                        ('ToS/contracts/', 'ToS/doctrine/semantic-interchange/')))
                if (validator_identity(grammar) != self.grammar_sha256
                        or validator_identity(self.grammar_root) != self.grammar_sha256):
                    raise CorpusStoreError('source grammar or validator changed during admission')
                return transition
        # General record/claim transitions still need the scoped owner-rule
        # adapter. They cannot borrow a retirement-only result or skip checks.
        del affected
        grammar_paths = tuple(sorted(
            path for path in candidate.paths
            if path.endswith('.json')
            and path.startswith(('ToS/contracts/', 'ToS/doctrine/semantic-interchange/'))
        ))
        # Check the small source grammar overlay before copying the full
        # candidate. This rejects a base/batch grammar drift without spending
        # the admission cost of materializing unrelated source members. The
        # complete candidate identity checks below remain authoritative.
        with stage_timing('source_grammar_preflight', members=len(grammar_paths)):
            grammar = candidate.materialize(grammar_paths)
            if (validator_identity(grammar) != self.grammar_sha256
                    or validator_identity(self.grammar_root) != self.grammar_sha256):
                raise CorpusStoreError(
                    'source grammar or validator identity changed before full materialization'
                )
        with stage_timing(
            'source_materialize',
            members=len(candidate.paths),
            mode='full_audit',
        ):
            root = candidate.materialize(candidate.paths)
        from build_source_witness_catalog import render_outputs, write_outputs
        from validate_source_witness_foundation import validate_foundation, source_snapshot_membership
        before_identity = validator_identity(root)
        if before_identity != self.grammar_sha256:
            raise CorpusStoreError('source schema or validator identity changed before admission')
        paths = {path.relative_to(root).as_posix() for path in root.rglob('*') if path.is_file()}
        for relative in paths:
            path = root / relative
            if (not is_source_member(relative)
                    or path.is_symlink() or path.resolve() != path.absolute()):
                raise CorpusStoreError('source member is outside the corpus admission boundary')
        # Catalogs are disposable validator inputs in this isolated view. They
        # are not copied back into source objects or the software checkout.
        self._supply_evidence(root)
        with stage_timing('catalog_render', members=len(paths)):
            catalog_outputs = render_outputs(root)
        with stage_timing('catalog_write', outputs=len(catalog_outputs)):
            write_outputs(root, catalog_outputs)
        members = frozenset(path.relative_to(root).as_posix() for path in root.rglob('*') if path.is_file())
        with stage_timing('foundation_validate', members=len(members)):
            with source_snapshot_membership(root, members):
                issues = validate_foundation(root, payload_source_root=self.payload_source_root)
        if issues:
            first = '; '.join(f'{path}: {message}' for path, message in issues[:8])
            raise CorpusStoreError(f'source admission rejected ({len(issues)} issues): {first}')
        # The output was rendered from this immutable candidate immediately
        # above and foundation validation checked its parity. Reuse those
        # in-memory rows for the transport index; never read an accepted or
        # stale generated catalog as authority.
        with stage_timing('source_index', members=len(paths)):
            index = source_index(root, paths, base=base, catalog_outputs=catalog_outputs)
        for identity, path in retirement_ids.items():
            if identity in index.identities and index.identities[identity] != path:
                raise CorpusStoreError('retirement event ID has another source owner')
            index.identities[identity] = path
        if validator_identity(root) != before_identity or validator_identity(self.grammar_root) != self.grammar_sha256:
            raise CorpusStoreError('source grammar or validator changed during admission')
        return index
