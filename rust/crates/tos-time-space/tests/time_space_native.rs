//! Synthetic owner-contract fixtures only. No real native profiles or authority.
//! Seven preserved oracles plus three prior deltas; focused N8/Q4 regressions.
use tos_foundation::{
    Digest256, ExactRecordRef, JsonLimits, JsonMode, JsonString, JsonValue, OperationBindingLimits,
    RecordVersion, StableId, check_operation_binding, parse_json,
};
use tos_time_space::*;

fn lim() -> DomainLimits {
    DomainLimits {
        max_work: 10_000_000,
        max_requested_bytes: 10_000_000,
        max_items: 100_000,
    }
}
fn pin(id: &str) -> ExactRecordRef {
    ExactRecordRef {
        id: StableId::parse(id).unwrap(),
        version: RecordVersion::new(1).unwrap(),
        digest: Digest256::from_bytes([0; 32]),
    }
}
fn q(n: i128) -> Rational {
    Rational::integer(n).unwrap()
}
fn span(a: i128, c: i128) -> Span {
    Span::new(q(a), q(c), true, false).unwrap()
}
fn axis() -> AxisBinding {
    AxisBinding {
        spec: pin("fixture.axis-spec"),
        instance: pin("fixture.axis"),
        trs: pin("fixture.trs"),
        rules: pin("fixture.rules"),
    }
}
fn joint(s: &str) -> JointContext {
    JointContext {
        context: pin("fixture.context"),
        selection: pin("fixture.frozen-cut"),
        scope: pin("fixture.scope"),
        carrier: pin("fixture.joint-carrier"),
        scenario: pin(s),
    }
}
fn endpoint(id: &str) -> TypedEndpoint {
    TypedEndpoint {
        record: pin(id),
        kind_profile: pin("fixture.place-kind"),
    }
}
fn edge(id: &str, a: &str, c: &str, valid: WorldValidity) -> ScopedEdge {
    ScopedEdge {
        exact: pin(id),
        source: endpoint(a),
        target: endpoint(c),
        relation_profile: pin("fixture.spatial-link"),
        joint: joint("fixture.scenario-1"),
        axis: axis(),
        validity: valid,
        provenance: vec![pin("fixture.source-claim")],
    }
}
fn actual(a: i128, c: i128) -> WorldValidity {
    WorldValidity::Actual {
        extent: span(a, c),
        claim: pin("fixture.validity-claim"),
    }
}
fn frame() -> FrameBinding {
    FrameBinding {
        spec: pin("fixture.frame-spec"),
        instance: pin("fixture.frame"),
        crs: pin("fixture.local-crs"),
        rules: pin("fixture.cartesian-rules"),
        axes_units: pin("fixture.axes-units"),
        geometry_profile: pin("fixture.rectangle-profile"),
        domain: SpaceDomain::PhysicalLocal,
        capability: GeometryCapability::CartesianRational2d,
    }
}
fn geometry(g: Geometry) -> GeometryAssertion {
    GeometryAssertion {
        exact: pin("fixture.geometry-assertion"),
        subject: pin("fixture.place"),
        frame: frame(),
        geometry: g,
        role: GeometryRole::ActualExtent,
        source_claim: pin("fixture.geometry-source"),
        precision_profile: pin("fixture.precision"),
    }
}
fn rect(a: i128, b: i128, c: i128, d: i128) -> Rectangle {
    Rectangle::new(Point { x: q(a), y: q(b) }, Point { x: q(c), y: q(d) }).unwrap()
}
fn problem(n: usize) -> OrderProblem {
    OrderProblem {
        exact: pin("fixture.endpoint-model"),
        axis: axis(),
        domain: OrderDomain::DenseUnboundedRational,
        interval_count: n,
        constraints: vec![],
        anchors: vec![],
    }
}
fn constraint(left: usize, op: OrderOp, right: usize) -> SourcedConstraint {
    SourcedConstraint {
        atom: OrderAtom { left, op, right },
        source: pin("fixture.constraint-claim"),
    }
}
fn labels(image: JointTruthImage<'_>) -> Vec<bool> {
    let JointTruthImage::Values {
        witnesses,
        complete,
    } = image
    else {
        panic!("nonempty image")
    };
    assert!(complete);
    witnesses.into_iter().map(|w| w.value).collect()
}

#[test]
fn seven_oracles_and_three_deltas() {
    // 1: numeric ordinals in distinct exact TRS cannot be ordered together.
    let a = axis();
    let mut c = axis();
    c.trs = pin("fixture.other-era");
    assert!(matches!(
        require_same_time_axis(&a, &c, lim()),
        Err(DomainError::Incompatible(_))
    ));
    assert!(matches!(
        require_transport(
            None,
            &TypedMapEndpoint {
                kind: pin("fixture.axis-kind"),
                domain: a.trs.clone()
            },
            &TypedMapEndpoint {
                kind: pin("fixture.axis-kind"),
                domain: c.trs.clone()
            },
            &pin("fixture.ordinal-7"),
            lim()
        ),
        Err(DomainError::NeedsBinding(_))
    ));
    // 2: one joint carrier {(true,false),(false,true)}; no product of marginals.
    let rows = [
        JointTruthRow {
            joint: joint("fixture.scenario-1"),
            first: true,
            second: false,
        },
        JointTruthRow {
            joint: joint("fixture.scenario-2"),
            first: false,
            second: true,
        },
    ];
    let coverage = JointCoverage::Complete {
        coverage: pin("fixture.declared-finite-coverage"),
    };
    assert_eq!(
        labels(
            project_joint_truth(
                &rows,
                &joint("fixture.scenario-1"),
                &coverage,
                TruthProjection::First,
                lim()
            )
            .unwrap()
        ),
        vec![true, false]
    );
    assert_eq!(
        labels(
            project_joint_truth(
                &rows,
                &joint("fixture.scenario-1"),
                &coverage,
                TruthProjection::Second,
                lim()
            )
            .unwrap()
        ),
        vec![false, true]
    );
    assert_eq!(
        labels(
            project_joint_truth(
                &rows,
                &joint("fixture.scenario-1"),
                &coverage,
                TruthProjection::Conjunction,
                lim()
            )
            .unwrap()
        ),
        vec![false]
    );
    // 3: world-time filtering only; source/system frozen cut is supplied upstream.
    let states = [HistoricalState {
        exact: pin("fixture.bridge-state"),
        subject: endpoint("fixture.bridge"),
        joint: joint("fixture.scenario-1"),
        axis: axis(),
        validity: actual(1140, 1160),
        payload: &"bridge claim",
    }];
    assert_eq!(
        select_states(
            &states,
            q(1150),
            &axis(),
            &joint("fixture.scenario-1"),
            lim()
        )
        .unwrap()
        .included
        .len(),
        1
    );
    assert!(
        select_states::<&str>(&[], q(1150), &axis(), &joint("fixture.scenario-1"), lim())
            .unwrap()
            .included
            .is_empty()
    );
    // This does not test construction of the source2020/system2024 selector.
    // 4: same labels preserve both exact subjects, with no sameReferent verdict.
    let names = [
        HistoricalState {
            exact: pin("fixture.label-state-17"),
            subject: endpoint("fixture.place-17"),
            joint: joint("fixture.scenario-1"),
            axis: axis(),
            validity: actual(0, 10),
            payload: &"Новая Александрия",
        },
        HistoricalState {
            exact: pin("fixture.label-state-92"),
            subject: endpoint("fixture.place-92"),
            joint: joint("fixture.scenario-1"),
            axis: axis(),
            validity: actual(0, 10),
            payload: &"Новая Александрия",
        },
    ];
    assert_eq!(
        select_states(&names, q(1), &axis(), &joint("fixture.scenario-1"), lim())
            .unwrap()
            .included
            .len(),
        2
    );
    // 5: missing frame transport and metric are explicit, not Euclidean guesses.
    let f = frame();
    let mut g = frame();
    g.instance = pin("fixture.other-frame");
    assert!(matches!(
        compatible_frames(Some(&f), Some(&g), lim()),
        Err(DomainError::Incompatible(_))
    ));
    let area = geometry(Geometry::Rectangle(rect(0, 0, 10, 10)));
    let boundary = geometry(Geometry::Point(Point { x: q(0), y: q(5) }));
    assert!(matches!(
        physical_distance_from_topology(&area, &boundary),
        Err(DomainError::Unsupported(_))
    ));
    // 6: closed rectangle boundary: Covers true, Contains false.
    assert!(topology(&area, &boundary, TopologyPredicate::Covers, lim()).unwrap());
    assert!(!topology(&area, &boundary, TopologyPredicate::Contains, lim()).unwrap());
    // 7: narrative and physical domains have no automatic Earth interpretation.
    let mut narrative = frame();
    narrative.domain = SpaceDomain::Narrative;
    assert!(matches!(
        compatible_frames(Some(&narrative), Some(&f), lim()),
        Err(DomainError::Incompatible(_))
    ));
    let mut n = boundary.clone();
    n.frame = narrative.clone();
    let mut na = area.clone();
    na.frame = narrative;
    assert!(matches!(
        topology(&na, &n, TopologyPredicate::Covers, lim()),
        Err(DomainError::Unsupported(_))
    ));
    // D1: endpoint Meets and set-membership meet are separate operations.
    let x = span(2, 3);
    let y = span(3, 4);
    assert_eq!(
        relate_intervals(
            ProperInterval::new(x).unwrap(),
            &axis(),
            ProperInterval::new(y).unwrap(),
            &axis(),
            lim()
        )
        .unwrap(),
        Allen::Meets
    );
    assert_eq!(x.intersection(y), None);
    // D2: matching endpoint IDs do not permit a cross-time path.
    let e1 = edge("fixture.e1", "fixture.a", "fixture.b", actual(0, 1));
    let e2 = edge("fixture.e2", "fixture.b", "fixture.c", actual(2, 3));
    let left = [TemporalPath::from_edge(&e1, lim()).unwrap()];
    let right = [TemporalPath::from_edge(&e2, lim()).unwrap()];
    assert!(compose_paths(&left, &right, lim()).unwrap().is_empty());
    // D3: Starts(A,B) plus Before(B,A) has an actual strict-cycle certificate.
    let mut p = problem(2);
    p.constraints = vec![
        constraint(0, OrderOp::Equal, 2),
        constraint(1, OrderOp::Less, 3),
        constraint(3, OrderOp::Less, 0),
    ];
    let OrderOutcome::StrictCycle(cycle) = solve_order(&p, lim()).unwrap() else {
        panic!("cycle")
    };
    assert!(verify_order_cycle(&p, &cycle, lim()).unwrap());
    let mut ok = problem(2);
    ok.constraints.push(constraint(1, OrderOp::Less, 2));
    ok.anchors = vec![
        Anchor {
            variable: 0,
            value: q(0),
            source: pin("fixture.anchor0"),
        },
        Anchor {
            variable: 3,
            value: q(3),
            source: pin("fixture.anchor3"),
        },
    ];
    let OrderOutcome::Witness(values) = solve_order(&ok, lim()).unwrap() else {
        panic!("witness")
    };
    assert!(verify_order_witness(&ok, &values, lim()).unwrap());
}

#[test]
fn addressed_port_boundaries_and_n8() {
    // One unresolved edge cannot erase a known disjoint meet or break associativity.
    let a = edge(
        "fixture.unknown",
        "fixture.a",
        "fixture.b",
        WorldValidity::Unresolved {
            expression: pin("fixture.unknown-date"),
        },
    );
    let b = edge("fixture.b", "fixture.b", "fixture.c", actual(0, 1));
    let c = edge("fixture.c", "fixture.c", "fixture.d", actual(2, 3));
    let pa = [TemporalPath::from_edge(&a, lim()).unwrap()];
    let pb = [TemporalPath::from_edge(&b, lim()).unwrap()];
    let pc = [TemporalPath::from_edge(&c, lim()).unwrap()];
    let ab = compose_paths(&pa, &pb, lim()).unwrap();
    assert_eq!(ab[0].known_validity(), None);
    assert_eq!(ab[0].necessary_actual_constraint(), Some(span(0, 1)));
    assert!(compose_paths(&ab, &pc, lim()).unwrap().is_empty());
    assert!(
        compose_paths(&pa, &compose_paths(&pb, &pc, lim()).unwrap(), lim())
            .unwrap()
            .is_empty()
    );
    let mut other = edge(
        "fixture.other-scenario",
        "fixture.b",
        "fixture.c",
        actual(0, 1),
    );
    other.joint.scenario = pin("fixture.scenario-2");
    assert!(
        compose_paths(
            &pa,
            &[TemporalPath::from_edge(&other, lim()).unwrap()],
            lim()
        )
        .unwrap()
        .is_empty()
    );
    // Unbounded/missing/inclusive boundaries and possibility are never conflated.
    assert!(matches!(
        resolve_boundaries(
            &Boundary::Unknown,
            &Boundary::Unbounded {
                basis: pin("fixture.unbounded-basis")
            }
        )
        .unwrap(),
        BoundaryResolution::Unresolved { .. }
    ));
    assert!(Span::new(q(1), q(1), true, false).is_err());
    // Old reference's adjacent-segment predicate receives a precise name.
    let a = geometry(Geometry::Rectangle(rect(0, 0, 1, 1)));
    let b = geometry(Geometry::Rectangle(rect(1, 0, 2, 1)));
    assert!(topology(&a, &b, TopologyPredicate::AdjacentAlongSegment, lim()).unwrap());
    assert!(topology(&a, &a, TopologyPredicate::SharedBoundarySegment, lim()).unwrap());
    assert!(!topology(&a, &a, TopologyPredicate::AdjacentAlongSegment, lim()).unwrap());
    // Exact rational comparison never overflows cross-products.
    let big = i128::MAX;
    assert!(Rational::new(big - 1, big).unwrap() < Rational::new(big, big - 1).unwrap());
    assert!(matches!(
        q(big).checked_add(q(1)),
        Err(DomainError::NumericRange)
    ));
    // Complete relation image for two unconstrained proper dense-Q intervals.
    let free = problem(2);
    let RelationImage::Exact { possible } = possible_allen(&free, 0, 1, lim()).unwrap() else {
        panic!("nonempty model")
    };
    assert_eq!(possible.len(), 13);
    for (_, w) in possible {
        assert!(verify_order_witness(&free, &w, lim()).unwrap());
    }
    // Anchor provenance and every anchor value are validated, including duplicates.
    let mut p = problem(1);
    p.anchors = vec![
        Anchor {
            variable: 0,
            value: q(0),
            source: pin("fixture.a0"),
        },
        Anchor {
            variable: 0,
            value: q(1),
            source: pin("fixture.a1"),
        },
    ];
    let OrderOutcome::StrictCycle(mut cycle) = solve_order(&p, lim()).unwrap() else {
        panic!("anchor cycle")
    };
    assert!(verify_order_cycle(&p, &cycle, lim()).unwrap());
    let anchor_step = cycle
        .iter_mut()
        .find(|s| matches!(s.premise, Premise::AnchorOrder { .. }))
        .unwrap();
    anchor_step.premise = Premise::AnchorOrder {
        left_anchor: 1,
        right_anchor: 0,
    };
    assert!(verify_order_cycle(&p, &cycle, lim()).is_err());
    let mut many = problem(1);
    many.anchors = (0..100)
        .map(|_| Anchor {
            variable: 0,
            value: q(0),
            source: pin("fixture.repeated-anchor"),
        })
        .collect();
    let low = DomainLimits {
        max_work: 500,
        max_requested_bytes: 100_000,
        max_items: 100_000,
    };
    assert!(matches!(
        solve_order(&many, low),
        Err(DomainError::ResourceLimit)
    ));
    assert!(matches!(
        possible_allen(
            &free,
            0,
            1,
            DomainLimits {
                max_work: 1,
                max_requested_bytes: 1,
                max_items: 1
            }
        ),
        Err(DomainError::ResourceLimit)
    ));
}

#[test]
fn ts_censor_1_positive_support_is_not_actual_extent() {
    let observed = WorldValidity::ObservedPrefix {
        observed: span(0, 2),
        censor_basis: pin("fixture.censor"),
    };
    let states = [
        HistoricalState {
            exact: pin("fixture.observed-state"),
            subject: endpoint("fixture.place"),
            joint: joint("fixture.scenario-1"),
            axis: axis(),
            validity: observed,
            payload: &"observed",
        },
        HistoricalState {
            exact: pin("fixture.actual-state"),
            subject: endpoint("fixture.place"),
            joint: joint("fixture.scenario-1"),
            axis: axis(),
            validity: actual(0, 2),
            payload: &"actual",
        },
    ];
    let inside =
        select_states(&states, q(1), &axis(), &joint("fixture.scenario-1"), lim()).unwrap();
    assert_eq!(inside.included.len(), 2);
    assert!(inside.unresolved.is_empty());
    // At the open cutoff, after it and before the observed window: unresolved
    // observed state, but actual exhaustive extent can establish exclusion.
    for at in [q(2), q(3), q(-1)] {
        let out = select_states(&states, at, &axis(), &joint("fixture.scenario-1"), lim()).unwrap();
        assert!(out.included.is_empty());
        assert_eq!(out.unresolved.len(), 1);
        assert!(std::ptr::eq(out.unresolved[0], &states[0]));
    }
    let observed = |lo, hi| WorldValidity::ObservedPrefix {
        observed: span(lo, hi),
        censor_basis: pin("fixture.censor"),
    };
    let a = edge(
        "fixture.censored-a",
        "fixture.a",
        "fixture.b",
        observed(0, 1),
    );
    let b = edge(
        "fixture.censored-b",
        "fixture.b",
        "fixture.c",
        observed(2, 3),
    );
    let pa = [TemporalPath::from_edge(&a, lim()).unwrap()];
    let pb = [TemporalPath::from_edge(&b, lim()).unwrap()];
    assert_eq!(pa[0].positive_support(), Some(span(0, 1)));
    assert_eq!(pa[0].necessary_actual_constraint(), None);
    assert_eq!(pa[0].known_validity(), None);
    let ab = compose_paths(&pa, &pb, lim()).unwrap();
    assert_eq!(ab.len(), 1);
    assert_eq!(ab[0].positive_support(), None);
    assert_eq!(ab[0].necessary_actual_constraint(), None);
    assert_eq!(ab[0].known_validity(), None);
    assert!(ab[0].has_censored_observation());
    assert!(std::ptr::eq(ab[0].edges()[0], &a));
    assert!(std::ptr::eq(ab[0].edges()[1], &b));
    // Overlapping positive observations yield positive support only.
    let broad = edge(
        "fixture.censored-broad",
        "fixture.a",
        "fixture.b",
        observed(0, 4),
    );
    let broad = [TemporalPath::from_edge(&broad, lim()).unwrap()];
    let supported = compose_paths(&broad, &pb, lim()).unwrap();
    assert_eq!(supported[0].positive_support(), Some(span(2, 3)));
    assert_eq!(supported[0].known_validity(), None);
    // An actual bound outside the observed window remains a possible path;
    // both parenthesizations preserve it through another unknown edge.
    let b = edge("fixture.actual-b", "fixture.b", "fixture.c", actual(2, 3));
    let c = edge(
        "fixture.unknown-c",
        "fixture.c",
        "fixture.d",
        WorldValidity::Unresolved {
            expression: pin("fixture.unknown-date"),
        },
    );
    let pb = [TemporalPath::from_edge(&b, lim()).unwrap()];
    let pc = [TemporalPath::from_edge(&c, lim()).unwrap()];
    let ab = compose_paths(&pa, &pb, lim()).unwrap();
    assert_eq!(ab.len(), 1);
    let left = compose_paths(&ab, &pc, lim()).unwrap();
    let bc = compose_paths(&pb, &pc, lim()).unwrap();
    let right = compose_paths(&pa, &bc, lim()).unwrap();
    for paths in [&left, &right] {
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].necessary_actual_constraint(), Some(span(2, 3)));
        assert_eq!(paths[0].positive_support(), None);
        assert_eq!(paths[0].known_validity(), None);
        assert_eq!(paths[0].edges().len(), 3);
        for (edge, expected) in paths[0].edges().iter().zip([&a, &b, &c]) {
            assert!(std::ptr::eq(*edge, expected));
        }
    }
    // Actual disjoint extents still prove impossibility with a censored prefix.
    let d = edge("fixture.actual-d", "fixture.c", "fixture.d", actual(4, 5));
    let pd = [TemporalPath::from_edge(&d, lim()).unwrap()];
    assert!(compose_paths(&ab, &pd, lim()).unwrap().is_empty());
    assert!(
        compose_paths(&pa, &compose_paths(&pb, &pd, lim()).unwrap(), lim())
            .unwrap()
            .is_empty()
    );
}

// A deliberately synthetic actual-record store for the adapter test only.
struct FixtureStore {
    value_pin: ExactRecordRef,
    profile_pin: ExactRecordRef,
    value: TsOutputValue,
    profile: TsProfileBody,
}
impl TsOutputStore for FixtureStore {
    fn resolve_value<'a>(
        &'a self,
        _: &JsonValue,
        _: DomainLimits,
    ) -> std::result::Result<ResolvedRecord<'a, TsOutputValue>, DomainError> {
        Ok(ResolvedRecord {
            exact: &self.value_pin,
            body: &self.value,
        })
    }
    fn resolve_profile<'a>(
        &'a self,
        _: &JsonValue,
        _: DomainLimits,
    ) -> std::result::Result<ResolvedRecord<'a, TsProfileBody>, DomainError> {
        Ok(ResolvedRecord {
            exact: &self.profile_pin,
            body: &self.profile,
        })
    }
}
fn json(s: &str) -> JsonValue {
    parse_json(
        s.as_bytes(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root()
}
fn text(s: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(s))
}
fn reference(id: &str) -> JsonValue {
    json(&format!(
        r#"{{"id":"{id}","version":1,"digest":"sha256:{}"}}"#,
        "0".repeat(64)
    ))
}
fn object(entries: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn envelope() -> JsonValue {
    object(vec![
        ("schema_version", text("tos_operation_binding_v1")),
        ("run_id", text("fixture.ts-run")),
        ("operator_ref", reference("fixture.operator")),
        ("context_ref", reference("fixture.context")),
        ("selection_ref", reference("fixture.selection")),
        (
            "inputs",
            JsonValue::Array(vec![object(vec![
                ("slot", text("in")),
                ("profile_ref", reference("fixture.input-profile")),
                ("record_ref", reference("fixture.input")),
            ])]),
        ),
        (
            "dependency_manifest",
            object(vec![
                ("manifest_ref", reference("fixture.manifest")),
                ("observable_ref", reference("fixture.observable")),
                ("coverage", text("Incomplete")),
                ("entries", JsonValue::Array(vec![])),
            ]),
        ),
        (
            "value",
            object(vec![
                ("kind", text("Produced")),
                ("output_ref", reference("fixture.output")),
                ("output_profile_ref", reference("fixture.profile")),
            ]),
        ),
        ("execution", text("Completed")),
        (
            "applicability",
            object(vec![
                ("kind", text("ApplicabilityUnresolved")),
                ("reason", text("Synthetic test only")),
            ]),
        ),
        ("assessment", object(vec![("kind", text("NotRelevant"))])),
        ("certificates", JsonValue::Array(vec![])),
        ("warrant_refs", JsonValue::Array(vec![])),
        (
            "authority_boundary",
            text("StructuralBindingOnlyNoAdmissionOrProofVerification"),
        ),
    ])
}
#[test]
fn actual_output_profile_projection_is_required() {
    let e = envelope();
    let checked = check_operation_binding(
        &e,
        OperationBindingLimits {
            max_envelope_bytes: 1_000_000,
            max_nodes: 1_000_000,
            max_depth: 64,
            max_refs: 1_000,
        },
    )
    .unwrap();
    let limits = OutputCheckLimits {
        local: lim(),
        value_resolver: lim(),
        profile_resolver: lim(),
    };
    assert!(matches!(
        check_ts_output(&checked, None, TsCodomain::TopologyValue, limits),
        Err(DomainError::NeedsBinding(_))
    ));
    let mut store = FixtureStore {
        value_pin: pin("fixture.output"),
        profile_pin: pin("fixture.profile"),
        value: TsOutputValue::TopologyValue(true),
        profile: TsProfileBody {
            projection: TsProjection::DirectDomainValueV1,
            codomain: TsCodomain::TopologyValue,
        },
    };
    assert!(matches!(
        check_ts_output(&checked, Some(&store), TsCodomain::TopologyValue, limits),
        Ok(CheckedTsOutput::Produced { .. })
    ));
    assert!(matches!(
        check_ts_output(&checked, Some(&store), TsCodomain::SpanMeet, limits),
        Err(DomainError::Incompatible(_))
    ));
    store.value_pin.version = RecordVersion::new(2).unwrap();
    assert!(matches!(
        check_ts_output(&checked, Some(&store), TsCodomain::TopologyValue, limits),
        Err(DomainError::Incompatible(_))
    ));
    store.value_pin.version = RecordVersion::new(1).unwrap();
    store.profile.codomain = TsCodomain::RelationImage;
    store.value = TsOutputValue::RelationImage(RelationImage::Exact { possible: vec![] });
    assert!(matches!(
        check_ts_output(&checked, Some(&store), TsCodomain::RelationImage, limits),
        Err(DomainError::Invalid(_))
    ));
}
