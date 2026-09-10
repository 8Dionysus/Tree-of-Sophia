from __future__ import annotations

import copy
import hashlib
import io
import json
import os
import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from contextlib import closing

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'src'))
from tos_access import knowledge as k
from tos_access.core import ToSAccessCore
from tos_access.doctor import doctor_report
from tos_access.exploration import ExplorationService, EXECUTION_VERSION
from tos_access.projection_store import ProjectionReader
from tos_access.query_store import COMPILER_VERSION, QueryStore, QueryStoreRequired
from test_access_contract import write_fixture


def write_store(path, graph, *, corpus=None, bindings=None):
    """Independent ABI fixture: the query tests do not call the production builder."""
    corpus = corpus or {}
    metadata = {'schema': 'tos_query_store_v1', 'complete': True,
                'compiler_version': COMPILER_VERSION,
                'snapshot_bindings': bindings or {},
                'graph_header': {key: value for key, value in graph.items() if key not in ('nodes','relations')},
                'corpus_header': {key: value for key, value in corpus.items() if key not in ('nodes','resources','manifests','branches','relation_edges','relation_packs','graph_views','source_navigation')},
                'source_navigation_header': {key: value for key, value in corpus.get('source_navigation',{}).items() if key not in ('nodes','edges','rights')},
                'catalog': {'schema': 'fixture-only'},
                'exploration_revision': k._stable_digest({'execution_version': EXECUTION_VERSION, 'source_revision': graph.get('source_revision'),
                    'nodes': sorted([n['id'], n['content_revision']] for n in graph['nodes']),
                    'relations': sorted([r['id'], r['content_revision']] for r in graph['relations'])})}
    with closing(sqlite3.connect(path)) as db:
        db.executescript('''CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT);
        CREATE TABLE knowledge_nodes(id TEXT PRIMARY KEY,native_id TEXT,entity_id TEXT,source_graph TEXT,kind_id TEXT,type_id TEXT,label TEXT,search_text TEXT,payload TEXT);
        CREATE TABLE knowledge_relations(id TEXT PRIMARY KEY,native_id TEXT,from_id TEXT,to_id TEXT,source_graph TEXT,predicate_id TEXT,relation_type_id TEXT,label TEXT,search_text TEXT,payload TEXT);
        CREATE INDEX nodes_entity ON knowledge_nodes(entity_id);
        CREATE INDEX nodes_native ON knowledge_nodes(native_id);
        CREATE INDEX relation_from ON knowledge_relations(from_id,id);
        CREATE INDEX relation_to ON knowledge_relations(to_id,id);
        CREATE TABLE source_nodes(id TEXT PRIMARY KEY,kind_id TEXT,packet_id TEXT,payload TEXT);
        CREATE TABLE source_edges(id TEXT PRIMARY KEY,from_id TEXT,to_id TEXT,edge_kind TEXT,predicate_id TEXT,payload TEXT);
        CREATE TABLE source_rights(id TEXT PRIMARY KEY,payload TEXT);
        CREATE TABLE source_rights_scopes(right_id TEXT,scope_id TEXT);
        CREATE TABLE raw_records(collection TEXT,key TEXT,position INTEGER,payload TEXT,PRIMARY KEY(collection,key));''')
        db.executemany('INSERT INTO metadata VALUES (?,?)', [(key,json.dumps(value)) for key,value in metadata.items()])
        for table,items,fields,title in [('knowledge_nodes',graph['nodes'],('id','native_id','entity_id','source_graph','kind_id','type_id'),'title'),
                                         ('knowledge_relations',graph['relations'],('id','native_id','from_id','to_id','source_graph','predicate_id','relation_type_id'),'label')]:
            for item in items:
                values = [item.get(field) for field in fields] + [item.get('display',{}).get(title,{}).get('default',''), k._searchable(item), json.dumps(item)]
                db.execute(f'INSERT INTO {table} VALUES ({",".join("?" for _ in values)})', values)
        for name,items in corpus.items():
            if isinstance(items,list):
                db.executemany('INSERT INTO raw_records VALUES (?,?,?,?)', [('corpus/'+name,str(i),i,json.dumps(item)) for i,item in enumerate(items)])
        nav=corpus.get('source_navigation',{})
        for node in nav.get('nodes',[]):
            db.execute('INSERT INTO source_nodes VALUES (?,?,?,?)',(node['node_id'],node.get('node_kind'),node.get('properties',{}).get('packet_id'),json.dumps(node)))
        for edge in nav.get('edges',[]):
            db.execute('INSERT INTO source_edges VALUES (?,?,?,?,?,?)',(edge['edge_id'],edge['from_id'],edge['to_id'],edge.get('edge_kind'),edge.get('predicate_id'),json.dumps(edge)))
        for right in nav.get('rights',[]):
            db.execute('INSERT INTO source_rights VALUES (?,?)',(right['rights_id'],json.dumps(right)))
            db.executemany('INSERT INTO source_rights_scopes VALUES (?,?)',[(right['rights_id'],scope) for scope in right.get('scope_refs',[])])
        db.commit()


class QueryStoreTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        write_fixture(self.root)
        self.core = ToSAccessCore.discover(self.root)
        self.graph = copy.deepcopy(self.core.knowledge_graph())
        self.corpus = self.core.index()
        self.path = self.root/'queries.sqlite3'
        inputs = [(name,getattr(self.core, attr)) for name,attr in [
            ('ToS/derived-exports/tos_corpus_index.min.json','index_path'),
            ('ToS/derived-exports/philosophy_graph_projection.min.json','philosophy_graph_projection_path'),
            ('ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json','bibliographic_graph_path'),
            ('ToS/doctrine/semantic-interchange/entity-types.v1.json','entity_type_registry_path'),
            ('ToS/doctrine/semantic-interchange/relation-types.v1.json','relation_type_registry_path')]]
        self.bindings = {name: hashlib.sha256(path.read_bytes()).hexdigest() for name,path in inputs}
        write_store(self.path,self.graph,corpus=self.corpus,bindings=self.bindings)
        self.store = QueryStore(self.path,snapshot_bindings=self.bindings)

    def test_search_and_inspect_preserve_packets(self):
        for query in ('','Alpha','tos','FIXTURE','source_refs','á'):
            for offset in (0,1,100):
                self.assertEqual(self.store.search(query,offset=offset,limit=2),k.search_knowledge_graph(self.graph,query,offset=offset,limit=2))
        for node in self.graph['nodes']:
            for identifier in (node['id'],node['native_id'],node['entity_id']):
                self.assertEqual(self.store.inspect_node(identifier,2),k.inspect_knowledge_node(self.graph,identifier,2))
        for relation in self.graph['relations']:
            self.assertEqual(self.store.inspect_relation(relation['id']),k.inspect_knowledge_relation(self.graph,relation['id']))

    def test_lens_grammar_and_focus_packet_parity(self):
        base={'schema_version':'tos_lens_spec_v1','lens_id':'fixture-query','limits':{'nodes':4,'relations':5,'groups':3},'explain':True}
        specs=[base,
               {**base,'composition':{'endpoint_policy':'independent','sort_nodes':[{'field':'display.title.default','direction':'desc'}]}},
               {**base,'seed':{'text_query':'fixture'}},
               {**base,'node_query':{'filters':[{'field':'kind_id','op':'in','value':['work','item']}]},'traversal':{'depth':2}},
               {**base,'path_query':[{'path_id':'incoming','quantifier':'not_exists','steps':[{'direction':'incoming'}]}]},
               {**base,'pagination':{'nodes':2,'relations':2}},
               {**base,'relation_query':{'enabled':False}}]
        for spec in specs:
            with self.subTest(spec=spec):
                self.assertEqual(self.store.execute_lens(spec),k.execute_knowledge_lens(self.graph,spec))
        for depth in (0,1,2):
            for profile in ('all','overview'):
                for direction in ('incoming','outgoing','either'):
                    node=self.graph['nodes'][0]['id']
                    self.assertEqual(self.store.focus(node,depth=depth,profile=profile,direction=direction,node_limit=5,relation_limit=4),
                                     k.focus_knowledge_node(self.graph,node,depth=depth,profile=profile,direction=direction,node_limit=5,relation_limit=4))

    def test_exploration_pages_equal_without_loading_graph(self):
        fail=lambda: (_ for _ in ()).throw(AssertionError('full graph requested'))
        indexed=ExplorationService(fail,query_store_provider=lambda:self.store,work_limit=4)
        legacy=ExplorationService(lambda:self.graph,work_limit=4)
        query={'focus_node_id':self.graph['nodes'][0]['id'],'max_depth':3,'page_nodes':2,'page_relations':2}
        left,right=indexed.explore(query),legacy.explore(query)
        while True:
            lc,rc=left['page']['next_cursor'],right['page']['next_cursor']
            left['page']['next_cursor']=right['page']['next_cursor']=None
            self.assertEqual(left,right)
            if lc is None:
                break
            left,right=indexed.explore({'cursor':lc}),legacy.explore({'cursor':rc})

    def test_core_routes_and_source_dossier_do_not_load_graph_or_index(self):
        methods=[('knowledge_search',('fixture',)),('knowledge_node',(self.graph['nodes'][0]['id'],)),
                 ('knowledge_focus',(self.graph['nodes'][0]['id'],)),('source_descend',('tos.work.fixture',)),
                 ('source_dossier',('tos.work.fixture',)),('source_dossier',('tos.link.fixture.download',)),
                 ('search',('fixture',)),('resources',()),('node',('a',)),('graph_view',('corpus-topology',)),('summary',())]
        expected=[getattr(self.core,name)(*args) for name,args in methods]
        with patch.dict(os.environ,{'TOS_QUERY_STORE_PATH':str(self.path)}),patch.object(ToSAccessCore,'knowledge_graph',side_effect=AssertionError('full graph')),patch.object(ToSAccessCore,'index',side_effect=AssertionError('full corpus')):
            for (name,args),packet in zip(methods,expected):
                with self.subTest(method=name):
                    self.assertEqual(getattr(self.core,name)(*args),packet)

    def test_identity_ambiguity_carriers_and_provenance_boundaries(self):
        graph = copy.deepcopy(self.graph)
        original = graph['nodes'][0]
        original['entity_id'] = 'tos.subject.synthetic.shared'
        alias = {**copy.deepcopy(original),'id':'philosophy:alias','source_graph':'philosophy'}
        ambiguous = {**copy.deepcopy(original),'id':'philosophy:other','entity_id':'tos.subject.synthetic.other','source_graph':'philosophy'}
        graph['nodes'].extend([alias,ambiguous])
        graph['nodes'].sort(key=lambda n:n['id'])
        edge = {**copy.deepcopy(graph['relations'][0]),'id':'provenance:shared-maker',
                'from_id':alias['id'],'to_id':ambiguous['id'],'relation_type_id':'tos.relation.made-by'}
        graph['relations'].append(edge)
        graph['relations'].sort(key=lambda r:r['id'])
        path=self.root/'identity.sqlite3'
        write_store(path,graph)
        store=QueryStore(path)
        self.assertTrue(store.inspect_node(original['native_id'])['ambiguous_native_id'])
        with self.assertRaisesRegex(ValueError,'ambiguous'):
            store.focus(original['native_id'])
        for profile in ('overview','all'):
            for cap in (1,2,5):
                with self.subTest(profile=profile,cap=cap):
                    expected=k.focus_knowledge_node(graph,original['entity_id'],profile=profile,node_limit=cap)
                    result=store.focus(original['entity_id'],profile=profile,node_limit=cap)
                    self.assertEqual(result,expected)
                    if profile=='overview':
                        self.assertNotIn(edge['id'],[r['id'] for r in result['relations']])
        before={p.name for p in self.root.iterdir()}
        store.search('')
        store.inspect_node(original['id'])
        self.assertEqual(before,{p.name for p in self.root.iterdir()})
        with self.assertRaisesRegex(QueryStoreRequired,'readonly'):
            with store.connect() as db:
                db.execute("DELETE FROM knowledge_nodes")

    def test_explicit_compiler_snapshot_is_served_without_normalization(self):
        from tos_access.knowledge_compile import compile_knowledge_store
        output = self.root/'compiled.sqlite3'
        compile_knowledge_store(self.root,output,allow_legacy=True)
        store = QueryStore(output,snapshot_bindings=self.bindings)
        expected = {**self.graph,'source_revision':store.header['source_revision']}
        dossier = self.core.source_dossier('tos.work.fixture')
        legacy_exploration = ExplorationService(lambda: expected)
        legacy_exploration._index(expected)
        self.assertEqual(store.revision, legacy_exploration.revision)
        with patch.object(k,'build_knowledge_graph',side_effect=AssertionError('implicit normalization')):
            self.assertEqual(store.search('fixture'),k.search_knowledge_graph(expected,'fixture'))
            self.assertEqual(store.execute_lens({'schema_version':'tos_lens_spec_v1','lens_id':'fixture'}),
                             k.execute_knowledge_lens(expected,{'schema_version':'tos_lens_spec_v1','lens_id':'fixture'}))
            with patch.dict(os.environ,{'TOS_QUERY_STORE_PATH':str(output)}),patch.object(ToSAccessCore,'index',side_effect=AssertionError('full corpus')):
                self.assertEqual(self.core.source_dossier('tos.work.fixture'),dossier)

    def test_compiled_search_keeps_literal_unicode_and_json_substring_semantics(self):
        from tos_access.knowledge_compile import compile_knowledge_store
        corpus = copy.deepcopy(self.corpus)
        corpus['nodes'][0]['label'] = 'ÄΩ Straße Καλημέρα русский 漢字甲 %_ "quotes" / slash 🙂漢字 e\u0301Ω İX foo   bar OR AND NEAR'
        self.core.index_path.write_text(json.dumps(corpus,ensure_ascii=False))
        graph = self.core.knowledge_graph()
        output=self.root/'unicode.sqlite3'
        compile_knowledge_store(self.root,output,allow_legacy=True)
        store=QueryStore(output)
        self.assertEqual(store.metadata['search_accelerator']['mode'],'fts5-trigram')
        graph={**graph,'source_revision':store.header['source_revision']}
        for query in ('漢字甲','Straße','STRASSE','Καλημέρα','русский','%_','%_ ', '"quotes"','source_refs','a\x00b','\n','a','', '🙂漢字','e\u0301Ω','İX','o   b','OR AND','NEAR',r'\"q'):
            with self.subTest(query=query):
                self.assertEqual(store.search(query,limit=2),k.search_knowledge_graph(graph,query,limit=2))
                spec={'schema_version':'tos_lens_spec_v1','lens_id':'literal','seed':{'text_query':query}}
                self.assertEqual(store.execute_lens(spec),k.execute_knowledge_lens(graph,spec))

    def test_http_and_native_mcp_use_completed_store_and_report_build_required(self):
        import asyncio
        import http.client
        import threading
        from tos_access.http_server import make_server
        from tos_access.mcp_server import build_server
        from tos_access.knowledge_compile import compile_knowledge_store
        output=self.root/'adapter.sqlite3'
        compile_knowledge_store(self.root,output,allow_legacy=True)
        with patch.dict(os.environ,{'TOS_QUERY_STORE_PATH':str(output)}),patch.object(ToSAccessCore,'knowledge_graph',side_effect=AssertionError('full graph')):
            server=make_server(self.core,port=0)
            thread=threading.Thread(target=server.serve_forever,daemon=True)
            thread.start()
            try:
                connection=http.client.HTTPConnection('127.0.0.1',server.server_port)
                connection.request('GET','/health')
                result=connection.getresponse()
                self.assertEqual(result.status,200,json.loads(result.read()))
                connection.close()
                mcp=build_server(self.root)
                async def check_mcp():
                    first=await mcp._tool_manager.call_tool('tos_knowledge_explore',{'request':{'focus_node_id':'tos.work.fixture','page_nodes':1}})
                    follow=await mcp._tool_manager.call_tool('tos_knowledge_explore',{'request':{'cursor':first['page']['next_cursor']}})
                    self.assertEqual(follow['page']['number'],2)
                asyncio.run(check_mcp())
                with patch.dict(os.environ,{'TOS_QUERY_STORE_PATH':str(self.root/'missing.sqlite3')}):
                    for method,path,body in [('GET','/api/knowledge/search',None),
                                             ('POST','/api/knowledge/explore',json.dumps({'focus_node_id':'tos.work.fixture'}))]:
                        connection=http.client.HTTPConnection('127.0.0.1',server.server_port)
                        connection.request(method,path,body=body,headers={'Content-Type':'application/json'})
                        result=connection.getresponse()
                        self.assertEqual(result.status,503)
                        self.assertEqual(json.loads(result.read())['code'],'query_store_build_required')
                        connection.close()
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=5)

    def test_doctor_reports_missing_partitioned_store_without_materializing_corpus(self):
        from scripts.partitioned_projection_common import write_partitioned_payload

        for relative in (
            'ToS/derived-exports/tos_corpus_index.min.json',
            'ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json',
        ):
            path = self.root / relative
            payload = json.loads(path.read_text(encoding='utf-8'))
            if payload.get('schema_version') == 'tos_source_witness_bibliographic_graph_v1':
                payload['input_digests'] = {}
            write_partitioned_payload(path, payload)

        with patch.object(ProjectionReader, 'materialize', side_effect=AssertionError('doctor materialized corpus')):
            report = doctor_report(tos_root=self.root)

        self.assertFalse(report['ok'])
        self.assertIn('query-store', report['required_failures'])
        self.assertIn('build required', next(item['error'] for item in report['checks'] if item['check_id'] == 'query-store'))
        self.assertTrue(next(item for item in report['checks'] if item['check_id'] == 'corpus-index-schema')['ok'])

    def test_cli_reports_missing_partitioned_store_without_materializing_corpus(self):
        from scripts.partitioned_projection_common import write_partitioned_payload
        from tos_access.cli import main

        for relative in (
            'ToS/derived-exports/tos_corpus_index.min.json',
            'ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json',
        ):
            path = self.root / relative
            payload = json.loads(path.read_text(encoding='utf-8'))
            if payload.get('schema_version') == 'tos_source_witness_bibliographic_graph_v1':
                payload['input_digests'] = {}
            write_partitioned_payload(path, payload)

        commands = (
            ('knowledge', 'catalog'),
            ('knowledge', 'search', 'fixture'),
            ('knowledge', 'node', 'tos.work.fixture'),
            ('knowledge', 'relation', 'missing'),
            ('knowledge', 'focus', 'tos.work.fixture'),
            ('lens', 'open', 'missing'),
            ('lens', 'compile', '-'),
        )
        with (
            patch.object(ProjectionReader, 'materialize', side_effect=AssertionError('CLI materialized corpus')),
            patch.object(ToSAccessCore, 'index', side_effect=AssertionError('CLI loaded index')),
            patch.object(ToSAccessCore, 'knowledge_graph', side_effect=AssertionError('CLI loaded graph')),
            patch('sys.stdin', io.StringIO('{}')),
        ):
            for command in commands:
                with self.subTest(command=command), self.assertRaisesRegex(SystemExit, 'query store build required'):
                    main(['--root', str(self.root), *command])

    def test_missing_incomplete_stale_and_replaced_stores_fail_closed(self):
        absent=self.root/'absent.sqlite3'
        with self.assertRaises(QueryStoreRequired): QueryStore(absent)
        self.assertFalse(absent.exists())
        with self.assertRaisesRegex(QueryStoreRequired,'stale'): QueryStore(self.path,snapshot_bindings={})
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE metadata SET value='false' WHERE key='complete'")
            db.commit()
        with self.assertRaisesRegex(QueryStoreRequired,'incomplete'): QueryStore(self.path)
        with self.assertRaisesRegex(QueryStoreRequired,'changed'): self.store.search('')

    def test_connect_rejects_atomic_replacement_during_sqlite_open(self):
        replacement = self.root / 'replacement.sqlite3'
        with closing(sqlite3.connect(replacement)) as db:
            db.execute('CREATE TABLE replacement_marker(value TEXT)')
            db.commit()

        original_connect = sqlite3.connect
        replaced = False

        def replace_before_open(*args, **kwargs):
            nonlocal replaced
            if not replaced:
                os.replace(replacement, self.path)
                replaced = True
            return original_connect(*args, **kwargs)

        with patch('tos_access.query_store.sqlite3.connect', side_effect=replace_before_open):
            with self.assertRaisesRegex(QueryStoreRequired, 'snapshot changed'):
                with self.store.connect():
                    self.fail('a replaced snapshot must never be yielded')
        self.assertTrue(replaced)

    def test_unknown_compiler_version_fails_closed(self):
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE metadata SET value='\"old-semantic-compiler\"' WHERE key='compiler_version'")
            db.commit()
        with self.assertRaisesRegex(QueryStoreRequired, 'unsupported or incomplete'):
            QueryStore(self.path)


if __name__ == '__main__':
    unittest.main()
