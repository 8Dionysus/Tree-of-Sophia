"""Selected raw edges survive Claim compaction in both scene implementations."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import shutil
import subprocess
import sys
import unittest

ACCESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ACCESS / 'src'))
from tos_access.knowledge import knowledge_scene


def claim_graph():
    """Small explicit topology, not corpus data or semantic admission."""
    nodes = [{'id': id, 'entity_id': 'tos.fixture.' + id,
              'content_revision': 'a' * 64, 'semantics': {}, 'epistemic': {}}
             for id in ('subject', 'claim', 'object', 'support', 'member')]
    nodes[1].update(type_id='tos.entity.claim', semantics={'claim': {
        'subject_node_id': 'subject', 'object_node_id': 'object',
        'relation_type_id': 'tos.relation.fixture', 'predicate_mapping_status': 'mapped',
        'value_member_node_ids': ['member']}})
    relations = [{'id': id, 'from_id': 'claim', 'to_id': target,
                  'relation_type_id': type, 'content_revision': 'b' * 64}
                 for id, target, type in (
                     ('subject-leg', 'subject', 'tos.relation.has-subject'),
                     ('object-leg', 'object', 'tos.relation.has-object'),
                     ('support-edge', 'support', 'tos.relation.claim-supported-by'),
                     ('member-edge', 'member', 'tos.relation.claim-value-member'))]
    return {'nodes': nodes, 'relations': relations}


class KnowledgeSceneFocusTests(unittest.TestCase):
    def test_selected_claim_relation_wins_over_claim_node_focus(self):
        graph = claim_graph()
        before = copy.deepcopy(graph)
        for relation in graph['relations']:
            with self.subTest(relation=relation['id']):
                scene = knowledge_scene(**graph, focus_node_id='claim', focus_relation_id=relation['id'])
                compact = scene['compact']
                self.assertIn(relation['id'], compact['relation_ids'])
                self.assertEqual(compact['claim_paths'], [])
                self.assertIn({'node_id': 'claim', 'reason': 'focus-relation'}, compact['retained_claims'])
                self.assertEqual(set(compact['relation_ids']), {row['id'] for row in graph['relations']})
                self.assertEqual(set(compact['vertex_ids']), {row['id'] for row in scene['vertices']})
        self.assertEqual(graph, before)

    def test_ordinary_claim_focus_keeps_vertex_and_complete_context_path(self):
        graph = claim_graph()
        for focus in (None, 'subject', 'claim'):
            with self.subTest(focus=focus):
                scene = knowledge_scene(**graph, focus_node_id=focus)
                compact = scene['compact']
                self.assertEqual(len(compact['claim_paths']), 1)
                path = compact['claim_paths'][0]
                self.assertEqual(path['claim_node_id'], 'claim')
                self.assertEqual(set(path['reading']['relation_context_ids']), {row['id'] for row in graph['relations']})
                self.assertEqual('tos-scene:entity:tos.fixture.claim' in compact['vertex_ids'], focus == 'claim')
                self.assertFalse(path['reading']['standalone'])

    @unittest.skipUnless(shutil.which('node'), 'Node is required for shared scene parity')
    def test_python_shared_exact_scene_parity(self):
        graph = claim_graph()
        cases = [{'graph': graph, 'focus': focus, 'relation': relation}
                 for focus, relation in [(None, None), ('subject', None), ('claim', None),
                                         *[('claim', row['id']) for row in graph['relations']],
                                         ('support', None), ('member', None)]]
        shared = ACCESS / 'shared/knowledge-scene.ts'
        script = ("import fs from 'node:fs';import {knowledgeScene} from " + json.dumps(shared.as_uri())
                  + ";const cases=JSON.parse(fs.readFileSync(0,'utf8'));"
                  + "process.stdout.write(JSON.stringify(cases.map(c=>knowledgeScene(c.graph.nodes,c.graph.relations,c.focus,c.relation))));")
        completed = subprocess.run(['node', '--experimental-strip-types', '--input-type=module', '-e', script],
                                   input=json.dumps(cases), text=True, capture_output=True, check=True, timeout=15)
        actual = json.loads(completed.stdout)
        self.assertEqual(len(actual), len(cases))
        for case, scene in zip(cases, actual, strict=True):
            with self.subTest(focus=case['focus'], relation=case['relation']):
                self.assertEqual(scene, knowledge_scene(**case['graph'], focus_node_id=case['focus'],
                                                       focus_relation_id=case['relation']))


if __name__ == '__main__':
    unittest.main()
