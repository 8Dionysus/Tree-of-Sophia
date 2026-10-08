//! Fixity-bound German number-label starts; navigation text stays private.
use crate::{
    constructor_library::Out,
    jenseits_numbered_structure::{Held, append, array, field, n, o, q, render, set},
    research_execution::ResearchExecution,
    source_text_foundation::{Part, ensure, s, sha},
    transfer_target_passages::{Poppler, descendants, f},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Seek, SeekFrom},
    path::Path,
};
#[path = "nietzsche_transfer_source_structure/constants.rs"]
mod constants;
#[path = "nietzsche_transfer_source_structure/records.rs"]
mod records;
use constants::*;
type Result<T> = std::result::Result<T, String>;
const CAP: u64 = 4 * 1024 * 1024;
const BUILDER: &str = "rust/crates/tos-compiler/src/nietzsche_transfer_source_structure.rs";
fn u(v: &Value) -> Result<usize> {
    v.as_u64()
        .and_then(|x| usize::try_from(x).ok())
        .ok_or("required bounded integer".into())
}
fn v(value: &Value) -> Result<Out> {
    match value {
        Value::Null => Ok(Out::Null),
        Value::Bool(x) => Ok(Out::Bool(*x)),
        Value::String(x) => Ok(q(x)),
        Value::Number(x) => x
            .as_u64()
            .map(n)
            .or_else(|| x.as_i64().map(Out::Signed))
            .ok_or("required integer".into()),
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
    address_manifest: &'a Value,
    address_binding: Out,
    navigation_binding: Out,
    work_boundary: Out,
    inputs: Vec<Out>,
    paths: &'a BTreeMap<&'static str, String>,
    event_at: &'a str,
    series: Vec<Out>,
    trials: Vec<Out>,
    overrides: Vec<String>,
    reviewed: BTreeSet<u64>,
    total: usize,
    machine: usize,
    map_digest: &'a str,
    anchor_digest: &'a str,
    rights_ref: &'a str,
}
fn paths(c: &Value, generation: Option<&str>) -> Result<BTreeMap<&'static str, String>> {
    let root = s(&c["output_dir"])?;
    let dir = generation.map_or_else(|| root.to_string(), |g| format!("{root}/native-{g}"));
    Ok([
        ("map", "hierarchical-numbered-unit-page-map.json"),
        ("anchors", "hierarchical-numbered-unit-anchors.jsonl"),
        ("provenance", "provenance.numbered-unit-page-map.jsonl"),
    ]
    .into_iter()
    .map(|(k, f)| (k, format!("{dir}/{f}")))
    .collect())
}
fn input(reference: &str, role: &str, digest: &str) -> Out {
    o(vec![
        ("ref", q(reference)),
        ("role", q(role)),
        ("sha256", q(digest)),
    ])
}
struct Witness {
    manifest: Value,
    entry: Value,
    inventory: Value,
    held: Vec<Held>,
    payload: Held,
    profile: String,
}
impl Witness {
    fn open(
        ctx: &ResearchExecution,
        source: &ResearchExecution,
        item: &str,
        media: &str,
        profile: &str,
    ) -> Result<Self> {
        let mut held = Vec::new();
        let mut values = Vec::new();
        for name in [
            "item.manifest.json",
            "resource-inventory.json",
            "rights.json",
        ] {
            let mut h = Held::open(ctx, &format!("{item}/{name}"), CAP)?;
            values.push(
                serde_json::from_slice::<Value>(&ctx.read_file(&mut h.file, CAP)?)
                    .map_err(|e| e.to_string())?,
            );
            held.push(h);
        }
        let manifest = values.remove(0);
        let inventory = values.remove(0);
        let rights = values.remove(0);
        ensure(
            rights.is_object()
                && manifest["rights_ref"] == held[2].reference
                && manifest["resource_inventory_ref"] == held[1].reference,
            "witness metadata binding",
        )?;
        let entries = array(&manifest["payload_files"])?
            .iter()
            .filter(|p| p["media_type"] == media)
            .collect::<Vec<_>>();
        ensure(entries.len() == 1, "unique witness media")?;
        let entry = entries[0].clone();
        let payload = Held::open(
            source,
            &format!("{item}/{}", s(&entry["relative_path"])?),
            128 * 1024 * 1024,
        )?;
        ensure(
            entry["sha256"] == payload.digest
                && entry["file_id"] == format!("tos.file.sha256.{}", payload.digest)
                && entry["byte_size"].as_u64() == Some(payload.metadata.len()),
            "witness payload fixity",
        )?;
        let files = array(&inventory["files"])?
            .iter()
            .filter(|r| r["file_id"] == entry["file_id"])
            .collect::<Vec<_>>();
        ensure(files.len() == 1, "unique witness inventory")?;
        let selected = files[0].clone();
        ensure(
            selected["profile"] == profile && selected["file_sha256"] == entry["sha256"],
            "witness inventory binding",
        )?;
        Ok(Self {
            manifest,
            entry,
            inventory: selected,
            held,
            payload,
            profile: profile.into(),
        })
    }
    fn binding(&self) -> Result<Out> {
        Ok(o(vec![
            ("item_ref", v(&self.manifest["item_id"])?),
            ("file_ref", v(&self.entry["file_id"])?),
            ("file_sha256", v(&self.entry["sha256"])?),
            ("inventory_profile", q(&self.profile)),
            (
                "inventory",
                o(vec![
                    ("ref", q(&self.held[1].reference)),
                    ("sha256", q(&self.held[1].digest)),
                ]),
            ),
            (
                "rights",
                o(vec![
                    ("ref", q(&self.held[2].reference)),
                    ("sha256", q(&self.held[2].digest)),
                ]),
            ),
        ]))
    }
    fn inputs(&self, navigation: bool) -> Result<Vec<Out>> {
        Ok(vec![
            input(
                s(&self.entry["file_id"])?,
                if navigation {
                    "fixity-bound-navigation-only-provider-ocr"
                } else {
                    "fixity-bound-source-page-address-witness"
                },
                &self.payload.digest,
            ),
            input(
                &self.held[1].reference,
                if navigation {
                    "tracked-text-free-navigation-resource-inventory"
                } else {
                    "tracked-text-free-address-resource-inventory"
                },
                &self.held[1].digest,
            ),
            input(
                &self.held[2].reference,
                if navigation {
                    "navigation-witness-rights-basis"
                } else {
                    "address-witness-rights-basis"
                },
                &self.held[2].digest,
            ),
        ])
    }
    fn verify(&mut self, ctx: &ResearchExecution, source: &ResearchExecution) -> Result<()> {
        for h in &mut self.held {
            h.verify(ctx)?;
        }
        self.payload.verify(source)
    }
}
fn numeric(raw: &str) -> String {
    let x = raw
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| match c.to_ascii_lowercase() {
            'i' | 'l' | 'x' => '1',
            'o' => '0',
            's' => '5',
            'z' => '2',
            other => other,
        })
        .collect::<String>();
    if !x.is_empty() && x.bytes().all(|c| c.is_ascii_digit()) {
        x
    } else {
        String::new()
    }
}
fn pdf_numbers(ctx: &ResearchExecution, page: &Value) -> Result<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    let mut fragments = Vec::new();
    for line in array(&page["lines"])? {
        let words = array(&line["words"])?;
        if !(1..=2).contains(&words.len()) {
            continue;
        }
        let x = f(&line["x_min"])?;
        let y = f(&line["y_min"])?;
        if !(150.0..=220.0).contains(&x) || !(50.0..=520.0).contains(&y) {
            continue;
        }
        let raw = words.iter().map(s).collect::<Result<Vec<_>>>()?.join("");
        let key = numeric(&raw);
        if !key.is_empty() {
            result.insert(key);
        }
        fragments.push((x, y, raw));
    }
    for (lx, ly, lraw) in &fragments {
        for (rx, ry, rraw) in &fragments {
            ctx.tick(1)?;
            if 0.0 < rx - lx && rx - lx <= 15.0 && (ry - ly).abs() <= 3.0 {
                let key = numeric(&format!("{lraw}{rraw}"));
                if !key.is_empty() {
                    result.insert(key);
                }
            }
        }
    }
    Ok(result)
}
fn native_event(event: &mut Out, review: &Held) -> Result<()> {
    set(
        event,
        "agent_refs",
        Out::Array(vec![q("software:tos-rust"), q("software:poppler-26.01.0")]),
    )?;
    let method = field(event, "method")?;
    set(
        method,
        "artifact_digest",
        q(sha(include_bytes!(
            "nietzsche_transfer_source_structure.rs"
        ))),
    )?;
    set(
        method,
        "runtime",
        q("Tree of Sophia Rust; Poppler 26.01.0; retained source-visible review decisions"),
    )?;
    for (reference, role, digest) in [
        (
            &review.reference[..],
            "retained-source-visible-review",
            review.digest.clone(),
        ),
        (
            BUILDER,
            "native-generator-source",
            sha(include_bytes!("nietzsche_transfer_source_structure.rs")),
        ),
        (
            "rust/crates/tos-compiler/src/nietzsche_transfer_source_structure/config.json",
            "native-generator-config",
            sha(include_bytes!(
                "nietzsche_transfer_source_structure/config.json"
            )),
        ),
        (
            "rust/crates/tos-compiler/src/nietzsche_transfer_source_structure/constants.rs",
            "native-generator-constants",
            sha(include_bytes!(
                "nietzsche_transfer_source_structure/constants.rs"
            )),
        ),
        (
            "rust/crates/tos-compiler/src/nietzsche_transfer_source_structure/records.rs",
            "native-generator-records",
            sha(include_bytes!(
                "nietzsche_transfer_source_structure/records.rs"
            )),
        ),
    ] {
        append(field(event, "inputs")?, input(reference, role, &digest))?;
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
    let (mut schemas, mut schema_holds) = (vec![], vec![]);
    for name in [
        "hierarchical-source-numbered-unit-page-map",
        "source-anchor",
        "provenance-event",
    ] {
        let reference = format!("ToS/contracts/{name}.schema.json");
        let mut h = Held::open(ctx, &reference, CAP)?;
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
    let source = ctx.select_directory(options.input_root.unwrap_or(ctx.root()))?;
    let configs: Vec<Value> = serde_json::from_slice(include_bytes!(
        "nietzsche_transfer_source_structure/config.json"
    ))
    .map_err(|e| e.to_string())?;
    let mut poppler = Poppler::open(ctx)?;
    let mut outputs = Vec::new();
    let mut summaries = Vec::new();
    let mut witnesses = Vec::new();
    let mut held = Vec::new();
    for original in configs {
        let mut config = original.clone();
        let legacy = paths(&config, None)?;
        let selected = paths(&config, options.generation)?;
        let mut review = Held::open(ctx, &legacy["provenance"], CAP)?;
        let event: Value = serde_json::from_slice(&ctx.read_file(&mut review.file, CAP)?)
            .map_err(|e| e.to_string())?;
        ensure(
            event["event_id"] == config["event_id"],
            "retained event identity",
        )?;
        if let Some(g) = options.generation {
            for k in ["map_id", "event_id"] {
                config[k] = json!(format!("{}.native-{g}", s(&original[k])?));
            }
        }
        let mut address = Witness::open(
            ctx,
            &source,
            s(&config["address_item_dir"])?,
            s(&config["address_media_type"])?,
            s(&config["address_profile"])?,
        )?;
        let mut nav = if config["address_item_dir"] == config["navigation_item_dir"] {
            None
        } else {
            Some(Witness::open(
                ctx,
                &source,
                s(&config["navigation_item_dir"])?,
                s(&config["navigation_media_type"])?,
                s(&config["navigation_profile"])?,
            )?)
        };
        let mut numbers = BTreeMap::new();
        let start = u(&config["represented_start_page"])?;
        let end = u(&config["represented_end_page"])?;
        ensure(
            start <= end && end <= u(&address.inventory["summary"]["page_count"])?,
            "represented page range",
        )?;
        let navigation = nav.as_mut().unwrap_or(&mut address);
        if s(&config["candidate_profile"])?.starts_with("poppler-") {
            for first in (start..=end).step_by(8) {
                for (page, lines) in
                    poppler.pages(ctx, &navigation.payload.file, first, (first + 7).min(end))?
                {
                    numbers.insert(page, pdf_numbers(ctx, &lines)?);
                }
            }
        } else {
            ensure(
                navigation.inventory["summary"]["page_count"] == 525,
                "navigation page count",
            )?;
            let wanted = (start + 2..=end + 2).collect::<BTreeSet<_>>();
            navigation
                .payload
                .file
                .seek(SeekFrom::Start(0))
                .map_err(|e| e.to_string())?;
            crate::transfer_source_passages::visit_xml_pages(
                ctx,
                &navigation.payload.file,
                false,
                525,
                &wanted,
                |page, node| {
                    let mut words = Vec::new();
                    descendants(node, "WORD", &mut words);
                    let keys = words
                        .into_iter()
                        .filter_map(|w| {
                            let text = w
                                .content
                                .iter()
                                .take_while(|p| matches!(p, Part::Text(_)))
                                .filter_map(|p| {
                                    if let Part::Text(s) = p {
                                        Some(s.as_str())
                                    } else {
                                        None
                                    }
                                })
                                .collect::<String>();
                            let x = numeric(&text);
                            (!x.is_empty()).then_some(x)
                        })
                        .collect::<BTreeSet<_>>();
                    numbers.insert(page, keys);
                    Ok(())
                },
            )?;
        }
        let navigation = nav.as_ref().unwrap_or(&address);
        let mut inputs = address.inputs(false)?;
        if navigation.entry["file_id"] != address.entry["file_id"] {
            inputs.extend(navigation.inputs(true)?);
        }
        let mut boundary_binding = Out::Null;
        if let Some(reference) = config["work_boundary_path"].as_str() {
            let mut h = Held::open(ctx, reference, CAP)?;
            let boundary: Value = serde_json::from_slice(&ctx.read_file(&mut h.file, CAP)?)
                .map_err(|e| e.to_string())?;
            let members = array(&boundary["members"])?
                .iter()
                .filter(|m| {
                    m["work_ref"] == config["work_ref"]
                        && m["expression_ref"] == config["expression_ref"]
                })
                .collect::<Vec<_>>();
            ensure(members.len() == 1, "source work boundary resolution")?;
            let m = members[0];
            ensure(
                m["start_page"] == config["represented_start_page"]
                    && m["end_page"] == config["represented_end_page"],
                "source work boundary range",
            )?;
            boundary_binding = o(vec![
                ("ref", q(reference)),
                ("sha256", q(&h.digest)),
                ("member_sequence", v(&m["sequence"])?),
                ("start_page", v(&m["start_page"])?),
                ("end_page", v(&m["end_page"])?),
                ("epistemic_status", v(&m["epistemic_status"])?),
                ("review_status", v(&m["review_status"])?),
            ]);
            inputs.push(input(reference, "tracked-source-work-boundary", &h.digest));
            held.push(h);
        }
        let mut d = Data {
            config: &config,
            address_manifest: &address.manifest,
            address_binding: address.binding()?,
            navigation_binding: navigation.binding()?,
            work_boundary: boundary_binding,
            inputs,
            paths: &selected,
            event_at: options.event_at.unwrap_or(s(&event["started_at"])?),
            series: vec![],
            trials: vec![],
            overrides: vec![],
            reviewed: BTreeSet::new(),
            total: 0,
            machine: 0,
            map_digest: "",
            anchor_digest: "",
            rights_ref: &address.held[2].reference,
        };
        let mut anchors = Vec::new();
        for (seq, series) in array(&config["series"])?.iter().enumerate() {
            let key = s(&series["series_key"])?;
            let pages = array(&series["start_pages"])?;
            let lower = u(&series["start_page"])?;
            let upper = u(&series["end_page"])?;
            ensure(
                lower >= start
                    && upper <= end
                    && lower <= upper
                    && !pages.is_empty()
                    && pages.len() <= 300,
                "bounded source series",
            )?;
            let overrides = series["overrides"]
                .as_object()
                .ok_or("source overrides object")?;
            let mut units = Vec::new();
            let mut previous = lower;
            let before = d.machine;
            ensure(
                overrides.keys().all(|k| {
                    k.parse::<usize>()
                        .is_ok_and(|n| n > 0 && n <= pages.len() && n.to_string() == *k)
                }),
                "source override key range",
            )?;
            for (index, value) in pages.iter().enumerate() {
                ctx.tick(1)?;
                let unit = (index + 1).to_string();
                let page = u(value)?;
                ensure(
                    previous <= page && page <= upper,
                    "source series pages ordered within range",
                )?;
                previous = page;
                let offset = config["navigation_offset"]
                    .as_i64()
                    .ok_or("navigation offset")?;
                let nav_page = usize::try_from(page as i64 - offset)
                    .map_err(|_| "navigation page underflow")?;
                let basis = if let Some(p) = overrides.get(&unit) {
                    ensure(u(p)? == page, "source override page drift")?;
                    d.overrides.push(format!("{key}:{unit}"));
                    d.reviewed.insert(page as u64);
                    "source_visible_gap_review"
                } else {
                    ensure(
                        numbers.get(&nav_page).is_some_and(|x| x.contains(&unit)),
                        &format!(
                            "source number candidate absent: {} {key}:{unit} page {nav_page}",
                            s(&config["slug"])?
                        ),
                    )?;
                    d.machine += 1;
                    if nav.is_some() {
                        "provider_djvu_xml_number_candidate"
                    } else {
                        "embedded_pdf_bbox_number_candidate"
                    }
                };
                let prefix = if address.profile == "pdf_pages_v1" {
                    "pdf-page"
                } else {
                    "djvu-page"
                };
                let resource = format!("{prefix}-{page:04}");
                ensure(
                    array(&address.inventory["resources"])?
                        .iter()
                        .filter(|r| {
                            r["resource_id"] == resource
                                && r["locator"]["page_index"].as_u64() == Some(page as u64)
                        })
                        .count()
                        == 1,
                    "source page inventory",
                )?;
                let slug = s(&config["slug"])?;
                let expression = s(&config["expression_ref"])?.rsplit('.').next().unwrap();
                let original_id = format!(
                    "tos.anchor.friedrich-nietzsche.{slug}.{expression}.{key}.unit-{unit}.source-start-page"
                );
                let anchor = options
                    .generation
                    .map_or(original_id.clone(), |g| format!("{original_id}.native-{g}"));
                units.push(o(vec![
                    ("sequence", n((index + 1) as u64)),
                    ("unit_key", q(&unit)),
                    ("source_page", n(page as u64)),
                    ("resource_id", q(resource)),
                    ("navigation_page", n(nav_page as u64)),
                    ("anchor_ref", q(&anchor)),
                    ("basis", q(basis)),
                    ("status", q("proposed")),
                    ("human_review_performed", Out::Bool(false)),
                ]));
                let a = o(vec![
                    ("schema_version", q("tos_source_anchor_v1")),
                    ("anchor_id", q(anchor)),
                    ("item_id", v(&address.manifest["item_id"])?),
                    ("file_id", v(&address.entry["file_id"])?),
                    ("file_sha256", v(&address.entry["sha256"])?),
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
                                    q("tos-hierarchical-source-numbered-unit-start-v1"),
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
                                    "series-scoped fixed-page number-candidate verification plus bounded source-visible OCR-gap review",
                                ),
                            ),
                            ("version", q("1")),
                            (
                                "configuration_ref",
                                q(format!("{}#{key}-{unit}", selected["map"])),
                            ),
                        ]),
                    ),
                    ("status", q("proposed")),
                    ("provenance_event_ref", v(&config["event_id"])?),
                    ("anchor_version", n(1)),
                    ("supersedes_anchor_ref", Out::Null),
                    ("review_ref", Out::Null),
                ]);
                let raw = render(&a, false)?;
                validate("source-anchor", &raw)?;
                anchors.extend(raw);
            }
            d.total += pages.len();
            d.series.push(o(vec![
                ("series_key", q(key)),
                ("series_sequence", n((seq + 1) as u64)),
                ("series_kind", v(&series["series_kind"])?),
                ("start_page", n(lower as u64)),
                ("end_page", n(upper as u64)),
                ("expected_unit_count", n(pages.len() as u64)),
                ("status", q("proposed")),
                ("unit_starts", Out::Array(units)),
            ]));
            let mut gaps = overrides.keys().cloned().collect::<Vec<_>>();
            gaps.sort_by_key(|k| k.parse::<usize>().unwrap());
            d.trials.push(o(vec![
                ("series_key", q(key)),
                ("expected_unit_count", n(pages.len() as u64)),
                (
                    "machine_number_candidate_count",
                    n((d.machine - before) as u64),
                ),
                (
                    "source_visible_override_unit_keys",
                    Out::Array(gaps.iter().map(q).collect()),
                ),
            ]));
        }
        ensure(
            d.machine + d.overrides.len() == d.total,
            "source machine/review accounting",
        )?;
        let mut map = records::map(&d)?;
        if options.generation.is_none() {
            set(&mut map, "authority_boundary", q(LEGACY_AUTHORITY))?;
        }
        let map_raw = render(&map, true)?;
        let map_digest = sha(&map_raw);
        let anchor_digest = sha(&anchors);
        d.map_digest = &map_digest;
        d.anchor_digest = &anchor_digest;
        let mut new_event = records::event(&d)?;
        if options.generation.is_none() {
            let Out::Array(warnings) = field(&mut new_event, "warnings")? else {
                return Err("event warnings array".into());
            };
            warnings[4] = q(LEGACY_WARNING);
        } else {
            native_event(&mut new_event, &review)?;
        }
        let event_raw = render(&new_event, false)?;
        validate("hierarchical-source-numbered-unit-page-map", &map_raw)?;
        validate("provenance-event", &event_raw)?;
        summaries.push(json!({"work":config["work_ref"],"numbered_units":d.total,"machine_matches":d.machine,"reviewed_overrides":d.overrides.len(),"reviewed_pages":d.reviewed.len()}));
        for (key, raw) in [
            ("map", map_raw),
            ("anchors", anchors),
            ("provenance", event_raw),
        ] {
            outputs.push((selected[key].clone(), raw));
        }
        held.push(review);
        witnesses.push(address);
        if let Some(w) = nav {
            witnesses.push(w);
        }
    }
    for h in held.iter_mut().chain(schema_holds.iter_mut()) {
        h.verify(ctx)?;
    }
    for w in &mut witnesses {
        w.verify(ctx, &source)?;
    }
    poppler.verify()?;
    let mut missing = Vec::new();
    for (reference, bytes) in &outputs {
        match std::fs::symlink_metadata(ctx.root().join(reference)) {
            Ok(_) => {
                let retained = ctx.read(reference)?;
                ensure(
                    retained == *bytes,
                    &format!(
                        "source structure differs: {reference}; expected_sha256={} retained_sha256={}",
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_page_numeric_candidates_preserve_two_fragment_rules() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new(dir.path(), 30).unwrap();
        assert_eq!(numeric("I.o-sZ"), "1052");
        assert_eq!(numeric("б7"), "7");
        assert_eq!(numeric("foo"), "");
        let page = json!({"lines":[{"x_min":150,"y_min":50,"words":["2"]},{"x_min":165,"y_min":53,"words":["7."]},{"x_min":181,"y_min":53,"words":["8"]},{"x_min":149,"y_min":50,"words":["9"]},{"x_min":200,"y_min":53,"words":["1","2","3"]}]});
        let got = pdf_numbers(&ctx, &page).unwrap();
        assert!(got.contains("27"));
        assert!(!got.contains("78"));
        assert!(!got.contains("9"));
        assert!(!got.contains("123"));
        assert_eq!(render(&v(&json!(-2)).unwrap(), false).unwrap(), b"-2\n");
    }
}

#[cfg(test)]
mod xml_contract_tests {
    use super::*;
    #[test]
    fn djvu_stream_requires_one_root_and_counts_every_page() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new(dir.path(), 30).unwrap();
        let wanted = BTreeSet::from([1]);
        let check = |raw: &[u8]| {
            crate::transfer_source_passages::visit_xml_pages(
                &ctx,
                raw,
                false,
                1,
                &wanted,
                |_, _| Ok(()),
            )
        };
        assert!(check(b"<DjVuXML><BODY><OBJECT/></BODY></DjVuXML>").is_ok());
        for raw in [
            b"<wrong><OBJECT/></wrong>".as_slice(),
            b"<DjVuXML><OBJECT/></DjVuXML><DjVuXML/>",
            b"<DjVuXML><OBJECT/><OBJECT/></DjVuXML>",
            b"<DjVuXML><OBJECT>",
        ] {
            assert!(check(raw).is_err());
        }
    }
}
