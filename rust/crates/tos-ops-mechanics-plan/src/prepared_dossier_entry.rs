//! Standalone native prepared-dossier dispatch. Whole-process custody is the
//! existing philosophy executor; source, work and IO share ResearchExecution.
use crate::prepared_dossier_native::{NativePreparedDossierInputs, run_native};
use crate::prepared_dossier_readiness::{
    DEFAULT_DOC_ROOT, OBSERVATORY_TABLE_CHOICES, PreparedSourceProfile,
};
use serde_json::Value;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI32},
};
use std::time::{Duration, Instant};
use tos_compiler::research_execution::ResearchExecution;

struct Options {
    inputs: NativePreparedDossierInputs,
    max_seconds: u64,
    scratch_bytes: u64,
}
fn help_requested(args: &[String]) -> bool {
    args.iter()
        .any(|argument| matches!(argument.as_str(), "--help" | "-h"))
}
fn help_text() -> String {
    format!(
        concat!(
            "Usage: tos-ops-mechanics-plan --prepared-dossier [OPTIONS]\n",
            "Inspect or run prepared philosophy dossier planting.\n\n",
            "Source and output:\n",
            "  --source-root PATH       Selected repository root (required)\n",
            "  --doc-root PATH          DOCX root (default: {})\n",
            "  --output-root PATH       Output root (default: source root)\n",
            "  --source-profile NAME    tos_legacy_python_observed_json_v1 or tos_published_json_v1\n",
            "                           Default: tos_legacy_python_observed_json_v1\n",
            "  --max-seconds N          Whole-operation deadline (default: 600; range: 1..3600)\n",
            "  --scratch-bytes N        Reserved scratch space (default: 268435456; must be positive)\n\n",
            "Prepared dossiers:\n",
            "  --table TABLE            Limit readiness output to one master-table package; planting is aggregate-only.\n",
            "                           Choices: table-i, table-ii, table-iii\n",
            "  --readiness              Print planting readiness JSON and exit.\n",
            "  --plant                  Run the supported planting package.\n\n",
            "Value options accept --option=value; boolean flags do not.\n",
            "-h, --help                 Show this help and exit.\n"
        ),
        DEFAULT_DOC_ROOT
    )
}
fn set_once<T>(slot: &mut Option<T>, value: T, name: &str) -> Result<(), String> {
    if slot.replace(value).is_some() {
        return Err(format!("duplicate prepared-dossier option: {name}"));
    }
    Ok(())
}
fn checked_path(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute()
        || path.as_os_str().len() > 4096
        || path
            .components()
            .any(|p| !matches!(p, Component::RootDir | Component::Normal(_)))
    {
        return Err(format!(
            "{label} must be a bounded canonical absolute directory path"
        ));
    }
    Ok(())
}
fn options(args: &[String]) -> Result<Options, String> {
    if args.first().map(String::as_str) != Some("--prepared-dossier") {
        return Err("prepared-dossier entry marker required".into());
    }
    let mut repository_root = None;
    let mut doc_root = None;
    let mut output_root = None;
    let mut selected_table_id = None;
    let mut max_seconds = None;
    let mut scratch_bytes = None;
    let mut source_profile = None;
    let mut readiness = false;
    let mut plant = false;
    let mut index = 1;
    while index < args.len() {
        let argument = args[index].as_str();
        index += 1;
        let (option, inline_value) = argument
            .split_once('=')
            .map_or((argument, None), |(option, value)| (option, Some(value)));
        match (option, inline_value) {
            ("--readiness", None) => {
                if readiness {
                    return Err("duplicate --readiness".into());
                }
                readiness = true;
                continue;
            }
            ("--plant", None) => {
                if plant {
                    return Err("duplicate --plant".into());
                }
                plant = true;
                continue;
            }
            ("--readiness" | "--plant", Some(_)) => {
                return Err(format!("flag does not take a value: {option}"));
            }
            _ => {}
        }
        let value = match inline_value {
            Some(value) => value.to_owned(),
            None => {
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("missing value for {option}"))?;
                index += 1;
                value.clone()
            }
        };
        match option {
            "--source-root" => set_once(&mut repository_root, PathBuf::from(&value), option)?,
            "--doc-root" => set_once(&mut doc_root, PathBuf::from(&value), option)?,
            "--output-root" => set_once(&mut output_root, PathBuf::from(&value), option)?,
            "--table" => {
                if !OBSERVATORY_TABLE_CHOICES.contains(&value.as_str()) {
                    return Err("table must be table-i, table-ii or table-iii".into());
                }
                set_once(&mut selected_table_id, value.clone(), option)?;
            }
            "--max-seconds" => set_once(
                &mut max_seconds,
                value.parse::<u64>().map_err(|_| "invalid max-seconds")?,
                option,
            )?,
            "--scratch-bytes" => set_once(
                &mut scratch_bytes,
                value.parse::<u64>().map_err(|_| "invalid scratch-bytes")?,
                option,
            )?,
            "--source-profile" => set_once(
                &mut source_profile,
                match value.as_str() {
                    "tos_legacy_python_observed_json_v1" => {
                        PreparedSourceProfile::LegacyPythonObserved
                    }
                    "tos_published_json_v1" => PreparedSourceProfile::PublishedStrict,
                    _ => return Err("unknown prepared-dossier source profile".into()),
                },
                option,
            )?,
            _ => return Err(format!("unknown prepared-dossier option: {option}")),
        }
    }
    let repository_root = repository_root.ok_or("--source-root is required")?;
    let doc_root = doc_root.unwrap_or_else(|| PathBuf::from(DEFAULT_DOC_ROOT));
    let output_root = output_root.unwrap_or_else(|| repository_root.clone());
    for (path, label) in [
        (&repository_root, "source-root"),
        (&doc_root, "doc-root"),
        (&output_root, "output-root"),
    ] {
        checked_path(path, label)?;
    }
    let max_seconds = max_seconds.unwrap_or(600);
    let scratch_bytes = scratch_bytes.unwrap_or(256 * 1024 * 1024);
    if !(1..=3600).contains(&max_seconds) || scratch_bytes == 0 {
        return Err(
            "prepared-dossier max-seconds must be 1..3600 and scratch-bytes positive".into(),
        );
    }
    // This is the explicit source decoding profile of the maintained Python
    // caller, not a fallback after strict parsing fails.
    let source_profile = source_profile.unwrap_or(PreparedSourceProfile::LegacyPythonObserved);
    Ok(Options {
        inputs: NativePreparedDossierInputs {
            repository_root,
            doc_root,
            output_root,
            source_profile,
            selected_table_id,
            plant,
            readiness,
        },
        max_seconds,
        scratch_bytes,
    })
}
pub fn run_supervised(
    args: &[String],
    cancelled: &AtomicI32,
    started: Instant,
) -> Result<i32, String> {
    if help_requested(args) {
        std::io::stdout()
            .lock()
            .write_all(help_text().as_bytes())
            .map_err(|error| format!("write prepared-dossier help: {error}"))?;
        return Ok(0);
    }
    let options = options(args)?;
    let deadline = started
        .checked_add(Duration::from_secs(options.max_seconds))
        .ok_or("prepared-dossier deadline overflow")?;
    if Instant::now() >= deadline {
        return Err("prepared-dossier deadline before worker setup".into());
    }
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut argv = vec![
        executable
            .to_str()
            .ok_or("prepared-dossier executable must be UTF-8")?
            .to_owned(),
        "--prepared-dossier-worker".into(),
    ];
    argv.extend_from_slice(&args[1..]);
    crate::executor::run_philosophy_product(
        argv,
        crate::executor::Limits {
            command_wall: Duration::from_secs(options.max_seconds),
            lane_wall: Duration::from_secs(options.max_seconds),
            ..crate::executor::Limits::default()
        },
        cancelled,
        deadline,
    )
    .map_err(|e| e.to_string())
}
pub fn run(args: &[String], cancelled: Arc<AtomicBool>) -> Result<Value, String> {
    let options = options(args)?;
    let operation = ResearchExecution::new_philosophy_products_with_cancellation(
        &options.inputs.repository_root,
        options.max_seconds,
        options.scratch_bytes,
        cancelled,
    )?;
    run_native(options.inputs, &operation)
}

/// Preserve the maintained readiness stdout representation; product-file bytes
/// and this report share the exact planting source owner's JSON writer.
pub fn report_bytes(report: &Value) -> Result<Vec<u8>, String> {
    crate::prepared_dossier_render::readiness_report_bytes(report)
}
