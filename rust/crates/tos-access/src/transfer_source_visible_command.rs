//! Explicit private-review receipt reconstruction; review status remains owned.
use std::{io::Write, path::PathBuf};
use tos_compiler::{
    research_execution::ResearchExecution,
    transfer_source_visible::{self, Action, Options},
};
const HELP: &str = "tos transfer-source-visible --source-root ABS --build|--check|--validate-tracked [--local-input-root ABS] [--generation NAME] [--max-seconds 1..600] [--scratch-bytes RESERVED_BYTES]\n\nReproduce four page-render hashes and aggregate diagnostics from the explicit private model review bundle. Historical v1 is immutable; a new generation writes a fresh text-free receipt. Build needs an admitted output quota. No new source-visible review or textual acceptance is performed. Renders use bounded memory and are not retained. Tracked validation reads no private payload.\n";
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if args.first().map(String::as_str) != Some("transfer-source-visible") {
        return None;
    }
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        return Some(if out.write_all(HELP.as_bytes()).is_ok() {
            0
        } else {
            2
        });
    }
    let result = (|| -> Result<serde_json::Value, String> {
        let (mut root, mut input, mut generation, mut mode, mut seconds, mut scratch) =
            (None, None, None, None, None, None);
        let mut it = args.iter().skip(1);
        while let Some(k) = it.next() {
            if matches!(k.as_str(), "--build" | "--check" | "--validate-tracked") {
                if mode.replace(k.as_str()).is_some() {
                    return Err("choose exactly one action".into());
                }
                continue;
            }
            let value = it.next().ok_or_else(|| format!("missing value for {k}"))?;
            let selected = match k.as_str() {
                "--source-root" | "--repo-root" => &mut root,
                "--local-input-root" => &mut input,
                "--generation" => &mut generation,
                "--max-seconds" => &mut seconds,
                "--scratch-bytes" => &mut scratch,
                _ => return Err(format!("unknown option {k}")),
            };
            if selected.replace(value.clone()).is_some() {
                return Err(format!("duplicate option {k}"));
            }
        }
        let root = PathBuf::from(root.ok_or("source root required")?);
        let input = input.map(PathBuf::from);
        if !root.is_absolute() || input.as_ref().is_some_and(|p| !p.is_absolute()) {
            return Err("roots must be absolute".into());
        }
        let mode = mode.ok_or("action required")?;
        let action = match mode {
            "--build" => Action::Build,
            "--check" => Action::Check,
            "--validate-tracked" => Action::ValidateTracked,
            _ => unreachable!(),
        };
        let seconds = seconds
            .as_deref()
            .unwrap_or("180")
            .parse::<u64>()
            .map_err(|_| "invalid seconds")?;
        let ctx = if matches!(action, Action::Build) {
            ResearchExecution::new_with_scratch(
                &root,
                seconds,
                scratch
                    .ok_or("build requires admitted scratch-bytes")?
                    .parse::<u64>()
                    .map_err(|_| "invalid scratch bytes")?,
            )?
        } else {
            if scratch.is_some() {
                return Err("read-only action does not reserve writes".into());
            }
            ResearchExecution::new(&root, seconds)?
        };
        transfer_source_visible::run(
            &ctx,
            Options {
                input_root: input.as_deref(),
                generation: generation.as_deref(),
                action,
            },
        )
    })();
    Some(match result {
        Ok(value) => {
            if writeln!(out, "{}", serde_json::to_string_pretty(&value).unwrap()).is_ok() {
                0
            } else {
                2
            }
        }
        Err(error) => {
            let _ = writeln!(err, "source-visible receipt refused: {error}");
            1
        }
    })
}
