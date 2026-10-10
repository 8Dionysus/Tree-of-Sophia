//! Explicit source, private input and output selection for lexical derivatives.
use std::{io::Write, path::PathBuf};
use tos_compiler::{lexical_derivatives, research_execution::ResearchExecution};
const HELP: &str = "tos zarathustra-morphology-input --source-root ABS --local-input-root ABS --local-output-root ABS --check|--build [--plan REL] [--generation NAME] [--receipt REL] [--scratch-bytes RESERVED_BYTES] [--max-seconds 1..600]\n\nReconstruct the frozen exact-form census. Without a generation, check the retained private packet and historical receipt. Build requires a separate output root, a new generation and admitted bytes; its receipt commits the generation after the private mode-0600 packet. Repeating the same build verifies matching outputs and completes an interrupted generation. Conflicting outputs are refused. No morphology provider, source acceptance or rights decision.\n";
const RECURRENCE_HELP: &str = "tos zarathustra-recurrence-projection --source-root ABS --local-output-root ABS --check|--build [--plan REL] [--generation NAME --event-at RFC3339] [--scratch-bytes RESERVED_BYTES] [--max-seconds 1..600]\n\nCompute frequency, structural range and exact rational dispersion from the hash-only lexical projection fixed by the plan. Check retains historical provenance; a fresh build requires a separate output root, generation, event time and admitted bytes. Matching retries preserve outputs; conflicts refuse. Source strings and semantic judgments are outside this projection.\n";
const USAGE_HELP: &str = "tos zarathustra-usage-context --source-root ABS --local-input-root ABS --local-output-root ABS --check|--build [--plan REL] [--generation NAME --event-at RFC3339] [--receipt REL --provenance REL] [--scratch-bytes RESERVED_BYTES] [--max-seconds 1..600]\n\nBuild the complete page-bounded private concordance for the frozen exact-form method control. The private JSONL stays mode0600; the receipt and provenance expose only fixity, counts and source references. Checks preserve historical provenance; a new generation requires a separate output root and admitted bytes.\n";
const SOURCE_RECURRENCE_HELP: &str = "tos semantic-source-recurrence --source-root ABS --local-input-root ABS --local-output-root ABS --check|--build [--plan REL] [--generation NAME --event-at RFC3339] [--scratch-bytes RESERVED_BYTES] [--max-seconds 1..600]\n\nReturn every selected occurrence to fixity-bound raw TEI Unicode character offsets. Preserves historical evidence on check; new generations use private mode0600 packets and text-free Rust receipts. No semantic, rights or publication admission.\n";
const CONTEXT_HELP: &str = "tos zarathustra-morphology-context --source-root ABS --local-input-root ABS --local-output-root ABS --a-raw-output ABS --check|--build [--plan REL] [--generation NAME --event-at RFC3339] [--receipt REL --provenance REL] [--scratch-bytes RESERVED_BYTES] [--max-seconds 1..600]\n\nVerify the retained provider A stream and freeze the preselected first/median/last raw TEI contexts. This command runs no provider. Private packets stay mode0600; source-withholding receipts preserve historical evidence or record a new Rust generation.\n";
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    let command = args.first().map(String::as_str);
    let recurrence = command == Some("zarathustra-recurrence-projection");
    let usage = command == Some("zarathustra-usage-context");
    let source_recurrence = command == Some("semantic-source-recurrence");
    let context = command == Some("zarathustra-morphology-context");
    if command != Some("zarathustra-morphology-input")
        && !recurrence
        && !usage
        && !source_recurrence
        && !context
    {
        return None;
    }
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        return Some(
            if out
                .write_all(
                    if context {
                        CONTEXT_HELP
                    } else if source_recurrence {
                        SOURCE_RECURRENCE_HELP
                    } else if recurrence {
                        RECURRENCE_HELP
                    } else if usage {
                        USAGE_HELP
                    } else {
                        HELP
                    }
                    .as_bytes(),
                )
                .is_ok()
            {
                0
            } else {
                2
            },
        );
    }
    let result = (|| -> Result<serde_json::Value, String> {
        let (
            mut root,
            mut input,
            mut output,
            mut plan,
            mut generation,
            mut at,
            mut a_raw,
            mut receipt,
            mut provenance,
            mut seconds,
            mut scratch,
            mut mode,
        ) = (
            None, None, None, None, None, None, None, None, None, None, None, None,
        );
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
                "--event-at" => &mut at,
                "--a-raw-output" => &mut a_raw,
                "--receipt" => &mut receipt,
                "--provenance" => &mut provenance,
                "--max-seconds" => &mut seconds,
                "--scratch-bytes" => &mut scratch,
                _ => return Err(format!("unknown option {key}")),
            };
            if slot.replace(value.as_str()).is_some() {
                return Err(format!("duplicate option {key}"));
            }
        }
        let root = PathBuf::from(root.ok_or("explicit source root required")?);
        if recurrence && (input.is_some() || receipt.is_some()) {
            return Err("recurrence does not accept private input or receipt redirection".into());
        }
        if source_recurrence && receipt.is_some() {
            return Err("source recurrence retains plan-selected receipt paths".into());
        }
        if !usage && !context && provenance.is_some() {
            return Err("provenance redirection belongs to usage context".into());
        }
        if !recurrence && !usage && !source_recurrence && !context && at.is_some() {
            return Err("morphology census has no provenance timestamp option".into());
        }
        if !context && a_raw.is_some() {
            return Err("provider A selection belongs to morphology context".into());
        }
        let input = if recurrence {
            root.clone()
        } else {
            PathBuf::from(input.ok_or("explicit local input root required")?)
        };
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
        if context {
            let a_raw = PathBuf::from(a_raw.ok_or("explicit provider A output required")?);
            if !a_raw.is_absolute() {
                return Err("absolute provider A output required".into());
            }
            return lexical_derivatives::morphology_context::run(
                &ctx,
                lexical_derivatives::morphology_context::Options {
                    build,
                    input_root: &input,
                    output_root: &output,
                    a_raw_output: &a_raw,
                    plan: plan.unwrap_or(lexical_derivatives::morphology_context::PLAN),
                    generation,
                    event_at: at,
                    receipt,
                    provenance,
                },
            );
        }
        if source_recurrence {
            return lexical_derivatives::semantic_recurrence::run(
                &ctx,
                lexical_derivatives::semantic_recurrence::Options {
                    build,
                    input_root: &input,
                    output_root: &output,
                    plan: plan.unwrap_or(lexical_derivatives::semantic_recurrence::PLAN),
                    generation,
                    event_at: at,
                },
            );
        }
        if recurrence {
            return lexical_derivatives::recurrence::run(
                &ctx,
                lexical_derivatives::recurrence::Options {
                    build,
                    output_root: &output,
                    plan: plan.unwrap_or(lexical_derivatives::recurrence::PLAN),
                    generation,
                    event_at: at,
                },
            );
        }
        if usage {
            return lexical_derivatives::usage_context::run(
                &ctx,
                lexical_derivatives::usage_context::Options {
                    build,
                    input_root: &input,
                    output_root: &output,
                    plan: plan.unwrap_or(lexical_derivatives::usage_context::PLAN),
                    generation,
                    event_at: at,
                    receipt,
                    provenance,
                },
            );
        }
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
            let _ = writeln!(
                err,
                "{} refused: {error}",
                command.unwrap_or("lexical derivative")
            );
            1
        }
    })
}
