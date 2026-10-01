//! Named installed CLI caller for the exact Antonovsky structural family.
//! Explicit source selection is local technical derivation, never public data
//! selection, corpus admission, canon or publication permission.
use std::{io::Write, path::PathBuf};
use tos_compiler::antonovsky_structural::{Action, run};
const HELP: &str = "tos structural-paragraph --source-root ABS (--build|--check|--validate-tracked|--issue-identities|--import-challenger ABS)\n\nReads the exact owner-bound Antonovsky PDF/Poppler layer for reconstruction.\n--validate-tracked reads tracked text-free metadata only.\n--private-model emits private exact source words for maintained local consumers.\nWriting modes require prior host storage admission and explicit owner authority.\n";
fn arguments(args: &[String]) -> Result<(PathBuf, Action), String> {
    let mut root = None;
    let mut action = None;
    let mut i = 1;
    while i < args.len() {
        let option = args[i].as_str();
        if option == "--source-root" {
            if root.is_some() {
                return Err("duplicate --source-root".into());
            }
            i += 1;
            let p = PathBuf::from(args.get(i).ok_or("missing source root")?);
            if !p.is_absolute() {
                return Err("--source-root must be absolute".into());
            }
            root = Some(p);
        } else {
            let next = match option {
                "--build" => Action::Build,
                "--check" => Action::Check,
                "--validate-tracked" => Action::ValidateTracked,
                "--private-model" => Action::PrivateModel,
                "--issue-identities" => Action::IssueIdentities,
                "--import-challenger" => {
                    i += 1;
                    let p = PathBuf::from(args.get(i).ok_or("missing challenger directory")?);
                    if !p.is_absolute() {
                        return Err("challenger directory must be absolute".into());
                    }
                    Action::ImportChallenger(p)
                }
                _ => return Err(format!("unknown structural-paragraph option {option}")),
            };
            if action.replace(next).is_some() {
                return Err("choose exactly one structural-paragraph action".into());
            }
        }
        i += 1;
    }
    Ok((
        root.ok_or("missing --source-root")?,
        action.ok_or("missing action")?,
    ))
}
pub fn run_if_requested(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    if args.first().map(String::as_str) != Some("structural-paragraph") {
        return None;
    }
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        return Some(if stdout.write_all(HELP.as_bytes()).is_ok() {
            0
        } else {
            1
        });
    }
    let (root, action) = match arguments(args) {
        Ok(v) => v,
        Err(e) => {
            let _ = writeln!(stderr, "invalid_request: {e}");
            return Some(2);
        }
    };
    Some(match run(&root, action) {
        Ok(bytes) => match stdout.write_all(&bytes).and_then(|_| stdout.flush()) {
            Ok(()) => 0,
            Err(e) => {
                let _ = writeln!(stderr, "output failed: {e}");
                1
            }
        },
        Err(e) => {
            let _ = writeln!(stderr, "ERROR: {e}");
            1
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).into()).collect()
    }
    #[test]
    fn missing_data_does_not_discover_cwd() {
        assert!(arguments(&args(&["structural-paragraph", "--validate-tracked"])).is_err());
        assert!(
            arguments(&args(&[
                "structural-paragraph",
                "--source-root",
                ".",
                "--check"
            ]))
            .is_err()
        );
    }
    #[test]
    fn mutating_modes_are_explicit_and_exclusive() {
        assert!(
            arguments(&args(&[
                "structural-paragraph",
                "--source-root",
                "/source",
                "--build",
                "--check"
            ]))
            .is_err()
        );
        assert!(matches!(
            arguments(&args(&[
                "structural-paragraph",
                "--source-root",
                "/source",
                "--check"
            ]))
            .unwrap()
            .1,
            Action::Check
        ));
    }
    #[test]
    fn help_opens_no_source() {
        let mut out = vec![];
        let mut err = vec![];
        assert_eq!(
            run_if_requested(
                &args(&["structural-paragraph", "--help"]),
                &mut out,
                &mut err
            ),
            Some(0)
        );
        assert!(err.is_empty());
        assert!(
            String::from_utf8(out)
                .unwrap()
                .contains("--validate-tracked")
        );
    }
}
