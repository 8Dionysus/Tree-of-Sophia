//! Bounded native backup/restore continuation over the real Parmenides source.

#[test]
#[ignore = "requires dedicated PostgreSQL databases and pinned pg_dump/pg_restore tools"]
fn real_parmenides_backup_restore_cold_reads_and_continues_native_revision() {
    let report = super::postgres_real_revision::run_parmenides_backup_restore_regression();
    assert_eq!(report["backup"]["status"], "backup_complete");
    assert_eq!(report["restore"]["status"], "restore_verified");
    assert_eq!(report["restore"]["source_admission"], false);
    assert_eq!(report["restore"]["source_currentness_granted"], false);
    assert_eq!(report["continuation"]["replayed"], true);
    println!("TOS_PG_REAL_BACKUP_RESTORE {report}");
}
