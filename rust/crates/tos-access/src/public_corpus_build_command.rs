use std::{io::Write, path::PathBuf};

pub const HELP: &str = "tos-access public-corpus-build --source-root ABS --selection REL --output FRESH_ABS --max-input-bytes N --max-output-bytes N --max-seconds N\n\nBuild a deterministic read-only public corpus snapshot from explicitly selected source text packets. Every source, packet, raw text and rights digest is checked. Private/local-only material is refused. This operation does not create or admit source identities, assess text, or grant publication rights.\n";

pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        let _ = out.write_all(HELP.as_bytes());
        return 0;
    }
    if args.first().map(String::as_str) != Some("public-corpus-build") {
        return 0;
    }
    let mut fields = std::collections::BTreeMap::<&str, &str>::new();
    let mut it = args[1..].chunks_exact(2);
    for pair in &mut it {
        if pair[0].starts_with("--") && fields.insert(pair[0].as_str(), pair[1].as_str()).is_none()
        {
        } else {
            let _ = writeln!(err, "invalid or duplicate option");
            return 2;
        }
    }
    if !it.remainder().is_empty() || fields.len() != 6 {
        let _ = err.write_all(HELP.as_bytes());
        return 2;
    }
    let get = |k: &str| {
        fields
            .get(k)
            .copied()
            .ok_or_else(|| format!("required {k}"))
    };
    let parse = |k: &str| -> Result<usize, String> {
        let n = get(k)?
            .parse::<usize>()
            .map_err(|_| format!("invalid {k}"))?;
        if n == 0 {
            Err(format!("{k} must be positive"))
        } else {
            Ok(n)
        }
    };
    let result = (|| -> Result<serde_json::Value, String> {
        let root = PathBuf::from(get("--source-root")?);
        let selection = PathBuf::from(get("--selection")?);
        let output = PathBuf::from(get("--output")?);
        if selection.is_absolute() {
            return Err("--selection must be source-root relative".into());
        }
        let max_seconds = parse("--max-seconds")? as u64;
        tos_compiler::public_corpus_snapshot::build(
            &root,
            &selection,
            &output,
            tos_compiler::public_corpus_snapshot::Limits {
                max_input_bytes: parse("--max-input-bytes")?,
                max_output_bytes: parse("--max-output-bytes")?,
                max_seconds,
            },
        )
    })();
    match result {
        Ok(value) => {
            let _ = writeln!(out, "{}", value);
            0
        }
        Err(message) => {
            let _ = writeln!(err, "public corpus build refused: {message}");
            2
        }
    }
}
