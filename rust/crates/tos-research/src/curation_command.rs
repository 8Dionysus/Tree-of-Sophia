//! Source-selected curation and stage entry for research producers.
use serde_json::Value;
use std::{
    collections::BTreeSet,
    io::Write,
    path::{Path, PathBuf},
};
use tos_source_store::execution::Execution;
use tos_validation::{
    FormatProfile, SchemaBackendProbe, SchemaResource,
    edition_curation::{self, SCHEMA_URI, SelectedCuration, SelectedDocument},
};

const SCHEMA_PATH: &str = "ToS/contracts/edition-curation.v1.schema.json";
const HELP: &str = "usage: tos-research build|check --curation PATH --stage STAGE [--source-root ABS] [--local-input-root ABS] [OPTIONS]\n       tos-research validate --curation PATH [--stage STAGE] [--source-root ABS]\n\nThe selected source root defaults to the current directory. The curation, item, edition and referenced recipes are validated before a stage is selected. Historical command aliases remain available during stage migration.\n";

struct Curation {
    reference: String,
    raw: Vec<u8>,
    manifest_ref: String,
    manifest: Vec<u8>,
    edition_ref: String,
    edition: Vec<u8>,
    value: Value,
}
impl Curation {
    fn verify_unchanged(&self, ctx: &Execution) -> Result<(), String> {
        for (reference, before) in [
            (&self.reference, &self.raw),
            (&self.manifest_ref, &self.manifest),
            (&self.edition_ref, &self.edition),
        ] {
            if ctx.read(reference)? != *before {
                return Err(format!("selected source changed: {reference}"));
            }
        }
        Ok(())
    }
    fn selected(&self) -> SelectedCuration<'_> {
        SelectedCuration {
            curation: SelectedDocument {
                reference: &self.reference,
                raw: &self.raw,
            },
            manifest: SelectedDocument {
                reference: &self.manifest_ref,
                raw: &self.manifest,
            },
            edition: SelectedDocument {
                reference: &self.edition_ref,
                raw: &self.edition,
            },
        }
    }
}

fn select_curation(
    ctx: &Execution,
    reference: &str,
    schema: &SchemaBackendProbe,
) -> Result<Curation, String> {
    tos_foundation::RelativePath::parse(reference).map_err(|e| e.to_string())?;
    let raw = ctx.read(reference)?;
    edition_curation::validate(&raw, schema).map_err(|e| format!("invalid curation: {e:?}"))?;
    let value: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let parent = Path::new(reference)
        .parent()
        .ok_or("curation parent required")?;
    let manifest_name = value["manifest"]
        .as_str()
        .ok_or("curation manifest required")?;
    let manifest_ref = parent
        .join(manifest_name)
        .to_str()
        .ok_or("manifest path must be UTF-8")?
        .to_owned();
    let edition = parent
        .parent()
        .and_then(Path::parent)
        .ok_or("curation edition directory required")?;
    let edition_ref = edition
        .join("edition.json")
        .to_str()
        .ok_or("edition path must be UTF-8")?
        .to_owned();
    let manifest = ctx.read(&manifest_ref)?;
    let edition = ctx.read(&edition_ref)?;
    Ok(Curation {
        reference: reference.into(),
        raw,
        manifest_ref,
        manifest,
        edition_ref,
        edition,
        value,
    })
}

fn references(value: &Value, current: &str) -> Result<BTreeSet<String>, String> {
    let mut refs = BTreeSet::new();
    for recipe in value["recipes"]
        .as_array()
        .ok_or("curation recipes required")?
    {
        if recipe["stage"] == "recipe-reference" {
            refs.insert(
                recipe["curation_ref"]
                    .as_str()
                    .ok_or("curation reference required")?
                    .to_owned(),
            );
        }
        if recipe["stage"] == "source-passages" {
            refs.insert(
                recipe["marker_profile"]["normalization_curation_ref"]
                    .as_str()
                    .ok_or("normalization curation required")?
                    .to_owned(),
            );
            for route in recipe["work_routes"]
                .as_array()
                .ok_or("work routes required")?
            {
                refs.insert(
                    route["marker_curation_ref"]
                        .as_str()
                        .ok_or("marker curation required")?
                        .to_owned(),
                );
            }
        }
    }
    refs.remove(current);
    Ok(refs)
}

fn execute(args: &[String], out: &mut dyn Write) -> Result<(), String> {
    let action = args[0].as_str();
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        return write!(out, "{HELP}").map_err(|e| e.to_string());
    }
    let (mut curation, mut stage, mut root, mut input_root, mut seconds) =
        (None, None, None, None, None);
    let mut options = args.iter().skip(1);
    while let Some(option) = options.next() {
        let slot = match option.as_str() {
            "--curation" => &mut curation,
            "--stage" => &mut stage,
            "--source-root" => &mut root,
            "--local-input-root" => &mut input_root,
            "--max-seconds" => &mut seconds,
            _ => return Err(format!("unknown curation option: {option}")),
        };
        let value = options
            .next()
            .ok_or_else(|| format!("{option} requires a value"))?;
        if slot.replace(value.as_str()).is_some() {
            return Err(format!("duplicate option: {option}"));
        }
    }
    let curation = curation.ok_or("--curation is required")?;
    if action == "validate" && input_root.is_some() {
        return Err("--local-input-root applies to check or build".into());
    }
    if action != "validate" && stage.is_none() {
        return Err("--stage is required".into());
    }
    let root = match root {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir().map_err(|e| e.to_string())?,
    };
    if !root.is_absolute() {
        return Err("source root must be absolute".into());
    }
    let seconds = seconds
        .unwrap_or("180")
        .parse::<u64>()
        .map_err(|_| "invalid --max-seconds")?;
    let ctx = Execution::new(&root, seconds)?;
    let path = Path::new(curation);
    let relative = if path.is_absolute() {
        path.strip_prefix(&root)
            .map_err(|_| "curation must belong to source root")?
    } else {
        path
    };
    let relative = relative.to_str().ok_or("curation path must be UTF-8")?;
    tos_foundation::RelativePath::parse(relative).map_err(|e| e.to_string())?;
    let schema = SchemaBackendProbe::new(
        [SchemaResource {
            uri: SCHEMA_URI.into(),
            raw: ctx.read(SCHEMA_PATH)?,
        }],
        FormatProfile::AssertedSourceCandidateV1,
    )
    .map_err(|e| format!("curation schema: {e:?}"))?;
    let selected = select_curation(&ctx, relative, &schema)?;
    let refs = references(&selected.value, relative)?
        .into_iter()
        .map(|reference| select_curation(&ctx, &reference, &schema))
        .collect::<Result<Vec<_>, _>>()?;
    let bindings = refs.iter().map(Curation::selected).collect::<Vec<_>>();
    let chosen = selected.selected();
    edition_curation::validate_bindings(
        chosen.curation,
        chosen.manifest,
        chosen.edition,
        &bindings,
        &schema,
    )
    .map_err(|e| format!("curation binding: {e:?}"))?;
    if let Some(stage) = stage {
        let recipes = selected.value["recipes"]
            .as_array()
            .ok_or("curation recipes required")?;
        let choices = recipes
            .iter()
            .filter(|recipe| {
                recipe["stage"] == stage
                    || (recipe["stage"] == "recipe-reference" && recipe["recipe_stage"] == stage)
            })
            .collect::<Vec<_>>();
        if choices.len() != 1 {
            return Err(format!(
                "stage {stage} must identify exactly one selected recipe"
            ));
        }
        let (owner, recipe) = if choices[0]["stage"] == "recipe-reference" {
            let reference = choices[0]["curation_ref"]
                .as_str()
                .ok_or("curation reference required")?;
            let owner = refs
                .iter()
                .find(|value| value.reference == reference)
                .ok_or("referenced curation missing")?;
            let recipe = owner.value["recipes"]
                .as_array()
                .ok_or("curation recipes required")?
                .iter()
                .find(|recipe| recipe["stage"] == stage)
                .ok_or("referenced stage missing")?;
            (owner, recipe)
        } else {
            (&selected, choices[0])
        };
        if action == "check" && stage == "numbered-structure" {
            let input = Path::new(input_root.ok_or("--local-input-root is required")?);
            let result = crate::numbered_check::check(
                &ctx,
                &owner.reference,
                &owner.raw,
                &owner.value,
                recipe,
                &owner.manifest,
                input,
                &schema,
            )?;
            selected.verify_unchanged(&ctx)?;
            for referenced in &refs {
                referenced.verify_unchanged(&ctx)?;
            }
            writeln!(out, "{result}").map_err(|error| error.to_string())?;
            return Ok(());
        }
        if action == "validate" && stage == "numbered-structure" {
            crate::numbered_structure::validate_frozen_map(
                &ctx,
                &owner.raw,
                &owner.value,
                recipe,
                &schema,
            )?;
        }
    }
    if action != "validate" {
        return Err(format!(
            "stage {} is awaiting migration; historical command aliases remain available",
            stage.unwrap()
        ));
    }
    let result = serde_json::json!({"status":"validated","curation_ref":relative,"stage":stage});
    writeln!(out, "{result}").map_err(|e| e.to_string())?;
    Ok(())
}

pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if !args
        .first()
        .is_some_and(|action| matches!(action.as_str(), "build" | "check" | "validate"))
    {
        return None;
    }
    Some(match execute(args, out) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(err, "curation: {error}");
            2
        }
    })
}
