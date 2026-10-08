//! Text-free co-availability of the bounded golden transfer candidate frame.
use crate::{
    research_execution::ResearchExecution,
    source_text_foundation::{ensure, fresh_or_matching, load, s, schema, sha, utc_now},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
#[path = "transfer_route_readiness/records.rs"]
mod records;
type Result<T> = std::result::Result<T, String>;
const GOLD: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1";
const TARGET: &str = "transfer-target-passage-candidates.v1.json";
const SOURCE: &str = "transfer-source-passage-candidates.v1.json";
const SCHEMA: &str = "ToS/contracts/transfer-route-readiness-projection.schema.json";
const SCHEMA_URI: &str =
    "https://tree-of-sophia.local/ToS/contracts/transfer-route-readiness-projection.schema.json";
const PROJECTION_ID: &str = "tos.transfer-route-readiness.golden-kernel-v1";
const LEGACY_EVENT: &str =
    "tos.event.projection.golden-kernel-transfer-route-readiness-v1.2026-08-09";
const LEGACY_EVENT_SHA: &str = "75becae2f58c85dcc30d7098eb90f87d2a38706649c5e69226a0b05f4d7448e4";
const LEGACY_BUILDER: &str = "scripts/build_golden_kernel_transfer_route_readiness.py";
const LEGACY_BUILDER_SHA: &str = "9fb18fe2ff6eb14672dfec407f0c2210e0b7f6f4286c1fc0987633709e147675";
const BUILDER: &str = "rust/crates/tos-compiler/src/transfer_route_readiness.rs";
const LEGACY_AUTHORITY: &str = "text-free co-availability over independently materialized source and target candidates only; shared structural route identity and frozen-page intersection create no bilingual passage pair, alignment, accepted text, eligibility, gold, human task, semantic claim, publication authority, or canon effect";
fn path(name: &str) -> String {
    format!("{GOLD}/{name}")
}
fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or("required array".into())
}
fn render(v: &Value) -> Result<Vec<u8>> {
    let mut b = serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?;
    b.push(b'\n');
    Ok(b)
}
fn projection(
    target_payload: &Value,
    source_payload: &Value,
    target_binding: &Value,
    source_binding: &Value,
    event_id: &str,
) -> Result<Value> {
    let targets = array(&target_payload["passage_candidates"])?;
    let sources = array(&source_payload["passage_candidates"])?;
    ensure(
        targets.len() == 35 && sources.len() == 35,
        "candidate sets must each contain 35 routes",
    )?;
    for v in [target_payload, source_payload] {
        ensure(
            v["effects"]["source_to_target_passage_alignment_created"] == false
                && v["summary"]["eligible_target_unit_count"] == 0,
            "candidate-set authority boundary drifted",
        )?;
    }
    let mut source_by_target = BTreeMap::new();
    let mut target_ids = BTreeSet::new();
    for source in sources {
        ensure(
            source_by_target
                .insert(s(&source["target_passage_candidate_id"])?, source)
                .is_none(),
            "duplicate source candidate target",
        )?;
    }
    for target in targets {
        ensure(
            target_ids.insert(s(&target["passage_candidate_id"])?),
            "duplicate target candidate",
        )?;
    }
    ensure(
        target_ids == source_by_target.keys().copied().collect(),
        "source candidate set does not close target frame",
    )?;
    let mut routes = Vec::new();
    let mut pages = Vec::<Value>::new();
    let mut page_indices = BTreeMap::new();
    let mut intersecting = 0;
    for target in targets {
        let source = source_by_target[s(&target["passage_candidate_id"])?];
        ensure(
            source["status"] == "materialized-layer-exact-candidate"
                && ["qualified_unit_key", "work_ref", "frozen_page_candidate_id"]
                    .iter()
                    .all(|k| source[*k] == target[*k]),
            "candidate identity or availability drifted",
        )?;
        for v in [source, target] {
            s(&v["private_content_ref"])?;
            let digest = s(&v["private_content_sha256"])?;
            ensure(
                digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()),
                "private candidate digest malformed",
            )?;
            ensure(
                v["eligible_for_variant_execution"] == false,
                "candidate eligibility widened",
            )?;
        }
        ensure(
            source["accepted_source_text"] == false
                && source["source_to_target_alignment_created"] == false
                && target["accepted_target_text"] == false
                && target["target_gold_status"] == "not_started"
                && target["human_review_performed"] == false,
            "candidate authority drifted",
        )?;
        let status = s(&target["status"])?;
        let (intersection, class) = match status {
            "proposed-intersecting-layer-exact" => (
                true,
                "dual-private-layer-candidates-frozen-page-intersecting",
            ),
            "rejected-nonintersecting-layer-exact" => (
                false,
                "dual-private-layer-candidates-target-nonintersecting",
            ),
            _ => return Err("unsupported target status".into()),
        };
        ensure(
            target["candidate_page_intersects_passage"] == intersection,
            "target intersection/status drifted",
        )?;
        ensure(
            source["source_passage_candidate_id"]
                == s(&target["passage_candidate_id"])?.replace(
                    "tos-target-passage-candidate-",
                    "tos-source-passage-candidate-",
                ),
            "source candidate ID differs from its declared target",
        )?;
        let route_id = s(&target["passage_candidate_id"])?.replace(
            "tos-target-passage-candidate-",
            "tos-transfer-route-readiness-",
        );
        routes.push(records::route(
            target,
            source,
            &route_id,
            status,
            class,
            intersection,
        ));
        let page_id = s(&target["frozen_page_candidate_id"])?;
        let index = *page_indices.entry(page_id).or_insert_with(|| {
            pages.push(records::page(&target["frozen_page_candidate_id"]));
            pages.len() - 1
        });
        pages[index]["route_readiness_refs"]
            .as_array_mut()
            .ok_or("page routes")?
            .push(json!(route_id));
        let count = if intersection {
            intersecting += 1;
            "dual_intersecting_route_count"
        } else {
            "dual_target_nonintersecting_route_count"
        };
        pages[index][count] = json!(pages[index][count].as_u64().ok_or("page count")? + 1);
    }
    ensure(
        pages.len() == 20
            && pages.iter().all(|p| {
                p["dual_intersecting_route_count"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
            }),
        "all twenty frozen pages need an intersecting route",
    )?;
    ensure(
        intersecting == 32 && routes.len() - intersecting == 3,
        "expected 32 intersecting and 3 nonintersecting routes",
    )?;
    let output = records::projection(
        target_binding,
        source_binding,
        routes,
        pages,
        intersecting,
        3,
        event_id,
    );
    let raw = serde_json::to_string(&output).map_err(|e| e.to_string())?;
    for key in [
        "text",
        "words",
        "records",
        "private_content_ref",
        "private_content_sha256",
    ] {
        ensure(
            !raw.contains(&format!("\"{key}\":")),
            "private content field escaped projection",
        )?;
    }
    Ok(output)
}
pub struct Options<'a> {
    pub build: bool,
    pub output_ref: Option<&'a str>,
    pub provenance_ref: Option<&'a str>,
    pub event_id: Option<&'a str>,
}
pub fn run(ctx: &ResearchExecution, opts: Options<'_>) -> Result<Value> {
    let output = opts
        .output_ref
        .map(str::to_owned)
        .unwrap_or_else(|| path("transfer-route-readiness.v1.json"));
    let provenance = opts
        .provenance_ref
        .map(str::to_owned)
        .unwrap_or_else(|| path("transfer-provenance.jsonl"));
    let target_ref = path(TARGET);
    let source_ref = path(SOURCE);
    for p in [&output, &provenance] {
        tos_foundation::RelativePath::parse(p).map_err(|e| e.to_string())?;
        ensure(
            p.starts_with("ToS/source-witnesses/")
                && ![target_ref.as_str(), source_ref.as_str(), SCHEMA, BUILDER]
                    .contains(&p.as_str()),
            "output aliases input or leaves owner",
        )?;
    }
    ensure(output != provenance, "projection aliases provenance")?;
    let (target_raw, target) = load(ctx, &target_ref)?;
    let (source_raw, source) = load(ctx, &source_ref)?;
    let schema_raw = ctx.read(SCHEMA)?;
    let existing = match std::fs::symlink_metadata(ctx.root().join(&provenance)) {
        Ok(_) => ctx.read(&provenance)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            ensure(opts.build, "provenance absent")?;
            vec![]
        }
        Err(e) => return Err(e.to_string()),
    };
    let event_id = opts.event_id.unwrap_or(LEGACY_EVENT);
    ensure(
        event_id.starts_with("tos.event.") && event_id.len() < 512,
        "event identity",
    )?;
    let mut found = None;
    for line in existing.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
        tos_foundation::parse_json(
            line,
            tos_foundation::JsonMode::PublishedStrict,
            tos_foundation::JsonLimits::default(),
        )
        .map_err(|e| e.to_string())?;
        let v: Value = serde_json::from_slice(line).map_err(|e| e.to_string())?;
        if v["event_id"] == event_id {
            ensure(found.is_none(), "duplicate provenance event")?;
            found = Some((line.to_vec(), v));
        }
    }
    ensure(
        opts.build || found.is_some(),
        "selected provenance event absent",
    )?;
    let historical = event_id == LEGACY_EVENT;
    if historical {
        ensure(
            found
                .as_ref()
                .is_some_and(|(raw, _)| sha(raw) == LEGACY_EVENT_SHA),
            "historical event must retain exact original bytes",
        )?;
    }
    let target_binding = json!({"ref":target_ref,"sha256":sha(&target_raw)});
    let source_binding = json!({"ref":source_ref,"sha256":sha(&source_raw)});
    let mut projected = projection(&target, &source, &target_binding, &source_binding, event_id)?;
    if historical {
        projected["authority_boundary"] = json!(LEGACY_AUTHORITY);
    }
    schema(ctx, SCHEMA, &projected)?;
    let rendered = render(&projected)?;
    let builder_ref = if historical { LEGACY_BUILDER } else { BUILDER };
    let builder_digest = if historical {
        LEGACY_BUILDER_SHA.to_owned()
    } else if let Some((_, event)) = &found {
        s(&event["method"]["artifact_digest"])?.to_owned()
    } else {
        sha(include_bytes!("transfer_route_readiness.rs"))
    };
    let event_inputs = vec![
        json!({"ref":target_ref,"sha256":sha(&target_raw),"role":"target-candidate-set"}),
        json!({"ref":source_ref,"sha256":sha(&source_raw),"role":"source-candidate-set"}),
        json!({"ref":SCHEMA,"sha256":sha(&schema_raw),"role":"readiness-contract"}),
        json!({"ref":builder_ref,"sha256":builder_digest,"role":"readiness-builder"}),
    ];
    let event = if let Some((_, event)) = &found {
        event.clone()
    } else {
        records::event(
            &event_inputs,
            &rendered,
            &projected["summary"],
            &output,
            event_id,
            &utc_now()?,
            &builder_digest,
        )
    };
    schema(ctx, "ToS/contracts/provenance-event.schema.json", &event)?;
    ensure(
        event["event_type"] == "export"
            && event["status"] == "completed_with_warnings"
            && event["rights_basis_ref"].is_null()
            && event["inputs"] == json!(event_inputs)
            && event["outputs"]
                == json!([{"ref":output,"role":"tracked-text-free-transfer-route-readiness-projection","sha256":sha(&rendered)}])
            && event["method"]["artifact_digest"] == builder_digest
            && event["method"]["configuration"] == projected["summary"]
            && event["method"]["maker_type"] == "software"
            && event["receipt_refs"] == json!([output]),
        "provenance input/output/method closure drift",
    )?;
    let mut provenance_bytes = existing.clone();
    if found.is_none() {
        if !provenance_bytes.is_empty() && !provenance_bytes.ends_with(b"\n") {
            provenance_bytes.push(b'\n');
        }
        provenance_bytes.extend(serde_json::to_vec(&event).map_err(|e| e.to_string())?);
        provenance_bytes.push(b'\n');
    }
    ensure(
        ctx.read(&target_ref)? == target_raw
            && ctx.read(&source_ref)? == source_raw
            && ctx.read(SCHEMA)? == schema_raw,
        "input changed before output",
    )?;
    let new_output = fresh_or_matching(ctx, &output, &rendered)?;
    ensure(opts.build || !new_output, "projection absent")?;
    if opts.build {
        if new_output {
            ctx.write(&output, &rendered, 0o644, true)?;
        }
        if found.is_none() {
            // Existing shared event streams are append-only; retain every earlier byte.
            if !existing.is_empty() {
                ensure(
                    ctx.read(&provenance)? == existing,
                    "provenance changed before append",
                )?;
            }
            if existing.is_empty() {
                ctx.write(&provenance, &provenance_bytes, 0o644, true)?;
            } else {
                ctx.write_replacing_exact(&provenance, &provenance_bytes, 0o644, &existing)?;
            }
        }
    }
    ctx.check()?;
    Ok(
        json!({"status":"passed","native_executor":"tos transfer-route-readiness","projection_ref":output,"provenance_ref":provenance,"event_id":event_id,"summary":projected["summary"],"historical_provenance_preserved":historical,"private_text_read":false,"publication_authorized":false,"canon_effect":false}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value {
        serde_json::from_str(include_str!("transfer_route_readiness/public-parity.json")).unwrap()
    }
    #[test]
    fn exact_projection_matches_previous_producer_and_preserves_authority() {
        let f = fixture();
        let p = projection(
            &f["target"],
            &f["source"],
            &f["target_binding"],
            &f["source_binding"],
            LEGACY_EVENT,
        )
        .unwrap();
        assert_eq!(sha(&render(&p).unwrap()), f["projection_sha256"]);
        assert_eq!(p["routes"].as_array().unwrap().len(), 35);
        assert_eq!(p["frozen_pages"].as_array().unwrap().len(), 20);
        let probe = tos_validation::SchemaBackendProbe::new(
            [tos_validation::SchemaResource {
                uri: SCHEMA_URI.into(),
                raw: include_bytes!(
                    "../../../../ToS/contracts/transfer-route-readiness-projection.schema.json"
                )
                .to_vec(),
            }],
            tos_validation::FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        assert!(
            probe
                .is_valid_raw(SCHEMA_URI, &render(&p).unwrap())
                .unwrap()
        );
        for key in [
            "source_to_target_passage_alignment",
            "eligible_for_variant_execution",
            "target_gold",
        ] {
            let mut v = p.clone();
            v["routes"][0][key] = json!(true);
            assert!(
                !probe
                    .is_valid_raw(SCHEMA_URI, &render(&v).unwrap())
                    .unwrap()
            );
        }
    }
    #[test]
    fn rejects_identity_intersection_and_authority_drift() {
        let f = fixture();
        for (side, key, value) in [
            ("target", "candidate_page_intersects_passage", json!(false)),
            ("source", "target_passage_candidate_id", json!("unbound")),
            ("source", "accepted_source_text", json!(true)),
            ("target", "eligible_for_variant_execution", json!(true)),
        ] {
            let mut v = f.clone();
            v[side]["passage_candidates"][0][key] = value;
            assert!(
                projection(
                    &v["target"],
                    &v["source"],
                    &v["target_binding"],
                    &v["source_binding"],
                    LEGACY_EVENT
                )
                .is_err(),
                "{key}"
            );
        }
    }
}
