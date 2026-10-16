//! Output-blind first/median/last contexts from exact raw TEI and retained A.
use super::semantic_recurrence::{step, text_node, unicode_slice};
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
pub const PLAN: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/morphology-contextual-episode.selected-form-b.v1.json";
const BUILDER: &str = "rust/crates/tos-compiler/src/lexical_derivatives/morphology_context.rs";
const RECEIPT_SHA: &str = "0baff212b114ae9d0781ad0e26cf93a25066ad42f371252dc8106d89440d936f";
const EVENT_SHA: &str = "b1d7e32b50d2d542807db610afb899fa503c718fa598135580e0362b97aff13c";
const EVENT_ID: &str = "tos.event.annotation.zarathustra-morphology-context-b-v1.2026-08-10";
const AUTHORITY: &str = "This output-blind private raw-TEI context packet supports the selected machine disambiguation proposal B.";
const A_EVENT_REF: &str = "owner-local-artifacts/tree-of-sophia-foundation-lab/tos-historical-german-morphology-v1/zarathustra-dwdsmor-a-20260730t0009z/variant-A/raw-output/dwdsmor-census.jsonl";
fn u(v: &Value) -> Result<u64> {
    v.as_u64()
        .ok_or("unsigned contextual morphology integer required".into())
}
fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array()
        .ok_or("contextual morphology array required".into())
}
fn valid(schemas: &tos_validation::SchemaBackendProbe, name: &str, value: &Value) -> Result<()> {
    ensure(
        schemas
            .is_valid_value(
                &format!("https://tree-of-sophia.local/ToS/contracts/{name}.schema.json"),
                value,
            )
            .map_err(|e| format!("contextual morphology schema: {e:?}"))?,
        &format!("{name} schema refused"),
    )
}
fn element<'a>(root: &'a Node, path: &str) -> Result<&'a Node> {
    ensure(path.len() <= 16384, "context element path bound")?;
    let mut steps = path.split('/');
    let (name, index) = step(steps.next().ok_or("context element root")?)?;
    ensure(
        name == root.name && index == 1,
        "context element root mismatch",
    )?;
    let mut node = root;
    for raw in steps {
        let (name, index) = step(raw)?;
        ensure(
            name != "text()" && name != "tail()",
            "context element step required",
        )?;
        node = node
            .children(name)
            .get(index - 1)
            .copied()
            .ok_or("context element selector missing")?;
    }
    Ok(node)
}
fn context_unit(path: &str) -> Result<(String, &'static str)> {
    let parts = path.split('/').collect::<Vec<_>>();
    for (name, kind) in [("p", "paragraph"), ("lg", "verse-group")] {
        for i in (0..parts.len()).rev() {
            if step(parts[i])?.0 == name {
                return Ok((parts[..=i].join("/"), kind));
            }
        }
    }
    Err("selected occurrence lacks paragraph/verse context".into())
}
fn flatten(ctx: &ResearchExecution, root: &Node, target: &str) -> Result<(String, u64)> {
    fn walk(
        ctx: &ResearchExecution,
        n: &Node,
        target: &str,
        text: &mut String,
        codepoints: &mut u64,
        base: &mut Option<u64>,
    ) -> Result<()> {
        for p in &n.content {
            ctx.tick(1)?;
            match p {
                Part::Child(n) => walk(ctx, n, target, text, codepoints, base)?,
                Part::Text(t) => {
                    if std::ptr::eq(t.as_str(), target) {
                        ensure(base.is_none(), "context target occurs twice")?;
                        *base = Some(*codepoints);
                    }
                    *codepoints = codepoints
                        .checked_add(t.chars().count() as u64)
                        .ok_or("context codepoint overflow")?;
                    ensure(
                        t.len() <= PACKET_CAP - text.len(),
                        "context text byte bound",
                    )?;
                    text.push_str(t);
                }
            }
        }
        Ok(())
    }
    let (mut text, mut count, mut base) = (String::new(), 0, None);
    walk(ctx, root, target, &mut text, &mut count, &mut base)?;
    Ok((text, base.ok_or("target outside selected context")?))
}
fn inspect_a(ctx: &ResearchExecution, held: &mut Held, trigger: &Value) -> Result<Value> {
    let expected = &trigger["private_raw_output"];
    ensure(
        held.digest == s(&expected["sha256"])? && held.metadata.len() == u(&expected["bytes"])?,
        "private A stream fixity drift",
    )?;
    let mut selected = None;
    let mut pending = Vec::new();
    let mut remaining = held.metadata.len();
    let mut buffer = [0u8; 65536];
    let mut rows = 0u64;
    let mut line = |raw: &[u8]| -> Result<()> {
        ctx.tick(1)?;
        if raw.iter().all(u8::is_ascii_whitespace) {
            return Ok(());
        }
        rows += 1;
        ensure(rows <= 1_000_000, "A stream row bound")?;
        let row = crate::zarathustra_lexical::parse(raw, META_CAP as usize)?;
        ensure(row.is_object(), "A stream row object required")?;
        if row["exact_form_sha256"] == trigger["exact_form_sha256"] {
            ensure(selected.is_none(), "selected A row must resolve once")?;
            ensure(
                sha(raw) == s(&trigger["selected_row_sha256"])?,
                "selected A row digest drift",
            )?;
            selected = Some(row);
        }
        Ok(())
    };
    while remaining > 0 {
        let n = remaining.min(buffer.len() as u64) as usize;
        ctx.read_exact(&mut held.file, &mut buffer[..n])?;
        remaining -= n as u64;
        for part in buffer[..n].split_inclusive(|b| *b == b'\n') {
            ensure(
                part.len() <= META_CAP as usize - pending.len(),
                "A stream line bound",
            )?;
            pending.extend_from_slice(part);
            if pending.last() == Some(&b'\n') {
                line(&pending)?;
                pending.clear();
            }
        }
    }
    if !pending.is_empty() {
        line(&pending)?;
    }
    let row = selected.ok_or("selected A row missing")?;
    ensure(
        row["form_key"] == trigger["form_key"]
            && row["normalized_form_sha256"] == trigger["normalized_form_sha256"]
            && row["input_preserved"] == true
            && row["unknown"] == false,
        "selected A identity/preservation drift",
    )?;
    let analyses = array(&row["lemma_analyses"])?;
    let categories = analyses
        .iter()
        .map(|v| s(&v["pos"]).map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?
        .into_iter()
        .collect::<Vec<_>>();
    let separable = analyses.iter().filter(|v| v["syninfo"] == "SEP").count();
    ensure(
        analyses.len() as u64 == u(&trigger["lemma_analysis_count"])?
            && json!(categories) == trigger["provider_pos_categories"]
            && separable as u64 == u(&trigger["separable_candidate_count"])?,
        "selected A ambiguity shape drift",
    )?;
    Ok(
        json!({"result_receipt":trigger["result_receipt"],"private_raw_output_sha256":expected["sha256"],"selected_row_sha256":trigger["selected_row_sha256"],"selected_row_match_count":1,"input_preserved":true,"lemma_analysis_count":analyses.len(),"provider_pos_categories":categories,"separable_candidate_count":separable}),
    )
}
fn contexts(
    input: &ResearchExecution,
    plan: &Value,
    schemas: &tos_validation::SchemaBackendProbe,
    held: &mut Vec<Held>,
) -> Result<(Vec<Value>, Value, Vec<Value>)> {
    let recurrence = &plan["source_recurrence"];
    let local = &recurrence["local_bundle"];
    let bundle = read_json_bounded(input, s(&local["ref"])?, META_CAP, held)?;
    let pinned = held.last().unwrap();
    use std::os::unix::fs::PermissionsExt;
    ensure(
        pinned.digest == s(&local["sha256"])?
            && pinned.metadata.len() == u(&local["bytes"])?
            && pinned.metadata.permissions().mode() & 0o777 == 0o600,
        "private recurrence bundle fixity/mode drift",
    )?;
    let trigger = &plan["a_trigger"];
    for k in ["form_key", "exact_form_sha256", "normalized_form_sha256"] {
        ensure(
            bundle["selected_form"][k] == trigger[k],
            "context recurrence selected-form identity drift",
        )?;
    }
    let occurrences = array(&bundle["occurrences"])?;
    ensure(
        occurrences.len() as u64 == u(&recurrence["occurrence_count"])?,
        "context recurrence count drift",
    )?;
    let mut previous = None;
    for o in occurrences {
        input.tick(1)?;
        let key = (
            u(&o["part_order"])?,
            u(&o["token_ordinal"])?,
            s(&o["occurrence_id"])?,
        );
        ensure(
            previous.is_none_or(|p| p < key),
            "context recurrence order must be strict",
        )?;
        previous = Some(key);
    }
    let mut bindings = BTreeMap::new();
    for v in array(&bundle["source_bindings"])? {
        ensure(
            bindings.insert(s(&v["item_ref"])?, v).is_none(),
            "context duplicate source binding",
        )?;
    }
    let mut roots = BTreeMap::new();
    let mut inputs = Vec::new();
    let mut rows = Vec::new();
    let mut ids = BTreeSet::new();
    let ranks = array(&plan["selection"]["recurrence_ranks"])?;
    for rank in ranks {
        let rank = u(rank)?;
        let role = match rank {
            1 => "first",
            73 => "inclusive-median",
            145 => "last",
            _ => return Err("unsupported output-blind rank".into()),
        };
        let o = occurrences
            .get(rank as usize - 1)
            .ok_or("context rank outside census")?;
        let item = s(&o["item_ref"])?;
        let b = *bindings.get(item).ok_or("context source binding missing")?;
        ensure(
            b["payload_sha256"] == o["source_file_sha256"] && b["part_order"] == o["part_order"],
            "context occurrence source binding drift",
        )?;
        if !roots.contains_key(item) {
            let mut h = Held::open(input, s(&b["payload_ref"])?, META_CAP)?;
            ensure(
                h.digest == s(&b["payload_sha256"])?,
                "context source payload digest drift",
            )?;
            let raw = input.read_file(&mut h.file, META_CAP)?;
            let root = xml_with_doctype(input, &raw, false)?;
            roots.insert(item.to_owned(), root);
            inputs.push(json!({"ref":b["payload_ref"],"sha256":b["payload_sha256"]}));
            held.push(h);
        }
        let root = &roots[item];
        let path = s(&o["text_node_path"])?;
        let last = path.rsplit('/').next().ok_or("context target step")?;
        let (kind, index) = step(last)?;
        ensure(
            (kind == "text()" || kind == "tail()") && index == 1,
            "context target requires first text/tail node",
        )?;
        let node = text_node(root, path)?;
        let start = u(&o["start_offset"])?;
        let end = u(&o["end_offset"])?;
        let target = unicode_slice(node, start, end)?;
        ensure(
            sha(target.as_bytes()) == s(&trigger["exact_form_sha256"])?,
            "context raw TEI target drift",
        )?;
        let (context_path, context_kind) = context_unit(path)?;
        let (context_text, base) = flatten(input, element(root, &context_path)?, node)?;
        let target_start = base
            .checked_add(start)
            .ok_or("context target offset overflow")?;
        let target_end = base
            .checked_add(end)
            .ok_or("context target offset overflow")?;
        ensure(
            unicode_slice(&context_text, target_start, target_end)? == target,
            "context target offset return drift",
        )?;
        let digest = sha(context_text.as_bytes());
        let id = format!(
            "morphology-context:sha256:{}",
            sha(format!(
                "{}\n{}\n{digest}",
                s(&plan["episode_id"])?,
                s(&o["occurrence_id"])?
            )
            .as_bytes())
        );
        ensure(ids.insert(id.clone()), "context IDs must be unique")?;
        let row = json!({"schema_version":"tos_morphology_contextual_episode_row_v1","episode_id":plan["episode_id"],"context_id":id,"selection_rank":rank,"selection_role":role,"form_key":trigger["form_key"],"exact_form_sha256":trigger["exact_form_sha256"],"occurrence_id":o["occurrence_id"],"item_ref":item,"part_order":o["part_order"],"source_file_sha256":o["source_file_sha256"],"page_resource_id":o["page_resource_id"],"section_resource_id":o["section_resource_id"],"text_node_path":path,"source_node_start_offset":start,"source_node_end_offset":end,"context_unit_kind":context_kind,"context_node_path":context_path,"context_text":context_text,"context_sha256":digest,"target_start_offset":target_start,"target_end_offset":target_end,"target_exact_form":target,"target_return_verified":true,"sentence_boundary_claimed":false,"exact_surface_mutated":false,"input_variant":"unchanged-historical-context","authority":"unreviewed-source-visible-context-for-machine-proposal-only"});
        valid(schemas, "morphology-contextual-episode-row", &row)?;
        rows.push(row);
    }
    inputs.sort_by(|a, b| a["ref"].as_str().cmp(&b["ref"].as_str()));
    let mut kinds = BTreeMap::<String, u64>::new();
    let mut sizes = Vec::new();
    for row in &rows {
        *kinds
            .entry(s(&row["context_unit_kind"])?.into())
            .or_default() += 1;
        sizes.push(s(&row["context_text"])?.chars().count() as u64);
    }
    let summary = json!({"target_return_verified_count":rows.len(),"context_unit_kind_counts":kinds,"total_context_codepoints":sizes.iter().sum::<u64>(),"minimum_context_codepoints":sizes.iter().min(),"maximum_context_codepoints":sizes.iter().max(),"sentence_boundary_claimed":false,"exact_surface_mutated":false});
    Ok((rows, summary, inputs))
}
pub struct Options<'a> {
    pub build: bool,
    pub input_root: &'a Path,
    pub output_root: &'a Path,
    pub a_raw_output: &'a Path,
    pub plan: &'a str,
    pub generation: Option<&'a str>,
    pub event_at: Option<&'a str>,
    pub receipt: Option<&'a str>,
    pub provenance: Option<&'a str>,
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
    ensure(
        opt.generation.is_some() || (opt.receipt.is_none() && opt.provenance.is_none()),
        "retained context metadata cannot be redirected",
    )?;
    if let Some(g) = opt.generation {
        ensure(valid_generation(g), "generation syntax")?;
    }
    let a_parent = opt
        .a_raw_output
        .parent()
        .ok_or("explicit A output parent required")?;
    let a_name = opt
        .a_raw_output
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("explicit A output filename required")?;
    if opt.build {
        for root in [ctx.root(), opt.input_root, a_parent] {
            ensure(
                !opt.output_root.starts_with(root) && !root.starts_with(opt.output_root),
                "context output must be separate from source/private inputs",
            )?;
        }
    }
    let mut held = Vec::new();
    let schemas = super::schemas(
        ctx,
        &[
            "morphology-contextual-episode-plan",
            "morphology-contextual-episode-row",
            "morphology-contextual-episode-receipt",
            "provenance-event",
        ],
        &mut held,
    )?;
    let plan = read_json_bounded(ctx, opt.plan, META_CAP, &mut held)?;
    let plan_sha = held.last().unwrap().digest.clone();
    valid(&schemas, "morphology-contextual-episode-plan", &plan)?;
    let recurrence = &plan["source_recurrence"];
    let trigger = &plan["a_trigger"];
    for binding in [
        &plan["research"],
        &plan["parent_morphology_plan"],
        &recurrence["plan"],
        &recurrence["receipt"],
        &trigger["result_receipt"],
    ] {
        digest_bound(ctx, s(&binding["ref"])?, s(&binding["sha256"])?, &mut held)?;
    }
    let a_ctx = ctx.select_directory(a_parent)?;
    let mut a = Held::open(&a_ctx, a_name, 128 * 1024 * 1024)?;
    let closure = inspect_a(&a_ctx, &mut a, trigger)?;
    let input = ctx.select_directory(opt.input_root)?;
    let mut inputs_held = Vec::new();
    let (rows, summary, source_inputs) = contexts(&input, &plan, &schemas, &mut inputs_held)?;
    let mut packet = Vec::new();
    for row in &rows {
        let raw = canonical(row.clone())?;
        ensure(
            raw.len() <= PACKET_CAP - packet.len(),
            "context packet byte bound",
        )?;
        packet.extend_from_slice(&raw);
    }
    let builder = Held::open(ctx, BUILDER, META_CAP)?;
    ensure(
        builder.digest == sha(include_bytes!("morphology_context.rs")),
        "running morphology context generator source drift",
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
    let packet_ref = ref_for(s(&plan["local_packet"]["relative_path"])?)?;
    let receipt_ref = opt
        .receipt
        .map(str::to_owned)
        .map_or_else(|| ref_for(s(&plan["tracked_receipt_ref"])?), Ok)?;
    let event_ref = opt
        .provenance
        .map(str::to_owned)
        .map_or_else(|| ref_for(s(&plan["provenance_ref"])?), Ok)?;
    ensure(
        packet_ref != receipt_ref && packet_ref != event_ref && receipt_ref != event_ref,
        "context output paths must differ",
    )?;
    let mut part_counts = BTreeMap::<String, u64>::new();
    for row in &rows {
        *part_counts
            .entry(u(&row["part_order"])?.to_string())
            .or_default() += 1;
    }
    let mut receipt = json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/morphology-contextual-episode-receipt.schema.json","schema_version":"tos_morphology_contextual_episode_receipt_v1","receipt_id":format!("morphology-contextual-episode-receipt:zarathustra-selected-form-b-v1{suffix}"),"status":"context-packet-materialized-b-unacquired","plan":{"ref":opt.plan,"sha256":plan_sha},"generator":{"ref":BUILDER,"sha256":builder_sha},"trigger_closure":closure,"source_recurrence":{"plan":recurrence["plan"],"receipt":recurrence["receipt"],"local_bundle_sha256":recurrence["local_bundle"]["sha256"],"complete_occurrence_count":recurrence["occurrence_count"],"source_payload_fixity_match_count":source_inputs.len()},"selection":{"method":plan["selection"]["method"],"recurrence_ranks":plan["selection"]["recurrence_ranks"],"rank_roles":plan["selection"]["rank_roles"],"row_count":rows.len(),"part_counts":part_counts,"b_output_visible_during_selection":false,"semantic_labels_used":false},"local_packet":{"relative_path":packet_ref,"format":"jsonl","schema_ref":plan["local_packet"]["schema_ref"],"schema_version":plan["local_packet"]["schema_version"],"sha256":sha(&packet),"bytes":packet.len(),"mode":"0600","row_count":rows.len(),"source_bearing":true},"context_summary":summary,"variant_state":{"a":"existing-context-free-candidate-set","b":"admitted-unacquired","c":"blocked-question-inapplicable","b_acquisition_requires_artifact_audit":true,"b_execution_requires_fresh_host_preflight":true,"human_work_scheduled":false},"content_exposure":{"local_exact_strings":true,"local_context":true,"local_occurrence_positions":true,"tracked_exact_strings":false,"tracked_context":false,"tracked_occurrence_positions":false,"tracked_form_hashes":true},"rights_and_visibility":plan["rights_and_visibility"],"competence_boundary":plan["competence_boundary"],"semantic_boundary":plan["semantic_boundary"],"provenance_event_ref":event_id,"authority_boundary":AUTHORITY});
    if opt.generation.is_none() {
        let old = read_json_bounded(ctx, s(&plan["tracked_receipt_ref"])?, META_CAP, &mut held)?;
        ensure(
            opt.plan == PLAN && held.last().unwrap().digest == RECEIPT_SHA,
            "unknown retained morphology context receipt",
        )?;
        receipt["generator"] = old["generator"].clone();
        receipt["authority_boundary"] = old["authority_boundary"].clone();
        ensure(
            receipt == old,
            "historical morphology context receipt drift",
        )?;
    }
    valid(&schemas, "morphology-contextual-episode-receipt", &receipt)?;
    let receipt_bytes = canonical(receipt)?;
    let mut event_inputs = vec![
        json!({"ref":opt.plan,"role":"frozen-output-blind-contextual-morphology-plan","sha256":plan_sha}),
        json!({"ref":trigger["result_receipt"]["ref"],"role":"tracked-source-free-direct-a-census-result","sha256":trigger["result_receipt"]["sha256"]}),
        json!({"ref":A_EVENT_REF,"role":"private-direct-a-provider-stream","sha256":trigger["private_raw_output"]["sha256"]}),
        json!({"ref":recurrence["plan"]["ref"],"role":"tracked-selected-form-recurrence-plan","sha256":recurrence["plan"]["sha256"]}),
        json!({"ref":recurrence["receipt"]["ref"],"role":"tracked-complete-recurrence-receipt","sha256":recurrence["receipt"]["sha256"]}),
        json!({"ref":recurrence["local_bundle"]["ref"],"role":"private-complete-raw-witness-recurrence-bundle","sha256":recurrence["local_bundle"]["sha256"]}),
        json!({"ref":plan["research"]["ref"],"role":"ordered-and-refreshed-historical-german-morphology-research","sha256":plan["research"]["sha256"]}),
    ];
    for v in &source_inputs {
        event_inputs.push(
            json!({"ref":v["ref"],"role":"selected-fixity-bound-raw-tei","sha256":v["sha256"]}),
        );
    }
    let at = opt.event_at.unwrap_or("2026-08-10T22:00:00Z");
    let mut event = json!({"schema_version":"tos_provenance_event_v1","event_id":event_id,"event_type":"annotation","started_at":at,"ended_at":at,"agent_refs":["software:tos-native-rust"],"inputs":event_inputs,"outputs":[{"ref":packet_ref,"role":"private-output-blind-raw-tei-context-packet","sha256":sha(&packet)},{"ref":receipt_ref,"role":"tracked-text-and-position-free-context-receipt","sha256":sha(&receipt_bytes)}],"method":{"maker_type":"software","name":"raw-tei-first-median-last-morphology-context-freeze","version":"1","artifact_digest":builder_sha,"runtime":"Rust bounded XML character data and streaming exact A record validation","device":"CPU","configuration":{"selection":plan["selection"]["method"],"recurrence_ranks":plan["selection"]["recurrence_ranks"],"context_units":plan["context_policy"]["unit_rules"],"network_allowed":false,"b_output_visible_during_selection":false,"c_admitted_for_question":false,"human_work_scheduled":false},"prompt_or_instruction_ref":plan["research"]["ref"]},"status":"completed_with_warnings","warnings":["exact context and occurrence positions remain ignored mode-0600 local-only material","paragraph and verse-group boundaries are transparent TEI context units, not accepted sentence or sense boundaries","the packet admits only a machine B proposal after separate artifact and host gates","C remains blocked because normalization is not decision-relevant to this episode","This event records the morphology context packet. German competence, morphology and lemma assessment, semantics, publication and human work retain their recorded states."],"receipt_refs":[receipt_ref,opt.plan,plan["research"]["ref"]],"rights_basis_ref":null,"event_version":1,"supersedes_event_ref":null});
    if opt.generation.is_none() {
        let old = read_json_bounded(ctx, s(&plan["provenance_ref"])?, META_CAP, &mut held)?;
        ensure(
            held.last().unwrap().digest == EVENT_SHA,
            "unknown retained morphology context provenance",
        )?;
        for k in ["agent_refs", "warnings"] {
            event[k] = old[k].clone();
        }
        for k in ["artifact_digest", "runtime"] {
            event["method"][k] = old["method"][k].clone();
        }
        ensure(
            event == old,
            "historical morphology context provenance drift",
        )?;
    }
    valid(&schemas, "provenance-event", &event)?;
    let event_bytes = canonical(event)?;
    a.verify(&a_ctx)?;
    for h in &mut inputs_held {
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
        ensure(p && r && e, "morphology context outputs missing")?;
    }
    ctx.check()?;
    Ok(
        json!({"status":if opt.build{"materialized"}else{"verified"},"packet_ref":packet_ref,"packet_sha256":sha(&packet),"packet_bytes":packet.len(),"receipt_ref":receipt_ref,"receipt_sha256":sha(&receipt_bytes),"provenance_ref":event_ref,"provenance_sha256":sha(&event_bytes),"context_summary":summary,"selection_count":rows.len(),"budget":ctx.budget_report()}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_content_context_keeps_unicode_target_offsets() {
        let ctx = ResearchExecution::new(&std::env::current_dir().unwrap(), 10).unwrap();
        let root = xml_with_doctype(
            &ctx,
            "<TEI><text><p>α<hi>β</hi>🙂 tail</p><lg><l>x<hi>ß</hi>z</l></lg></text></TEI>"
                .as_bytes(),
            false,
        )
        .unwrap();
        let path = "TEI/text[1]/p[1]/hi[1]/tail()[1]";
        let node = text_node(&root, path).unwrap();
        let (p, kind) = context_unit(path).unwrap();
        assert_eq!(kind, "paragraph");
        let (text, base) = flatten(&ctx, element(&root, &p).unwrap(), node).unwrap();
        assert_eq!(text, "αβ🙂 tail");
        assert_eq!(base, 2);
        assert_eq!(unicode_slice(&text, base, base + 1).unwrap(), "🙂");
        let path = "TEI/text[1]/lg[1]/l[1]/hi[1]/text()[1]";
        assert_eq!(context_unit(path).unwrap().1, "verse-group");
        assert!(context_unit("TEI/text[1]/div[1]/text()[1]").is_err());
    }
}
