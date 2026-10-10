//! Candidate-only eternal-return review preparation. Exact witness text stays private.
use crate::research_execution::ResearchExecution;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use tos_foundation::Digest256;

type Result<T> = std::result::Result<T, String>;
const WORK: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra";
const PARENT: &str = "ToS/candidate-intake/zarathustra/eternal-return-concept-candidate-v1";
const ALIGN_SUFFIX: &str =
    "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1";
const GENERATOR: &str = "scripts/build_zarathustra_eternal_return_review_preparation_v1.py";
// Selected maintained rendering-recipe identity. Native implementation and
// invocation identities belong to the independent execution receipt.
const RECIPE_SHA256: &str = "335df2c5ad52229d449cf6a22cdef4d5dabca49163b3154d85aed1e1a212027b";
const OUTPUTS: [(&str, &str); 8] = [
    ("gaps", "gap-review-candidates.v1.jsonl"),
    ("speakers", "speaker-attribution-candidates.v1.jsonl"),
    ("matrix", "interpretation-review-matrix.v1.json"),
    ("worklist", "review-worklist.v1.json"),
    ("coverage", "coverage-receipt.v1.json"),
    ("summary", "summary.v1.json"),
    ("provenance", "provenance.jsonl"),
    ("manifest", "manifest.v1.json"),
];
fn route(name: &str) -> String {
    format!("{PARENT}/review-preparation-v1/{name}")
}
fn private_ref() -> String {
    format!(
        "{WORK}/gold-sets/foundation-pilot-v1/local-content/eternal-return-concept-candidate-v1/review-preparation-v1/review-preparation-analysis.v1.json"
    )
}
fn constants() -> Value {
    serde_json::from_str(include_str!("research_eternal_return_constants.json"))
        .expect("checked static constants")
}
pub(crate) fn s(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(|| "string required".into())
}
pub(crate) fn a(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or_else(|| "array required".into())
}
pub(crate) fn object(v: &Value) -> Result<&serde_json::Map<String, Value>> {
    v.as_object().ok_or_else(|| "object required".into())
}
pub(crate) fn read(root: &ResearchExecution, p: &str) -> Result<Vec<u8>> {
    tos_foundation::RelativePath::parse(p).map_err(|e| e.to_string())?;
    root.read(p)
}

pub(crate) fn load(root: &ResearchExecution, p: &str) -> Result<Value> {
    let raw = read(root, p)?;
    root.tick(raw.len() as u64)?;
    // Reject duplicate published keys before converting into the working JSON representation.
    tos_foundation::parse_json(
        &raw,
        tos_foundation::JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::new(raw.len().max(1), 64, 2_000_000, 4300)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let value = serde_json::from_slice(&raw).map_err(|e| format!("{p}: {e}"))?;
    root.tick(1)?;
    Ok(value)
}
pub(crate) fn load_lines(root: &ResearchExecution, p: &str) -> Result<Vec<Value>> {
    String::from_utf8(read(root, p)?)
        .map_err(|e| e.to_string())?
        .lines()
        .map(|l| {
            root.tick(l.len() as u64)?;
            serde_json::from_str(l).map_err(|e| format!("{p}: {e}"))
        })
        .collect()
}
pub(crate) fn digest(b: &[u8]) -> String {
    Digest256::of_bytes(b).to_hex()
}
pub(crate) fn text_digest(v: &Value) -> Result<String> {
    Ok(digest(s(v)?.as_bytes()))
}
pub(crate) fn bytes(v: &Value, pretty: bool) -> Result<Vec<u8>> {
    let mut sorted = v.clone();
    sorted.sort_all_objects();
    let mut b = if pretty {
        serde_json::to_vec_pretty(&sorted)
    } else {
        serde_json::to_vec(&sorted)
    }
    .map_err(|e| e.to_string())?;
    b.push(b'\n');
    Ok(b)
}
pub(crate) fn lines(rows: &[Value]) -> Result<Vec<u8>> {
    let mut b = Vec::new();
    for r in rows {
        b.extend(bytes(r, false)?)
    }
    Ok(b)
}
pub(crate) fn indexed(
    root: &ResearchExecution,
    rows: Vec<Value>,
    key: &str,
) -> Result<BTreeMap<String, Value>> {
    let mut m = BTreeMap::new();
    for row in rows {
        root.tick(1)?;
        let k = s(&row[key])?.to_owned();
        if m.insert(k, row).is_some() {
            return Err(format!("duplicate {key}"));
        }
    }
    Ok(m)
}
// The selected Python rendering recipe interpolates a nullable ordinal as None,
// rather than the JSON spelling null. This label does not assign an ordinal.
fn reading_ref(part: usize, ordinal: &Value) -> String {
    let ordinal = if ordinal.is_null() {
        "None".to_owned()
    } else {
        ordinal.to_string()
    };
    format!("p{part}.r{ordinal}")
}
pub(crate) fn count(rows: &[Value], key: &str) -> Result<Value> {
    let mut m = BTreeMap::<String, usize>::new();
    for r in rows {
        *m.entry(s(&r[key])?.into()).or_default() += 1;
    }
    Ok(json!(m))
}
pub(crate) fn verify_inputs(root: &ResearchExecution, plan: &Value) -> Result<()> {
    for (name, r) in object(&plan["inputs"])? {
        root.tick(1)?;
        if digest(&read(root, s(&r["ref"])?)?) != s(&r["sha256"])? {
            return Err(format!("input drift: {name}"));
        }
    }
    Ok(())
}

/// The parent native producer supplies source-returned, alignment-keyed units.
pub fn source_ordered_rows(
    root: &ResearchExecution,
    units: &BTreeMap<String, Value>,
) -> Result<Vec<Value>> {
    let public = indexed(
        root,
        load_lines(root, &format!("{PARENT}/evidence-spine.v1.jsonl"))?,
        "alignment_ref",
    )?;
    let private=indexed(root, a(&load(root,&format!("{WORK}/gold-sets/foundation-pilot-v1/local-content/eternal-return-concept-candidate-v1/eternal-return-analysis.v1.json"))?["evidence"] )?.clone(),"alignment_ref")?;
    let spine = indexed(
        root,
        load_lines(
            root,
            &format!("{WORK}/{ALIGN_SUFFIX}/alignment-spine.v1.jsonl"),
        )?,
        "alignment_id",
    )?;
    let mut out = Vec::new();
    for part in 1..=4 {
        root.tick(1)?;
        let packet = load(
            root,
            &format!("{WORK}/{ALIGN_SUFFIX}/part-{part}.translation-alignment-packet.v1.json"),
        )?;
        for al in a(&packet["alignments"])? {
            root.tick(1)?;
            let aid = s(&al["alignment_id"])?;
            let Some(p) = public.get(aid) else { continue };
            let sp = spine.get(aid).ok_or("missing alignment spine")?;
            let reading = reading_ref(part, &sp["reading_ordinal_within_part"]);
            if !["p3.r2", "p3.r13", "p3.r16", "p4.r19"].contains(&reading.as_str())
                || !["core", "supporting"].contains(&s(&p["evidence_class"])?)
            {
                continue;
            }
            let exact = private.get(aid).ok_or("missing parent exact return")?;
            if text_digest(&exact["de_text"])? != s(&p["de_exact_sha256"])?
                || text_digest(&exact["ru_text"])? != s(&p["ru_exact_sha256"])?
            {
                return Err(format!("parent exact-text digest drift: {aid}"));
            }
            let mut row = units.get(aid).ok_or("missing hydrated alignment")?.clone();
            object(&row)?;
            for (k, v) in object(p)? {
                row[k] = v.clone()
            }
            row["de_text"] = exact["de_text"].clone();
            row["ru_text"] = exact["ru_text"].clone();
            out.push(row);
        }
    }
    if count(&out, "reading_ref")? != json!({"p3.r2":31,"p3.r13":50,"p3.r16":34,"p4.r19":44}) {
        return Err("speaker population drift".into());
    }
    Ok(out)
}

pub fn speaker_rule(reading: &str, p: usize) -> Result<Value> {
    let mut r = json!({"status":"proposed","alternative_roles":[],"exception_group":null});
    let (role, cue) = match reading {
        "p3.r2" => match p {
            1 => (
                "external_narrator",
                "chapter framing introduces Zarathustra's speech",
            ),
            3 | 4 => {
                r["alternative_roles"] = json!(["zarathustra_quoting_internal_voice"]);
                r["status"] = json!("ambiguous");
                r["exception_group"] = json!("p3.r2.embedded_gravity_voice");
                (
                    "spirit_of_gravity_as_dwarf_voice",
                    "embedded second-person taunt inside Zarathustra's narration",
                )
            }
            14 => ("dwarf", "explicit speech report names the dwarf"),
            _ => (
                "zarathustra_as_storyteller",
                "first-person address or narrated vision within Zarathustra's announced speech",
            ),
        },
        "p3.r13" => match p {
            1 => {
                r["alternative_roles"] = json!(["external_narrator", "zarathustra"]);
                r["status"] = json!("ambiguous");
                r["exception_group"] = json!("p3.r13.opening_transition");
                (
                    "mixed_external_narrator_and_zarathustra",
                    "narrator frame ends in direct Zarathustra speech",
                )
            }
            5 | 6 | 50 => (
                "external_narrator",
                "explicit narrative report outside quoted exchange",
            ),
            7..=9 | 16..=19 | 36 | 39..=49 => {
                if (39..=49).contains(&p) {
                    r["alternative_roles"] = json!(["animals_voicing_a_hypothetical_zarathustra"]);
                    r["status"] = json!("ambiguous");
                    r["exception_group"] = json!("p3.r13.animals_voice_zarathustra_formula")
                }
                (
                    "animals_eagle_and_serpent",
                    "animals' reply, including their representation of what Zarathustra teaches or would say",
                )
            }
            _ => ("zarathustra", "Zarathustra's direct answer to the animals"),
        },
        "p3.r16" => {
            if p == 1 {
                r["status"] = json!("ambiguous");
                r["alternative_roles"] = json!(["editorial_or_authorial_subtitle"]);
                r["exception_group"] = json!("p3.r16.heading_voice");
                (
                    "paratext_heading",
                    "parenthetical alternate title, not a dramatic utterance",
                )
            } else {
                (
                    "zarathustra_song_voice",
                    "first-person refrain within the Yes-and-Amen song",
                )
            }
        }
        "p4.r19" => match p {
            1 | 2 | 7 => ("external_narrator", "explicit narrative report"),
            3..=6 => (
                "ugliest_man",
                "speech explicitly introduced and closed as the ugliest man's",
            ),
            8 | 9 => {
                r["alternative_roles"] = json!(["external_narrator", "zarathustra"]);
                r["status"] = json!("ambiguous");
                r["exception_group"] = json!("p4.r19.narrator_to_midnight_transition");
                (
                    "mixed_external_narrator_and_zarathustra",
                    "narrator frame transitions into Zarathustra's altered voice",
                )
            }
            10 => (
                "zarathustra",
                "explicit narrator report followed by Zarathustra's direct command",
            ),
            _ => {
                r["alternative_roles"] = json!(["personified_midnight_or_bell_voice"]);
                r["status"] = json!("ambiguous");
                r["exception_group"] = json!("p4.r19.performative_midnight_voice");
                (
                    "zarathustra_midnight_song_voice",
                    "Zarathustra performs a midnight song while attributing speech to midnight, bell, pain, or joy",
                )
            }
        },
        _ => return Err(format!("unknown reading: {reading}")),
    };
    r["primary_role"] = json!(role);
    r["cue"] = json!(cue);
    Ok(r)
}
type Ids = BTreeMap<(String, String), String>;
fn bindings(root: &ResearchExecution, rows: &[Value]) -> Result<Vec<(String, String)>> {
    let c = constants();
    let mut out = vec![("packet".into(), "review-preparation-v1".into())];
    for g in a(&c["gaps"])? {
        root.tick(1)?;
        out.push(("gap".into(), s(&g["alignment_ref"])?.into()))
    }
    for r in rows {
        root.tick(1)?;
        out.push(("speaker".into(), s(&r["alignment_ref"])?.into()))
    }
    for axis in a(&c["axes"])? {
        root.tick(1)?;
        out.push(("axis".into(), s(axis)?.into()))
    }
    out.sort();
    Ok(out)
}
fn identity_map(root: &ResearchExecution, expected: &[(String, String)]) -> Result<Ids> {
    let v = load(root, &route("identity-issuance.v1.json"))?;
    let mut ids = Ids::new();
    let mut unique = BTreeSet::new();
    for r in a(&v["records"])? {
        root.tick(1)?;
        let k = (s(&r["kind"])?.into(), s(&r["binding"])?.into());
        let id = s(&r["id"])?.to_owned();
        if ids.insert(k, id.clone()).is_some() || !unique.insert(id) {
            return Err("identity issuance mismatch".into());
        }
    }
    if ids.keys().cloned().collect::<Vec<_>>() != expected {
        return Err("identity issuance mismatch".into());
    }
    Ok(ids)
}
fn id(ids: &Ids, kind: &str, binding: &str) -> Result<String> {
    ids.get(&(kind.into(), binding.into()))
        .cloned()
        .ok_or_else(|| format!("identity missing: {kind}:{binding}"))
}
fn issue(root: &ResearchExecution, expected: &[(String, String)], plan: &Value) -> Result<()> {
    let path = route("identity-issuance.v1.json");
    if root.join(&path).exists() {
        return Err("identity issuance already exists; refusing to remint".into());
    }
    let mut entropy = fs::File::open("/dev/urandom").map_err(|e| e.to_string())?;
    let mut records = Vec::new();
    for (k, b) in expected {
        root.tick(1)?;
        let prefix = match k.as_str() {
            "packet" => "review-preparation",
            "gap" => "gap-review-candidate",
            "speaker" => "speaker-attribution-candidate",
            "axis" => "interpretation-review-candidate",
            _ => return Err("unknown identity kind".into()),
        };
        let mut raw = [0u8; 16];
        root.read_exact(&mut entropy, &mut raw)?;
        let token = raw.iter().map(|v| format!("{v:02x}")).collect::<String>();
        records
            .push(json!({"kind":k,"binding":b,"id":format!("tos.annotation.{prefix}.sid-{token}")}))
    }
    write(
        root,
        &path,
        &bytes(
            &json!({"schema_version":"tos_zarathustra_eternal_return_review_preparation_identity_issuance_v1","identity_policy":"opaque-id-independent-of-source-text-label-speaker-name-and-current-interpretation","issued_at":plan["frozen_at"],"records":records}),
            true,
        )?,
        0o644,
    )
}
fn gap_rows(
    root: &ResearchExecution,
    units: &BTreeMap<String, Value>,
    ids: &Ids,
    verse: &Value,
) -> Result<(Vec<Value>, Vec<Value>)> {
    let c = constants();
    let parent = load(root, &format!("{PARENT}/concept-candidate.v1.json"))?;
    let mut public = Vec::new();
    let mut private = Vec::new();
    for spec in a(&c["gaps"])? {
        root.tick(1)?;
        let aid = s(&spec["alignment_ref"])?;
        let row = units.get(aid).ok_or("missing gap alignment")?;
        if row["reading"] != spec["reading_ref"] {
            return Err(format!("gap reading drift: {aid}"));
        }
        let code = s(&spec["gap_code"])?;
        let related = if code == "machine_target_gap_not_translation_omission" {
            json!([{"text_unit_ref":verse["text_unit_ref"],"text_sha256":verse["text_sha256"],"relation":"related_verse_line_outside_paragraph_alignment_scope_not_accepted_translation_mapping"}])
        } else {
            json!([])
        };
        let mut tracked = json!({"schema_version":"tos_zarathustra_eternal_return_gap_review_candidate_v1","gap_review_candidate_id":id(ids,"gap",aid)?,"parent_annotation_ref":parent["annotation_id"],"alignment_shape":row["shape"],"alignment_status":row["status"],"source_anchor_refs":row["source_anchor_refs"],"target_anchor_refs":row["target_anchor_refs"],"de_exact_sha256":text_digest(&row["de_text"])?,"ru_exact_sha256":text_digest(&row["ru_text"])?,"related_witness_units":related,"observed_text_preserved":true,"proposal_applied_to_witness":false,"proposal_applied_to_alignment":false,"review_status":"unreviewed","accepted":false,"human_judgment":false,"translation_equivalence_asserted":false,"graph_effect":false,"canon_effect":false,"source_text_included":false});
        for (k, v) in object(spec)? {
            tracked[k] = v.clone()
        }
        let proposal = match code {
            "ru_formula_broken_by_letterspacing" => {
                json!({"language":"ru","candidate_tokens":["вѣчнаго","возвращенія"],"operation":"collapse_letterspacing_for_search_index_only"})
            }
            "ru_formula_blocked_by_ocr_substitution" => {
                json!({"language":"ru","observed_token":"вЪчное","candidate_token":"вѣчное","operation":"single_token_ocr_correction_candidate_requires_page_image_review"})
            }
            _ => Value::Null,
        };
        let mut p = tracked.clone();
        p["de_text"] = row["de_text"].clone();
        p["ru_text"] = row["ru_text"].clone();
        p["reversible_proposal"] = proposal;
        p["related_ru_verse"] = if code == "machine_target_gap_not_translation_omission" {
            verse.clone()
        } else {
            Value::Null
        };
        public.push(tracked);
        private.push(p);
    }
    Ok((public, private))
}
fn speakers(root: &ResearchExecution, rows: &[Value], ids: &Ids) -> Result<Vec<Value>> {
    let mut counts = BTreeMap::<String, usize>::new();
    let mut out = Vec::new();
    for row in rows {
        root.tick(1)?;
        let reading = s(&row["reading_ref"])?;
        let position = counts.entry(reading.into()).or_default();
        *position += 1;
        let r = speaker_rule(reading, *position)?;
        out.push(json!({"schema_version":"tos_zarathustra_speaker_attribution_candidate_v1","speaker_attribution_candidate_id":id(ids,"speaker",s(&row["alignment_ref"])?)?,"parent_annotation_ref":row["annotation_ref"],"evidence_ref":row["evidence_id"],"alignment_ref":row["alignment_ref"],"reading_ref":reading,"source_order_within_selected_reading":position,"primary_role":r["primary_role"],"alternative_roles":r["alternative_roles"],"attribution_status":r["status"],"attribution_cue":r["cue"],"exception_group":r["exception_group"],"source_anchor_refs":row["source_anchor_refs"],"target_anchor_refs":row["target_anchor_refs"],"de_exact_sha256":row["de_exact_sha256"],"ru_exact_sha256":row["ru_exact_sha256"],"accepted":false,"review_status":"unreviewed","human_judgment":false,"materialized_claim":false,"graph_effect":false,"canon_effect":false}))
    }
    Ok(out)
}
fn matrix(root: &ResearchExecution, speakers: &[Value], ids: &Ids) -> Result<Value> {
    let c = constants();
    let templates = indexed(
        root,
        load_lines(root, &format!("{PARENT}/interpretation-templates.v1.jsonl"))?,
        "claim_code",
    )?;
    let evidence = indexed(
        root,
        load_lines(root, &format!("{PARENT}/evidence-spine.v1.jsonl"))?,
        "alignment_ref",
    )?;
    let by_evidence = indexed(root, speakers.to_vec(), "evidence_ref")?;
    let mut axes = Vec::new();
    for codev in a(&c["axes"])? {
        root.tick(1)?;
        let code = s(codev)?;
        let template = templates.get(code).ok_or("missing axis template")?;
        let mut counter = Vec::new();
        for aid in a(&c["counterpressure"][code])? {
            root.tick(1)?;
            counter.push(
                evidence
                    .get(s(aid)?)
                    .ok_or("missing counterpressure evidence")?["evidence_id"]
                    .clone(),
            )
        }
        let mut roles = BTreeMap::<String, usize>::new();
        let mut missing = 0;
        for ev in a(&template["evidence_refs"])? {
            root.tick(1)?;
            if let Some(sp) = by_evidence.get(s(ev)?) {
                *roles.entry(s(&sp["primary_role"])?.into()).or_default() += 1
            } else {
                missing += 1
            }
        }
        axes.push(json!({"interpretation_review_candidate_id":id(ids,"axis",code)?,"interpretation_template_ref":template["interpretation_template_id"],"axis_code":code,"status":if code=="amor_fati_cross_work"{"blocked_cross_work"}else{"prepared_for_review"},"positive_evidence_refs":template["evidence_refs"],"counterpressure_evidence_refs":counter,"speaker_role_counts_within_core_readings":roles,"positive_evidence_without_core_reading_speaker_candidate_count":missing,"candidate_synthesis":c["axis_analysis"][code]["candidate_synthesis"],"counterpressure_summary":c["axis_analysis"][code]["counterpressure_summary"],"speaker_dependency":c["axis_analysis"][code]["speaker_dependency"],"review_questions":c["questions"][code],"accepted":false,"human_judgment":false,"materialized_claim":false,"semantic_fact_asserted":false,"graph_effect":false,"canon_effect":false}));
    }
    Ok(
        json!({"schema_version":"tos_zarathustra_eternal_return_interpretation_review_matrix_v1","parent_annotation_ref":load(root,&format!("{PARENT}/concept-candidate.v1.json"))?["annotation_id"],"axes":axes,"comparison_law":"Positive evidence and counterpressure coexist; counts describe their occurrence, while assessment weighs their meaning.","accepted_axis_count":0,"human_review_count":0,"graph_effect":false,"canon_effect":false}),
    )
}
/// Outputs contain all public bytes; private bytes are returned separately for mode-0600 storage.
pub struct Prepared {
    pub outputs: BTreeMap<String, Vec<u8>>,
    pub private: Vec<u8>,
    pub summary: Value,
}
struct SelectedReviewPlan {
    reference: String,
    raw: Vec<u8>,
    digest: String,
    value: Value,
    custom: bool,
}
fn parse_plan(root: &ResearchExecution, raw: &[u8]) -> Result<Value> {
    root.tick(raw.len() as u64)?;
    tos_foundation::parse_json(
        raw,
        tos_foundation::JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::new(64 * 1024, 64, 2_000_000, 4300)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    root.check()?;
    let value = serde_json::from_slice(raw).map_err(|e| e.to_string());
    root.check()?;
    value
}
fn profile_semantics(original: &Value, value: &Value) -> Result<()> {
    let lineage = &value["input_profile_lineage"];
    if !lineage.is_object()
        || lineage["profile_version"].as_u64().is_none_or(|v| v < 2)
        || lineage["supersedes_plan_ref"] != route("plan.v1.json")
        || lineage["supersedes_plan_sha256"] != REVIEW_DEFAULT_PLAN_SHA256
        || value["plan_id"].as_str().is_none_or(|v| v.is_empty())
        || value["plan_id"] == original["plan_id"]
        || value["status"] != "proposed-technical-input-profile-successor"
    {
        return Err("technical profile requires distinct identity, proposal status and exact predecessor lineage".into());
    }
    let changed = [
        "parent_candidate_manifest",
        "parent_interpretation_templates",
        "parent_private_exact_analysis",
        "paragraph_alignment_manifest",
    ]
    .iter()
    .any(|label| value["inputs"][*label]["sha256"] != original["inputs"][*label]["sha256"]);
    if !changed {
        return Err("technical profile requires a selected input SHA change".into());
    }
    let mut comparable = value.clone();
    let fields = comparable.as_object_mut().ok_or("plan object required")?;
    fields.remove("input_profile_lineage");
    for key in ["plan_id", "status"] {
        fields.insert(key.into(), original[key].clone());
    }
    for label in [
        "parent_candidate_manifest",
        "parent_interpretation_templates",
        "parent_private_exact_analysis",
        "paragraph_alignment_manifest",
    ] {
        let record = &mut comparable["inputs"][label];
        let hash = s(&record["sha256"])?;
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("technical profile input SHA256 required".into());
        }
        record
            .as_object_mut()
            .ok_or("plan input object required")?
            .insert("sha256".into(), original["inputs"][label]["sha256"].clone());
    }
    if comparable != *original {
        return Err(
            "technical profile changed source semantics, input membership or unselected pins"
                .into(),
        );
    }
    Ok(())
}
fn select_review_plan(root: &ResearchExecution, reference: &str) -> Result<SelectedReviewPlan> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let default_ref = route("plan.v1.json");
    let mut original_file = root.source_file(&default_ref, 64 * 1024)?;
    let original_metadata = original_file.metadata().map_err(|e| e.to_string())?;
    let original_raw = root.read_file(&mut original_file, 64 * 1024)?;
    root.verify_file_unchanged(&original_file, &original_metadata)?;
    if digest(&original_raw) != REVIEW_DEFAULT_PLAN_SHA256 {
        return Err("default plan identity drift".into());
    }
    let original = parse_plan(root, &original_raw)?;
    let custom = reference != default_ref;
    if !custom {
        return Ok(SelectedReviewPlan {
            reference: reference.into(),
            raw: original_raw,
            digest: REVIEW_DEFAULT_PLAN_SHA256.into(),
            value: original,
            custom,
        });
    }
    let path = Path::new(reference);
    if path.is_absolute()
        || path
            .components()
            .any(|p| !matches!(p, std::path::Component::Normal(_)))
    {
        return Err("profile must be a normal repository relative path".into());
    }
    let root_metadata = root
        .root_directory()
        .metadata()
        .map_err(|e| e.to_string())?;
    let mut file = root.source_file(reference, 64 * 1024)?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    let uid = unsafe { libc::geteuid() };
    if root_metadata.permissions().mode() & 0o7777 != 0o700
        || root_metadata.uid() != uid
        || before.permissions().mode() & 0o7777 != 0o600
        || before.uid() != uid
    {
        return Err("custom profile requires owned private 0700 carrier and 0600 plan".into());
    }
    let raw = root.read_file(&mut file, 64 * 1024)?;
    root.verify_file_unchanged(&file, &before)?;
    let value = parse_plan(root, &raw)?;
    profile_semantics(&original, &value)?;
    root.tick(7)?;
    root.check()?;
    Ok(SelectedReviewPlan {
        reference: reference.into(),
        digest: digest(&raw),
        raw,
        value,
        custom,
    })
}
fn selected_input_refs(
    root: &ResearchExecution,
    selected: &SelectedReviewPlan,
) -> Result<Vec<Value>> {
    root.tick(selected.raw.len() as u64)?;
    let doc = tos_foundation::parse_json(
        &selected.raw,
        tos_foundation::JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::new(64 * 1024, 64, 2_000_000, 4300)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    root.check()?;
    let rows = doc
        .root()
        .object_get("inputs")
        .and_then(|v| v.as_object())
        .ok_or("plan inputs object required")?;
    let refs = rows
        .iter()
        .map(|(_, row)| {
            root.tick(1)?;
            row.object_get("ref")
                .and_then(|v| v.as_str())
                .map(|r| json!(r))
                .ok_or("input ref required".into())
        })
        .collect::<Result<Vec<Value>>>()?;
    root.check()?;
    Ok(refs)
}

pub fn prepare(
    root: &ResearchExecution,
    units: &BTreeMap<String, Value>,
    verse: &Value,
) -> Result<Prepared> {
    let selected = select_review_plan(root, &route("plan.v1.json"))?;
    verify_inputs(root, &selected.value)?;
    prepare_selected(root, units, verse, &selected)
}
fn prepare_selected(
    root: &ResearchExecution,
    units: &BTreeMap<String, Value>,
    verse: &Value,
    selected: &SelectedReviewPlan,
) -> Result<Prepared> {
    let plan = &selected.value;
    let rows = source_ordered_rows(root, units)?;
    let expected = bindings(root, &rows)?;
    let ids = identity_map(root, &expected)?;
    if s(&verse["text_unit_ref"])? != "tos.text-unit.sid-d523bc897647d01030f767c2faf5266b"
        || s(&verse["text_sha256"])?
            != "0627928ac02cab8159b250950b4a9b23088c9fe5886ebedf86634be3488da558"
        || text_digest(&verse["text"])? != s(&verse["text_sha256"])?
    {
        return Err("related Russian verse text drift".into());
    }
    let (gaps, private_gaps) = gap_rows(root, units, &ids, verse)?;
    let speakers = speakers(root, &rows, &ids)?;
    let matrix = matrix(root, &speakers, &ids)?;
    let mut groups = BTreeSet::<String>::new();
    for row in &speakers {
        root.tick(1)?;
        if !row["exception_group"].is_null() {
            groups.insert(s(&row["exception_group"])?.into());
        }
    }
    let mut items = Vec::new();
    for (n, row) in gaps.iter().enumerate() {
        root.tick(1)?;
        items.push(json!({"work_item_code":format!("gap.{}",n+1),"kind":"gap_exception_bundle","candidate_refs":[row["gap_review_candidate_id"]],"review_status":"unreviewed"}))
    }
    for group in &groups {
        root.tick(1)?;
        let refs: Vec<Value> = speakers
            .iter()
            .filter(|r| r["exception_group"] == json!(group))
            .map(|r| r["speaker_attribution_candidate_id"].clone())
            .collect();
        items.push(json!({"work_item_code":format!("speaker.{group}"),"kind":"speaker_boundary_bundle","candidate_refs":refs,"review_status":"unreviewed"}))
    }
    for axis in a(&matrix["axes"])? {
        root.tick(1)?;
        items.push(json!({"work_item_code":format!("axis.{}",s(&axis["axis_code"])?),"kind":"interpretation_axis_bundle","candidate_refs":[axis["interpretation_review_candidate_id"]],"review_status":"unreviewed"}))
    }
    let packet = id(&ids, "packet", "review-preparation-v1")?;
    let worklist = json!({"schema_version":"tos_zarathustra_eternal_return_review_worklist_v1","review_preparation_ref":packet,"work_items":items,"work_item_count":items.len(),"speaker_candidate_population":speakers.len(),"compression_law":"all candidates remain inspectable; future human attention is routed to grouped exceptions rather than every candidate row","review_outcome_recorded":false,"review_ledger_ref":null});
    let coverage = json!({"schema_version":"tos_zarathustra_eternal_return_review_preparation_coverage_v1","gap_population":5,"gap_candidates_prepared":gaps.len(),"gap_exact_source_return_count":private_gaps.len(),"alignment_scope_gap_with_related_ru_verse_count":gaps.iter().filter(|r|r["related_witness_units"].as_array().is_some_and(|a|!a.is_empty())).count(),"speaker_population":rows.len(),"speaker_candidates_prepared":speakers.len(),"speaker_reading_counts":count(&speakers,"reading_ref")?,"speaker_status_counts":count(&speakers,"attribution_status")?,"speaker_primary_role_counts":count(&speakers,"primary_role")?,"speaker_exception_group_count":groups.len(),"interpretation_axis_count":a(&matrix["axes"])?.len(),"five_primary_axes_prepared":a(&matrix["axes"])?.iter().take(5).all(|r|r["status"]=="prepared_for_review"),"amor_fati_cross_work_blocked":a(&matrix["axes"])?.last().is_some_and(|r|r["status"]=="blocked_cross_work"),"source_return_verified":true,"complete_for_declared_scope":gaps.len()==5&&speakers.len()==159});
    let summary = json!({"schema_version":"tos_zarathustra_eternal_return_review_preparation_summary_v1","review_preparation_id":packet,"gap_candidate_count":gaps.len(),"speaker_candidate_count":speakers.len(),"speaker_exception_group_count":groups.len(),"interpretation_review_candidate_count":a(&matrix["axes"])?.len(),"future_review_work_item_count":items.len(),"witness_correction_count":0,"alignment_mutation_count":0,"accepted_candidate_count":0,"human_review_count":0,"materialized_claim_count":0,"review_ledger_write_count":0,"graph_effect":false,"canon_effect":false});
    let input_refs = selected_input_refs(root, selected)?;
    let mut output_refs: Vec<String> = OUTPUTS.iter().map(|(_, n)| route(n)).collect();
    output_refs.push(private_ref());
    let provenance = json!({"schema_version":"tos_zarathustra_eternal_return_review_preparation_event_v1","event_id":"tos.event.zarathustra-eternal-return-review-preparation-v1.build","event_type":"candidate_review_preparation_built","event_at":plan["frozen_at"],"input_refs":input_refs,"output_refs":output_refs,"authority_effect":"candidate_only_no_human_review_graph_or_canon_effect"});
    let mut outputs = BTreeMap::new();
    outputs.insert(route(OUTPUTS[0].1), lines(&gaps)?);
    outputs.insert(route(OUTPUTS[1].1), lines(&speakers)?);
    for (n, v) in [
        (OUTPUTS[2].1, &matrix),
        (OUTPUTS[3].1, &worklist),
        (OUTPUTS[4].1, &coverage),
        (OUTPUTS[5].1, &summary),
    ] {
        outputs.insert(route(n), bytes(v, true)?);
    }
    outputs.insert(route(OUTPUTS[6].1), lines(&[provenance])?);
    let private_rows = indexed(root, rows.clone(), "alignment_ref")?;
    let speaker_evidence:Vec<Value>=speakers.iter().map(|sp|{let row=&private_rows[sp["alignment_ref"].as_str().unwrap()];json!({"speaker_attribution_candidate_id":sp["speaker_attribution_candidate_id"],"alignment_ref":sp["alignment_ref"],"de_text":row["de_text"],"ru_text":row["ru_text"]})}).collect();
    let c = constants();
    let mut counter_refs = BTreeSet::new();
    for vals in object(&c["counterpressure"])?.values() {
        for aid in a(vals)? {
            counter_refs.insert(s(aid)?.to_owned());
        }
    }
    let mut counter_return = Vec::new();
    for aid in counter_refs {
        root.tick(1)?;
        let row = units
            .get(&aid)
            .ok_or("missing counterpressure source return")?;
        counter_return
            .push(json!({"alignment_ref":aid,"de_text":row["de_text"],"ru_text":row["ru_text"]}))
    }
    let private = bytes(
        &json!({"schema_version":"tos_zarathustra_eternal_return_review_preparation_private_analysis_v1","review_preparation_id":packet,"content_posture":"private_exact_source_return_and_reversible_analysis_not_semantic_authority","gaps":private_gaps,"speaker_evidence":speaker_evidence,"counterpressure_source_return":counter_return}),
        true,
    )?;
    let artifacts: Vec<Value> = OUTPUTS
        .iter()
        .filter(|(role, _)| *role != "manifest")
        .map(|(role, n)| json!({"role":role,"ref":route(n),"sha256":digest(&outputs[&route(n)])}))
        .collect();
    let manifest = json!({"schema_version":"tos_zarathustra_eternal_return_review_preparation_manifest_v1","route_id":"zarathustra-eternal-return-review-preparation-v1","review_preparation_id":packet,"plan_ref":selected.reference,"plan_sha256":selected.digest,"identity_issuance_ref":route("identity-issuance.v1.json"),"identity_issuance_sha256":digest(&read(root,&route("identity-issuance.v1.json"))?),"generator_ref":GENERATOR,"generator_sha256":RECIPE_SHA256,"artifacts":artifacts,"private_artifact":{"ref":private_ref(),"sha256":digest(&private),"mode":"0600","tracked":false},"accepted_candidate_count":0,"human_review_count":0,"review_ledger_write_count":0,"graph_effect":false,"canon_effect":false});
    outputs.insert(route(OUTPUTS[7].1), bytes(&manifest, true)?);
    root.tick(private.len() as u64 + outputs.values().map(|v| v.len() as u64).sum::<u64>())?;
    Ok(Prepared {
        outputs,
        private,
        summary,
    })
}
pub(crate) fn mode600(root: &ResearchExecution, p: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    root.tick(1)?;
    tos_foundation::RelativePath::parse(p).map_err(|e| e.to_string())?;
    let file = root.source_file(p, 64 * 1024 * 1024)?;
    if file
        .metadata()
        .map_err(|e| e.to_string())?
        .permissions()
        .mode()
        & 0o777
        != 0o600
    {
        return Err(format!("private text layer is not 0600: {p}"));
    }
    Ok(())
}

/// Hydrate only fields consumed by this review producer, returning exact anchor slices.
pub fn hydrate_units(root: &ResearchExecution) -> Result<BTreeMap<String, Value>> {
    let spine = indexed(
        root,
        load_lines(
            root,
            &format!("{WORK}/{ALIGN_SUFFIX}/alignment-spine.v1.jsonl"),
        )?,
        "alignment_id",
    )?;
    root.tick(1)?;
    let mut cache = BTreeMap::<String, (String, Vec<usize>, String)>::new();
    let mut units = BTreeMap::new();
    for part in 1..=4 {
        root.tick(1)?;
        let packet = load(
            root,
            &format!("{WORK}/{ALIGN_SUFFIX}/part-{part}.translation-alignment-packet.v1.json"),
        )?;
        let source_anchors = indexed(
            root,
            a(&packet["source_side"]["anchors"])?.clone(),
            "anchor_ref",
        )?;
        let target_anchors = indexed(
            root,
            a(&packet["target_side"]["anchors"])?.clone(),
            "anchor_ref",
        )?;
        for alignment in a(&packet["alignments"])? {
            root.tick(1)?;
            let aid = s(&alignment["alignment_id"])?;
            let mut texts = Vec::new();
            for (side, refs) in [
                ("source_side", "ordered_source_anchor_refs"),
                ("target_side", "ordered_target_anchor_refs"),
            ] {
                let anchors = if side == "source_side" {
                    &source_anchors
                } else {
                    &target_anchors
                };
                let mut selected = Vec::new();
                for r in a(&alignment[refs])? {
                    root.tick(1)?;
                    let reference = s(r)?;
                    let anchor = anchors.get(reference).ok_or("missing anchor")?;
                    let layer_ref = s(&anchor["text_layer_ref"])?;
                    if !cache.contains_key(layer_ref) {
                        mode600(root, layer_ref)?;
                        let raw = read(root, layer_ref)?;
                        if digest(&raw) != s(&anchor["text_layer_sha256"])? {
                            return Err(format!("private text layer drift: {layer_ref}"));
                        }
                        root.tick(raw.len() as u64)?;
                        let text = String::from_utf8(raw).map_err(|e| e.to_string())?;
                        let mut offsets: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
                        offsets.push(text.len());
                        cache.insert(
                            layer_ref.into(),
                            (text, offsets, s(&anchor["text_layer_sha256"])?.to_owned()),
                        );
                    }
                    let (layer, offsets, layer_digest) = &cache[layer_ref];
                    if layer_digest != s(&anchor["text_layer_sha256"])? {
                        return Err(format!("private text layer drift: {layer_ref}"));
                    }
                    let start = anchor["selector"]["start"]
                        .as_u64()
                        .ok_or("invalid anchor start")? as usize;
                    let end = anchor["selector"]["end"]
                        .as_u64()
                        .ok_or("invalid anchor end")? as usize;
                    let chars = offsets.len() - 1;
                    if start > end || end > chars {
                        return Err("anchor range exceeds source".into());
                    }
                    let text = layer[offsets[start]..offsets[end]].to_owned();
                    if digest(text.as_bytes()) != s(&anchor["exact_sha256"])? {
                        return Err(format!("anchor return mismatch: {reference}"));
                    }
                    selected.push(text);
                }
                texts.push(selected.join("\n"));
            }
            let sp = spine.get(aid).ok_or("missing alignment spine")?;
            let row = json!({"alignment_id":aid,"part":part,"reading":reading_ref(part, &sp["reading_ordinal_within_part"]),"status":alignment["status"],"shape":alignment["correspondence_shape"],"source_anchor_refs":alignment["ordered_source_anchor_refs"],"target_anchor_refs":alignment["ordered_target_anchor_refs"],"de_text":texts[0],"ru_text":texts[1]});
            if units.insert(aid.into(), row).is_some() {
                return Err("duplicate alignment identity".into());
            }
        }
    }
    Ok(units)
}
pub fn return_ru_verse(root: &ResearchExecution) -> Result<Value> {
    root.reserve_structural_reads()?;
    let model = crate::antonovsky_structural::reconstruct_from_directory(
        root.root(),
        root.root_directory(),
        root.deadline(),
    )?;
    let mut charged = 0;
    root.charge_structural(&model, &mut charged)?;
    let ids = crate::antonovsky_structural::load_identities(root.root(), &model)?;
    root.charge_structural(&model, &mut charged)?;
    let wanted = "tos.text-unit.sid-d523bc897647d01030f767c2faf5266b";
    for row in &model.rows {
        root.tick(1)?;
        let binding = crate::antonovsky_structural::row_binding(row, &model.lines);
        if ids["logical_rows"][&binding] == wanted {
            if row.digest != "0627928ac02cab8159b250950b4a9b23088c9fe5886ebedf86634be3488da558" {
                return Err("related Russian verse text drift".into());
            }
            let refs: Vec<String> = row
                .lines
                .iter()
                .map(|i| model.lines[*i].source_line_ref.clone())
                .collect();
            return Ok(
                json!({"text_unit_ref":wanted,"text":row.text,"text_sha256":row.digest,"physical_line_refs":refs}),
            );
        }
    }
    Err("related Russian verse row not found".into())
}
pub(crate) fn write(root: &ResearchExecution, p: &str, payload: &[u8], mode: u32) -> Result<()> {
    tos_foundation::RelativePath::parse(p).map_err(|e| e.to_string())?;
    root.write(p, payload, mode, p.ends_with("/identity-issuance.v1.json"))
}

/// Named Access command; source root is explicit and no Python process is started.
fn retained_regular(root: &ResearchExecution, reference: &str, mode: u32) -> Result<Vec<u8>> {
    use std::os::unix::fs::PermissionsExt;
    let mut file = root.source_file(reference, 64 * 1024 * 1024)?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    if before.permissions().mode() & 0o7777 != mode {
        return Err("retained product mode drift".into());
    }
    let raw = root.read_file(&mut file, 64 * 1024 * 1024)?;
    root.verify_file_unchanged(&file, &before)?;
    root.check()?;
    Ok(raw)
}
pub(crate) fn retained_json_lines(root: &ResearchExecution, raw: &[u8]) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for line in std::str::from_utf8(raw).map_err(|e| e.to_string())?.lines() {
        root.tick(1)?;
        let row = serde_json::from_str(line).map_err(|e| e.to_string())?;
        root.check()?;
        rows.push(row);
    }
    root.check()?;
    Ok(rows)
}
pub(crate) struct RetainedProducts {
    pub tracked: BTreeMap<String, Vec<u8>>,
    pub private: Value,
    pub manifest: Value,
    pub manifest_raw: Vec<u8>,
    pub plan_raw: Vec<u8>,
}
pub(crate) fn retained_products(
    root: &ResearchExecution,
    route_ref: &str,
    plan_sha256: &str,
    generator: &str,
    recipe_sha256: &str,
    outputs: &[(&str, &str)],
    private_ref: &str,
) -> Result<RetainedProducts> {
    let plan_ref = format!("{route_ref}/plan.v1.json");
    let plan_raw = read(root, &plan_ref)?;
    if digest(&plan_raw) != plan_sha256 {
        return Err("default retained plan identity drift".into());
    }
    let plan: Value = serde_json::from_slice(&plan_raw).map_err(|e| e.to_string())?;
    root.check()?;
    verify_inputs(root, &plan)?;
    let manifest_ref = format!("{route_ref}/manifest.v1.json");
    let manifest_raw = retained_regular(root, &manifest_ref, 0o644)?;
    let manifest: Value = serde_json::from_slice(&manifest_raw).map_err(|e| e.to_string())?;
    root.check()?;
    if manifest["plan_ref"] != plan_ref
        || manifest["plan_sha256"] != digest(&plan_raw)
        || manifest["generator_ref"] != generator
        || manifest["generator_sha256"] != recipe_sha256
        || manifest["identity_issuance_ref"] != format!("{route_ref}/identity-issuance.v1.json")
        || manifest["identity_issuance_sha256"]
            != digest(&read(
                root,
                &format!("{route_ref}/identity-issuance.v1.json"),
            )?)
    {
        return Err("retained manifest input/source identity drift".into());
    }
    let mut tracked = BTreeMap::new();
    let mut roles = BTreeSet::new();
    for artifact in a(&manifest["artifacts"])? {
        root.tick(1)?;
        let role = s(&artifact["role"])?;
        let expected = outputs
            .iter()
            .find(|(r, _)| *r == role && *r != "manifest")
            .ok_or("unexpected retained artifact role")?;
        let reference = format!("{route_ref}/{}", expected.1);
        if artifact["ref"] != reference || !roles.insert(role.to_owned()) {
            return Err("retained artifact membership drift".into());
        }
        let raw = retained_regular(root, &reference, 0o644)?;
        if digest(&raw) != s(&artifact["sha256"])? {
            return Err("retained artifact digest drift".into());
        }
        tracked.insert(role.to_owned(), raw);
    }
    if roles.len() != outputs.len() - 1 {
        return Err("retained artifact population drift".into());
    }
    if manifest["private_artifact"]["ref"] != private_ref.to_owned()
        || manifest["private_artifact"]["mode"] != "0600"
        || manifest["private_artifact"]["tracked"] != false
    {
        return Err("retained private artifact identity drift".into());
    }
    let private_raw = retained_regular(root, private_ref, 0o600)?;
    if digest(&private_raw) != s(&manifest["private_artifact"]["sha256"])? {
        return Err("retained private digest drift".into());
    }
    let private_value: Value = serde_json::from_slice(&private_raw).map_err(|e| e.to_string())?;
    root.check()?;
    Ok(RetainedProducts {
        tracked,
        private: private_value,
        manifest,
        manifest_raw,
        plan_raw,
    })
}

const REVIEW_DEFAULT_PLAN_SHA256: &str =
    "d16900b7d259d525da24a8fd2ed1567567872b2e01929f75e5988349f08877ff";

fn validate_tracked(root: &ResearchExecution) -> Result<Value> {
    let route_ref = format!("{PARENT}/review-preparation-v1");
    let retained = retained_products(
        root,
        &route_ref,
        REVIEW_DEFAULT_PLAN_SHA256,
        GENERATOR,
        RECIPE_SHA256,
        &OUTPUTS,
        &private_ref(),
    )?;
    let decode = |role: &str| retained_json_lines(root, &retained.tracked[role]);
    let speakers = decode("speakers")?;
    let gaps = decode("gaps")?;
    let expected = bindings(root, &speakers)?;
    let ids = identity_map(root, &expected)?;
    let private_speakers = indexed(
        root,
        a(&retained.private["speaker_evidence"])?.clone(),
        "alignment_ref",
    )?;
    let private_gaps = indexed(root, a(&retained.private["gaps"])?.clone(), "alignment_ref")?;
    if speakers.len() != private_speakers.len() || gaps.len() != private_gaps.len() {
        return Err("retained Review private population drift".into());
    }
    for (rows, private_rows, kind, field) in [
        (
            &speakers,
            &private_speakers,
            "speaker",
            "speaker_attribution_candidate_id",
        ),
        (&gaps, &private_gaps, "gap", "gap_review_candidate_id"),
    ] {
        for row in rows {
            root.tick(1)?;
            let aid = s(&row["alignment_ref"])?;
            let exact = private_rows
                .get(aid)
                .ok_or("retained Review exact return absent")?;
            if row[field] != id(&ids, kind, aid)?
                || exact[field] != row[field]
                || text_digest(&exact["de_text"])? != s(&row["de_exact_sha256"])?
                || text_digest(&exact["ru_text"])? != s(&row["ru_exact_sha256"])?
                || row["accepted"] != false
                || row["human_judgment"] != false
                || row["graph_effect"] != false
                || row["canon_effect"] != false
            {
                return Err("retained Review evidence identity/return/ceiling drift".into());
            }
        }
    }
    let matrix: Value =
        serde_json::from_slice(&retained.tracked["matrix"]).map_err(|e| e.to_string())?;
    root.check()?;
    for axis in a(&matrix["axes"])? {
        root.tick(1)?;
        if axis["interpretation_review_candidate_id"] != id(&ids, "axis", s(&axis["axis_code"])?)?
            || axis["accepted"] != false
            || axis["materialized_claim"] != false
            || axis["semantic_fact_asserted"] != false
            || axis["graph_effect"] != false
            || axis["canon_effect"] != false
        {
            return Err("retained Review axis identity/ceiling drift".into());
        }
    }
    let summary: Value =
        serde_json::from_slice(&retained.tracked["summary"]).map_err(|e| e.to_string())?;
    root.check()?;
    let coverage: Value =
        serde_json::from_slice(&retained.tracked["coverage"]).map_err(|e| e.to_string())?;
    root.check()?;
    if summary["speaker_candidate_count"] != speakers.len()
        || summary["gap_candidate_count"] != gaps.len()
        || coverage["speaker_candidates_prepared"] != speakers.len()
        || coverage["gap_candidates_prepared"] != gaps.len()
        || coverage["speaker_reading_counts"] != count(&speakers, "reading_ref")?
        || summary["review_preparation_id"] != id(&ids, "packet", "review-preparation-v1")?
    {
        return Err("retained Review count/packet identity drift".into());
    }
    for value in [&retained.manifest, &summary] {
        if value["accepted_candidate_count"] != 0
            || value["human_review_count"] != 0
            || value["review_ledger_write_count"] != 0
            || value["graph_effect"] != false
            || value["canon_effect"] != false
        {
            return Err("retained Review authority ceiling drift".into());
        }
    }
    let report = json!({"status":"validated-existing-receipts-no-regeneration","plan_ref":format!("{route_ref}/plan.v1.json"),"plan_sha256":digest(&retained.plan_raw),"manifest_sha256":digest(&retained.manifest_raw),"generated_outputs_validated":8,"private_outputs_validated":1,"identity_count":ids.len(),"algorithm_equivalence_asserted":false,"provider_invoked":false,"writes":false});
    root.check()?;
    Ok(report)
}
pub fn run(root: &Path, args: &[String]) -> Result<Value> {
    let execution = ResearchExecution::new(root, 180)?;
    run_scoped(&execution, args)
}
pub fn run_scoped(root: &ResearchExecution, args: &[String]) -> Result<Value> {
    let mut mode = None;
    let mut issuance = false;
    let mut plan_ref = route("plan.v1.json");
    let mut plan_seen = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        root.tick(1)?;
        match arg.as_str() {
            "--build" | "--check" | "--preview" | "--validate-tracked" => {
                if mode.replace(arg.as_str()).is_some() {
                    return Err("exactly one mode required".into());
                }
            }
            "--issue-identities" => issuance = true,
            "--plan-ref" => {
                if plan_seen {
                    return Err("duplicate --plan-ref".into());
                }
                plan_seen = true;
                plan_ref = args.next().ok_or("--plan-ref needs a reference")?.clone();
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    let mode = mode.ok_or("one of --build, --check, --preview, --validate-tracked is required")?;
    if issuance && mode != "--build" {
        return Err("--issue-identities is valid only with --build".into());
    }
    if mode == "--validate-tracked" {
        if plan_ref != route("plan.v1.json") || issuance {
            return Err("retained validator uses original default plan without issuance".into());
        }
        return validate_tracked(root);
    }
    let selected = select_review_plan(root, &plan_ref)?;
    if selected.custom && issuance {
        return Err("custom profile cannot issue identities".into());
    }
    verify_inputs(root, &selected.value)?;
    let units = hydrate_units(root)?;
    let rows = source_ordered_rows(root, &units)?;
    let expected = bindings(root, &rows)?;
    if selected.custom {
        identity_map(root, &expected)?;
    }
    if mode == "--preview" {
        root.check()?;
        return Ok(json!({"identity_count":expected.len(),"speaker_candidate_count":rows.len()}));
    }
    if issuance {
        issue(root, &expected, &selected.value)?
    }
    identity_map(root, &expected)?;
    let verse = return_ru_verse(root)?;
    let prepared = prepare_selected(root, &units, &verse, &selected)?;
    if mode == "--build" {
        for (p, b) in &prepared.outputs {
            root.tick(1)?;
            write(root, p, b, 0o644)?
        }
        write(root, &private_ref(), &prepared.private, 0o600)?;
    } else {
        for (p, b) in &prepared.outputs {
            root.tick(1)?;
            if read(root, p)? != *b {
                return Err(format!("tracked parity mismatch: {p}"));
            }
        }
        if read(root, &private_ref())? != prepared.private {
            return Err(format!("private parity mismatch: {}", private_ref()));
        }
        mode600(root, &private_ref())?;
    }
    root.tick(1)?;
    Ok(prepared.summary)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn technical_profile_preserves_review_semantics_and_unselected_pins() {
        let hash = "a".repeat(64);
        let original = json!({"plan_id":"original","status":"frozen","frozen_at":"historical","scope":{"speaker_count":159},"inputs":{
            "parent_candidate_manifest":{"ref":"parent/manifest","sha256":hash},
            "parent_interpretation_templates":{"ref":"parent/templates","sha256":hash},
            "parent_private_exact_analysis":{"ref":"parent/private","sha256":hash},
            "paragraph_alignment_manifest":{"ref":"paragraph/manifest","sha256":hash},
            "russian_structural_manifest":{"ref":"structural/manifest","sha256":hash},
            "semantic_identity_research":{"ref":"research","sha256":hash},
            "review_checklist":{"ref":"checklist","sha256":hash}}});
        let mut successor = original.clone();
        successor["plan_id"] = json!("technical-successor");
        successor["status"] = json!("proposed-technical-input-profile-successor");
        successor["input_profile_lineage"] = json!({"profile_version":2,"supersedes_plan_ref":route("plan.v1.json"),"supersedes_plan_sha256":REVIEW_DEFAULT_PLAN_SHA256});
        successor["inputs"]["parent_candidate_manifest"]["sha256"] = json!("b".repeat(64));
        assert!(profile_semantics(&original, &successor).is_ok());
        for (field, value) in [
            ("scope", json!({"speaker_count":158})),
            ("frozen_at", json!("rewritten")),
        ] {
            let mut invalid = successor.clone();
            invalid[field] = value;
            assert!(profile_semantics(&original, &invalid).is_err());
        }
        let mut invalid = successor.clone();
        invalid["inputs"]["review_checklist"]["sha256"] = json!("c".repeat(64));
        assert!(profile_semantics(&original, &invalid).is_err());
        invalid = successor.clone();
        invalid["inputs"]["parent_candidate_manifest"]["ref"] = json!("different/manifest");
        assert!(profile_semantics(&original, &invalid).is_err());
        invalid = successor.clone();
        invalid["inputs"]["parent_candidate_manifest"]["sha256"] = json!("a".repeat(64));
        assert!(profile_semantics(&original, &invalid).is_err());
    }

    #[test]
    fn dramatic_voice_boundaries() {
        assert_eq!(speaker_rule("p3.r2", 14).unwrap()["primary_role"], "dwarf");
        assert_eq!(
            speaker_rule("p3.r13", 39).unwrap()["alternative_roles"],
            json!(["animals_voicing_a_hypothetical_zarathustra"])
        );
        assert_eq!(speaker_rule("p4.r19", 11).unwrap()["status"], "ambiguous");
        assert!(speaker_rule("p9.r9", 1).is_err());
    }
    #[test]
    fn full_voice_population_preserves_ambiguity() {
        let mut rows = Vec::new();
        for (reading, n) in [
            ("p3.r2", 31),
            ("p3.r13", 50),
            ("p3.r16", 34),
            ("p4.r19", 44),
        ] {
            for p in 1..=n {
                rows.push(speaker_rule(reading, p).unwrap())
            }
        }
        assert_eq!(
            count(&rows, "status").unwrap(),
            json!({"ambiguous":51,"proposed":108})
        );
        assert_eq!(
            rows.iter()
                .filter(|r| r["primary_role"] == "animals_eagle_and_serpent")
                .count(),
            19
        );
    }
    #[test]
    fn exact_json_bytes_with_unicode() {
        assert_eq!(
            bytes(&json!({"z":null,"a":"ѣ"}), true).unwrap(),
            "{\n  \"a\": \"ѣ\",\n  \"z\": null\n}\n".as_bytes()
        );
    }
    #[test]
    fn identity_publication_never_replaces_existing_issuance() {
        let dir = tempfile::tempdir().unwrap();
        let execution =
            ResearchExecution::new_with_scratch(dir.path(), 180, 16 * 1024 * 1024).unwrap();
        let ref_ = "route/identity-issuance.v1.json";
        write(&execution, ref_, b"first", 0o644).unwrap();
        assert!(write(&execution, ref_, b"second", 0o644).is_err());
        assert_eq!(read(&execution, ref_).unwrap(), b"first");
    }
    #[test]
    fn selected_source_rejects_path_escape_and_symlink() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("source"), b"source").unwrap();
        symlink(dir.path().join("source"), dir.path().join("alias")).unwrap();
        let execution =
            ResearchExecution::new_with_scratch(dir.path(), 180, 16 * 1024 * 1024).unwrap();
        assert!(read(&execution, "../source").is_err());
        assert!(read(&execution, "alias").is_err());
    }
    #[test]
    fn duplicate_identity_binding_fails() {
        let rows = vec![json!({"alignment_ref":"a"}), json!({"alignment_ref":"a"})];
        let directory = tempfile::tempdir().unwrap();
        let root = ResearchExecution::new(directory.path(), 180).unwrap();
        assert!(indexed(&root, rows, "alignment_ref").is_err());
    }
}

/// Frozen provenance retains authored input order separately from sorted output keys.
pub(crate) fn ordered_input_refs(root: &ResearchExecution, p: &str) -> Result<Vec<Value>> {
    let raw = read(root, p)?;
    root.tick(raw.len() as u64)?;
    let doc = tos_foundation::parse_json(
        &raw,
        tos_foundation::JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::new(raw.len().max(1), 64, 2_000_000, 4300)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let entries = doc
        .root()
        .object_get("inputs")
        .and_then(|v| v.as_object())
        .ok_or("plan inputs object required")?;
    entries
        .iter()
        .map(|(_, r)| {
            r.object_get("ref")
                .and_then(|v| v.as_str())
                .map(|s| json!(s))
                .ok_or_else(|| "input ref required".into())
        })
        .collect()
}
