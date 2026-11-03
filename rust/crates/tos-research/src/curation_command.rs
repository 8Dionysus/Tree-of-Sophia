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
const HELP: &str = "usage: tos-research build|check --curation PATH --stage STAGE [--source-root ABS] [--local-input-root ABS] [--local-output-root ABS] [--tracked-output-root ABS] [--scratch-bytes N] [OPTIONS]\n       tos-research validate --curation PATH [--stage STAGE] [--source-root ABS]\n\nThe selected source root defaults to the current directory. The curation, item, edition and referenced recipes are validated before a stage is selected. Historical command aliases remain available during stage migration.\n";

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
    let (
        mut curation,
        mut stage,
        mut root,
        mut input_root,
        mut output_root,
        mut tracked_root,
        mut scratch,
        mut seconds,
    ) = (None, None, None, None, None, None, None, None);
    let mut options = args.iter().skip(1);
    while let Some(option) = options.next() {
        let slot = match option.as_str() {
            "--curation" => &mut curation,
            "--stage" => &mut stage,
            "--source-root" => &mut root,
            "--local-input-root" => &mut input_root,
            "--local-output-root" => &mut output_root,
            "--tracked-output-root" => &mut tracked_root,
            "--scratch-bytes" => &mut scratch,
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
    if action == "validate"
        && (input_root.is_some()
            || output_root.is_some()
            || tracked_root.is_some()
            || scratch.is_some())
    {
        return Err("local input and output roots apply to check or build".into());
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
    if action != "build" && (tracked_root.is_some() || scratch.is_some()) {
        return Err("tracked output root and scratch apply to build".into());
    }
    let ctx = if action == "build" {
        let bytes = scratch
            .ok_or("build requires --scratch-bytes")?
            .parse::<u64>()
            .map_err(|_| "invalid scratch bytes")?;
        Execution::new_with_scratch(&root, seconds, bytes)?
    } else {
        Execution::new(&root, seconds)?
    };
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
        // The TEI implementation retains its historical plan-specific checks until its
        // record templates and rules are fully selected from curation.
        if matches!(action, "check" | "build") && stage == "tei-technical-markup" {
            let plan_ref = recipe["plan_ref"]
                .as_str()
                .ok_or("TEI plan reference required")?;
            let plan = ctx.read(plan_ref)?;
            edition_curation::validate_recipe_plan(
                &owner.raw,
                stage,
                SelectedDocument {
                    reference: plan_ref,
                    raw: &plan,
                },
                &schema,
            )
            .map_err(|e| format!("TEI plan binding: {e:?}"))?;
            let plan_value: Value =
                serde_json::from_slice(&plan).map_err(|error| error.to_string())?;
            let plan_items = plan_value["source_items"]
                .as_array()
                .ok_or("TEI plan source items required")?;
            let selected_items = recipe["source_item_refs"]
                .as_array()
                .ok_or("TEI selected item references required")?;
            let mut unique = BTreeSet::new();
            if plan_items.len() != selected_items.len()
                || plan_items
                    .iter()
                    .zip(selected_items)
                    .any(|(item, expected)| {
                        item["item_ref"] != *expected
                            || expected
                                .as_str()
                                .is_none_or(|value| !unique.insert(value.to_owned()))
                    })
            {
                return Err("TEI selected items differ from plan or contain duplicates".into());
            }
            let mut frozen = Vec::new();
            for output in recipe["frozen_outputs"]
                .as_array()
                .ok_or("TEI frozen outputs required")?
            {
                let role = output["role"].as_str().ok_or("TEI output role required")?;
                let reference = output["repository_path"]
                    .as_str()
                    .ok_or("TEI output path required")?;
                let raw = ctx.read(reference)?;
                edition_curation::validate_frozen_output_fixity(
                    &owner.raw,
                    stage,
                    role,
                    SelectedDocument {
                        reference,
                        raw: &raw,
                    },
                    &schema,
                )
                .map_err(|e| format!("TEI frozen output: {e:?}"))?;
                frozen.push((reference.to_owned(), raw));
            }
            let input = Path::new(input_root.ok_or("--local-input-root is required")?);
            let output = Path::new(output_root.ok_or("--local-output-root is required")?);
            if !input.is_absolute() || !output.is_absolute() {
                return Err("local roots must be absolute".into());
            }
            let tracked = if action == "build" {
                let path = Path::new(tracked_root.ok_or("build requires --tracked-output-root")?);
                if !path.is_absolute() {
                    return Err("tracked output root must be absolute".into());
                }
                let selected = ctx.select_directory(path)?;
                let tracked_canonical =
                    std::fs::canonicalize(path).map_err(|error| error.to_string())?;
                for other in [&root, input, output] {
                    let other = std::fs::canonicalize(other).map_err(|error| error.to_string())?;
                    if tracked_canonical.starts_with(&other)
                        || other.starts_with(&tracked_canonical)
                    {
                        return Err(
                            "tracked output root must be separate from all selected roots".into(),
                        );
                    }
                }
                use std::os::fd::AsRawFd;
                if std::fs::read_dir(format!(
                    "/proc/self/fd/{}",
                    selected.root_directory().as_raw_fd()
                ))
                .map_err(|error| error.to_string())?
                .next()
                .is_some()
                {
                    return Err("tracked output root must be empty".into());
                }
                Some((path, selected))
            } else {
                None
            };
            let build = action == "build";
            let mut result = crate::tei_technical_markup::run(
                &ctx,
                crate::tei_technical_markup::Options {
                    plan_ref,
                    recipe,
                    executor_label: "tos-research tei-technical-markup",
                    input_root: Some(input),
                    output_root: Some(output),
                    tracked_output: tracked.as_ref().map(|(_, selected)| selected),
                    action: if build {
                        crate::tei_technical_markup::Action::Build {
                            issue_identities: false,
                        }
                    } else {
                        crate::tei_technical_markup::Action::Check
                    },
                    event_id: None,
                },
            )?;
            if build && result["tracked_output_count"].as_u64() != u64::try_from(frozen.len()).ok()
            {
                return Err("rebuilt TEI output membership count differs from curation".into());
            }
            if ctx.read(plan_ref)? != plan {
                return Err("selected TEI plan changed during verification".into());
            }
            for (reference, raw) in frozen {
                if ctx.read(&reference)? != raw {
                    return Err("selected TEI output changed during verification".into());
                }
                if let Some((_, target)) = &tracked {
                    if target.read(&reference)? != raw {
                        return Err("rebuilt TEI output differs from selected source".into());
                    }
                }
            }
            selected.verify_unchanged(&ctx)?;
            for referenced in &refs {
                referenced.verify_unchanged(&ctx)?;
            }
            result["stage"] = serde_json::json!(stage);
            result["curation_ref"] = serde_json::json!(relative);
            let sources = [
                (
                    "rust/crates/tos-research/src/curation_command.rs",
                    include_bytes!("curation_command.rs").as_slice(),
                ),
                (
                    "rust/crates/tos-research/src/tei_technical_markup.rs",
                    include_bytes!("tei_technical_markup.rs").as_slice(),
                ),
                (
                    "rust/crates/tos-research/src/tei_technical_markup/records.rs",
                    include_bytes!("tei_technical_markup/records.rs").as_slice(),
                ),
                (
                    "rust/crates/tos-research/src/navigation_xml.rs",
                    include_bytes!("navigation_xml.rs").as_slice(),
                ),
                (
                    "rust/crates/tos-research/src/research_support.rs",
                    include_bytes!("research_support.rs").as_slice(),
                ),
            ];
            result["current_execution"] = serde_json::json!({
                "embedded_source_digests": sources.iter().map(|(reference, bytes)| serde_json::json!({"ref":reference,"sha256":tos_foundation::Digest256::of_bytes(bytes).to_hex()})).collect::<Vec<_>>(),
                "curation": {"ref": owner.reference, "sha256": tos_foundation::Digest256::of_bytes(&owner.raw).to_hex()},
                "plan": {"ref": plan_ref, "sha256": tos_foundation::Digest256::of_bytes(&plan).to_hex()},
            });
            result["implementation"] = serde_json::json!("retained-plan-specific-executor");
            if build {
                result["replay"] = serde_json::json!("historical-source-bound");
            }
            writeln!(out, "{result}").map_err(|error| error.to_string())?;
            return Ok(());
        }
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
