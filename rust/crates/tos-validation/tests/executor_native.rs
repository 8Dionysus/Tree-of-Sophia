//! Native end-to-end probe of the dedicated, bounded schema worker.
//! A green probe is not a source-admission result.

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
mod linux {
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use tos_foundation::Digest256;
    use tos_validation::executor::{
        BoundedSchemaExecutor, ExactWorkerIdentity, ExecutorBudget, ExecutorOutcome,
    };
    use tos_validation::{FormatProfile, SchemaResource};

    fn worker() -> (ExactWorkerIdentity, usize, Duration) {
        let path = PathBuf::from(env!("CARGO_BIN_EXE_tos-schema-worker"));
        let started = Instant::now();
        let image = std::fs::read(&path).expect("Cargo-built dedicated worker");
        let image_bytes = image.len();
        let identity = ExactWorkerIdentity {
            absolute_path: path,
            sha256: Digest256::of_bytes(&image),
        };
        (identity, image_bytes, started.elapsed())
    }

    fn resources() -> Vec<SchemaResource> {
        vec![SchemaResource {
            uri: "https://treeofsophia.local/tests/integer.schema.json".to_owned(),
            raw: br#"{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://treeofsophia.local/tests/integer.schema.json","type":"integer"}"#.to_vec(),
        }]
    }

    #[test]
    fn candidate_scalar_reuse_preserves_exact_inputs_and_budget_refusals() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use tos_validation::executor::VerifiedWorkerImageHandle;
        use tos_validation::item_rules::ItemRefusal;
        use tos_validation::source_cut::{
            CandidateCutWorkerSchemaExecutor, CutSchemaExecutor, CutWorkerLimits,
        };

        let (identity, _, _) = worker();
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(30);
        let budget = ExecutorBudget::laboratory();
        let image = VerifiedWorkerImageHandle::prepare(identity, budget, deadline, &cancelled)
            .expect("exact sealed worker");
        let contract = "ToS/contracts/scalar-reuse.schema.json";
        let selected = vec![(
            contract.to_owned(),
            SchemaResource {
                uri: format!("https://tree-of-sophia.local/{contract}"),
                raw: br#"{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://tree-of-sophia.local/ToS/contracts/scalar-reuse.schema.json","type":"integer","$defs":{"integer":{"type":"integer"}}}"#.to_vec(),
            },
        )];
        let mut candidate =
            CandidateCutWorkerSchemaExecutor::from_asserted_source_candidate_resources(
                17u64,
                selected,
                &image,
                budget,
                CutWorkerLimits {
                    max_receipts: 2,
                    max_receipt_bytes: 4096,
                },
                deadline,
                &cancelled,
            )
            .unwrap();
        // Real valid and invalid executions exhaust the two-check budget.
        assert!(
            candidate
                .check("subject", b"7", contract, deadline, &cancelled)
                .unwrap()
        );
        assert!(
            !candidate
                .check_reusing_scalar("subject", br#""x""#, contract, deadline, &cancelled)
                .unwrap()
        );
        for _ in 0..3 {
            assert!(
                candidate
                    .check_reusing_scalar("subject", b"7", contract, deadline, &cancelled)
                    .unwrap()
            );
            assert!(
                !candidate
                    .check_reusing_scalar("subject", br#""x""#, contract, deadline, &cancelled)
                    .unwrap()
            );
        }
        // Same decoded value with different bytes, a different locator, and a
        // different schema fragment each need another actual execution.
        for (path, raw, root) in [
            ("subject", b" 7".as_slice(), contract.to_owned()),
            ("other", b"7".as_slice(), contract.to_owned()),
            (
                "subject",
                b"7".as_slice(),
                format!("{contract}#/$defs/integer"),
            ),
        ] {
            assert!(matches!(
                candidate.check_reusing_scalar(path, raw, &root, deadline, &cancelled),
                Err(ItemRefusal::BudgetCheck {
                    check: "Cut scalar receipt count",
                    used: Some(3),
                    limit: Some(2)
                })
            ));
        }
        assert!(
            candidate
                .check_reusing_scalar(
                    "subject",
                    b"7",
                    "ToS/contracts/absent.schema.json",
                    deadline,
                    &cancelled
                )
                .is_err()
        );
        assert_eq!(*candidate.input_identity(), 17);
        assert_eq!(candidate.selected_source_revision(), None);
        // Reuse cannot bypass cancellation, even for an already valid input.
        cancelled.store(true, Ordering::Relaxed);
        assert!(
            candidate
                .check_reusing_scalar("subject", b"7", contract, deadline, &cancelled)
                .is_err()
        );
    }

    #[test]
    fn dedicated_worker_evaluates_exact_raw_instances_and_rejects_duplicate_members() {
        let (worker, image_bytes, identity_elapsed) = worker();
        let resources = resources();
        let profile = FormatProfile::AssertedSourceCandidateV1;
        let root_uri = resources[0].uri.as_str();
        let budget = ExecutorBudget::laboratory();
        let evaluation_started = Instant::now();
        let valid =
            BoundedSchemaExecutor::evaluate(&worker, &resources, profile, root_uri, b"7", budget);
        let evaluation_elapsed = evaluation_started.elapsed();
        let ExecutorOutcome::SchemaValid(valid_identity) = valid else {
            eprintln!(
                "worker probe failure: image_bytes={image_bytes} identity_read_hash_ms={} evaluation_ms={} profile={profile:?} execution_wall_ms={} cleanup_grace_ms={} cpu_seconds={} address_space_bytes={}",
                identity_elapsed.as_millis(),
                evaluation_elapsed.as_millis(),
                budget.execution_wall.as_millis(),
                budget.cleanup_grace.as_millis(),
                budget.cpu_seconds,
                budget.address_space_bytes,
            );
            panic!("real worker did not complete valid instance: {valid:?}");
        };
        assert_eq!(valid_identity.worker_sha256, worker.sha256);
        assert_eq!(valid_identity.instance_sha256, Digest256::of_bytes(b"7"));
        assert_eq!(valid_identity.profile, profile);
        let invalid = BoundedSchemaExecutor::evaluate(
            &worker, &resources, profile, root_uri, br#""x""#, budget,
        );
        let ExecutorOutcome::SchemaInvalid(invalid_identity) = invalid else {
            panic!("real worker did not complete invalid instance: {invalid:?}");
        };
        assert_eq!(invalid_identity.worker_sha256, worker.sha256);
        assert_eq!(
            invalid_identity.schema_set_sha256,
            valid_identity.schema_set_sha256
        );
        assert_eq!(
            invalid_identity.instance_sha256,
            Digest256::of_bytes(br#""x""#)
        );
        assert_eq!(invalid_identity.profile, profile);
        let object_schema = vec![SchemaResource {
            uri: "https://treeofsophia.local/tests/object.schema.json".to_owned(),
            raw: br#"{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://treeofsophia.local/tests/object.schema.json","type":"object"}"#.to_vec(),
        }];
        assert!(matches!(
            BoundedSchemaExecutor::evaluate(
                &worker,
                &object_schema,
                profile,
                &object_schema[0].uri,
                br#"{"a":1,"\u0061":2}"#,
                budget,
            ),
            ExecutorOutcome::InputRejected(_)
        ));
    }
}
