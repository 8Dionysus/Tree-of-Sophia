//! Explicit source and private custody roots for the native authored-route bridge.
use std::{io::Write, path::PathBuf};
const HELP: &str = "tos authored-canon-bridge --source-root ABS --local-input-root ABS --local-output-root ABS --check|--build [--plan-ref REPO_PATH --event-id ID] [--max-seconds 1..600] [--scratch-bytes RESERVED_BYTES]\n\nReconstruct the exact twelve-paragraph DTA and authored-route bridge, separate source/authored segmentations, private normalization receipts and text-free 92-node/125-relation inventory. Historical bytes and provenance are preserved. A new plan requires separate outputs, opaque identities and bridge_id. Build requires admitted output bytes. Local source text remains private and grants no review, translation, semantic, publication or canon authority.\n";
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if args.first().map(String::as_str) != Some("authored-canon-bridge") {
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
            mut event,
            mut seconds,
            mut scratch,
            mut mode,
        ) = (None, None, None, None, None, None, None, None);
        let mut it = args.iter().skip(1);
        while let Some(key) = it.next() {
            if matches!(key.as_str(), "--build" | "--check") {
                if mode.replace(key == "--build").is_some() {
                    return Err("choose exactly one action".into());
                }
                continue;
            }
            let value = it
                .next()
                .ok_or_else(|| format!("missing value for {key}"))?;
            let selected = match key.as_str() {
                "--source-root" => &mut root,
                "--local-input-root" => &mut input,
                "--local-output-root" => &mut output,
                "--plan-ref" => &mut plan,
                "--event-id" => &mut event,
                "--max-seconds" => &mut seconds,
                "--scratch-bytes" => &mut scratch,
                _ => return Err(format!("unknown option {key}")),
            };
            if selected.replace(value.clone()).is_some() {
                return Err(format!("duplicate option {key}"));
            }
        }
        let root = PathBuf::from(root.ok_or("source root required")?);
        let input = PathBuf::from(input.ok_or("local input root required")?);
        let output = PathBuf::from(output.ok_or("local output root required")?);
        if [&root, &input, &output].iter().any(|p| !p.is_absolute()) {
            return Err("all roots must be absolute".into());
        }
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
                    .ok_or("build requires admitted scratch bytes")?
                    .parse()
                    .map_err(|_| "invalid scratch bytes")?,
            )?
        } else {
            if scratch.is_some() {
                return Err("read-only check does not reserve writes".into());
            }
            tos_compiler::research_execution::ResearchExecution::new(&root, seconds)?
        };
        let argv = serde_json::json!(std::env::args().collect::<Vec<_>>());
        tos_compiler::authored_canon_bridge::run(
            &ctx,
            tos_compiler::authored_canon_bridge::Options {
                build,
                input_root: &input,
                output_root: &output,
                plan_ref: plan.as_deref(),
                event_id: event.as_deref(),
                argv: &argv,
                runtime_version: env!("CARGO_PKG_VERSION"),
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
            let _ = writeln!(err, "Authored canon bridge refused: {error}");
            1
        }
    })
}
