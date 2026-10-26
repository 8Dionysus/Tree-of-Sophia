//! Bounded, source-returning publisher for explicitly selected public packets.
//! This copies only exact selected source units; it does not assess meaning or rights.

use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};
use tos_foundation::Digest256;

const MAX_VERSIONS: usize = 100;
const MAX_UNITS_PER_VERSION: usize = 200_000;
const MAX_TOTAL_UNITS: usize = 500_000;
const MAX_TEXT_LAYER_BYTES: usize = 8 * 1024 * 1024;
const MAX_SEARCH_INDEX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
    pub max_seconds: u64,
}

fn s<'a>(v: &'a Value, k: &str) -> Result<&'a str, String> {
    v.get(k)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string {k}"))
}
fn arr<'a>(v: &'a Value, k: &str) -> Result<&'a [Value], String> {
    v.get(k)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing array {k}"))
}
fn digest(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_hex()
}
fn check_deadline(deadline: Instant) -> Result<(), String> {
    if Instant::now() >= deadline {
        Err("public corpus build deadline exceeded".into())
    } else {
        Ok(())
    }
}

/// Open only repository-relative references with no parent, absolute, or symlink components.
fn source_path(root: &Path, reference: &str) -> Result<PathBuf, String> {
    let rel = Path::new(reference);
    if rel.as_os_str().is_empty()
        || rel.is_absolute()
        || rel.components().any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(format!("unsafe source reference: {reference}"));
    }
    let mut current = root.to_path_buf();
    for part in rel.components() {
        current.push(part);
        let meta = fs::symlink_metadata(&current)
            .map_err(|e| format!("source reference unavailable: {reference}: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err(format!("symlink source reference refused: {reference}"));
        }
    }
    Ok(current)
}
fn read_ref(
    root: &Path,
    reference: &str,
    expected: &str,
    cap: usize,
    total: &mut usize,
) -> Result<Vec<u8>, String> {
    let path = source_path(root, reference)?;
    let mut file = crate::safe_open::open_regular(&path, cap as u64).map_err(|e| e.to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > cap as u64 {
        return Err(format!("source file invalid or oversized: {reference}"));
    }
    let mut raw = Vec::with_capacity(meta.len() as usize);
    file.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    *total = total.checked_add(raw.len()).ok_or("input size overflow")?;
    if *total > cap {
        return Err("input byte limit exceeded".into());
    }
    if digest(&raw) != expected {
        return Err(format!("source digest mismatch: {reference}"));
    }
    Ok(raw)
}
fn public_rights(rights: &Value, packet: &Value, v: &Value) -> Result<(), String> {
    let rv = &packet["rights_and_visibility"];
    if packet["schema_version"] != "tos_source_text_unit_packet_v1"
        || packet["source_layer"]["visibility"] != "public"
        || packet["source_layer"]["publication_authorized"] != true
        || rv["source_visibility"] != "public"
        || rv["packet_visibility"] != "public"
        || rv["effective_visibility"] != "public"
        || rv["publication_authorized"] != true
        || rv["private_source_used"] != false
    {
        return Err("source or packet is not explicitly authorized public material".into());
    }
    let rights_ref = s(v, "rightsRef")?;
    let refs = rv["rights_record_refs"]
        .as_array()
        .ok_or("packet rights refs absent")?;
    if !refs.iter().any(|x| x.as_str() == Some(rights_ref)) {
        return Err("selected rights record is not bound by packet".into());
    }
    if rights["visibility"] != "public_payload"
        || !matches!(
            rights["redistribution_posture"].as_str(),
            Some("authorized" | "authorized_with_conditions")
        )
        || !matches!(
            rights["assessment_status"].as_str(),
            Some("licensed" | "permission_granted" | "public_domain_reviewed")
        )
        || rights["license_uri"] != v["license"]["url"]
    {
        return Err(
            "rights record does not explicitly authorize this public payload and license".into(),
        );
    }
    if !rights["jurisdictions_reviewed"]
        .as_array()
        .is_some_and(|rows| {
            !rows.is_empty()
                && rows.len() <= 64
                && rows.iter().all(|row| {
                    row.as_str()
                        .is_some_and(|value| value.len() >= 2 && value.len() <= 128)
                })
        })
    {
        return Err("public rights require an explicit reviewed jurisdiction".into());
    }
    let scope = &packet["source_scope"];
    let file_sha = s(scope, "file_sha256")?;
    Digest256::from_hex(file_sha).map_err(|_| "source file digest is malformed")?;
    if s(scope, "file_ref")? != format!("tos.file.sha256.{file_sha}") {
        return Err("source File identity and digest disagree".into());
    }
    let rights_scope = rights["scope_refs"]
        .as_array()
        .ok_or("rights scope refs absent")?;
    for field in ["item_ref", "file_ref"] {
        if !rights_scope
            .iter()
            .any(|r| r.as_str() == scope[field].as_str())
        {
            return Err("rights record does not cover the selected source item and file".into());
        }
    }
    let attribution = s(v, "attribution")?;
    if !rights["permissions"].as_array().is_some_and(|ps| {
        ps.iter()
            .any(|p| p.as_str().is_some_and(|p| p.contains(attribution)))
    }) {
        return Err("rights record does not bind the returned attribution".into());
    }
    Ok(())
}

fn char_byte_map(source: &str, deadline: Instant) -> Result<Vec<usize>, String> {
    if source.len() > MAX_TEXT_LAYER_BYTES {
        return Err("text layer exceeds the fixed byte limit".into());
    }
    let mut offsets = Vec::with_capacity(source.len().min(MAX_TEXT_LAYER_BYTES));
    for (ordinal, (byte, _)) in source.char_indices().enumerate() {
        if ordinal & 0xffff == 0 {
            check_deadline(deadline)?;
        }
        offsets.push(byte);
    }
    offsets.push(source.len());
    Ok(offsets)
}

fn anchor_slice(
    unit: &Value,
    anchors: &HashMap<&str, &Value>,
    text_ref: &str,
    source: &str,
    byte_offsets: &[usize],
) -> Result<(usize, usize), String> {
    let refs = unit["ordered_anchor_refs"]
        .as_array()
        .ok_or("unit anchor refs absent")?;
    if refs.len() != 1 {
        return Err(
            "public delivery requires exactly one contiguous content anchor per unit".into(),
        );
    }
    let aref = refs[0].as_str().ok_or("anchor ref malformed")?;
    let anchor = anchors.get(aref).copied().ok_or("selected anchor absent")?;
    let selector = &anchor["selector"];
    if anchor["anchor_role"] != "content"
        || anchor["text_layer_ref"] != text_ref
        || selector["type"] != "text_position"
        || selector["position_unit"] != "unicode_code_point"
        || selector["interval"] != "half_open"
    {
        return Err("unit anchor is not one source-layer content span".into());
    }
    let start = selector["start"].as_u64().ok_or("anchor start invalid")? as usize;
    let end = selector["end"].as_u64().ok_or("anchor end invalid")? as usize;
    let cps = byte_offsets.len().saturating_sub(1);
    if end < start || end > cps {
        return Err("anchor lies outside source text".into());
    }
    let selected = source
        .get(byte_offsets[start]..byte_offsets[end])
        .ok_or("anchor byte interval invalid")?;
    if digest(selected.as_bytes()) != s(anchor, "exact_sha256")? {
        return Err("anchor text digest mismatch".into());
    }
    Ok((start, end))
}

fn surface_delimiter(ch: char) -> bool {
    ch.is_whitespace()
        || ch.is_ascii_punctuation()
        || matches!(ch as u32,
        0x037e | 0x0387 | 0x055a..=0x055f | 0x0589..=0x058a | 0x05be | 0x05c0 | 0x05c3 | 0x05c6 |
        0x0609..=0x060a | 0x060c | 0x061b | 0x061d..=0x061f | 0x066a..=0x066d | 0x0700..=0x070d |
        0x0964..=0x0965 | 0x0e4f | 0x0f0b | 0x0f0d..=0x0f14 | 0x2000..=0x206f | 0x2e00..=0x2e7f |
        0x3001..=0x303f | 0xfe10..=0xfe1f | 0xfe30..=0xfe4f | 0xff01..=0xff0f | 0xff1a..=0xff20 |
        0xff3b..=0xff40 | 0xff5b..=0xff65)
}

/// Index each exact whitespace/punctuation-delimited surface form. No case folding,
/// normalization, lemmatization, or language-specific token rules are applied here.
fn search_terms(
    text: &str,
    _source: &str,
    _byte_offsets: &[usize],
    _unit_start: usize,
    _reference: &str,
    _unit_id: &str,
    ordinal: usize,
    deadline: Instant,
) -> Result<BTreeMap<String, Vec<Value>>, String> {
    let mut terms = BTreeMap::<String, Vec<Value>>::new();
    let mut token_start: Option<(usize, usize)> = None;
    let mut cp_count = 0usize;
    let flush = |start_cp: usize,
                 start_byte: usize,
                 end_cp: usize,
                 end_byte: usize,
                 terms: &mut BTreeMap<String, Vec<Value>>|
     -> Result<(), String> {
        if end_cp <= start_cp {
            return Ok(());
        }
        let form = text
            .get(start_byte..end_byte)
            .ok_or("surface form byte interval invalid")?
            .to_owned();

        let row = json!([ordinal, start_cp, end_cp]);
        terms.entry(form).or_default().push(row);
        Ok(())
    };
    for (byte, ch) in text.char_indices() {
        if cp_count & 0x3ff == 0 {
            check_deadline(deadline)?;
        }
        if surface_delimiter(ch) {
            if let Some((start_cp, start_byte)) = token_start.take() {
                flush(start_cp, start_byte, cp_count, byte, &mut terms)?;
            }
        } else if token_start.is_none() {
            token_start = Some((cp_count, byte));
        }
        cp_count += 1;
    }
    if let Some((start_cp, start_byte)) = token_start {
        flush(start_cp, start_byte, cp_count, text.len(), &mut terms)?;
    }
    Ok(terms)
}

/// Build a deterministic snapshot directory. `output` must not already exist.
pub fn build(
    source_root: &Path,
    selection_path: &Path,
    output: &Path,
    limits: Limits,
) -> Result<Value, String> {
    if !source_root.is_absolute()
        || selection_path.is_absolute()
        || !output.is_absolute()
        || limits.max_input_bytes == 0
        || limits.max_output_bytes == 0
        || limits.max_seconds == 0
        || limits.max_seconds > 600
    {
        return Err("absolute paths and finite positive limits are required".into());
    }
    if output.exists() {
        return Err("output directory must be fresh".into());
    }
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(limits.max_seconds))
        .ok_or("deadline overflow")?;
    let selection_abs = source_path(
        source_root,
        selection_path
            .to_str()
            .ok_or("selection path is not UTF-8")?,
    )?;
    let selection_meta = fs::symlink_metadata(&selection_abs).map_err(|e| e.to_string())?;
    if selection_meta.file_type().is_symlink()
        || !selection_meta.is_file()
        || selection_meta.len() > limits.max_input_bytes as u64
    {
        return Err("selection invalid or oversized".into());
    }
    let mut selection_file =
        crate::safe_open::open_regular(&selection_abs, limits.max_input_bytes as u64)
            .map_err(|e| e.to_string())?;
    let mut selection_raw = Vec::new();
    selection_file
        .read_to_end(&mut selection_raw)
        .map_err(|e| e.to_string())?;
    let selection: Value = serde_json::from_slice(&selection_raw).map_err(|e| e.to_string())?;
    if s(&selection, "schema_version")? != "tos_public_corpus_selection_v1" {
        return Err("unsupported selection schema".into());
    }
    let mut profiles = selection
        .get("profiles")
        .cloned()
        .ok_or("profiles absent")?;
    let docs = arr(&selection, "documents")?;
    if docs.is_empty() || docs.len() > 1000 {
        return Err("document count out of bounds".into());
    }
    let mut doc_ids = HashSet::new();
    for doc in docs {
        if !doc_ids.insert(s(doc, "id")?.to_owned()) {
            return Err("duplicate document id".into());
        }
        let _ = s(doc, "title")?;
        let _ = s(doc, "author")?;
        if arr(doc, "versions")?.is_empty() {
            return Err("document has no versions".into());
        }
    }
    for profile_name in ["zarathustra", "antiquity"] {
        let profile = &profiles[profile_name];
        let _ = s(profile, "label")?;
        let ids = arr(profile, "documentIds")?;
        let initial = s(profile, "initialDocumentId")?;
        if ids.is_empty()
            || !ids.iter().any(|id| id.as_str() == Some(initial))
            || ids
                .iter()
                .any(|id| id.as_str().is_none_or(|id| !doc_ids.contains(id)))
        {
            return Err(format!(
                "profile {profile_name} does not select existing documents"
            ));
        }
    }
    let mut input_spent = selection_raw.len();
    if input_spent > limits.max_input_bytes {
        return Err("input byte limit exceeded".into());
    }
    let mut output_spent = 0usize;
    let mut total_units = 0usize;
    let mut out_docs = Vec::new();
    let mut staged = Vec::<(PathBuf, Vec<u8>)>::new();
    for doc in docs {
        check_deadline(deadline)?;
        let mut versions = Vec::new();
        let mut version_ids = HashSet::new();
        for version in arr(doc, "versions")? {
            check_deadline(deadline)?;
            if versions.len() >= MAX_VERSIONS {
                return Err("version count exceeds fixed limit".into());
            }
            if !version_ids.insert(s(version, "id")?.to_owned()) {
                return Err("duplicate version id".into());
            }
            for field in [
                "id",
                "label",
                "language",
                "role",
                "sourceUrl",
                "attribution",
            ] {
                let _ = s(version, field)?;
            }
            let _ = s(&version["license"], "label")?;
            let _ = s(&version["license"], "url")?;
            let packet_ref = s(version, "packetRef")?;
            let packet_raw = read_ref(
                source_root,
                packet_ref,
                s(version, "packetSha256")?,
                limits.max_input_bytes,
                &mut input_spent,
            )?;
            let packet: Value = serde_json::from_slice(&packet_raw).map_err(|e| e.to_string())?;
            let rights_ref = s(version, "rightsRef")?;
            let rights_raw = read_ref(
                source_root,
                rights_ref,
                s(version, "rightsSha256")?,
                limits.max_input_bytes,
                &mut input_spent,
            )?;
            let rights: Value = serde_json::from_slice(&rights_raw).map_err(|e| e.to_string())?;
            public_rights(&rights, &packet, version)?;
            let seg_ref = s(version, "segmentationRef")?;
            let segmentation = packet["segmentations"]
                .as_array()
                .ok_or("segmentations absent")?
                .iter()
                .find(|x| x["segmentation_id"] == seg_ref)
                .ok_or("selected segmentation absent")?;
            let source_ref = s(&packet["source_layer"], "text_layer_ref")?;
            let source_layer_raw = read_ref(
                source_root,
                source_ref,
                s(&packet["source_layer"], "text_layer_sha256")?,
                limits.max_input_bytes,
                &mut input_spent,
            )?;
            // Text layer refs name the source-returnable text payload in this carrier profile.
            let text =
                String::from_utf8(source_layer_raw).map_err(|_| "source text is not UTF-8")?;
            let byte_offsets = char_byte_map(&text, deadline)?;
            let all_units = arr(&packet, "units")?;
            if all_units.len() > MAX_UNITS_PER_VERSION {
                return Err("packet unit count exceeds fixed limit".into());
            }
            let mut units_by_id = HashMap::<&str, &Value>::with_capacity(all_units.len());
            for unit in all_units {
                check_deadline(deadline)?;
                let id = s(unit, "unit_id")?;
                if units_by_id.insert(id, unit).is_some() {
                    return Err("duplicate source unit id".into());
                }
            }
            let mut anchors = HashMap::<&str, &Value>::new();
            for anchor in arr(&packet, "anchors")? {
                check_deadline(deadline)?;
                let id = s(anchor, "anchor_ref")?;
                if anchors.insert(id, anchor).is_some() {
                    return Err("duplicate source anchor id".into());
                }
            }
            let ordered_refs = segmentation["ordered_unit_refs"]
                .as_array()
                .ok_or("ordered unit refs absent")?;
            let mut position_by_ref = HashMap::<&str, usize>::with_capacity(ordered_refs.len());
            for (position, unit_ref) in ordered_refs.iter().enumerate() {
                let id = unit_ref.as_str().ok_or("segmentation unit ref malformed")?;
                if position_by_ref.insert(id, position).is_some() {
                    return Err("duplicate unit ref in segmentation".into());
                }
            }
            let selected_refs: Vec<&Value> = if let Some(unit_refs) = version.get("unitRefs") {
                let refs = unit_refs.as_array().ok_or("unitRefs must be an array")?;
                let mut cursor = 0usize;
                for r in refs {
                    let id = r.as_str().ok_or("unit ref malformed")?;
                    let Some(&pos) = position_by_ref.get(id) else {
                        return Err(
                            "unitRefs must be an ordered subset of the selected segmentation"
                                .into(),
                        );
                    };
                    if pos < cursor {
                        return Err(
                            "unitRefs must be an ordered subset of the selected segmentation"
                                .into(),
                        );
                    }
                    cursor = pos + 1;
                }
                refs.iter()
                    .map(|r| {
                        units_by_id
                            .get(r.as_str().ok_or("unit ref malformed")?)
                            .copied()
                            .ok_or_else(|| "selected unit absent".to_owned())
                    })
                    .collect::<Result<_, String>>()?
            } else {
                let refs = ordered_refs;
                refs.iter()
                    .map(|r| {
                        units_by_id
                            .get(r.as_str().ok_or("unit ref malformed")?)
                            .copied()
                            .ok_or_else(|| "segmented unit absent".to_owned())
                    })
                    .collect::<Result<_, String>>()?
            };
            if selected_refs.is_empty() {
                return Err("selected segmentation has no units".into());
            }
            if selected_refs.len() > MAX_UNITS_PER_VERSION {
                return Err("unit count exceeds fixed per-version limit".into());
            }
            total_units = total_units
                .checked_add(selected_refs.len())
                .ok_or("unit count overflow")?;
            if total_units > MAX_TOTAL_UNITS {
                return Err("unit count exceeds fixed total limit".into());
            }
            let structure = version.get("structure").cloned().unwrap_or(json!([]));
            let mut labels_by_unit = HashMap::<&str, &str>::new();
            if let Some(rows) = structure.as_array() {
                for row in rows {
                    if let (Some(id), Some(label)) = (row["unitId"].as_str(), row["label"].as_str())
                    {
                        labels_by_unit.entry(id).or_insert(label);
                    }
                }
            }
            let language = s(version, "language")?;
            let mut seen = HashSet::new();
            let mut units = Vec::new();
            let mut index_terms = BTreeMap::<String, Vec<Value>>::new();
            let mut index_bytes = 0usize;
            let mut index_overflow = false;
            for (i, unit) in selected_refs.iter().enumerate() {
                if i & 0x3ff == 0 {
                    check_deadline(deadline)?;
                }
                let id = s(unit, "unit_id")?.to_owned();
                if !seen.insert(id.clone()) {
                    return Err("duplicate selected unit".into());
                }
                let (start, end) = anchor_slice(unit, &anchors, source_ref, &text, &byte_offsets)?;
                let body = text
                    .get(byte_offsets[start]..byte_offsets[end])
                    .ok_or("source unit byte interval invalid")?
                    .to_owned();
                let n = body.len();
                if n > 65_536 {
                    return Err("unit exceeds 65536 UTF-8 bytes".into());
                }
                let reference = labels_by_unit
                    .get(id.as_str())
                    .copied()
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("Unit {}", i + 1));
                if !index_overflow {
                    let unit_terms = search_terms(
                        &body,
                        &text,
                        &byte_offsets,
                        start,
                        &reference,
                        &id,
                        i + 1,
                        deadline,
                    )?;
                    for (form, mut occurrences) in unit_terms {
                        index_bytes = index_bytes
                            .checked_add(
                                form.len()
                                    + serde_json::to_vec(&occurrences)
                                        .map_err(|e| e.to_string())?
                                        .len()
                                    + 40,
                            )
                            .ok_or("index size overflow")?;
                        if index_bytes > MAX_SEARCH_INDEX_BYTES {
                            index_overflow = true;
                            index_terms.clear();
                            break;
                        }
                        index_terms
                            .entry(form)
                            .or_default()
                            .append(&mut occurrences);
                    }
                }
                units.push(json!({"id":id,"ordinal":i+1,"kind":s(unit,"unit_kind")?,"language":language,"reference":reference,"text":body}));
            }
            let revision = s(&packet["source_layer"], "text_layer_sha256")?.to_owned();
            let version_id = s(version, "id")?.to_owned();
            let mut pages = Vec::new();
            let mut unit_summaries = Vec::<Value>::new();
            let mut page_units = Vec::<Value>::new();
            let mut first = 1usize;
            let mut bytes = 0usize;
            for unit in units {
                check_deadline(deadline)?;
                let add = serde_json::to_vec(&unit).map_err(|e| e.to_string())?.len() + 1;
                if !page_units.is_empty() && (page_units.len() >= 40 || bytes + add > 262_144) {
                    flush_page(
                        output,
                        s(doc, "id")?,
                        &version_id,
                        &revision,
                        first,
                        &mut page_units,
                        &mut pages,
                        &mut unit_summaries,
                        &mut staged,
                        &mut output_spent,
                        limits.max_output_bytes,
                    )?;
                    first = unit_summaries.len() + 1;
                    bytes = 0;
                }
                bytes += add;
                page_units.push(unit);
            }
            if !page_units.is_empty() {
                flush_page(
                    output,
                    s(doc, "id")?,
                    &version_id,
                    &revision,
                    first,
                    &mut page_units,
                    &mut pages,
                    &mut unit_summaries,
                    &mut staged,
                    &mut output_spent,
                    limits.max_output_bytes,
                )?;
            }
            let scope = &packet["source_scope"];
            for field in [
                "work_ref",
                "expression_ref",
                "edition_ref",
                "item_ref",
                "file_ref",
            ] {
                let _ = s(scope, field)?;
            }
            if scope["work_ref"] != doc["id"] {
                return Err("document identity does not match source work".into());
            }
            let index_value = json!({"terms":index_terms.into_iter().map(|(form,occurrences)|json!({"form":form,"occurrences":occurrences})).collect::<Vec<_>>()});
            let index_raw = if index_overflow {
                Vec::new()
            } else {
                serde_json::to_vec(&index_value).map_err(|e| e.to_string())?
            };
            let search_index = if !index_overflow
                && index_raw.len() <= MAX_SEARCH_INDEX_BYTES
                && output_spent
                    .checked_add(index_raw.len())
                    .is_some_and(|n| n <= limits.max_output_bytes)
            {
                let sha = digest(&index_raw);
                let path = format!("indexes/{sha}.json");
                output_spent += index_raw.len();
                staged.push((output.join(&path), index_raw.clone()));
                Some(json!({"path":path,"sha256":sha,"bytes":index_raw.len()}))
            } else {
                None
            };
            let analysis =
                if let Some(analysis_ref) = version.get("analysisRef").and_then(Value::as_str) {
                    let raw = read_ref(
                        source_root,
                        analysis_ref,
                        s(version, "analysisSha256")?,
                        limits.max_input_bytes,
                        &mut input_spent,
                    )?;
                    if raw.len() > 8 * 1024 * 1024 {
                        return Err("analysis carrier exceeds 8 MiB".into());
                    }
                    let value: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
                    if value["schema_version"] != "tos_public_morphology_candidates_v1"
                        || value["textLayerSha256"] != revision
                        || value["language"] != language
                        || !value["forms"].is_object()
                        || !value["source"].is_object()
                    {
                        return Err(
                            "analysis carrier does not match selected public source layer".into(),
                        );
                    }
                    let sha = digest(&raw);
                    let path = format!("analyses/{sha}.json");
                    output_spent = output_spent
                        .checked_add(raw.len())
                        .ok_or("output size overflow")?;
                    if output_spent > limits.max_output_bytes {
                        return Err("output byte limit exceeded".into());
                    }
                    let descriptor = json!({"path":path,"sha256":sha,"bytes":raw.len()});
                    staged.push((output.join(&path), raw));
                    Some(descriptor)
                } else {
                    None
                };
            versions.push(json!({"id":version_id,"label":version["label"],"language":version["language"],"role":version["role"],"revision":revision,"unitCount":selected_refs.len(),"source":{"workId":scope["work_ref"],"expressionId":scope["expression_ref"],"editionId":scope["edition_ref"],"itemId":scope["item_ref"],"fileId":scope["file_ref"],"fileSha256":scope["file_sha256"],"textLayerRef":source_ref,"textLayerSha256":revision},"sourceUrl":version["sourceUrl"],"attribution":version["attribution"],"license":version["license"],"pages":pages,"units":unit_summaries,"structure":structure,"searchIndex":search_index,"analysis":analysis}));
        }
        out_docs.push(
            json!({"id":doc["id"],"title":doc["title"],"author":doc["author"],"versions":versions}),
        );
    }
    // Authored routes stay with their ToS owner; delivery verifies only bounds
    // and exact targets, never the proposed interpretation.
    for profile_name in ["zarathustra", "antiquity"] {
        check_deadline(deadline)?;
        let profile = &mut profiles[profile_name];
        if let Some(reference) = profile
            .get("journeysRef")
            .and_then(Value::as_str)
            .map(str::to_owned)
        {
            let sha = s(profile, "journeysSha256")?.to_owned();
            let raw = read_ref(
                source_root,
                &reference,
                &sha,
                limits.max_input_bytes,
                &mut input_spent,
            )?;
            if raw.len() > 262_144 {
                return Err("journeys carrier exceeds 262144 bytes".into());
            }
            let carrier: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
            if carrier["schema_version"] != "tos_public_research_journeys_v1"
                || carrier["profile"] != profile_name
                || carrier["posture"] != "proposed-reading-questions"
            {
                return Err("journeys carrier does not match selected profile and posture".into());
            }
            profile["journeys"] = Value::Array(arr(&carrier, "journeys")?.to_vec());
            profile["journeysSource"] =
                json!({"ref":reference,"sha256":sha,"posture":carrier["posture"]});
        }
        if let Some(items) = profile.get("journeys") {
            let items = items.as_array().ok_or("journeys must be an array")?;
            if items.is_empty() || items.len() > 20 {
                return Err("journey count outside bounds".into());
            }
            let allowed = arr(profile, "documentIds")?;
            let mut ids = HashSet::new();
            for item in items {
                if !ids.insert(s(item, "id")?) {
                    return Err("duplicate journey id".into());
                }
                for field in ["id", "title", "question"] {
                    if s(item, field)?.is_empty() || s(item, field)?.len() > 8192 {
                        return Err("journey text outside bounds".into());
                    }
                }
                validate_journey_address(item, allowed, &out_docs)?;
                if let Some(steps) = item.get("steps") {
                    for field in ["readingHypothesis", "strongAlternative"] {
                        if s(item, field)?.is_empty() || s(item, field)?.len() > 16_384 {
                            return Err("journey assessment outside bounds".into());
                        }
                    }
                    let steps = steps.as_array().ok_or("journey steps must be an array")?;
                    if steps.len() < 3 || steps.len() > 20 {
                        return Err("journey step count outside bounds".into());
                    }
                    let mut step_ids = HashSet::new();
                    for step in steps {
                        if !step_ids.insert(s(step, "id")?) {
                            return Err("duplicate journey step id".into());
                        }
                        for field in ["id", "title", "voice", "prompt"] {
                            if s(step, field)?.is_empty() || s(step, field)?.len() > 16_384 {
                                return Err("journey step text outside bounds".into());
                            }
                        }
                        validate_journey_address(step, allowed, &out_docs)?;
                    }
                }
            }
        }
    }
    // Materialize doc manifests and top manifest only after all source/rights checks pass.
    let mut top_docs = Vec::new();
    for d in &out_docs {
        let raw = serde_json::to_vec(d).map_err(|e| e.to_string())?;
        let sha = digest(&raw);
        let path = format!("documents/{sha}.json");
        output_spent = output_spent
            .checked_add(raw.len())
            .ok_or("output size overflow")?;
        if output_spent > limits.max_output_bytes {
            return Err("output byte limit exceeded".into());
        }
        staged.push((output.join(&path), raw.clone()));
        top_docs.push(json!({"id":d["id"],"title":d["title"],"author":d["author"],"manifest":{"path":path,"sha256":sha,"bytes":raw.len()}}));
    }
    let base = json!({"schema_version":"tos_public_corpus_snapshot_v1","profiles":profiles,"documents":top_docs});
    let revision = digest(&serde_json::to_vec(&base).map_err(|e| e.to_string())?);
    let manifest = json!({"schema_version":"tos_public_corpus_snapshot_v1","revision":revision,"profiles":profiles,"documents":top_docs});
    let raw = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    output_spent = output_spent
        .checked_add(raw.len())
        .ok_or("output size overflow")?;
    if output_spent > limits.max_output_bytes {
        return Err("output byte limit exceeded".into());
    }
    staged.push((output.join("manifest.json"), raw));
    fs::create_dir(output).map_err(|e| format!("cannot create fresh output directory: {e}"))?;
    let write_result = (|| -> Result<(), String> {
        for (path, bytes) in staged {
            check_deadline(deadline)?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::write(path, bytes).map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_dir_all(output);
        return Err(error);
    }
    Ok(manifest)
}

fn validate_journey_address(
    target: &Value,
    allowed: &[Value],
    docs: &[Value],
) -> Result<(), String> {
    let document_id = s(target, "documentId")?;
    if !allowed.iter().any(|id| id.as_str() == Some(document_id)) {
        return Err("journey target outside profile".into());
    }
    let doc = docs
        .iter()
        .find(|d| d["id"] == document_id)
        .ok_or("journey document absent")?;
    let version_id = s(target, "versionId")?;
    let version = arr(doc, "versions")?
        .iter()
        .find(|v| v["id"] == version_id)
        .ok_or("journey version absent")?;
    let unit_id = s(target, "unitId")?;
    if !arr(version, "units")?.iter().any(|u| u["id"] == unit_id) {
        return Err("journey unit absent in selected version".into());
    }
    Ok(())
}

fn flush_page(
    output: &Path,
    doc_id: &str,
    version_id: &str,
    revision: &str,
    first: usize,
    units: &mut Vec<Value>,
    pages: &mut Vec<Value>,
    summaries: &mut Vec<Value>,
    staged: &mut Vec<(PathBuf, Vec<u8>)>,
    spent: &mut usize,
    cap: usize,
) -> Result<(), String> {
    let body = json!({"documentId":doc_id,"versionId":version_id,"revision":revision,"units":std::mem::take(units)});
    let raw = serde_json::to_vec(&body).map_err(|e| e.to_string())?;
    if raw.len() > 262_144 {
        return Err("page exceeds 262144 bytes".into());
    }
    *spent = spent.checked_add(raw.len()).ok_or("output size overflow")?;
    if *spent > cap {
        return Err("output byte limit exceeded".into());
    }
    let sha = digest(&raw);
    let path = format!("pages/{sha}.json");
    let count = body["units"].as_array().map_or(0, Vec::len);
    let page_bytes = raw.len();
    let page_index = pages.len();
    if let Some(rows) = body["units"].as_array() {
        for row in rows {
            summaries.push(json!({"id":row["id"],"ordinal":row["ordinal"],"page":page_index}));
        }
    }
    staged.push((output.join(&path), raw));
    pages.push(
        json!({"path":path,"sha256":sha,"bytes":page_bytes,"firstOrdinal":first,"count":count}),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(root: &Path, public: bool) -> (PathBuf, PathBuf) {
        let text = b"Alpha beta";
        fs::write(root.join("source.txt"), text).unwrap();
        let packet_path = "packet.json";
        let rights_path = "rights.json";
        let packet = json!({
            "schema_version":"tos_source_text_unit_packet_v1","packet_id":"packet","packet_version":1,
            "rights_and_visibility":{"source_visibility":if public{"public"}else{"local_only"},"packet_visibility":if public{"public"}else{"local_only"},"effective_visibility":if public{"public"}else{"local_only"},"private_source_used":false,"publication_authorized":public,"rights_record_refs":[rights_path]},
            "source_layer":{"visibility":if public{"public"}else{"local_only"},"publication_authorized":public,"text_layer_ref":"source.txt","text_layer_sha256":digest(text)},
            "source_scope":{"work_ref":"d","expression_ref":"expression","edition_ref":"edition","item_ref":"item","file_ref":format!("tos.file.sha256.{}",digest(text)),"file_sha256":digest(text)},
            "anchors":[{"anchor_ref":"a1","anchor_role":"content","text_layer_ref":"source.txt","exact_sha256":digest(b"Alpha"),"selector":{"type":"text_position","position_unit":"unicode_code_point","interval":"half_open","start":0,"end":5}},{"anchor_ref":"a2","anchor_role":"content","text_layer_ref":"source.txt","exact_sha256":digest(b"beta"),"selector":{"type":"text_position","position_unit":"unicode_code_point","interval":"half_open","start":6,"end":10}}],
            "units":[{"unit_id":"u1","unit_kind":"surface_token","ordered_anchor_refs":["a1"]},{"unit_id":"u2","unit_kind":"surface_token","ordered_anchor_refs":["a2"]}],
            "segmentations":[{"segmentation_id":"seg","ordered_unit_refs":["u1","u2"]}]
        });
        let rights = json!({"visibility":if public{"public_payload"}else{"local_only"},"assessment_status":if public{"licensed"}else{"not_assessed"},"redistribution_posture":if public{"authorized"}else{"not_authorized"},"jurisdictions_reviewed":["US"],"scope_refs":["item",format!("tos.file.sha256.{}",digest(text))],"license_uri":"https://creativecommons.org/licenses/by-sa/4.0/","permissions":["Attribution: Test source"]});
        let packet_raw = serde_json::to_vec(&packet).unwrap();
        let rights_raw = serde_json::to_vec(&rights).unwrap();
        fs::write(root.join(packet_path), &packet_raw).unwrap();
        fs::write(root.join(rights_path), &rights_raw).unwrap();
        let selection = json!({"schema_version":"tos_public_corpus_selection_v1","profiles":{"zarathustra":{"label":"Z","documentIds":["d"],"initialDocumentId":"d"},"antiquity":{"label":"A","documentIds":["d"],"initialDocumentId":"d"}},"documents":[{"id":"d","title":"Test","author":"Author","versions":[{"id":"v","label":"Version","language":"en","role":"source","packetRef":packet_path,"packetSha256":digest(&packet_raw),"segmentationRef":"seg","rightsRef":rights_path,"rightsSha256":digest(&rights_raw),"sourceUrl":"https://example.test/text","attribution":"Test source","license":{"label":"CC BY-SA 4.0","url":"https://creativecommons.org/licenses/by-sa/4.0/"}}]}]});
        let selection_raw = serde_json::to_vec(&selection).unwrap();
        fs::write(root.join("selection.json"), selection_raw).unwrap();
        (root.join("selection.json"), root.join("output"))
    }
    #[test]
    fn refuses_a_journey_to_an_absent_unit_before_publishing() {
        let root = tempfile::tempdir().unwrap();
        let (_, output) = fixture(root.path(), true);
        let path = root.path().join("selection.json");
        let mut selection: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        selection["profiles"]["zarathustra"]["journeys"] = json!([{"id":"j","title":"Question","question":"Read?","documentId":"d","versionId":"v","unitId":"missing"}]);
        fs::write(path, serde_json::to_vec(&selection).unwrap()).unwrap();
        let error = build(
            root.path(),
            Path::new("selection.json"),
            &output,
            Limits {
                max_input_bytes: 100_000,
                max_output_bytes: 100_000,
                max_seconds: 5,
            },
        )
        .unwrap_err();
        assert_eq!(error, "journey unit absent in selected version");
        assert!(!output.exists());
    }
    #[test]
    fn refuses_public_material_without_a_reviewed_jurisdiction() {
        let root = tempfile::tempdir().unwrap();
        let (_, output) = fixture(root.path(), true);
        let rights_path = root.path().join("rights.json");
        let mut rights: Value = serde_json::from_slice(&fs::read(&rights_path).unwrap()).unwrap();
        rights["jurisdictions_reviewed"] = json!([]);
        let raw = serde_json::to_vec(&rights).unwrap();
        fs::write(&rights_path, &raw).unwrap();
        let path = root.path().join("selection.json");
        let mut selection: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        selection["documents"][0]["versions"][0]["rightsSha256"] = json!(digest(&raw));
        fs::write(path, serde_json::to_vec(&selection).unwrap()).unwrap();
        let error = build(
            root.path(),
            Path::new("selection.json"),
            &output,
            Limits {
                max_input_bytes: 100_000,
                max_output_bytes: 100_000,
                max_seconds: 5,
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            "public rights require an explicit reviewed jurisdiction"
        );
        assert!(!output.exists());
    }
    #[test]
    fn delivers_exact_public_units_and_refuses_local_only_before_output() {
        let public = tempfile::tempdir().unwrap();
        let (selection, output) = fixture(public.path(), true);
        let manifest = build(
            public.path(),
            Path::new("selection.json"),
            &output,
            Limits {
                max_input_bytes: 100_000,
                max_output_bytes: 100_000,
                max_seconds: 5,
            },
        )
        .unwrap();
        assert_eq!(manifest["schema_version"], "tos_public_corpus_snapshot_v1");
        let doc_path = manifest["documents"][0]["manifest"]["path"]
            .as_str()
            .unwrap();
        let doc: Value = serde_json::from_slice(&fs::read(output.join(doc_path)).unwrap()).unwrap();
        assert_eq!(doc["versions"][0]["unitCount"], 2);
        assert_eq!(doc["versions"][0]["units"][0]["page"], 0);
        assert_eq!(doc["versions"][0]["units"][1]["page"], 0);
        let index_path = doc["versions"][0]["searchIndex"]["path"].as_str().unwrap();
        let index_raw = fs::read(output.join(index_path)).unwrap();
        assert_eq!(
            digest(&index_raw),
            doc["versions"][0]["searchIndex"]["sha256"]
        );
        let index: Value = serde_json::from_slice(&index_raw).unwrap();
        assert!(
            index["terms"]
                .as_array()
                .unwrap()
                .iter()
                .any(|term| term["form"] == "Alpha"
                    && term["occurrences"][0][0] == 1
                    && term["occurrences"][0][1] == 0)
        );
        let page_path = doc["versions"][0]["pages"][0]["path"].as_str().unwrap();
        let page: Value =
            serde_json::from_slice(&fs::read(output.join(page_path)).unwrap()).unwrap();
        assert_eq!(page["units"][0]["text"], "Alpha");
        assert_eq!(page["units"][1]["text"], "beta");
        drop(selection);

        let private = tempfile::tempdir().unwrap();
        let (selection, _) = fixture(private.path(), false);
        let rejected = private.path().join("refused");
        assert!(
            build(
                private.path(),
                Path::new("selection.json"),
                &rejected,
                Limits {
                    max_input_bytes: 100_000,
                    max_output_bytes: 100_000,
                    max_seconds: 5
                }
            )
            .is_err()
        );
        assert!(!rejected.exists());
        drop(selection);
    }
}
