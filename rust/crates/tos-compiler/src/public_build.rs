//! Shared limits and normalization binding for native snapshot preparation.

use crate::{
    Error, Limits, NATIVE_KNOWLEDGE_ADAPTER_PROFILES, NativeFamilyInputs, NativeProducerLimits,
    Result, catalog::CatalogLimits, knowledge_scope::ScopeLimits,
    knowledge_search::SearchBuildLimits, knowledge_stage::StageLimits,
    public_capture::PublicCaptureLimits, public_graph::PublicRepositoryRoot,
};
use tos_foundation::Digest256;

#[derive(Clone, Copy, Debug)]
pub struct PublicBuildLimits {
    pub capture: PublicCaptureLimits,
    pub stage: StageLimits,
    pub native: NativeProducerLimits,
    pub scope: ScopeLimits,
    pub catalog: CatalogLimits,
    pub search: SearchBuildLimits,
    pub max_stage_transfer_work_bytes: u64,
    pub max_build_seconds: u64,
    /// Explicit normalized-producer Rust + SQLite allowance, independent of disk.
    pub max_state_bytes: usize,
    pub max_json_visits: usize,
}

impl PublicBuildLimits {
    fn validate(self) -> Result<()> {
        self.capture.validate()?;
        self.stage.validate()?;
        let n = self.native;
        n.navigation_prepare.validate()?;
        n.navigation_nodes.validate()?;
        n.navigation_materialize.validate()?;
        n.navigation_dependencies.validate()?;
        n.navigation_relations.validate()?;
        n.claims_prepare.validate()?;
        n.claims.validate()?;
        n.philosophy_prepare.validate()?;
        n.philosophy.validate()?;
        n.titles.validate()?;
        n.inherited.validate()?;
        n.finalize.validate()?;
        NativeFamilyInputs::bounded_from(n).topology.validate()?;
        self.search.validate()?;
        if self.scope.max_sources == 0
            || self.scope.max_rows == 0
            || self.scope.max_index_work_bytes == 0
            || self.catalog.max_rows == 0
            || self.catalog.max_row_bytes == 0
            || self.catalog.max_catalog_bytes == 0
            || self.catalog.max_catalog_entries == 0
            || self.catalog.max_aggregate_entries == 0
            || self.catalog.max_aggregate_bytes == 0
            || self.catalog.max_staging_pages == 0
            || self.max_build_seconds == 0
            || self.max_stage_transfer_work_bytes == 0
        {
            return Err(Error::Budget("public D1 build limits"));
        }
        Ok(())
    }
}

/// One local caller profile derived from existing compiler defaults and the
/// historical full-producer caps. Its SQLite page limits are per DB;
/// no aggregate host disk or sorter-spill quota is inferred from them.
/// State/JSON ceilings remain unset for the caller to supply.
pub fn portable_public_limits(max_build_seconds: u64) -> Result<PublicBuildLimits> {
    public_limits(max_build_seconds, Limits::default(), 10_000_000)
}

/// Construct one finite public build profile from the caller's explicit
/// per-database byte, cumulative work and posting-count allowances.
/// Filesystem capacity and process admission remain the caller's responsibility.
pub fn public_limits(
    max_build_seconds: u64,
    base: Limits,
    max_search_postings: u64,
) -> Result<PublicBuildLimits> {
    base.validate()?;
    // SearchBuildLimits::validate admits at most 8,000,000 source/rank bytes
    // and Unicode scalars; use that same ceiling for every normalized family.
    let row = base.max_row_bytes.min(8_000_000);
    let rows = base.max_rows;
    let work = base.max_work_bytes;
    // Semantic joins retain two topology pages at once under their existing
    // 64-MiB pair ceiling. Derive the shared native page width from that law.
    let paired_row_bytes = row
        .checked_mul(2)
        .ok_or(Error::Budget("public D1 pair page arithmetic"))?;
    let page_rows = 8usize.min((64 * 1024 * 1024) / paired_row_bytes);
    if page_rows == 0 {
        return Err(Error::Budget("public D1 pair page arithmetic"));
    }
    let page_bytes = page_rows
        .checked_mul(row)
        .ok_or(Error::Budget("public D1 page arithmetic"))?;
    let page_bytes_u64 =
        u64::try_from(page_bytes).map_err(|_| Error::Budget("public D1 page arithmetic"))?;
    let endpoint_rows = rows
        .checked_mul(2)
        .ok_or(Error::Budget("public D1 endpoint arithmetic"))?;
    let native = NativeProducerLimits {
        navigation_prepare: crate::NavigationPrepareLimits {
            max_nodes: rows,
            max_edges: rows,
            max_endpoint_refs: endpoint_rows,
            max_page_rows: page_rows,
            max_row_bytes: row,
            max_header_bytes: 1024 * 1024,
            max_work_bytes: work,
        },
        navigation_nodes: crate::NavigationNodeLimits {
            max_raw_bytes: row,
            max_output_bytes: row,
            max_ancestor_cache_bytes: 4 * 1024 * 1024,
        },
        navigation_materialize: crate::NavigationMaterializeLimits {
            max_nodes: rows,
            max_edges: rows,
            max_placeholders: rows,
            max_page_rows: page_rows,
            max_raw_bytes: row,
            max_output_bytes: row,
            max_page_bytes: page_bytes,
            max_work_bytes: work,
        },
        navigation_dependencies: crate::NavigationRelationLimits {
            max_edges: rows,
            max_page_rows: page_rows,
            max_raw_bytes: row,
            max_context_bytes: row,
            max_page_bytes: page_bytes_u64,
            max_work_bytes: work,
        },
        navigation_relations: crate::NavigationRelationNormalizeLimits {
            max_raw_bytes: row,
            max_output_bytes: row,
            max_registry_bytes: 4 * 1024 * 1024,
            max_claim_contexts: 4096,
            max_global_input_bytes: row,
        },
        claims_prepare: crate::ClaimPrepareLimits {
            max_nodes: rows,
            max_edges: rows,
            max_claims: rows,
            max_page_rows: page_rows,
            max_row_bytes: row,
            max_work_bytes: work,
        },
        claims: crate::knowledge_source_claims::ClaimNormalizeLimits {
            max_raw_bytes: row,
            max_output_bytes: row,
            max_page_rows: page_rows,
            max_contexts: 4096,
            max_work_bytes: work,
        },
        philosophy_prepare: crate::PhilosophyPrepareLimits {
            max_nodes: rows,
            max_edges: rows,
            max_edge_view_bindings: rows,
            max_page_rows: page_rows,
            max_row_bytes: row,
            max_work_bytes: work,
        },
        philosophy: crate::PhilosophyMaterializeLimits {
            max_raw_bytes: row,
            max_output_bytes: row,
            max_registry_bytes: 4 * 1024 * 1024,
            max_page_rows: page_rows,
            max_rows: rows,
            max_work_bytes: work,
        },
        titles: crate::GlobalTitleLimits {
            max_nodes: rows,
            max_page_rows: page_rows,
            max_page_bytes: page_bytes,
            max_node_bytes: row,
            max_title_bytes: 64 * 1024,
            max_work_bytes: work,
        },
        inherited: crate::InheritedViewLimits {
            max_relations: rows,
            max_endpoint_evidence_rows: endpoint_rows,
            max_view_tokens: rows,
            max_page_rows: page_rows,
            max_page_bytes: page_bytes_u64,
            max_row_bytes: row,
            max_work_bytes: work,
        },
        finalize: crate::NativeFinalizeLimits {
            max_rows: rows,
            max_page_rows: page_rows,
            max_page_bytes: page_bytes,
            max_row_bytes: row,
            max_view_ids_per_node: 64,
            max_context_sources: 64,
            max_work_bytes: work,
        },
    };
    let mut stage_sqlite = base;
    stage_sqlite.max_row_bytes = row;
    let limits = PublicBuildLimits {
        capture: PublicCaptureLimits {
            max_input_bytes: base.max_output_bytes,
            max_rows: rows,
            max_staging_bytes: base.max_output_bytes,
            max_work_bytes: work,
            max_sql_vm_steps: base.max_sql_vm_steps,
            sqlite_cache_kib: base.sqlite_cache_kib,
        },
        stage: StageLimits {
            sqlite: stage_sqlite,
            max_temp_bytes: base.max_output_bytes,
            max_seek_rows: 1024,
            max_seek_bytes: page_bytes_u64,
        },
        native,
        scope: ScopeLimits {
            max_sources: NATIVE_KNOWLEDGE_ADAPTER_PROFILES.len(),
            max_rows: rows,
            max_index_work_bytes: work,
        },
        // Intermediate route rows are bounded separately from the small
        // rendered catalog, for both public D1 and managed native snapshots.
        catalog: CatalogLimits {
            max_aggregate_entries: rows,
            max_aggregate_bytes: usize::try_from(base.max_output_bytes.min(256 * 1024 * 1024))
                .map_err(|_| Error::Budget("public catalog aggregate bytes"))?,
            ..CatalogLimits::default()
        },
        search: SearchBuildLimits {
            max_payload_bytes: row,
            max_document_chars: row,
            max_document_bytes: 64_000_000,
            max_rank_field_bytes: row,
            max_postings: max_search_postings,
            max_work_bytes: work,
            gram_batch_rows: 64,
        },
        max_stage_transfer_work_bytes: work,
        max_build_seconds,
        max_state_bytes: 0,
        max_json_visits: 0,
    };
    limits.validate()?;
    Ok(limits)
}

pub(crate) fn processor_binding(
    root: &PublicRepositoryRoot,
    registry: &crate::KnowledgeRegistry,
    descriptor: &[u8],
    entity: &[u8],
    relation: &[u8],
) -> Result<crate::native_normalization_binding::NativeNormalizationBinding> {
    let processor = Digest256::from_hex(root.software_binding())
        .map_err(|_| Error::Invalid("public D1 processor software binding"))?;
    Ok(
        crate::native_normalization_binding::NativeNormalizationBinding::from_current_inputs(
            processor, registry, descriptor, entity, relation,
        ),
    )
}
