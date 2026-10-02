//! Source-addressed bilingual concept dossier; generated candidates have no admission effect.
use crate::research_eternal_return::{
    a, bytes, count, digest, indexed, lines, load, load_lines, mode600, object, read, s,
    text_digest, verify_inputs, write,
};
use crate::research_execution::ResearchExecution;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
type Result<T> = std::result::Result<T, String>;
type Ids = BTreeMap<(String, String), String>;
const WORK: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra";
const ROUTE: &str = "ToS/candidate-intake/zarathustra/eternal-return-concept-candidate-v1";
const ALIGN: &str = "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1";
const GENERATOR: &str = "scripts/build_zarathustra_eternal_return_concept_candidate_v1.py";
// Selected maintained rendering-recipe identity. Native implementation and
// invocation identities belong to the independent execution receipt.
const RECIPE_SHA256: &str = "35f9a992d0f9460baea7649dde7abed4f615188a376b915041ce49c734400c12";
const OUTPUTS: [(&str, &str); 11] = [
    ("concept", "concept-candidate.v1.json"),
    ("evidence", "evidence-spine.v1.jsonl"),
    ("formulas", "formula-candidates.v1.jsonl"),
    ("templates", "interpretation-templates.v1.jsonl"),
    ("exclusions", "exclusion-ledger.v1.jsonl"),
    ("promotion", "promotion-readiness.v1.json"),
    ("coverage", "coverage-receipt.v1.json"),
    ("summary", "summary.v1.json"),
    ("audit", "independent-agent-audit.v1.json"),
    ("provenance", "provenance.jsonl"),
    ("manifest", "manifest.v1.json"),
];
fn route(n: &str) -> String {
    format!("{ROUTE}/{n}")
}

const DEFAULT_PLAN_SHA256: &str =
    "2ce7877b1c83a83647ca2ab3cc5e57e3ad2ce9c552e4953defeab049a05ca6e9";
struct SelectedConceptPlan {
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
        || lineage["supersedes_plan_sha256"] != DEFAULT_PLAN_SHA256
        || value["plan_id"].as_str().is_none_or(|v| v.is_empty())
        || value["plan_id"] == original["plan_id"]
        || value["status"] != "proposed-technical-input-profile-successor"
    {
        return Err("technical profile requires distinct identity, proposal status and exact predecessor lineage".into());
    }
    let mut comparable = value.clone();
    let fields = comparable.as_object_mut().ok_or("plan object required")?;
    fields.remove("input_profile_lineage");
    for key in ["plan_id", "status"] {
        fields.insert(key.into(), original[key].clone());
    }
    for label in ["parallel_lexical_manifest", "morphology_theme_manifest"] {
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
fn select_concept_plan(root: &ResearchExecution, reference: &str) -> Result<SelectedConceptPlan> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let default_ref = route("plan.v1.json");
    let mut original_file = root.source_file(&default_ref, 64 * 1024)?;
    let original_metadata = original_file.metadata().map_err(|e| e.to_string())?;
    let original_raw = root.read_file(&mut original_file, 64 * 1024)?;
    root.verify_file_unchanged(&original_file, &original_metadata)?;
    if digest(&original_raw) != DEFAULT_PLAN_SHA256 {
        return Err("default plan identity drift".into());
    }
    let original = parse_plan(root, &original_raw)?;
    let custom = reference != default_ref;
    if !custom {
        return Ok(SelectedConceptPlan {
            reference: reference.into(),
            raw: original_raw,
            digest: DEFAULT_PLAN_SHA256.into(),
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
    Ok(SelectedConceptPlan {
        reference: reference.into(),
        digest: digest(&raw),
        raw,
        value,
        custom,
    })
}
fn selected_input_refs(
    root: &ResearchExecution,
    selected: &SelectedConceptPlan,
) -> Result<Vec<Value>> {
    root.tick(selected.raw.len() as u64)?;
    let doc = tos_foundation::parse_json(
        &selected.raw,
        tos_foundation::JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::new(64 * 1024, 64, 2_000_000, 4300)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
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
fn private_ref() -> String {
    format!(
        "{WORK}/gold-sets/foundation-pilot-v1/local-content/eternal-return-concept-candidate-v1/eternal-return-analysis.v1.json"
    )
}
fn templates() -> Value {
    serde_json::from_str(include_str!(
        "research_eternal_return_concept_templates.json"
    ))
    .expect("checked authored templates")
}
fn core(reading: &str) -> bool {
    ["p3.r2", "p3.r13", "p3.r16", "p4.r19"].contains(&reading)
}
fn strings(v: &Value) -> Result<Vec<String>> {
    a(v)?.iter().map(|v| s(v).map(str::to_owned)).collect()
}
fn has(tokens: &[String], prefixes: &[&str]) -> bool {
    tokens
        .iter()
        .any(|x| prefixes.iter().any(|p| x.starts_with(p)))
}
fn exact(tokens: &[String], words: &[&str]) -> bool {
    tokens.iter().any(|x| words.contains(&x.as_str()))
}
pub fn contiguous_count(tokens: &[String], pattern: &[String]) -> usize {
    if pattern.is_empty() {
        tokens.len() + 1
    } else {
        tokens
            .windows(pattern.len())
            .filter(|w| *w == pattern)
            .count()
    }
}
pub fn signal_codes(de: &[String], ru: &[String]) -> Vec<String> {
    let rules = [
        ("de_recurrence_noun", has(de, &["wiederkunft"])),
        ("de_recurrence_verb", has(de, &["wiederkehr"])),
        ("de_eternity", has(de, &["ewig"])),
        ("de_ring", has(de, &["ring"])),
        ("de_time", has(de, &["zeit"])),
        ("de_moment", has(de, &["augenblick"])),
        ("de_joy", has(de, &["freud"])),
        ("de_pain", has(de, &["schmerz", "leid"])),
        ("de_love", has(de, &["lieb"])),
        ("de_life", has(de, &["leb"])),
        ("de_affirmation", exact(de, &["ja"]) || has(de, &["bejah"])),
        (
            "de_universal_scope",
            exact(de, &["all", "alle", "allen", "aller", "alles"]) || has(de, &["gleich", "selb"]),
        ),
        ("ru_recurrence", has(ru, &["возвращ"])),
        ("ru_eternity", has(ru, &["вечн"])),
        ("ru_ring", has(ru, &["кольц"])),
        ("ru_time", has(ru, &["врем"])),
        ("ru_moment", has(ru, &["мгновен"])),
        ("ru_joy", has(ru, &["радост"])),
        ("ru_pain", has(ru, &["боль", "страд"])),
        ("ru_love", has(ru, &["люб"])),
        ("ru_life", has(ru, &["жизн"])),
        (
            "ru_affirmation",
            exact(ru, &["да"]) || has(ru, &["утвержд"]),
        ),
        (
            "ru_universal_scope",
            exact(ru, &["все", "всё", "всего", "вся"]) || has(ru, &["одинаков", "тотже"]),
        ),
    ];
    let mut out: Vec<String> = rules
        .iter()
        .filter(|(_, b)| *b)
        .map(|(n, _)| (*n).into())
        .collect();
    out.sort();
    out
}
fn exclusion_code(row: &Value) -> Result<Option<&'static str>> {
    let de = strings(&row["de"])?;
    let ru = strings(&row["ru"])?;
    let reading = s(&row["reading"])?;
    if reading == "p1.r1" && exact(&de, &["wieder"]) && exact(&de, &["mensch"]) {
        Ok(Some("return_to_humans_not_eternal_return"))
    } else if reading == "p3.r9" && (has(&de, &["heimkehr"]) || has(&ru, &["возврат"])) {
        Ok(Some("return_home_not_eternal_return"))
    } else {
        Ok(None)
    }
}
pub fn evidence_class(row: &Value, signals: &[String]) -> Result<Option<&'static str>> {
    if exclusion_code(row)?.is_some() {
        return Ok(Some("excluded"));
    }
    let recurrence = exact(
        signals,
        &["de_recurrence_noun", "de_recurrence_verb", "ru_recurrence"],
    );
    let eternity = exact(signals, &["de_eternity", "ru_eternity"]);
    let ring = exact(signals, &["de_ring", "ru_ring"]);
    let universal = exact(signals, &["de_universal_scope", "ru_universal_scope"]);
    let de = strings(&row["de"])?;
    let eternal_again = contiguous_count(&de, &["ewig".into(), "wieder".into()]) > 0;
    if core(s(&row["reading"])?) {
        if exact(signals, &["de_recurrence_noun"])
            || eternal_again
            || (recurrence && (eternity || ring || universal))
        {
            return Ok(Some("core"));
        }
        if !signals.is_empty() {
            return Ok(Some("supporting"));
        }
    } else if recurrence || eternity {
        return Ok(Some("ambiguous"));
    }
    Ok(None)
}
fn claim_codes(row: &Value, signals: &[String], klass: &str) -> Result<Vec<String>> {
    let mut result = BTreeSet::new();
    let recurrence = signals
        .iter()
        .any(|x| x.ends_with("recurrence") || x.contains("recurrence_"));
    if klass == "core" {
        result.insert("cosmological_recurrence".into());
        result.insert("poetic_symbolic_affirmation".into());
    }
    if row["reading"] == "p3.r2" {
        result.insert("existential_test_and_affirmation".into());
    }
    if signals
        .iter()
        .any(|x| x.ends_with("time") || x.ends_with("moment"))
    {
        result.insert("temporality_and_augenblick".into());
    }
    if signals.iter().any(|x| x.ends_with("ring")) {
        result.insert("ring_and_wholeness".into());
    }
    if signals.iter().any(|x| {
        ["life", "joy", "pain", "love", "affirmation"]
            .iter()
            .any(|p| x.ends_with(p))
    }) {
        result.insert("life_joy_pain_affirmation".into());
    }
    if klass == "excluded" && exclusion_code(row)? == Some("return_to_humans_not_eternal_return") {
        result.insert("distinguished_from_return_as_action".into());
    }
    if recurrence && klass == "core" {
        result.insert("existential_test_and_affirmation".into());
    }
    Ok(result.into_iter().collect())
}
/// Native lexical algorithms supply token/quality fields; native anchor return supplies exact strings.
pub fn hydrate_units(root: &ResearchExecution) -> Result<Vec<Value>> {
    let source = crate::research_eternal_return::hydrate_units(root)?;
    let mut rows = crate::research_parallel_lexical::load_parallel(root)?;
    let spine = indexed(
        root,
        load_lines(root, &format!("{WORK}/{ALIGN}/alignment-spine.v1.jsonl"))?,
        "alignment_id",
    )?;
    for row in &mut rows {
        root.tick(1)?;
        let aid = s(&row["alignment_id"])?.to_owned();
        let exact = source
            .get(&aid)
            .ok_or("missing source-returned alignment")?;
        for (k, v) in object(exact)? {
            row[k] = v.clone()
        }
        let sp = spine.get(&aid).ok_or("missing alignment spine")?;
        row["source_paragraph_unit_refs"] = sp["source_paragraph_unit_refs"].clone();
        row["target_paragraph_unit_refs"] = sp["target_paragraph_unit_refs"].clone();
    }
    rows.sort_by_key(|r| {
        let reading = r["reading"].as_str().unwrap_or("");
        let n = reading
            .split_once('r')
            .and_then(|(_, r)| r.parse::<u64>().ok())
            .unwrap_or(10000);
        (
            r["part"].as_u64().unwrap_or(0),
            n,
            r["alignment_id"].as_str().unwrap_or("").to_owned(),
        )
    });
    Ok(rows)
}
fn identity_bindings(root: &ResearchExecution, units: &[Value]) -> Result<Vec<(String, String)>> {
    let t = templates();
    let mut bindings = vec![("annotation".into(), "eternal-return-dossier-v1".into())];
    for f in a(&t["formulas"])? {
        root.tick(1)?;
        bindings.push(("formula".into(), s(&f[0])?.into()))
    }
    for c in a(&t["claim_order"])? {
        root.tick(1)?;
        bindings.push(("template".into(), s(c)?.into()))
    }
    for row in units {
        root.tick(1)?;
        let signals = signal_codes(&strings(&row["de"])?, &strings(&row["ru"])?);
        if let Some(klass) = evidence_class(row, &signals)? {
            bindings.push((
                "evidence".into(),
                format!("{klass}|{}", s(&row["alignment_id"])?),
            ))
        }
    }
    bindings.sort();
    Ok(bindings)
}
fn identity_map(root: &ResearchExecution, bindings: &[(String, String)]) -> Result<Ids> {
    let v = load(root, &route("identity-issuance.v1.json"))?;
    let mut result = Ids::new();
    let mut unique = BTreeSet::new();
    for r in a(&v["records"])? {
        root.tick(1)?;
        let key = (s(&r["kind"])?.into(), s(&r["binding"])?.into());
        let id = s(&r["id"])?.to_owned();
        if result.insert(key, id.clone()).is_some() || !unique.insert(id) {
            return Err("duplicate opaque identity or binding".into());
        }
    }
    if result.keys().cloned().collect::<Vec<_>>() != bindings {
        return Err("identity issuance binding mismatch".into());
    }
    Ok(result)
}
fn id(ids: &Ids, k: &str, b: &str) -> Result<String> {
    ids.get(&(k.into(), b.into()))
        .cloned()
        .ok_or_else(|| format!("missing identity: {k}|{b}"))
}
fn issue(root: &ResearchExecution, bindings: &[(String, String)], plan: &Value) -> Result<()> {
    let reference = route("identity-issuance.v1.json");
    if root.join(&reference).exists() {
        return Err("identity issuance already exists; refusing to remint".into());
    }
    let mut entropy = fs::File::open("/dev/urandom").map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    for (k, b) in bindings {
        root.tick(1)?;
        let prefix = match k.as_str() {
            "annotation" => "eternal-return-candidate",
            "formula" => "formula-candidate",
            "template" => "interpretation-template",
            "evidence" => "semantic-evidence-candidate",
            _ => return Err("unknown identity kind".into()),
        };
        let mut raw = [0u8; 16];
        root.read_exact(&mut entropy, &mut raw)?;
        let token = raw.iter().map(|b| format!("{b:02x}")).collect::<String>();
        rows.push(json!({"kind":k,"binding":b,"id":format!("tos.annotation.{prefix}.sid-{token}")}))
    }
    write(
        root,
        &reference,
        &bytes(
            &json!({"schema_version":"tos_eternal_return_candidate_identity_issuance_v1","identity_policy":"opaque-id-independent-of-source-text-label-translation-and-current-interpretation","issued_at":plan["frozen_at"],"records":rows}),
            true,
        )?,
        0o644,
    )
}
fn fill_template(value: &mut Value, annotation: &str, ids: &Ids) -> Result<()> {
    match value {
        Value::String(v) => {
            if v == "@annotation" {
                *v = annotation.into()
            } else if let Some(raw) = v.strip_prefix("@id:") {
                let (k, b) = raw
                    .split_once('|')
                    .ok_or("bad static identity placeholder")?;
                *v = id(ids, k, b)?
            }
        }
        Value::Array(rows) => {
            for row in rows {
                fill_template(row, annotation, ids)?
            }
        }
        Value::Object(m) => {
            for row in m.values_mut() {
                fill_template(row, annotation, ids)?
            }
        }
        _ => {}
    }
    Ok(())
}
fn build_analysis(root: &ResearchExecution, units: &[Value], ids: &Ids) -> Result<Value> {
    let t = templates();
    let annotation = id(ids, "annotation", "eternal-return-dossier-v1")?;
    let mut evidence = Vec::new();
    let mut private_evidence = Vec::new();
    let mut exclusions = Vec::new();
    let mut claim_evidence = BTreeMap::<String, BTreeSet<String>>::new();
    let mut formula_evidence = BTreeMap::<String, BTreeSet<String>>::new();
    let mut formula_counts = BTreeMap::<String, usize>::new();
    for row in units {
        root.tick(1)?;
        let de = strings(&row["de"])?;
        let ru = strings(&row["ru"])?;
        let signals = signal_codes(&de, &ru);
        let klass = evidence_class(row, &signals)?;
        for f in a(&t["formulas"])? {
            root.tick(1)?;
            let code = s(&f[0])?;
            let language = s(&f[1][0])?;
            let pattern = strings(&f[1][1])?;
            let cnt = contiguous_count(if language == "de" { &de } else { &ru }, &pattern);
            if cnt > 0 {
                *formula_counts.entry(code.into()).or_default() += cnt;
                if klass.is_some() {
                    formula_evidence
                        .entry(code.into())
                        .or_default()
                        .insert(s(&row["alignment_id"])?.into());
                }
            }
        }
        let Some(klass) = klass else { continue };
        let eid = id(
            ids,
            "evidence",
            &format!("{klass}|{}", s(&row["alignment_id"])?),
        )?;
        let claims = claim_codes(row, &signals, klass)?;
        let mut claim_refs = Vec::new();
        for code in claims {
            claim_evidence
                .entry(code.clone())
                .or_default()
                .insert(eid.clone());
            claim_refs.push(id(ids, "template", &code)?);
        }
        let speaker_sensitive = ["p3.r2", "p3.r13", "p4.r19"].contains(&s(&row["reading"])?);
        let tracked = json!({"schema_version":"tos_zarathustra_eternal_return_evidence_candidate_v1","evidence_id":eid,"annotation_ref":annotation,"evidence_class":klass,"part":row["part"],"reading_ref":row["reading"],"alignment_ref":row["alignment_id"],"alignment_status":row["status"],"alignment_shape":row["shape"],"source_anchor_refs":row["source_anchor_refs"],"target_anchor_refs":row["target_anchor_refs"],"source_paragraph_unit_refs":row["source_paragraph_unit_refs"],"target_paragraph_unit_refs":row["target_paragraph_unit_refs"],"de_exact_sha256":text_digest(&row["de_text"])?,"ru_exact_sha256":text_digest(&row["ru_text"])?,"signal_codes":signals,"interpretation_template_refs":claim_refs,"positive_evidence_eligible":row["positive_evidence_eligible"],"speaker_sensitive":speaker_sensitive,"speaker_attribution_status":"unresolved_source_visible_review_required","accepted":false,"review_refs":[],"graph_effect":false,"canon_effect":false,"source_text_included":false});
        let mut private = tracked.clone();
        private["de_text"] = row["de_text"].clone();
        private["ru_text"] = row["ru_text"].clone();
        private["de_tokens"] = row["de"].clone();
        private["ru_tokens"] = row["ru"].clone();
        private["exclusion_code"] = json!(exclusion_code(row)?);
        evidence.push(tracked);
        private_evidence.push(private);
        if klass == "excluded" {
            exclusions.push(json!({"schema_version":"tos_zarathustra_eternal_return_exclusion_v1","evidence_ref":eid,"annotation_ref":annotation,"exclusion_code":exclusion_code(row)?,"reading_ref":row["reading"],"alignment_ref":row["alignment_id"],"accepted":false,"review_refs":[],"semantic_equivalence_asserted":false,"graph_effect":false,"canon_effect":false}));
        }
    }
    for rule in a(&t["method_controls"])? {
        root.tick(1)?;
        exclusions.push(json!({"schema_version":"tos_zarathustra_eternal_return_exclusion_v1","control_kind":"method_rule","control_code":rule[0],"rationale":rule[1],"evidence_ref":null,"annotation_ref":annotation,"accepted":false,"review_refs":[],"semantic_equivalence_asserted":false,"graph_effect":false,"canon_effect":false}));
    }
    let mut formulas = Vec::new();
    let mut private_formulas = Vec::new();
    for f in a(&t["formulas"])? {
        root.tick(1)?;
        let code = s(&f[0])?;
        let language = &f[1][0];
        let pattern = strings(&f[1][1])?;
        let fid = id(ids, "formula", code)?;
        let refs: Vec<String> = formula_evidence
            .get(code)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default();
        let cnt = *formula_counts.get(code).unwrap_or(&0);
        formulas.push(json!({"schema_version":"tos_zarathustra_formula_candidate_v1","formula_candidate_id":fid,"annotation_ref":annotation,"formula_code":code,"language":language,"token_width":pattern.len(),"occurrence_count":cnt,"aligned_evidence_unit_count":refs.len(),"alignment_refs":refs,"status":if cnt>=3{"proposed"}else{"ambiguous"},"accepted":false,"review_refs":[],"source_text_included":false,"graph_effect":false,"canon_effect":false}));
        private_formulas.push(json!({"formula_candidate_id":fid,"formula_code":code,"language":language,"normalized_tokens":pattern,"display_formula":pattern.join(" "),"occurrence_count":cnt,"alignment_refs":refs}));
    }
    let mut claims = Vec::new();
    for codev in a(&t["claim_order"])? {
        root.tick(1)?;
        let code = s(codev)?;
        let spec = &t["claim_specs"][code];
        let refs: Vec<String> = claim_evidence
            .get(code)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default();
        claims.push(json!({"schema_version":"tos_zarathustra_interpretation_template_v1","interpretation_template_id":id(ids,"template",code)?,"annotation_ref":annotation,"claim_code":code,"claim_kind":spec["kind"],"candidate_statement":spec["statement"],"status":if refs.is_empty(){json!("deferred")}else{spec["status"].clone()},"evidence_refs":refs,"accepted":false,"review_refs":[],"human_judgment":false,"materialized_claim":false,"semantic_fact_asserted":false,"graph_effect":false,"canon_effect":false}));
    }
    let mut anchors = BTreeSet::new();
    for row in &evidence {
        root.tick(1)?;
        for k in ["source_anchor_refs", "target_anchor_refs"] {
            for r in a(&row[k])? {
                anchors.insert(s(r)?.to_owned());
            }
        }
    }
    let mut concept = t["concept"].clone();
    fill_template(&mut concept, &annotation, ids)?;
    concept["anchor_refs"] = json!(anchors);
    Ok(
        json!({"concept":concept,"evidence":evidence,"formulas":formulas,"claims":claims,"exclusions":exclusions,"private":{"schema_version":"tos_zarathustra_eternal_return_private_analysis_v1","annotation_id":annotation,"content_posture":"private_exact_source_return_and_mutable_analysis_not_semantic_authority","evidence":private_evidence,"formulas":private_formulas,"claim_specs":t["claim_specs"]}}),
    )
}
pub struct Prepared {
    pub outputs: BTreeMap<String, Vec<u8>>,
    pub private: Vec<u8>,
    pub summary: Value,
}
pub fn prepare(root: &ResearchExecution, units: &[Value]) -> Result<Prepared> {
    let selected = select_concept_plan(root, &route("plan.v1.json"))?;
    verify_inputs(root, &selected.value)?;
    prepare_selected(root, units, &selected)
}
fn prepare_selected(
    root: &ResearchExecution,
    units: &[Value],
    selected: &SelectedConceptPlan,
) -> Result<Prepared> {
    let plan = &selected.value;
    let bindings = identity_bindings(root, units)?;
    let ids = identity_map(root, &bindings)?;
    let analysis = build_analysis(root, units, &ids)?;
    let evidence = a(&analysis["evidence"])?;
    let formulas = a(&analysis["formulas"])?;
    let claims = a(&analysis["claims"])?;
    let exclusions = a(&analysis["exclusions"])?;
    let annotation = s(&analysis["concept"]["annotation_id"])?;
    let status_counts = count(evidence, "evidence_class")?;
    let mut source_anchors = BTreeSet::new();
    let mut target_anchors = BTreeSet::new();
    let mut part_counts = BTreeMap::<String, usize>::new();
    let mut bilingual = 0;
    for row in evidence {
        root.tick(1)?;
        *part_counts.entry(row["part"].to_string()).or_default() += 1;
        for r in a(&row["source_anchor_refs"])? {
            source_anchors.insert(s(r)?.to_owned());
        }
        for r in a(&row["target_anchor_refs"])? {
            target_anchors.insert(s(r)?.to_owned());
        }
        if !a(&row["source_anchor_refs"])?.is_empty() && !a(&row["target_anchor_refs"])?.is_empty()
        {
            bilingual += 1
        }
    }
    let mut core_readings = vec!["p3.r2", "p3.r13", "p3.r16", "p4.r19"];
    core_readings.sort();
    let t = templates();
    let gap_refs: Vec<Value> = a(&t["alignment_gaps"])?
        .iter()
        .map(|r| r["alignment_ref"].clone())
        .collect();
    let coverage = json!({"schema_version":"tos_zarathustra_eternal_return_coverage_receipt_v1","annotation_ref":annotation,"corpus_parts_scanned":4,"alignment_units_scanned":units.len(),"selected_evidence_unit_count":evidence.len(),"evidence_class_counts":status_counts,"part_counts":part_counts,"reading_counts":count(evidence,"reading_ref")?,"distinct_source_anchor_count":source_anchors.len(),"distinct_target_anchor_count":target_anchors.len(),"core_readings_declared":core_readings,"negative_control_count":exclusions.len(),"source_return_verified_during_build":true,"bilingual_selected_evidence_count":bilingual,"one_sided_scope_residue_count":evidence.len()-bilingual,"de_ru_parallel_evidence_complete":bilingual==evidence.len(),"parallel_coverage_posture":"preserve one-sided alignment residue without treating it as bilingual support","independent_alignment_gap_count":a(&t["alignment_gaps"] )?.len(),"independent_alignment_gap_refs":gap_refs,"unselected_units_are_not_negative_findings":true});
    let summary = json!({"schema_version":"tos_zarathustra_eternal_return_candidate_summary_v1","annotation_id":annotation,"evidence_unit_count":evidence.len(),"evidence_class_counts":status_counts,"formula_candidate_count":formulas.len(),"interpretation_template_count":claims.len(),"exclusion_count":exclusions.len(),"accepted_candidate_count":0,"human_review_count":0,"sign_identity_asserted":false,"concept_identity_asserted":false,"semantic_fact_asserted":false,"translation_equivalence_asserted":false,"graph_effect":false,"canon_effect":false});
    let mut promotion = t["promotion"].clone();
    fill_template(&mut promotion, annotation, &ids)?;
    promotion["ready_for_source_visible_semantic_review"] =
        json!(status_counts["core"].as_u64().unwrap_or(0) > 0 && !exclusions.is_empty());
    let mut output_refs: Vec<String> = OUTPUTS.iter().map(|(_, n)| route(n)).collect();
    output_refs.push(private_ref());
    let provenance = vec![
        json!({"schema_version":"tos_zarathustra_eternal_return_provenance_event_v1","event_id":"tos.event.zarathustra-eternal-return-concept-candidate-v1.plan","event_type":"plan_frozen","event_at":plan["frozen_at"],"input_refs":selected_input_refs(root,selected)?,"output_refs":[selected.reference],"authority_effect":"none"}),
        json!({"schema_version":"tos_zarathustra_eternal_return_provenance_event_v1","event_id":"tos.event.zarathustra-eternal-return-concept-candidate-v1.build","event_type":"candidate_dossier_built","event_at":plan["frozen_at"],"input_refs":[selected.reference,route("identity-issuance.v1.json"),format!("{WORK}/{ALIGN}/manifest.v1.json")],"output_refs":output_refs,"authority_effect":"candidate_only_zero_graph_and_canon_effect"}),
    ];
    let mut outputs = BTreeMap::new();
    outputs.insert(route(OUTPUTS[0].1), bytes(&analysis["concept"], true)?);
    for (n, rows) in [
        (OUTPUTS[1].1, evidence),
        (OUTPUTS[2].1, formulas),
        (OUTPUTS[3].1, claims),
        (OUTPUTS[4].1, exclusions),
    ] {
        outputs.insert(route(n), lines(rows)?);
    }
    for (n, v) in [
        (OUTPUTS[5].1, &promotion),
        (OUTPUTS[6].1, &coverage),
        (OUTPUTS[7].1, &summary),
    ] {
        outputs.insert(route(n), bytes(v, true)?);
    }
    outputs.insert(route(OUTPUTS[9].1), lines(&provenance)?);
    let mut audit = t["audit"].clone();
    fill_template(&mut audit, annotation, &ids)?;
    outputs.insert(route(OUTPUTS[8].1), bytes(&audit, true)?);
    let private = bytes(&analysis["private"], true)?;
    let artifacts: Vec<Value> = OUTPUTS
        .iter()
        .filter(|(role, _)| *role != "manifest")
        .map(|(role, n)| json!({"role":role,"ref":route(n),"sha256":digest(&outputs[&route(n)])}))
        .collect();
    let manifest = json!({"schema_version":"tos_zarathustra_eternal_return_candidate_manifest_v1","route_id":"zarathustra-eternal-return-concept-candidate-v1","plan_ref":selected.reference,"plan_sha256":selected.digest,"identity_issuance_ref":route("identity-issuance.v1.json"),"identity_issuance_sha256":digest(&read(root,&route("identity-issuance.v1.json"))?),"generator_ref":GENERATOR,"generator_sha256":RECIPE_SHA256,"artifacts":artifacts,"private_artifact":{"ref":private_ref(),"sha256":digest(&private),"mode":"0600","tracked":false},"accepted_candidate_count":0,"human_review_count":0,"graph_effect":false,"canon_effect":false});
    outputs.insert(route(OUTPUTS[10].1), bytes(&manifest, true)?);
    root.tick(private.len() as u64 + outputs.values().map(|v| v.len() as u64).sum::<u64>())?;
    Ok(Prepared {
        outputs,
        private,
        summary,
    })
}
pub fn run(root: &Path, args: &[String]) -> Result<Value> {
    let execution = ResearchExecution::new(root, 180)?;
    run_scoped(&execution, args)
}
pub fn run_scoped(root: &ResearchExecution, args: &[String]) -> Result<Value> {
    let mut mode = None;
    let mut issuance = false;
    let mut plan_ref = route("plan.v1.json");
    let mut seen_plan_ref = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        root.tick(1)?;
        match arg.as_str() {
            "--build" | "--check" | "--preview" => {
                if mode.replace(arg.as_str()).is_some() {
                    return Err("exactly one mode required".into());
                }
            }
            "--issue-identities" => issuance = true,
            "--plan-ref" => {
                if seen_plan_ref {
                    return Err("duplicate --plan-ref".into());
                }
                seen_plan_ref = true;
                root.tick(1)?;
                plan_ref = args.next().ok_or("--plan-ref needs a reference")?.clone();
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    let mode = mode.ok_or("one of --build, --check, --preview is required")?;
    if issuance && mode != "--build" {
        return Err("--issue-identities is valid only with --build".into());
    }
    let selected = select_concept_plan(root, &plan_ref)?;
    if selected.custom && issuance {
        return Err("custom profile cannot issue identities".into());
    }
    verify_inputs(root, &selected.value)?;
    let units = hydrate_units(root)?;
    let bindings = identity_bindings(root, &units)?;
    if selected.custom {
        identity_map(root, &bindings)?;
    }
    if mode == "--preview" {
        return Ok(json!({"identity_count":bindings.len(),"unit_count":units.len()}));
    }
    if issuance {
        issue(root, &bindings, &selected.value)?
    }
    let prepared = prepare_selected(root, &units, &selected)?;
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
    fn contiguous_formula_counts_overlap() {
        let words = vec![
            "ewig".into(),
            "wieder".into(),
            "ewig".into(),
            "wieder".into(),
        ];
        assert_eq!(
            contiguous_count(&words, &["ewig".into(), "wieder".into()]),
            2
        );
        assert_eq!(contiguous_count(&words, &vec!["x".into(); 5]), 0);
    }
    #[test]
    fn local_returns_are_excluded_even_with_recurrence_signal() {
        let row = json!({"reading":"p1.r1","de":["wieder","mensch"],"ru":[]});
        let sig = signal_codes(&strings(&row["de"]).unwrap(), &[]);
        assert_eq!(evidence_class(&row, &sig).unwrap(), Some("excluded"));
    }
    #[test]
    fn circle_and_eternity_outside_core_do_not_become_core() {
        let row = json!({"reading":"p1.r2","de":["ewigkeit","ring"],"ru":[]});
        let sig = signal_codes(&strings(&row["de"]).unwrap(), &[]);
        assert_eq!(evidence_class(&row, &sig).unwrap(), Some("ambiguous"));
        let row = json!({"reading":"p3.r2","de":["wiederkehr","alle"],"ru":[]});
        let sig = signal_codes(&strings(&row["de"]).unwrap(), &[]);
        assert_eq!(evidence_class(&row, &sig).unwrap(), Some("core"));
    }
    #[test]
    fn no_semantic_admission_in_authored_templates() {
        let t = templates();
        assert_eq!(t["concept"]["review_status"], "unreviewed");
        assert_eq!(t["promotion"]["ready_for_canon"], false);
        assert_eq!(
            t["claim_specs"]["amor_fati_cross_work"]["status"],
            "deferred"
        );
    }
}
