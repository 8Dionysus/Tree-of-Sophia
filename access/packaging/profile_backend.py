#!/usr/bin/env python3
"""Opt-in local measurement; modifies only the explicitly supplied scratch cache.

Source variants live in memory. No imports, source edits, HTTP mutations or
semantic acceptance. Run under the host resource/storage admission route.
"""
import argparse
import gc
import hashlib
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'src'))
from tos_access.core import ToSAccessCore
from tos_access.knowledge import build_knowledge_graph, _stable_digest, knowledge_catalog, search_knowledge_graph
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
    reference = build_knowledge_graph(*inputs)
    expected = _stable_digest(reference)
    for name, operation in [('catalog', lambda: knowledge_catalog(reference,inputs[0],inputs[1],inputs[3],inputs[4])),
                            ('search', lambda: search_knowledge_graph(reference,'Заратустра',limit=6))]:
        for attempt in range(2):
            started = time.monotonic(); packet = operation(); seconds = time.monotonic()-started
            emit({'query':name, 'attempt':attempt, 'seconds':seconds,
                  'bytes':len(json.dumps(packet,ensure_ascii=False).encode())})
    del packet, reference
    gc.collect()
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
        emit({'scenario':scenario, 'seconds':time.monotonic()-started,
              'status':report['status'], 'steps':report['steps_by_kind'],
              'cache':report['cache'], 'cache_file_bytes':cache_path.stat().st_size,
              'equal':scenario != 'interrupt'})
        del cache, evaluate
        gc.collect()
    assert {str(p.relative_to(args.root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths} == input_digests, 'source inputs changed during measurement'


if __name__ == '__main__':
    main()
