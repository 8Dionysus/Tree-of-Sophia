//! Hierarchical Mysl target starts and frozen transfer-page crosswalks.
//! PDF navigation remains private and every derived address stays proposed.
use crate::{
    constructor_library::Out,
    jenseits_numbered_structure::{Held, append, array, field, n, o, q, render, set},
    research_execution::ResearchExecution,
    source_text_foundation::{ensure, s, sha},
    transfer_target_passages::{Poppler, f},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    path::Path,
};
#[path = "mysl_transfer_target_structure/constants.rs"]
mod constants;
#[path = "mysl_transfer_target_structure/records.rs"]
mod records;
use constants::*;
type Result<T> = std::result::Result<T, String>;
const CAP: u64 = 4 * 1024 * 1024;
const BUILDER: &str = "rust/crates/tos-compiler/src/mysl_transfer_target_structure.rs";
fn u(v: &Value) -> Result<usize> {
    v.as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .ok_or("required bounded integer".into())
}
fn v(value: &Value) -> Result<Out> {
    match value {
        Value::Null => Ok(Out::Null),
        Value::Bool(v) => Ok(Out::Bool(*v)),
        Value::String(v) => Ok(q(v)),
        Value::Number(v) => v
            .as_u64()
            .map(n)
            .ok_or("expected nonnegative integer".into()),
        Value::Array(a) => Ok(Out::Array(a.iter().map(v).collect::<Result<_>>()?)),
        Value::Object(m) => Ok(Out::Object(
            m.iter()
                .map(|(k, x)| Ok((k.clone(), v(x)?)))
                .collect::<Result<_>>()?,
        )),
    }
}
struct Data<'a> {
    config: &'a Value,
    manifest: &'a Value,
    pdf_entry: &'a Value,
    member: &'a Value,
    paths: &'a BTreeMap<&'static str, String>,
    inventory_digest: &'a str,
    boundary_digest: &'a str,
    rights_digest: &'a str,
    plan_digest: &'a str,
    transfer_anchors_digest: &'a str,
    event_at: &'a str,
    series_payloads: &'a [Out],
    series_trials: &'a [Out],
    override_unit_refs: &'a [String],
    reviewed_pages: &'a BTreeSet<u64>,
    total_units: usize,
    total_machine_matches: usize,
    map_digest: &'a str,
    anchor_digest: &'a str,
    crosswalk_digest: &'a str,
    candidates: &'a [Value],
    possible_routes: usize,
    pages_with_starts: usize,
    random_pages: usize,
    hard_pages: usize,
    crosswalk_candidates: &'a [Out],
    crosswalk_inputs: &'a [Out],
}
fn paths(config: &Value, generation: Option<&str>) -> Result<BTreeMap<&'static str, String>> {
    let original = s(&config["output_dir"])?;
    let dir = generation.map_or_else(|| original.to_owned(), |g| format!("{original}/native-{g}"));
    Ok([
        ("map", "hierarchical-numbered-unit-page-map.json"),
        ("anchors", "hierarchical-numbered-unit-anchors.jsonl"),
        ("map_provenance", "provenance.numbered-unit-page-map.jsonl"),
        ("crosswalk", "transfer-candidate-page-crosswalk.v1.json"),
        (
            "crosswalk_provenance",
            "provenance.transfer-candidate-page-crosswalk.jsonl",
        ),
    ]
    .into_iter()
    .map(|(k, n)| (k, format!("{dir}/{n}")))
    .collect())
}
#[derive(Debug)]
struct Candidate {
    page: usize,
    key: String,
}
fn candidates(
    ctx: &ResearchExecution,
    poppler: &Poppler,
    pdf: &File,
    start: usize,
    end: usize,
) -> Result<Vec<Candidate>> {
    let mut out = vec![];
    ensure(start > 0 && start <= end && end <= 831, "work page range")?;
    for first in (start..=end).step_by(8) {
        for (page, p) in poppler.pages(ctx, pdf, first, (first + 7).min(end))? {
            let mut lines = array(&p["lines"])?.iter().collect::<Vec<_>>();
            lines.sort_by_key(|v| v["source_order"].as_u64());
            for line in lines {
                ctx.tick(1)?;
                let words = array(&line["words"])?;
                if words.len() != 1
                    || !(145.0..=175.0).contains(&f(&line["x_min"])?)
                    || !(40.0..=490.0).contains(&f(&line["y_min"])?)
                {
                    continue;
                }
                let raw = s(&words[0])?.trim();
                let key = raw.strip_suffix('.').unwrap_or(raw);
                if key.is_empty()
                    || key.len() > 3
                    || key.starts_with('0')
                    || !key.bytes().all(|b| b.is_ascii_digit())
                {
                    continue;
                }
                out.push(Candidate {
                    page,
                    key: key.into(),
                });
                ensure(out.len() <= 4096, "bounded PDF candidates")?;
            }
        }
    }
    Ok(out)
}
fn ordered(count: usize, rows: &[&Candidate]) -> (BTreeMap<String, usize>, BTreeSet<String>) {
    let mut next = 1;
    let mut pages = BTreeMap::new();
    for c in rows {
        if let Ok(n) = c.key.parse::<usize>() {
            if n >= next && n <= count {
                pages.insert(c.key.clone(), c.page);
                next = n + 1;
            }
        }
    }
    let missing = (1..=count)
        .map(|n| n.to_string())
        .filter(|k| !pages.contains_key(k))
        .collect();
    (pages, missing)
}
fn anchor_id(config: &Value, series: &str, key: &str, generation: Option<&str>) -> Result<String> {
    let slug = s(&config["slug"])?;
    let expression = s(&config["expression_ref"])?.rsplit('.').next().unwrap();
    let original = format!(
        "tos.anchor.friedrich-nietzsche.{slug}.{expression}.{series}.unit-{key}.pdf-start-page"
    );
    Ok(generation.map_or_else(|| original.clone(), |g| format!("{original}.native-{g}")))
}
fn historical_warning(event: &mut Out, index: usize, text: &str) -> Result<()> {
    let Out::Array(rows) = field(event, "warnings")? else {
        return Err("event warnings array".into());
    };
    let row = rows.get_mut(index).ok_or("event warning position")?;
    *row = q(text);
    Ok(())
}
fn native_event(event: &mut Out, review_ref: &str, review_sha: &str) -> Result<()> {
    set(
        event,
        "agent_refs",
        Out::Array(vec![q("software:tos-rust"), q("software:poppler-26.01.0")]),
    )?;
    let method = field(event, "method")?;
    set(
        method,
        "artifact_digest",
        q(sha(include_bytes!("mysl_transfer_target_structure.rs"))),
    )?;
    set(
        method,
        "runtime",
        q("Tree of Sophia Rust; Poppler 26.01.0; retained scan-review decisions"),
    )?;
    for (reference, role, digest) in [
        (
            review_ref,
            "retained-source-visible-review",
            review_sha.to_owned(),
        ),
        (
            BUILDER,
            "native-generator-source",
            sha(include_bytes!("mysl_transfer_target_structure.rs")),
        ),
        (
            "rust/crates/tos-compiler/src/mysl_transfer_target_structure/config.json",
            "native-generator-config",
            sha(include_bytes!("mysl_transfer_target_structure/config.json")),
        ),
        (
            "rust/crates/tos-compiler/src/mysl_transfer_target_structure/constants.rs",
            "native-generator-constants",
            sha(include_bytes!(
                "mysl_transfer_target_structure/constants.rs"
            )),
        ),
        (
            "rust/crates/tos-compiler/src/mysl_transfer_target_structure/records.rs",
            "native-generator-records",
            sha(include_bytes!("mysl_transfer_target_structure/records.rs")),
        ),
    ] {
        append(
            field(event, "inputs")?,
            o(vec![
                ("ref", q(reference)),
                ("role", q(role)),
                ("sha256", q(digest)),
            ]),
        )?;
    }
    Ok(())
}
pub struct Options<'a> {
    pub build: bool,
    pub input_root: Option<&'a Path>,
    pub generation: Option<&'a str>,
    pub event_at: Option<&'a str>,
}
pub fn run(ctx: &ResearchExecution, options: Options<'_>) -> Result<Value> {
    ensure(
        !options.build || options.generation.is_some(),
        "build requires new generation",
    )?;
    ensure(
        options.generation.is_some() == options.event_at.is_some(),
        "generation and event-at required together",
    )?;
    if let Some(g) = options.generation {
        ensure(
            !g.is_empty()
                && g.len() <= 48
                && g.as_bytes()[0].is_ascii_alphanumeric()
                && g.as_bytes().last().unwrap().is_ascii_alphanumeric()
                && g.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "generation syntax",
        )?;
    }
    let mut held = Vec::new();
    let mut values = Vec::new();
    for reference in [
        MANIFEST_PATH,
        INVENTORY_PATH,
        RIGHTS_PATH,
        WORK_BOUNDARY_PATH,
        TRANSFER_PLAN_PATH,
        TRANSFER_ANCHOR_PATH,
    ] {
        let mut h = Held::open(ctx, reference, CAP)?;
        let raw = ctx.read_file(&mut h.file, CAP)?;
        let value = if reference == TRANSFER_ANCHOR_PATH {
            Value::Null
        } else {
            serde_json::from_slice(&raw).map_err(|e| e.to_string())?
        };
        held.push(h);
        values.push(value);
    }
    let (mut schemas, mut schema_holds) = (vec![], vec![]);
    for reference in [
        "ToS/contracts/hierarchical-target-numbered-unit-page-map.schema.json",
        "ToS/contracts/transfer-candidate-target-structural-crosswalk.schema.json",
        "ToS/contracts/source-anchor.schema.json",
        "ToS/contracts/provenance-event.schema.json",
    ] {
        let mut h = Held::open(ctx, reference, CAP)?;
        schemas.push(tos_validation::SchemaResource {
            uri: format!("https://tree-of-sophia.local/{reference}"),
            raw: ctx.read_file(&mut h.file, CAP)?,
        });
        schema_holds.push(h);
    }
    let schemas = tos_validation::SchemaBackendProbe::new(
        schemas,
        tos_validation::FormatProfile::AssertedSourceCandidateV1,
    )
    .map_err(|e| format!("prepare schemas: {e:?}"))?;
    let validate = |name: &str, raw: &[u8]| -> Result<()> {
        ensure(
            schemas
                .is_valid_raw(
                    &format!("https://tree-of-sophia.local/ToS/contracts/{name}.schema.json"),
                    raw,
                )
                .map_err(|e| format!("schema: {e:?}"))?,
            &format!("{name} schema refused"),
        )
    };
    let (manifest, inventory, rights, boundary, plan) =
        (&values[0], &values[1], &values[2], &values[3], &values[4]);
    ensure(
        rights.is_object()
            && manifest["rights_ref"] == RIGHTS_PATH
            && manifest["resource_inventory_ref"] == INVENTORY_PATH,
        "target metadata binding",
    )?;
    let entries = array(&manifest["payload_files"])?;
    ensure(
        entries.len() == 1 && entries[0]["media_type"] == "application/pdf",
        "one PDF required",
    )?;
    let entry = &entries[0];
    let source = ctx.select_directory(options.input_root.unwrap_or(ctx.root()))?;
    let mut pdf = Held::open(
        &source,
        &format!("{TARGET_ITEM_DIR}/{}", s(&entry["relative_path"])?),
        128 * 1024 * 1024,
    )?;
    ensure(
        entry["sha256"] == pdf.digest
            && entry["byte_size"].as_u64() == Some(pdf.metadata.len())
            && entry["file_id"] == format!("tos.file.sha256.{}", pdf.digest),
        "target payload fixity",
    )?;
    let files = array(&inventory["files"])?;
    ensure(files.len() == 1, "target inventory file count")?;
    let inv = &files[0];
    ensure(
        inv["profile"] == "pdf_pages_v1"
            && inv["file_id"] == entry["file_id"]
            && inv["file_sha256"] == entry["sha256"]
            && inv["summary"]["page_count"] == 831,
        "target inventory binding",
    )?;
    let configs: Vec<Value> =
        serde_json::from_slice(include_bytes!("mysl_transfer_target_structure/config.json"))
            .map_err(|e| e.to_string())?;
    let mut poppler = Poppler::open(ctx)?;
    let mut outputs = Vec::new();
    let mut summaries = Vec::new();
    let mut reviews = Vec::new();
    for original_config in configs {
        ctx.check()?;
        let legacy_paths = paths(&original_config, None)?;
        let selected_paths = paths(&original_config, options.generation)?;
        let mut events = Vec::new();
        let mut review_holds = Vec::new();
        for key in ["map_provenance", "crosswalk_provenance"] {
            let mut h = Held::open(ctx, &legacy_paths[key], CAP)?;
            events.push(
                serde_json::from_slice::<Value>(&ctx.read_file(&mut h.file, CAP)?)
                    .map_err(|e| e.to_string())?,
            );
            review_holds.push(h);
        }
        ensure(
            events[0]["event_id"] == original_config["map_event_id"]
                && events[1]["event_id"] == original_config["crosswalk_event_id"],
            "retained event identity",
        )?;
        let mut config = original_config.clone();
        if let Some(g) = options.generation {
            for key in [
                "map_id",
                "map_event_id",
                "crosswalk_id",
                "crosswalk_event_id",
            ] {
                config[key] = json!(format!("{}.native-{g}", s(&original_config[key])?));
            }
        }
        let members = array(&boundary["members"])?
            .iter()
            .filter(|m| {
                m["sequence"] == config["member_sequence"]
                    && m["work_ref"] == config["work_ref"]
                    && m["expression_ref"] == config["expression_ref"]
            })
            .collect::<Vec<_>>();
        ensure(members.len() == 1, "work boundary resolution")?;
        let member = members[0];
        ensure(
            member["start_page"] == config["start_page"]
                && member["end_page"] == config["end_page"],
            "work boundary pages",
        )?;
        let rows = candidates(
            ctx,
            &poppler,
            &pdf.file,
            u(&config["start_page"])?,
            u(&config["end_page"])?,
        )?;
        let (mut series_payloads, mut trials, mut anchors, mut override_refs, mut starts) =
            (vec![], vec![], vec![], vec![], vec![]);
        let mut reviewed = BTreeSet::new();
        let mut total = 0;
        let mut matched = 0;
        for (seq, series) in array(&config["series"])?.iter().enumerate() {
            let key = s(&series["series_key"])?;
            let count = u(&series["expected_unit_count"])?;
            ensure(count <= 300, "series unit bound")?;
            let lower = u(&series["start_page"])?;
            let upper = u(&series["end_page"])?;
            let selected = rows
                .iter()
                .filter(|r| lower <= r.page && r.page <= upper)
                .collect::<Vec<_>>();
            let (mut pages, missing) = ordered(count, &selected);
            let overrides = series["overrides"]
                .as_object()
                .ok_or("series overrides object")?;
            ensure(
                missing == overrides.keys().cloned().collect(),
                "series bbox gaps differ",
            )?;
            let machine = pages.len();
            matched += machine;
            total += count;
            for (k, p) in overrides {
                pages.insert(k.clone(), u(p)?);
                override_refs.push(format!("{key}:{k}"));
                reviewed.insert(u(p)? as u64);
            }
            ensure(
                pages.len() == count
                    && (1..count).all(|n| pages[&n.to_string()] <= pages[&(n + 1).to_string()]),
                "series coverage or order",
            )?;
            let mut units = Vec::new();
            for number in 1..=count {
                let unit = number.to_string();
                let page = pages[&unit];
                let resource = format!("pdf-page-{page:04}");
                ensure(
                    array(&inv["resources"])?
                        .iter()
                        .filter(|v| {
                            v["resource_id"] == resource
                                && v["locator"]["page_index"].as_u64() == Some(page as u64)
                        })
                        .count()
                        == 1,
                    "target page inventory",
                )?;
                let anchor_id = anchor_id(&config, key, &unit, options.generation)?;
                units.push(o(vec![
                    ("sequence", n(number as u64)),
                    ("unit_key", q(&unit)),
                    ("pdf_page", n(page as u64)),
                    ("resource_id", q(resource)),
                    ("anchor_ref", q(&anchor_id)),
                    (
                        "basis",
                        q(if overrides.contains_key(&unit) {
                            "source_visible_gap_review"
                        } else {
                            "embedded_pdf_bbox_order_candidate"
                        }),
                    ),
                    ("status", q("proposed")),
                    ("human_review_performed", Out::Bool(false)),
                ]));
                starts.push((format!("{key}:{unit}"), page));
                let slug = s(&config["slug"])?;
                let expression = s(&config["expression_ref"])?.rsplit('.').next().unwrap();
                let anchor = o(vec![
                    ("schema_version", q("tos_source_anchor_v1")),
                    ("anchor_id", q(anchor_id)),
                    ("item_id", v(&manifest["item_id"])?),
                    ("file_id", v(&entry["file_id"])?),
                    ("file_sha256", v(&entry["sha256"])?),
                    (
                        "passage_id",
                        q(format!(
                            "tos.passage.friedrich-nietzsche.{slug}.{expression}.{key}.unit-{unit}"
                        )),
                    ),
                    (
                        "selectors",
                        Out::Array(vec![
                            o(vec![
                                ("type", q("structural")),
                                (
                                    "path",
                                    Out::Array(vec![
                                        q(format!("work:{slug}")),
                                        q(format!("expression:{expression}")),
                                        q(format!("series:{key}")),
                                        q(format!("numbered-unit:{unit}")),
                                    ]),
                                ),
                                (
                                    "scheme",
                                    q("tos-hierarchical-target-numbered-unit-start-v1"),
                                ),
                            ]),
                            o(vec![
                                ("type", q("page_region")),
                                ("page", n(page as u64)),
                                ("x", n(0)),
                                ("y", n(0)),
                                ("width", n(1)),
                                ("height", n(1)),
                                ("coordinate_space", q("normalized_0_1")),
                            ]),
                        ]),
                    ),
                    (
                        "selector_method",
                        o(vec![
                            ("maker_type", q("mixed")),
                            (
                                "method",
                                q(
                                    "series-scoped ordered embedded-PDF bbox numeral candidate plus bounded target-visible gap review",
                                ),
                            ),
                            ("version", q("1")),
                            (
                                "configuration_ref",
                                q(format!("{}#{key}-{unit}", selected_paths["map"])),
                            ),
                        ]),
                    ),
                    ("status", q("proposed")),
                    ("provenance_event_ref", v(&config["map_event_id"])?),
                    ("anchor_version", n(1)),
                    ("supersedes_anchor_ref", Out::Null),
                    ("review_ref", Out::Null),
                ]);
                let raw = render(&anchor, false)?;
                validate("source-anchor", &raw)?;
                anchors.extend(raw);
            }
            series_payloads.push(o(vec![
                ("series_key", q(key)),
                ("series_sequence", n((seq + 1) as u64)),
                ("series_kind", v(&series["series_kind"])?),
                ("start_page", n(lower as u64)),
                ("end_page", n(upper as u64)),
                ("expected_unit_count", n(count as u64)),
                ("status", q("proposed")),
                ("unit_starts", Out::Array(units)),
            ]));
            trials.push(o(vec![
                ("series_key", q(key)),
                ("expected_unit_count", n(count as u64)),
                ("ordered_bbox_candidate_match_count", n(machine as u64)),
                (
                    "source_visible_override_unit_keys",
                    Out::Array(overrides.keys().map(q).collect()),
                ),
            ]));
        }
        let frozen = array(&plan["candidate_target_units"])?
            .iter()
            .filter(|v| v["work_ref"] == config["work_ref"])
            .cloned()
            .collect::<Vec<_>>();
        ensure(frozen.len() == 6, "frozen candidate count")?;
        let mut cross_candidates = vec![];
        let (mut possible, mut with_starts) = (0, 0);
        for candidate in &frozen {
            let page = u(&candidate["page"])?;
            let prior = starts
                .iter()
                .filter(|(_, p)| *p < page)
                .last()
                .ok_or("preceding unit absent")?;
            let after = starts
                .iter()
                .find(|(_, p)| *p > page)
                .ok_or("following unit absent")?;
            let on = starts
                .iter()
                .filter(|(_, p)| *p == page)
                .collect::<Vec<_>>();
            let mut refs = vec![q(&prior.0)];
            refs.extend(on.iter().map(|(k, _)| q(k)));
            possible += refs.len();
            with_starts += usize::from(!on.is_empty());
            cross_candidates.push(o(vec![
                ("candidate_unit_id", v(&candidate["unit_id"])?),
                ("candidate_anchor_ref", v(&candidate["anchor_ref"])?),
                ("target_pdf_page", n(page as u64)),
                ("stratum", v(&candidate["stratum"])?),
                (
                    "page_relation",
                    q(if on.is_empty() {
                        "within-one-proposed-target-numbered-unit"
                    } else {
                        "prior-target-unit-spill-plus-unit-starts"
                    }),
                ),
                ("possible_target_unit_refs", Out::Array(refs)),
                (
                    "starts_on_page_target_unit_refs",
                    Out::Array(on.iter().map(|(k, _)| q(k)).collect()),
                ),
                (
                    "next_proposed_start",
                    o(vec![
                        ("target_unit_ref", q(&after.0)),
                        ("target_pdf_page", n(after.1 as u64)),
                    ]),
                ),
                ("source_parallel_route_status", q("not_materialized")),
                ("exact_page_end_boundary_known", Out::Bool(false)),
                ("eligible_for_variant_execution", Out::Bool(false)),
                ("target_gold_status", q("not_started")),
            ]));
        }
        let mut d = Data {
            config: &config,
            manifest,
            pdf_entry: entry,
            member,
            paths: &selected_paths,
            inventory_digest: &held[1].digest,
            boundary_digest: &held[3].digest,
            rights_digest: &held[2].digest,
            plan_digest: &held[4].digest,
            transfer_anchors_digest: &held[5].digest,
            event_at: options.event_at.unwrap_or(s(&events[0]["started_at"])?),
            series_payloads: &series_payloads,
            series_trials: &trials,
            override_unit_refs: &override_refs,
            reviewed_pages: &reviewed,
            total_units: total,
            total_machine_matches: matched,
            map_digest: "",
            anchor_digest: "",
            crosswalk_digest: "",
            candidates: &frozen,
            possible_routes: possible,
            pages_with_starts: with_starts,
            random_pages: frozen.iter().filter(|v| v["stratum"] == "random").count(),
            hard_pages: frozen.iter().filter(|v| v["stratum"] == "hard").count(),
            crosswalk_candidates: &cross_candidates,
            crosswalk_inputs: &[],
        };
        let mut map = records::map(&d)?;
        if options.generation.is_none() {
            set(&mut map, "authority_boundary", q(LEGACY_MAP_AUTHORITY))?;
        }
        if options.generation.is_some() {
            set(
                &mut map,
                "supersedes_map_ref",
                v(&original_config["map_id"])?,
            )?;
        }
        let map_raw = render(&map, true)?;
        let map_hash = sha(&map_raw);
        let anchor_hash = sha(&anchors);
        d.map_digest = &map_hash;
        d.anchor_digest = &anchor_hash;
        let mut event = records::map_event(&d)?;
        if options.generation.is_none() {
            historical_warning(&mut event, 4, LEGACY_MAP_WARNING)?;
            // This retained segmentation event predates the later rights revision.
            // Current rights still own the crosswalk and every fresh generation.
            let prior_rights = array(&events[0]["inputs"])?
                .iter()
                .filter(|v| v["ref"] == RIGHTS_PATH)
                .collect::<Vec<_>>();
            ensure(
                prior_rights.len() == 1
                    && prior_rights[0]["sha256"]
                        == "4c72113047353c6148af2ef3580e4128e8a968558f45331542081d7e0e761de0",
                "historical map rights binding differs",
            )?;
            let Out::Array(inputs) = field(&mut event, "inputs")? else {
                return Err("event input array".into());
            };
            set(&mut inputs[3], "sha256", q(s(&prior_rights[0]["sha256"])?))?;
        }
        if options.generation.is_some() {
            native_event(
                &mut event,
                &legacy_paths["map_provenance"],
                &review_holds[0].digest,
            )?;
        }
        let event_raw = render(&event, false)?;
        let mut cross = records::crosswalk(&d)?;
        if options.generation.is_none() {
            set(
                &mut cross,
                "authority_boundary",
                q(LEGACY_CROSSWALK_AUTHORITY),
            )?;
        }
        // Retained crosswalk bytes include a later plan/rights update. The
        // earlier event still names its original inputs and original output.
        // Reproduce those separate historical layers; a fresh generation binds
        // every current input and the bytes actually emitted in that generation.
        let historical_event_cross_hash = if options.generation.is_none() {
            const PLAN_SHA: &str =
                "adad0534a5ce61f3eaa821aaa19fcc7257958c7baac06b75f30b321f65f36cb6";
            const RIGHTS_SHA: &str =
                "4c72113047353c6148af2ef3580e4128e8a968558f45331542081d7e0e761de0";
            let mut earlier = cross.clone();
            for (key, reference, expected) in [
                ("transfer_plan", TRANSFER_PLAN_PATH, PLAN_SHA),
                ("target_rights", RIGHTS_PATH, RIGHTS_SHA),
            ] {
                let recorded = array(&events[1]["inputs"])?
                    .iter()
                    .filter(|v| v["ref"] == reference)
                    .collect::<Vec<_>>();
                ensure(
                    recorded.len() == 1 && recorded[0]["sha256"] == expected,
                    "historical crosswalk input binding differs",
                )?;
                set(
                    field(field(&mut earlier, "inputs")?, key)?,
                    "sha256",
                    q(expected),
                )?;
            }
            let digest = sha(&render(&earlier, true)?);
            let recorded = array(&events[1]["outputs"])?;
            ensure(
                recorded.len() == 1
                    && recorded[0]["ref"] == legacy_paths["crosswalk"]
                    && recorded[0]["sha256"] == digest,
                "historical crosswalk output binding differs",
            )?;
            Some(digest)
        } else {
            None
        };
        let mut cross_raw = render(&cross, true)?;
        if options.generation.is_none() {
            // Preserve the one retained indentation change in the later plan
            // digest line. It changes exact bytes, never JSON field meaning.
            let current = format!("\n      \"sha256\": \"{}\"", held[4].digest);
            let retained = format!("\n    \"sha256\": \"{}\"", held[4].digest);
            let text = String::from_utf8(cross_raw).map_err(|_| "crosswalk UTF8")?;
            ensure(
                text.matches(&current).count() == 1,
                "historical plan digest formatting differs",
            )?;
            cross_raw = text.replacen(&current, &retained, 1).into_bytes();
        }
        let cross_hash = sha(&cross_raw);
        d.crosswalk_digest = historical_event_cross_hash
            .as_deref()
            .unwrap_or(&cross_hash);
        d.event_at = options.event_at.unwrap_or(s(&events[1]["started_at"])?);
        let bindings = [
            (
                TRANSFER_PLAN_PATH,
                "frozen-golden-kernel-transfer-candidate-plan",
                held[4].digest.as_str(),
            ),
            (
                TRANSFER_ANCHOR_PATH,
                "frozen-target-page-anchor-set",
                held[5].digest.as_str(),
            ),
            (
                selected_paths["map"].as_str(),
                "tracked-hierarchical-target-numbered-unit-page-map",
                map_hash.as_str(),
            ),
            (RIGHTS_PATH, "target-rights-basis", held[2].digest.as_str()),
        ]
        .into_iter()
        .map(|(r, role, digest)| {
            o(vec![
                ("ref", q(r)),
                ("role", q(role)),
                ("sha256", q(digest)),
            ])
        })
        .collect::<Vec<_>>();
        d.crosswalk_inputs = &bindings;
        let mut cross_event = records::crosswalk_event(&d)?;
        if options.generation.is_none() {
            historical_warning(&mut cross_event, 3, LEGACY_CROSSWALK_WARNING)?;
            let Out::Array(inputs) = field(&mut cross_event, "inputs")? else {
                return Err("crosswalk event inputs".into());
            };
            for (at, reference) in [(0, TRANSFER_PLAN_PATH), (3, RIGHTS_PATH)] {
                let recorded = array(&events[1]["inputs"])?
                    .iter()
                    .find(|v| v["ref"] == reference)
                    .ok_or("historical crosswalk input absent")?;
                set(&mut inputs[at], "sha256", q(s(&recorded["sha256"])?))?;
            }
        }
        if options.generation.is_some() {
            native_event(
                &mut cross_event,
                &legacy_paths["crosswalk_provenance"],
                &review_holds[1].digest,
            )?;
        }
        let cross_event_raw = render(&cross_event, false)?;
        for (name, bytes) in [
            ("hierarchical-target-numbered-unit-page-map", &map_raw),
            ("transfer-candidate-target-structural-crosswalk", &cross_raw),
            ("provenance-event", &event_raw),
            ("provenance-event", &cross_event_raw),
        ] {
            validate(name, bytes)?;
        }
        summaries.push(json!({"work":config["work_ref"],"numbered_units":total,"machine_matches":matched,"reviewed_overrides":override_refs.len(),"candidate_pages":frozen.len(),"possible_target_routes":possible}));
        for (key, raw) in [
            ("map", map_raw),
            ("anchors", anchors),
            ("map_provenance", event_raw),
            ("crosswalk", cross_raw),
            ("crosswalk_provenance", cross_event_raw),
        ] {
            outputs.push((selected_paths[key].clone(), raw));
        }
        reviews.extend(review_holds);
    }
    for h in held
        .iter_mut()
        .chain(schema_holds.iter_mut())
        .chain(reviews.iter_mut())
    {
        h.verify(ctx)?;
    }
    pdf.verify(&source)?;
    poppler.verify()?;
    let mut missing = Vec::new();
    for (reference, bytes) in &outputs {
        match std::fs::symlink_metadata(ctx.root().join(reference)) {
            Ok(_) => {
                let retained = ctx.read(reference)?;
                ensure(
                    retained == *bytes,
                    &format!(
                        "structure differs: {reference}; expected_sha256={} retained_sha256={}",
                        sha(bytes),
                        sha(&retained)
                    ),
                )?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ensure(options.build, &format!("output missing: {reference}"))?;
                missing.push(reference.as_str());
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    for (reference, bytes) in &outputs {
        if missing.contains(&reference.as_str()) {
            ctx.write(reference, bytes, 0o644, true)?;
        }
    }
    Ok(
        json!({"status":if options.build{"built"}else{"current"},"generator":BUILDER,"generation":options.generation,"historical_reconstruction_only":options.generation.is_none(),"works":summaries,"outputs_written":missing.len(),"outputs":outputs.iter().map(|(r,b)|json!({"ref":r,"bytes":b.len(),"sha256":sha(b)})).collect::<Vec<_>>(),"accepted_source_text":false,"canon_effect":false,"execution_budget":ctx.budget_report()}),
    )
}
