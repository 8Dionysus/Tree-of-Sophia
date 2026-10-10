//! One source-derived local calibration string, with tracked text-free bindings.
//! This route retains historical evidence and never accepts text or rights.
use crate::{
    german_triangulation::{self, tokens, with_witnesses},
    research_execution::ResearchExecution,
    research_text_comparison::space,
    source_text_foundation::{
        ensure, fresh_or_matching_limit, load, private_boundary, s, schema, sha,
    },
    transfer_target_passages::{encode, json_lines, jsonl, read_optional},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};
type Result<T> = std::result::Result<T, String>;
const GOLD: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1";
const SCHEMA: &str = "ToS/contracts/bounded-translation-research-input.schema.json";
const BUILDER: &str = "rust/crates/tos-compiler/src/bounded_translation_input.rs";
const CAP: usize = 4 * 1024 * 1024;
fn first_sentence(paragraph: &str) -> Result<String> {
    let end = paragraph
        .find('.')
        .ok_or("first paragraph has no full-stop boundary")?;
    let sentence = paragraph[..=end]
        .split(space)
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    ensure(!sentence.is_empty(), "selected sentence empty")?;
    Ok(sentence)
}
fn builder_sha() -> String {
    let mut h = tos_foundation::Digest256Hasher::new();
    for raw in [
        include_bytes!("bounded_translation_input.rs").as_slice(),
        include_bytes!("bounded_translation_input/templates.json").as_slice(),
        include_bytes!("german_triangulation.rs").as_slice(),
        include_bytes!("german_triangulation/templates.json").as_slice(),
        include_bytes!("research_html.rs").as_slice(),
        include_bytes!("research_html_entities.rs").as_slice(),
        include_bytes!("research_text_comparison.rs").as_slice(),
        include_bytes!("source_text_foundation.rs").as_slice(),
        include_bytes!("source_philosophy_dossier_docx.rs").as_slice(),
        include_bytes!("transfer_target_passages.rs").as_slice(),
    ] {
        h.update(&(raw.len() as u64).to_be_bytes());
        h.update(raw);
    }
    h.finalize().to_hex()
}
#[derive(Clone, Copy)]
pub enum Action {
    Build,
    Check,
    ValidateTracked,
}
pub struct Options<'a> {
    pub action: Action,
    pub input_root: Option<&'a Path>,
    pub output_root: Option<&'a Path>,
    pub generation: Option<&'a str>,
    pub prepared_at: Option<&'a str>,
}
pub fn run(ctx: &ResearchExecution, opts: Options<'_>) -> Result<Value> {
    let generation = opts.generation.unwrap_or("v1");
    ensure(
        regex::Regex::new(r"^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$")
            .unwrap()
            .is_match(generation),
        "generation component",
    )?;
    let historical = generation == "v1";
    let templates: Value =
        serde_json::from_str(include_str!("bounded_translation_input/templates.json"))
            .map_err(|e| e.to_string())?;
    let mut packet = templates["packet"].clone();
    let mut private = templates["private"].clone();
    let output = format!(
        "{GOLD}/bounded-translation-research-input.za-i-vorrede-1-opening-sentence.{generation}.json"
    );
    let journal = if historical {
        format!("{GOLD}/provenance.bounded-translation-research-input.jsonl")
    } else {
        format!("{GOLD}/provenance.bounded-translation-research-input.{generation}.jsonl")
    };
    let private_ref = format!(
        "{GOLD}/local-content/translation/research-inputs/za-i-vorrede-1-opening-sentence.{generation}.json"
    );
    private_boundary(ctx, &private_ref)?;
    let existing = read_optional(ctx, &output)?;
    if historical {
        ensure(
            opts.prepared_at
                .is_none_or(|v| Some(v) == packet["prepared_at"].as_str()),
            "historical timestamp is immutable",
        )?;
    } else {
        let at = opts
            .prepared_at
            .map(str::to_owned)
            .or_else(|| {
                existing
                    .as_ref()
                    .and_then(|raw| serde_json::from_slice::<Value>(raw).ok())
                    .and_then(|v| v["prepared_at"].as_str().map(str::to_owned))
            })
            .ok_or("fresh generation requires prepared-at timestamp")?;
        packet["prepared_at"] = json!(at);
        packet["packet_id"] = json!(format!(
            "tos.bounded-translation-research-input.za-i-vorrede-1-opening-sentence.{generation}"
        ));
        packet["local_artifact"]["ref"] = json!(private_ref);
        private["artifact_id"] = json!(format!(
            "tos.local-translation-research-input.za-i-vorrede-1-opening-sentence.{generation}"
        ));
        private["prepared_at"] = packet["prepared_at"].clone();
    }
    let mut inputs = BTreeMap::new();
    for binding in packet["bindings"]
        .as_object_mut()
        .ok_or("bindings")?
        .values_mut()
    {
        let reference = s(&binding["ref"])?.to_owned();
        let raw = ctx.read(&reference)?;
        binding["sha256"] = json!(sha(&raw));
        inputs.insert(reference, raw);
    }
    // These inputs remain text-free. Validate the current, retained triangulation
    // before deriving a fresh private string from the exact witnessed payloads.
    german_triangulation::run(
        ctx,
        german_triangulation::Options {
            action: german_triangulation::Action::ValidateTracked,
            input_root: None,
            generation: None,
            prepared_at: None,
        },
    )?;
    let triangulation_ref = s(&packet["bindings"]["german_source_triangulation"]["ref"])?;
    let triangulation: Value =
        serde_json::from_slice(&inputs[triangulation_ref]).map_err(|e| e.to_string())?;
    let private_bytes = if matches!(opts.action, Action::ValidateTracked) {
        ensure(
            opts.input_root.is_none(),
            "tracked validation reads no private witness input",
        )?;
        let (_, stored) = load(ctx, &output)?;
        for key in [
            "ref",
            "source_text_sha256",
            "source_text_codepoints",
            "normalized_alpha_tokens",
            "normalized_sequence_sha256",
            "gitignored",
            "source_text_copied_into_tracked_admission",
            "source_text_present_locally",
        ] {
            ensure(
                stored["local_artifact"][key] == packet["local_artifact"][key],
                "tracked source derivation summary drift",
            )?;
        }
        if historical {
            ensure(
                stored["local_artifact"] == packet["local_artifact"],
                "historical local artifact digest drift",
            )?;
        } else {
            packet["local_artifact"]["artifact_sha256"] =
                stored["local_artifact"]["artifact_sha256"].clone();
            packet["local_artifact"]["artifact_byte_size"] =
                stored["local_artifact"]["artifact_byte_size"].clone();
        }
        None
    } else {
        Some(with_witnesses(
            ctx,
            opts.input_root
                .ok_or("explicit local input root required")?,
            &triangulation,
            |ep, dp, _, naumann| {
                let dta = first_sentence(dp.first().ok_or("DTA paragraph absent")?)?;
                let ekgwb = first_sentence(ep.first().ok_or("eKGWB paragraph absent")?)?;
                let alpha = tokens(&dta)?;
                ensure(
                    alpha.len() == 20
                        && dta.as_bytes() == ekgwb.as_bytes()
                        && alpha == tokens(&ekgwb)?
                        && naumann.get(..20) == Some(alpha.as_slice()),
                    "exact opening sentence corroboration drift",
                )?;
                // The packet selects an exact historical sentence, never a replacement
                // obtained just by changing its source digest or matching token count.
                ensure(
                    sha(dta.as_bytes())
                        == s(&templates["packet"]["local_artifact"]["source_text_sha256"])?
                        && dta.chars().count() as u64
                            == templates["packet"]["local_artifact"]["source_text_codepoints"]
                                .as_u64()
                                .ok_or("source length")?,
                    "selected exact source sentence drift",
                )?;
                private["source_text"] = json!(dta);
                private["source_text_sha256"] = json!(sha(dta.as_bytes()));
                private["source_text_codepoints"] = json!(dta.chars().count());
                private["normalized_sequence_sha256"] = json!(sha(alpha.join(" ").as_bytes()));
                let bytes = encode(ctx, &private, true)?;
                for key in [
                    "source_text_sha256",
                    "source_text_codepoints",
                    "normalized_alpha_tokens",
                    "normalized_sequence_sha256",
                ] {
                    packet["local_artifact"][key] = private[key].clone();
                }
                packet["local_artifact"]["artifact_sha256"] = json!(sha(&bytes));
                packet["local_artifact"]["artifact_byte_size"] = json!(bytes.len());
                Ok(bytes)
            },
        )?)
    };
    schema(ctx, SCHEMA, &packet)?;
    let encoded = encode(ctx, &packet, true)?;
    let journal_bytes = if historical {
        let raw = ctx.read(&journal)?;
        ensure(
            sha(&raw) == s(&templates["historical_provenance_sha256"])?,
            "retained provenance history changed",
        )?;
        for (_, row) in json_lines(ctx, &raw)? {
            schema(ctx, "ToS/contracts/provenance-event.schema.json", &row)?;
        }
        raw
    } else {
        let mut event = templates["event"].clone();
        event["event_id"] = json!(format!(
            "tos.event.segmentation.zarathustra-bounded-translation-input.za-i-vorrede-1-opening-sentence.{generation}"
        ));
        event["started_at"] = packet["prepared_at"].clone();
        event["ended_at"] = packet["prepared_at"].clone();
        event["agent_refs"] = json!(["software:tos-compiler"]);
        event["method"]["runtime"] = json!("Rust tos-compiler");
        event["method"]["artifact_digest"] = json!(builder_sha());
        event["method"]["configuration"]["native_builder_ref"] = json!(BUILDER);
        // Refresh evidence digests without reading the sealed authored translation
        // into the local calibration artifact or asserting a new review.
        for item in event["inputs"].as_array_mut().ok_or("event inputs")? {
            let reference = s(&item["ref"])?;
            if let Some(raw) = inputs.get(reference) {
                item["sha256"] = json!(sha(raw));
            }
        }
        event["outputs"][0]["ref"] = json!(output);
        event["outputs"][0]["sha256"] = json!(sha(&encoded));
        event["outputs"][1]["ref"] = json!(private_ref);
        event["outputs"][1]["sha256"] = packet["local_artifact"]["artifact_sha256"].clone();
        event["receipt_refs"] = json!([output]);
        // The current rights posture was already updated by its owner; retain
        // the actual current warning rather than republishing the obsolete one.
        event["warnings"][0] = json!(
            "The DTA rights record remains subject to its recorded current conditions; this local-only route opens no publication or rights gate."
        );
        schema(ctx, "ToS/contracts/provenance-event.schema.json", &event)?;
        jsonl(ctx, &[event])?
    };
    let destination = opts
        .output_root
        .map(|p| ctx.select_directory(p))
        .transpose()?;
    ensure(
        matches!(opts.action, Action::ValidateTracked) || destination.is_some(),
        "explicit local output root required",
    )?;
    let mut missing_private = false;
    if let Some(dest) = &destination {
        if let Some(bytes) = &private_bytes {
            missing_private = fresh_or_matching_limit(dest, &private_ref, bytes, CAP)?;
        } else {
            let bytes = dest.read(&private_ref)?;
            ensure(
                bytes.len() as u64
                    == packet["local_artifact"]["artifact_byte_size"]
                        .as_u64()
                        .ok_or("artifact size")?
                    && sha(&bytes) == s(&packet["local_artifact"]["artifact_sha256"])?,
                "local artifact digest/size drift",
            )?;
            let actual: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let text = s(&actual["source_text"])?;
            ensure(
                sha(text.as_bytes()) == s(&packet["local_artifact"]["source_text_sha256"])?,
                "private source digest drift",
            )?;
            private["source_text"] = json!(text);
            for key in [
                "source_text_sha256",
                "source_text_codepoints",
                "normalized_alpha_tokens",
                "normalized_sequence_sha256",
            ] {
                private[key] = packet["local_artifact"][key].clone();
            }
            ensure(
                encode(ctx, &private, true)? == bytes,
                "private artifact shape drift",
            )?;
        }
    }
    for (reference, raw) in inputs {
        ensure(
            ctx.read(&reference)? == raw,
            "metadata or rights changed before output",
        )?;
    }
    let missing_packet = fresh_or_matching_limit(ctx, &output, &encoded, CAP)?;
    let missing_journal = fresh_or_matching_limit(ctx, &journal, &journal_bytes, CAP)?;
    ensure(
        matches!(opts.action, Action::Build)
            || !(missing_private || missing_packet || missing_journal),
        "output absent",
    )?;
    let mut written = vec![];
    if missing_private {
        destination.as_ref().unwrap().write(
            &private_ref,
            private_bytes.as_ref().unwrap(),
            0o600,
            true,
        )?;
        written.push(private_ref.clone());
    }
    if missing_packet {
        ctx.write(&output, &encoded, 0o644, true)?;
        written.push(output.clone());
    }
    if missing_journal {
        ctx.write(&journal, &journal_bytes, 0o644, true)?;
        written.push(journal.clone());
    }
    Ok(
        json!({"status":"passed", "packet":output,"packet_sha256":sha(&encoded),"provenance":journal,"private_artifact_ref":private_ref,"private_artifact_sha256":packet["local_artifact"]["artifact_sha256"],"historical_replay":historical,"private_witnesses_checked":private_bytes.is_some(),"private_artifact_checked":destination.is_some(),"written":written,"source_text_emitted_to_tracked_output":false,"source_or_translation_acceptance_performed":false}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_sentence_matches_the_existing_synthetic_boundary_cases() {
        assert_eq!(
            first_sentence("Erste synthetische Aussage. Zweite Aussage.").unwrap(),
            "Erste synthetische Aussage."
        );
        assert_eq!(
            first_sentence("Mehrere   Leerzeichen\nbleiben lesbar. Danach.").unwrap(),
            "Mehrere Leerzeichen bleiben lesbar."
        );
        assert!(first_sentence("Synthetischer Text ohne Abschluss").is_err());
    }
}
