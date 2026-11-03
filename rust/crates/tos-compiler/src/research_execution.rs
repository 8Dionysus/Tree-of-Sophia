//! Compatibility route for research callers while they migrate to the execution owner.
pub use tos_source_store::execution::{
    Execution as ResearchExecution, SqliteExecutionScope as ResearchSqliteScope,
};

#[cfg(feature = "research")]
impl tos_source_store::execution::StructuralReadSource for crate::antonovsky_structural::Model {
    fn root_identity(&self) -> Result<(u64, u64), String> {
        crate::antonovsky_structural::Model::root_identity(self)
    }

    fn source_read_bytes(&self) -> u64 {
        crate::antonovsky_structural::Model::source_read_bytes(self)
    }
}

/// Select the retained reading-workbench read profile.
pub fn new_reading_v1(
    root: &std::path::Path,
    max_seconds: u64,
    available_bytes: Option<u64>,
) -> Result<ResearchExecution, String> {
    ResearchExecution::new_with_read_limits(
        root,
        max_seconds,
        available_bytes,
        512 * 1024 * 1024,
        4 * 1024 * 1024 * 1024,
    )
}

/// Select the retained visual-result verification read profile.
pub fn new_visual_result(
    root: &std::path::Path,
    max_seconds: u64,
    available_bytes: Option<u64>,
) -> Result<ResearchExecution, String> {
    ResearchExecution::new_with_read_limits(
        root,
        max_seconds,
        available_bytes,
        8 * 1024 * 1024 * 1024,
        20 * 1024 * 1024 * 1024,
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn retained_read_profiles_preserve_operation_limits() {
        let dir = tempfile::tempdir().unwrap();
        let reading = super::new_reading_v1(dir.path(), 30, None)
            .unwrap()
            .budget_report();
        let visual = super::new_visual_result(dir.path(), 30, None)
            .unwrap()
            .budget_report();
        assert_eq!(reading["file_bytes_max"], 512_u64 * 1024 * 1024);
        assert_eq!(
            reading["logical_source_and_sqlite_read_bytes_max"],
            4_u64 * 1024 * 1024 * 1024
        );
        assert_eq!(visual["file_bytes_max"], 8_u64 * 1024 * 1024 * 1024);
        assert_eq!(
            visual["logical_source_and_sqlite_read_bytes_max"],
            20_u64 * 1024 * 1024 * 1024
        );
    }
}
