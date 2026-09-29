//! Offline maintenance joins exact catalog, semantic and prepared carrier lanes.
//! A caller-owned transaction is mandatory; any error requires its full rollback.
//! The native file owner must check source currentness before committing it.
use crate::prepared_catalog_index::{CatalogChange, CatalogIndex, CatalogMaintenanceLimits};
use crate::prepared_catalog_semantics::{self as catalog, CatalogInputs, CatalogOrder, CatalogRow};
use crate::prepared_semantic_index::{
    self as semantic, SemanticChange, SemanticMaintenanceLimits, SemanticRows,
};
use crate::{Error, Result, local_prepared as prepared};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;
use tos_foundation::{Digest256, JsonString, JsonValue};

#[derive(Clone, Debug)]
pub struct MaintenanceReceipt {
    pub binding: JsonValue,
    pub source_header: Option<JsonValue>,
    pub catalog: Option<JsonValue>,
    pub catalog_digest: String,
    pub semantic_report: Option<JsonValue>,
    pub semantic_report_sha256: Option<String>,
    pub sql_mutations: u64,
    pub publication_changed: bool,
    pub consumer_switched: bool,
    pub source_transition_verified: bool,
    pub semantic_acceptance: bool,
}
fn digest(value: &JsonValue, cap: usize) -> Result<String> {
    Ok(Digest256::of_bytes(prepared::compact(value, cap)?.as_bytes()).to_hex())
}
fn registry_digest(value: &JsonValue, cap: usize) -> Result<String> {
    let view: Value = serde_json::from_str(&prepared::compact(value, cap)?)
        .map_err(|_| Error::Invalid("maintenance registry JSON"))?;
    crate::knowledge_normalization::stable_digest(&view)
}
fn required<'a>(value: &'a JsonValue, key: &str) -> Result<&'a str> {
    value
        .object_get(key)
        .and_then(JsonValue::as_str)
        .ok_or(Error::Invalid("maintenance required string"))
}
fn same(a: &JsonValue, b: &JsonValue, cap: usize) -> Result<bool> {
    let a: Value = serde_json::from_str(&prepared::compact(a, cap)?)
        .map_err(|_| Error::Invalid("maintenance equality JSON"))?;
    let b: Value = serde_json::from_str(&prepared::compact(b, cap)?)
        .map_err(|_| Error::Invalid("maintenance equality JSON"))?;
    crate::prepared_semantic_kernel::python_eq(&a, &b)
}
fn remaining(tx: &Transaction<'_>, start: u64, limits: prepared::PublicationLimits) -> Result<u64> {
    let used = tx
        .total_changes()
        .checked_sub(start)
        .ok_or(Error::Invalid("maintenance SQL counter"))?;
    limits
        .max_mutations
        .checked_sub(used)
        .filter(|n| *n > 0)
        .ok_or(Error::Budget("combined maintenance mutations"))
}
fn semantic_limits(
    tx: &Transaction<'_>,
    start: u64,
    limits: prepared::PublicationLimits,
    mut semantic: SemanticMaintenanceLimits,
) -> Result<SemanticMaintenanceLimits> {
    semantic.max_bytes = semantic.max_bytes.min(limits.max_bytes);
    semantic.max_writes = semantic.max_writes.min(remaining(tx, start, limits)?);
    Ok(semantic)
}
fn columns(kind: &str) -> Result<&'static [&'static str]> {
    match kind {
        "node" => Ok(&[
            "id",
            "entity_id",
            "native_id",
            "source_graph",
            "kind_id",
            "type_id",
        ]),
        "relation" => Ok(&[
            "id",
            "native_id",
            "source_graph",
            "from_id",
            "to_id",
            "predicate_id",
            "relation_type_id",
        ]),
        _ => Err(Error::Invalid("maintenance row kind")),
    }
}
fn body(
    tx: &Transaction<'_>,
    kind: &str,
    id: &str,
    limits: prepared::PublicationLimits,
) -> Result<JsonValue> {
    let columns = columns(kind)?;
    let sql = format!(
        "SELECT {},CASE WHEN length(CAST(json AS BLOB))<=? THEN json ELSE NULL END FROM knowledge_{kind}s WHERE id=?",
        columns.join(",")
    );
    let found: Option<(Vec<String>, Option<String>)> = tx
        .query_row(&sql, params![limits.max_row_bytes, id], |r| {
            let mut values = Vec::new();
            for i in 0..columns.len() {
                values.push(r.get(i)?);
            }
            Ok((values, r.get(columns.len())?))
        })
        .optional()?;
    let (values, raw) = found.ok_or(Error::Invalid("maintenance selected row absent"))?;
    let raw = raw.ok_or(Error::Budget("maintenance selected row bytes"))?;
    let key = format!("knowledge_{kind}_digest:{id}");
    let checksum = prepared::metadata(tx, &key, 1024)?;
    if checksum.as_object().is_none_or(|fields| fields.len() != 1)
        || checksum.object_get("sha256").and_then(JsonValue::as_str)
            != Some(Digest256::of_bytes(raw.as_bytes()).to_hex().as_str())
    {
        return Err(Error::Invalid("maintenance row checksum"));
    }
    let item = prepared::parse(&raw, limits.max_row_bytes)?;
    if item.as_object().is_none() {
        return Err(Error::Invalid("maintenance row object"));
    }
    for (key, value) in columns.iter().zip(values) {
        if prepared::index_value(&item, key)? != value {
            return Err(Error::Invalid("maintenance identity columns"));
        }
    }
    Ok(item)
}
pub(crate) fn selected(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    inputs: &CatalogInputs,
    limits: prepared::PublicationLimits,
) -> Result<JsonValue> {
    match expected.object_get("publication_epoch") {
        Some(JsonValue::Number(epoch))
            if epoch.kind == tos_foundation::JsonNumberKind::Int
                && epoch
                    .lexeme
                    .parse::<u64>()
                    .is_ok_and(|epoch| epoch <= prepared::MAX_ADDRESS) => {}
        _ => {
            return Err(Error::Invalid(
                "maintenance publication epoch must be a safe integer",
            ));
        }
    }
    limits.validate()?;
    if tx.is_autocommit() {
        return Err(Error::Invalid("maintenance caller transaction"));
    }
    if !same(
        &prepared::snapshot_binding(tx)?,
        expected,
        limits.max_metadata_bytes,
    )? {
        return Err(Error::Invalid("maintenance selected binding"));
    }
    let top = prepared::metadata(
        tx,
        "knowledge_reader_top",
        65536.min(limits.max_metadata_bytes),
    )?;
    if required(&top, "read_model_schema")? != prepared::SCHEMA {
        return Err(Error::Invalid("maintenance selected schema"));
    }
    let revision = prepared::metadata(tx, "data_revision", 1024)?;
    if revision.as_object().is_none_or(|fields| fields.len() != 1)
        || revision.object_get("sha256").and_then(JsonValue::as_str)
            != top.object_get("data_revision").and_then(JsonValue::as_str)
    {
        return Err(Error::Invalid("maintenance data revision"));
    }
    let state:Option<(u64,Option<String>)>=tx.query_row("SELECT max_pages,CASE WHEN length(CAST(descriptor AS BLOB))<=? THEN descriptor ELSE NULL END FROM prepared_state WHERE singleton=1",[limits.max_metadata_bytes],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let (max_pages, raw) = state.ok_or(Error::Invalid("maintenance descriptor absent"))?;
    let raw = raw.ok_or(Error::Budget("maintenance descriptor bytes"))?;
    let descriptor = prepared::parse(&raw, limits.max_metadata_bytes)?;
    if digest(&descriptor, limits.max_metadata_bytes)? != required(&top, "data_revision")?
        || catalog::catalog_owner_digest(
            descriptor
                .object_get("header")
                .ok_or(Error::Invalid("maintenance descriptor header"))?,
            limits.max_metadata_bytes,
        )? != inputs.header_digest()?
    {
        return Err(Error::Invalid("maintenance descriptor/header differs"));
    }
    let normalization = inputs
        .header
        .object_get("normalization_binding")
        .ok_or(Error::Invalid("maintenance normalization"))?;
    if !same(
        normalization,
        top.object_get("normalization_binding")
            .ok_or(Error::Invalid("maintenance selected normalization"))?,
        limits.max_metadata_bytes,
    )? {
        return Err(Error::Invalid("maintenance normalization differs"));
    }
    if normalization
        .object_get("entity_registry_digest")
        .and_then(JsonValue::as_str)
        != Some(registry_digest(&inputs.entity_registry, limits.max_metadata_bytes)?.as_str())
        || normalization
            .object_get("relation_registry_digest")
            .and_then(JsonValue::as_str)
            != Some(registry_digest(&inputs.relation_registry, limits.max_metadata_bytes)?.as_str())
    {
        return Err(Error::Invalid("maintenance registry digest"));
    }
    let selected_catalog = prepared::metadata(tx, "knowledge_catalog", limits.max_metadata_bytes)?;
    if digest(&selected_catalog, limits.max_metadata_bytes)? != required(&top, "catalog_sha256")? {
        return Err(Error::Invalid("maintenance catalog checksum"));
    }
    let page: u64 = tx.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    let pages: u64 = tx.query_row("PRAGMA page_count", [], |r| r.get(0))?;
    if pages > max_pages
        || pages
            .checked_mul(page)
            .ok_or(Error::Budget("maintenance pages"))?
            > limits.max_bytes
    {
        return Err(Error::Budget("maintenance whole database bytes"));
    }
    crate::local_prepared_search::page_cap(tx, max_pages.min(limits.max_bytes / page))?;
    Ok(selected_catalog)
}
fn source_order(inputs: &CatalogInputs, item: &JsonValue, token: u64) -> Result<CatalogOrder> {
    match inputs.source_order_profile {
        catalog::SourceOrderProfile::OwnerSequence => Ok(CatalogOrder::OwnerSequence(token)),
        catalog::SourceOrderProfile::SourceGraphId => Ok(CatalogOrder::SourceGraphId(
            prepared::python_value_string(
                item.object_get("source_graph").unwrap_or(&JsonValue::Null),
            )?,
            prepared::python_value_string(item.object_get("id").unwrap_or(&JsonValue::Null))?,
        )),
    }
}

pub fn bootstrap_prepared_catalog_transaction(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    inputs: &CatalogInputs,
    limits: prepared::PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
) -> Result<MaintenanceReceipt> {
    let start = tx.total_changes();
    let selected_catalog = selected(tx, expected, inputs, limits)?;
    let mut index = CatalogIndex::new(tx, catalog_limits)?;
    // Both SQL cursors stream exact sparse owner order; nested exact row
    // lookups retain only the current carrier.
    let mut node_statement=tx.prepare("SELECT id,source_order FROM prepared_documents WHERE kind='node' ORDER BY source_order,doc_id")?;
    let mut relation_statement=tx.prepare("SELECT id,source_order FROM prepared_documents WHERE kind='relation' ORDER BY source_order,doc_id")?;
    let nodes = node_statement.query_map([], |row| {
        Ok(("node", row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
    })?;
    let relations = relation_statement.query_map([], |row| {
        Ok(("relation", row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
    })?;
    let row_stream = nodes.chain(relations).map(|row| {
        let (kind, id, token) = row?;
        let item = body(tx, kind, &id, limits)?;
        let order = source_order(inputs, &item, token)?;
        remaining(tx, start, limits)?;
        Ok(CatalogRow {
            kind: kind.to_owned(),
            id,
            source_order: order,
            item,
        })
    });
    let output = index.bootstrap(inputs, row_stream)?;

    if catalog::catalog_owner_digest(&output, limits.max_metadata_bytes)?
        != catalog::catalog_owner_digest(&selected_catalog, limits.max_metadata_bytes)?
        || catalog::catalog_owner_digest(
            &catalog::finalized_header(inputs, &output)?,
            limits.max_metadata_bytes,
        )? != inputs.header_digest()?
    {
        return Err(Error::Invalid("maintenance catalog/header reproduction"));
    }
    selected(tx, expected, inputs, limits)?;
    if tx.total_changes() - start > limits.max_mutations {
        return Err(Error::Budget("joined catalog bootstrap mutations"));
    }
    Ok(MaintenanceReceipt {
        binding: expected.clone(),
        source_header: None,
        catalog: None,
        catalog_digest: catalog::catalog_owner_digest(&output, limits.max_metadata_bytes)?,
        semantic_report: None,
        semantic_report_sha256: None,
        sql_mutations: tx.total_changes() - start,
        publication_changed: false,
        consumer_switched: false,
        source_transition_verified: false,
        semantic_acceptance: false,
    })
}

/// Capture once before either auxiliary lane writes. Exact original owner JSON
/// is retained; no iterator can change an earlier replacement between passes.
fn capture<I: IntoIterator<Item = Result<prepared::PreparedChange>>>(
    changes: I,
    limits: prepared::PublicationLimits,
    frame_budget: bool,
) -> Result<Vec<prepared::PreparedChange>> {
    let mut retained = Vec::new();
    let mut bytes = 0usize;
    let mut seen = std::collections::BTreeSet::new();
    for change in changes {
        let change = change?;
        if retained.len() >= limits.max_changes {
            return Err(Error::Budget("maintenance change count"));
        }
        if !matches!(change.kind.as_str(), "node" | "relation")
            || !matches!(change.operation.as_str(), "insert" | "update" | "delete")
            || change.identifier.is_empty()
            || change.identifier.chars().count() > 4096
            || !seen.insert((change.kind.clone(), change.identifier.clone()))
        {
            return Err(Error::Invalid("maintenance exact unique targets"));
        }
        let item = if change.operation == "delete" {
            if change.item.is_some() || change.source_order.is_some() {
                return Err(Error::Invalid("maintenance deletion replacement/order"));
            }
            None
        } else {
            let item = change
                .item
                .as_ref()
                .ok_or(Error::Invalid("maintenance replacement missing"))?;
            if required(item, "id")? != change.identifier {
                return Err(Error::Invalid("maintenance replacement identity"));
            }
            for column in columns(&change.kind)? {
                prepared::index_value(item, column)?;
            }
            let raw = prepared::compact(item, limits.max_row_bytes)?;
            Some(prepared::parse(&raw, limits.max_row_bytes)?)
        };
        let available = limits
            .max_change_bytes
            .checked_sub(bytes)
            .ok_or(Error::Budget("maintenance input bytes"))?;
        let size = if frame_budget {
            let order = change
                .source_order
                .map(|n| prepared::parse(&n.to_string(), 64))
                .transpose()?
                .unwrap_or(JsonValue::Null);
            let frame = JsonValue::Array(vec![
                text(&change.operation),
                text(&change.kind),
                text(&change.identifier),
                order,
                item.clone().unwrap_or(JsonValue::Null),
            ]);
            prepared::compact(&frame, available)?.len()
        } else {
            item.as_ref()
                .map(|item| prepared::compact(item, available).map(|raw| raw.len()))
                .transpose()?
                .unwrap_or(0)
        };
        bytes = bytes
            .checked_add(size)
            .ok_or(Error::Budget("maintenance input bytes"))?;
        retained.push(prepared::PreparedChange { item, ..change });
    }
    Ok(retained)
}
fn text(s: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(s))
}
pub fn apply_catalogued_prepared_delta_transaction(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    before: &CatalogInputs,
    after: &CatalogInputs,
    changes: &[prepared::PreparedChange],
    limits: prepared::PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
) -> Result<MaintenanceReceipt> {
    let retained = capture(changes.iter().cloned().map(Ok), limits, false)?;
    apply_catalogued_retained(
        tx,
        expected,
        before,
        after,
        &retained,
        limits,
        catalog_limits,
    )
}
fn apply_catalogued_retained(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    before: &CatalogInputs,
    after: &CatalogInputs,
    retained: &[prepared::PreparedChange],
    limits: prepared::PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
) -> Result<MaintenanceReceipt> {
    apply_catalogued_retained_with_transition(
        tx,
        expected,
        before,
        after,
        retained,
        limits,
        catalog_limits,
        None,
    )
}
fn apply_catalogued_retained_with_transition(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    before: &CatalogInputs,
    after: &CatalogInputs,
    retained: &[prepared::PreparedChange],
    limits: prepared::PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    reviewed: Option<&prepared::ReviewedNormalizationTransition<'_>>,
) -> Result<MaintenanceReceipt> {
    let start = tx.total_changes();
    let old_catalog = selected(tx, expected, before, limits)?;
    after.validate()?;
    if retained.len() > limits.max_changes {
        return Err(Error::Budget("joined catalog change count"));
    }
    let mut contributor_changes = Vec::new();
    let mut bytes = 0usize;
    for change in retained {
        let stored: Option<u64> = tx
            .query_row(
                "SELECT source_order FROM prepared_documents WHERE kind=? AND id=?",
                params![change.kind, change.identifier],
                |r| r.get(0),
            )
            .optional()?;
        if stored.is_none() != (change.operation == "insert") {
            return Err(Error::Invalid("joined catalog target operation"));
        }
        let old = stored
            .map(|_| body(tx, &change.kind, &change.identifier, limits))
            .transpose()?;
        if let Some(old) = &old {
            bytes = bytes
                .checked_add(prepared::compact(old, limits.max_row_bytes)?.len())
                .ok_or(Error::Budget("joined catalog selected bytes"))?;
        }
        let order = if let Some(item) = &change.item {
            bytes = bytes
                .checked_add(prepared::compact(item, limits.max_row_bytes)?.len())
                .ok_or(Error::Budget("joined catalog selected bytes"))?;
            let token = change
                .source_order
                .or(stored)
                .filter(|n| *n <= prepared::MAX_ADDRESS)
                .ok_or(Error::Invalid("joined catalog insertion order"))?;
            Some(source_order(after, item, token)?)
        } else {
            None
        };
        if bytes > limits.max_change_bytes {
            return Err(Error::Budget("joined catalog selected bytes"));
        }
        contributor_changes.push(CatalogChange {
            operation: change.operation.clone(),
            kind: change.kind.clone(),
            id: change.identifier.clone(),
            expected_old_digest: old
                .as_ref()
                .map(|old| catalog::catalog_owner_digest(old, limits.max_row_bytes))
                .transpose()?,
            new_item: change.item.clone(),
            source_order: order,
        });
    }
    let mut index = CatalogIndex::new(tx, catalog_limits)?;
    let old_digest = catalog::catalog_owner_digest(&old_catalog, limits.max_metadata_bytes)?;
    let output = if reviewed.is_some() {
        if !contributor_changes.is_empty() {
            return Err(Error::Invalid(
                "normalization migration changes contributors",
            ));
        }
        index.transition_normalization(before, after, &old_digest)?
    } else {
        index.apply_delta(before, after, &contributor_changes, Some(&old_digest))?
    };
    let header = catalog::finalized_header(after, &output)?;
    let catalog_json = output.clone();
    let mut remaining_limits = limits;
    remaining_limits.max_mutations = remaining(tx, start, limits)?;
    let binding = if let Some(reviewed) = reviewed {
        if !retained.is_empty() {
            return Err(Error::Invalid("normalization migration cannot change rows"));
        }
        prepared::transition_prepared_normalization_transaction(
            tx,
            expected,
            &header,
            &catalog_json,
            remaining_limits,
            reviewed,
        )?
    } else {
        prepared::apply_prepared_delta_transaction(
            tx,
            expected,
            &header,
            &catalog_json,
            retained.iter().cloned(),
            remaining_limits,
        )?
    };
    if tx.total_changes() - start > limits.max_mutations {
        return Err(Error::Budget("joined catalog mutations"));
    }
    Ok(MaintenanceReceipt {
        binding,
        source_header: Some(header),
        catalog: Some(output.clone()),
        catalog_digest: catalog::catalog_owner_digest(&output, limits.max_metadata_bytes)?,
        semantic_report: None,
        semantic_report_sha256: None,
        sql_mutations: tx.total_changes() - start,
        publication_changed: true,
        consumer_switched: false,
        source_transition_verified: false,
        semantic_acceptance: false,
    })
}

pub fn bootstrap_prepared_maintenance_transaction(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    inputs: &CatalogInputs,
    limits: prepared::PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    semantic_cap: SemanticMaintenanceLimits,
    ordered_rows: Option<&mut dyn SemanticRows>,
    normalization_processor_sha256: &str,
) -> Result<MaintenanceReceipt> {
    inputs.validate()?;
    limits.validate()?;
    let start = tx.total_changes();
    let report = semantic::bootstrap_semantic_index_transaction(
        tx,
        expected,
        &inputs.entity_registry,
        &inputs.relation_registry,
        ordered_rows,
        normalization_processor_sha256,
        semantic_limits(tx, start, limits, semantic_cap)?,
    )?;
    let mut catalog_cap = limits;
    catalog_cap.max_mutations = remaining(tx, start, limits)?;
    let mut receipt =
        bootstrap_prepared_catalog_transaction(tx, expected, inputs, catalog_cap, catalog_limits)?;
    if tx.total_changes() - start > limits.max_mutations {
        return Err(Error::Budget("combined maintenance mutations"));
    }
    receipt.semantic_report_sha256 = Some(digest(&report, semantic_cap.max_output_bytes)?);
    receipt.semantic_report = Some(report);
    receipt.sql_mutations = tx.total_changes() - start;
    Ok(receipt)
}
fn replace_field(value: &mut JsonValue, key: &str, replacement: JsonValue) -> Result<()> {
    let JsonValue::Object(fields) = value else {
        return Err(Error::Invalid("maintenance owner object"));
    };
    if let Some((_, value)) = fields
        .iter_mut()
        .find(|(name, _)| name.as_str() == Some(key))
    {
        *value = replacement;
    } else {
        fields.push((JsonString::from_utf8(key), replacement));
    }
    Ok(())
}
pub fn apply_semantic_prepared_delta_transaction<
    I: IntoIterator<Item = Result<prepared::PreparedChange>>,
>(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    before: &CatalogInputs,
    after: &CatalogInputs,
    changes: I,
    limits: prepared::PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    semantic_cap: SemanticMaintenanceLimits,
    normalization_processor_sha256: &str,
) -> Result<MaintenanceReceipt> {
    before.validate()?;
    after.validate()?;
    limits.validate()?;
    let start = tx.total_changes();
    if after
        .header
        .object_get("counts")
        .is_some_and(|counts| counts.as_object().is_none())
    {
        return Err(Error::Invalid("semantic header counts object"));
    }
    let retained = capture(changes, limits, true)?;
    let semantic_changes: Vec<_> = retained
        .iter()
        .map(|change| SemanticChange {
            operation: change.operation.clone(),
            kind: change.kind.clone(),
            identifier: change.identifier.clone(),
            item: change.item.clone(),
            source_order: change.source_order,
        })
        .collect();
    let report = semantic::apply_semantic_delta_transaction(
        tx,
        expected,
        required(&after.header, "source_revision")?,
        &semantic_changes,
        &after.entity_registry,
        &after.relation_registry,
        normalization_processor_sha256,
        semantic_limits(tx, start, limits, semantic_cap)?,
    )?;
    let mut after = after.clone();
    let mut counts = after
        .header
        .object_get("counts")
        .cloned()
        .unwrap_or(JsonValue::Object(Vec::new()));
    replace_field(&mut counts, "semantic_validation", report.clone())?;
    replace_field(&mut after.header, "counts", counts)?;
    let mut catalog_cap = limits;
    catalog_cap.max_mutations = remaining(tx, start, limits)?;
    drop(semantic_changes);
    let mut receipt = apply_catalogued_retained(
        tx,
        expected,
        before,
        &after,
        &retained,
        catalog_cap,
        catalog_limits,
    )?;
    let verification = semantic::verify_semantic_index_binding_transaction(
        tx,
        &receipt.binding,
        normalization_processor_sha256,
        semantic_limits(tx, start, limits, semantic_cap)?,
    )?;
    if tx.total_changes() - start > limits.max_mutations {
        return Err(Error::Budget("combined maintenance mutations"));
    }
    receipt.semantic_report_sha256 =
        Some(required(&verification, "semantic_report_sha256")?.to_owned());
    receipt.semantic_report = Some(report);
    receipt.sql_mutations = tx.total_changes() - start;
    Ok(receipt)
}

/// Pair an owner-reviewed implementation-only normalization migration. This
/// changes no normalized rows or registry/configuration inputs. Source owners
/// must pair their own dependency/context state before committing this same
/// transaction; any error requires whole-transaction rollback.
pub fn transition_prepared_normalization_transaction(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    before: &CatalogInputs,
    after: &CatalogInputs,
    reviewed: &prepared::ReviewedNormalizationTransition<'_>,
    limits: prepared::PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    semantic_cap: SemanticMaintenanceLimits,
) -> Result<MaintenanceReceipt> {
    before.validate()?;
    after.validate()?;
    limits.validate()?;
    let start = tx.total_changes();
    selected(tx, expected, before, limits)?;
    // Preserve actual registry/lens/order identities, not merely their claimed
    // normalization digests. Only implementation and source revision move.
    if !same(
        &before.entity_registry,
        &after.entity_registry,
        limits.max_metadata_bytes,
    )? || !same(
        &before.relation_registry,
        &after.relation_registry,
        limits.max_metadata_bytes,
    )? || !same(
        &JsonValue::Array(before.lenses.clone()),
        &JsonValue::Array(after.lenses.clone()),
        limits.max_metadata_bytes,
    )? || before.source_order_profile != after.source_order_profile
    {
        return Err(Error::Invalid(
            "normalization migration changes catalog inputs",
        ));
    }
    let stable_header = |header: &JsonValue| -> Result<JsonValue> {
        let fields = header
            .as_object()
            .ok_or(Error::Invalid("normalization migration header"))?;
        Ok(JsonValue::Object(
            fields
                .iter()
                .filter(|(key, _)| {
                    !matches!(
                        key.as_str(),
                        Some("normalization_binding" | "source_revision")
                    )
                })
                .cloned()
                .collect(),
        ))
    };
    if !same(
        &stable_header(&before.header)?,
        &stable_header(&after.header)?,
        limits.max_metadata_bytes,
    )? {
        return Err(Error::Invalid(
            "normalization migration changes header contract",
        ));
    }
    let report = semantic::apply_semantic_delta_transaction(
        tx,
        expected,
        required(&after.header, "source_revision")?,
        &[],
        &before.entity_registry,
        &before.relation_registry,
        reviewed.before_processor,
        semantic_limits(tx, start, limits, semantic_cap)?,
    )?;
    let mut successor = after.clone();
    let mut counts = successor
        .header
        .object_get("counts")
        .cloned()
        .unwrap_or(JsonValue::Object(Vec::new()));
    replace_field(&mut counts, "semantic_validation", report.clone())?;
    replace_field(&mut successor.header, "counts", counts)?;
    let mut remaining_limits = limits;
    remaining_limits.max_mutations = remaining(tx, start, limits)?;
    let mut receipt = apply_catalogued_retained_with_transition(
        tx,
        expected,
        before,
        &successor,
        &[],
        remaining_limits,
        catalog_limits,
        Some(reviewed),
    )?;
    semantic::transition_pending_normalization_transaction(
        tx,
        expected,
        &receipt.binding,
        reviewed,
        semantic_limits(tx, start, limits, semantic_cap)?,
    )?;
    let verification = semantic::verify_semantic_index_binding_transaction(
        tx,
        &receipt.binding,
        reviewed.after_processor,
        semantic_limits(tx, start, limits, semantic_cap)?,
    )?;
    successor.header = receipt
        .source_header
        .clone()
        .ok_or(Error::Invalid("normalization migration finalized header"))?;
    selected(tx, &receipt.binding, &successor, limits)?;
    receipt.semantic_report_sha256 =
        Some(required(&verification, "semantic_report_sha256")?.to_owned());
    receipt.semantic_report = Some(report);
    receipt.sql_mutations = tx
        .total_changes()
        .checked_sub(start)
        .ok_or(Error::Invalid("normalization migration mutation counter"))?;
    if receipt.sql_mutations > limits.max_mutations {
        return Err(Error::Budget("normalization migration combined mutations"));
    }
    Ok(receipt)
}
