//! Source-selected curation and stage entry for research producers.
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::{Path, PathBuf},
};
use tos_source_store::execution::Execution;
use tos_validation::{
    FormatProfile, SchemaBackendProbe, SchemaResource,
    edition_curation::{self, SCHEMA_URI, SelectedCuration, SelectedDocument},
};

const SCHEMA_PATH: &str = "ToS/contracts/edition-curation.v1.schema.json";
const HELP: &str = "usage: tos-research build|check --curation PATH --stage STAGE [--source-root ABS] [--local-input-root ABS] [--boundary-root ABS] [--local-output-root ABS] [--tracked-output-root ABS] [--scratch-bytes N] [OPTIONS]\n       tos-research validate --curation PATH [--stage STAGE] [--source-root ABS]\n\nThe selected source root defaults to the current directory. The curation, item, edition and referenced recipes are validated before a stage is selected. Historical command aliases remain available during stage migration.\n";

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

fn select_empty_output(
    ctx: &Execution,
    path: &Path,
    others: &[&Path],
) -> Result<Execution, String> {
    use std::os::{fd::AsRawFd, unix::fs::MetadataExt};
    if !path.is_absolute() {
        return Err("tracked output root must be absolute".into());
    }
    let selected = ctx.select_directory(path)?;
    let canonical = std::fs::canonicalize(path).map_err(|error| error.to_string())?;
    let resolved = std::fs::metadata(&canonical).map_err(|error| error.to_string())?;
    let held = selected
        .root_directory()
        .metadata()
        .map_err(|error| error.to_string())?;
    if resolved.dev() != held.dev() || resolved.ino() != held.ino() {
        return Err("tracked output root changed during selection".into());
    }
    for other in others {
        let other = std::fs::canonicalize(other).map_err(|error| error.to_string())?;
        if canonical.starts_with(&other) || other.starts_with(&canonical) {
            return Err("tracked output root must be separate from all selected roots".into());
        }
    }
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
    Ok(selected)
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

fn references(
    value: &Value,
    current: &str,
    stage: Option<&str>,
) -> Result<BTreeSet<String>, String> {
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
        if recipe["stage"] == "paragraph-alignment"
            && stage.is_none_or(|selected| selected == "paragraph-alignment")
        {
            refs.insert(
                recipe["target_curation_ref"]
                    .as_str()
                    .ok_or("paragraph target curation required")?
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

#[cfg(test)]
mod reference_tests {
    use super::*;

    #[test]
    fn paragraph_target_reference_is_selected_only_for_its_stage_or_full_validation() {
        let value = serde_json::json!({"recipes":[{"stage":"paragraph-alignment","target_curation_ref":"target/edition-curation.v1.json"}]});
        let expected = BTreeSet::from(["target/edition-curation.v1.json".to_owned()]);
        assert_eq!(
            references(&value, "current/edition-curation.v1.json", None).unwrap(),
            expected
        );
        assert_eq!(
            references(
                &value,
                "current/edition-curation.v1.json",
                Some("paragraph-alignment")
            )
            .unwrap(),
            expected
        );
        assert!(
            references(
                &value,
                "current/edition-curation.v1.json",
                Some("tei-technical-markup")
            )
            .unwrap()
            .is_empty()
        );
    }
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
        mut boundary_root,
        mut output_root,
        mut tracked_root,
        mut scratch,
        mut seconds,
    ) = (None, None, None, None, None, None, None, None, None);
    let mut options = args.iter().skip(1);
    while let Some(option) = options.next() {
        let slot = match option.as_str() {
            "--curation" => &mut curation,
            "--stage" => &mut stage,
            "--source-root" => &mut root,
            "--local-input-root" => &mut input_root,
            "--boundary-root" => &mut boundary_root,
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
            || boundary_root.is_some()
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
    let refs = references(&selected.value, relative, stage)?
        .into_iter()
        .map(|reference| select_curation(&ctx, &reference, &schema))
        .collect::<Result<Vec<_>, _>>()?;
    let bindings = refs.iter().map(Curation::selected).collect::<Vec<_>>();
    let chosen = selected.selected();
    edition_curation::validate_bindings_for_stage(
        chosen.curation,
        chosen.manifest,
        chosen.edition,
        &bindings,
        &schema,
        stage,
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
                    || (recipe["stage"] == "pdf-structural-paragraph"
                        && recipe["legacy_layout"]["stage"] == stage)
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
        if boundary_root.is_some() && stage != "sentence-alignment" {
            return Err("boundary root applies to sentence alignment".into());
        }
        if matches!(action, "check" | "build") && stage == "lexical-recurrence" {
            if input_root.is_some() {
                return Err("lexical recurrence reads the selected source cut".into());
            }
            let output =
                Path::new(output_root.ok_or("lexical recurrence requires --local-output-root")?);
            if !output.is_absolute() {
                return Err("lexical recurrence output root must be absolute".into());
            }
            let target = if action == "build" {
                let path = Path::new(tracked_root.ok_or("build requires --tracked-output-root")?);
                Some(select_empty_output(&ctx, path, &[&root, output])?)
            } else {
                None
            };
            let plan_ref = recipe["plan_ref"]
                .as_str()
                .ok_or("recurrence plan required")?;
            let plan_raw = ctx.read(plan_ref)?;
            let digest = |raw: &[u8]| tos_foundation::Digest256::of_bytes(raw).to_hex();
            if recipe["plan_sha256"] != digest(&plan_raw) {
                return Err("recurrence plan fixity differs".into());
            }
            let plan: Value = serde_json::from_slice(&plan_raw).map_err(|e| e.to_string())?;
            let source_ref = plan["source_projection"]["ref"]
                .as_str()
                .ok_or("recurrence source reference required")?;
            let source_raw = ctx.read(source_ref)?;
            if plan["source_projection"]["sha256"] != digest(&source_raw) {
                return Err("recurrence source fixity differs".into());
            }
            let source: Value = serde_json::from_slice(&source_raw).map_err(|e| e.to_string())?;
            let source_items = source["source_items"]
                .as_array()
                .ok_or("recurrence source items required")?
                .iter()
                .map(|item| {
                    item["item_ref"]
                        .as_str()
                        .ok_or("recurrence source item required")
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
            let selected_items = recipe["source_item_refs"]
                .as_array()
                .ok_or("recurrence selected items required")?
                .iter()
                .map(|item| item.as_str().ok_or("recurrence selected item required"))
                .collect::<Result<BTreeSet<_>, _>>()?;
            if source_items != selected_items {
                return Err("recurrence selected source items differ".into());
            }
            let mut expected = BTreeMap::new();
            for artifact in recipe["retained_artifacts"]
                .as_array()
                .ok_or("recurrence artifacts required")?
            {
                let role = artifact["role"]
                    .as_str()
                    .ok_or("recurrence artifact role required")?;
                let pointer = artifact["plan_pointer"]
                    .as_str()
                    .ok_or("recurrence artifact pointer required")?;
                let reference = plan
                    .pointer(pointer)
                    .and_then(Value::as_str)
                    .ok_or("recurrence plan output missing")?;
                let raw = ctx.read(reference)?;
                if artifact["sha256"] != digest(&raw)
                    || expected
                        .insert(role, (reference.to_owned(), digest(&raw)))
                        .is_some()
                {
                    return Err("recurrence artifact fixity or role differs".into());
                }
                if role == "projection" {
                    let projection: Value =
                        serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
                    let identities = recipe["record_identities"]
                        .as_array()
                        .ok_or("recurrence identities required")?;
                    if identities.len() != 1
                        || identities[0]["role"] != "projection"
                        || identities[0]["generation_policy"] != "native-generation"
                        || projection["projection_id"] != identities[0]["base_id"]
                    {
                        return Err("recurrence projection identity differs".into());
                    }
                }
            }
            if expected.keys().copied().collect::<BTreeSet<_>>()
                != BTreeSet::from(["event", "projection"])
            {
                return Err("recurrence artifact roles differ".into());
            }
            // Preserve the retained producer profile until the compatibility aliases retire.
            let options = crate::lexical_recurrence::Options {
                history: Some(crate::lexical_recurrence::History {
                    projection_sha256: &expected["projection"].1,
                    event_sha256: &expected["event"].1,
                    projection_id: recipe["record_identities"][0]["base_id"]
                        .as_str()
                        .ok_or("recurrence projection identity required")?,
                }),
                build: false,
                output_root: output,
                plan: plan_ref,
                generation: None,
                event_at: None,
            };
            let result = if let Some(target) = target.as_ref() {
                crate::lexical_recurrence::replay_selected(&ctx, options, target)?
            } else {
                crate::lexical_recurrence::run(&ctx, options)?
            };
            if result["projection_sha256"] != expected["projection"].1
                || result["provenance_sha256"] != expected["event"].1
                || ctx.read(plan_ref)? != plan_raw
                || ctx.read(source_ref)? != source_raw
            {
                return Err("recurrence selected sources or results changed".into());
            }
            selected.verify_unchanged(&ctx)?;
            for referenced in &refs {
                referenced.verify_unchanged(&ctx)?;
            }
            let mut result = result;
            result["stage"] = serde_json::json!(stage);
            result["curation_ref"] = serde_json::json!(relative);
            result["current_curation_sha256"] = serde_json::json!(digest(&owner.raw));
            result["current_execution_source_sha256"] =
                serde_json::json!(digest(include_bytes!("curation_command.rs")));
            result["current_producer_sha256"] =
                serde_json::json!(digest(include_bytes!("lexical_recurrence.rs")));
            result["current_custody_sha256"] =
                serde_json::json!(digest(include_bytes!("lexical_recurrence_support.rs")));
            result["current_rational_sha256"] = serde_json::json!(digest(include_bytes!(
                "../../tos-foundation/src/rational.rs"
            )));
            writeln!(out, "{result}").map_err(|error| error.to_string())?;
            return Ok(());
        }
        if matches!(action, "check" | "build") && stage == "hierarchical-source-structure" {
            if output_root.is_some() {
                return Err(
                    "hierarchical source structure writes selected text-free outputs only".into(),
                );
            }
            let input = Path::new(
                input_root.ok_or("hierarchical source structure requires --local-input-root")?,
            );
            if !input.is_absolute() {
                return Err("hierarchical source input root must be absolute".into());
            }
            let fields: BTreeSet<_> = recipe
                .as_object()
                .ok_or("hierarchical source recipe required")?
                .keys()
                .map(String::as_str)
                .collect();
            if fields
                != BTreeSet::from([
                    "stage",
                    "configuration",
                    "tools",
                    "historical_text",
                    "candidate_geometry",
                    "ocr_substitutions",
                    "identity",
                    "frozen_outputs",
                ])
            {
                return Err("unsupported hierarchical source recipe field".into());
            }
            let mut expected = BTreeMap::new();
            let mut frozen = Vec::new();
            for output in recipe["frozen_outputs"]
                .as_array()
                .ok_or("hierarchical source outputs required")?
            {
                let role = output["role"]
                    .as_str()
                    .ok_or("hierarchical source output role required")?;
                let reference = output["repository_path"]
                    .as_str()
                    .ok_or("hierarchical source output path required")?;
                let digest = output["expected_sha256"]
                    .as_str()
                    .ok_or("hierarchical source output digest required")?;
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
                .map_err(|e| format!("hierarchical source frozen output: {e:?}"))?;
                if expected
                    .insert(reference.to_owned(), digest.to_owned())
                    .is_some()
                {
                    return Err("duplicate hierarchical source output".into());
                }
                frozen.push((reference.to_owned(), raw));
            }
            let target = if action == "build" {
                let path = Path::new(tracked_root.ok_or("build requires --tracked-output-root")?);
                Some(select_empty_output(&ctx, path, &[&root, input])?)
            } else {
                None
            };
            let mut result = tos_compiler::nietzsche_transfer_source_structure::replay_selected(
                &ctx,
                input,
                &recipe["configuration"],
                &recipe["candidate_geometry"],
                &recipe["ocr_substitutions"],
                &recipe["identity"],
                &recipe["historical_text"],
                &recipe["tools"],
                &expected,
                target.as_ref(),
            )?;
            if target.is_some() {
                result["status"] = serde_json::json!("built");
                result["outputs_written"] = serde_json::json!(expected.len());
            }
            for (reference, raw) in frozen {
                if ctx.read(&reference)? != raw {
                    return Err("hierarchical source frozen output changed during replay".into());
                }
            }
            selected.verify_unchanged(&ctx)?;
            for referenced in &refs {
                referenced.verify_unchanged(&ctx)?;
            }
            result["stage"] = serde_json::json!(stage);
            result["curation_ref"] = serde_json::json!(relative);
            result["current_curation_sha256"] =
                serde_json::json!(tos_foundation::Digest256::of_bytes(&owner.raw).to_hex());
            result["current_execution_sources"] = serde_json::json!({
                "producer":tos_foundation::Digest256::of_bytes(include_bytes!("../../tos-compiler/src/nietzsche_transfer_source_structure.rs")).to_hex(),
                "records":tos_foundation::Digest256::of_bytes(include_bytes!("../../tos-compiler/src/nietzsche_transfer_source_structure/records.rs")).to_hex(),
                "constants":tos_foundation::Digest256::of_bytes(include_bytes!("../../tos-compiler/src/nietzsche_transfer_source_structure/constants.rs")).to_hex(),
                "compatibility_config":tos_foundation::Digest256::of_bytes(include_bytes!("../../tos-compiler/src/nietzsche_transfer_source_structure/config.json")).to_hex()
            });
            writeln!(out, "{result}").map_err(|error| error.to_string())?;
            return Ok(());
        }
        if matches!(action, "check" | "build") && stage == "sentence-alignment" {
            if output_root.is_some() {
                return Err("sentence alignment check uses retained outputs".into());
            }
            let plan_ref = recipe["plan_ref"]
                .as_str()
                .ok_or("sentence plan reference required")?;
            let plan_raw = ctx.read(plan_ref)?;
            edition_curation::validate_recipe_plan(
                &owner.raw,
                stage,
                SelectedDocument {
                    reference: plan_ref,
                    raw: &plan_raw,
                },
                &schema,
            )
            .map_err(|e| format!("sentence plan binding: {e:?}"))?;
            let plan: Value = serde_json::from_slice(&plan_raw).map_err(|e| e.to_string())?;
            let items = recipe["source_item_refs"]
                .as_array()
                .ok_or("sentence items required")?;
            if items.len() != 2
                || items[0] != plan["source"]["item_ref"]
                || items[1] != plan["target"]["item_ref"]
            {
                return Err("sentence plan items differ from curation".into());
            }
            let mut frozen = Vec::new();
            let mut roles = BTreeSet::new();
            for output in recipe["frozen_outputs"]
                .as_array()
                .ok_or("sentence outputs required")?
            {
                let role = output["role"]
                    .as_str()
                    .ok_or("sentence output role required")?;
                let reference = output["repository_path"]
                    .as_str()
                    .ok_or("sentence output path required")?;
                if !roles.insert(role) || plan["outputs"][role] != reference {
                    return Err("sentence output selection differs from plan".into());
                }
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
                .map_err(|e| format!("sentence frozen output: {e:?}"))?;
                frozen.push((reference.to_owned(), raw));
            }
            let expected: BTreeSet<_> = [
                "source_sentence_packet_ref",
                "target_sentence_packet_ref",
                "alignment_packet_ref",
                "provenance_event_ref",
            ]
            .into_iter()
            .collect();
            if roles != expected {
                return Err("sentence output membership differs from plan".into());
            }
            let input = Path::new(input_root.ok_or("--local-input-root is required")?);
            let boundary = boundary_root.map(Path::new).unwrap_or(&root);
            if !input.is_absolute() || !boundary.is_absolute() {
                return Err("local roots must be absolute".into());
            }
            let target = if action == "build" {
                let path = Path::new(tracked_root.ok_or("build requires --tracked-output-root")?);
                Some(select_empty_output(&ctx, path, &[&root, input, boundary])?)
            } else {
                None
            };
            let argv = serde_json::json!(std::env::args().collect::<Vec<_>>());
            let mut result = crate::sentence_alignment::run(
                &ctx,
                crate::sentence_alignment::Options {
                    plan_ref,
                    input_root: input,
                    boundary_root: boundary,
                    tracked_output: target.as_ref(),
                    build: action == "build",
                    event_id: None,
                    argv: &argv,
                    history: crate::sentence_alignment::History {
                        agent_ref: recipe["agent_ref"]
                            .as_str()
                            .ok_or("sentence agent required")?,
                        plan_schema_version: recipe["plan_schema_version"]
                            .as_str()
                            .ok_or("sentence plan schema required")?,
                        builder: recipe["legacy_builder_ref"]
                            .as_str()
                            .ok_or("historical builder required")?,
                        event: recipe["event_id"]
                            .as_str()
                            .ok_or("historical event required")?,
                        builder_sha256: recipe["legacy_builder_sha256"]
                            .as_str()
                            .ok_or("historical builder digest required")?,
                        derivation_prefix: recipe["derivation_prefix"]
                            .as_str()
                            .ok_or("derivation prefix required")?,
                    },
                },
            )?;
            for (reference, raw) in frozen {
                if ctx.read(&reference)? != raw {
                    return Err("selected sentence output changed during verification".into());
                }
                if let Some(target) = &target {
                    if target.read(&reference)? != raw {
                        return Err("rebuilt sentence output differs from selected source".into());
                    }
                }
            }
            selected.verify_unchanged(&ctx)?;
            for referenced in &refs {
                referenced.verify_unchanged(&ctx)?;
            }
            result["stage"] = serde_json::json!(stage);
            result["curation_ref"] = serde_json::json!(relative);
            result["current_curation_sha256"] =
                serde_json::json!(tos_foundation::Digest256::of_bytes(&owner.raw).to_hex());
            result["current_plan_sha256"] =
                serde_json::json!(tos_foundation::Digest256::of_bytes(&plan_raw).to_hex());
            result["current_execution_sources"] = serde_json::json!({
                "sentence_alignment": tos_foundation::Digest256::of_bytes(include_bytes!("sentence_alignment.rs")).to_hex(),
                "sentence_records": tos_foundation::Digest256::of_bytes(include_bytes!("sentence_alignment/records.rs")).to_hex(),
            });
            if action == "build" {
                result["replay"] = serde_json::json!("historical-source-bound");
            }
            writeln!(out, "{result}").map_err(|error| error.to_string())?;
            return Ok(());
        }
        if matches!(action, "check" | "build") && stage == "paragraph-alignment" {
            if input_root.is_some() || output_root.is_some() {
                return Err("paragraph alignment uses selected source custody".into());
            }
            let target_ref = recipe["target_curation_ref"]
                .as_str()
                .ok_or("paragraph target curation required")?;
            let target_curation = refs
                .iter()
                .find(|r| r.reference == target_ref)
                .ok_or("paragraph target curation missing")?;
            let structures: Vec<_> = target_curation.value["recipes"]
                .as_array()
                .ok_or("target recipes required")?
                .iter()
                .filter(|r| r["stage"] == "pdf-structural-paragraph")
                .collect();
            if structures.len() != 1 {
                return Err("target structural recipe must be unique".into());
            }
            let structure = structures[0];
            let target_plan_ref = format!(
                "{}/plan.v1.json",
                structure["layout_plan_dir"]
                    .as_str()
                    .ok_or("target layout plan directory required")?
            );
            let target_plan = ctx.read(&target_plan_ref)?;
            let target_plan_sha = tos_foundation::Digest256::of_bytes(&target_plan).to_hex();
            let target_plan_value: Value =
                serde_json::from_slice(&target_plan).map_err(|e| e.to_string())?;
            if structure["layout_plan_sha256"] != target_plan_sha
                || target_plan_value["source_item"]["item_ref"]
                    != target_curation.value["owner_item_ref"]
                || target_plan_value["source_item"]["edition_ref"]
                    != target_curation.value["edition_ref"]
            {
                return Err("paragraph target structural plan binding differs".into());
            }
            let template_ref = recipe["historical_template_ref"]
                .as_str()
                .ok_or("paragraph template required")?;
            let template = ctx.read(template_ref)?;
            edition_curation::validate_recipe_template(
                &owner.raw,
                stage,
                "historical-template",
                SelectedDocument {
                    reference: template_ref,
                    raw: &template,
                },
                &schema,
            )
            .map_err(|e| format!("paragraph template binding: {e:?}"))?;
            let manifest_ref = recipe["frozen_manifest"]["ref"]
                .as_str()
                .ok_or("paragraph manifest required")?;
            let manifest = ctx.read(manifest_ref)?;
            let selected_manifest = edition_curation::select_frozen_manifest(
                &owner.raw,
                stage,
                SelectedDocument {
                    reference: manifest_ref,
                    raw: &manifest,
                },
                &schema,
            )
            .map_err(|e| format!("paragraph manifest binding: {e:?}"))?;
            let members = selected_manifest["generated_outputs"]
                .as_object()
                .ok_or("paragraph manifest outputs required")?;
            let mut expected = BTreeSet::new();
            for reference in members.keys() {
                let raw = ctx.read(reference)?;
                edition_curation::validate_frozen_manifest_member(
                    &owner.raw,
                    stage,
                    SelectedDocument {
                        reference: manifest_ref,
                        raw: &manifest,
                    },
                    SelectedDocument {
                        reference,
                        raw: &raw,
                    },
                    &schema,
                )
                .map_err(|e| format!("paragraph output binding: {e:?}"))?;
                expected.insert(reference.clone());
            }
            if !expected.insert(manifest_ref.to_owned()) {
                return Err("paragraph manifest aliases generated output".into());
            }
            let target = if action == "build" {
                let path = Path::new(tracked_root.ok_or("build requires --tracked-output-root")?);
                Some(select_empty_output(&ctx, path, &[&root])?)
            } else {
                None
            };
            let mut result = crate::paragraph_alignment::replay_selected(
                &ctx,
                recipe,
                &target_plan_ref,
                &expected,
                target.as_ref(),
            )?;
            if ctx.read(template_ref)? != template
                || ctx.read(manifest_ref)? != manifest
                || ctx.read(&target_plan_ref)? != target_plan
            {
                return Err("paragraph curation inputs changed during replay".into());
            }
            selected.verify_unchanged(&ctx)?;
            for referenced in &refs {
                referenced.verify_unchanged(&ctx)?;
            }
            result["stage"] = serde_json::json!(stage);
            result["curation_ref"] = serde_json::json!(relative);
            result["current_curation_sha256"] =
                serde_json::json!(tos_foundation::Digest256::of_bytes(&owner.raw).to_hex());
            result["target_curation_sha256"] = serde_json::json!(
                tos_foundation::Digest256::of_bytes(&target_curation.raw).to_hex()
            );
            result["target_plan_sha256"] = serde_json::json!(target_plan_sha);
            result["current_execution_sources"] = serde_json::json!({"paragraph_alignment":tos_foundation::Digest256::of_bytes(include_bytes!("paragraph_alignment.rs")).to_hex(),"paragraph_template":tos_foundation::Digest256::of_bytes(include_bytes!("paragraph_alignment_templates.json")).to_hex()});
            writeln!(out, "{result}").map_err(|error| error.to_string())?;
            return Ok(());
        }
        if matches!(action, "check" | "build") && stage == "pdf-technical-markup" {
            if input_root.is_some() || output_root.is_some() {
                return Err("retained PDF layout uses the selected source root".into());
            }
            let plan_raw = crate::legacy_pdf_layout::validate_recipe(&ctx, recipe)?;
            let plan: Value = serde_json::from_slice(&plan_raw).map_err(|e| e.to_string())?;
            let history = &recipe["legacy_layout"];
            let maintained_ref = history["maintained_recipe_ref"]
                .as_str()
                .ok_or("retained recipe required")?;
            let maintained = ctx.read(maintained_ref)?;
            if tos_foundation::Digest256::of_bytes(&maintained).to_hex()
                != history["maintained_recipe_sha256"]
            {
                return Err("retained PDF builder digest differs from curation".into());
            }
            let mut frozen = std::collections::BTreeMap::new();
            for entry in history["frozen_outputs"]
                .as_array()
                .ok_or("PDF layout outputs required")?
            {
                let role = entry["role"]
                    .as_str()
                    .ok_or("PDF layout output role required")?;
                let reference = entry["repository_path"]
                    .as_str()
                    .ok_or("PDF layout output reference required")?;
                if plan["outputs"][role] != reference {
                    return Err("PDF layout output differs from plan".into());
                }
                let raw = ctx.read(reference)?;
                if tos_foundation::Digest256::of_bytes(&raw).to_hex() != entry["expected_sha256"]
                    || frozen.insert(reference.to_owned(), raw).is_some()
                {
                    return Err("PDF layout output digest or membership drift".into());
                }
            }
            let target = if action == "build" {
                let path = Path::new(tracked_root.ok_or("build requires --tracked-output-root")?);
                Some(select_empty_output(&ctx, path, &[&root])?)
            } else {
                None
            };
            let mut result =
                tos_compiler::antonovsky_structural::technical_markup::replay_selected(
                    &ctx,
                    target.as_ref(),
                )?;
            if result["tracked_output_count"].as_u64() != u64::try_from(frozen.len()).ok() {
                return Err("PDF layout output count differs from curation".into());
            }
            for (reference, raw) in frozen {
                if ctx.read(&reference)? != raw {
                    return Err("selected PDF layout output changed during replay".into());
                }
                if let Some(target) = &target {
                    if target.read(&reference)? != raw {
                        return Err("rebuilt PDF layout output differs from source".into());
                    }
                }
            }
            if ctx.read(maintained_ref)? != maintained {
                return Err("retained PDF builder changed during replay".into());
            }
            selected.verify_unchanged(&ctx)?;
            for referenced in &refs {
                referenced.verify_unchanged(&ctx)?;
            }
            result["stage"] = serde_json::json!(stage);
            result["producer"] = serde_json::json!("retained-compiler-pdf-layout");
            result["curation_ref"] = serde_json::json!(relative);
            result["current_curation_sha256"] =
                serde_json::json!(tos_foundation::Digest256::of_bytes(&owner.raw).to_hex());
            writeln!(out, "{result}").map_err(|error| error.to_string())?;
            return Ok(());
        }
        if matches!(action, "check" | "build") && stage == "pdf-structural-paragraph" {
            if input_root.is_some() || output_root.is_some() {
                return Err("retained PDF producer uses the selected source root".into());
            }
            let target = if action == "build" {
                let path = Path::new(tracked_root.ok_or("build requires --tracked-output-root")?);
                Some(select_empty_output(&ctx, path, &[&root])?)
            } else {
                None
            };
            let mut result = crate::legacy_pdf_layout::replay(&ctx, recipe, target.as_ref())?;
            selected.verify_unchanged(&ctx)?;
            for referenced in &refs {
                referenced.verify_unchanged(&ctx)?;
            }
            result["stage"] = serde_json::json!(stage);
            result["curation_ref"] = serde_json::json!(relative);
            result["current_curation_sha256"] =
                serde_json::json!(tos_foundation::Digest256::of_bytes(&owner.raw).to_hex());
            writeln!(out, "{result}").map_err(|error| error.to_string())?;
            return Ok(());
        }
        if matches!(action, "check" | "build") && stage == "structural-source-routes" {
            if input_root.is_some() || output_root.is_some() {
                return Err("structural source routes use selected source metadata only".into());
            }
            let mut frozen = Vec::new();
            for output in recipe["frozen_outputs"]
                .as_array()
                .ok_or("structural frozen outputs required")?
            {
                let role = output["role"]
                    .as_str()
                    .ok_or("structural output role required")?;
                let reference = output["repository_path"]
                    .as_str()
                    .ok_or("structural output path required")?;
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
                .map_err(|e| format!("structural frozen output: {e:?}"))?;
                frozen.push((reference.to_owned(), raw));
            }
            let target = if action == "build" {
                let path = Path::new(tracked_root.ok_or("build requires --tracked-output-root")?);
                Some(select_empty_output(&ctx, path, &[&root])?)
            } else {
                None
            };
            let mut result = crate::structural_source_routes::run(
                &ctx,
                recipe,
                action == "build",
                None,
                target.as_ref(),
            )?;
            if result["outputs"].as_array().map(Vec::len) != Some(frozen.len()) {
                return Err("rebuilt structural output membership differs from curation".into());
            }
            for (reference, raw) in frozen {
                if ctx.read(&reference)? != raw {
                    return Err("selected structural output changed during verification".into());
                }
                if let Some(target) = &target {
                    if target.read(&reference)? != raw {
                        return Err("rebuilt structural output differs from selected source".into());
                    }
                }
            }
            selected.verify_unchanged(&ctx)?;
            for referenced in &refs {
                referenced.verify_unchanged(&ctx)?;
            }
            result["stage"] = serde_json::json!(stage);
            result["curation_ref"] = serde_json::json!(relative);
            result["current_curation_sha256"] =
                serde_json::json!(tos_foundation::Digest256::of_bytes(&owner.raw).to_hex());
            if action == "build" {
                result["replay"] = serde_json::json!("historical-source-bound");
            }
            writeln!(out, "{result}").map_err(|error| error.to_string())?;
            return Ok(());
        }
        if matches!(action, "check" | "build") && stage == "numbered-label-correspondence" {
            if input_root.is_some() || output_root.is_some() {
                return Err("numbered labels use selected source metadata only".into());
            }
            let mut frozen = Vec::new();
            for output in recipe["frozen_outputs"]
                .as_array()
                .ok_or("label frozen outputs required")?
            {
                let role = output["role"]
                    .as_str()
                    .ok_or("label output role required")?;
                let reference = output["repository_path"]
                    .as_str()
                    .ok_or("label output path required")?;
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
                .map_err(|e| format!("label frozen output: {e:?}"))?;
                frozen.push((reference.to_owned(), raw));
            }
            let target = if action == "build" {
                let path = Path::new(tracked_root.ok_or("build requires --tracked-output-root")?);
                Some(select_empty_output(&ctx, path, &[&root])?)
            } else {
                None
            };
            let mut result = crate::numbered_label_correspondence::run(
                &ctx,
                recipe,
                action == "build",
                None,
                target.as_ref(),
            )?;
            if result["outputs"].as_array().map(Vec::len) != Some(frozen.len()) {
                return Err("rebuilt label output membership differs from curation".into());
            }
            for (reference, raw) in frozen {
                if ctx.read(&reference)? != raw {
                    return Err("selected label output changed during verification".into());
                }
                if let Some(target) = &target {
                    if target.read(&reference)? != raw {
                        return Err("rebuilt label output differs from selected source".into());
                    }
                }
            }
            selected.verify_unchanged(&ctx)?;
            for referenced in &refs {
                referenced.verify_unchanged(&ctx)?;
            }
            result["stage"] = serde_json::json!(stage);
            result["curation_ref"] = serde_json::json!(relative);
            result["current_curation_sha256"] =
                serde_json::json!(tos_foundation::Digest256::of_bytes(&owner.raw).to_hex());
            if action == "build" {
                result["replay"] = serde_json::json!("historical-source-bound");
            }
            writeln!(out, "{result}").map_err(|error| error.to_string())?;
            return Ok(());
        }
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
                let selected = select_empty_output(&ctx, path, &[&root, input, output])?;
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
