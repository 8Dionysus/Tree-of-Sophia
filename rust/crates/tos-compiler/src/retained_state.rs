//! Capacity-based logical state of the existing typed receipt owners.
//! Borrowed aliases and inline subfields are charged by their enclosing owner.
use tos_foundation::{OwnedState, Result};

tos_foundation::impl_owned_state!(crate::SourceBinding {
    owner_profile,
    source_cut,
    through_commit_seq,
    membership_root,
    index_generation,
    route_map_version,
    reader_abi,
    projection_root_sha256,
    complete
});
tos_foundation::impl_owned_state!(crate::knowledge_stage::StageReceipt {
    binding,
    source_cut,
    membership_root,
    input_collections,
    verified_inputs,
    input_rows,
    node_rows,
    relation_rows,
    node_root_sha256,
    relation_root_sha256,
    sqlite_sha256,
    sqlite_size_bytes
});
tos_foundation::impl_owned_state!(crate::knowledge_stage::InputCollectionReceipt {
    source_graph,
    collection,
    input_role,
    adapter_profile,
    expected_count,
    expected_root_sha256
});
tos_foundation::impl_owned_state!(crate::knowledge_full::FullKnowledgeReceipt {
    source_scope,
    catalog,
    search,
    seal
});
tos_foundation::impl_owned_state!(crate::knowledge_scope::ScopeReceipt {
    source_count,
    node_count,
    relation_count,
    source_scope_root_sha256
});
tos_foundation::impl_owned_state!(crate::knowledge_catalog_index::CatalogIndexReceipt {
    descriptor_sha256,
    catalog_packet_sha256,
    catalog_index_root_sha256,
    source_count,
    facet_field_count,
    facet_value_count,
    route_count,
    packet_bytes
});
tos_foundation::impl_owned_state!(crate::knowledge_search::SearchIndexReceipt {
    profile,
    node_documents,
    relation_documents,
    postings,
    distinct_grams,
    document_chars,
    work_bytes,
    search_index_root_sha256
});
tos_foundation::impl_owned_state!(crate::knowledge_seal::KnowledgeSealReceipt {
    model_abi,
    managed_source_root_sha256,
    navigation_original_root_sha256,
    philosophy_original_root_sha256,
    corpus_original_root_sha256,
    graph_root_sha256,
    graph_header_sha256,
    node_root_sha256,
    relation_root_sha256,
    node_count,
    relation_count,
    catalog_packet_sha256,
    catalog_index_root_sha256,
    source_scope_root_sha256,
    search_index_root_sha256
});
tos_foundation::impl_owned_state!(crate::knowledge_native::NativeProducerReceipt {
    final_rows,
    navigation_original,
    philosophy_original,
    corpus_original,
    base_node_root_sha256,
    endpoint_title_root_sha256,
    claim_group_root_sha256,
    placeholder_absence_root_sha256,
    navigation_nodes,
    navigation_relations,
    claim_nodes,
    claim_relations,
    philosophy_nodes,
    philosophy_relations,
    canon_nodes,
    canon_relations,
    candidate_relations,
    repository_nodes,
    repository_relations,
    semantic_relations,
    indexed_nodes,
    indexed_relations
});
tos_foundation::impl_owned_state!(crate::knowledge_native_finalize::NativeFinalizeReceipt {
    source_cut,
    nodes,
    relations,
    readable_rows,
    node_root_sha256,
    relation_root_sha256
});
tos_foundation::impl_owned_state!(
    crate::knowledge_navigation_original::NavigationOriginalReceipt {
        profile,
        descriptor_sha256,
        source_cut,
        membership_root,
        source_graph,
        nodes,
        edges,
        rights,
        node_input_root_sha256,
        edge_input_root_sha256,
        header_sha256,
        rights_root_sha256,
        component_root_sha256,
        total_bytes,
        member_index_root_sha256,
        member_index_bytes
    }
);
tos_foundation::impl_owned_state!(
    crate::knowledge_philosophy_original::PhilosophyOriginalReceipt {
        profile,
        descriptor_sha256,
        source_cut,
        membership_root,
        source_graph,
        nodes,
        edges,
        node_input_root_sha256,
        edge_input_root_sha256,
        header_sha256,
        nodes_root_sha256,
        edges_root_sha256,
        component_root_sha256,
        total_bytes
    }
);
tos_foundation::impl_owned_state!(crate::knowledge_corpus_original::CorpusOriginalReceipt {
    profile,
    descriptor_sha256,
    source_cut,
    membership_root,
    origin,
    header_sha256,
    collections,
    component_root_sha256,
    total_bytes
});
tos_foundation::impl_owned_state!(crate::knowledge_corpus_original::CorpusOriginalOrigin {
    profile,
    source_git_commit,
    source_git_tree,
    capture_manifest_sha256,
    native_producer,
    source_path,
    source_sha256,
    source_size_bytes,
    members,
    member_root_sha256
});
tos_foundation::impl_owned_state!(crate::knowledge_corpus_original::CorpusOriginalMember {
    path,
    size_bytes,
    sha256
});
tos_foundation::impl_owned_state!(
    crate::knowledge_corpus_original::CorpusOriginalCollectionReceipt {
        collection,
        rows,
        ordered_root_sha256
    }
);
tos_foundation::impl_owned_state!(crate::source_corpus::NativeCorpusSourceReceipt {
    profile,
    source_revision,
    source_membership_sha256,
    source_members,
    source_cut,
    descriptor_sha256,
    software_commit,
    software_tree,
    software_manifest_sha256,
    owner_program_sha256,
    owner_schema_sha256,
    repository_inventory_root_sha256,
    canon_source_root_sha256,
    navigation_catalog_root_sha256,
    output_sha256,
    output_bytes,
    schema_units
});
tos_foundation::impl_owned_state!(crate::knowledge_selected::KnowledgeSelectedExpectation {
    model_sha256,
    model_size_bytes,
    owner_receipt_id,
    model_abi,
    managed_source_root_sha256,
    descriptor_sha256,
    descriptor_version,
    semantic_primitive_profile,
    source_cut,
    through_commit_seq,
    membership_root,
    entity_registry_id,
    entity_registry_version,
    entity_registry_sha256,
    relation_registry_id,
    relation_registry_version,
    relation_registry_sha256,
    graph_root_sha256,
    navigation_original_root_sha256,
    philosophy_original_root_sha256,
    corpus_original_root_sha256,
    catalog_packet_sha256,
    catalog_index_root_sha256,
    source_scope_root_sha256,
    search_index_root_sha256,
    node_count,
    relation_count,
    index_generation,
    route_map_version,
    reader_abi,
    authority_boundary,
    source_scopes,
    complete
});
tos_foundation::impl_owned_state!(crate::knowledge_selected::ExpectedSourceScope {
    source_graph,
    input_role,
    adapter_profile,
    node_count,
    relation_count,
    node_root_sha256,
    relation_root_sha256
});
tos_foundation::impl_owned_state!(crate::managed_source::ManagedSourceGenerationV1 {
    domain,
    store_id,
    installed_generation_sha256,
    through_commit_seq,
    selected_audit_generation,
    epoch,
    definition_sha256,
    bootstrap_source_revision,
    bootstrap_membership_sha256,
    bootstrap_members,
    domain_sha256,
    database_oid,
    schema_profile_sha256,
    state_sha256,
    log_sha256,
    current_membership_sha256,
    current_members,
    history_membership_sha256,
    history_members,
    inventory_projection_sha256
});
tos_foundation::impl_owned_state!(crate::managed_source::ManagedSourceGenerationV2 {
    domain,
    store_id,
    installed_generation_sha256,
    through_commit_seq,
    selected_audit_generation,
    epoch,
    definition_sha256,
    bootstrap_source_revision,
    bootstrap_membership_sha256,
    bootstrap_members,
    domain_sha256,
    database_oid,
    schema_profile_sha256,
    addressed_metadata_tree_sha256,
    addressed_metadata_members,
    state_profile_sha256,
    addressed_inventory_root_sha256,
    log_sha256,
    addressed_current_tree_sha256,
    current_members,
    addressed_history_tree_sha256,
    history_members,
    inventory_projection_sha256
});
tos_foundation::impl_owned_state!(crate::managed_source::ManagedSourceProofV1 {
    schema,
    generation,
    initial_export_source_revision,
    initial_export_membership_sha256,
    initial_export_members,
    delta
});
tos_foundation::impl_owned_state!(crate::managed_source::ManagedSourceProofV2 {
    schema,
    generation,
    initial_export_source_revision,
    initial_export_membership_sha256,
    initial_export_members,
    delta
});
tos_foundation::impl_owned_state!(crate::managed_source::ManagedSourceDeltaV1 {
    parent_model_sha256,
    parent_model_size_bytes,
    parent_source_proof_sha256,
    parent_through_commit_seq,
    committed_delta_sha256,
    committed_member_root_sha256
});
tos_foundation::impl_owned_state!(crate::vocabulary::RegisteredSource {
    source_graph_id,
    owner_ref,
    input_role,
    adapter_profile,
    representative_priority
});

impl OwnedState for crate::KnowledgeSourceBasis {
    fn owned_heap_bytes(&self) -> Result<usize> {
        match self {
            Self::V1Cut { source_revision } => source_revision.owned_heap_bytes(),
            Self::ManagedCurrent { proof } => proof.owned_heap_bytes(),
            Self::ManagedCurrentV2 { proof } => proof.owned_heap_bytes(),
        }
    }
}
