//! Source-bound verification of curated numbered navigation.
use crate::{numbered_matching, numbered_structure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, Metadata},
    path::Path,
};
use tos_foundation::{JsonLimits, JsonMode, RelativePath, parse_json};
use tos_source_store::execution::Execution;
use tos_validation::{
    SchemaBackendProbe,
    edition_curation::{self, SelectedDocument},
};

const EXISTING_FILE_LIMIT: u64 = 128 * 1024 * 1024;
const EXISTING_METADATA_LIMIT: usize = 4 * 1024 * 1024;

fn required<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{name} required"))
}
fn values<'a>(value: &'a Value, name: &str) -> Result<&'a Vec<Value>, String> {
    value.as_array().ok_or_else(|| format!("{name} required"))
}
fn object(raw: &[u8]) -> Result<Value, String> {
    let limits = JsonLimits::new(EXISTING_METADATA_LIMIT, 128, 2_000_000, 4300)
        .map_err(|error| error.to_string())?;
    parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|error| error.to_string())?;
    let value: Value = serde_json::from_slice(raw).map_err(|error| error.to_string())?;
    if !value.is_object() {
        return Err("metadata object required".into());
    }
    Ok(value)
}
fn locator(recipe: &Value, item_dir: &str, roles: &[&str]) -> Result<String, String> {
    let mut matches = values(&recipe["locators"], "locators")?
        .iter()
        .filter(|row| {
            row["role"]
                .as_str()
                .is_some_and(|role| roles.contains(&role))
        });
    let row = matches.next().ok_or("selected locator missing")?;
    if matches.next().is_some() {
        return Err("selected locator is ambiguous".into());
    }
    let reference = if let Some(path) = row["repository_path"].as_str() {
        path.to_owned()
    } else if let Some(path) = row["item_relative_path"].as_str() {
        format!("{item_dir}/{path}")
    } else if row["item_root"] == true {
        item_dir.to_owned()
    } else {
        return Err("invalid selected locator".into());
    };
    RelativePath::parse(&reference).map_err(|error| error.to_string())?;
    Ok(reference)
}
struct Held {
    reference: String,
    file: File,
    metadata: Metadata,
    digest: String,
}
impl Held {
    fn open(ctx: &Execution, reference: String, entry: &Value) -> Result<Self, String> {
        let mut file = ctx.source_file(&reference, EXISTING_FILE_LIMIT)?;
        let metadata = file.metadata().map_err(|error| error.to_string())?;
        if entry["byte_size"].as_u64() != Some(metadata.len()) {
            return Err("payload byte size differs".into());
        }
        let digest = ctx.hash_file(&mut file, EXISTING_FILE_LIMIT)?;
        if entry["sha256"] != digest || entry["file_id"] != format!("tos.file.sha256.{digest}") {
            return Err("payload fixity or identity differs".into());
        }
        Ok(Self {
            reference,
            file,
            metadata,
            digest,
        })
    }
    fn verify(&mut self, ctx: &Execution) -> Result<(), String> {
        ctx.verify_file_unchanged(&self.file, &self.metadata)?;
        let current = ctx.source_file(&self.reference, self.metadata.len())?;
        ctx.verify_file_unchanged(&current, &self.metadata)?;
        if ctx.hash_file(&mut self.file, self.metadata.len())? != self.digest {
            return Err("held payload bytes changed".into());
        }
        Ok(())
    }
}

/// Recompute numbered page selections and verify every retained output's fixity.
pub fn check(
    ctx: &Execution,
    curation_ref: &str,
    curation: &[u8],
    owner: &Value,
    recipe: &Value,
    manifest_raw: &[u8],
    input_root: &Path,
    schema: &SchemaBackendProbe,
) -> Result<Value, String> {
    if !input_root.is_absolute() {
        return Err("local input root must be absolute".into());
    }
    numbered_matching::preflight(recipe)?;
    numbered_structure::validate_frozen_map(ctx, curation, owner, recipe, schema)?;
    let item_dir = Path::new(curation_ref)
        .parent()
        .and_then(Path::to_str)
        .ok_or("curation item path required")?;
    let inventory_ref = locator(recipe, item_dir, &["inventory_path"])?;
    let rights_ref = locator(recipe, item_dir, &["rights_path", "rights_ref"])?;
    let inventory_raw = ctx.read(&inventory_ref)?;
    let rights_raw = ctx.read(&rights_ref)?;
    let manifest = object(manifest_raw)?;
    let inventory = object(&inventory_raw)?;
    let _rights = object(&rights_raw)?;
    if manifest["resource_inventory_ref"] != inventory_ref || manifest["rights_ref"] != rights_ref {
        return Err("manifest metadata locators differ".into());
    }
    let mut outputs = Vec::new();
    for output in values(&recipe["frozen_outputs"], "frozen outputs")? {
        let role = required(&output["role"], "output role")?;
        let reference = required(&output["repository_path"], "output path")?;
        let raw = ctx.read(reference)?;
        edition_curation::validate_frozen_output_fixity(
            curation,
            "numbered-structure",
            role,
            SelectedDocument {
                reference,
                raw: &raw,
            },
            schema,
        )
        .map_err(|error| format!("frozen output: {error:?}"))?;
        outputs.push((role.to_owned(), reference.to_owned(), raw));
    }
    let roles = values(&recipe["payload_roles"], "payload roles")?;
    let entries = values(&manifest["payload_files"], "payload files")?;
    if roles.len() != entries.len() {
        return Err("payload role count differs".into());
    }
    let inventory_files = values(&inventory["files"], "inventory files")?;
    if inventory_files.len() != roles.len() {
        return Err("inventory role count differs".into());
    }
    let page_count = recipe["candidate_profile"]["source_page_count"]
        .as_u64()
        .ok_or("source page count required")?;
    let mut selected = BTreeMap::new();
    let mut matched_files = BTreeSet::new();
    for role in roles {
        let name = required(&role["role"], "payload role")?;
        let media = required(&role["media_type"], "payload media type")?;
        let suffix = role["relative_path_suffix"].as_str();
        let mut candidates = entries.iter().filter(|entry| {
            entry["media_type"] == media
                && entry["relative_path"]
                    .as_str()
                    .is_some_and(|path| suffix.is_none_or(|suffix| path.ends_with(suffix)))
        });
        let entry = candidates.next().ok_or("payload role missing")?;
        if candidates.next().is_some()
            || !matched_files.insert(required(&entry["file_id"], "file identity")?.to_owned())
        {
            return Err("payload role is ambiguous".into());
        }
        let profile = required(&role["inventory_profile"], "inventory profile")?;
        let mut files = inventory_files
            .iter()
            .filter(|file| file["profile"] == profile);
        let file = files.next().ok_or("inventory profile missing")?;
        if files.next().is_some()
            || file["file_id"] != entry["file_id"]
            || file["summary"]["page_count"].as_u64() != Some(page_count)
            || (!file["file_sha256"].is_null() && file["file_sha256"] != entry["sha256"])
        {
            return Err("inventory identity or page count differs".into());
        }
        let relative = required(&entry["relative_path"], "payload relative path")?;
        RelativePath::parse(relative).map_err(|error| error.to_string())?;
        if selected
            .insert(
                name.to_owned(),
                (format!("{item_dir}/{relative}"), entry.clone()),
            )
            .is_some()
        {
            return Err("duplicate payload role".into());
        }
    }
    let mut boundary_before = None;
    if recipe["candidate_profile"]["kind"] == "pdf-bbox" {
        let boundary_ref = locator(recipe, item_dir, &["work_boundary_path"])?;
        let boundary_raw = ctx.read(&boundary_ref)?;
        let boundary = object(&boundary_raw)?;
        boundary_before = Some((boundary_ref, boundary_raw));
        let profile = &recipe["candidate_profile"];
        let members = values(&boundary["members"], "work boundary members")?
            .iter()
            .filter(|member| {
                member["sequence"] == profile["member_sequence"]
                    && member["work_ref"] == recipe["work_ref"]
                    && member["expression_ref"] == recipe["expression_ref"]
            })
            .collect::<Vec<_>>();
        if members.len() != 1
            || members[0]["start_page"] != profile["first_page"]
            || members[0]["end_page"] != profile["last_page"]
        {
            return Err("work boundary differs from selected pages".into());
        }
    }
    let source = ctx.select_directory(input_root)?;
    let mut payloads = BTreeMap::new();
    for (role, (reference, entry)) in selected {
        payloads.insert(role, Held::open(&source, reference, &entry)?);
    }
    let keys = numbered_structure::keys(ctx, recipe)?;
    let matches = match recipe["candidate_profile"]["kind"].as_str() {
        Some("abbyy") => {
            let held = payloads.get_mut("abbyy").ok_or("OCR payload required")?;
            let raw = source.read_file(&mut held.file, held.metadata.len())?;
            let candidates = numbered_matching::abbyy_candidates(ctx, recipe, &raw)?;
            numbered_matching::weighted(ctx, recipe, &keys, &candidates)?
        }
        Some("pdf-bbox") => {
            let held = payloads.get("pdf").ok_or("PDF payload required")?;
            let candidates = numbered_matching::pdf_candidates(ctx, recipe, &held.file)?;
            numbered_matching::monotonic(&keys, &candidates)?
        }
        _ => return Err("unsupported numbered navigation profile".into()),
    };
    let matched_count = matches.pages.len();
    for key in values(&recipe["required_ocr_keys"], "required OCR keys")? {
        if !matches.pages.contains_key(required(key, "OCR key")?) {
            return Err("required OCR key missing".into());
        }
    }
    for evidence in values(&recipe["ocr_evidence"], "OCR evidence")? {
        let key = required(&evidence["unit_key"], "OCR evidence key")?;
        if matches.pages.get(key).copied() != evidence["pdf_page"].as_u64()
            || matches.raw.get(key).map(String::as_str) != evidence["raw_token"].as_str()
        {
            return Err("retained OCR evidence differs".into());
        }
    }
    let matches = numbered_matching::apply_overrides(recipe, &keys, matches)?;
    let map_raw = outputs
        .iter()
        .find(|(role, _, _)| role == "map")
        .map(|(_, _, raw)| raw)
        .ok_or("frozen map required")?;
    let map = object(map_raw)?;
    for unit in values(&map["unit_starts"], "map units")? {
        let key = required(&unit["unit_key"], "unit key")?;
        if matches.pages.get(key).copied() != unit["pdf_page"].as_u64() {
            return Err("recomputed numbered page differs from frozen map".into());
        }
    }
    for payload in payloads.values_mut() {
        payload.verify(&source)?;
    }
    if ctx.read(&inventory_ref)? != inventory_raw || ctx.read(&rights_ref)? != rights_raw {
        return Err("selected metadata changed during verification".into());
    }
    if let Some((reference, before)) = boundary_before {
        if ctx.read(&reference)? != before {
            return Err("work boundary changed during verification".into());
        }
    }
    numbered_structure::validate_frozen_map(ctx, curation, owner, recipe, schema)?;
    for (role, reference, before) in &outputs {
        let raw = ctx.read(reference)?;
        if &raw != before {
            return Err("frozen output changed during verification".into());
        }
        edition_curation::validate_frozen_output_fixity(
            curation,
            "numbered-structure",
            role,
            SelectedDocument {
                reference,
                raw: &raw,
            },
            schema,
        )
        .map_err(|error| format!("frozen output: {error:?}"))?;
    }
    Ok(
        json!({"status":"candidate-pages-and-frozen-fixity-verified","stage":"numbered-structure","unit_count":keys.len(),"machine_matches":matched_count,"reviewed_overrides":matches.skipped.len(),"frozen_outputs_verified":outputs.len()}),
    )
}
