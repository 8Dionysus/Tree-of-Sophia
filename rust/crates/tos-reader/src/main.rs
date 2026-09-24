//! Thin, trusted-local CLI for exact v1 corpus reads. It grants no public access.

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::io::{self, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use tos_foundation::{Digest256, JsonLimits, SourceRevision};
use tos_source_store::{CorpusReader, ReadLimits, Selector};

const USAGE: &str = "tos-reader --store ABSOLUTE_ROOT --revision SHA256 --source-id ID \
--stage-dir ABSOLUTE_DIR --max-manifest-bytes N --max-manifest-entries N \
--max-selected-object-bytes N --json-max-depth N --json-max-visits N \
--json-max-integer-digits N";
const CAPABILITIES: &str = "{\"schema_version\":\"tos_reader_capabilities_v1\",\"store_format\":\"tos_corpus_snapshot_v1\",\"selection\":\"exact_revision_and_source_id\",\"platform\":\"linux\",\"minimum_kernel\":\"5.6\",\"required_open_api\":\"openat2\",\"path_traversal\":\"beneath_no_symlinks\",\"unsafe_fallback\":false}";

fn required(values: &mut BTreeMap<String, OsString>, name: &str) -> Result<OsString, String> {
    values
        .remove(name)
        .ok_or_else(|| format!("missing {name}; usage: {USAGE}"))
}

fn required_text(values: &mut BTreeMap<String, OsString>, name: &str) -> Result<String, String> {
    required(values, name)?
        .into_string()
        .map_err(|_| format!("{name} must be UTF-8 text"))
}

fn required_usize(values: &mut BTreeMap<String, OsString>, name: &str) -> Result<usize, String> {
    required_text(values, name)?
        .parse::<usize>()
        .map_err(|_| format!("{name} must be a positive integer"))
        .and_then(|value| {
            if value == 0 {
                Err(format!("{name} must be positive"))
            } else {
                Ok(value)
            }
        })
}

fn required_u64(values: &mut BTreeMap<String, OsString>, name: &str) -> Result<u64, String> {
    required_text(values, name)?
        .parse::<u64>()
        .map_err(|_| format!("{name} must be a positive integer"))
        .and_then(|value| {
            if value == 0 {
                Err(format!("{name} must be positive"))
            } else {
                Ok(value)
            }
        })
}

fn arguments() -> Result<BTreeMap<String, OsString>, String> {
    let mut args = env::args_os().skip(1);
    let mut values = BTreeMap::new();
    while let Some(raw_name) = args.next() {
        let name = raw_name
            .into_string()
            .map_err(|_| format!("option names must be UTF-8; usage: {USAGE}"))?;
        if name == "--help" {
            println!(
                "{USAGE}\nRequires Linux 5.6+ with openat2; no weaker path-open fallback. Run --capabilities for the versioned platform contract."
            );
            std::process::exit(0);
        }
        if name == "--capabilities" {
            println!("{CAPABILITIES}");
            std::process::exit(0);
        }
        if !name.starts_with("--") {
            return Err(format!("unexpected argument {name}; usage: {USAGE}"));
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {name}"))?;
        if values.insert(name.clone(), value).is_some() {
            return Err(format!("duplicate option {name}"));
        }
    }
    Ok(values)
}

fn run() -> Result<(), String> {
    let mut values = arguments()?;
    let store = PathBuf::from(required(&mut values, "--store")?);
    let stage_dir = PathBuf::from(required(&mut values, "--stage-dir")?);
    if !store.is_absolute() || !stage_dir.is_absolute() {
        return Err("--store and --stage-dir must be absolute paths".to_owned());
    }
    let revision = SourceRevision(
        Digest256::from_hex(&required_text(&mut values, "--revision")?)
            .map_err(|error| error.to_string())?,
    );
    let source_id = required_text(&mut values, "--source-id")?;
    let max_manifest_bytes = required_usize(&mut values, "--max-manifest-bytes")?;
    let max_manifest_entries = required_usize(&mut values, "--max-manifest-entries")?;
    let max_selected_object_bytes = required_u64(&mut values, "--max-selected-object-bytes")?;
    let max_depth = required_usize(&mut values, "--json-max-depth")?;
    let max_visits = required_usize(&mut values, "--json-max-visits")?;
    let max_integer_digits = required_usize(&mut values, "--json-max-integer-digits")?;
    if let Some(name) = values.keys().next() {
        return Err(format!("unknown option {name}; usage: {USAGE}"));
    }
    let json = JsonLimits::new(
        max_manifest_bytes,
        max_depth,
        max_visits,
        max_integer_digits,
    )
    .map_err(|error| error.to_string())?;
    let limits = ReadLimits {
        max_manifest_bytes,
        max_manifest_entries,
        max_selected_object_bytes,
        json,
    };
    let reader = CorpusReader::open_existing(&store, limits).map_err(|error| error.to_string())?;
    let snapshot = reader
        .load_exact(revision)
        .map_err(|error| error.to_string())?;
    let descriptor = reader
        .resolve(&snapshot, Selector::SourceId(&source_id))
        .map_err(|error| error.to_string())?;
    // The selected bytes remain private until the complete object passes fixity checks.
    let mut stage = tempfile::tempfile_in(stage_dir).map_err(|error| error.to_string())?;
    reader
        .read_selected(
            &snapshot,
            &descriptor,
            max_selected_object_bytes,
            &mut stage,
        )
        .map_err(|error| error.to_string())?;
    stage.flush().map_err(|error| error.to_string())?;
    stage
        .seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    let stdout = io::stdout();
    let mut output = stdout.lock();
    io::copy(&mut stage, &mut output).map_err(|error| error.to_string())?;
    output.flush().map_err(|error| error.to_string())?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tos-reader: {error}");
            ExitCode::from(2)
        }
    }
}
