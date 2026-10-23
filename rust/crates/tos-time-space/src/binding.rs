use crate::{
    Allen, AxisBinding, JointContext, OrderOutcome, RelationImage, Span,
    budget::{Budget, DomainError, DomainLimits, Result, push},
};
use tos_foundation::{
    CheckedOperationBinding, Digest256, ExactRecordRef, ExecutionStatus, JsonValue,
    ValueDisposition,
};

/// Exact, domain-owned map endpoints. No map is synthesized from equal numbers,
/// a label, a date string, a CRS name or an upstream structural K0 check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedMapEndpoint {
    pub kind: ExactRecordRef,
    pub domain: ExactRecordRef,
}
#[derive(Clone, Debug)]
pub struct DeclaredTransport {
    pub map: ExactRecordRef,
    pub source: TypedMapEndpoint,
    pub target: TypedMapEndpoint,
    pub rule: ExactRecordRef,
    pub applicability: TransportApplicability,
}
#[derive(Clone, Debug)]
pub enum TransportApplicability {
    /// Caller has resolved the exact applicability record and supplied the
    /// domain decision for this input; this is not a universal map certificate.
    AdmittedInput {
        input: ExactRecordRef,
        warrant: ExactRecordRef,
    },
    OutsideDomain {
        decision: ExactRecordRef,
    },
    Unknown {
        question: ExactRecordRef,
    },
}
pub fn require_transport<'a>(
    m: Option<&'a DeclaredTransport>,
    from: &TypedMapEndpoint,
    to: &TypedMapEndpoint,
    input: &ExactRecordRef,
    limits: DomainLimits,
) -> Result<&'a DeclaredTransport> {
    let mut b = Budget::new(limits)?;
    let m = m.ok_or(DomainError::NeedsBinding("exact transport/rule/domain"))?;
    for r in [
        &m.map,
        &m.rule,
        &m.source.kind,
        &m.source.domain,
        &m.target.kind,
        &m.target.domain,
        &from.kind,
        &from.domain,
        &to.kind,
        &to.domain,
        input,
    ] {
        b.exact(r)?;
    }
    if &m.source != from || &m.target != to {
        return Err(DomainError::Incompatible("typed map endpoints"));
    }
    match &m.applicability {
        TransportApplicability::AdmittedInput {
            input: admitted,
            warrant,
        } => {
            b.exact(warrant)?;
            if !b.eq_ref(admitted, input)? {
                return Err(DomainError::NeedsBinding(
                    "input-specific transport applicability",
                ));
            }
        }
        TransportApplicability::OutsideDomain { decision } => {
            b.exact(decision)?;
            return Err(DomainError::Incompatible("outside exact transport domain"));
        }
        TransportApplicability::Unknown { question } => {
            b.exact(question)?;
            return Err(DomainError::NeedsBinding("transport applicability"));
        }
    }
    Ok(m)
}
pub fn require_same_time_axis(
    a: &AxisBinding,
    c: &AxisBinding,
    limits: DomainLimits,
) -> Result<()> {
    if a.matches(c, &mut Budget::new(limits)?)? {
        Ok(())
    } else {
        Err(DomainError::Incompatible(
            "TRS/axis/rules: explicit crosswalk needed",
        ))
    }
}

/// A bounded projection of an upstream joint carrier, not a new possible-worlds
/// kernel. Complete is an explicit upstream coverage assertion with a pin; no
/// authority, scientific completeness or K0 semantic disposition is issued here.
#[derive(Clone, Debug)]
pub enum JointCoverage {
    Complete { coverage: ExactRecordRef },
    Observed,
}
#[derive(Clone, Debug)]
pub struct JointTruthRow {
    pub joint: JointContext,
    pub first: bool,
    pub second: bool,
}
#[derive(Clone, Copy, Debug)]
pub enum TruthProjection {
    First,
    Second,
    Conjunction,
}
#[derive(Debug)]
pub struct TruthWitness<'a> {
    pub value: bool,
    pub scenario: &'a ExactRecordRef,
}
#[derive(Debug)]
pub enum JointTruthImage<'a> {
    NoAdmissibleModel {
        coverage: &'a ExactRecordRef,
    },
    ObservedNoWitness,
    Values {
        witnesses: Vec<TruthWitness<'a>>,
        complete: bool,
    },
}
pub fn project_joint_truth<'a>(
    rows: &'a [JointTruthRow],
    universe: &JointContext,
    coverage: &'a JointCoverage,
    projection: TruthProjection,
    limits: DomainLimits,
) -> Result<JointTruthImage<'a>> {
    let mut b = Budget::new(limits)?;
    universe.admit(&mut b)?;
    let complete = match coverage {
        JointCoverage::Complete { coverage } => {
            b.exact(coverage)?;
            true
        }
        JointCoverage::Observed => false,
    };
    let mut witnesses = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        b.item()?;
        r.joint.admit(&mut b)?;
        if !b.eq_ref(&r.joint.context, &universe.context)?
            || !b.eq_ref(&r.joint.selection, &universe.selection)?
            || !b.eq_ref(&r.joint.scope, &universe.scope)?
            || !b.eq_ref(&r.joint.carrier, &universe.carrier)?
        {
            return Err(DomainError::Incompatible("joint truth carrier/universe"));
        }
        for prior in &rows[..i] {
            if b.eq_ref(&prior.joint.scenario, &r.joint.scenario)? {
                return Err(DomainError::Invalid("duplicate joint scenario"));
            }
        }
        b.work(1)?;
        let value = match projection {
            TruthProjection::First => r.first,
            TruthProjection::Second => r.second,
            TruthProjection::Conjunction => r.first && r.second,
        };
        if !witnesses
            .iter()
            .any(|w: &TruthWitness<'_>| w.value == value)
        {
            push(
                &mut b,
                &mut witnesses,
                TruthWitness {
                    value,
                    scenario: &r.joint.scenario,
                },
            )?;
        }
    }
    if rows.is_empty() {
        return Ok(match coverage {
            JointCoverage::Complete { coverage } => JointTruthImage::NoAdmissibleModel { coverage },
            JointCoverage::Observed => JointTruthImage::ObservedNoWitness,
        });
    }
    Ok(JointTruthImage::Values {
        witnesses,
        complete,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TsCodomain {
    SpanMeet,
    AllenRelation,
    OrderSolution,
    RelationImage,
    TopologyValue,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TsProjection {
    DirectDomainValueV1,
}
/// Actual typed profile record body resolved by the store. No synthetic native
/// profile identifier, issuer or authority is supplied in this crate.
#[derive(Debug)]
pub struct TsProfileBody {
    pub projection: TsProjection,
    pub codomain: TsCodomain,
}
#[derive(Debug)]
pub enum TsOutputValue {
    SpanMeet(Option<Span>),
    AllenRelation(Allen),
    OrderSolution(OrderOutcome),
    RelationImage(RelationImage),
    TopologyValue(bool),
}
impl TsOutputValue {
    fn codomain(&self) -> TsCodomain {
        match self {
            Self::SpanMeet(_) => TsCodomain::SpanMeet,
            Self::AllenRelation(_) => TsCodomain::AllenRelation,
            Self::OrderSolution(_) => TsCodomain::OrderSolution,
            Self::RelationImage(_) => TsCodomain::RelationImage,
            Self::TopologyValue(_) => TsCodomain::TopologyValue,
        }
    }
    fn check_shape(&self, b: &mut Budget) -> Result<()> {
        match self {
            Self::OrderSolution(OrderOutcome::Witness(values)) => witness_shape(values, b),
            Self::OrderSolution(OrderOutcome::StrictCycle(cycle))
            | Self::RelationImage(RelationImage::NoAdmissibleModel(cycle)) => cycle_shape(cycle, b),
            Self::RelationImage(RelationImage::Exact { possible }) => {
                if possible.is_empty() || possible.len() > 13 {
                    return Err(DomainError::Invalid("exact relation image shape"));
                }
                let mut width = None;
                for (i, (label, values)) in possible.iter().enumerate() {
                    b.item()?;
                    for (other, _) in &possible[..i] {
                        b.work(1)?;
                        if other == label {
                            return Err(DomainError::Invalid("duplicate Allen image label"));
                        }
                    }
                    witness_shape(values, b)?;
                    if width.is_some_and(|n| n != values.len()) {
                        return Err(DomainError::Invalid("joint image witness width"));
                    }
                    width = Some(values.len());
                }
                Ok(())
            }
            _ => {
                b.work(1)?;
                Ok(())
            }
        }
    }
}
fn witness_shape(values: &[crate::Rational], b: &mut Budget) -> Result<()> {
    if values.is_empty() || values.len() % 2 != 0 {
        return Err(DomainError::Invalid("endpoint witness shape"));
    }
    for pair in values.chunks_exact(2) {
        b.item()?;
        b.work(1)?;
        if pair[0] >= pair[1] {
            return Err(DomainError::Invalid("nonproper witness interval"));
        }
    }
    Ok(())
}
fn cycle_shape(cycle: &[crate::CycleStep], b: &mut Budget) -> Result<()> {
    if cycle.is_empty() {
        return Err(DomainError::Invalid("empty cycle shape"));
    }
    let mut strict = false;
    for (i, step) in cycle.iter().enumerate() {
        b.item()?;
        b.work(1)?;
        if step.to != cycle[(i + 1) % cycle.len()].from
            || matches!(step.premise, crate::Premise::Hypothesis(_))
        {
            return Err(DomainError::Invalid("base cycle shape"));
        }
        strict |= step.strict;
    }
    if !strict {
        return Err(DomainError::Invalid("nonstrict cycle shape"));
    }
    Ok(())
}
pub struct ResolvedRecord<'a, T> {
    pub exact: &'a ExactRecordRef,
    pub body: &'a T,
}
/// Resolve actual records with their named profile. Implementations must verify
/// exact record/body binding under their owner digest/profile, not raw-byte SHA
/// equality by default. Resolver work/storage has a separately supplied budget.
pub trait TsOutputStore {
    fn resolve_value<'a>(
        &'a self,
        reference: &JsonValue,
        limits: DomainLimits,
    ) -> Result<ResolvedRecord<'a, TsOutputValue>>;
    fn resolve_profile<'a>(
        &'a self,
        reference: &JsonValue,
        limits: DomainLimits,
    ) -> Result<ResolvedRecord<'a, TsProfileBody>>;
}
// K0 has already checked wire grammar. This bounded second check prevents an
// otherwise usable resolver from returning a different version or record pin.
fn wire_matches(v: &JsonValue, r: &ExactRecordRef, b: &mut Budget) -> Result<bool> {
    let entries = v
        .as_object()
        .ok_or(DomainError::Invalid("wire exact reference"))?;
    if entries.len() != 3 {
        return Err(DomainError::Invalid("wire exact reference fields"));
    }
    b.exact(r)?;
    let mut id = None;
    let mut version = None;
    let mut digest = None;
    for (key, value) in entries {
        b.work(key.units().len())?;
        match key.as_str() {
            Some("id") => {
                let s = value.as_str().ok_or(DomainError::Invalid("wire id"))?;
                b.work(s.len())?;
                id = Some(s);
            }
            Some("version") => {
                if let JsonValue::Number(n) = value {
                    b.work(n.lexeme.len())?;
                }
                version = value.as_u64();
            }
            Some("digest") => {
                let s = value.as_str().ok_or(DomainError::Invalid("wire digest"))?;
                b.work(s.len())?;
                digest = Some(
                    Digest256::from_prefixed(s).map_err(|_| DomainError::Invalid("wire digest"))?,
                );
            }
            _ => return Err(DomainError::Invalid("wire exact reference key")),
        }
    }
    Ok(id == Some(r.id.as_str()) && version == Some(r.version.get()) && digest == Some(r.digest))
}
pub enum CheckedTsOutput<'a> {
    NoCompleteValue {
        value: ValueDisposition,
        execution: ExecutionStatus,
    },
    Produced {
        exact: &'a ExactRecordRef,
        profile: &'a ExactRecordRef,
        value: &'a TsOutputValue,
    },
}
#[derive(Clone, Copy, Debug)]
pub struct OutputCheckLimits {
    pub local: DomainLimits,
    pub value_resolver: DomainLimits,
    pub profile_resolver: DomainLimits,
}
/// K0 structural success is only the input. Resolve output and profile, apply
/// that profile's declared projection and check the actual projected codomain.
/// A resolver implementing this trait is a real remaining integration binding;
/// absence is explicit NeedsBinding, never a fabricated successful producer.
pub fn check_ts_output<'a>(
    run: &CheckedOperationBinding<'_>,
    store: Option<&'a dyn TsOutputStore>,
    expected_codomain: TsCodomain,
    limits: OutputCheckLimits,
) -> Result<CheckedTsOutput<'a>> {
    let mut b = Budget::new(limits.local)?;
    let value = run.value_disposition();
    let execution = run.execution_status();
    if value != ValueDisposition::Produced || execution != ExecutionStatus::Completed {
        return Ok(CheckedTsOutput::NoCompleteValue { value, execution });
    }
    let store = store.ok_or(DomainError::NeedsBinding("actual TS output/profile store"))?;
    let value_ref = run
        .output_ref()
        .ok_or(DomainError::Invalid("K0 produced output_ref"))?;
    let profile_ref = run
        .output_profile_ref()
        .ok_or(DomainError::Invalid("K0 output_profile_ref"))?;
    let value = store.resolve_value(value_ref, limits.value_resolver)?;
    let profile = store.resolve_profile(profile_ref, limits.profile_resolver)?;
    if !wire_matches(value_ref, value.exact, &mut b)?
        || !wire_matches(profile_ref, profile.exact, &mut b)?
    {
        return Err(DomainError::Incompatible(
            "resolved output/profile exact pin",
        ));
    }
    match profile.body.projection {
        TsProjection::DirectDomainValueV1 => {}
    }
    if profile.body.codomain != expected_codomain || value.body.codomain() != expected_codomain {
        return Err(DomainError::Incompatible(
            "actual projected output codomain",
        ));
    }
    value.body.check_shape(&mut b)?;
    Ok(CheckedTsOutput::Produced {
        exact: value.exact,
        profile: profile.exact,
        value: value.body,
    })
}
