//! Source-owned strict public packet comparison. HTTP supervision supplies raw
//! bounded files; this owner reuses FND's grammar, numeric kinds and ordered keys.
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json_with_state_budget};

const COMMAND: &str = "verify-public-packets";
#[derive(Clone, Copy)]
enum Shape {
    Json,
    JsonLines,
    Csv,
}
struct Options {
    actual: PathBuf,
    expected: PathBuf,
    source_root: String,
    shape: Shape,
    json: JsonLimits,
    state_bytes: usize,
    deadline: Instant,
}
fn active(deadline: Instant) -> Result<(), String> {
    if Instant::now() >= deadline {
        Err("public packet comparison deadline exceeded".into())
    } else {
        Ok(())
    }
}
fn options(args: &[String]) -> Result<Options, String> {
    let mut fields = std::collections::BTreeMap::new();
    for pair in args[1..].chunks(2) {
        if pair.len() != 2 || fields.insert(pair[0].as_str(), pair[1].as_str()).is_some() {
            return Err("comparator options must be unique flag/value pairs".into());
        }
    }
    let take = |name| {
        fields
            .get(name)
            .copied()
            .ok_or_else(|| format!("required {name}"))
    };
    let positive = |name| -> Result<usize, String> {
        let value = take(name)?
            .parse::<usize>()
            .map_err(|_| format!("invalid {name}"))?;
        if value == 0 {
            Err(format!("{name} must be positive"))
        } else {
            Ok(value)
        }
    };
    let milliseconds = positive("--max-milliseconds")?;
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(
            u64::try_from(milliseconds).map_err(|_| "deadline overflow")?,
        ))
        .ok_or("deadline overflow")?;
    let json = JsonLimits::new(
        positive("--max-bytes")?,
        positive("--max-depth")?,
        positive("--max-visits")?,
        positive("--max-integer-digits")?,
    )
    .map_err(|error| error.to_string())?;
    let state_bytes = positive("--max-state-bytes")?;
    let actual = PathBuf::from(take("--actual")?);
    let expected = PathBuf::from(take("--expected")?);
    let source_root = take("--source-root")?.to_owned();
    if !actual.is_absolute()
        || !expected.is_absolute()
        || !Path::new(&source_root).is_absolute()
        || source_root.ends_with('/')
    {
        return Err("selected paths require absolute files and non-trailing source root".into());
    }
    let shape = match take("--shape")? {
        "json" => Shape::Json,
        "jsonl" => Shape::JsonLines,
        "csv" => Shape::Csv,
        _ => return Err("shape must be json/jsonl/csv".into()),
    };
    if fields.len() != 10 {
        return Err("unknown comparator option".into());
    }
    Ok(Options {
        actual,
        expected,
        source_root,
        shape,
        json,
        state_bytes,
        deadline,
    })
}
fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
fn read_held(path: &Path, cap: usize, deadline: Instant) -> Result<Vec<u8>, String> {
    active(deadline)?;
    let file: File = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| e.to_string())?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    if !before.is_file() || before.len() > cap as u64 {
        return Err("packet file absent or oversized".into());
    }
    let mut raw = Vec::new();
    let mut reader = &file;
    let mut chunk = [0u8; 64 * 1024];
    loop {
        active(deadline)?;
        let remaining = cap
            .checked_add(1)
            .and_then(|bound| bound.checked_sub(raw.len()))
            .ok_or("packet cap overflow")?;
        let chunk_size = remaining.min(chunk.len());
        let count = reader
            .read(&mut chunk[..chunk_size])
            .map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        if raw.len().checked_add(count).is_none_or(|size| size > cap) {
            return Err("packet file grew beyond cap".into());
        }
        raw.extend_from_slice(&chunk[..count]);
    }
    let after = file.metadata().map_err(|e| e.to_string())?;
    let named = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !same_file(&before, &after) || !same_file(&before, &named) || !named.is_file() {
        return Err("packet file changed during read".into());
    }
    active(deadline)?;
    Ok(raw)
}
fn normalized<'a>(value: &'a str, root: &str) -> &'a str {
    if value == root {
        return "Tree-of-Sophia";
    }
    value
        .strip_prefix(root)
        .and_then(|tail| tail.strip_prefix('/'))
        .unwrap_or(value)
}
fn equal(a: &JsonValue, b: &JsonValue, root: &str, deadline: Instant) -> Result<bool, String> {
    active(deadline)?;
    Ok(match (a, b) {
        (JsonValue::Null, JsonValue::Null) => true,
        (JsonValue::Bool(a), JsonValue::Bool(b)) => a == b,
        (JsonValue::Number(a), JsonValue::Number(b)) => a.kind == b.kind && a.lexeme == b.lexeme,
        (JsonValue::String(a), JsonValue::String(b)) => match (a.as_str(), b.as_str()) {
            (Some(a), Some(b)) => normalized(a, root) == normalized(b, root),
            _ => a == b,
        },
        (JsonValue::Array(a), JsonValue::Array(b)) => {
            if a.len() != b.len() {
                false
            } else {
                let mut same = true;
                for (a, b) in a.iter().zip(b) {
                    if !equal(a, b, root, deadline)? {
                        same = false;
                        break;
                    }
                }
                same
            }
        }
        (JsonValue::Object(a), JsonValue::Object(b)) => {
            if a.len() != b.len() {
                false
            } else {
                let mut same = true;
                for ((ka, a), (kb, b)) in a.iter().zip(b) {
                    if ka != kb || !equal(a, b, root, deadline)? {
                        same = false;
                        break;
                    }
                }
                same
            }
        }
        _ => false,
    })
}
fn compare_json(a: &[u8], b: &[u8], options: &Options, visits: &mut usize) -> Result<(), String> {
    active(options.deadline)?;
    let remaining = options
        .json
        .max_visits
        .checked_sub(*visits)
        .filter(|v| *v > 0)
        .ok_or("packet aggregate visit cap exceeded")?;
    let mut limits = options.json;
    limits.max_visits = remaining;
    let left = parse_json_with_state_budget(
        a,
        JsonMode::PublishedStrict,
        limits,
        options.state_bytes / 2,
    )
    .map_err(|e| format!("actual packet: {e}"))?;
    active(options.deadline)?;
    *visits = visits.checked_add(left.visits()).ok_or("visit overflow")?;
    limits.max_visits = options
        .json
        .max_visits
        .checked_sub(*visits)
        .filter(|v| *v > 0)
        .ok_or("packet aggregate visit cap exceeded")?;
    let right = parse_json_with_state_budget(
        b,
        JsonMode::PublishedStrict,
        limits,
        options.state_bytes / 2,
    )
    .map_err(|e| format!("expected packet: {e}"))?;
    *visits = visits.checked_add(right.visits()).ok_or("visit overflow")?;
    if left.root().as_object().is_none() || right.root().as_object().is_none() {
        return Err("packet requires JSON objects".into());
    }
    if !equal(
        left.root(),
        right.root(),
        &options.source_root,
        options.deadline,
    )? {
        return Err("public packet ordered keys/numeric lexemes/shape/value differ".into());
    }
    Ok(())
}
fn rows(raw: &[u8]) -> impl Iterator<Item = &[u8]> {
    raw.split(|byte| *byte == b'\n')
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
}
fn compare(actual: &[u8], expected: &[u8], options: &Options) -> Result<(), String> {
    let mut visits = 0;
    match options.shape {
        Shape::Json => compare_json(actual, expected, options, &mut visits)?,
        Shape::JsonLines => {
            let mut a = rows(actual);
            let mut b = rows(expected);
            loop {
                match (a.next(), b.next()) {
                    (None, None) => break,
                    (Some(a), Some(b)) => compare_json(a, b, options, &mut visits)?,
                    _ => return Err("JSONL row count differs".into()),
                }
            }
        }
        Shape::Csv => {
            active(options.deadline)?;
            std::str::from_utf8(actual).map_err(|_| "actual CSV invalid UTF8")?;
            std::str::from_utf8(expected).map_err(|_| "expected CSV invalid UTF8")?;
            if actual != expected
                || actual
                    .split(|byte| *byte == b'\n')
                    .next()
                    .is_none_or(|row| row.iter().all(u8::is_ascii_whitespace))
            {
                return Err("full CSV packet/header differs".into());
            }
        }
    }
    active(options.deadline)
}
pub fn run_if_requested(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    if args.first().is_none_or(|command| command != COMMAND) {
        return None;
    }
    let result = (|| {
        let selected = options(args)?;
        let actual = read_held(&selected.actual, selected.json.max_bytes, selected.deadline)?;
        let expected = read_held(
            &selected.expected,
            selected.json.max_bytes,
            selected.deadline,
        )?;
        compare(&actual, &expected, &selected)?;
        writeln!(
            stdout,
            "{{\"schema\":\"tos_public_packet_comparison_v1\",\"equal\":true}}"
        )
        .map_err(|e| e.to_string())
    })();
    Some(match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(stderr, "native public packet comparator: {error}");
            2
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn selected(shape: Shape) -> Options {
        Options {
            actual: PathBuf::new(),
            expected: PathBuf::new(),
            source_root: "/selected".into(),
            shape,
            json: JsonLimits::new(8192, 64, 10000, 4300).unwrap(),
            state_bytes: 1024 * 1024,
            deadline: Instant::now() + Duration::from_secs(1),
        }
    }
    #[test]
    fn typed_lexemes_magnitude_and_object_order_are_preserved() {
        let options = selected(Shape::Json);
        for (actual, expected) in [
            (
                br#"{"n":9007199254740993}"#.as_slice(),
                br#"{"n":9007199254740992}"#.as_slice(),
            ),
            (br#"{"n":1.0}"#.as_slice(), br#"{"n":1}"#.as_slice()),
            (br#"{"n":-0.0}"#.as_slice(), br#"{"n":0.0}"#.as_slice()),
            (
                br#"{"10":1,"2":2}"#.as_slice(),
                br#"{"2":2,"10":1}"#.as_slice(),
            ),
        ] {
            assert!(compare(actual, expected, &options).is_err());
        }
        assert!(
            compare(
                br#"{"n":9007199254740993}"#,
                br#"{"n":9007199254740993}"#,
                &options
            )
            .is_ok()
        );
    }
    #[test]
    fn prefix_normalization_preserves_keys_and_wtf16() {
        let options = selected(Shape::Json);
        assert!(
            compare(
                br#"{"root":"/selected/ToS/a","s":"\ud800"}"#,
                br#"{"root":"ToS/a","s":"\ud800"}"#,
                &options
            )
            .is_ok()
        );
        assert!(
            compare(
                br#"{"root":"/selected-other/ToS/a"}"#,
                br#"{"root":"ToS/a"}"#,
                &options
            )
            .is_err()
        );
        assert!(
            compare(
                br#"{"root":"/selected"}"#,
                br#"{"root":"Tree-of-Sophia"}"#,
                &options
            )
            .is_ok()
        );
    }
    #[test]
    fn jsonl_csv_utf8_and_duplicate_failure_controls() {
        assert!(
            compare(
                b"{\"n\":1}\n",
                b"{\"n\":1}\n{\"n\":2}\n",
                &selected(Shape::JsonLines)
            )
            .is_err()
        );
        assert!(compare(b"header\nA\n", b"header\nB\n", &selected(Shape::Csv)).is_err());
        assert!(
            compare(
                b"{\"s\":\"\xff\"}",
                b"{\"s\":\"\xff\"}",
                &selected(Shape::Json)
            )
            .is_err()
        );
        assert!(
            compare(
                br#"{"n":1,"n":1}"#,
                br#"{"n":1,"n":1}"#,
                &selected(Shape::Json)
            )
            .is_err()
        );
    }
}
