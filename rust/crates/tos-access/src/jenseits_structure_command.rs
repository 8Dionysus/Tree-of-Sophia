//! Explicit local source structure generation with separate payload custody.
use std::{io::Write, path::PathBuf};
const HELP: &str = "tos jenseits-numbered-structure --source-root ABS --local-input-root ABS --check|--build [--generation NAME --event-at RFC3339] [--scratch-bytes RESERVED_BYTES] [--max-seconds 1..600]\n\nReconstruct the retained text-free ABBYY numbered-unit map, or produce a separate native generation with its own provenance. Build requires a generation, event timestamp and admitted bytes. Checks are read-only. No source text, translation, rights or canon admission.\n";
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    let command = args.first().map(String::as_str);
    let polilov = command == Some("jenseits-polilov-numbered-structure");
    if command != Some("jenseits-numbered-structure") && !polilov {
        return None;
    }
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        let help = if polilov {
            HELP.replace(
                "jenseits-numbered-structure",
                "jenseits-polilov-numbered-structure",
            )
            .replace("ABBYY", "Poppler PDF")
        } else {
            HELP.into()
        };
        return Some(if out.write_all(help.as_bytes()).is_ok() {
            0
        } else {
            2
        });
    }
    let result = (|| -> Result<serde_json::Value, String> {
        let (mut root, mut input, mut mode, mut generation, mut at, mut seconds, mut scratch) =
            (None, None, None, None, None, None, None);
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
                "--generation" => &mut generation,
                "--event-at" => &mut at,
                "--max-seconds" => &mut seconds,
                "--scratch-bytes" => &mut scratch,
                _ => return Err(format!("unknown option {key}")),
            };
            if slot.replace(value.as_str()).is_some() {
                return Err(format!("duplicate option {key}"));
            }
        }
        let root = PathBuf::from(root.ok_or("source root required")?);
        let input = PathBuf::from(input.ok_or("explicit local input root required")?);
        if !root.is_absolute() || !input.is_absolute() {
            return Err("absolute source and local input roots required".into());
        }
        let build = mode.ok_or("action required")?;
        let seconds = seconds
            .unwrap_or("180")
            .parse()
            .map_err(|_| "invalid seconds")?;
        let ctx = if build {
            tos_compiler::research_execution::ResearchExecution::new_with_scratch(
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
            tos_compiler::research_execution::ResearchExecution::new(&root, seconds)?
        };
        if polilov {
            tos_compiler::jenseits_polilov_numbered_structure::run(
                &ctx,
                tos_compiler::jenseits_polilov_numbered_structure::Options {
                    build,
                    input_root: Some(&input),
                    generation,
                    event_at: at,
                },
            )
        } else {
            tos_compiler::jenseits_numbered_structure::run(
                &ctx,
                tos_compiler::jenseits_numbered_structure::Options {
                    build,
                    input_root: Some(&input),
                    generation,
                    event_at: at,
                },
            )
        }
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
            let _ = writeln!(err, "Jenseits numbered structure refused: {e}");
            1
        }
    })
}
