//! Native explicit-source adapter for the exact DTA source-text foundation.
use std::{io::Write, path::PathBuf};
const TARGET_HELP: &str = "tos target-text-foundation --source-root ABS --local-input-root ABS --local-output-root ABS --build|--check [--plan REPO_PATH] [--event-id ID] [--max-seconds 1..600] [--scratch-bytes RESERVED_BYTES]\n\nReplays exact PDF/Poppler bbox/text/address/unit bytes and retained provenance. New native builds require a separate event ID and fresh plan-selected output paths. Build requires admitted scratch bytes; local text and bbox remain Git-ignored and mode0600. Poppler is the declared native extraction backend. No textual or publication admission.\n";
const HELP: &str = "tos source-text-foundation --source-root ABS --local-input-root ABS --local-output-root ABS --build|--check [--plan REPO_PATH] [--event-id ID] [--max-seconds 1..600] [--scratch-bytes RESERVED_BYTES]\n\nChecks replay exact XML/text/address/unit bytes and retained provenance closure.\nNew builds require a separate event ID and fresh plan-selected output paths. Existing exact outputs may be reused; differing records are never overwritten.\n--build requires admitted remaining scratch bytes. All extracted text stays Git-ignored, mode0600 and local-only; no source or publication admission is granted.\n";
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    let target = args.first().map(String::as_str) == Some("target-text-foundation");
    if !target && args.first().map(String::as_str) != Some("source-text-foundation") {
        return None;
    }
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        return Some(
            if out
                .write_all(if target {
                    TARGET_HELP.as_bytes()
                } else {
                    HELP.as_bytes()
                })
                .is_ok()
            {
                0
            } else {
                2
            },
        );
    }
    let result = (|| {
        let mut source = None;
        let mut input = None;
        let mut output = None;
        let mut mode = None;
        let mut event = None;
        let mut plan = None;
        let mut seconds = None;
        let mut scratch = None;
        let mut it = args.iter().skip(1);
        while let Some(key) = it.next() {
            if matches!(key.as_str(), "--build" | "--check") {
                if mode.replace(key == "--build").is_some() {
                    return Err("choose exactly one build/check mode".into());
                }
                continue;
            }
            let value = it
                .next()
                .ok_or_else(|| format!("missing value for {key}"))?;
            let target = match key.as_str() {
                "--source-root" => &mut source,
                "--local-input-root" => &mut input,
                "--local-output-root" => &mut output,
                "--event-id" => &mut event,
                "--plan" => &mut plan,
                "--max-seconds" => &mut seconds,
                "--scratch-bytes" => &mut scratch,
                _ => return Err(format!("unknown option {key}")),
            };
            if target.replace(value.clone()).is_some() {
                return Err(format!("duplicate option {key}"));
            }
        }
        let root = PathBuf::from(source.ok_or("--source-root is required")?);
        let input = PathBuf::from(input.ok_or("--local-input-root is required")?);
        let output = PathBuf::from(output.ok_or("--local-output-root is required")?);
        if [&root, &input, &output].iter().any(|p| !p.is_absolute()) {
            return Err("all selected roots must be absolute".into());
        }
        let build = mode.ok_or("--build or --check is required")?;
        let max_seconds = seconds
            .as_deref()
            .unwrap_or("60")
            .parse::<u64>()
            .map_err(|_| "invalid max-seconds")?;
        let ctx = if build {
            tos_compiler::research_execution::ResearchExecution::new_with_scratch(
                &root,
                max_seconds,
                scratch
                    .ok_or("--build requires --scratch-bytes")?
                    .parse::<u64>()
                    .map_err(|_| "invalid scratch-bytes")?,
            )?
        } else {
            if scratch.is_some() {
                return Err("check does not reserve write space".into());
            }
            tos_compiler::research_execution::ResearchExecution::new(&root, max_seconds)?
        };
        let argv = serde_json::json!(std::env::args().collect::<Vec<_>>());
        let opts = tos_compiler::source_text_foundation::Options {
            plan_ref: plan.as_deref().unwrap_or(if target {
                tos_compiler::target_text_foundation::PLAN
            } else {
                tos_compiler::source_text_foundation::PLAN
            }),
            input_root: &input,
            output_root: &output,
            build,
            event_id: event.as_deref(),
            argv: &argv,
        };
        if target {
            tos_compiler::target_text_foundation::run(&ctx, opts)
        } else {
            tos_compiler::source_text_foundation::run(&ctx, opts)
        }
    })();
    Some(match result {
        Ok(value) => {
            if writeln!(out, "{}", serde_json::to_string_pretty(&value).unwrap()).is_ok() {
                0
            } else {
                2
            }
        }
        Err(e) => {
            let _ = writeln!(err, "source-text foundation refused: {e}");
            1
        }
    })
}
