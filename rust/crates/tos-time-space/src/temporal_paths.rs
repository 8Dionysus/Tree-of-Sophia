use crate::{
    AxisBinding, Rational, Span,
    budget::{Budget, DomainError, DomainLimits, Result, push},
};
use tos_foundation::ExactRecordRef;

/// All pins are upstream-resolved actual records; equality never issues authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JointContext {
    pub context: ExactRecordRef,
    pub selection: ExactRecordRef,
    pub scope: ExactRecordRef,
    pub carrier: ExactRecordRef,
    pub scenario: ExactRecordRef,
}
impl JointContext {
    pub(crate) fn admit(&self, b: &mut Budget) -> Result<()> {
        for r in [
            &self.context,
            &self.selection,
            &self.scope,
            &self.carrier,
            &self.scenario,
        ] {
            b.exact(r)?;
        }
        Ok(())
    }
    fn matches(&self, rhs: &Self, b: &mut Budget) -> Result<bool> {
        self.admit(b)?;
        rhs.admit(b)?;
        Ok(self == rhs)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedEndpoint {
    pub record: ExactRecordRef,
    pub kind_profile: ExactRecordRef,
}
impl TypedEndpoint {
    fn matches(&self, rhs: &Self, b: &mut Budget) -> Result<bool> {
        Ok(b.eq_ref(&self.record, &rhs.record)?
            && b.eq_ref(&self.kind_profile, &rhs.kind_profile)?)
    }
    fn admit(&self, b: &mut Budget) -> Result<()> {
        b.exact(&self.record)?;
        b.exact(&self.kind_profile)
    }
}
/// Possibility regions cannot be supplied here. ObservedPrefix supplies positive
/// support only: outside that window is unresolved, not false or eternally true.
#[derive(Clone, Debug)]
pub enum WorldValidity {
    Actual {
        extent: Span,
        claim: ExactRecordRef,
    },
    ObservedPrefix {
        observed: Span,
        censor_basis: ExactRecordRef,
    },
    Unresolved {
        expression: ExactRecordRef,
    },
}
impl WorldValidity {
    fn admit(&self, b: &mut Budget) -> Result<()> {
        match self {
            Self::Actual { claim, .. } => b.exact(claim),
            Self::ObservedPrefix { censor_basis, .. } => b.exact(censor_basis),
            Self::Unresolved { expression } => b.exact(expression),
        }
    }
    fn actual_constraint(&self) -> Option<Span> {
        match self {
            Self::Actual { extent, .. } => Some(*extent),
            Self::ObservedPrefix { .. } | Self::Unresolved { .. } => None,
        }
    }
    fn positive_support(&self) -> Option<Span> {
        match self {
            Self::Actual { extent, .. } => Some(*extent),
            Self::ObservedPrefix { observed, .. } => Some(*observed),
            Self::Unresolved { .. } => None,
        }
    }
}
#[derive(Debug)]
pub struct ScopedEdge {
    pub exact: ExactRecordRef,
    pub source: TypedEndpoint,
    pub target: TypedEndpoint,
    pub relation_profile: ExactRecordRef,
    pub joint: JointContext,
    pub axis: AxisBinding,
    pub validity: WorldValidity,
    pub provenance: Vec<ExactRecordRef>,
}
fn admit_edge(e: &ScopedEdge, b: &mut Budget) -> Result<()> {
    b.exact(&e.exact)?;
    e.source.admit(b)?;
    e.target.admit(b)?;
    b.exact(&e.relation_profile)?;
    e.joint.admit(b)?;
    e.axis.admit(b)?;
    e.validity.admit(b)?;
    for r in &e.provenance {
        b.item()?;
        b.exact(r)?;
    }
    Ok(())
}
#[derive(Debug)]
pub struct TemporalPath<'a> {
    edges: Vec<&'a ScopedEdge>,
    constraint: Option<Span>,
    positive: Option<Span>,
    unresolved: bool,
}
impl<'a> TemporalPath<'a> {
    pub fn edges(&self) -> &[&'a ScopedEdge] {
        &self.edges
    }
    pub fn known_validity(&self) -> Option<Span> {
        if self.unresolved {
            None
        } else {
            self.constraint
        }
    }
    /// Necessary actual-extent bound. None means no supplied upper constraint,
    /// never proof of an unbounded extent. Observed windows do not restrict it.
    pub fn necessary_actual_constraint(&self) -> Option<Span> {
        self.constraint
    }
    /// A window on which every edge is positively supported. None means no
    /// established joint positive window, not a proved-impossible actual path.
    pub fn positive_support(&self) -> Option<Span> {
        self.positive
    }
    pub fn from_edge(e: &'a ScopedEdge, limits: DomainLimits) -> Result<Self> {
        let mut b = Budget::new(limits)?;
        admit_edge(e, &mut b)?;
        let mut edges = Vec::new();
        push(&mut b, &mut edges, e)?;
        Ok(Self {
            edges,
            constraint: e.validity.actual_constraint(),
            positive: e.validity.positive_support(),
            unresolved: e.validity.actual_constraint().is_none(),
        })
    }
    pub fn has_censored_observation(&self) -> bool {
        self.edges
            .iter()
            .any(|e| matches!(&e.validity, WorldValidity::ObservedPrefix { .. }))
    }
}
fn admit_path(p: &TemporalPath<'_>, b: &mut Budget) -> Result<()> {
    for e in &p.edges {
        b.item()?;
        admit_edge(e, b)?;
    }
    Ok(())
}
/// Exact join of supplied paths, never a declaration of transitive source truth.
/// Different scenarios are excluded, not Cartesianized. Unknown spans produce
/// an unresolved path containing every original edge, never an empty/eternal one.
/// Only disjoint necessary actual constraints reject a path. Disjoint observed
/// positive windows remove known joint support, not actual path possibility.
pub fn compose_paths<'a>(
    left: &[TemporalPath<'a>],
    right: &[TemporalPath<'a>],
    limits: DomainLimits,
) -> Result<Vec<TemporalPath<'a>>> {
    let mut b = Budget::new(limits)?;
    for p in left.iter().chain(right) {
        b.item()?;
        admit_path(p, &mut b)?;
    }
    let mut out = Vec::new();
    for a in left {
        for c in right {
            b.work(1)?;
            let ae = a.edges.last().ok_or(DomainError::Invalid("empty path"))?;
            let ce = c.edges.first().ok_or(DomainError::Invalid("empty path"))?;
            if !ae.target.matches(&ce.source, &mut b)? {
                continue;
            }
            // Context/scope/selection differences are separate query universes.
            // A caller must explicitly choose a common universe before composing.
            if !b.eq_ref(&ae.joint.context, &ce.joint.context)?
                || !b.eq_ref(&ae.joint.selection, &ce.joint.selection)?
                || !b.eq_ref(&ae.joint.scope, &ce.joint.scope)?
                || !b.eq_ref(&ae.joint.carrier, &ce.joint.carrier)?
            {
                return Err(DomainError::Incompatible(
                    "joint context/selection/scope/carrier",
                ));
            }
            if !b.eq_ref(&ae.joint.scenario, &ce.joint.scenario)? {
                continue;
            }
            if !ae.axis.matches(&ce.axis, &mut b)? {
                return Err(DomainError::Incompatible(
                    "time axis without exact transport",
                ));
            }
            let constraint = match (a.constraint, c.constraint) {
                (Some(x), Some(y)) => match x.intersection(y) {
                    Some(z) => Some(z),
                    None => continue,
                },
                (x, None) | (None, x) => x,
            };
            let positive = match (a.positive, c.positive) {
                (Some(x), Some(y)) => x.intersection(y),
                _ => None,
            };
            let mut edges = Vec::new();
            for e in a.edges.iter().chain(&c.edges) {
                b.work(1)?;
                push(&mut b, &mut edges, *e)?;
            }
            push(
                &mut b,
                &mut out,
                TemporalPath {
                    edges,
                    constraint,
                    positive,
                    unresolved: a.unresolved || c.unresolved,
                },
            )?;
        }
    }
    Ok(out)
}
/// Independent clocks; these exact references are carried, never coerced into
/// one clock. The snapshot selector/knowledge admission remains upstream.
#[derive(Clone, Debug)]
pub struct TemporalContextTimes {
    pub world_axis: AxisBinding,
    pub source_time: ExactRecordRef,
    pub knowledge_cut: ExactRecordRef,
    pub system_cut: ExactRecordRef,
    pub query_context: ExactRecordRef,
}
#[derive(Debug)]
pub struct HistoricalState<'a, T> {
    pub exact: ExactRecordRef,
    pub subject: TypedEndpoint,
    pub joint: JointContext,
    pub axis: AxisBinding,
    pub validity: WorldValidity,
    pub payload: &'a T,
}
#[derive(Debug)]
pub struct StateSelection<'a, T> {
    pub included: Vec<&'a HistoricalState<'a, T>>,
    pub unresolved: Vec<&'a HistoricalState<'a, T>>,
}
/// Filter an already frozen upstream list at one world-time coordinate.
/// Keep overlapping revisions and unresolved dates; no last-write-wins.
/// Outside an observed positive window remains unresolved. Only Actual declares
/// an exhaustive extent and therefore permits exclusion by non-membership.
pub fn select_states<'a, T>(
    states: &'a [HistoricalState<'a, T>],
    at: Rational,
    axis: &AxisBinding,
    joint: &JointContext,
    limits: DomainLimits,
) -> Result<StateSelection<'a, T>> {
    let mut b = Budget::new(limits)?;
    axis.admit(&mut b)?;
    joint.admit(&mut b)?;
    let mut out = StateSelection {
        included: Vec::new(),
        unresolved: Vec::new(),
    };
    for state in states {
        b.item()?;
        b.exact(&state.exact)?;
        state.subject.admit(&mut b)?;
        state.validity.admit(&mut b)?;
        if !axis.matches(&state.axis, &mut b)? || !joint.matches(&state.joint, &mut b)? {
            return Err(DomainError::Incompatible("state universe/axis"));
        }
        match &state.validity {
            WorldValidity::Actual { extent, .. } => {
                if extent.contains(at) {
                    push(&mut b, &mut out.included, state)?;
                }
            }
            WorldValidity::ObservedPrefix { observed, .. } if observed.contains(at) => {
                push(&mut b, &mut out.included, state)?;
            }
            _ => push(&mut b, &mut out.unresolved, state)?,
        }
    }
    Ok(out)
}
/// Event occurrence and state replacement are supplied distinct assertions.
/// A transition is not inferred from sorting dates, renaming, or nearby places.
#[derive(Debug)]
pub struct StateTransition<'a, T> {
    pub event: ExactRecordRef,
    pub occurrence_claim: ExactRecordRef,
    pub occurrence: Span,
    pub before: &'a HistoricalState<'a, T>,
    pub after: &'a HistoricalState<'a, T>,
    pub transition_rule: ExactRecordRef,
}
pub fn check_transition<T>(t: &StateTransition<'_, T>, limits: DomainLimits) -> Result<()> {
    let mut b = Budget::new(limits)?;
    for r in [
        &t.event,
        &t.occurrence_claim,
        &t.transition_rule,
        &t.before.exact,
        &t.after.exact,
    ] {
        b.exact(r)?;
    }
    if !t.before.subject.matches(&t.after.subject, &mut b)?
        || !t.before.axis.matches(&t.after.axis, &mut b)?
        || !t.before.joint.matches(&t.after.joint, &mut b)?
    {
        return Err(DomainError::Incompatible(
            "transition subject/universe/axis",
        ));
    }
    // Only interface consistency. No assumption of exclusive/exhaustive states,
    // transition instant, causation, or continuity beyond the explicit claim.
    Ok(())
}
