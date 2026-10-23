use crate::{
    Rational,
    budget::{Budget, DomainError, DomainLimits, Result},
};
use tos_foundation::ExactRecordRef;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpaceDomain {
    PhysicalEarth,
    PhysicalLocal,
    Narrative,
    Semantic,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeometryCapability {
    CartesianRational2d,
    Geodetic,
    Unknown,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameBinding {
    pub spec: ExactRecordRef,
    pub instance: ExactRecordRef,
    pub crs: ExactRecordRef,
    pub rules: ExactRecordRef,
    pub axes_units: ExactRecordRef,
    pub geometry_profile: ExactRecordRef,
    pub domain: SpaceDomain,
    pub capability: GeometryCapability,
}
impl FrameBinding {
    fn admit(&self, b: &mut Budget) -> Result<()> {
        for r in [
            &self.spec,
            &self.instance,
            &self.crs,
            &self.rules,
            &self.axes_units,
            &self.geometry_profile,
        ] {
            b.exact(r)?;
        }
        Ok(())
    }
}
fn gate(a: Option<&FrameBinding>, c: Option<&FrameBinding>, b: &mut Budget) -> Result<()> {
    let a = a.ok_or(DomainError::NeedsBinding("left frame"))?;
    let c = c.ok_or(DomainError::NeedsBinding("right frame"))?;
    a.admit(b)?;
    c.admit(b)?;
    if a.domain != c.domain {
        return Err(DomainError::Incompatible("space domains without transport"));
    }
    if a != c {
        return Err(DomainError::Incompatible(
            "exact frame/CRS/rules/axes/profile without transport",
        ));
    }
    Ok(())
}
pub fn compatible_frames(
    a: Option<&FrameBinding>,
    c: Option<&FrameBinding>,
    limits: DomainLimits,
) -> Result<()> {
    gate(a, c, &mut Budget::new(limits)?)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Point {
    pub x: Rational,
    pub y: Rational,
}
/// Exact closed axis-aligned nondegenerate rectangle, not an arbitrary polygon.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rectangle {
    min: Point,
    max: Point,
}
impl Rectangle {
    pub fn new(min: Point, max: Point) -> Result<Self> {
        if min.x >= max.x || min.y >= max.y {
            return Err(DomainError::Invalid("rectangle extent"));
        }
        Ok(Self { min, max })
    }
    pub fn min(self) -> Point {
        self.min
    }
    pub fn max(self) -> Point {
        self.max
    }
    fn covers_point(self, p: Point) -> bool {
        self.min.x <= p.x && p.x <= self.max.x && self.min.y <= p.y && p.y <= self.max.y
    }
    fn interior_point(self, p: Point) -> bool {
        self.min.x < p.x && p.x < self.max.x && self.min.y < p.y && p.y < self.max.y
    }
    fn covers_rect(self, c: Self) -> bool {
        self.covers_point(c.min) && self.covers_point(c.max)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Geometry {
    Point(Point),
    Rectangle(Rectangle),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeometryRole {
    ActualExtent,
    RepresentativePoint,
    PossibleLocation,
    Centroid,
    BoundingBox,
}
#[derive(Clone, Debug)]
pub struct GeometryAssertion {
    pub exact: ExactRecordRef,
    pub subject: ExactRecordRef,
    pub frame: FrameBinding,
    pub geometry: Geometry,
    pub role: GeometryRole,
    pub source_claim: ExactRecordRef,
    pub precision_profile: ExactRecordRef,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyPredicate {
    Covers,
    Contains,
    Intersects,
    Touches,
    Overlaps,
    SharedBoundarySegment,
    AdjacentAlongSegment,
}
/// Geometric value only, at the supplied precision. It cannot establish a
/// place's identity, administrative membership or an assertion's source truth.
pub fn topology(
    a: &GeometryAssertion,
    c: &GeometryAssertion,
    predicate: TopologyPredicate,
    limits: DomainLimits,
) -> Result<bool> {
    let mut b = Budget::new(limits)?;
    for x in [a, c] {
        for r in [&x.exact, &x.subject, &x.source_claim, &x.precision_profile] {
            b.exact(r)?;
        }
        if x.role != GeometryRole::ActualExtent {
            return Err(DomainError::Unsupported(
                "topology requires actual extents; proxy/uncertainty operation is separate",
            ));
        }
    }
    gate(Some(&a.frame), Some(&c.frame), &mut b)?;
    match a.frame.capability {
        GeometryCapability::Unknown => {
            return Err(DomainError::NeedsBinding("Cartesian rational capability"));
        }
        GeometryCapability::Geodetic => return Err(DomainError::Unsupported("geodetic geometry")),
        GeometryCapability::CartesianRational2d => {}
    }
    if !matches!(
        a.frame.domain,
        SpaceDomain::PhysicalEarth | SpaceDomain::PhysicalLocal
    ) {
        return Err(DomainError::Unsupported(
            "physical topology on narrative/semantic domain",
        ));
    }
    b.work(32)?;
    let Geometry::Rectangle(a) = a.geometry else {
        return Err(DomainError::Unsupported("left geometry requires rectangle"));
    };
    use TopologyPredicate::*;
    Ok(match c.geometry {
        Geometry::Point(p) => {
            let cover = a.covers_point(p);
            let interior = a.interior_point(p);
            match predicate {
                Covers | Intersects => cover,
                Contains => interior,
                Touches => cover && !interior,
                _ => return Err(DomainError::Unsupported("point predicate")),
            }
        }
        Geometry::Rectangle(c) => {
            // Compare exact coordinates instead of subtracting possibly extreme
            // rationals; topology needs order, not numerical metric arithmetic.
            let lx = a.min.x.max(c.min.x);
            let hx = a.max.x.min(c.max.x);
            let ly = a.min.y.max(c.min.y);
            let hy = a.max.y.min(c.max.y);
            let intersects = lx <= hx && ly <= hy;
            let touch = intersects && (lx == hx || ly == hy);
            match predicate {
                Covers | Contains => a.covers_rect(c),
                Intersects => intersects,
                Touches => touch,
                Overlaps => lx < hx && ly < hy && !a.covers_rect(c) && !c.covers_rect(a),
                AdjacentAlongSegment => touch && (lx < hx || ly < hy),
                SharedBoundarySegment => {
                    let vertical = (a.min.x == c.min.x
                        || a.min.x == c.max.x
                        || a.max.x == c.min.x
                        || a.max.x == c.max.x)
                        && ly < hy;
                    let horizontal = (a.min.y == c.min.y
                        || a.min.y == c.max.y
                        || a.max.y == c.min.y
                        || a.max.y == c.max.y)
                        && lx < hx;
                    vertical || horizontal
                }
            }
        }
    })
}
/// A graph and a frame alone do not supply a physical metric. No shortest-path
/// distance is returned as metres and no missing CRS transform is invented.
pub fn physical_distance_from_topology(
    _a: &GeometryAssertion,
    _b: &GeometryAssertion,
) -> Result<Rational> {
    Err(DomainError::Unsupported(
        "explicit metric/profile and coordinate transport required",
    ))
}
