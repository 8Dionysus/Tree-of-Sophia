from __future__ import annotations

import copy
import json
import shutil
import sqlite3
import stat
import subprocess
import tempfile
import unittest
from collections import Counter, defaultdict
from pathlib import Path

from jsonschema import Draft202012Validator


REPO = Path(__file__).resolve().parents[1]
ROUTE = REPO / "ToS/candidate-intake/zarathustra/concept-workbench-v1"
PRIVATE = REPO / "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/concept-workbench-v1"


def load(name: str) -> dict:
    return json.loads((ROUTE / name).read_text(encoding="utf-8"))


def jsonl(name: str) -> list[dict]:
    return [json.loads(line) for line in (ROUTE / name).read_text(encoding="utf-8").splitlines()]


class ZarathustraConceptWorkbenchV1Tests(unittest.TestCase):
    def run_concept_search(self, query: str, language: str, *extra: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python", "scripts/query_zarathustra_concept_workbench_v1.py",
             "--query", query, "--language", language, *extra],
            cwd=REPO, text=True, capture_output=True,
        )

    def run_word_analysis(self, query: str, language: str, *extra: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python", "scripts/prepare_zarathustra_word_analysis_v1.py",
             "--query", query, "--language", language, *extra],
            cwd=REPO, text=True, capture_output=True,
        )

    def test_builder_parity_and_request_contract(self):
        result = subprocess.run(
            ["python", "scripts/build_zarathustra_concept_workbench_v1.py", "--check"],
            cwd=REPO, text=True, capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        schema = load("concept-request.v2.schema.json")
        Draft202012Validator(schema).validate(load("requests/fate.concept-request.v2.json"))
        manifest = load("manifest.v1.json")
        self.assertEqual(manifest["concept_search_query_ref"],
                         "scripts/query_zarathustra_concept_workbench_v1.py")
        self.assertEqual(manifest["concept_search_result_schema_ref"],
                         "ToS/candidate-intake/zarathustra/concept-workbench-v1/concept-search-result.v1.schema.json")
        self.assertEqual(manifest["word_analysis_task_schema_ref"],
                         "ToS/candidate-intake/zarathustra/concept-workbench-v1/word-analysis-task.v1.schema.json")
        self.assertEqual(manifest["word_analysis_prepare_ref"],
                         "scripts/prepare_zarathustra_word_analysis_v1.py")

    def test_second_request_and_relation_allowlist_need_no_code_change(self):
        request = load("requests/fate.concept-request.v2.json")
        request.update({
            "request_identity_key": "tos.concept-request-key.sid-1234567890abcdef1234567890abcdef",
            "request_key": "fate_clone", "labels": {"de": "Geschick", "ru": "участь", "en": "destiny"},
            "relation_policy": {
                "hub_not_clique": True,
                "structural_types": ["form_membership_candidate", "lexical_realization",
                                     "morphological_realization", "semantic_neighbor_candidate"],
                "allowed_types": [],
            },
        })
        output = ROUTE / "outputs/fate_clone-v2-1234567890abcdef1234567890abcdef"
        private_request = PRIVATE / "requests/fate_clone-v2-1234567890abcdef1234567890abcdef.request-analysis.v1.json"
        with tempfile.NamedTemporaryFile("w", suffix=".json", encoding="utf-8") as handle:
            json.dump(request, handle, ensure_ascii=False)
            handle.flush()
            try:
                result = subprocess.run(
                    ["python", "scripts/build_zarathustra_concept_workbench_v1.py", "--build",
                     "--issue-identities", "--request", handle.name],
                    cwd=REPO, text=True, capture_output=True,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                checked = subprocess.run(
                    ["python", "scripts/build_zarathustra_concept_workbench_v1.py", "--check",
                     "--request", handle.name], cwd=REPO, text=True, capture_output=True,
                )
                self.assertEqual(checked.returncode, 0, checked.stderr)

                graph = json.loads((output / "candidate-graph.v1.json").read_text(encoding="utf-8"))
                node_ids = {row["id"] for row in graph["nodes"]}
                degree = Counter()
                adjacency = defaultdict(set)
                for edge in graph["edges"]:
                    endpoints = edge["subject_refs"] + edge["object_refs"]
                    self.assertTrue(set(endpoints).issubset(node_ids))
                    for endpoint in endpoints:
                        degree[endpoint] += 1
                    for left in edge["subject_refs"]:
                        for right in edge["object_refs"]:
                            adjacency[left].add(right)
                            adjacency[right].add(left)
                self.assertTrue(all(degree[node] >= 1 for node in node_ids))
                concept = next(row["id"] for row in graph["nodes"] if row["kind"] == "concept_candidate")
                reached, frontier = {concept}, [concept]
                while frontier:
                    current = frontier.pop()
                    for neighbor in adjacency[current] - reached:
                        reached.add(neighbor)
                        frontier.append(neighbor)
                self.assertTrue(all(row["id"] in reached for row in graph["nodes"]
                                    if row["kind"] == "occurrence_candidate"))
                self.assertEqual({edge["relation_type"] for edge in graph["edges"]},
                                 {"form_membership_candidate", "lexical_realization",
                                  "morphological_realization", "semantic_neighbor_candidate"})

                canonical = load("identity-issuance.v1.json")
                clone = json.loads((output / "identity-issuance.v1.json").read_text(encoding="utf-8"))
                local_kinds = {"form", "occurrence", "relation"}
                canonical_ids = {row["id"] for row in canonical["records"] if row["kind"] in local_kinds}
                clone_ids = {row["id"] for row in clone["records"] if row["kind"] in local_kinds}
                self.assertTrue(canonical_ids.isdisjoint(clone_ids))
            finally:
                shutil.rmtree(output, ignore_errors=True)
                private_request.unlink(missing_ok=True)

        scoped_paths = {(
            "ToS/candidate-intake/zarathustra/concept-workbench-v1/outputs/"
            "fate_clone-v2-1234567890abcdef1234567890abcdef",
            str(PRIVATE.relative_to(REPO) / "requests/fate_clone-v2-1234567890abcdef1234567890abcdef.request-analysis.v1.json"),
        )}
        for identity, version in (
            ("tos.concept-request-key.sid-1234567890abffffffffffffffffffff", 2),
            ("tos.concept-request-key.sid-1234567890abcdef1234567890abcdef", 3),
        ):
            request["request_identity_key"] = identity
            request["request_version"] = version
            with tempfile.NamedTemporaryFile("w", suffix=".json", encoding="utf-8") as handle:
                json.dump(request, handle, ensure_ascii=False)
                handle.flush()
                preview_result = subprocess.run(
                    ["python", "scripts/build_zarathustra_concept_workbench_v1.py", "--preview",
                     "--request", handle.name], cwd=REPO, text=True, capture_output=True,
                )
            self.assertEqual(preview_result.returncode, 0, preview_result.stderr)
            preview = json.loads(preview_result.stdout)
            scoped_paths.add((preview["request_output_root"], preview["private_request_ref"]))
        self.assertEqual(len(scoped_paths), 3)

        request.pop("request_version")
        with tempfile.NamedTemporaryFile("w", suffix=".json", encoding="utf-8") as handle:
            json.dump(request, handle, ensure_ascii=False)
            handle.flush()
            invalid = subprocess.run(
                ["python", "scripts/build_zarathustra_concept_workbench_v1.py", "--preview", "--request", handle.name],
                cwd=REPO, text=True, capture_output=True,
            )
        self.assertNotEqual(invalid.returncode, 0)

    def test_complete_witness_local_population_including_verse(self):
        forms = load("all-form-coverage.v1.json")
        summary = load("summary.v1.json")
        contexts = jsonl("context-unit-spine.v1.jsonl")
        speakers = jsonl("speaker-state-candidates.v1.jsonl")
        self.assertEqual(forms["witness_context_unit_counts"], {"de": 3815, "ru": 3928})
        self.assertEqual(forms["unit_kind_counts"], {
            "de_paragraph": 3447, "de_verse_line": 368,
            "ru_paragraph": 3569, "ru_verse_line": 359,
        })
        self.assertEqual(len(contexts), 7743)
        self.assertEqual(len(speakers), 7743)
        self.assertEqual(summary["alignment_unit_count"], 3423)
        self.assertEqual(summary["alignment_status_counts"], {
            "proposed": 3303, "ambiguous": 111, "deferred": 9,
        })
        self.assertEqual(forms["minimum_form_frequency"], 1)
        self.assertTrue(forms["frequency_one_forms_retained"])
        self.assertTrue(forms["all_analysis_forms_have_explicit_state"])

    def test_english_is_on_demand_source_first_and_etymology_cited(self):
        tasks = jsonl("outputs/fate/english-on-demand-worklist.v1.jsonl")
        self.assertEqual(len(tasks), 57)
        self.assertTrue(all(row["target_language"] == "en" for row in tasks))
        self.assertTrue(all(row["source_text_included"] is False for row in tasks))
        self.assertTrue(all(row["etymology_state"] == "citation_required_before_claim" for row in tasks))
        self.assertTrue(all(row["recognized_english_comparator_state"] == "sealed_until_candidate_frozen"
                            for row in tasks))
        private = json.loads((PRIVATE / "requests/fate.request-analysis.v1.json").read_text(encoding="utf-8"))
        self.assertEqual(len(private["english_tasks"]), 57)
        self.assertTrue(all(row["source_context"] and row["source_surface"]
                            and row["candidate_materialized"] is False
                            for row in private["english_tasks"]))

        task = tasks[0]
        candidate = {
            "schema_version": "tos_zarathustra_english_translation_candidate_v1",
            "translation_candidate_id": "tos.annotation.english-translation-candidate.sid-0123456789abcdef0123456789abcdef",
            "english_task_ref": task["english_task_id"],
            "request_ref": task["request_ref"],
            "source_occurrence_ref": task["source_occurrence_ref"],
            "source_context_unit_ref": task["source_context_unit_ref"],
            "target_language": "en", "source_echo_sha256": task["source_form_sha256"],
            "source_context_echo_sha256": "0" * 64,
            "literal_gloss": "fate", "contextual_translation": "fate",
            "semantic_alternatives": [{"rendering": "destiny", "preserves": ["directed course"],
                                        "loses_or_risks": ["may overstate purpose"]}],
            "untranslatability_note": "English alternatives divide a German semantic range differently.",
            "analysis": {
                "morphology": "neuter singular noun", "syntax": "nominal occurrence",
                "historical_sense": "historical sense remains a cited candidate",
                "contextual_semantics": "contextual sense remains an occurrence-bound candidate",
                "etymology_state": "sourced_candidate",
                "etymology_findings": [{
                    "claim": "bounded etymological proposal",
                    "translation_consequence": "retain fate and destiny as alternatives",
                    "citations": [{"reference_id": "tos-ref.dwds.pfeifer-etymwb",
                                   "locator": "exact headword article",
                                   "url": "https://www.dwds.de/wb/etymwb/Schicksal",
                                   "accessed_at": "2026-09-02"}],
                    "epistemic_status": "proposed",
                }],
                "intra_work_recurrence": "compare repeated source occurrences before fixing one rendering",
            },
            "russian_witness_comparison": {
                "consulted": True,
                "role": "historical_translation_comparator_not_source_authority",
                "divergence_note": "Russian rendering is evidence of a translator decision, not German meaning.",
            },
            "maker": {"maker_kind": "ai", "model_id": "test-model",
                      "context_isolation": "current_task_only", "training_exposure": "unknown"},
            "provenance_event_ref": "tos.event.synthetic.english-candidate",
            "status": "ai_generated_unreviewed_not_translation_truth",
            "accepted": False, "review_status": "unreviewed",
            "translation_truth_asserted": False, "semantic_fact_asserted": False,
            "graph_effect": False, "canon_effect": False,
        }
        validator = Draft202012Validator(load("english-translation-candidate.v1.schema.json"))
        validator.validate(candidate)
        unsupported = copy.deepcopy(candidate)
        unsupported["analysis"]["etymology_findings"][0]["citations"] = []
        self.assertTrue(list(validator.iter_errors(unsupported)))

    def test_exact_occurrence_layer_and_work_scope_are_separate(self):
        forms = load("all-form-coverage.v1.json")
        self.assertEqual(forms["source_item_exact_occurrence_counts"], {"de": 86287, "ru": 93643})
        self.assertEqual(forms["work_scope_exact_occurrence_counts"], {"de": 84491, "ru": 93643})
        self.assertEqual(forms["outside_work_exact_occurrence_counts"], {"de": 1796, "ru": 0})
        self.assertEqual(forms["work_scope_exact_form_counts"], {"de": 11118, "ru": 17443})
        self.assertEqual(forms["work_scope_analysis_form_counts"], {"de": 9909, "ru": 15152})
        self.assertTrue(forms["exact_and_analysis_tokens_distinct"])
        self.assertFalse(forms["line_join_reconstruction_overwrites_witness"])

    def test_fate_acceptance_probe_and_negative_controls(self):
        coverage = load("outputs/fate/coverage-receipt.v1.json")
        self.assertEqual(coverage["acceptance_probe"], {
            "de_direct_occurrences": 26, "ru_direct_occurrences": 29,
            "de_direct_prose_occurrences": 26, "de_direct_verse_occurrences": 0,
            "ru_direct_prose_occurrences": 29, "ru_direct_verse_occurrences": 0,
            "parallel_alignment_intersection": 20, "de_only_alignment_groups": 4,
            "ru_only_alignment_groups": 7, "alignment_union": 31,
            "lowercase_los_hard_negative_occurrences": 10,
            "verhaengniss_family_semantic_candidates": 6,
        })
        self.assertEqual(coverage["evidence_tier_counts"], {
            "direct_or_morphological": 55, "semantic_neighbor": 52,
        })
        self.assertEqual(coverage["requested_seed_exact_absence_count"], 4)
        self.assertTrue(coverage["mechanically_complete_with_explicit_seed_gaps"])
        self.assertTrue(coverage["complete_for_declared_request"])
        exclusions = jsonl("outputs/fate/exclusion-ledger.v1.jsonl")
        self.assertEqual(Counter(x["control_code"] for x in exclusions), Counter({
            "de_los_particle_or_command": 10,
            "semantic_neighbor_not_direct_mention": 7,
            "outside_zarathustra_work_scope": 8,
        }))

    def test_context_barriers_and_occurrence_closure(self):
        contexts = jsonl("context-unit-spine.v1.jsonl")
        occurrences = jsonl("outputs/fate/occurrence-spine.v1.jsonl")
        by_ref = {x["context_unit_ref"]: x for x in contexts}
        self.assertTrue(all(x["context_unit_ref"] in by_ref for x in occurrences))
        self.assertTrue(all(x["speaker_state_candidate_ref"] for x in occurrences))
        for row in contexts:
            for field in ("previous_context_unit_ref", "next_context_unit_ref"):
                neighbor = row[field]
                if neighbor:
                    self.assertEqual(by_ref[neighbor]["language"], row["language"])
                    self.assertEqual(by_ref[neighbor]["reading_ref"], row["reading_ref"])
        grouped: dict[tuple[str, str], list[int]] = defaultdict(list)
        for row in contexts:
            grouped[(row["language"], row["reading_ref"])].append(row["witness_order"])
        self.assertTrue(all(values == sorted(values) for values in grouped.values()))

    def test_graph_is_candidate_hub_not_occurrence_clique(self):
        graph = load("outputs/fate/candidate-graph.v1.json")
        occurrences = jsonl("outputs/fate/occurrence-spine.v1.jsonl")
        relations = jsonl("outputs/fate/relation-candidates.v1.jsonl")
        policy = load("requests/fate.concept-request.v2.json")["relation_policy"]
        allowed = set(policy["structural_types"]) | set(policy["allowed_types"])
        self.assertEqual(graph["topology"], "concept_hub_not_occurrence_clique")
        self.assertLess(len(relations), len(occurrences) ** 2)
        self.assertTrue({x["relation_type"] for x in relations}.issubset(allowed))
        node_ids = {x["id"] for x in graph["nodes"]}
        self.assertTrue(all(set(edge["subject_refs"] + edge["object_refs"]).issubset(node_ids)
                            for edge in graph["edges"]))
        self.assertEqual(Counter(x["kind"] for x in graph["nodes"])["form_family_candidate"], 25)
        self.assertTrue(all(any(form["id"] in edge["object_refs"] for edge in graph["edges"])
                            for form in graph["nodes"] if form["kind"] == "form_family_candidate"))
        degree = Counter(ref for edge in graph["edges"] for ref in edge["subject_refs"] + edge["object_refs"])
        self.assertTrue(all(degree[node] >= 1 for node in node_ids))
        provenance = jsonl("provenance.jsonl")
        self.assertEqual(len(provenance), 1)
        self.assertTrue(all(row["provenance_event_ref"] == provenance[0]["event_id"] for row in relations))
        for row in relations:
            self.assertEqual(set(row["evidence_refs"]), {item["evidence_ref"] for item in row["evidence_roles"]})
            if row["relation_type"] == "translation_parallel_candidate":
                self.assertEqual({item["role"] for item in row["evidence_roles"]},
                                 {"subject_occurrence", "object_occurrence"})
        self.assertEqual(graph["accepted_edge_count"], 0)
        self.assertFalse(graph["semantic_fact_asserted"])
        self.assertFalse(graph["graph_effect"])
        self.assertFalse(graph["canon_effect"])
        self.assertTrue(all(not x["accepted"] and not x["graph_effect"] and not x["canon_effect"] for x in occurrences))

    def test_private_boundary_and_label_independent_identity(self):
        for name in ("workbench-index.v1.sqlite3", "requests/fate.request-analysis.v1.json"):
            self.assertEqual(stat.S_IMODE((PRIVATE / name).stat().st_mode), 0o600)
        database = sqlite3.connect(f"file:{PRIVATE / 'workbench-index.v1.sqlite3'}?mode=ro&immutable=1", uri=True)
        states = database.execute("SELECT morphology_state,count(*) FROM analysis_forms GROUP BY morphology_state").fetchall()
        schicksal = database.execute("SELECT in_work_scope,count(*) FROM exact_occurrences WHERE language='de' AND analysis_key='schicksal' GROUP BY in_work_scope").fetchall()
        database.close()
        self.assertEqual(states, [("unresolved_candidate_available_for_request_expansion", 25061)])
        self.assertEqual(schicksal, [(0, 3), (1, 21)])
        issuance = load("identity-issuance.v1.json")
        identities = [x["id"] for x in issuance["records"]]
        self.assertEqual(len(identities), len(set(identities)))
        bindings = "\n".join(x["binding"] for x in issuance["records"]).casefold()
        self.assertNotIn("schicksal", bindings)
        self.assertNotIn("судьба", bindings)
        self.assertEqual(load("summary.v1.json")["accepted_candidate_count"], 0)
        self.assertEqual(load("summary.v1.json")["human_review_count"], 0)

    def test_inflected_russian_concept_query_returns_original_german_source(self):
        result = self.run_concept_search("судьбы", "ru", "--limit", "100")
        self.assertEqual(result.returncode, 0, result.stderr)
        payload = json.loads(result.stdout)
        Draft202012Validator(load("concept-search-result.v1.schema.json")).validate(payload)

        concept = load("outputs/fate/concept-candidate.v1.json")
        self.assertEqual(payload["query_analysis"]["match_method"], "morphology_alias_candidate")
        self.assertEqual(payload["query_analysis"]["matched_alias"], "судьба")
        self.assertEqual(payload["query_analysis"]["resolution_tier"], "direct")
        self.assertEqual(payload["concept_candidate_ref"], concept["concept_candidate_id"])
        self.assertEqual(payload["provenance"]["query_adapter_ref"],
                         "scripts/query_zarathustra_concept_workbench_v1.py")
        self.assertEqual(payload["concept_search_route"]["identity_posture"],
                         "stable_navigation_identity_not_semantic_concept_identity")
        self.assertIsNone(payload["concept_search_route"]["accepted_concept_ref"])
        self.assertEqual(payload["coverage"]["total_source_results"], 26)
        self.assertEqual(payload["coverage"]["returned_source_results"], 26)
        self.assertFalse(payload["coverage"]["semantic_neighbors_included"])
        self.assertEqual(Counter(item["source_surface"] for item in payload["results"]), Counter({
            "Schicksal": 21,
            "Schicksale": 4,
            "Menschen-Schicksals": 1,
        }))

        for item in payload["results"]:
            self.assertEqual(item["source_language"], "de")
            self.assertEqual(item["evidence_tier"], "direct_or_morphological")
            self.assertIn(item["source_surface"], item["source_context"])
            self.assertEqual(item["source_headword_candidate"], "Schicksal")
            self.assertEqual(item["source_headword_status"], "request_seed_not_accepted_lemma")
            self.assertTrue(item["source_existing_occurrence_ref"])
            self.assertTrue(item["source_context_unit_ref"])
            self.assertGreaterEqual(item["occurrence_ordinal_within_context"], 1)
            self.assertTrue(item["realization_relation_candidate_ref"])
            self.assertEqual(
                [step["step"] for step in item["query_to_source_path"]],
                ["query_alias_match", "routes_to_concept_candidate",
                 "candidate_realization", "source_return"],
            )

    def test_concept_search_keeps_direct_semantic_and_negative_routes_distinct(self):
        direct = self.run_concept_search("судьба", "ru", "--limit", "0")
        self.assertEqual(direct.returncode, 0, direct.stderr)
        direct_payload = json.loads(direct.stdout)
        self.assertEqual(direct_payload["query_analysis"]["match_method"], "exact_alias")
        self.assertEqual(direct_payload["coverage"]["total_source_results"], 26)
        self.assertEqual(direct_payload["results"], [])

        expanded = self.run_concept_search(
            "судьба", "ru", "--include-semantic-neighbors", "--limit", "0",
        )
        self.assertEqual(expanded.returncode, 0, expanded.stderr)
        expanded_payload = json.loads(expanded.stdout)
        self.assertEqual(expanded_payload["coverage"]["total_source_results"], 57)
        self.assertEqual(expanded_payload["coverage"]["source_evidence_tier_counts"], {
            "direct_or_morphological": 26,
            "semantic_neighbor": 31,
        })
        self.assertTrue(expanded_payload["coverage"]["semantic_neighbors_included"])

        semantic_alias = self.run_concept_search("рок", "ru", "--limit", "0")
        self.assertEqual(semantic_alias.returncode, 0, semantic_alias.stderr)
        semantic_payload = json.loads(semantic_alias.stdout)
        self.assertEqual(semantic_payload["query_analysis"]["resolution_tier"], "semantic_neighbor")
        self.assertEqual(semantic_payload["query_analysis"]["resolution_status"], "ambiguous")

        negative = self.run_concept_search("los", "de", "--limit", "0")
        self.assertNotEqual(negative.returncode, 0)
        self.assertIn("no concept-search route", negative.stderr)

    def test_word_analysis_task_returns_exact_source_and_agent_contract(self):
        result = self.run_word_analysis("судьбы", "ru", "--rank", "1")
        self.assertEqual(result.returncode, 0, result.stderr)
        task = json.loads(result.stdout)
        Draft202012Validator(load("word-analysis-task.v1.schema.json")).validate(task)

        self.assertEqual(task["query_analysis"]["matched_alias"], "судьба")
        self.assertEqual(task["source"]["language"], "de")
        self.assertIn(task["source"]["surface"], task["source"]["exact_context"])
        self.assertEqual(task["source"]["surface_sha256"],
                         __import__("hashlib").sha256(task["source"]["surface"].encode()).hexdigest())
        self.assertEqual(task["source"]["context_sha256"],
                         __import__("hashlib").sha256(task["source"]["exact_context"].encode()).hexdigest())
        self.assertEqual(task["analysis_policy"]["required_stages"], [
            "morphology", "syntax", "historical_sense", "sourced_etymology",
            "contextual_semantics", "intra_work_recurrence",
            "russian_witness_comparison", "english_rendering",
        ])
        self.assertEqual(task["analysis_policy"]["etymology"]["minimum_citation_count"], 1)
        self.assertEqual(task["recurrence_navigation"]["total_source_results"], 26)
        self.assertEqual(task["recurrence_navigation"]["current_rank"], 1)
        self.assertEqual(task["recurrence_navigation"]["next_rank"], 2)
        self.assertTrue(task["analysis_policy"]["etymology"]["evidence_route_ref"].endswith(
            "ETYMOLOGY_EVIDENCE_ROUTE_RESEARCH.md"
        ))
        self.assertFalse(task["authority"]["accepted"])
        self.assertFalse(task["authority"]["semantic_fact_asserted"])
        self.assertFalse(task["authority"]["canon_effect"])

    def test_word_analysis_candidate_validation_binds_context_and_citations(self):
        prepared = self.run_word_analysis("судьбы", "ru", "--rank", "1")
        self.assertEqual(prepared.returncode, 0, prepared.stderr)
        task = json.loads(prepared.stdout)
        candidate = {
            "schema_version": "tos_zarathustra_english_translation_candidate_v1",
            "translation_candidate_id": "tos.annotation.english-translation-candidate.sid-0123456789abcdef0123456789abcdef",
            "english_task_ref": task["english_on_demand_task_ref"],
            "request_ref": task["request_ref"],
            "source_occurrence_ref": task["source"]["occurrence_candidate_ref"],
            "source_context_unit_ref": task["source"]["context_unit_ref"],
            "target_language": "en",
            "source_echo_sha256": task["source"]["surface_sha256"],
            "source_context_echo_sha256": task["source"]["context_sha256"],
            "literal_gloss": "fate",
            "contextual_translation": "fate",
            "semantic_alternatives": [{
                "rendering": "destiny", "preserves": ["directed course"],
                "loses_or_risks": ["may overstate purpose"],
            }],
            "untranslatability_note": "The alternatives divide the source range differently.",
            "analysis": {
                "morphology": "neuter singular noun",
                "syntax": "nominal occurrence",
                "historical_sense": "historical sense remains a cited candidate",
                "contextual_semantics": "sense and rhetorical force remain occurrence-bound proposals",
                "etymology_state": "sourced_candidate",
                "etymology_findings": [{
                    "claim": "bounded etymological proposal",
                    "translation_consequence": "retain competing English renderings",
                    "citations": [{
                        "reference_id": "tos-ref.synthetic",
                        "locator": "exact headword article",
                        "url": "https://example.invalid/Schicksal",
                        "accessed_at": "2026-09-03",
                    }],
                    "epistemic_status": "proposed",
                }],
                "intra_work_recurrence": "compare exact-form recurrence before fixing a rendering",
            },
            "russian_witness_comparison": {
                "consulted": True,
                "role": "historical_translation_comparator_not_source_authority",
                "divergence_note": "The Russian witness is a translator decision, not German meaning.",
            },
            "maker": {
                "maker_kind": "ai", "model_id": "test-model",
                "context_isolation": "current_task_only", "training_exposure": "unknown",
            },
            "provenance_event_ref": "tos.event.synthetic.word-analysis",
            "status": "ai_generated_unreviewed_not_translation_truth",
            "accepted": False, "review_status": "unreviewed",
            "translation_truth_asserted": False, "semantic_fact_asserted": False,
            "graph_effect": False, "canon_effect": False,
        }
        with tempfile.NamedTemporaryFile("w", suffix=".json", encoding="utf-8") as handle:
            json.dump(candidate, handle, ensure_ascii=False)
            handle.flush()
            checked = self.run_word_analysis(
                "судьбы", "ru", "--rank", "1", "--validate-candidate", handle.name,
            )
        self.assertEqual(checked.returncode, 0, checked.stderr)
        self.assertTrue(json.loads(checked.stdout)["valid"])

        candidate["source_context_echo_sha256"] = "0" * 64
        with tempfile.NamedTemporaryFile("w", suffix=".json", encoding="utf-8") as handle:
            json.dump(candidate, handle, ensure_ascii=False)
            handle.flush()
            rebound = self.run_word_analysis(
                "судьбы", "ru", "--rank", "1", "--validate-candidate", handle.name,
            )
        self.assertNotEqual(rebound.returncode, 0)
        self.assertIn("source context digest", rebound.stderr)

        candidate["source_context_echo_sha256"] = task["source"]["context_sha256"]
        candidate["analysis"]["etymology_findings"][0]["citations"] = []
        with tempfile.NamedTemporaryFile("w", suffix=".json", encoding="utf-8") as handle:
            json.dump(candidate, handle, ensure_ascii=False)
            handle.flush()
            uncited = self.run_word_analysis(
                "судьбы", "ru", "--rank", "1", "--validate-candidate", handle.name,
            )
        self.assertNotEqual(uncited.returncode, 0)


if __name__ == "__main__":
    unittest.main()
