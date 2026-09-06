"""Policy invariants over synthetic records; not evidence of agent competence.

The variation space is exact dependencies, trusted scope/authority, chronology,
review permutations, and source/reviewer duplication. Authentication and the
substantive accuracy of prose remain outside this pure engine's claim.
"""
from __future__ import annotations

import copy
from dataclasses import replace
import itertools
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "mechanics/growth-cycle/parts/branch-growth-cycle/scripts"))

from knowledge_assessment import AssessmentEngine, Record, SubjectContext, Submission


NOW = "2026-09-05T12:00:00Z"
START = "2026-09-01T00:00:00Z"
END = "2026-10-01T00:00:00Z"


class AssessmentPolicyTests(unittest.TestCase):
    def setUp(self):
        self.policy = Record.from_payload(
            "tos.policy.knowledge-assessment", 1,
            json.loads((ROOT / "ToS/doctrine/semantic-interchange/assessment-policy.v1.json").read_text()),
        )
        self.subject = Record.from_payload("tos.claim.fixture", 1, {"claim": "synthetic assertion"})
        self.source = Record.from_payload("tos.file.fixture-a", 1, {"text": "synthetic source A"}, origin_id="source-a")
        self.source_b = Record.from_payload("tos.file.fixture-b", 1, {"text": "synthetic source B"}, origin_id="source-b")
        self.eval_evidence = Record.from_payload("tos.review.fixture-calibration", 1, {"synthetic": True})
        self.executor = Record.from_payload("tos.method.fixture-review", 1, {"procedure_ref": "fixture:source-check", "model_ref": "fixture:not-a-real-model"})
        self.records = [self.subject, self.source, self.source_b, self.eval_evidence, self.executor]
        self.competencies = []
        self.authorities = []
        for actor in ("assessor-a", "assessor-b"):
            competence = Record.from_payload(f"tos.competence.{actor}", 1, {
                "schema_version": "tos_knowledge_assessment_competence_v1",
                "competence_id": f"tos.competence.{actor}", "competence_version": 1,
                "actor_id": actor, "assertion_layers": ["bibliographic_assertion", "semantic_interpretation", "identity_assertion"],
                "languages": ["ru", "de"], "profile_ids": ["source-observation", "interpretation", "identity", "high-consequence"],
                "execution_profiles": [self.executor.ref], "state": "verified", "valid_from": START, "valid_until": END,
                "evidence_refs": [self.eval_evidence.ref], "issuer_ref": "fixture:trusted-issuer-not-a-real-competence-claim",
            })
            self.competencies.append(competence)
            self.authorities.append(Record.from_payload(f"tos.authority.{actor}", 1, {
                "schema_version": "tos_knowledge_assessment_authority_v1",
                "authority_id": f"tos.authority.{actor}", "authority_version": 1,
                "actor_id": actor, "actor_kind": "agent", "policy": self.policy.ref,
                "profile_ids": ["source-observation", "interpretation", "identity", "high-consequence"],
                "assertion_layers": ["bibliographic_assertion", "semantic_interpretation", "identity_assertion"],
                "languages": ["ru", "de"], "uses": ["research"], "subject_prefixes": ["tos.claim."],
                "decisions": ["admit", "admit-with-limits", "reject", "dispute", "defer", "withdraw"],
                "competence_refs": [competence.ref], "independence_group": actor,
                "can_supersede_others": False, "state": "active", "valid_from": START, "valid_until": END,
                "issuer_ref": "fixture:trusted-operator-grant",
            }))
        self.context = SubjectContext(self.subject, "bibliographic_assertion", "low", ("de",), "extractor", "research", access_allowed=True)

    def engine(self):
        return AssessmentEngine(ROOT, self.policy, self.authorities, self.competencies, self.records)

    def review(self, index=0, *, decision="admit", profile="source-observation", name=None):
        actor = self.authorities[index].payload["actor_id"]
        payload = {
            "schema_version": "tos_knowledge_assessment_v1",
            "assessment_id": name or f"tos.review.{actor}", "subject": self.subject.ref,
            "policy": self.policy.ref, "profile_id": profile,
            "authority": self.authorities[index].ref, "competence": self.competencies[index].ref,
            "reviewer": {"actor_id": actor, "kind": "agent"}, "decision": decision,
            "rationale": "Синтетическая проверка механики; не реальное содержательное review.",
            "language": "ru", "evidence": [{"record": self.source.ref, "stance": "supports", "locator": "fixture paragraph A"}],
            "counterevidence_search": {"status": "searched", "note": "Синтетическая проверка поля, не реальный поиск."},
            "limits": ["synthetic fixture only"] if decision == "admit-with-limits" else [],
            "method": {"procedure_ref": "fixture:source-check", "invocation_ref": "fixture:no-real-invocation", "model_ref": "fixture:not-a-real-model", "execution_profile": self.executor.ref},
            "issued_at": NOW, "supersedes": [],
        }
        return Submission(payload, actor, self.executor)

    def run_reviews(self, *reviews, context=None, now=NOW):
        return self.engine().evaluate(context or self.context, reviews, now=now)

    def test_agent_admission_without_human_review_and_without_source_mutation(self):
        review = self.review()
        before = copy.deepcopy(review.assessment)
        result = self.run_reviews(review)
        self.assertEqual(result["status"], "admitted")
        self.assertTrue(result["can_use"])
        self.assertEqual(result["reviewer_kinds"], ["agent"])
        self.assertEqual(review.assessment, before)
        self.assertFalse(result["is_semantic_evaluation"])

    def test_every_exact_dependency_is_bound(self):
        for key in ("subject", "policy", "authority", "competence"):
            for dimension, value in (("id", "unrelated"), ("version", 2), ("digest", "sha256:" + "0" * 64)):
                with self.subTest(key=key, dimension=dimension):
                    review = self.review()
                    review.assessment[key][dimension] = value
                    self.assertFalse(self.run_reviews(review)["can_use"])
        for key in ("record",):
            review = self.review()
            review.assessment["evidence"][0][key]["digest"] = "sha256:" + "0" * 64
            self.assertFalse(self.run_reviews(review)["can_use"])

    def test_authenticated_principal_cannot_be_replaced_by_claimed_reviewer(self):
        review = replace(self.review(), principal_id="intruder")
        self.assertIn("reviewer.authentication", self.run_reviews(review)["invalid_assessments"][0]["reasons"])
        review = self.review()
        review.assessment["reviewer"]["kind"] = "human"
        self.assertFalse(self.run_reviews(review)["can_use"])

    def test_payload_cannot_grant_itself_authority_or_lower_target_scope(self):
        for field, value in (("authority_grant", self.authorities[0].payload), ("risk", "low"), ("authenticated", True)):
            review = self.review()
            review.assessment[field] = value
            self.assertFalse(self.run_reviews(review)["can_use"])
        for context in (
            replace(self.context, risk="high"), replace(self.context, assertion_layer="lived_witness"),
            replace(self.context, requested_use="canon"), replace(self.context, languages=("ja",)),
            replace(self.context, access_allowed=False), replace(self.context, maker_id="assessor-a"),
        ):
            with self.subTest(context=context):
                self.assertFalse(self.run_reviews(self.review(), context=context)["can_use"])

    def test_usable_research_does_not_require_canon_or_publication_admission(self):
        for use in ("canon", "publication", "public-research"):
            self.assertFalse(self.run_reviews(self.review(), context=replace(self.context, requested_use=use))["can_use"])
        self.assertTrue(self.run_reviews(self.review())["can_use"])

    def test_competence_and_grant_expiry_or_revocation_invalidate_admission(self):
        review = self.review()
        self.assertFalse(self.run_reviews(review, now=END)["can_use"])
        for collection, state in ((self.authorities, "revoked"), (self.competencies, "revoked")):
            old = collection[0]
            payload = old.payload
            payload["state"] = state
            version_key = "authority_version" if collection is self.authorities else "competence_version"
            payload[version_key] = old.version + 1
            collection[0] = Record.from_payload(old.id, old.version + 1, payload)
            self.assertFalse(self.run_reviews(review)["can_use"])
            collection[0] = old

    def test_executor_substitution_does_not_inherit_competence(self):
        other = Record.from_payload(self.executor.id, 2, {"procedure_ref": "fixture:source-check", "model_ref": "fixture:other-model"})
        self.assertFalse(self.run_reviews(replace(self.review(), execution_profile=other))["can_use"])
        review = self.review()
        review.assessment["method"]["model_ref"] = "fixture:other-model"
        self.assertFalse(self.run_reviews(review)["can_use"])

    def test_current_subject_and_source_changes_make_old_assessment_stale(self):
        review = self.review()
        for index in (0, 1, 3, 4):
            old = self.records[index]
            self.records[index] = Record.from_payload(old.id, old.version + 1, {"changed": True})
            self.assertFalse(self.run_reviews(review)["can_use"])
            self.records[index] = old

    def test_positive_judgment_requires_support_and_counterevidence_search(self):
        for mutation in ("no_evidence", "context_only", "not_searched"):
            review = self.review()
            if mutation == "no_evidence": review.assessment["evidence"] = []
            if mutation == "context_only": review.assessment["evidence"][0]["stance"] = "context"
            if mutation == "not_searched": review.assessment["counterevidence_search"]["status"] = "not-searched"
            self.assertFalse(self.run_reviews(review)["can_use"])
        review = self.review(decision="defer")
        review.assessment["evidence"] = []
        review.assessment["counterevidence_search"]["status"] = "not-searched"
        self.assertEqual(self.run_reviews(review)["status"], "deferred")

    def test_conflicting_qualified_reviews_are_not_majority_erased(self):
        reviews = [self.review(), self.review(1, decision="reject"), self.review(name="tos.review.repeat-a")]
        results = [self.run_reviews(*order) for order in itertools.permutations(reviews)]
        self.assertTrue(all(result == results[0] for result in results))
        self.assertEqual(results[0]["status"], "disputed")
        self.assertFalse(results[0]["can_use"])
        self.assertEqual(len(results[0]["assessment_refs"]), 3)

    def test_replay_is_idempotent_but_identity_collision_fails_closed(self):
        review = self.review()
        self.assertEqual(self.run_reviews(review), self.run_reviews(review, review))
        altered = copy.deepcopy(review)
        altered.assessment["decision"] = "reject"
        self.assertFalse(self.run_reviews(review, altered)["can_use"])
        self.assertIn("assessment.identity-collision", self.run_reviews(review, altered)["invalid_assessments"][0]["reasons"])

    def test_independence_uses_trusted_groups_not_actor_names_or_call_count(self):
        context = replace(self.context, risk="high")
        a = self.review(profile="high-consequence")
        b = self.review(1, profile="high-consequence")
        self.assertTrue(self.run_reviews(a, b, context=context)["can_use"])
        self.assertFalse(self.run_reviews(a, self.review(profile="high-consequence", name="tos.review.repeat"), context=context)["can_use"])
        old = self.authorities[1]
        payload = old.payload
        payload["independence_group"] = "assessor-a"
        self.authorities[1] = Record.from_payload(old.id, old.version, payload)
        b = self.review(1, profile="high-consequence")
        self.assertFalse(self.run_reviews(a, b, context=context)["can_use"])

    def test_aliases_and_copies_do_not_multiply_supporting_origins(self):
        context = replace(self.context, assertion_layer="identity_assertion", risk="high")
        alias = Record.from_payload("tos.file.fixture-alias", 1, {"wrapped": self.source.payload}, origin_id="source-a")
        self.records.append(alias)
        reviews = [self.review(index, profile="identity") for index in (0, 1)]
        for review in reviews:
            review.assessment["evidence"].append({"record": alias.ref, "stance": "supports", "locator": "same source"})
        self.assertFalse(self.run_reviews(*reviews, context=context)["can_use"])
        for review in reviews:
            review.assessment["evidence"].append({"record": self.source_b.ref, "stance": "supports", "locator": "source B"})
        self.assertTrue(self.run_reviews(*reviews, context=context)["can_use"])

    def test_scope_limits_survive_admission(self):
        result = self.run_reviews(self.review(decision="admit-with-limits"))
        self.assertEqual(result["status"], "admitted-with-limits")
        self.assertEqual(result["limits"], ["synthetic fixture only"])

    def test_one_actor_with_multiple_grants_cannot_fill_multiple_independent_seats(self):
        policy = self.policy.payload
        profile = next(p for p in policy['profiles'] if p['profile_id'] == 'high-consequence')
        profile['min_reviewers'] = 3
        profile['min_independence_groups'] = 3
        self.policy = Record.from_payload(self.policy.id, self.policy.version, policy)
        # A may use groups X/Y/Z, while B and C only use X. There are three
        # actors and three group names, but at most two independent seats.
        reviews = []
        for actor, groups in (('assessor-a', ('x', 'y', 'z')), ('assessor-b', ('x',)), ('assessor-c', ('x',))):
            competence = self.competencies[0].payload
            competence['actor_id'] = actor
            competence['competence_id'] = 'tos.competence.' + actor
            calibrated = Record.from_payload(competence['competence_id'], 1, competence)
            self.competencies = [item for item in self.competencies if item.id != calibrated.id] + [calibrated]
            for group in groups:
                grant = self.authorities[0].payload
                grant.update(authority_id=f'tos.authority.{actor}-{group}', actor_id=actor,
                             policy=self.policy.ref, independence_group=group, competence_refs=[calibrated.ref])
                authority = Record.from_payload(grant['authority_id'], 1, grant)
                self.authorities.append(authority)
                review = self.review(profile='high-consequence', name=f'tos.review.{actor}-{group}')
                review.assessment.update(authority=authority.ref, competence=calibrated.ref,
                                         reviewer={'actor_id': actor, 'kind': 'agent'})
                reviews.append(replace(review, principal_id=actor))
        result = self.run_reviews(*reviews, context=replace(self.context, risk='high'))
        self.assertFalse(result['invalid_assessments'])
        self.assertFalse(result['can_use'])

    def test_equal_source_bytes_do_not_become_independent_by_changing_origin_name(self):
        copied = Record.from_payload('tos.file.relabelled-copy', 1, self.source.payload, origin_id='invented-other-origin')
        self.records.append(copied)
        reviews = [self.review(index, profile='identity') for index in (0, 1)]
        for review in reviews:
            review.assessment['evidence'].append({'record': copied.ref, 'stance': 'supports', 'locator': 'copy'})
        self.assertFalse(self.run_reviews(*reviews, context=replace(self.context, assertion_layer='identity_assertion', risk='high'))['can_use'])

    def test_access_context_requires_an_actual_boolean_permission(self):
        for value in ('false', 1, [], {}, None):
            self.assertFalse(self.run_reviews(self.review(), context=replace(self.context, access_allowed=value))['can_use'])

    def test_record_payload_access_cannot_mutate_exact_identity(self):
        before = self.source.ref
        self.source.payload['text'] = 'silently changed'
        self.assertEqual(self.source.ref, before)
        for version in (0, -1, True, 1.5):
            with self.assertRaises(ValueError):
                Record.from_payload('tos.fixture', version, {})
        with self.assertRaises(ValueError):
            Record.from_payload('tos.fixture', 1, {'number': float('nan')})

    def test_supersession_and_withdrawal_preserve_history_without_resurrection(self):
        a = self.review()
        b = self.review(decision="reject", name="tos.review.correction")
        b.assessment["supersedes"] = [Record.from_payload(a.assessment["assessment_id"], 1, a.assessment).ref]
        self.assertEqual(self.run_reviews(a, b)["status"], "rejected")
        c = self.review(decision="withdraw", name="tos.review.withdrawal")
        c.assessment["supersedes"] = [Record.from_payload(b.assessment["assessment_id"], 1, b.assessment).ref]
        result = self.run_reviews(c, a, b)
        self.assertFalse(result["can_use"])
        self.assertEqual(len(result["superseded_assessment_refs"]), 2)
        self.assertEqual(result["status"], "unreviewed")

    def test_invalid_or_unauthorized_successor_does_not_suppress_valid_review(self):
        a = self.review()
        b = self.review(1, decision="reject", name="tos.review.other-rejection")
        b.assessment["supersedes"] = [Record.from_payload(a.assessment["assessment_id"], 1, a.assessment).ref]
        self.assertTrue(self.run_reviews(a, b)["can_use"])
        b.assessment["authority"]["digest"] = "sha256:" + "0" * 64
        self.assertTrue(self.run_reviews(a, b)["can_use"])

    def test_future_judgment_and_unregistered_profile_are_not_admitted(self):
        review = self.review()
        review.assessment["issued_at"] = "2027-01-01T00:00:00Z"
        self.assertFalse(self.run_reviews(review)["can_use"])
        self.assertFalse(self.run_reviews(self.review(profile="invented-profile"))["can_use"])

    def test_committed_supersession_survives_later_authority_revocation(self):
        a = self.review()
        grant = self.authorities[1].payload
        grant['can_supersede_others'] = True
        self.authorities[1] = Record.from_payload(self.authorities[1].id, 1, grant)
        b = self.review(1, decision='reject')
        b.assessment['supersedes'] = [Record.from_payload(a.assessment['assessment_id'], 1, a.assessment).ref]
        self.assertEqual(self.run_reviews(a, b)['status'], 'rejected')
        # These are trusted committed events, not user-submitted claims that a
        # review was valid in the past. Revocation cannot undo historical edges.
        grant['state'] = 'revoked'
        grant['authority_version'] = 2
        self.authorities[1] = Record.from_payload(self.authorities[1].id, 2, grant)
        result = self.engine().evaluate(self.context, (), now=NOW, trusted_history=(a, b))
        self.assertFalse(result['can_use'])
        self.assertIn(a.assessment['assessment_id'], [ref['id'] for ref in result['superseded_assessment_refs']])

    def test_new_assessment_can_correct_a_committed_but_currently_stale_review(self):
        a = self.review()
        old = self.authorities[0].payload
        old['authority_version'] = 2
        self.authorities[0] = Record.from_payload(self.authorities[0].id, 2, old)
        b = self.review(decision='reject', name='tos.review.new-grant')
        b.assessment['supersedes'] = [Record.from_payload(a.assessment['assessment_id'], 1, a.assessment).ref]
        result = self.engine().evaluate(self.context, (b,), now=NOW, trusted_history=(a,))
        self.assertEqual(result['status'], 'rejected')
        self.assertNotIn(b.assessment['assessment_id'], [row['assessment_id'] for row in result['invalid_assessments']])

    def make_journal(self):
        from assessment_journal import AssessmentJournal
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        return AssessmentJournal(Path(temporary.name) / 'owner-assessments')

    def test_journal_restart_and_exact_command_replay(self):
        from assessment_journal import AssessmentJournal, JournalConflict
        journal = self.make_journal()
        args = dict(command_id='review-one', expected_revision=None, now=NOW)
        receipt = journal.append(self.engine(), self.context, [self.review()], **args)
        self.assertTrue(receipt['current_admission']['can_use'])
        reopened = AssessmentJournal(journal.directory)
        self.assertEqual(reopened.inspect(self.engine(), self.context, now=NOW)['revision'], receipt['revision'])
        repeated = reopened.append(self.engine(), self.context, [self.review()], **args)
        self.assertTrue(repeated['replayed'])
        self.assertEqual(repeated['revision'], receipt['revision'])
        self.assertEqual(reopened.inspect(self.engine(), self.context, now=NOW)['batch_count'], 1)
        with self.assertRaises(JournalConflict):
            reopened.append(self.engine(), self.context, [self.review(decision='reject')], **args)

    def test_journal_stale_writer_and_mixed_invalid_batch_have_no_partial_effect(self):
        from assessment_journal import JournalConflict, AssessmentRejected
        journal = self.make_journal()
        first = journal.append(self.engine(), self.context, [self.review()], command_id='one', expected_revision=None, now=NOW)
        with self.assertRaises(JournalConflict):
            journal.append(self.engine(), self.context, [self.review(1)], command_id='two', expected_revision=None, now=NOW)
        invalid = replace(self.review(name='tos.review.invalid'), principal_id='intruder')
        with self.assertRaises(AssessmentRejected):
            journal.append(self.engine(), self.context, [self.review(1), invalid], command_id='two', expected_revision=first['revision'], now=NOW)
        self.assertEqual(journal.inspect(self.engine(), self.context, now=NOW)['revision'], first['revision'])
        self.assertEqual(journal.inspect(self.engine(), self.context, now=NOW)['batch_count'], 1)

    def test_journal_publication_failure_can_be_retried_before_or_after_commit(self):
        from unittest.mock import patch
        for after_publish in (False, True):
            journal = self.make_journal()
            publish = journal._publish_head
            def interrupted(home, revision):
                if after_publish:
                    publish(home, revision)
                raise OSError('simulated interruption around atomic publication')
            args = dict(command_id='one', expected_revision=None, now=NOW)
            with patch.object(journal, '_publish_head', side_effect=interrupted):
                with self.assertRaises(OSError):
                    journal.append(self.engine(), self.context, [self.review()], **args)
            before = journal.inspect(self.engine(), self.context, now=NOW)
            self.assertEqual(before['batch_count'], int(after_publish))
            result = journal.append(self.engine(), self.context, [self.review()], **args)
            self.assertEqual(result['replayed'], after_publish)
            self.assertEqual(journal.inspect(self.engine(), self.context, now=NOW)['batch_count'], 1)

    def test_journal_historical_receipt_is_not_current_admission(self):
        journal = self.make_journal()
        review = self.review()
        args = dict(command_id='one', expected_revision=None)
        first = journal.append(self.engine(), self.context, [review], now=NOW, **args)
        repeat = journal.append(self.engine(), self.context, [review], now=END, **args)
        self.assertTrue(first['current_admission']['can_use'])
        self.assertTrue(repeat['receipt']['admission_at_commit']['can_use'])
        self.assertFalse(repeat['current_admission']['can_use'])
        self.assertEqual(first['revision'], repeat['revision'])

    def test_journal_restart_keeps_withdrawn_supersession_history(self):
        from assessment_journal import AssessmentJournal
        journal = self.make_journal()
        a = self.review()
        first = journal.append(self.engine(), self.context, [a], command_id='one', expected_revision=None, now=NOW)
        b = self.review(decision='reject', name='tos.review.correction')
        b.assessment['supersedes'] = [Record.from_payload(a.assessment['assessment_id'], 1, a.assessment).ref]
        second = journal.append(self.engine(), self.context, [b], command_id='two', expected_revision=first['revision'], now=NOW)
        c = self.review(decision='withdraw', name='tos.review.withdraw')
        c.assessment['supersedes'] = [Record.from_payload(b.assessment['assessment_id'], 1, b.assessment).ref]
        journal.append(self.engine(), self.context, [c], command_id='three', expected_revision=second['revision'], now=NOW)
        reopened = AssessmentJournal(journal.directory)
        result = reopened.inspect(self.engine(), self.context, now=NOW)
        self.assertEqual(result['batch_count'], 3)
        self.assertEqual(result['current_admission']['status'], 'unreviewed')
        self.assertEqual(len(result['current_admission']['superseded_assessment_refs']), 2)

    def test_journal_duplicate_events_are_not_duplicate_assessments(self):
        journal = self.make_journal()
        a = self.review()
        first = journal.append(self.engine(), self.context, [a, a], command_id='one', expected_revision=None, now=NOW)
        self.assertEqual(journal.inspect(self.engine(), self.context, now=NOW)['batch_count'], 1)
        second = journal.append(self.engine(), self.context, [a, self.review(1)], command_id='two', expected_revision=first['revision'], now=NOW)
        result = journal.inspect(self.engine(), self.context, now=NOW)
        self.assertEqual(result['revision'], second['revision'])
        self.assertEqual(len(result['current_admission']['assessment_refs']), 2)

    def test_journal_concurrent_writers_publish_only_one_expected_head(self):
        from concurrent.futures import ThreadPoolExecutor
        from threading import Barrier
        from assessment_journal import JournalConflict
        journal = self.make_journal()
        barrier = Barrier(2)
        def write(index):
            barrier.wait(timeout=5)
            try:
                return journal.append(self.engine(), self.context, [self.review(index)],
                                      command_id=f'writer-{index}', expected_revision=None, now=NOW)
            except JournalConflict:
                return None
        with ThreadPoolExecutor(max_workers=2) as pool:
            results = list(pool.map(write, (0, 1)))
        self.assertEqual(sum(result is not None for result in results), 1)
        self.assertEqual(journal.inspect(self.engine(), self.context, now=NOW)['batch_count'], 1)

    def test_journal_corrupt_or_missing_committed_record_is_not_skipped(self):
        from assessment_journal import JournalCorruption
        for corruption in ('changed-body', 'missing-body', 'bad-head'):
            journal = self.make_journal()
            result = journal.append(self.engine(), self.context, [self.review()],
                                    command_id='one', expected_revision=None, now=NOW)
            home = journal._home(self.subject.id)
            body = home / (result['revision'] + '.json')
            if corruption == 'changed-body':
                body.write_text('{}', encoding='utf-8')
            elif corruption == 'missing-body':
                body.rename(home / 'retained-damaged-batch.json')
            else:
                (home / 'head').write_text('broken', encoding='ascii')
            with self.assertRaises(JournalCorruption):
                journal.inspect(self.engine(), self.context, now=NOW)

    def test_journal_denied_access_does_not_expose_historical_receipt(self):
        journal = self.make_journal()
        args = dict(command_id='one', expected_revision=None, now=NOW)
        journal.append(self.engine(), self.context, [self.review()], **args)
        with self.assertRaises(PermissionError):
            journal.append(self.engine(), replace(self.context, access_allowed=False), [self.review()], **args)
        with self.assertRaises(PermissionError):
            journal.inspect(self.engine(), replace(self.context, access_allowed=False), now=NOW)

    def test_journal_lock_wait_is_bounded_and_does_not_publish(self):
        from assessment_journal import AssessmentJournal, JournalBusy
        journal = self.make_journal()
        contender = AssessmentJournal(journal.directory, lock_timeout_seconds=0)
        with journal._locked(journal._home(self.subject.id)):
            with self.assertRaises(JournalBusy):
                contender.append(self.engine(), self.context, [self.review()],
                                 command_id='busy', expected_revision=None, now=NOW)
        self.assertEqual(journal.inspect(self.engine(), self.context, now=NOW)['batch_count'], 0)


if __name__ == "__main__":
    unittest.main()
