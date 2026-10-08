//! Explicit-source adapters for text-free transfer projections.
use std::{io::Write, path::PathBuf};
const HELP: &str = "tos transfer-route-readiness --source-root ABS --build|--check [--output REPO_PATH] [--provenance REPO_PATH] [--event-id ID] [--max-seconds 1..600] [--scratch-bytes RESERVED_BYTES]\n\nChecks exact candidate, projection and provenance closure without reading private text. New builds need a new event ID, fresh projection output and admitted scratch bytes. Earlier journal bytes are retained; a changed journal refuses the update. This projection grants no alignment, eligibility, gold, publication or canon admission.\n";
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if args.first().map(String::as_str) != Some("transfer-route-readiness") {
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
        let mut root = None;
        let mut output = None;
        let mut provenance = None;
        let mut event = None;
        let mut seconds = None;
        let mut scratch = None;
        let mut build = None;
        let mut it = args.iter().skip(1);
        while let Some(k) = it.next() {
            if matches!(k.as_str(), "--build" | "--check" | "--validate") {
                if build.replace(k == "--build").is_some() {
                    return Err("choose one build/check mode".into());
                }
                continue;
            }
            let v = it.next().ok_or_else(|| format!("missing value for {k}"))?;
            let selected = match k.as_str() {
                "--source-root" | "--repo-root" => &mut root,
                "--output" => &mut output,
                "--provenance" => &mut provenance,
                "--event-id" => &mut event,
                "--max-seconds" => &mut seconds,
                "--scratch-bytes" => &mut scratch,
                _ => return Err(format!("unknown option {k}")),
            };
            if selected.replace(v.clone()).is_some() {
                return Err(format!("duplicate option {k}"));
            }
        }
        let root = PathBuf::from(root.ok_or("--source-root is required")?);
        if !root.is_absolute() {
            return Err("source root must be absolute".into());
        }
        let build = build.ok_or("--build or --check is required")?;
        let seconds = seconds
            .as_deref()
            .unwrap_or("60")
            .parse::<u64>()
            .map_err(|_| "invalid max-seconds")?;
        let ctx = if build {
            tos_compiler::research_execution::ResearchExecution::new_with_scratch(
                &root,
                seconds,
                scratch
                    .ok_or("--build requires --scratch-bytes")?
                    .parse::<u64>()
                    .map_err(|_| "invalid scratch-bytes")?,
            )?
        } else {
            if scratch.is_some() {
                return Err("check does not reserve write space".into());
            }
            tos_compiler::research_execution::ResearchExecution::new(&root, seconds)?
        };
        tos_compiler::transfer_route_readiness::run(
            &ctx,
            tos_compiler::transfer_route_readiness::Options {
                build,
                output_ref: output.as_deref(),
                provenance_ref: provenance.as_deref(),
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
            let _ = writeln!(err, "transfer metadata refused: {e}");
            1
        }
    })
}
