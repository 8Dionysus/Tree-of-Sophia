//! Preserve an unattested observation and construct a bounded collation proposal.
//! Exact inputs, current rights and source/semantic authority remain independent.
use crate::{
    research_execution::ResearchExecution,
    research_text_comparison::{opcodes, space},
    source_text_foundation::{
        encode, ensure, fresh_or_matching_limit, metadata, private_boundary,
        private_input_boundary, s, schema, sha, utc_now,
    },
    transfer_target_passages::{n, read_optional},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    os::unix::fs::PermissionsExt,
    path::Path,
};
use unicode_normalization::UnicodeNormalization;
#[path = "antonovsky_collation/records.rs"]
mod records;
type Result<T> = std::result::Result<T, String>;
const CAP: usize = 4 * 1024 * 1024;
const PLAN: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/antonovsky-2007-1911-opening-sentence-collation.plan.v1.json";
const EVENT: &str =
    "tos.event.alignment.antonovsky-2007-1911-opening-sentence-collation.2026-08-12";
const BUILDER: &str = "rust/crates/tos-compiler/src/antonovsky_collation.rs";
const LEGACY: &str = "scripts/build_antonovsky_2007_1911_collation.py";
const SOFTWARE_AGENT: &str = "software:tos-antonovsky-2007-1911-collation-builder";
fn a(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or("required array".into())
}
fn parsed(raw: &[u8]) -> Result<Value> {
    tos_foundation::parse_json(
        raw,
        tos_foundation::JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::default(),
    )
    .map_err(|e| e.to_string())?;
    serde_json::from_slice(raw).map_err(|e| e.to_string())
}
fn utf8(raw: &[u8]) -> Result<&str> {
    std::str::from_utf8(raw).map_err(|_| "private input UTF8".into())
}
fn canonical(v: &Value) -> Result<Vec<u8>> {
    fn sort(v: &Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(
                m.iter()
                    .map(|(k, v)| (k.clone(), sort(v)))
                    .collect::<BTreeMap<_, _>>()
                    .into_iter()
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(sort).collect()),
            _ => v.clone(),
        }
    }
    encode(&sort(v), false)
}
struct Inputs<'a> {
    root: &'a ResearchExecution,
    files: Vec<(String, File, String, u64)>,
}
impl<'a> Inputs<'a> {
    fn new(root: &'a ResearchExecution) -> Self {
        Self {
            root,
            files: vec![],
        }
    }
    fn read(&mut self, r: &str, digest: Option<&str>) -> Result<Vec<u8>> {
        let mut f = self.root.source_file(r, CAP as u64)?;
        let raw = self.root.read_file(&mut f, CAP as u64)?;
        let actual = sha(&raw);
        ensure(digest.is_none_or(|d| d == actual), "input digest drift")?;
        self.files.push((r.into(), f, actual, raw.len() as u64));
        Ok(raw)
    }
    fn json(&mut self, r: &str, digest: Option<&str>) -> Result<Value> {
        parsed(&self.read(r, digest)?)
    }
    fn large(&mut self, r: &str, digest: &str) -> Result<u64> {
        let mut f = self.root.source_file(r, 256 * 1024 * 1024)?;
        let size = f.metadata().map_err(|e| e.to_string())?.len();
        ensure(
            self.root.hash_file(&mut f, 256 * 1024 * 1024)? == digest,
            "witness payload digest drift",
        )?;
        self.files.push((r.into(), f, digest.into(), size));
        Ok(size)
    }
    fn verify(&mut self) -> Result<()> {
        for (r, f, digest, size) in &mut self.files {
            ensure(
                self.root.hash_file(f, 256 * 1024 * 1024)? == *digest,
                "held input changed",
            )?;
            let mut current = self.root.source_file(r, *size)?;
            ensure(
                self.root.hash_file(&mut current, *size)? == *digest,
                "input path changed",
            )?;
        }
        Ok(())
    }
    fn details(&self, r: &str) -> Result<(String, u64)> {
        self.files
            .iter()
            .find(|(p, _, _, _)| p == r)
            .map(|(_, _, h, n)| (h.clone(), *n))
            .ok_or("input was not checked".into())
    }
}
fn observation(artifact: &mut Inputs<'_>, plan: &Value) -> Result<(String, Value)> {
    let e = &plan["human_observation"];
    let mut records = vec![];
    for key in ["autosave", "closure", "closure_receipt"] {
        records.push(artifact.json(
            s(&e[format!("{key}_relative_path")])?,
            Some(s(&e[format!("{key}_sha256")])?),
        )?);
    }
    let (auto, closure, receipt) = (&records[0], &records[1], &records[2]);
    ensure(
        auto["schema_version"] == "tos_human_review_workbench_state_v1"
            && auto["protocol_id"] == e["expected_protocol_id"]
            && auto["status"] == e["expected_state_status"]
            && auto["submitted_at_utc"].is_null()
            && auto["reviewer_ref"] == e["reviewer_ref"],
        "Workbench identity/status drift",
    )?;
    ensure(
        closure["schema_version"] == "tos_sparse_calibration_closure_v1"
            && closure["status"] == "closed-no-human-debt"
            && closure["attestation_status"] == "not-collected"
            && closure["promotion_authorized"] == false
            && closure["source_autosave_sha256"] == e["autosave_sha256"]
            && closure["counts"]["human_debt_units"] == 0,
        "closure attestation, promotion or debt drift",
    )?;
    let selected = a(&closure["selected_calibration_units"])?;
    ensure(selected.len() == 1, "selected observation count")?;
    let selected = &selected[0];
    ensure(
        selected["unit_id"] == e["sample_id"]
            && selected["classification"] == "rare-independent-transcription-calibration"
            && selected["attestation_status"] == "not-collected"
            && selected["promotion_authorized"] == false
            && selected["active_seconds"].as_f64() == e["expected_active_seconds"].as_f64(),
        "selected observation status drift",
    )?;
    ensure(
        receipt["schema_version"] == "tos_sparse_calibration_closure_receipt_v1"
            && receipt["status"] == "closure-fixed"
            && receipt["closure_sha256"] == e["closure_sha256"]
            && receipt["source_autosave_sha256"] == e["autosave_sha256"],
        "closure receipt binding drift",
    )?;
    let rows = a(&auto["rows"])?
        .iter()
        .filter(|r| r["unit_id"] == e["sample_id"])
        .collect::<Vec<_>>();
    ensure(rows.len() == 1, "observation row count")?;
    let row = rows[0];
    let values = &row["values"];
    ensure(values.is_object(), "observation values")?;
    let text = s(&values["diplomatic_transcription"])?;
    ensure(
        values["decision"] == e["expected_decision"]
            && row["active_seconds"].as_f64() == e["expected_active_seconds"].as_f64()
            && e["expected_active_seconds"].as_f64().is_some()
            && selected["calibration_transcription_sha256"] == sha(text.as_bytes()),
        "observation value binding drift",
    )?;
    let fields = a(&selected["observed_fields"])?
        .iter()
        .map(s)
        .collect::<Result<BTreeSet<_>>>()?;
    ensure(
        fields
            == BTreeSet::from([
                "decision",
                "diplomatic_transcription",
                "layout_and_reading_order",
                "page_and_region_resolved",
                "source_damage_or_ambiguity",
                "source_legibility",
            ]),
        "observed fields drift",
    )?;
    s(&values["source_damage_or_ambiguity"])?;
    Ok((text.into(), values.clone()))
}
fn select_source(text: &str, source: &Value) -> Result<(String, String, String, String)> {
    let chars = text.chars().collect::<Vec<_>>();
    ensure(
        chars.len() == n(&source["text_layer_codepoints"])?
            && text.len() == n(&source["text_layer_bytes"])?
            && sha(text.as_bytes()) == s(&source["text_layer_sha256"])?,
        "observation text length/fixity drift",
    )?;
    let dots = chars
        .iter()
        .enumerate()
        .filter_map(|(i, &c)| (c == '.').then_some(i))
        .take(2)
        .collect::<Vec<_>>();
    ensure(dots.len() == 2, "two sentence guards required")?;
    let heading_end = dots[0] + 1;
    let end = dots[1] + 1;
    let mut start = heading_end;
    while start < end && space(chars[start]) {
        start += 1;
    }
    ensure(
        heading_end == n(&source["heading_end"])?
            && start == n(&source["sentence_start"])?
            && end == n(&source["sentence_end"])?,
        "source sentence selection drift",
    )?;
    let mut spans = vec![];
    for key in ["heading", "interstitial", "sentence", "remainder"] {
        let start = n(&source[format!("{key}_start")])?;
        let end = n(&source[format!("{key}_end")])?;
        ensure(start <= end && end <= chars.len(), "source selector bounds")?;
        let value = chars[start..end].iter().collect::<String>();
        if key != "interstitial" {
            ensure(
                sha(value.as_bytes()) == s(&source[format!("{key}_sha256")])?,
                "source span digest drift",
            )?;
        }
        spans.push(value);
    }
    ensure(
        !spans[1].is_empty()
            && spans[1].chars().all(space)
            && spans[2].chars().count() == n(&source["sentence_codepoints"])?
            && spans[2].len() == n(&source["sentence_bytes"])?
            && !spans[2].starts_with(space)
            && spans[2].ends_with('.')
            && n(&source["remainder_end"])? == chars.len(),
        "source span scope/whitespace drift",
    )?;
    let mut it = spans.into_iter();
    Ok((
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
    ))
}
fn verify_witnesses(
    tracked: &mut Inputs<'_>,
    local: &mut Inputs<'_>,
    plan: &Value,
) -> Result<String> {
    let source = &plan["witness_2007"];
    private_input_boundary(tracked.root, s(&source["source_relative_ref"])?)?;
    local.large(
        s(&source["source_relative_ref"])?,
        s(&source["file_sha256"])?,
    )?;
    let rights = tracked.json(
        s(&source["rights_ref"])?,
        Some(s(&source["rights_sha256"])?),
    )?;
    ensure(
        rights["visibility"] == "local_only"
            && rights["redistribution_posture"] == "not_authorized"
            && rights["derivative_posture"] == "local_research_only"
            && rights["review_status"] == "unreviewed",
        "2007 rights posture drift",
    )?;
    let manifest = tracked.json(s(&source["item_manifest_ref"])?, None)?;
    let payloads = a(&manifest["payload_files"])?;
    ensure(
        manifest["item_id"] == source["item_ref"]
            && payloads.len() == 1
            && payloads[0]["file_id"] == source["file_ref"]
            && payloads[0]["sha256"] == source["file_sha256"],
        "2007 manifest binding drift",
    )?;
    let inventory = tracked.json(s(&source["resource_inventory_ref"])?, None)?;
    let mut resources = vec![];
    for file in a(&inventory["files"])? {
        if file["file_id"] == source["file_ref"] {
            resources.extend(
                a(&file["resources"])?
                    .iter()
                    .filter(|r| r["resource_id"] == source["page_resource_id"]),
            );
        }
    }
    ensure(
        resources.len() == 1
            && resources[0]["locator"]
                == json!({"page_index":source["page_number"],"width_points":source["page_width_points"],"height_points":source["page_height_points"],"rotation_degrees":source["page_rotation_degrees"]}),
        "2007 page geometry drift",
    )?;
    let target = &plan["witness_1911"];
    private_boundary(tracked.root, s(&target["text_layer_ref"])?)?;
    let bytes = local.read(
        s(&target["text_layer_ref"])?,
        Some(s(&target["text_layer_sha256"])?),
    )?;
    let text = utf8(&bytes)?;
    let chars = text.chars().collect::<Vec<_>>();
    let start = n(&target["sentence_start"])?;
    let end = n(&target["sentence_end"])?;
    ensure(
        start == 0
            && end > 0
            && end < chars.len()
            && chars.iter().position(|c| *c == '.').map(|i| i + 1) == Some(end),
        "1911 sentence selector drift",
    )?;
    let sentence = chars[start..end].iter().collect::<String>();
    ensure(
        sentence.chars().count() == n(&target["sentence_codepoints"])?
            && sha(sentence.as_bytes()) == s(&target["sentence_sha256"])?,
        "1911 sentence fixity drift",
    )?;
    for key in ["text_layer_record", "rights"] {
        tracked.json(
            s(&target[format!("{key}_ref")])?,
            Some(s(&target[format!("{key}_sha256")])?),
        )?;
    }
    let unit = tracked.json(
        s(&target["text_unit_packet_ref"])?,
        Some(s(&target["text_unit_packet_sha256"])?),
    )?;
    let anchors = a(&unit["anchors"])?
        .iter()
        .filter(|r| r["anchor_ref"] == target["anchor_ref"])
        .collect::<Vec<_>>();
    ensure(
        anchors.len() == 1
            && anchors[0]["exact_sha256"] == target["sentence_sha256"]
            && anchors[0]["selector"]["start"] == start
            && anchors[0]["selector"]["end"] == end,
        "1911 anchor binding drift",
    )?;
    Ok(sentence)
}
fn comparison_views(
    ctx: &ResearchExecution,
    left: &str,
    right: &str,
    plan: &Value,
) -> Result<(Vec<Value>, Vec<Value>)> {
    let mut public = vec![];
    let mut private = vec![];
    let expected = a(&plan["comparison_method"]["views"])?;
    ensure(expected.len() == 4, "comparison view count")?;
    for (idx, name) in [
        "exact",
        "unicode-nfc",
        "whitespace-collapsed",
        "alphanumeric-casefold",
    ]
    .into_iter()
    .enumerate()
    {
        ensure(expected[idx]["view_id"] == name, "comparison view order")?;
        let transform = |t: &str| -> Result<String> {
            match name {
                "exact" => Ok(t.into()),
                "unicode-nfc" => Ok(t.nfc().collect()),
                "whitespace-collapsed" => Ok(t
                    .split(space)
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")),
                _ => {
                    static ALNUM: std::sync::LazyLock<regex::Regex> =
                        std::sync::LazyLock::new(|| regex::Regex::new(r"^[\p{L}\p{N}]$").unwrap());
                    let filtered = t
                        .chars()
                        .filter(|c| {
                            let mut b = [0; 4];
                            ALNUM.is_match(c.encode_utf8(&mut b))
                        })
                        .collect::<String>();
                    tos_foundation::python_casefold_unicode16_v1(&filtered, CAP, CAP * 3, CAP * 3)
                        .map_err(|e| e.to_string())
                }
            }
        };
        let l = transform(left)?.chars().collect::<Vec<_>>();
        let r = transform(right)?.chars().collect::<Vec<_>>();
        let ops = opcodes(ctx, &l, &r)?;
        let script = ops
            .iter()
            .map(|o| format!("{}:{}:{}:{}:{}", o.tag, o.i1, o.i2, o.j1, o.j2))
            .collect::<Vec<_>>()
            .join(";");
        let mut tags = json!({});
        let mut extents = json!({});
        for tag in ["equal", "replace", "delete", "insert"] {
            tags[tag] = json!(ops.iter().filter(|o| o.tag == tag).count());
            extents[tag] = json!(
                ops.iter()
                    .filter(|o| o.tag == tag)
                    .map(|o| (o.i2 - o.i1).max(o.j2 - o.j1))
                    .sum::<usize>()
            );
        }
        let equal = ops
            .iter()
            .filter(|o| o.tag == "equal")
            .map(|o| o.i2 - o.i1)
            .sum::<usize>();
        let ratio = if l.is_empty() && r.is_empty() {
            1.0
        } else {
            2.0 * equal as f64 / (l.len() + r.len()) as f64
        };
        let v = json!({"view_id":name,"normalization":expected[idx]["normalization"],"normalization_version":expected[idx]["normalization_version"],"witness_a_codepoints":l.len(),"witness_b_codepoints":r.len(),"exact_equal":l==r,"similarity_metric":"sequence_matcher_ratio","similarity_score_ppm":(ratio*1000000.0).round_ties_even() as u64,"opcode_count":ops.len(),"opcode_tag_counts":tags,"opcode_extent_counts":extents,"private_edit_script_sha256":sha(script.as_bytes())});
        let mut want = expected[idx].clone();
        want["similarity_metric"] = json!("sequence_matcher_ratio");
        ensure(v == want, "comparison view drift from frozen plan")?;
        let mut full = v.clone();
        full["opcodes"]=json!(ops.iter().map(|o|json!({"tag":o.tag,"witness_a_start":o.i1,"witness_a_end":o.i2,"witness_b_start":o.j1,"witness_b_end":o.j2})).collect::<Vec<_>>());
        public.push(v);
        private.push(full);
    }
    Ok((public, private))
}
fn entity(
    reference: &str,
    role: &str,
    digest: &str,
    size: u64,
    media: &str,
    availability: &str,
    disclosure: &str,
    at: &Value,
) -> Value {
    json!({"entity_ref":reference,"role":role,"sha256":digest,"size_bytes":size,"media_type":media,"availability":availability,"content_disclosure":disclosure,"fixity_verified":true,"fixity_verified_at":at})
}
fn input_entities(
    plan: &Value,
    plan_ref: &str,
    tracked: &Inputs<'_>,
    local: &Inputs<'_>,
    artifact: &Inputs<'_>,
    at: &Value,
) -> Result<Vec<Value>> {
    let obs = &plan["human_observation"];
    let source = &plan["witness_2007"];
    let target = &plan["witness_1911"];
    let mut rows = vec![];
    for (key, role) in [
        (
            "autosave",
            "unattested-human-workbench-observation-autosave",
        ),
        (
            "closure",
            "no-gold-no-attestation-sparse-calibration-closure",
        ),
        ("closure_receipt", "sparse-calibration-closure-receipt"),
    ] {
        let r = s(&obs[format!("{key}_relative_path")])?;
        let (h, n) = artifact.details(r)?;
        rows.push(entity(
            &format!("abyss-stack:artifact/{r}"),
            role,
            &h,
            n,
            "application/json",
            "owner_local",
            "private_content",
            at,
        ));
    }
    for (r, role, media) in [
        (
            s(&source["source_relative_ref"])?,
            "exact-2007-local-pdf-witness",
            "application/pdf",
        ),
        (
            s(&target["text_layer_ref"])?,
            "exact-1911-private-raw-text-layer",
            "text/plain; charset=utf-8",
        ),
    ] {
        let (h, n) = local.details(r)?;
        rows.push(entity(
            r,
            role,
            &h,
            n,
            media,
            "ignored_local",
            "private_content",
            at,
        ));
    }
    for (r, role) in [
        (plan_ref, "tracked-text-free-collation-plan"),
        (
            s(&plan["research_ref"])?,
            "ordered-witness-text-collation-research",
        ),
        (s(&plan["contract_ref"])?, "witness-text-collation-contract"),
        (
            s(&obs["assurance_ref"])?,
            "solo-human-plus-ai-assurance-boundary",
        ),
        (
            s(&obs["method_research_ref"])?,
            "human-assurance-method-research",
        ),
        (s(&source["item_manifest_ref"])?, "exact-2007-item-manifest"),
        (
            s(&source["resource_inventory_ref"])?,
            "exact-2007-page-resource-inventory",
        ),
        (s(&source["rights_ref"])?, "2007-layered-rights"),
        (
            s(&target["text_layer_record_ref"])?,
            "1911-private-layer-record",
        ),
        (
            s(&target["text_unit_packet_ref"])?,
            "1911-sentence-unit-proposal",
        ),
        (s(&target["rights_ref"])?, "1911-layered-rights"),
    ] {
        let (h, n) = tracked.details(r)?;
        rows.push(entity(
            r,
            role,
            &h,
            n,
            if r.ends_with(".json") || r.ends_with(".jsonl") {
                "application/json"
            } else {
                "text/markdown; charset=utf-8"
            },
            "tracked",
            "public_metadata_only",
            at,
        ));
    }
    Ok(rows)
}
fn builder_sha() -> String {
    let mut h = tos_foundation::Digest256Hasher::new();
    for raw in [
        include_bytes!("antonovsky_collation.rs").as_slice(),
        include_bytes!("antonovsky_collation/records.rs").as_slice(),
        include_bytes!("antonovsky_collation/historical-event.json").as_slice(),
        include_bytes!("antonovsky_collation/historical-plan.json").as_slice(),
        include_bytes!("antonovsky_collation/authority-boundary.json").as_slice(),
        include_bytes!("research_text_comparison.rs").as_slice(),
        include_bytes!("source_text_foundation.rs").as_slice(),
    ] {
        h.update(&(raw.len() as u64).to_be_bytes());
        h.update(raw);
    }
    h.finalize().to_hex()
}
fn event(
    ctx: &ResearchExecution,
    plan: &Value,
    plan_ref: &str,
    plan_sha: &str,
    event_id: &str,
    historical: bool,
    inputs: &[Value],
    replay_argv: &Value,
    runtime_version: &str,
    at: &Value,
    outputs: &[(String, Vec<u8>)],
    private: &[(String, Vec<u8>)],
) -> Result<Value> {
    let mut e: Value =
        serde_json::from_str(include_str!("antonovsky_collation/historical-event.json"))
            .map_err(|e| e.to_string())?;
    let out = outputs
        .iter()
        .map(|(r, b)| {
            entity(
                r,
                "tracked-text-free-anchor-layer-unit-or-collation-record",
                &sha(b),
                b.len() as u64,
                "application/json",
                "tracked",
                "public_metadata_only",
                at,
            )
        })
        .collect::<Vec<_>>();
    let side = private
        .iter()
        .map(|(r, b)| {
            let text = plan["outputs"]["private_text_ref"] == *r;
            entity(
                r,
                if text {
                    "ignored-local-human-observation-text-layer"
                } else {
                    "ignored-local-reconstructive-collation-detail"
                },
                &sha(b),
                b.len() as u64,
                if text {
                    "text/plain; charset=utf-8"
                } else {
                    "application/json"
                },
                "ignored_local",
                "private_content",
                at,
            )
        })
        .collect::<Vec<_>>();
    if historical {
        ensure(
            e["entities"]["inputs"] == json!(inputs)
                && e["entities"]["outputs"] == json!(out)
                && e["entities"]["byproducts"] == json!(side),
            "historical provenance entity membership/fixity drift",
        )?;
        return Ok(e);
    }
    e["event_id"] = json!(event_id);
    e["record_binding"]["manifest_ref"] = json!(plan_ref);
    e["activity"]["started_at"] = at.clone();
    e["activity"]["ended_at"] = at.clone();
    e["entities"] = json!({"inputs":inputs,"outputs":out,"byproducts":side});
    let builder_digest = builder_sha();
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let executor = ctx.select_directory(executable.parent().ok_or("executable parent")?)?;
    let name = executable
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("executable basename")?;
    let mut file = executor.source_file(name, 256 * 1024 * 1024)?;
    let executable_sha = executor.hash_file(&mut file, 256 * 1024 * 1024)?;
    e["responsibility"] = json!([{"agent_ref":SOFTWARE_AGENT,"agent_kind":"software","role":"executor","responsibility_posture":"performed","evidence_binding":{"ref":BUILDER,"sha256":builder_digest},"human_evidence_status":"not_applicable"}]);
    e["method"]["procedure"] = json!({"name":plan["comparison_method"]["name"],"version":plan["comparison_method"]["version"],"purpose":plan["question"]});
    let argv = replay_argv.clone();
    e["method"]["command_capture"] = json!({"disclosure":"inline","argv":argv,"argv_sha256":sha(&canonical(&argv)?),"withholding_reason":null});
    e["method"]["configuration_binding"] = json!({"ref":plan_ref,"sha256":plan_sha});
    e["method"]["software_components"] = json!([{"name":"Tree of Sophia Antonovsky witness collation builder","version":plan["comparison_method"]["version"],"role":"observation-materialization-and-collation-proposal-builder","artifact_ref":BUILDER,"artifact_sha256":builder_digest,"verification_status":"verified"},{"name":"tos-access","version":runtime_version,"role":"native-executable","artifact_ref":"runtime:tos-native-executable","artifact_sha256":executable_sha,"verification_status":"verified"}]);
    e["method"]["environment"] = json!({"runtime":"Rust native executable","runtime_version":runtime_version,"runtime_artifact_sha256":executable_sha,"backend":"rust-sequence-matcher-autojunk-false","hardware_target":"cpu","unicode_version":"16.0.0","environment_profile_binding":{"ref":plan_ref,"sha256":plan_sha}});
    let obs = &plan["human_observation"];
    let source = &plan["witness_2007"];
    let target = &plan["witness_1911"];
    let bindings = [
        (
            s(&source["source_relative_ref"])?.to_string(),
            s(&plan["outputs"]["source_anchor_ref"])?.to_string(),
        ),
        (
            format!(
                "abyss-stack:artifact/{}",
                s(&obs["autosave_relative_path"])?
            ),
            s(&plan["outputs"]["source_text_layer_ref"])?.to_string(),
        ),
        (
            format!(
                "abyss-stack:artifact/{}",
                s(&obs["autosave_relative_path"])?
            ),
            s(&plan["outputs"]["source_text_unit_packet_ref"])?.to_string(),
        ),
        (
            format!(
                "abyss-stack:artifact/{}",
                s(&obs["autosave_relative_path"])?
            ),
            s(&plan["outputs"]["collation_packet_ref"])?.to_string(),
        ),
        (
            s(&target["text_layer_ref"])?.to_string(),
            s(&plan["outputs"]["collation_packet_ref"])?.to_string(),
        ),
    ];
    for (i, (input, output)) in bindings.iter().enumerate() {
        e["derivations"][i]["derivation_id"] = json!(format!(
            "tos.derivation.antonovsky.{}.{}",
            &sha(event_id.as_bytes())[..16],
            i + 1
        ));
        e["derivations"][i]["input_entity_ref"] = json!(input);
        e["derivations"][i]["output_entity_ref"] = json!(output);
    }
    e["measurements"][0]["value"] = json!(
        inputs
            .iter()
            .map(|r| r["size_bytes"].as_u64().unwrap_or(0))
            .sum::<u64>()
    );
    e["measurements"][1]["value"] = json!(
        outputs
            .iter()
            .chain(private)
            .map(|(_, b)| b.len() as u64)
            .sum::<u64>()
    );
    e["measurements"][2]["value"] = obs["expected_active_seconds"].clone();
    e["rights_and_visibility"]["rights_record_bindings"] = json!([{"ref":source["rights_ref"],"sha256":source["rights_sha256"]},{"ref":target["rights_ref"],"sha256":target["rights_sha256"]}]);
    Ok(e)
}
pub struct Options<'a> {
    pub build: bool,
    pub input_root: &'a Path,
    pub artifact_root: &'a Path,
    pub output_root: Option<&'a Path>,
    pub plan_ref: Option<&'a str>,
    pub event_id: Option<&'a str>,
    pub argv: &'a Value,
    pub runtime_version: &'a str,
}
pub fn run(ctx: &ResearchExecution, opts: Options<'_>) -> Result<Value> {
    let plan_ref = opts.plan_ref.unwrap_or(PLAN);
    let event_id = opts.event_id.unwrap_or(EVENT);
    let historical = plan_ref == PLAN;
    ensure(
        (event_id == EVENT) == historical,
        "a new plan requires its own event identity",
    )?;
    ensure(
        regex::Regex::new(r"^tos\.event\.[a-z0-9]+(?:[.-][a-z0-9]+)*$")
            .unwrap()
            .is_match(event_id),
        "event identity syntax",
    )?;
    let local_ctx = ctx.select_directory(opts.input_root)?;
    let artifact_ctx = ctx.select_directory(opts.artifact_root)?;
    let out = if let Some(p) = opts.output_root {
        ctx.select_output_directory(p, opts.build)?
    } else {
        ctx.select_directory(opts.input_root)?
    };
    let mut tracked = Inputs::new(ctx);
    let mut local = Inputs::new(&local_ctx);
    let mut artifact = Inputs::new(&artifact_ctx);
    let plan_raw = tracked.read(plan_ref, None)?;
    let plan_sha = sha(&plan_raw);
    let plan = parsed(&plan_raw)?;
    ensure(
        plan["schema_version"] == "tos_antonovsky_2007_1911_opening_sentence_collation_plan_v1",
        "collation plan schema version",
    )?;
    ensure(
        plan["authority_boundary"]
            == serde_json::from_str::<Value>(include_str!(
                "antonovsky_collation/authority-boundary.json"
            ))
            .map_err(|e| e.to_string())?,
        "plan authority boundary widened",
    )?;
    if historical {
        ensure(
            plan_sha == "13d73e9570e83588b8d8f5c36b79f5740def5b5157bc6d00ef2e908faf9cff23",
            "historical plan bytes changed; use a new plan and event",
        )?;
    }
    let existing_event = read_optional(ctx, s(&plan["outputs"]["provenance_event_ref"])?)?
        .map(|raw| parsed(&raw))
        .transpose()?;
    ensure(
        opts.build || existing_event.is_some(),
        "check requires a retained event",
    )?;
    let observed = if historical {
        plan["created_at"].clone()
    } else if let Some(event) = &existing_event {
        event["activity"]["started_at"].clone()
    } else {
        json!(utc_now()?)
    };
    s(&observed)?;
    let invocation = if !historical {
        existing_event
            .as_ref()
            .map(|event| &event["method"]["command_capture"]["argv"])
            .unwrap_or(opts.argv)
    } else {
        opts.argv
    };
    ensure(
        invocation.as_array().is_some_and(|args| {
            !args.is_empty() && args.len() <= 64 && args.iter().all(Value::is_string)
        }),
        "captured argv shape",
    )?;
    if !historical {
        let original: Value =
            serde_json::from_str(include_str!("antonovsky_collation/historical-plan.json"))
                .map_err(|e| e.to_string())?;
        for (key, value) in plan["outputs"].as_object().ok_or("output references")? {
            ensure(
                original["outputs"][key] != *value,
                "new event must use separate historical output references",
            )?;
        }
        let ids = plan["opaque_ids"].as_object().ok_or("opaque identities")?;
        let original_ids = original["opaque_ids"]
            .as_object()
            .ok_or("historical identities")?;
        ensure(
            ids.len() == original_ids.len()
                && ids.keys().eq(original_ids.keys())
                && ids
                    .values()
                    .all(|value| !original_ids.values().any(|old| old == value)),
            "new event must use separate opaque output identities",
        )?;
    }
    for key in ["research_ref", "contract_ref"] {
        tracked.read(s(&plan[key])?, None)?;
    }
    for key in ["assurance", "method_research"] {
        tracked.read(
            s(&plan["human_observation"][format!("{key}_ref")])?,
            Some(s(&plan["human_observation"][format!("{key}_sha256")])?),
        )?;
    }
    let mut refs = BTreeSet::new();
    for value in plan["outputs"]
        .as_object()
        .ok_or("output references")?
        .values()
    {
        let r = s(value)?;
        ensure(
            r.starts_with("ToS/source-witnesses/")
                && r != plan_ref
                && !Path::new(r).is_absolute()
                && Path::new(r)
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
                && refs.insert(r),
            "unsafe or duplicate output reference",
        )?;
    }
    ensure(refs.len() == 7, "output membership count")?;
    for key in ["private_text_ref", "private_detail_ref"] {
        private_boundary(ctx, s(&plan["outputs"][key])?)?;
    }
    let target = verify_witnesses(&mut tracked, &mut local, &plan)?;
    let (text, values) = observation(&mut artifact, &plan)?;
    let (heading, interstitial, sentence, remainder) = select_source(&text, &plan["witness_2007"])?;
    let (public_views, private_views) = comparison_views(ctx, &sentence, &target, &plan)?;
    let private = vec![
        (
            s(&plan["outputs"]["private_text_ref"])?.to_string(),
            text.as_bytes().to_vec(),
        ),
        (
            s(&plan["outputs"]["private_detail_ref"])?.to_string(),
            encode(
                &records::private_detail(&plan, &values, &private_views),
                true,
            )?,
        ),
    ];
    let builder = if historical { LEGACY } else { BUILDER };
    let anchor = records::source_anchor(&plan, plan_ref, &plan_sha, event_id)?;
    let anchor_raw = encode(&anchor, true)?;
    let layer = records::source_layer(
        &plan,
        plan_ref,
        &plan_sha,
        &sha(&anchor_raw),
        &values,
        event_id,
    )?;
    let units = records::source_unit_packet(
        &plan,
        plan_ref,
        builder,
        event_id,
        &text,
        &heading,
        &interstitial,
        &sentence,
        &remainder,
    )?;
    let units_raw = encode(&units, true)?;
    let collation = records::collation_packet(
        &plan,
        plan_ref,
        &plan_sha,
        builder,
        event_id,
        &sha(&units_raw),
        &sha(&private[1].1),
        &public_views,
    )?;
    let mut outputs = vec![];
    for (key, kind, contract, value) in [
        (
            "source_anchor_ref",
            "anchor",
            "source-anchor-v2.schema.json",
            anchor,
        ),
        (
            "source_text_layer_ref",
            "layer",
            "source-text-layer.schema.json",
            layer,
        ),
        (
            "source_text_unit_packet_ref",
            "units",
            "source-text-unit-packet-v1.schema.json",
            units,
        ),
        (
            "collation_packet_ref",
            "collation",
            "witness-text-collation-packet-v1.schema.json",
            collation,
        ),
    ] {
        let r = s(&plan["outputs"][key])?;
        schema(ctx, &format!("ToS/contracts/{contract}"), &value)?;
        if kind != "collation" {
            metadata(ctx, kind, r, &value)?;
        } else {
            ensure(
                tos_validation::source_foundation_labs::witness_text_collation_semantic_messages(
                    &value,
                )
                .map_err(|e| format!("collation semantics: {e:?}"))?
                .is_empty(),
                "collation closure semantics",
            )?;
        }
        let bytes = encode(&value, true)?;
        if kind == "units" {
            let report =
                tos_validation::layer_family_rules::inspect_supplied_source_text_unit_semantics(
                    r,
                    &bytes,
                    s(&plan["outputs"]["private_text_ref"])?,
                    text.as_bytes(),
                    &plan_sha,
                    tos_validation::item_rules::ItemLimits {
                        max_member_bytes: CAP,
                        max_total_bytes: (32 * CAP) as u64,
                        max_state_bytes: 32 * CAP,
                        max_issues: 64,
                        deadline: ctx.deadline(),
                    },
                )
                .map_err(|e| format!("unit semantics: {e:?}"))?;
            ensure(
                report.state == tos_validation::text_rules::TextRuleState::Checked
                    && report.issues.is_empty(),
                "unit source reconstruction semantics",
            )?;
        }
        outputs.push((r.to_string(), bytes));
    }
    let inputs = input_entities(&plan, plan_ref, &tracked, &local, &artifact, &observed)?;
    let e = event(
        ctx,
        &plan,
        plan_ref,
        &plan_sha,
        event_id,
        historical,
        &inputs,
        invocation,
        opts.runtime_version,
        &observed,
        &outputs,
        &private,
    )?;
    if let Some(existing) = existing_event {
        ensure(
            existing == e,
            "retained provenance differs from actual bound inputs/outputs/method",
        )?;
    }
    schema(ctx, "ToS/contracts/provenance-event-v2.schema.json", &e)?;
    let issues = tos_validation::provenance_rules::semantic_issues(&e, 64, ctx.deadline())
        .map_err(|e| format!("provenance semantics: {e:?}"))?;
    ensure(issues.is_empty(), "provenance closure semantics")?;
    outputs.push((
        s(&plan["outputs"]["provenance_event_ref"])?.to_string(),
        encode(&e, true)?,
    ));
    let mut protected = vec![text.as_str(), sentence.as_str(), target.as_str()];
    for key in [
        "diplomatic_transcription",
        "layout_and_reading_order",
        "source_damage_or_ambiguity",
        "notes",
    ] {
        if let Some(v) = values[key].as_str().filter(|v| v.chars().count() >= 32) {
            protected.push(v);
        }
    }
    for raw in outputs
        .iter()
        .map(|(_, b)| b)
        .chain(std::iter::once(&plan_raw))
    {
        let view = utf8(raw)?;
        ensure(
            !protected.iter().any(|p| view.contains(p)),
            "tracked record exposes private source or observation text",
        )?;
    }
    tracked.verify()?;
    local.verify()?;
    artifact.verify()?;
    let mut pending = vec![];
    for (r, b) in &private {
        let missing = fresh_or_matching_limit(&out, r, b, CAP)?;
        if !missing {
            let f = out.source_file(r, CAP as u64)?;
            ensure(
                f.metadata()
                    .map_err(|e| e.to_string())?
                    .permissions()
                    .mode()
                    & 0o777
                    == 0o600,
                "private output must remain 0600",
            )?;
        }
        pending.push((&out, r, b, 0o600, missing));
    }
    for (r, b) in &outputs {
        pending.push((ctx, r, b, 0o644, fresh_or_matching_limit(ctx, r, b, CAP)?));
    }
    ensure(
        opts.build || pending.iter().all(|(_, _, _, _, m)| !*m),
        "required output absent",
    )?;
    let mut written = 0;
    for (context, r, b, mode, missing) in pending {
        if missing {
            context.write(r, b, mode, true)?;
            written += 1;
        }
    }
    Ok(
        json!({"status":"passed","plan_ref":plan_ref,"event_id":event_id,"historical_replay":historical,"native_executor":"tos antonovsky-collation","human_observation_status":"unattested_preserved","human_active_seconds":plan["human_observation"]["expected_active_seconds"],"comparison_view_count":public_views.len(),"tracked_record_count":outputs.len(),"private_output_count":private.len(),"written":written,"collation_status":"proposed","human_review_performed":false,"gold_created":false,"textual_equivalence_established":false,"expression_derivation_established":false,"translation_or_semantic_relation_created":false,"graph_or_canon_effect":false,"publication_authorized":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_sentence_keeps_codepoint_guards_and_original_whitespace() {
        let text = "Заг. \nТекст. Хвост";
        let mut source = json!({"text_layer_codepoints":text.chars().count(),"text_layer_bytes":text.len(),"text_layer_sha256":sha(text.as_bytes()),"heading_start":0,"heading_end":4,"heading_sha256":sha("Заг.".as_bytes()),"interstitial_start":4,"interstitial_end":6,"sentence_start":6,"sentence_end":12,"sentence_sha256":sha("Текст.".as_bytes()),"sentence_codepoints":6,"sentence_bytes":"Текст.".len(),"remainder_start":12,"remainder_end":18,"remainder_sha256":sha(" Хвост".as_bytes())});
        assert_eq!(
            select_source(text, &source).unwrap(),
            (
                "Заг.".into(),
                " \n".into(),
                "Текст.".into(),
                " Хвост".into()
            )
        );
        source["sentence_start"] = json!(4);
        assert!(select_source(text, &source).is_err());
    }
    #[test]
    fn collation_semantics_keep_unreviewed_proposals_and_private_sources_closed() {
        use tos_validation::source_foundation_labs::witness_text_collation_semantic_messages as inspect;
        let plan: Value =
            serde_json::from_str(include_str!("antonovsky_collation/historical-plan.json"))
                .unwrap();
        // The retained public output carries no transcription or edit script.
        let source:Value=serde_json::from_slice(include_bytes!("../../../../ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/witness-text-collation.antonovsky-2007-1911-opening-sentence.v1.json")).unwrap();
        assert!(inspect(&source).unwrap().is_empty());
        let mut accepted = source.clone();
        accepted["collations"][0]["status"] = json!("accepted");
        let issues = inspect(&accepted).unwrap();
        assert!(
            issues
                .iter()
                .any(|s| s == "decided collation lacks human review")
        );
        assert!(issues.iter().any(|s| s == "non-human collation acceptance"));
        let mut publication = source.clone();
        publication["rights_and_visibility"]["publication_authorized"] = json!(true);
        assert!(
            inspect(&publication)
                .unwrap()
                .iter()
                .any(|s| s == "private-source collation authorizes publication")
        );
        let mut promoted = source.clone();
        promoted["collations"][0]["interpretive_boundary"]["textual_equivalence_established"] =
            json!(true);
        assert!(
            inspect(&promoted)
                .unwrap()
                .iter()
                .any(|s| s == "collation interpretive boundary widened")
        );
        let mut projection = source.clone();
        projection["projections"] =
            json!([{"source_collation_refs":[source["collations"][0]["collation_id"]]}]);
        assert!(
            inspect(&projection)
                .unwrap()
                .iter()
                .any(|s| s == "projection uses a collation that is not accepted")
        );
        assert_eq!(plan["authority_boundary"]["human_review_performed"], false);
    }
}
