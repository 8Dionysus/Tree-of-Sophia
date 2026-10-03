//! Explicit source-selected research producers. Generated candidates retain
//! their source, provenance and review boundaries; this command grants no
//! semantic, rights, publication or canon admission.

use std::{io::Write, path::PathBuf};

pub const COMMANDS: &[&str] = &[
    "zarathustra-reading-workbench-v1",
    "zarathustra-parallel-lexical-candidates-v1",
    "zarathustra-de-ru-paragraph-alignment-v1",
    "zarathustra-concept-workbench-v1",
    "zarathustra-eternal-return-review-preparation-v1",
    "zarathustra-morphology-theme-candidates-v1",
    "zarathustra-eternal-return-concept-candidate-v1",
];

pub const HELP: &str = "usage: tos COMMAND --source-root ABSOLUTE_SOURCE_DIRECTORY [--max-seconds 1..600] [--scratch-bytes RESERVED_REMAINING_BYTES] [PRODUCER_OPTIONS]\n\nResearch producers:\n  zarathustra-reading-workbench-v1\n  zarathustra-parallel-lexical-candidates-v1\n  zarathustra-de-ru-paragraph-alignment-v1\n  zarathustra-concept-workbench-v1\n  zarathustra-eternal-return-review-preparation-v1\n  zarathustra-morphology-theme-candidates-v1\n  zarathustra-eternal-return-concept-candidate-v1\n\nThe source directory is explicit. Default whole-operation deadline: 180 seconds. Writes and SQLite production require an explicit remaining scratch quota after the carrier baseline; this option does not grant storage. Private source material remains private.\nGenerated candidates do not grant semantic, rights, publication or canon admission.\n";

fn entry_identity(command: &str) -> (&'static str, &'static [u8]) {
    match command {
        "zarathustra-reading-workbench-v1" => (
            "research_reading_workbench.rs",
            include_bytes!("../../tos-compiler/src/research_reading_workbench.rs"),
        ),
        "zarathustra-parallel-lexical-candidates-v1" => (
            "research_parallel_lexical.rs",
            include_bytes!("../../tos-compiler/src/research_parallel_lexical.rs"),
        ),
        "zarathustra-de-ru-paragraph-alignment-v1" => (
            "research_paragraph_alignment.rs",
            include_bytes!("../../tos-compiler/src/research_paragraph_alignment.rs"),
        ),
        "zarathustra-concept-workbench-v1" => (
            "research_concept_workbench.rs",
            include_bytes!("../../tos-compiler/src/research_concept_workbench.rs"),
        ),
        "zarathustra-eternal-return-review-preparation-v1" => (
            "research_eternal_return.rs",
            include_bytes!("../../tos-compiler/src/research_eternal_return.rs"),
        ),
        "zarathustra-morphology-theme-candidates-v1" => (
            "research_morphology_theme.rs",
            include_bytes!("../../tos-compiler/src/research_morphology_theme.rs"),
        ),
        "zarathustra-eternal-return-concept-candidate-v1" => (
            "research_eternal_return_concept.rs",
            include_bytes!("../../tos-compiler/src/research_eternal_return_concept.rs"),
        ),
        _ => unreachable!("recognized research producer"),
    }
}

fn selected_args(args: &[String]) -> Result<(PathBuf, u64, Option<u64>, Vec<String>), String> {
    let mut root = None;
    let mut producer = Vec::new();
    let mut max_seconds = None;
    let mut scratch_bytes = None;
    let mut options = args.iter().skip(1);
    while let Some(option) = options.next() {
        if option == "--source-root" {
            let value = options.next().ok_or("--source-root requires a value")?;
            if root.replace(PathBuf::from(value)).is_some() {
                return Err("duplicate --source-root".into());
            }
        } else if option == "--max-seconds" {
            let value = options.next().ok_or("--max-seconds requires a value")?;
            let seconds = value.parse::<u64>().map_err(|_| "invalid --max-seconds")?;
            if !(1..=600).contains(&seconds) || max_seconds.replace(seconds).is_some() {
                return Err("--max-seconds must occur once and be 1..600".into());
            }
        } else if option == "--scratch-bytes" {
            let value = options.next().ok_or("--scratch-bytes requires a value")?;
            let bytes = value
                .parse::<u64>()
                .map_err(|_| "invalid --scratch-bytes")?;
            if bytes == 0 || bytes == u64::MAX || scratch_bytes.replace(bytes).is_some() {
                return Err(
                    "--scratch-bytes must occur once with finite positive remaining bytes".into(),
                );
            }
        } else {
            producer.push(option.clone());
        }
    }
    let root = root.ok_or("--source-root is required")?;
    if !root.is_absolute() || root.is_symlink() {
        return Err("research source root must be an absolute directory without a symlink".into());
    }
    Ok((root, max_seconds.unwrap_or(180), scratch_bytes, producer))
}

pub fn run_if_requested(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    let command = args.first()?.as_str();
    if !COMMANDS.contains(&command) {
        return None;
    }
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        return Some(if write!(stdout, "{HELP}").is_ok() {
            0
        } else {
            2
        });
    }
    let result = selected_args(args).and_then(|(root, seconds, scratch_bytes, producer)| {
        let root = if command == "zarathustra-reading-workbench-v1" {
            tos_compiler::research_execution::ResearchExecution::new_reading_v1(
                &root,
                seconds,
                scratch_bytes,
            )?
        } else {
            match scratch_bytes {
                Some(bytes) => {
                    tos_compiler::research_execution::ResearchExecution::new_with_scratch(
                        &root, seconds, bytes,
                    )?
                }
                None => tos_compiler::research_execution::ResearchExecution::new(&root, seconds)?,
            }
        };
        let value = match command {
            "zarathustra-reading-workbench-v1" => {
                tos_compiler::research_reading_workbench::run_scoped(&root, &producer)
            }
            "zarathustra-parallel-lexical-candidates-v1" => {
                tos_compiler::research_parallel_lexical::run_scoped(&root, &producer)
            }
            "zarathustra-de-ru-paragraph-alignment-v1" => {
                tos_compiler::research_paragraph_alignment::run_scoped(&root, &producer)
            }
            "zarathustra-concept-workbench-v1" => {
                tos_compiler::research_concept_workbench::run_scoped(&root, &producer)
            }
            "zarathustra-eternal-return-review-preparation-v1" => {
                tos_compiler::research_eternal_return::run_scoped(&root, &producer)
            }
            "zarathustra-morphology-theme-candidates-v1" => {
                tos_compiler::research_morphology_theme::run_scoped(&root, &producer)
            }
            "zarathustra-eternal-return-concept-candidate-v1" => {
                tos_compiler::research_eternal_return_concept::run_scoped(&root, &producer)
            }
            _ => unreachable!("recognized research producer"),
        }
        .map_err(|error| format!("{error}; execution_budget={}", root.budget_report()))?;
        Ok((value, root.budget_report()))
    });
    Some(match result {
        Ok((mut value, budget)) => {
            // The output files render their frozen v1 recipe. The executing
            // implementation is disclosed separately, without rewriting that
            // historical provenance or implying new source/review admission.
            let (entry, raw) = entry_identity(command);
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    "native_execution".into(),
                    serde_json::json!({
                        "schema_version": "tos_native_research_execution_v1",
                        "command": command,
                        "kernel_entry_ref": format!("rust/crates/tos-compiler/src/{entry}"),
                        "kernel_entry_sha256": tos_foundation::Digest256::of_bytes(raw).to_hex(),
                        "output_provenance_posture": "frozen_v1_recipe_rendering",
                        "execution_budget": budget,
                        "semantic_admission": false,
                        "canon_admission": false
                    }),
                );
            }
            match writeln!(stdout, "{value}") {
                Ok(()) => 0,
                Err(_) => 2,
            }
        }
        Err(error) => {
            let _ = writeln!(stderr, "research producer: {error}");
            2
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_route_requires_explicit_absolute_root_before_production() {
        assert!(selected_args(&[COMMANDS[0].into(), "--preview".into()]).is_err());
        assert!(selected_args(&[COMMANDS[0].into(), "--source-root".into(), ".".into()]).is_err());
        assert!(
            selected_args(&[
                COMMANDS[0].into(),
                "--source-root".into(),
                "/".into(),
                "--source-root".into(),
                "/".into()
            ])
            .is_err()
        );
    }

    #[test]
    fn unrelated_commands_are_not_research_operations() {
        assert_eq!(
            run_if_requested(&["source".into()], &mut Vec::new(), &mut Vec::new()),
            None
        );
    }
}
