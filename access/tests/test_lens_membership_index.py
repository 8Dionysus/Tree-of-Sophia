"""Indexed positive list predicates retain native semantics and atomic growth."""
import copy
from contextlib import closing
import os
from pathlib import Path
import sqlite3
import tempfile
import unittest

from tos_access import knowledge as k
from tos_access.lens_membership_index import prepare_membership_index_transaction, compile_plan
from tos_access.compact_lens_store import prepare_compact_lens_store_transaction
from tos_access.prepared_publication import publish_prepared, apply_prepared_delta_transaction, PreparedChange
from tos_access.published_lens import PublishedLensService, PublishedLensLimits
from tos_access.published_read_model import PublishedKnowledgeReadModel, PublishedReadBudgetExceeded, PublishedReadModelError
from test_prepared_publication import fixture
from test_indexed_lens import lens


class LensMembershipIndexTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='lens-memberships-', dir=os.environ.get('TMPDIR'))
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / 'snapshot.sqlite'
        self.graph, self.catalog = fixture()
        seed = self.graph['nodes'][0]
        self.graph['nodes'] = []
        for n in range(100):
            node = copy.deepcopy(seed)
            node.update(id=f'n{n:03}', entity_id=f'e{n}', native_id=f'native{n}',
                        source_graph='repository' if n%10 == 0 else 'philosophy',
                        view_ids=['common'] + (['Свобода'] if n%2 else ['other']),
                        graph_layers=['alpha'] + (['beta'] if n%3 else []))
            self.graph['nodes'].append(node)
        relation = self.graph['relations'][0]
        self.graph['relations'] = []
        for n in range(99):
            item = copy.deepcopy(relation)
            item.update(id=f'r{n:03}', from_id=f'n{n:03}', to_id=f'n{n+1:03}',
                        view_ids=['common','Свобода'], graph_layers=['beta'])
            self.graph['relations'].append(item)
        self.binding = publish_prepared(self.path, graph=self.graph, catalog=self.catalog)
        self.reader = PublishedKnowledgeReadModel(self.path, self.binding)

    def spec(self, filters=None, mode='all', **extras):
        query = {'filters':filters or [{'field':'view_ids','op':'contains','value':'common'}], 'match':mode}
        return lens(detail='compact', node_query=query,
                    relation_query={'filters':[{'field':'view_ids','op':'contains','value':'common'}]},
                    limits={'nodes':100,'relations':100,'groups':8}, **extras)

    def install(self, db):
        db.execute('BEGIN IMMEDIATE')
        prepare_compact_lens_store_transaction(db, expected_binding=self.binding)
        report = prepare_membership_index_transaction(db, expected_binding=self.binding)
        db.commit()
        return report

    def test_broad_exact_selection_counts_order_and_large_endpoint_basis(self):
        with self.assertRaises(PublishedReadBudgetExceeded):
            PublishedLensService(self.reader, limits=PublishedLensLimits(max_candidates=3)).execute(self.spec())
        with closing(sqlite3.connect(self.path)) as db:
            before = list(db.execute('SELECT * FROM edge_meta ORDER BY key,part'))
            self.assertEqual(self.install(db)['rows'],199)
            self.assertEqual(list(db.execute('SELECT * FROM edge_meta ORDER BY key,part')), before)
        result = PublishedLensService(self.reader, limits=PublishedLensLimits(max_candidates=3)).execute(self.spec())
        self.assertEqual(result, k.execute_knowledge_lens(self.graph,self.spec()))
        self.assertEqual(len(result['nodes']),100)
        self.assertEqual(len(result['relations']),99)

    def test_boolean_membership_and_native_case_sensitive_list_semantics(self):
        with closing(sqlite3.connect(self.path)) as db:self.install(db)
        cases = [
            ('all',[{'field':'view_ids','op':'contains','value':['common','Свобода']}]),
            ('all',[{'field':'view_ids','op':'eq','value':'Свобода'}, {'field':'graph_layers','op':'contains','value':'beta'}]),
            ('any',[{'field':'view_ids','op':'in','value':['Свобода','other']}, {'field':'graph_layers','op':'eq','value':'alpha'}]),
            ('any',[{'field':'view_ids','op':'contains','value':['other','Свобода']}, {'field':'graph_layers','op':'eq','value':'beta'}]),
            ('all',[{'field':'view_ids','op':'contains','value':'свобода'}]),
        ]
        for mode, filters in cases:
            with self.subTest(mode=mode,filters=filters):
                spec=self.spec(filters,mode)
                self.assertEqual(PublishedLensService(self.reader,limits=PublishedLensLimits(max_candidates=3)).execute(spec),
                                 k.execute_knowledge_lens(self.graph,spec))
        scoped = {**self.spec(), 'sources':['philosophy']}
        self.assertEqual(PublishedLensService(self.reader,limits=PublishedLensLimits(max_candidates=3)).execute(scoped),
                         k.execute_knowledge_lens(self.graph,scoped))
        for op, value in [('contains',[]),('eq',['common']),('neq','common'),('exists',False),('in',[2])]:
            group={'enabled':True,'match':'all','filters':[{'field':'view_ids','op':op,'value':value}]}
            self.assertIsNone(compile_plan('node',group))
        self.assertIsNone(compile_plan('node',{'enabled':True,'match':'any','filters':[
            {'field':'view_ids','op':'in','value':[str(n) for n in range(1000)]}]}))

    def test_membership_changes_rollback_and_successor_are_addressed(self):
        with closing(sqlite3.connect(self.path)) as db:
            self.install(db)
            before=list(db.iterdump())
            changed=copy.deepcopy(self.graph['nodes'][0]);changed['view_ids']=['replacement']
            inserted=copy.deepcopy(changed);inserted.update(id='new',entity_id='new',native_id='new')
            header={key:value for key,value in self.graph.items() if key not in ('nodes','relations')}
            header={**header,'source_revision':'c'*64}
            changes=[PreparedChange('update','node','n000',changed),PreparedChange('insert','node','new',inserted,100),
                     PreparedChange('delete','relation','r000')]
            def apply():return apply_prepared_delta_transaction(db,expected_binding=self.binding,source_header=header,
                catalog={**self.catalog,'source_revision':header['source_revision']},changes=changes)
            db.execute('BEGIN IMMEDIATE');apply();db.rollback()
            self.assertEqual(list(db.iterdump()),before)
            db.execute('BEGIN IMMEDIATE');binding=apply();db.commit()
            self.assertEqual(db.execute("SELECT count(*) FROM knowledge_lens_memberships WHERE kind='relation' AND id='r000'").fetchone()[0],0)
            graph={**self.graph,'source_revision':'c'*64,'nodes':[changed,*self.graph['nodes'][1:],inserted],
                   'relations':self.graph['relations'][1:]}
            spec=self.spec([{'field':'view_ids','op':'eq','value':'replacement'}])
            reader=PublishedKnowledgeReadModel(self.path,binding)
            self.assertEqual(PublishedLensService(reader).execute(spec),k.execute_knowledge_lens(graph,spec))

    def test_partial_install_and_stale_index_refuse(self):
        with closing(sqlite3.connect(self.path)) as db:
            before=list(db.iterdump())
            db.execute('BEGIN IMMEDIATE')
            with self.assertRaisesRegex(PublishedReadModelError,'source budget'):
                prepare_membership_index_transaction(db,expected_binding=self.binding,max_rows=1)
            db.rollback();self.assertEqual(list(db.iterdump()),before)
            self.install(db)
            db.execute("DELETE FROM knowledge_lens_memberships WHERE id='n000'")
            db.commit()
            with self.assertRaisesRegex(PublishedReadModelError,'stale'):
                PublishedLensService(self.reader).execute(self.spec())
