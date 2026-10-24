//! One addressed source target. Unexecuted in the source-only delivery.
//! Local v1.2.0 construction fixtures are preserved candidate records, not
//! fabricated native assessments, registered source profiles or authority.
use super::*;
use serde_json::{Value, json};
use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use tos_foundation::{JsonLimits, JsonMode, OperationBindingLimits, parse_json};

fn session(cancel: &AtomicBool) -> Session<'_> {
    Session::new(
        Limits {
            max_input_bytes: 4_000_000,
            max_nodes: 8_000_000,
            max_work: usize::MAX / 4,
            max_refs: 20_000,
            max_depth: 96,
            max_items: 1024,
            max_state_bytes: 1_000_000_000,
            deadline: Instant::now() + Duration::from_secs(30),
        },
        cancel,
    )
    .unwrap()
}
#[test]
fn source_session_keeps_original_admission_and_spent_prefixes() {
    use std::cell::Cell;
    // Numerical accounting only: no source/owner/frame/assessment fixture.
    struct NumericSource {
        read: Cell<usize>,
        live: Cell<usize>,
    }
    impl SourceAdmission for NumericSource {
        fn inspect(&self, bytes: usize) -> Result<()> {
            self.read.set(
                self.read
                    .get()
                    .checked_sub(bytes)
                    .ok_or(Error::ResourceLimit)?,
            );
            Ok(())
        }
        fn retain(&self, bytes: usize) -> Result<()> {
            self.live.set(
                self.live
                    .get()
                    .checked_sub(bytes)
                    .ok_or(Error::ResourceLimit)?,
            );
            Ok(())
        }
    }
    let cancel = AtomicBool::new(false);
    let payer = NumericSource {
        read: Cell::new(5),
        live: Cell::new(2),
    };
    let other = NumericSource {
        read: Cell::new(5),
        live: Cell::new(2),
    };
    let limits = Limits {
        max_input_bytes: 100,
        max_nodes: 100,
        max_work: 100,
        max_refs: 10,
        max_depth: 4,
        max_items: 10,
        max_state_bytes: 100,
        deadline: Instant::now() + Duration::from_secs(30),
    };
    let mut source = Session::new_source(limits, &cancel, &payer).unwrap();
    assert!(source.has_source_payer(&payer));
    assert!(!source.has_source_payer(&other));
    source.admit_source_prefix(3, 1).unwrap();
    assert_eq!(payer.read.get(), 2);
    assert!(source.admit_source_prefix(3, 1).is_err());
    assert_eq!(source.charges(), (3, 1, 8, 0, 0));
    assert_eq!(payer.read.get(), 2); // failure cannot refill prior spend
    source.retain_source_state(2).unwrap();
    assert!(source.retain_source_state(1).is_err());
    assert_eq!(source.charges().4, 3); // local prefix survives payer refusal
    assert_eq!(payer.live.get(), 0);
    assert_eq!(source.source_allowance().unwrap().work, 92);
    let mut supplied = Session::new(limits, &cancel).unwrap();
    assert!(supplied.admit_source_prefix(1, 1).is_err());
    assert_eq!(supplied.charges(), (0, 0, 0, 0, 0));
}
fn k0() -> OperationBindingLimits {
    OperationBindingLimits {
        max_envelope_bytes: 100_000,
        max_nodes: 1_000_000,
        max_depth: 64,
        max_refs: 1000,
    }
}

/// Resource/numeric regression only. No native owner, grant, profile or
/// completeness witness is constructed. I is the unchanged SOURCE ceiling.
#[test]
fn source_numeric_canonical_and_revision_share_original_allowance() {
    use std::cell::Cell;
    const I: usize = 33_554_432;
    struct Original {
        read: Cell<usize>,
        live: Cell<usize>,
        fail_after: Cell<Option<usize>>,
    }
    impl SourceAdmission for Original {
        fn inspect(&self, n: usize) -> Result<()> {
            if let Some(left) = self.fail_after.get() {
                if left == 0 {
                    self.fail_after.set(None);
                    return Err(Error::Upstream("original numeric payer refusal"));
                }
                self.fail_after.set(Some(left - 1));
            }
            self.read
                .set(self.read.get().checked_sub(n).ok_or(Error::ResourceLimit)?);
            Ok(())
        }
        fn retain(&self, n: usize) -> Result<()> {
            self.live
                .set(self.live.get().checked_sub(n).ok_or(Error::ResourceLimit)?);
            Ok(())
        }
    }
    let original = Original {
        read: Cell::new(I),
        live: Cell::new(I),
        fail_after: Cell::new(None),
    };
    let cancel = AtomicBool::new(false);
    let limits = Limits {
        max_input_bytes: I,
        max_work: I,
        max_nodes: 1_000_000,
        max_refs: 4096,
        max_items: 4096,
        max_depth: 96,
        max_state_bytes: I,
        deadline: Instant::now() + Duration::from_secs(30),
    };
    let mut s = Session::new_source(limits, &cancel, &original).unwrap();
    // Versions 1/2 and their number equality pass with max_work == max_input;
    // neither count nor digest can request the original I after prior spend.
    s.admit_source_prefix(7, 1).unwrap();
    s.admit(&json!(1)).unwrap();
    s.admit(&json!(2)).unwrap();
    assert!(s.equal(&json!(1), &json!(1)).unwrap());
    assert!(!s.equal(&json!(1), &json!(2)).unwrap());
    let one_point_zero: Value = serde_json::from_str("1.0").unwrap();
    assert_eq!(
        s.equal(&json!(1), &one_point_zero).unwrap(),
        json!(1) == one_point_zero
    );
    let pin = exact("numeric.bound");
    s.admit(&pin).unwrap();
    assert_eq!(s.exact(&pin).unwrap().version(), 1);
    let mut pin2 = pin.clone();
    pin2["version"] = json!(2);
    s.admit(&pin2).unwrap();
    assert_eq!(s.exact(&pin2).unwrap().version(), 2);
    let mut bound_v2 = typed("numeric.bound");
    bound_v2["record"]["version"] = json!(2);
    s.admit(&bound_v2).unwrap();
    assert_eq!(s.typed(&bound_v2).unwrap().1.version(), 2);
    for input in [
        "1",
        "2",
        "1.0",
        "1e15",
        "1e-4",
        "-0.0",
        "18446744073709551616",
        "{\"z\":[1,2,0.5],\"a\":\"Ж\\n\"}",
    ] {
        let value: Value = serde_json::from_str(input).unwrap();
        let expected = crate::source_bibliographic_render::encode(&value, I).unwrap();
        let before = s.charges();
        let before_read = original.read.get();
        let actual = s.source_canonical(&value).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            s.digest(&value).unwrap(),
            tos_foundation::Digest256::of_bytes(&expected)
        );
        assert!(s.charges().2 > before.2 && original.read.get() < before_read);
    }
    let prepared: Value =
        serde_json::from_str(include_str!("fixtures/prepared-pwe-values.json")).unwrap();
    let object = prepared["body"]["values"][0].clone();
    let old = json!({"schema_version":"tos_pwe_name_assignment_claim_v1",
        "claim_id":"tos.claim.numeric-source", "claim_type":"relation",
        "assertion_layer":"bibliographic_assertion", "subject_ref":object["subject"]["record"]["id"],
        "predicate":"pwe_name_assignment", "object":object, "evidence_refs":["numeric.original"],
        "maker":{"maker_type":"software","agent_ref":"numeric.original"},
        "provenance_event_ref":"numeric.original", "epistemic_status":"reported",
        "review_status":"unreviewed", "claim_version":1, "visibility":"public_metadata_only"});
    let mut new = old.clone();
    new["claim_version"] = json!(2);
    new["object"]["limitations"] = json!("Numeric resource regression; description only.");
    let p = s.prepare(&old["object"]).unwrap();
    s.check_claim_content(&p, &old).unwrap();
    let occurrences = s.preserved_basis(&p).unwrap();
    assert!(!occurrences.slots.is_empty());
    let p2 = s.prepare(&new["object"]).unwrap();
    s.check_claim_content(&p2, &new).unwrap();
    assert_eq!(
        s.source_canonical(&old).unwrap(),
        crate::source_bibliographic_render::encode(&old, I).unwrap()
    );
    assert_eq!(
        s.source_canonical(&new).unwrap(),
        crate::source_bibliographic_render::encode(&new, I).unwrap()
    );
    let old_ref =
        json!({"id":old["claim_id"],"version":1,"digest":s.digest(&old).unwrap().to_prefixed()});
    let new_ref =
        json!({"id":new["claim_id"],"version":2,"digest":s.digest(&new).unwrap().to_prefixed()});
    for _ in 0..2 {
        let before = s.charges();
        s.description_revision(&old, &old_ref, &new, &new_ref)
            .unwrap();
        assert!(s.charges().2 > before.2 && s.charges().4 > before.4);
    }
    let mut changed = new.clone();
    changed["evidence_refs"] = json!(["numeric.changed"]);
    let changed_ref = json!({"id":changed["claim_id"],"version":2,"digest":s.digest(&changed).unwrap().to_prefixed()});
    assert!(matches!(
        s.description_revision(&old, &old_ref, &changed, &changed_ref),
        Err(Error::Malformed(_))
    ));
    // The SOURCE descriptor path is admitted separately; invalid content does
    // not become a descriptor merely because its resource port now exists.
    let null = Value::Null;
    let registry = RegistryDescriptor {
        value: &null,
        reference: s.exact(&pin).unwrap(),
    };
    assert!(matches!(
        s.metadata(&null, &null, &null, &registry),
        Err(Error::Malformed(_))
    ));
    // Three successful prepass/wire debits precede the first parser prefix.
    // Its original error must survive the generic foundation error wrapper.
    let before = s.charges();
    original.fail_after.set(Some(3));
    assert_eq!(
        s.digest(&json!(1)),
        Err(Error::Upstream("original numeric payer refusal"))
    );
    assert!(s.charges().2 > before.2); // spent prepass prefix is retained
    let before_retry = s.charges();
    s.admit(&json!(2)).unwrap();
    assert!(s.charges().2 > before_retry.2); // SAME Session, no refill
    let mut exhausted_limits = limits;
    exhausted_limits.max_work = 2;
    let mut exhausted = Session::new_source(exhausted_limits, &cancel, &original).unwrap();
    assert_eq!(exhausted.admit(&json!(12)), Err(Error::ResourceLimit));
    assert_eq!(exhausted.charges().2, 1);
    assert_eq!(exhausted.admit(&json!(12)), Err(Error::ResourceLimit));
    assert_eq!(exhausted.charges().2, 2);
}
fn native(v: &Value) -> tos_foundation::JsonValue {
    parse_json(
        &serde_json::to_vec(v).unwrap(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root()
}
fn exact(id: &str) -> Value {
    json!({"id":id,"version":1,"digest":format!("sha256:{}","0".repeat(64))})
}
fn typed(id: &str) -> Value {
    json!({"kind_binding":exact("fixture.kind"),"record":exact(id)})
}
struct FixtureJointOwner;
impl JointCarrierOwner for FixtureJointOwner {
    fn current_fence(&mut self, c: &JointCarrier<'_>) -> Result<()> {
        if c.reference["id"] == "fixture.carrier" {
            Ok(())
        } else {
            Err(Error::Denied)
        }
    }
    fn admissible_world(&mut self, c: &JointCarrier<'_>, w: &JointWorld<'_>) -> Result<()> {
        self.current_fence(c)?;
        if matches!(
            w.world_ref["id"].as_str(),
            Some("fixture.world-a" | "fixture.world-b")
        ) {
            Ok(())
        } else {
            Err(Error::Denied)
        }
    }
}

#[test]
fn pwe_native_contract() {
    let cancel = AtomicBool::new(false);
    let record: Value =
        serde_json::from_str(include_str!("fixtures/prepared-pwe-values.json")).unwrap();
    let profile: Value =
        serde_json::from_str(include_str!("fixtures/prepared-values-profile.json")).unwrap();
    let wrapper_schema: Value = serde_json::from_str(include_str!(
        "fixtures/prepared-values-record-schema-specification.json"
    ))
    .unwrap();
    let container_schema: Value = serde_json::from_str(include_str!(
        "fixtures/preserved-candidate-schema-specification.json"
    ))
    .unwrap();
    let item_schema: Value = serde_json::from_str(include_str!(
        "fixtures/single-value-schema-specification.json"
    ))
    .unwrap();
    let envelope: Value =
        serde_json::from_str(include_str!("fixtures/construction-envelope.json")).unwrap();
    let schema_records = [&wrapper_schema, &container_schema, &item_schema];
    let n = native(&envelope);
    let mut s = session(&cancel);
    let projected = s
        .preserved_candidate(&n, k0(), &profile, &record, schema_records)
        .unwrap();
    assert_eq!(projected.values().len(), 3);
    assert_eq!(projected.body(), &record["body"]);
    assert!(!projected.semantic_resolution_executed());
    // All three items still pass, but that does not validate the container.
    let mut wrong = record.clone();
    wrong["record_version"] = json!(3);
    wrong["body"]["semantic_resolution_executed"] = json!(true);
    for item in wrong["body"]["values"].as_array().unwrap() {
        session(&cancel).prepare(item).unwrap();
    }
    let mut wrong_envelope = envelope.clone();
    wrong_envelope["value"]["output_ref"]["version"] = json!(3);
    wrong_envelope["value"]["output_ref"]["digest"] =
        json!(session(&cancel).digest(&wrong).unwrap().to_prefixed());
    let n = native(&wrong_envelope);
    assert!(matches!(
        session(&cancel).preserved_candidate(&n, k0(), &profile, &wrong, schema_records),
        Err(Error::Malformed(
            "producer cannot declare semantic resolution"
        ))
    ));
    // A profile with a different projection has a valid new exact digest but
    // cannot pass this adapter's declared wrapper -> /body contract.
    let mut wrong_profile = profile.clone();
    wrong_profile["body"]["projection"]["pointer"] = json!("/values");
    let mut wrong_envelope = envelope;
    wrong_envelope["value"]["output_profile_ref"]["digest"] = json!(
        session(&cancel)
            .digest(&wrong_profile)
            .unwrap()
            .to_prefixed()
    );
    let n = native(&wrong_envelope);
    assert!(matches!(
        session(&cancel).preserved_candidate(&n, k0(), &wrong_profile, &record, schema_records),
        Err(Error::Malformed("fixed profile constant"))
    ));

    // Marginal witnesses in different worlds are not a joint witness. Even
    // all observed positives do not issue universal/completeness evidence.
    let a = typed("fixture.person-a");
    let b = typed("fixture.person-b");
    let wa = exact("fixture.world-a");
    let wb = exact("fixture.world-b");
    let aa = [a.clone()];
    let bb = [b.clone()];
    let worlds = [
        JointWorld {
            world_ref: &wa,
            positive: &aa,
            negative: &[],
        },
        JointWorld {
            world_ref: &wb,
            positive: &bb,
            negative: &[],
        },
    ];
    let carrier_ref = exact("fixture.carrier");
    let context = typed("fixture.context");
    let scope = typed("fixture.scope");
    let carrier = JointCarrier {
        reference: &carrier_ref,
        context: &context,
        scope: &scope,
    };
    let mut owner = FixtureJointOwner;
    assert_eq!(
        session(&cancel)
            .attribution(
                &carrier,
                AttributionQuestion::PossibleJoint(&a, &b),
                &worlds,
                &mut owner
            )
            .unwrap()
            .answer,
        AttributionAnswer::NoObservedWitness
    );
    assert!(matches!(
        session(&cancel)
            .attribution(
                &carrier,
                AttributionQuestion::Possible(&a),
                &worlds,
                &mut owner
            )
            .unwrap()
            .answer,
        AttributionAnswer::Witness(_)
    ));
    assert_eq!(
        session(&cancel)
            .attribution(
                &carrier,
                AttributionQuestion::Necessary(&a),
                &worlds,
                &mut owner
            )
            .unwrap()
            .answer,
        AttributionAnswer::NeedsCoverage
    );
    assert_eq!(
        session(&cancel)
            .attribution(
                &carrier,
                AttributionQuestion::Excluded(&a),
                &worlds,
                &mut owner
            )
            .unwrap()
            .answer,
        AttributionAnswer::Counterexample(session(&cancel).exact(&wa).unwrap())
    );
}

/// Synthetic source wrapper around an unchanged local candidate. It is not a
/// canonical Claim, registered source profile, assessment or authority fixture.
#[test]
fn description_revision_preserves_source_evidence() {
    let cancel = AtomicBool::new(false);
    let prepared: Value =
        serde_json::from_str(include_str!("fixtures/prepared-pwe-values.json")).unwrap();
    let object = prepared["body"]["values"][0].clone();
    let old = json!({
        "schema_version":"tos_pwe_name_assignment_claim_v1","claim_id":"tos.claim.fixture-pwe-description",
        "claim_type":"relation","assertion_layer":"bibliographic_assertion",
        "subject_ref":object["subject"]["record"]["id"],"predicate":"pwe_name_assignment","object":object,
        "evidence_refs":["tos.file.fixture-source"],
        "supporting_quotes":[{"anchor_ref":"tos.anchor.fixture","exact":"fixture wording","translation_posture":"source"}],
        "maker":{"maker_type":"software","agent_ref":"fixture.pwe-d2"},
        "provenance_event_ref":"tos.event.fixture","epistemic_status":"reported","review_status":"unreviewed",
        "claim_version":1,"visibility":"public_metadata_only","polarity":"positive",
        "confidence":{"value":0.5,"meaning":"maker_declared_uncertainty_not_truth_probability"},
        "alternative_claim_refs":["tos.claim.fixture-alternative"],"counterevidence_refs":["tos.file.fixture-counterevidence"],
        "qualifiers":{"source_context":"fixture"},"supersedes_claim_ref":null,"assessment_refs":[],
        "extensions":{"source_note":"opaque fixture source","nullable":null,"empty":[]}
    });
    let reference = |claim: &Value| -> Value {
        json!({"id":claim["claim_id"],"version":claim["claim_version"],
        "digest":session(&cancel).digest(claim).unwrap().to_prefixed()})
    };
    let old_ref = reference(&old);
    let mut description = old.clone();
    description["claim_version"] = json!(2);
    description["object"]["limitations"] =
        json!("Fixture explanation revised; source and proposition remain unchanged.");
    description["object"]["account"] = json!("Fixture presentation correction only.");
    let description_ref = reference(&description);
    session(&cancel)
        .description_revision(&old, &old_ref, &description, &description_ref)
        .unwrap();
    // Same-ID next-version records with fresh exact refs: rejection must be
    // caused by the description-only guard, not stale digest/version checks.
    for (key, value) in [
        ("evidence_refs", json!(["tos.file.fixture-other-source"])),
        ("evidence_refs", json!([])),
        ("evidence_refs", Value::Null),
        ("supporting_quotes", json!([])),
        ("supporting_quotes", Value::Null),
        (
            "supporting_quotes",
            json!([{"anchor_ref":"tos.anchor.fixture","exact":"changed fixture wording","translation_posture":"source"}]),
        ),
        ("provenance_event_ref", json!("tos.event.fixture-other")),
        (
            "maker",
            json!({"maker_type":"software","agent_ref":"fixture.other"}),
        ),
        ("counterevidence_refs", json!([])),
        ("alternative_claim_refs", json!([])),
        ("qualifiers", json!({"source_context":"changed"})),
        (
            "confidence",
            json!({"value":0.7,"meaning":"maker_declared_uncertainty_not_truth_probability"}),
        ),
        ("epistemic_status", json!("inferred")),
        ("review_status", json!("accepted")),
        (
            "supersedes_claim_ref",
            json!("tos.claim.fixture-predecessor"),
        ),
        ("assessment_refs", json!([exact("fixture.assessment")])),
        ("schema_version", json!("fixture_other_schema")),
        ("visibility", json!("public")),
        ("polarity", json!("unknown")),
        ("extensions", json!({})),
    ] {
        let mut changed = description.clone();
        changed[key] = value;
        let changed_ref = reference(&changed);
        assert_eq!(
            session(&cancel).description_revision(&old, &old_ref, &changed, &changed_ref),
            Err(Error::Malformed(
                "description revision protected top-level field"
            )),
            "{key}"
        );
    }
    // Do not collapse optional absent/null/empty to one default state; additions
    // of opaque source fields and equal-size key replacements are also refused.
    for key in [
        "evidence_refs",
        "supporting_quotes",
        "supersedes_claim_ref",
        "assessment_refs",
        "extensions",
        "polarity",
    ] {
        let mut changed = description.clone();
        changed.as_object_mut().unwrap().remove(key);
        let changed_ref = reference(&changed);
        assert_eq!(
            session(&cancel).description_revision(&old, &old_ref, &changed, &changed_ref),
            Err(Error::Malformed("description revision top-level field set")),
            "removed {key}"
        );
    }
    let mut changed = description.clone();
    changed["unknown_source_field"] = json!([]);
    let changed_ref = reference(&changed);
    assert_eq!(
        session(&cancel).description_revision(&old, &old_ref, &changed, &changed_ref),
        Err(Error::Malformed("description revision top-level field set"))
    );
    let mut changed = description.clone();
    changed.as_object_mut().unwrap().remove("extensions");
    changed["unknown_source_field"] = Value::Null;
    let changed_ref = reference(&changed);
    assert!(
        session(&cancel)
            .description_revision(&old, &old_ref, &changed, &changed_ref)
            .is_err()
    );
}

/// Pure source-reader regression over the retained local candidates. This is
/// not a native owner, grant, assessment or admitted production-source fixture.
#[test]
fn native_reader_preserves_unknown_and_ordered_exact_slots() {
    let cancel = AtomicBool::new(false);
    let record: Value =
        serde_json::from_str(include_str!("fixtures/prepared-pwe-values.json")).unwrap();
    let value = &record["body"]["values"][0];
    let mut s = session(&cancel);
    let account = s.prepare(value).unwrap();
    let basis = s.preserved_basis(&account).unwrap();
    assert!(!basis.slots.is_empty());
    assert!(!basis.unresolved.is_empty());
    assert_eq!(basis.slots[0].role, "subject");
    assert_eq!(basis.slots[0].inner, 0);
    assert_eq!(basis.slots[1].role, "subject");
    assert_eq!(basis.slots[1].inner, 1);
    assert!(
        basis
            .slots
            .iter()
            .enumerate()
            .any(|(i, a)| basis.slots[..i].iter().any(|b| a.reference == b.reference))
    );
    for missing in &basis.unresolved {
        assert_eq!(missing.binding["state"], "unresolved");
        assert!(missing.binding["source_wording"].as_str().is_some());
    }
    assert!(matches!(
        s.exact_basis(&account),
        Err(Error::NeedsBinding(_))
    ));
}
