//! Explicit native transfer target passage entry; no implicit source discovery.
use std::{io::Write, path::PathBuf};
use tos_compiler::{
    transfer_target_passages::{self, Action, Options},
    research_execution::ResearchExecution,
};
const HELP:&str="tos transfer-target-passages --source-root ABS --build|--check|--validate-tracked [--local-input-root ABS --local-output-root ABS] [--generation NAME --event-id ID] [--max-seconds 1..600] [--scratch-bytes RESERVED_BYTES]\n\nExact private bbox slices for the frozen twenty-page / thirty-five-route transfer frame. Build and check require explicit private roots. Tracked validation reads no PDF. The default v1 retains historical provenance; a fresh generation requires a new event ID and fresh output paths. Build requires an admitted output quota. No text, alignment, gold, canon or publication admission.\n";
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if args.first().map(String::as_str) != Some("transfer-target-passages") {
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
        let mut source = None;
        let mut input = None;
        let mut output = None;
        let mut mode = None;
        let mut generation = None;
        let mut event = None;
        let mut scratch = None;
        let mut seconds = None;
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
                "--source-root" | "--repo-root" => &mut source,
                "--local-input-root" => &mut input,
                "--local-output-root" => &mut output,
                "--generation" => &mut generation,
                "--event-id" => &mut event,
                "--scratch-bytes" => &mut scratch,
                "--max-seconds" => &mut seconds,
                _ => return Err(format!("unknown option {k}")),
            };
            if selected.replace(value.clone()).is_some() {
                return Err(format!("duplicate option {k}"));
            }
        }
        let root = PathBuf::from(source.ok_or("source root required")?);
        let input = input.map(PathBuf::from);
        let output = output.map(PathBuf::from);
        if !root.is_absolute()
            || input.as_ref().is_some_and(|p| !p.is_absolute())
            || output.as_ref().is_some_and(|p| !p.is_absolute())
        {
            return Err("roots must be absolute".into());
        }
        let action=match mode.ok_or("action required")? {"--build"=>Action::Build,"--check"=>Action::Check,"--validate-tracked"=>Action::ValidateTracked,_=>unreachable!()};
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
                return Err("check does not reserve writes".into());
            }
            ResearchExecution::new(&root, seconds)?
        };
        transfer_target_passages::run(
            &ctx,
            Options {
                generation: generation.as_deref(),
                input_root: input.as_deref(),
                output_root: output.as_deref(),
                action,
                event_id: event.as_deref(),
            },
        )
    })();
    Some(match result {
        Ok(v) => {
            if writeln!(out, "{}", serde_json::to_string_pretty(&v).unwrap()).is_ok() {
                0
            } else {
                2
            }
        }
        Err(e) => {
            let _ = writeln!(err, "transfer target passages refused: {e}");
            1
        }
    })
}
