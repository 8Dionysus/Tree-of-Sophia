//! Text-free series-qualified label intersections and candidate source routes.
//! Historical exact checks preserve the original event and producer declaration.
use crate::{
    constructor_library::Out,
    research_execution::ResearchExecution,
    source_text_foundation::{ensure, fresh_or_matching, s, schema, sha},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
};
type Result<T> = std::result::Result<T, String>;
fn q(v: impl Into<String>) -> Out {
    Out::String(v.into())
}
fn n(v: usize) -> Out {
    Out::Integer(v as u64)
}
fn b(v: bool) -> Out {
    Out::Bool(v)
}
fn o(v: Vec<(&str, Out)>) -> Out {
    Out::Object(v.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
fn arr(v: &[&str]) -> Out {
    Out::Array(v.iter().map(|s| q(*s)).collect())
}
fn render(v: &Out, pretty: bool) -> Result<Vec<u8>> {
    let mut raw = if pretty {
        serde_json::to_vec_pretty(v)
    } else {
        serde_json::to_vec(v)
    }
    .map_err(|e| e.to_string())?;
    raw.push(b'\n');
    ensure(raw.len() <= CAP as usize, "structural output byte limit")?;
    Ok(raw)
}
fn positive(v: &Value) -> Result<Out> {
    let x = v
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or("positive page required")?;
    Ok(Out::Integer(x))
}
fn text(v: &Value) -> Result<Out> {
    Ok(q(s(v)?))
}
fn file_ref(reference: &str, digest: &str) -> Out {
    o(vec![("ref", q(reference)), ("sha256", q(digest))])
}
fn input(reference: &str, role: &str, digest: &str) -> Out {
    o(vec![
        ("ref", q(reference)),
        ("role", q(role)),
        ("sha256", q(digest)),
    ])
}
struct Held {
    reference: String,
    file: File,
    digest: String,
}
fn load(ctx: &ResearchExecution, reference: &str, held: &mut Vec<Held>) -> Result<(Value, String)> {
    let mut file = ctx.source_file(reference, CAP)?;
    let raw = ctx.read_file(&mut file, CAP)?;
    let value: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    ensure(value.is_object(), "structural input object required")?;
    let digest = sha(&raw);
    held.push(Held {
        reference: reference.into(),
        file,
        digest: digest.clone(),
    });
    Ok((value, digest))
}
struct Unit<'a> {
    series: &'a str,
    key: &'a str,
    anchor: &'a str,
    page: u64,
}
fn units<'a>(
    v: &'a Value,
    page_key: &str,
    expected: usize,
) -> Result<(Vec<String>, BTreeMap<String, Unit<'a>>)> {
    let series = v["series"].as_array().ok_or("series absent")?;
    ensure(series.len() <= expected, "series count bound")?;
    let (mut order, mut map) = (Vec::new(), BTreeMap::new());
    let mut seen = BTreeSet::new();
    for row in series {
        let series = s(&row["series_key"])?;
        ensure(
            series.len() <= 64 && !series.contains(':') && seen.insert(series),
            "duplicate or invalid series key",
        )?;
        for unit in row["unit_starts"].as_array().ok_or("unit starts absent")? {
            ensure(map.len() < expected, "numbered unit count bound")?;
            let key = s(&unit["unit_key"])?;
            ensure(
                key.len() <= 32 && !key.contains(':'),
                "invalid numbered key",
            )?;
            let qualified = format!("{series}:{key}");
            let page = unit[page_key]
                .as_u64()
                .filter(|p| *p > 0)
                .ok_or("numbered page required")?;
            ensure(
                map.insert(
                    qualified.clone(),
                    Unit {
                        series,
                        key,
                        anchor: s(&unit["anchor_ref"])?,
                        page,
                    },
                )
                .is_none(),
                "duplicate qualified key",
            )?;
            order.push(qualified);
        }
    }
    ensure(map.len() == expected, "qualified label count drifted")?;
    Ok((order, map))
}
fn witness(value: &Value, reference: &str, digest: &str, source: bool) -> Result<Out> {
    let file = if source {
        &value["address_witness"]
    } else {
        &value["scan_file"]
    };
    Ok(o(vec![
        ("expression_ref", text(&value["expression_ref"])?),
        ("edition_ref", text(&value["edition_ref"])?),
        (
            "item_ref",
            text(if source {
                &file["item_ref"]
            } else {
                &value["item_ref"]
            })?,
        ),
        ("file_ref", text(&file["file_ref"])?),
        ("file_sha256", text(&file["file_sha256"])?),
        ("numbered_unit_map_ref", q(reference)),
        ("numbered_unit_map_sha256", q(digest)),
    ]))
}
pub struct Generation<'a> {
    pub name: &'a str,
    pub event_at: &'a str,
}
fn event(
    id: &str,
    at: &str,
    inputs: Vec<Out>,
    output: &str,
    digest: &str,
    role: &str,
    method: &str,
    configuration: Out,
    warnings: Vec<Out>,
    native: bool,
) -> Out {
    o(vec![
        ("schema_version", q("tos_provenance_event_v1")),
        ("event_id", q(id)),
        ("event_type", q("alignment")),
        ("started_at", q(at)),
        ("ended_at", q(at)),
        (
            "agent_refs",
            arr(&[if native {
                "software:tos-rust"
            } else {
                "software:python-standard-library"
            }]),
        ),
        ("inputs", Out::Array(inputs)),
        ("outputs", Out::Array(vec![input(output, role, digest)])),
        (
            "method",
            o(vec![
                ("maker_type", q("software")),
                ("name", q(method)),
                ("version", q("1")),
                (
                    "artifact_digest",
                    if native {
                        q(sha(include_bytes!("nietzsche_transfer_source_routes.rs")))
                    } else {
                        Out::Null
                    },
                ),
                (
                    "runtime",
                    q(if native {
                        "Tree of Sophia native Rust"
                    } else {
                        "Python standard library"
                    }),
                ),
                ("device", Out::Null),
                ("configuration", configuration),
                (
                    "prompt_or_instruction_ref",
                    q("ToS/doctrine/CORPUS_FOUNDATION.md#address-law"),
                ),
            ]),
        ),
        ("status", q("completed_with_warnings")),
        ("warnings", Out::Array(warnings)),
        ("receipt_refs", arr(&[output])),
        ("rights_basis_ref", Out::Null),
        ("event_version", n(1)),
        ("supersedes_event_ref", Out::Null),
    ])
}
fn event_at(
    ctx: &ResearchExecution,
    reference: &str,
    id: &str,
    generation: &Option<Generation<'_>>,
    held: &mut Vec<Held>,
) -> Result<String> {
    if let Some(g) = generation {
        return Ok(g.event_at.into());
    }
    let (v, _) = load(ctx, reference, held)?;
    ensure(
        s(&v["event_id"])? == id,
        "historical event identity differs",
    )?;
    Ok(s(&v["started_at"])?.into())
}
fn build_one(
    ctx: &ResearchExecution,
    c: &Config,
    generation: &Option<Generation<'_>>,
    held: &mut Vec<Held>,
) -> Result<Vec<(String, Vec<u8>, &'static str)>> {
    let (source, source_digest) = load(ctx, c.source, held)?;
    let (target, target_digest) = load(ctx, c.target, held)?;
    let (crosswalk, crosswalk_digest) = load(ctx, c.crosswalk, held)?;
    let work = format!("tos.work.friedrich-nietzsche.{}", c.slug);
    ensure(
        s(&source["work_ref"])? == work
            && source["work_ref"] == target["work_ref"]
            && source["work_ref"] == crosswalk["work_ref"],
        "selected work closure failed",
    )?;
    let (_, source_units) = units(&source, "source_page", c.expected)?;
    let (target_order, target_units) = units(&target, "pdf_page", c.expected)?;
    ensure(
        source_units.keys().eq(target_units.keys()),
        "qualified label intersection drifted",
    )?;
    let mut rights = Vec::new();
    let mut rights_inputs = Vec::new();
    let mut seen_rights = BTreeSet::new();
    for (role, which) in [
        ("source_address", "address_witness"),
        ("source_navigation", "navigation_witness"),
    ] {
        let entry = &source[which]["rights"];
        let reference = s(&entry["ref"])?;
        if !seen_rights.insert(reference.to_owned()) {
            continue;
        }
        let (_, digest) = load(ctx, reference, held)?;
        ensure(
            s(&entry["sha256"])? == digest,
            "source witness rights binding drifted",
        )?;
        rights.push(o(vec![
            ("role", q(role)),
            ("ref", q(reference)),
            ("sha256", q(&digest)),
        ]));
        rights_inputs.push(input(reference, &format!("{role}-rights-basis"), &digest));
    }
    let (_, target_rights_digest) = load(ctx, TARGET_RIGHTS, held)?;
    rights.push(o(vec![
        ("role", q("target")),
        ("ref", q(TARGET_RIGHTS)),
        ("sha256", q(&target_rights_digest)),
    ]));
    rights_inputs.push(input(
        TARGET_RIGHTS,
        "target-rights-basis",
        &target_rights_digest,
    ));
    let authority = if generation.is_some() {
        AUTHORITY
    } else {
        "mechanical pairing and routing of identical series-qualified structural number-label keys already materialized independently in exact source and target witness maps; it reads no witness text and establishes neither exact passage boundaries nor passage or translation alignment, accepted German or Russian, eligibility, gold, semantics, rights, or canon authority"
    };
    let base = format!(
        "ToS/source-witnesses/works/friedrich-nietzsche/{}/alignments/structure/{}",
        c.slug, c.pair_slug
    );
    let directory = if let Some(g) = generation {
        format!("{base}/native-{}", g.name)
    } else {
        base
    };
    let pair_ref = format!("{directory}/hierarchical-numbered-unit-label-correspondence.json");
    let pair_prov = format!("{directory}/provenance.numbered-unit-label-correspondence.jsonl");
    let route_ref = format!("{directory}/transfer-candidate-source-structural-route.v1.json");
    let route_prov =
        format!("{directory}/provenance.transfer-candidate-source-structural-route.jsonl");
    let suffix = generation
        .as_ref()
        .map(|g| format!("native-{}", g.name))
        .unwrap_or_else(|| "2026-08-08".into());
    let pair_id = format!(
        "tos.event.hierarchical-numbered-unit-label-correspondence.friedrich-nietzsche.{}.{}",
        c.slug, suffix
    );
    let route_id = format!(
        "tos.event.transfer-candidate-source-structural-route.friedrich-nietzsche.{}.{}",
        c.slug, suffix
    );
    let old_map_id = format!(
        "tos.map.hierarchical-numbered-unit-label-correspondence.friedrich-nietzsche.{}",
        c.slug
    );
    let old_route_id = format!(
        "tos.route-set.transfer-candidate-source-structural.friedrich-nietzsche.{}",
        c.slug
    );
    let map_id = if generation.is_some() {
        format!("{old_map_id}.{suffix}")
    } else {
        old_map_id.clone()
    };
    let route_set_id = if generation.is_some() {
        format!("{old_route_id}.{suffix}")
    } else {
        old_route_id
    };
    let pair_at = event_at(ctx, &pair_prov, &pair_id, generation, held)?;
    let route_at = event_at(ctx, &route_prov, &route_id, generation, held)?;
    let pairs: Vec<Out> = target_order
        .iter()
        .enumerate()
        .map(|(i, key)| {
            let s = &source_units[key];
            let t = &target_units[key];
            o(vec![
                ("sequence", n(i + 1)),
                ("qualified_unit_key", q(key)),
                ("series_key", q(t.series)),
                ("unit_key", q(t.key)),
                ("source_anchor_ref", q(s.anchor)),
                ("source_page", Out::Integer(s.page)),
                ("target_anchor_ref", q(t.anchor)),
                ("target_pdf_page", Out::Integer(t.page)),
                (
                    "basis",
                    q("shared_materialized_series_qualified_number_label_key"),
                ),
                ("status", q("proposed")),
                ("human_review_performed", b(false)),
                ("translation_alignment_claimed", b(false)),
            ])
        })
        .collect();
    let pair = o(vec![
        ("$schema", q(CORRESPONDENCE_SCHEMA)),
        (
            "schema_version",
            q("tos_hierarchical_numbered_unit_label_correspondence_v1"),
        ),
        ("map_id", q(map_id)),
        ("work_ref", q(&work)),
        (
            "source_witness",
            witness(&source, c.source, &source_digest, true)?,
        ),
        (
            "target_witness",
            witness(&target, c.target, &target_digest, false)?,
        ),
        ("rights_basis", Out::Array(rights)),
        (
            "map_authority",
            q("mechanical_shared_series_qualified_number_label_candidate_only"),
        ),
        (
            "method",
            o(vec![
                (
                    "name",
                    q("exact-shared-series-qualified-structural-label-key-intersection"),
                ),
                ("version", q("1")),
                ("maker_type", q("software")),
                ("local_payloads_read", b(false)),
                ("pairing_key", q("series_key:unit_key")),
                ("requires_materialized_label_in_both_maps", b(true)),
                ("source_to_target_text_compared", b(false)),
                ("translation_alignment_inferred", b(false)),
                ("semantic_matching_used", b(false)),
                ("no_text_emitted", b(true)),
            ]),
        ),
        ("pairings", Out::Array(pairs)),
        (
            "summary",
            o(vec![
                ("source_numbered_unit_count", n(c.expected)),
                ("target_numbered_unit_count", n(c.expected)),
                ("shared_qualified_label_count", n(c.expected)),
                ("pairing_count", n(c.expected)),
                ("source_only_qualified_keys", arr(&[])),
                ("target_only_qualified_keys", arr(&[])),
                ("all_pairing_statuses", arr(&["proposed"])),
                ("human_review_performed", b(false)),
                ("translation_alignment_claimed", b(false)),
            ]),
        ),
        ("source_text_included", b(false)),
        ("target_text_included", b(false)),
        ("provenance_ref", q(&pair_prov)),
        ("provenance_event_ref", q(&pair_id)),
        ("map_version", n(1)),
        (
            "supersedes_map_ref",
            if generation.is_some() {
                q(old_map_id)
            } else {
                Out::Null
            },
        ),
        ("authority_boundary", q(authority)),
        ("does_not_establish", arr(DOES_NOT_ESTABLISH)),
    ]);
    let pair_raw = render(&pair, true)?;
    let pair_digest = sha(&pair_raw);
    let mut pair_inputs = vec![
        input(
            c.source,
            "tracked-source-hierarchical-numbered-unit-map",
            &source_digest,
        ),
        input(
            c.target,
            "tracked-target-hierarchical-numbered-unit-map",
            &target_digest,
        ),
    ];
    pair_inputs.extend(rights_inputs);
    let pair_event = event(
        &pair_id,
        &pair_at,
        pair_inputs,
        &pair_ref,
        &pair_digest,
        "tracked-text-free-series-qualified-label-pairing-candidates",
        "exact-shared-series-qualified-structural-label-key-intersection",
        o(vec![
            ("pairing_key", q("series_key:unit_key")),
            ("expected_pairing_count", n(c.expected)),
            ("local_payloads_read", b(false)),
            ("source_to_target_text_compared", b(false)),
            ("translation_alignment_inferred", b(false)),
            ("human_review_count", n(0)),
        ]),
        vec![
            q(format!(
                "The {} pairs assert only a shared structural series-qualified number-label key.",
                c.expected
            )),
            q("No source or target witness text was read, compared, transcribed, or accepted."),
            q(if generation.is_some() {
                "Shared numbering supports structural navigation. Passage and translation alignment, equivalence, quality and semantics require their own source-visible assessment."
            } else {
                "Shared numbering does not establish passage or translation alignment, equivalence, quality, or semantics."
            }),
        ],
        generation.is_some(),
    );
    let input_candidates = crosswalk["candidates"]
        .as_array()
        .ok_or("candidates absent")?;
    ensure(
        !input_candidates.is_empty() && input_candidates.len() <= 4096,
        "candidate count bound",
    )?;
    let mut candidates = Vec::new();
    let mut route_count = 0;
    let mut candidate_ids = BTreeSet::new();
    for candidate in input_candidates {
        let id = s(&candidate["candidate_unit_id"])?;
        ensure(candidate_ids.insert(id), "duplicate candidate identity")?;
        let keys = candidate["possible_target_unit_refs"]
            .as_array()
            .ok_or("candidate routes absent")?;
        ensure(
            !keys.is_empty() && keys.len() <= c.expected,
            "candidate route count bound",
        )?;
        let mut routes = Vec::new();
        let mut seen = BTreeSet::new();
        for key in keys {
            let key = s(key)?;
            ensure(seen.insert(key), "duplicate candidate qualified key")?;
            let s = source_units
                .get(key)
                .ok_or("candidate route lacks pairing")?;
            let t = target_units
                .get(key)
                .ok_or("candidate route lacks target")?;
            routes.push(o(vec![
                ("qualified_unit_key", q(key)),
                ("source_anchor_ref", q(s.anchor)),
                ("source_page", Out::Integer(s.page)),
                ("target_anchor_ref", q(t.anchor)),
                ("target_pdf_page", Out::Integer(t.page)),
                ("basis", q("series_qualified_number_label_candidate")),
                ("status", q("proposed")),
                ("exact_passage_end_boundary_known", b(false)),
                ("source_to_target_passage_alignment_claimed", b(false)),
                ("translation_alignment_claimed", b(false)),
            ]));
        }
        route_count += routes.len();
        candidates.push(o(vec![
            ("candidate_unit_id", q(id)),
            (
                "candidate_anchor_ref",
                text(&candidate["candidate_anchor_ref"])?,
            ),
            ("target_pdf_page", positive(&candidate["target_pdf_page"])?),
            ("stratum", text(&candidate["stratum"])?),
            ("source_route_status", q("structurally-routable-ineligible")),
            ("possible_source_structural_routes", Out::Array(routes)),
            ("eligible_for_variant_execution", b(false)),
            ("target_gold_status", q("not_started")),
        ]));
    }
    let candidate_count = candidates.len();
    let route = o(vec![
        ("$schema", q(ROUTE_SCHEMA)),
        (
            "schema_version",
            q("tos_transfer_candidate_source_structural_route_v1"),
        ),
        ("route_set_id", q(route_set_id)),
        ("status", q("prepared-structural-source-routes-ineligible")),
        ("work_ref", q(work)),
        (
            "inputs",
            o(vec![
                (
                    "target_only_crosswalk",
                    file_ref(c.crosswalk, &crosswalk_digest),
                ),
                (
                    "source_numbered_unit_map",
                    file_ref(c.source, &source_digest),
                ),
                (
                    "target_numbered_unit_map",
                    file_ref(c.target, &target_digest),
                ),
                ("label_correspondence", file_ref(&pair_ref, &pair_digest)),
            ]),
        ),
        (
            "method",
            o(vec![
                (
                    "name",
                    q("target-candidate-qualified-label-to-source-structural-route"),
                ),
                ("version", q("1")),
                ("local_payloads_read", b(false)),
                ("source_or_target_text_read", b(false)),
                ("route_key", q("series_key:unit_key")),
                ("translation_alignment_inferred", b(false)),
                ("semantic_matching_used", b(false)),
            ]),
        ),
        (
            "summary",
            o(vec![
                ("candidate_page_count", n(candidate_count)),
                (
                    "structurally_source_routable_candidate_page_count",
                    n(candidate_count),
                ),
                ("possible_target_route_count", n(route_count)),
                ("source_structural_route_count", n(route_count)),
                ("human_review_count", n(0)),
                ("eligible_target_unit_count", n(0)),
                ("target_gold_count", n(0)),
            ]),
        ),
        ("candidates", Out::Array(candidates)),
        ("source_text_included", b(false)),
        ("target_text_included", b(false)),
        (
            "effects",
            o(vec![
                ("candidate_frame_changed", b(false)),
                ("exact_passage_boundary_created", b(false)),
                ("source_text_accepted", b(false)),
                ("target_text_accepted", b(false)),
                ("source_to_target_passage_alignment_created", b(false)),
                ("translation_alignment_created", b(false)),
                ("target_unit_eligible", b(false)),
                ("target_gold_created", b(false)),
                ("semantic_work_opened", b(false)),
                ("human_work_scheduled", b(false)),
            ]),
        ),
        ("provenance_event_ref", q(&route_id)),
        ("does_not_establish", arr(DOES_NOT_ESTABLISH)),
        ("authority_boundary", q(authority)),
    ]);
    let route_raw = render(&route, true)?;
    let route_digest = sha(&route_raw);
    let route_event = event(
        &route_id,
        &route_at,
        vec![
            input(
                c.crosswalk,
                "tracked-target-only-candidate-crosswalk",
                &crosswalk_digest,
            ),
            input(
                &pair_ref,
                "tracked-series-qualified-label-correspondence",
                &pair_digest,
            ),
            input(
                c.source,
                "tracked-source-hierarchical-numbered-unit-map",
                &source_digest,
            ),
            input(
                c.target,
                "tracked-target-hierarchical-numbered-unit-map",
                &target_digest,
            ),
        ],
        &route_ref,
        &route_digest,
        "tracked-text-free-transfer-candidate-source-structural-routes",
        "target-candidate-qualified-label-to-source-structural-route",
        o(vec![
            ("candidate_page_count", n(candidate_count)),
            ("source_structural_route_count", n(route_count)),
            ("local_payloads_read", b(false)),
            ("source_to_target_text_compared", b(false)),
            ("translation_alignment_inferred", b(false)),
            ("eligible_target_unit_count", n(0)),
            ("target_gold_count", n(0)),
            ("human_review_count", n(0)),
        ]),
        vec![
            q(format!(
                "All {candidate_count} frozen target pages have one or more possible source structural routes ({route_count} total)."
            )),
            q(
                "Routes are address candidates only; exact passage ends and passage or translation alignment remain unestablished.",
            ),
            q(if generation.is_some() {
                "This event records metadata-only source routes. Text review, human work, semantic assessment, eligibility and gold retain their existing states."
            } else {
                "No text was read or accepted, and no human, semantic, eligibility, or gold work was opened."
            }),
        ],
        generation.is_some(),
    );
    Ok(vec![
        (pair_ref, pair_raw, SCHEMA_PAIR),
        (
            pair_prov,
            render(&pair_event, false)?,
            "ToS/contracts/provenance-event.schema.json",
        ),
        (route_ref, route_raw, SCHEMA_ROUTE),
        (
            route_prov,
            render(&route_event, false)?,
            "ToS/contracts/provenance-event.schema.json",
        ),
    ])
}
pub fn run(
    ctx: &ResearchExecution,
    build: bool,
    generation: Option<Generation<'_>>,
) -> Result<Value> {
    ensure(
        !build || generation.is_some(),
        "build requires a new generation and event timestamp",
    )?;
    if let Some(g) = &generation {
        ensure(
            !g.name.is_empty()
                && g.name.len() <= 64
                && g.name
                    .bytes()
                    .all(|v| v.is_ascii_lowercase() || v.is_ascii_digit() || v == b'-'),
            "generation must be a lowercase name, digits or hyphens",
        )?;
    }
    let mut held = Vec::new();
    let mut outputs = Vec::new();
    for c in CONFIGS {
        ctx.tick(1)?;
        outputs.extend(build_one(ctx, c, &generation, &mut held)?);
    }
    for (_, raw, contract) in &outputs {
        let value: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
        schema(ctx, contract, &value)?;
    }
    for mut input in held {
        ensure(
            ctx.hash_file(&mut input.file, CAP)? == input.digest,
            "input changed during structural generation",
        )?;
        let mut current = ctx.source_file(&input.reference, CAP)?;
        ensure(
            ctx.hash_file(&mut current, CAP)? == input.digest,
            "selected input path changed during structural generation",
        )?;
    }
    let mut writes = Vec::new();
    for (reference, raw, _) in &outputs {
        if build {
            writes.push(fresh_or_matching(ctx, reference, raw)?);
        } else {
            let mut file = ctx.source_file(reference, CAP)?;
            let retained = ctx.read_file(&mut file, CAP)?;
            ensure(
                retained == *raw,
                &format!(
                    "retained structural output differs: {reference}; expected_sha256={}; retained_sha256={}",
                    sha(raw),
                    sha(&retained)
                ),
            )?;
        }
    }
    let mut changed = 0;
    if build {
        for ((reference, raw, _), write) in outputs.iter().zip(writes) {
            if write {
                ctx.write(reference, raw, 0o644, true)?;
                changed += 1;
            }
        }
    }
    Ok(
        json!({"status":"PASS","mode":if build{"build"}else{"check"},"works":2,"pairings":140,"changed_files":changed,"native_kernel_sha256":sha(include_bytes!("nietzsche_transfer_source_routes.rs")),"historical_bytes_reconstructed":generation.is_none(),"historical_execution_claimed":false,"local_payloads_read":false,"translation_alignment_claimed":false,"outputs":outputs.iter().map(|(r,b,_)|json!({"ref":r,"bytes":b.len(),"sha256":sha(b)})).collect::<Vec<_>>(),"execution_budget":ctx.budget_report()}),
    )
}

const CAP: u64 = 2 * 1024 * 1024;
const SCHEMA_PAIR: &str =
    "ToS/contracts/hierarchical-numbered-unit-label-correspondence.schema.json";
const SCHEMA_ROUTE: &str = "ToS/contracts/transfer-candidate-source-structural-route.schema.json";
const TARGET_RIGHTS: &str = "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/rights.json";
const CORRESPONDENCE_SCHEMA: &str = "https://tree-of-sophia.local/ToS/contracts/hierarchical-numbered-unit-label-correspondence.schema.json";
const ROUTE_SCHEMA: &str = "https://tree-of-sophia.local/ToS/contracts/transfer-candidate-source-structural-route.schema.json";
const AUTHORITY: &str = "This map pairs and routes identical series-qualified structural number-label keys from independently prepared exact source and target maps. The operation uses the maps as structural metadata.";
const DOES_NOT_ESTABLISH: &[&str] = &[
    "source_text",
    "target_text",
    "exact_line_boundaries",
    "exact_passage_end_boundaries",
    "source_to_target_passage_alignment",
    "translation_correspondence",
    "translation_equivalence",
    "translation_quality",
    "textual_identity",
    "accepted_german",
    "accepted_russian",
    "target_gold",
    "eligible_target_unit",
    "semantics",
    "rights_clearance",
    "canon_promotion",
];
struct Config {
    slug: &'static str,
    pair_slug: &'static str,
    source: &'static str,
    target: &'static str,
    crosswalk: &'static str,
    expected: usize,
}
const CONFIGS: &[Config] = &[
    Config {
        slug: "zur-genealogie-der-moral",
        pair_slug: "naumann-1892-second-svasyan-mysl-1996",
        source: "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/de-naumann-1892-second/editions/leipzig-c-g-naumann-1892-second-edition/items/wikimedia-commons-unc-scan-pdf/structure/hierarchical-numbered-unit-page-map.json",
        target: "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/ru-svasyan-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/hierarchical-numbered-unit-page-map.json",
        crosswalk: "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/ru-svasyan-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/transfer-candidate-page-crosswalk.v1.json",
        expected: 78,
    },
    Config {
        slug: "der-antichrist",
        pair_slug: "naumann-1906-flerova-mysl-1996",
        source: "ToS/source-witnesses/collections/friedrich-nietzsche/nietzsches-werke-erste-abtheilung-band-viii-naumann-1906/editions/leipzig-c-g-naumann-1906/items/wikimedia-commons-stanford-scan-djvu/structure/hierarchical-numbered-unit-page-map.json",
        target: "ToS/source-witnesses/works/friedrich-nietzsche/der-antichrist/expressions/ru-flerova-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/hierarchical-numbered-unit-page-map.json",
        crosswalk: "ToS/source-witnesses/works/friedrich-nietzsche/der-antichrist/expressions/ru-flerova-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/transfer-candidate-page-crosswalk.v1.json",
        expected: 62,
    },
];
