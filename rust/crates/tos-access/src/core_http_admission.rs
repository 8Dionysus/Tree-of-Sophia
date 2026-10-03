//! Explicit private HTTP query allowances. Every scalar comes from the caller;
//! this adapter supplies no quota, deadline, source authority or fallback.
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JsonLimits {
    max_bytes: usize,
    max_depth: usize,
    max_visits: usize,
    max_integer_digits: usize,
}
impl JsonLimits {
    pub(crate) fn native(&self) -> Result<tos_foundation::JsonLimits, &'static str> {
        if self.max_bytes == 0
            || self.max_depth == 0
            || self.max_visits == 0
            || self.max_integer_digits == 0
        {
            return Err("Core HTTP allowance must be positive");
        }
        Ok(tos_foundation::JsonLimits {
            max_bytes: self.max_bytes,
            max_depth: self.max_depth,
            max_visits: self.max_visits,
            max_integer_digits: self.max_integer_digits,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InspectBudget {
    max_open_vm_steps: u64,
    max_read_vm_steps: u64,
    max_matches: usize,
    max_rows: u64,
    max_field_bytes: usize,
    max_payload_bytes: usize,
    max_decoded_bytes: u64,
    max_response_bytes: usize,
    json: JsonLimits,
}
impl InspectBudget {
    pub(crate) fn native(&self) -> Result<tos_query::InspectBudget, &'static str> {
        if self.max_open_vm_steps == 0
            || self.max_read_vm_steps == 0
            || self.max_matches == 0
            || self.max_rows == 0
            || self.max_field_bytes == 0
            || self.max_payload_bytes == 0
            || self.max_decoded_bytes == 0
            || self.max_response_bytes == 0
        {
            return Err("Core HTTP allowance must be positive");
        }
        Ok(tos_query::InspectBudget {
            max_open_vm_steps: self.max_open_vm_steps,
            max_read_vm_steps: self.max_read_vm_steps,
            max_matches: self.max_matches,
            max_rows: self.max_rows,
            max_field_bytes: self.max_field_bytes,
            max_payload_bytes: self.max_payload_bytes,
            max_decoded_bytes: self.max_decoded_bytes,
            max_response_bytes: self.max_response_bytes,
            json: self.json.native()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CatalogBudget {
    max_open_vm_steps: u64,
    max_read_vm_steps: u64,
    max_packet_bytes: usize,
    max_decoded_bytes: usize,
    json: JsonLimits,
}
impl CatalogBudget {
    pub(crate) fn native(&self) -> Result<tos_query::CatalogBudget, &'static str> {
        if self.max_open_vm_steps == 0
            || self.max_read_vm_steps == 0
            || self.max_packet_bytes == 0
            || self.max_decoded_bytes == 0
        {
            return Err("Core HTTP allowance must be positive");
        }
        Ok(tos_query::CatalogBudget {
            max_open_vm_steps: self.max_open_vm_steps,
            max_read_vm_steps: self.max_read_vm_steps,
            max_packet_bytes: self.max_packet_bytes,
            max_decoded_bytes: self.max_decoded_bytes,
            json: self.json.native()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LensBudget {
    inspect: InspectBudget,
    max_candidates: usize,
    max_path_steps: usize,
    max_adjacency_rows: usize,
    block_size: usize,
}
impl LensBudget {
    pub(crate) fn native(&self) -> Result<tos_query::knowledge_lens::LensBudget, &'static str> {
        if self.max_candidates == 0
            || self.max_path_steps == 0
            || self.max_adjacency_rows == 0
            || self.block_size == 0
        {
            return Err("Core HTTP allowance must be positive");
        }
        Ok(tos_query::knowledge_lens::LensBudget {
            inspect: self.inspect.native()?,
            max_candidates: self.max_candidates,
            max_path_steps: self.max_path_steps,
            max_adjacency_rows: self.max_adjacency_rows,
            block_size: self.block_size,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExplorationBudget {
    read: InspectBudget,
    max_work_units: usize,
    max_session_nodes: usize,
    max_session_relations: usize,
    max_state_bytes: usize,
    max_checkpoint_bytes: usize,
    max_checkpoints: usize,
}
impl ExplorationBudget {
    pub(crate) fn native(
        &self,
    ) -> Result<tos_query::knowledge_exploration::ExplorationBudget, &'static str> {
        if self.max_work_units == 0
            || self.max_session_nodes == 0
            || self.max_session_relations == 0
            || self.max_state_bytes == 0
            || self.max_checkpoint_bytes == 0
            || self.max_checkpoints == 0
        {
            return Err("Core HTTP allowance must be positive");
        }
        Ok(tos_query::knowledge_exploration::ExplorationBudget {
            read: self.read.native()?,
            max_work_units: self.max_work_units,
            max_session_nodes: self.max_session_nodes,
            max_session_relations: self.max_session_relations,
            max_state_bytes: self.max_state_bytes,
            max_checkpoint_bytes: self.max_checkpoint_bytes,
            max_checkpoints: self.max_checkpoints,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SearchDocumentBudget {
    max_carrier_bytes: usize,
    max_document_bytes: usize,
    max_document_code_points: usize,
    json: JsonLimits,
}
impl SearchDocumentBudget {
    pub(crate) fn native(&self) -> Result<tos_query::SearchDocumentBudget, &'static str> {
        if self.max_carrier_bytes == 0
            || self.max_document_bytes == 0
            || self.max_document_code_points == 0
        {
            return Err("Core HTTP allowance must be positive");
        }
        Ok(tos_query::SearchDocumentBudget {
            max_carrier_bytes: self.max_carrier_bytes,
            max_document_bytes: self.max_document_bytes,
            max_document_code_points: self.max_document_code_points,
            json: self.json.native()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GramSeekBudget {
    max_lookups: usize,
    max_candidates: u64,
    max_vm_steps: u64,
    max_rows: u64,
    max_decoded_bytes: u64,
}
impl GramSeekBudget {
    pub(crate) fn native(&self) -> Result<tos_query::search_index::GramSeekBudget, &'static str> {
        if self.max_lookups == 0
            || self.max_candidates == 0
            || self.max_vm_steps == 0
            || self.max_rows == 0
            || self.max_decoded_bytes == 0
        {
            return Err("Core HTTP allowance must be positive");
        }
        Ok(tos_query::search_index::GramSeekBudget {
            max_lookups: self.max_lookups,
            max_candidates: self.max_candidates,
            max_vm_steps: self.max_vm_steps,
            max_rows: self.max_rows,
            max_decoded_bytes: self.max_decoded_bytes,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PostingSeekBudget {
    max_probes: u64,
    max_rows: u64,
    max_decoded_bytes: u64,
    max_vm_steps: u64,
    page_rows: usize,
}
impl PostingSeekBudget {
    pub(crate) fn native(
        &self,
    ) -> Result<tos_query::search_index::PostingSeekBudget, &'static str> {
        if self.max_probes == 0
            || self.max_rows == 0
            || self.max_decoded_bytes == 0
            || self.max_vm_steps == 0
            || self.page_rows == 0
        {
            return Err("Core HTTP allowance must be positive");
        }
        Ok(tos_query::search_index::PostingSeekBudget {
            max_probes: self.max_probes,
            max_rows: self.max_rows,
            max_decoded_bytes: self.max_decoded_bytes,
            max_vm_steps: self.max_vm_steps,
            page_rows: self.page_rows,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateReadBudget {
    max_vm_steps: u64,
    max_decoded_bytes: u64,
    max_payload_bytes: usize,
    max_field_bytes: usize,
    max_document_chars: u64,
}
impl CandidateReadBudget {
    pub(crate) fn native(
        &self,
    ) -> Result<tos_query::search_candidate::CandidateReadBudget, &'static str> {
        if self.max_vm_steps == 0
            || self.max_decoded_bytes == 0
            || self.max_payload_bytes == 0
            || self.max_field_bytes == 0
            || self.max_document_chars == 0
        {
            return Err("Core HTTP allowance must be positive");
        }
        Ok(tos_query::search_candidate::CandidateReadBudget {
            max_vm_steps: self.max_vm_steps,
            max_decoded_bytes: self.max_decoded_bytes,
            max_payload_bytes: self.max_payload_bytes,
            max_field_bytes: self.max_field_bytes,
            max_document_chars: self.max_document_chars,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateVerifyBudget {
    document: SearchDocumentBudget,
    max_rank_field_bytes: usize,
    max_rank_values: usize,
}
impl CandidateVerifyBudget {
    pub(crate) fn native(
        &self,
    ) -> Result<tos_query::search_candidate::CandidateVerifyBudget, &'static str> {
        if self.max_rank_field_bytes == 0 || self.max_rank_values == 0 {
            return Err("Core HTTP allowance must be positive");
        }
        Ok(tos_query::search_candidate::CandidateVerifyBudget {
            document: self.document.native()?,
            max_rank_field_bytes: self.max_rank_field_bytes,
            max_rank_values: self.max_rank_values,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SearchKindBudget {
    grams: GramSeekBudget,
    postings: PostingSeekBudget,
    candidate: CandidateReadBudget,
    verify: CandidateVerifyBudget,
    max_candidate_vm_steps: u64,
    max_candidate_decoded_bytes: u64,
    max_verified_chars: u64,
    max_verified_bytes: u64,
    max_observed_candidates: usize,
    max_observed_bytes: u64,
    max_selected_result_bytes: usize,
}
impl SearchKindBudget {
    pub(crate) fn native(&self) -> Result<tos_query::SearchKindBudget, &'static str> {
        if self.max_candidate_vm_steps == 0
            || self.max_candidate_decoded_bytes == 0
            || self.max_verified_chars == 0
            || self.max_verified_bytes == 0
            || self.max_observed_candidates == 0
            || self.max_observed_bytes == 0
            || self.max_selected_result_bytes == 0
        {
            return Err("Core HTTP allowance must be positive");
        }
        Ok(tos_query::SearchKindBudget {
            grams: self.grams.native()?,
            postings: self.postings.native()?,
            candidate: self.candidate.native()?,
            verify: self.verify.native()?,
            max_candidate_vm_steps: self.max_candidate_vm_steps,
            max_candidate_decoded_bytes: self.max_candidate_decoded_bytes,
            max_verified_chars: self.max_verified_chars,
            max_verified_bytes: self.max_verified_bytes,
            max_observed_candidates: self.max_observed_candidates,
            max_observed_bytes: self.max_observed_bytes,
            max_selected_result_bytes: self.max_selected_result_bytes,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LegacySearchBudget {
    inspect: InspectBudget,
    document: SearchDocumentBudget,
    max_candidates: usize,
    max_document_bytes: u64,
    max_document_code_points: u64,
    max_retained_per_kind: usize,
    max_retained_bytes: usize,
    block_size: usize,
}
impl LegacySearchBudget {
    pub(crate) fn native(
        &self,
    ) -> Result<tos_query::knowledge_legacy_search::LegacySearchBudget, &'static str> {
        if self.max_candidates == 0
            || self.max_document_bytes == 0
            || self.max_document_code_points == 0
            || self.max_retained_per_kind == 0
            || self.max_retained_bytes == 0
            || self.block_size == 0
        {
            return Err("Core HTTP allowance must be positive");
        }
        Ok(tos_query::knowledge_legacy_search::LegacySearchBudget {
            inspect: self.inspect.native()?,
            document: self.document.native()?,
            max_candidates: self.max_candidates,
            max_document_bytes: self.max_document_bytes,
            max_document_code_points: self.max_document_code_points,
            max_retained_per_kind: self.max_retained_per_kind,
            max_retained_bytes: self.max_retained_bytes,
            block_size: self.block_size,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IndexedPageBudget {
    nodes: SearchKindBudget,
    relations: SearchKindBudget,
    max_open_vm_steps: u64,
    max_response_bytes: usize,
    max_cursor_bytes: usize,
    json: JsonLimits,
}
impl IndexedPageBudget {
    pub(crate) fn native(&self) -> Result<tos_query::IndexedPageBudget, &'static str> {
        if self.max_open_vm_steps == 0 || self.max_response_bytes == 0 || self.max_cursor_bytes == 0
        {
            return Err("Core HTTP allowance must be positive");
        }
        Ok(tos_query::IndexedPageBudget {
            nodes: self.nodes.native()?,
            relations: self.relations.native()?,
            max_open_vm_steps: self.max_open_vm_steps,
            max_response_bytes: self.max_response_bytes,
            max_cursor_bytes: self.max_cursor_bytes,
            json: self.json.native()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KnowledgeContractBudget {
    max_input_bytes: usize,
    max_registry_bytes: usize,
    max_response_bytes: usize,
    json: JsonLimits,
}
impl KnowledgeContractBudget {
    pub(crate) fn native(
        &self,
    ) -> Result<tos_query::knowledge_contracts::KnowledgeContractBudget, &'static str> {
        if self.max_input_bytes == 0 || self.max_registry_bytes == 0 || self.max_response_bytes == 0
        {
            return Err("Core HTTP allowance must be positive");
        }
        Ok(tos_query::knowledge_contracts::KnowledgeContractBudget {
            max_input_bytes: self.max_input_bytes,
            max_registry_bytes: self.max_registry_bytes,
            max_response_bytes: self.max_response_bytes,
            json: self.json.native()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectedKnowledgeBudgets {
    catalog: CatalogBudget,
    inspect: InspectBudget,
    lens: LensBudget,
    exploration: ExplorationBudget,
}
impl SelectedKnowledgeBudgets {
    pub(crate) fn native(
        &self,
    ) -> Result<crate::knowledge::SelectedKnowledgeBudgets, &'static str> {
        Ok(crate::knowledge::SelectedKnowledgeBudgets {
            catalog: self.catalog.native()?,
            inspect: self.inspect.native()?,
            lens: self.lens.native()?,
            exploration: self.exploration.native()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HttpAdmission {
    pub(crate) max_startup_receipt_bytes: usize,
    pub(crate) selected: SelectedKnowledgeBudgets,
    pub(crate) legacy: LegacySearchBudget,
    pub(crate) indexed: IndexedPageBudget,
    pub(crate) contracts: KnowledgeContractBudget,
    max_request_bytes: usize,
    max_response_bytes: usize,
    max_mcp_frame_bytes: usize,
    max_line_bytes: usize,
    query_seconds: f64,
    checkpoint_ttl_seconds: u64,
    checkpoint_max_entries: usize,
    checkpoint_max_encoded_bytes: usize,
}
impl HttpAdmission {
    pub(crate) fn profile(&self) -> Result<crate::AccessProfile, &'static str> {
        if self.max_startup_receipt_bytes == 0
            || self.max_startup_receipt_bytes > 64 * 1024 * 1024
            || self.max_request_bytes == 0
            || self.max_response_bytes == 0
            || self.max_mcp_frame_bytes == 0
            || self.max_line_bytes == 0
            || !self.query_seconds.is_finite()
            || self.query_seconds <= 0.0
        {
            return Err("Core HTTP transport allowance invalid");
        }
        let timeout = std::time::Duration::try_from_secs_f64(self.query_seconds)
            .map_err(|_| "Core HTTP timeout invalid")?;
        Ok(crate::AccessProfile::new(
            self.max_request_bytes,
            self.max_response_bytes,
            self.max_line_bytes,
        )
        .with_mcp_frame_budget(self.max_mcp_frame_bytes)
        .with_query_timeout(timeout))
    }
    pub(crate) fn checkpoints(
        &self,
    ) -> Result<crate::exploration_checkpoints::ProcessExplorationCheckpoints, &'static str> {
        crate::exploration_checkpoints::ProcessExplorationCheckpoints::new(
            crate::exploration_checkpoints::CheckpointLimits {
                ttl: std::time::Duration::from_secs(self.checkpoint_ttl_seconds),
                max_entries: self.checkpoint_max_entries,
                max_encoded_bytes: self.checkpoint_max_encoded_bytes,
            },
        )
        .map_err(|_| "Core HTTP checkpoint allowance invalid")
    }
}
