//! The maintained v9 D1 table and index shape. Rows are produced by the
//! public-snapshot builder; these declarations alone grant no publication.

use crate::{
    Error, Result,
    d1_public_sql::{SqlSink, quote},
};

pub(crate) const BASE_TABLES: &[&str] = &[
    "edge_meta",
    "philosophy_nodes",
    "philosophy_edges",
    "philosophy_aux",
    "philosophy_clusters",
    "philosophy_cluster_nodes",
    "philosophy_cluster_edges",
    "philosophy_review_packets",
    "corpus_items",
    "corpus_edges",
    "corpus_packs",
    "knowledge_nodes",
    "knowledge_relations",
    "knowledge_search_documents",
    "knowledge_search_grams",
    "knowledge_search_gram_stats",
    "knowledge_lens_order",
    "knowledge_compact_lens",
    "knowledge_lens_memberships",
    "source_navigation_nodes",
    "source_navigation_node_payload",
    "source_navigation_edges",
    "source_navigation_edge_payload",
    "source_navigation_rights",
    "source_navigation_rights_payload",
];

pub(crate) fn begin(sink: &mut SqlSink) -> Result<()> {
    sink.line("PRAGMA foreign_keys=OFF;")?;
    for table in BASE_TABLES {
        sink.line(&format!("DROP TABLE IF EXISTS {table}_next;"))?;
    }
    for statement in [
        "CREATE TABLE edge_meta_next (key TEXT NOT NULL, part INTEGER NOT NULL, json_chunk TEXT NOT NULL, PRIMARY KEY (key, part));",
        "CREATE TABLE philosophy_nodes_next (id TEXT PRIMARY KEY, ord INTEGER NOT NULL, view_mask INTEGER NOT NULL, layer_mask INTEGER NOT NULL, json TEXT NOT NULL, search_text TEXT NOT NULL);",
        "CREATE TABLE philosophy_edges_next (id TEXT PRIMARY KEY, ord INTEGER NOT NULL, from_id TEXT NOT NULL, to_id TEXT NOT NULL, predicate_id TEXT NOT NULL, view_mask INTEGER NOT NULL, layer_mask INTEGER NOT NULL, json TEXT NOT NULL, search_text TEXT NOT NULL);",
        "CREATE TABLE philosophy_aux_next (collection TEXT NOT NULL, ord INTEGER NOT NULL, id TEXT NOT NULL, json TEXT NOT NULL, search_text TEXT NOT NULL, PRIMARY KEY (collection, ord));",
        "CREATE TABLE philosophy_clusters_next (id TEXT NOT NULL, ord INTEGER NOT NULL, sort_key TEXT NOT NULL, view_mask INTEGER NOT NULL, layer_mask INTEGER NOT NULL, part INTEGER NOT NULL, json_chunk TEXT NOT NULL, PRIMARY KEY (id, part));",
        "CREATE TABLE philosophy_cluster_nodes_next (cluster_id TEXT NOT NULL, cluster_ord INTEGER NOT NULL, sort_key TEXT NOT NULL, member_ord INTEGER NOT NULL, item_id TEXT NOT NULL, view_mask INTEGER NOT NULL, layer_mask INTEGER NOT NULL, json TEXT NOT NULL, PRIMARY KEY (cluster_id, member_ord));",
        "CREATE TABLE philosophy_cluster_edges_next (cluster_id TEXT NOT NULL, cluster_ord INTEGER NOT NULL, sort_key TEXT NOT NULL, member_ord INTEGER NOT NULL, item_id TEXT NOT NULL, view_mask INTEGER NOT NULL, layer_mask INTEGER NOT NULL, json TEXT NOT NULL, PRIMARY KEY (cluster_id, member_ord));",
        "CREATE TABLE philosophy_review_packets_next (view_id TEXT PRIMARY KEY, json TEXT NOT NULL);",
        "CREATE TABLE corpus_items_next (collection TEXT NOT NULL, ord INTEGER NOT NULL, id TEXT NOT NULL, resource_kind TEXT, owner_branch TEXT, json TEXT NOT NULL, search_text TEXT NOT NULL, PRIMARY KEY (collection, ord));",
        "CREATE TABLE corpus_edges_next (id TEXT NOT NULL, ord INTEGER PRIMARY KEY, from_id TEXT NOT NULL, to_id TEXT NOT NULL, pack_id TEXT, owner_branch TEXT, json TEXT NOT NULL);",
        "CREATE TABLE corpus_packs_next (id TEXT PRIMARY KEY, ord INTEGER NOT NULL, json TEXT NOT NULL);",
        "CREATE TABLE knowledge_nodes_next (id TEXT PRIMARY KEY, entity_id TEXT NOT NULL, native_id TEXT NOT NULL, source_graph TEXT NOT NULL, kind_id TEXT NOT NULL, type_id TEXT NOT NULL, title_text TEXT NOT NULL, summary_text TEXT NOT NULL, search_text TEXT NOT NULL, json TEXT NOT NULL);",
        "CREATE TABLE knowledge_relations_next (id TEXT PRIMARY KEY, native_id TEXT NOT NULL, source_graph TEXT NOT NULL, from_id TEXT NOT NULL, to_id TEXT NOT NULL, predicate_id TEXT NOT NULL, relation_type_id TEXT NOT NULL, label_text TEXT NOT NULL, explanation_text TEXT NOT NULL, search_text TEXT NOT NULL, json TEXT NOT NULL);",
        "CREATE TABLE knowledge_search_documents_next (kind TEXT NOT NULL, position INTEGER NOT NULL, id TEXT NOT NULL, source_graph TEXT NOT NULL, kind_id TEXT NOT NULL, predicate_id TEXT NOT NULL, id_lower TEXT NOT NULL, native_id_lower TEXT NOT NULL, identity_values TEXT NOT NULL, visible_values TEXT NOT NULL, document_chars INTEGER NOT NULL, document_digest TEXT NOT NULL, PRIMARY KEY (kind, position));",
        "CREATE TABLE knowledge_search_grams_next (kind TEXT NOT NULL, n INTEGER NOT NULL, gram TEXT NOT NULL, position INTEGER NOT NULL, PRIMARY KEY (kind, n, gram, position));",
        "CREATE TABLE knowledge_search_gram_stats_next (kind TEXT NOT NULL, n INTEGER NOT NULL, gram TEXT NOT NULL, postings INTEGER NOT NULL, PRIMARY KEY (kind, n, gram));",
        "CREATE TABLE knowledge_lens_order_next (kind TEXT NOT NULL, id TEXT NOT NULL, sort_key TEXT NOT NULL, from_id TEXT NOT NULL, to_id TEXT NOT NULL, PRIMARY KEY (kind, id));",
        "CREATE TABLE source_navigation_nodes_next (node_id TEXT PRIMARY KEY, ord INTEGER NOT NULL, node_kind TEXT NOT NULL, source_ref TEXT NOT NULL, label TEXT NOT NULL, identity_status TEXT NOT NULL, properties_json TEXT NOT NULL, json TEXT NOT NULL);",
        "CREATE TABLE source_navigation_node_payload_next (id TEXT NOT NULL, part INTEGER NOT NULL, json_chunk TEXT NOT NULL, PRIMARY KEY (id, part));",
        "CREATE TABLE source_navigation_edges_next (edge_id TEXT PRIMARY KEY, ord INTEGER NOT NULL, from_id TEXT NOT NULL, to_id TEXT NOT NULL, edge_kind TEXT NOT NULL, predicate_id TEXT NOT NULL, review_status TEXT NOT NULL, source_refs_json TEXT NOT NULL, json TEXT NOT NULL);",
        "CREATE TABLE source_navigation_edge_payload_next (id TEXT NOT NULL, part INTEGER NOT NULL, json_chunk TEXT NOT NULL, PRIMARY KEY (id, part));",
        "CREATE TABLE source_navigation_rights_next (rights_id TEXT PRIMARY KEY, ord INTEGER NOT NULL, scope_refs_json TEXT NOT NULL, json TEXT NOT NULL);",
        "CREATE TABLE source_navigation_rights_payload_next (id TEXT NOT NULL, part INTEGER NOT NULL, json_chunk TEXT NOT NULL, PRIMARY KEY (id, part));",
    ] {
        sink.line(statement)?;
    }
    for statement in [
        "DROP TABLE IF EXISTS knowledge_compact_lens_state;",
        "DROP TABLE IF EXISTS knowledge_lens_membership_state;",
        "CREATE TABLE knowledge_compact_lens_next(kind TEXT NOT NULL,id TEXT NOT NULL,source_sha256 TEXT NOT NULL,seed_sha256 TEXT NOT NULL,json TEXT NOT NULL,PRIMARY KEY(kind,id));",
        "CREATE TABLE knowledge_lens_memberships_next(kind TEXT NOT NULL,field TEXT NOT NULL,value TEXT NOT NULL,id TEXT NOT NULL,sort_key TEXT NOT NULL,PRIMARY KEY(kind,field,value,id));",
    ] {
        sink.line(statement)?;
    }
    Ok(())
}

pub(crate) fn finish(sink: &mut SqlSink, binding_prefix: &str, binding_suffix: &str) -> Result<()> {
    for table in BASE_TABLES {
        sink.line(&format!("DROP TABLE IF EXISTS {table};"))?;
        sink.line(&format!("ALTER TABLE {table}_next RENAME TO {table};"))?;
    }
    for statement in [
        "CREATE INDEX philosophy_edges_from_idx ON philosophy_edges(from_id);",
        "CREATE INDEX philosophy_edges_to_idx ON philosophy_edges(to_id);",
        "CREATE INDEX philosophy_cluster_nodes_item_idx ON philosophy_cluster_nodes(item_id);",
        "CREATE INDEX philosophy_cluster_edges_item_idx ON philosophy_cluster_edges(item_id);",
        "CREATE INDEX corpus_items_id_idx ON corpus_items(id);",
        "CREATE INDEX corpus_edges_from_idx ON corpus_edges(from_id);",
        "CREATE INDEX corpus_edges_to_idx ON corpus_edges(to_id);",
        "CREATE INDEX corpus_edges_id_idx ON corpus_edges(id);",
        "CREATE INDEX knowledge_nodes_native_idx ON knowledge_nodes(native_id);",
        "CREATE INDEX knowledge_nodes_entity_idx ON knowledge_nodes(entity_id);",
        "CREATE INDEX knowledge_nodes_source_kind_idx ON knowledge_nodes(source_graph, kind_id);",
        "CREATE INDEX knowledge_nodes_source_type_idx ON knowledge_nodes(source_graph, type_id);",
        "CREATE INDEX knowledge_relations_native_idx ON knowledge_relations(native_id);",
        "CREATE INDEX knowledge_relations_source_predicate_idx ON knowledge_relations(source_graph, predicate_id);",
        "CREATE INDEX knowledge_relations_source_type_idx ON knowledge_relations(source_graph, relation_type_id);",
        "CREATE INDEX knowledge_relations_from_idx ON knowledge_relations(from_id);",
        "CREATE INDEX knowledge_relations_to_idx ON knowledge_relations(to_id);",
        "CREATE INDEX knowledge_search_documents_source_kind_idx ON knowledge_search_documents(kind,source_graph,kind_id,position);",
        "CREATE INDEX knowledge_search_documents_source_predicate_idx ON knowledge_search_documents(kind,source_graph,predicate_id,position);",
        "CREATE INDEX knowledge_lens_order_sort ON knowledge_lens_order(kind,sort_key,id);",
        "CREATE INDEX knowledge_lens_order_from ON knowledge_lens_order(kind,from_id,sort_key,id);",
        "CREATE INDEX knowledge_lens_order_to ON knowledge_lens_order(kind,to_id,sort_key,id);",
        "CREATE INDEX knowledge_lens_order_pair ON knowledge_lens_order(kind,from_id,to_id,id);",
        "CREATE INDEX knowledge_lens_memberships_order ON knowledge_lens_memberships(kind,field,value,sort_key,id);",
        "CREATE INDEX knowledge_lens_memberships_row ON knowledge_lens_memberships(kind,id);",
        "CREATE INDEX source_navigation_nodes_kind_idx ON source_navigation_nodes(node_kind);",
        "CREATE INDEX source_navigation_nodes_packet_idx ON source_navigation_nodes(json_extract(properties_json, '$.packet_id'));",
        "CREATE INDEX source_navigation_edges_from_seek_idx ON source_navigation_edges(from_id, edge_id);",
        "CREATE INDEX source_navigation_edges_to_seek_idx ON source_navigation_edges(to_id, edge_id);",
        "CREATE INDEX source_navigation_edges_predicate_idx ON source_navigation_edges(predicate_id);",
        "CREATE INDEX source_navigation_rights_scope_idx ON source_navigation_rights(scope_refs_json);",
        "PRAGMA optimize;",
    ] {
        sink.line(statement)?;
    }
    // The authored migration owns the serving clock and the keyset indexes.
    // Its triggers must be created after the complete edge_meta replacement.
    sink.line(include_str!(
        "../../../../access/deploy/cloudflare-worker/migrations/0001-exploration.sql"
    ))?;
    sink.line("UPDATE knowledge_exploration_clock SET epoch=epoch+1 WHERE singleton=1;")?;
    let clock = "(SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1)";
    if binding_prefix.len() + binding_suffix.len() > 131072 {
        return Err(Error::Budget("public D1 auxiliary binding bytes"));
    }
    let binding = format!(
        "{}||{clock}||{}",
        quote(binding_prefix)?,
        quote(binding_suffix)?
    );
    for (state, schema) in [
        (
            "knowledge_compact_lens_state",
            "tos_compact_lens_carrier_v1",
        ),
        (
            "knowledge_lens_membership_state",
            "tos_lens_membership_index_v1",
        ),
    ] {
        sink.line(&format!("CREATE TABLE {state}(singleton INTEGER PRIMARY KEY CHECK(singleton=1),schema TEXT NOT NULL,binding TEXT NOT NULL,valid INTEGER NOT NULL CHECK(valid IN (0,1)));") )?;
        sink.line(&format!("INSERT INTO {state} VALUES(1,{},({binding}),CASE WHEN typeof({clock})='integer' AND {clock}>=0 AND {clock}<=9007199254740991 THEN 1 ELSE -1 END);", quote(schema)?))?;
    }
    for (table, state, prefix) in [
        (
            "knowledge_nodes",
            "knowledge_compact_lens_state",
            "compact_lens_node",
        ),
        (
            "knowledge_relations",
            "knowledge_compact_lens_state",
            "compact_lens_relation",
        ),
        (
            "knowledge_nodes",
            "knowledge_lens_membership_state",
            "membership_knowledge_nodes",
        ),
        (
            "knowledge_relations",
            "knowledge_lens_membership_state",
            "membership_knowledge_relations",
        ),
        (
            "knowledge_lens_memberships",
            "knowledge_lens_membership_state",
            "membership_knowledge_lens_memberships",
        ),
    ] {
        for action in ["INSERT", "UPDATE", "DELETE"] {
            sink.line(&format!("CREATE TRIGGER {prefix}_{} AFTER {action} ON {table} BEGIN UPDATE {state} SET valid=0 WHERE singleton=1; END;", action.to_ascii_lowercase()))?;
        }
    }
    Ok(())
}
