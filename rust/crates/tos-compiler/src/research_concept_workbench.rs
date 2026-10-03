//! Complete candidate-only concept producer. Exact text remains private.
use crate::research_execution::ResearchExecution;
use regex::Regex;
use rusqlite::params;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
};
use tos_foundation::{Digest256, JsonLimits, JsonMode, parse_json};
use tos_validation::{FormatProfile, SchemaBackendProbe, SchemaResource};
#[path = "research_concept_workbench_render.rs"]
mod render;
#[path = "research_concept_workbench_source.rs"]
mod source;
#[path = "research_concept_workbench_sql.rs"]
mod sql;
type Result<T> = std::result::Result<T, String>;
type Rows = Vec<Value>;
type Forms = Vec<Value>;
type Ids = BTreeMap<(String, String), String>;
fn read(root: &ResearchExecution, p: &str) -> Result<Vec<u8>> {
    root.read(p)
}
pub const WORK: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra";
pub const ROUTE: &str = "ToS/candidate-intake/zarathustra/concept-workbench-v1";
const GENERATOR: &str = "scripts/build_zarathustra_concept_workbench_v1.py";
// Selected maintained rendering-recipe identity, separate from native execution.
const GENERATOR_SHA: &str = "c60ef9660b5bbfbc06a3ce2f710f45bd5761f8f8ced55c4352c6f16e90a57a2d";
const QUERY_SHA: &str = "ca9bb4c046f306acfcbbdd06d1bf95306d3fad1444babfc4cba735276e9832ec";
const WORD_SHA: &str = "fba47c5368663fe6877b725174ee80b9118294b8af89a78ed2ca041d9839f9fb";
const QUERY: &str = "scripts/query_zarathustra_concept_workbench_v1.py";
const WORD: &str = "scripts/prepare_zarathustra_word_analysis_v1.py";
fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
fn n(v: &Value, k: &str) -> u64 {
    v[k].as_u64().unwrap_or(0)
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn hash(raw: impl AsRef<[u8]>) -> String {
    Digest256::of_bytes(raw.as_ref()).to_hex()
}
fn limits() -> Result<JsonLimits> {
    JsonLimits::new(64 * 1024 * 1024, 128, 4_000_000, 4300).map_err(|e| e.to_string())
}
fn load(root: &ResearchExecution, p: &str) -> Result<Value> {
    let raw = read(root, p)?;
    parse_json(&raw, JsonMode::PublishedStrict, limits()?).map_err(|e| format!("{p}: {e}"))?;
    serde_json::from_slice(&raw).map_err(|e| format!("{p}: {e}"))
}
fn plan_input_refs(root: &ResearchExecution, raw: &[u8]) -> Result<Vec<Value>> {
    let parsed =
        parse_json(&raw, JsonMode::PublishedStrict, limits()?).map_err(|e| e.to_string())?;
    let rows = parsed
        .root()
        .object_get("inputs")
        .and_then(|v| v.as_object())
        .ok_or("plan inputs")?;
    let refs = rows
        .iter()
        .map(|(_, v)| {
            v.object_get("ref")
                .and_then(|v| v.as_str())
                .map(|s| json!(s))
                .ok_or("plan input reference".into())
        })
        .collect::<Result<Vec<_>>>()?;
    root.check()?;
    Ok(refs)
}
const DEFAULT_PLAN_SHA: &str = "66558b6f0046c82417204a7b78945035e17c0ae780ff70ef736df43d0882cad6";
struct ConceptPlan {
    reference: String,
    digest: String,
    raw: Vec<u8>,
    value: Value,
    custom: bool,
}
fn validate_concept_plan_semantics(
    value: &Value,
    original: &Value,
    default_ref: &str,
) -> Result<()> {
    let lineage = &value["input_profile_lineage"];
    if !lineage.is_object()
        || lineage["profile_version"].as_u64().is_none_or(|n| n < 2)
        || lineage["supersedes_plan_ref"] != default_ref
        || lineage["supersedes_plan_sha256"] != DEFAULT_PLAN_SHA
        || value["plan_id"].as_str().is_none_or(|id| id.is_empty())
        || value["plan_id"] == original["plan_id"]
        || value["status"] != "proposed-technical-input-profile-successor"
    {
        return Err("Concept technical plan requires distinct identity, proposal status and exact predecessor".into());
    }
    let mut comparable = value.clone();
    let object = comparable
        .as_object_mut()
        .ok_or("Concept plan object required")?;
    object.remove("input_profile_lineage");
    for key in ["plan_id", "status"] {
        object.insert(key.into(), original[key].clone());
    }
    for label in [
        "paragraph_alignment_manifest",
        "parallel_lexical_manifest",
        "morphology_theme_manifest",
        "eternal_return_review_preparation_manifest",
        "german_exact_occurrence_database",
    ] {
        let record = &mut object.get_mut("inputs").ok_or("Concept inputs required")?[label];
        let digest = s(record, "sha256");
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("Concept technical input SHA256 required".into());
        }
        record
            .as_object_mut()
            .ok_or("Concept input record required")?
            .insert("sha256".into(), original["inputs"][label]["sha256"].clone());
    }
    if comparable != *original {
        return Err(
            "Concept technical plan changes frozen semantics, input references or nonselected pins"
                .into(),
        );
    }
    Ok(())
}
fn select_concept_plan(root: &ResearchExecution, reference: &str) -> Result<ConceptPlan> {
    use std::os::unix::fs::MetadataExt;
    let default_ref = format!("{ROUTE}/plan.v1.json");
    let bounded = |reference: &str, private: bool| -> Result<Vec<u8>> {
        let mut file = root.source_file(reference, 64 * 1024)?;
        let meta = file.metadata().map_err(|e| e.to_string())?;
        if private {
            let carrier = root
                .root_directory()
                .metadata()
                .map_err(|e| e.to_string())?;
            let uid = unsafe { libc::geteuid() };
            if carrier.uid() != uid
                || carrier.permissions().mode() & 0o7777 != 0o700
                || meta.uid() != uid
                || meta.permissions().mode() & 0o7777 != 0o600
            {
                return Err("custom Concept plan requires owned 0700 carrier and 0600 plan".into());
            }
        }
        let bytes = root.read_file(&mut file, 64 * 1024)?;
        root.verify_file_unchanged(&file, &meta)?;
        root.check()?;
        parse_json(&bytes, JsonMode::PublishedStrict, limits()?).map_err(|e| e.to_string())?;
        root.check()?;
        Ok(bytes)
    };
    let original_raw = bounded(&default_ref, false)?;
    if hash(&original_raw) != DEFAULT_PLAN_SHA {
        return Err("frozen Concept plan drift".into());
    }
    let original: Value = serde_json::from_slice(&original_raw).map_err(|e| e.to_string())?;
    let custom = reference != default_ref;
    let raw = if custom {
        bounded(reference, true)?
    } else {
        original_raw
    };
    let value: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    root.check()?;
    if custom {
        root.tick(5)?;
        validate_concept_plan_semantics(&value, &original, &default_ref)?;
        root.check()?;
    }
    let digest = hash(&raw);
    root.check()?;
    Ok(ConceptPlan {
        reference: reference.into(),
        digest,
        raw,
        value,
        custom,
    })
}
fn verify_plan_input(root: &ResearchExecution, r: &Value) -> Result<()> {
    let reference = s(r, "ref");
    let expected = s(r, "sha256");
    if file_hash(root, reference)? == expected {
        return Ok(());
    }
    if let Some(stem) = Path::new(reference).file_stem().and_then(|v| v.to_str()) {
        if reference.starts_with("scripts/") && reference.ends_with(".py") {
            let retained =
                format!("ToS/research-packets/retained-builder-inputs/{stem}/{expected}.py");
            if file_hash(root, &retained)? == expected {
                return Ok(());
            }
        }
    }
    Err(format!("input drift: {reference}"))
}
fn lines(root: &ResearchExecution, p: &str) -> Result<Rows> {
    let raw = String::from_utf8(read(root, p)?).map_err(|e| e.to_string())?;
    raw.lines()
        .map(|l| serde_json::from_str(l).map_err(|e| format!("{p}: {e}")))
        .collect()
}
fn file_hash(root: &ResearchExecution, p: &str) -> Result<String> {
    let mut file = root.source_file(p, 256 * 1024 * 1024)?;
    root.hash_file(&mut file, 256 * 1024 * 1024)
}
// Borrow source Values: Python sort_keys=True without duplicating the whole DOM.
struct OrderedJson<'a>(&'a Value);
impl serde::Serialize for OrderedJson<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::{SerializeMap, SerializeSeq};
        match self.0 {
            Value::Object(object) => {
                let ordered: BTreeMap<_, _> = object.iter().collect();
                let mut map = serializer.serialize_map(Some(ordered.len()))?;
                for (key, value) in ordered {
                    map.serialize_entry(key, &OrderedJson(value))?;
                }
                map.end()
            }
            Value::Array(array) => {
                let mut seq = serializer.serialize_seq(Some(array.len()))?;
                for value in array {
                    seq.serialize_element(&OrderedJson(value))?;
                }
                seq.end()
            }
            value => serde::Serialize::serialize(value, serializer),
        }
    }
}
fn encode(v: &Value, pretty: bool) -> Vec<u8> {
    let ordered = OrderedJson(v);
    let v = &ordered;
    let mut b = if pretty {
        serde_json::to_vec_pretty(v).unwrap()
    } else {
        serde_json::to_vec(v).unwrap()
    };
    b.push(b'\n');
    b
}
fn jsonl(rows: &[Value]) -> Vec<u8> {
    rows.iter().flat_map(|v| encode(v, false)).collect()
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct ValueOrd(Value);
impl Ord for ValueOrd {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        match (&self.0, &o.0) {
            (Value::Number(a), Value::Number(b)) => a.as_u64().cmp(&b.as_u64()),
            _ => self.0.as_str().cmp(&o.0.as_str()),
        }
    }
}
impl PartialOrd for ValueOrd {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl From<Value> for ValueOrd {
    fn from(v: Value) -> Self {
        Self(v)
    }
}
fn unique(items: impl Iterator<Item = Value>) -> Value {
    json!(
        items
            .map(ValueOrd)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>()
    )
}
fn count(rows: &[Value], k: &str) -> Value {
    let mut c = BTreeMap::<String, u64>::new();
    for r in rows {
        *c.entry(s(r, k).into()).or_default() += 1;
    }
    json!(c)
}
fn fields(v: &Value, keys: &[&str]) -> Value {
    let mut o = serde_json::Map::new();
    for k in keys {
        o.insert((*k).into(), v[*k].clone());
    }
    Value::Object(o)
}
fn binding(request: &Value) -> String {
    format!(
        "{}|v{}",
        s(request, "request_identity_key"),
        n(request, "request_version")
    )
}
fn local(request: &Value, b: &str) -> String {
    format!("{}|{b}", binding(request))
}
fn event(request: &Value) -> String {
    format!(
        "tos.event.zarathustra-concept-workbench-v1.build.sid-{}",
        &hash(binding(request))[..32]
    )
}
fn form_binding(v: &Value) -> String {
    format!(
        "{}|{}|{}|{}",
        s(v, "selection_kind"),
        s(v, "language"),
        s(v, "analysis_key_sha256"),
        s(v, "probe_normalized_sha256")
    )
}
fn validate(root: &ResearchExecution, reference: &str, value: &Value) -> Result<()> {
    let raw = read(root, reference)?;
    let schema: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let uri = s(&schema, "$id");
    let probe = SchemaBackendProbe::new(
        vec![SchemaResource {
            uri: uri.into(),
            raw,
        }],
        FormatProfile::LegacyPythonObserved20260923,
    )
    .map_err(|e| format!("schema: {e:?}"))?;
    if !probe
        .is_valid_raw(uri, &encode(value, false))
        .map_err(|e| format!("schema: {e:?}"))?
    {
        return Err(format!("schema invalid: {reference}"));
    }
    Ok(())
}
#[derive(Clone)]
struct Config {
    request_ref: String,
    issuance: String,
    private_request: String,
    private_db: String,
    outputs: Vec<(String, String)>,
}
impl Config {
    fn new(request_ref: String, r: &Value) -> Self {
        let default = request_ref == format!("{ROUTE}/requests/fate.concept-request.v2.json")
            && s(r, "request_key") == "fate";
        let scope = if default {
            "fate".into()
        } else {
            format!(
                "{}-v{}-{}",
                s(r, "request_key"),
                n(r, "request_version"),
                s(r, "request_identity_key")
                    .rsplit('-')
                    .next()
                    .unwrap_or("")
            )
        };
        let base = format!("{ROUTE}/outputs/{scope}");
        let private =
            format!("{WORK}/gold-sets/foundation-pilot-v1/local-content/concept-workbench-v1");
        let mut outputs = vec![];
        for (k, name) in [
            ("contexts", "context-unit-spine.v1.jsonl"),
            ("speakers", "speaker-state-candidates.v1.jsonl"),
            ("all_forms", "all-form-coverage.v1.json"),
            ("speaker_worklist", "speaker-exception-worklist.v1.json"),
        ] {
            outputs.push((k.into(), format!("{ROUTE}/{name}")));
        }
        for (k, name) in [
            ("concept", "concept-candidate.v1.json"),
            ("forms", "form-family-candidates.v1.jsonl"),
            ("occurrences", "occurrence-spine.v1.jsonl"),
            ("relations", "relation-candidates.v1.jsonl"),
            ("gaps", "gap-ledger.v1.jsonl"),
            ("exclusions", "exclusion-ledger.v1.jsonl"),
            ("graph", "candidate-graph.v1.json"),
            ("coverage", "coverage-receipt.v1.json"),
            ("english_tasks", "english-on-demand-worklist.v1.jsonl"),
        ] {
            outputs.push((k.into(), format!("{base}/{name}")));
        }
        for (k, name) in [
            ("summary", "summary.v1.json"),
            ("provenance", "provenance.jsonl"),
            ("manifest", "manifest.v1.json"),
        ] {
            outputs.push((
                k.into(),
                format!("{}/{name}", if default { ROUTE } else { &base }),
            ));
        }
        Self {
            request_ref,
            issuance: format!(
                "{}/identity-issuance.v1.json",
                if default { ROUTE } else { &base }
            ),
            private_request: format!("{private}/requests/{scope}.request-analysis.v1.json"),
            private_db: format!("{private}/workbench-index.v1.sqlite3"),
            outputs,
        }
    }
    fn output(&self, k: &str) -> &str {
        &self.outputs.iter().find(|(key, _)| key == k).unwrap().1
    }
}
// Orthographic and reversible signatures are proposals, never lemma assignments.
pub fn signatures(value: &str, language: &str) -> BTreeSet<String> {
    let mut base = crate::research_parallel_lexical::base_key(value);
    if language == "de" {
        base = base.replace('ä', "a").replace('ö', "o").replace('ü', "u");
    }
    let suffixes: &[&str] = if language == "de" {
        &[
            "ern", "est", "em", "en", "er", "es", "te", "st", "et", "e", "n", "s", "t",
        ]
    } else {
        &[
            "иями",
            "ями",
            "ами",
            "ого",
            "ему",
            "ому",
            "ими",
            "ыми",
            "аться",
            "яться",
            "ить",
            "ать",
            "ять",
            "ешь",
            "ишь",
            "ете",
            "ите",
            "ут",
            "ют",
            "ат",
            "ят",
            "ла",
            "ли",
            "ло",
            "его",
            "ой",
            "ей",
            "ий",
            "ый",
            "ая",
            "яя",
            "ую",
            "юю",
            "ов",
            "ев",
            "ам",
            "ям",
            "ах",
            "ях",
            "ом",
            "ем",
            "ою",
            "ею",
            "ы",
            "и",
            "а",
            "я",
            "у",
            "ю",
            "е",
            "о",
            "ь",
        ]
    };
    let mut out = BTreeSet::from([base.clone()]);
    for suffix in suffixes {
        if let Some(stem) = base.strip_suffix(suffix) {
            if stem.chars().count() >= 4 {
                out.insert(stem.into());
            }
        }
    }
    out
}
fn form_inventory(root: &ResearchExecution, occ: &[Value]) -> Result<Forms> {
    let mut rows = Vec::<Value>::new();
    let mut keys = BTreeMap::<(String, String), usize>::new();
    for o in occ.iter().filter(|v| v["in_work_scope"] == true) {
        root.tick(1)?;
        let k = (s(o, "language").into(), s(o, "analysis_key").into());
        let i=*keys.entry(k).or_insert_with(||{rows.push(json!({"language":o["language"],"analysis_key":o["analysis_key"],"analysis_key_sha256":o["analysis_key_sha256"],"occurrence_count":0,"exact_hashes":[],"normalized_hashes":[],"parts":[],"readings":[]}));rows.len()-1});
        let r = &mut rows[i];
        r["occurrence_count"] = json!(n(r, "occurrence_count") + 1);
        for (dest, src) in [
            ("exact_hashes", "exact_sha256"),
            ("normalized_hashes", "normalized_sha256"),
            ("parts", "part"),
            ("readings", "reading_ref"),
        ] {
            root.tick(1)?;
            if !o[src].is_null() {
                let mut a = arr(&r[dest]).to_vec();
                if !a.contains(&o[src]) {
                    a.push(o[src].clone());
                    r[dest] = unique(a.into_iter());
                }
            }
        }
    }
    Ok(rows)
}
fn select_forms(
    root: &ResearchExecution,
    request: &Value,
    forms: &[Value],
) -> Result<(Forms, Rows)> {
    let mut selected = Vec::<Value>::new();
    let mut indices = BTreeMap::new();
    let mut gaps = vec![];
    let ranks = [
        "direct_exact",
        "direct_morphology",
        "semantic_exact",
        "semantic_morphology",
    ];
    for (field, tier) in [
        ("lexical_probes", "direct"),
        ("semantic_neighbor_probes", "semantic_neighbor"),
    ] {
        root.tick(1)?;
        for lang in ["de", "ru"] {
            root.tick(1)?;
            for display in arr(&request[field][lang]) {
                root.tick(1)?;
                let text = display.as_str().unwrap();
                let norm = if lang == "ru" {
                    crate::research_parallel_lexical::ru_key(text)
                } else {
                    crate::research_parallel_lexical::base_key(text)
                };
                let sig = signatures(&norm, lang);
                let mut exact_seen = false;
                for form in forms.iter().filter(|v| s(v, "language") == lang) {
                    root.tick(1)?;
                    let key = s(form, "analysis_key");
                    let exact = key == norm;
                    let mut method = "";
                    if exact {
                        method = "exact_analysis_form";
                    } else {
                        let variants: Vec<_> = if lang == "de" && key.contains('-') {
                            std::iter::once(key).chain(key.split('-')).collect()
                        } else {
                            vec![key]
                        };
                        if variants
                            .iter()
                            .any(|v| !signatures(v, lang).is_disjoint(&sig))
                        {
                            method = "shared_reversible_signature";
                        } else if lang == "de" && norm.ends_with("niss") {
                            let stem = &norm[..norm.len() - 4];
                            if stem.chars().count() >= 4 && key.starts_with(stem) {
                                method = "historical_niss_prefix_candidate";
                            }
                        }
                    }
                    if method.is_empty() {
                        continue;
                    }
                    exact_seen |= exact;
                    let kind = format!(
                        "{}_{}",
                        if tier == "direct" {
                            "direct"
                        } else {
                            "semantic"
                        },
                        if exact { "exact" } else { "morphology" }
                    );
                    let mut r = form.clone();
                    r["selection_kind"] = json!(kind);
                    r["selection_method"] = json!(method);
                    r["probe_normalized_sha256"] = json!(hash(&norm));
                    r["probe_display"] = display.clone();
                    r["status"] = json!(if exact { "proposed" } else { "ambiguous" });
                    let fk = (lang.to_string(), key.to_string());
                    if let Some(&i) = indices.get(&fk) {
                        let old: &Value = &selected[i];
                        if ranks.iter().position(|x| *x == kind).unwrap()
                            < ranks
                                .iter()
                                .position(|x| *x == s(old, "selection_kind"))
                                .unwrap()
                        {
                            selected[i] = r;
                        }
                    } else {
                        indices.insert(fk, selected.len());
                        selected.push(r);
                    }
                }
                if !exact_seen {
                    gaps.push(json!({"schema_version":"tos_zarathustra_concept_probe_gap_v1","gap_code":"probe_exact_form_unobserved","tier":tier,"language":lang,"probe_normalized_sha256":hash(norm),"review_status":"unreviewed","accepted":false,"graph_effect":false,"canon_effect":false}));
                }
            }
        }
    }
    Ok((selected, gaps))
}
fn select_occurrences(root: &ResearchExecution, all: &[Value], forms: &[Value]) -> Result<Rows> {
    let mut local_counts = BTreeMap::<(String, String), u64>::new();
    let forms: BTreeMap<_, _> = forms
        .iter()
        .map(|v| ((s(v, "language"), s(v, "analysis_key")), v))
        .collect();
    let mut out = vec![];
    for o in all.iter().filter(|v| v["in_work_scope"] == true) {
        root.tick(1)?;
        let Some(f) = forms.get(&(s(o, "language"), s(o, "analysis_key"))) else {
            continue;
        };
        let key = if o["context_unit_ref"].is_null() {
            format!("unmapped:{}", s(o, "source_locator_sha256"))
        } else {
            s(o, "context_unit_ref").into()
        };
        let ordinal = local_counts
            .entry((s(o, "language").into(), key))
            .or_default();
        *ordinal += 1;
        let tier = if s(f, "selection_kind").starts_with("semantic") {
            "semantic_neighbor"
        } else {
            "direct_or_morphological"
        };
        let mut row = fields(
            o,
            &[
                "language",
                "part",
                "reading_ref",
                "unit_kind",
                "context_unit_ref",
                "witness_order",
                "token_ordinal",
                "existing_occurrence_ref",
                "analysis_key_sha256",
                "source_locator_sha256",
            ],
        );
        row["binding"] = json!(format!(
            "{tier}|{}|{}|{}",
            s(o, "language"),
            s(o, "existing_occurrence_ref"),
            s(o, "exact_sha256")
        ));
        row["occurrence_ordinal_within_context"] = json!(ordinal);
        row["surface_private"] = o["surface"].clone();
        row["exact_form_sha256"] = o["exact_sha256"].clone();
        row["form_binding"] = json!(form_binding(f));
        row["evidence_tier"] = json!(tier);
        row["selection_kind"] = f["selection_kind"].clone();
        row["status"] = json!(if s(f, "selection_kind").ends_with("exact") {
            "proposed"
        } else {
            "ambiguous"
        });
        out.push(row);
    }
    Ok(out)
}
fn speakers(root: &ResearchExecution, units: &[Value]) -> Result<Rows> {
    let de = [
        (
            "zarathustra",
            r"(?:sprach|antwortete|rief|sagte)\s+zarathustra|zarathustra\s+(?:sprach|antwortete|rief|sagte)",
        ),
        (
            "animals_eagle_and_serpent",
            r"(?:sprachen|antworteten|sagten)\s+(?:seine\s+)?thiere",
        ),
        (
            "dwarf",
            r"(?:sprach|antwortete|sagte|murmelte)\s+(?:der\s+)?zwerg",
        ),
        (
            "ugliest_man",
            r"(?:sprach|antwortete|sagte)\s+(?:der\s+)?hässlichste\s+mensch",
        ),
        (
            "soothsayer",
            r"(?:sprach|antwortete|sagte)\s+(?:der\s+)?wahrsager",
        ),
        (
            "magician",
            r"(?:sprach|antwortete|sagte)\s+(?:der\s+)?zauberer",
        ),
        (
            "shadow",
            r"(?:sprach|antwortete|sagte)\s+(?:der\s+)?schatten",
        ),
        (
            "kings",
            r"(?:sprachen|antworteten|sagten)\s+(?:die\s+)?könige",
        ),
        (
            "people_or_crowd",
            r"(?:sprach|sprachen|rief|riefen)\s+(?:das\s+)?volk",
        ),
    ];
    let ru = [
        (
            "zarathustra",
            r"(?:сказал|говорил|отвечал|воскликнул)\s+заратустра|заратустра\s+(?:сказал|говорил|отвечал|воскликнул)",
        ),
        (
            "animals_eagle_and_serpent",
            r"(?:сказали|говорили|отвечали)\s+(?:его\s+)?животн",
        ),
        (
            "dwarf",
            r"(?:сказал|говорил|ответил|пробормотал)\s+(?:карлик|карлика)",
        ),
        ("soothsayer", r"(?:сказал|говорил|ответил)\s+прорицатель"),
        ("magician", r"(?:сказал|говорил|ответил)\s+волшебник"),
        ("shadow", r"(?:сказала|говорила|ответила)\s+тень"),
        ("kings", r"(?:сказали|говорили|ответили)\s+цари"),
        ("people_or_crowd", r"(?:сказал|говорил|кричал)\s+народ"),
    ];
    let compile = |p: &[(&str, &str)]| {
        p.iter()
            .map(|(r, p)| Ok((r.to_string(), Regex::new(p).map_err(|e| e.to_string())?)))
            .collect::<Result<Vec<_>>>()
    };
    let de = compile(&de)?;
    let ru = compile(&ru)?;
    let mut ordered = units.to_vec();
    ordered.sort_by(|a, b| {
        (s(a, "language"), n(a, "witness_order")).cmp(&(s(b, "language"), n(b, "witness_order")))
    });
    let mut states = BTreeMap::<(String, String), String>::new();
    let mut out = vec![];
    for row in ordered {
        root.tick(1)?;
        let key = (
            s(&row, "language").to_string(),
            s(&row, "reading_ref").to_string(),
        );
        let state = states
            .get(&key)
            .cloned()
            .unwrap_or("zarathustra_or_external_narrator".into());
        let source_text = s(&row, "text");
        let text = tos_foundation::python_casefold_unicode16_v1(
            source_text,
            source_text.chars().count(),
            source_text.chars().count().saturating_mul(3),
            source_text.len().saturating_mul(3),
        )
        .map_err(|e| e.to_string())?
        .replace('\n', " ");
        let detected = if key.0 == "de" { &de } else { &ru }
            .iter()
            .find(|(_, re)| re.is_match(&text))
            .map(|(r, _)| r.clone());
        let (primary, alternatives, status, cue, group) = if s(&row, "unit_kind") == "verse_line" {
            (
                "song_or_performative_voice".into(),
                json!(["zarathustra", "named_or_personified_voice"]),
                "ambiguous",
                "witness_attested_verse_line",
                Some(format!("{}.{}.verse_voice", key.0, key.1)),
            )
        } else if let Some(role) = detected {
            states.insert(key.clone(), role.clone());
            (
                format!("mixed_external_narrator_and_{role}"),
                json!(["external_narrator", role]),
                "ambiguous",
                "explicit_speech_reporting_cue_inside_unit",
                Some(format!("{}.{}.explicit_transition.{role}", key.0, key.1)),
            )
        } else if state != "zarathustra_or_external_narrator" {
            (
                state,
                json!(["external_narrator"]),
                "proposed",
                "continuation_of_last_explicit_state_within_reading",
                None,
            )
        } else {
            (
                state,
                json!(["zarathustra", "external_narrator"]),
                "unresolved",
                "no_explicit_cue_or_prior_state_in_reading",
                Some(format!("{}.{}.unresolved_opening_state", key.0, key.1)),
            )
        };
        let mut r = fields(
            &row,
            &[
                "context_unit_ref",
                "language",
                "part",
                "reading_ref",
                "unit_kind",
                "witness_order",
            ],
        );
        r["binding"] = row["context_unit_ref"].clone();
        r["primary_role"] = json!(primary);
        r["alternative_roles"] = alternatives;
        r["attribution_status"] = json!(status);
        r["attribution_cue"] = json!(cue);
        r["exception_group"] = json!(group);
        for k in ["accepted", "human_judgment", "graph_effect", "canon_effect"] {
            root.tick(1)?;
            r[k] = json!(false);
        }
        out.push(r);
    }
    if out.len() != 7743 {
        return Err(format!("speaker population drift: {}", out.len()));
    }
    Ok(out)
}
fn english_tasks(
    root: &ResearchExecution,
    request: &Value,
    occ: &[Value],
    units: &[Value],
) -> Result<Rows> {
    let by: BTreeMap<_, _> = units
        .iter()
        .map(|u| (s(u, "context_unit_ref"), u))
        .collect();
    let mut members = BTreeMap::<String, BTreeSet<String>>::new();
    for u in units.iter().filter(|u| s(u, "language") == "ru") {
        root.tick(1)?;
        for link in arr(&u["alignment_links"]) {
            root.tick(1)?;
            members
                .entry(s(link, "alignment_ref").into())
                .or_default()
                .insert(s(u, "context_unit_ref").into());
        }
    }
    let mut out = vec![];
    for o in occ {
        root.tick(1)?;
        if s(o, "language") != s(&request["english_generation"], "source_language")
            || o["context_unit_ref"].is_null()
        {
            continue;
        }
        let unit = by[s(o, "context_unit_ref")];
        let alignment = unique(
            arr(&unit["alignment_links"])
                .iter()
                .map(|l| l["alignment_ref"].clone()),
        );
        let ru = unique(arr(&alignment).iter().flat_map(|a| {
            members
                .get(a.as_str().unwrap())
                .into_iter()
                .flatten()
                .map(|r| json!(r))
        }));
        out.push(json!({"binding":format!("english_on_demand|{}",s(o,"binding")),"source_occurrence_binding":o["binding"],"source_context_unit_ref":o["context_unit_ref"],"source_form_sha256":o["exact_form_sha256"],"source_surface_private":o["surface_private"],"alignment_refs":alignment,"russian_comparator_context_refs":ru,"required_views":request["english_generation"]["required_views"],"required_analysis_stages":request["english_generation"]["required_analysis_stages"]}));
    }
    Ok(out)
}
fn relation(
    kind: &str,
    left: Vec<String>,
    object_kind: &str,
    right: Vec<String>,
    status: &str,
    support: usize,
    binding: String,
) -> Value {
    json!({"binding":binding,"relation_type":kind,"subject_bindings":left,"object_kind":object_kind,"object_bindings":right,"status":status,"support":support})
}
fn relations(root: &ResearchExecution, occ: &[Value], units: &[Value]) -> Result<Rows> {
    let by: BTreeMap<_, _> = units
        .iter()
        .map(|u| (s(u, "context_unit_ref"), u))
        .collect();
    let mut out = BTreeMap::new();
    let mut add = |r: Value| {
        out.insert(s(&r, "binding").to_string(), r);
    };
    let mut parallel = BTreeMap::<String, BTreeMap<String, BTreeSet<String>>>::new();
    let mut reprises = BTreeMap::<(String, String, String), Vec<&Value>>::new();
    let mut direct = BTreeMap::<String, Vec<&Value>>::new();
    for o in occ {
        root.tick(1)?;
        let b = s(o, "binding");
        let kind = match s(o, "selection_kind") {
            "direct_exact" => "lexical_realization",
            "direct_morphology" => "morphological_realization",
            _ => "semantic_neighbor_candidate",
        };
        add(relation(
            kind,
            vec![b.into()],
            "concept_candidate",
            vec!["concept".into()],
            s(o, "status"),
            1,
            format!("{kind}|{b}|concept"),
        ));
        add(relation(
            "form_membership_candidate",
            vec![b.into()],
            "form_candidate",
            vec![s(o, "form_binding").into()],
            s(o, "status"),
            1,
            format!("form_membership_candidate|{b}|{}", s(o, "form_binding")),
        ));
        if !o["context_unit_ref"].is_null() {
            add(relation(
                "spoken_by_candidate",
                vec![b.into()],
                "speaker_candidate",
                vec![s(o, "context_unit_ref").into()],
                "ambiguous",
                1,
                format!("spoken_by_candidate|{b}|{}", s(o, "context_unit_ref")),
            ));
            for l in arr(&by[s(o, "context_unit_ref")]["alignment_links"]) {
                root.tick(1)?;
                parallel
                    .entry(s(l, "alignment_ref").into())
                    .or_default()
                    .entry(s(o, "language").into())
                    .or_default()
                    .insert(b.into());
            }
        }
        if !o["reading_ref"].is_null() {
            reprises
                .entry((
                    s(o, "language").into(),
                    s(o, "reading_ref").into(),
                    s(o, "form_binding").into(),
                ))
                .or_default()
                .push(o);
        }
        if s(o, "evidence_tier") == "direct_or_morphological" {
            direct.entry(s(o, "language").into()).or_default().push(o);
        }
    }
    for (aid, langs) in parallel {
        root.tick(1)?;
        let left = langs
            .get("de")
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        let right = langs
            .get("ru")
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        if !left.is_empty() && !right.is_empty() {
            let digest = hash(
                left.iter()
                    .chain(&right)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("|"),
            );
            let mut r = relation(
                "translation_parallel_candidate",
                left.clone(),
                "occurrence_candidate",
                right.clone(),
                "ambiguous",
                left.len().min(right.len()),
                format!("translation_parallel_candidate|{aid}|{digest}"),
            );
            r["alignment_ref"] = json!(aid);
            add(r);
        }
    } // Ordering of these groups is immaterial: final binding sort is canonical.
    for mut rows in reprises.into_values() {
        root.tick(1)?;
        rows.sort_by_key(|r| (n(r, "witness_order"), n(r, "token_ordinal")));
        for pair in rows.windows(2) {
            root.tick(1)?;
            let l = s(pair[0], "binding");
            let r = s(pair[1], "binding");
            add(relation(
                "same_reading_reprise",
                vec![l.into()],
                "occurrence_candidate",
                vec![r.into()],
                "proposed",
                1,
                format!("same_reading_reprise|{l}|{r}"),
            ));
        }
    }
    for mut rows in direct.into_values() {
        root.tick(1)?;
        rows.sort_by_key(|r| (n(r, "part"), n(r, "witness_order"), n(r, "token_ordinal")));
        for pair in rows.windows(2) {
            root.tick(1)?;
            let l = s(pair[0], "binding");
            let r = s(pair[1], "binding");
            add(relation(
                "precedes_in_work",
                vec![l.into()],
                "occurrence_candidate",
                vec![r.into()],
                "proposed",
                1,
                format!("precedes_in_work|{l}|{r}"),
            ));
        }
    }
    Ok(out.into_values().collect())
}
fn exclusions(
    root: &ResearchExecution,
    request: &Value,
    all: &[Value],
    selected: &[Value],
) -> Result<Rows> {
    let mut out = vec![];
    for control in arr(&request["negative_controls"]) {
        root.tick(1)?;
        let lang = s(control, "language");
        let norm = if lang == "ru" {
            crate::research_parallel_lexical::ru_key(s(control, "form"))
        } else {
            crate::research_parallel_lexical::base_key(s(control, "form"))
        };
        for o in all {
            root.tick(1)?;
            if !(o["in_work_scope"] == true
                && s(o, "language") == lang
                && s(o, "analysis_key") == norm)
            {
                continue;
            }
            out.push(json!({"schema_version":"tos_zarathustra_concept_exclusion_candidate_v1","control_code":control["control_code"],"language":lang,"existing_occurrence_ref":o["existing_occurrence_ref"],"context_unit_ref":o["context_unit_ref"],"exact_form_sha256":o["exact_sha256"],"exclusion_status":"request_declared_negative_control","review_status":"unreviewed","accepted":false,"graph_effect":false,"canon_effect":false}));
        }
    }
    let keys: BTreeSet<_> = selected
        .iter()
        .map(|f| (s(f, "language"), s(f, "analysis_key")))
        .collect();
    for o in all {
        root.tick(1)?;
        if !(o["in_work_scope"] != true && keys.contains(&(s(o, "language"), s(o, "analysis_key"))))
        {
            continue;
        }
        out.push(json!({"schema_version":"tos_zarathustra_concept_exclusion_candidate_v1","control_code":"outside_zarathustra_work_scope","language":o["language"],"existing_occurrence_ref":o["existing_occurrence_ref"],"context_unit_ref":null,"exact_form_sha256":o["exact_sha256"],"exclusion_status":"separate_appended_work","review_status":"source_scope_verified","accepted":false,"graph_effect":false,"canon_effect":false}));
    }
    Ok(out)
}
fn identity_bindings(
    root: &ResearchExecution,
    request: &Value,
    speakers: &[Value],
    forms: &[Value],
    occ: &[Value],
    rels: &[Value],
    tasks: &[Value],
) -> Result<Vec<(String, String)>> {
    let mut out = vec![
        ("workbench".into(), "foundation-v1".into()),
        ("request".into(), binding(request)),
        ("concept".into(), format!("concept|{}", binding(request))),
    ];
    for row in speakers {
        root.tick(1)?;
        out.push(("speaker".into(), s(row, "binding").into()));
    }
    for row in forms {
        root.tick(1)?;
        out.push(("form".into(), local(request, &form_binding(row))));
    }
    for (kind, rows) in [
        ("occurrence", occ),
        ("relation", rels),
        ("english_task", tasks),
    ] {
        root.tick(1)?;
        for row in rows {
            root.tick(1)?;
            out.push((kind.into(), local(request, s(row, "binding"))));
        }
    }
    out.sort();
    Ok(out)
}
fn identities(
    root: &ResearchExecution,
    c: &Config,
    expected: &[(String, String)],
    issue: bool,
    frozen: &str,
) -> Result<Ids> {
    if issue {
        if root.join(&c.issuance).exists() {
            return Err("identity issuance already exists; refusing to remint".into());
        }
        let prefixes = BTreeMap::from([
            ("workbench", "concept-workbench"),
            ("request", "concept-request"),
            ("concept", "concept-candidate"),
            ("speaker", "speaker-state-candidate"),
            ("form", "form-family-candidate"),
            ("occurrence", "concept-occurrence-candidate"),
            ("relation", "concept-relation-candidate"),
            ("english_task", "english-translation-task"),
        ]);
        let records:Rows=expected.iter().map(|(k,b)|json!({"kind":k,"binding":b,"id":format!("tos.annotation.{}.sid-{}",prefixes[k.as_str()],&hash(format!("{k}\n{b}"))[..32])})).collect();
        write(
            root,
            &c.issuance,
            &encode(
                &json!({"schema_version":"tos_zarathustra_concept_workbench_identity_issuance_v1","identity_policy":"opaque-id-independent-of-source-text-label-translation-speaker-name-and-current-interpretation","issued_at":frozen,"records":records}),
                true,
            ),
            0o644,
        )?;
    }
    let p = load(root, &c.issuance)?;
    let mut ids = Ids::new();
    for r in arr(&p["records"]) {
        root.tick(1)?;
        let k = (s(r, "kind").into(), s(r, "binding").into());
        if ids.insert(k, s(r, "id").into()).is_some() {
            return Err("duplicate identity binding".into());
        }
    }
    if ids.keys().cloned().collect::<Vec<_>>() != expected
        || ids.values().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err("identity issuance mismatch".into());
    }
    Ok(ids)
}
pub fn hydrate_units(root: &ResearchExecution) -> Result<Rows> {
    root.tick(1)?;
    let maps = source::source_maps(root)?;
    let mut units = source::de_units(root, &maps)?;
    units.extend(source::ru_units(root, &maps)?.0);
    Ok(units)
}
fn write(root: &ResearchExecution, reference: &str, raw: &[u8], mode: u32) -> Result<()> {
    root.write(
        reference,
        raw,
        mode,
        reference.ends_with("identity-issuance.v1.json"),
    )
}
fn git_boundary(ignore: Option<i32>, tracked: Option<i32>, path: &str) -> Result<()> {
    let ignore = ignore.ok_or("git ignore probe terminated by signal")?;
    let tracked = tracked.ok_or("git tracked probe terminated by signal")?;
    if ![0, 1].contains(&ignore) || ![0, 1].contains(&tracked) {
        return Err(format!(
            "private artifact Git probe failed: ignore={ignore},tracked={tracked}"
        ));
    }
    if ignore != 0 || tracked != 1 {
        return Err(format!(
            "private artifact tracking boundary violation: {path}"
        ));
    }
    Ok(())
}
pub fn run(root: &Path, args: &[String]) -> Result<Value> {
    let execution = ResearchExecution::new(root, 180)?;
    run_scoped(&execution, args)
}
pub fn run_scoped(root: &ResearchExecution, args: &[String]) -> Result<Value> {
    root.tick(1)?;
    let mut mode = "";
    let mut issue = false;
    let mut plan_ref = format!("{ROUTE}/plan.v1.json");
    let mut plan_seen = false;
    let mut request_ref = format!("{ROUTE}/requests/fate.concept-request.v2.json");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--build" | "--check" | "--preview" => {
                if !mode.is_empty() {
                    return Err("exactly one mode required".into());
                }
                mode = a;
            }
            "--issue-identities" => issue = true,
            "--plan-ref" => {
                if plan_seen {
                    return Err("duplicate --plan-ref".into());
                }
                plan_seen = true;
                plan_ref = it.next().ok_or("--plan-ref value")?.clone();
            }
            "--request" => request_ref = it.next().ok_or("--request value")?.clone(),
            _ => return Err(format!("unknown argument: {a}")),
        }
    }
    if mode.is_empty() {
        return Err("--build, --check, or --preview required".into());
    }
    if issue && mode != "--build" {
        return Err("--issue-identities is valid only with --build".into());
    }
    if Path::new(&request_ref).is_absolute()
        || Path::new(&request_ref)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("request must be repository relative".into());
    }
    let request = load(root, &request_ref)?;
    validate(
        root,
        &format!("{ROUTE}/concept-request.v2.schema.json"),
        &request,
    )?;
    let c = Config::new(request_ref, &request);
    let selected_plan = select_concept_plan(root, &plan_ref)?;
    if selected_plan.custom && issue {
        return Err("custom Concept plan cannot issue identities".into());
    }
    let plan = &selected_plan.value;
    for (label, r) in plan["inputs"].as_object().ok_or("plan inputs")? {
        root.tick(1)?;
        verify_plan_input(root, r).map_err(|e| format!("{label}: {e}"))?;
    }
    for role in ["reference_register", "etymology_route"] {
        root.tick(1)?;
        let r = &request["english_generation"][role];
        if file_hash(root, s(r, "ref"))? != s(r, "sha256") {
            return Err(format!("English generation reference drift: {role}"));
        }
    }
    let maps = source::source_maps(root)?;
    let de = source::de_units(root, &maps)?;
    let (ru, raw) = source::ru_units(root, &maps)?;
    let all = source::occurrences(
        root,
        &de,
        &ru,
        &raw,
        s(
            &plan["inputs"]["german_exact_occurrence_database"],
            "sha256",
        ),
    )?;
    let census = source::census(root, &all)?;
    let units: Rows = de.into_iter().chain(ru).collect();
    let speakers = speakers(root, &units)?;
    let forms = form_inventory(root, &all)?;
    let (selected, gaps) = select_forms(root, &request, &forms)?;
    let occ = select_occurrences(root, &all, &selected)?;
    let tasks = english_tasks(root, &request, &occ, &units)?;
    let allowed: BTreeSet<_> = arr(&request["relation_policy"]["structural_types"])
        .iter()
        .chain(arr(&request["relation_policy"]["allowed_types"]))
        .filter_map(Value::as_str)
        .collect();
    let rels: Rows = relations(root, &occ, &units)?
        .into_iter()
        .filter(|r| allowed.contains(s(r, "relation_type")))
        .collect();
    let excl = exclusions(root, &request, &all, &selected)?;
    let expected = identity_bindings(root, &request, &speakers, &selected, &occ, &rels, &tasks)?;
    let preview = json!({"identity_count":expected.len(),"witness_context_unit_count":units.len(),"speaker_candidate_count":speakers.len(),"all_exact_occurrence_count":all.len(),"all_analysis_form_count":forms.len(),"selected_form_candidate_count":selected.len(),"occurrence_candidate_count":occ.len(),"english_on_demand_task_count":tasks.len(),"relation_candidate_count":rels.len(),"exclusion_count":excl.len(),"request_output_root":Path::new(c.output("concept")).parent().unwrap().to_string_lossy(),"private_request_ref":c.private_request});
    if mode == "--preview" {
        if selected_plan.custom {
            identities(root, &c, &expected, false, s(plan, "frozen_at"))?;
        }
        root.check()?;
        return Ok(preview);
    }
    let ids = identities(root, &c, &expected, issue, s(&plan, "frozen_at"))?;
    let (mut outputs, private_request, summary) = render::render(
        root, &c, &plan, &request, &units, &speakers, &forms, &selected, &occ, &rels, &gaps, &excl,
        &tasks, &ids, &census, &maps,
    )?;
    let private_db = sql::build(root, &c, &units, &speakers, &all)?;
    render::manifest(
        root,
        &c,
        &selected_plan,
        &request,
        &ids,
        &private_db,
        &private_request,
        &mut outputs,
    )?;
    let private = [
        (c.private_db.as_str(), private_db),
        (c.private_request.as_str(), private_request),
    ];
    if mode == "--build" {
        for (p, b) in outputs {
            root.tick(1)?;
            write(root, &p, &b, 0o644)?;
        }
        for (p, b) in private {
            root.tick(1)?;
            write(root, p, &b, 0o600)?;
        }
        let mut result = preview;
        result.as_object_mut().unwrap().remove("identity_count");
        for (k, v) in summary.as_object().unwrap() {
            root.tick(1)?;
            result[k] = v.clone();
        }
        Ok(result)
    } else {
        for (p, b) in outputs {
            root.tick(1)?;
            if read(root, &p)? != b {
                return Err(format!("tracked parity mismatch: {p}"));
            }
        }
        for (p, b) in private {
            root.tick(1)?;
            let file = root.source_file(p, 256 * 1024 * 1024)?;
            let meta = file.metadata().map_err(|e| e.to_string())?;
            if !meta.file_type().is_file() || meta.permissions().mode() & 0o777 != 0o600 {
                return Err(format!("private mode/type mismatch: {p}"));
            }
            if read(root, p)? != b {
                return Err(format!("private parity mismatch: {p}"));
            }
            root.tick(1)?;
            use crate::owned_native_child::{CaptureLimits, capture};
            let limits = CaptureLimits {
                max_stdin_bytes: 0,
                max_stdout_bytes: 4096,
                max_stderr_bytes: 16384,
            };
            let ignored = capture(
                std::process::Command::new("git")
                    .args(["check-ignore", "-q", "--", p])
                    .current_dir({
                        use std::os::fd::AsRawFd;
                        format!("/proc/self/fd/{}", root.root_directory().as_raw_fd())
                    }),
                None,
                limits,
                root.deadline(),
            )?;
            let tracked = capture(
                std::process::Command::new("git")
                    .args(["ls-files", "--error-unmatch", "--", p])
                    .current_dir({
                        use std::os::fd::AsRawFd;
                        format!("/proc/self/fd/{}", root.root_directory().as_raw_fd())
                    }),
                None,
                limits,
                root.deadline(),
            )?;
            git_boundary(ignored.status.code(), tracked.status.code(), p)?;
        }
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture_request() -> Value {
        json!({"request_key":"fate","request_identity_key":"tos.concept-request-key.sid-0123456789abcdef0123456789abcdef","request_version":2,"lexical_probes":{"de":["Schicksal"],"ru":["судьба"]},"semantic_neighbor_probes":{"de":["Verhängniss"],"ru":[]}})
    }
    #[test]
    fn python_recursive_sorted_json_exact_bytes_preserves_array_and_scalars() {
        // Independent CPython sort_keys=True byte contract; preserve_order feature unification.
        let value = json!({"z":[{"β":true,"a":null},"Ж",2,-1,1.5],"a":{"z":"Straße","b":false}});
        assert_eq!(
            encode(&value, false),
            r#"{"a":{"b":false,"z":"Straße"},"z":[{"a":null,"β":true},"Ж",2,-1,1.5]}
"#
            .as_bytes()
        );
        assert_eq!(
            encode(&value, true),
            r#"{
  "a": {
    "b": false,
    "z": "Straße"
  },
  "z": [
    {
      "a": null,
      "β": true
    },
    "Ж",
    2,
    -1,
    1.5
  ]
}
"#
            .as_bytes()
        );
    }

    #[test]
    fn reversible_expansion_retains_low_frequency_and_semantic_tier() {
        let r = fixture_request();
        let all = vec![
            json!({"language":"de","analysis_key":"schicksals","analysis_key_sha256":hash("schicksals"),"occurrence_count":1,"exact_hashes":[hash("Schicksals")],"normalized_hashes":[],"parts":[1],"readings":["p1.r1"]}),
            json!({"language":"ru","analysis_key":"судьбы","analysis_key_sha256":hash("судьбы"),"occurrence_count":1,"exact_hashes":[hash("судьбы")],"normalized_hashes":[],"parts":[1],"readings":["p1.r1"]}),
            json!({"language":"de","analysis_key":"verhängnisse","analysis_key_sha256":hash("verhängnisse"),"occurrence_count":1,"exact_hashes":[],"normalized_hashes":[],"parts":[1],"readings":["p1.r1"]}),
        ];
        let directory = tempfile::tempdir().unwrap();
        let execution =
            ResearchExecution::new_with_scratch(directory.path(), 180, 16 * 1024 * 1024).unwrap();
        let (selected, gaps) = select_forms(&execution, &r, &all).unwrap();
        assert_eq!(selected.len(), 3);
        assert_eq!(gaps.len(), 3);
        assert_eq!(s(&selected[0], "selection_kind"), "direct_morphology");
        assert_eq!(s(&selected[1], "selection_kind"), "direct_morphology");
        assert_eq!(s(&selected[2], "selection_kind"), "semantic_morphology");
        assert_eq!(n(&selected[0], "occurrence_count"), 1);
    }
    #[test]
    fn git_probe_failure_is_not_an_untracked_artifact() {
        assert!(git_boundary(Some(0), Some(1), "private").is_ok());
        assert!(
            git_boundary(Some(0), Some(128), "private")
                .unwrap_err()
                .contains("probe failed")
        );
        assert!(git_boundary(Some(128), Some(1), "private").is_err());
        assert!(git_boundary(Some(1), Some(1), "private").is_err());
        assert!(git_boundary(Some(0), Some(0), "private").is_err());
        assert!(git_boundary(Some(0), None, "private").is_err());
    }
    #[test]
    fn off_journal_workbench_recipe_vacuum_closes_owned_scratch() {
        let directory = tempfile::tempdir().unwrap();
        // Explicit test-only envelope for production inode ceilings. Empty
        // inputs make actual fixture bytes small; this is not a host grant.
        let root =
            ResearchExecution::new_with_scratch(directory.path(), 180, 512 * 1024 * 1024).unwrap();
        let request = fixture_request();
        let config = Config::new(
            format!("{ROUTE}/requests/fate.concept-request.v2.json"),
            &request,
        );
        let bytes = sql::build(&root, &config, &[], &[], &[]).unwrap();
        assert_eq!(&bytes[..16], b"SQLite format 3\0");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
        let report = root.budget_report();
        // SQL inodes are closed; only a directory lease may remain when the
        // parent retains allocation after removal of the owned empty child.
        assert!(
            report["physical_scratch"]["reserved_current_bytes"]
                .as_u64()
                .unwrap()
                <= 64 * 1024
        );
        assert!(report["read_returned_bytes"].as_u64().unwrap() > 0);
        assert!(report["write_returned_bytes"].as_u64().unwrap() > 0);
    }
    #[test]
    fn historical_long_s_and_ligatures_follow_full_casefold() {
        assert!(signatures("ſchickſal", "de").contains("schicksal"));
        assert!(signatures("ﬃ", "de").contains("ffi"));
        assert!(signatures("SCHICKSÄLE", "de").contains("schicksal"));
    }
    #[test]
    fn request_identity_and_version_separate_every_request_local_object() {
        let r = fixture_request();
        let mut r2 = r.clone();
        r2["request_version"] = json!(3);
        assert_ne!(
            local(&r, "occurrence|opaque"),
            local(&r2, "occurrence|opaque")
        );
        let c = Config::new(format!("{ROUTE}/requests/other.json"), &r2);
        assert!(
            c.output("concept")
                .contains("fate-v3-0123456789abcdef0123456789abcdef")
        );
        assert!(
            c.private_request
                .contains("fate-v3-0123456789abcdef0123456789abcdef")
        );
    }
    #[test]
    fn normalized_path_and_symlink_guards_refuse_escape() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let root =
            ResearchExecution::new_with_scratch(directory.path(), 180, 16 * 1024 * 1024).unwrap();
        fs::write(root.root().join("owned"), b"retained").unwrap();
        symlink(root.root().join("owned"), root.root().join("alias")).unwrap();
        assert!(read(&root, "alias").is_err());
        assert!(read(&root, "../owned").is_err());
        assert!(write(&root, "../escaped", b"unsafe", 0o644).is_err());
        symlink(root.root(), root.root().join("aliased-dir")).unwrap();
        assert!(write(&root, "aliased-dir/new", b"unsafe", 0o644).is_err());
        assert_eq!(fs::read(root.root().join("owned")).unwrap(), b"retained");
    }
    #[test]
    fn issuance_is_no_clobber_and_outputs_have_exact_private_mode() {
        let directory = tempfile::tempdir().unwrap();
        let root =
            ResearchExecution::new_with_scratch(directory.path(), 180, 16 * 1024 * 1024).unwrap();
        let name = "owned/identity-issuance.v1.json";
        write(&root, name, b"first", 0o644).unwrap();
        assert!(write(&root, name, b"second", 0o644).is_err());
        assert_eq!(read(&root, name).unwrap(), b"first");
        write(&root, "local/private.json", b"private", 0o600).unwrap();
        assert_eq!(
            fs::metadata(root.root().join("local/private.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    #[test]
    fn scope_exclusion_cannot_enter_form_inventory() {
        let rows = vec![
            json!({"in_work_scope":false,"language":"de","analysis_key":"schicksal"}),
            json!({"in_work_scope":true,"language":"ru","analysis_key":"судьба","analysis_key_sha256":hash("судьба"),"exact_sha256":hash("судьба"),"normalized_sha256":hash("судьба"),"part":1,"reading_ref":"p1.r1"}),
        ];
        let directory = tempfile::tempdir().unwrap();
        let execution =
            ResearchExecution::new_with_scratch(directory.path(), 180, 16 * 1024 * 1024).unwrap();
        let forms = form_inventory(&execution, &rows).unwrap();
        assert_eq!(forms.len(), 1);
        assert_eq!(s(&forms[0], "language"), "ru");
    }
    #[test]
    fn technical_plan_only_changes_five_technical_digests() {
        let original: Value = serde_json::from_str(include_str!(
            "../../../../ToS/candidate-intake/zarathustra/concept-workbench-v1/plan.v1.json"
        ))
        .unwrap();
        let default_ref = format!("{ROUTE}/plan.v1.json");
        let mut accepted = original.clone();
        accepted["plan_id"] = json!("candidate-plan:concept-technical-profile-control");
        accepted["status"] = json!("proposed-technical-input-profile-successor");
        accepted["input_profile_lineage"] = json!({"profile_version":2,"supersedes_plan_ref":default_ref,"supersedes_plan_sha256":DEFAULT_PLAN_SHA});
        for label in [
            "paragraph_alignment_manifest",
            "parallel_lexical_manifest",
            "morphology_theme_manifest",
            "eternal_return_review_preparation_manifest",
            "german_exact_occurrence_database",
        ] {
            accepted["inputs"][label]["sha256"] = json!("a".repeat(64));
        }
        validate_concept_plan_semantics(&accepted, &original, &default_ref).unwrap();
        for (pointer, replacement) in [
            ("/inputs/review_checklist/sha256", json!("b".repeat(64))),
            (
                "/inputs/german_exact_occurrence_database/ref",
                json!("other.sqlite3"),
            ),
            ("/frozen_at", Value::Null),
            ("/research_question", json!("different semantic request")),
            ("/status", json!("accepted")),
            (
                "/input_profile_lineage/supersedes_plan_sha256",
                json!("b".repeat(64)),
            ),
            (
                "/inputs/parallel_lexical_manifest/sha256",
                json!("not-a-digest"),
            ),
        ] {
            let mut rejected = accepted.clone();
            *rejected.pointer_mut(pointer).unwrap() = replacement;
            assert!(
                validate_concept_plan_semantics(&rejected, &original, &default_ref).is_err(),
                "{pointer}"
            );
        }
        let mut authority = accepted.clone();
        authority["accepted_candidate_count"] = json!(1);
        assert!(validate_concept_plan_semantics(&authority, &original, &default_ref).is_err());
    }
}
