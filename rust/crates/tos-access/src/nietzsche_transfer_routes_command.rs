//! Text-free structural source routes selected explicitly by the operator.
use std::{io::Write, path::PathBuf};
const HELP: &str = "tos nietzsche-transfer-source-routes --source-root ABS --check|--build [--generation NAME --event-at RFC3339] [--scratch-bytes RESERVED_BYTES] [--max-seconds 1..600]\n\nChecks both retained historical maps, candidate source routes and their events exactly. A new generation creates eight files under separate native-NAME source-owned alignment directories and records Rust provenance. An existing different output is preserved. No witness payloads, source text acceptance, translation assessment or rights admission.\n";
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if args.first().map(String::as_str) != Some("nietzsche-transfer-source-routes") {
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
        let (mut root, mut action, mut generation, mut at, mut seconds, mut scratch) =
            (None, None, None, None, None, None);
        let mut it = args.iter().skip(1);
        while let Some(key) = it.next() {
            if matches!(key.as_str(), "--check" | "--build") {
                if action.replace(key == "--build").is_some() {
                    return Err("choose one action".into());
                }
                continue;
            }
            let value = it
                .next()
                .ok_or_else(|| format!("missing value for {key}"))?;
            let slot = match key.as_str() {
                "--source-root" => &mut root,
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
        if !root.is_absolute() {
            return Err("absolute source root required".into());
        }
        let build = action.ok_or("action required")?;
        let seconds = seconds
            .unwrap_or("180")
            .parse()
            .map_err(|_| "invalid seconds")?;
        let generation = match (generation, at) {
            (None, None) => None,
            (Some(name), Some(event_at)) => {
                Some(tos_compiler::nietzsche_transfer_source_routes::Generation { name, event_at })
            }
            _ => return Err("supply both generation and event timestamp".into()),
        };
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
        tos_compiler::nietzsche_transfer_source_routes::run(&ctx, build, generation)
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
            let _ = writeln!(err, "Nietzsche structural routes refused: {e}");
            1
        }
    })
}
