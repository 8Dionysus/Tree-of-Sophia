//! Temporary bridge until the selected PDF structure producer moves into research.
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tos_compiler::antonovsky_structural as legacy;
use tos_source_store::execution::Execution;

pub fn validate_recipe(ctx: &Execution, recipe: &Value) -> Result<Vec<u8>, String> {
    if recipe["layout_plan_dir"] != legacy::V1 || recipe["structural_output_dir"] != legacy::V2 {
        return Err("selected PDF recipe is unsupported by the retained producer".into());
    }
    let plan_raw = ctx.read(&format!("{}/plan.v1.json", legacy::V1))?;
    if recipe["layout_plan_sha256"] != tos_foundation::Digest256::of_bytes(&plan_raw).to_hex() {
        return Err("selected PDF plan digest differs from curation".into());
    }
    let plan: Value = serde_json::from_slice(&plan_raw).map_err(|e| e.to_string())?;
    let policy = &plan["extraction_policy"];
    let extraction = &recipe["extraction"];
    let screening_ref = plan["technical_screening_plan_ref"]
        .as_str()
        .ok_or("PDF screening plan required")?;
    let screening: Value =
        serde_json::from_slice(&ctx.read(screening_ref)?).map_err(|e| e.to_string())?;
    if recipe["fixity"]["pdf_sha256"] != plan["source_item"]["file_sha256"]
        || recipe["fixity"]["text_sha256"] != screening["private_text_layer_sha256"]
    {
        return Err("selected PDF fixity differs from retained plans".into());
    }
    if extraction["tool"] != "pdftotext"
        || extraction["arguments"] != json!(["-bbox-layout", "SOURCE_PDF", "-"])
        || extraction["version"] != policy["software_version"]
        || extraction["expected_bbox_bytes"] != policy["expected_bbox_bytes"]
        || extraction["expected_warning_count"] != policy["expected_warning_count"]
        || extraction["expected_warning"] != policy["expected_warning"]
        || policy["expected_bbox_sha256"] != recipe["fixity"]["bbox_sha256"]
        || plan["expected_counts"]["layout_block_paragraph_candidates"]
            != recipe["layout_rules"]["expected_census"]["poppler_blocks"]
    {
        return Err("selected PDF extraction differs from retained plan".into());
    }
    Ok(plan_raw)
}

pub fn replay(
    ctx: &Execution,
    recipe: &Value,
    target: Option<&Execution>,
) -> Result<Value, String> {
    let plan_raw = validate_recipe(ctx, recipe)?;
    let mut expected = BTreeMap::new();
    for entry in recipe["outputs"].as_array().ok_or("PDF outputs required")? {
        let filename = entry["filename"].as_str().ok_or("PDF filename required")?;
        if filename.is_empty() || filename.contains('/') || filename == "." || filename == ".." {
            return Err("PDF output filename must be a single component".into());
        }
        let reference = format!("{}/{filename}", legacy::V2);
        let digest = entry["expected_sha256"]
            .as_str()
            .ok_or("PDF output digest required")?;
        if expected.insert(reference, digest).is_some() {
            return Err("duplicate PDF output".into());
        }
    }
    ctx.reserve_structural_reads()?;
    let model = legacy::reconstruct_from_directory_with_recipe(
        ctx.root(),
        ctx.root_directory(),
        ctx.deadline(),
        recipe,
    )?;
    let mut charged = 0;
    ctx.charge_structural(&model, &mut charged)?;
    let artifacts = legacy::make_outputs(ctx.root(), &model)?;
    ctx.charge_structural(&model, &mut charged)?;
    if artifacts.len() != expected.len() {
        return Err("PDF output membership differs from curation".into());
    }
    for (reference, digest) in &expected {
        let rebuilt = artifacts.get(reference).ok_or("PDF output missing")?;
        if tos_foundation::Digest256::of_bytes(rebuilt).to_hex() != *digest
            || ctx.read(reference)? != *rebuilt
        {
            return Err(format!("PDF output fixity differs: {reference}"));
        }
    }
    if let Some(target) = target {
        for (reference, raw) in &artifacts {
            target.write(reference, raw, 0o644, true)?;
        }
        for (reference, raw) in &artifacts {
            if target.read(reference)? != *raw {
                return Err(format!("PDF output replay differs: {reference}"));
            }
        }
    }
    ctx.check()?;
    Ok(
        json!({"status":"passed", "tracked_output_count":artifacts.len(), "producer":"retained-compiler-pdf-structure", "plan_sha256":tos_foundation::Digest256::of_bytes(&plan_raw).to_hex()}),
    )
}
