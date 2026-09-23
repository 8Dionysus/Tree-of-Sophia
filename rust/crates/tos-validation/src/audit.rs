//! Full-cut validation orchestration. This is an internal engine component,
//! not a source-admission issuer: STO must still attest the input cut and CMD
//! must linearize current authority before any mechanical result is sealed.

use std::collections::BTreeSet;

use tos_foundation::{Digest256, Digest256Hasher};

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
    fn next_member(&mut self) -> Result<Option<AuditMember>, String>;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AuditLimits {
    pub max_members: u64,
    pub max_member_bytes: usize,
    pub max_total_bytes: u64,
    pub max_issues: usize,
    pub max_reads: usize,
    pub max_facts: usize,
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
    RuleIndeterminate {
        rule_id: &'static str,
        reason: String,
    },
    RuleUnsupported {
        rule_id: &'static str,
        profile: String,
    },
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
    reads: Vec<PredicateRead>,
    facts: Vec<ValidationFact>,
}

impl AuditSink {
    fn new(limits: AuditLimits) -> Self {
        Self {
            limits,
            issues: Vec::new(),
            reads: Vec::new(),
            facts: Vec::new(),
        }
    }

    pub fn issue(&mut self, issue: AuditIssue) -> Result<(), AuditRefusal> {
        if self.issues.len() >= self.limits.max_issues {
            return Err(AuditRefusal::BudgetExceeded);
        }
        self.issues.push(issue);
        Ok(())
    }

    pub fn read(&mut self, read: PredicateRead) -> Result<(), AuditRefusal> {
        if self.reads.len() >= self.limits.max_reads {
            return Err(AuditRefusal::BudgetExceeded);
        }
        self.reads.push(read);
        Ok(())
    }

    pub fn fact(&mut self, fact: ValidationFact) -> Result<(), AuditRefusal> {
        if self.facts.len() >= self.limits.max_facts {
            return Err(AuditRefusal::BudgetExceeded);
        }
        self.facts.push(fact);
        Ok(())
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

    let mut sink = AuditSink::new(limits);
    let mut hasher = Digest256Hasher::new();
    hasher.update(b"tos-val-full-membership-v1\0");
    let mut count = 0u64;
    let mut total_bytes = 0u64;
    let mut previous_path = None::<String>;
    loop {
        let member = match stream.next_member() {
            Ok(Some(member)) => member,
            Ok(None) => break,
            Err(_) => return AuditResult::Refused(AuditRefusal::IncompleteMemberStream),
        };
        count = match count.checked_add(1) {
            Some(value) if value <= limits.max_members => value,
            _ => return AuditResult::Refused(AuditRefusal::BudgetExceeded),
        };
        total_bytes = match total_bytes.checked_add(member.raw.len() as u64) {
            Some(value) if value <= limits.max_total_bytes => value,
            _ => return AuditResult::Refused(AuditRefusal::BudgetExceeded),
        };
        if member.raw.len() > limits.max_member_bytes
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
            if let Err(reason) = rule.inspect(&member, &mut sink) {
                return AuditResult::Refused(reason);
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
        if let Err(reason) = rule.finish(&mut sink) {
            return AuditResult::Refused(reason);
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
        fn next_member(&mut self) -> Result<Option<AuditMember>, String> {
            if self.0.is_empty() {
                Ok(None)
            } else {
                self.0.remove(0).map(Some)
            }
        }
    }

    struct Rule(&'static str);

    impl AuditRule for Rule {
        fn row_id(&self) -> &'static str {
            self.0
        }
        fn inspect(
            &mut self,
            _member: &AuditMember,
            _sink: &mut AuditSink,
        ) -> Result<(), AuditRefusal> {
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
            max_total_bytes: 64,
            max_issues: 4,
            max_reads: 4,
            max_facts: 4,
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
        let mut modules: Vec<Rule> = REQUIRED_GENERAL_ROWS.into_iter().map(Rule).collect();
        let mut refs: Vec<&mut dyn AuditRule> = modules
            .iter_mut()
            .map(|rule| rule as &mut dyn AuditRule)
            .collect();
        let mut truncated = VecStream(vec![Ok(member), Err("cut read failed".into())]);
        assert_eq!(
            run_full_probe(&mut truncated, expected, &mut refs, limits()),
            AuditResult::Refused(AuditRefusal::IncompleteMemberStream)
        );
        let mut empty = VecStream(vec![]);
        assert_eq!(
            run_full_probe(&mut empty, expected, &mut refs, limits()),
            AuditResult::Refused(AuditRefusal::MembershipMismatch)
        );
    }
}
