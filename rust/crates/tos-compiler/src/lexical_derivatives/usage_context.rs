//! Complete page-bounded KWIC for the frozen exact-form method control.
use super::{
    Held, META_CAP, PACKET_CAP, ResearchExecution, Result, canonical, digest_bound, ensure,
    generated_path, present, read_json_bounded, s, sha, valid_generation,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
const BUILDER: &str = "rust/crates/tos-compiler/src/lexical_derivatives/usage_context.rs";
pub const PLAN: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/lexical-indexes/dta-first-editions-parts-1-4-v1/usage-context-plan.v1.json";
const RETAINED_RECEIPT_SHA: &str =
    "2c0b0cb5cd36bee49a12f9c6c6452383e121b000fa556e99b73bbb9082a76a19";
const RETAINED_EVENT_SHA: &str = "a322010940c12055066cf7a283f6bdc5271b50e30170ad55ec3c33036b2d65fe";
const ROW_FIELDS: [&str; 26] = [
    "schema_version",
    "context_id",
    "question_id",
    "form_key",
    "exact_form_sha256",
    "occurrence_id",
    "item_ref",
    "part_order",
    "source_file_sha256",
    "token_ordinal",
    "page_resource_id",
    "section_resource_id",
    "text_node_path",
    "start_offset",
    "end_offset",
    "editorial_status",
    "target_exact_form",
    "left_exact_tokens",
    "right_exact_tokens",
    "left_token_count",
    "right_token_count",
    "requested_window_each_side",
    "page_start_clipped",
    "page_end_clipped",
    "source_database_sha256",
    "authority",
];
fn u(v: &Value) -> Result<u64> {
    v.as_u64()
        .ok_or("unsigned usage-context integer required".into())
}
fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or("usage-context array required".into())
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or("usage-context count overflow".into())
}
fn validate(schemas: &tos_validation::SchemaBackendProbe, name: &str, value: &Value) -> Result<()> {
    super::validate(schemas, name, value)
}
#[derive(Default)]
struct Part {
    pages: BTreeSet<String>,
    sections: BTreeSet<String>,
    count: u64,
    unsectioned: u64,
}
fn rows(
    ctx: &ResearchExecution,
    file: &std::fs::File,
    plan: &Value,
    database_sha: &str,
    schemas: &tos_validation::SchemaBackendProbe,
) -> Result<(Vec<u8>, Value)> {
    let db = ctx.open_sqlite_readonly_for_ordered_scan(file)?;
    ensure(
        db.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            == "ok",
        "usage lexical quick_check failed",
    )?;
    let db_plan: String = db
        .query_row(
            "SELECT value FROM metadata WHERE key='plan_sha256'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    ensure(
        db_plan == s(&plan["source_lexical_index"]["index_plan_sha256"])?,
        "usage database index-plan digest drift",
    )?;
    let control = &plan["recurrence_control"];
    let digest = s(&control["exact_form_sha256"])?;
    let key = s(&control["form_key"])?;
    let window = u(&plan["context_policy"]["window_tokens_each_side"])?;
    ensure((1..=1024).contains(&window), "usage context window bound")?;
    let (form_key,exact,exact_sha,form_count):(String,String,String,i64)=db.query_row("SELECT form_key,exact_form,exact_form_sha256,occurrence_count FROM forms WHERE exact_form_sha256=?1",[digest],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|e|e.to_string())?;
    ensure(
        form_key == key && exact_sha == digest && sha(exact.as_bytes()) == digest,
        "usage target form digest or key drift",
    )?;
    ensure(
        form_count > 0 && form_count as u64 == u(&control["expected_tuple"]["occurrence_count"])?,
        "usage target occurrence count drift",
    )?;
    let mut targets=db.prepare("SELECT o.occurrence_id,o.item_ref,i.part_order,i.file_sha256,o.token_ordinal,o.form_key,o.exact_form,o.exact_form_sha256,o.page_resource_id,o.section_resource_id,o.text_node_path,o.start_offset,o.end_offset,o.editorial_status FROM occurrences AS o JOIN source_items AS i ON i.item_ref=o.item_ref WHERE o.exact_form_sha256=?1 ORDER BY i.part_order,o.token_ordinal").map_err(|e|e.to_string())?;
    let mut scan = targets.query([digest]).map_err(|e| e.to_string())?;
    let mut context=db.prepare("SELECT token_ordinal,exact_form,occurrence_id FROM occurrences WHERE item_ref=?1 AND page_resource_id=?2 AND token_ordinal BETWEEN ?3 AND ?4 ORDER BY token_ordinal").map_err(|e|e.to_string())?;
    let mut page_lookup = db
        .prepare("SELECT 1 FROM pages WHERE item_ref=?1 AND resource_id=?2")
        .map_err(|e| e.to_string())?;
    let mut section_lookup = db
        .prepare("SELECT 1 FROM sections WHERE item_ref=?1 AND resource_id=?2")
        .map_err(|e| e.to_string())?;
    let mut packet = Vec::new();
    let mut context_ids = BTreeSet::new();
    let mut occurrence_ids = BTreeSet::new();
    let mut pages = BTreeSet::new();
    let mut sections = BTreeSet::new();
    let mut parts: BTreeMap<(u64, String), Part> = BTreeMap::new();
    let (
        mut count,
        mut editorial,
        mut min_left,
        mut max_left,
        mut min_right,
        mut max_right,
        mut clipped_left,
        mut clipped_right,
        mut token_count,
        mut unsectioned,
    ) = (
        0u64,
        0u64,
        u64::MAX,
        0u64,
        u64::MAX,
        0u64,
        0u64,
        0u64,
        0u64,
        0u64,
    );
    let mut prior = None;
    while let Some(row) = scan.next().map_err(|e| e.to_string())? {
        ctx.tick(1)?;
        ensure(count < 1_000_000, "usage row count bound")?;
        let get = |i| row.get::<_, String>(i).map_err(|e| e.to_string());
        let num = |i| -> Result<u64> {
            u64::try_from(row.get::<_, i64>(i).map_err(|e| e.to_string())?)
                .map_err(|_| "negative usage position".into())
        };
        let occurrence = get(0)?;
        let item = get(1)?;
        let part = num(2)?;
        let source_file = get(3)?;
        let ordinal = num(4)?;
        let row_key = get(5)?;
        let target = get(6)?;
        let target_sha = get(7)?;
        let page = get(8)?;
        let section = row.get::<_, Option<String>>(9).map_err(|e| e.to_string())?;
        let text_path = get(10)?;
        let start = num(11)?;
        let finish = num(12)?;
        let editorial_status = get(13)?;
        ensure(
            row_key == key && target_sha == digest && sha(target.as_bytes()) == digest,
            "usage occurrence surface digest drift",
        )?;
        let order = (part, ordinal);
        ensure(
            prior.is_none_or(|p| p < order),
            "strict usage occurrence order",
        )?;
        prior = Some(order);
        let center = i64::try_from(ordinal).map_err(|_| "usage ordinal overflow")?;
        let w = window as i64;
        let high = center
            .checked_add(w)
            .ok_or("usage context window overflow")?;
        let mut near = context
            .query(rusqlite::params![item, page, center - w, high])
            .map_err(|e| e.to_string())?;
        let (mut left, mut right, mut recovered) = (Vec::new(), Vec::new(), 0u64);
        while let Some(token) = near.next().map_err(|e| e.to_string())? {
            ctx.tick(1)?;
            let position: i64 = token.get(0).map_err(|e| e.to_string())?;
            let surface: String = token.get(1).map_err(|e| e.to_string())?;
            let id: String = token.get(2).map_err(|e| e.to_string())?;
            if id == occurrence {
                ensure(
                    position == center && surface == target,
                    "usage target recovery drift",
                )?;
                recovered += 1;
            }
            if position < center {
                left.push(surface);
            } else if position > center {
                right.push(surface);
            }
            ensure(
                left.len() <= window as usize && right.len() <= window as usize,
                "usage context exceeds frozen window",
            )?;
        }
        ensure(
            recovered == 1,
            "usage target not uniquely recoverable on page",
        )?;
        let context_id = format!(
            "usage-context:sha256:{}",
            sha(format!("{}\n{database_sha}\n{occurrence}", s(&plan["plan_id"])?).as_bytes())
        );
        ensure(
            context_ids.insert(context_id.clone()) && occurrence_ids.insert(occurrence.clone()),
            "duplicate usage context identity",
        )?;
        let (l, r) = (left.len() as u64, right.len() as u64);
        let value = json!({"schema_version":"tos_lexical_usage_context_row_v1","context_id":context_id,"question_id":"zarathustra-work-identity-control-context-v1","form_key":row_key,"exact_form_sha256":target_sha,"occurrence_id":occurrence,"item_ref":item,"part_order":part,"source_file_sha256":source_file,"token_ordinal":ordinal,"page_resource_id":page,"section_resource_id":section,"text_node_path":text_path,"start_offset":start,"end_offset":finish,"editorial_status":editorial_status,"target_exact_form":target,"left_exact_tokens":left,"right_exact_tokens":right,"left_token_count":l,"right_token_count":r,"requested_window_each_side":window,"page_start_clipped":l<window,"page_end_clipped":r<window,"source_database_sha256":database_sha,"authority":"unreviewed-source-visible-method-control"});
        validate(schemas, "lexical-usage-context-row", &value)?;
        let raw = canonical(value)?;
        ensure(
            raw.len() <= PACKET_CAP - packet.len(),
            "usage packet byte bound",
        )?;
        packet.extend_from_slice(&raw);
        if pages.insert((item.clone(), page.clone())) {
            let found: i64 = page_lookup
                .query_row(rusqlite::params![item, page], |r| r.get(0))
                .map_err(|e| format!("usage page selector: {e}"))?;
            ensure(found == 1, "usage page selector does not resolve")?;
        }
        let part_summary = parts.entry((part, item.clone())).or_default();
        part_summary.count += 1;
        part_summary.pages.insert(page);
        if let Some(section) = section {
            if sections.insert((item.clone(), section.clone())) {
                let found: i64 = section_lookup
                    .query_row(rusqlite::params![item, section], |r| r.get(0))
                    .map_err(|e| format!("usage section selector: {e}"))?;
                ensure(found == 1, "usage section selector does not resolve")?;
            }
            part_summary.sections.insert(section);
        } else {
            part_summary.unsectioned += 1;
            unsectioned += 1;
        }
        count += 1;
        editorial += u64::from(editorial_status != "witness-text");
        min_left = min_left.min(l);
        max_left = max_left.max(l);
        min_right = min_right.min(r);
        max_right = max_right.max(r);
        clipped_left += u64::from(l < window);
        clipped_right += u64::from(r < window);
        token_count = add(token_count, l + 1 + r)?;
    }
    ensure(
        count > 0 && count == form_count as u64,
        "usage complete occurrence census does not close",
    )?;
    let summary = json!({"row_count":count,"target_occurrence_count":count,"source_item_count":parts.len(),"page_count":pages.len(),"section_count":sections.len(),"unsectioned_occurrence_count":unsectioned,"source_editorial_occurrence_count":editorial,"minimum_left_token_count":min_left,"maximum_left_token_count":max_left,"minimum_right_token_count":min_right,"maximum_right_token_count":max_right,"page_start_clipped_count":clipped_left,"page_end_clipped_count":clipped_right,"total_local_context_token_count":token_count,"semantic_fields_populated":0});
    for (a, b) in [
        ("row_count", "occurrence_count"),
        ("target_occurrence_count", "occurrence_count"),
        ("source_item_count", "part_range"),
        ("page_count", "page_range"),
        ("section_count", "section_range"),
        (
            "unsectioned_occurrence_count",
            "unsectioned_occurrence_count",
        ),
        (
            "source_editorial_occurrence_count",
            "source_editorial_occurrence_count",
        ),
    ] {
        ensure(
            summary[a] == control["expected_tuple"][b],
            &format!("usage {a} drift"),
        )?;
    }
    ensure(
        parts.len() == 4,
        "usage part summary must close over four Items",
    )?;
    let part_values=parts.into_iter().map(|((order,item),p)|json!({"part_order":order,"item_ref":item,"occurrence_count":p.count,"page_count":p.pages.len(),"section_count":p.sections.len(),"unsectioned_occurrence_count":p.unsectioned})).collect::<Vec<_>>();
    Ok((
        packet,
        json!({"summary":summary,"parts":part_values,"identity_closure":{"unique_context_id_count":context_ids.len(),"unique_occurrence_id_count":occurrence_ids.len(),"target_digest_match_count":count,"page_selector_resolution_count":count,"section_selector_resolution_count":count-unsectioned,"source_file_digest_resolution_count":count,"complete_occurrence_census":true}}),
    ))
}
pub struct Options<'a> {
    pub build: bool,
    pub input_root: &'a Path,
    pub output_root: &'a Path,
    pub plan: &'a str,
    pub generation: Option<&'a str>,
    pub event_at: Option<&'a str>,
    pub receipt: Option<&'a str>,
    pub provenance: Option<&'a str>,
}
pub fn run(ctx: &ResearchExecution, options: Options<'_>) -> Result<Value> {
    ensure(
        !options.build || options.generation.is_some(),
        "build requires a new generation",
    )?;
    ensure(
        options.generation.is_some() == options.event_at.is_some(),
        "generation and event-at must be selected together",
    )?;
    ensure(
        options.generation.is_some() || (options.receipt.is_none() && options.provenance.is_none()),
        "retained usage metadata cannot be redirected",
    )?;
    if let Some(g) = options.generation {
        ensure(valid_generation(g), "generation syntax")?;
    }
    if options.build {
        ensure(
            !options.output_root.starts_with(ctx.root())
                && !ctx.root().starts_with(options.output_root)
                && !options.output_root.starts_with(options.input_root)
                && !options.input_root.starts_with(options.output_root),
            "usage output must be separate from source and input roots",
        )?;
    }
    let mut held = Vec::new();
    let schemas = super::schemas(
        ctx,
        &[
            "lexical-usage-context-plan",
            "lexical-usage-context-row",
            "lexical-usage-context-receipt",
            "provenance-event",
        ],
        &mut held,
    )?;
    let plan = read_json_bounded(ctx, options.plan, META_CAP, &mut held)?;
    let plan_sha = held.last().unwrap().digest.clone();
    validate(&schemas, "lexical-usage-context-plan", &plan)?;
    let source = &plan["source_lexical_index"];
    let control = &plan["recurrence_control"];
    for (obj, r, h) in [
        (source, "index_plan_ref", "index_plan_sha256"),
        (
            source,
            "tracked_projection_ref",
            "tracked_projection_sha256",
        ),
        (control, "plan_ref", "plan_sha256"),
    ] {
        digest_bound(ctx, s(&obj[r])?, s(&obj[h])?, &mut held)?;
    }
    let recurrence = read_json_bounded(
        ctx,
        s(&control["projection_ref"])?,
        PACKET_CAP as u64,
        &mut held,
    )?;
    ensure(
        held.last().unwrap().digest == s(&control["projection_sha256"])?,
        "usage recurrence projection digest drift",
    )?;
    let matches = array(&recurrence["rows"])?
        .iter()
        .filter(|r| r["exact_form_sha256"] == control["exact_form_sha256"])
        .collect::<Vec<_>>();
    ensure(
        matches.len() == 1,
        "usage recurrence control must resolve once",
    )?;
    for (k, v) in control["expected_tuple"]
        .as_object()
        .ok_or("usage expected tuple object required")?
    {
        ensure(matches[0][k] == *v, &format!("usage recurrence {k} drift"))?;
    }
    ensure(
        matches[0]["form_key"] == control["form_key"],
        "usage recurrence form key drift",
    )?;
    let input = ctx.select_directory(options.input_root)?;
    let mut database = Held::open(
        &input,
        s(&source["local_database_relative_path"])?,
        256 * 1024 * 1024,
    )?;
    ensure(
        database.digest == s(&source["local_database_sha256"])?
            && database.metadata.len() == u(&source["local_database_bytes"])?,
        "usage database fixity drift",
    )?;
    let (packet, aggregate) = rows(&input, &database.file, &plan, &database.digest, &schemas)?;
    let research = Held::open(ctx, s(&plan["research_ref"])?, META_CAP)?;
    let research_sha = research.digest.clone();
    held.push(research);
    let builder = Held::open(ctx, BUILDER, META_CAP)?;
    ensure(
        builder.digest == sha(include_bytes!("usage_context.rs")),
        "running usage generator source drift",
    )?;
    let builder_sha = builder.digest.clone();
    held.push(builder);
    let suffix = options
        .generation
        .map(|g| format!(".native-{g}"))
        .unwrap_or_default();
    let event_id = format!("{}{suffix}", s(&plan["provenance_event_ref"])?);
    let ref_for = |r: &str| -> Result<String> {
        options
            .generation
            .map(|g| generated_path(r, g))
            .transpose()
            .map(|v| v.unwrap_or(r.into()))
    };
    let local = &plan["local_bundle"];
    let packet_ref = ref_for(s(&local["relative_path"])?)?;
    let receipt_ref = if let Some(r) = options.receipt {
        r.to_owned()
    } else {
        ref_for(s(&plan["tracked_receipt_ref"])?)?
    };
    let event_ref = if let Some(r) = options.provenance {
        r.to_owned()
    } else {
        ref_for(s(&plan["provenance_ref"])?)?
    };
    ensure(
        packet_ref != receipt_ref && packet_ref != event_ref && receipt_ref != event_ref,
        "usage output paths must differ",
    )?;
    let policy = &plan["context_policy"];
    let mut receipt = json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/lexical-usage-context-receipt.schema.json","schema_version":"tos_lexical_usage_context_receipt_v1","generated_or_authored":"generated_from_local_lexical_projection","receipt_id":format!("lexical-usage-context-receipt:zarathustra-work-identity-control-v1{suffix}"),"plan":{"ref":options.plan,"sha256":plan_sha},"generator":{"ref":BUILDER,"sha256":builder_sha},"source_database":{"relative_path":source["local_database_relative_path"],"sha256":database.digest,"bytes":database.metadata.len(),"quick_check":"ok"},
        "source_projections":{"index_plan":{"ref":source["index_plan_ref"],"sha256":source["index_plan_sha256"]},"lexical_projection":{"ref":source["tracked_projection_ref"],"sha256":source["tracked_projection_sha256"]},"recurrence_plan":{"ref":control["plan_ref"],"sha256":control["plan_sha256"]},"recurrence_projection":{"ref":control["projection_ref"],"sha256":control["projection_sha256"]}},
        "recurrence_control":{"form_key":control["form_key"],"exact_form_sha256":control["exact_form_sha256"],"selection_basis":control["selection_basis"],"observed_tuple":control["expected_tuple"]},"local_bundle":{"relative_path":packet_ref,"format":local["format"],"schema_ref":local["schema_ref"],"schema_version":local["schema_version"],"sha256":sha(&packet),"bytes":packet.len(),"mode":local["mode"],"row_count":aggregate["summary"]["row_count"],"required_fields":ROW_FIELDS},
        "context_policy":{"policy_id":policy["policy_id"],"window_tokens_each_side":policy["window_tokens_each_side"],"boundary":policy["boundary"],"sampling":policy["sampling"],"row_order":policy["row_order"],"sentence_boundary_claimed":policy["sentence_boundary_claimed"]},"summary":aggregate["summary"],"parts":aggregate["parts"],"identity_closure":aggregate["identity_closure"],"content_exposure":plan["content_exposure"],"rights_and_visibility":plan["rights_and_visibility"],"semantic_boundary":plan["semantic_boundary"],"provenance_event_ref":event_id,"authority_boundary":"This bundle materializes complete private exact-form usage context for one preselected method control and records the associated source-withholding receipt."});
    if options.generation.is_none() {
        let prior = read_json_bounded(ctx, s(&plan["tracked_receipt_ref"])?, META_CAP, &mut held)?;
        ensure(
            options.plan == PLAN && held.last().unwrap().digest == RETAINED_RECEIPT_SHA,
            "unknown retained usage receipt",
        )?;
        for k in ["generator", "authority_boundary"] {
            receipt[k] = prior[k].clone();
        }
        ensure(receipt == prior, "historical usage receipt drift")?;
    }
    validate(&schemas, "lexical-usage-context-receipt", &receipt)?;
    let receipt_bytes = canonical(receipt)?;
    let timestamp = options.event_at.unwrap_or("2026-08-02T14:50:00Z");
    let mut event = json!({"schema_version":"tos_provenance_event_v1","event_id":event_id,"event_type":"export","started_at":timestamp,"ended_at":timestamp,"agent_refs":["software:tos-native-rust",format!("software:sqlite-{}",rusqlite::version())],
        "inputs":[{"ref":options.plan,"role":"frozen-question-scoped-usage-context-plan","sha256":plan_sha},{"ref":source["local_database_relative_path"],"role":"private-fixity-bound-source-bearing-lexical-database","sha256":database.digest},{"ref":source["tracked_projection_ref"],"role":"tracked-hash-count-and-resource-lexical-projection","sha256":source["tracked_projection_sha256"]},{"ref":control["projection_ref"],"role":"tracked-hash-only-recurrence-control-projection","sha256":control["projection_sha256"]},{"ref":plan["research_ref"],"role":"ordered-usage-context-method-research","sha256":research_sha}],
        "outputs":[{"ref":packet_ref,"role":"private-source-bearing-complete-usage-context-bundle","sha256":sha(&packet)},{"ref":receipt_ref,"role":"tracked-source-withholding-usage-context-receipt","sha256":sha(&receipt_bytes)}],
        "method":{"maker_type":"software","name":"page-bounded-exact-kwic-method-control","version":"1","artifact_digest":builder_sha,"runtime":format!("Rust with SQLite {}",rusqlite::version()),"device":"CPU","configuration":{"selection":"complete-preselected-exact-form-occurrence-census","target_occurrences":aggregate["summary"]["target_occurrence_count"],"window_tokens_each_side":policy["window_tokens_each_side"],"boundary":"same-item-and-page-only","source_strings_local_only":true,"tracked_source_strings":false,"sentence_boundary_claimed":false,"future_challengers_scheduled":false,"human_work_scheduled":false},"prompt_or_instruction_ref":plan["research_ref"]},
        "status":"completed_with_warnings","warnings":["the local bundle contains exact sequential source context and remains ignored mode-0600 local-only material","A fixed page-bounded token window supplies the concordance baseline; sentence and sense boundaries require their own source-visible assessment.","The preselected identity control anchors the comparison; recurrence ranking, sign candidacy and linguistic identity require their own assessment.","The tracked receipt carries text-free identities, fixity and provenance; exact source context remains in the private local bundle.","future public use requires independently reacquired publication material and a fresh rights and operator approval gate"],"receipt_refs":[receipt_ref,options.plan,plan["research_ref"]],"rights_basis_ref":null,"event_version":1,"supersedes_event_ref":null});
    if options.generation.is_none() {
        let prior = read_json_bounded(ctx, s(&plan["provenance_ref"])?, META_CAP, &mut held)?;
        ensure(
            held.last().unwrap().digest == RETAINED_EVENT_SHA,
            "unknown retained usage provenance",
        )?;
        for k in ["agent_refs", "warnings"] {
            event[k] = prior[k].clone();
        }
        for k in ["artifact_digest", "runtime"] {
            event["method"][k] = prior["method"][k].clone();
        }
        ensure(event == prior, "historical usage provenance drift")?;
    }
    validate(&schemas, "provenance-event", &event)?;
    let event_bytes = canonical(event)?;
    database.verify(&input)?;
    for h in &mut held {
        h.verify(ctx)?;
    }
    let output = ctx.select_output_directory(options.output_root, options.build)?;
    let packet_present = present(&output, &packet_ref, &packet, 0o600)?;
    let (receipt_present, event_present) = if options.generation.is_some() {
        (
            present(&output, &receipt_ref, &receipt_bytes, 0o644)?,
            present(&output, &event_ref, &event_bytes, 0o644)?,
        )
    } else {
        (true, true)
    };
    if options.build {
        if !packet_present {
            output.write(&packet_ref, &packet, 0o600, true)?;
        }
        if !receipt_present {
            output.write(&receipt_ref, &receipt_bytes, 0o644, true)?;
        }
        if !event_present {
            output.write(&event_ref, &event_bytes, 0o644, true)?;
        }
    } else {
        ensure(
            packet_present && receipt_present && event_present,
            "usage output missing",
        )?;
    }
    ctx.check()?;
    Ok(
        json!({"status":if options.build{"materialized"}else{"verified"},"packet_ref":packet_ref,"packet_sha256":sha(&packet),"packet_bytes":packet.len(),"receipt_ref":receipt_ref,"receipt_sha256":sha(&receipt_bytes),"provenance_ref":event_ref,"provenance_sha256":sha(&event_bytes),"summary":aggregate["summary"],"identity_closure":aggregate["identity_closure"],"budget":ctx.budget_report()}),
    )
}
