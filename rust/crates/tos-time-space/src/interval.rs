use crate::{Rational, budget::{Budget, DomainError, DomainLimits, Result}};
use tos_foundation::ExactRecordRef;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AxisBinding {
    pub spec: ExactRecordRef,
    pub instance: ExactRecordRef,
    pub trs: ExactRecordRef,
    pub rules: ExactRecordRef,
}
impl AxisBinding {
    pub(crate) fn admit(&self, b: &mut Budget) -> Result<()> {
        for r in [&self.spec, &self.instance, &self.trs, &self.rules] { b.exact(r)?; } Ok(())
    }
    pub(crate) fn matches(&self, rhs: &Self, b: &mut Budget) -> Result<bool> {
        self.admit(b)?; rhs.admit(b)?; Ok(self == rhs)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Boundary {
    Known { value: Rational, inclusive: Option<bool>, resolution: Option<ExactRecordRef> },
    Unknown,
    Unspecified,
    NoBoundDeclared { source_profile: ExactRecordRef },
    Unbounded { basis: ExactRecordRef },
    NotApplicable,
}
/// Set-membership interval, distinct from an actual event or a dating claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span { lower: Rational, upper: Rational, lower_closed: bool, upper_closed: bool }
impl Span {
    pub fn new(lower: Rational, upper: Rational, lower_closed: bool, upper_closed: bool) -> Result<Self> {
        if lower > upper { return Err(DomainError::Invalid("reversed bounds")); }
        if lower == upper && !(lower_closed && upper_closed) {
            return Err(DomainError::Invalid("empty span; use None"));
        }
        Ok(Self { lower, upper, lower_closed, upper_closed })
    }
    pub fn lower(self) -> Rational { self.lower }
    pub fn upper(self) -> Rational { self.upper }
    pub fn lower_closed(self) -> bool { self.lower_closed }
    pub fn upper_closed(self) -> bool { self.upper_closed }
    pub fn contains(self, x: Rational) -> bool {
        (self.lower < x || self.lower_closed && self.lower == x)
            && (x < self.upper || self.upper_closed && x == self.upper)
    }
    pub fn intersection(self, rhs: Self) -> Option<Self> {
        let lo = self.lower.max(rhs.lower); let hi = self.upper.min(rhs.upper);
        let lc = self.contains(lo) && rhs.contains(lo);
        let uc = self.contains(hi) && rhs.contains(hi);
        if lo > hi || lo == hi && !(lc && uc) { None }
        else { Some(Self { lower: lo, upper: hi, lower_closed: lc, upper_closed: uc }) }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProperInterval(Span);
impl ProperInterval {
    pub fn new(span: Span) -> Result<Self> {
        if span.lower >= span.upper { return Err(DomainError::Invalid("proper interval required")); }
        Ok(Self(span))
    }
    pub fn span(self) -> Span { self.0 }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActualExtent { Instant(Rational), Interval(ProperInterval) }
/// A bucket/range of ignorance; no From<DatingPossibility> for ActualExtent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DatingPossibility {
    PositionRegion { axis: AxisBinding, region: Span, source_claim: ExactRecordRef },
    ConstraintModel { model: ExactRecordRef },
    Unresolved { expression: ExactRecordRef },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoundaryResolution<'a> {
    Resolved(Span),
    Unresolved { lower: &'a Boundary, upper: &'a Boundary },
}
pub fn resolve_boundaries<'a>(lower: &'a Boundary, upper: &'a Boundary) -> Result<BoundaryResolution<'a>> {
    match (lower, upper) {
        (Boundary::Known { value: a, inclusive: Some(lc), .. },
         Boundary::Known { value: b, inclusive: Some(uc), .. }) => Ok(BoundaryResolution::Resolved(Span::new(*a, *b, *lc, *uc)?)),
        _ => Ok(BoundaryResolution::Unresolved { lower, upper }),
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Allen {
    Before, Meets, Overlaps, Starts, During, Finishes, Equals,
    FinishedBy, Contains, StartedBy, OverlappedBy, MetBy, After,
}
pub const ALLEN_RELATIONS: [Allen; 13] = [Allen::Before, Allen::Meets, Allen::Overlaps,
    Allen::Starts, Allen::During, Allen::Finishes, Allen::Equals, Allen::FinishedBy,
    Allen::Contains, Allen::StartedBy, Allen::OverlappedBy, Allen::MetBy, Allen::After];
/// Endpoint-order relation, separate from set intersection/inclusion and from
/// the existing tos-query coarse source-envelope comparison vocabulary.
pub fn relate_intervals(a: ProperInterval, axis_a: &AxisBinding, c: ProperInterval,
    axis_c: &AxisBinding, limits: DomainLimits) -> Result<Allen> {
    let mut b = Budget::new(limits)?;
    if !axis_a.matches(axis_c, &mut b)? { return Err(DomainError::Incompatible("exact TRS/rules without transport")); }
    b.work(16)?; Ok(allen(a, c))
}
impl Allen {
    pub fn converse(self) -> Self {
        match self {
            Self::Before => Self::After, Self::Meets => Self::MetBy,
            Self::Overlaps => Self::OverlappedBy, Self::Starts => Self::StartedBy,
            Self::During => Self::Contains, Self::Finishes => Self::FinishedBy,
            Self::Equals => Self::Equals, other => match other {
                Self::After => Self::Before, Self::MetBy => Self::Meets,
                Self::OverlappedBy => Self::Overlaps, Self::StartedBy => Self::Starts,
                Self::Contains => Self::During, Self::FinishedBy => Self::Finishes,
                _ => unreachable!(),
            },
        }
    }
}
/// Pure endpoint-order observable. Axis/TRS equality is checked by the bound
/// caller before using this mathematical primitive on sourced assignments.
pub fn allen(a: ProperInterval, b: ProperInterval) -> Allen {
    let (s, e, u, v) = (a.0.lower, a.0.upper, b.0.lower, b.0.upper);
    if e < u { return Allen::Before; } if e == u { return Allen::Meets; }
    if v < s { return Allen::After; } if v == s { return Allen::MetBy; }
    if s == u { return if e < v { Allen::Starts } else if e == v { Allen::Equals } else { Allen::StartedBy }; }
    if e == v { return if u < s { Allen::Finishes } else { Allen::FinishedBy }; }
    if u < s && e < v { return Allen::During; }
    if s < u && v < e { return Allen::Contains; }
    if s < u { Allen::Overlaps } else { Allen::OverlappedBy }
}
