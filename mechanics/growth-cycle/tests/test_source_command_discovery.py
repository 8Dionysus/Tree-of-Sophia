"""Grant-free handler discovery and its shared dispatch/key boundary."""
from contextlib import ExitStack
from dataclasses import replace
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[3]
MECHANIC = ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'
sys.path.insert(0, str(MECHANIC))
import source_commands as source
import source_command_contracts as contract


class SourceCommandDiscoveryTests(unittest.TestCase):
    def test_catalogue_is_the_exact_connected_handler_grammar(self):
        handlers = source.command_handlers()
        discovered = source.discover_commands()
        self.assertEqual(discovered['handlers'], [handler.public() for handler in handlers])
        self.assertEqual(discovered['handler_count'], len(handlers))
        schemas = {schema for handler in handlers for schema in handler.owner_schemas}
        self.assertEqual(schemas, {
            'tos_local_source_command_owner_v1', *source.CREATION_CONFIGS,
            *source.PROFILE_CREATION_CONFIGS, source.CORPUS_CONFIG, source.CLAIM_FORM_CONFIG,
            source.REVISION_CONFIG, source.PROFILE_REVISION_CONFIG, source.CORPUS_REVISION_CONFIG,
            source.CORPUS_SELECTED_REVISION_CONFIG, source.CLAIM_CONFIG, source.CLAIM_VALUE_CONFIG,
            source.CLAIM_STRUCTURED_CONFIG, source.CLAIM_REFERENCE_CONFIG,
            source.CLAIM_REVISION_CONFIG, source.CLAIM_VALUE_REVISION_CONFIG,
            source.CLAIM_STRUCTURED_REVISION_CONFIG, source.CLAIM_REFERENCE_REVISION_CONFIG,
            source.CLAIM_LAYER_REVISION_CONFIG, source.TEXT_UNIT_CONFIG, source.OWNER_PROFILE_CONFIG,
            source.OWNER_CLAIM_CONFIG, source.OWNER_CLAIM_REFERENCE_CONFIG,
            'tos_local_work_expression_owner_v1', 'tos_local_expression_responsibility_owner_v1',
            'tos_local_expression_edition_owner_v1'})
        self.assertEqual(len(schemas), sum(len(handler.owner_schemas) for handler in handlers))
        for handler in handlers:
            with self.subTest(handler=handler.handler_id):
                self.assertTrue(handler.definition and handler.preconditions and handler.typed_handles)
                self.assertTrue((ROOT / handler.owner_route).is_file())
                self.assertTrue((ROOT / handler.public()['implementation_ref']).is_file())
                for handle in handler.typed_handles:
                    self.assertTrue((ROOT / handle).is_file(), handle)
                for schema in handler.owner_schemas:
                    self.assertIs(source.command_handler(schema), handler)
                for op in handler.operations:
                    request = {key: None for key in op.keys}
                    request.update(schema_version=handler.request_schema, operation=op.name)
                    self.assertIs(handler.validate_request(request), op)
                    shape = op.shape(handler.request_schema)
                    self.assertEqual(set(shape['required']), set(request))
                    self.assertEqual(set(shape['properties']), set(request))
                    self.assertFalse(shape['additionalProperties'])
                    mutations = [{**request, 'source_root': '/not-a-selector'},
                                 {**request, 'schema_version': handler.request_schema + '-unknown'},
                                 {**request, 'operation': 'schema.execute'},
                                 *({key: value for key, value in request.items() if key != removed}
                                   for removed in request)]
                    for mutation in mutations:
                        with self.assertRaises(ValueError):
                            handler.validate_request(mutation)

    def test_discovery_has_no_owner_source_clock_network_or_authority_input(self):
        source.command_handlers()  # Code imports are allowed; no owner or corpus content is selected.
        import source_owner_context
        import source_metadata_snapshot
        with ExitStack() as stack:
            for owner, name in ((source, '_read'), (source, '_configuration'),
                    (source_owner_context.OwnerLocalSourceContext, 'load'),
                    (source_metadata_snapshot, 'PublicationSnapshot'), (Path, 'read_bytes'), (Path, 'read_text')):
                stack.enter_context(patch.object(owner, name, side_effect=AssertionError('discovery attempted data IO')))
            result = source.run_local_command(None, {'schema_version': source.DISCOVERY_REQUEST, 'operation': 'discover'})
            self.assertEqual(result, source.discover_commands())
            self.assertEqual(result['authorization_status'], 'not_evaluated')
            self.assertFalse(result['reads_owner_configuration'])
            self.assertFalse(result['reads_source_targets'])
            self.assertFalse(result['grants_admission'])
            for handoff in result['owner_handoffs']:
                self.assertFalse(handoff['dispatched_here'])
            for row in result['handlers']:
                self.assertEqual(row['authorization_status'], 'not_evaluated')
                self.assertFalse(row['grants_admission'])
                self.assertNotIn('allowed_operations', row)
                for op in row['operations']:
                    self.assertFalse(op['grants_admission'])
            invalid = [
                {'schema_version': source.DISCOVERY_REQUEST, 'operation': 'discover', 'owner_config': '/private/grant'},
                {'schema_version': source.DISCOVERY_REQUEST, 'operation': 'discover', 'source_path': '/private/body'},
                {'schema_version': source.DISCOVERY_REQUEST, 'operation': 'discover', 'extensions': {}},
                {'schema_version': source.DISCOVERY_REQUEST, 'operation': 'record.revise'},
                {'schema_version': source.DISCOVERY_REQUEST, 'operation': 'discover', 'handler_id': '../assessment_journal'},
                {'schema_version': source.DISCOVERY_REQUEST, 'operation': 'discover', 'handler_id': []},
            ]
            for request in invalid:
                with self.subTest(request=request), self.assertRaises(ValueError):
                    source.run_local_command(None, request)
            with self.assertRaises(ValueError):
                source.run_local_command(Path('/private/never-open-this-grant'),
                    {'schema_version': source.DISCOVERY_REQUEST, 'operation': 'discover'})
            for handler in source.command_handlers():
                with self.assertRaises(PermissionError):
                    source.run_local_command(None, {'schema_version': handler.request_schema, 'operation': 'describe'})
            with self.assertRaises(PermissionError):
                source.run_local_command(None, {'schema_version': 'unknown', 'operation': 'discover'})

    def test_descriptor_is_used_for_configuration_dispatch_and_request_keys(self):
        parsed = {'schema_version': 'test-only:handler', 'source_root': '/not-read'}
        configure = Mock(return_value=(parsed, 'unchanged-config-digest', Path('/not-read/target')))
        run = Mock(return_value={'sentinel': 'no mutation'})
        operation = contract.operation('test-only', {'handler_owned_key'}, definition='Synthetic dispatch boundary.')
        handler = contract.Handler('test-only', ('test-only:handler',), (operation,), run,
            'Synthetic descriptor; never in production catalogue.', configure=configure, manages_publication=True)
        owner = Path('/independently-selected-owner')
        with patch.object(source, 'command_handlers', return_value=(handler,)), \
                patch.object(source, '_read', return_value=json.dumps(parsed).encode()) as read:
            request = {'schema_version': contract.REQUEST, 'operation': 'test-only', 'handler_owned_key': None}
            self.assertEqual(source.run_local_command(owner, request), {'sentinel': 'no mutation'})
            configure.assert_called_once_with(parsed, owner_config=owner)
            run.assert_called_once_with(owner, parsed, 'unchanged-config-digest', Path('/not-read/target'), request)
            with self.assertRaises(ValueError):
                source.run_local_command(owner, {**request, 'execute': 'untrusted'})
            self.assertEqual(run.call_count, 1)
            changed = replace(handler, operations=(replace(operation, keys=frozenset({'different_key'})),))
            with patch.object(source, 'command_handlers', return_value=(changed,)):
                with self.assertRaises(ValueError):
                    source.run_local_command(owner, request)
            with self.assertRaises(ValueError):
                source.command_handler('unknown-extension')

    def test_direct_connected_handlers_reject_extension_keys_before_data_io(self):
        handlers = source.command_handlers()
        import source_owner_context
        import source_metadata_snapshot
        with patch.object(source, '_read', side_effect=AssertionError('invalid shape performed source IO')), \
                patch.object(source_owner_context.OwnerLocalSourceContext, 'load', side_effect=AssertionError('opened private context')), \
                patch.object(source_metadata_snapshot, 'PublicationSnapshot', side_effect=AssertionError('opened corpus')):
            for handler in handlers:
                for schema in handler.owner_schemas:
                    with self.subTest(handler=handler.handler_id, owner_schema=schema), self.assertRaises(ValueError):
                        handler.run(Path('/unused/owner'), {'schema_version': schema,
                            'source_root': '/unused', 'claim_id': 'tos.claim.unused'}, 'unused', Path('/unused/target'),
                            {'schema_version': handler.request_schema, 'operation': 'describe', 'extensions': {'execute': 'never'}})

    def test_cli_is_grant_free_fresh_process_deterministic_and_filterable(self):
        with tempfile.TemporaryDirectory() as temporary:
            env = {**os.environ, 'PYTHONDONTWRITEBYTECODE': '1'}
            command = [sys.executable, str(MECHANIC / 'source_commands.py')]
            first = subprocess.run([*command, '--discover'], input='stdin is not parsed with --discover',
                text=True, capture_output=True, cwd=temporary, env=env, check=True)
            second = subprocess.run(command, input=json.dumps({'schema_version': source.DISCOVERY_REQUEST,
                'operation': 'discover'}), text=True, capture_output=True, cwd=temporary, env=env, check=True)
            self.assertEqual(json.loads(first.stdout), json.loads(second.stdout))
            self.assertEqual(json.loads(first.stdout), source.discover_commands())
            selected = subprocess.run([*command, '--discover', '--handler', 'native-expression-responsibility'],
                text=True, capture_output=True, cwd=temporary, env=env, check=True)
            self.assertEqual(json.loads(selected.stdout), source.discover_commands({'schema_version': source.DISCOVERY_REQUEST,
                'operation': 'discover', 'handler_id': 'native-expression-responsibility'}))
            self.assertEqual(len(json.loads(selected.stdout)['handlers']), 1)
            for args in (['--discover', '--owner-config', '/not-opened'], ['--discover', '--handler', 'unknown'],
                         ['--handler', 'native-expression-responsibility']):
                failed = subprocess.run([*command, *args], input='', text=True, capture_output=True,
                                        cwd=temporary, env=env)
                self.assertEqual(failed.returncode, 2)
            self.assertEqual(list(Path(temporary).iterdir()), [])

    def test_fresh_import_discovery_never_opens_corpus_contracts_or_private_data(self):
        code = '''
import json, os, sys
def audit(event, args):
    if event == 'open' and isinstance(args[0], (str, bytes, os.PathLike)):
        path = os.fsdecode(args[0])
        if '/ToS/' in path or '/owner-local/' in path or '/private/' in path:
            raise AssertionError('discovery attempted source or private data IO')
    if event in {'socket.connect', 'socket.getaddrinfo', 'subprocess.Popen', 'os.system',
                 'os.mkdir', 'os.remove', 'os.rmdir', 'os.rename'}:
        raise AssertionError('discovery attempted network, process or mutation')
sys.addaudithook(audit)
sys.path.insert(0, sys.argv[1])
import source_commands
print(json.dumps(source_commands.discover_commands()))
'''
        result = subprocess.run([sys.executable, '-c', code, str(MECHANIC)],
            env={**os.environ, 'PYTHONDONTWRITEBYTECODE': '1'}, text=True, capture_output=True, check=True)
        self.assertEqual(json.loads(result.stdout), source.discover_commands())


if __name__ == '__main__':
    unittest.main()
