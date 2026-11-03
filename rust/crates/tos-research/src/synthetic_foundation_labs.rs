//! Public synthetic fixture definitions and the native owner validation route.
//! These are authored test inputs, never evidence of a translation or review act.
use crate::research_support::{encode, ensure, read_optional, s, sha};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs::File, time::Instant};
use tos_source_store::execution::Execution as ResearchExecution;
use tos_validation::{
    FormatProfile, SchemaBackendProbe, SchemaResource,
    item_rules::{ItemLimits, ItemRefusal},
    layer_family_rules::LayerFamilySource,
    source_foundation_labs::{SourceFoundationLab, inspect_source_foundation_lab},
};
type Result<T> = std::result::Result<T, String>;
const BUILDER: &str = "rust/crates/tos-compiler/src/synthetic_foundation_labs.rs";
// The shared text-metadata owner accepts packets up to 2 MiB. The compiled
// recipes and their selected dependencies fit this same admitted envelope.
const CAP: usize = 2 * 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_lab_keeps_expected_invalid_variant_and_checks_every_control() {
        let root = tempfile::tempdir().unwrap();
        let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for reference in [
            "ToS/contracts/source-text-unit-packet-v1.schema.json",
            "ToS/research-packets/foundation-laboratory-2026-07/SOURCE_TEXT_UNIT_SEGMENTATION_RESEARCH_2026-08-11.md",
        ] {
            let path = root.path().join(reference);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::copy(source_root.join(reference), path).unwrap();
        }
        let ctx = ResearchExecution::new(root.path(), 30).unwrap();
        let prepared = prepare(&ctx, "source-text-unit-v1").unwrap();
        assert_eq!(prepared.files.len(), 6);
        let manifest: Value = serde_json::from_slice(&prepared.files[&prepared.manifest]).unwrap();
        assert_eq!(manifest["variants"][2]["variant_id"], "C");
        assert_eq!(manifest["variants"][2]["expected_semantic_valid"], false);
        assert!(!root.path().join(&prepared.manifest).exists());
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipeBook {
    schema_version: String,
    recipes: BTreeMap<String, Recipe>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Recipe {
    legacy: String,
    manifest: String,
    contract: String,
    research: String,
    files: Vec<(String, String)>,
}
fn substitute(value: &mut Value, old: &str) {
    match value {
        Value::String(text) if text == old => *text = BUILDER.into(),
        Value::Array(rows) => {
            for row in rows {
                substitute(row, old);
            }
        }
        Value::Object(fields) => {
            for row in fields.values_mut() {
                substitute(row, old);
            }
        }
        _ => {}
    }
}
struct Prepared {
    files: BTreeMap<String, Vec<u8>>,
    inputs: Vec<(String, File, String)>,
    manifest: String,
}
fn prepare(ctx: &ResearchExecution, name: &str) -> Result<Prepared> {
    let (recipe, lab) = recipe(name)?;
    let mut inputs = Vec::new();
    let mut external = BTreeMap::new();
    for reference in [&recipe.contract, &recipe.research] {
        let mut file = ctx.source_file(reference, CAP as u64)?;
        let raw = ctx.read_file(&mut file, CAP as u64)?;
        inputs.push((reference.to_owned(), file, sha(&raw)));
        external.insert(reference.to_string(), raw);
    }
    let mut files = BTreeMap::new();
    let mut manifest = None;
    for (reference, raw) in &recipe.files {
        ctx.tick(raw.len() as u64)?;
        if reference == &recipe.manifest {
            manifest = Some(serde_json::from_str::<Value>(raw).map_err(|e| e.to_string())?);
            continue;
        }
        let bytes = if reference.ends_with(".json") {
            let mut value: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
            substitute(&mut value, &recipe.legacy);
            encode(&value, true)?
        } else {
            raw.as_bytes().to_vec()
        };
        ensure(bytes.len() <= CAP, "synthetic output bound")?;
        files.insert(reference.to_string(), bytes);
    }
    let mut manifest = manifest.ok_or("missing lab manifest definition")?;
    let builder = include_bytes!("historical/synthetic_foundation_labs.rs.txt");
    manifest["builder"] = json!({"ref":BUILDER,"sha256":sha(builder)});
    for key in ["contract", "research"] {
        let reference = s(&manifest[key]["ref"])?;
        manifest[key]["sha256"] = json!(sha(external
            .get(reference)
            .ok_or("unselected manifest dependency")?));
    }
    for key in ["plan", "source", "input_fixture"] {
        if manifest.get(key).is_some() {
            let reference = s(&manifest[key]["ref"])?;
            manifest[key]["sha256"] = json!(sha(files
                .get(reference)
                .ok_or("manifest fixture binding")?));
        }
    }
    for key in ["inputs", "analysis_artifacts"] {
        if let Some(rows) = manifest.get_mut(key).and_then(Value::as_array_mut) {
            for row in rows {
                let reference = s(&row["ref"])?;
                row["sha256"] = json!(sha(files
                    .get(reference)
                    .ok_or("manifest analysis binding")?));
            }
        }
    }
    for row in manifest["variants"].as_array_mut().ok_or("lab variants")? {
        let reference = s(&row["packet_ref"])?;
        row["packet_sha256"] = json!(sha(files.get(reference).ok_or("variant bytes")?));
    }
    files.insert(recipe.manifest.clone(), encode(&manifest, true)?);
    validate_overlay(
        ctx,
        &files,
        external,
        BUILDER,
        builder,
        &recipe.contract,
        lab,
    )?;
    Ok(Prepared {
        files,
        inputs,
        manifest: recipe.manifest,
    })
}
pub(crate) fn validate_overlay(
    ctx: &ResearchExecution,
    files: &BTreeMap<String, Vec<u8>>,
    external: BTreeMap<String, Vec<u8>>,
    builder_ref: &str,
    builder: &[u8],
    contract_ref: &str,
    lab: SourceFoundationLab,
) -> Result<()> {
    let contract = format!("https://tree-of-sophia.local/{}", contract_ref);
    let schema = SchemaBackendProbe::new(
        [SchemaResource {
            uri: contract.clone(),
            raw: files
                .get(contract_ref)
                .or_else(|| external.get(contract_ref))
                .ok_or("lab contract absent")?
                .clone(),
        }],
        FormatProfile::AssertedSourceCandidateV1,
    )
    .map_err(|e| format!("lab schema: {e:?}"))?;
    let mut generation = tos_foundation::Digest256Hasher::new();
    for (reference, raw) in files.iter().chain(external.iter()) {
        ctx.tick((reference.len() + raw.len()) as u64)?;
        generation.update(&(reference.len() as u64).to_be_bytes());
        generation.update(reference.as_bytes());
        generation.update(tos_foundation::Digest256::of_bytes(raw).as_bytes());
    }
    let mut source = Overlay {
        ctx,
        files,
        external,
        builder_ref,
        builder,
        schema: &schema,
        contract: contract_ref,
        read_bytes: 0,
        generation: generation.finalize().to_hex(),
    };
    let limits = ItemLimits {
        max_member_bytes: CAP,
        max_total_bytes: (32 * CAP) as u64,
        max_state_bytes: 32 * CAP,
        max_issues: 128,
        deadline: ctx.deadline(),
    };
    let report = inspect_source_foundation_lab(
        &mut source,
        limits,
        lab,
        &files.keys().cloned().collect::<Vec<_>>(),
    )
    .map_err(|e| format!("synthetic lab closure: {e:?}"))?;
    ensure(
        report.unimplemented.is_empty() && report.ordered_issues.is_empty(),
        &format!("synthetic lab closure: {:?}", report.ordered_issues),
    )?;
    for request in report.schema_checks {
        ctx.check()?;
        ensure(
            request.contract == contract_ref,
            "unselected schema request",
        )?;
        let valid = schema
            .is_valid_raw(&contract, &encode(&request.instance, false)?)
            .map_err(|e| format!("synthetic schema execution: {e:?}"))?;
        if let Some(expected) = request.expected_valid {
            ensure(valid == expected, "synthetic schema verdict changed")?;
        }
        if let Some(expected) = request.expected_rejected {
            ensure(
                (!valid || request.semantic_rejected.unwrap_or(false)) == expected,
                "synthetic negative control changed",
            )?;
        }
        if request.expected_valid.is_none() && request.expected_rejected.is_none() {
            ensure(valid, "synthetic packet schema invalid")?;
        }
    }
    Ok(())
}
struct Overlay<'a> {
    ctx: &'a ResearchExecution,
    files: &'a BTreeMap<String, Vec<u8>>,
    external: BTreeMap<String, Vec<u8>>,
    builder_ref: &'a str,
    builder: &'a [u8],
    schema: &'a SchemaBackendProbe,
    contract: &'a str,
    read_bytes: u64,
    generation: String,
}
impl LayerFamilySource for Overlay<'_> {
    fn current(
        &mut self,
        path: &str,
        max: usize,
        deadline: Instant,
    ) -> std::result::Result<Option<Vec<u8>>, ItemRefusal> {
        self.checkpoint(deadline)?;
        let value = if path == self.builder_ref {
            Some(self.builder)
        } else {
            self.files
                .get(path)
                .or_else(|| self.external.get(path))
                .map(Vec::as_slice)
        };
        if let Some(raw) = value {
            if raw.len() > max {
                return Err(ItemRefusal::Budget);
            }
            self.read_bytes += raw.len() as u64;
            if self.read_bytes > (32 * CAP) as u64 {
                return Err(ItemRefusal::Budget);
            }
        }
        Ok(value.map(<[u8]>::to_vec))
    }
    fn recorded(
        &mut self,
        path: &str,
        digest: &str,
        max: usize,
        deadline: Instant,
    ) -> std::result::Result<Option<Vec<u8>>, ItemRefusal> {
        if let Some(raw) = self
            .current(path, max, deadline)?
            .filter(|raw| sha(raw) == digest)
        {
            return Ok(Some(raw));
        }
        // Only explicitly selected archive bytes can establish an old binding.
        // Current schema validation continues to use the current contract.
        if path == self.contract
            && digest.len() == 64
            && digest.bytes().all(|b| b.is_ascii_hexdigit())
        {
            let archive = format!("ToS/contracts/history/{digest}.json");
            return Ok(self.current(&archive, max, deadline)?.filter(|raw| {
                sha(raw) == digest
                    && serde_json::from_slice::<Value>(raw)
                        .ok()
                        .is_some_and(|schema| {
                            schema["$id"] == format!("https://tree-of-sophia.local/{path}")
                        })
            }));
        }
        Ok(None)
    }
    fn schema(
        &mut self,
        _path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
    ) -> std::result::Result<bool, ItemRefusal> {
        self.checkpoint(deadline)?;
        if contract != self.contract {
            return Err(ItemRefusal::Source("unexpected lab contract".into()));
        }
        self.schema
            .is_valid_raw(&format!("https://tree-of-sophia.local/{contract}"), raw)
            .map_err(|e| ItemRefusal::Source(format!("schema: {e:?}")))
    }
    fn generation(&self) -> String {
        self.generation.clone()
    }
    fn checkpoint(&self, deadline: Instant) -> std::result::Result<(), ItemRefusal> {
        if Instant::now() >= deadline {
            return Err(ItemRefusal::Deadline);
        }
        self.ctx.check().map_err(ItemRefusal::Source)
    }
}
pub fn run(ctx: &ResearchExecution, name: &str, build: bool) -> Result<Value> {
    let mut prepared = prepare(ctx, name)?;
    // All bindings and negative controls are resolved before any generated write.
    let mut changes = Vec::new();
    for (reference, raw) in &prepared.files {
        let prior = read_optional(ctx, reference)?;
        if prior.as_deref() != Some(raw) {
            ensure(build, &format!("synthetic fixture stale: {reference}"))?;
            changes.push((reference, raw, prior));
        }
    }
    for (reference, file, digest) in &mut prepared.inputs {
        ensure(
            ctx.hash_file(file, CAP as u64)? == *digest,
            "held lab dependency changed",
        )?;
        let mut current = ctx.source_file(reference, CAP as u64)?;
        ensure(
            ctx.hash_file(&mut current, CAP as u64)? == *digest,
            "lab dependency path changed",
        )?;
    }
    let count = changes.len();
    // Manifest is committed last. A partial interruption remains visibly stale
    // and can be completed by an idempotent repeat using the same recipe.
    changes.sort_by_key(|(reference, _, _)| *reference == &prepared.manifest);
    for (reference, raw, prior) in changes {
        match prior {
            Some(old) => ctx.write_replacing_exact(reference, raw, 0o644, &old)?,
            None => ctx.write(reference, raw, 0o644, true)?,
        }
    }
    Ok(
        json!({"status":"passed","laboratory":name,"records":prepared.files.len(),"written":count,"variants":3,"schema_and_semantic_negative_controls":"passed","source_posture":"public_synthetic_only","human_review_performed":false,"source_admission_performed":false,"canon_effect":false}),
    )
}

fn recipe(name: &str) -> Result<(Recipe, SourceFoundationLab)> {
    let raw = include_bytes!("synthetic_foundation_labs/recipes.v1.json");
    let limits = tos_foundation::JsonLimits::new(CAP, 64, 100_000, 4_300)
        .map_err(|e| format!("recipe limits: {e}"))?;
    tos_foundation::parse_json(raw, tos_foundation::JsonMode::PublishedStrict, limits)
        .map_err(|e| format!("synthetic recipe data: {e}"))?;
    let mut book: RecipeBook = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    ensure(
        book.schema_version == "tos_synthetic_lab_recipes_v1",
        "synthetic recipe version",
    )?;
    let lab = match name {
        "source-text-unit-v1" => SourceFoundationLab::SourceTextUnitV1,
        "translation-alignment-v1" => SourceFoundationLab::TranslationAlignmentV1,
        "semantic-annotation-v2" => SourceFoundationLab::SemanticAnnotationV2,
        _ => return Err("unknown synthetic laboratory".into()),
    };
    let recipe = book
        .recipes
        .remove(name)
        .ok_or("synthetic recipe missing")?;
    for reference in [
        &recipe.legacy,
        &recipe.manifest,
        &recipe.contract,
        &recipe.research,
    ] {
        tos_foundation::RelativePath::parse(reference)
            .map_err(|e| format!("synthetic recipe path: {e}"))?;
    }
    let output_root = std::path::Path::new(&recipe.manifest)
        .parent()
        .ok_or("synthetic manifest parent")?;
    let research_root = std::path::Path::new("ToS/research-packets");
    ensure(
        output_root.starts_with(research_root) && output_root != research_root,
        "synthetic output owner",
    )?;
    let mut references = std::collections::BTreeSet::new();
    for (reference, raw) in &recipe.files {
        tos_foundation::RelativePath::parse(reference)
            .map_err(|e| format!("synthetic output path: {e}"))?;
        ensure(
            std::path::Path::new(reference).starts_with(output_root)
                && raw.len() <= CAP
                && references.insert(reference.as_str()),
            "synthetic output membership or scope",
        )?;
    }
    ensure(
        references.contains(recipe.manifest.as_str()),
        "synthetic manifest template missing",
    )?;
    Ok((recipe, lab))
}
