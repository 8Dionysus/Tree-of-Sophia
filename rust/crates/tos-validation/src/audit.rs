//! Full-cut validation orchestration. This is an internal engine component,
//! not a source-admission issuer: STO must still attest the input cut and CMD
//! must linearize current authority before any mechanical result is sealed.

use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

use tos_foundation::{Digest256, Digest256Hasher};

use crate::global_facts::{GlobalBudget, GlobalFact, GlobalFactStore, GlobalIssue, GlobalRefusal};
use crate::{PredicateRead, ValidationFact};

/// The source-derived, general-audit rows. A source operation may exclude a
/// row only through a separately reviewed owner profile, never by omission
/// from a caller-supplied module list.
pub(crate) const REQUIRED_GENERAL_ROWS: [&str; 14] = [
    "tos.val.source.member-shape.v1",
    "tos.val.source.profile-route.v1",
    "tos.val.source.identity-version.v1",
    "tos.val.source.bibliographic-links.v1",
    "tos.val.source.item-fixity.v1",
    "tos.val.source.rights-visibility.v1",
    "tos.val.source.provenance.v1",
    "tos.val.source.claim-closure.v1",
    "tos.val.source.bibliographic-topology.v1",
    "tos.val.source.artifact-representation.v1",
    "tos.val.source.text-layers.v1",
    "tos.val.source.transfer-research.v1",
    "tos.val.source.catalog-currentness.v1",
    "tos.val.source.retirement.v1",
];

/// One exact immutable member supplied by a cut reader. The path is a
/// source-relative member name, not authority to open a host filesystem path.
#[derive(Debug)]
pub(crate) struct AuditMember {
    pub path: String,
    pub raw: Vec<u8>,
}

/// The reader must bind this expectation to a sealed STO cut. The engine
/// independently recomputes the ordered membership transcript; merely
/// constructing these fields never proves that the reader was complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MembershipExpectation {
    pub count: u64,
    pub digest: Digest256,
}

pub(crate) trait MemberStream {
    /// A production implementation must honor this deadline while reading a
    /// sealed cut. A timestamp check cannot interrupt a blocked reader.
    fn next_member(&mut self, deadline: Instant) -> Result<Option<AuditMember>, String>;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AuditLimits {
    pub max_members: u64,
    pub max_member_bytes: usize,
    pub max_path_bytes: usize,
    pub max_total_bytes: u64,
    pub max_wall: Duration,
    pub max_issues: usize,
    pub max_issue_bytes: usize,
    pub max_reads: usize,
    pub max_read_bytes: usize,
    pub max_facts: usize,
    pub max_fact_bytes: usize,
    pub global: GlobalBudget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuditIssue {
    pub rule_id: &'static str,
    pub path: String,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AuditRefusal {
    MissingRules(Vec<&'static str>),
    DuplicateRule(&'static str),
    UnknownRule(&'static str),
    IncompleteMemberStream,
    UnorderedMemberPath,
    DuplicateMemberPath,
    MembershipMismatch,
    BudgetExceeded,
    DeadlineExceeded,
    RuleIndeterminate {
        rule_id: &'static str,
        reason: String,
    },
    RuleUnsupported {
        rule_id: &'static str,
        profile: String,
    },
    GlobalFacts(GlobalRefusal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AuditResult {
    /// No mechanical-valid or attestation constructor is exposed here.
    MechanicallyCleanProbe {
        membership: MembershipExpectation,
        reads: Vec<PredicateRead>,
        facts: Vec<ValidationFact>,
    },
    Invalid {
        membership: MembershipExpectation,
        issues: Vec<AuditIssue>,
    },
    Refused(AuditRefusal),
}

pub(crate) struct AuditSink {
    limits: AuditLimits,
    issues: Vec<AuditIssue>,
    issue_bytes: usize,
    reads: Vec<PredicateRead>,
    read_bytes: usize,
    facts: Vec<ValidationFact>,
    fact_bytes: usize,
    global: Option<GlobalFactStore>,
}

impl AuditSink {
    fn new(
        limits: AuditLimits,
        scratch_dir: Option<&Path>,
        attempt_id: &str,
    ) -> Result<Self, AuditRefusal> {
        Ok(Self {
            limits,
            issues: Vec::new(),
            issue_bytes: 0,
            reads: Vec::new(),
            read_bytes: 0,
            facts: Vec::new(),
            fact_bytes: 0,
            global: Some(
                GlobalFactStore::new(limits.global, scratch_dir, attempt_id)
                    .map_err(AuditRefusal::GlobalFacts)?,
            ),
        })
    }

    pub fn issue(&mut self, issue: AuditIssue) -> Result<(), AuditRefusal> {
        let size = string_bytes(&[issue.rule_id, &issue.path, &issue.code])?;
        if self.issues.len() >= self.limits.max_issues
            || self
                .issue_bytes
                .checked_add(size)
                .is_none_or(|next| next > self.limits.max_issue_bytes)
        {
            return Err(AuditRefusal::BudgetExceeded);
        }
        self.issue_bytes += size;
        self.issues.push(issue);
        Ok(())
    }

    pub fn read(&mut self, read: PredicateRead) -> Result<(), AuditRefusal> {
        let size = read_bytes(&read)?;
        if self.reads.len() >= self.limits.max_reads
            || self
                .read_bytes
                .checked_add(size)
                .is_none_or(|next| next > self.limits.max_read_bytes)
        {
            return Err(AuditRefusal::BudgetExceeded);
        }
        self.read_bytes += size;
        self.reads.push(read);
        Ok(())
    }

    pub fn fact(&mut self, fact: ValidationFact) -> Result<(), AuditRefusal> {
        let size = string_bytes(&[&fact.namespace, &fact.key, &fact.value_digest])?;
        if self.facts.len() >= self.limits.max_facts
            || self
                .fact_bytes
                .checked_add(size)
                .is_none_or(|next| next > self.limits.max_fact_bytes)
        {
            return Err(AuditRefusal::BudgetExceeded);
        }
        self.fact_bytes += size;
        self.facts.push(fact);
        Ok(())
    }

    pub fn global_fact(&mut self, fact: GlobalFact) -> Result<(), AuditRefusal> {
        self.global
            .as_mut()
            .expect("global store remains until all rules finish")
            .push(fact)
            .map_err(AuditRefusal::GlobalFacts)
    }
}

fn string_bytes(values: &[&str]) -> Result<usize, AuditRefusal> {
    values
        .iter()
        .try_fold(0usize, |total, value| total.checked_add(value.len()))
        .ok_or(AuditRefusal::BudgetExceeded)
}

fn read_bytes(read: &PredicateRead) -> Result<usize, AuditRefusal> {
    match read {
        PredicateRead::ExactRecord {
            id,
            version,
            digest,
        } => string_bytes(&[id, version, digest]),
        PredicateRead::ExactPath { path, digest } => string_bytes(&[path, digest]),
        PredicateRead::ExactBytes { locator, digest } => string_bytes(&[locator, digest]),
        PredicateRead::IdentityKey { namespace, key, .. }
        | PredicateRead::AbsentKey { namespace, key } => string_bytes(&[namespace, key]),
        PredicateRead::RefEndpoint {
            endpoint_type, id, ..
        } => string_bytes(&[endpoint_type, id]),
        PredicateRead::UniqueKey {
            namespace,
            key,
            owner,
        } => string_bytes(&[namespace, key, owner]),
        PredicateRead::Range {
            namespace,
            lower,
            upper,
            generation,
        } => string_bytes(&[namespace, lower, upper, generation]),
        PredicateRead::Prefix {
            namespace,
            prefix,
            generation,
        } => string_bytes(&[namespace, prefix, generation]),
        PredicateRead::ReverseRefs {
            target,
            relation,
            generation,
        } => string_bytes(&[target, relation, generation]),
        PredicateRead::Interval {
            scope, generation, ..
        } => string_bytes(&[scope, generation]),
        PredicateRead::SchemaResource { uri, digest } => string_bytes(&[uri, digest]),
        PredicateRead::Registry {
            uri,
            version,
            digest,
        } => string_bytes(&[uri, version, digest]),
    }
}

/// A code-owned rule module. Returning success certifies only that this module
/// processed the member and emitted every observation its own owner contract
/// requires. Rules must refuse unsupported profiles in `inspect` or `finish`.
pub(crate) trait AuditRule {
    fn row_id(&self) -> &'static str;
    fn inspect(&mut self, member: &AuditMember, sink: &mut AuditSink) -> Result<(), AuditRefusal>;
    fn finish(&mut self, sink: &mut AuditSink) -> Result<(), AuditRefusal>;
}

/// Domain-separated, framed membership transcript over the exact path and raw
/// SHA-256 of each member. The path order and duplicate check make this stable
/// across a streaming reader and prevent a missing/duplicated entry from
/// appearing as a successful enumeration.
fn feed_member(hasher: &mut Digest256Hasher, member: &AuditMember) {
    hasher.update(&(member.path.len() as u64).to_be_bytes());
    hasher.update(member.path.as_bytes());
    hasher.update(&(member.raw.len() as u64).to_be_bytes());
    hasher.update(Digest256::of_bytes(&member.raw).as_bytes());
}

/// Runs all registered general rows over one reader transcript. It does not
/// reopen members, skip failures, truncate issues, or turn a partial run into
/// a valid result. A future STO adapter must supply the authenticated cut.
pub(crate) fn run_full_probe(
    stream: &mut impl MemberStream,
    expected: MembershipExpectation,
    rules: &mut [&mut dyn AuditRule],
    limits: AuditLimits,
    scratch_dir: Option<&Path>,
    attempt_id: &str,
) -> AuditResult {
    let mut seen = BTreeSet::new();
    for rule in rules.iter() {
        let id = rule.row_id();
        if !REQUIRED_GENERAL_ROWS.contains(&id) {
            return AuditResult::Refused(AuditRefusal::UnknownRule(id));
        }
        if !seen.insert(id) {
            return AuditResult::Refused(AuditRefusal::DuplicateRule(id));
        }
    }
    let missing: Vec<_> = REQUIRED_GENERAL_ROWS
        .into_iter()
        .filter(|id| !seen.contains(id))
        .collect();
    if !missing.is_empty() {
        return AuditResult::Refused(AuditRefusal::MissingRules(missing));
    }

    if limits.max_wall.is_zero() {
        return AuditResult::Refused(AuditRefusal::BudgetExceeded);
    }
    let deadline = match Instant::now().checked_add(limits.max_wall) {
        Some(deadline) => deadline,
        None => return AuditResult::Refused(AuditRefusal::BudgetExceeded),
    };

    let mut sink = match AuditSink::new(limits, scratch_dir, attempt_id) {
        Ok(sink) => sink,
        Err(reason) => return AuditResult::Refused(reason),
    };
    let mut hasher = Digest256Hasher::new();
    hasher.update(b"tos-val-full-membership-v1\0");
    let mut count = 0u64;
    let mut total_bytes = 0u64;
    let mut previous_path = None::<String>;
    loop {
        if Instant::now() >= deadline {
            return AuditResult::Refused(AuditRefusal::DeadlineExceeded);
        }
        let member = match stream.next_member(deadline) {
            Ok(Some(member)) => member,
            Ok(None) => break,
            Err(_) => return AuditResult::Refused(AuditRefusal::IncompleteMemberStream),
        };
        if Instant::now() >= deadline {
            return AuditResult::Refused(AuditRefusal::DeadlineExceeded);
        }
        count = match count.checked_add(1) {
            Some(value) if value <= limits.max_members => value,
            _ => return AuditResult::Refused(AuditRefusal::BudgetExceeded),
        };
        total_bytes = match total_bytes.checked_add(member.raw.len() as u64) {
            Some(value) if value <= limits.max_total_bytes => value,
            _ => return AuditResult::Refused(AuditRefusal::BudgetExceeded),
        };
        if member.raw.len() > limits.max_member_bytes
            || member.path.len() > limits.max_path_bytes
            || member.path.is_empty()
            || member.path.starts_with('/')
            || member
                .path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return AuditResult::Refused(AuditRefusal::BudgetExceeded);
        }
        if let Some(previous) = &previous_path {
            if member.path == *previous {
                return AuditResult::Refused(AuditRefusal::DuplicateMemberPath);
            }
            if member.path < *previous {
                return AuditResult::Refused(AuditRefusal::UnorderedMemberPath);
            }
        }
        previous_path = Some(member.path.clone());
        feed_member(&mut hasher, &member);
        for rule in rules.iter_mut() {
            if Instant::now() >= deadline {
                return AuditResult::Refused(AuditRefusal::DeadlineExceeded);
            }
            if let Err(reason) = rule.inspect(&member, &mut sink) {
                return AuditResult::Refused(reason);
            }
            if Instant::now() >= deadline {
                return AuditResult::Refused(AuditRefusal::DeadlineExceeded);
            }
        }
    }
    let actual = MembershipExpectation {
        count,
        digest: hasher.finalize(),
    };
    if actual.count != expected.count || actual.digest != expected.digest {
        return AuditResult::Refused(AuditRefusal::MembershipMismatch);
    }
    for rule in rules.iter_mut() {
        if Instant::now() >= deadline {
            return AuditResult::Refused(AuditRefusal::DeadlineExceeded);
        }
        if let Err(reason) = rule.finish(&mut sink) {
            return AuditResult::Refused(reason);
        }
        if Instant::now() >= deadline {
            return AuditResult::Refused(AuditRefusal::DeadlineExceeded);
        }
    }
    let global_issues = match sink
        .global
        .take()
        .expect("global store available")
        .finish(deadline)
    {
        Ok(issues) => issues,
        Err(GlobalRefusal::DeadlineExceeded) => {
            return AuditResult::Refused(AuditRefusal::DeadlineExceeded);
        }
        Err(reason) => return AuditResult::Refused(AuditRefusal::GlobalFacts(reason)),
    };
    if Instant::now() >= deadline {
        return AuditResult::Refused(AuditRefusal::DeadlineExceeded);
    }
    for issue in global_issues {
        let (path, code) = match issue {
            GlobalIssue::DuplicateOwner { namespace, key } => {
                (key, format!("global.duplicate-owner:{namespace}"))
            }
            GlobalIssue::MissingTarget { namespace, key } => {
                (key, format!("global.missing-target:{namespace}"))
            }
            GlobalIssue::InvalidInterval { namespace, source } => {
                (source, format!("global.invalid-interval:{namespace}"))
            }
            GlobalIssue::OverlappingInterval {
                namespace,
                left,
                right,
            } => (right, format!("global.overlap:{namespace}:{left}")),
        };
        if sink
            .issue(AuditIssue {
                rule_id: "tos.val.global-facts.v1",
                path,
                code,
            })
            .is_err()
        {
            return AuditResult::Refused(AuditRefusal::BudgetExceeded);
        }
    }
    if sink.issues.is_empty() {
        AuditResult::MechanicallyCleanProbe {
            membership: actual,
            reads: sink.reads,
            facts: sink.facts,
        }
    } else {
        AuditResult::Invalid {
            membership: actual,
            issues: sink.issues,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct VecStream(Vec<Result<AuditMember, String>>);

    impl MemberStream for VecStream {
        fn next_member(&mut self, _deadline: Instant) -> Result<Option<AuditMember>, String> {
            if self.0.is_empty() {
                Ok(None)
            } else {
                self.0.remove(0).map(Some)
            }
        }
    }

    struct Rule(&'static str, bool);

    impl AuditRule for Rule {
        fn row_id(&self) -> &'static str {
            self.0
        }
        fn inspect(
            &mut self,
            _member: &AuditMember,
            sink: &mut AuditSink,
        ) -> Result<(), AuditRefusal> {
            if self.1 {
                for path in ["a.json", "b.json"] {
                    sink.global_fact(GlobalFact::Owner {
                        namespace: "source-id".into(),
                        key: "same-id".into(),
                        source: path.into(),
                    })?;
                }
            }
            Ok(())
        }
        fn finish(&mut self, _sink: &mut AuditSink) -> Result<(), AuditRefusal> {
            Ok(())
        }
    }

    fn limits() -> AuditLimits {
        AuditLimits {
            max_members: 2,
            max_member_bytes: 32,
            max_path_bytes: 64,
            max_total_bytes: 64,
            max_wall: Duration::from_secs(2),
            max_issues: 4,
            max_issue_bytes: 256,
            max_reads: 4,
            max_read_bytes: 256,
            max_facts: 4,
            max_fact_bytes: 256,
            global: GlobalBudget {
                max_facts: 8,
                max_memory_bytes: 1024,
                max_spill_bytes: 0,
                max_runs: 0,
                max_issues: 4,
                max_merge_head_bytes: 0,
                max_issue_bytes: 256,
            },
        }
    }

    fn expected(member: &AuditMember) -> MembershipExpectation {
        let mut hasher = Digest256Hasher::new();
        hasher.update(b"tos-val-full-membership-v1\0");
        feed_member(&mut hasher, member);
        MembershipExpectation {
            count: 1,
            digest: hasher.finalize(),
        }
    }

    #[test]
    fn missing_required_rule_refuses_before_reading_any_member() {
        let member = AuditMember {
            path: "a.json".into(),
            raw: b"{}".to_vec(),
        };
        let mut stream = VecStream(vec![Ok(member)]);
        let result = run_full_probe(
            &mut stream,
            MembershipExpectation {
                count: 0,
                digest: Digest256::of_bytes(b""),
            },
            &mut [],
            limits(),
            None,
            "missing",
        );
        assert!(
            matches!(result, AuditResult::Refused(AuditRefusal::MissingRules(ids)) if ids.len() == 14)
        );
        assert_eq!(stream.0.len(), 1);
    }

    #[test]
    fn complete_module_inventory_still_refuses_truncated_and_changed_cut() {
        let member = AuditMember {
            path: "a.json".into(),
            raw: b"{}".to_vec(),
        };
        let expected = expected(&member);
        let mut modules: Vec<Rule> = REQUIRED_GENERAL_ROWS
            .into_iter()
            .map(|id| Rule(id, false))
            .collect();
        let mut refs: Vec<&mut dyn AuditRule> = modules
            .iter_mut()
            .map(|rule| rule as &mut dyn AuditRule)
            .collect();
        let mut truncated = VecStream(vec![Ok(member), Err("cut read failed".into())]);
        assert_eq!(
            run_full_probe(
                &mut truncated,
                expected,
                &mut refs,
                limits(),
                None,
                "truncated"
            ),
            AuditResult::Refused(AuditRefusal::IncompleteMemberStream)
        );
        let mut empty = VecStream(vec![]);
        assert_eq!(
            run_full_probe(&mut empty, expected, &mut refs, limits(), None, "empty"),
            AuditResult::Refused(AuditRefusal::MembershipMismatch)
        );
    }

    #[test]
    fn full_cut_reports_global_duplicate_after_complete_member_scan() {
        let member = AuditMember {
            path: "a.json".into(),
            raw: b"{}".to_vec(),
        };
        let expected = expected(&member);
        let mut modules: Vec<Rule> = REQUIRED_GENERAL_ROWS
            .into_iter()
            .enumerate()
            .map(|(index, id)| Rule(id, index == 0))
            .collect();
        let mut refs: Vec<&mut dyn AuditRule> = modules
            .iter_mut()
            .map(|rule| rule as &mut dyn AuditRule)
            .collect();
        let mut stream = VecStream(vec![Ok(member)]);
        let result = run_full_probe(
            &mut stream,
            expected,
            &mut refs,
            limits(),
            None,
            "duplicate",
        );
        assert!(
            matches!(result, AuditResult::Invalid { issues, .. } if issues.iter().any(|issue| issue.code == "global.duplicate-owner:source-id"))
        );
    }

    #[test]
    fn issue_read_and_fact_byte_budgets_refuse_large_observations() {
        let mut sink = AuditSink::new(limits(), None, "bytes").unwrap();
        assert_eq!(
            sink.issue(AuditIssue {
                rule_id: REQUIRED_GENERAL_ROWS[0],
                path: "a".repeat(300),
                code: "bad".into()
            }),
            Err(AuditRefusal::BudgetExceeded)
        );
        assert_eq!(
            sink.read(PredicateRead::ExactPath {
                path: "a".repeat(300),
                digest: "sha256:x".into()
            }),
            Err(AuditRefusal::BudgetExceeded)
        );
        assert_eq!(
            sink.fact(ValidationFact {
                namespace: "n".into(),
                key: "k".repeat(300),
                value_digest: "v".into()
            }),
            Err(AuditRefusal::BudgetExceeded)
        );
        assert!(sink.issues.is_empty() && sink.reads.is_empty() && sink.facts.is_empty());
    }
}
