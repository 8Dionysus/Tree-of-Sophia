//! Explicit text-free structural map builder/checker over an owned source root.
use std::{io::Write, path::PathBuf};
const HELP: &str = "tos jenseits-label-correspondence --source-root ABS --check|--build [--output-directory REL --map-id ID --event-id ID --event-at RFC3339] [--scratch-bytes RESERVED_BYTES] [--max-seconds 1..600]\n\nCheck defaults to exact reconstruction of the retained historical map and event. Building requires all four new-output options and admitted bytes; existing different bytes are preserved. A supplied output selection can also be checked. No source text, translation assessment or rights admission.\n";
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if args.first().map(String::as_str) != Some("jenseits-label-correspondence") {
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
            mut mode,
            mut directory,
            mut map_id,
            mut event_id,
            mut at,
            mut seconds,
            mut scratch,
        ) = (None, None, None, None, None, None, None, None);
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
                "--output-directory" => &mut directory,
                "--map-id" => &mut map_id,
                "--event-id" => &mut event_id,
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
        let build = mode.ok_or("action required")?;
        let seconds = seconds
            .unwrap_or("180")
            .parse()
            .map_err(|_| "invalid seconds")?;
        let fresh = match (directory, map_id, event_id, at) {
            (None, None, None, None) => None,
            (Some(directory), Some(map_id), Some(event_id), Some(event_at)) => {
                Some(tos_compiler::jenseits_label_correspondence::Selection {
                    directory,
                    map_id,
                    event_id,
                    event_at,
                })
            }
            _ => return Err("supply all four output-selection options".into()),
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
        tos_compiler::jenseits_label_correspondence::run(&ctx, build, fresh)
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
            let _ = writeln!(err, "Jenseits label correspondence refused: {e}");
            1
        }
    })
}
