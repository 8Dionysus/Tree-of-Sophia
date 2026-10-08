//! Explicit native DTA technical structure entry; no implicit source discovery.
use std::{io::Write, path::PathBuf};
use tos_compiler::{
    dta_technical_markup::{self, Action, Options},
    research_execution::ResearchExecution,
};
const HELP: &str = "tos dta-technical-markup --source-root ABS --build|--check|--validate-tracked [--local-input-root ABS --local-output-root ABS] [--plan REPO_PATH] [--event-id ID] [--issue-identities] [--max-seconds 1..600] [--scratch-bytes RESERVED_BYTES]\n\nObserved TEI structure, exact private text layers and stable opaque identities.\nBuild/check require explicit local input and output roots. Tracked validation reads no private payload. Fresh native output requires a new event ID and fresh plan-selected paths; existing differing outputs are preserved. Build needs an admitted scratch quota. This route grants no textual, linguistic, translation or publication admission.\n";
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if args.first().map(String::as_str) != Some("dta-technical-markup") {
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
        let mut source = None;
        let mut input = None;
        let mut output = None;
        let mut mode = None;
        let mut plan = None;
        let mut event = None;
        let mut scratch = None;
        let mut seconds = None;
        let mut issue = false;
        let mut it = args.iter().skip(1);
        while let Some(k) = it.next() {
            if matches!(k.as_str(), "--build" | "--check" | "--validate-tracked") {
                if mode.replace(k.as_str()).is_some() {
                    return Err("choose exactly one action".into());
                }
                continue;
            }
            if k == "--issue-identities" {
                if issue {
                    return Err("duplicate issuance flag".into());
                }
                issue = true;
                continue;
            }
            let value = it.next().ok_or_else(|| format!("missing value for {k}"))?;
            let selected = match k.as_str() {
                "--source-root" | "--repo-root" => &mut source,
                "--local-input-root" => &mut input,
                "--local-output-root" => &mut output,
                "--plan" => &mut plan,
                "--event-id" => &mut event,
                "--scratch-bytes" => &mut scratch,
                "--max-seconds" => &mut seconds,
                _ => return Err(format!("unknown option {k}")),
            };
            if selected.replace(value.clone()).is_some() {
                return Err(format!("duplicate option {k}"));
            }
        }
        let root = PathBuf::from(source.ok_or("source root required")?);
        let input = input.map(PathBuf::from);
        let output = output.map(PathBuf::from);
        if !root.is_absolute()
            || input.as_ref().is_some_and(|p| !p.is_absolute())
            || output.as_ref().is_some_and(|p| !p.is_absolute())
        {
            return Err("roots must be absolute".into());
        }
        let action = match mode.ok_or("action required")? {
            "--build" => Action::Build {
                issue_identities: issue,
            },
            "--check" if !issue => Action::Check,
            "--validate-tracked" if !issue => Action::ValidateTracked,
            _ => return Err("issuance is valid only with build".into()),
        };
        let seconds = seconds
            .as_deref()
            .unwrap_or("180")
            .parse::<u64>()
            .map_err(|_| "invalid seconds")?;
        let ctx = if matches!(action, Action::Build { .. }) {
            ResearchExecution::new_with_scratch(
                &root,
                seconds,
                scratch
                    .ok_or("build requires admitted scratch-bytes")?
                    .parse::<u64>()
                    .map_err(|_| "invalid scratch bytes")?,
            )?
        } else {
            if scratch.is_some() {
                return Err("check does not reserve writes".into());
            }
            ResearchExecution::new(&root, seconds)?
        };
        dta_technical_markup::run(
            &ctx,
            Options {
                plan_ref: plan.as_deref().unwrap_or(dta_technical_markup::PLAN),
                input_root: input.as_deref(),
                output_root: output.as_deref(),
                action,
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
            let _ = writeln!(err, "DTA technical markup refused: {e}");
            1
        }
    })
}
