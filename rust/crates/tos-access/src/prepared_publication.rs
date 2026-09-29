//! Offline framed-input adapter for the maintained local publication owner.
//! Rows stream twice into the same compiler operation; the adapter neither
//! assembles a graph nor selects a consumer. No runtime owner is implied.
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tos_compiler::{
    local_prepared::{self, BootstrapSearch, PreparedChange, PreparedRows, PublicationLimits},
    local_prepared_bulk::BulkBootstrapLimits,
    local_prepared_reuse::PreparedSearchReuse,
};
use tos_foundation::{JsonLimits, JsonMode, JsonValue, emit_python_compact_json, parse_json};

fn parse(raw: &[u8], cap: usize) -> Result<JsonValue, String> {
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4096).map_err(|e| e.to_string())?;
    Ok(parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| e.to_string())?
        .into_root())
}
// The real stdin adapter polls before each bounded read; checking only around
// BufRead::fill_buf cannot interrupt a pipe whose writer remains open and idle.
struct DeadlineStdin {
    deadline: Instant,
}
impl Read for DeadlineStdin {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "prepared input deadline",
                ));
            }
            let mut input = libc::pollfd {
                fd: libc::STDIN_FILENO,
                events: libc::POLLIN,
                revents: 0,
            };
            let timeout = remaining.as_millis().clamp(1, 1000) as i32;
            // Same Linux poll/read mechanism as the maintained native executor.
            // This early CLI command owns stdin; no competing buffered reader.
            let ready = unsafe { libc::poll(&mut input, 1, timeout) };
            if ready < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if ready == 0 {
                continue;
            }
            if Instant::now() >= self.deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "prepared input deadline",
                ));
            }
            if input.revents & libc::POLLNVAL != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "prepared stdin descriptor",
                ));
            }
            let count =
                unsafe { libc::read(libc::STDIN_FILENO, output.as_mut_ptr().cast(), output.len()) };
            if count >= 0 {
                return Ok(count as usize);
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
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
fn bootstrap_search(frame: &JsonValue) -> Result<BootstrapSearch, String> {
    let donor = frame.object_get("search_reuse");
    let scratch = frame.object_get("search_scratch_path");
    let scratch_limits = frame.object_get("search_scratch_limits");
    if donor.is_some() && (scratch.is_some() || scratch_limits.is_some()) {
        return Err("prepared donor and bulk are mutually exclusive".into());
    }
    if scratch.is_some() != scratch_limits.is_some() {
        return Err("prepared bulk requires scratch path and limits".into());
    }
    if let Some(request) = donor {
        exact(
            request,
            &[
                "path",
                "binding",
                "max_source_bytes",
                "max_copy_bytes",
                "max_copy_rows",
                "max_batch_bytes",
                "max_queries",
                "max_vm_steps",
                "max_validation_state_bytes",
            ],
        )?;
        return Ok(BootstrapSearch::Reuse(PreparedSearchReuse {
            path: path(text(request, "path")?)?,
            binding: field(request, "binding")?.clone(),
            max_source_bytes: uint(field(request, "max_source_bytes")?)?,
            max_copy_bytes: uint(field(request, "max_copy_bytes")?)?,
            max_copy_rows: uint(field(request, "max_copy_rows")?)?,
            max_batch_bytes: usize::try_from(uint(field(request, "max_batch_bytes")?)?)
                .map_err(|_| "prepared donor batch size range")?,
            max_queries: uint(field(request, "max_queries")?)?,
            max_vm_steps: uint(field(request, "max_vm_steps")?)?,
            max_validation_state_bytes: usize::try_from(uint(field(
                request,
                "max_validation_state_bytes",
            )?)?)
            .map_err(|_| "prepared donor state size range")?,
            progress: false,
        }));
    }
    if let (Some(scratch), Some(values)) = (scratch, scratch_limits) {
        exact(
            values,
            &[
                "max_bytes",
                "max_mutations",
                "max_cached_terms",
                "max_cached_bytes",
                "batch_size",
                "max_cached_tails",
                "max_tail_bytes",
            ],
        )?;
        let size = |key: &str| -> Result<usize, String> {
            usize::try_from(uint(field(values, key)?)?)
                .map_err(|_| "prepared bulk size range".into())
        };
        return Ok(BootstrapSearch::Bulk {
            scratch_path: path(
                scratch
                    .as_str()
                    .ok_or("prepared scratch path must be a string")?,
            )?,
            limits: BulkBootstrapLimits {
                max_bytes: uint(field(values, "max_bytes")?)?,
                max_mutations: uint(field(values, "max_mutations")?)?,
                max_cached_terms: size("max_cached_terms")?,
                max_cached_bytes: size("max_cached_bytes")?,
                batch_size: size("batch_size")?,
                max_cached_tails: size("max_cached_tails")?,
                max_tail_bytes: size("max_tail_bytes")?,
            },
        });
    }
    Ok(BootstrapSearch::Buffered)
}

fn run(
    input: &mut dyn BufRead,
    stdout: &mut dyn Write,
    seconds: u64,
    deadline: Instant,
) -> Result<(), String> {
    let raw = line(input, 16_843_008, deadline)?;
    let frame = parse(&raw, 16_843_008)?;
    if uint(field(&frame, "max_seconds")?)? != seconds {
        return Err("prepared stdin/argv deadline differs".into());
    }
    if Instant::now() >= deadline {
        return Err("prepared deadline expired".into());
    }
    let values = field(&frame, "limits")?;
    exact(
        values,
        &[
            "max_bytes",
            "max_mutations",
            "max_row_bytes",
            "max_metadata_bytes",
            "max_changes",
            "max_change_bytes",
        ],
    )?;
    let size = |key: &str| -> Result<usize, String> {
        usize::try_from(uint(field(values, key)?)?).map_err(|_| "prepared limits size range".into())
    };
    let limits = PublicationLimits {
        max_bytes: uint(field(values, "max_bytes")?)?,
        max_mutations: uint(field(values, "max_mutations")?)?,
        max_row_bytes: size("max_row_bytes")?,
        max_metadata_bytes: size("max_metadata_bytes")?,
        max_changes: size("max_changes")?,
        max_change_bytes: size("max_change_bytes")?,
    };
    limits.validate().map_err(|e| e.to_string())?;
    let operation = text(&frame, "operation")?;
    let path = path(text(&frame, "path")?)?;
    let header = field(&frame, "header")?;
    let catalog = field(&frame, "catalog")?;
    let binding = match operation {
        "bootstrap" => {
            let mut keys = vec![
                "operation",
                "path",
                "header",
                "catalog",
                "limits",
                "max_seconds",
            ];
            for key in [
                "search_reuse",
                "search_scratch_path",
                "search_scratch_limits",
            ] {
                if frame.object_get(key).is_some() {
                    keys.push(key);
                }
            }
            exact(&frame, &keys)?;
            let search = bootstrap_search(&frame)?;
            let mut rows = StreamRows {
                input,
                deadline,
                row_cap: limits.max_row_bytes,
            };
            local_prepared::publish_prepared_rows_with_search_until(
                &path, header, catalog, &mut rows, limits, search, deadline,
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
            .and_then(|seconds| {
                if seconds == 0 {
                    return Err("prepared positive max-seconds required".into());
                }
                let deadline = Instant::now()
                    .checked_add(Duration::from_secs(seconds))
                    .ok_or("prepared deadline range")?;
                let mut input = BufReader::with_capacity(8192, DeadlineStdin { deadline });
                run(&mut input, stdout, seconds, deadline)
            }) {
            Ok(()) => 0,
            Err(error) => {
                let _ = writeln!(stderr, "prepared publication: {error}");
                2
            }
        }
    })
}
