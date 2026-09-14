#!/usr/bin/env python3
"""Opt-in local measurement; modifies only the explicitly supplied scratch cache.

Source variants live in memory. No imports, source edits, HTTP mutations or
semantic acceptance. Run under the host resource/storage admission route.
"""
import argparse
import copy
import gc
import hashlib
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'src'))
from tos_access.core import ToSAccessCore
from tos_access.knowledge import (
    addressed_update_knowledge_graph,
    build_knowledge_graph,
    focus_knowledge_node,
    inspect_knowledge_node,
    inspect_knowledge_relation,
    _stable_digest,
    knowledge_catalog,
    search_knowledge_graph,
)
from tos_access.normalization_cache import NormalizationCache, normalization_processor_digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--scenarios', nargs='+', default=['cold', 'warm', 'metadata', 'deletion', 'review', 'interrupt', 'resume'],
                        choices=['cold', 'warm', 'metadata', 'deletion', 'review', 'interrupt', 'resume'])
    args = parser.parse_args()
    if args.scenarios[0] != 'cold' or ('resume' in args.scenarios and
            (args.scenarios.index('resume') == 0 or args.scenarios[args.scenarios.index('resume')-1] != 'interrupt')):
        parser.error('start with cold; resume must immediately follow interrupt')
    cache_path = args.cache.resolve()
    report_path = cache_path.with_suffix('.profile.jsonl')
    if cache_path.is_relative_to(args.root.resolve()) or cache_path.exists() or report_path.exists():
        parser.error('use a new scratch cache outside the source repository')
    report_path.parent.mkdir(parents=True, exist_ok=True)
    def emit(packet):
        line = json.dumps(packet)
        with report_path.open('a', encoding='utf-8') as stream:
            stream.write(line+'\n')
        print(line, flush=True)
    core = ToSAccessCore.discover(args.root)
    paths = [core.index_path, core.philosophy_graph_projection_path, core.bibliographic_graph_path,
             core.entity_type_registry_path, core.relation_type_registry_path]
    inputs = [json.loads(path.read_text()) for path in paths]
    processor = normalization_processor_digest(args.root/'access/src/tos_access/knowledge.py')
    input_digests = {str(p.relative_to(args.root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}
    emit({'processor':processor, 'inputs':input_digests})
    # Use the real core lifecycle for the baseline.  This profiler keeps all
    # source variants in memory and therefore measures the pure addressed
    # transform below; the owner-level durable source transition and consumer
    # publication path is covered by the access contract tests.
    reference = core.knowledge_graph()
    expected = _stable_digest(reference)
    query_measurements = []
    first_node_id = reference['nodes'][0]['id'] if reference.get('nodes') else None
    first_relation_id = reference['relations'][0]['id'] if reference.get('relations') else None
    query_operations = [
        ('catalog', lambda: knowledge_catalog(reference, inputs[0], inputs[1], inputs[3], inputs[4])),
        ('search', lambda: search_knowledge_graph(reference, 'Заратустра', limit=6)),
    ]
    if first_node_id:
        query_operations.extend([
            ('focus', lambda: focus_knowledge_node(reference, first_node_id, depth=1, profile='overview')),
            ('inspect-node', lambda: inspect_knowledge_node(reference, first_node_id, relation_limit=6)),
        ])
    if first_relation_id:
        query_operations.append(
            ('inspect-relation', lambda: inspect_knowledge_relation(reference, first_relation_id))
        )
    for name, operation in query_operations:
        for attempt in range(2):
            started = time.monotonic(); packet = operation(); seconds = time.monotonic()-started
            result_bytes = len(json.dumps(packet,ensure_ascii=False).encode())
            query_measurements.append({'operation': name, 'attempt': attempt,
                                       'seconds': seconds, 'bytes': result_bytes})
            emit({'query':name, 'attempt':attempt, 'seconds':seconds, 'bytes':result_bytes})

    # Measure the addressed replacement path against the full builder oracle.
    # The source owner supplies one exact raw carrier; no source scan is
    # hidden in the addressed call.  A missing direct carrier is a bounded
    # diagnostic skip, never a synthetic success.
    addressed_matrix = {
        'schema': 'tos_access_addressed_measurement_v1',
        'processor': processor,
        'cases': [],
    }
    # Keep the large benchmark-only snapshots out of the later cache
    # scenarios.  Their lifetime is intentionally bounded to this measurement
    # block; retaining them would make the scenario memory envelope a property
    # of the profiler rather than of the backend path under test.
    full_changed = addressed_result = addressed_graph = changed_inputs = None
    replacement = raw_carrier = raw_carriers = None
    raw_carriers = inputs[1].get('nodes') if isinstance(inputs[1], dict) else None
    raw_carrier = copy.deepcopy(raw_carriers[0]) if isinstance(raw_carriers, list) and raw_carriers and isinstance(raw_carriers[0], dict) else None
    raw_source_id = (raw_carrier.get('node_id') or raw_carrier.get('id') or raw_carrier.get('path')) if raw_carrier else None
    if not isinstance(raw_source_id, str) or not raw_source_id:
        addressed_matrix['cases'].append({'operation': 'edit', 'path': 'addressed-update', 'status': 'skipped',
                                          'reason': 'no-addressable-philosophy-carrier'})
    else:
        replacement = copy.deepcopy(raw_carrier)
        replacement['profile_probe'] = 'addressed-edit'
        changed_inputs = copy.deepcopy(inputs)
        changed_inputs[1]['nodes'][0] = replacement
        full_started = time.monotonic()
        full_changed = build_knowledge_graph(*changed_inputs)
        full_seconds = time.monotonic() - full_started
        addressed_started = time.monotonic()
        addressed_result = addressed_update_knowledge_graph(
            reference, 'philosophy', raw_source_id, replacement,
            inputs[3], inputs[4],
            source_revision=full_changed['source_revision'], return_report=True,
        )
        addressed_seconds = time.monotonic() - addressed_started
        addressed_graph = addressed_result['graph']
        if addressed_graph != full_changed:
            raise AssertionError('addressed/full graph parity failed')
        successor_search = search_knowledge_graph(addressed_graph, 'addressed-edit')
        successor_focus = focus_knowledge_node(addressed_graph, f'philosophy:{raw_source_id}', depth=1)
        addressed_matrix['cases'].append({
            'operation': 'edit', 'path': 'addressed-update', 'status': 'measured',
            'source_graph': 'philosophy', 'source_id': raw_source_id,
            'full_seconds': full_seconds, 'addressed_seconds': addressed_seconds,
            'full_source_records_read': 'complete',
            'publication': 'pure-transform; durable core transition is contract-tested',
            'addressed_input_traversal': addressed_result['report']['input_traversal'],
            'recomputed': addressed_result['report']['recomputed'],
            'equal_to_full': True,
            'previous_snapshot_mutated': addressed_result['report']['snapshot']['previous_snapshot_mutated'],
            'successor_queries': {
                'search_matching_nodes': successor_search['counts']['matching_nodes'],
                'search_source_revision': successor_search['source_revision'],
                'focus_source_revision': successor_focus['source_revision'],
            },
        })
    # The later cache scenarios measure their own builder lifetimes; do not
    # retain the core's addressed successor beyond this caller-adoption probe.
    core._addressed_graph = None
    core._addressed_source_state = None
    del full_changed, addressed_result, addressed_graph, changed_inputs, replacement, raw_carrier, raw_carriers
    gc.collect()
    emit({'addressed_matrix': addressed_matrix})
    del packet, reference
    gc.collect()
    scenario_measurements = []
    for scenario in args.scenarios:
        if scenario == 'metadata':
            inputs[1]['nodes'][0]['profile_probe'] = 'metadata-only'
        elif scenario == 'deletion':
            inputs[1]['edges'].pop()
        elif scenario == 'review':
            inputs[2]['claim_traces'][0]['review_status'] = 'profile-unreviewed'
        elif scenario == 'interrupt':
            inputs[1]['nodes'][0]['profile_probe'] = 'resume-probe'
        if scenario not in ('cold', 'warm', 'resume'):
            started = time.monotonic()
            reference = build_knowledge_graph(*inputs)
            expected = _stable_digest(reference)
            emit({'reference':scenario, 'seconds':time.monotonic()-started})
            del reference; gc.collect()
        started = time.monotonic()
        cache = NormalizationCache(cache_path, processor, keep_runs=2)
        calls = 0
        evaluate = cache.scheduler.evaluate
        def interrupted(node):
            nonlocal calls
            calls += 1
            if calls == 50000:
                raise RuntimeError('profile-interruption')
            return evaluate(node)
        if scenario == 'interrupt':
            cache.scheduler.evaluate = interrupted
        try:
            with cache:
                graph = build_knowledge_graph(*inputs)
                assert _stable_digest(graph) == expected, 'cached/full result differs'
                del graph
                if scenario == 'interrupt':
                    raise AssertionError('corpus did not reach the interruption point')
        except RuntimeError as error:
            if scenario != 'interrupt' or str(error) != 'profile-interruption':
                raise
        report = cache.processing_report
        scenario_packet = {'scenario':scenario, 'seconds':time.monotonic()-started,
                           'status':report['status'], 'steps':report['steps_by_kind'],
                           'cache':report['cache'], 'cache_file_bytes':cache_path.stat().st_size,
                           'equal':scenario != 'interrupt'}
        scenario_measurements.append(scenario_packet)
        emit(scenario_packet)
        del cache, evaluate
        gc.collect()
    emit({
        'measurement_matrix': {
            'schema': 'tos_access_measurement_matrix_v1',
            'processor': processor,
            'queries': query_measurements,
            'addressed': addressed_matrix['cases'],
            'scenarios': scenario_measurements,
            'scope': {
                'source_assembly': 'full-builder-or-addressed-one-record',
                'semantic_acceptance': False,
                'deployment': False,
            },
        }
    })
    assert {str(p.relative_to(args.root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths} == input_digests, 'source inputs changed during measurement'


if __name__ == '__main__':
    main()
