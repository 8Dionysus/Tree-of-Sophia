//! Optional exact-model probe for component parser lifetime overlap. The caller
//! pins an existing candidate; this test never builds or publishes a model.
use super::*;
use crate::knowledge_payload_read::{
    RuntimeKnowledgeOwnedBudget, RuntimeKnowledgeReadUsage, with_runtime_knowledge_read_context,
};
use std::{
    cell::Cell,
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[test]
#[ignore = "requires explicitly pinned real model and selection"]
fn real_model_components_drop_graph_parser_before_originals() {
    const CHILD: &str = "TOS_COMPONENT_BUDGET_CHILD";
    const TEST: &str = "controlled_cold_model::component_budget_tests::real_model_components_drop_graph_parser_before_originals";
    if std::env::var_os(CHILD).is_none() {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--ignored", "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        eprint!("{}", String::from_utf8_lossy(&result.stderr));
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        return;
    }
    let model = std::env::var("TOS_COMPONENT_MODEL").unwrap();
    let selection = std::fs::read(std::env::var("TOS_COMPONENT_SELECTION").unwrap()).unwrap();
    assert_eq!(
        Digest256::of_bytes(&selection).to_hex(),
        std::env::var("TOS_COMPONENT_SELECTION_SHA256").unwrap()
    );
    let selected: serde_json::Value = serde_json::from_slice(&selection).unwrap();
    let expected: KnowledgeSelectedExpectation =
        serde_json::from_value(selected["expectation"].clone()).unwrap();
    let limits: ColdOpenLimits = serde_json::from_value(selected["cold_limits"].clone()).unwrap();
    let mut file = File::open(&model).unwrap();
    assert_eq!(file.metadata().unwrap().len(), expected.model_size_bytes);
    let mut digest = tos_foundation::Digest256Hasher::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    assert_eq!(digest.finalize().to_hex(), expected.model_sha256);
    // A deliberately finite component budget below the real producer's whole
    // 512 MiB budget, which also retains its source capture and SQLite heap.
    const CAP: usize = 250 * 1024 * 1024;
    let peak = Cell::new(0usize);
    let remaining = |bytes: usize| {
        peak.set(peak.get().max(bytes));
        CAP.checked_sub(bytes)
            .ok_or(Error::Budget("component probe retained state"))
    };
    let deadline = Instant::now() + Duration::from_secs(300);
    let cancelled = Arc::new(AtomicBool::new(false));
    let heap = crate::sqlite_budget::DedicatedSessionSqliteHeap::establish(
        64 * 1024 * 1024,
        &remaining,
        deadline,
        &cancelled,
    )
    .unwrap();
    let db = rusqlite::Connection::open_with_flags(
        &model,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .unwrap();
    let work = Arc::new(AtomicU64::new(0));
    let vm = Arc::new(AtomicU64::new(0));
    let budget = RuntimeKnowledgeOwnedBudget {
        remaining_after_retained: &remaining,
        original_work: &work,
        original_work_limit: 2 * limits.max_work_bytes,
        original_sql_vm: &vm,
        original_sql_vm_limit: limits.max_vm_steps,
        original_sqlite_heap: &heap,
        remaining_json_visits: 100_000_000,
        owner_deadline: deadline,
        operation_deadline: deadline,
        cancelled: &cancelled,
    };
    let roots = with_runtime_knowledge_read_context(
        &budget,
        &mut RuntimeKnowledgeReadUsage::default(),
        |context| {
            crate::knowledge_selected::configure_selected_sql_with_owned_context(
                &db, limits, context,
            )?;
            crate::knowledge_selected::verify_core_and_scope_with_owned_context(
                &db,
                &expected,
                limits,
                KnowledgePayloadLayout::from_model_abi(&expected.model_abi),
                context,
            )
        },
    )
    .unwrap();
    let escaping_basis = std::mem::size_of::<KnowledgeSourceBasis>() + limits.max_row_bytes;
    let escaping_navigation =
        std::mem::size_of::<crate::NavigationOriginalReceipt>() + 8 * 4096 + 4096;
    peak.set(0);
    let legacy = with_runtime_knowledge_read_context(
        &budget,
        &mut RuntimeKnowledgeReadUsage::default(),
        |context| {
            let state = context.owned_state();
            let _outputs = state.hold(escaping_basis + escaping_navigation)?;
            crate::knowledge_selected::verify_native_graph_root_with_owned_context(
                &db,
                &expected,
                limits,
                roots.0,
                roots.1,
                context,
                |_| {
                    crate::knowledge_selected::verify_catalog_with_owned_context(
                        &db, &expected, limits, context,
                    )?;
                    crate::knowledge_philosophy_original::verify_with_owned_state(
                        &db,
                        &expected,
                        limits,
                        &mut 0,
                        Some(state),
                    )?;
                    crate::knowledge_corpus_original::verify_with_owned_state(
                        &db,
                        &expected,
                        limits,
                        &mut 0,
                        Some(state),
                    )?;
                    crate::knowledge_navigation_original::verify_with_owned_state(
                        &db, &expected, limits, context,
                    )?;
                    Ok(())
                },
            )
        },
    );
    eprintln!(
        "legacy nested components: result={legacy:?} attempted_peak={} cap={CAP}",
        peak.get()
    );
    legacy.unwrap();
    let nested_peak = peak.get();
    // New independent verification session, same source bytes and finite caps.
    // Its component phases share the one state and existing work/VM counters.
    peak.set(0);
    let before = work.load(Ordering::Acquire);
    let verified = with_runtime_knowledge_read_context(
        &budget,
        &mut RuntimeKnowledgeReadUsage::default(),
        |context| {
            let _outputs = context
                .owned_state()
                .hold(escaping_basis + escaping_navigation)?;
            verify_owned_components(
                &db,
                &expected,
                limits,
                roots.0,
                roots.1,
                context,
                escaping_basis,
            )
        },
    )
    .unwrap();
    let delta = work.load(Ordering::Acquire) - before;
    assert!(delta > 0 && delta <= limits.max_work_bytes);
    assert!(peak.get() < nested_peak && peak.get() <= CAP);
    assert_eq!(
        Some(&verified.1.component_root_sha256),
        expected.navigation_original_root_sha256.as_ref()
    );
    assert_eq!(
        Some(&verified.2.component_root_sha256),
        expected.philosophy_original_root_sha256.as_ref()
    );
    assert_eq!(
        Some(&verified.3.component_root_sha256),
        expected.corpus_original_root_sha256.as_ref()
    );
    eprintln!(
        "sequential components: retained_peak={} nested_peak={nested_peak} cap={CAP} shared_work_delta={delta}; all three Original roots exact",
        peak.get()
    );
}
