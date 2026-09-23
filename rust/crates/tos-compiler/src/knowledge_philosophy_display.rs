//! Bounded Python-compatible display projection for ordinary philosophy nodes.
//!
//! This is one semantic primitive, not `philosophy-node-edge-v1` materialization.
//! Registry label selection, source admission, relation endpoint titles,
//! placeholders, inherited views and readable context remain separate owners.

use crate::{Error, Result, knowledge_normalization::SourceRow};
use serde_json::{Map, Value};
use std::io::Write;

const MAX_DISPLAY_BYTES: usize = 1024 * 1024;
const MISSING_SUMMARY_RU: &str = "Развёрнутое описание пока не добавлено.";
const MISSING_SUMMARY_EN: &str = "A detailed description has not been added yet.";
const MISSING_EXPLANATION_RU: &str = "Отдельное пояснение к этой связи пока не добавлено.";
const MISSING_EXPLANATION_EN: &str =
    "A separate explanation of this relationship has not been added yet.";

struct CappedWriter {
    bytes: usize,
    ceiling: usize,
}
impl Write for CappedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(buf.len())
            .filter(|next| *next <= self.ceiling)
            .ok_or_else(|| std::io::Error::other("philosophy display byte ceiling"))?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn text(value: Option<&Value>) -> Option<String> {
    value?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_object()?.get(key)
}

fn language_key(key: &str) -> bool {
    if matches!(key, "default" | "original") {
        return true;
    }
    let mut parts = key.split('-');
    let first = parts.next().unwrap_or_default();
    if first.eq_ignore_ascii_case("i") || first.eq_ignore_ascii_case("x") {
        let mut count = 0;
        for part in parts {
            count += 1;
            if part.is_empty() || part.len() > 8 || !part.bytes().all(|c| c.is_ascii_alphanumeric())
            {
                return false;
            }
        }
        return count > 0;
    }
    if first.len() < 2 || first.len() > 8 || !first.bytes().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    parts.all(|part| {
        !part.is_empty() && part.len() <= 8 && part.bytes().all(|c| c.is_ascii_alphanumeric())
    })
}

fn form_items(value: Option<&Value>) -> Map<String, Value> {
    let mut forms = Map::new();
    if let Some(source) = value.and_then(Value::as_object) {
        for (key, member) in source {
            if language_key(key) {
                forms.insert(
                    key.clone(),
                    text(Some(member)).map(Value::String).unwrap_or(Value::Null),
                );
            }
        }
    }
    forms
}

fn display_text(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(forms) = value.as_object() {
        for key in ["default", "ru", "en", "original"] {
            if let Some(result) = text(forms.get(key)) {
                return Some(result);
            }
        }
        for (key, member) in forms {
            if !matches!(key.as_str(), "default" | "ru" | "en" | "original") && language_key(key) {
                if let Some(result) = text(Some(member)) {
                    return Some(result);
                }
            }
        }
        None
    } else {
        text(Some(value))
    }
}

fn localized(default: &str) -> Map<String, Value> {
    let mut forms = Map::new();
    forms.insert("default".into(), Value::String(default.to_owned()));
    for key in ["ru", "en", "original"] {
        forms.insert(key.into(), Value::Null);
    }
    forms
}

fn localized_from(value: Option<&Value>, fallback: &str) -> Map<String, Value> {
    if value.is_some_and(Value::is_object) {
        let mut forms = localized(fallback);
        forms.extend(form_items(value));
        let default = display_text(Some(&Value::Object(form_items(value))))
            .unwrap_or_else(|| fallback.to_owned());
        forms.insert("default".into(), Value::String(default));
        forms
    } else {
        localized(&text(value).unwrap_or_else(|| fallback.to_owned()))
    }
}

fn humanize(value: &str) -> String {
    let mut result = String::new();
    let mut separated = false;
    for ch in value.chars() {
        if matches!(ch, '-' | '_' | '.') {
            if !separated {
                result.push(' ');
            }
            separated = true;
        } else {
            result.push(ch);
            separated = false;
        }
    }
    let result = result.trim();
    if result.is_empty() {
        "unnamed".into()
    } else {
        result.into()
    }
}

fn first_text(values: &[Option<&Value>]) -> Option<String> {
    values.iter().find_map(|value| text(*value))
}

fn check_size(value: &Value) -> Result<()> {
    let mut writer = CappedWriter {
        bytes: 0,
        ceiling: MAX_DISPLAY_BYTES,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| Error::Budget("philosophy display bytes"))
}

/// Exact `_node_display` branch for ordinary `tos_philosophy_graph_projection_v2`
/// carriers. `effective_type_labels` must be the labels from the selected
/// source mapping (or registry type fallback), not invented from `kind_id`.
/// Unsupported source variants refuse; a complete source adapter must route
/// them through the remaining shared normalization functions.
pub fn ordinary_philosophy_node_display(
    row: &SourceRow,
    kind_id: &str,
    effective_type_labels: Option<&Value>,
) -> Result<Value> {
    let item = row.value();
    let props = field(item, "properties").and_then(Value::as_object);
    if kind_id.is_empty()
        || text(field(item, "node_id")).is_none()
        || field(item, "display").is_some()
        || props.is_some_and(|p| p.contains_key("variant_labels") || p.contains_key("value"))
        || kind_id == "claim"
        || kind_id == "temporal-assertion"
        || text(field(item, "node_kind")).as_deref() == Some("literal")
    {
        return Err(Error::Invalid("unsupported philosophy display variant"));
    }
    let prop = |key| props.and_then(|p| p.get(key));
    let explicit = first_text(&[
        field(item, "label"),
        field(item, "canonical_label"),
        prop("preferred_label"),
    ]);
    let path_label = first_text(&[
        field(item, "title"),
        field(item, "name"),
        field(item, "path"),
        field(item, "declared_path"),
    ]);
    let label = explicit
        .clone()
        .or_else(|| path_label.clone())
        .unwrap_or_else(|| {
            humanize(
                &text(field(item, "node_id"))
                    .or_else(|| text(field(item, "id")))
                    .unwrap_or_else(|| "node".into()),
            )
        });
    let multilingual_labels = field(item, "multilingual").and_then(|m| field(m, "label"));
    let mut title = localized_from(None, &label);
    for (language, wording) in form_items(multilingual_labels) {
        if title.get(&language).is_none_or(Value::is_null) {
            title.insert(language, wording);
        }
    }
    if explicit.is_none() && path_label.is_none() {
        let other = Value::Object(
            title
                .iter()
                .filter(|(key, _)| key.as_str() != "default")
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        );
        title.insert(
            "default".into(),
            Value::String(display_text(Some(&other)).unwrap_or(label)),
        );
    }
    let registry_labels = effective_type_labels.and_then(Value::as_object);
    let kind_fallback = registry_labels
        .and_then(|m| text(m.get("default")))
        .unwrap_or_else(|| humanize(kind_id));
    let mut kind_label = localized_from(None, &kind_fallback);
    for (language, wording) in form_items(effective_type_labels) {
        if kind_label.get(&language).is_none_or(Value::is_null) {
            kind_label.insert(language, wording);
        }
    }
    let authored_summary = first_text(&[
        field(item, "distilled_thesis"),
        field(item, "summary"),
        field(item, "description"),
        field(item, "role"),
        field(item, "purpose"),
        prop("distilled_thesis"),
        prop("summary"),
        prop("description"),
        prop("role"),
        prop("purpose"),
        prop("comment"),
    ]);
    let summary_state = if authored_summary.is_some() {
        "source-derived"
    } else {
        "metadata-synthesis"
    };
    let summary_default = authored_summary
        .clone()
        .unwrap_or_else(|| MISSING_SUMMARY_RU.into());
    let mut summary = localized_from(None, &summary_default);
    if authored_summary.is_none() {
        summary.insert("ru".into(), Value::String(MISSING_SUMMARY_RU.into()));
        summary.insert("en".into(), Value::String(MISSING_SUMMARY_EN.into()));
    }
    let source_title_available = explicit.is_some()
        || text(field(item, "title")).is_some()
        || text(field(item, "name")).is_some()
        || display_text(Some(&Value::Object(
            title
                .iter()
                .filter(|(key, _)| key.as_str() != "default")
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        )))
        .is_some();
    let mut provenance = Map::new();
    provenance.insert(
        "title".into(),
        Value::String(
            if explicit.is_some() {
                "projected-label"
            } else if path_label.is_some() {
                "projected-path"
            } else {
                "identifier-fallback"
            }
            .into(),
        ),
    );
    provenance.insert("summary".into(), Value::String(summary_state.into()));
    provenance.insert(
        "source_title_available".into(),
        Value::Bool(source_title_available),
    );
    provenance.insert(
        "source_summary_available".into(),
        Value::Bool(authored_summary.is_some()),
    );
    let mut display = Map::new();
    display.insert("title".into(), Value::Object(title));
    display.insert("kind_label".into(), Value::Object(kind_label));
    display.insert("summary".into(), Value::Object(summary));
    display.insert("summary_state".into(), Value::String(summary_state.into()));
    display.insert("provenance".into(), Value::Object(provenance));
    let display = Value::Object(display);
    check_size(&display)?;
    Ok(display)
}

/// Exact `_relation_display` ordinary branch, given the two normalized
/// endpoint title objects and the selected effective relation-type entry.
/// No ID/title inference from a source path is permitted. The caller must
/// obtain endpoint titles by indexed exact normalized-ID seek.
pub fn ordinary_philosophy_relation_display(
    row: &SourceRow,
    predicate_id: &str,
    left_title: &Value,
    right_title: &Value,
    effective_relation_type: &Value,
) -> Result<Value> {
    let item = row.value();
    let props = field(item, "properties").and_then(Value::as_object);
    if predicate_id.is_empty()
        || text(field(item, "edge_id")).is_none()
        || text(field(item, "from_id")).is_none()
        || text(field(item, "to_id")).is_none()
        || field(item, "display").is_some()
        || !left_title.is_object()
        || !right_title.is_object()
        || !effective_relation_type.is_object()
    {
        return Err(Error::Invalid(
            "unsupported philosophy relation display variant",
        ));
    }
    let prop = |key| props.and_then(|p| p.get(key));
    let labels = field(effective_relation_type, "labels").and_then(Value::as_object);
    let mappings = field(effective_relation_type, "source_mappings")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("philosophy relation type mappings"))?;
    let mapped_predicates = mappings
        .iter()
        .filter_map(|mapping| text(field(mapping, "source_predicate_id")))
        .collect::<std::collections::BTreeSet<_>>();
    let exact_type_label = mapped_predicates.len() <= 1;
    let label_fallback = text(prop("relation_label"))
        .or_else(|| {
            exact_type_label
                .then(|| labels.and_then(|entry| text(entry.get("default"))))
                .flatten()
        })
        .unwrap_or_else(|| humanize(predicate_id));
    let mut label = localized_from(None, &label_fallback);
    if exact_type_label {
        for (language, wording) in form_items(field(effective_relation_type, "labels")) {
            if label.get(&language).is_none_or(Value::is_null) {
                label.insert(language, wording);
            }
        }
    }
    let inverse = field(effective_relation_type, "inverse_labels");
    let inverse_label = if inverse.is_some_and(|value| !value.is_null()) {
        let fallback =
            text(inverse.and_then(|value| field(value, "default"))).unwrap_or_else(|| {
                humanize(
                    &text(field(item, "inverse_predicate_id"))
                        .unwrap_or_else(|| "inverse relation".into()),
                )
            });
        Value::Object(localized_from(inverse, &fallback))
    } else {
        Value::Null
    };
    let left_default = text(field(left_title, "default"))
        .or_else(|| text(field(item, "from_id")))
        .unwrap_or_else(|| "unknown source".into());
    let right_default = text(field(right_title, "default"))
        .or_else(|| text(field(item, "to_id")))
        .unwrap_or_else(|| "unknown target".into());
    let label_default = text(label.get("default")).unwrap_or_else(|| humanize(predicate_id));
    let statement_default = format!("{left_default} — {label_default} → {right_default}.");
    let mut statement = localized_from(None, &statement_default);
    for (language, wording) in &label {
        if matches!(language.as_str(), "default" | "original") {
            continue;
        }
        let Some(wording) = text(Some(wording)) else {
            continue;
        };
        let localized_left = text(field(left_title, language))
            .or_else(|| text(field(left_title, "original")))
            .unwrap_or_else(|| left_default.clone());
        let localized_right = text(field(right_title, language))
            .or_else(|| text(field(right_title, "original")))
            .unwrap_or_else(|| right_default.clone());
        statement.insert(
            language.clone(),
            Value::String(format!("{localized_left} — {wording} → {localized_right}.")),
        );
    }
    let explanation_value = first_text(&[
        field(item, "note"),
        field(item, "comment"),
        prop("comment"),
        prop("note"),
        prop("description"),
    ]);
    let explanation_state = if explanation_value.is_some() {
        "source-derived"
    } else {
        "metadata-synthesis"
    };
    let explanation_default = explanation_value
        .clone()
        .unwrap_or_else(|| MISSING_EXPLANATION_RU.into());
    let mut explanation = localized_from(None, &explanation_default);
    if explanation_value.is_none() {
        explanation.insert("ru".into(), Value::String(MISSING_EXPLANATION_RU.into()));
        explanation.insert("en".into(), Value::String(MISSING_EXPLANATION_EN.into()));
    }
    let provenance_label = if text(prop("relation_label")).is_some() {
        "projected-predicate-label"
    } else if exact_type_label && labels.is_some_and(|labels| !labels.is_empty()) {
        "registry-label"
    } else {
        "identifier-fallback"
    };
    let mut provenance = Map::new();
    provenance.insert("label".into(), Value::String(provenance_label.into()));
    provenance.insert(
        "statement".into(),
        Value::String("endpoint-label-synthesis".into()),
    );
    provenance.insert(
        "explanation".into(),
        Value::String(explanation_state.into()),
    );
    provenance.insert(
        "source_explanation_available".into(),
        Value::Bool(explanation_value.is_some()),
    );
    let mut display = Map::new();
    display.insert("label".into(), Value::Object(label));
    display.insert("inverse_label".into(), inverse_label);
    display.insert("statement".into(), Value::Object(statement));
    display.insert("explanation".into(), Value::Object(explanation));
    display.insert(
        "explanation_state".into(),
        Value::String(explanation_state.into()),
    );
    display.insert("provenance".into(), Value::Object(provenance));
    let display = Value::Object(display);
    check_size(&display)?;
    Ok(display)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn frozen_python_oracle_prepared_dossier() {
        let raw = r#"{"node_id":"atlas-dossier:A01","node_type":"prepared-dossier","label":"ToS Deep Research: A01 — Протоклинопись и учётные онтологии","multilingual":{"label":{"en":"ToS Deep Research: A01 — Proto-Cuneiform and Accounting Ontologies","original":null,"ru":"ToS Deep Research: A01 — Протоклинопись и учётные онтологии"}},"properties":{}}"#;
        let row = SourceRow::parse(raw.as_bytes(), 2048).unwrap();
        let labels = json!({"default":"Prepared dossier","ru":"Подготовленное досье","en":"Prepared dossier"});
        let actual =
            ordinary_philosophy_node_display(&row, "prepared-dossier", Some(&labels)).unwrap();
        let expected = json!({
            "kind_label":{"default":"Prepared dossier","en":"Prepared dossier","original":null,"ru":"Подготовленное досье"},
            "provenance":{"source_summary_available":false,"source_title_available":true,"summary":"metadata-synthesis","title":"projected-label"},
            "summary":{"default":"Развёрнутое описание пока не добавлено.","en":"A detailed description has not been added yet.","original":null,"ru":"Развёрнутое описание пока не добавлено."},
            "summary_state":"metadata-synthesis",
            "title":{"default":"ToS Deep Research: A01 — Протоклинопись и учётные онтологии","en":"ToS Deep Research: A01 — Proto-Cuneiform and Accounting Ontologies","original":null,"ru":"ToS Deep Research: A01 — Протоклинопись и учётные онтологии"}
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn frozen_python_oracle_authored_summary_and_language_tag() {
        let raw = r#"{"node_id":"candidate-node:one","label":"  ","node_type":"candidate-node","multilingual":{"label":{"en":"English title","x-source":"Original title"}},"properties":{"preferred_label":"Название","summary":"Русское описание"}}"#;
        let row = SourceRow::parse(raw.as_bytes(), 1024).unwrap();
        let labels = json!({"default":"Candidate node","en":"Candidate node","ru":"Кандидат"});
        let actual =
            ordinary_philosophy_node_display(&row, "candidate-node", Some(&labels)).unwrap();
        let expected = json!({
            "kind_label":{"default":"Candidate node","en":"Candidate node","original":null,"ru":"Кандидат"},
            "provenance":{"source_summary_available":true,"source_title_available":true,"summary":"source-derived","title":"projected-label"},
            "summary":{"default":"Русское описание","en":null,"original":null,"ru":null},
            "summary_state":"source-derived",
            "title":{"default":"Название","en":"English title","original":null,"ru":null,"x-source":"Original title"}
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn unsupported_variants_and_malformed_rows_refuse() {
        let row = SourceRow::parse(br#"{"node_id":"n","display":{"title":"x"}}"#, 1024).unwrap();
        assert!(ordinary_philosophy_node_display(&row, "n", None).is_err());
        let row = SourceRow::parse(
            br#"{"node_id":"n","properties":{"variant_labels":[]}}"#,
            1024,
        )
        .unwrap();
        assert!(ordinary_philosophy_node_display(&row, "n", None).is_err());
        let row = SourceRow::parse(br#"{"path":"ToS/x","label":"X"}"#, 1024).unwrap();
        assert!(ordinary_philosophy_node_display(&row, "n", None).is_err());
        assert!(SourceRow::parse(br#"{"node_id":"n","node_id":"m"}"#, 1024).is_err());
    }

    #[test]
    fn frozen_python_oracle_projection_pressure_relation() {
        let raw = r#"{"edge_id":"edge:atlas:node-type:concept","from_id":"philosophy.atlas","predicate_id":"has_node_type_pressure","to_id":"atlas-node-type:concept","properties":{"count":884}}"#;
        let row = SourceRow::parse(raw.as_bytes(), 1024).unwrap();
        let relation_type = json!({
            "labels":{"default":"records projection pressure","ru":"фиксирует давление проекции","en":"records projection pressure"},
            "source_mappings":[
                {"source_graph":"philosophy","source_predicate_id":"has_node_type_pressure","scope":"edge"},
                {"source_graph":"philosophy","source_predicate_id":"has_relation_pressure","scope":"edge"}
            ],
            "inverse_labels":null
        });
        let actual = ordinary_philosophy_relation_display(
            &row,
            "has_node_type_pressure",
            &json!({"default":"Philosophy Atlas"}),
            &json!({"default":"concept"}),
            &relation_type,
        )
        .unwrap();
        let expected = json!({
            "explanation":{"default":"Отдельное пояснение к этой связи пока не добавлено.","en":"A separate explanation of this relationship has not been added yet.","original":null,"ru":"Отдельное пояснение к этой связи пока не добавлено."},
            "explanation_state":"metadata-synthesis",
            "inverse_label":null,
            "label":{"default":"has node type pressure","en":null,"original":null,"ru":null},
            "provenance":{"explanation":"metadata-synthesis","label":"identifier-fallback","source_explanation_available":false,"statement":"endpoint-label-synthesis"},
            "statement":{"default":"Philosophy Atlas — has node type pressure → concept.","en":null,"original":null,"ru":null}
        });
        assert_eq!(actual, expected);
    }
}
