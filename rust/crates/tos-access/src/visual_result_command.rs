//! Select retained visual laboratory evidence without starting a provider.
use std::{collections::BTreeMap, io::Write, path::PathBuf};
use tos_compiler::{
    research_execution::ResearchExecution,
    research_visual_result::{self, Options},
};
const HELP: &str = "tos zarathustra-visual-retrieval-result --source-root ABS --artifact-root ABS\n  --run EXPERIMENT/RUN/variant-C --prior-run EXPERIMENT/PRIOR/variant-C\n  --query-content ABS --inspect-run [--max-seconds 1..600]\n  OR the same inputs with --check|--build --model-root ABS --implementation-root ABS\n  [--local-output-root ABS --generation NAME --scratch-bytes RESERVED_BYTES]\n\nReconstruct retained image fixity, vector normalization, source returns, ranking, control comparisons and private-content withholding. Inspection reports model files as unverified and cannot register a full result. Full check/build also verifies the exact model and recorded runner/bridge bytes. No model, Python runtime or network is executed. Build requires a distinct generation and a separate output root; matching replay preserves bytes. Provider review and publication authority remain separate.\n";
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if args.first().map(String::as_str) != Some("zarathustra-visual-retrieval-result") {
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
        let mut options = BTreeMap::new();
        let mut mode = None;
        let mut it = args.iter().skip(1);
        while let Some(k) = it.next() {
            if ["--inspect-run", "--check", "--build"].contains(&k.as_str()) {
                if mode.replace(k.as_str()).is_some() {
                    return Err("choose one action".into());
                }
                continue;
            }
            if ![
                "--source-root",
                "--artifact-root",
                "--run",
                "--prior-run",
                "--query-content",
                "--model-root",
                "--implementation-root",
                "--local-output-root",
                "--generation",
                "--scratch-bytes",
                "--max-seconds",
            ]
            .contains(&k.as_str())
            {
                return Err(format!("unknown option {k}"));
            }
            if options
                .insert(
                    k.as_str(),
                    it.next().ok_or("option value required")?.as_str(),
                )
                .is_some()
            {
                return Err(format!("duplicate option {k}"));
            }
        }
        let get = |k: &str| {
            options
                .get(k)
                .copied()
                .ok_or_else(|| format!("explicit {k} required"))
        };
        let path = |k: &str| -> Result<PathBuf, String> {
            let p = PathBuf::from(get(k)?);
            if !p.is_absolute() {
                return Err(format!("absolute {k} required"));
            }
            Ok(p)
        };
        let mode = mode.ok_or("action required")?;
        let inspect = mode == "--inspect-run";
        let build = mode == "--build";
        let source = path("--source-root")?;
        let artifacts = path("--artifact-root")?;
        let query = path("--query-content")?;
        let generation = options.get("--generation").copied();
        let model = options
            .contains_key("--model-root")
            .then(|| path("--model-root"))
            .transpose()?;
        let implementation = options
            .contains_key("--implementation-root")
            .then(|| path("--implementation-root"))
            .transpose()?;
        let output = options
            .contains_key("--local-output-root")
            .then(|| path("--local-output-root"))
            .transpose()?;
        if inspect
            && (model.is_some()
                || implementation.is_some()
                || output.is_some()
                || generation.is_some())
        {
            return Err(
                "inspection accepts only source, retained artifact and query inputs".into(),
            );
        }
        if !inspect && (model.is_none() || implementation.is_none()) {
            return Err("full result requires explicit model and implementation roots".into());
        }
        if (build || generation.is_some()) && output.is_none() {
            return Err("generation requires explicit separate output root".into());
        }
        let scratch = if build {
            Some(
                get("--scratch-bytes")?
                    .parse()
                    .map_err(|_| "invalid scratch bytes")?,
            )
        } else {
            if options.contains_key("--scratch-bytes") {
                return Err("read does not reserve output bytes".into());
            }
            None
        };
        let seconds = options
            .get("--max-seconds")
            .unwrap_or(&"180")
            .parse()
            .map_err(|_| "invalid seconds")?;
        let ctx = ResearchExecution::new_visual_result(&source, seconds, scratch)?;
        research_visual_result::run(
            &ctx,
            Options {
                inspect,
                build,
                artifact_root: &artifacts,
                run: get("--run")?,
                prior_run: get("--prior-run")?,
                query_content: &query,
                model_root: model.as_deref(),
                implementation_root: implementation.as_deref(),
                output_root: output.as_deref(),
                generation,
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
            let _ = writeln!(err, "visual result refused: {e}");
            1
        }
    })
}
