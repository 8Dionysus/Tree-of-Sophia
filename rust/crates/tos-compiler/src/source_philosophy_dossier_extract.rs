//! Prepared-DOCX row extraction draft for a future native source planting role.
//! This remains outside the immutable source checkout and writes no files.
use crate::source_philosophy_dossier_docx::{
    DocxDocument, DocxValidationIssue, row_value, table_body_rows, table_family,
    validate_identity_and_headers,
};
use serde_json::{Map, Value, json};

#[derive(Clone, Debug)]
pub struct PreparedDossier {
    pub table_id: String,
    pub dossier_id: String,
    pub title: String,
    pub source_document: String,
    pub docx_section: String,
    pub paragraph_count: usize,
    pub table_count: usize,
    pub table_row: String,
    pub master_table: String,
    pub master_status: String,
    pub master_confidence: String,
    pub branch_path: Option<String>,
    pub branch_role: Option<String>,
    pub admission_status: String,
    pub identity_diagnostics: Vec<String>,
    pub node_rows: Vec<Value>,
    pub relation_rows: Vec<Value>,
    pub source_rows: Vec<Value>,
    pub term_rows: Vec<Value>,
    pub transmission_rows: Vec<Value>,
    pub metadata_identity_posture: String,
    pub metadata_headers: Vec<Vec<String>>,
    pub coverage_tables: Vec<Value>,
    pub intake_metadata: Value,
}

fn aliases(family: &str, field: &str) -> &'static [&'static str] {
    match (family, field) {
        ("proposed_nodes", "original_node_id") => &["Node ID"],
        ("proposed_nodes", "node_kind_label") => &["Тип узла", "Тип"],
        ("proposed_nodes", "label") => &["Название"],
        ("proposed_nodes", "period") => &["Период"],
        ("proposed_nodes", "connections") => &[
            "Связи",
            "Связи / функция",
            "Основные связи",
            "Ключевые связи",
        ],
        ("proposed_nodes", "priority") => &["Приоритет", "Приор."],
        ("proposed_relations", "original_relation_id") => &["Edge ID"],
        ("proposed_relations", "source_endpoint_label") => {
            &["Source node", "Source", "Исходный узел"]
        }
        ("proposed_relations", "relation_label") => &["Relation", "Отношение"],
        ("proposed_relations", "target_endpoint_label") => {
            &["Target node", "Target", "Целевой узел"]
        }
        ("proposed_relations", "comment") => &["Комментарий"],
        ("proposed_relations", "confidence") => &["Уверенность", "Увер.", "Ув."],
        ("corpus_or_edition_anchors", "source_local_id") => &["ID", "Код", "Маркер"],
        ("corpus_or_edition_anchors", "source_label") => &[
            "Источник / корпус",
            "Источник",
            "Корпус / архив",
            "Корпус / портал",
            "Корпус",
            "ID / источник",
        ],
        ("corpus_or_edition_anchors", "source_type") => &["Тип", "Тип / дата", "Тип / содержание"],
        ("corpus_or_edition_anchors", "source_date_or_layer") => &["Дата / слой"],
        ("corpus_or_edition_anchors", "contribution") => &[
            "Что даёт",
            "Что даёт ToS",
            "Что даёт / где искать",
            "Что даёт / доступ",
        ],
        ("corpus_or_edition_anchors", "source_access") => &[
            "Доступ / где искать",
            "Доступ",
            "Доступ / stable URL",
            "Доступ / надёжность",
            "Доступ / замечание",
        ],
        ("corpus_or_edition_anchors", "source_locator") => &["Ссылка", "URL / DOI"],
        ("corpus_or_edition_anchors", "reliability") => &[
            "Надёжность",
            "Надёжность / ограничение",
            "Надёжность / ограничения",
            "Надёжность и ограничения",
            "Надёжность / caveat",
            "Ограничения",
            "Доступ / надёжность",
        ],
        ("control_or_review_anchors", "source_local_id") => &["ID", "Код", "Маркер"],
        ("control_or_review_anchors", "source_label") => &["Источник", "ID / источник"],
        ("control_or_review_anchors", "source_type") => &["Тип", "Тип / дата", "Тип / содержание"],
        ("control_or_review_anchors", "contribution") => &["Зачем нужен"],
        ("control_or_review_anchors", "limitations") => &[
            "Ограничения",
            "Ограничение",
            "Ограничения / контроль",
            "Ограничения / доступ",
        ],
        ("control_or_review_anchors", "source_access") => &[
            "Доступ / где искать",
            "Доступ",
            "Доступ / stable URL",
            "Ограничения / доступ",
        ],
        ("control_or_review_anchors", "source_locator") => &["Ссылка", "URL / DOI"],
        ("risk_control_source_needs", "problem") => &["Проблема"],
        ("risk_control_source_needs", "risk") => &["Риск", "Главный риск", "В чём риск"],
        ("risk_control_source_needs", "risk_explanation") => &[
            "В чём опасность",
            "Почему критичен",
            "Почему опасен",
            "В чём искажение",
            "Как искажает строку",
            "Почему возникает",
            "Почему критичен для T2-51",
            "Почему существенен",
            "Проявление",
            "В чём ловушка",
        ],
        ("risk_control_source_needs", "control") => &[
            "Как контролировать в ToS",
            "Контроль в ToS",
            "Контроль ToS",
            "Контроль",
            "Что контролировать",
            "Что именно контролировать",
            "Контрольный принцип",
        ],
        ("risk_control_source_needs", "needed_sources") => {
            &["Какие источники нужны", "Нужные источники", "Что требуется"]
        }
        ("terms", "term") => &["Термин"],
        ("terms", "language") => &["Язык"],
        ("terms", "transliteration") => &[
            "Транслитерация",
            "Транслитерация / форма",
            "Транслитерация / перевод",
            "Транслитерация / аббр.",
        ],
        ("terms", "meaning") => &["Краткое значение", "Значение"],
        ("terms", "role_in_tos") => &["Роль в ToS"],
        ("incoming_transmissions", "from_or_to") => {
            &["Источник / предыдущий узел", "Источник / previous node"]
        }
        ("incoming_transmissions", "transmitted") => &["Что передано"],
        ("incoming_transmissions", "transmission_channel") => &["Канал передачи", "Канал"],
        ("incoming_transmissions", "confidence") => &["Уверенность"],
        ("incoming_transmissions", "note") => &["Примечание"],
        ("outgoing_transmissions", "from_or_to") => &["Следующий узел / эпоха"],
        ("outgoing_transmissions", "transmitted") => &["Что передаётся"],
        ("outgoing_transmissions", "transmission_channel") => &["Канал"],
        ("outgoing_transmissions", "confidence") => &["Уверенность"],
        ("outgoing_transmissions", "verify_next") => &["Что проверить дальше", "Проверить дальше"],
        _ => &[],
    }
}

fn field(row: &[String], header: &[String], family: &str, name: &str) -> String {
    row_value(header, row, aliases(family, name))
}

fn string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Bool(true)) => "True".into(),
        Some(Value::Bool(false) | Value::Null) | None => String::new(),
        Some(Value::Number(number)) if number.as_f64() == Some(0.0) => String::new(),
        Some(value) => value.to_string(),
    }
}

fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Number(number)) => number.as_f64().is_some_and(|value| value != 0.0),
        Some(Value::Array(values)) => !values.is_empty(),
        Some(Value::Object(values)) => !values.is_empty(),
        _ => true,
    }
}

fn slugify(value: &str) -> String {
    let points = value.chars().count();
    let value = tos_foundation::python_lower_unicode16_v1(value, points, usize::MAX, usize::MAX)
        .expect("Unicode16 lowercase with input-derived unbounded output limits")
        .replace('ё', "е");
    let mut slug = String::with_capacity(value.len());
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || ('а'..='я').contains(&character)
        {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character);
            separator = false;
        } else {
            separator = true;
        }
    }
    if slug.is_empty() {
        "unnamed".into()
    } else {
        slug
    }
}

fn route_projection(
    route: &Value,
    package: &Value,
    table_id: &str,
    master_status: &str,
    master_confidence: &str,
) -> Result<Map<String, Value>, String> {
    let route_kind = string(route.get("route_kind"));
    let defaults = package
        .get("route_defaults")
        .and_then(Value::as_object)
        .ok_or("package route_defaults is not an object")?;
    let default_kind = string(defaults.get("route_kind"));
    let policy = package.get("review_policy").and_then(Value::as_object);
    let manual_status = policy
        .and_then(|policy| policy.get("manual_review_statuses"))
        .and_then(Value::as_array)
        .is_some_and(|values| {
            values
                .iter()
                .any(|value| string(Some(value)) == master_status)
        });
    let confidence = master_confidence.trim().parse::<i64>().ok();
    let confidence_limit = policy
        .and_then(|policy| policy.get("manual_review_max_confidence"))
        .and_then(Value::as_i64);
    let manual_by_policy = manual_status
        || confidence
            .zip(confidence_limit)
            .is_some_and(|(value, limit)| value <= limit);
    let manual_review =
        string(route.get("review_posture")) == "manual_review_required" || manual_by_policy;
    let non_default_route = route_kind != default_kind;
    if !manual_review && !non_default_route {
        return Ok(Map::new());
    }
    let mut result = Map::new();
    result.insert(
        "review_posture".into(),
        Value::String(if manual_review {
            "manual_review_required".into()
        } else {
            string(route.get("review_posture"))
        }),
    );
    result.insert("route_kind".into(), Value::String(route_kind));
    if manual_review {
        result.insert(
            "master_confidence".into(),
            Value::String(master_confidence.into()),
        );
        result.insert("master_status".into(), Value::String(master_status.into()));
    }
    let review_reason = if truthy(route.get("review_reason")) {
        string(route.get("review_reason"))
    } else if manual_by_policy {
        format!(
            "{table_id} master status {} and confidence {} require manual review under the package review policy",
            if master_status.is_empty() {
                "unknown"
            } else {
                master_status
            },
            if master_confidence.is_empty() {
                "unknown"
            } else {
                master_confidence
            }
        )
    } else {
        String::new()
    };
    if !review_reason.is_empty() {
        result.insert("review_reason".into(), Value::String(review_reason));
    }
    if let Some(constraints) = route.get("route_constraints").and_then(Value::as_array) {
        result.insert(
            "route_constraints".into(),
            Value::Array(
                constraints
                    .iter()
                    .map(|value| Value::String(string(Some(value))))
                    .collect(),
            ),
        );
    }
    Ok(result)
}

fn merge_route(row: &mut Value, route_fields: &Map<String, Value>) {
    if let Some(object) = row.as_object_mut() {
        object.extend(route_fields.clone());
    }
}

fn add_table_id(row: &mut Value, table_id: &str) {
    if table_id != "table-i" {
        row["table_id"] = Value::String(table_id.to_owned());
    }
}

fn source_ref_for(kind: &str, table_id: &str) -> String {
    match kind {
        "proposed_nodes" => format!(
            "ToS/philosophy/graph-workbench/proposed-nodes/{table_id}-prepared-dossiers.jsonl"
        ),
        "proposed_relations" => format!(
            "ToS/philosophy/graph-workbench/proposed-relations/{table_id}-prepared-dossiers.jsonl"
        ),
        "terms" => "ToS/philosophy/atlas/dossiers/term-index.jsonl".into(),
        "transmission" => "ToS/philosophy/atlas/dossiers/transmission-backlog.jsonl".into(),
        _ => "ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl".into(),
    }
}

fn route_error(error: impl Into<String>) -> Vec<DocxValidationIssue> {
    vec![DocxValidationIssue {
        code: "docx_route_invalid".into(),
        message: error.into(),
        blocking: true,
    }]
}

pub fn extract_dossier(
    document: &DocxDocument,
    table_id: &str,
    dossier_id: &str,
    source_document: &str,
    docx_section: &str,
    master_row: &Value,
    route: Option<&Value>,
    blocked: Option<&Value>,
    package: &Value,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<PreparedDossier, Vec<DocxValidationIssue>> {
    let validation = validate_identity_and_headers(
        document, table_id, dossier_id, master_row, route, blocked, check,
    )?;
    let normalized = master_row
        .get("normalized")
        .and_then(Value::as_object)
        .ok_or_else(|| route_error("master normalized value is not an object"))?;
    let master_status = string(normalized.get("status"));
    let master_confidence = string(normalized.get("confidence"));
    let route_fields = if let Some(route) = route {
        route_projection(route, package, table_id, &master_status, &master_confidence)
            .map_err(route_error)?
    } else {
        Map::new()
    };
    let branch_path = route
        .and_then(|route| route.get("branch_path"))
        .map(|value| string(Some(value)));
    let branch_role = route
        .and_then(|route| route.get("branch_role"))
        .map(|value| string(Some(value)));
    let admission_status = if route.is_some() {
        "admitted".to_owned()
    } else {
        blocked
            .and_then(|value| value.get("posture"))
            .filter(|value| truthy(Some(value)))
            .map(|value| string(Some(value)))
            .unwrap_or_else(|| "blocked".into())
    };
    let mut dossier = PreparedDossier {
        table_id: table_id.into(),
        dossier_id: dossier_id.into(),
        title: validation.title.clone(),
        source_document: source_document.into(),
        docx_section: docx_section.into(),
        paragraph_count: validation.paragraph_count,
        table_count: validation.table_count,
        table_row: validation.table_row,
        master_table: match table_id {
            "table-i" => "I",
            "table-ii" => "II",
            "table-iii" => "III",
            _ => table_id,
        }
        .into(),
        master_status,
        master_confidence,
        branch_path,
        branch_role,
        admission_status,
        identity_diagnostics: validation.identity_diagnostics,
        node_rows: Vec::new(),
        relation_rows: Vec::new(),
        source_rows: Vec::new(),
        term_rows: Vec::new(),
        transmission_rows: Vec::new(),
        metadata_identity_posture: validation.metadata_identity_posture,
        metadata_headers: validation.metadata_headers,
        coverage_tables: Vec::new(),
        intake_metadata: json!({
            "creator": document.metadata.creator,
            "custom_generator": document.metadata.custom_generator,
            "last_modified_by": document.metadata.last_modified_by,
            "signature_part_count": document.metadata.signature_part_count,
            "sha256": document.sha256,
            "size_bytes": document.size_bytes,
        }),
    };
    let overrides = route
        .and_then(|route| route.get("reviewed_node_label_overrides"))
        .and_then(Value::as_object);
    for (table_index, table) in document.tables.iter().enumerate() {
        check(1).map_err(|error| route_error(format!("plant budget refused: {error}")))?;
        let header = table.rows.first().cloned().unwrap_or_default();
        let family = table_family(&header);
        let body = table_body_rows(table);
        let blocked_input = route.is_none();
        let (coverage_class, coverage_family, underlying_family) = if blocked_input {
            (
                "quarantined_identity_mismatch",
                "blocked_master_identity_mismatch",
                Some(family),
            )
        } else if matches!(
            family,
            "proposed_nodes"
                | "proposed_relations"
                | "corpus_or_edition_anchors"
                | "control_or_review_anchors"
                | "risk_control_source_needs"
                | "terms"
                | "incoming_transmissions"
                | "outgoing_transmissions"
        ) {
            ("structured_primary_extracted", family, None)
        } else if family == "dossier_identity_metadata" {
            (
                "identity_metadata_examined",
                "dossier_identity_metadata",
                None,
            )
        } else {
            (
                "deferred_context",
                if family == "dossier_identity_metadata_alias" {
                    "other_context"
                } else {
                    family
                },
                None,
            )
        };
        let mut coverage = json!({
            "coverage_class": coverage_class,
            "family": coverage_family,
            "header": header,
            "row_count": body.len(),
            "source_table_index": table_index + 1,
        });
        if let Some(underlying_family) = underlying_family {
            coverage["underlying_family"] = Value::String(underlying_family.into());
        }
        dossier.coverage_tables.push(coverage);
        if blocked_input {
            continue;
        }
        for (row_index, cells) in body {
            let work = cells
                .iter()
                .try_fold(1u64, |total, cell| total.checked_add(cell.len() as u64))
                .ok_or_else(|| route_error("plant work accounting overflow"))?;
            check(work).map_err(|error| route_error(format!("plant budget refused: {error}")))?;
            let values = |name: &str| field(cells, &header, family, name);
            match family {
                "proposed_nodes" => {
                    let original_id = {
                        let value = values("original_node_id");
                        if value.is_empty() {
                            format!("{dossier_id}-node-{row_index:03}")
                        } else {
                            value
                        }
                    };
                    let node_kind_label = values("node_kind_label");
                    let node_kind = if node_kind_label.is_empty() {
                        "unspecified".into()
                    } else {
                        slugify(&node_kind_label).replace('-', "_")
                    };
                    let mut label = values("label");
                    if label.is_empty() {
                        label.clone_from(&original_id);
                    }
                    let mut override_projection = Map::new();
                    if let Some(value) = overrides.and_then(|values| values.get(&original_id)) {
                        let Some(value) = value.as_object() else {
                            return Err(route_error(format!(
                                "{dossier_id} node label override for {original_id} must be an object"
                            )));
                        };
                        let reviewed_label = crate::source_philosophy_dossier_docx::scrub(&string(
                            value.get("label"),
                        ));
                        let reason = crate::source_philosophy_dossier_docx::scrub(&string(
                            value.get("reason"),
                        ));
                        if reviewed_label.is_empty() || reason.is_empty() {
                            return Err(route_error(format!(
                                "{dossier_id} node label override for {original_id} requires label and reason"
                            )));
                        }
                        label = reviewed_label;
                        override_projection.insert(
                            "label_normalization".into(),
                            json!("reviewed_source_punctuation_normalization"),
                        );
                        override_projection
                            .insert("label_override_reason".into(), Value::String(reason));
                    }
                    let candidate_id = format!(
                        "{table_id}-{}-node-{row_index:03}",
                        dossier_id.to_lowercase()
                    );
                    let mut row = json!({
                        "atlas_row_id": dossier_id,
                        "authority_posture": "prepared_research_candidate",
                        "branch_path": dossier.branch_path,
                        "candidate_id": candidate_id,
                        "canon_status": "pre-canon",
                        "connections": values("connections"),
                        "dossier_id": dossier_id,
                        "label": label,
                        "node_kind": node_kind,
                        "node_kind_label": node_kind_label,
                        "original_node_id": original_id,
                        "period": values("period"),
                        "priority": values("priority"),
                        "source_document": source_document,
                        "source_row_index": row_index,
                        "source_table_index": table_index + 1,
                        "source_ref": source_ref_for("proposed_nodes", table_id),
                    });
                    row.as_object_mut().unwrap().extend(override_projection);
                    add_table_id(&mut row, table_id);
                    merge_route(&mut row, &route_fields);
                    dossier.node_rows.push(row);
                }
                "proposed_relations" => {
                    let relation_label = values("relation_label");
                    let relation_kind = slugify(&relation_label).replace('-', "_");
                    let candidate_id = format!(
                        "{table_id}-{}-relation-{:03}",
                        dossier_id.to_lowercase(),
                        dossier.relation_rows.len() + 1
                    );
                    let original_id = values("original_relation_id");
                    let mut row = json!({
                        "atlas_row_id": dossier_id,
                        "authority_posture": "prepared_research_candidate",
                        "branch_path": dossier.branch_path,
                        "candidate_id": candidate_id,
                        "canon_status": "pre-canon",
                        "comment": values("comment"),
                        "confidence": values("confidence"),
                        "dossier_id": dossier_id,
                        "relation_kind": if relation_kind.is_empty() { "related_to" } else { relation_kind.as_str() },
                        "relation_label": relation_label,
                        "source_document": source_document,
                        "source_endpoint_label": values("source_endpoint_label"),
                        "source_ref": source_ref_for("proposed_relations", table_id),
                        "source_row_index": row_index,
                        "source_table_index": table_index + 1,
                        "target_endpoint_label": values("target_endpoint_label"),
                    });
                    if !original_id.is_empty() {
                        row["original_relation_id"] = Value::String(original_id);
                    }
                    add_table_id(&mut row, table_id);
                    merge_route(&mut row, &route_fields);
                    dossier.relation_rows.push(row);
                }
                "corpus_or_edition_anchors" => {
                    let mut row = json!({
                        "anchor_kind": "corpus_or_edition_anchor",
                        "atlas_row_id": dossier_id,
                        "branch_path": dossier.branch_path,
                        "contribution": values("contribution"),
                        "dossier_id": dossier_id,
                        "reliability": values("reliability"),
                        "route_status": "source_anchor_backlog",
                        "source_document": source_document,
                        "source_label": values("source_label"),
                        "source_ref": source_ref_for("source", table_id),
                        "source_row_index": row_index,
                        "source_table_index": table_index + 1,
                        "source_type": values("source_type"),
                    });
                    for (key, field_name) in [
                        ("source_date_or_layer", "source_date_or_layer"),
                        ("source_local_id", "source_local_id"),
                        ("source_locator", "source_locator"),
                        ("source_access", "source_access"),
                    ] {
                        let value = values(field_name);
                        if !value.is_empty() {
                            row[key] = Value::String(value);
                        }
                    }
                    add_table_id(&mut row, table_id);
                    merge_route(&mut row, &route_fields);
                    dossier.source_rows.push(row);
                }
                "control_or_review_anchors" => {
                    let mut row = json!({
                        "anchor_kind": "control_or_review_anchor",
                        "atlas_row_id": dossier_id,
                        "branch_path": dossier.branch_path,
                        "contribution": values("contribution"),
                        "dossier_id": dossier_id,
                        "limitations": values("limitations"),
                        "route_status": "source_anchor_backlog",
                        "source_document": source_document,
                        "source_label": values("source_label"),
                        "source_ref": source_ref_for("source", table_id),
                        "source_row_index": row_index,
                        "source_table_index": table_index + 1,
                        "source_type": values("source_type"),
                    });
                    for (key, field_name) in [
                        ("source_local_id", "source_local_id"),
                        ("source_locator", "source_locator"),
                        ("source_access", "source_access"),
                    ] {
                        let value = values(field_name);
                        if !value.is_empty() {
                            row[key] = Value::String(value);
                        }
                    }
                    add_table_id(&mut row, table_id);
                    merge_route(&mut row, &route_fields);
                    dossier.source_rows.push(row);
                }
                "risk_control_source_needs" => {
                    let problem = values("problem");
                    let risk = values("risk");
                    let explanation = values("risk_explanation");
                    let mut row = json!({
                        "anchor_kind": "risk_control_source_need",
                        "atlas_row_id": dossier_id,
                        "branch_path": dossier.branch_path,
                        "control": values("control"),
                        "dossier_id": dossier_id,
                        "needed_sources": values("needed_sources"),
                        "problem": problem,
                        "risk": risk,
                        "route_status": "source_anchor_backlog",
                        "source_document": source_document,
                        "source_ref": source_ref_for("source", table_id),
                        "source_row_index": row_index,
                        "source_table_index": table_index + 1,
                    });
                    if !explanation.is_empty() {
                        row["risk_explanation"] = Value::String(explanation);
                    }
                    add_table_id(&mut row, table_id);
                    merge_route(&mut row, &route_fields);
                    dossier.source_rows.push(row);
                }
                "terms" => {
                    let mut row = json!({
                        "atlas_row_id": dossier_id,
                        "branch_path": dossier.branch_path,
                        "dossier_id": dossier_id,
                        "language": values("language"),
                        "meaning": values("meaning"),
                        "role_in_tos": values("role_in_tos"),
                        "source_document": source_document,
                        "source_ref": source_ref_for("terms", table_id),
                        "source_row_index": row_index,
                        "source_table_index": table_index + 1,
                        "term": values("term"),
                        "transliteration": values("transliteration"),
                    });
                    add_table_id(&mut row, table_id);
                    merge_route(&mut row, &route_fields);
                    dossier.term_rows.push(row);
                }
                "incoming_transmissions" | "outgoing_transmissions" => {
                    let incoming = family == "incoming_transmissions";
                    let mut row = json!({
                        "atlas_row_id": dossier_id,
                        "branch_path": dossier.branch_path,
                        "confidence": values("confidence"),
                        "dossier_id": dossier_id,
                        "direction": if incoming { "incoming" } else { "outgoing" },
                        "from_or_to": values("from_or_to"),
                        "source_document": source_document,
                        "source_ref": source_ref_for("transmission", table_id),
                        "source_row_index": row_index,
                        "source_table_index": table_index + 1,
                        "transmission_channel": values("transmission_channel"),
                        "transmitted": values("transmitted"),
                    });
                    if incoming {
                        row["note"] = Value::String(values("note"));
                    } else {
                        row["verify_next"] = Value::String(values("verify_next"));
                    }
                    add_table_id(&mut row, table_id);
                    merge_route(&mut row, &route_fields);
                    dossier.transmission_rows.push(row);
                }
                _ => {}
            }
        }
    }
    Ok(dossier)
}

pub fn admissions(dossiers: &[PreparedDossier]) -> (Vec<&PreparedDossier>, Vec<&PreparedDossier>) {
    let admitted = dossiers
        .iter()
        .filter(|dossier| dossier.admission_status == "admitted")
        .collect();
    let blocked = dossiers
        .iter()
        .filter(|dossier| dossier.admission_status != "admitted")
        .collect();
    (admitted, blocked)
}
