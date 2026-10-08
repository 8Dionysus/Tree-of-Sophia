//! Complete raw TEI return for a fixity-selected exact-form observation.
use super::{
    Held, META_CAP, PACKET_CAP, ResearchExecution, Result, canonical, digest_bound, ensure,
    generated_path, present, read_json_bounded, s, sha, valid_generation,
};
use crate::source_text_foundation::{Node, Part, xml_with_doctype};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
const BUILDER: &str = "rust/crates/tos-compiler/src/lexical_derivatives/semantic_recurrence.rs";
pub const PLAN: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/semantic-source-recurrence-plan.v1.json";
const RECEIPT_SHA: &str = "b50417321708d85ee6ec1dfc2225fe6e2206cd590fd043516a7d894843da438f";
const EVENT_SHA: &str = "b56a8a6cbdeab1fa6d6c971dea6e6419582de0b16629d65b981e6f5835f310af";
const EVENT_ID: &str = "tos.event.annotation.zarathustra-semantic-source-recurrence-v1.2026-08-10";
const AUTHORITY: &str = "This bundle returns the complete private witness context for one preselected exact-form hash and records aggregate recurrence observations.";
fn u(v: &Value) -> Result<u64> {
    v.as_u64()
        .ok_or("unsigned source recurrence integer required".into())
}
fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array()
        .ok_or("source recurrence array required".into())
}
/// The stored TEI selectors are a finite child-path grammar, never arbitrary XPath.
fn step(raw: &str) -> Result<(&str, usize)> {
    let (name, index) = if let Some((name, position)) = raw.split_once('[') {
        let digits = position
            .strip_suffix(']')
            .ok_or("TEI selector closing bracket")?;
        ensure(
            !digits.is_empty()
                && !digits.starts_with('0')
                && digits.bytes().all(|c| c.is_ascii_digit()),
            "TEI selector index",
        )?;
        (
            name,
            digits
                .parse::<usize>()
                .map_err(|_| "TEI selector index overflow")?,
        )
    } else {
        (raw, 1)
    };
    ensure(
        !name.is_empty()
            && (name == "text()"
                || name == "tail()"
                || name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_.:-".contains(&c))),
        "TEI selector name",
    )?;
    Ok((name, index))
}
pub(super) fn text_node<'a>(root: &'a Node, path: &str) -> Result<&'a str> {
    ensure(path.len() <= 16384, "TEI selector byte bound")?;
    let mut steps = path.strip_prefix('/').unwrap_or(path).split('/');
    let (name, index) = step(steps.next().ok_or("empty TEI selector")?)?;
    ensure(name == root.name && index == 1, "TEI selector root")?;
    let mut node = root;
    let mut parent: Option<(&Node, usize)> = None;
    while let Some(raw) = steps.next() {
        let (name, index) = step(raw)?;
        if name == "text()" || name == "tail()" {
            ensure(
                steps.next().is_none(),
                "TEI character data must terminate selector",
            )?;
            let content = if name == "text()" {
                node.content.as_slice()
            } else {
                let (p, i) = parent.ok_or("TEI root has no tail")?;
                &p.content[i + 1..]
            };
            return content
                .iter()
                .filter_map(|p| match p {
                    Part::Text(t) if !t.is_empty() => Some(t.as_str()),
                    _ => None,
                })
                .nth(index - 1)
                .ok_or("TEI character data selector missing".into());
        }
        let mut matches = node
            .content
            .iter()
            .enumerate()
            .filter_map(|(i, p)| match p {
                Part::Child(n) if n.name == name => Some((i, n)),
                _ => None,
            });
        let (i, next) = matches.nth(index - 1).ok_or("TEI child selector missing")?;
        parent = Some((node, i));
        node = next;
    }
    Err("TEI source return requires a character-data selector".into())
}
pub(super) fn unicode_slice(text: &str, start: u64, end: u64) -> Result<String> {
    ensure(end >= start, "source offset order")?;
    let start = usize::try_from(start).map_err(|_| "source offset overflow")?;
    let end = usize::try_from(end).map_err(|_| "source offset overflow")?;
    ensure(
        end <= text.chars().count(),
        "source offset outside character data",
    )?;
    Ok(text.chars().skip(start).take(end - start).collect())
}
struct Source {
    expected: Value,
    root: Node,
}
#[derive(Default)]
struct PartCount {
    count: u64,
    pages: BTreeSet<String>,
    sections: BTreeSet<String>,
}
fn reconstruct(
    ctx: &ResearchExecution,
    input: &ResearchExecution,
    plan: &Value,
    plan_ref: &str,
    plan_sha: &str,
    lexical: &Value,
    recurrence: &Value,
    row: &Value,
    database: &Held,
    held: &mut Vec<Held>,
    payloads: &mut Vec<Held>,
) -> Result<Value> {
    let selected = &plan["selected_source_observation"];
    let target_sha = s(&selected["exact_form_sha256"])?;
    let mut sources = BTreeMap::new();
    let mut bindings = Vec::new();
    let mut last_order = 0;
    for expected in array(&plan["source_items"])? {
        ctx.tick(1)?;
        let item = s(&expected["item_ref"])?;
        let order = u(&expected["part_order"])?;
        ensure(
            order == last_order + 1,
            "source recurrence parts must be ordered 1..4",
        )?;
        last_order = order;
        let rows = array(&lexical["source_items"])?
            .iter()
            .filter(|v| v["item_ref"] == item)
            .collect::<Vec<_>>();
        ensure(rows.len() == 1, "source recurrence Item must resolve once")?;
        let source = rows[0];
        ensure(
            source["file_sha256"] == expected["file_sha256"] && source["part_order"] == order,
            "source recurrence Item digest/order drift",
        )?;
        let manifest_ref = s(&source["manifest_ref"])?;
        let manifest = read_json_bounded(ctx, manifest_ref, META_CAP, held)?;
        let manifest_sha = held.last().unwrap().digest.clone();
        ensure(
            manifest["item_id"] == item,
            "source recurrence manifest identity drift",
        )?;
        let files = array(&manifest["payload_files"])?
            .iter()
            .filter(|v| v["sha256"] == expected["file_sha256"])
            .collect::<Vec<_>>();
        ensure(files.len() == 1, "source payload must resolve once")?;
        let payload_ref = Path::new(manifest_ref)
            .parent()
            .ok_or("manifest parent")?
            .join(s(&files[0]["relative_path"])?)
            .to_str()
            .ok_or("payload reference UTF-8")?
            .to_owned();
        let mut payload = Held::open(input, &payload_ref, META_CAP)?;
        ensure(
            payload.digest == s(&expected["file_sha256"])?,
            "source payload digest drift",
        )?;
        let raw = input.read_file(&mut payload.file, META_CAP)?;
        let root = xml_with_doctype(input, &raw, false)?;
        ensure(root.name == "TEI", "source root must be TEI")?;
        bindings.push(json!({"part_order":order,"item_ref":item,"manifest_ref":manifest_ref,"manifest_sha256":manifest_sha,"payload_ref":payload_ref,"payload_sha256":payload.digest}));
        payloads.push(payload);
        ensure(
            sources
                .insert(
                    item.to_owned(),
                    Source {
                        expected: expected.clone(),
                        root,
                    },
                )
                .is_none(),
            "duplicate source recurrence Item",
        )?;
    }
    ensure(last_order == 4, "source recurrence requires four Items")?;
    let db = input.open_sqlite_readonly_for_ordered_scan(&database.file)?;
    ensure(
        db.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            == "ok",
        "source recurrence SQLite quick_check",
    )?;
    let form:(String,String,String,String,String,i64)=db.query_row("SELECT form_key,exact_form,normalized_form,exact_form_sha256,normalized_form_sha256,occurrence_count FROM forms WHERE exact_form_sha256=?",[target_sha],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(|e|e.to_string())?;
    ensure(
        form.0 == s(&selected["form_key"])?
            && form.3 == target_sha
            && sha(form.1.as_bytes()) == target_sha
            && sha(form.2.as_bytes()) == form.4
            && form.5 > 0
            && form.5 as u64 == u(&plan["expected_tracked_recurrence_tuple"]["occurrence_count"])?,
        "source recurrence selected form drift",
    )?;
    let mut statement=db.prepare("SELECT o.occurrence_id,o.item_ref,s.part_order,s.file_sha256,o.token_ordinal,o.form_key,o.exact_form,o.exact_form_sha256,o.page_resource_id,o.section_resource_id,o.text_node_path,o.start_offset,o.end_offset,o.editorial_status FROM occurrences o JOIN source_items s ON s.item_ref=o.item_ref WHERE o.exact_form_sha256=? ORDER BY s.part_order,o.token_ordinal,o.occurrence_id").map_err(|e|e.to_string())?;
    let mut cursor = statement.query([target_sha]).map_err(|e| e.to_string())?;
    let (mut occurrences, mut parts, mut pages, mut sections, mut nodes, mut ids) = (
        Vec::new(),
        BTreeMap::<(u64, String), PartCount>::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
    );
    let (mut editorial, mut unsectioned) = (0, 0);
    let mut previous = None;
    while let Some(r) = cursor.next().map_err(|e| e.to_string())? {
        ctx.tick(1)?;
        let get = |i| r.get::<_, String>(i).map_err(|e| e.to_string());
        let number = |i| {
            let n = r.get::<_, i64>(i).map_err(|e| e.to_string())?;
            u64::try_from(n).map_err(|_| String::from("negative source recurrence integer"))
        };
        let occurrence = get(0)?;
        let item = get(1)?;
        let part = number(2)?;
        let file = get(3)?;
        let ordinal = number(4)?;
        let page = get(8)?;
        let section = r.get::<_, Option<String>>(9).map_err(|e| e.to_string())?;
        let path = get(10)?;
        let start = number(11)?;
        let end = number(12)?;
        let status = get(13)?;
        let src = sources.get(&item).ok_or("unknown source recurrence Item")?;
        ensure(
            src.expected["part_order"] == part
                && src.expected["file_sha256"] == file
                && get(5)? == form.0
                && get(6)? == form.1
                && get(7)? == target_sha,
            "source occurrence identity drift",
        )?;
        ensure(
            ids.insert(occurrence.clone()),
            "duplicate source recurrence occurrence",
        )?;
        let key = (part, ordinal, occurrence.clone());
        ensure(
            previous.as_ref().is_none_or(|p| p < &key),
            "source recurrence occurrence order",
        )?;
        previous = Some(key);
        let node = text_node(&src.root, &path)?;
        let returned = unicode_slice(node, start, end)?;
        ensure(
            returned == form.1 && sha(returned.as_bytes()) == target_sha,
            "raw TEI character return drift",
        )?;
        nodes.insert((item.clone(), path.clone()));
        pages.insert((item.clone(), page.clone()));
        let counts = parts.entry((part, item.clone())).or_default();
        counts.count += 1;
        counts.pages.insert(page.clone());
        if let Some(section) = &section {
            sections.insert((item.clone(), section.clone()));
            counts.sections.insert(section.clone());
        } else {
            unsectioned += 1;
        }
        editorial += u64::from(status != "witness-text");
        occurrences.push(json!({"occurrence_id":occurrence,"item_ref":item,"part_order":part,"source_file_sha256":file,"token_ordinal":ordinal,"page_resource_id":page,"section_resource_id":section,"text_node_path":path,"start_offset":start,"end_offset":end,"editorial_status":status,"text_node_sha256":sha(node.as_bytes()),"raw_return_sha256":sha(returned.as_bytes()),"raw_return_verified":true}));
        ensure(
            occurrences.len() <= 1_000_000,
            "source recurrence occurrence bound",
        )?;
    }
    ensure(
        occurrences.len() as i64 == form.5,
        "source recurrence full census does not close",
    )?;
    let tokens = array(&recurrence["segmentation_totals"]["parts"])?;
    let total = u(&recurrence["segmentation_totals"]["token_count"])?;
    let count = occurrences.len() as u64;
    let mut token_total = 0u64;
    let mut divergence = 0u128;
    for token in tokens {
        let t = u(&token["token_count"])?;
        let i = s(&token["item_ref"])?;
        let p = u(&token["part_order"])?;
        ensure(sources.contains_key(i), "unknown recurrence token part")?;
        token_total = token_total
            .checked_add(t)
            .ok_or("part token total overflow")?;
        let c = parts.get(&(p, i.to_owned())).map_or(0, |v| v.count);
        divergence = divergence
            .checked_add((t as u128 * count as u128).abs_diff(c as u128 * total as u128))
            .ok_or("part divergence overflow")?;
    }
    ensure(
        tokens.len() == 4 && token_total == total && total > 0,
        "source recurrence token closure",
    )?;
    let tuple = json!({"occurrence_count":count,"part_range":parts.len(),"section_range":sections.len(),"page_range":pages.len(),"part_dp_millionths":super::recurrence::round_even(divergence,2*total as u128*count as u128,1_000_000)?,"maximum_part_share_millionths":super::recurrence::round_even(parts.values().map(|v|v.count).max().unwrap_or(0)as u128,count as u128,1_000_000)?,"source_editorial_occurrence_count":editorial,"unsectioned_occurrence_count":unsectioned});
    ensure(
        tuple == plan["expected_tracked_recurrence_tuple"],
        "independent raw-witness tuple drift",
    )?;
    let part_values=parts.into_iter().map(|((order,item),p)|json!({"part_order":order,"item_ref":item,"occurrence_count":p.count,"page_count":p.pages.len(),"section_count":p.sections.len()})).collect::<Vec<_>>();
    Ok(
        json!({"schema_version":"tos_semantic_source_recurrence_private_bundle_v1","bundle_id":"semantic-source-recurrence:zarathustra-initial-selected-form-v1","plan_ref":plan_ref,"plan_sha256":plan_sha,"selected_form":{"form_key":form.0,"exact_form":form.1,"normalized_form":form.2,"exact_form_sha256":form.3,"normalized_form_sha256":form.4},"source_database_sha256":database.digest,"source_bindings":bindings,"recurrence_row":row,"recurrence_row_sha256":sha(&canonical(row.clone())?),"observed_tuple":tuple,"parts":part_values,"raw_text_node_return_count":nodes.len(),"raw_offset_return_count":count,"occurrences":occurrences,"authority_boundary":AUTHORITY}),
    )
}
pub struct Options<'a> {
    pub build: bool,
    pub input_root: &'a Path,
    pub output_root: &'a Path,
    pub plan: &'a str,
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
        "generation and event-at must be selected together",
    )?;
    if let Some(g) = opt.generation {
        ensure(valid_generation(g), "generation syntax")?;
    }
    if opt.build {
        ensure(
            !opt.output_root.starts_with(ctx.root())
                && !ctx.root().starts_with(opt.output_root)
                && !opt.output_root.starts_with(opt.input_root)
                && !opt.input_root.starts_with(opt.output_root),
            "source recurrence output must be separate from source and input",
        )?;
    }
    let mut held = Vec::new();
    let schemas = super::schemas(
        ctx,
        &[
            "lexical-index-projection",
            "lexical-recurrence-projection",
            "provenance-event",
        ],
        &mut held,
    )?;
    let plan = read_json_bounded(ctx, opt.plan, META_CAP, &mut held)?;
    let plan_sha = held.last().unwrap().digest.clone();
    ensure(
        plan["schema_version"] == "tos_semantic_source_recurrence_plan_v1"
            && plan["status"] == "frozen-before-output",
        "source recurrence plan profile",
    )?;
    let boundary = plan["authority_boundary"]
        .as_object()
        .ok_or("source recurrence authority object")?;
    ensure(
        boundary.get("source_observation_only") == Some(&Value::Bool(true))
            && boundary.len() == 16
            && boundary
                .iter()
                .all(|(k, v)| v == &(Value::Bool(k == "source_observation_only"))),
        "source recurrence observation authority boundary",
    )?;
    for key in [
        "source_observation_only",
        "packet_stage_change_authorized",
        "accepted_german",
        "morphology",
        "lemma",
        "sense",
        "motif",
        "philosophical_importance",
        "translation",
        "sign_candidate",
        "human_task",
        "semantic_claim",
        "graph_effect",
        "canon_effect",
        "transfer",
        "publication",
    ] {
        ensure(
            boundary.contains_key(key),
            "source recurrence authority field missing",
        )?;
    }
    ensure(
        plan["local_output"]["mode"] == "0600"
            && plan["local_output"]["source_values_local_only"] == true
            && plan["local_output"]["occurrence_positions_local_only"] == true,
        "source recurrence private output boundary",
    )?;
    let selected = &plan["selected_source_observation"];
    ensure(
        selected["selection_reopened"] == false && selected["source_value_tracked"] == false,
        "source recurrence selection boundary",
    )?;
    let ri = &plan["recurrence_input"];
    for (obj, r, h) in [
        (selected, "initial_plan_ref", "initial_plan_sha256"),
        (selected, "packet_ref", "packet_sha256"),
        (ri, "recurrence_plan_ref", "recurrence_plan_sha256"),
    ] {
        digest_bound(ctx, s(&obj[r])?, s(&obj[h])?, &mut held)?;
    }
    let mut projection = |r: &str, h: &str, schema: &str| -> Result<Value> {
        let v = read_json_bounded(ctx, s(&ri[r])?, PACKET_CAP as u64, &mut held)?;
        ensure(
            held.last().unwrap().digest == s(&ri[h])?,
            "source recurrence projection digest drift",
        )?;
        ensure(
            schemas
                .is_valid_value(
                    &format!("https://tree-of-sophia.local/ToS/contracts/{schema}.schema.json"),
                    &v,
                )
                .map_err(|e| format!("source recurrence schema: {e:?}"))?,
            "source recurrence projection schema",
        )?;
        Ok(v)
    };
    let lexical = projection(
        "lexical_projection_ref",
        "lexical_projection_sha256",
        "lexical-index-projection",
    )?;
    let recurrence = projection(
        "recurrence_projection_ref",
        "recurrence_projection_sha256",
        "lexical-recurrence-projection",
    )?;
    let matches = array(&recurrence["rows"])?
        .iter()
        .filter(|v| v["exact_form_sha256"] == selected["exact_form_sha256"])
        .collect::<Vec<_>>();
    ensure(
        matches.len() == 1 && matches[0]["form_key"] == selected["form_key"],
        "source recurrence selected row closure",
    )?;
    let row = matches[0];
    let expected = plan["expected_tracked_recurrence_tuple"]
        .as_object()
        .ok_or("source recurrence tuple required")?;
    ensure(
        expected.len() == 8 && expected.iter().all(|(k, v)| row[k] == *v),
        "source recurrence tracked tuple drift",
    )?;
    let input = ctx.select_directory(opt.input_root)?;
    let mut database = Held::open(&input, s(&ri["local_database_ref"])?, 256 * 1024 * 1024)?;
    ensure(
        database.digest == s(&ri["local_database_sha256"])?,
        "source recurrence database digest drift",
    )?;
    // Source metadata and payloads have separate descriptor roots and custody.
    let mut payloads = Vec::new();
    let mut bundle = reconstruct(
        ctx,
        &input,
        &plan,
        opt.plan,
        &plan_sha,
        &lexical,
        &recurrence,
        row,
        &database,
        &mut held,
        &mut payloads,
    )?;
    let builder = Held::open(ctx, BUILDER, META_CAP)?;
    ensure(
        builder.digest == sha(include_bytes!("semantic_recurrence.rs")),
        "running source recurrence generator source drift",
    )?;
    let builder_sha = builder.digest.clone();
    held.push(builder);
    let suffix = opt
        .generation
        .map(|g| format!(".native-{g}"))
        .unwrap_or_default();
    let event_id = format!("{EVENT_ID}{suffix}");
    let ref_for = |r: &str| -> Result<String> {
        opt.generation
            .map(|g| generated_path(r, g))
            .transpose()
            .map(|v| v.unwrap_or(r.into()))
    };
    let packet_ref = ref_for(s(&plan["local_output"]["ref"])?)?;
    let receipt_ref = ref_for(s(&plan["tracked_outputs"]["receipt_ref"])?)?;
    let event_ref = ref_for(s(&plan["tracked_outputs"]["provenance_ref"])?)?;
    ensure(
        packet_ref != receipt_ref && packet_ref != event_ref && receipt_ref != event_ref,
        "source recurrence output paths must differ",
    )?;
    if opt.generation.is_some() {
        bundle["bundle_id"] = json!(format!("{}{suffix}", s(&bundle["bundle_id"])?));
    }
    let packet = canonical(bundle.clone())?;
    let count = &bundle["raw_offset_return_count"];
    let bindings = array(&bundle["source_bindings"])?;
    let public_bindings = bindings
        .iter()
        .map(|v| {
            let mut v = v.clone();
            v.as_object_mut().unwrap().remove("payload_ref");
            v
        })
        .collect::<Vec<_>>();
    let mut receipt = json!({"schema_version":"tos_semantic_source_recurrence_receipt_v1","receipt_id":format!("semantic-source-recurrence-receipt:zarathustra-initial-selected-form-v1{suffix}"),"status":"completed-source-observation-no-promotion","plan":{"ref":opt.plan,"sha256":plan_sha},"generator":{"ref":BUILDER,"sha256":builder_sha},"selected_source_observation":{"exact_form_sha256":selected["exact_form_sha256"],"form_key":selected["form_key"],"packet_ref":selected["packet_ref"],"packet_sha256":selected["packet_sha256"],"selection_reopened":false,"source_value_tracked":false},"recurrence_sources":{"recurrence_plan_ref":ri["recurrence_plan_ref"],"recurrence_plan_sha256":ri["recurrence_plan_sha256"],"recurrence_projection_ref":ri["recurrence_projection_ref"],"recurrence_projection_sha256":ri["recurrence_projection_sha256"],"recurrence_row_sha256":bundle["recurrence_row_sha256"],"local_database_ref":ri["local_database_ref"],"local_database_sha256":database.digest,"local_database_bytes":database.metadata.len()},"local_bundle":{"ref":packet_ref,"sha256":sha(&packet),"bytes":packet.len(),"mode":"0600","occurrence_count":count,"source_values_local_only":true,"occurrence_positions_local_only":true},"observed_tuple":bundle["observed_tuple"],"parts":bundle["parts"],"source_bindings":public_bindings,"verification":{"complete_occurrence_census":true,"raw_text_node_return_count":bundle["raw_text_node_return_count"],"raw_offset_return_count":count,"raw_offset_return_match_count":count,"source_payload_fixity_match_count":bindings.len(),"tracked_recurrence_tuple_match":true,"independent_part_size_aware_recalculation":true,"rust_xml_no_external_resolution":true},"content_exposure":{"local_exact_strings":true,"local_occurrence_positions":true,"tracked_exact_strings":false,"tracked_occurrence_positions":false,"tracked_form_hashes":true,"dictionary_recovery_possible":true,"confidentiality_claimed":false},"packet_effect":{"packet_ref":selected["packet_ref"],"packet_changed":false,"ladder_stage_changed":false,"human_work_scheduled":false,"promotion_authorized":false},"authority_boundary":plan["authority_boundary"],"provenance_event_ref":event_id});
    if opt.generation.is_none() {
        let old = read_json_bounded(
            ctx,
            s(&plan["tracked_outputs"]["receipt_ref"])?,
            META_CAP,
            &mut held,
        )?;
        ensure(
            opt.plan == PLAN && held.last().unwrap().digest == RECEIPT_SHA,
            "unknown retained source recurrence receipt",
        )?;
        receipt["generator"] = old["generator"].clone();
        receipt["verification"]
            .as_object_mut()
            .unwrap()
            .remove("rust_xml_no_external_resolution");
        receipt["verification"]["xmllint_nonet"] = old["verification"]["xmllint_nonet"].clone();
        ensure(receipt == old, "historical source recurrence receipt drift")?;
    }
    let receipt_bytes = canonical(receipt)?;
    let execution = &plan["execution"];
    let started = opt.event_at.unwrap_or(s(&execution["started_at"])?);
    let ended = opt.event_at.unwrap_or(s(&execution["ended_at"])?);
    let mut event = json!({"schema_version":"tos_provenance_event_v1","event_id":event_id,"event_type":"annotation","started_at":started,"ended_at":ended,"agent_refs":["software:tos-native-rust"],"inputs":[{"ref":opt.plan,"role":"frozen-selected-form-source-recurrence-plan","sha256":plan_sha},{"ref":selected["packet_ref"],"role":"unchanged-observational-semantic-ladder-packet","sha256":selected["packet_sha256"]},{"ref":ri["recurrence_projection_ref"],"role":"tracked-hash-only-four-part-recurrence-projection","sha256":ri["recurrence_projection_sha256"]},{"ref":ri["local_database_ref"],"role":"ignored-local-source-bearing-lexical-database","sha256":database.digest}],"outputs":[{"ref":packet_ref,"role":"ignored-private-complete-raw-witness-recurrence-bundle","sha256":sha(&packet)},{"ref":receipt_ref,"role":"tracked-source-withholding-recurrence-receipt","sha256":sha(&receipt_bytes)}],"method":{"maker_type":"software","name":"complete-exact-form-raw-witness-recurrence-return","version":"1","artifact_digest":builder_sha,"runtime":format!("Rust bounded XML and exact u128 rational arithmetic; SQLite {}",rusqlite::version()),"device":"abyss-machine-cpu","configuration":{"selection_reopened":false,"selected_exact_form_sha256":selected["exact_form_sha256"],"complete_occurrence_census":true,"occurrence_count":count,"source_item_count":array(&bundle["parts"])?.len(),"raw_offset_returns_verified":count,"tracked_source_values":false,"tracked_occurrence_positions":false,"packet_changed":false,"human_work_scheduled":false,"automatic_promotion_authorized":false,"publication_authorized":false},"prompt_or_instruction_ref":opt.plan},"status":"completed_with_warnings","warnings":["the complete exact form and all occurrence positions remain in an ignored mode-0600 local bundle","form hashes are navigational fingerprints and do not provide confidentiality","mechanical recurrence across four source Items does not establish one lemma, sense, motif, sign, or philosophical importance","the existing semantic packet and every language, human, semantic, graph, canon, transfer, and publication gate remain unchanged"],"receipt_refs":[receipt_ref,opt.plan],"rights_basis_ref":null,"event_version":1,"supersedes_event_ref":null});
    if opt.generation.is_none() {
        let old = read_json_bounded(
            ctx,
            s(&plan["tracked_outputs"]["provenance_ref"])?,
            META_CAP,
            &mut held,
        )?;
        ensure(
            held.last().unwrap().digest == EVENT_SHA,
            "unknown retained source recurrence provenance",
        )?;
        event["agent_refs"] = old["agent_refs"].clone();
        for k in ["artifact_digest", "runtime"] {
            event["method"][k] = old["method"][k].clone();
        }
        ensure(
            event == old,
            "historical source recurrence provenance drift",
        )?;
    }
    ensure(
        schemas
            .is_valid_value(
                "https://tree-of-sophia.local/ToS/contracts/provenance-event.schema.json",
                &event,
            )
            .map_err(|e| format!("source recurrence provenance schema: {e:?}"))?,
        "source recurrence provenance schema refused",
    )?;
    let event_bytes = canonical(event)?;
    for raw in [&receipt_bytes, &event_bytes] {
        for key in [
            "exact_form",
            "normalized_form",
            "occurrence_id",
            "text_node_path",
            "token_ordinal",
            "start_offset",
            "end_offset",
        ] {
            let needle = format!("\"{key}\"");
            ensure(
                !raw.windows(needle.len()).any(|w| w == needle.as_bytes()),
                "tracked source recurrence leaks source data",
            )?;
        }
    }
    database.verify(&input)?;
    for h in &mut payloads {
        h.verify(&input)?;
    }
    for h in &mut held {
        h.verify(ctx)?;
    }
    let output = ctx.select_output_directory(opt.output_root, opt.build)?;
    let p = present(&output, &packet_ref, &packet, 0o600)?;
    let (r, e) = if opt.generation.is_some() {
        (
            present(&output, &receipt_ref, &receipt_bytes, 0o644)?,
            present(&output, &event_ref, &event_bytes, 0o644)?,
        )
    } else {
        (true, true)
    };
    if opt.build {
        for (exists, reference, raw, mode) in [
            (p, packet_ref.as_str(), packet.as_slice(), 0o600),
            (r, receipt_ref.as_str(), receipt_bytes.as_slice(), 0o644),
            (e, event_ref.as_str(), event_bytes.as_slice(), 0o644),
        ] {
            if !exists {
                output.write(reference, raw, mode, true)?;
            }
        }
    } else {
        ensure(p && r && e, "source recurrence outputs missing")?;
    }
    ctx.check()?;
    Ok(
        json!({"status":if opt.build{"materialized"}else{"verified"},"packet_ref":packet_ref,"packet_sha256":sha(&packet),"packet_bytes":packet.len(),"receipt_ref":receipt_ref,"receipt_sha256":sha(&receipt_bytes),"provenance_ref":event_ref,"provenance_sha256":sha(&event_bytes),"observed_tuple":bundle["observed_tuple"],"raw_text_node_return_count":bundle["raw_text_node_return_count"],"raw_offset_return_count":count,"budget":ctx.budget_report()}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_character_data_paths_and_unicode_offsets() {
        let root = std::env::current_dir().unwrap();
        let ctx = ResearchExecution::new(&root, 10).unwrap();
        let n = xml_with_doctype(
            &ctx,
            "<TEI><text><p>α<!--c-->β<x/>γ<y/>δ<?pi p?>ε</p><p>z</p></text></TEI>".as_bytes(),
            false,
        )
        .unwrap();
        assert_eq!(text_node(&n, "TEI/text[1]/p[1]/text()[1]").unwrap(), "α");
        assert_eq!(text_node(&n, "TEI/text[1]/p[1]/text()[2]").unwrap(), "β");
        assert_eq!(
            text_node(&n, "TEI/text[1]/p[1]/x[1]/tail()[1]").unwrap(),
            "γ"
        );
        assert_eq!(
            text_node(&n, "TEI/text[1]/p[1]/x[1]/tail()[2]").unwrap(),
            "δ"
        );
        assert_eq!(
            text_node(&n, "TEI/text[1]/p[1]/y[1]/tail()[2]").unwrap(),
            "ε"
        );
        assert_eq!(text_node(&n, "/TEI/text[1]/p[2]/text()[1]").unwrap(), "z");
        assert!(text_node(&n, "TEI/text[0]/text()[1]").is_err());
        assert!(text_node(&n, "TEI/text[1]/p[3]/text()[1]").is_err());
        assert!(text_node(&n, "TEI/text[1]/p[1]").is_err());
        assert_eq!(unicode_slice("α🙂ß", 1, 3).unwrap(), "🙂ß");
        assert!(unicode_slice("α🙂ß", 1, 4).is_err());
        assert!(
            xml_with_doctype(
                &ctx,
                b"<!DOCTYPE TEI [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><TEI>&x;</TEI>",
                false
            )
            .is_err()
        );
    }
}
