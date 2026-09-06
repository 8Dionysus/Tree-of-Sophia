"""Pure, bounded application of ToS assessment policy to trusted owner inputs.

This module neither authenticates a caller nor judges source prose. An owner
adapter must supply authenticated bindings, admitted policy/grants/competence,
and the current, access-filtered record snapshot. Incoming assessments cannot
populate those trusted inputs. No network, model call, publication or write is
performed here.
"""
from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass
from datetime import datetime, timezone
from functools import lru_cache
import hashlib
import json
from pathlib import Path
from typing import Any, Iterable, Mapping, Protocol, Sequence

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource


MAX_ASSESSMENTS = 1024
MAX_RECORD_BYTES = 1_048_576
POSITIVE = frozenset({"admit", "admit-with-limits"})


def _canonical(payload: Any) -> bytes:
    return json.dumps(payload, ensure_ascii=False, allow_nan=False,
                      sort_keys=True, separators=(",", ":")).encode("utf-8")


@dataclass(frozen=True)
class Record:
    """Immutable JSON record; payload access never hands out owned mutable data."""

    id: str
    version: int
    _bytes: bytes
    origin_id: str | None = None

    @classmethod
    def from_payload(cls, id: str, version: int, payload: Mapping[str, Any],
                     *, origin_id: str | None = None) -> Record:
        if not isinstance(id, str) or not id.strip():
            raise ValueError("record.id must be nonempty")
        if type(version) is not int or version < 1:
            raise ValueError("record.version must be a positive integer")
        if not isinstance(payload, dict):
            raise ValueError("record.payload must be a JSON object")
        encoded = _canonical(payload)
        if len(encoded) > MAX_RECORD_BYTES:
            raise ValueError("record exceeds bounded assessment input size")
        if origin_id is not None and (not isinstance(origin_id, str) or not origin_id.strip()):
            raise ValueError("record.origin_id must be nonempty when present")
        return cls(id, version, encoded, origin_id)

    @property
    def payload(self) -> dict[str, Any]:
        return json.loads(self._bytes)

    @property
    def size_bytes(self) -> int:
        return len(self._bytes)

    @property
    def ref(self) -> dict[str, Any]:
        return {"id": self.id, "version": self.version,
                "digest": "sha256:" + hashlib.sha256(self._bytes).hexdigest()}


@dataclass(frozen=True)
class SubjectContext:
    record: Record
    assertion_layer: str
    risk: str
    languages: tuple[str, ...]
    maker_id: str
    requested_use: str
    access_allowed: bool = False


class ExecutionBinding(Protocol):
    @property
    def ref(self) -> dict[str, Any]: ...


@dataclass(frozen=True)
class Submission:
    assessment: dict[str, Any]
    principal_id: str
    execution_profile: ExecutionBinding


def _instant(value: str) -> datetime:
    instant = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if instant.tzinfo is None:
        raise ValueError("an explicit timezone is required")
    return instant.astimezone(timezone.utc)


@lru_cache(maxsize=8)
def _validators(root: Path) -> dict[str, Draft202012Validator]:
    schemas = {}
    for suffix in ("", "-policy", "-authority", "-competence", "-batch"):
        path = root / f"ToS/contracts/knowledge-assessment{suffix}.schema.json"
        schema = json.loads(path.read_text(encoding="utf-8"))
        Draft202012Validator.check_schema(schema)
        schemas[suffix or "-assessment"] = schema
    registry = Registry().with_resources(
        (schema["$id"], Resource.from_contents(schema)) for schema in schemas.values())
    return {name: Draft202012Validator(schema, registry=registry, format_checker=FormatChecker())
            for name, schema in schemas.items()}


def _index(records: Iterable[Record]) -> dict[str, Record]:
    result: dict[str, Record] = {}
    for record in records:
        if record.id in result and result[record.id] != record:
            raise ValueError(f"snapshot has conflicting current records: {record.id}")
        result[record.id] = record
    return result


def _resolve(ref: dict[str, Any], records: Mapping[str, Record]) -> Record | None:
    record = records.get(ref["id"])
    return record if record is not None and record.ref == ref else None


def _languages_fit(required: Iterable[str], allowed: Sequence[str]) -> bool:
    accepted = {item.casefold() for item in allowed}
    return "*" in accepted or all(item.casefold() in accepted for item in required)


def _support_count(records: Sequence[Record]) -> int:
    """Count provenance components, joining both shared origins and equal bytes."""
    components: list[set[str]] = []
    for record in records:
        if record.origin_id is None:
            continue
        keys = {"origin:" + record.origin_id, "bytes:" + record.ref["digest"]}
        separate = []
        for component in components:
            if component & keys:
                keys |= component
            else:
                separate.append(component)
        components = separate + [keys]
    return len(components)


def _independent_seats(options: Mapping[str, set[str]]) -> int:
    """Maximum actor/group matching: an actor cannot fill two independent seats.

    Separate actor and group counts are insufficient when one actor has several
    grants. Iterative augmenting paths avoid a recursion limit on long histories.
    """
    actor_to_group: dict[str, str] = {}
    group_to_actor: dict[str, str] = {}
    for actor in sorted(options):
        queue = [actor]
        seen = {actor}
        parent: dict[str, str] = {}
        found = False
        for current in queue:
            for group in sorted(options[current]):
                if group in parent:
                    continue
                parent[group] = current
                if group not in group_to_actor:
                    step: str | None = group
                    while step is not None:
                        owner = parent[step]
                        previous = actor_to_group.get(owner)
                        actor_to_group[owner] = step
                        group_to_actor[step] = owner
                        step = previous
                    found = True
                    break
                next_actor = group_to_actor[group]
                if next_actor not in seen:
                    seen.add(next_actor)
                    queue.append(next_actor)
            if found:
                break
    return len(actor_to_group)


class AssessmentEngine:
    def __init__(self, root: Path, policy: Record, authorities: Sequence[Record],
                 competencies: Sequence[Record], records: Sequence[Record]):
        self.validators = _validators(root.resolve())
        self.policy = policy
        self.authorities = _index(authorities)
        self.competencies = _index(competencies)
        self.records = _index([*records, policy, *authorities, *competencies])
        for name, collection in (("policy", [policy]), ("authority", authorities),
                                 ("competence", competencies)):
            for record in collection:
                payload = record.payload
                self.validators["-" + name].validate(payload)
                if payload[name + "_id"] != record.id or payload[name + "_version"] != record.version:
                    raise ValueError(f"{name} envelope disagrees with its owned record")
                if name != "policy" and _instant(payload["valid_from"]) >= _instant(payload["valid_until"]):
                    raise ValueError(f"{name} has empty or reversed validity interval")
        self.profiles = {}
        for profile in policy.payload["profiles"]:
            if profile["profile_id"] in self.profiles:
                raise ValueError("duplicate assessment profile")
            if profile["min_independence_groups"] > profile["min_reviewers"]:
                raise ValueError("independence groups exceed required reviewers")
            self.profiles[profile["profile_id"]] = profile

    def _qualify(self, submission: Submission, context: SubjectContext,
                 now: datetime) -> list[str]:
        assessment = submission.assessment
        if not self.validators["-assessment"].is_valid(assessment):
            return ["assessment.schema"]
        reasons = []
        if assessment["subject"] != context.record.ref or _resolve(assessment["subject"], self.records) is None:
            reasons.append("subject.stale-or-mismatched")
        if assessment["policy"] != self.policy.ref:
            reasons.append("policy.stale-or-mismatched")
        if submission.principal_id != assessment["reviewer"]["actor_id"]:
            reasons.append("reviewer.authentication")
        if context.access_allowed is not True:
            reasons.append("subject.access-denied")
        if not context.languages or not context.maker_id:
            reasons.append("subject.scope-incomplete")
        issued = _instant(assessment["issued_at"])
        if issued > now:
            reasons.append("assessment.future")
        profile = self.profiles.get(assessment["profile_id"])
        grant = _resolve(assessment["authority"], self.authorities)
        competence = _resolve(assessment["competence"], self.competencies)
        if profile is None:
            reasons.append("profile.unregistered")
        if grant is None:
            reasons.append("authority.stale-or-missing")
        if competence is None:
            reasons.append("competence.stale-or-missing")
        if profile is None or grant is None or competence is None:
            return sorted(set(reasons))
        authority, calibration = grant.payload, competence.payload
        languages = (*context.languages, assessment["language"])
        reviewer = assessment["reviewer"]
        if (context.assertion_layer not in profile["assertion_layers"]
                or context.risk not in profile["risk_tiers"]
                or context.requested_use not in profile["uses"]
                or reviewer["kind"] not in profile["reviewer_kinds"]
                or not _languages_fit(languages, profile["languages"])):
            reasons.append("profile.outside-scope")
        if not profile["allow_self_review"] and context.maker_id == reviewer["actor_id"]:
            reasons.append("reviewer.self-review")
        if (authority["actor_id"] != reviewer["actor_id"]
                or authority["actor_kind"] != reviewer["kind"]
                or authority["policy"] != self.policy.ref
                or assessment["competence"] not in authority["competence_refs"]
                or assessment["decision"] not in authority["decisions"]
                or context.requested_use not in authority["uses"]
                or not any(context.record.id.startswith(prefix) for prefix in authority["subject_prefixes"])):
            reasons.append("authority.binding-or-scope")
        for prefix, record, expected_state in (("authority", authority, "active"),
                                               ("competence", calibration, "verified")):
            if record["state"] != expected_state:
                reasons.append(prefix + ".inactive")
            start, end = _instant(record["valid_from"]), _instant(record["valid_until"])
            if not start <= issued <= now < end:
                reasons.append(prefix + ".outside-validity")
            if (record["actor_id"] != reviewer["actor_id"]
                    or assessment["profile_id"] not in record["profile_ids"]
                    or context.assertion_layer not in record["assertion_layers"]
                    or not _languages_fit(languages, record["languages"])):
                reasons.append(prefix + ".outside-scope")
        if any(_resolve(ref, self.records) is None for ref in calibration["evidence_refs"]):
            reasons.append("competence.evidence-stale")
        method = assessment["method"]
        executor = submission.execution_profile
        current_executor = _resolve(executor.ref, self.records)
        if (method["execution_profile"] != executor.ref
                or current_executor is None
                or executor.ref not in calibration["execution_profiles"]
                or method["procedure_ref"] != current_executor.payload.get("procedure_ref")
                or method["model_ref"] != current_executor.payload.get("model_ref")):
            reasons.append("method.unqualified-execution")
        supporting = []
        for evidence in assessment["evidence"]:
            record = _resolve(evidence["record"], self.records)
            if record is None:
                reasons.append("evidence.stale-or-missing")
            elif evidence["stance"] == "supports":
                if record.id == context.record.id:
                    reasons.append("evidence.circular-support")
                if record.origin_id is None:
                    reasons.append("evidence.origin-missing")
                supporting.append(record)
        if assessment["decision"] in POSITIVE:
            if _support_count(supporting) < profile["min_supporting_origins"]:
                reasons.append("evidence.insufficient-origins")
            if (profile["require_counterevidence_search"]
                    and assessment["counterevidence_search"]["status"] != "searched"):
                reasons.append("evidence.countersearch-required")
        return sorted(set(reasons))

    def evaluate(self, context: SubjectContext, reviews: Sequence[Submission],
                 *, now: str, trusted_history: Sequence[Submission] = ()) -> dict[str, Any]:
        """Recompute one bounded subject's current admission without state writes.

        Only the source-owner journal supplies trusted_history: events qualified
        at their original commit. Submitted judgments cannot declare their own
        historical validity. Current use is still requalified against current
        dependencies; committed supersession remains historical after revocation.
        Truncating history is not a pagination mechanism.
        """
        if len(reviews) + len(trusted_history) > MAX_ASSESSMENTS:
            raise ValueError("assessment work limit exceeded; do not truncate history")
        instant = _instant(now)
        history: dict[str, Submission] = {}
        refs: dict[str, dict[str, Any]] = {}
        for submission in trusted_history:
            assessment = submission.assessment
            self.validators['-assessment'].validate(assessment)
            if (assessment['subject']['id'] != context.record.id
                    or assessment['reviewer']['actor_id'] != submission.principal_id
                    or assessment['method']['execution_profile'] != submission.execution_profile.ref
                    or _instant(assessment['issued_at']) > instant):
                raise ValueError('trusted history has an inconsistent binding')
            identifier = assessment['assessment_id']
            ref = Record.from_payload(identifier, 1, assessment).ref
            if identifier in refs and refs[identifier] != ref:
                raise ValueError('trusted history has an identity collision')
            history[identifier], refs[identifier] = submission, ref
        permanent_superseded: set[str] = set()
        for submission in history.values():
            for ref in submission.assessment['supersedes']:
                if refs.get(ref['id']) != ref:
                    raise ValueError('trusted history is missing an exact supersession target')
                permanent_superseded.add(ref['id'])
        grouped: dict[str, list[Submission]] = defaultdict(list)
        for submission in (*trusted_history, *reviews):
            assessment_id = submission.assessment.get("assessment_id")
            if not isinstance(assessment_id, str):
                assessment_id = "<invalid-id>"
            grouped[assessment_id].append(submission)
        qualified: dict[str, Submission] = {}
        invalid: dict[str, list[str]] = {}
        for assessment_id, variants in sorted(grouped.items()):
            try:
                signatures = {_canonical({"assessment": item.assessment,
                                          "principal_id": item.principal_id,
                                          "execution_profile": item.execution_profile.ref}) for item in variants}
            except (TypeError, ValueError, UnicodeError):
                invalid[assessment_id] = ["assessment.schema"]
                continue
            if any(len(signature) > MAX_RECORD_BYTES for signature in signatures):
                invalid[assessment_id] = ["assessment.size-limit"]
                continue
            if len(signatures) != 1:
                invalid[assessment_id] = ["assessment.identity-collision"]
                continue
            submission = variants[0]
            reasons = self._qualify(submission, context, instant)
            if reasons:
                invalid[assessment_id] = reasons
            else:
                qualified[assessment_id] = submission
                refs[assessment_id] = Record.from_payload(assessment_id, 1, submission.assessment).ref

        # Validate exact same-subject supersession before it can suppress anything.
        # Cascading invalidity and cycles fail closed; incoming order is irrelevant.
        dependencies: dict[str, set[str]] = {}
        for assessment_id, submission in qualified.items():
            assessment = submission.assessment
            dependencies[assessment_id] = set()
            for ref in assessment["supersedes"]:
                target_id = ref["id"]
                previous = qualified.get(target_id) or history.get(target_id)
                if (previous is None or ref != refs.get(target_id)
                        or target_id == assessment_id
                        or previous.assessment['subject'] != assessment['subject']
                        or _instant(previous.assessment["issued_at"]) > _instant(assessment["issued_at"])):
                    invalid.setdefault(assessment_id, []).append("supersession.invalid-target")
                    continue
                authority = self.authorities[assessment["authority"]["id"]].payload
                if (previous.principal_id != submission.principal_id
                        and not authority["can_supersede_others"]):
                    invalid.setdefault(assessment_id, []).append("supersession.unauthorized")
                dependencies[assessment_id].add(target_id)
        remaining = set(qualified) - set(invalid)
        ordered: list[str] = []
        while remaining:
            invalid_children = {key for key in remaining if dependencies[key] & (set(invalid) - set(history))}
            for key in invalid_children:
                invalid[key] = ["supersession.invalid-target"]
            remaining -= invalid_children
            ready = sorted(key for key in remaining if not dependencies[key] & remaining)
            if not ready:
                for key in remaining:
                    invalid[key] = ["supersession.cycle-or-invalid-ancestor"]
                break
            ordered.extend(ready)
            remaining.difference_update(ready)
        superseded: set[str] = set(permanent_superseded)
        for key in ordered:
            superseded.update(dependencies[key])
        active = [qualified[key] for key in sorted(set(ordered) - superseded)]
        judgments = [item for item in active if item.assessment["decision"] != "withdraw"]
        positives = [item for item in judgments if item.assessment["decision"] in POSITIVE]
        negatives = [item for item in judgments if item.assessment["decision"] == "reject"]
        disputes = [item for item in judgments if item.assessment["decision"] == "dispute"]
        # Quorums never combine unrelated profiles into a weaker accidental rule.
        has_quorum = False
        for profile_id, profile in self.profiles.items():
            voters = [item for item in positives if item.assessment["profile_id"] == profile_id]
            actors = {item.principal_id for item in voters}
            groups: dict[str, set[str]] = defaultdict(set)
            for item in voters:
                groups[item.principal_id].add(self.authorities[item.assessment["authority"]["id"]].payload["independence_group"])
            if len(actors) >= profile["min_reviewers"] and _independent_seats(groups) >= profile["min_independence_groups"]:
                has_quorum = True
        limits = sorted({limit for item in judgments for limit in item.assessment["limits"]})
        if disputes or (positives and negatives):
            status = "disputed"
        elif negatives:
            status = "rejected"
        elif has_quorum:
            status = "admitted-with-limits" if limits else "admitted"
        elif judgments:
            status = "deferred"
        else:
            status = "unreviewed"
        return {
            "schema_version": "tos_knowledge_admission_v1",
            "subject": context.record.ref, "policy": self.policy.ref, "use": context.requested_use,
            "status": status, "can_use": status in {"admitted", "admitted-with-limits"},
            "is_semantic_evaluation": False,
            "reviewer_kinds": sorted({item.assessment["reviewer"]["kind"] for item in judgments}),
            "assessment_refs": [refs[item.assessment["assessment_id"]] for item in active],
            "superseded_assessment_refs": [refs[key] for key in sorted(superseded)],
            "invalid_assessments": [{"assessment_id": key, "reasons": sorted(set(reasons))}
                                    for key, reasons in sorted(invalid.items())],
            "limits": limits,
        }
