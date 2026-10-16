//! Exact private sentence selectors and a non-promoting source/target proposal.
use crate::{
    research_execution::ResearchExecution,
    source_text_foundation::{
        encode, ensure, fresh_or_matching, load, metadata, private_boundary, s, schema, sha,
        utc_now,
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
#[path = "opening_sentence_alignment/records.rs"]
mod records;
type Result<T> = std::result::Result<T, String>;
pub use crate::source_text_foundation::Options;
pub const PLAN: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/za-i-vorrede-1-opening-sentence-alignment.plan.v1.json";
const BUILDER: &str = "rust/crates/tos-compiler/src/opening_sentence_alignment.rs";
const LEGACY_BUILDER: &str = "scripts/build_zarathustra_opening_sentence_alignment.py";
const LEGACY_EVENT: &str =
    "tos.event.alignment.za-i-vorrede-1.dta-1883-antonovsky-1911-opening-sentence.2026-08-12";
const LEGACY_SHA: &str = "80d312d541718d5fc5e57caa5689202102ec527fe311e29f362c341e76f9d52d";
const CAP: usize = 2 * 1024 * 1024;
struct Side {
    text: String,
    sentence_end_bytes: usize,
}
impl Side {
    fn sentence(&self) -> &str {
        &self.text[..self.sentence_end_bytes]
    }
    fn remainder(&self) -> &str {
        &self.text[self.sentence_end_bytes..]
    }
}
fn select_sentence(text: String, side: &Value) -> Result<Side> {
    ensure(
        sha(text.as_bytes()) == s(&side["text_layer_sha256"])?
            && Some(text.chars().count() as u64) == side["scope_end"].as_u64(),
        "private layer fixity/length drift",
    )?;
    ensure(side["sentence_start"] == 0, "sentence must start at zero")?;
    let end = text
        .find('.')
        .map(|i| i + 1)
        .ok_or("first full stop absent")?;
    ensure(
        end < text.len()
            && text[..end].chars().count() as u64
                == side["sentence_end"].as_u64().ok_or("sentence end")?,
        "first full stop boundary drift",
    )?;
    // Match the maintained Python whitespace guard, including U+001C..U+001F.
    ensure(
        text[end..]
            .chars()
            .next()
            .is_some_and(|c| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)),
        "post-sentence whitespace guard drift",
    )?;
    ensure(
        sha(text[..end].as_bytes()) == s(&side["sentence_sha256"])?
            && sha(text[end..].as_bytes()) == s(&side["remainder_sha256"])?,
        "sentence/remainder digest drift",
    )?;
    Ok(Side {
        text,
        sentence_end_bytes: end,
    })
}
fn read_side(
    ctx: &ResearchExecution,
    input: &ResearchExecution,
    name: &str,
    side: &Value,
    retained: &mut BTreeMap<String, Vec<u8>>,
) -> Result<Side> {
    let mut pairs = vec![
        ("text_layer_ref", "text_layer_record_sha256"),
        ("rights_ref", "rights_sha256"),
        ("layout_packet_ref", "layout_packet_sha256"),
    ];
    if name == "source" {
        pairs.push((
            "edition_reading_admission_ref",
            "edition_reading_admission_sha256",
        ));
    } else {
        pairs.extend([
            ("expression_record_ref", "expression_record_sha256"),
            ("responsibility_claims_ref", "responsibility_claims_sha256"),
        ]);
    }
    for (rk, hk) in pairs {
        let reference = s(&side[rk])?;
        let raw = ctx.read(reference)?;
        ensure(
            raw.len() <= CAP && sha(&raw) == s(&side[hk])?,
            "tracked source/rights dependency drift",
        )?;
        retained.insert(reference.into(), raw);
    }
    let (_, layer) = load(ctx, s(&side["text_layer_ref"])?)?;
    let r = &layer["representation"];
    ensure(
        r["content_ref"] == side["private_content_ref"]
            && r["content_sha256"] == side["text_layer_sha256"]
            && r["language"] == side["language"]
            && r["tracked_content"] == false
            && r["publication_authorized"] == false,
        "text layer identity/visibility drift",
    )?;
    let reference = s(&side["private_content_ref"])?;
    private_boundary(ctx, reference)?;
    let mut file = input.source_file(reference, CAP as u64)?;
    let raw = input.read_file(&mut file, CAP as u64)?;
    select_sentence(String::from_utf8(raw).map_err(|e| e.to_string())?, side)
}
fn limits(ctx: &ResearchExecution) -> tos_validation::item_rules::ItemLimits {
    tos_validation::item_rules::ItemLimits {
        max_member_bytes: CAP,
        max_total_bytes: (32 * CAP) as u64,
        max_state_bytes: 32 * CAP,
        max_issues: 64,
        deadline: ctx.deadline(),
    }
}
fn entity(reference: &str, role: &str, raw: &[u8], private: bool, at: &str) -> Value {
    json!({"entity_ref":reference,"role":role,"sha256":sha(raw),"size_bytes":raw.len(),"media_type":if private{"text/plain; charset=utf-8"}else{"application/json"},"availability":if private{"ignored_local"}else{"tracked"},"content_disclosure":if private{"private_content"}else{"public_metadata_only"},"fixity_verified":true,"fixity_verified_at":at})
}
fn validate_event(
    ctx: &ResearchExecution,
    plan: &Value,
    plan_ref: &str,
    plan_raw: &[u8],
    event: &Value,
    inputs: &[Value],
    outputs: &[Value],
    derivations: &[Value],
) -> Result<()> {
    schema(ctx, "ToS/contracts/provenance-event-v2.schema.json", event)?;
    let issues = tos_validation::provenance_rules::semantic_issues(event, 64, ctx.deadline())
        .map_err(|e| format!("provenance: {e:?}"))?;
    ensure(issues.is_empty(), &format!("provenance issues: {issues:?}"))?;
    ensure(
        event["entities"]["inputs"] == json!(inputs)
            && event["entities"]["outputs"] == json!(outputs),
        "provenance exact input/output closure drift",
    )?;
    let binding = json!({"ref":plan_ref,"sha256":sha(plan_raw)});
    ensure(
        event["method"]["configuration_binding"] == binding
            && event["method"]["environment"]["environment_profile_binding"] == binding
            && event["method"]["command_capture"]["argv_sha256"]
                == sha(&encode(&event["method"]["command_capture"]["argv"], false)?),
        "provenance configuration/capture drift",
    )?;
    let rights = ["source", "target"]
        .map(|n| json!({"ref":plan[n]["rights_ref"],"sha256":plan[n]["rights_sha256"]}));
    ensure(
        event["rights_and_visibility"]["rights_record_bindings"] == json!(rights)
            && event["rights_and_visibility"]["publication_authorized"] == false
            && event["review_and_authority"]["promotion_authorized"] == false
            && event["review_and_authority"]["accepted_uses"] == json!([])
            && event["evidence_authentication"]["signature_status"] == "unsigned",
        "provenance rights/authority drift",
    )?;
    let actual = event["derivations"]
        .as_array()
        .ok_or("derivations absent")?;
    ensure(actual.len() == derivations.len(), "derivation count")?;
    for (a, b) in actual.iter().zip(derivations) {
        for key in [
            "derivation_id",
            "input_entity_ref",
            "output_entity_ref",
            "relation",
            "influence_asserted",
        ] {
            ensure(a[key] == b[key], "derivation closure drift")?;
        }
    }
    ctx.check()
}
pub fn run(ctx: &ResearchExecution, opts: Options<'_>) -> Result<Value> {
    let (plan_raw, plan) = load(ctx, opts.plan_ref)?;
    ensure(
        plan["schema_version"] == "tos_zarathustra_opening_sentence_alignment_plan_v1"
            && plan["method"]["name"]
                == "exact-first-full-stop-inclusive-ordinal-correspondence-proposal"
            && plan["method"]["version"] == "1"
            && plan["method"]["terminator"] == "U+002E"
            && plan["method"]["normalization"] == "none"
            && plan["method"]["dehyphenation"] == false
            && plan["method"]["translation_performed"] == false,
        "unsupported alignment plan/method",
    )?;
    let input = ctx.select_directory(opts.input_root)?;
    let mut retained = BTreeMap::from([(opts.plan_ref.to_owned(), plan_raw.clone())]);
    let source = read_side(ctx, &input, "source", &plan["source"], &mut retained)?;
    let target = read_side(ctx, &input, "target", &plan["target"], &mut retained)?;
    let mut names = BTreeSet::new();
    for key in [
        "source_sentence_packet_ref",
        "target_sentence_packet_ref",
        "alignment_packet_ref",
        "provenance_event_ref",
    ] {
        let r = s(&plan["outputs"][key])?;
        tos_foundation::RelativePath::parse(r).map_err(|e| e.to_string())?;
        ensure(
            r.starts_with("ToS/source-witnesses/")
                && !retained.contains_key(r)
                && ![
                    s(&plan["source"]["private_content_ref"])?,
                    s(&plan["target"]["private_content_ref"])?,
                ]
                .contains(&r)
                && names.insert(r),
            "output aliases source or leaves owner",
        )?;
    }
    let event_ref = s(&plan["outputs"]["provenance_event_ref"])?;
    let existing = match std::fs::symlink_metadata(ctx.root().join(event_ref)) {
        Ok(_) => Some(load(ctx, event_ref)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    ensure(
        opts.build || existing.is_some(),
        "check requires retained provenance",
    )?;
    let at = if let Some((_, e)) = &existing {
        s(&e["activity"]["ended_at"])?.to_owned()
    } else {
        utc_now()?
    };
    let event_id = if let Some((_, e)) = &existing {
        s(&e["event_id"])?.to_owned()
    } else {
        let id = opts
            .event_id
            .ok_or("fresh build requires explicit --event-id")?;
        ensure(
            id.starts_with("tos.event.") && id != LEGACY_EVENT && id.len() < 512,
            "new event identity",
        )?;
        id.into()
    };
    if let Some(id) = opts.event_id {
        ensure(id == event_id, "selected event differs from retained event")?;
    }
    let historical = existing.as_ref().is_some_and(|(_, e)| {
        e["method"]["software_components"][0]["artifact_ref"] == LEGACY_BUILDER
    });
    if historical {
        ensure(
            event_id == LEGACY_EVENT,
            "unsupported historical event identity",
        )?;
    }
    let legacy_wording = historical
        && existing.as_ref().unwrap().1["method"]["software_components"][0]["artifact_sha256"]
            == LEGACY_SHA;
    let uv = std::char::UNICODE_VERSION;
    let unicode = format!("{}.{}.{}", uv.0, uv.1, uv.2);
    let (builder, unicode, created) = if historical {
        (LEGACY_BUILDER, "16.0.0", s(&plan["created_at"])?)
    } else {
        (BUILDER, unicode.as_str(), at.as_str())
    };
    let mut records_map = BTreeMap::new();
    for (name, side) in [("source", &source), ("target", &target)] {
        let method = records::method(
            &plan,
            &plan[name]["language"],
            opts.plan_ref,
            builder,
            &event_id,
            unicode,
            created,
        );
        let mut packet = records::unit_packet(
            &plan,
            name,
            &side.text,
            side.sentence(),
            side.remainder(),
            &method,
        );
        if legacy_wording {
            packet["units"][0]["status_reason"] = json!(
                "The exact first U+002E boundary is mechanically selected for a bounded alignment proposal; it is not accepted sentence analysis or accepted text."
            );
        }
        let reference = s(&plan["outputs"][format!("{name}_sentence_packet_ref")])?;
        schema(
            ctx,
            "ToS/contracts/source-text-unit-packet-v1.schema.json",
            &packet,
        )?;
        metadata(ctx, "units", reference, &packet)?;
        let raw = encode(&packet, true)?;
        let report =
            tos_validation::layer_family_rules::inspect_supplied_source_text_unit_semantics(
                reference,
                &raw,
                s(&plan[name]["private_content_ref"])?,
                side.text.as_bytes(),
                &sha(&plan_raw),
                limits(ctx),
            )
            .map_err(|e| format!("unit replay: {e:?}"))?;
        ensure(
            report.state == tos_validation::text_rules::TextRuleState::Checked
                && report.issues.is_empty(),
            &format!("unit replay issues: {:?}", report.issues),
        )?;
        records_map.insert(reference.to_owned(), raw);
    }
    let source_ref = s(&plan["outputs"]["source_sentence_packet_ref"])?;
    let target_ref = s(&plan["outputs"]["target_sentence_packet_ref"])?;
    let mut alignment = records::alignment_packet(
        &plan,
        opts.plan_ref,
        builder,
        &event_id,
        created,
        &sha(&plan_raw),
        source_ref,
        &sha(&records_map[source_ref]),
        target_ref,
        &sha(&records_map[target_ref]),
    );
    if legacy_wording {
        alignment["alignments"][0]["evidence"][1]["description"] = json!(
            "The source expression has bounded edition-reading admission; this does not accept the sentence segmentation or alignment."
        );
    }
    schema(
        ctx,
        "ToS/contracts/translation-alignment-packet-v1.schema.json",
        &alignment,
    )?;
    let report = tos_validation::layer_family_rules::inspect_supplied_translation_alignment(
        &alignment,
        limits(ctx),
    )
    .map_err(|e| format!("alignment: {e:?}"))?;
    ensure(
        report.issues.is_empty() && report.unsupported.is_empty(),
        &format!("alignment issues: {:?}", report.issues),
    )?;
    records_map.insert(
        s(&plan["outputs"]["alignment_packet_ref"])?.to_owned(),
        encode(&alignment, true)?,
    );
    let mut inputs = vec![];
    for (name, side) in [("source", &source), ("target", &target)] {
        inputs.push(entity(
            s(&plan[name]["private_content_ref"])?,
            &format!("ignored-local-{name}-paragraph-text-layer"),
            side.text.as_bytes(),
            true,
            &at,
        ));
    }
    let tracked = [
        (opts.plan_ref, "tracked-text-free-alignment-plan"),
        (
            s(&plan["source"]["text_layer_ref"])?,
            "source-text-layer-record",
        ),
        (
            s(&plan["target"]["text_layer_ref"])?,
            "target-text-layer-record",
        ),
        (
            s(&plan["source"]["layout_packet_ref"])?,
            "source-layout-packet",
        ),
        (
            s(&plan["target"]["layout_packet_ref"])?,
            "target-layout-packet",
        ),
        (
            s(&plan["source"]["edition_reading_admission_ref"])?,
            "source-edition-reading-admission",
        ),
        (
            s(&plan["target"]["expression_record_ref"])?,
            "target-expression-record",
        ),
        (
            s(&plan["target"]["responsibility_claims_ref"])?,
            "target-translated-by-responsibility-claims",
        ),
    ];
    for (r, role) in tracked {
        inputs.push(entity(r, role, &retained[r], false, &at));
    }
    let mut outputs = vec![];
    for key in [
        "source_sentence_packet_ref",
        "target_sentence_packet_ref",
        "alignment_packet_ref",
    ] {
        let r = s(&plan["outputs"][key])?;
        outputs.push(entity(
            r,
            "tracked-text-free-source-target-or-alignment-record",
            &records_map[r],
            false,
            &at,
        ));
    }
    let mut derivations = vec![];
    for output in &outputs {
        for name in ["source", "target"] {
            derivations.push(json!({"derivation_id":format!("tos.derivation.za-i-vorrede-1.opening-sentence-alignment.{}",derivations.len()+1),"input_entity_ref":plan[name]["private_content_ref"],"output_entity_ref":output["entity_ref"],"relation":"aggregation_from","influence_asserted":true,"description":"The exact private layer contributes only its fixed selector and digest to this text-free proposal; no source text is disclosed."}));
        }
    }
    let event = if let Some((_, e)) = &existing {
        e.clone()
    } else {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let selected = ctx.select_directory(executable.parent().ok_or("executable parent")?)?;
        let mut f = selected.source_file(
            executable
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or("executable name")?,
            256 * 1024 * 1024,
        )?;
        let digest = selected.hash_file(&mut f, 256 * 1024 * 1024)?;
        records::provenance(records::Provenance {
            plan: &plan,
            plan_ref: opts.plan_ref,
            plan_digest: &sha(&plan_raw),
            event_id: &event_id,
            builder_ref: BUILDER,
            builder_digest: &sha(include_bytes!("opening_sentence_alignment.rs")),
            executable_digest: &digest,
            argv: opts.argv,
            argv_digest: &sha(&encode(opts.argv, false)?),
            inputs: &inputs,
            outputs: &outputs,
            derivations: &derivations,
            made_at: &at,
        })
    };
    validate_event(
        ctx,
        &plan,
        opts.plan_ref,
        &plan_raw,
        &event,
        &inputs,
        &outputs,
        &derivations,
    )?;
    records_map.insert(
        event_ref.into(),
        if let Some((raw, _)) = existing {
            raw
        } else {
            encode(&event, true)?
        },
    );
    for raw in std::iter::once(&plan_raw).chain(records_map.values()) {
        let rendered = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
        for private in [
            &source.text,
            source.sentence(),
            &target.text,
            target.sentence(),
        ] {
            ensure(
                !rendered.contains(private),
                "tracked plan or output exposes private text",
            )?;
        }
    }
    // Repeat exact input checks before the first write, under the same deadline.
    for (r, raw) in &retained {
        ensure(
            ctx.read(r)? == *raw,
            "source/rights dependency changed before output",
        )?;
    }
    for (name, side) in [("source", &source), ("target", &target)] {
        ensure(
            input.read(s(&plan[name]["private_content_ref"])?)? == side.text.as_bytes(),
            "private source changed before output",
        )?;
    }
    let mut writes = vec![];
    for (r, raw) in &records_map {
        if fresh_or_matching(ctx, r, raw)? {
            ensure(opts.build, "tracked output absent")?;
            writes.push((r, raw));
        }
    }
    if opts.build {
        for (r, raw) in writes {
            ctx.write(r, raw, 0o644, true)?;
        }
    }
    ctx.check()?;
    Ok(
        json!({"status":"passed","question":plan["question"],"source_sentence_selector":[plan["source"]["sentence_start"],plan["source"]["sentence_end"]],"source_sentence_sha256":plan["source"]["sentence_sha256"],"target_sentence_selector":[plan["target"]["sentence_start"],plan["target"]["sentence_end"]],"target_sentence_sha256":plan["target"]["sentence_sha256"],"tracked_record_count":records_map.len(),"sentence_segmentations":"proposed","alignment_status":"proposed","human_review_performed":false,"translation_fidelity_established":false,"lexical_equivalence_established":false,"semantic_or_canon_effect":false,"publication_authorized":false,"historical_provenance_preserved":historical,"execution_truth_authenticated":false,"native_executor":"tos opening-sentence-alignment","event_id":event_id}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "opening_sentence_alignment/synthetic-parity.json"
        ))
        .unwrap()
    }
    #[test]
    fn exact_sentence_guard_counts_codepoints_and_preserves_whitespace() {
        let f = fixture();
        let plan = &f["plan"];
        let text = f["texts"]["source"].as_str().unwrap();
        let side = &plan["source"];
        let selected = select_sentence(text.into(), side).unwrap();
        assert_eq!(selected.sentence(), "Ä 😀.");
        assert_eq!(selected.remainder(), "\nB");
        assert!(selected.sentence_end_bytes > selected.sentence().chars().count());
        for key in ["sentence_end", "scope_end"] {
            let mut changed = side.clone();
            changed[key] = json!(1);
            assert!(select_sentence(text.into(), &changed).is_err());
        }
        for modified in ["Ä 😀!\nB", "Ä 😀.B", "Ä 😀."] {
            let mut changed = side.clone();
            changed["text_layer_sha256"] = json!(sha(modified.as_bytes()));
            changed["scope_end"] = json!(modified.chars().count());
            assert!(select_sentence(modified.into(), &changed).is_err());
        }
    }
    #[test]
    fn synthetic_sentence_and_alignment_records_match_previous_producer_bytes() {
        let f = fixture();
        let plan = &f["plan"];
        let mut packets = BTreeMap::new();
        for name in ["source", "target"] {
            let text = f["texts"][name].as_str().unwrap();
            let side = select_sentence(text.into(), &plan[name]).unwrap();
            let method = records::method(
                plan,
                &plan[name]["language"],
                PLAN,
                LEGACY_BUILDER,
                LEGACY_EVENT,
                f["unicode_version"].as_str().unwrap(),
                plan["created_at"].as_str().unwrap(),
            );
            let packet =
                records::unit_packet(plan, name, text, side.sentence(), side.remainder(), &method);
            let raw = encode(&packet, true).unwrap();
            assert_eq!(sha(&raw), f["expected_sha256"][name].as_str().unwrap());
            packets.insert(name, raw);
        }
        let alignment = records::alignment_packet(
            plan,
            PLAN,
            LEGACY_BUILDER,
            LEGACY_EVENT,
            plan["created_at"].as_str().unwrap(),
            f["plan_digest"].as_str().unwrap(),
            plan["outputs"]["source_sentence_packet_ref"]
                .as_str()
                .unwrap(),
            &sha(&packets["source"]),
            plan["outputs"]["target_sentence_packet_ref"]
                .as_str()
                .unwrap(),
            &sha(&packets["target"]),
        );
        assert_eq!(
            sha(&encode(&alignment, true).unwrap()),
            f["expected_sha256"]["alignment"].as_str().unwrap()
        );
        let ctx = ResearchExecution::new(&std::env::temp_dir(), 30).unwrap();
        let report = tos_validation::layer_family_rules::inspect_supplied_translation_alignment(
            &alignment,
            limits(&ctx),
        )
        .unwrap();
        assert!(report.issues.is_empty() && report.unsupported.is_empty());
        let uri = "https://tree-of-sophia.local/ToS/contracts/translation-alignment-packet-v1.schema.json";
        let probe = tos_validation::SchemaBackendProbe::new(
            [tos_validation::SchemaResource {
                uri: uri.into(),
                raw: include_bytes!(
                    "../../../../ToS/contracts/translation-alignment-packet-v1.schema.json"
                )
                .to_vec(),
            }],
            tos_validation::FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        assert!(
            probe
                .is_valid_raw(uri, &encode(&alignment, false).unwrap())
                .unwrap()
        );
    }
}
