//! Explicit native entrypoint for the independent public provenance A/B/C operations.
use std::{io::Write, path::PathBuf};
const HELP: &str = "tos provenance-event-v2-lab --source-root ABS --prepare|--variant A|B|C|--finalize|--check [--scratch-bytes RESERVED_BYTES] [--max-seconds 1..600]\n\nPrepare the native environment, run each independent variant, then finalize its exact manifest. Variant C records an honest ASCII failure and returns 7. Check is read-only. Writes require admitted bytes. Records withhold host-local argv paths and never establish execution truth, human review or publication authority.\n";
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if args.first().map(String::as_str) != Some("provenance-event-v2-lab") {
        return None;
    }
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        return Some(if out.write_all(HELP.as_bytes()).is_ok() {
            0
        } else {
            2
        });
    }
    let result = (|| -> Result<(serde_json::Value, i32), String> {
        let (mut root, mut mode, mut variant, mut scratch, mut seconds) =
            (None, None, None, None, None);
        let mut it = args.iter().skip(1);
        while let Some(key) = it.next() {
            if matches!(
                key.as_str(),
                "--prepare" | "--finalize" | "--check" | "--variant"
            ) {
                if mode.replace(key.as_str()).is_some() {
                    return Err("choose one action".into());
                }
                if key == "--variant" {
                    variant = Some(it.next().ok_or("missing variant")?.as_str());
                }
                continue;
            }
            let value = it
                .next()
                .ok_or_else(|| format!("missing value for {key}"))?;
            let slot = match key.as_str() {
                "--source-root" => &mut root,
                "--scratch-bytes" => &mut scratch,
                "--max-seconds" => &mut seconds,
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
        let mode = mode.ok_or("action required")?;
        let seconds = seconds
            .unwrap_or("180")
            .parse()
            .map_err(|_| "invalid seconds")?;
        let ctx = if mode == "--check" {
            if scratch.is_some() {
                return Err("check does not reserve writes".into());
            }
            tos_compiler::research_execution::ResearchExecution::new(&root, seconds)?
        } else {
            tos_compiler::research_execution::ResearchExecution::new_with_scratch(
                &root,
                seconds,
                scratch
                    .ok_or("write requires admitted bytes")?
                    .parse()
                    .map_err(|_| "invalid scratch bytes")?,
            )?
        };
        use tos_compiler::provenance_event_lab;
        Ok(match mode {
            "--prepare" => (
                provenance_event_lab::prepare(&ctx, env!("CARGO_PKG_VERSION"))?,
                0,
            ),
            "--variant" => provenance_event_lab::variant(
                &ctx,
                variant.ok_or("variant required")?,
                &std::env::args().collect::<Vec<_>>(),
                env!("CARGO_PKG_VERSION"),
            )?,
            _ => (
                provenance_event_lab::finalize(&ctx, mode == "--finalize")?,
                0,
            ),
        })
    })();
    Some(match result {
        Ok((value, code)) => {
            if writeln!(out, "{}", serde_json::to_string_pretty(&value).unwrap()).is_ok() {
                code
            } else {
                2
            }
        }
        Err(e) => {
            let _ = writeln!(err, "Provenance lab refused: {e}");
            1
        }
    })
}
