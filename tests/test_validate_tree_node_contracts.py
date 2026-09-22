from __future__ import annotations

import copy
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = REPO_ROOT / "scripts" / "validate_tree_node_contracts.py"
SPEC = importlib.util.spec_from_file_location("validate_tree_node_contracts", MODULE_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"unable to load validator module from {MODULE_PATH}")
validate_tree_node_contracts = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(validate_tree_node_contracts)


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


class ValidateTreeNodeContractsTests(unittest.TestCase):
    def synthetic_source(self):
        return {
            'schema_version': 'tos_canonical_node_v1', 'record_version': 1,
            'node_id': 'tos.source.synthetic', 'node_type': 'source',
            'source_anchor': 'synthetic:source', 'key_terms': ['question'],
            'distilled_thesis': 'A synthetic source-linked question.',
            'relations': [{'relation': 'commentary-on', 'target_ref': 'synthetic:related'}],
            'interpretation_layers': ['source_linked'],
            'language_witnesses': [
                {'language': language, 'role': role,
                 'segments': [{'segment_id': identifier, 'text': text} for identifier, text in
                              [('opening', 'A question.'), ('return', 'An open answer.')]]}
                for language, role in [('el', 'canonical_source'), ('ja', 'working_translation'),
                                       ('ar', 'bridge_translation')]],
            'translation_tensions': [{'segment_id': 'return', 'note': 'Synthetic qualified reading.'}],
        }

    def validate_sources(self, *payloads):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            schema = root / 'ToS/contracts/tos-node-contract.schema.json'
            schema.parent.mkdir(parents=True)
            schema.write_bytes((REPO_ROOT / 'ToS/contracts/tos-node-contract.schema.json').read_bytes())
            for index, payload in enumerate(payloads):
                path = root / 'ToS/canon/source' / str(index) / 'node.json'
                path.parent.mkdir(parents=True)
                path.write_text(payload if isinstance(payload, str) else json.dumps(payload), encoding='utf-8')
            return validate_tree_node_contracts.run_validation(root)

    def test_new_languages_and_segment_counts_follow_one_shared_contract(self):
        source = self.synthetic_source()
        self.assertEqual(self.validate_sources(source), [])
        source['lineage_relations'] = copy.deepcopy(source['relations'])
        for witness in source['language_witnesses']:
            witness['segments'].append({'segment_id': 'further-question', 'text': 'Further inquiry.'})
        second = copy.deepcopy(source)
        second['node_id'] = 'tos.source.another'
        self.assertEqual(self.validate_sources(source, second), [])

    def test_cross_field_defects_refuse_both_canon_validation_and_form_reading(self):
        from source_witness_human_forms import _validate_canonical_node

        for defect in ('family', 'duplicate-segment', 'missing-segment', 'segment-order',
                       'tension', 'alias', 'duplicate-language'):
            source = self.synthetic_source()
            if defect == 'family':
                source['node_id'] = 'tos.concept.synthetic'
            elif defect == 'duplicate-segment':
                source['language_witnesses'][0]['segments'].append(
                    copy.deepcopy(source['language_witnesses'][0]['segments'][0]))
            elif defect == 'missing-segment':
                source['language_witnesses'][1]['segments'].pop()
            elif defect == 'segment-order':
                source['language_witnesses'][1]['segments'].reverse()
            elif defect == 'tension':
                source['translation_tensions'][0]['segment_id'] = 'unbound'
            elif defect == 'alias':
                source['lineage_relations'] = [{'relation': 'parallel', 'target_ref': 'synthetic:other'}]
            else:
                source['language_witnesses'][1]['language'] = 'el'
            with self.subTest(defect=defect):
                self.assertTrue(self.validate_sources(source))
                with self.assertRaises(ValueError):
                    _validate_canonical_node(source)

    def test_canonical_identity_has_one_authored_home(self):
        source = self.synthetic_source()
        self.assertTrue(any('duplicate canonical node_id' in message
                            for _, message in self.validate_sources(source, copy.deepcopy(source))))

    def test_ambiguous_json_cannot_hide_a_source_identity_or_value(self):
        source = json.dumps(self.synthetic_source())
        for suffix in ('"node_id":"tos.source.another"', '"unrecognized":NaN',
                       '"unrecognized":Infinity', '"unrecognized":1e9999'):
            with self.subTest(suffix=suffix):
                self.assertTrue(any('invalid JSON' in message for _, message in
                                    self.validate_sources(source[:-1] + ',' + suffix + '}')))

    def write_canonical_source(self, root, source):
        from source_witness_human_forms import CANONICAL_NODE_SCHEMA_REF

        schema = root / CANONICAL_NODE_SCHEMA_REF
        schema.parent.mkdir(parents=True, exist_ok=True)
        schema.write_bytes((REPO_ROOT / CANONICAL_NODE_SCHEMA_REF).read_bytes())
        relative = 'ToS/canon/source/synthetic/node.json'
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(source if isinstance(source, str) else json.dumps(source), encoding='utf-8')
        return relative

    def test_canonical_form_reader_binds_typed_values_and_preserves_input_bytes(self):
        from source_witness_human_forms import _read_canonical_source

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for original, altered in ((False, 0), (1, 1.0), (None, '')):
                source = self.synthetic_source()
                source['field_languages'] = {'distilled_thesis': {
                    'language': None, 'script': None, 'qualification': {'unknown': original}}}
                relative = self.write_canonical_source(root, source)
                # Key order and whitespace do not change the immutable subject.
                equivalent = dict(reversed(list(source.items())))
                _, raw, actual, _ = _read_canonical_source(root, relative, equivalent)
                self.assertEqual(raw, (root / relative).read_bytes())
                self.assertEqual(actual, source)
                forged = copy.deepcopy(source)
                forged['field_languages']['distilled_thesis']['qualification']['unknown'] = altered
                with self.subTest(original=original, altered=altered), self.assertRaises(ValueError):
                    _read_canonical_source(root, relative, forged)
                forged['field_languages']['distilled_thesis']['qualification'].pop('unknown')
                with self.subTest(original=original, absent=True), self.assertRaises(ValueError):
                    _read_canonical_source(root, relative, forged)

    def test_canonical_form_reader_refuses_ambiguous_or_malformed_original_bytes(self):
        from source_witness_human_forms import _read_canonical_source

        source = json.dumps(self.synthetic_source())
        ambiguous = source[:-1] + ',"source_anchor":"synthetic:source"}'
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for raw in (ambiguous, '[]', source[:-1] + ',"unrecognized":NaN}'):
                relative = self.write_canonical_source(root, raw)
                with self.subTest(raw=raw[:40]), self.assertRaises(ValueError):
                    _read_canonical_source(root, relative)

    def test_malformed_witness_shape_reports_schema_issue(self):
        source = self.synthetic_source()
        source['language_witnesses'] = [None, {'segments': 'unreadable'}]
        self.assertTrue(self.validate_sources(source))

    def test_versioned_native_names_require_explicit_opt_in_and_preserve_legacy(self):
        schema = load_json(REPO_ROOT / 'ToS/contracts/tos-node-contract.schema.json')
        validator = Draft202012Validator(schema)
        legacy = load_json(REPO_ROOT / 'ToS/public-compatibility/event_node.example.json')
        for key in ('schema_version', 'record_version', 'preferred_label', 'variant_labels', 'field_languages'):
            legacy.pop(key, None)
        self.assertTrue(validator.is_valid(legacy))
        source = {**legacy, 'schema_version': 'tos_canonical_node_v1', 'record_version': 1,
            'preferred_label': 'Synthetic name', 'variant_labels': [{'value': 'Синтетическое имя',
                'language': 'ru', 'script': 'Cyrl', 'source_ref': 'synthetic:wording', 'status': 'unverified'}],
            'field_languages': {'preferred_label': {'language': 'en', 'script': 'Latn'},
                'distilled_thesis': {'language': None, 'script': None, 'qualification': {'unknown': False}}}}
        validator.validate(source)
        self.assertEqual(source['node_id'], legacy['node_id'])
        for change in ({'record_version': 0}, {'record_version': True}, {'record_version': 9007199254740992},
                       {'schema_version': 'tos_canonical_node_v2'}, {'record_id': source['node_id']},
                       {'field_languages': {'notes': {'language': 'en', 'script': None}}}):
            with self.subTest(change=change):
                self.assertFalse(validator.is_valid({**source, **change}))
        for key in ('schema_version', 'record_version'):
            broken = {k: v for k, v in source.items() if k != key}
            self.assertFalse(validator.is_valid(broken))
        for key in ('preferred_label', 'variant_labels', 'field_languages'):
            self.assertFalse(validator.is_valid({**legacy, key: source[key]}))

    def test_duplicate_witness_languages_fail_contract_validation(self) -> None:
        payload = load_json(REPO_ROOT / "ToS" / "public-compatibility" / "source_node.example.json")
        assert isinstance(payload, dict)
        payload = copy.deepcopy(payload)
        payload["language_witnesses"][1]["language"] = payload["language_witnesses"][0]["language"]

        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp) / "Tree-of-Sophia"
            schema_path = repo_root / "ToS" / "contracts" / "tos-node-contract.schema.json"
            node_path = repo_root / "ToS" / "canon" / "source" / "test-route" / "node.json"
            schema_path.parent.mkdir(parents=True, exist_ok=True)
            node_path.parent.mkdir(parents=True, exist_ok=True)
            schema_path.write_text(
                (REPO_ROOT / "ToS" / "contracts" / "tos-node-contract.schema.json").read_text(encoding="utf-8"),
                encoding="utf-8",
            )
            node_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

            issues = validate_tree_node_contracts.run_validation(repo_root)

        self.assertIn(
            (
                "ToS/canon/source/test-route/node.json",
                "language_witnesses contains a duplicate language: de",
            ),
            issues,
        )

    def test_duplicate_extension_witness_languages_fail_contract_validation(self) -> None:
        payload = load_json(REPO_ROOT / "ToS" / "public-compatibility" / "source_node.example.json")
        assert isinstance(payload, dict)
        payload = copy.deepcopy(payload)
        payload["language_witnesses"][0]["language"] = "el"
        payload["language_witnesses"][1]["language"] = "el"

        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp) / "Tree-of-Sophia"
            schema_path = repo_root / "ToS" / "contracts" / "tos-node-contract.schema.json"
            node_path = repo_root / "ToS" / "canon" / "source" / "test-route" / "node.json"
            schema_path.parent.mkdir(parents=True, exist_ok=True)
            node_path.parent.mkdir(parents=True, exist_ok=True)
            schema_path.write_text(
                (REPO_ROOT / "ToS" / "contracts" / "tos-node-contract.schema.json").read_text(encoding="utf-8"),
                encoding="utf-8",
            )
            node_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

            issues = validate_tree_node_contracts.run_validation(repo_root)

        self.assertIn(
            (
                "ToS/canon/source/test-route/node.json",
                "language_witnesses contains a duplicate language: el",
            ),
            issues,
        )


if __name__ == "__main__":
    unittest.main()
