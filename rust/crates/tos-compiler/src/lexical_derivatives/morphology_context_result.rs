//! Text-free registration of the retained contextual provider experiment.
use super::morphology_result::{
    EXPERIMENT, duration, equal, external, file_record, lines, mode, uint,
};
use super::semantic_recurrence::unicode_slice;
use super::{
    Held, META_CAP, ResearchExecution, Result, canonical, ensure, generated_path, present,
    read_json, s, sha, valid_generation,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};
pub const PLAN: &str = super::morphology_context::PLAN;
const BASE: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/";
const BUILDER: &str =
    "rust/crates/tos-compiler/src/lexical_derivatives/morphology_context_result.rs";
const RUN: &str = "zarathustra-zdl-b-20260812t1354z";
const PLAN_SHA: &str = "73a88433cda008de222f288027d4ac2fc1a97357a47a5a0e99d9e67f95a34785";
const CONTEXT_SHA: &str = "0baff212b114ae9d0781ad0e26cf93a25066ad42f371252dc8106d89440d936f";
const PACKET_SHA: &str = "d82a2d81b370f3131fb56e0b1328866120cb3313903ffdf115debb4f0e34aa66";
const FORM_SHA: &str = "0007489cd4b0a84b926a341d3540ae1e8a2ff9cfc2062069dbf5a4e994f6ef37";
const RUNNER_SHA: &str = "e86c0cafe281e73832a9475b5a54e4a12d75f26b8e8389a73e4595b9d6ffb672";
const RUNTIME_SHA: &str = "89c8a70b779c3782cfdf5b10bdd7963736c181afa86918a92d4d967aebdc820c";
const ARTIFACT_SET: &str = "591824890eefdbc9f2d79897640a47fdd5c128b87f7eb29e70203f5dda97f561";
const RECORD: &str = "sha256:d6356a650a3f033469759a380264cbe00c1401b5713f90b97df674e04363a80b";
const SUBJECT: &str = "sha256:9768ec14c9f4e0115bf285b7e50f24521cecbd7533c20fb00e0bb7204aba9ff4";
const SUBJECTS: &str = "sha256:df79c75c0e521bc5f1b379d7b06f27d7d433b5b8bac048fa5ed0ff088d48eed2";
const CONTROLS: [&str; 5] = [
    "abi_signature",
    "sbom",
    "ml_bom",
    "slsa_in_toto",
    "sigstore_cosign",
];
const PIPELINE: [&str; 6] = [
    "tok2vec",
    "tagger",
    "morphologizer",
    "parser",
    "ner",
    "trainable_lemmatizer",
];
const PROVIDER_COMMIT: &str = "7eabc17097a3ea39f5cc9c030a605ff7edc20ae4";
const WHEEL: &str = "9d35263ac80e80e9730ee21830ffdbe96cf256b72c71e30326ae5865456ade9a";
const EVENT_ID: &str = "tos.event.annotation.zarathustra-morphology-context-b-result.2026-08-12";
fn refs() -> [String; 4] {
    [
        format!("{BASE}morphology-contextual-episode.selected-form-b.receipt.v1.json"),
        format!("{BASE}morphology-contextual-episode.selected-form-b.artifact-admission.v1.json"),
        format!("{BASE}morphology-contextual-episode.selected-form-b.result.v1.json"),
        format!("{BASE}provenance.morphology-contextual-episode.selected-form-b.result.v1.jsonl"),
    ]
}
fn exact_keys(v: &Value, keys: &[&str], label: &str) -> Result<()> {
    let o = v.as_object().ok_or("provider object required")?;
    ensure(
        o.len() == keys.len() && keys.iter().all(|k| o.contains_key(*k)),
        label,
    )
}
fn token_string(v: &Value) -> Result<&str> {
    v.as_str().ok_or("provider string required".into())
}
fn token(v: &Value, context: &str, n: usize) -> Result<()> {
    exact_keys(
        v,
        &[
            "dep",
            "end_offset",
            "ent_type",
            "head_token_index",
            "is_sent_start",
            "lemma",
            "morph",
            "pos",
            "start_offset",
            "tag",
            "text",
            "token_index",
            "whitespace",
        ],
        "provider token shape drift",
    )?;
    let (start, end) = (uint(&v["start_offset"])?, uint(&v["end_offset"])?);
    ensure(
        start < end && unicode_slice(context, start, end)? == s(&v["text"])?,
        "provider token source return drift",
    )?;
    token_string(&v["whitespace"])?;
    ensure(
        v["head_token_index"].is_null()
            || v["head_token_index"].as_u64().is_some_and(|v| v < n as u64),
        "token head index drift",
    )?;
    ensure(v["morph"].is_object(), "token morphology object required")?;
    Ok(())
}
#[derive(Default)]
struct Contexts {
    rows: usize,
    exact: usize,
    counts: BTreeMap<String, u64>,
    pos: BTreeMap<String, u64>,
    tag: BTreeMap<String, u64>,
}
impl Contexts {
    fn row(&mut self, encoded: &[u8]) -> Result<()> {
        self.row_with_form(encoded, FORM_SHA)
    }
    fn row_with_form(&mut self, encoded: &[u8], form_sha: &str) -> Result<()> {
        ensure(self.rows < 3, "context row count bound")?;
        let r = crate::zarathustra_lexical::parse(encoded, META_CAP as usize)?;
        exact_keys(
            &r,
            &[
                "authority",
                "context_id",
                "context_sha256",
                "context_text",
                "episode_id",
                "exact_form_sha256",
                "form_key",
                "input_preserved",
                "item_ref",
                "occurrence_id",
                "part_order",
                "provider",
                "schema_version",
                "selection_rank",
                "selection_role",
                "target_end_offset",
                "target_exact_form",
                "target_start_offset",
                "target_tokens",
                "tokenization",
                "tokens",
            ],
            "provider context row fields drift",
        )?;
        ensure(
            canonical(r.clone())? == encoded,
            "provider context row must be canonical JSONL",
        )?;
        for (k, v) in [
            (
                "schema_version",
                json!("tos_zdl_contextual_morphology_row_v1"),
            ),
            (
                "episode_id",
                json!("zarathustra-selected-form-context-b-v1"),
            ),
            ("exact_form_sha256", json!(form_sha)),
            ("form_key", json!(format!("lexical-form:sha256:{form_sha}"))),
            ("input_preserved", json!(true)),
            (
                "authority",
                json!("unreviewed-contextual-provider-proposal"),
            ),
            ("selection_rank", json!([1, 73, 145][self.rows])),
            (
                "selection_role",
                json!(["first", "inclusive-median", "last"][self.rows]),
            ),
            ("part_order", json!([1, 3, 4][self.rows])),
        ] {
            equal(&r[k], v, k)?;
        }
        equal(
            &r["provider"],
            json!({"artifact":"ZDL de_zdl_lg","version":"4.0.0","source_commit":PROVIDER_COMMIT,"wheel_sha256":WHEEL,"spacy_version":"3.8.11","pipeline":PIPELINE,"surface_normalized_before_analysis":false,"confidence_scores_exposed":false}),
            "provider identity",
        )?;
        let context = s(&r["context_text"])?;
        let target = s(&r["target_exact_form"])?;
        let (start, end) = (
            uint(&r["target_start_offset"])?,
            uint(&r["target_end_offset"])?,
        );
        ensure(
            start < end
                && sha(context.as_bytes()) == s(&r["context_sha256"])?
                && sha(target.as_bytes()) == form_sha
                && unicode_slice(context, start, end)? == target,
            "provider context source return drift",
        )?;
        let tokens = r["tokens"]
            .as_array()
            .ok_or("provider token array required")?;
        let target_tokens = r["target_tokens"]
            .as_array()
            .ok_or("target token array required")?;
        ensure(
            !tokens.is_empty() && tokens.len() <= 10000 && !target_tokens.is_empty(),
            "provider token count bound",
        )?;
        let mut text = String::new();
        let mut expected = Vec::new();
        for (i, t) in tokens.iter().enumerate() {
            equal(&t["token_index"], json!(i), "provider token order")?;
            token(t, context, tokens.len())?;
            for piece in [s(&t["text"])?, token_string(&t["whitespace"])?] {
                ensure(
                    piece.len() <= META_CAP as usize - text.len(),
                    "context reconstruction byte bound",
                )?;
                text.push_str(piece);
            }
            if uint(&t["start_offset"])? < end && uint(&t["end_offset"])? > start {
                expected.push(t);
            }
        }
        ensure(text == context, "provider token reconstruction drift")?;
        ensure(
            target_tokens.iter().eq(expected.iter().copied()),
            "target token alignment drift",
        )?;
        let exact = target_tokens.len() == 1
            && uint(&target_tokens[0]["start_offset"])? == start
            && uint(&target_tokens[0]["end_offset"])? == end
            && s(&target_tokens[0]["text"])? == target;
        let covered = uint(&target_tokens[0]["start_offset"])? <= start
            && uint(&target_tokens.last().unwrap()["end_offset"])? >= end;
        ensure(covered, "target token coverage incomplete")?;
        equal(
            &r["tokenization"],
            json!({"token_count":tokens.len(),"target_token_count":target_tokens.len(),"exact_single_token_alignment":exact,"split_or_expanded_alignment":!exact,"target_covered":covered}),
            "tokenization summary",
        )?;
        self.rows += 1;
        self.exact += usize::from(exact);
        *self
            .counts
            .entry(target_tokens.len().to_string())
            .or_default() += 1;
        for t in target_tokens {
            for (key, map) in [("pos", &mut self.pos), ("tag", &mut self.tag)] {
                let label = token_string(&t[key])?;
                ensure(label.len() <= 128, "provider label bound")?;
                *map.entry(if label.is_empty() { "<none>" } else { label }.into())
                    .or_default() += 1;
            }
        }
        Ok(())
    }
    fn finish(self, digest: &str) -> Result<Value> {
        ensure(self.rows == 3, "frozen contextual selection incomplete")?;
        Ok(
            json!({"stream_sha256":digest,"row_count":3,"selection_ranks":[1,73,145],"selection_roles":["first","inclusive-median","last"],"part_orders":[1,3,4],"exact_single_token_alignment_count":self.exact,"split_or_expanded_alignment_count":3-self.exact,"target_token_count_distribution":self.counts,"target_pos":self.pos,"target_tag":self.tag}),
        )
    }
}
pub fn inspect(ctx: &ResearchExecution, path: &Path) -> Result<Value> {
    let (selected, mut h) = external(ctx, path, 16 * 1024 * 1024)?;
    mode(&h, true)?;
    let mut rows = Contexts::default();
    lines(&selected, &mut h, |r| rows.row(r))?;
    h.verify(&selected)?;
    let raw = rows.finish(&h.digest)?;
    super::morphology_result::label_contract(
        ctx,
        "morphology-contextual-result-receipt",
        &raw,
        &[("target_pos", "countMap"), ("target_tag", "countMap")],
    )?;
    Ok(raw)
}
pub struct Options<'a> {
    pub build: bool,
    pub artifact_root: &'a Path,
    pub run: &'a str,
    pub resource: &'a str,
    pub runtime_manifest: &'a Path,
    pub runner: &'a Path,
    pub source_packet: &'a Path,
    pub registry: &'a Path,
    pub gate_receipt: &'a Path,
    pub output_root: &'a Path,
    pub generation: Option<&'a str>,
    pub event_at: Option<&'a str>,
}
pub fn run(ctx: &ResearchExecution, opt: Options<'_>) -> Result<Value> {
    ensure(
        !opt.build || opt.generation.is_some(),
        "build requires a new generation",
    )?;
    ensure(
        opt.generation.is_some() == opt.event_at.is_some(),
        "generation and event-at must be paired",
    )?;
    if let Some(g) = opt.generation {
        ensure(valid_generation(g), "generation syntax")?;
    }
    ensure(
        opt.run == format!("{EXPERIMENT}/{RUN}/variant-B")
            && opt.resource == format!("{EXPERIMENT}/resource-runs/{RUN}.json"),
        "run/resource route drift",
    )?;
    if opt.build {
        for root in [
            ctx.root(),
            opt.artifact_root,
            opt.runtime_manifest.parent().ok_or("runtime parent")?,
            opt.runner.parent().ok_or("runner parent")?,
            opt.source_packet.parent().ok_or("packet parent")?,
            opt.registry.parent().ok_or("registry parent")?,
            opt.gate_receipt.parent().ok_or("gate parent")?,
        ] {
            ensure(
                !opt.output_root.starts_with(root) && !root.starts_with(opt.output_root),
                "result output must be separate from inputs",
            )?;
        }
    }
    let mut source_held = Vec::new();
    let schemas = super::schemas(
        ctx,
        &["morphology-contextual-result-receipt", "provenance-event"],
        &mut source_held,
    )?;
    let [context_ref, admission_ref, result_ref, event_ref] = refs();
    let plan = read_json(ctx, PLAN, &mut source_held)?;
    ensure(
        source_held.last().unwrap().digest == PLAN_SHA,
        "context plan digest drift",
    )?;
    let context_receipt = read_json(ctx, &context_ref, &mut source_held)?;
    ensure(
        source_held.last().unwrap().digest == CONTEXT_SHA,
        "context receipt digest drift",
    )?;
    let admission = read_json(ctx, &admission_ref, &mut source_held)?;
    let admission_sha = source_held.last().unwrap().digest.clone();
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
        ("metrics", "metrics/contextual-summary.json", false),
    ] {
        let v = read_json(&artifacts, &format!("{}/{rel}", opt.run), &mut held)?;
        records[name] = file_record(held.last().unwrap(), private)?;
        values.insert(name, v);
    }
    let resource = read_json(&artifacts, opt.resource, &mut held)?;
    records["resource_launch"] = file_record(held.last().unwrap(), false)?;
    let (run, experiment, preflight, execution, repeat, metrics) = (
        &values["run_receipt"],
        &values["experiment_spec"],
        &values["preflight"],
        &values["execution"],
        &values["repeat_determinism"],
        &values["metrics"],
    );
    let mut stream = Held::open(
        &artifacts,
        &format!("{}/raw-output/zdl-contextual-morphology.jsonl", opt.run),
        16 * 1024 * 1024,
    )?;
    records["raw_output"] = file_record(&stream, true)?;
    let mut contexts = Contexts::default();
    lines(&artifacts, &mut stream, |r| contexts.row(r))?;
    let raw = contexts.finish(&stream.digest)?;
    held.push(stream);
    for (k, v) in [
        ("run_id", json!(RUN)),
        ("experiment_id", json!(EXPERIMENT)),
        ("variant", json!("B")),
        ("status", json!("awaiting-triggered-review")),
        ("manual_review_refs", json!([])),
        ("model_inspection_refs", json!([])),
        ("errors", json!([])),
        ("retention_decision", json!("retain")),
    ] {
        equal(&run[k], v, &format!("run {k}"))?;
    }
    equal(
        &experiment["experiment_id"],
        json!(EXPERIMENT),
        "experiment",
    )?;
    equal(
        &experiment["source_plan_sha256_by_variant"]["B"],
        json!(PLAN_SHA),
        "experiment B plan",
    )?;
    equal(&preflight["decision"], json!("ready"), "preflight")?;
    equal(&preflight["variant"], json!("B"), "preflight variant")?;
    for k in ["source_plan_admission", "runtime_admission"] {
        equal(&preflight[k]["verified"], json!(true), k)?;
    }
    for (k, v) in [
        (
            "schema_version",
            json!("tos_zdl_contextual_morphology_execution_v1"),
        ),
        ("source_plan_sha256", json!(PLAN_SHA)),
        ("source_packet_sha256", json!(PACKET_SHA)),
        ("runner_sha256", json!(RUNNER_SHA)),
        ("runtime_manifest_sha256", json!(RUNTIME_SHA)),
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
    ensure(runner.digest == RUNNER_SHA, "runner digest drift")?;
    let (packet_ctx, mut packet) = external(ctx, opt.source_packet, 16 * 1024 * 1024)?;
    ensure(packet.digest == PACKET_SHA, "private packet digest drift")?;
    mode(&packet, true)?;
    let (runtime_ctx, mut runtime_file) = external(ctx, opt.runtime_manifest, META_CAP)?;
    ensure(
        runtime_file.digest == RUNTIME_SHA,
        "runtime manifest digest drift",
    )?;
    let runtime = crate::zarathustra_lexical::parse(
        &runtime_ctx.read_file(&mut runtime_file.file, META_CAP)?,
        META_CAP as usize,
    )?;
    records["runtime_manifest"] = file_record(&runtime_file, false)?;
    let (registry_ctx, mut registry_file) = external(ctx, opt.registry, META_CAP)?;
    let registry = crate::zarathustra_lexical::parse(
        &registry_ctx.read_file(&mut registry_file.file, META_CAP)?,
        META_CAP as usize,
    )?;
    records["registry_record"] = file_record(&registry_file, false)?;
    for (k, v) in [
        ("status", json!("verified")),
        ("runtime_id", json!("zdl-de-zdl-lg-4.0.0-7eabc170-py312")),
        ("experiment_id", json!(EXPERIMENT)),
        ("variant", json!("B")),
        ("artifact_set_sha256", json!(ARTIFACT_SET)),
    ] {
        equal(&runtime[k], v, k)?;
    }
    let gate = &runtime["artifact_admission"];
    for (k, v) in [
        ("verdict", json!("allow")),
        ("record_id", json!(RECORD)),
        ("subject_digest", json!(SUBJECT)),
        ("subjects_aggregate_digest", json!(SUBJECTS)),
        ("trust_root_mode", json!("local_dev")),
    ] {
        equal(&gate[k], v, &format!("gate {k}"))?;
    }
    let (gate_ctx, mut gate_file) = external(ctx, opt.gate_receipt, META_CAP)?;
    equal(
        &gate["gate_receipt_sha256"],
        json!(gate_file.digest),
        "gate receipt digest",
    )?;
    for (k, v) in [
        ("record_id", json!(RECORD)),
        ("subject_digest", json!(SUBJECT)),
        ("artifact_subjects_digest", json!(SUBJECTS)),
        ("latest_eligible", json!(true)),
        ("lifecycle_state", json!("manually-verified")),
        ("trust_root_mode", json!("local_dev")),
        ("verification_ok", json!(true)),
        ("required_controls", json!(CONTROLS)),
        ("present_controls", json!(CONTROLS)),
        ("verified_controls", json!(CONTROLS)),
    ] {
        equal(&registry[k], v, &format!("registry {k}"))?;
    }
    equal(
        &registry["artifact_subject_store"]["ok"],
        json!(true),
        "subject store",
    )?;
    equal(
        &registry["artifact_subject_store"]["files"],
        json!(44),
        "subject store files",
    )?;
    equal(
        &metrics["experiment_id"],
        json!(EXPERIMENT),
        "metrics experiment",
    )?;
    equal(&metrics["variant"], json!("B"), "metrics variant")?;
    equal(
        &metrics["provider"],
        json!({"model_version":"4.0.0","spacy_version":"3.8.11","pipeline":PIPELINE,"source_commit":PROVIDER_COMMIT,"wheel_sha256":WHEEL,"confidence_scores_exposed":false}),
        "metrics provider",
    )?;
    equal(
        &metrics["input"],
        json!({"plan_sha256":PLAN_SHA,"packet_sha256":PACKET_SHA,"row_count":3,"selection_ranks":[1,73,145],"selection_roles":["first","inclusive-median","last"],"exact_surface_mutated":false,"b_output_visible_during_selection":false}),
        "metrics input",
    )?;
    equal(
        &metrics["tokenization"],
        json!({"exact_single_token_alignment_count":raw["exact_single_token_alignment_count"],"split_or_expanded_alignment_count":raw["split_or_expanded_alignment_count"],"target_token_count_distribution":raw["target_token_count_distribution"]}),
        "tokenization",
    )?;
    equal(
        &metrics["proposal_distributions"],
        json!({"target_pos":raw["target_pos"],"target_tag":raw["target_tag"]}),
        "proposal distributions",
    )?;
    equal(
        &metrics["quality"],
        json!({"status":"unmeasured-no-german-competent-gold","provider_proposal_is_accuracy":false,"machine_agreement_or_disagreement_is_gold":false}),
        "quality boundary",
    )?;
    equal(
        &metrics["followup"],
        json!({"status":"closed-machine-proposal-awaiting-real-trigger","c_status":"blocked-question-inapplicable","human_work_scheduled":false,"semantic_effect":false}),
        "followup boundary",
    )?;
    equal(
        &metrics["rights"],
        json!({"execution":"owner-local-private-research-only","redistribution":"blocked","source_content_public":false}),
        "rights boundary",
    )?;
    equal(
        &metrics["bytes"],
        json!({"runtime":runtime["runtime_bytes"],"raw_output":records["raw_output"]["bytes"],"source_packet":packet.metadata.len(),"metrics":records["metrics"]["bytes"]}),
        "byte accounting",
    )?;
    for (k, v) in [
        ("deterministic", json!(true)),
        ("mismatch_count", json!(0)),
        ("row_count", json!(3)),
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
    equal(&resource["ok"], json!(true), "resource launch")?;
    equal(
        &resource["execution"]["returncode"],
        json!(0),
        "resource return code",
    )?;
    equal(
        &resource["request"]["force"],
        json!(false),
        "resource force",
    )?;
    let systemd = &resource["execution"]["systemd"];
    equal(&systemd["result"], json!("success"), "resource service")?;
    let peaks = &resource["startup_admission"]["demand_observation"]["peaks"];
    equal(&peaks["ok"], json!(true), "resource peaks")?;
    for (label, v) in [
        ("plan status", &plan["status"]),
        (
            "context receipt state",
            &context_receipt["variant_state"]["b"],
        ),
        ("historical admission", &admission["status"]),
    ] {
        ensure(
            [
                "ready-to-materialize-context-packet",
                "admitted-unacquired",
                "artifact-acquired-admission-denied-b-not-run",
            ]
            .contains(&s(v)?),
            label,
        )?;
    }
    let builder = Held::open(ctx, BUILDER, META_CAP)?;
    ensure(
        builder.digest == sha(include_bytes!("morphology_context_result.rs")),
        "running context result generator source drift",
    )?;
    let builder_sha = builder.digest.clone();
    source_held.push(builder);
    let suffix = opt
        .generation
        .map(|g| format!(".native-{g}"))
        .unwrap_or_default();
    let event_id = format!("{EVENT_ID}{suffix}");
    let output_ref = opt
        .generation
        .map(|g| generated_path(&result_ref, g))
        .transpose()?
        .unwrap_or(result_ref.clone());
    let output_event = opt
        .generation
        .map(|g| generated_path(&event_ref, g))
        .transpose()?
        .unwrap_or(event_ref.clone());
    let mut performance = metrics["performance"].clone();
    ensure(performance.is_object(), "performance object required")?;
    performance["host_service_wall_seconds"] = json!(duration(&systemd["service_runtime"])?);
    performance["host_service_cpu_seconds"] = json!(duration(&systemd["cpu_time_consumed"])?);
    for (k, source) in [
        ("host_cgroup_footprint_peak_mib", "footprint_peak_mib"),
        ("host_cgroup_memory_peak_bytes", "memory_peak_bytes"),
        ("host_cgroup_swap_peak_bytes", "memory_swap_peak_bytes"),
    ] {
        performance[k] = peaks[source].clone();
    }
    performance["resource_force_used"] = json!(false);
    let mut receipt = json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/morphology-contextual-result-receipt.schema.json","schema_version":"tos_morphology_contextual_result_receipt_v1","generated_or_authored":"generated_from_private_contextual_provider_run","receipt_id":format!("morphology-contextual-result:zarathustra-selected-form-b.zdl-4.0.0.v1{suffix}"),"recorded_at_utc":run["finished_at_utc"],"status":"b-executed-machine-proposal-awaiting-real-trigger","experiment_id":EXPERIMENT,"variant":"B",
        "question":{"plan":{"ref":PLAN,"sha256":PLAN_SHA},"context_receipt":{"ref":context_ref,"sha256":CONTEXT_SHA},"historical_negative_admission":{"ref":admission_ref,"sha256":admission_sha,"retained":true,"superseded":false},"frozen_before_b_output":true},
        "generator":{"ref":BUILDER,"sha256":builder_sha},"private_run":{"artifact_owner":"abyss-machine:storage/artifacts/tree-of-sophia-foundation-lab","relative_ref":opt.run,"run_id":RUN,"status":"awaiting-triggered-review","started_at_utc":run["started_at_utc"],"finished_at_utc":run["finished_at_utc"],"retention_decision":"retain","visibility":"owner-local-only","manual_review_count":0,"model_inspection_count":0},
        "implementation":{"owner":"abyss-stack","commit":"a321401b792b24e9b7dd6cbae4e7085b8fafe0e2","runner_sha256":RUNNER_SHA},
        "provider":{"artifact":"ZDL de_zdl_lg","version":"4.0.0","spacy_version":"3.8.11","source_commit":PROVIDER_COMMIT,"principal_wheel_sha256":WHEEL,"pipeline":PIPELINE,"confidence_scores_exposed":false,"execution_posture":"unreviewed-contextual-provider-proposal"},
        "artifact_admission":{"record_id":RECORD,"record_file_sha256":registry_file.digest,"subject_digest":SUBJECT,"subjects_aggregate_digest":SUBJECTS,"lifecycle_state":"manually-verified","latest_eligible":true,"trust_root_mode":"local_dev","required_controls":CONTROLS,"present_controls":CONTROLS,"verified_controls":CONTROLS,"subject_store_file_count":44,"trust_gate_verdict":"allow","rights_effect":"none"},
        "source_input":{"work_ref":"tos.work.friedrich-nietzsche.also-sprach-zarathustra","packet_sha256":PACKET_SHA,"packet_bytes":packet.metadata.len(),"packet_mode":"0600","row_count":3,"selection_ranks":[1,73,145],"selection_roles":["first","inclusive-median","last"],"part_orders":[1,3,4],"exact_surface_mutated":false,"source_text_accepted":false},
        "runtime":{"runtime_id":runtime["runtime_id"],"manifest_sha256":RUNTIME_SHA,"artifact_set_sha256":ARTIFACT_SET,"runtime_bytes":runtime["runtime_bytes"],"network_used":false},"private_artifacts":records,
        "tokenization":metrics["tokenization"],"proposal_distributions":metrics["proposal_distributions"],"repeat_determinism":{"deterministic":true,"pass_1_stream_sha256":raw["stream_sha256"],"pass_2_stream_sha256":raw["stream_sha256"],"mismatch_count":0,"second_pass_raw_output_retained":false},"performance":performance,
        "quality":{"status":"unmeasured-no-german-competent-gold","german_competent_gold_count":0,"accepted_tokenization_count":0,"accepted_morphology_count":0,"accepted_lemma_count":0,"provider_proposal_is_accuracy":false,"machine_repeatability_is_gold":false},
        "followup":{"status":"closed-machine-proposal-awaiting-real-trigger","c_status":"blocked-question-inapplicable","human_work_scheduled":false,"automatic_review_opened":false,"automatic_promotion_authorized":false},
        "rights_and_visibility":{"execution":"owner-local-private-research-only","redistribution":"blocked","private_source_and_raw_output":"owner-local-only","tracked_receipt_contains_source_strings":false,"tracked_receipt_contains_sequence":false,"tracked_receipt_contains_context":false,"tracked_receipt_contains_provider_lemma_strings":false,"source_payload_publication_authorized":false,"raw_output_publication_authorized":false,"tracked_receipt_publication_authorized":false,"future_site_route":"blocked"},
        "semantic_boundary":{"creates_accepted_source":false,"creates_tokenization":false,"creates_morphology":false,"creates_lemma":false,"creates_lexeme":false,"creates_sign_candidate":false,"creates_sign":false,"creates_semantic_claim":false,"creates_translation":false,"creates_graph_fact":false,"changes_canon":false,"opens_human_backlog":false},"provenance_event_ref":event_id,
        "authority_boundary":"This receipt records one private, deterministic, source-bound contextual provider execution and its measured resource cost."});
    if opt.generation.is_none() {
        let old = read_json(ctx, &result_ref, &mut source_held)?;
        ensure(
            source_held.last().unwrap().digest == OLD_RECEIPT_SHA,
            "unknown retained context result",
        )?;
        receipt["generator"] = old["generator"].clone();
        receipt["authority_boundary"] = old["authority_boundary"].clone();
        equal(&receipt, old, "historical context result")?;
    }
    super::validate(&schemas, "morphology-contextual-result-receipt", &receipt)?;
    let encoded = if opt.generation.is_none() {
        let h = source_held.last_mut().unwrap();
        h.verify(ctx)?;
        ctx.read_file(&mut h.file, META_CAP)?
    } else {
        canonical(receipt.clone())?
    };
    let mut event = json!({"schema_version":"tos_provenance_event_v1","event_id":event_id,"event_type":"annotation","started_at":opt.event_at.map(Value::from).unwrap_or(run["started_at_utc"].clone()),"ended_at":opt.event_at.map(Value::from).unwrap_or(run["finished_at_utc"].clone()),"agent_refs":["software:tos-native-rust"],
        "inputs":[{"ref":PLAN,"role":"frozen-output-blind-contextual-plan","sha256":PLAN_SHA},{"ref":context_ref,"role":"text-free-context-packet-receipt","sha256":CONTEXT_SHA},{"ref":admission_ref,"role":"retained-historical-negative-artifact-admission","sha256":admission_sha},{"ref":format!("owner-local-artifacts/tree-of-sophia-foundation-lab/{}/run.receipt.json",opt.run),"role":"private-contextual-run-receipt","sha256":records["run_receipt"]["sha256"]},{"ref":format!("owner-local-artifacts/tree-of-sophia-foundation-lab/{}/metrics/contextual-summary.json",opt.run),"role":"private-text-free-metrics","sha256":records["metrics"]["sha256"]},{"ref":format!("owner-local-artifacts/tree-of-sophia-foundation-lab/{}",opt.resource),"role":"owner-resource-run-receipt","sha256":records["resource_launch"]["sha256"]}],
        "outputs":[{"ref":output_ref,"role":"tracked-text-free-contextual-result-receipt","sha256":sha(&encoded)}],
        "method":{"maker_type":"software","name":"Tree of Sophia private contextual morphology result recorder","version":"1","artifact_digest":receipt["generator"]["sha256"],"runtime":"Rust bounded retained laboratory result verifier","device":"CPU","configuration":{"variant":"B","row_count":3,"selection_ranks":[1,73,145],"source_strings_tracked":false,"german_competent_gold_count":0,"human_work_scheduled":false,"semantic_effect":false,"redistribution":"blocked"},"prompt_or_instruction_ref":"ToS/research-packets/foundation-laboratory-2026-07/HISTORICAL_GERMAN_MORPHOLOGY_B_EXECUTABLE_STOP_LINE_2026-08-12.md"},
        "status":"completed_with_warnings","warnings":["provider output is an unreviewed machine proposal and German accuracy remains unmeasured","source packet and raw provider output remain owner-local and redistribution-blocked","the earlier denied artifact remains retained historical evidence rather than being rewritten","linguistic, semantic, graph, canon, publication and human-backlog decisions retain their existing owners and status"],"receipt_refs":[output_ref],"rights_basis_ref":null,"event_version":1,"supersedes_event_ref":null});
    if opt.generation.is_none() {
        let old = read_json(ctx, &event_ref, &mut source_held)?;
        ensure(
            source_held.last().unwrap().digest == OLD_EVENT_SHA,
            "unknown retained context result provenance",
        )?;
        event["agent_refs"] = old["agent_refs"].clone();
        event["method"]["runtime"] = old["method"]["runtime"].clone();
        equal(&event, old, "historical context result provenance")?;
    }
    super::validate(&schemas, "provenance-event", &event)?;
    let event_bytes = canonical(event)?;
    for h in &mut source_held {
        h.verify(ctx)?;
    }
    for h in &mut held {
        h.verify(&artifacts)?;
    }
    runner.verify(&runner_ctx)?;
    packet.verify(&packet_ctx)?;
    runtime_file.verify(&runtime_ctx)?;
    registry_file.verify(&registry_ctx)?;
    gate_file.verify(&gate_ctx)?;
    if opt.generation.is_some() {
        let output = ctx.select_output_directory(opt.output_root, opt.build)?;
        let a = present(&output, &output_ref, &encoded, 0o644)?;
        let b = present(&output, &output_event, &event_bytes, 0o644)?;
        if opt.build {
            if !a {
                output.write(&output_ref, &encoded, 0o644, true)?;
            }
            if !b {
                output.write(&output_event, &event_bytes, 0o644, true)?;
            }
        } else {
            ensure(a && b, "context result outputs missing")?;
        }
    }
    ctx.check()?;
    Ok(
        json!({"status":receipt["status"],"run_id":RUN,"receipt_ref":output_ref,"provenance_ref":output_event,"row_count":3,"exact_single_token_alignment_count":raw["exact_single_token_alignment_count"],"quality":"unmeasured-no-german-competent-gold"}),
    )
}
const OLD_RECEIPT_SHA: &str = "ff401fe42c3f2c8a496f6b944e85d348e2e952017ebe38cb3604b69cb62346ba";
const OLD_EVENT_SHA: &str = "b6d3fdcd174881bd7e22c255a312755eef7416c4d9c8be34613505c5f2c73be8";
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_token_source_return_and_bounds() {
        let mut v = json!({"dep":"","end_offset":2,"ent_type":"","head_token_index":null,"is_sent_start":true,"lemma":"x","morph":{},"pos":"ADV","start_offset":1,"tag":"","text":"🙂","token_index":0,"whitespace":""});
        token(&v, "α🙂x", 1).unwrap();
        v["end_offset"] = json!(3);
        assert!(token(&v, "α🙂x", 1).is_err());
        v["end_offset"] = json!(2);
        v["head_token_index"] = json!(true);
        assert!(token(&v, "α🙂x", 1).is_err());
    }
}
#[cfg(test)]
mod retained_inspector_tests {
    use super::*;
    #[test]
    fn contextual_aggregate_preserves_offsets_without_returning_source() {
        let surface = "Testform";
        let context = "Alpha Testform omega";
        let form_sha = sha(surface.as_bytes());
        let provider = json!({"artifact":"ZDL de_zdl_lg","version":"4.0.0","source_commit":PROVIDER_COMMIT,"wheel_sha256":WHEEL,"spacy_version":"3.8.11","pipeline":PIPELINE,"surface_normalized_before_analysis":false,"confidence_scores_exposed":false});
        let mut tokens = Vec::new();
        let mut cursor = 0;
        for (i, (text, ws, pos, tag)) in [
            ("Alpha", " ", "NOUN", "NN"),
            (surface, " ", "ADV", "ADV"),
            ("omega", "", "NOUN", "NN"),
        ]
        .into_iter()
        .enumerate()
        {
            tokens.push(json!({"dep":"dep","end_offset":cursor+text.len(),"ent_type":"","head_token_index":i,"is_sent_start":i==0,"lemma":text.to_lowercase(),"morph":{},"pos":pos,"start_offset":cursor,"tag":tag,"text":text,"token_index":i,"whitespace":ws}));
            cursor += text.len() + ws.len();
        }
        let mut contexts = Contexts::default();
        for (rank, role, part) in [
            (1, "first", 1),
            (73, "inclusive-median", 3),
            (145, "last", 4),
        ] {
            let row = json!({"authority":"unreviewed-contextual-provider-proposal","context_id":format!("private-{rank}"),"context_sha256":sha(context.as_bytes()),"context_text":context,"episode_id":"zarathustra-selected-form-context-b-v1","exact_form_sha256":form_sha,"form_key":format!("lexical-form:sha256:{form_sha}"),"input_preserved":true,"item_ref":"tos.item.private","occurrence_id":format!("tos.occurrence.private-{rank}"),"part_order":part,"provider":provider,"schema_version":"tos_zdl_contextual_morphology_row_v1","selection_rank":rank,"selection_role":role,"target_end_offset":14,"target_exact_form":surface,"target_start_offset":6,"target_tokens":[tokens[1]],"tokenization":{"token_count":3,"target_token_count":1,"exact_single_token_alignment":true,"split_or_expanded_alignment":false,"target_covered":true},"tokens":tokens});
            contexts
                .row_with_form(&canonical(row).unwrap(), &form_sha)
                .unwrap();
        }
        let output = contexts.finish("test-stream").unwrap();
        assert_eq!(output["exact_single_token_alignment_count"], 3);
        assert_eq!(output["target_pos"], json!({"ADV":3}));
        let text = String::from_utf8(canonical(output).unwrap()).unwrap();
        assert!(!text.contains(surface) && !text.contains(context));
    }
}
