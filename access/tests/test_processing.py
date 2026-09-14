import ast
import copy
import json
import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path
from contextlib import closing
from unittest.mock import patch

ACCESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ACCESS / 'src'))
from tos_access.processing import Input, Task, ProcessingScheduler, processing_input_changes
from tos_access.normalization_cache import NormalizationCache, normalization_processor_digest
from tos_access.knowledge import build_knowledge_graph, validate_knowledge_semantics


class ProcessingTests(unittest.TestCase):
    def test_validation_cache_avoids_public_digest_framing_and_keeps_parity(self):
        from test_knowledge_contract import KnowledgeContractTests
        KnowledgeContractTests.setUpClass()
        entities = KnowledgeContractTests.entity_type_registry
        relations = KnowledgeContractTests.relation_type_registry
        corpus, philosophy = KnowledgeContractTests().fixture()
        graph = build_knowledge_graph(corpus, philosophy, {}, entities, relations)
        expected = validate_knowledge_semantics(graph, entities, relations)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'cache.sqlite'
            for attempt in range(2):
                with NormalizationCache(path, 'v1') as cache:
                    with patch('tos_access.knowledge._stable_digest', side_effect=AssertionError('public digest framing used for private validation cache')):
                        self.assertEqual(validate_knowledge_semantics(graph, entities, relations), expected)
                if attempt:
                    self.assertEqual(cache.scheduler.executed, 0)
            graph['nodes'][0]['attributes']['nonfinite'] = float('nan')
            with self.assertRaises(ValueError):
                with NormalizationCache(path, 'v1'):
                    validate_knowledge_semantics(graph, entities, relations)

    def test_cached_output_is_not_reserialized_for_transport(self):
        with closing(sqlite3.connect(':memory:')) as db:
            value={'large-output':'retained'}
            task=Task('output','v1',(),None,lambda _:value)
            first=ProcessingScheduler(db);first.evaluate(task);first.finish()
            repeat=ProcessingScheduler(db)
            encode=json.dumps;output_encodes=[]
            def record(item,*args,**kwargs):
                if item==value:
                    output_encodes.append(item)
                return encode(item,*args,**kwargs)
            with patch('tos_access.processing.json.dumps',side_effect=record):
                self.assertEqual(repeat.evaluate(task)[0],value)
            self.assertEqual(len(output_encodes),1)  # Canonical dependency digest only.
            repeat.finish()

    def test_warm_receipts_do_not_commit_per_step_but_new_outputs_are_durable(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder)/'cache.sqlite'
            tasks = [Task(f'task:{i}','v1',(),None,lambda _, i=i:i) for i in range(12)]
            with NormalizationCache(path,'v1') as first:
                for task in tasks:
                    first.scheduler.evaluate(task)
            with NormalizationCache(path,'v1') as warm:
                statements=[];warm.connection.set_trace_callback(statements.append)
                for task in tasks:
                    warm.scheduler.evaluate(task)
                self.assertLess(sum(sql=='COMMIT' for sql in statements),2)
                warm.scheduler.evaluate(Task('new','v1',(),None,lambda _:99))
                with closing(sqlite3.connect(path)) as observer:
                    self.assertEqual(observer.execute('SELECT count(*) FROM completed_steps').fetchone()[0],13)

    def test_warm_task_does_not_decode_unused_dependency_payloads(self):
        with closing(sqlite3.connect(':memory:')) as db:
            source = Input('source:large', {'payload':'dependency-only'})
            task = Task('project', 'v1', (source,), None, lambda values: len(values[0]['payload']))
            first = ProcessingScheduler(db); expected = first.evaluate(task); first.finish()
            decode = json.loads
            def bounded_decode(value, *args, **kwargs):
                if 'dependency-only' in value:
                    raise AssertionError('warm task decoded an unused dependency')
                return decode(value, *args, **kwargs)
            repeat = ProcessingScheduler(db)
            with patch('tos_access.processing.json.loads', side_effect=bounded_decode):
                self.assertEqual(repeat.evaluate(task), expected)
            repeat.finish()
            self.assertEqual(repeat.executed, 0)

    def test_abandoned_run_recovers_and_failed_initialization_releases_lock(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'cache.sqlite'
            task = Task('completed', 'v1', (), None, lambda _: 42)
            abandoned = NormalizationCache(path, 'v1')
            abandoned.scheduler.evaluate(task)
            run_id = abandoned.scheduler.run_id
            # Model process resource teardown without publishing/finishing a run.
            abandoned.connection.close(); abandoned.lock.close()
            with NormalizationCache(path, 'v1') as recovered:
                self.assertEqual(recovered.connection.execute('SELECT status FROM processing_runs WHERE id=?', (run_id,)).fetchone()[0], 'interrupted')
                self.assertEqual(recovered.scheduler.evaluate(task)[0], 42)
                self.assertEqual(recovered.scheduler.reused, 1)
            with patch('tos_access.normalization_cache.sqlite3.connect', side_effect=OSError('cannot open')):
                with self.assertRaises(OSError):
                    NormalizationCache(path, 'v1')
            with NormalizationCache(path, 'v1') as retry:
                self.assertEqual(retry.scheduler.evaluate(task)[0], 42)

    def test_input_delta_pages_conserve_changes_and_reject_retired_baselines(self):
        with closing(sqlite3.connect(':memory:')) as db:
            first = ProcessingScheduler(db)
            for key, value in [('source-node:a', 1), ('source-node:b', 2), ('source-node:c', 3)]:
                first.evaluate(Input(key, value))
            first.finish()
            second = ProcessingScheduler(db)
            for key, value in [('source-node:a', 4), ('source-node:c', 3), ('source-node:d', 5), ('internal', 6)]:
                second.evaluate(Input(key, value))
            # An incomplete scan cannot report disappearance of b.
            partial = processing_input_changes(db, second.run_id)
            self.assertEqual(partial['coverage'], 'partial')
            self.assertNotIn('removed', [item['change'] for item in partial['items']])
            second.finish()
            expected = processing_input_changes(db, second.run_id)['items']
            self.assertEqual([(item['id'], item['change']) for item in expected],
                             [('source-node:a', 'changed'), ('source-node:b', 'removed'), ('source-node:d', 'added')])
            for size in (1, 2, 3, 10):
                actual, after = [], ''
                while True:
                    page = processing_input_changes(db, second.run_id, after=after, limit=size)
                    actual.extend(page['items'])
                    if page['next_after'] is None:
                        break
                    self.assertGreater(page['next_after'], after)
                    after = page['next_after']
                self.assertEqual(actual, expected)
            self.assertEqual(len(processing_input_changes(db, second.run_id, source_only=False)['items']), 4)
            for limit in (0, 1001, True, 1.5):
                with self.assertRaises(ValueError):
                    processing_input_changes(db, second.run_id, limit=limit)
            second.prune_history(1)
            with self.assertRaisesRegex(KeyError, 'baseline was retired'):
                processing_input_changes(db, second.run_id)

    def test_untrusted_cached_outputs_recompute_and_json_null_is_reusable(self):
        with closing(sqlite3.connect(':memory:')) as db:
            task = Task('null', 'v1', (), None, lambda _: None)
            first = ProcessingScheduler(db); first.evaluate(task); first.finish()
            second = ProcessingScheduler(db); second.evaluate(task); second.finish()
            self.assertEqual((second.executed, second.reused), (0, 1))
            db.execute("UPDATE completed_steps SET payload='123'")
            corrupt = ProcessingScheduler(db)
            self.assertEqual(corrupt.evaluate(task)[0], None)
            corrupt.finish()
            self.assertEqual((corrupt.executed, corrupt.reused), (1, 0))
            db.execute('UPDATE completed_step_usage SET digest=NULL')
            legacy = ProcessingScheduler(db); legacy.evaluate(task); legacy.finish()
            self.assertEqual((legacy.executed, legacy.reused), (1, 0))

    def test_same_as_revalidates_exact_review_claim_and_evidence_dependencies(self):
        from test_knowledge_contract import KnowledgeContractTests
        KnowledgeContractTests.setUpClass()
        entities=KnowledgeContractTests.entity_type_registry
        types=KnowledgeContractTests.relation_type_registry
        corpus, philosophy=KnowledgeContractTests().fixture()
        graph=build_knowledge_graph(corpus,philosophy,{},entities,types)
        left,right=copy.deepcopy(graph['nodes'][:2])
        left.update(id='left',entity_id='tos.work.left',type_id='tos.entity.work')
        right.update(id='right',entity_id='tos.work.right',type_id='tos.entity.work')
        claim=copy.deepcopy(left); claim.update(id='claim',entity_id='tos.claim.identity',type_id='tos.entity.claim')
        claim['semantics']={'claim':{'claim_id':'tos.claim.identity','claim_version':1,'relation_type_id':'tos.relation.same-as',
            'subject_node_id':'left','object_node_id':'right','subject_entity_id':'tos.work.left',
            'object_entity_id':'tos.work.right','evidence_node_ids':['evidence']}}
        review=copy.deepcopy(left); review.update(id='review',type_id='tos.entity.review')
        review['attributes']={'claim_ref':'tos.claim.identity','claim_version':1,'decision':'accepted'}
        evidence=copy.deepcopy(left); evidence.update(id='evidence',type_id='tos.entity.evidence')
        relation=copy.deepcopy(graph['relations'][0]); relation.update(id='equiv',from_id='left',to_id='right',relation_type_id='tos.relation.same-as')
        relation['attributes']={'claim_ref':'tos.claim.identity','review_node_id':'review'}
        relation['epistemic']['review_posture']='accepted'
        graph={'nodes':[left,right,claim,review,evidence],'relations':[relation]}
        message='same_as relation equiv lacks resolved evidence and exact-version review'
        self.assertNotIn(message,validate_knowledge_semantics(graph,entities,types)['violations'])
        variants=[]
        for field,value in (('claim_version',2),('decision','rejected'),('claim_ref','wrong')):
            changed=copy.deepcopy(graph); changed['nodes'][3]['attributes'][field]=value; variants.append(changed)
        changed=copy.deepcopy(graph); changed['nodes'].pop(); variants.append(changed)
        changed=copy.deepcopy(graph); changed['nodes'][-1]['type_id']='tos.entity.work'; variants.append(changed)
        changed=copy.deepcopy(graph); changed['nodes'][2]['semantics']['claim']['object_entity_id']='wrong'; variants.append(changed)
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/'validation.sqlite'
            for changed in variants:
                with NormalizationCache(path,'v1'): validate_knowledge_semantics(graph,entities,types)
                with NormalizationCache(path,'v1'): actual=validate_knowledge_semantics(changed,entities,types)
                self.assertIn(message,actual['violations'])
                self.assertEqual(actual,validate_knowledge_semantics(changed,entities,types))

    def test_failed_input_scan_does_not_report_unvisited_records_as_removed(self):
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/'steps.sqlite'
            with NormalizationCache(path,'v1') as first:
                first.scheduler.evaluate(Input('one',1)); first.scheduler.evaluate(Input('two',2))
            cache=NormalizationCache(path,'v1')
            with self.assertRaisesRegex(RuntimeError,'interrupted'):
                with cache:
                    cache.scheduler.evaluate(Input('one',3))
                    raise RuntimeError('interrupted')
            self.assertEqual(cache.processing_report['input_coverage'],'partial')
            self.assertEqual(cache.processing_report['input_changes']['removed']['count'],0)
            self.assertEqual(cache.processing_report['removed_task_ids'],[])

    def test_bounded_cache_retention_and_eviction_preserve_projection(self):
        from test_knowledge_contract import KnowledgeContractTests
        corpus, philosophy=KnowledgeContractTests().fixture()
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/'steps.sqlite'
            evicted=0
            for turn in range(12):
                philosophy['nodes'][0]['label']='Version ' + str(turn)
                with NormalizationCache(path,'v1',max_cache_bytes=16000,max_cache_entries=8,keep_runs=2) as cache:
                    actual=build_knowledge_graph(corpus,philosophy)
                self.assertEqual(actual,build_knowledge_graph(corpus,philosophy))
                evicted+=cache.processing_report['cache']['evicted_outputs']
                with closing(sqlite3.connect(path)) as db:
                    size,count=db.execute('SELECT coalesce(sum(length(CAST(payload AS BLOB))),0),count(*) FROM completed_steps').fetchone()
                    self.assertLessEqual(size,16000); self.assertLessEqual(count,8)
                    self.assertLessEqual(db.execute('SELECT count(*) FROM processing_runs').fetchone()[0],3)
                    for table in ('processing_tasks','processing_dependencies'):
                        self.assertEqual(db.execute(f'SELECT count(*) FROM {table} WHERE run_id NOT IN (SELECT id FROM processing_runs)').fetchone()[0],0)
            self.assertGreater(evicted,0)
            with NormalizationCache(path,'v1'):
                with self.assertRaisesRegex(RuntimeError,'another builder'): NormalizationCache(path,'v1')
            with NormalizationCache(path,'v1',max_cache_bytes=1) as tiny:
                self.assertEqual(build_knowledge_graph(corpus,philosophy),actual)
            self.assertGreater(tiny.processing_report['cache']['uncached_oversized_outputs'],0)

    def test_changed_record_finalization_and_validation_match_full_rebuild(self):
        from test_knowledge_contract import KnowledgeContractTests
        KnowledgeContractTests.setUpClass()
        entities=copy.deepcopy(KnowledgeContractTests.entity_type_registry)
        relations=copy.deepcopy(KnowledgeContractTests.relation_type_registry)
        corpus, philosophy=KnowledgeContractTests().fixture()
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/'steps.sqlite'
            def build():
                return build_knowledge_graph(corpus,philosophy,{},entities,relations)
            with NormalizationCache(path,'v1'): baseline=build()
            with NormalizationCache(path,'v1') as repeat: self.assertEqual(build(),baseline)
            self.assertEqual(repeat.misses,0)
            philosophy['nodes'][0]['extra_note']='changed source metadata'
            with NormalizationCache(path,'v1') as changed: actual=build()
            self.assertEqual(actual,build())
            stats=changed.processing_report['steps_by_kind']
            self.assertEqual(stats['final-node']['executed'],1)
            self.assertEqual(stats['validate-node']['executed'],1)
            self.assertGreater(stats['validate-relation']['reused'],0)
            changes=changed.processing_report['input_changes']['changed']
            self.assertIn('source-node:philosophy:a',changes['sample_ids'])
            # Deletions and changing inherited views must remove old state,
            # including a deleted node replaced by an unresolved endpoint.
            for turn in range(8):
                if turn%3==0: philosophy['edges'][0]['view_ids']=['new-view'] if turn%2 else []
                elif turn%3==1: philosophy['nodes'][0]['label']='Label '+str(turn)
                else: philosophy['nodes'][-1]['extra_field']=turn
                with NormalizationCache(path,'v1'): actual=build()
                self.assertEqual(actual,build())
            philosophy['edges'].pop()
            with NormalizationCache(path,'v1') as removed: actual=build()
            self.assertEqual(actual,build())
            self.assertGreater(removed.processing_report['input_changes']['removed']['count'],0)

    def test_incremental_validation_rechecks_negative_and_missing_dependencies(self):
        from test_knowledge_contract import KnowledgeContractTests
        KnowledgeContractTests.setUpClass()
        entities=copy.deepcopy(KnowledgeContractTests.entity_type_registry)
        relation_types=copy.deepcopy(KnowledgeContractTests.relation_type_registry)
        corpus, philosophy=KnowledgeContractTests().fixture()
        baseline=build_knowledge_graph(corpus,philosophy,{},entities,relation_types)
        cases=[copy.deepcopy(baseline)]
        wrong=copy.deepcopy(baseline); wrong['nodes'][0]['type_id']='tos.entity.thing'; cases.append(wrong)
        wrong=copy.deepcopy(baseline); wrong['relations'][0]['to_id']='missing'; cases.append(wrong)
        wrong=copy.deepcopy(baseline); wrong['relations'].append(copy.deepcopy(wrong['relations'][0])); cases.append(wrong)
        wrong=copy.deepcopy(baseline); wrong['nodes'].append(copy.deepcopy(wrong['nodes'][0])); cases.append(wrong)
        wrong=copy.deepcopy(baseline); wrong['relations'][0]['relation_type_id']='tos.relation.same-as'; cases.append(wrong)
        wrong=copy.deepcopy(baseline); wrong['nodes'][0]['type_id']='tos.entity.claim'; wrong['nodes'][0]['semantics']={'claim':[]}; cases.append(wrong)
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/'validation.sqlite'
            for graph in cases+list(reversed(cases)):
                expected=validate_knowledge_semantics(graph,entities,relation_types)
                with NormalizationCache(path,'v1'):
                    actual=validate_knowledge_semantics(graph,entities,relation_types)
                self.assertEqual(actual,expected)
            # A registry change invalidates checks even with identical records.
            entities['types'][1]['abstract']=not entities['types'][1].get('abstract',False)
            with NormalizationCache(path,'v1'):
                actual=validate_knowledge_semantics(baseline,entities,relation_types)
            self.assertEqual(actual,validate_knowledge_semantics(baseline,entities,relation_types))

    def test_failure_resume_and_unchanged_intermediate_output_prune_work(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'steps.sqlite'
            executed = []
            def pipeline(extra, fail=False):
                source = Input('source:a', {'label':'Alpha','extra':extra})
                node = Task('node:a','v1',(source,),None,lambda v: executed.append('node') or v[0])
                title = Task('title:a','v1',(node,),None,lambda v: executed.append('title') or v[0]['label'])
                def relation(v):
                    executed.append('relation')
                    if fail: raise RuntimeError('interrupted pure step')
                    return {'statement':v[0] + ' relates Beta'}
                return Task('relation:a-b','v1',(title,),None,relation)
            db=sqlite3.connect(path)
            initial=ProcessingScheduler(db)
            with self.assertRaises(RuntimeError):
                try: initial.evaluate(pipeline(1,True))
                except Exception as error: initial.finish(error); raise
            self.assertIsNone(db.execute('SELECT * FROM processing_publication').fetchone())
            db.close()
            db=sqlite3.connect(path)
            resumed=ProcessingScheduler(db);resumed.evaluate(pipeline(1));resumed.finish()
            self.assertEqual((resumed.executed,resumed.reused),(1,2))
            before=list(executed)
            changed=ProcessingScheduler(db);changed.evaluate(pipeline(2));changed.finish()
            self.assertEqual(executed[len(before):],['node','title'])
            self.assertEqual((changed.executed,changed.reused),(2,1))
            db.close()

    def test_cycles_conflicting_identity_and_competing_publication_fail_closed(self):
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/'steps.sqlite'
            db=sqlite3.connect(path)
            cycle=Task('cycle','v1',(),None,lambda _:None)
            object.__setattr__(cycle,'dependencies',(cycle,))
            scheduler=ProcessingScheduler(db)
            with self.assertRaisesRegex(ValueError,'cyclic'):scheduler.evaluate(cycle)
            scheduler.evaluate(Input('source',1))
            with self.assertRaisesRegex(ValueError,'conflicting'):scheduler.evaluate(Input('source',2))
            # Catching a bad dependency must not make the run publishable.
            with self.assertRaisesRegex(RuntimeError,'failed tasks'):scheduler.finish()
            self.assertIsNone(db.execute('SELECT * FROM processing_publication').fetchone())
            self.assertEqual(scheduler.report()['status'],'failed')
            db2=sqlite3.connect(path)
            first=ProcessingScheduler(db);second=ProcessingScheduler(db2)
            first.evaluate(Task('one','v1',(),None,lambda _:1));first.finish()
            second.evaluate(Task('two','v1',(),None,lambda _:2))
            with self.assertRaisesRegex(RuntimeError,'baseline changed'):second.finish()
            self.assertEqual(db.execute('SELECT run_id FROM processing_publication').fetchone()[0],first.run_id)
            db2.close();db.close()

    def test_actual_normalization_dependencies_and_removals(self):
        from test_knowledge_contract import KnowledgeContractTests
        corpus, philosophy=KnowledgeContractTests().fixture()
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/'steps.sqlite'
            with NormalizationCache(path,'v1') as first: initial=build_knowledge_graph(corpus,philosophy)
            with NormalizationCache(path,'v1') as repeat: self.assertEqual(build_knowledge_graph(corpus,philosophy),initial)
            self.assertEqual(repeat.misses,0)
            philosophy['nodes'][0]['unrelated_note']='new metadata, unchanged label'
            with NormalizationCache(path,'v1') as changed: output=build_knowledge_graph(corpus,philosophy)
            self.assertEqual(output,build_knowledge_graph(corpus,philosophy))
            db=sqlite3.connect(path)
            statuses=db.execute("SELECT id,cache_key FROM processing_tasks WHERE run_id=? AND kind='task'",(changed.scheduler.run_id,)).fetchall()
            previous=dict(db.execute("SELECT id,cache_key FROM processing_tasks WHERE run_id=? AND kind='task'",(repeat.scheduler.run_id,)))
            changed_ids=[id for id,key in statuses if previous.get(id)!=key]
            self.assertTrue(any(id.startswith('node:') for id in changed_ids))
            self.assertFalse(any(id.startswith('relation:') for id in changed_ids),changed_ids)
            title_edges=db.execute("SELECT count(*) FROM processing_dependencies WHERE run_id=? AND dependency_id LIKE 'endpoint-title:%'",(changed.scheduler.run_id,)).fetchone()[0]
            self.assertGreater(title_edges,0)
            db.close()
            with NormalizationCache(path,'v1') as smaller: build_knowledge_graph({}, {})
            self.assertTrue(smaller.processing_report['removed_task_ids'])
            with NormalizationCache(path,'v1') as unused: pass
            self.assertEqual(unused.processing_report['status'],'skipped')
            db=sqlite3.connect(path)
            self.assertEqual(db.execute('SELECT run_id FROM processing_publication').fetchone()[0],smaller.scheduler.run_id)
            db.close()

    def test_processor_digest_tracks_normalizers_and_helpers_not_lens_queries(self):
        # Source fragments are sufficient: hashing does not execute code.
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/'processor.py'
            initial="LIMIT=1\ndef helper(x): return x+LIMIT\ndef _normalize_node(x): return helper(x)\ndef _normalize_relation(x): return helper(x)\ndef query(x): return x\n"
            path.write_text(initial)
            before=normalization_processor_digest(path)
            path.write_text(initial.replace('return x\n','return x+99\n'))
            self.assertEqual(normalization_processor_digest(path),before)
            path.write_text(initial.replace('LIMIT=1','LIMIT=2'))
            self.assertNotEqual(normalization_processor_digest(path),before)


if __name__=='__main__': unittest.main()
