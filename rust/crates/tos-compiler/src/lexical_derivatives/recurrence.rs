//! Exact rational recurrence over a fixity-selected hash-only lexical projection.
use super::{
    Held, META_CAP, PACKET_CAP, ResearchExecution, Result, canonical, ensure, generated_path,
    present, s, sha, valid_generation,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
const BUILDER: &str = "rust/crates/tos-compiler/src/lexical_derivatives/recurrence.rs";
pub const PLAN: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/lexical-indexes/dta-first-editions-parts-1-4-v1/recurrence-plan.v1.json";
const RETAINED_PROJECTION_SHA: &str =
    "7b5ecdd8dc9f3911bbf25a29da1c820d836b26502a4e925e89baf39bde011edb";
const RETAINED_EVENT_SHA: &str = "edda5ab455924ddbe7ad9ff321f637d5028e91389f72e3bacd314a1d815eaafc";
fn u(v: &Value) -> Result<u64> {
    v.as_u64()
        .ok_or("unsigned recurrence integer required".into())
}
fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or("recurrence array required".into())
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or("recurrence count overflow".into())
}
fn round_even(numerator: u128, denominator: u128, scale: u64) -> Result<u64> {
    ensure(denominator != 0, "zero recurrence denominator")?;
    let n = numerator
        .checked_mul(scale as u128)
        .ok_or("recurrence scale overflow")?;
    let (q, r) = (n / denominator, n % denominator);
    let increment = r > denominator - r || (r == denominator - r && q % 2 == 1);
    u64::try_from(q + u128::from(increment)).map_err(|_| "recurrence rounded value overflow".into())
}
fn sum_counts(rows: &[Value]) -> Result<u64> {
    rows.iter()
        .try_fold(0, |total, row| add(total, u(&row["occurrence_count"])?))
}
fn row_projection(
    ctx: &ResearchExecution,
    row: &Value,
    parts: &BTreeMap<String, u64>,
    total: u64,
    scale: u64,
) -> Result<Value> {
    ctx.tick(1)?;
    let digest = s(&row["exact_form_sha256"])?;
    ensure(
        s(&row["form_key"])? == format!("lexical-form:sha256:{digest}"),
        "recurrence form key mismatch",
    )?;
    let count = u(&row["occurrence_count"])?;
    ensure(count > 0 && count <= total, "recurrence occurrence range")?;
    let hits = array(&row["source_items"])?;
    ensure(
        !hits.is_empty() && hits.len() <= 4,
        "recurrence source hit range",
    )?;
    let mut observed: BTreeMap<&str, u64> = parts.keys().map(|key| (key.as_str(), 0)).collect();
    let mut seen = BTreeSet::new();
    let (mut sections, mut pages, mut section_count) = (0u64, 0u64, 0u64);
    for hit in hits {
        ctx.tick(1)?;
        let item = s(&hit["item_ref"])?;
        ensure(
            seen.insert(item) && parts.contains_key(item),
            "unknown or duplicate recurrence part",
        )?;
        let item_count = u(&hit["occurrence_count"])?;
        ensure(
            item_count > 0 && item_count <= parts[item],
            "invalid recurrence part count",
        )?;
        *observed.get_mut(item).ok_or("missing recurrence part")? = item_count;
        let page_hits = array(&hit["page_hits"])?;
        let section_hits = array(&hit["section_hits"])?;
        ctx.tick((page_hits.len() + section_hits.len()) as u64)?;
        ensure(
            !page_hits.is_empty() && sum_counts(page_hits)? == item_count,
            "recurrence page-hit counts do not close",
        )?;
        pages = add(pages, page_hits.len() as u64)?;
        sections = add(sections, section_hits.len() as u64)?;
        section_count = add(section_count, sum_counts(section_hits)?)?;
    }
    ensure(
        observed.values().try_fold(0, |a, b| add(a, *b))? == count,
        "recurrence part counts do not close",
    )?;
    let unsectioned = u(&row["unsectioned_occurrence_count"])?;
    ensure(
        add(section_count, unsectioned)? == count,
        "recurrence section counts do not close",
    )?;
    let numerator = parts.iter().try_fold(0u128, |sum, (item, part_count)| {
        let a = *part_count as u128 * count as u128;
        let b = observed[item.as_str()] as u128 * total as u128;
        sum.checked_add(a.abs_diff(b))
            .ok_or("recurrence rational sum overflow")
    })?;
    let denominator = (total as u128 * count as u128)
        .checked_mul(2)
        .ok_or("recurrence denominator overflow")?;
    Ok(json!({
        "form_key":row["form_key"],"exact_form_sha256":digest,"normalized_form_sha256":row["normalized_form_sha256"],
        "occurrence_count":count,"part_range":hits.len(),"section_range":sections,"page_range":pages,
        "part_dp_millionths":round_even(numerator,denominator,scale)?,
        "maximum_part_share_millionths":round_even(*observed.values().max().unwrap() as u128,count as u128,scale)?,
        "source_editorial_occurrence_count":row["source_editorial_occurrence_count"],"unsectioned_occurrence_count":unsectioned,
    }))
}
fn load(ctx: &ResearchExecution, reference: &str, cap: u64, held: &mut Vec<Held>) -> Result<Value> {
    let mut h = Held::open(ctx, reference, cap)?;
    let raw = ctx.read_file(&mut h.file, cap)?;
    let value = crate::zarathustra_lexical::parse(&raw, cap as usize)?;
    ensure(value.is_object(), "recurrence object required")?;
    held.push(h);
    Ok(value)
}
fn validate(schemas: &tos_validation::SchemaBackendProbe, name: &str, value: &Value) -> Result<()> {
    // Source Value was decoded under PublishedStrict once. Generated values
    // contain only those checked values and typed fields from this reducer.
    ensure(
        schemas
            .is_valid_value(
                &format!("https://tree-of-sophia.local/ToS/contracts/{name}.schema.json"),
                value,
            )
            .map_err(|e| format!("recurrence schema: {e:?}"))?,
        &format!("{name} schema refused"),
    )
}
pub struct Options<'a> {
    pub build: bool,
    pub output_root: &'a Path,
    pub plan: &'a str,
    pub generation: Option<&'a str>,
    pub event_at: Option<&'a str>,
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
    if let Some(g) = options.generation {
        ensure(valid_generation(g), "generation syntax")?;
    }
    if options.build {
        ensure(
            !options.output_root.starts_with(ctx.root())
                && !ctx.root().starts_with(options.output_root),
            "recurrence output must be separate from source",
        )?;
    }
    let mut held = Vec::new();
    let mut resources = Vec::new();
    for name in [
        "lexical-recurrence-plan",
        "lexical-index-projection",
        "lexical-recurrence-projection",
        "provenance-event",
    ] {
        let reference = format!("ToS/contracts/{name}.schema.json");
        let mut h = Held::open(ctx, &reference, META_CAP)?;
        resources.push(tos_validation::SchemaResource {
            uri: format!("https://tree-of-sophia.local/{reference}"),
            raw: ctx.read_file(&mut h.file, META_CAP)?,
        });
        held.push(h);
    }
    let schemas = tos_validation::SchemaBackendProbe::new(
        resources,
        tos_validation::FormatProfile::AssertedSourceCandidateV1,
    )
    .map_err(|e| format!("recurrence schema preparation: {e:?}"))?;
    let plan = load(ctx, options.plan, META_CAP, &mut held)?;
    let plan_sha = held.last().unwrap().digest.clone();
    validate(&schemas, "lexical-recurrence-plan", &plan)?;
    let source_ref = s(&plan["source_projection"]["ref"])?;
    let source = load(ctx, source_ref, PACKET_CAP as u64, &mut held)?;
    let source_sha = held.last().unwrap().digest.clone();
    ensure(
        source_sha == s(&plan["source_projection"]["sha256"])?,
        "lexical projection differs from frozen recurrence plan bytes",
    )?;
    validate(&schemas, "lexical-index-projection", &source)?;
    ctx.check()?;
    ensure(
        source["schema_version"] == plan["source_projection"]["schema_version"],
        "recurrence source schema drift",
    )?;
    for key in [
        "source_item_count",
        "exact_form_row_count",
        "token_occurrence_count",
    ] {
        ensure(
            source["summary"][key] == plan["source_projection"][key],
            &format!("recurrence source {key} drift"),
        )?;
    }
    let mut ordered = array(&source["source_items"])?
        .iter()
        .map(|v| {
            Ok((
                u(&v["part_order"])?,
                s(&v["item_ref"])?.to_owned(),
                u(&v["token_occurrence_count"])?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    ordered.sort();
    ensure(
        ordered.iter().map(|v| v.0).collect::<Vec<_>>() == [1, 2, 3, 4],
        "recurrence source part order must be 1..4",
    )?;
    let mut parts = BTreeMap::new();
    let mut part_totals = Vec::new();
    let mut total = 0;
    for (order, item, count) in ordered {
        ensure(
            count > 0 && parts.insert(item.clone(), count).is_none(),
            "duplicate or empty recurrence part",
        )?;
        total = add(total, count)?;
        part_totals.push(json!({"part_order":order,"item_ref":item,"token_count":count}));
    }
    ensure(
        total == u(&source["summary"]["token_occurrence_count"])?,
        "recurrence source token total drift",
    )?;
    let inputs = array(&source["form_rows"])?;
    ensure(
        !inputs.is_empty() && inputs.len() <= 1_000_000,
        "recurrence input row bound",
    )?;
    let mut rows = Vec::new();
    let mut previous: Option<&str> = None;
    for row in inputs {
        let digest = s(&row["exact_form_sha256"])?;
        ensure(
            previous.is_none_or(|v| v < digest),
            "strict recurrence digest order",
        )?;
        previous = Some(digest);
        rows.push(row_projection(
            ctx,
            row,
            &parts,
            total,
            u(&plan["calculation_law"]["scale"])?,
        )?);
    }
    let mut token_count = 0;
    let mut single_part = 0;
    let mut all_parts = 0;
    let mut singletons = 0;
    let mut min_dp = u64::MAX;
    let mut max_dp = 0;
    for row in &rows {
        token_count = add(token_count, u(&row["occurrence_count"])?)?;
        single_part += u64::from(row["part_range"] == 1);
        all_parts += u64::from(row["part_range"] == 4);
        singletons += u64::from(row["occurrence_count"] == 1);
        let dp = u(&row["part_dp_millionths"])?;
        min_dp = min_dp.min(dp);
        max_dp = max_dp.max(dp);
    }
    ensure(
        token_count == total && rows.len() as u64 == u(&source["summary"]["exact_form_row_count"])?,
        "recurrence form row totals do not close",
    )?;
    let summary = json!({"row_count":rows.len(),"token_occurrence_count":token_count,"single_part_form_count":single_part,"all_four_parts_form_count":all_parts,"singleton_form_count":singletons,"minimum_part_dp_millionths":min_dp,"maximum_part_dp_millionths":max_dp,"semantic_fields_populated":0});
    let research_ref = s(&plan["research_ref"])?;
    let research = Held::open(ctx, research_ref, META_CAP)?;
    let research_sha = research.digest.clone();
    held.push(research);
    let builder = Held::open(ctx, BUILDER, META_CAP)?;
    ensure(
        builder.digest == sha(include_bytes!("recurrence.rs")),
        "running recurrence generator source drift",
    )?;
    let builder_sha = builder.digest.clone();
    held.push(builder);
    let suffix = options
        .generation
        .map(|g| format!(".native-{g}"))
        .unwrap_or_default();
    let event_id = format!("{}{suffix}", s(&plan["provenance_event_ref"])?);
    let base_output = s(&plan["output"]["ref"])?;
    let base_event = s(&plan["output"]["provenance_ref"])?;
    let output_ref = options
        .generation
        .map(|g| generated_path(base_output, g))
        .transpose()?
        .unwrap_or(base_output.into());
    let event_ref = options
        .generation
        .map(|g| generated_path(base_event, g))
        .transpose()?
        .unwrap_or(base_event.into());
    ensure(
        output_ref != event_ref,
        "recurrence output paths must differ",
    )?;
    let mut projection = json!({
        "$schema":"https://tree-of-sophia.local/ToS/contracts/lexical-recurrence-projection.schema.json","schema_version":"tos_lexical_recurrence_projection_v1","generated_or_authored":"generated_from_tracked_lexical_projection",
        "projection_id":format!("lexical-recurrence-projection:zarathustra-dta-first-editions-parts-1-4-v1{suffix}"),"plan":{"ref":options.plan,"sha256":plan_sha},"source_projection":{"ref":source_ref,"sha256":source_sha,"schema_version":source["schema_version"]},"generator":{"ref":BUILDER,"sha256":builder_sha},"work_ref":plan["work_ref"],
        "segmentation_totals":{"part_count":4,"section_count":source["summary"]["section_count"],"page_count":source["summary"]["body_page_count"],"token_count":total,"parts":part_totals},"method_views":{"A":"absolute-frequency","B":"structural-range","C":"tupleized-part-size-aware-dispersion"},"summary":summary,"rows":rows,"content_exposure":plan["content_exposure"],
        "rights_and_visibility":{"source_payload_visibility":"local-only","tracked_projection_visibility":"local-only","future_site_route":"blocked","rights_review_required_before_public_route":true},"semantic_boundary":plan["semantic_boundary"],"provenance_event_ref":event_id,
        "authority_boundary":"This projection records deterministic hash-based exact-form recurrence observations from the selected lexical projection.",
    });
    if options.generation.is_none() {
        let prior = load(ctx, base_output, PACKET_CAP as u64, &mut held)?;
        ensure(
            options.plan == PLAN && held.last().unwrap().digest == RETAINED_PROJECTION_SHA,
            "unknown historical recurrence projection",
        )?;
        for key in ["generator", "authority_boundary"] {
            projection[key] = prior[key].clone();
        }
        ensure(
            projection == prior,
            "historical recurrence projection drift",
        )?;
    }
    validate(&schemas, "lexical-recurrence-projection", &projection)?;
    ctx.check()?;
    let projection_bytes = canonical(projection)?;
    let timestamp = options
        .event_at
        .unwrap_or(s(&plan["output"]["recorded_at"])?);
    let mut event = json!({"schema_version":"tos_provenance_event_v1","event_id":event_id,"event_type":"export","started_at":timestamp,"ended_at":timestamp,"agent_refs":["software:tos-native-rust"],
        "inputs":[{"ref":options.plan,"role":"frozen-exact-form-recurrence-observation-plan","sha256":plan_sha},{"ref":source_ref,"role":"tracked-hash-count-and-resource-lexical-projection","sha256":source_sha},{"ref":research_ref,"role":"ordered-recurrence-method-research","sha256":research_sha}],
        "outputs":[{"ref":output_ref,"role":"tracked-hash-only-exact-form-recurrence-observation","sha256":sha(&projection_bytes)}],
        "method":{"maker_type":"software","name":"tupleized-exact-form-recurrence-projection","version":"1","artifact_digest":builder_sha,"runtime":"Rust checked u128 rational arithmetic; nearest integer ties to even","device":"CPU","configuration":{"source_item_count":4,"exact_form_rows":inputs.len(),"token_occurrences":total,"method_views":["absolute-frequency","structural-range","tupleized-part-size-aware-dispersion"],"dp_scale":plan["calculation_law"]["scale"],"rounding":plan["calculation_law"]["rounding"],"composite_score_created":false,"tracked_exact_strings":false,"sign_candidate_materialized":false,"human_work_scheduled":false},"prompt_or_instruction_ref":options.plan},
        "status":"completed_with_warnings","warnings":["frequency and dispersion remain source observations and cannot nominate a stable sign","form hashes are low-entropy navigational fingerprints and do not provide confidentiality","the tracked projection contains no exact strings, sequence, context, or occurrence positions","source text, German competence, morphology, lemma, lexeme, translation, sign, semantics, publication, and human review remain outside this projection"],
        "receipt_refs":[output_ref,options.plan,research_ref],"rights_basis_ref":null,"event_version":1,"supersedes_event_ref":null,
    });
    if options.generation.is_none() {
        let prior = load(ctx, base_event, META_CAP, &mut held)?;
        ensure(
            held.last().unwrap().digest == RETAINED_EVENT_SHA,
            "unknown historical recurrence event",
        )?;
        event["agent_refs"] = prior["agent_refs"].clone();
        event["method"]["artifact_digest"] = prior["method"]["artifact_digest"].clone();
        event["method"]["runtime"] = prior["method"]["runtime"].clone();
        ensure(event == prior, "historical recurrence event drift")?;
    }
    validate(&schemas, "provenance-event", &event)?;
    let event_bytes = canonical(event)?;
    for h in &mut held {
        h.verify(ctx)?;
    }
    let output = ctx.select_output_directory(options.output_root, options.build)?;
    let projection_present = present(&output, &output_ref, &projection_bytes, 0o644)?;
    let event_present = present(&output, &event_ref, &event_bytes, 0o644)?;
    if options.build {
        if !projection_present {
            output.write(&output_ref, &projection_bytes, 0o644, true)?;
        }
        if !event_present {
            output.write(&event_ref, &event_bytes, 0o644, true)?;
        }
    } else {
        ensure(
            projection_present && event_present,
            "recurrence output missing",
        )?;
    }
    ctx.check()?;
    Ok(
        json!({"status":if options.build {"materialized"} else {"verified"},"projection_ref":output_ref,"projection_sha256":sha(&projection_bytes),"provenance_ref":event_ref,"provenance_sha256":sha(&event_bytes),"summary":summary,"budget":ctx.budget_report()}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rational_ties_even_and_overflow() {
        for (n, d, s, w) in [
            (0, 1, 1_000_000, 0),
            (1, 2, 1_000_000, 500000),
            (99, 100, 1_000_000, 990000),
            (1, 2, 1, 0),
            (3, 2, 1, 2),
            (5, 2, 1, 2),
            (7, 2, 1, 4),
        ] {
            assert_eq!(round_even(n, d, s).unwrap(), w);
        }
        assert!(round_even(1, 0, 1).is_err());
        assert!(round_even(u128::MAX, 1, 2).is_err());
    }
}
