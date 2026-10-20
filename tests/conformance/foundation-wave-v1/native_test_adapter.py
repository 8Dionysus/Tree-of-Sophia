#!/usr/bin/env python3
"""Use a previously compiled, explicitly selected test executable. Never compiles."""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile

def invoke_exact_bytes(binary, raw, wire):
    """Test transport only: no expected values, schema emulation or numeric coercion."""
    if len(raw) > 1_048_576:
        raise ValueError('bounded test request exceeds 1 MiB')
    with tempfile.TemporaryDirectory(prefix='tos-rt3-wire-') as td:
        req, actual = pathlib.Path(td) / 'request.json', pathlib.Path(td) / 'actual.json'
        req.write_bytes(raw)
        env = dict(os.environ, TOS_RT3_REQUEST=str(req), TOS_RT3_ACTUAL=str(actual))
        env.pop('TOS_RT3_INTEGER_WIRE', None)
        if wire:
            env['TOS_RT3_INTEGER_WIRE'] = '1'
        proc = subprocess.run([str(binary), '--ignored', '--exact', 'rt3_execute_request'],
                              env=env, capture_output=True, timeout=60)
        if proc.returncode or not actual.exists():
            raise RuntimeError((proc.stdout + proc.stderr).decode('utf-8', errors='replace'))
        return actual.read_bytes()


def js_roundtrip(node, raw):
    # Only the outer JSON is parsed. raw_json evidence stays a JS String.
    script = "const fs=require('fs');process.stdout.write(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8'))));"
    return subprocess.run([str(node), '-e', script], input=raw, capture_output=True,
                          timeout=30, check=True).stdout


def safe_tree(value):
    if isinstance(value, float):
        raise AssertionError('Float escaped exact integer wire')
    if isinstance(value, int) and not isinstance(value, bool):
        assert abs(value) <= 9_007_199_254_740_991, 'unsafe numeric leaf escaped'
    if isinstance(value, dict):
        for child in value.values():
            safe_tree(child)
    elif isinstance(value, list):
        for child in value:
            safe_tree(child)


def js_interop(binary, node, out):
    """Compact parameter rows in the existing adapter, run only by explicit admission."""
    out.mkdir(parents=True, exist_ok=False)
    max_safe = 9_007_199_254_740_991
    def position(n):
        return {'type': 'CoordinatePosition', 'value': n,
                'axis': {'trs': {'id': 'X', 'version': 'rules1'}, 'unit': 'seconds', 'origin': 'O'}}
    def request(begin, end, extra=None, encoding='SafeJsonNumberV1'):
        data = {'beginning': position(begin), 'end': position(end),
                'duration_kind': 'Elapsed', 'negative_policy': 'Signed'}
        if extra:
            data.update(extra)
        return {'schema_version': 'tos_exact_integer_wire_v1', 'integer_encoding': encoding,
                'operation': 'difference', 'payload': {'input': data, 'context': {}}}
    rows = []
    for n in [max_safe - 1, max_safe, -max_safe + 1, -max_safe]:
        rows.append((f'safe-{n}', request(0, n), 'Encoded', str(n), [], True))
    # Accepted native magnitude String profile: wide result is not an unsafe Number.
    for begin, end in [(-max_safe, max_safe), (max_safe, -max_safe)]:
        rows.append((f'wide-safe-{end}', request(begin, end), 'Encoded', str(end - begin), [], True))
    for n in [max_safe + 1, max_safe + 2, -max_safe - 1, -max_safe - 2]:
        rows.append((f'unsafe-{n}', request(0, n), 'Unsupported', None, [str(n)], False))
    for sign in [1, -1]:
        a, b = sign * (max_safe + 1), sign * (max_safe + 2)
        rows.append((f'opaque-{sign}', request(0, 1, {'opaque_evidence': [a, b]}),
                     'Unsupported', None, [str(a), str(b)], False))
    context_ref = request(0, 1)
    context_ref['payload']['context'] = {'source_refs': [
        {'id': 'fixture:version-A', 'version': max_safe + 1},
        {'id': 'fixture:version-B', 'version': max_safe + 2}]}
    rows.append(('unsafe-context-versions', context_ref, 'Unsupported', None,
                 [str(max_safe + 1), str(max_safe + 2)], False))
    for token in ['4.0', '-0']:
        raw = json.dumps(request(0, 'TOKEN')).replace('"TOKEN"', token).encode()
        rows.append((f'lexeme-{token}', raw, 'Unsupported', None, [token], False))
    rows.append(('unimplemented-decimal', request('9007199254740992', '9007199254740993',
                 encoding='CanonicalDecimalStringV1'), 'Unsupported', None, [], False))
    report = {'product_expectations_received': False, 'owner_readback_verified': False,
              'node': str(node), 'node_version': subprocess.check_output([str(node), '--version'], text=True).strip(),
              'node_sha256': hashlib.sha256(node.resolve().read_bytes()).hexdigest(),
              'binary': str(binary), 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'rows': []}
    for name, req, outcome, magnitude, raw_tokens, safe_request in rows:
        raw = req if isinstance(req, bytes) else json.dumps(req, ensure_ascii=False).encode()
        folder = out / name
        folder.mkdir()
        (folder / 'request.json').write_bytes(raw)
        actual = invoke_exact_bytes(binary, raw, True)
        (folder / 'actual.json').write_bytes(actual)
        value = json.loads(actual)
        safe_tree(value)
        assert value['schema_version'] == 'tos_exact_integer_wire_result_v1'
        assert value['outcome'] == outcome
        if outcome == 'Encoded':
            assert value['representation'] == 'StructuredJson'
            assert value['elapsed_magnitude_profile'] == 'CanonicalDecimalI128StringV1'
            assert value['payload']['duration']['magnitude'] == magnitude
        else:
            assert value['representation'] == 'OpaqueRawJson'
            assert all(k not in value for k in ['payload', 'cursor', 'next', 'exhausted'])
            opaque = value['raw_json']
            assert isinstance(opaque, str)
            assert value['emitted_json_sha256'] == 'sha256:' + hashlib.sha256(opaque.encode()).hexdigest()
            assert all(token in opaque for token in raw_tokens)
        js = js_roundtrip(node, actual)
        (folder / 'js-response.json').write_bytes(js)
        assert json.loads(js) == value
        # Unsafe evidence is opaque after refusal; never parse it in JS as numeric JSON.
        repeated_request = js_roundtrip(node, raw) if safe_request else raw
        (folder / 'repeat-request.json').write_bytes(repeated_request)
        repeated = invoke_exact_bytes(binary, repeated_request, True)
        (folder / 'repeat-actual.json').write_bytes(repeated)
        assert json.loads(repeated) == value
        report['rows'].append({'name': name, 'status': 'Pass', 'request_via_JS': safe_request,
                               'response_via_JS': True, 'repeat_request_verified': True})
        (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    # Separate native-only control: only its canonical duration String enters JS.
    # Native unsafe endpoint echoes are NOT claimed as Safe-profile JSON.
    for begin, end in [(-(2**63), 2**63 - 1), (2**63 - 1, -(2**63))]:
        native = request(begin, end)
        native = {'operation': native['operation'], **native['payload']}
        raw = json.dumps(native).encode()
        actual = invoke_exact_bytes(binary, raw, False)
        magnitude = json.loads(actual)['duration']['magnitude']
        assert magnitude == str(end - begin)
        control = json.dumps({'magnitude': magnitude}).encode()
        assert json.loads(js_roundtrip(node, control)) == {'magnitude': magnitude}
        assert json.loads(invoke_exact_bytes(binary, raw, False))['duration']['magnitude'] == magnitude
        suffix = 'positive' if end > begin else 'negative'
        (out / f'native-wide-{suffix}.json').write_bytes(actual)
        report['rows'].append({'name': f'native-wide-{suffix}', 'status': 'Pass',
                               'scope': 'Native duration String only; unsafe native echoes not JS-safe'})
    (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return 0


def js_decimal_interop(binary, node, out):
    """Opt-in scoped rows; historical Safe18 and legacy transport stay unchanged."""
    out.mkdir(parents=True, exist_ok=False)
    encoding = 'CanonicalDecimalStringV1'
    scope = 'DifferenceCoordinateI64V1'

    def request(begin='9007199254740992', end='9007199254740993', policy='Signed'):
        def position(value):
            return {'type': 'CoordinatePosition', 'value': value,
                    'axis': {'trs': {'id': 'independent-axis', 'version': 'rules1'},
                             'unit': 'tick', 'origin': 'independent-origin'}}
        data = {'beginning': position(begin), 'end': position(end), 'duration_kind': 'Elapsed'}
        if policy is not None:
            data['negative_policy'] = policy
        return {'schema_version': 'tos_exact_integer_wire_v1', 'integer_encoding': encoding,
                'operation': 'difference', 'payload': {'input': data, 'context': {}}}

    rows = []

    def add(name, req, status=None, magnitude=None, reason=None, refusal=False,
            via_js=True, tokens=(), scoped=True):
        rows.append((name, req, status, magnitude, reason, refusal, via_js, tokens, scoped))

    for name, begin, end in [
        ('adjacent-positive', '9007199254740992', '9007199254740993'),
        ('adjacent-negative', '-9007199254740993', '-9007199254740992'),
        ('full-positive', str(-(2**63)), str(2**63 - 1)),
        ('full-negative', str(2**63 - 1), str(-(2**63))),
        ('zero', '0', '0'), ('negative-one', '0', '-1'),
    ]:
        # Independent arbitrary precision oracle; product receives only request.
        add(name, request(begin, end), 'Resolved', str(int(end) - int(begin)))
    add('reverse-policy-omitted', request('4', '1', None),
        'PolicyRequired', reason='NegativeDurationPolicyRequired')
    add('reverse-rejected', request('4', '1', 'Reject'),
        'Reversed', reason='EndBeforeBeginning')
    for key, value, reason in [('id', 'different-axis', 'NoCrosswalk'),
                               ('version', 'rules2', 'NoCrosswalk'),
                               ('unit', 'other-unit', 'IncompatibleUnits'),
                               ('origin', 'other-origin', 'IncompatibleOrigins')]:
        req = request()
        axis = req['payload']['input']['end']['axis']
        (axis['trs'] if key in ('id', 'version') else axis)[key] = value
        add('axis-' + key, req, 'Incomparable', reason=reason)
    for index, token in enumerate(['+1', '-0', '01', ' 1', '1 ', '1.0', '1e3', '١', '', '-']):
        add('grammar-' + str(index), request(token, '10'),
            reason='NonCanonicalCoordinateString', refusal=True)
    for name, token, reason in [
        ('above-i64', str(2**63), 'CoordinateOutOfRange'),
        ('below-i64', str(-(2**63) - 1), 'CoordinateOutOfRange'),
        ('39-digits', '9' * 39, 'CoordinateOutOfRange'),
        ('40-digits', '9' * 40, 'DecimalCoordinateDigitLimit'),
    ]:
        add(name, request(token, '10'), reason=reason, refusal=True)
    for name, value in [('number', 4), ('bool', True), ('null', None)]:
        add('non-string-' + name, request(value, '10'),
            reason='DecimalCoordinateStringRequired', refusal=True)
    add('unsafe-number', request(9007199254740993, '10'),
        reason='ExactIntegerOutOfWireRange', refusal=True, via_js=False,
        tokens=('9007199254740993',))
    for token in ['4.0', '-0']:
        raw = json.dumps(request('TOKEN', '10')).replace('"TOKEN"', token).encode()
        add('numeric-lexeme-' + token, raw,
            reason='JsonFloatNotSupported' if token == '4.0' else 'NonCanonicalJsonInteger',
            refusal=True, via_js=False, tokens=(token,))
    for name in ['calendar', 'ordinal', 'resolution', 'uncertainty', 'duration',
                 'opaque', 'context', 'string-version-context', 'numeric-axis-version',
                 'invalid-policy']:
        req = request()
        data = req['payload']['input']
        reason = 'DecimalDifferenceProfileNotSupported'
        if name in ('calendar', 'ordinal'):
            data['beginning']['type'] = 'CalendarPosition' if name == 'calendar' else 'OrdinalPosition'
        elif name == 'resolution':
            data['beginning']['resolution'] = 'year'
        elif name == 'uncertainty':
            data['beginning']['uncertainty'] = None
        elif name == 'duration':
            data['duration_kind'] = 'Calendar'
        elif name == 'opaque':
            data['opaque_evidence'] = {'coordinate': '9007199254740993'}
        elif name in ('context', 'string-version-context'):
            req['payload']['context'] = ({'system_as_of': 'Current'} if name == 'context'
                                         else {'record_version': '9007199254740993'})
            reason = 'DecimalDifferenceContextNotSupported'
        elif name == 'numeric-axis-version':
            data['beginning']['axis']['trs']['version'] = 1
        else:
            data['negative_policy'] = 'Implicit'
        add('scope-' + name, req, reason=reason, refusal=True)
    other = request()
    other['operation'] = 'compare_positions'
    add('other-operation', other, reason='IntegerEncodingNotImplemented',
        refusal=True, scoped=False)

    report = {'scope': scope, 'integer_encoding': encoding,
              'product_expectations_received': False, 'owner_writeback_verified': False,
              'source_module_basis': '259347a9c00c6ceeff18306265f847557162304f',
              'node': str(node), 'node_sha256': hashlib.sha256(node.resolve().read_bytes()).hexdigest(),
              'node_version': subprocess.check_output([str(node), '--version'], text=True).strip(),
              'binary': str(binary), 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
              'rows': []}
    for name, req, status, magnitude, reason, refusal, via_js, tokens, scoped in rows:
        folder = out / name
        folder.mkdir()
        raw = req if isinstance(req, bytes) else json.dumps(req, ensure_ascii=False).encode()
        original = json.loads(raw)  # Python retains Int exactly; never used as product output.
        (folder / 'request.json').write_bytes(raw)
        actual = invoke_exact_bytes(binary, raw, True)
        (folder / 'actual.json').write_bytes(actual)
        value = json.loads(actual)
        safe_tree(value)
        assert value['schema_version'] == 'tos_exact_integer_wire_result_v1'
        if scoped:
            assert value['integer_encoding'] == encoding
            assert value['supported_scope'] == scope
        else:
            # Existing generic refusal is Safe transport, never Decimal support.
            assert value['integer_encoding'] == 'SafeJsonNumberV1'
            assert 'supported_scope' not in value
        assert value['operation'] == original['operation']
        assert value['outcome'] == ('Unsupported' if refusal else 'Encoded')
        if refusal:
            assert value['reason'] == reason
            assert value['representation'] == 'OpaqueRawJson'
            assert all(key not in value for key in ('payload', 'next', 'cursor', 'exhausted'))
            opaque = value['raw_json']
            assert isinstance(opaque, str) and json.loads(opaque) == original
            assert value['emitted_json_sha256'] == 'sha256:' + hashlib.sha256(opaque.encode()).hexdigest()
            assert all(token in opaque for token in tokens)
        else:
            assert value['representation'] == 'StructuredJson'
            assert value['elapsed_magnitude_profile'] == 'CanonicalDecimalI128StringV1'
            payload = value['payload']
            assert payload['status'] == status
            assert payload['preserved_input'] == original['payload']['input']
            for endpoint in ('beginning', 'end'):
                assert isinstance(payload['preserved_input'][endpoint]['value'], str)
            if magnitude is not None:
                assert payload['duration']['kind'] == 'Elapsed'
                assert payload['duration']['magnitude'] == magnitude
                assert isinstance(payload['duration']['magnitude'], str)
            else:
                assert 'duration' not in payload
            if reason is not None:
                assert payload['reason'] == reason
        client_response = js_roundtrip(node, actual)
        (folder / 'js-response.json').write_bytes(client_response)
        assert json.loads(client_response) == value
        repeated_request = js_roundtrip(node, raw) if via_js else raw
        (folder / 'repeat-request.json').write_bytes(repeated_request)
        assert json.loads(repeated_request) == original
        repeated = invoke_exact_bytes(binary, repeated_request, True)
        (folder / 'repeat-actual.json').write_bytes(repeated)
        assert json.loads(repeated) == value
        report['rows'].append({'name': name, 'status': 'Pass', 'outcome': value['outcome'],
                               'request_via_JS': via_js, 'response_via_JS': True,
                               'repeat_request_verified': True})
        (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return 0


def main():
    if len(sys.argv) > 2:
        args = argparse.ArgumentParser(description=__doc__)
        args.add_argument('binary', type=pathlib.Path)
        mode = args.add_mutually_exclusive_group(required=True)
        mode.add_argument('--integer-wire', action='store_true')
        mode.add_argument('--js-interop', action='store_true')
        mode.add_argument('--js-decimal-interop', action='store_true')
        args.add_argument('--node', type=pathlib.Path, default=pathlib.Path('/usr/bin/node'))
        args.add_argument('--evidence-dir', type=pathlib.Path)
        selected = args.parse_args()
        if not selected.binary.is_absolute() or not selected.binary.is_file():
            args.error('explicit existing absolute test executable required')
        if selected.integer_wire:
            raw = sys.stdin.buffer.read(1_048_577)
            sys.stdout.buffer.write(invoke_exact_bytes(selected.binary, raw, True))
            return 0
        if not selected.node.is_absolute() or not selected.node.is_file():
            args.error('explicit existing absolute Node executable required')
        if selected.evidence_dir is None or not selected.evidence_dir.is_absolute():
            args.error('--js-interop requires an absolute new --evidence-dir')
        if selected.js_decimal_interop:
            return js_decimal_interop(selected.binary, selected.node, selected.evidence_dir)
        return js_interop(selected.binary, selected.node, selected.evidence_dir)
    if len(sys.argv) != 2:
        raise SystemExit('usage: native_test_adapter.py /absolute/test-executable')
    binary = pathlib.Path(sys.argv[1])
    if not binary.is_absolute() or not binary.is_file():
        raise SystemExit('An explicit existing absolute test executable is required')
    request = json.load(sys.stdin)
    assert 'expected' not in request and 'id' not in request
    with tempfile.TemporaryDirectory(prefix='tos-rt3-') as td:
        root = pathlib.Path(td)
        req = root / 'request.json'
        actual = root / 'actual.json'
        req.write_text(json.dumps(request))
        env = dict(os.environ, TOS_RT3_REQUEST=str(req), TOS_RT3_ACTUAL=str(actual))
        proc = subprocess.run([str(binary), '--ignored', '--exact', 'rt3_execute_request'],
                              env=env, capture_output=True, text=True, check=False)
        if proc.returncode or not actual.exists():
            sys.stderr.write(proc.stdout + proc.stderr)
            return proc.returncode or 1
        sys.stdout.write(actual.read_text())
    return 0

if __name__ == '__main__':
    sys.exit(main())
