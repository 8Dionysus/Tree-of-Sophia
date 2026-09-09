"""Bounded private construction from one already acquired exact EPUB member.

The protected grant selects reading, derivation, source, policy and private
destination separately. There is no download, implicit context fallback,
metadata admission, text assessment, normalization or first segmentation here.
"""
from __future__ import annotations

import base64
from datetime import datetime, timezone
import hashlib
import io
import os
from pathlib import Path
import re
import stat
import struct
import sys
import time
import zipfile
import zlib

import source_commands as source
import source_command_contracts as contract
import source_item_deposit as deposit
import source_text_unit_commands as units
from source_owner_context import OwnerLocalSourceContext, _absolute, _open, _read as private_read
from native_text_binding import NativeTextBindingResolver
from source_revisions import _file_refs


CONFIG = 'tos_local_text_layer_create_owner_v1'
OPERATION = 'text-layer.create'
LAYER_SCHEMA = 'ToS/contracts/source-text-layer.schema.json'
ANCHOR_SCHEMA = 'ToS/contracts/source-anchor-v2.schema.json'
INPUT_FILE = 'source-create-inputs.json'
CONFIG_FILE = units.CONFIG_FILE
RECEIPT_FILE = units.RECEIPT_FILE
MAX_PACKAGE_BYTES = 12 * 1024 * 1024
MAX_CONTROL_BYTES = 18 * 1024 * 1024
MAX_FILES = 12
MAX_SECONDS = 60
IMPLEMENTATIONS = (*units.IMPLEMENTATIONS,
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_text_layer_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_item_deposit.py',
    'scripts/source_metadata_snapshot.py', 'scripts/source_text_layer_proposal.py')


def _encoded(value):
    return source._canonical(value) + b'\n'


def _grant(value, fields):
    source._keys(value, fields | {'authority_ref', 'expires_at'})
    if (not isinstance(value['authority_ref'], str) or not value['authority_ref'].strip()
            or source._instant(value['expires_at']) <= datetime.now(timezone.utc)):
        raise PermissionError('separate native source grant is absent or expired')


def configuration(config, *, owner_config):
    source._keys(config, {'schema_version', 'uid', 'principal_id', 'authority_ref', 'expires_at',
        'source_context_ref', 'source_path', 'allowed_operations', 'source_scope', 'source_record_refs',
        'source_record_sha256', 'manifest_sha256', 'source_access', 'derivation_access',
        'member', 'selector', 'policy', 'identities', 'maker', 'language', 'limits'})
    raw = private_read(Path(owner_config), source.MAX_COMMAND_BYTES, confidential_file=True)
    if source._json_object(raw) != config:
        raise source.JournalConflict('native layer grant changed while selecting it')
    if (config['schema_version'] != CONFIG or type(config['uid']) is not int or config['uid'] != os.getuid()
            or config['allowed_operations'] != [OPERATION]
            or source._instant(config['expires_at']) <= datetime.now(timezone.utc)
            or any(not isinstance(config[k], str) or not config[k].strip() for k in ('principal_id', 'authority_ref'))):
        raise PermissionError('native layer creation is not currently delegated')
    access, derivation = config['source_access'], config['derivation_access']
    _grant(access, {'read_scope', 'access_allowed', 'payload_root', 'byte_size'})
    _grant(derivation, {'derivation_allowed', 'operation', 'rights_record_refs', 'content_visibility'})
    if (access['read_scope'] != 'exact_acquired_file' or access['access_allowed'] is not True
            or derivation['derivation_allowed'] is not True or derivation['operation'] != 'structural_extraction'
            or derivation['content_visibility'] != 'local_only'
            or type(access['byte_size']) is not int or not 1 <= access['byte_size'] <= deposit.MAX_BYTES):
        raise PermissionError('native extraction needs distinct exact reading and local derivation grants')
    payload_root = _absolute(access['payload_root'])
    try:
        os.close(_open(payload_root, directory=True))
    except OSError:
        raise ValueError('native extraction payload owner is absent or unsafe') from None
    context = OwnerLocalSourceContext.load(config['source_context_ref'])
    path = context.path(config['source_path'])
    if (context.role(config['source_path']) != 'owner-local-root' or path.name != 'source-text-layer.v1.json'
            or len(Path(config['source_path']).parts) < 7
            or any(part in {'payload', 'local-content', 'catalog'} or part.startswith('.') for part in Path(config['source_path']).parts)
            or payload_root.is_relative_to(context.private_root) or context.private_root.is_relative_to(payload_root)):
        raise PermissionError('native layer needs disjoint exact payload and private output roots')
    units._private_directory(context, path.parent.parent)
    source._keys(config['source_scope'], {'work_ref', 'expression_ref', 'edition_ref', 'item_ref', 'file_ref', 'file_sha256'})
    source._keys(config['source_record_refs'], {'work', 'expression', 'edition', 'item'})
    source._keys(config['source_record_sha256'], {'work', 'expression', 'edition', 'item'})
    for kind in config['source_record_refs']:
        ref = config['source_record_refs'][kind]
        if context.role(ref) != 'source-contract-root' or Path(ref).name != kind + '.json':
            raise PermissionError('acquired source identity needs its exact authored metadata locator')
        if not re.fullmatch(r'tos\.' + kind + r'\.[a-z0-9]+(?:[.-][a-z0-9]+)*', config['source_scope'][kind + '_ref']):
            raise ValueError('native source identity kind differs')
    for value in [config['manifest_sha256'], config['source_scope']['file_sha256'], *config['source_record_sha256'].values()]:
        if not isinstance(value, str) or not re.fullmatch('[a-f0-9]{64}', value):
            raise ValueError('native construction requires exact raw input digests')
    if config['source_scope']['file_ref'] != 'tos.file.sha256.' + config['source_scope']['file_sha256']:
        raise ValueError('native extraction File identity must equal its raw digest')
    source._keys(config['member'], {'member_path', 'member_sha256'})
    member = config['member']
    if (not isinstance(member['member_path'], str) or not _member_path(member['member_path'])
            or not isinstance(member['member_sha256'], str) or not re.fullmatch('[a-f0-9]{64}', member['member_sha256'])):
        raise ValueError('native extraction needs one exact bounded container member')
    source._keys(config['identities'], {'layer_id', 'anchor_id', 'passage_id', 'provenance_event_id'})
    kinds = {'layer_id': 'text-layer', 'anchor_id': 'anchor', 'passage_id': 'passage', 'provenance_event_id': 'event'}
    for key, kind in kinds.items():
        if not isinstance(config['identities'][key], str) or not re.fullmatch(r'tos\.' + kind + r'\.sid-[a-f0-9]{32}', config['identities'][key]):
            raise ValueError('native extraction requires distinct owner-delegated opaque identities')
    source._keys(config['maker'], {'maker_type', 'agent_ref', 'method', 'version'})
    if (config['maker']['maker_type'] != 'software' or config['maker']['agent_ref'] != config['principal_id']
            or any(not isinstance(config['maker'][k], str) or not config['maker'][k].strip() for k in ('method', 'version'))
            or not isinstance(config['language'], str) or not re.fullmatch('[a-z]{2,3}(?:-[A-Za-z0-9]+)*', config['language'])):
        raise ValueError('native extraction must name its software maker, method and language')
    source._keys(config['limits'], {'max_output_bytes', 'max_seconds'})
    if (type(config['limits']['max_output_bytes']) is not int or not 1 <= config['limits']['max_output_bytes'] <= 8 * 1024 * 1024
            or type(config['limits']['max_seconds']) is not int or not 1 <= config['limits']['max_seconds'] <= MAX_SECONDS):
        raise ValueError('native extraction limits exceed the bounded profile')
    from source_text_layer_proposal import validate_extraction_profile
    validate_extraction_profile(config['selector'], config['policy'])
    resolver = NativeTextBindingResolver(context.public_root, owner_context=context, read_bytes=source._read)
    for name in (LAYER_SCHEMA, ANCHOR_SCHEMA, units.PROVENANCE_SCHEMA):
        resolver._read(name, schema=True)
    digest = source._digest(source._canonical({'owner_configuration_bytes': source._digest(raw),
        'context': context.snapshot(), 'contracts': resolver.schema_digests,
        'payload_root_pins': deposit._pins(payload_root / 'root-pin')}))
    return config, digest, path


def _member_path(value):
    return (bool(value) and len(value.encode('utf-8')) <= 1024 and '\x00' not in value and '\\' not in value
            and not value.startswith('/') and all(part not in {'', '.', '..'} for part in value.rstrip('/').split('/')))


def _metadata(config, resolver):
    scope = config['source_scope']
    for kind, ref in config['source_record_refs'].items():
        row = resolver._record(ref, expected=config['source_record_sha256'][kind])
        resolver._validate(row, 'corpus-record.schema.json')
        if row.get('record_type') != kind or row.get('record_id') != scope[kind + '_ref']:
            raise ValueError('native extraction metadata identity differs from its exact grant')
    layer_source = {key: scope[key] for key in ('work_ref', 'expression_ref', 'edition_ref', 'item_ref')}
    layer_source.update(source_file_ref=scope['file_ref'], source_file_sha256=scope['file_sha256'])
    manifest = resolver._source_scope(config, scope, {'source_binding': layer_source})
    item = resolver._record(config['source_record_refs']['item'])
    resolver._read(item['item_manifest_ref'], expected=config['manifest_sha256'])
    entry = next(row for row in manifest['payload_files'] if row['file_id'] == scope['file_ref'])
    if (manifest['visibility'] != 'local_only' or entry['media_type'] != 'application/epub+zip'
            or entry['byte_size'] != config['source_access']['byte_size']
            or len(Path(entry['relative_path']).parts) != 2 or Path(entry['relative_path']).parts[0] != 'payload'
            or not _member_path(entry['relative_path'])):
        raise PermissionError('native extraction requires an exact already acquired local-only EPUB')
    refs = config['derivation_access']['rights_record_refs']
    if not isinstance(refs, list) or not 1 <= len(refs) <= 16 or len({row.get('ref') for row in refs if isinstance(row, dict)}) != len(refs):
        raise ValueError('native derivation rights bindings must be finite and unique')
    if manifest['rights_ref'] not in {row.get('ref') for row in refs}:
        raise PermissionError('native derivation cannot omit its Item rights')
    exact_decisions = []
    for binding in refs:
        source._keys(binding, {'ref', 'sha256'})
        rights = resolver._record(binding['ref'], expected=binding['sha256'])
        resolver._validate(rights, 'rights-record.schema.json')
        if (rights['review_status'] in {'superseded', 'legal_review_requested'}
                or rights['assessment_status'] in {'permission_denied', 'conflicting_evidence'}):
            raise PermissionError('native derivation rights are inactive or denied')
        if binding['ref'] == manifest['rights_ref']:
            if (not {scope['item_ref'], scope['file_ref']}.issubset(rights['scope_refs'])
                    or rights['derivative_posture'] not in {'local_research_only', 'allowed'}):
                raise PermissionError('native extraction lacks exact Item/File local-derivation rights')
        layer_id = config['identities']['layer_id']
        selected = [row for row in rights.get('layer_assessments', []) if layer_id in row['scope_refs']]
        if selected:
            exact_decisions.extend(selected)
        elif layer_id in rights['scope_refs']:
            exact_decisions.append(rights)
        if not {layer_id, scope['item_ref'], scope['file_ref']}.intersection(rights['scope_refs']):
            raise PermissionError('native extraction rights address another source or layer')
    if not exact_decisions or any(row['derivative_posture'] not in {'local_research_only', 'allowed'}
            or row['assessment_status'] in {'permission_denied', 'conflicting_evidence'}
            or row['review_status'] in {'superseded', 'legal_review_requested'} for row in exact_decisions):
        raise PermissionError('new content-bearing layer requires its own exact local-derivation rights basis')
    return entry


def _deadline(deadline):
    if time.monotonic() > deadline:
        raise ValueError('native extraction exceeded its bounded execution interval')


def _zip_directory(stream, size):
    """Bound actual directory count before constructing ZipInfo objects."""
    stream.seek(max(0, size - 65557))
    tail = stream.read(65557)
    index = tail.rfind(b'PK\x05\x06')
    if index < 0 or index + 22 > len(tail):
        raise ValueError('native extraction needs an ordinary bounded ZIP directory')
    position = max(0, size - 65557) + index
    _, disk, directory_disk, disk_count, count, length, offset, comment = struct.unpack('<4s4H2IH', tail[index:index + 22])
    if (disk or directory_disk or disk_count != count or not 1 <= count <= deposit.MAX_ZIP_MEMBERS
            or length > deposit.MAX_ZIP_DIRECTORY_BYTES or offset + length != position
            or index + 22 + comment != len(tail)):
        raise ValueError('native ZIP directory exceeds its declared resource profile')
    if position >= 20:
        stream.seek(position - 20)
        if stream.read(4) == b'PK\x06\x07':
            raise ValueError('ZIP64 is outside the bounded native extraction profile')
    stream.seek(offset)
    directory = stream.read(length)
    cursor = actual = 0
    while cursor < len(directory):
        if cursor + 46 > len(directory) or directory[cursor:cursor + 4] != b'PK\x01\x02':
            raise ValueError('native ZIP central record is malformed')
        name_size, extra_size, comment_size = struct.unpack_from('<3H', directory, cursor + 28)
        end = cursor + 46 + name_size + extra_size + comment_size
        actual += 1
        if actual > deposit.MAX_ZIP_MEMBERS or end > len(directory) or name_size > 1024:
            raise ValueError('native ZIP actual directory count or name allocation exceeds its budget')
        if b'\x00' in directory[cursor + 46:cursor + 46 + name_size]:
            raise ValueError('native ZIP member has an ambiguous name')
        extra = cursor + 46 + name_size
        while extra < cursor + 46 + name_size + extra_size:
            if extra + 4 > cursor + 46 + name_size + extra_size:
                raise ValueError('native ZIP extra record is malformed')
            kind, width = struct.unpack_from('<HH', directory, extra)
            extra += 4 + width
            if kind == 1 or extra > cursor + 46 + name_size + extra_size:
                raise ValueError('native ZIP64 or malformed extra record is unsupported')
        cursor = end
    if actual != count:
        raise ValueError('native ZIP actual and declared directory counts differ')
    return offset


def _read_member(stream, size, selected, deadline):
    directory_offset = _zip_directory(stream, size)
    stream.seek(0)
    with zipfile.ZipFile(stream) as archive:
        entries = archive.infolist()
        if (len(entries) > deposit.MAX_ZIP_MEMBERS or len({row.filename for row in entries}) != len(entries)
                or sum(row.file_size for row in entries) > deposit.MAX_EXPANDED_BYTES
                or any(not _member_path(row.filename) or row.file_size > deposit.MAX_MEMBER_BYTES
                    or row.compress_size > size or row.flag_bits & ~0x808
                    or row.compress_type not in {zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED}
                    or stat.S_ISLNK(row.external_attr >> 16) for row in entries)):
            raise ValueError('native ZIP member identity, compression or expansion limits differ')
        matches = [row for row in entries if row.filename == selected['member_path']]
        if len(matches) != 1:
            raise ValueError('native extraction selected no unique container member')
        entry = matches[0]
        stream.seek(entry.header_offset)
        header = stream.read(30)
        if len(header) != 30 or header[:4] != b'PK\x03\x04':
            raise ValueError('native ZIP local member header is malformed')
        flags, method = struct.unpack_from('<HH', header, 6)
        name_size, extra_size = struct.unpack_from('<HH', header, 26)
        if name_size > 1024 or extra_size > 65535 or flags != entry.flag_bits or method != entry.compress_type:
            raise ValueError('native ZIP local and central member declarations differ')
        name = stream.read(name_size)
        if name.decode('utf-8' if flags & 0x800 else 'cp437') != entry.filename:
            raise ValueError('native ZIP local and central member names differ')
        stream.seek(extra_size, io.SEEK_CUR)
        content_start = stream.tell()
        next_offsets = [row.header_offset for row in entries if row.header_offset > entry.header_offset]
        if content_start + entry.compress_size > min([directory_offset, *next_offsets]):
            raise ValueError('native ZIP selected compressed data overlaps another member or directory')
        remaining, result = entry.compress_size, bytearray()
        decoder = zlib.decompressobj(-15) if method == zipfile.ZIP_DEFLATED else None
        while remaining:
            _deadline(deadline)
            chunk = stream.read(min(65536, remaining))
            if not chunk:
                raise ValueError('native ZIP member ended before its bound compressed size')
            remaining -= len(chunk)
            decoded = decoder.decompress(chunk, deposit.MAX_MEMBER_BYTES - len(result) + 1) if decoder else chunk
            result.extend(decoded)
            if len(result) > deposit.MAX_MEMBER_BYTES or decoder and (decoder.unconsumed_tail or decoder.unused_data):
                raise ValueError('native ZIP actual member expansion exceeds its bound or has trailing data')
        if decoder and not decoder.eof or len(result) != entry.file_size or zlib.crc32(result) != entry.CRC:
            raise ValueError('native ZIP member decompression, exact size or checksum differs')
        raw = bytes(result)
        if hashlib.sha256(raw).hexdigest() != selected['member_sha256']:
            raise source.JournalConflict('native extraction member differs from its exact raw binding')
        return raw


def _payload(config, entry, *, deadline):
    """One explicit third-root input; not an OwnerLocalSourceContext fallback."""
    _deadline(deadline)
    item = Path(config['source_record_refs']['item']).parent.relative_to('ToS/source-witnesses')
    path = Path(config['source_access']['payload_root']) / item / entry['relative_path']
    descriptor = None
    try:
        pins = deposit._pins(path)
        descriptor, before = deposit._file(path)
        if before.st_size != config['source_access']['byte_size']:
            raise source.JournalConflict('native extraction input size differs from the granted File')
        with os.fdopen(os.dup(descriptor), 'rb') as stream:
            digest, count = hashlib.sha256(), 0
            while True:
                _deadline(deadline)
                chunk = stream.read(1024 * 1024)
                if not chunk:
                    break
                count += len(chunk)
                if count > before.st_size:
                    raise source.JournalConflict('native extraction input grew during exact reading')
                digest.update(chunk)
            if count != before.st_size or digest.hexdigest() != config['source_scope']['file_sha256']:
                raise source.JournalConflict('native extraction File fixity differs from its grant')
            raw = _read_member(stream, before.st_size, config['member'], deadline)
        after = os.fstat(descriptor)
        current, current_info = deposit._file(path)
        os.close(current)
        if (deposit._identity(after) != deposit._identity(before) or deposit._identity(current_info) != deposit._identity(before)
                or deposit._pins(path) != pins):
            raise source.JournalConflict('native extraction input or an ancestor changed during reading')
        return raw, {'identity': list(deposit._identity(before)), 'parents': pins}
    except (OSError, UnicodeError, zipfile.BadZipFile, zlib.error, struct.error):
        raise ValueError('native extraction input is unsafe, unsupported or changed') from None
    finally:
        if descriptor is not None:
            os.close(descriptor)


def _runtime():
    path = Path(sys.executable).resolve()
    raw = source._read(path, 64 * 1024 * 1024)
    return source._digest(raw)


def _prepare(config, *, deadline, exclude=None):
    from source_text_layer_proposal import extract_xhtml_text, build_text_layer_proposal
    context = OwnerLocalSourceContext.load(config['source_context_ref'])
    resolver = NativeTextBindingResolver(context.public_root, owner_context=context, read_bytes=source._read)
    entry = _metadata(config, resolver)  # Both rights scopes, BEFORE payload I/O.
    inventory = units._identity_snapshot(context, config, exclude=exclude,
                                         identities=list(config['identities'].values()))
    member, payload_identity = _payload(config, entry, deadline=deadline)
    text = extract_xhtml_text(member, selector=config['selector'], policy=config['policy'])
    _deadline(deadline)
    if len(text.encode('utf-8')) > config['limits']['max_output_bytes']:
        raise ValueError('native layer output exceeds the independently delegated byte limit')
    base = Path(config['source_path']).parent
    payload_ref = (Path(config['source_record_refs']['item']).parent / entry['relative_path']).as_posix()
    refs = {'layer_ref': config['source_path'], 'anchor_ref': (base / 'source-anchor.v2.json').as_posix(),
        'content_ref': (base / 'content.txt').as_posix(), 'policy_ref': (base / 'extraction-policy.json').as_posix(),
        'configuration_ref': (base / CONFIG_FILE).as_posix(),
        'configuration_sha256': source._digest(_encoded(config))[7:], 'source_payload_ref': payload_ref}
    output = build_text_layer_proposal(exact_text=text, source_scope=config['source_scope'],
        identities=config['identities'], refs=refs, member=config['member'], selector=config['selector'],
        policy=config['policy'], maker=config['maker'], language=config['language'],
        rights_record_refs=config['derivation_access']['rights_record_refs'])
    resolver._validate(output['layer'], 'source-text-layer.schema.json')
    resolver._validate(output['anchor'], 'source-anchor-v2.schema.json')
    from validate_source_witness_foundation import _source_text_layer_semantic_issues, _anchor_v2_semantic_issues
    if (_source_text_layer_semantic_issues(output['layer']) or _anchor_v2_semantic_issues(output['anchor'])
            or output['content'] != text.encode('utf-8') or output['policy'] != config['policy']):
        raise ValueError('native construction differs from its exact source-layer contract')
    implementations = {ref: source._digest(source._read(source.ROOT / ref, source.MAX_SET_BYTES)) for ref in IMPLEMENTATIONS}
    inputs = {'schema_version': 'tos_native_construction_inputs_v1', 'context': context.snapshot(),
        'inputs': [[ref, category, digest] for (ref, category), digest in sorted(resolver._inputs.items())],
        'payload': payload_identity, 'implementation': implementations, 'runtime': _runtime()}
    files = {'source-text-layer.v1.json': _encoded(output['layer']),
        'source-anchor.v2.json': _encoded(output['anchor']), 'extraction-policy.json': _encoded(output['policy']),
        'content.txt': output['content'], CONFIG_FILE: _encoded(config), INPUT_FILE: _encoded(inputs)}
    if sum(map(len, files.values())) > MAX_PACKAGE_BYTES:
        raise ValueError('native layer package exceeds its bounded byte budget')
    dependencies = source._digest(source._canonical({'source_snapshot': resolver.snapshot(),
        'inputs': source._digest(files[INPUT_FILE]), 'identity_inventory': inventory}))
    entities = [{'entity_ref': payload_ref, 'role': 'exact-acquired-file',
        'sha256': config['source_scope']['file_sha256'], 'size_bytes': entry['byte_size'],
        'media_type': 'application/epub+zip', 'availability': 'owner_local', 'content_disclosure': 'private_content',
        'fixity_verified': True, 'fixity_verified_at': datetime.now(timezone.utc).isoformat()},
        {'entity_ref': payload_ref + '!/' + config['member']['member_path'], 'role': 'exact-container-member',
         'sha256': config['member']['member_sha256'], 'size_bytes': len(member), 'media_type': 'application/xhtml+xml',
         'availability': 'owner_local', 'content_disclosure': 'private_content', 'fixity_verified': True,
         'fixity_verified_at': datetime.now(timezone.utc).isoformat()}]
    return files, dependencies, {'rights': config['derivation_access']['rights_record_refs'],
        'entities': entities, 'event_type': 'native_extraction'}


def _private_package(context, directory):
    units._private_directory(context, directory)
    pins, before = deposit._pins(directory / 'package-pin'), directory.stat()
    files, total = {}, 0
    with os.scandir(directory) as entries:
        for entry in entries:
            if len(files) >= MAX_FILES or not stat.S_ISREG(entry.stat(follow_symlinks=False).st_mode):
                raise source.JournalCorruption('private construction package has foreign or excess files')
            limit = 8 * 1024 * 1024 if entry.name == 'content.txt' else 2 * source.MAX_COMMAND_BYTES
            raw = private_read(Path(entry.path), min(limit, MAX_PACKAGE_BYTES - total),
                               private_root=context.private_root)
            total += len(raw)
            files[entry.name] = raw
    after = directory.stat()
    if ((before.st_dev, before.st_ino, before.st_mtime_ns, before.st_ctime_ns)
            != (after.st_dev, after.st_ino, after.st_mtime_ns, after.st_ctime_ns)
            or deposit._pins(directory / 'package-pin') != pins):
        raise source.JournalConflict('private construction package or its ancestors changed')
    return files


def _verify_receipt(files, *, config, configuration_digest, request, source_id, source_version=1):
    if RECEIPT_FILE not in files:
        raise source.JournalCorruption('private construction lacks its retained receipt')
    receipt = source._json_object(files[RECEIPT_FILE])
    source._keys(receipt, {'schema_version', 'command_id', 'request_digest', 'principal_id', 'authority_ref',
        'owner_configuration', 'recorded_at', 'source_path', 'source', 'dependencies', 'files', 'grants_admission'})
    body = source._json_object(files[Path(config['source_path']).name])
    subject = source.Record.from_payload(source_id, source_version, body)
    source._instant(receipt['recorded_at'])
    if (receipt['schema_version'] != 'tos_local_source_create_receipt_v1'
            or receipt['command_id'] != request['command_id'] or receipt['request_digest'] != source._digest(source._canonical(request))
            or receipt['owner_configuration'] != configuration_digest or request['expected_configuration'] != configuration_digest
            or receipt['source_path'] != config['source_path'] or receipt['source'] != subject.ref
            or request['expected_source'] is not None or request['expected_revision'] is not None
            or receipt['principal_id'] != config['principal_id'] or receipt['authority_ref'] != config['authority_ref']
            or receipt['dependencies'] != request['expected_dependencies'] or receipt['grants_admission'] is not False
            or receipt['files'] != _file_refs({k: v for k, v in files.items() if k != RECEIPT_FILE})
            or source._json_object(files[CONFIG_FILE]) != config
            or source._json_object(files['source-create-request.json']) != request):
        raise source.JournalConflict('private retained construction differs from its exact request or delegation')
    return receipt


def _control_path(context, target, request):
    key = source._digest(source._canonical({'target': str(target), 'command_id': request['command_id']}))[7:]
    return context.private_root / ('.native-construction-' + key + '.pending')


def _bounded_names(directory, limit):
    names = set()
    with os.scandir(directory) as entries:
        for entry in entries:
            if len(names) >= limit:
                raise source.JournalCorruption('private construction directory exceeds its entry budget')
            names.add(entry.name)
    return names


def _control_files(context, control):
    units._private_directory(context, control)
    if _bounded_names(control, 2) - {'plan.json', 'output'}:
        raise source.JournalCorruption('private construction control is incomplete or has foreign residue')
    raw = private_read(control / 'plan.json', MAX_CONTROL_BYTES, private_root=context.private_root)
    plan = source._json_object(raw)
    source._keys(plan, {'schema_version', 'target_ref', 'request_digest', 'files'})
    if plan['schema_version'] != 'tos_native_construction_stage_v1' or not isinstance(plan['files'], dict) or not 1 <= len(plan['files']) <= MAX_FILES:
        raise source.JournalCorruption('private construction control shape differs')
    files, total = {}, 0
    try:
        for name, value in plan['files'].items():
            if not isinstance(name, str) or Path(name).name != name or name.startswith('.') or not isinstance(value, str):
                raise ValueError('invalid file slot')
            body = base64.b64decode(value, validate=True)
            total += len(body)
            if total > MAX_PACKAGE_BYTES:
                raise ValueError('over-budget file set')
            files[name] = body
    except (ValueError, TypeError) as error:
        raise source.JournalCorruption('private construction control contains invalid bounded bytes') from error
    return plan, files


def _write_new(path, raw):
    """Exclusive staged file, no replacement or cleanup of retained evidence."""
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, 'wb') as stream:
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())
    source._sync_directory(path.parent)


def install_private_package(context, target, files, *, request, guard, verify_retained):
    """Bounded repeatable private staging; original and committed data survive.

    A durable plan owns exact bytes before output writes. A retry fills only
    absent staged files and can resume after any completed file. A torn file,
    absent/torn plan or foreign residue fails closed for explicit owner review;
    it is never deleted or overwritten. One control per exact target/command
    prevents failed retries from creating an unbounded family of stages.
    """
    units._private_directory(context, target.parent)
    target_pins = deposit._pins(target)
    control = _control_path(context, target, request)
    if not os.path.lexists(control):
        control.mkdir(mode=0o700)
        source._sync_directory(context.private_root)
        plan = {'schema_version': 'tos_native_construction_stage_v1',
            'target_ref': target.relative_to(context.private_root).as_posix(),
            'request_digest': source._digest(source._canonical(request)),
            'files': {name: base64.b64encode(body).decode('ascii') for name, body in files.items()}}
        raw = _encoded(plan)
        if len(raw) > MAX_CONTROL_BYTES:
            raise ValueError('private construction recovery plan exceeds its bounded byte budget')
        _write_new(control / 'plan.json', raw)
    plan, retained = _control_files(context, control)
    control_pins = deposit._pins(control / 'plan.json')
    if (plan['target_ref'] != target.relative_to(context.private_root).as_posix()
            or plan['request_digest'] != source._digest(source._canonical(request))):
        raise source.JournalConflict('private construction control belongs to another exact target or command')
    verify_retained(retained)
    guard()
    if deposit._pins(control / 'plan.json') != control_pins or _control_files(context, control) != (plan, retained):
        raise source.JournalConflict('private construction recovery control changed before staging')
    output = control / 'output'
    if not os.path.lexists(output):
        output.mkdir(mode=0o700)
        source._sync_directory(control)
    units._private_directory(context, output)
    if _bounded_names(output, MAX_FILES) - set(retained):
        raise source.JournalCorruption('private construction stage contains unbound output')
    for name, raw in retained.items():
        path = output / name
        if os.path.lexists(path):
            if private_read(path, len(raw), private_root=context.private_root) != raw:
                raise source.JournalCorruption('private construction has torn or changed staged bytes; retained for owner review')
        else:
            _write_new(path, raw)
    if _private_package(context, output) != retained:
        raise source.JournalConflict('private staged construction differs from its retained plan')
    guard()
    if (deposit._pins(target) != target_pins or deposit._pins(control / 'plan.json') != control_pins
            or _control_files(context, control) != (plan, retained)):
        raise source.JournalConflict('private construction destination ancestor or recovery control changed before commit')
    source._publish_new_directory(output, target)
    if _private_package(context, target) != retained:
        raise source.JournalCorruption('committed private construction failed its stored-byte verification')
    return retained


def _verify_layer(files, *, config, configuration_digest, request, deadline, exclude=None):
    expected, dependencies, _ = _prepare(config, deadline=deadline, exclude=exclude)
    required = set(expected) | {RECEIPT_FILE, 'source-create-request.json', 'source-create-environment.json', 'source-create-provenance.jsonl'}
    if set(files) != required or any(files.get(name) != raw for name, raw in expected.items()):
        raise source.JournalConflict('native layer retry changed retained outputs or exact input dependencies')
    receipt = _verify_receipt(files, config=config, configuration_digest=configuration_digest, request=request,
        source_id=config['identities']['layer_id'])
    return receipt, dependencies


def run_command(owner_config, config, configuration_digest, path, request):
    source.command_handler(config['schema_version']).validate_request(request)
    operation = request['operation']
    context = OwnerLocalSourceContext.load(config['source_context_ref'])
    deadline = time.monotonic() + config['limits']['max_seconds']
    target = path.parent
    def result(receipt=None, *, replayed=False):
        return {'schema_version': 'tos_local_text_layer_create_result_v1',
            'authentication': 'local-unix-account', 'owner_configuration': configuration_digest,
            'target_exists': os.path.lexists(target), 'expected_source': None, 'expected_revision': None,
            'supported_operations': [OPERATION], 'command_operations': ['describe', 'prepare-create', OPERATION],
            'receipt_sha256': source._digest(source._canonical(receipt)) if receipt is not None else None,
            'replayed': replayed, 'grants_admission': False, 'content_disclosure': 'withheld'}
    if operation == 'describe':
        return result()
    if operation == 'prepare-create':
        _, dependencies, _ = _prepare(config, deadline=deadline)
        return {**result(), 'expected_dependencies': dependencies}
    if not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256:
        raise ValueError('native construction command identity must be finite')
    with source._locked(context.public_root / 'ToS/source-witnesses/historical-create'), source._locked(context.private_root / 'native-create'):
        os.close(_open(context.private_root / '.native-create.writer.lock', private_root=context.private_root))
        if source._configuration(owner_config)[1:] != (configuration_digest, path):
            raise source.JournalConflict('native layer delegation changed before construction')
        if os.path.lexists(target):
            files = _private_package(context, target)
            receipt, dependencies = _verify_layer(files, config=config, configuration_digest=configuration_digest,
                request=request, deadline=deadline, exclude=target)
            if (_prepare(config, deadline=deadline, exclude=target)[1] != dependencies
                    or _private_package(context, target) != files or source._configuration(owner_config)[1:] != (configuration_digest, path)):
                raise source.JournalConflict('native layer dependencies or package changed during retry')
            return result(receipt, replayed=True)
        if (request['expected_configuration'] != configuration_digest or request['expected_source'] is not None
                or request['expected_revision'] is not None):
            raise source.JournalConflict('native layer requires an exact grant and an absent initial source')
        started_at, started_ns = datetime.now(timezone.utc).isoformat(), time.perf_counter_ns()
        files, dependencies, inputs = _prepare(config, deadline=deadline)
        if dependencies != request['expected_dependencies']:
            raise source.JournalConflict('prepared native extraction dependencies changed')
        source._capture_creation_provenance({**config, 'source_root': str(context.public_root),
                'provenance_event_id': config['identities']['provenance_event_id']}, request, files, started_at, started_ns,
            procedure_name='exact-native-text-layer-structural-extraction',
            additional_software_refs=IMPLEMENTATIONS[-1:] + IMPLEMENTATIONS[-4:-3], native_inputs=inputs)
        layer = source._json_object(files[path.name])
        subject = source.Record.from_payload(layer['layer_id'], layer['layer_version'], layer)
        receipt = {'schema_version': 'tos_local_source_create_receipt_v1', 'command_id': request['command_id'],
            'request_digest': source._digest(source._canonical(request)), 'principal_id': config['principal_id'],
            'authority_ref': config['authority_ref'], 'owner_configuration': configuration_digest,
            'recorded_at': datetime.now(timezone.utc).isoformat(), 'source_path': config['source_path'],
            'source': subject.ref, 'dependencies': dependencies, 'files': _file_refs(files), 'grants_admission': False}
        files[RECEIPT_FILE] = _encoded(receipt)
        def guard():
            _deadline(deadline)
            if (_prepare(config, deadline=deadline)[1] != dependencies
                    or source._configuration(owner_config)[1:] != (configuration_digest, path)
                    or context.snapshot() != OwnerLocalSourceContext.load(config['source_context_ref']).snapshot()):
                raise source.JournalConflict('native layer exact inputs, context or grant changed before commit')
        def verify_retained(retained):
            _verify_layer(retained, config=config, configuration_digest=configuration_digest,
                          request=request, deadline=deadline)
        retained = install_private_package(context, target, files, request=request, guard=guard, verify_retained=verify_retained)
        return result(source._json_object(retained[RECEIPT_FILE]))


def command_handlers():
    return (contract.Handler('owner-local-text-layer-create', (CONFIG,), (contract.describe(),
        contract.operation('prepare-create', definition='Prepare one independently selected exact private structural extraction.', grants=(OPERATION,)),
        contract.operation(OPERATION, contract.COMMIT_KEYS,
            definition='Create an immutable unreviewed private TextLayer and source anchor, with exact retry and bounded retained recovery.',
            mutation='private_text_layer_package', grants=(OPERATION,))), run_command,
        'Bounded source-owner extraction from one already acquired EPUB member; not text assessment or segmentation.',
        configure=configuration, typed_handles=(LAYER_SCHEMA, ANCHOR_SCHEMA, units.PROVENANCE_SCHEMA),
        profile_selection='An independently protected grant selects exact File/member/selector, separate reading/derivation rights and one private destination.',
        preconditions=('Grant-free discovery does not read source contexts, private paths, grants, selectors, rights or payload bytes.',
            'Runtime requires a bounded supported source, exact local derivative rights and unchanged source/implementation dependencies.')),)
