//! Native transport over the Command-owned managed current-source hold.
//! Packets, disclosure leases and output flush stay inside that callback.
use tos_command::source_managed_selection::{
    ManagedAgentSelectedParent, ManagedSelectedProof, ManagedSelectionError,
};
use tos_foundation::Digest256;
use tos_query::{CatalogCurrentAuthority, InspectCurrentAuthority};

#[derive(Debug)]
pub enum ManagedKnowledgeWriteError {
    Managed(ManagedSelectionError),
    Access(crate::AccessError),
}

pub trait ManagedKnowledgeWriter {
    fn write_current_knowledge<'authority>(
        &self,
        coordinator: &mut tos_command::DurablePgCoordinator,
        store: &tos_segment_store::SegmentStore,
        filesystem: &tos_command::source_creation_store::CreationFilesystem,
        package: &tos_command::source_creation::ManagedSerializedCreation,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        vocabulary: &tos_compiler::QueryVocabulary,
        descriptor_raw: &[u8],
        catalog: &mut dyn CatalogCurrentAuthority<'authority>,
        inspect: &mut dyn InspectCurrentAuthority<'authority>,
        checkpoints: &mut dyn tos_query::knowledge_exploration::ExplorationCheckpoints,
        request: crate::KnowledgeRequest,
        budgets: crate::knowledge::SelectedKnowledgeBudgets,
        profile: crate::AccessProfile,
        stdout: &mut dyn std::io::Write,
        stderr: &mut dyn std::io::Write,
        deadline: std::time::Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<i32, ManagedKnowledgeWriteError>;
}

impl<P: ManagedSelectedProof> ManagedKnowledgeWriter for ManagedAgentSelectedParent<P> {
    fn write_current_knowledge<'authority>(
        &self,
        coordinator: &mut tos_command::DurablePgCoordinator,
        store: &tos_segment_store::SegmentStore,
        filesystem: &tos_command::source_creation_store::CreationFilesystem,
        package: &tos_command::source_creation::ManagedSerializedCreation,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        vocabulary: &tos_compiler::QueryVocabulary,
        descriptor_raw: &[u8],
        catalog: &mut dyn CatalogCurrentAuthority<'authority>,
        inspect: &mut dyn InspectCurrentAuthority<'authority>,
        checkpoints: &mut dyn tos_query::knowledge_exploration::ExplorationCheckpoints,
        request: crate::KnowledgeRequest,
        budgets: crate::knowledge::SelectedKnowledgeBudgets,
        profile: crate::AccessProfile,
        stdout: &mut dyn std::io::Write,
        stderr: &mut dyn std::io::Write,
        deadline: std::time::Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<i32, ManagedKnowledgeWriteError> {
        let open_steps = match &request {
            crate::KnowledgeRequest::Catalog => budgets.catalog.max_open_vm_steps,
            crate::KnowledgeRequest::Node { .. } | crate::KnowledgeRequest::Relation { .. } => {
                budgets.inspect.max_open_vm_steps
            }
            _ => {
                return Err(ManagedKnowledgeWriteError::Managed(
                    ManagedSelectionError::Source(
                        tos_command::source_command::SourceCommandError::Unsupported(
                            "managed selected consumer is catalog and navigation node/relation only",
                        ),
                    ),
                ));
            }
        };
        self.with_current_knowledge(
            coordinator,
            store,
            filesystem,
            package,
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            vocabulary,
            descriptor_raw,
            catalog,
            inspect,
            open_steps,
            deadline,
            cancelled,
            |reader, bound, catalog, inspect| {
                let packet = crate::checked_execute(profile.deadline_probe(), |probe| {
                    crate::knowledge::execute_selected_knowledge(
                        reader,
                        bound,
                        catalog,
                        inspect,
                        checkpoints,
                        request,
                        budgets,
                        probe,
                    )
                })?;
                Ok(crate::cli::write_packet(packet, profile, stdout, stderr))
            },
        )
        .map_err(ManagedKnowledgeWriteError::Managed)?
        .map_err(ManagedKnowledgeWriteError::Access)
    }
}
