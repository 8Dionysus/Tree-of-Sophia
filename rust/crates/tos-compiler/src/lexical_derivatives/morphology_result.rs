//! Text-free registration of a retained DWDSmor run. Provider execution and
//! runtime admission remain with the laboratory owner.
use super::{
    Held, META_CAP, ResearchExecution, Result, canonical, ensure, generated_path, present,
    read_json, s, sha, valid_generation,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, os::unix::fs::PermissionsExt, path::Path};
use tos_foundation::unicode_predicates16 as uc;
pub const PLAN: &str = super::MORPHOLOGY_PLAN;
pub const RECEIPT: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/morphology-census-result.a-dwdsmor-open-0.18.0.v1.json";
const BUILDER: &str = "rust/crates/tos-compiler/src/lexical_derivatives/morphology_result.rs";
const PLAN_SHA: &str = "6075b2926b49c4b79b0a186e5338eaa7dd33e847ea1b38b322ef3403dcfcd4c5";
const INPUT_SHA: &str = "1c734e6f8371e28b863d58e216b7a649aa6d603192f1f7310056e8caa022eb9a";
const OLD_RECEIPT_SHA: &str = "9dbf0733e48164e25c19aceb26cf678a006fa376fd1c828f848352080482301f";
pub(super) const EXPERIMENT: &str = "tos-historical-german-morphology-v1";
type Counter = BTreeMap<String, u64>;
pub(super) fn uint(v: &Value) -> Result<u64> {
    v.as_u64()
        .ok_or("unsigned morphology result integer required".into())
}
pub(super) fn equal(actual: &Value, expected: Value, label: &str) -> Result<()> {
    ensure(actual == &expected, &format!("{label} drift"))
}
fn count(map: &mut Counter, key: impl Into<String>, amount: u64) -> Result<()> {
    let key = key.into();
    ensure(
        key.len() <= 256 && (map.contains_key(&key) || map.len() < 4096),
        "provider label bound",
    )?;
    let v = map.entry(key).or_default();
    *v = v.checked_add(amount).ok_or("provider count overflow")?;
    Ok(())
}
fn case_bucket(s: &str) -> &'static str {
    let (mut lower, mut upper, mut title, mut any, mut previous) = (true, true, true, false, false);
    for c in s.chars() {
        let (lo, up, ti) = (uc::lowercase(c), uc::uppercase(c), uc::titlecase(c));
        if lo || up || ti {
            any = true;
            lower &= lo;
            upper &= up;
            title &= if previous { lo } else { up || ti };
            previous = true;
        } else {
            previous = false;
        }
    }
    if any && lower {
        "lower"
    } else if any && upper {
        "upper"
    } else if any && title {
        "title"
    } else {
        "mixed-or-uncased"
    }
}
fn character_bucket(s: &str) -> &'static str {
    if s.chars().any(uc::digit) {
        "contains-digit"
    } else if s.chars().any(|c| "-'’‐‑".contains(c)) {
        "contains-joiner"
    } else if s.chars().all(uc::alphabetic) {
        "alphabetic-only"
    } else {
        "contains-other"
    }
}
fn label(v: &Value) -> Result<String> {
    Ok(match v {
        Value::Null => "<none>".into(),
        Value::Bool(false) => "<none>".into(),
        Value::Bool(true) => "True".into(),
        Value::String(s) => {
            if s.is_empty() {
                "<none>".into()
            } else {
                s.clone()
            }
        }
        Value::Number(n) => {
            if n.as_f64() == Some(0.) {
                "<none>".into()
            } else {
                n.to_string()
            }
        }
        _ => return Err("provider label must be scalar".into()),
    })
}
fn analyses(v: &Value, pos: &mut Counter, category: &mut Counter) -> Result<u64> {
    let rows = v.as_array().ok_or("provider analyses must be array")?;
    ensure(rows.len() <= 10000, "analysis count bound")?;
    for row in rows {
        let obj = row.as_object().ok_or("provider analysis must be object")?;
        ensure(
            obj.values().all(|v| !v.is_array() && !v.is_object()),
            "provider analysis must contain scalar values",
        )?;
        count(pos, label(&row["pos"])?, 1)?;
        count(category, label(&row["category"])?, 1)?;
    }
    Ok(rows.len() as u64)
}
#[derive(Default)]
struct Census {
    rows: u64,
    tokens: u64,
    covered: u64,
    covered_tokens: u64,
    root: u64,
    root_tokens: u64,
    lemma_total: u64,
    root_total: u64,
    lemma_counts: Counter,
    root_counts: Counter,
    pos: Counter,
    category: Counter,
    root_pos: Counter,
    root_category: Counter,
    unknown_types: [Counter; 4],
    unknown_tokens: [Counter; 4],
    max_occurrences: u64,
    max_length: usize,
    previous: Option<(String, String)>,
}
impl Census {
    fn row(&mut self, encoded: &[u8]) -> Result<()> {
        let row = crate::zarathustra_lexical::parse(encoded, META_CAP as usize)?;
        let obj = row.as_object().ok_or("provider row must be object")?;
        let keys = [
            "schema_version",
            "form_key",
            "exact_form",
            "exact_form_sha256",
            "normalized_form_sha256",
            "occurrence_count",
            "input_preserved",
            "provider",
            "lemma_analyses",
            "root_analyses",
            "lemma_analysis_count",
            "root_analysis_count",
            "unknown",
            "authority",
        ];
        ensure(
            obj.len() == keys.len() && keys.iter().all(|k| obj.contains_key(*k)),
            "provider row fields drift",
        )?;
        ensure(
            canonical(row.clone())? == encoded,
            "provider row is not canonical JSONL",
        )?;
        let surface = s(&row["exact_form"])?;
        let digest = s(&row["exact_form_sha256"])?;
        let occurrences = uint(&row["occurrence_count"])?;
        ensure(
            !surface.is_empty()
                && surface.len() <= 65536
                && sha(surface.as_bytes()) == digest
                && occurrences > 0,
            "provider surface identity/count drift",
        )?;
        equal(
            &row["schema_version"],
            json!("tos_dwdsmor_analysis_row_v1"),
            "provider schema",
        )?;
        equal(
            &row["form_key"],
            json!(format!("lexical-form:sha256:{digest}")),
            "provider form key",
        )?;
        ensure(
            s(&row["normalized_form_sha256"])?.len() == 64,
            "normalized digest length",
        )?;
        equal(
            &row["input_preserved"],
            json!(true),
            "provider preservation",
        )?;
        equal(
            &row["provider"],
            json!({"artifact":"DWDSmor Open","version":"0.18.0","source_commit":"f97b92ce2a5d6db8750afbdb222eb39470e57cf6","wheel_sha256":"395a15e15286b0c191b42355b6e3c2a43c8959621ccf3563336c2e30399a2973","surface_normalized_before_analysis":false}),
            "provider identity",
        )?;
        equal(
            &row["authority"],
            json!("unreviewed-provider-candidate"),
            "provider authority",
        )?;
        let order = (digest.to_owned(), surface.to_owned());
        ensure(
            self.previous.as_ref().is_none_or(|v| v < &order),
            "frozen provider row order",
        )?;
        let lemma = analyses(&row["lemma_analyses"], &mut self.pos, &mut self.category)?;
        let root = analyses(
            &row["root_analyses"],
            &mut self.root_pos,
            &mut self.root_category,
        )?;
        equal(&row["lemma_analysis_count"], json!(lemma), "lemma count")?;
        equal(&row["root_analysis_count"], json!(root), "root count")?;
        equal(&row["unknown"], json!(lemma == 0), "unknown state")?;
        ensure(self.rows < 1_000_000, "provider row count bound")?;
        self.rows += 1;
        self.tokens = self
            .tokens
            .checked_add(occurrences)
            .ok_or("token count overflow")?;
        self.lemma_total += lemma;
        self.root_total += root;
        count(&mut self.lemma_counts, lemma.to_string(), 1)?;
        count(&mut self.root_counts, root.to_string(), 1)?;
        if lemma > 0 {
            self.covered += 1;
            self.covered_tokens += occurrences;
        } else {
            let len = surface.chars().count();
            let frequency = match occurrences {
                1 => "1",
                2..=4 => "2-4",
                5..=9 => "5-9",
                10..=49 => "10-49",
                _ => "50-plus",
            };
            let length = match len {
                0..=2 => "1-2",
                3..=5 => "3-5",
                6..=10 => "6-10",
                11..=20 => "11-20",
                _ => "21-plus",
            };
            for (i, key) in [
                frequency,
                length,
                case_bucket(surface),
                character_bucket(surface),
            ]
            .iter()
            .enumerate()
            {
                count(&mut self.unknown_types[i], *key, 1)?;
                count(&mut self.unknown_tokens[i], *key, occurrences)?;
            }
            self.max_occurrences = self.max_occurrences.max(occurrences);
            self.max_length = self.max_length.max(len);
        }
        if root > 0 {
            self.root += 1;
            self.root_tokens += occurrences;
        }
        self.previous = Some(order);
        Ok(())
    }
    fn finish(self, digest: &str) -> Result<Value> {
        ensure(self.rows > 0, "empty morphology census")?;
        let mut residue = json!({"review_status":"unreviewed-mechanical-aggregation","triggers_contextual_followup":false,"type_count":self.rows-self.covered,"token_weight":self.tokens-self.covered_tokens,"maximum_occurrence_count":self.max_occurrences,"maximum_codepoint_length":self.max_length,"source_strings_included":false});
        for (i, key) in [
            "occurrence_frequency",
            "codepoint_length",
            "case_shape",
            "character_shape",
        ]
        .iter()
        .enumerate()
        {
            residue[*key] =
                json!({"type_counts":self.unknown_types[i],"token_weights":self.unknown_tokens[i]});
        }
        Ok(
            json!({"stream_sha256":digest,"row_count":self.rows,"token_occurrence_count":self.tokens,"covered_type_count":self.covered,"covered_token_count":self.covered_tokens,"unknown_type_count":self.rows-self.covered,"unknown_token_count":self.tokens-self.covered_tokens,"root_type_count":self.root,"root_token_count":self.root_tokens,"lemma_analysis_total":self.lemma_total,"root_analysis_total":self.root_total,"lemma_analysis_count":self.lemma_counts,"root_analysis_count":self.root_counts,"provider_pos":self.pos,"provider_category":self.category,"unknown_residue":residue}),
        )
    }
}
pub(super) fn lines(
    ctx: &ResearchExecution,
    held: &mut Held,
    mut visit: impl FnMut(&[u8]) -> Result<()>,
) -> Result<()> {
    let mut pending = Vec::new();
    let mut left = held.metadata.len();
    let mut buffer = [0u8; 65536];
    while left > 0 {
        let n = left.min(buffer.len() as u64) as usize;
        ctx.read_exact(&mut held.file, &mut buffer[..n])?;
        left -= n as u64;
        for part in buffer[..n].split_inclusive(|b| *b == b'\n') {
            ensure(
                part.len() <= META_CAP as usize - pending.len(),
                "provider line byte bound",
            )?;
            pending.extend_from_slice(part);
            if pending.last() == Some(&b'\n') {
                ctx.tick(pending.len() as u64)?;
                visit(&pending)?;
                pending.clear();
            }
        }
    }
    ensure(
        pending.is_empty(),
        "provider JSONL must terminate with newline",
    )?;
    Ok(())
}
pub fn inspect(ctx: &ResearchExecution, path: &Path) -> Result<Value> {
    let (selected, mut h) = external(ctx, path, 128 * 1024 * 1024)?;
    mode(&h, true)?;
    let mut census = Census::default();
    lines(&selected, &mut h, |raw| census.row(raw))?;
    h.verify(&selected)?;
    let raw = census.finish(&h.digest)?;
    label_contract(
        ctx,
        "morphology-census-result-receipt",
        &raw,
        &[
            ("provider_pos", "providerPosCountMap"),
            ("provider_category", "providerCategoryCountMap"),
        ],
    )?;
    Ok(raw)
}
pub(super) fn label_contract(
    ctx: &ResearchExecution,
    name: &str,
    raw: &Value,
    maps: &[(&str, &str)],
) -> Result<()> {
    let mut held = Vec::new();
    let schema = read_json(ctx, &format!("ToS/contracts/{name}.schema.json"), &mut held)?;
    for (key, definition) in maps {
        let rule = &schema["$defs"][*definition]["propertyNames"];
        let keys = raw[*key].as_object().ok_or("provider count map required")?;
        if let Some(allowed) = rule["enum"].as_array() {
            for key in keys.keys() {
                ensure(
                    allowed.iter().any(|v| v.as_str() == Some(key)),
                    "provider label outside text-free contract",
                )?;
            }
        } else if let Some(pattern) = rule["pattern"].as_str() {
            ensure(pattern.len() <= 1024, "provider label pattern bound")?;
            let regex = regex::RegexBuilder::new(pattern)
                .size_limit(65536)
                .build()
                .map_err(|e| e.to_string())?;
            for key in keys.keys() {
                ensure(
                    regex.is_match(key),
                    "provider label outside text-free contract",
                )?;
            }
        } else {
            return Err("provider label contract required".into());
        }
    }
    for h in &mut held {
        h.verify(ctx)?;
    }
    Ok(())
}
pub(super) fn external(
    ctx: &ResearchExecution,
    path: &Path,
    cap: u64,
) -> Result<(ResearchExecution, Held)> {
    ensure(
        path.is_absolute(),
        "absolute explicit evidence path required",
    )?;
    let selected = ctx.select_directory(path.parent().ok_or("evidence parent required")?)?;
    let name = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("evidence filename required")?;
    let held = Held::open(&selected, name, cap)?;
    Ok((selected, held))
}
pub(super) fn mode(h: &Held, private: bool) -> Result<()> {
    let m = h.metadata.permissions().mode() & 0o777;
    ensure(
        m == 0o600 || (!private && m == 0o644),
        "private evidence mode drift",
    )
}
pub(super) fn file_record(h: &Held, private: bool) -> Result<Value> {
    mode(h, private)?;
    Ok(
        json!({"sha256":h.digest,"bytes":h.metadata.len(),"mode":format!("{:04o}",h.metadata.permissions().mode()&0o777),"source_bearing":private}),
    )
}
fn close_float(v: &Value, expected: f64, label: &str) -> Result<()> {
    ensure(
        v.as_f64()
            .is_some_and(|v| v.is_finite() && (v - expected).abs() <= 1e-15),
        &format!("{label} drift"),
    )
}
pub(super) fn duration(v: &Value) -> Result<f64> {
    let v = s(v)?
        .trim_end_matches('s')
        .parse::<f64>()
        .map_err(|_| "host duration invalid")?;
    ensure(v.is_finite() && v >= 0., "host duration bound")?;
    Ok(v)
}
pub struct Options<'a> {
    pub build: bool,
    pub artifact_root: &'a Path,
    pub run: &'a str,
    pub runtime_manifest: &'a Path,
    pub runner: &'a Path,
    pub source_packet: &'a Path,
    pub output_root: &'a Path,
    pub generation: Option<&'a str>,
}
pub fn run(ctx: &ResearchExecution, opt: Options<'_>) -> Result<Value> {
    ensure(
        !opt.build || opt.generation.is_some(),
        "build requires a new generation",
    )?;
    if let Some(g) = opt.generation {
        ensure(valid_generation(g), "generation syntax")?;
    }
    let parts = opt.run.split('/').collect::<Vec<_>>();
    ensure(
        parts.len() == 3
            && parts[0] == EXPERIMENT
            && !parts[1].is_empty()
            && parts[2] == "variant-A",
        "experiment/variant run route drift",
    )?;
    if opt.build {
        for root in [
            ctx.root(),
            opt.artifact_root,
            opt.runtime_manifest.parent().ok_or("runtime parent")?,
            opt.runner.parent().ok_or("runner parent")?,
            opt.source_packet.parent().ok_or("packet parent")?,
        ] {
            ensure(
                !opt.output_root.starts_with(root) && !root.starts_with(opt.output_root),
                "result output must be separate from inputs",
            )?;
        }
    }
    let mut source_held = Vec::new();
    let schemas = super::schemas(ctx, &["morphology-census-result-receipt"], &mut source_held)?;
    let plan = read_json(ctx, PLAN, &mut source_held)?;
    ensure(
        source_held.last().unwrap().digest == PLAN_SHA,
        "plan digest drift",
    )?;
    let artifacts = ctx.select_directory(opt.artifact_root)?;
    let mut held = Vec::new();
    let mut records = json!({});
    let mut values = BTreeMap::new();
    for (name, rel, private) in [
        ("run_receipt", "run.receipt.json", false),
        ("experiment_spec", "experiment.spec.json", false),
        ("preflight", "receipts/preflight.json", false),
        ("execution", "receipts/execution.json", true),
        (
            "repeat_determinism",
            "receipts/repeat-determinism.json",
            false,
        ),
        (
            "host_resource_launch",
            "receipts/host-resource-launch.json",
            false,
        ),
        ("metrics", "metrics/census-summary.json", false),
    ] {
        let value = read_json(&artifacts, &format!("{}/{rel}", opt.run), &mut held)?;
        records[name] = file_record(held.last().unwrap(), private)?;
        values.insert(name, value);
    }
    let (run, experiment, preflight, execution, repeat, resource, metrics) = (
        &values["run_receipt"],
        &values["experiment_spec"],
        &values["preflight"],
        &values["execution"],
        &values["repeat_determinism"],
        &values["host_resource_launch"],
        &values["metrics"],
    );
    let mut stream = Held::open(
        &artifacts,
        &format!("{}/raw-output/dwdsmor-census.jsonl", opt.run),
        128 * 1024 * 1024,
    )?;
    records["raw_output"] = file_record(&stream, true)?;
    let mut census = Census::default();
    lines(&artifacts, &mut stream, |r| census.row(r))?;
    let raw = census.finish(&stream.digest)?;
    held.push(stream);
    for (field, value) in [
        ("run_id", json!(parts[1])),
        ("experiment_id", json!(EXPERIMENT)),
        ("variant", json!("A")),
        ("status", json!("awaiting-triggered-review")),
        ("manual_review_refs", json!([])),
        ("model_inspection_refs", json!([])),
        ("errors", json!([])),
        ("retention_decision", json!("retain")),
    ] {
        equal(&run[field], value, &format!("run {field}"))?;
    }
    equal(
        &experiment["experiment_id"],
        json!(EXPERIMENT),
        "experiment",
    )?;
    equal(
        &experiment["source_plan_sha256"],
        json!(PLAN_SHA),
        "experiment plan",
    )?;
    equal(
        &run["experiment_spec_sha256"],
        preflight["experiment_sha256"].clone(),
        "suite experiment digest",
    )?;
    equal(&preflight["decision"], json!("ready"), "preflight")?;
    equal(&preflight["variant"], json!("A"), "preflight variant")?;
    for k in ["source_plan_admission", "runtime_admission"] {
        equal(&preflight[k]["verified"], json!(true), k)?;
    }
    for (k, v) in [
        ("schema_version", json!("tos_dwdsmor_census_execution_v1")),
        ("source_plan_sha256", json!(PLAN_SHA)),
        ("source_packet_sha256", json!(INPUT_SHA)),
        ("network_used", json!(false)),
        ("source_content_public", json!(false)),
    ] {
        equal(&execution[k], v, k)?;
    }
    for (key, digest) in [
        ("raw_output", "raw_output_sha256"),
        ("metrics", "metrics_sha256"),
        ("repeat_determinism", "repeat_receipt_sha256"),
    ] {
        equal(&execution[digest], records[key]["sha256"].clone(), digest)?;
    }
    let (runner_ctx, mut runner) = external(ctx, opt.runner, 16 * 1024 * 1024)?;
    equal(
        &execution["runner_sha256"],
        json!(runner.digest),
        "runner digest",
    )?;
    let (runtime_ctx, mut runtime_file) = external(ctx, opt.runtime_manifest, META_CAP)?;
    equal(
        &execution["runtime_manifest_sha256"],
        json!(runtime_file.digest),
        "runtime manifest digest",
    )?;
    let runtime = crate::zarathustra_lexical::parse(
        &runtime_ctx.read_file(&mut runtime_file.file, META_CAP)?,
        META_CAP as usize,
    )?;
    let (packet_ctx, mut packet) = external(ctx, opt.source_packet, 64 * 1024 * 1024)?;
    ensure(packet.digest == INPUT_SHA, "private input digest drift")?;
    mode(&packet, true)?;
    for (k, v) in [
        ("status", json!("verified")),
        ("experiment_id", json!(EXPERIMENT)),
        ("variant", json!("A")),
        (
            "artifact_set_sha256",
            run["method_revision"]["artifact_digest"].clone(),
        ),
    ] {
        equal(&runtime[k], v, &format!("runtime {k}"))?;
    }
    equal(
        &metrics["experiment_id"],
        json!(EXPERIMENT),
        "metrics experiment",
    )?;
    equal(&metrics["variant"], json!("A"), "metrics variant")?;
    equal(
        &metrics["provider"]["artifact"],
        json!("DWDSmor Open"),
        "metrics provider",
    )?;
    equal(
        &metrics["provider"]["version"],
        json!("0.18.0"),
        "metrics provider version",
    )?;
    equal(
        &metrics["input"],
        json!({"plan_sha256":PLAN_SHA,"packet_sha256":INPUT_SHA,"row_count":raw["row_count"],"token_occurrence_count":raw["token_occurrence_count"],"exact_surface_mutated":false}),
        "metrics input",
    )?;
    let mut coverage = metrics["coverage"].clone();
    ensure(coverage.is_object(), "coverage object required")?;
    for k in [
        "covered_type_count",
        "covered_token_count",
        "unknown_type_count",
        "unknown_token_count",
        "root_type_count",
        "root_token_count",
    ] {
        equal(&coverage[k], raw[k].clone(), k)?;
    }
    for (k, num, den) in [
        ("form_type_coverage", "covered_type_count", "row_count"),
        (
            "token_weighted_coverage",
            "covered_token_count",
            "token_occurrence_count",
        ),
        ("unknown_form_rate", "unknown_type_count", "row_count"),
    ] {
        close_float(
            &coverage[k],
            uint(&raw[num])? as f64 / uint(&raw[den])? as f64,
            k,
        )?;
    }
    let mut distributions = json!({});
    for k in [
        "lemma_analysis_count",
        "root_analysis_count",
        "provider_pos",
        "provider_category",
    ] {
        distributions[k] = raw[k].clone();
    }
    equal(
        &metrics["distributions"],
        distributions.clone(),
        "provider distributions",
    )?;
    for k in ["lemma_analysis_count", "root_analysis_count"] {
        let sum = raw[k]
            .as_object()
            .unwrap()
            .values()
            .try_fold(0u64, |sum, v| {
                uint(v).and_then(|n| sum.checked_add(n).ok_or("count overflow".into()))
            })?;
        equal(&raw["row_count"], json!(sum), "distribution denominator")?;
    }
    for k in ["provider_pos", "provider_category"] {
        let sum = raw[k]
            .as_object()
            .unwrap()
            .values()
            .map(uint)
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .sum::<u64>();
        equal(
            &raw["lemma_analysis_total"],
            json!(sum),
            "analysis distribution total",
        )?;
    }
    for (k, v) in [
        ("deterministic", json!(true)),
        ("mismatch_count", json!(0)),
        ("row_count", raw["row_count"].clone()),
        ("pass_1_stream_sha256", raw["stream_sha256"].clone()),
        ("pass_2_stream_sha256", raw["stream_sha256"].clone()),
    ] {
        equal(&repeat[k], v, k)?;
    }
    equal(
        &metrics["repeat_determinism"]["pass_1_stream_sha256"],
        raw["stream_sha256"].clone(),
        "metrics repeat",
    )?;
    equal(
        &metrics["bytes"],
        json!({"runtime":runtime["runtime_bytes"],"raw_output":records["raw_output"]["bytes"],"source_packet":packet.metadata.len(),"metrics":records["metrics"]["bytes"]}),
        "byte accounting",
    )?;
    equal(
        &metrics["accuracy"],
        json!({"status":"unmeasured-no-german-competent-gold","coverage_is_accuracy":false,"machine_agreement_is_gold":false}),
        "accuracy boundary",
    )?;
    equal(
        &metrics["followup"],
        json!({"status":"blocked-not-materialized","b_acquired":false,"c_acquired":false,"human_work_scheduled":false,"trigger":"reviewed-a-census-residue-or-concrete-source-translation-sign-retrieval-question"}),
        "followup boundary",
    )?;
    equal(&resource["ok"], json!(true), "resource launch")?;
    equal(
        &resource["execution"]["returncode"],
        json!(0),
        "resource return code",
    )?;
    equal(
        &resource["startup_admission"]["demand_observation"]["execution_succeeded"],
        json!(true),
        "resource execution",
    )?;
    let peaks = &resource["startup_admission"]["demand_observation"]["peaks"];
    equal(&peaks["ok"], json!(true), "resource peaks")?;
    let builder = Held::open(ctx, BUILDER, META_CAP)?;
    ensure(
        builder.digest == sha(include_bytes!("morphology_result.rs")),
        "running morphology result generator source drift",
    )?;
    let generator = json!({"ref":BUILDER,"sha256":builder.digest});
    source_held.push(builder);
    coverage["coverage_is_accuracy"] = json!(false);
    for k in ["lemma_analysis_total", "root_analysis_total"] {
        distributions[k] = raw[k].clone();
    }
    let mut performance = metrics["performance"].clone();
    ensure(performance.is_object(), "performance object required")?;
    performance["host_service_wall_seconds"] = json!(duration(
        &resource["execution"]["systemd"]["service_runtime"]
    )?);
    performance["host_service_cpu_seconds"] = json!(duration(
        &resource["execution"]["systemd"]["cpu_time_consumed"]
    )?);
    performance["host_cgroup_memory_peak_bytes"] = peaks["memory_peak_bytes"].clone();
    performance["host_cgroup_swap_peak_bytes"] = peaks["memory_swap_peak_bytes"].clone();
    let suffix = opt
        .generation
        .map(|v| format!(".native-{v}"))
        .unwrap_or_default();
    let mut receipt = json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/morphology-census-result-receipt.schema.json","schema_version":"tos_morphology_census_result_receipt_v1","generated_or_authored":"generated_from_private_provider_census","receipt_id":format!("morphology-census-result:zarathustra-dta-first-editions-parts-1-4.a.dwdsmor-open-0.18.0.v1{suffix}"),"recorded_at_utc":run["finished_at_utc"],"status":"a-census-executed-awaiting-triggered-review","experiment_id":EXPERIMENT,"variant":"A",
        "plan":{"plan_id":plan["plan_id"],"ref":PLAN,"sha256":PLAN_SHA,"frozen_before_variant_outputs":true},"generator":generator,
        "private_run":{"artifact_owner":"abyss-machine:storage/artifacts/tree-of-sophia-foundation-lab","relative_ref":opt.run,"run_id":parts[1],"status":run["status"],"started_at_utc":run["started_at_utc"],"finished_at_utc":run["finished_at_utc"],"retention_decision":"retain","visibility":"owner-local-only","manual_review_count":0,"model_inspection_count":0},
        "provider":{"artifact":"DWDSmor Open","edition":metrics["provider"]["edition"],"version":metrics["provider"]["version"],"source_commit":metrics["provider"]["source_commit"],"wheel_sha256":metrics["provider"]["wheel_sha256"],"execution_posture":"proposal-only-preserve-all-analyses-and-unknowns"},
        "source_input":{"work_ref":plan["source_lexical_index"]["work_ref"],"packet_ref":plan["a_census"]["local_packet"]["relative_path"],"packet_sha256":INPUT_SHA,"packet_bytes":packet.metadata.len(),"packet_mode":"0600","exact_form_row_count":raw["row_count"],"token_occurrence_count":raw["token_occurrence_count"],"exact_surface_mutated":false,"source_text_accepted":false,"rights_cleared":false},
        "runtime":{"runtime_id":runtime["runtime_id"],"manifest_sha256":execution["runtime_manifest_sha256"],"artifact_set_sha256":runtime["artifact_set_sha256"],"runtime_bytes":runtime["runtime_bytes"],"license":"GPL-2.0-only","network_used":false},
        "private_artifacts":records,"coverage":coverage,"distributions":distributions,"mechanical_unknown_residue":raw["unknown_residue"],
        "repeat_determinism":{"deterministic":true,"pass_1_stream_sha256":raw["stream_sha256"],"pass_2_stream_sha256":raw["stream_sha256"],"mismatch_count":0,"second_pass_raw_output_retained":false},"performance":performance,
        "accuracy":{"status":"unmeasured-no-german-competent-gold","german_competent_gold_count":0,"accepted_morphology_count":0,"accepted_lemma_count":0,"coverage_is_accuracy":false,"machine_agreement_is_gold":false},
        "followup":{"status":"blocked-not-materialized","b_acquired":false,"c_acquired":false,"human_work_scheduled":false,"mechanical_residue_is_reviewed_residue":false,"trigger":metrics["followup"]["trigger"]},
        "rights_and_visibility":{"private_source_and_raw_output":"owner-local-only","tracked_receipt_contains_source_strings":false,"tracked_receipt_contains_sequence":false,"tracked_receipt_contains_context":false,"tracked_receipt_contains_provider_lemma_strings":false,"source_payload_publication_authorized":false,"raw_output_publication_authorized":false,"tracked_receipt_publication_authorized":false,"future_site_route":"blocked"},
        "semantic_boundary":{"creates_accepted_source":false,"creates_morphology":false,"creates_lemma":false,"creates_lexeme":false,"creates_sign_candidate":false,"creates_sign":false,"creates_semantic_claim":false,"creates_translation":false,"creates_graph_fact":false,"opens_human_backlog":false},
        "authority_boundary":"This receipt records an exhaustive, deterministic, text-free mechanical provider census and its measured resource cost."});
    if opt.generation.is_none() {
        let old = read_json(ctx, RECEIPT, &mut source_held)?;
        ensure(
            source_held.last().unwrap().digest == OLD_RECEIPT_SHA,
            "unknown retained result receipt",
        )?;
        receipt["generator"] = old["generator"].clone();
        receipt["authority_boundary"] = old["authority_boundary"].clone();
        equal(&receipt, old, "historical morphology result")?;
    }
    super::validate(&schemas, "morphology-census-result-receipt", &receipt)?;
    for h in &mut held {
        h.verify(&artifacts)?;
    }
    for h in &mut source_held {
        h.verify(ctx)?;
    }
    runner.verify(&runner_ctx)?;
    runtime_file.verify(&runtime_ctx)?;
    packet.verify(&packet_ctx)?;
    let receipt_ref = opt
        .generation
        .map(|g| generated_path(RECEIPT, g))
        .transpose()?
        .unwrap_or(RECEIPT.into());
    if opt.generation.is_some() {
        let encoded = canonical(receipt.clone())?;
        let output = ctx.select_output_directory(opt.output_root, opt.build)?;
        let exists = present(&output, &receipt_ref, &encoded, 0o644)?;
        if opt.build && !exists {
            output.write(&receipt_ref, &encoded, 0o644, true)?;
        } else {
            ensure(exists, "result receipt missing")?;
        }
    }
    ctx.check()?;
    Ok(
        json!({"status":receipt["status"],"run_id":parts[1],"receipt_ref":receipt_ref,"covered_type_count":raw["covered_type_count"],"unknown_type_count":raw["unknown_type_count"],"accuracy":"unmeasured-no-german-competent-gold","followup":"blocked-not-materialized"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_unicode_unknown_buckets() {
        assert_eq!(case_bucket("ǅelta"), "title");
        assert_eq!(case_bucket("αβ🙂"), "lower");
        assert_eq!(case_bucket("A.B"), "upper");
        assert_eq!(case_bucket("Hello World"), "title");
        assert_eq!(character_bucket("x²"), "contains-digit");
        assert_eq!(character_bucket("alpha-beta"), "contains-joiner");
        assert_eq!(character_bucket("ß"), "alphabetic-only");
        assert_eq!(character_bucket("x\u{0301}"), "contains-other");
    }
}
#[cfg(test)]
mod retained_inspector_tests {
    use super::*;
    #[test]
    fn provider_census_recomputes_without_returning_source_strings() {
        let provider = json!({"artifact":"DWDSmor Open","version":"0.18.0","source_commit":"f97b92ce2a5d6db8750afbdb222eb39470e57cf6","wheel_sha256":"395a15e15286b0c191b42355b6e3c2a43c8959621ccf3563336c2e30399a2973","surface_normalized_before_analysis":false});
        let mut rows = Vec::new();
        for (text, count, analyses) in [
            ("bekannt", 4, json!([{"pos":"V","category":null}])),
            ("unbekannt", 2, json!([])),
        ] {
            let digest = sha(text.as_bytes());
            rows.push(json!({"schema_version":"tos_dwdsmor_analysis_row_v1","form_key":format!("lexical-form:sha256:{digest}"),"exact_form":text,"exact_form_sha256":digest,"normalized_form_sha256":"a".repeat(64),"occurrence_count":count,"input_preserved":true,"provider":provider,"lemma_analyses":analyses,"root_analyses":[],"lemma_analysis_count":analyses.as_array().unwrap().len(),"root_analysis_count":0,"unknown":analyses.as_array().unwrap().is_empty(),"authority":"unreviewed-provider-candidate"}));
        }
        rows.sort_by(|a, b| {
            a["exact_form_sha256"]
                .as_str()
                .cmp(&b["exact_form_sha256"].as_str())
        });
        let mut census = Census::default();
        for row in rows {
            census.row(&canonical(row).unwrap()).unwrap();
        }
        let raw = census.finish("test-stream").unwrap();
        for (k, n) in [
            ("row_count", 2),
            ("token_occurrence_count", 6),
            ("covered_type_count", 1),
            ("unknown_type_count", 1),
            ("unknown_token_count", 2),
            ("lemma_analysis_total", 1),
        ] {
            assert_eq!(raw[k], n);
        }
        let output = String::from_utf8(canonical(raw).unwrap()).unwrap();
        assert!(!output.contains("bekannt"));
    }
}
