//! Text-free intersection of two independently materialized numbered-label maps.
//! Historical checks reconstruct exact receipts without claiming their execution.
use crate::{
    constructor_library::Out,
    research_execution::ResearchExecution,
    source_text_foundation::{ensure, fresh_or_matching, s, schema, sha},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
type Result<T> = std::result::Result<T, String>;
const CAP: u64 = 2 * 1024 * 1024;
fn q(v: impl Into<String>) -> Out {
    Out::String(v.into())
}
fn n(v: u64) -> Out {
    Out::Integer(v)
}
fn o(v: Vec<(&str, Out)>) -> Out {
    Out::Object(v.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
fn set(value: &mut Out, key: &str, replacement: Out) -> Result<()> {
    if let Out::Object(fields) = value {
        fields
            .iter_mut()
            .find(|(k, _)| k == key)
            .ok_or("absent ordered field")?
            .1 = replacement;
        Ok(())
    } else {
        Err("ordered object required".into())
    }
}
fn field<'a>(value: &'a mut Out, key: &str) -> Result<&'a mut Out> {
    if let Out::Object(fields) = value {
        Ok(&mut fields
            .iter_mut()
            .find(|(k, _)| k == key)
            .ok_or("absent ordered field")?
            .1)
    } else {
        Err("ordered object required".into())
    }
}
fn render(value: &Out, pretty: bool) -> Result<Vec<u8>> {
    let mut raw = if pretty {
        serde_json::to_vec_pretty(value)
    } else {
        serde_json::to_vec(value)
    }
    .map_err(|e| e.to_string())?;
    raw.push(b'\n');
    ensure(raw.len() <= CAP as usize, "label output byte limit")?;
    Ok(raw)
}
fn witness(value: &Value, reference: &str, digest: &str) -> Result<Out> {
    Ok(o(vec![
        ("expression_ref", q(s(&value["expression_ref"])?)),
        ("edition_ref", q(s(&value["edition_ref"])?)),
        ("item_ref", q(s(&value["item_ref"])?)),
        ("file_ref", q(s(&value["scan_file"]["file_ref"])?)),
        ("file_sha256", q(s(&value["scan_file"]["file_sha256"])?)),
        ("numbered_unit_map_ref", q(reference)),
        ("numbered_unit_map_sha256", q(digest)),
    ]))
}
fn units(value: &Value) -> Result<(Vec<&str>, BTreeMap<&str, &Value>)> {
    let rows = value["unit_starts"]
        .as_array()
        .ok_or("unit starts absent")?;
    ensure(rows.len() <= 299, "numbered-label count bound")?;
    let mut order = Vec::new();
    let mut map = BTreeMap::new();
    for row in rows {
        let key = s(&row["unit_key"])?;
        ensure(!key.is_empty() && key.len() <= 32, "numbered label syntax")?;
        ensure(
            map.insert(key, row).is_none(),
            "duplicate numbered-unit key",
        )?;
        order.push(key);
        let _ = s(&row["anchor_ref"])?;
        ensure(
            row["pdf_page"].as_u64().is_some_and(|p| p > 0),
            "numbered unit PDF page",
        )?;
    }
    Ok((order, map))
}
fn pairings(source: &Value, target: &Value) -> Result<Vec<Out>> {
    ensure(
        s(&source["work_ref"])? == s(&target["work_ref"])?
            && s(&source["work_ref"])? == "tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese",
        "source and target work refs differ from selected work",
    )?;
    let (_, src) = units(source)?;
    let (order, dst) = units(target)?;
    let source_only = src
        .keys()
        .copied()
        .filter(|k| !dst.contains_key(k))
        .collect::<BTreeSet<_>>();
    ensure(
        src.len() == 299
            && dst.len() == 298
            && source_only == BTreeSet::from(["237a"])
            && dst.keys().all(|k| src.contains_key(k)),
        "numbered-unit intersection drifted",
    )?;
    order
        .iter()
        .enumerate()
        .map(|(i, key)| {
            let left = src[key];
            let right = dst[key];
            Ok(o(vec![
                ("sequence", n((i + 1) as u64)),
                ("unit_key", q(*key)),
                ("source_anchor_ref", q(s(&left["anchor_ref"])?)),
                ("source_pdf_page", n(left["pdf_page"].as_u64().unwrap())),
                ("target_anchor_ref", q(s(&right["anchor_ref"])?)),
                ("target_pdf_page", n(right["pdf_page"].as_u64().unwrap())),
                ("basis", q("shared_materialized_number_label_key")),
                ("status", q("proposed")),
                ("human_review_performed", Out::Bool(false)),
                ("translation_alignment_claimed", Out::Bool(false)),
            ]))
        })
        .collect()
}
pub struct Selection<'a> {
    pub directory: &'a str,
    pub map_id: &'a str,
    pub event_id: &'a str,
    pub event_at: &'a str,
}
pub fn run(ctx: &ResearchExecution, build: bool, fresh: Option<Selection<'_>>) -> Result<Value> {
    ensure(
        !build || fresh.is_some(),
        "build requires a new map, event, timestamp and output directory",
    )?;
    let mut held = Vec::new();
    let mut values = Vec::new();
    let mut hashes = Vec::new();
    for reference in [SOURCE_MAP, TARGET_MAP, SOURCE_RIGHTS, TARGET_RIGHTS] {
        let mut file = ctx.source_file(reference, CAP)?;
        let raw = ctx.read_file(&mut file, CAP)?;
        let value: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        ensure(value.is_object(), "input JSON object required")?;
        hashes.push(sha(&raw));
        values.push(value);
        held.push((reference, file));
    }
    let digests: [String; 4] = hashes.try_into().map_err(|_| "input count")?;
    let pairs = pairings(&values[0], &values[1])?;
    // The frozen event records its actual timestamp, which may differ from
    // the former command's default. Read its declaration without recreating
    // or asserting that historical execution.
    let historical_at = if fresh.is_none() {
        let mut file = ctx.source_file(PROVENANCE_PATH, CAP)?;
        let event: Value =
            serde_json::from_slice(&ctx.read_file(&mut file, CAP)?).map_err(|e| e.to_string())?;
        ensure(
            s(&event["event_id"])? == EVENT_ID,
            "historical event identity differs",
        )?;
        s(&event["started_at"])?.to_owned()
    } else {
        String::new()
    };
    let (map_ref, provenance_ref, map_id, event_id, event_at) = if let Some(selected) = &fresh {
        ensure(
            selected.directory.starts_with(ALIGNMENT_DIR)
                && selected.directory.len() > ALIGNMENT_DIR.len() + 1
                && selected.directory.as_bytes()[ALIGNMENT_DIR.len()] == b'/'
                && std::path::Path::new(selected.directory)
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))),
            "select a new source-owned alignment subdirectory",
        )?;
        ensure(
            selected.map_id != MAP_ID && selected.event_id != EVENT_ID,
            "preserve historical map/event identity",
        )?;
        (
            format!(
                "{}/numbered-unit-label-correspondence.json",
                selected.directory
            ),
            format!(
                "{}/provenance.numbered-unit-label-correspondence.jsonl",
                selected.directory
            ),
            selected.map_id,
            selected.event_id,
            selected.event_at,
        )
    } else {
        (
            MAP_PATH.into(),
            PROVENANCE_PATH.into(),
            MAP_ID,
            EVENT_ID,
            historical_at.as_str(),
        )
    };
    let mut map = map_output(
        &values[0],
        &values[1],
        pairs,
        &digests,
        map_id,
        event_id,
        &provenance_ref,
    )?;
    if fresh.is_some() {
        set(&mut map, "map_version", n(1))?;
        set(&mut map, "supersedes_map_ref", q(MAP_ID))?;
    }
    if fresh.is_none() {
        // Frozen v3 wording predates the current producer's prose update.
        set(
            &mut map,
            "authority_boundary",
            q(
                "mechanical pairing of identical structural number-label keys already materialized independently in two exact witness maps; no text comparison, exact passage boundary, source-to-target passage alignment, translation correspondence, equivalence or quality, semantics, rights clearance, or canon authority",
            ),
        )?;
    }
    let map_raw = render(&map, true)?;
    let mut event = event_output(&digests, &map_ref, &sha(&map_raw), event_id, event_at)?;
    if fresh.is_some() {
        set(
            &mut event,
            "agent_refs",
            Out::Array(vec![q("software:tos-rust")]),
        )?;
        let method = field(&mut event, "method")?;
        set(
            method,
            "artifact_digest",
            q(sha(include_bytes!("jenseits_label_correspondence.rs"))),
        )?;
        set(method, "runtime", q("Tree of Sophia native Rust"))?;
        set(
            method,
            "configuration",
            o(vec![
                ("pairing_key", q("unit_key")),
                ("expected_pairing_count", n(298)),
                ("source_only_unit_keys", Out::Array(vec![q("237a")])),
                ("local_payloads_read", Out::Bool(false)),
                ("source_to_target_text_compared", Out::Bool(false)),
                ("translation_alignment_inferred", Out::Bool(false)),
            ]),
        )?;
        if let Out::Array(warnings) = field(&mut event, "warnings")? {
            warnings.pop();
        }
        set(&mut event, "event_version", n(1))?;
        set(&mut event, "supersedes_event_ref", Out::Null)?;
    }
    if fresh.is_none() {
        if let Out::Array(warnings) = field(&mut event, "warnings")? {
            warnings[2] = q(
                "Shared numbering does not establish exact passage alignment, translation correspondence, equivalence, quality, or semantics.",
            );
            warnings[4] = q(
                "The superseding event refreshes the source rights-basis digest after a layered assessment; it does not rerun payload or text comparison and does not establish rights clearance.",
            );
        }
    }
    let event_raw = render(&event, false)?;
    schema(
        ctx,
        "ToS/contracts/parallel-numbered-unit-label-map.schema.json",
        &serde_json::from_slice(&map_raw).map_err(|e| e.to_string())?,
    )?;
    schema(
        ctx,
        "ToS/contracts/provenance-event.schema.json",
        &serde_json::from_slice(&event_raw).map_err(|e| e.to_string())?,
    )?;
    for ((reference, mut file), digest) in held.into_iter().zip(&digests) {
        ensure(
            ctx.hash_file(&mut file, CAP)? == *digest,
            "input changed during label generation",
        )?;
        let mut current = ctx.source_file(reference, CAP)?;
        ensure(
            ctx.hash_file(&mut current, CAP)? == *digest,
            "selected input path changed during label generation",
        )?;
    }
    let outputs = [(map_ref, map_raw), (provenance_ref, event_raw)];
    let mut writes = Vec::new();
    for (reference, raw) in &outputs {
        if build {
            writes.push(fresh_or_matching(ctx, reference, raw)?);
        } else {
            let mut file = ctx.source_file(reference, CAP)?;
            let retained = ctx.read_file(&mut file, CAP)?;
            ensure(
                retained == *raw,
                &format!(
                    "retained numbered-label output differs: {reference}; expected_sha256={}; retained_sha256={}",
                    sha(raw),
                    sha(&retained)
                ),
            )?;
        }
    }
    let mut changed = 0;
    if build {
        for ((reference, raw), write) in outputs.iter().zip(writes) {
            if write {
                ctx.write(reference, raw, 0o644, true)?;
                changed += 1;
            }
        }
    }
    Ok(
        json!({"status":"PASS","mode":if build{"build"}else{"check"},"pairings":298,"source_only_unit_keys":["237a"],
        "changed_files":changed,"native_kernel_sha256":sha(include_bytes!("jenseits_label_correspondence.rs")),
        "historical_bytes_reconstructed":fresh.is_none(),"historical_execution_claimed":false,
        "outputs":outputs.iter().map(|(r,b)|json!({"ref":r,"bytes":b.len(),"sha256":sha(b)})).collect::<Vec<_>>(),
        "local_payloads_read":false,"translation_alignment_claimed":false,"execution_budget":ctx.budget_report()}),
    )
}

const SOURCE_MAP: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/structure/numbered-unit-page-map.json";
const TARGET_MAP: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/ru-polilov-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/numbered-unit-page-map.json";
const SOURCE_RIGHTS: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/rights.json";
const TARGET_RIGHTS: &str = "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/rights.json";
const MAP_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/naumann-1886-polilov-mysl-1996/numbered-unit-label-correspondence.json";
const PROVENANCE_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/naumann-1886-polilov-mysl-1996/provenance.numbered-unit-label-correspondence.jsonl";
const MAP_ID: &str = "tos.parallel-numbered-unit-label-map.friedrich-nietzsche.jenseits-von-gut-und-boese.naumann-1886-to-polilov-mysl-1996";
const EVENT_ID: &str = "tos.event.parallel-numbered-unit-label-map.friedrich-nietzsche.jenseits-von-gut-und-boese.layered-rights-refresh.2026-08-02";
const ALIGNMENT_DIR: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/naumann-1886-polilov-mysl-1996";
fn map_output(
    source: &Value,
    target: &Value,
    pairings: Vec<Out>,
    digests: &[String; 4],
    map_id: &str,
    event_id: &str,
    provenance_ref: &str,
) -> Result<Out> {
    Ok(o(vec![
        (
            "$schema",
            q(
                "https://tree-of-sophia.local/ToS/contracts/parallel-numbered-unit-label-map.schema.json",
            ),
        ),
        (
            "schema_version",
            q("tos_parallel_numbered_unit_label_map_v1"),
        ),
        ("map_id", q(map_id)),
        ("work_ref", q(s(&source["work_ref"])?)),
        ("source_witness", witness(source, SOURCE_MAP, &digests[0])?),
        ("target_witness", witness(target, TARGET_MAP, &digests[1])?),
        (
            "rights_basis",
            Out::Array(vec![
                o(vec![
                    ("role", q("source")),
                    (
                        "ref",
                        q(
                            "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/rights.json",
                        ),
                    ),
                    ("sha256", q(&digests[2])),
                ]),
                o(vec![
                    ("role", q("target")),
                    (
                        "ref",
                        q(
                            "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/rights.json",
                        ),
                    ),
                    ("sha256", q(&digests[3])),
                ]),
            ]),
        ),
        (
            "map_authority",
            q("mechanical_shared_number_label_candidate_only"),
        ),
        ("source_text_included", Out::Bool(false)),
        (
            "method",
            o(vec![
                ("name", q("exact-shared-structural-label-key-intersection")),
                ("version", q("1")),
                ("maker_type", q("software")),
                ("local_payloads_read", Out::Bool(false)),
                ("pairing_key", q("unit_key")),
                ("requires_materialized_label_in_both_maps", Out::Bool(true)),
                ("source_to_target_text_compared", Out::Bool(false)),
                ("translation_alignment_inferred", Out::Bool(false)),
                ("semantic_matching_used", Out::Bool(false)),
                ("no_source_text_emitted", Out::Bool(true)),
            ]),
        ),
        ("pairings", Out::Array(pairings)),
        (
            "unpaired_units",
            Out::Array(vec![o(vec![
                ("side", q("source")),
                ("unit_key", q("237a")),
                (
                    "reason",
                    q("target_witness_has_corresponding_prose_without_repeated_number_label"),
                ),
                ("target_unit_materialized", Out::Bool(false)),
                ("translation_alignment_claimed", Out::Bool(false)),
                ("status", q("proposed")),
            ])]),
        ),
        (
            "summary",
            o(vec![
                ("source_numbered_unit_count", n(299)),
                ("target_numbered_unit_count", n(298)),
                ("shared_materialized_label_count", n(298)),
                ("pairing_count", n(298)),
                ("source_only_unit_keys", Out::Array(vec![q("237a")])),
                ("target_only_unit_keys", Out::Array(vec![])),
                ("all_pairing_statuses", Out::Array(vec![q("proposed")])),
                ("human_review_performed", Out::Bool(false)),
                ("translation_alignment_claimed", Out::Bool(false)),
            ]),
        ),
        ("provenance_ref", q(provenance_ref)),
        ("provenance_event_ref", q(event_id)),
        ("map_version", n(3)),
        ("supersedes_map_ref", Out::Null),
        (
            "authority_boundary",
            q(
                "This map pairs identical structural number-label keys from two independently prepared exact witness maps.",
            ),
        ),
        (
            "does_not_establish",
            Out::Array(vec![
                q("source_text"),
                q("target_text"),
                q("exact_line_boundaries"),
                q("exact_passage_end_boundaries"),
                q("source_to_target_passage_alignment"),
                q("translation_correspondence"),
                q("translation_equivalence"),
                q("translation_quality"),
                q("textual_identity"),
                q("semantics"),
                q("rights_clearance"),
                q("canon_promotion"),
            ]),
        ),
    ]))
}

fn event_output(
    digests: &[String; 4],
    map_ref: &str,
    map_digest: &str,
    event_id: &str,
    event_at: &str,
) -> Result<Out> {
    Ok(o(vec![
        ("schema_version", q("tos_provenance_event_v1")),
        ("event_id", q(event_id)),
        ("event_type", q("alignment")),
        ("started_at", q(event_at)),
        ("ended_at", q(event_at)),
        (
            "agent_refs",
            Out::Array(vec![q("software:python-standard-library")]),
        ),
        (
            "inputs",
            Out::Array(vec![
                o(vec![
                    (
                        "ref",
                        q(
                            "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/structure/numbered-unit-page-map.json",
                        ),
                    ),
                    ("role", q("tracked-source-numbered-label-map")),
                    ("sha256", q(&digests[0])),
                ]),
                o(vec![
                    (
                        "ref",
                        q(
                            "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/ru-polilov-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/numbered-unit-page-map.json",
                        ),
                    ),
                    ("role", q("tracked-target-numbered-label-map")),
                    ("sha256", q(&digests[1])),
                ]),
                o(vec![
                    (
                        "ref",
                        q(
                            "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/rights.json",
                        ),
                    ),
                    ("role", q("source-rights-basis")),
                    ("sha256", q(&digests[2])),
                ]),
                o(vec![
                    (
                        "ref",
                        q(
                            "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/rights.json",
                        ),
                    ),
                    ("role", q("target-rights-basis")),
                    ("sha256", q(&digests[3])),
                ]),
            ]),
        ),
        (
            "outputs",
            Out::Array(vec![o(vec![
                ("ref", q(map_ref)),
                (
                    "role",
                    q("tracked-text-free-shared-number-label-pairing-candidates"),
                ),
                ("sha256", q(map_digest)),
            ])]),
        ),
        (
            "method",
            o(vec![
                ("maker_type", q("software")),
                ("name", q("exact-shared-structural-label-key-intersection")),
                ("version", q("1")),
                ("artifact_digest", Out::Null),
                ("runtime", q("Python standard library")),
                ("device", Out::Null),
                (
                    "configuration",
                    o(vec![
                        ("pairing_key", q("unit_key")),
                        ("expected_pairing_count", n(298)),
                        ("source_only_unit_keys", Out::Array(vec![q("237a")])),
                        ("local_payloads_read", Out::Bool(false)),
                        ("source_to_target_text_compared", Out::Bool(false)),
                        ("translation_alignment_inferred", Out::Bool(false)),
                        ("source_numbered_unit_map_changed", Out::Bool(false)),
                        ("target_numbered_unit_map_changed", Out::Bool(false)),
                        ("pairing_content_changed", Out::Bool(false)),
                        ("rights_binding_refreshed", Out::Bool(true)),
                    ]),
                ),
                (
                    "prompt_or_instruction_ref",
                    q("ToS/doctrine/CORPUS_FOUNDATION.md#address-law"),
                ),
            ]),
        ),
        ("status", q("completed_with_warnings")),
        (
            "warnings",
            Out::Array(vec![
                q(
                    "The 298 pairs assert only that the same structural number-label key is independently materialized in both witness maps.",
                ),
                q(
                    "No source or target text was read, compared, transcribed, or accepted by this pairing route.",
                ),
                q(
                    "Shared numbering supports structural navigation. Exact passage alignment, translation correspondence, equivalence, quality and semantics require their own source-visible assessment.",
                ),
                q(
                    "Source-only 237a remains unpaired because the target does not materialize a repeated 237/237a label.",
                ),
                q(
                    "The superseding event refreshes the source rights-basis digest after a layered assessment. Payload and text comparison retain their earlier execution evidence; rights clearance follows its own owner decision.",
                ),
            ]),
        ),
        ("receipt_refs", Out::Array(vec![q(map_ref)])),
        ("rights_basis_ref", Out::Null),
        ("event_version", n(2)),
        (
            "supersedes_event_ref",
            q(
                "tos.event.parallel-numbered-unit-label-map.friedrich-nietzsche.jenseits-von-gut-und-boese.rights-refresh.2026-08-01",
            ),
        ),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intersection_preserves_target_order_and_refuses_duplicate_or_materialized_237a() {
        let rows=(1..=298).map(|i|json!({"unit_key":i.to_string(),"anchor_ref":format!("tos.anchor.test.unit-{i}"),"pdf_page":i})).collect::<Vec<_>>();
        let mut source = json!({"work_ref":"tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese","unit_starts":rows});
        let mut target = source.clone();
        source["unit_starts"]
            .as_array_mut()
            .unwrap()
            .push(json!({"unit_key":"237a","anchor_ref":"tos.anchor.test.extra","pdf_page":237}));
        target["unit_starts"].as_array_mut().unwrap().reverse();
        let pairs = pairings(&source, &target).unwrap();
        let value = serde_json::to_value(Out::Array(pairs)).unwrap();
        assert_eq!(value[0]["unit_key"], "298");
        assert_eq!(value[297]["unit_key"], "1");
        assert_eq!(value[0]["translation_alignment_claimed"], false);
        assert!(
            !value
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["unit_key"] == "237a")
        );
        let first = target["unit_starts"][0].clone();
        target["unit_starts"][1] = first;
        assert!(pairings(&source, &target).is_err());
        target["unit_starts"][1] = source["unit_starts"][298].clone();
        assert!(pairings(&source, &target).is_err());
    }
}
