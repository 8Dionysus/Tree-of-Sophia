//! Installed whole v1 Antonovsky layout builder/checker. Explicit source and
//! scratch selections carry no corpus, publication or storage permission.
use std::{io::Write, path::PathBuf};
use tos_compiler::antonovsky_structural::technical_markup::{Action, run};
const HELP: &str = "tos technical-markup --source-root ABS (--build [--issue-identities]|--check|--validate-tracked) [--max-seconds 1..600] [--scratch-bytes N]\n\nExact Antonovsky v1 page/panel/block, screening and private-layer producer.\n--build requires an explicitly admitted remaining scratch quota.\n--check includes both private layers and their mode0600; tracked validation opens no PDF.\nHistorical Python bytes are source provenance only, never executable fallback.\n";
fn arguments(args: &[String]) -> Result<(PathBuf, Action, u64, Option<u64>), String> {
    let mut root = None;
    let mut mode = None;
    let mut issue = false;
    let mut seconds = None;
    let mut scratch = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--source-root" | "--repo-root" => {
                i += 1;
                let p = PathBuf::from(args.get(i).ok_or("missing source root")?);
                if !p.is_absolute() || root.replace(p).is_some() {
                    return Err("source root must be absolute and unique".into());
                }
            }
            "--build" | "--check" | "--validate-tracked" => {
                if mode.replace(args[i].as_str()).is_some() {
                    return Err("choose exactly one action".into());
                }
            }
            "--issue-identities" => {
                if issue {
                    return Err("duplicate issuance flag".into());
                }
                issue = true;
            }
            "--max-seconds" => {
                i += 1;
                let n = args
                    .get(i)
                    .ok_or("missing max-seconds")?
                    .parse::<u64>()
                    .map_err(|_| "invalid max-seconds")?;
                if !(1..=600).contains(&n) || seconds.replace(n).is_some() {
                    return Err("max-seconds must be unique and1..600".into());
                }
            }
            "--scratch-bytes" => {
                i += 1;
                let n = args
                    .get(i)
                    .ok_or("missing scratch-bytes")?
                    .parse::<u64>()
                    .map_err(|_| "invalid scratch-bytes")?;
                if n == 0 || scratch.replace(n).is_some() {
                    return Err("scratch-bytes must be positive and unique".into());
                }
            }
            other => return Err(format!("unknown technical-markup option {other}")),
        }
        i += 1;
    }
    let action = match mode.ok_or("missing action")? {
        "--build" => Action::Build {
            issue_identities: issue,
        },
        "--check" if !issue => Action::Check,
        "--validate-tracked" if !issue => Action::ValidateTracked,
        _ => return Err("--issue-identities is valid only with --build".into()),
    };
    if matches!(action, Action::Build { .. }) && scratch.is_none() {
        return Err("--build requires admitted --scratch-bytes quota".into());
    }
    Ok((
        root.ok_or("missing --source-root")?,
        action,
        seconds.unwrap_or(180),
        scratch,
    ))
}
pub fn run_if_requested(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    if args.first().map(String::as_str) != Some("technical-markup") {
        return None;
    }
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        return Some(
            if stdout
                .write_all(HELP.as_bytes())
                .and_then(|_| stdout.flush())
                .is_ok()
            {
                0
            } else {
                1
            },
        );
    }
    let (root, action, seconds, scratch) = match arguments(args) {
        Ok(v) => v,
        Err(error) => {
            let _ = writeln!(stderr, "invalid_request: {error}");
            return Some(2);
        }
    };
    Some(match run(&root, action, seconds, scratch) {
        Ok(raw) => match stdout.write_all(&raw).and_then(|_| stdout.flush()) {
            Ok(()) => 0,
            Err(error) => {
                let _ = writeln!(stderr, "output failed: {error}");
                1
            }
        },
        Err(error) => {
            let _ = writeln!(stderr, "error: {error}");
            1
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }
    #[test]
    fn explicit_source_issuance_and_scratch_contract() {
        assert!(arguments(&args(&["technical-markup", "--check"])).is_err());
        assert!(
            arguments(&args(&[
                "technical-markup",
                "--source-root",
                "/source",
                "--check",
                "--issue-identities"
            ]))
            .is_err()
        );
        assert!(
            arguments(&args(&[
                "technical-markup",
                "--source-root",
                "/source",
                "--build"
            ]))
            .is_err()
        );
        assert!(matches!(
            arguments(&args(&[
                "technical-markup",
                "--source-root",
                "/source",
                "--build",
                "--issue-identities",
                "--scratch-bytes",
                "1048576"
            ]))
            .unwrap()
            .1,
            Action::Build {
                issue_identities: true
            }
        ));
    }
    #[test]
    fn help_is_source_free() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(
            run_if_requested(&args(&["technical-markup", "--help"]), &mut out, &mut err),
            Some(0)
        );
        assert!(!out.is_empty() && err.is_empty());
    }
}
