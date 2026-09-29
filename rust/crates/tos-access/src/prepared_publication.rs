//! Offline framed-input adapter for the maintained local publication owner.
//! Rows stream twice into the same compiler operation; the adapter neither
//! assembles a graph nor selects a consumer. No runtime owner is implied.
use std::{
    io::{BufRead, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tos_compiler::local_prepared::{self, PreparedChange, PreparedRows, PublicationLimits};
use tos_foundation::{JsonLimits, JsonMode, JsonValue, emit_python_compact_json, parse_json};

fn parse(raw: &[u8], cap: usize) -> Result<JsonValue, String> {
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4096).map_err(|e| e.to_string())?;
    Ok(parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| e.to_string())?
        .root()
        .clone())
}
fn line(input: &mut dyn BufRead, cap: usize, deadline: Instant) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    loop {
        if Instant::now() >= deadline {
            return Err("prepared input deadline".into());
        }
        let buf = input.fill_buf().map_err(|e| e.to_string())?;
        if buf.is_empty() {
            return Err("incomplete prepared input".into());
        }
        let end = buf.iter().position(|b| *b == b'\n');
        let count = end.unwrap_or(buf.len());
        if output.len().checked_add(count).is_none_or(|n| n > cap) {
            return Err("prepared input frame byte budget".into());
        }
        output.extend_from_slice(&buf[..count]);
        input.consume(count + usize::from(end.is_some()));
        if end.is_some() {
            return Ok(output);
        }
    }
}
fn field<'a>(value: &'a JsonValue, key: &str) -> Result<&'a JsonValue, String> {
    value
        .object_get(key)
        .ok_or_else(|| format!("prepared input missing {key}"))
}
fn text<'a>(value: &'a JsonValue, key: &str) -> Result<&'a str, String> {
    field(value, key)?
        .as_str()
        .ok_or_else(|| format!("prepared input {key} must be a string"))
}
fn uint(value: &JsonValue) -> Result<u64, String> {
    match value {
        JsonValue::Number(n) if n.kind == tos_foundation::JsonNumberKind::Int => n
            .lexeme
            .parse()
            .map_err(|_| "prepared input unsigned integer".into()),
        _ => Err("prepared input unsigned integer".into()),
    }
}
fn exact(value: &JsonValue, keys: &[&str]) -> Result<(), String> {
    let fields = value.as_object().ok_or("prepared object required")?;
    if fields.len() != keys.len()
        || fields
            .iter()
            .any(|(k, _)| !k.as_str().is_some_and(|s| keys.contains(&s)))
    {
        return Err("prepared exact input frame fields".into());
    }
    Ok(())
}
fn encoded(value: &JsonValue, cap: usize) -> Result<Vec<u8>, String> {
    emit_python_compact_json(
        value,
        JsonLimits::new(cap, 96, 1_000_000, 4096).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn path(raw: &str) -> Result<PathBuf, String> {
    let p = Path::new(raw);
    if !p.is_absolute() || p.file_name().is_none() {
        return Err("prepared absolute file path required".into());
    }
    let parent = p.parent().ok_or("prepared parent required")?;
    if parent.is_symlink() || !parent.is_dir() {
        return Err("prepared existing non-symlink parent required".into());
    }
    Ok(p.to_owned())
}
struct StreamRows<'a> {
    input: &'a mut dyn BufRead,
    deadline: Instant,
    row_cap: usize,
}
impl PreparedRows for StreamRows<'_> {
    fn visit(
        &mut self,
        _kind: &str,
        sink: &mut dyn FnMut(&JsonValue) -> tos_compiler::Result<()>,
    ) -> tos_compiler::Result<()> {
        loop {
            let raw = line(
                self.input,
                self.row_cap
                    .checked_add(32)
                    .ok_or(tos_compiler::Error::Budget("prepared row frame"))?,
                self.deadline,
            )
            .map_err(tos_compiler::Error::Source)?;
            let value = parse(&raw, self.row_cap + 32).map_err(tos_compiler::Error::Source)?;
            if value.object_get("end") == Some(&JsonValue::Bool(true)) {
                exact(&value, &["end"]).map_err(tos_compiler::Error::Source)?;
                return Ok(());
            }
            exact(&value, &["row"]).map_err(tos_compiler::Error::Source)?;
            sink(field(&value, "row").map_err(tos_compiler::Error::Source)?)?;
        }
    }
}
fn decode_change(raw: &[u8], cap: usize) -> Result<PreparedChange, String> {
    let value = parse(raw, cap)?;
    exact(
        &value,
        &["operation", "kind", "identifier", "item", "source_order"],
    )?;
    let item = field(&value, "item")?;
    let order = field(&value, "source_order")?;
    Ok(PreparedChange {
        operation: text(&value, "operation")?.to_owned(),
        kind: text(&value, "kind")?.to_owned(),
        identifier: text(&value, "identifier")?.to_owned(),
        item: if *item == JsonValue::Null {
            None
        } else {
            Some(item.clone())
        },
        source_order: if *order == JsonValue::Null {
            None
        } else {
            Some(uint(order)?)
        },
    })
}
fn run(input: &mut dyn BufRead, stdout: &mut dyn Write, seconds: u64) -> Result<(), String> {
    // The caller supplies an explicit whole-operation deadline before any path or SQL write.
    let started = Instant::now();
    if seconds == 0 {
        return Err("prepared positive max-seconds required".into());
    }
    let deadline = started
        .checked_add(Duration::from_secs(seconds))
        .ok_or("prepared deadline range")?;
    let raw = line(input, 16_843_008, deadline)?;
    let frame = parse(&raw, 16_843_008)?;
    if uint(field(&frame, "max_seconds")?)? != seconds {
        return Err("prepared stdin/argv deadline differs".into());
    }
    if seconds == 0 {
        return Err("prepared positive max_seconds required".into());
    }
    let deadline = started
        .checked_add(Duration::from_secs(seconds))
        .ok_or("prepared deadline range")?;
    if Instant::now() >= deadline {
        return Err("prepared deadline expired".into());
    }
    let limits: PublicationLimits =
        serde_json::from_slice(&encoded(field(&frame, "limits")?, 4096)?)
            .map_err(|e| e.to_string())?;
    limits.validate().map_err(|e| e.to_string())?;
    let operation = text(&frame, "operation")?;
    let path = path(text(&frame, "path")?)?;
    let header = field(&frame, "header")?;
    let catalog = field(&frame, "catalog")?;
    let binding = match operation {
        "bootstrap" => {
            exact(
                &frame,
                &[
                    "operation",
                    "path",
                    "header",
                    "catalog",
                    "limits",
                    "max_seconds",
                ],
            )?;
            let mut rows = StreamRows {
                input,
                deadline,
                row_cap: limits.max_row_bytes,
            };
            local_prepared::publish_prepared_rows_until(
                &path, header, catalog, &mut rows, limits, deadline,
            )
            .map_err(|e| e.to_string())?
        }
        "delta" => {
            exact(
                &frame,
                &[
                    "operation",
                    "path",
                    "header",
                    "catalog",
                    "limits",
                    "max_seconds",
                    "expected_binding",
                ],
            )?;
            let mut changes = Vec::new();
            let mut bytes = 0usize;
            loop {
                let raw = line(input, limits.max_row_bytes.saturating_add(32768), deadline)?;
                if raw == b"{\"end\":true}" {
                    break;
                }
                if changes.len() >= limits.max_changes {
                    return Err("prepared change count budget".into());
                }
                bytes = bytes
                    .checked_add(raw.len())
                    .filter(|n| {
                        *n <= limits
                            .max_change_bytes
                            .saturating_add(limits.max_metadata_bytes)
                    })
                    .ok_or("prepared retained input change bytes")?;
                changes.push(raw);
            }
            local_prepared::apply_prepared_delta_until(
                &path,
                field(&frame, "expected_binding")?,
                header,
                catalog,
                changes.into_iter().map(|raw| {
                    decode_change(&raw, limits.max_row_bytes.saturating_add(32768))
                        .map_err(tos_compiler::Error::Source)
                }),
                limits,
                deadline,
            )
            .map_err(|e| e.to_string())?
        }
        _ => return Err("prepared operation must be bootstrap or delta".into()),
    };
    let output = encoded(&binding, 65536)?;
    stdout
        .write_all(&output)
        .and_then(|()| stdout.write_all(b"\n"))
        .map_err(|e| e.to_string())
}
pub fn run_if_requested(
    args: &[String],
    input: &mut dyn BufRead,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    if args.first().is_none_or(|a| a != "prepared-publication") {
        return None;
    }
    Some(if args.len() != 3 || args[1] != "--max-seconds" {
        let _ = writeln!(
            stderr,
            "prepared-publication --max-seconds N takes bounded framed stdin only"
        );
        2
    } else {
        match args[2]
            .parse::<u64>()
            .map_err(|_| "invalid prepared max-seconds".to_owned())
            .and_then(|seconds| run(input, stdout, seconds))
        {
            Ok(()) => 0,
            Err(error) => {
                let _ = writeln!(stderr, "prepared publication: {error}");
                2
            }
        }
    })
}
