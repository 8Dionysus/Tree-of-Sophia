//! Explicit source, private input and output selection for lexical derivatives.
use std::{io::Write, path::PathBuf};
use tos_compiler::{lexical_derivatives, research_execution::ResearchExecution};
const HELP: &str = "tos zarathustra-morphology-input --source-root ABS --local-input-root ABS --local-output-root ABS --check|--build [--plan REL] [--generation NAME] [--receipt REL] [--scratch-bytes RESERVED_BYTES] [--max-seconds 1..600]\n\nReconstruct the frozen exact-form census. Without a generation, check the retained private packet and historical receipt. Build requires a separate output root, a new generation and admitted bytes; its receipt commits the generation after the private mode-0600 packet. Repeating the same build verifies matching outputs and completes an interrupted generation. Conflicting outputs are refused. No morphology provider, source acceptance or rights decision.\n";
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if args.first().map(String::as_str) != Some("zarathustra-morphology-input") {
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
        let (
            mut root,
            mut input,
            mut output,
            mut plan,
            mut generation,
            mut receipt,
            mut seconds,
            mut scratch,
            mut mode,
        ) = (None, None, None, None, None, None, None, None, None);
        let mut it = args.iter().skip(1);
        while let Some(key) = it.next() {
            if matches!(key.as_str(), "--check" | "--build") {
                if mode.replace(key == "--build").is_some() {
                    return Err("choose one action".into());
                }
                continue;
            }
            let value = it
                .next()
                .ok_or_else(|| format!("missing value for {key}"))?;
            let slot = match key.as_str() {
                "--source-root" => &mut root,
                "--local-input-root" => &mut input,
                "--local-output-root" => &mut output,
                "--plan" => &mut plan,
                "--generation" => &mut generation,
                "--receipt" => &mut receipt,
                "--max-seconds" => &mut seconds,
                "--scratch-bytes" => &mut scratch,
                _ => return Err(format!("unknown option {key}")),
            };
            if slot.replace(value.as_str()).is_some() {
                return Err(format!("duplicate option {key}"));
            }
        }
        let root = PathBuf::from(root.ok_or("explicit source root required")?);
        let input = PathBuf::from(input.ok_or("explicit local input root required")?);
        let output = PathBuf::from(output.ok_or("explicit local output root required")?);
        if !root.is_absolute() || !input.is_absolute() || !output.is_absolute() {
            return Err("absolute roots required".into());
        }
        let build = mode.ok_or("action required")?;
        let seconds = seconds
            .unwrap_or("180")
            .parse()
            .map_err(|_| "invalid seconds")?;
        let ctx = if build {
            ResearchExecution::new_with_scratch(
                &root,
                seconds,
                scratch
                    .ok_or("build requires admitted bytes")?
                    .parse()
                    .map_err(|_| "invalid scratch bytes")?,
            )?
        } else {
            if scratch.is_some() {
                return Err("check does not reserve writes".into());
            }
            ResearchExecution::new(&root, seconds)?
        };
        lexical_derivatives::morphology_input(
            &ctx,
            lexical_derivatives::Options {
                build,
                input_root: &input,
                output_root: &output,
                plan: plan.unwrap_or(lexical_derivatives::MORPHOLOGY_PLAN),
                generation,
                receipt,
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
            let _ = writeln!(err, "zarathustra-morphology-input refused: {error}");
            1
        }
    })
}
