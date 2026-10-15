//! Bounded V9 backup/restore, projection interruption/resume, and native continuation over Parmenides.

#[test]
#[ignore = "requires dedicated PostgreSQL databases and pinned pg_dump/pg_restore tools"]
fn real_parmenides_backup_restore_cold_reads_and_continues_native_revision() {
    let report = super::postgres_real_revision::run_parmenides_backup_restore_regression();
    assert_eq!(report["backup"]["status"], "backup_complete");
    assert_eq!(report["restore"]["status"], "restore_verified");
    assert_eq!(report["restore"]["source_admission"], false);
    assert_eq!(report["restore"]["source_currentness_granted"], false);
    assert_eq!(report["backup"]["schema"], "tos_cmd2_quiescent_backup_v10");
    assert_eq!(
        report["private_package_schema_guard"]["fresh_schema_accepted"],
        true
    );
    assert_eq!(
        report["private_package_schema_guard"]["missing_guard_refused"],
        true
    );
    assert_eq!(
        report["private_package_schema_guard"]["guard_restored_before_backup"],
        true
    );
    assert_eq!(
        report["projection_interruption"]["rows_unchanged_after_kill"],
        true
    );
    assert_eq!(
        report["projection_interruption"]["status_after_kill"],
        "pending"
    );
    assert_eq!(
        report["projection_restore_resume"]["restored_status_before_resume"],
        "pending"
    );
    assert_eq!(
        report["projection_restore_resume"]["status_after_resume"],
        "current"
    );
    assert_eq!(report["continuation"]["replayed"], true);
    println!("TOS_PG_REAL_BACKUP_RESTORE {report}");
}
