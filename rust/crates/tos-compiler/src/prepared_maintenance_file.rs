//! Concrete offline file owners for the maintained joined maintenance operations.
//! A Python Connection is never transported here. Every callback refusal aborts
//! the same native transaction; source currentness is checked by the actual caller.
use crate::prepared_catalog_index::CatalogMaintenanceLimits;
use crate::prepared_catalog_semantics::CatalogInputs;
use crate::prepared_maintenance::{self as joined, MaintenanceReceipt};
use crate::prepared_semantic_index::{SemanticMaintenanceLimits, SemanticRows};
use crate::{Error, Result, local_prepared::PublicationLimits};
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};
use std::{fs, os::unix::fs::MetadataExt, path::Path, time::Instant};
use tos_foundation::JsonValue;

fn deadline_check(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(Error::Budget("maintenance whole operation deadline"));
    }
    Ok(())
}
fn check_path(path: &Path, original: &fs::Metadata) -> Result<()> {
    let now = fs::symlink_metadata(path)?;
    if !now.is_file()
        || now.file_type().is_symlink()
        || now.dev() != original.dev()
        || now.ino() != original.ino()
    {
        return Err(Error::Invalid("maintenance selected file changed"));
    }
    Ok(())
}
fn own<F>(
    path: &Path,
    limits: PublicationLimits,
    deadline: Instant,
    precommit: &mut dyn FnMut() -> Result<()>,
    operation: F,
) -> Result<MaintenanceReceipt>
where
    F: FnOnce(&Transaction<'_>) -> Result<MaintenanceReceipt>,
{
    limits.validate()?;
    deadline_check(deadline)?;
    if !path.is_absolute() {
        return Err(Error::Invalid("maintenance absolute file path"));
    }
    let original = fs::symlink_metadata(path)?;
    if !original.is_file() || original.file_type().is_symlink() || original.len() > limits.max_bytes
    {
        return Err(Error::Invalid("maintenance regular file/physical cap"));
    }
    // Keep the originally selected inode alive while SQLite owns its writer.
    let anchor = fs::File::open(path)?;
    let anchored = anchor.metadata()?;
    if anchored.dev() != original.dev() || anchored.ino() != original.ino() {
        return Err(Error::Invalid("maintenance file changed at anchor open"));
    }
    let mut db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    check_path(path, &original)?;
    db.progress_handler(1000, Some(move || Instant::now() >= deadline));
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let result = (|| {
        check_path(path, &original)?;
        deadline_check(deadline)?;
        let receipt = operation(&tx)?;
        check_path(path, &original)?;
        deadline_check(deadline)?;
        precommit()?;
        // Callback gives no authority to replace a selected file or outrun its
        // deadline. SQL metadata/binding checks remain with the joined operation.
        check_path(path, &original)?;
        deadline_check(deadline)?;
        Ok(receipt)
    })();
    match result {
        Ok(receipt) => {
            tx.commit()?;
            Ok(receipt)
        }
        Err(error) => {
            let _ = tx.rollback();
            Err(error)
        }
    }
}

pub fn bootstrap_prepared_maintenance_file(
    path: &Path,
    expected: &JsonValue,
    inputs: &CatalogInputs,
    limits: PublicationLimits,
    catalog: CatalogMaintenanceLimits,
    semantic: SemanticMaintenanceLimits,
    rows: Option<&mut dyn SemanticRows>,
    processor: &str,
    deadline: Instant,
    precommit: &mut dyn FnMut() -> Result<()>,
) -> Result<MaintenanceReceipt> {
    catalog.validate()?;
    semantic.validate()?;
    inputs.validate()?;
    own(path, limits, deadline, precommit, |tx| {
        joined::bootstrap_prepared_maintenance_transaction(
            tx, expected, inputs, limits, catalog, semantic, rows, processor,
        )
    })
}

pub fn apply_catalogued_prepared_delta_file(
    path: &Path,
    expected: &JsonValue,
    before: &CatalogInputs,
    after: &CatalogInputs,
    changes: &[crate::local_prepared::PreparedChange],
    limits: PublicationLimits,
    catalog: CatalogMaintenanceLimits,
    deadline: Instant,
    precommit: &mut dyn FnMut() -> Result<()>,
) -> Result<MaintenanceReceipt> {
    catalog.validate()?;
    before.validate()?;
    after.validate()?;
    own(path, limits, deadline, precommit, |tx| {
        joined::apply_catalogued_prepared_delta_transaction(
            tx, expected, before, after, changes, limits, catalog,
        )
    })
}

pub fn apply_semantic_prepared_delta_file(
    path: &Path,
    expected: &JsonValue,
    before: &CatalogInputs,
    after: &CatalogInputs,
    changes: &[crate::local_prepared::PreparedChange],
    limits: PublicationLimits,
    catalog: CatalogMaintenanceLimits,
    semantic: SemanticMaintenanceLimits,
    processor: &str,
    deadline: Instant,
    precommit: &mut dyn FnMut() -> Result<()>,
) -> Result<MaintenanceReceipt> {
    catalog.validate()?;
    semantic.validate()?;
    before.validate()?;
    after.validate()?;
    own(path, limits, deadline, precommit, |tx| {
        joined::apply_semantic_prepared_delta_transaction(
            tx,
            expected,
            before,
            after,
            changes.iter().cloned().map(Ok),
            limits,
            catalog,
            semantic,
            processor,
        )
    })
}
