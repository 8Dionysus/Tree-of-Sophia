//! Native regeneration and checks for authored public synthetic laboratory fixtures.
use std::{io::Write, path::PathBuf};
const HELP: &str = "tos synthetic-foundation-lab --source-root ABS --laboratory source-text-unit-v1|translation-alignment-v1|semantic-annotation-v2 --build|--check [--scratch-bytes RESERVED_BYTES] [--max-seconds 1..600]\n\nRegenerate the declared public synthetic A/B/C fixtures through the shared Rust schema and semantic validation rules. No private input, translation, human review, source admission or canon change. Build requires admitted bytes; check is read-only.\n";
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if args.first().map(String::as_str) != Some("synthetic-foundation-lab") {
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
        let (mut root, mut kind, mut seconds, mut scratch, mut mode) =
            (None, None, None, None, None);
        let mut it = args.iter().skip(1);
        while let Some(key) = it.next() {
            if matches!(key.as_str(), "--build" | "--check") {
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
                "--laboratory" => &mut kind,
                "--max-seconds" => &mut seconds,
                "--scratch-bytes" => &mut scratch,
                _ => return Err(format!("unknown option {key}")),
            };
            if slot.replace(value.clone()).is_some() {
                return Err(format!("duplicate option {key}"));
            }
        }
        let root = PathBuf::from(root.ok_or("source root required")?);
        if !root.is_absolute() {
            return Err("absolute source root required".into());
        }
        let kind = kind.ok_or("laboratory required")?;
        let build = mode.ok_or("action required")?;
        let seconds = seconds
            .as_deref()
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
        tos_compiler::synthetic_foundation_labs::run(&ctx, &kind, build)
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
            let _ = writeln!(err, "Synthetic foundation lab refused: {e}");
            1
        }
    })
}
