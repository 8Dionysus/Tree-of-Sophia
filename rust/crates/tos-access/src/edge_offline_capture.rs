//! Installed offline entry for bounded private D1 pair capture.
//!
//! This command keeps read-only SQLite snapshots open while it verifies the
//! prepared-source binding and captures exact partitioned projection bytes.
//! It emits an unapplied SQL candidate only; it does not admit the selected
//! D1/prepared pair, source or rights authority, currentness, or a consumer.

use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior, params,
    params_from_iter,
    types::{Value as SqlValue, ValueRef},
};
use serde_json::{Map, Value, json};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    os::fd::AsRawFd,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
use tos_compiler::{
    SearchBuildLimits,
    d1::{
        D1AuxiliaryStore, D1Cell, D1PairLimits, D1PrivatePreparedInput, D1PrivatePreparedMode,
        D1RowTransition, D1Table, MAX_D1_SQL_ROW_VALUE_BYTES, auxiliary_binding_candidates,
        emit_private_prepared_capture, private_prepared_target_revision,
    },
    d1_prepared_pair::inspect_private_prepared_source_inputs,
    d1_projection_snapshot::{D1ProjectionAccounting, D1ProjectionLimits, D1ProjectionSnapshot},
    local_prepared::PublicationLimits,
    prepared_source_binding::{PreparedSourceInputs, validate_prepared_source_state},
    project_private_knowledge_row, project_private_lens_auxiliary_rows,
    project_private_navigation_row,
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonString, JsonValue,
    canonical_bytes_v1, emit_python_compact_json, parse_json, parse_json_with_state_budget,
};

#[cfg(test)]
mod private_runtime_tests;
mod typed_snapshot;

const REQUEST_SCHEMA: &str = "tos_edge_offline_capture_request_v1";
const TYPED_REQUEST_SCHEMA: &str = "tos_edge_offline_capture_request_v2";
const TYPED_RESULT_SCHEMA: &str = "tos_edge_offline_capture_result_v2";

fn result_schema(request_schema: &str) -> &'static str {
    if request_schema == TYPED_REQUEST_SCHEMA {
        TYPED_RESULT_SCHEMA
    } else {
        "tos_edge_offline_capture_result_v1"
    }
}
// Prepared bindings are each capped at 1 MiB by the source binding validator;
// catch-up source inputs are capped at 1 MiB, and each projection root at
// 256 KiB. Outer string escaping plus fixed paths/fields fit inside this
// catalog-free request envelope. This is a request-file cap, not a heap claim.
const REQUEST_BYTES: usize = 10 * 1024 * 1024;
const REQUEST_JSON_VISITS: usize = 6_500_000;
// Foundation's state budget prices its retained values, strings, vectors,
// object indexes and recursive stack slots. The preflight tree is dropped
// before the serde request tree is constructed; allocator overhead/RSS are a
// separate process resource bound.
const REQUEST_JSON_STATE_BYTES: usize = 2 * 1024 * 1024 * 1024;
const CAPTURE_JSON_VISITS: usize = 6_500_000;
const PREPARED_SOURCE_BYTES: usize = 1_048_576;
const PREPARED_SCHEMA: &str = "tos_local_prepared_read_model_v1";
const D1_SCHEMA: &str = "tos_cloudflare_edge_read_model_v9";

#[derive(Clone, Copy)]
struct Limits {
    prepared: PublicationLimits,
    projection: D1ProjectionLimits,
    pair: D1PairLimits,
    max_postings: usize,
    max_manifest_rows: usize,
}

/// Cumulative SQLite material copied during one native D1 capture. Projection
/// part reads have their own separate cumulative accounting.
struct D1ReadBytes {
    used: u64,
    limit: u64,
}
impl D1ReadBytes {
    fn new(limits: Limits) -> Self {
        Self {
            used: 0,
            limit: limits.prepared.max_bytes,
        }
    }
    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        let bytes = u64::try_from(bytes).map_err(|_| invalid("D1 read byte count"))?;
        self.used = self
            .used
            .checked_add(bytes)
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| invalid("cumulative D1 read byte budget"))?;
        Ok(())
    }

    fn remaining(&self) -> Result<usize, String> {
        usize::try_from(
            self.limit
                .checked_sub(self.used)
                .ok_or_else(|| invalid("cumulative D1 read byte budget"))?,
        )
        .map_err(|_| invalid("D1 read byte count"))
    }
}

fn write_json_line(stdout: &mut dyn Write, value: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *stdout, value).map_err(|error| error.to_string())?;
    stdout.write_all(b"\n").map_err(|error| error.to_string())
}

/// Compose a JSON object from already-converted values. Large native receipts
/// use this shallow builder instead of one deeply nested `json!` invocation,
/// keeping serde_json's macro expansion below rustc's recursion limit while
/// preserving its ordinary `Value` conversion for each field.
fn capture_json_object<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

/// Bounds row/text material retained by a single offline capture before it is
/// cloned into the SQL-pair transition vector.
struct RetainedBytes {
    used: usize,
    limit: usize,
}

impl RetainedBytes {
    fn new(limits: Limits) -> Self {
        Self {
            used: 0,
            limit: limits.prepared.max_change_bytes,
        }
    }

    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        self.used = self
            .used
            .checked_add(bytes)
            .filter(|used| *used <= self.limit)
            .ok_or_else(|| invalid("private capture retained byte budget"))?;
        Ok(())
    }

    fn text(&mut self, value: &str) -> Result<(), String> {
        self.charge(value.len())
    }

    fn text_copy(&mut self, value: &str) -> Result<String, String> {
        self.text(value)?;
        Ok(value.to_owned())
    }

    fn key(&mut self, first: &str, second: &str) -> Result<(), String> {
        self.charge(
            first
                .len()
                .checked_add(second.len())
                .ok_or_else(|| invalid("private capture retained byte overflow"))?,
        )
    }

    fn row(&mut self, key: &str, row: &[D1Cell]) -> Result<(), String> {
        let mut bytes = key.len();
        for cell in row {
            let size = match cell {
                D1Cell::Null => 4,
                D1Cell::Integer(value) => {
                    let mut magnitude = value.unsigned_abs();
                    let mut digits = 1usize;
                    while magnitude >= 10 {
                        magnitude /= 10;
                        digits += 1;
                    }
                    digits + if *value < 0 { 1 } else { 0 }
                }
                D1Cell::Text(value) => value.len(),
            };
            bytes = bytes
                .checked_add(size)
                .ok_or_else(|| invalid("private capture retained byte overflow"))?;
        }
        self.charge(bytes)
    }

    fn source_row(&mut self, kind: &str, id: &str, row: Option<&str>) -> Result<(), String> {
        let bytes = kind
            .len()
            .checked_add(id.len())
            .and_then(|bytes| bytes.checked_add(row.map_or(0, str::len)))
            .ok_or_else(|| invalid("private capture retained byte overflow"))?;
        self.charge(bytes)
    }
}

fn invalid(message: &'static str) -> String {
    message.to_owned()
}

fn exact(value: &Value, keys: &[&str], label: &'static str) -> Result<(), String> {
    let object = value.as_object().ok_or_else(|| invalid(label))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(label));
    }
    Ok(())
}

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("request string field"))
}

fn foundation_string<'a>(value: &'a JsonValue, key: &str) -> Result<&'a str, String> {
    value
        .object_get(key)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid("request string field"))
}

fn positive(value: &Value, key: &str) -> Result<u64, String> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .filter(|n| *n > 0)
        .ok_or_else(|| invalid("request finite positive limit"))
}

fn nonnegative(value: &Value, key: &str) -> Result<u64, String> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("request finite nonnegative limit"))
}

fn limits(value: &Value, operation: &str) -> Result<Limits, String> {
    let has_projection = matches!(
        operation,
        "source-navigation-bootstrap" | "source-navigation-integrity"
    );
    let fields: &[&str] = if has_projection {
        &["prepared", "projection"]
    } else {
        &["prepared"]
    };
    exact(value, fields, "capture limits shape")?;
    let prepared = value
        .get("prepared")
        .ok_or_else(|| invalid("prepared limits"))?;
    let projection = value.get("projection");
    exact(
        prepared,
        &[
            "max_changes",
            "max_row_bytes",
            "max_metadata_bytes",
            "max_read_bytes",
            "max_rows",
            "max_retained_bytes",
            "max_sql_bytes",
            "max_postings",
            "max_manifest_rows",
        ],
        "prepared limit fields",
    )?;
    let max_changes = usize::try_from(positive(prepared, "max_changes")?)
        .map_err(|_| invalid("prepared max_changes"))?;
    let max_row_bytes = usize::try_from(positive(prepared, "max_row_bytes")?)
        .map_err(|_| invalid("prepared row bytes"))?;
    let max_metadata_bytes = usize::try_from(positive(prepared, "max_metadata_bytes")?)
        .map_err(|_| invalid("prepared metadata bytes"))?;
    let max_read_bytes = positive(prepared, "max_read_bytes")?;
    let max_rows =
        usize::try_from(positive(prepared, "max_rows")?).map_err(|_| invalid("prepared rows"))?;
    let max_retained_bytes = usize::try_from(positive(prepared, "max_retained_bytes")?)
        .map_err(|_| invalid("prepared retained bytes"))?;
    let max_sql_bytes = positive(prepared, "max_sql_bytes")?;
    let max_postings = usize::try_from(positive(prepared, "max_postings")?)
        .map_err(|_| invalid("prepared posting limit"))?;
    let max_manifest_rows = usize::try_from(positive(prepared, "max_manifest_rows")?)
        .map_err(|_| invalid("prepared manifest limit"))?;
    let projection_limits = if let Some(projection) = projection {
        exact(
            projection,
            &[
                "max_changes",
                "max_input_bytes",
                "max_opened_parts",
                "max_stored_read_bytes",
                "max_decoded_bytes",
                "max_keys",
                "max_written_parts",
                "max_written_decoded_bytes",
                "max_written_stored_bytes",
                "max_result_bytes",
            ],
            "projection limit fields",
        )?;
        // MutationLimits describes read and COW-output dimensions. This
        // producer reads immutable roots and emits SQL, so its read ceilings
        // are preserved independently; COW write/result ceilings do not
        // constrain an operation that creates no projection parts or delta.
        let max_opened_parts = nonnegative(projection, "max_opened_parts")?;
        let max_stored_read_bytes = nonnegative(projection, "max_stored_read_bytes")?;
        let max_decoded_bytes = nonnegative(projection, "max_decoded_bytes")?;
        let max_read_bytes = max_stored_read_bytes
            .checked_add(max_decoded_bytes)
            .ok_or_else(|| invalid("projection read byte limit overflow"))?;
        let max_keys = nonnegative(projection, "max_keys")?;
        for key in [
            "max_changes",
            "max_input_bytes",
            "max_written_parts",
            "max_written_decoded_bytes",
            "max_written_stored_bytes",
            "max_result_bytes",
        ] {
            let _ = nonnegative(projection, key)?;
        }
        D1ProjectionLimits {
            max_opened_parts,
            max_read_bytes,
            max_stored_read_bytes,
            max_decoded_bytes,
            max_keys,
            max_rows: max_rows as u64,
            max_changes: max_changes as u64,
            max_output_bytes: max_retained_bytes as u64,
        }
    } else {
        // The maintained source-navigation delta derives its snapshot-diff
        // limits from PreparedD1DeltaLimits; there is no separate caller
        // MutationLimits argument on that API.
        let max_opened_parts = max_changes.saturating_mul(4).min(256) as u64;
        let max_stored_read_bytes = max_opened_parts.saturating_mul(8 * 1024 * 1024);
        let max_decoded_bytes = max_read_bytes;
        D1ProjectionLimits {
            max_opened_parts,
            max_read_bytes: max_stored_read_bytes.saturating_add(max_decoded_bytes),
            max_stored_read_bytes,
            max_decoded_bytes,
            max_keys: (max_changes as u64).saturating_mul(16),
            max_rows: max_rows as u64,
            max_changes: max_changes as u64,
            max_output_bytes: max_retained_bytes as u64,
        }
    };
    // Derive internal emitter bounds from the maintained capture counters;
    // these are not another caller-selected budget. The source API separately
    // bounds changed rows, retained projections, search postings and manifest
    // scanning. Two serialized row sides plus SQL quoting and fixed row
    // framing fit under this conservative mechanical work allowance.
    let pair_transition_bound = (max_rows as u64)
        .checked_add(max_postings as u64)
        .and_then(|value| value.checked_add(max_manifest_rows as u64))
        .and_then(|value| value.checked_add(max_changes as u64))
        .ok_or_else(|| invalid("private pair transition limit overflow"))?;
    let max_work_bytes = u64::try_from(max_retained_bytes)
        .ok()
        .and_then(|bytes| bytes.checked_mul(2))
        .and_then(|bytes| bytes.checked_add(max_sql_bytes.checked_mul(2)?))
        .and_then(|bytes| bytes.checked_add(pair_transition_bound.checked_mul(2048)?))
        .ok_or_else(|| invalid("private pair work limit overflow"))?;
    let pair_limits = D1PairLimits {
        max_transitions: pair_transition_bound,
        max_work_bytes,
        max_sql_bytes,
        // Prepared row reads allow 4 MiB by default; the maintained SQL
        // producer independently caps emitted literal rows at 2,000,000 bytes.
        max_row_bytes: MAX_D1_SQL_ROW_VALUE_BYTES,
    };
    if max_changes > 1_000_000
        || max_row_bytes > 8 * 1024 * 1024
        || max_metadata_bytes > 32 * 1024 * 1024
        || max_read_bytes > (1u64 << 40)
        || u64::try_from(max_retained_bytes).map_or(true, |bytes| bytes > (1u64 << 40))
        || max_rows > 2_000_000
        || max_postings > 20_000_000
        || max_manifest_rows > 2_000_000
        || max_sql_bytes > (1u64 << 40)
        || projection_limits.max_opened_parts > 1_000_000
        || projection_limits.max_read_bytes > (1u64 << 40)
        || projection_limits.max_keys > 2_000_000
        || projection_limits.max_rows > 2_000_000
        || projection_limits.max_changes > 1_000_000
        || projection_limits.max_output_bytes > (1u64 << 40)
    {
        return Err(invalid("capture limits exceed native portable caps"));
    }
    Ok(Limits {
        prepared: PublicationLimits {
            max_bytes: max_read_bytes,
            max_mutations: positive(prepared, "max_rows")?,
            max_row_bytes,
            max_metadata_bytes,
            max_changes,
            max_change_bytes: max_retained_bytes,
        },
        projection: projection_limits,
        pair: pair_limits,
        max_postings,
        max_manifest_rows,
    })
}

fn input_path(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if !path.is_absolute() || path.is_symlink() {
        return Err(invalid(
            "capture input must be an absolute non-symlink path",
        ));
    }
    let resolved = fs::canonicalize(path).map_err(|error| error.to_string())?;
    if !resolved.is_file() {
        return Err(invalid("capture input must be a regular file"));
    }
    Ok(resolved)
}

fn output_path(value: &str) -> Result<PathBuf, String> {
    let supplied = Path::new(value);
    if !supplied.is_absolute() || supplied.file_name().is_none() {
        return Err(invalid("capture output must be a fresh absolute path"));
    }
    let supplied_parent = supplied
        .parent()
        .ok_or_else(|| invalid("capture output parent"))?;
    let name = supplied
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| invalid("capture output name"))?;

    // The maintained API creates target parents after selecting both SQL
    // destinations. Canonicalize the nearest existing ancestor, then append
    // only normalized, not-yet-existing directory components. This accepts
    // fresh nested parents without accepting a final symlink or `..` escape.
    let mut normalized_parent = PathBuf::new();
    for component in supplied_parent.components() {
        match component {
            std::path::Component::RootDir => normalized_parent.push("/"),
            std::path::Component::Normal(part) => normalized_parent.push(part),
            std::path::Component::ParentDir => {
                normalized_parent.pop();
            }
            std::path::Component::CurDir => {}
            std::path::Component::Prefix(_) => {
                return Err(invalid("capture output path prefix"));
            }
        }
    }
    let mut missing = Vec::new();
    let mut ancestor = normalized_parent.as_path();
    let canonical_parent = loop {
        match fs::canonicalize(ancestor) {
            Ok(canonical) => {
                if !canonical.is_dir() {
                    return Err(invalid("capture output parent is not a directory"));
                }
                break canonical;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let component = ancestor
                    .file_name()
                    .filter(|component| !component.is_empty())
                    .ok_or_else(|| invalid("capture output parent is unavailable"))?;
                missing.push(component.to_owned());
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| invalid("capture output parent is unavailable"))?;
            }
            Err(error) => return Err(error.to_string()),
        }
    };
    let mut parent = canonical_parent;
    for component in missing.iter().rev() {
        parent.push(component);
    }
    let path = parent.join(name);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path),
        Ok(_) => Err(invalid("capture output must be fresh and not a symlink")),
        Err(error) => Err(error.to_string()),
    }
}

fn validate_capture_outputs(
    forward: &Path,
    rollback: Option<&Path>,
    manifest: &Path,
) -> Result<(), String> {
    let mut paths = vec![forward.to_owned(), manifest.to_owned()];
    if let Some(rollback) = rollback {
        paths.push(rollback.to_owned());
    }
    if paths.iter().collect::<BTreeSet<_>>().len() != paths.len() {
        return Err(invalid("capture output paths must be distinct"));
    }
    Ok(())
}

struct HeldSqlite {
    connection: Connection,
    identity: HeldSqliteIdentity,
}

struct HeldSqliteIdentity {
    guard: File,
    selected_path: PathBuf,
    selected_identity: (u64, u64),
    sqlite_fd: Option<i32>,
    frame_identity: Option<(u64, String)>,
    snapshot_inventory: Option<Value>,
}

impl HeldSqliteIdentity {
    fn verify_selected_file_identity(&self) -> Result<(), String> {
        let guard = self.guard.metadata().map_err(|error| error.to_string())?;
        let current =
            fs::symlink_metadata(&self.selected_path).map_err(|error| error.to_string())?;
        let identity = self.selected_identity;
        if self.selected_path.is_symlink()
            || !current.is_file()
            || (guard.dev(), guard.ino()) != identity
            || (current.dev(), current.ino()) != identity
        {
            return Err(invalid(
                "selected SQLite path differs from its held snapshot identity",
            ));
        }
        if let Some(sqlite_fd) = self.sqlite_fd {
            let held = fs::metadata(format!("/proc/self/fd/{sqlite_fd}"))
                .map_err(|error| error.to_string())?;
            if (held.dev(), held.ino()) != identity {
                return Err(invalid("selected SQLite descriptor identity changed"));
            }
        }
        if let Some((frame_bytes, expected_sha256)) = &self.frame_identity {
            if guard.len() != *frame_bytes {
                return Err(invalid("typed snapshot frame length changed"));
            }
            let mut reread = self.guard.try_clone().map_err(|error| error.to_string())?;
            use std::io::Seek;
            reread
                .seek(std::io::SeekFrom::Start(0))
                .map_err(|error| error.to_string())?;
            let mut hasher = Digest256Hasher::new();
            let mut buffer = [0u8; 64 * 1024];
            let mut seen = 0u64;
            loop {
                let count = reread
                    .read(&mut buffer)
                    .map_err(|error| error.to_string())?;
                if count == 0 {
                    break;
                }
                seen = seen
                    .checked_add(count as u64)
                    .ok_or_else(|| invalid("typed snapshot frame length overflow"))?;
                if seen > *frame_bytes {
                    return Err(invalid("typed snapshot frame grew while held"));
                }
                hasher.update(&buffer[..count]);
            }
            if seen != *frame_bytes || hasher.finalize().to_hex() != *expected_sha256 {
                return Err(invalid("typed snapshot frame bytes changed while held"));
            }
        }
        Ok(())
    }
}

fn process_fds() -> Result<BTreeSet<i32>, String> {
    fs::read_dir("/proc/self/fd")
        .map_err(|error| format!("process descriptor inventory: {error}"))?
        .map(|entry| {
            let name = entry
                .map_err(|error| error.to_string())?
                .file_name()
                .into_string()
                .map_err(|_| invalid("process descriptor name"))?;
            name.parse::<i32>()
                .map_err(|_| invalid("process descriptor number"))
        })
        .collect()
}

fn open_read_only(
    path: &Path,
    vm_steps: u64,
    max_value_bytes: usize,
) -> Result<HeldSqlite, String> {
    let descriptors_before = process_fds()?;
    let identity_guard = tos_fd_open::open_absolute_regular(path, u64::MAX)
        .map_err(|error| format!("pin selected SQLite input: {error}"))?;
    let selected_metadata = identity_guard
        .metadata()
        .map_err(|error| error.to_string())?;
    let selected_identity = (selected_metadata.dev(), selected_metadata.ino());
    let identity_fd = identity_guard.as_raw_fd();
    let mut db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(|error| error.to_string())?;
    let mut used = 0u64;
    db.progress_handler(
        1000,
        Some(move || {
            used = used.saturating_add(1000);
            used > vm_steps
        }),
    );
    db.set_limit(
        rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
        i32::try_from(max_value_bytes).map_err(|_| invalid("SQLite value byte limit"))?,
    )
    .map_err(|error| error.to_string())?;
    db.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_SQL_LENGTH, 1_000_000)
        .map_err(|error| error.to_string())?;
    require_memory_temp_store(&db)?;
    let _: i64 = db
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let mut database_list = db
        .prepare("PRAGMA database_list")
        .map_err(|error| error.to_string())?;
    let main_path: String = database_list
        .query_row([], |row| row.get(2))
        .map_err(|error| error.to_string())?;
    drop(database_list);
    if Path::new(&main_path) != path {
        return Err(invalid("SQLite connection opened another input path"));
    }
    let sqlite_fd = process_fds()?
        .into_iter()
        .filter(|fd| *fd != identity_fd && !descriptors_before.contains(fd))
        .find(|fd| {
            fs::metadata(format!("/proc/self/fd/{fd}"))
                .ok()
                .is_some_and(|metadata| (metadata.dev(), metadata.ino()) == selected_identity)
        })
        .ok_or_else(|| invalid("SQLite connection does not hold the selected input"))?;
    let path_metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if path.is_symlink()
        || !path_metadata.is_file()
        || (path_metadata.dev(), path_metadata.ino()) != selected_identity
    {
        return Err(invalid(
            "SQLite held file identity differs from selected input",
        ));
    }
    Ok(HeldSqlite {
        connection: db,
        identity: HeldSqliteIdentity {
            guard: identity_guard,
            selected_path: path.to_owned(),
            selected_identity,
            sqlite_fd: Some(sqlite_fd),
            frame_identity: None,
            snapshot_inventory: None,
        },
    })
}

struct SnapshotFrameBudget {
    limit: u64,
    used: u64,
    schema_allocation_limit: u64,
}

impl SnapshotFrameBudget {
    fn remaining(&self) -> Result<u64, String> {
        self.limit
            .checked_sub(self.used)
            .ok_or_else(|| invalid("typed snapshot aggregate byte budget"))
    }
    fn consume(&mut self, bytes: u64) -> Result<(), String> {
        self.used = self
            .used
            .checked_add(bytes)
            .filter(|used| *used <= self.limit)
            .ok_or_else(|| invalid("typed snapshot aggregate byte budget"))?;
        Ok(())
    }
}

fn open_typed_snapshot(
    path: &Path,
    role: typed_snapshot::Role,
    input_field: &str,
    budget: &mut SnapshotFrameBudget,
    vm_steps: u64,
    max_cell_bytes: usize,
) -> Result<HeldSqlite, String> {
    let remaining = budget.remaining()?;
    let imported = typed_snapshot::import(
        path,
        role,
        input_field,
        remaining,
        budget.schema_allocation_limit,
        vm_steps,
        max_cell_bytes,
    )?;
    budget.consume(imported.frame_bytes)?;
    Ok(HeldSqlite {
        connection: imported.connection,
        identity: HeldSqliteIdentity {
            guard: imported.guard,
            selected_path: imported.selected_path,
            selected_identity: imported.selected_identity,
            sqlite_fd: None,
            frame_identity: Some((imported.frame_bytes, imported.frame_sha256)),
            snapshot_inventory: Some(imported.inventory),
        },
    })
}

fn open_selected_snapshot(
    path: &Path,
    vm_steps: u64,
    max_cell_bytes: usize,
    request_schema: &str,
    role: typed_snapshot::Role,
    input_field: &str,
    frame_budget: &mut Option<SnapshotFrameBudget>,
) -> Result<HeldSqlite, String> {
    if request_schema == TYPED_REQUEST_SCHEMA {
        let budget = frame_budget
            .as_mut()
            .ok_or_else(|| invalid("typed snapshot aggregate budget absent"))?;
        open_typed_snapshot(path, role, input_field, budget, vm_steps, max_cell_bytes)
    } else {
        open_read_only(path, vm_steps, max_cell_bytes)
    }
}

fn preflight_snapshot_frame_budget(
    request: &Value,
    operation: &str,
) -> Result<SnapshotFrameBudget, String> {
    let limit = positive(request, "snapshot_frame_max_bytes")?;
    let schema_allocation_limit = positive(request, "snapshot_schema_max_allocation_bytes")?;
    let mut fields = vec!["d1_database"];
    if operation != "source-navigation-integrity" {
        if !request["before_prepared_database"].is_null() {
            fields.push("before_prepared_database");
        }
        fields.push("after_prepared_database");
    }
    let mut total = 0u64;
    for field in fields {
        let path = input_path(string(request, field)?)?;
        let bytes = fs::metadata(path).map_err(|error| error.to_string())?.len();
        total = total
            .checked_add(bytes)
            .filter(|used| *used <= limit)
            .ok_or_else(|| invalid("typed snapshot aggregate byte budget"))?;
    }
    Ok(SnapshotFrameBudget {
        limit,
        used: 0,
        schema_allocation_limit,
    })
}

fn snapshot_transport_value(
    request_schema: &str,
    snapshots: &[(&str, &HeldSqliteIdentity)],
) -> Result<Option<Value>, String> {
    if request_schema == REQUEST_SCHEMA {
        return Ok(None);
    }
    let mut entries = Vec::with_capacity(snapshots.len());
    for (field, identity) in snapshots {
        let entry = identity
            .snapshot_inventory
            .as_ref()
            .ok_or_else(|| invalid("typed snapshot inventory absent"))?;
        if entry.get("input_field").and_then(Value::as_str) != Some(*field) {
            return Err(invalid("typed snapshot inventory role differs"));
        }
        entries.push(entry.clone());
    }
    Ok(Some(json!({
        "schema": "tos_edge_typed_snapshot_inventory_v1",
        "snapshots": entries,
    })))
}

fn attach_snapshot_transport(receipt: &mut Value, inventory: Option<Value>) -> Result<(), String> {
    let Some(inventory) = inventory else {
        return Ok(());
    };
    receipt
        .as_object_mut()
        .ok_or_else(|| invalid("native receipt is not an object"))?
        .insert("snapshot_transport".to_owned(), inventory);
    Ok(())
}

/// Keep SQLite's transient tables, indices, and sort runs in memory for these
/// reads. This is placement only: it caps neither SQLite memory nor main-file
/// WAL/SHM sidecars.
fn require_memory_temp_store(db: &Connection) -> Result<(), String> {
    let can_force_memory: i64 = db
        .query_row(
            "SELECT sqlite_compileoption_used('TEMP_STORE=1') OR sqlite_compileoption_used('TEMP_STORE=2') OR sqlite_compileoption_used('TEMP_STORE=3')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("SQLite temp-store compile mode unavailable: {error}"))?;
    if can_force_memory != 1 {
        return Err(invalid(
            "SQLite temp-store compile mode cannot prove memory placement",
        ));
    }
    db.execute_batch("PRAGMA temp_store=MEMORY;")
        .map_err(|error| error.to_string())?;
    let mode: i64 = db
        .query_row("PRAGMA temp_store", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if mode != 2 {
        return Err(invalid(
            "SQLite temp-store memory placement did not take effect",
        ));
    }
    Ok(())
}

fn foundation(value: &Value, cap: usize) -> Result<JsonValue, String> {
    let raw = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let limits =
        JsonLimits::new(cap, 128, CAPTURE_JSON_VISITS, 4300).map_err(|error| error.to_string())?;
    parse_json(&raw, JsonMode::PublishedStrict, limits)
        .map(|document| document.into_root())
        .map_err(|error| error.to_string())
}

fn foundation_raw(raw: &[u8], cap: usize) -> Result<JsonValue, String> {
    let limits =
        JsonLimits::new(cap, 128, CAPTURE_JSON_VISITS, 4300).map_err(|error| error.to_string())?;
    parse_json(raw, JsonMode::PublishedStrict, limits)
        .map(|document| document.into_root())
        .map_err(|error| error.to_string())
}

/// Read the maintained source pairing through its exact persisted binding.
/// The Python prepared-delta/bootstrap APIs accept this binding, not the
/// compiler's source-maintenance CatalogInputs; reader/catalog/lens metadata
/// are checked independently against the held D1 and prepared snapshots.
fn prepared_source_inputs_held(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    limits: Limits,
    read_bytes: &mut D1ReadBytes,
) -> Result<PreparedSourceInputs, String> {
    let cap = PREPARED_SOURCE_BYTES.min(limits.prepared.max_metadata_bytes);
    let mut statement = tx
        .prepare("SELECT CASE WHEN typeof(binding)='text' AND length(CAST(binding AS BLOB))<=?1 THEN binding END,CASE WHEN typeof(inputs)='text' AND length(CAST(inputs AS BLOB))<=?1 THEN inputs END,CASE WHEN typeof(sha256)='text' AND length(sha256)=64 THEN sha256 END,length(CAST(json_array(binding,inputs,sha256) AS BLOB)) FROM prepared_source_state WHERE singleton=1 LIMIT 2")
        .map_err(|error| error.to_string())?;
    let mut rows = statement.query([cap]).map_err(|error| error.to_string())?;
    let row = rows
        .next()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| invalid("prepared source selection absent"))?;
    let encoded_bytes: i64 = row.get(3).map_err(|error| error.to_string())?;
    let encoded_bytes = usize::try_from(encoded_bytes)
        .map_err(|_| invalid("prepared source selection byte count"))?;
    read_bytes.charge(encoded_bytes)?;
    let binding: Option<String> = row.get(0).map_err(|error| error.to_string())?;
    let inputs: Option<String> = row.get(1).map_err(|error| error.to_string())?;
    let digest: Option<String> = row.get(2).map_err(|error| error.to_string())?;
    if rows.next().map_err(|error| error.to_string())?.is_some() {
        return Err(invalid("prepared source selection is not unique"));
    }
    let binding = binding.ok_or_else(|| invalid("prepared source binding bytes"))?;
    let inputs = inputs.ok_or_else(|| invalid("prepared source inputs bytes"))?;
    let digest = digest.ok_or_else(|| invalid("prepared source digest"))?;
    validate_prepared_source_state(
        expected,
        binding.as_bytes(),
        inputs.as_bytes(),
        &digest,
        limits.prepared,
    )
    .map_err(|error| error.to_string())
}

fn compact(value: &Value, cap: usize) -> Result<String, String> {
    let typed = foundation(value, cap)?;
    let raw = emit_python_compact_json(
        &typed,
        JsonLimits::new(cap, 128, CAPTURE_JSON_VISITS, 4300).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    String::from_utf8(raw).map_err(|error| error.to_string())
}

fn compact_foundation(value: &JsonValue, cap: usize) -> Result<String, String> {
    let raw = emit_python_compact_json(
        value,
        JsonLimits::new(cap, 128, CAPTURE_JSON_VISITS, 4300).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    String::from_utf8(raw).map_err(|error| error.to_string())
}

fn compact_ordered_metadata(fields: Vec<(&str, Value)>, cap: usize) -> Result<String, String> {
    let mut ordered = Vec::with_capacity(fields.len());
    for (name, value) in fields {
        ordered.push((JsonString::from_utf8(name), foundation(&value, cap)?));
    }
    let raw = emit_python_compact_json(
        &JsonValue::Object(ordered),
        JsonLimits::new(cap, 128, CAPTURE_JSON_VISITS, 4300).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    String::from_utf8(raw).map_err(|error| error.to_string())
}

fn digest(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_hex()
}

fn implementation_digest() -> String {
    let mut hasher = Digest256Hasher::new();
    for source in [
        include_bytes!("edge_offline_capture.rs").as_slice(),
        include_bytes!("edge_offline_capture/typed_snapshot.rs").as_slice(),
        include_bytes!("../../tos-compiler/src/d1.rs").as_slice(),
        include_bytes!("../../tos-compiler/src/d1_prepared_pair.rs").as_slice(),
        include_bytes!("../../tos-compiler/src/d1_public_capture.rs").as_slice(),
        include_bytes!("../../tos-compiler/src/d1_public_metadata.rs").as_slice(),
        include_bytes!("../../tos-compiler/src/d1_public_rows.rs").as_slice(),
        include_bytes!("../../tos-compiler/src/d1_public_knowledge.rs").as_slice(),
        include_bytes!("../../tos-compiler/src/d1_public_lens.rs").as_slice(),
        include_bytes!("../../tos-compiler/src/d1_public_sql.rs").as_slice(),
        include_bytes!("../../tos-compiler/src/d1_projection_snapshot.rs").as_slice(),
        include_bytes!("../../tos-fd-open/src/lib.rs").as_slice(),
        include_bytes!("../../tos-compiler/src/knowledge_search.rs").as_slice(),
        include_bytes!("../../tos-compiler/src/prepared_catalog_semantics.rs").as_slice(),
        include_bytes!("../../tos-compiler/src/prepared_source_binding.rs").as_slice(),
        include_bytes!("../../tos-foundation/src/digest.rs").as_slice(),
        include_bytes!("../../tos-foundation/src/json.rs").as_slice(),
        include_bytes!("../../tos-foundation/src/unicode.rs").as_slice(),
        include_bytes!("../../tos-foundation/src/unicode_generated.rs").as_slice(),
    ] {
        hasher.update(&(source.len() as u64).to_be_bytes());
        hasher.update(source);
    }
    hasher.finalize().to_hex()
}

fn parse_meta_accounted(
    db: &Transaction<'_>,
    key: &str,
    limits: Limits,
    read_bytes: &mut D1ReadBytes,
) -> Result<(Value, Vec<D1RowTransition>, String), String> {
    let mut statement = db.prepare("SELECT part,CASE WHEN typeof(json_chunk)='text' AND length(CAST(json_chunk AS BLOB))<=?1 THEN json_chunk END FROM edge_meta WHERE key=?2 ORDER BY part LIMIT ?3")
        .map_err(|error| error.to_string())?;
    let mut rows = statement
        .query(params![
            limits.prepared.max_metadata_bytes,
            key,
            limits.prepared.max_changes + 1
        ])
        .map_err(|error| error.to_string())?;
    let mut raw = String::new();
    let mut transitions = Vec::new();
    let mut expected = 0i64;
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        if transitions.len() >= limits.prepared.max_changes {
            return Err(invalid("D1 metadata chunk count budget"));
        }
        let part: i64 = row.get(0).map_err(|error| error.to_string())?;
        let chunk_bytes = match row.get_ref(1).map_err(|error| error.to_string())? {
            ValueRef::Text(value) => value.len(),
            _ => return Err(invalid("D1 metadata chunk bytes")),
        };
        if raw
            .len()
            .checked_add(chunk_bytes)
            .is_none_or(|n| n > limits.prepared.max_metadata_bytes)
        {
            return Err(invalid("D1 metadata bytes budget"));
        }
        read_bytes.charge(chunk_bytes)?;
        let chunk: String = row.get(1).map_err(|error| error.to_string())?;
        if part != expected {
            return Err(invalid("D1 metadata chunk sequence"));
        }
        raw.push_str(&chunk);
        if raw.len() > limits.prepared.max_metadata_bytes {
            return Err(invalid("D1 metadata bytes budget"));
        }
        transitions.push(D1RowTransition {
            table: D1Table::EdgeMeta,
            before: Some(vec![
                D1Cell::Text(key.to_owned()),
                D1Cell::Integer(part),
                D1Cell::Text(chunk),
            ]),
            after: None,
        });
        expected += 1;
    }
    if transitions.is_empty() {
        return Err(invalid("D1 metadata key absent"));
    }
    let value: Value = serde_json::from_str(&raw).map_err(|_| invalid("D1 metadata JSON"))?;
    Ok((value, transitions, raw))
}

fn meta_exists(db: &Transaction<'_>, key: &str) -> Result<bool, String> {
    db.query_row(
        "SELECT 1 FROM edge_meta WHERE key=?1 LIMIT 1",
        [key],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(|error| error.to_string())
}

/// Read one complete private metadata key without assuming that concatenated
/// chunks form JSON. Search companions are text fragments and their exact
/// original bytes and part order are part of the selected row contract.
fn selected_meta_rows(
    db: &Transaction<'_>,
    key: &str,
    limits: Limits,
    retained: &mut RetainedBytes,
    read_bytes: &mut D1ReadBytes,
) -> Result<Vec<Vec<D1Cell>>, String> {
    let row_limit = i64::try_from(limits.prepared.max_changes)
        .map_err(|_| invalid("private metadata row limit"))?
        .checked_add(1)
        .ok_or_else(|| invalid("private metadata row limit"))?;
    let mut statement = db
        .prepare("SELECT part,CASE WHEN typeof(json_chunk)='text' AND length(CAST(json_chunk AS BLOB))<=?1 THEN json_chunk END FROM edge_meta WHERE key=?2 ORDER BY part LIMIT ?3")
        .map_err(|error| error.to_string())?;
    let mut selected = statement
        .query(params![limits.prepared.max_metadata_bytes, key, row_limit])
        .map_err(|error| error.to_string())?;
    let mut output = Vec::new();
    let mut metadata_bytes = 0usize;
    while let Some(row) = selected.next().map_err(|error| error.to_string())? {
        if output.len() >= limits.prepared.max_changes {
            return Err(invalid("private metadata chunk count budget"));
        }
        let part: i64 = row.get(0).map_err(|error| error.to_string())?;
        let chunk_bytes = match row.get_ref(1).map_err(|error| error.to_string())? {
            ValueRef::Text(value) => value.len(),
            _ => return Err(invalid("private metadata chunk bytes")),
        };
        metadata_bytes = metadata_bytes
            .checked_add(chunk_bytes)
            .filter(|bytes| *bytes <= limits.prepared.max_metadata_bytes)
            .ok_or_else(|| invalid("private metadata byte budget"))?;
        read_bytes.charge(chunk_bytes)?;
        let chunk: Option<String> = row.get(1).map_err(|error| error.to_string())?;
        let chunk = chunk.ok_or_else(|| invalid("private metadata chunk bytes"))?;
        let expected_part =
            i64::try_from(output.len()).map_err(|_| invalid("private metadata part number"))?;
        if part != expected_part {
            return Err(invalid("private metadata chunk sequence"));
        }
        let values = vec![
            D1Cell::Text(key.to_owned()),
            D1Cell::Integer(part),
            D1Cell::Text(chunk),
        ];
        let row_key = row_key(D1Table::EdgeMeta, &values)?;
        retained.row(&row_key, &values)?;
        output.push(values);
    }
    Ok(output)
}

fn table_exists(db: &Transaction<'_>, name: &str) -> Result<bool, String> {
    db.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1 LIMIT 1",
        [name],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(|error| error.to_string())
}

fn auxiliary_stores_accounted(
    db: &Transaction<'_>,
    top_raw: &str,
    limits: Limits,
    read_bytes: &mut D1ReadBytes,
) -> Result<Vec<D1AuxiliaryStore>, String> {
    let mut selected = Vec::new();
    for store in [
        D1AuxiliaryStore::CompactLens,
        D1AuxiliaryStore::LensMemberships,
    ] {
        let (table, state, _) = store.identity();
        let table_present = table_exists(db, table)?;
        let state_present = table_exists(db, state)?;
        if table_present != state_present {
            return Err(invalid("incomplete installed D1 lens store"));
        }
        if !table_present {
            continue;
        }
        let epoch: i64 = db
            .query_row(
                "SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .map_err(|_| invalid("D1 lens store publication clock absent"))?;
        if !(0..=9_007_199_254_740_991).contains(&epoch) {
            return Err(invalid("D1 lens store publication epoch"));
        }
        let mut statement = db
            .prepare(&format!(
                "SELECT schema,binding,valid,length(CAST(json_array(schema,binding,valid) AS BLOB)) FROM {state} WHERE singleton=1 LIMIT 2"
            ))
            .map_err(|error| error.to_string())?;
        let mut rows = statement.query([]).map_err(|error| error.to_string())?;
        let row = rows
            .next()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| invalid("D1 lens store state absent"))?;
        let json_bytes: i64 = row.get(3).map_err(|error| error.to_string())?;
        let json_bytes =
            usize::try_from(json_bytes).map_err(|_| invalid("D1 auxiliary JSON byte count"))?;
        if json_bytes > limits.prepared.max_row_bytes {
            return Err(invalid("D1 auxiliary JSON row byte budget"));
        }
        read_bytes.charge(json_bytes)?;
        if matches!(row.get_ref(1).map_err(|error| error.to_string())?, ValueRef::Text(value) if value.len() > limits.prepared.max_metadata_bytes)
        {
            return Err(invalid("D1 lens store binding byte budget"));
        }
        let schema: Option<String> = row.get(0).map_err(|error| error.to_string())?;
        let binding: Option<String> = row.get(1).map_err(|error| error.to_string())?;
        let valid: i64 = row.get(2).map_err(|error| error.to_string())?;
        if rows.next().map_err(|error| error.to_string())?.is_some()
            || schema.as_deref() != Some(store.schema())
            || binding
                .as_ref()
                .is_none_or(|value| value.len() > limits.prepared.max_metadata_bytes)
            || valid != 1
        {
            return Err(invalid("D1 lens store state framing or validity"));
        }
        let expected = auxiliary_binding_candidates(top_raw, epoch as u64)
            .map_err(|error| format!("D1 lens store binding: {error:?}"))?;
        if !binding
            .as_ref()
            .is_some_and(|value| expected.contains(value))
        {
            return Err(invalid("D1 lens store is bound to another reader/epoch"));
        }
        selected.push(store);
    }
    Ok(selected)
}

fn repository_root() -> Result<String, String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or_else(|| invalid("native repository-root build binding"))?;
    if !root.is_absolute() {
        return Err(invalid("native repository-root build binding"));
    }
    root.to_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("native repository-root UTF-8"))
}

fn project_navigation_exact(
    kind: &str,
    ordinal: i64,
    item: &mut JsonValue,
    repo_root: &str,
) -> Result<Vec<D1RowTransition>, String> {
    let original = item.clone();
    let rows = project_private_navigation_row(kind, ordinal, item, repo_root)
        .map_err(|error| error.to_string())?;
    if *item != original {
        return Err(invalid(
            "native navigation input requires explicit portable-path migration",
        ));
    }
    Ok(rows)
}

fn meta_transitions(
    key: &str,
    before: &[D1RowTransition],
    value: &Value,
    limits: Limits,
) -> Result<Vec<D1RowTransition>, String> {
    let raw = compact(value, limits.prepared.max_metadata_bytes)?;
    meta_raw_transitions(key, before, &raw, limits)
}

fn meta_raw_transitions(
    key: &str,
    before: &[D1RowTransition],
    raw: &str,
    limits: Limits,
) -> Result<Vec<D1RowTransition>, String> {
    if raw.len() > limits.prepared.max_metadata_bytes {
        return Err(invalid("metadata output byte budget"));
    }
    let chunk_size = 32_000usize;
    let mut after = Vec::new();
    if raw.is_empty() {
        after.push(String::new());
    } else {
        let mut start = 0;
        while start < raw.len() {
            let mut end = (start + chunk_size).min(raw.len());
            while end > start && !raw.is_char_boundary(end) {
                end -= 1;
            }
            if end == start {
                return Err(invalid("metadata UTF-8 chunk"));
            }
            after.push(raw[start..end].to_owned());
            start = end;
        }
    }
    if after.len() > limits.prepared.max_changes {
        return Err(invalid("metadata output chunk count"));
    }
    let old = before
        .iter()
        .filter_map(|transition| transition.before.clone())
        .collect::<Vec<_>>();
    let count = old.len().max(after.len());
    Ok((0..count)
        .filter_map(|index| {
            let prior = old.get(index).cloned();
            let successor = after.get(index).map(|chunk| {
                vec![
                    D1Cell::Text(key.to_owned()),
                    D1Cell::Integer(index as i64),
                    D1Cell::Text(chunk.clone()),
                ]
            });
            if prior == successor {
                None
            } else {
                Some(D1RowTransition {
                    table: D1Table::EdgeMeta,
                    before: prior,
                    after: successor,
                })
            }
        })
        .collect())
}

// Check borrowed SQLite cells before allocating owned row strings. The
// connection's portable SQLite length ceiling is separate from this request cap.
fn borrowed_row_bytes(
    row: &rusqlite::Row<'_>,
    columns: usize,
    cap: usize,
) -> Result<usize, String> {
    let mut bytes = 0usize;
    for index in 0..columns {
        let cell_bytes = match row.get_ref(index).map_err(|error| error.to_string())? {
            ValueRef::Null => 0,
            ValueRef::Integer(value) => value.to_string().len(),
            ValueRef::Text(value) => value.len(),
            _ => return Err(invalid("D1 row storage type")),
        };
        bytes = bytes
            .checked_add(cell_bytes)
            .ok_or_else(|| invalid("D1 row byte count overflow"))?;
        if bytes > cap {
            return Err(invalid("D1 row pre-copy byte budget"));
        }
    }
    Ok(bytes)
}

fn account_json_row(
    row: &rusqlite::Row<'_>,
    columns: usize,
    max_row_bytes: usize,
    read_bytes: &mut D1ReadBytes,
    budget: &'static str,
) -> Result<(), String> {
    borrowed_row_bytes(row, columns, max_row_bytes)?;
    let json_bytes: i64 = row.get(columns).map_err(|error| error.to_string())?;
    let json_bytes =
        usize::try_from(json_bytes).map_err(|_| invalid("D1 selected JSON byte count"))?;
    if json_bytes > max_row_bytes {
        return Err(invalid(budget));
    }
    read_bytes.charge(json_bytes)
}

fn row_to_sql(row: &rusqlite::Row<'_>, columns: usize) -> Result<Vec<D1Cell>, String> {
    let mut values = Vec::with_capacity(columns);
    for index in 0..columns {
        values.push(
            match row.get_ref(index).map_err(|error| error.to_string())? {
                ValueRef::Null => D1Cell::Null,
                ValueRef::Integer(value) => D1Cell::Integer(value),
                ValueRef::Text(value) => D1Cell::Text(
                    std::str::from_utf8(value)
                        .map_err(|_| invalid("D1 row UTF-8"))?
                        .to_owned(),
                ),
                _ => return Err(invalid("D1 row storage type")),
            },
        );
    }
    Ok(values)
}

fn row_key(table: D1Table, row: &[D1Cell]) -> Result<String, String> {
    let indexes: &[usize] = match table {
        D1Table::KnowledgeNodes | D1Table::KnowledgeRelations => &[0],
        D1Table::KnowledgeSearchDocuments => &[0, 1],
        D1Table::KnowledgeSearchGrams => &[0, 1, 2, 3],
        D1Table::KnowledgeSearchGramStats => &[0, 1, 2],
        D1Table::KnowledgeLensOrder | D1Table::KnowledgeCompactLens => &[0, 1],
        D1Table::KnowledgeLensMemberships => &[0, 1, 2, 3],
        D1Table::SourceNavigationNodes => &[0],
        D1Table::SourceNavigationNodePayload => &[0, 1],
        D1Table::SourceNavigationEdges => &[0],
        D1Table::SourceNavigationEdgePayload => &[0, 1],
        D1Table::SourceNavigationRights => &[0],
        D1Table::SourceNavigationRightsPayload => &[0, 1],
        D1Table::EdgeMeta => &[0, 1],
        _ => return Err(invalid("unsupported private capture row table")),
    };
    let parts = indexes
        .iter()
        .map(|index| match &row[*index] {
            D1Cell::Null => "n:".to_owned(),
            D1Cell::Integer(value) => format!("i:{value}"),
            D1Cell::Text(value) => format!("s:{}:{value}", value.len()),
        })
        .collect::<Vec<_>>();
    Ok(parts.join("\0"))
}

fn insert_rows(
    rows: &mut BTreeMap<(String, String), (D1Table, Vec<D1Cell>)>,
    projected: Vec<D1RowTransition>,
) -> Result<(), String> {
    for transition in projected {
        let row = transition
            .after
            .ok_or_else(|| invalid("projector returned no row"))?;
        let key = row_key(transition.table, &row)?;
        if rows
            .insert(
                (format!("{:?}", transition.table), key),
                (transition.table, row),
            )
            .is_some()
        {
            return Err(invalid("duplicate projected D1 row"));
        }
    }
    Ok(())
}

fn put_projected_rows(
    rows: &mut BTreeMap<(D1Table, String), Vec<D1Cell>>,
    projected: Vec<D1RowTransition>,
    retained: &mut RetainedBytes,
) -> Result<(), String> {
    for transition in projected {
        let row = transition
            .after
            .ok_or_else(|| invalid("projector returned no D1 row"))?;
        let key = row_key(transition.table, &row)?;
        insert_capture_row(rows, (transition.table, key), row, retained)?;
    }
    Ok(())
}

fn insert_capture_row(
    rows: &mut BTreeMap<(D1Table, String), Vec<D1Cell>>,
    key: (D1Table, String),
    row: Vec<D1Cell>,
    retained: &mut RetainedBytes,
) -> Result<(), String> {
    if let Some(prior) = rows.get(&key) {
        return if prior == &row {
            Ok(())
        } else {
            Err(invalid("conflicting private D1 capture rows"))
        };
    }
    retained.row(&key.1, &row)?;
    rows.insert(key, row);
    Ok(())
}

fn clone_capture_row(
    rows: &mut BTreeMap<(D1Table, String), Vec<D1Cell>>,
    key: (D1Table, String),
    row: &[D1Cell],
    retained: &mut RetainedBytes,
) -> Result<(), String> {
    if let Some(prior) = rows.get(&key) {
        return if prior.as_slice() == row {
            Ok(())
        } else {
            Err(invalid("conflicting private D1 capture rows"))
        };
    }
    retained.row(&key.1, row)?;
    rows.insert(key, row.to_vec());
    Ok(())
}

fn sqlite_value(value: &D1Cell) -> SqlValue {
    match value {
        D1Cell::Null => SqlValue::Null,
        D1Cell::Integer(number) => SqlValue::Integer(*number),
        D1Cell::Text(text) => SqlValue::Text(text.clone()),
    }
}

fn selected_tuple_accounted(
    db: &Transaction<'_>,
    table: D1Table,
    key: &[D1Cell],
    limits: Limits,
    read_bytes: &mut D1ReadBytes,
) -> Result<Option<Vec<D1Cell>>, String> {
    let (name, columns, keys) = table.shape();
    if key.len() != keys.len() {
        return Err(invalid("D1 selected key shape"));
    }
    let selector = keys
        .iter()
        .enumerate()
        .map(|(index, column)| format!("{column} IS ?{}", index + 1))
        .collect::<Vec<_>>()
        .join(" AND ");
    let json_size = format!(",length(CAST(json_array({}) AS BLOB))", columns.join(","));
    let sql = format!(
        "SELECT {}{json_size} FROM {name} WHERE {selector} LIMIT 2",
        columns.join(",")
    );
    let values = key.iter().map(sqlite_value).collect::<Vec<_>>();
    let mut statement = db.prepare(&sql).map_err(|error| error.to_string())?;
    let mut rows = statement
        .query(params_from_iter(values.iter()))
        .map_err(|error| error.to_string())?;
    let Some(row) = rows.next().map_err(|error| error.to_string())? else {
        return Ok(None);
    };
    account_json_row(
        row,
        columns.len(),
        limits.prepared.max_row_bytes,
        read_bytes,
        "D1 selected JSON row byte budget",
    )?;
    let selected = row_to_sql(row, columns.len())?;
    if rows.next().map_err(|error| error.to_string())?.is_some() {
        return Err(invalid("D1 selected primary key is not unique"));
    }
    let bytes = selected
        .iter()
        .map(|cell| match cell {
            D1Cell::Text(text) => text.len(),
            D1Cell::Integer(value) => value.to_string().len(),
            D1Cell::Null => 0,
        })
        .sum::<usize>();
    if bytes > limits.prepared.max_row_bytes {
        return Err(invalid("D1 selected row byte budget"));
    }
    Ok(Some(selected))
}

fn selected_rows_by_owner(
    db: &Transaction<'_>,
    store: D1AuxiliaryStore,
    kind: &str,
    id: &str,
    limits: Limits,
    read_bytes: &mut D1ReadBytes,
) -> Result<Vec<Vec<D1Cell>>, String> {
    let table = store.identity().2;
    let (name, columns, keys) = table.shape();
    let owner_column = if table == D1Table::KnowledgeCompactLens {
        "kind=?1 AND id=?2"
    } else {
        "kind=?1 AND id=?2"
    };
    let order = keys.join(",");
    let json_columns = columns.join(",");
    let sql = format!(
        "SELECT {json_columns},length(CAST(json_array({json_columns}) AS BLOB)) FROM {name} WHERE {owner_column} ORDER BY {order} LIMIT 513"
    );
    let mut statement = db.prepare(&sql).map_err(|error| error.to_string())?;
    let mut rows = statement
        .query(params![kind, id])
        .map_err(|error| error.to_string())?;
    let mut output = Vec::new();
    let mut bytes = 0usize;
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        if output.len() >= 512 {
            return Err(invalid("D1 auxiliary selected membership limit"));
        }
        account_json_row(
            row,
            columns.len(),
            limits.prepared.max_row_bytes,
            read_bytes,
            "D1 auxiliary selected row byte budget",
        )?;
        let values = row_to_sql(row, columns.len())?;
        bytes = bytes.saturating_add(
            values
                .iter()
                .map(|cell| match cell {
                    D1Cell::Text(text) => text.len(),
                    D1Cell::Integer(value) => value.to_string().len(),
                    D1Cell::Null => 0,
                })
                .sum::<usize>(),
        );
        if u64::try_from(bytes).map_or(true, |bytes| bytes > limits.prepared.max_bytes) {
            return Err(invalid("D1 auxiliary selected read byte budget"));
        }
        output.push(values);
    }
    Ok(output)
}

fn row_transitions(
    before: BTreeMap<(D1Table, String), Vec<D1Cell>>,
    after: BTreeMap<(D1Table, String), Vec<D1Cell>>,
    limits: Limits,
    retained: &mut RetainedBytes,
) -> Result<Vec<D1RowTransition>, String> {
    let mut keys = BTreeSet::new();
    for key in before.keys().chain(after.keys()) {
        if !keys.contains(key) {
            retained.charge(key.1.len())?;
            keys.insert(key.clone());
        }
    }
    if keys.len() as u64 > limits.pair.max_transitions {
        return Err(invalid("private D1 total row transition budget"));
    }
    let mut transitions = Vec::new();
    for key in keys {
        let old = before.get(&key);
        let new = after.get(&key);
        if old == new {
            continue;
        }
        for row in old.into_iter().chain(new) {
            // Charge before cloning each retained before/after row into the
            // returned transition vector.
            retained.row(&key.1, row)?;
        }
        transitions.push(D1RowTransition {
            table: key.0,
            before: old.cloned(),
            after: new.cloned(),
        });
    }
    if transitions.is_empty() {
        return Err(invalid("empty private D1 transition"));
    }
    Ok(transitions)
}

fn parsed_root(
    root: &tos_compiler::prepared_source_binding::SourceProjectionRoot,
) -> Result<D1ProjectionSnapshot, String> {
    D1ProjectionSnapshot::new(root.root_bytes.clone(), PathBuf::from(&root.namespace_path))
        .map_err(|error| error.to_string())
}

fn root_for<'a>(
    inputs: &'a PreparedSourceInputs,
    key: &str,
) -> Result<&'a tos_compiler::prepared_source_binding::SourceProjectionRoot, String> {
    inputs
        .roots()
        .get(key)
        .ok_or_else(|| invalid("prepared source-navigation root absent"))
}

fn prepared_descriptor(
    db: &Transaction<'_>,
    limits: Limits,
    read_bytes: &mut D1ReadBytes,
    expected_data_revision: &str,
) -> Result<Value, String> {
    let max_bytes = limits
        .prepared
        .max_metadata_bytes
        .min(read_bytes.remaining()?);
    let mut statement = db
        .prepare("SELECT CASE WHEN typeof(descriptor)='text' AND length(CAST(descriptor AS BLOB))<=?1 THEN descriptor END FROM prepared_state WHERE singleton=1 LIMIT 2")
        .map_err(|error| error.to_string())?;
    let mut rows = statement
        .query([max_bytes])
        .map_err(|error| error.to_string())?;
    let row = rows
        .next()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| invalid("prepared descriptor absent"))?;
    let descriptor_bytes = match row.get_ref(0).map_err(|error| error.to_string())? {
        ValueRef::Text(value) => value.len(),
        _ => return Err(invalid("prepared descriptor bytes")),
    };
    read_bytes.charge(descriptor_bytes)?;
    let raw: String = row.get(0).map_err(|error| error.to_string())?;
    if rows.next().map_err(|error| error.to_string())?.is_some() {
        return Err(invalid("prepared descriptor is not unique"));
    }
    if raw.len() > max_bytes {
        return Err(invalid("prepared descriptor byte budget"));
    }
    if digest(raw.as_bytes()) != expected_data_revision {
        return Err(invalid("prepared descriptor digest differs from binding"));
    }
    let value: Value =
        serde_json::from_str(&raw).map_err(|_| invalid("prepared descriptor JSON"))?;
    if compact(&value, limits.prepared.max_metadata_bytes)? != raw {
        return Err(invalid("prepared descriptor framing"));
    }
    Ok(value)
}

fn source_inputs_value(source: &PreparedSourceInputs) -> Result<Value, String> {
    serde_json::from_slice(source.raw()).map_err(|_| invalid("prepared source inputs JSON"))
}

fn source_scope_compatible(before: &Value, after: &Value) -> Result<(), String> {
    let left = before
        .get("roots")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("predecessor source roots"))?;
    let right = after
        .get("roots")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("successor source roots"))?;
    if left.keys().ne(right.keys())
        || left.iter().any(|(name, value)| {
            !matches!(
                name.as_str(),
                "source-catalog" | "bibliographic-claims" | "source-navigation"
            ) && right.get(name) != Some(value)
        })
    {
        return Err(invalid("nonparticipating prepared source root changed"));
    }
    let left_deps = before
        .get("dependencies")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("predecessor source dependencies"))?;
    let right_deps = after
        .get("dependencies")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("successor source dependencies"))?;
    if left_deps.keys().ne(right_deps.keys())
        || left_deps.iter().any(|(name, value)| {
            !matches!(
                name.as_str(),
                "claim-publication-profile" | "metadata-addition-publication-profile"
            ) && right_deps.get(name) != Some(value)
        })
    {
        return Err(invalid(
            "nonparticipating prepared source dependency changed",
        ));
    }
    Ok(())
}

fn exact_prepared_item(
    db: &Transaction<'_>,
    kind: &str,
    id: &str,
    limits: Limits,
    read_bytes: &mut D1ReadBytes,
) -> Result<Option<String>, String> {
    if !matches!(kind, "node" | "relation") || id.is_empty() || id.len() > 4096 {
        return Err(invalid("prepared row identity"));
    }
    let table = if kind == "node" {
        "knowledge_nodes"
    } else {
        "knowledge_relations"
    };
    let row_cap = read_bytes.remaining()?.min(limits.prepared.max_row_bytes);
    let mut statement = db
        .prepare(&format!("SELECT CASE WHEN typeof(json)='text' AND length(CAST(json AS BLOB))<=?1 THEN json END,length(CAST(json AS BLOB)) FROM {table} WHERE id=?2 LIMIT 2"))
        .map_err(|error| error.to_string())?;
    let mut rows = statement
        .query(params![row_cap, id])
        .map_err(|error| error.to_string())?;
    let Some(row) = rows.next().map_err(|error| error.to_string())? else {
        return Ok(None);
    };
    let raw_bytes: i64 = row.get(1).map_err(|error| error.to_string())?;
    let raw_bytes =
        usize::try_from(raw_bytes).map_err(|_| invalid("prepared row JSON byte count"))?;
    if raw_bytes > limits.prepared.max_row_bytes {
        return Err(invalid("prepared row byte budget"));
    }
    read_bytes.charge(raw_bytes)?;
    let raw: Option<String> = row.get(0).map_err(|error| error.to_string())?;
    if rows.next().map_err(|error| error.to_string())?.is_some() {
        return Err(invalid("prepared row identity is not unique"));
    }
    let raw = raw.ok_or_else(|| invalid("prepared row JSON bytes"))?;
    if raw.len() > limits.prepared.max_row_bytes {
        return Err(invalid("prepared row byte budget"));
    }
    let parsed: Value = serde_json::from_str(&raw).map_err(|_| invalid("prepared row JSON"))?;
    if parsed.get("id").and_then(Value::as_str) != Some(id)
        || compact(&parsed, limits.prepared.max_row_bytes)? != raw
    {
        return Err(invalid("prepared row identity or emitted JSON framing"));
    }
    let key = format!("knowledge_{kind}_digest:{id}");
    let (digest_row, _, _) = parse_meta_accounted(db, &key, limits, read_bytes)?;
    if digest_row != json!({"sha256":digest(raw.as_bytes())}) {
        return Err(invalid("prepared row digest manifest differs"));
    }
    Ok(Some(raw))
}

fn selected_key(table: D1Table, row: &[D1Cell]) -> Result<Vec<D1Cell>, String> {
    let (_, columns, keys) = table.shape();
    if row.len() != columns.len() {
        return Err(invalid("projected row shape"));
    }
    keys.iter()
        .map(|key| {
            let index = columns
                .iter()
                .position(|column| column == key)
                .ok_or_else(|| invalid("projected row key shape"))?;
            Ok(row[index].clone())
        })
        .collect()
}

fn projected_map(
    transitions: Vec<D1RowTransition>,
) -> Result<BTreeMap<(D1Table, String), Vec<D1Cell>>, String> {
    let mut output = BTreeMap::new();
    for transition in transitions {
        let row = transition
            .after
            .ok_or_else(|| invalid("projection produced no row"))?;
        let key = row_key(transition.table, &row)?;
        if output.insert((transition.table, key), row).is_some() {
            return Err(invalid("duplicate projected D1 row"));
        }
    }
    Ok(output)
}

fn read_search_indexes(db: &Transaction<'_>) -> Result<(), String> {
    for (name, expected, unique) in [
        ("knowledge_search_address_id_idx", vec!["kind", "id"], true),
        (
            "knowledge_search_address_tie_idx",
            vec!["kind", "id_lower", "position"],
            false,
        ),
    ] {
        let mut index_list = db
            .prepare("PRAGMA index_list(knowledge_search_documents)")
            .map_err(|error| error.to_string())?;
        let mut listed = index_list.query([]).map_err(|error| error.to_string())?;
        let mut actual_unique = None;
        while let Some(row) = listed.next().map_err(|error| error.to_string())? {
            let actual_name: String = row.get(1).map_err(|error| error.to_string())?;
            if actual_name == name {
                let is_unique: i64 = row.get(2).map_err(|error| error.to_string())?;
                actual_unique = Some(is_unique == 1);
            }
        }
        if actual_unique != Some(unique) {
            return Err(invalid("prepared D1 search address indexes absent"));
        }
        let mut statement = db
            .prepare(&format!("PRAGMA index_xinfo({name})"))
            .map_err(|error| error.to_string())?;
        let mut rows = statement.query([]).map_err(|error| error.to_string())?;
        let mut columns = Vec::new();
        while let Some(row) = rows.next().map_err(|error| error.to_string())? {
            let key: i64 = row.get(5).map_err(|error| error.to_string())?;
            if key == 1 {
                let column: String = row.get(2).map_err(|error| error.to_string())?;
                let descending: i64 = row.get(3).map_err(|error| error.to_string())?;
                let collation: String = row.get(4).map_err(|error| error.to_string())?;
                if descending != 0 || collation != "BINARY" {
                    return Err(invalid("prepared D1 search address index profile"));
                }
                columns.push(column);
            }
        }
        if columns != expected {
            return Err(invalid("prepared D1 search address index profile"));
        }
    }
    Ok(())
}

fn search_position(db: &Transaction<'_>, plural: &str, id: &str) -> Result<Option<i64>, String> {
    db.query_row(
        "SELECT position FROM knowledge_search_documents WHERE kind=?1 AND id=?2 LIMIT 2",
        params![plural, id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

fn private_manifest(
    db: &Transaction<'_>,
    kind: &str,
    limits: Limits,
    retained: &mut RetainedBytes,
    read_bytes: &mut D1ReadBytes,
) -> Result<BTreeMap<String, String>, String> {
    let prefix = format!("knowledge_{kind}_digest:");
    let upper = format!("knowledge_{kind}_digest;");
    let manifest_chunk_bytes = limits.prepared.max_metadata_bytes.min(128);
    let key_expr =
        format!("CASE WHEN typeof(key)='text' AND length(CAST(key AS BLOB))<=?1 THEN key END");
    let raw_expr = format!(
        "CASE WHEN typeof(json_chunk)='text' AND length(CAST(json_chunk AS BLOB))<=?2 THEN json_chunk END"
    );
    let json_size = format!(",length(CAST(json_array({key_expr},part,{raw_expr}) AS BLOB))");
    let mut statement = db
        .prepare(&format!("SELECT {key_expr},part,{raw_expr}{json_size} FROM edge_meta WHERE key>=?3 AND key<?4 ORDER BY key,part LIMIT ?5"))
        .map_err(|error| error.to_string())?;
    let mut rows = statement
        .query(params![
            prefix.len() as i64 + 4096,
            manifest_chunk_bytes as i64,
            prefix,
            upper,
            limits.max_manifest_rows + 1
        ])
        .map_err(|error| error.to_string())?;
    let mut output = BTreeMap::new();
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        if output.len() >= limits.max_manifest_rows {
            return Err(invalid("prepared digest manifest row budget"));
        }
        account_json_row(
            row,
            3,
            limits.prepared.max_row_bytes,
            read_bytes,
            "prepared digest manifest row byte budget",
        )?;
        let key: Option<String> = row.get(0).map_err(|error| error.to_string())?;
        let key = key.ok_or_else(|| invalid("prepared digest manifest key size"))?;
        let part: i64 = row.get(1).map_err(|error| error.to_string())?;
        let raw: Option<String> = row.get(2).map_err(|error| error.to_string())?;
        let id = key
            .strip_prefix(&prefix)
            .filter(|id| !id.is_empty() && id.len() <= 4096)
            .ok_or_else(|| invalid("prepared digest manifest key"))?;
        let raw = raw.ok_or_else(|| invalid("prepared digest manifest bytes"))?;
        let value: Value =
            serde_json::from_str(&raw).map_err(|_| invalid("prepared digest manifest JSON"))?;
        let sha = value
            .get("sha256")
            .and_then(Value::as_str)
            .filter(|sha| {
                sha.len() == 64
                    && sha
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
            .ok_or_else(|| invalid("prepared digest manifest SHA"))?;
        if part != 0
            || value.as_object().is_none_or(|object| object.len() != 1)
            || compact(&value, manifest_chunk_bytes)? != raw
        {
            return Err(invalid("prepared digest manifest framing"));
        }
        let id = retained.text_copy(id)?;
        let sha = retained.text_copy(sha)?;
        if output.insert(id, sha).is_some() {
            return Err(invalid("prepared digest manifest framing"));
        }
    }
    let table = if kind == "node" {
        "knowledge_nodes"
    } else {
        "knowledge_relations"
    };
    let count: i64 = db
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .map_err(|error| error.to_string())?;
    if count < 0 || count as usize != output.len() {
        return Err(invalid("prepared digest manifest does not cover rows"));
    }
    Ok(output)
}

fn all_row_ids(
    db: &Transaction<'_>,
    kind: &str,
    limits: Limits,
    retained: &mut RetainedBytes,
    read_bytes: &mut D1ReadBytes,
) -> Result<BTreeSet<String>, String> {
    let table = if kind == "node" {
        "knowledge_nodes"
    } else {
        "knowledge_relations"
    };
    let mut statement = db
        .prepare(&format!("SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=4096 THEN id END FROM {table} ORDER BY id LIMIT ?1"))
        .map_err(|error| error.to_string())?;
    let mut rows = statement
        .query([limits.max_manifest_rows as i64 + 1])
        .map_err(|error| error.to_string())?;
    let mut ids = BTreeSet::new();
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        if ids.len() >= limits.max_manifest_rows {
            return Err(invalid("prepared row inventory limit"));
        }
        let bytes = match row.get_ref(0).map_err(|error| error.to_string())? {
            ValueRef::Text(value) => value.len(),
            _ => return Err(invalid("prepared row inventory identity size")),
        };
        read_bytes.charge(bytes)?;
        let id: Option<String> = row.get(0).map_err(|error| error.to_string())?;
        let id = id.ok_or_else(|| invalid("prepared row inventory identity size"))?;
        if id.is_empty() {
            return Err(invalid("prepared row inventory identity"));
        }
        let id = retained.text_copy(&id)?;
        if !ids.insert(id) {
            return Err(invalid("prepared row inventory identity"));
        }
    }
    Ok(ids)
}

fn metadata_absent(db: &Transaction<'_>, key: &str) -> Result<(), String> {
    if meta_exists(db, key)? {
        return Err(invalid("unexpected predecessor metadata row"));
    }
    Ok(())
}

fn expected_aux_rows(
    kind: &str,
    id: &str,
    raw: &str,
    limits: Limits,
    stores: &[D1AuxiliaryStore],
) -> Result<BTreeMap<(D1Table, String), Vec<D1Cell>>, String> {
    let mut result = BTreeMap::new();
    if stores.is_empty() {
        return Ok(result);
    }
    let value = foundation_raw(raw.as_bytes(), limits.prepared.max_row_bytes)?;
    let rows = project_private_lens_auxiliary_rows(
        kind,
        id,
        raw,
        &value,
        stores.contains(&D1AuxiliaryStore::CompactLens),
        stores.contains(&D1AuxiliaryStore::LensMemberships),
    )
    .map_err(|error| error.to_string())?;
    let mut retained = RetainedBytes::new(limits);
    put_projected_rows(&mut result, rows, &mut retained)?;
    Ok(result)
}

fn verify_and_add_knowledge_side(
    db: &Transaction<'_>,
    projected: &BTreeMap<(D1Table, String), Vec<D1Cell>>,
    kind: &str,
    id: &str,
    present: bool,
    limits: Limits,
    auxiliary: &[D1AuxiliaryStore],
    destination: &mut BTreeMap<(D1Table, String), Vec<D1Cell>>,
    retained: &mut RetainedBytes,
    read_bytes: &mut D1ReadBytes,
) -> Result<(), String> {
    let plural = if kind == "node" { "nodes" } else { "relations" };
    let base_table = if kind == "node" {
        D1Table::KnowledgeNodes
    } else {
        D1Table::KnowledgeRelations
    };
    if !present {
        let actual_base = selected_tuple_accounted(
            db,
            base_table,
            &[D1Cell::Text(id.to_owned())],
            limits,
            read_bytes,
        )?;
        if actual_base.is_some() {
            return Err(invalid("D1 insertion base identity already exists"));
        }
        metadata_absent(db, &format!("knowledge_{kind}_digest:{id}"))?;
        let lens_key = vec![D1Cell::Text(kind.to_owned()), D1Cell::Text(id.to_owned())];
        if selected_tuple_accounted(
            db,
            D1Table::KnowledgeLensOrder,
            &lens_key,
            limits,
            read_bytes,
        )?
        .is_some()
        {
            return Err(invalid("D1 insertion lens identity already exists"));
        }
    }
    let metadata_keys = [
        format!("knowledge_{kind}_digest:{id}"),
        format!("knowledge_{kind}_payload:{id}"),
        format!("knowledge_{kind}_search:{id}"),
    ];
    for key in &metadata_keys {
        let mut expected = projected
            .iter()
            .filter(|((table, _), row)| {
                *table == D1Table::EdgeMeta
                    && matches!(row.first(), Some(D1Cell::Text(row_key)) if row_key == key)
            })
            .map(|(_, row)| row)
            .collect::<Vec<_>>();
        expected.sort_by_key(|row| match row.get(1) {
            Some(D1Cell::Integer(part)) => *part,
            _ => i64::MIN,
        });
        let actual = selected_meta_rows(db, key, limits, retained, read_bytes)?;
        if actual.len() != expected.len()
            || actual
                .iter()
                .zip(&expected)
                .any(|(actual, expected)| actual != *expected)
        {
            return Err(invalid("D1 predecessor metadata chunk closure differs"));
        }
        for row in actual {
            let key = (D1Table::EdgeMeta, row_key(D1Table::EdgeMeta, &row)?);
            if let Some(prior) = destination.get(&key) {
                if prior != &row {
                    return Err(invalid("conflicting D1 transition metadata row"));
                }
            } else {
                // selected_meta_rows charged this row before retaining it.
                destination.insert(key, row);
            }
        }
    }
    for ((table, _), row) in projected {
        if *table == D1Table::EdgeMeta {
            continue;
        }
        if matches!(
            table,
            D1Table::KnowledgeSearchGrams
                | D1Table::KnowledgeCompactLens
                | D1Table::KnowledgeLensMemberships
        ) {
            continue;
        }
        let key = selected_key(*table, row)?;
        let actual = selected_tuple_accounted(db, *table, &key, limits, read_bytes)?;
        if present && actual.as_deref() != Some(row.as_slice()) {
            return Err(invalid("D1 predecessor projected row differs"));
        }
        if !present && actual.is_some() {
            return Err(invalid("D1 insertion identity already exists"));
        }
    }

    let search_columns = "kind,position,id,source_graph,kind_id,predicate_id,id_lower,native_id_lower,identity_values,visible_values,document_chars,document_digest";
    let mut search_statement = db
        .prepare(&format!("SELECT {search_columns},length(CAST(json_array({search_columns}) AS BLOB)) FROM knowledge_search_documents WHERE kind=?1 AND id=?2 ORDER BY position LIMIT 2"))
        .map_err(|error| error.to_string())?;
    let mut search_rows = search_statement
        .query(params![plural, id])
        .map_err(|error| error.to_string())?;
    let actual_search = if let Some(row) = search_rows.next().map_err(|error| error.to_string())? {
        account_json_row(
            row,
            12,
            limits.prepared.max_row_bytes,
            read_bytes,
            "D1 search document row byte budget",
        )?;
        Some(row_to_sql(row, 12)?)
    } else {
        None
    };
    if search_rows
        .next()
        .map_err(|error| error.to_string())?
        .is_some()
        || present != actual_search.is_some()
        || actual_search
            .as_ref()
            .is_some_and(|row| !projected.values().any(|expected| expected == row))
    {
        return Err(invalid("D1 predecessor search document closure differs"));
    }
    if let Some(document) = actual_search {
        let position = match document[1] {
            D1Cell::Integer(position) => position,
            _ => return Err(invalid("D1 search document position")),
        };
        let mut gram_statement = db
            .prepare("SELECT kind,n,gram,position,length(CAST(json_array(kind,n,gram,position) AS BLOB)) FROM knowledge_search_grams WHERE kind=?1 AND position=?2 ORDER BY gram LIMIT ?3")
            .map_err(|error| error.to_string())?;
        let mut gram_rows = gram_statement
            .query(params![plural, position, limits.max_postings as i64 + 1])
            .map_err(|error| error.to_string())?;
        let mut actual_grams = BTreeSet::new();
        while let Some(row) = gram_rows.next().map_err(|error| error.to_string())? {
            if actual_grams.len() >= limits.max_postings {
                return Err(invalid("D1 predecessor posting row budget"));
            }
            account_json_row(
                row,
                4,
                limits.prepared.max_row_bytes,
                read_bytes,
                "D1 search gram row byte budget",
            )?;
            actual_grams.insert(row_to_sql(row, 4)?);
        }
        let expected_grams = projected
            .values()
            .filter(|row| {
                // A posting row begins with the plural kind, the trigram size,
                // and its gram. Position is the final column.
                row.len() == 4
                    && row[0] == D1Cell::Text(plural.to_owned())
                    && row[1] == D1Cell::Integer(3)
                    && row[3] == D1Cell::Integer(position)
            })
            .cloned()
            .collect::<BTreeSet<_>>();
        if actual_grams != expected_grams {
            return Err(invalid("D1 predecessor search posting closure differs"));
        }
    }

    for store in auxiliary {
        let table = store.identity().2;
        let actual = selected_rows_by_owner(db, *store, kind, id, limits, read_bytes)?;
        let expected = projected
            .iter()
            .filter(|((selected_table, _), _)| *selected_table == table)
            .map(|(_, row)| row.clone())
            .collect::<Vec<_>>();
        if actual.into_iter().collect::<BTreeSet<_>>()
            != expected.into_iter().collect::<BTreeSet<_>>()
        {
            return Err(invalid("D1 predecessor auxiliary row closure differs"));
        }
    }

    for ((table, key), row) in projected {
        if *table == D1Table::EdgeMeta {
            continue;
        }
        let map_key = (*table, key.clone());
        if let Some(prior) = destination.get(&map_key) {
            if prior != row {
                return Err(invalid("conflicting D1 transition source row"));
            }
            continue;
        }
        retained.row(key, row)?;
        destination.insert(map_key, row.clone());
    }
    Ok(())
}

fn search_limits(limits: Limits) -> Result<SearchBuildLimits, String> {
    Ok(SearchBuildLimits {
        max_payload_bytes: limits.prepared.max_row_bytes.min(8_000_000),
        max_document_chars: 8_000_000,
        max_document_bytes: 64_000_000,
        max_rank_field_bytes: 8_000_000,
        max_postings: u64::try_from(limits.max_postings)
            .map_err(|_| invalid("prepared search posting limit"))?,
        max_work_bytes: limits.pair.max_work_bytes,
        gram_batch_rows: 1024,
    })
}

fn lower_id(value: &str) -> Result<String, String> {
    tos_foundation::python_lower_unicode16_v1(value, 4096, 12_288, 12_288)
        .map_err(|error| error.to_string())
}

fn navigation_payload_rows_accounted(
    db: &Transaction<'_>,
    table: D1Table,
    id: &str,
    limits: Limits,
    read_bytes: &mut D1ReadBytes,
) -> Result<Vec<Vec<D1Cell>>, String> {
    let (name, columns, keys) = table.shape();
    if keys.len() != 2 || keys[0] != "id" || keys[1] != "part" {
        return Err(invalid("navigation payload table profile"));
    }
    let metadata_bytes = i64::try_from(limits.prepared.max_metadata_bytes)
        .map_err(|_| invalid("navigation payload byte limit"))?;
    let mut statement = db
        .prepare(&format!(
            "SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=4096 THEN id END,part,CASE WHEN typeof(json_chunk)='text' AND length(CAST(json_chunk AS BLOB))<=?1 THEN json_chunk END,length(CAST(json_array(CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=4096 THEN id END,part,CASE WHEN typeof(json_chunk)='text' AND length(CAST(json_chunk AS BLOB))<=?1 THEN json_chunk END) AS BLOB)) FROM {name} WHERE id=?2 ORDER BY part LIMIT ?3"
        ))
        .map_err(|error| error.to_string())?;
    let max_rows = usize::try_from(limits.prepared.max_mutations)
        .map_err(|_| invalid("navigation payload row limit"))?;
    let row_limit = i64::try_from(limits.prepared.max_mutations)
        .map_err(|_| invalid("navigation payload SQL row limit"))?
        .checked_add(1)
        .ok_or_else(|| invalid("navigation payload SQL row limit"))?;
    let mut rows = statement
        .query(params![metadata_bytes, id, row_limit])
        .map_err(|error| error.to_string())?;
    let mut output = Vec::new();
    let mut bytes = 0usize;
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        if output.len() >= max_rows {
            return Err(invalid("navigation payload row budget"));
        }
        let remaining = limits
            .prepared
            .max_metadata_bytes
            .checked_sub(bytes)
            .ok_or_else(|| invalid("navigation payload byte budget"))?;
        account_json_row(
            row,
            columns.len(),
            limits.prepared.max_row_bytes.min(remaining),
            read_bytes,
            "D1 payload JSON row byte budget",
        )?;
        let values = row_to_sql(row, columns.len())?;
        if values.get(1) != Some(&D1Cell::Integer(output.len() as i64)) {
            return Err(invalid("navigation payload part sequence"));
        }
        bytes = bytes.saturating_add(
            values
                .iter()
                .map(|cell| match cell {
                    D1Cell::Text(value) => value.len(),
                    D1Cell::Integer(value) => value.to_string().len(),
                    D1Cell::Null => 0,
                })
                .sum::<usize>(),
        );
        if bytes > limits.prepared.max_metadata_bytes {
            return Err(invalid("navigation payload byte budget"));
        }
        output.push(values);
    }
    Ok(output)
}

fn navigation_delta_transitions(
    db: &Transaction<'_>,
    before: &D1ProjectionSnapshot,
    after: &D1ProjectionSnapshot,
    limits: Limits,
    read_bytes: &mut D1ReadBytes,
) -> Result<
    (
        (
            BTreeMap<(D1Table, String), Vec<D1Cell>>,
            BTreeMap<(D1Table, String), Vec<D1Cell>>,
        ),
        Option<String>,
        u64,
        D1ProjectionAccounting,
    ),
    String,
> {
    let before_root: Value = serde_json::from_slice(before.root_bytes())
        .map_err(|_| invalid("predecessor source-navigation root JSON"))?;
    let after_root: Value = serde_json::from_slice(after.root_bytes())
        .map_err(|_| invalid("successor source-navigation root JSON"))?;
    if !matches!(
        before_root.get("logical_schema").and_then(Value::as_str),
        Some("tos_source_navigation_v1" | "tos_agent_source_navigation_rows_v1")
    ) || before_root.get("logical_schema") != after_root.get("logical_schema")
    {
        return Err(invalid("source-navigation delta projection profile"));
    }
    let before_specs = before_root
        .get("collections")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("predecessor source-navigation collections"))?;
    let after_specs = after_root
        .get("collections")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("successor source-navigation collections"))?;
    if before_specs.keys().ne(after_specs.keys())
        || before_specs
            .keys()
            .any(|name| !matches!(name.as_str(), "nodes" | "edges" | "rights"))
    {
        return Err(invalid("source-navigation collection set changed"));
    }
    for name in before_specs.keys() {
        let key = match name.as_str() {
            "nodes" => "node_id",
            "edges" => "edge_id",
            "rights" => "rights_id",
            _ => return Err(invalid("source-navigation collection identity")),
        };
        for spec in [&before_specs[name], &after_specs[name]] {
            if spec.get("key_field").and_then(Value::as_str) != Some(key)
                || spec.get("order_fields") != Some(&json!([key]))
            {
                return Err(invalid("source-navigation collection order profile"));
            }
        }
    }
    let old_header = before.metadata().map_err(|error| error.to_string())?;
    let next_header = after.metadata().map_err(|error| error.to_string())?;
    let mut old_policy = old_header.clone();
    let mut new_policy = next_header.clone();
    if let Some(object) = old_policy.as_object_mut() {
        object.remove("counts");
    }
    if let Some(object) = new_policy.as_object_mut() {
        object.remove("counts");
    }
    if old_policy != new_policy {
        return Err(invalid("source-navigation header policy changed"));
    }
    let (top, top_rows, top_raw) =
        parse_meta_accounted(db, "source_navigation_top", limits, &mut *read_bytes)?;
    let collections = ["nodes", "edges", "rights"];
    if top == json!({}) {
        for table in [
            "source_navigation_nodes",
            "source_navigation_node_payload",
            "source_navigation_edges",
            "source_navigation_edge_payload",
            "source_navigation_rights",
            "source_navigation_rights_payload",
        ] {
            let present: Option<i64> = db
                .query_row(&format!("SELECT 1 FROM {table} LIMIT 1"), [], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(|error| error.to_string())?;
            if present.is_some() {
                return Err(invalid(
                    "unavailable native navigation contains serving rows",
                ));
            }
        }
        if meta_exists(db, "source_navigation_header_digest")? {
            return Err(invalid("unavailable navigation has a header digest"));
        }
        let (changed_rows, accounting) =
            navigation_snapshot_diff_usage(before, after, &before_specs, limits)?;
        let _ = top_rows;
        return Ok((
            (BTreeMap::new(), BTreeMap::new()),
            None,
            changed_rows,
            accounting,
        ));
    }
    if top.get("schema_version").and_then(Value::as_str) != Some("tos_source_navigation_v1") {
        return Err(invalid("unsupported native source-navigation D1 product"));
    }
    for collection in collections {
        if !before_specs.contains_key(collection) {
            continue;
        }
        let count = before_specs[collection]
            .get("root")
            .and_then(|root| root.get("count"))
            .ok_or_else(|| invalid("source-navigation predecessor row count"))?;
        if top
            .get("counts")
            .and_then(Value::as_object)
            .and_then(|counts| counts.get(collection))
            != Some(count)
        {
            return Err(invalid("native navigation predecessor row count differs"));
        }
    }
    let (prior_digest, prior_digest_rows, _) = parse_meta_accounted(
        db,
        "source_navigation_header_digest",
        limits,
        &mut *read_bytes,
    )?;
    let prior_raw = top_rows
        .iter()
        .filter_map(|row| row.before.as_ref())
        .filter_map(|row| row.get(2))
        .filter_map(|cell| {
            if let D1Cell::Text(text) = cell {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<String>();
    if prior_digest != json!({"sha256":digest(prior_raw.as_bytes())}) {
        return Err(invalid("native navigation header digest differs"));
    }
    let mut before_rows = BTreeMap::new();
    let mut after_rows = BTreeMap::new();
    let mut retained = RetainedBytes::new(limits);
    let mut remaining_projection = limits.projection;
    remaining_projection.max_output_bytes = remaining_projection.max_output_bytes.min(
        u64::try_from(limits.prepared.max_change_bytes)
            .map_err(|_| invalid("private capture retained byte limit"))?,
    );
    let mut projection_usage = D1ProjectionAccounting::default();
    let mut changed_rows = 0u64;
    for collection in collections {
        if !before_specs.contains_key(collection) {
            continue;
        }
        let diff = before
            .diff_collection(after, collection, remaining_projection)
            .map_err(|error| error.to_string())?;
        let used = diff.accounting;
        add_projection_accounting(&mut projection_usage, used)?;
        changed_rows = changed_rows
            .checked_add(
                u64::try_from(diff.changes.len())
                    .map_err(|_| invalid("source-navigation diff row count"))?,
            )
            .ok_or_else(|| invalid("source-navigation diff row count overflow"))?;
        remaining_projection.max_opened_parts = remaining_projection
            .max_opened_parts
            .checked_sub(used.opened_parts)
            .ok_or_else(|| invalid("cumulative source-navigation part budget"))?;
        remaining_projection.max_read_bytes = remaining_projection
            .max_read_bytes
            .checked_sub(used.stored_bytes.saturating_add(used.decoded_bytes))
            .ok_or_else(|| invalid("cumulative source-navigation read budget"))?;
        remaining_projection.max_stored_read_bytes = remaining_projection
            .max_stored_read_bytes
            .checked_sub(used.stored_bytes)
            .ok_or_else(|| invalid("cumulative source-navigation stored budget"))?;
        remaining_projection.max_decoded_bytes = remaining_projection
            .max_decoded_bytes
            .checked_sub(used.decoded_bytes)
            .ok_or_else(|| invalid("cumulative source-navigation decoded budget"))?;
        remaining_projection.max_keys = remaining_projection
            .max_keys
            .checked_sub(used.keys)
            .ok_or_else(|| invalid("cumulative source-navigation key budget"))?;
        remaining_projection.max_changes = remaining_projection
            .max_changes
            .checked_sub(used.changes)
            .ok_or_else(|| invalid("cumulative source-navigation change budget"))?;
        remaining_projection.max_output_bytes = remaining_projection
            .max_output_bytes
            .checked_sub(used.output_bytes)
            .ok_or_else(|| invalid("cumulative source-navigation output budget"))?;
        let key_field = before_specs[collection]
            .get("key_field")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("source-navigation key field"))?;
        let table = match collection {
            "nodes" => D1Table::SourceNavigationNodes,
            "edges" => D1Table::SourceNavigationEdges,
            "rights" => D1Table::SourceNavigationRights,
            _ => return Err(invalid("source-navigation table")),
        };
        let payload_table = match collection {
            "nodes" => D1Table::SourceNavigationNodePayload,
            "edges" => D1Table::SourceNavigationEdgePayload,
            "rights" => D1Table::SourceNavigationRightsPayload,
            _ => return Err(invalid("source-navigation payload table")),
        };
        for change in diff.changes {
            let identity = change.key;
            let old = change.before.as_ref();
            let new = change.after.as_ref();
            let old_source_id = old
                .and_then(|row| row.get(key_field))
                .and_then(Value::as_str);
            let new_source_id = new
                .and_then(|row| row.get(key_field))
                .and_then(Value::as_str);
            if old.is_some_and(|_| old_source_id != Some(identity.as_str()))
                || new.is_some_and(|_| new_source_id != Some(identity.as_str()))
            {
                return Err(invalid("source-navigation row identity differs"));
            }
            let selected_key = vec![D1Cell::Text(identity.clone())];
            let actual = selected_tuple_accounted(db, table, &selected_key, limits, read_bytes)?;
            let payload = navigation_payload_rows_accounted(
                db,
                payload_table,
                &identity,
                limits,
                read_bytes,
            )?;
            let digest_key = format!(
                "source_navigation_row_digest:{collection}:{}",
                digest(identity.as_bytes())
            );
            let mut actual_raw = None;
            let mut digest_rows = Vec::new();
            if old.is_some() != actual.is_some() {
                return Err(invalid("native navigation predecessor identity differs"));
            }
            if let Some(row) = &actual {
                let inline_index = table
                    .shape()
                    .1
                    .iter()
                    .position(|name| *name == "json")
                    .ok_or_else(|| invalid("native navigation JSON column"))?;
                let inline = match &row[inline_index] {
                    D1Cell::Text(value) => value.clone(),
                    _ => return Err(invalid("native navigation JSON type")),
                };
                if inline.is_empty() != !payload.is_empty() {
                    return Err(invalid("native navigation payload selection differs"));
                }
                let raw = if inline.is_empty() {
                    payload
                        .iter()
                        .map(|row| match &row[2] {
                            D1Cell::Text(value) => Ok(value.as_str()),
                            _ => Err(invalid("native navigation payload bytes")),
                        })
                        .collect::<Result<Vec<_>, _>>()?
                        .join("")
                } else {
                    inline
                };
                let current: Value =
                    serde_json::from_str(&raw).map_err(|_| invalid("native navigation D1 JSON"))?;
                if old != Some(&current) {
                    return Err(invalid("native navigation predecessor source row differs"));
                }
                let (row_digest, rows, _) =
                    parse_meta_accounted(db, &digest_key, limits, &mut *read_bytes)?;
                if row_digest != json!({"sha256":digest(raw.as_bytes())}) {
                    return Err(invalid("native navigation predecessor row digest differs"));
                }
                digest_rows = rows;
                actual_raw = Some(raw);
            } else if !payload.is_empty() || meta_exists(db, &digest_key)? {
                return Err(invalid("orphan native navigation payload or digest"));
            }
            let ordinal = actual
                .as_ref()
                .and_then(|row| row.get(1))
                .and_then(|value| {
                    if let D1Cell::Integer(number) = value {
                        Some(*number)
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            if let Some(raw) = actual_raw.as_deref() {
                let mut typed = foundation_raw(raw.as_bytes(), limits.prepared.max_row_bytes)?;
                let projected = projected_map(project_navigation_exact(
                    collection,
                    ordinal,
                    &mut typed,
                    &repository_root()?,
                )?)?;
                for ((projected_table, _), row) in &projected {
                    if *projected_table == table {
                        if actual.as_ref() != Some(row) {
                            return Err(invalid(
                                "native navigation predecessor serving row differs",
                            ));
                        }
                    } else if *projected_table == payload_table {
                        let expected = projected
                            .iter()
                            .filter(|((candidate_table, _), _)| *candidate_table == payload_table)
                            .map(|(_, candidate)| candidate.clone())
                            .collect::<BTreeSet<_>>();
                        if payload.iter().cloned().collect::<BTreeSet<_>>() != expected {
                            return Err(invalid(
                                "native navigation predecessor payload rows differ",
                            ));
                        }
                    }
                }
                for row in digest_rows.iter().filter_map(|item| item.before.as_ref()) {
                    let key = (D1Table::EdgeMeta, row_key(D1Table::EdgeMeta, row)?);
                    clone_capture_row(&mut before_rows, key, row, &mut retained)?;
                }
                for (key, row) in projected {
                    if key.0 != D1Table::EdgeMeta {
                        insert_capture_row(&mut before_rows, key, row, &mut retained)?;
                    }
                }
            }
            if let Some(new_value) = new {
                let mut typed = foundation(new_value, limits.prepared.max_row_bytes)?;
                let projected = projected_map(project_navigation_exact(
                    collection,
                    ordinal,
                    &mut typed,
                    &repository_root()?,
                )?)?;
                merge_projected(&mut after_rows, projected, &mut retained)?;
            }
        }
    }
    if changed_rows == 0 || changed_rows as usize > limits.prepared.max_changes {
        return Err(invalid("native source-navigation change budget"));
    }
    for row in top_rows.iter().filter_map(|item| item.before.as_ref()) {
        let key = (D1Table::EdgeMeta, row_key(D1Table::EdgeMeta, row)?);
        clone_capture_row(&mut before_rows, key, row, &mut retained)?;
    }
    for row in prior_digest_rows
        .iter()
        .filter_map(|item| item.before.as_ref())
    {
        let key = (D1Table::EdgeMeta, row_key(D1Table::EdgeMeta, row)?);
        clone_capture_row(&mut before_rows, key, row, &mut retained)?;
    }
    let next_top_raw = navigation_top_raw_from_predecessor(&top_raw, after.root_bytes(), limits)?;
    // The metadata transitions are retained together with projected rows.
    // Their bytes are charged by the caller when merged into the capture.
    Ok((
        (before_rows, after_rows),
        Some(next_top_raw),
        changed_rows,
        projection_usage,
    ))
}

fn add_projection_accounting(
    total: &mut D1ProjectionAccounting,
    used: D1ProjectionAccounting,
) -> Result<(), String> {
    total.opened_parts = total
        .opened_parts
        .checked_add(used.opened_parts)
        .ok_or_else(|| invalid("projection opened-part accounting overflow"))?;
    total.stored_bytes = total
        .stored_bytes
        .checked_add(used.stored_bytes)
        .ok_or_else(|| invalid("projection stored-byte accounting overflow"))?;
    total.decoded_bytes = total
        .decoded_bytes
        .checked_add(used.decoded_bytes)
        .ok_or_else(|| invalid("projection decoded-byte accounting overflow"))?;
    total.keys = total
        .keys
        .checked_add(used.keys)
        .ok_or_else(|| invalid("projection key accounting overflow"))?;
    total.changes = total
        .changes
        .checked_add(used.changes)
        .ok_or_else(|| invalid("projection change accounting overflow"))?;
    total.output_bytes = total
        .output_bytes
        .checked_add(used.output_bytes)
        .ok_or_else(|| invalid("projection output accounting overflow"))?;
    Ok(())
}

fn navigation_snapshot_diff_usage(
    before: &D1ProjectionSnapshot,
    after: &D1ProjectionSnapshot,
    specs: &Map<String, Value>,
    limits: Limits,
) -> Result<(u64, D1ProjectionAccounting), String> {
    let mut remaining = limits.projection;
    remaining.max_output_bytes = remaining.max_output_bytes.min(
        u64::try_from(limits.prepared.max_change_bytes)
            .map_err(|_| invalid("private capture retained byte limit"))?,
    );
    let mut changed_rows = 0u64;
    let mut total = D1ProjectionAccounting::default();
    for collection in ["nodes", "edges", "rights"] {
        if !specs.contains_key(collection) {
            continue;
        }
        let diff = before
            .diff_collection(after, collection, remaining)
            .map_err(|error| error.to_string())?;
        let used = diff.accounting;
        add_projection_accounting(&mut total, used)?;
        changed_rows = changed_rows
            .checked_add(
                u64::try_from(diff.changes.len())
                    .map_err(|_| invalid("source-navigation diff row count"))?,
            )
            .ok_or_else(|| invalid("source-navigation diff row count overflow"))?;
        remaining.max_opened_parts = remaining
            .max_opened_parts
            .checked_sub(used.opened_parts)
            .ok_or_else(|| invalid("cumulative source-navigation part budget"))?;
        let read = used
            .stored_bytes
            .checked_add(used.decoded_bytes)
            .ok_or_else(|| invalid("source-navigation read accounting overflow"))?;
        remaining.max_read_bytes = remaining
            .max_read_bytes
            .checked_sub(read)
            .ok_or_else(|| invalid("cumulative source-navigation read budget"))?;
        remaining.max_stored_read_bytes = remaining
            .max_stored_read_bytes
            .checked_sub(used.stored_bytes)
            .ok_or_else(|| invalid("cumulative source-navigation stored budget"))?;
        remaining.max_decoded_bytes = remaining
            .max_decoded_bytes
            .checked_sub(used.decoded_bytes)
            .ok_or_else(|| invalid("cumulative source-navigation decoded budget"))?;
        remaining.max_keys = remaining
            .max_keys
            .checked_sub(used.keys)
            .ok_or_else(|| invalid("cumulative source-navigation key budget"))?;
        remaining.max_changes = remaining
            .max_changes
            .checked_sub(used.changes)
            .ok_or_else(|| invalid("cumulative source-navigation change budget"))?;
        remaining.max_output_bytes = remaining
            .max_output_bytes
            .checked_sub(used.output_bytes)
            .ok_or_else(|| invalid("cumulative source-navigation output budget"))?;
    }
    if changed_rows == 0 || changed_rows as usize > limits.prepared.max_changes {
        return Err(invalid("native source-navigation change budget"));
    }
    Ok((changed_rows, total))
}

fn bootstrap_transitions(
    db: &Transaction<'_>,
    after_source: &PreparedSourceInputs,
    nav: &D1ProjectionSnapshot,
    rights: &D1ProjectionSnapshot,
    limits: Limits,
    read_bytes: &mut D1ReadBytes,
) -> Result<
    (
        Vec<D1RowTransition>,
        Value,
        D1ProjectionAccounting,
        usize,
        String,
    ),
    String,
> {
    let mut counts = Map::new();
    let mut before_rows = BTreeMap::new();
    let mut after_rows = BTreeMap::new();
    let mut retained = RetainedBytes::new(limits);
    let mut projection_usage = D1ProjectionAccounting::default();
    let mut remaining_projection = limits.projection;
    remaining_projection.max_output_bytes = remaining_projection.max_output_bytes.min(
        u64::try_from(limits.prepared.max_change_bytes)
            .map_err(|_| invalid("private capture retained byte limit"))?,
    );
    let mut total_source_rows = 0u64;
    for (collection, table) in [("nodes", "nodes"), ("edges", "edges"), ("rights", "rights")] {
        let selected = if collection == "rights" { rights } else { nav };
        let collection_data = selected
            .read_collection(collection, remaining_projection)
            .map_err(|error| error.to_string())?;
        let key_field = match collection {
            "nodes" => "node_id",
            "edges" => "edge_id",
            "rights" => "rights_id",
            _ => return Err(invalid("bootstrap source collection")),
        };
        if collection_data.key_field.as_str() != Some(key_field) {
            return Err(invalid("bootstrap projection key field differs"));
        }
        let row_count = collection_data.rows.len() as u64;
        total_source_rows = total_source_rows
            .checked_add(row_count)
            .ok_or_else(|| invalid("bootstrap source row count overflow"))?;
        if total_source_rows > limits.projection.max_rows {
            return Err(invalid("private bootstrap source row budget"));
        }
        let used = collection_data.accounting;
        add_projection_accounting(&mut projection_usage, used)?;
        remaining_projection.max_opened_parts = remaining_projection
            .max_opened_parts
            .checked_sub(used.opened_parts)
            .ok_or_else(|| invalid("cumulative bootstrap part budget"))?;
        remaining_projection.max_read_bytes = remaining_projection
            .max_read_bytes
            .checked_sub(used.stored_bytes.saturating_add(used.decoded_bytes))
            .ok_or_else(|| invalid("cumulative bootstrap read budget"))?;
        remaining_projection.max_stored_read_bytes = remaining_projection
            .max_stored_read_bytes
            .checked_sub(used.stored_bytes)
            .ok_or_else(|| invalid("cumulative bootstrap stored-byte budget"))?;
        remaining_projection.max_decoded_bytes = remaining_projection
            .max_decoded_bytes
            .checked_sub(used.decoded_bytes)
            .ok_or_else(|| invalid("cumulative bootstrap decoded-byte budget"))?;
        remaining_projection.max_keys = remaining_projection
            .max_keys
            .checked_sub(used.keys)
            .ok_or_else(|| invalid("cumulative bootstrap key budget"))?;
        remaining_projection.max_rows = remaining_projection
            .max_rows
            .checked_sub(row_count)
            .ok_or_else(|| invalid("cumulative bootstrap row budget"))?;
        remaining_projection.max_output_bytes = remaining_projection
            .max_output_bytes
            .checked_sub(used.output_bytes)
            .ok_or_else(|| invalid("cumulative bootstrap output budget"))?;
        counts.insert(collection.to_owned(), json!(row_count));
        let root_json = selected.root_bytes();
        let root_value: Value =
            serde_json::from_slice(root_json).map_err(|_| invalid("projection root JSON"))?;
        let selected_repo_root = root_value
            .get("header")
            .and_then(|header| header.get("repository_root"))
            .and_then(Value::as_str);
        let repo_root = repository_root()?;
        if selected_repo_root.is_some_and(|selected| selected != repo_root.as_str())
            || !Path::new(&repo_root).is_absolute()
        {
            return Err(invalid(
                "source-navigation repository root differs from the producer",
            ));
        }
        for row in collection_data.rows.values() {
            let mut item = foundation(row, limits.prepared.max_row_bytes)?;
            put_projected_rows(
                &mut after_rows,
                project_navigation_exact(table, 0, &mut item, &repo_root)?,
                &mut retained,
            )?;
        }
    }
    let rights_root = foundation_raw(rights.root_bytes(), 256 * 1024)?;
    let rights_header = rights_root
        .object_get("header")
        .ok_or_else(|| invalid("rights header"))?;
    let header = rights_header
        .object_get("navigation_header")
        .cloned()
        .ok_or_else(|| invalid("rights snapshot navigation header required"))?;
    if header
        .object_get("schema_version")
        .and_then(JsonValue::as_str)
        != Some("tos_source_navigation_v1")
        || header
            .object_get("authority_boundary")
            .and_then(JsonValue::as_str)
            .is_none_or(str::is_empty)
    {
        return Err(invalid("complete native navigation header required"));
    }
    let header_raw = compact_foundation(&header, limits.prepared.max_metadata_bytes)?;
    let header: Value =
        serde_json::from_str(&header_raw).map_err(|_| invalid("navigation header JSON"))?;
    if header.get("counts") != Some(&Value::Object(counts.clone())) {
        return Err(invalid(
            "navigation header counts differ from retained rows",
        ));
    }
    let (prior_top, prior_top_rows, _) =
        parse_meta_accounted(db, "source_navigation_top", limits, read_bytes)?;
    if prior_top != json!({}) {
        return Err(invalid("native navigation product already present"));
    }
    let top_transitions = meta_raw_transitions(
        "source_navigation_top",
        &prior_top_rows,
        &header_raw,
        limits,
    )?;
    capture_transition_rows(
        &mut before_rows,
        &mut after_rows,
        top_transitions,
        &mut retained,
    )?;

    let header_digest_key = "source_navigation_header_digest";
    if meta_exists(db, header_digest_key)? {
        return Err(invalid("navigation header digest already present"));
    }
    let header_digest_raw = compact(
        &json!({"sha256": digest(header_raw.as_bytes())}),
        limits.prepared.max_metadata_bytes,
    )?;
    capture_transition_rows(
        &mut before_rows,
        &mut after_rows,
        meta_raw_transitions(header_digest_key, &[], &header_digest_raw, limits)?,
        &mut retained,
    )?;

    let transitions = row_transitions(before_rows, after_rows, limits, &mut retained)?;
    if transitions.is_empty() {
        return Err(invalid("empty private bootstrap product"));
    }
    let _ = after_source;
    Ok((
        transitions,
        header,
        projection_usage,
        retained.used,
        digest(header_raw.as_bytes()),
    ))
}

fn private_projection_root(value: &Value) -> Result<D1ProjectionSnapshot, String> {
    exact(
        value,
        &["expected_sha256", "namespace_path", "root_json"],
        "integrity projection root fields",
    )?;
    let expected = string(value, "expected_sha256")?;
    let root = string(value, "root_json")?;
    let namespace = PathBuf::from(string(value, "namespace_path")?);
    if root.len() > 256 * 1024 {
        return Err(invalid("integrity projection root byte budget"));
    }
    let snapshot = D1ProjectionSnapshot::new(root.as_bytes().to_vec(), namespace)
        .map_err(|error| error.to_string())?;
    if snapshot.snapshot_sha256() != expected {
        return Err(invalid("integrity projection root digest differs"));
    }
    Ok(snapshot)
}

fn private_trusted_projection_root(
    value: &Value,
) -> Result<(D1ProjectionSnapshot, String, String), String> {
    exact(
        value,
        &[
            "expected_sha256",
            "namespace_path",
            "root_json",
            "trusted_sha256",
        ],
        "trusted rights projection root fields",
    )?;
    let expected = string(value, "expected_sha256")?.to_owned();
    let trusted = string(value, "trusted_sha256")?.to_owned();
    if expected != trusted {
        return Err(invalid("rights expected and trusted digests differ"));
    }
    let root = private_projection_root(&json!({
        "expected_sha256": expected.clone(),
        "namespace_path": string(value, "namespace_path")?,
        "root_json": string(value, "root_json")?,
    }))?;
    Ok((root, expected, trusted))
}

fn projection_collection_count(root: &Value, collection: &str) -> Result<u64, String> {
    root.get("collections")
        .and_then(Value::as_object)
        .and_then(|collections| collections.get(collection))
        .and_then(|spec| spec.get("root"))
        .and_then(|descriptor| descriptor.get("count"))
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("integrity projection collection count"))
}

fn validate_navigation_integrity_inputs(
    navigation: &D1ProjectionSnapshot,
    rights: &D1ProjectionSnapshot,
    persisted_header: &Value,
    limits: Limits,
) -> Result<BTreeMap<String, u64>, String> {
    let navigation_root: Value = serde_json::from_slice(navigation.root_bytes())
        .map_err(|_| invalid("integrity navigation root JSON"))?;
    let rights_root: Value = serde_json::from_slice(rights.root_bytes())
        .map_err(|_| invalid("integrity rights root JSON"))?;
    let navigation_collections = navigation_root
        .get("collections")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("integrity navigation collections"))?;
    let rights_collections = rights_root
        .get("collections")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("integrity rights collections"))?;
    if !matches!(
        navigation_root
            .get("logical_schema")
            .and_then(Value::as_str),
        Some("tos_source_navigation_v1" | "tos_agent_source_navigation_rows_v1")
    ) || navigation_collections.len() != 2
        || !navigation_collections.contains_key("nodes")
        || !navigation_collections.contains_key("edges")
        || rights_root.get("logical_schema").and_then(Value::as_str)
            != Some("tos_source_navigation_rights_v1")
        || rights_collections.len() != 1
        || !rights_collections.contains_key("rights")
    {
        return Err(invalid("integrity navigation/rights projection profile"));
    }
    for (collection, key_field) in [("nodes", "node_id"), ("edges", "edge_id")] {
        let spec = &navigation_collections[collection];
        if spec.get("key_field").and_then(Value::as_str) != Some(key_field)
            || spec.get("order_fields") != Some(&json!([key_field]))
        {
            return Err(invalid("integrity navigation identity/order profile"));
        }
    }
    let rights_spec = &rights_collections["rights"];
    if rights_spec.get("key_field").and_then(Value::as_str) != Some("rights_id")
        || rights_spec.get("order_fields") != Some(&json!(["rights_id"]))
    {
        return Err(invalid("integrity rights identity/order profile"));
    }
    if persisted_header
        .get("schema_version")
        .and_then(Value::as_str)
        != Some("tos_source_navigation_v1")
    {
        return Err(invalid("complete existing native navigation required"));
    }
    let rights_header = rights
        .metadata()
        .map_err(|error| error.to_string())?
        .get("navigation_header")
        .cloned()
        .ok_or_else(|| invalid("integrity rights navigation header"))?;
    let policy = |value: &Value| {
        let mut policy = value.clone();
        if let Some(object) = policy.as_object_mut() {
            object.remove("counts");
        }
        policy
    };
    if !rights_header.is_object() || policy(&rights_header) != policy(persisted_header) {
        return Err(invalid(
            "native navigation header policy differs from admitted rights input",
        ));
    }
    let navigation_header = navigation.metadata().map_err(|error| error.to_string())?;
    match navigation_root
        .get("logical_schema")
        .and_then(Value::as_str)
    {
        Some("tos_source_navigation_v1") => {
            if navigation_header != *persisted_header {
                return Err(invalid(
                    "native navigation header differs from complete navigation input",
                ));
            }
        }
        Some("tos_agent_source_navigation_rows_v1") => {
            // This owner contract is a raw row root, deliberately without
            // copied global authority/count/source-revision metadata.
            if navigation_header != json!({"schema_version":"tos_agent_source_navigation_rows_v1"})
            {
                return Err(invalid("native Agent navigation raw header profile"));
            }
        }
        _ => return Err(invalid("integrity navigation header profile")),
    }
    let mut counts = BTreeMap::new();
    let mut total = 0u64;
    for collection in ["nodes", "edges", "rights"] {
        let root = if collection == "rights" {
            &rights_root
        } else {
            &navigation_root
        };
        let count = projection_collection_count(root, collection)?;
        total = total
            .checked_add(count)
            .ok_or_else(|| invalid("integrity source row count overflow"))?;
        counts.insert(collection.to_owned(), count);
    }
    if persisted_header.get("counts")
        != Some(&json!({
            "nodes": counts["nodes"],
            "edges": counts["edges"],
            "rights": counts["rights"],
        }))
        || total > limits.prepared.max_mutations
        || total > limits.projection.max_rows
    {
        return Err(invalid(
            "native navigation inventory differs or exceeds migration budget",
        ));
    }
    Ok(counts)
}

fn count_table_rows(db: &Transaction<'_>, table: D1Table) -> Result<i64, String> {
    let (name, _, _) = table.shape();
    db.query_row(&format!("SELECT count(*) FROM {name}"), [], |row| {
        row.get(0)
    })
    .map_err(|error| error.to_string())
}

fn read_navigation_integrity_rows(
    db: &Transaction<'_>,
    navigation: &D1ProjectionSnapshot,
    rights: &D1ProjectionSnapshot,
    counts: &BTreeMap<String, u64>,
    limits: Limits,
    retained: &mut RetainedBytes,
    after: &mut BTreeMap<(D1Table, String), Vec<D1Cell>>,
    projection_usage: &mut D1ProjectionAccounting,
    read_bytes: &mut D1ReadBytes,
) -> Result<BTreeMap<String, u64>, String> {
    let repo_root = repository_root()?;
    if !Path::new(&repo_root).is_absolute() {
        return Err(invalid("integrity repository root"));
    }
    let mut remaining = limits.projection;
    let mut verified = BTreeMap::new();
    for collection in ["nodes", "edges", "rights"] {
        let (base_table, payload_table) = match collection {
            "nodes" => (
                D1Table::SourceNavigationNodes,
                D1Table::SourceNavigationNodePayload,
            ),
            "edges" => (
                D1Table::SourceNavigationEdges,
                D1Table::SourceNavigationEdgePayload,
            ),
            "rights" => (
                D1Table::SourceNavigationRights,
                D1Table::SourceNavigationRightsPayload,
            ),
            _ => return Err(invalid("integrity collection")),
        };
        let snapshot = if collection == "rights" {
            rights
        } else {
            navigation
        };
        let selected = snapshot
            .read_collection(collection, remaining)
            .map_err(|error| error.to_string())?;
        let row_count = u64::try_from(selected.rows.len())
            .map_err(|_| invalid("integrity source row count"))?;
        if Some(&row_count) != counts.get(collection) {
            return Err(invalid("integrity projection count differs"));
        }
        let used = selected.accounting;
        projection_usage.opened_parts = projection_usage
            .opened_parts
            .checked_add(used.opened_parts)
            .ok_or_else(|| invalid("integrity opened-part count overflow"))?;
        projection_usage.stored_bytes = projection_usage
            .stored_bytes
            .checked_add(used.stored_bytes)
            .ok_or_else(|| invalid("integrity stored-byte count overflow"))?;
        projection_usage.decoded_bytes = projection_usage
            .decoded_bytes
            .checked_add(used.decoded_bytes)
            .ok_or_else(|| invalid("integrity decoded-byte count overflow"))?;
        projection_usage.keys = projection_usage
            .keys
            .checked_add(used.keys)
            .ok_or_else(|| invalid("integrity key count overflow"))?;
        projection_usage.changes = projection_usage
            .changes
            .checked_add(used.changes)
            .ok_or_else(|| invalid("integrity change count overflow"))?;
        projection_usage.output_bytes = projection_usage
            .output_bytes
            .checked_add(used.output_bytes)
            .ok_or_else(|| invalid("integrity output-byte count overflow"))?;
        remaining.max_opened_parts = remaining
            .max_opened_parts
            .checked_sub(used.opened_parts)
            .ok_or_else(|| invalid("cumulative integrity part budget"))?;
        remaining.max_read_bytes = remaining
            .max_read_bytes
            .checked_sub(used.stored_bytes.saturating_add(used.decoded_bytes))
            .ok_or_else(|| invalid("cumulative integrity read budget"))?;
        remaining.max_stored_read_bytes = remaining
            .max_stored_read_bytes
            .checked_sub(used.stored_bytes)
            .ok_or_else(|| invalid("cumulative integrity stored-byte budget"))?;
        remaining.max_decoded_bytes = remaining
            .max_decoded_bytes
            .checked_sub(used.decoded_bytes)
            .ok_or_else(|| invalid("cumulative integrity decoded-byte budget"))?;
        remaining.max_keys = remaining
            .max_keys
            .checked_sub(used.keys)
            .ok_or_else(|| invalid("cumulative integrity key budget"))?;
        remaining.max_rows = remaining
            .max_rows
            .checked_sub(row_count)
            .ok_or_else(|| invalid("cumulative integrity row budget"))?;
        remaining.max_output_bytes = remaining
            .max_output_bytes
            .checked_sub(used.output_bytes)
            .ok_or_else(|| invalid("cumulative integrity output budget"))?;

        let mut payload_rows = 0u64;
        let mut row_digests = 0u64;
        for (identifier, source_row) in &selected.rows {
            let actual = selected_tuple_accounted(
                db,
                base_table,
                &[D1Cell::Text(identifier.clone())],
                limits,
                read_bytes,
            )?
            .ok_or_else(|| invalid("native source row missing"))?;
            let columns = base_table.shape().1;
            let ordinal_index = columns
                .iter()
                .position(|column| *column == "ord")
                .ok_or_else(|| invalid("native source row order column"))?;
            let ordinal = match actual.get(ordinal_index) {
                Some(D1Cell::Integer(value)) => *value,
                _ => return Err(invalid("native source row ordinal")),
            };
            let mut typed = foundation(source_row, limits.prepared.max_row_bytes)?;
            let projected = projected_map(project_navigation_exact(
                collection, ordinal, &mut typed, &repo_root,
            )?)?;
            let expected_base = projected
                .iter()
                .find_map(|((table, _), row)| (*table == base_table).then_some(row))
                .ok_or_else(|| invalid("navigation projector base row"))?;
            if !matches!(expected_base.first(), Some(D1Cell::Text(id)) if id == identifier)
                || &actual != expected_base
            {
                return Err(invalid(
                    "native full source row differs from admitted input",
                ));
            }
            let mut expected_payload = projected
                .iter()
                .filter(|((table, _), _)| *table == payload_table)
                .map(|(_, row)| row.clone())
                .collect::<Vec<_>>();
            expected_payload.sort_by_key(|row| match row.get(1) {
                Some(D1Cell::Integer(part)) => *part,
                _ => i64::MIN,
            });
            let actual_payload = navigation_payload_rows_accounted(
                db,
                payload_table,
                identifier,
                limits,
                read_bytes,
            )?;
            if actual_payload != expected_payload {
                return Err(invalid("native source payload differs from admitted input"));
            }
            payload_rows = payload_rows
                .checked_add(expected_payload.len() as u64)
                .ok_or_else(|| invalid("integrity payload row count overflow"))?;
            for ((table, key), row) in projected {
                if table != D1Table::EdgeMeta {
                    continue;
                }
                if !key.starts_with("source_navigation_row_digest:")
                    || row.get(1) != Some(&D1Cell::Integer(0))
                {
                    return Err(invalid(
                        "navigation projector emitted unexpected integrity metadata",
                    ));
                }
                insert_capture_row(after, (table, key), row, retained)?;
                row_digests = row_digests
                    .checked_add(1)
                    .ok_or_else(|| invalid("integrity row digest count overflow"))?;
            }
        }
        let actual_base_rows = count_table_rows(db, base_table)?;
        let actual_payload_rows = count_table_rows(db, payload_table)?;
        if row_digests != row_count
            || actual_base_rows < 0
            || actual_base_rows as u64 != row_count
            || actual_payload_rows < 0
            || actual_payload_rows as u64 != payload_rows
        {
            return Err(invalid("native product has missing or orphan source rows"));
        }
        verified.insert(collection.to_owned(), row_count);
    }
    Ok(verified)
}

fn merge_projected(
    destination: &mut BTreeMap<(D1Table, String), Vec<D1Cell>>,
    projected: BTreeMap<(D1Table, String), Vec<D1Cell>>,
    retained: &mut RetainedBytes,
) -> Result<(), String> {
    for (key, row) in projected {
        insert_capture_row(destination, key, row, retained)?;
    }
    Ok(())
}

fn capture_transition_rows(
    before: &mut BTreeMap<(D1Table, String), Vec<D1Cell>>,
    after: &mut BTreeMap<(D1Table, String), Vec<D1Cell>>,
    transitions: Vec<D1RowTransition>,
    retained: &mut RetainedBytes,
) -> Result<(), String> {
    for transition in transitions {
        for (destination, row) in [
            (&mut *before, transition.before),
            (&mut *after, transition.after),
        ] {
            let Some(row) = row else { continue };
            let key = (transition.table, row_key(transition.table, &row)?);
            insert_capture_row(destination, key, row, retained)?;
        }
    }
    Ok(())
}

fn reader_top_raw_from_predecessor(
    predecessor_raw: &str,
    source_revision: &str,
    data_revision: &str,
    catalog_sha256: &str,
    lens_sha256: &str,
    limits: Limits,
) -> Result<String, String> {
    let predecessor = foundation_raw(
        predecessor_raw.as_bytes(),
        limits.prepared.max_metadata_bytes,
    )?;
    let fields = predecessor
        .as_object()
        .ok_or_else(|| invalid("D1 reader top object"))?;
    let mut seen = BTreeSet::new();
    let mut updated = Vec::with_capacity(fields.len());
    for (key, value) in fields {
        let name = key
            .as_str()
            .ok_or_else(|| invalid("D1 reader top field name"))?;
        let replacement = match name {
            "source_revision" => Some(source_revision),
            "data_revision" => Some(data_revision),
            "catalog_sha256" => Some(catalog_sha256),
            "lens_sha256" => Some(lens_sha256),
            _ => None,
        };
        if let Some(text) = replacement {
            seen.insert(name);
            let encoded = serde_json::to_vec(text).map_err(|error| error.to_string())?;
            updated.push((key.clone(), foundation_raw(&encoded, 128)?));
        } else {
            updated.push((key.clone(), value.clone()));
        }
    }
    if seen.len() != 4 {
        return Err(invalid("D1 reader top lacks a bound metadata field"));
    }
    let root = JsonValue::Object(updated);
    let raw = emit_python_compact_json(
        &root,
        JsonLimits::new(
            limits.prepared.max_metadata_bytes,
            128,
            CAPTURE_JSON_VISITS,
            4300,
        )
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    String::from_utf8(raw).map_err(|error| error.to_string())
}

fn navigation_top_raw_from_predecessor(
    predecessor_raw: &str,
    successor_root_raw: &[u8],
    limits: Limits,
) -> Result<String, String> {
    let predecessor = foundation_raw(
        predecessor_raw.as_bytes(),
        limits.prepared.max_metadata_bytes,
    )?;
    let successor_root = foundation_raw(
        successor_root_raw,
        usize::try_from(limits.projection.max_read_bytes)
            .map_err(|_| invalid("source-navigation root byte budget"))?,
    )?;
    let successor_collections = successor_root
        .object_get("collections")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| invalid("source-navigation successor collections"))?;
    let mut successor_counts = Vec::with_capacity(successor_collections.len());
    for (name, spec) in successor_collections {
        let count = spec
            .object_get("root")
            .and_then(|root| root.object_get("count"))
            .ok_or_else(|| invalid("source-navigation successor count"))?;
        if count.as_u64().is_none() {
            return Err(invalid("source-navigation successor count type"));
        }
        successor_counts.push((name.clone(), count.clone()));
    }
    let predecessor_fields = predecessor
        .as_object()
        .ok_or_else(|| invalid("source-navigation predecessor metadata object"))?;
    let mut seen_counts = false;
    let mut updated_fields = Vec::with_capacity(predecessor_fields.len());
    for (name, value) in predecessor_fields {
        if name.as_str() != Some("counts") {
            updated_fields.push((name.clone(), value.clone()));
            continue;
        }
        if seen_counts {
            return Err(invalid("duplicate source-navigation counts field"));
        }
        seen_counts = true;
        let predecessor_counts = value
            .as_object()
            .ok_or_else(|| invalid("source-navigation predecessor counts"))?;
        let mut updated_counts =
            Vec::with_capacity(predecessor_counts.len().max(successor_counts.len()));
        let mut found = BTreeSet::new();
        for (collection, old_count) in predecessor_counts {
            let collection_name = collection
                .as_str()
                .ok_or_else(|| invalid("source-navigation count field name"))?;
            if let Some((_, next_count)) = successor_counts
                .iter()
                .find(|(candidate, _)| candidate == collection)
            {
                updated_counts.push((collection.clone(), next_count.clone()));
                found.insert(collection_name.to_owned());
            } else {
                updated_counts.push((collection.clone(), old_count.clone()));
            }
        }
        for (collection, next_count) in &successor_counts {
            let collection_name = collection
                .as_str()
                .ok_or_else(|| invalid("source-navigation successor field name"))?;
            if !found.contains(collection_name) {
                updated_counts.push((collection.clone(), next_count.clone()));
            }
        }
        updated_fields.push((name.clone(), JsonValue::Object(updated_counts)));
    }
    if !seen_counts {
        return Err(invalid("source-navigation predecessor lacks counts"));
    }
    let raw = emit_python_compact_json(
        &JsonValue::Object(updated_fields),
        JsonLimits::new(
            limits.prepared.max_metadata_bytes,
            128,
            CAPTURE_JSON_VISITS,
            4300,
        )
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    String::from_utf8(raw).map_err(|error| error.to_string())
}

fn manifest_sha(value: &Value) -> Result<&str, String> {
    let sha = value
        .as_object()
        .filter(|object| object.len() == 1)
        .and_then(|_| value.get("sha256"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("prepared digest frame"))?;
    if sha.len() != 64
        || !sha
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid("prepared digest frame SHA"));
    }
    Ok(sha)
}

#[derive(Clone)]
struct CapturedChange {
    kind: String,
    id: String,
    after_digest: Option<String>,
    operation: Option<String>,
}

fn add_change(
    changes: &mut BTreeMap<(String, String), CapturedChange>,
    kind: &str,
    id: &str,
    operation: Option<&str>,
    after_digest: Option<&str>,
    max: usize,
    retained: &mut RetainedBytes,
) -> Result<(), String> {
    if !matches!(kind, "node" | "relation") || id.is_empty() || id.len() > 4096 {
        return Err(invalid("prepared change identity"));
    }
    let bytes = kind
        .len()
        .checked_add(id.len())
        .and_then(|bytes| bytes.checked_mul(2))
        .and_then(|bytes| bytes.checked_add(operation.map_or(0, str::len)))
        .and_then(|bytes| bytes.checked_add(after_digest.map_or(0, str::len)))
        .ok_or_else(|| invalid("private capture retained byte overflow"))?;
    retained.charge(bytes)?;
    let key = (kind.to_owned(), id.to_owned());
    if changes
        .insert(
            key,
            CapturedChange {
                kind: kind.to_owned(),
                id: id.to_owned(),
                after_digest: after_digest.map(str::to_owned),
                operation: operation.map(str::to_owned),
            },
        )
        .is_some()
    {
        return Err(invalid("duplicate prepared change identity"));
    }
    if changes.len() > max {
        return Err(invalid("prepared change count budget"));
    }
    Ok(())
}

fn native_address_plans(
    expected_revision: &str,
    positions_before: &BTreeMap<(String, String), i64>,
    positions_after: &BTreeMap<(String, String), i64>,
    high_water_before: &BTreeMap<String, i64>,
    high_water_after: &BTreeMap<String, i64>,
    retained: &mut RetainedBytes,
) -> Result<Vec<Value>, String> {
    let mut plans = Vec::new();
    for (kind, plural) in [("node", "nodes"), ("relation", "relations")] {
        let has_positions = positions_before
            .keys()
            .any(|(selected, _)| selected == kind)
            || positions_after.keys().any(|(selected, _)| selected == kind);
        if !has_positions {
            continue;
        }
        let mut before = Map::new();
        for ((selected, id), position) in positions_before {
            if selected == kind {
                retained.text(id)?;
                before.insert(id.clone(), json!(position));
            }
        }
        let mut after = Map::new();
        for ((selected, id), position) in positions_after {
            if selected == kind {
                retained.text(id)?;
                after.insert(id.clone(), json!(position));
            }
        }
        let mut changed_ids = Vec::new();
        for ((selected, id), position) in positions_before {
            if selected == kind
                && positions_after.get(&(kind.to_owned(), id.clone())) != Some(position)
            {
                retained.text(id)?;
                changed_ids.push(id.clone());
            }
        }
        for ((selected, id), position) in positions_after {
            if selected == kind
                && positions_before.get(&(kind.to_owned(), id.clone())) != Some(position)
                && !changed_ids.iter().any(|changed| changed == id)
            {
                retained.text(id)?;
                changed_ids.push(id.clone());
            }
        }
        changed_ids.sort();
        let initial = high_water_before
            .get(kind)
            .copied()
            .ok_or_else(|| invalid("native address plan initial high-water absent"))?;
        let final_high = high_water_after
            .get(kind)
            .copied()
            .ok_or_else(|| invalid("native address plan final high-water absent"))?;
        retained.text(kind)?;
        plans.push(json!({
            "schema": "tos_rust_d1_search_address_plan_v1",
            "base_revision": expected_revision,
            "kind": plural,
            "high_water_before": initial,
            "high_water_after": final_high,
            "before": before,
            "after": after,
            "changed_ids": changed_ids,
            "source_closure_verified": false,
            "committed": false
        }));
    }
    Ok(plans)
}

fn selected_manifest(
    d1_tx: &Transaction<'_>,
    after_tx: &Transaction<'_>,
    after_source: &PreparedSourceInputs,
    after_descriptor: &Value,
    operation: &str,
    expected_revision: &str,
    limits: Limits,
    retained: &mut RetainedBytes,
    read_bytes: &mut D1ReadBytes,
) -> Result<(BTreeMap<(String, String), CapturedChange>, usize), String> {
    let mut changes = BTreeMap::new();
    if operation != "prepared-catchup" {
        let frames = after_descriptor
            .get("changes")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("prepared delta descriptor frames"))?;
        if frames.is_empty() || frames.len() > limits.prepared.max_changes {
            return Err(invalid("prepared delta frame count"));
        }
        for frame in frames {
            let fields = frame
                .as_array()
                .filter(|fields| fields.len() == 6)
                .ok_or_else(|| invalid("prepared delta frame shape"))?;
            let operation = fields[0]
                .as_str()
                .ok_or_else(|| invalid("prepared delta operation"))?;
            let kind = fields[1]
                .as_str()
                .ok_or_else(|| invalid("prepared delta kind"))?;
            let id = fields[2]
                .as_str()
                .ok_or_else(|| invalid("prepared delta id"))?;
            let after_digest = if fields[5].is_null() {
                None
            } else {
                Some(manifest_sha(&fields[5])?)
            };
            if !matches!(operation, "insert" | "update" | "delete")
                || (operation == "insert" && fields[3].as_str().is_some())
                || (operation == "delete" && after_digest.is_some())
                || (operation != "delete" && after_digest.is_none())
            {
                return Err(invalid("prepared delta frame operation/digest"));
            }
            add_change(
                &mut changes,
                kind,
                id,
                Some(operation),
                after_digest,
                limits.prepared.max_changes,
                retained,
            )?;
        }
        return Ok((changes, 0));
    }

    let mut manifest_rows_scanned = 0usize;
    for kind in ["node", "relation"] {
        let before_manifest = private_manifest(d1_tx, kind, limits, retained, &mut *read_bytes)?;
        let after_manifest = private_manifest(after_tx, kind, limits, retained, &mut *read_bytes)?;
        manifest_rows_scanned = manifest_rows_scanned
            .checked_add(before_manifest.len())
            .and_then(|count| count.checked_add(after_manifest.len()))
            .ok_or_else(|| invalid("prepared digest manifest row count overflow"))?;
        let before_ids = all_row_ids(d1_tx, kind, limits, retained, &mut *read_bytes)?;
        let after_ids = all_row_ids(after_tx, kind, limits, retained, &mut *read_bytes)?;
        if before_manifest.len() != before_ids.len()
            || before_manifest.keys().any(|id| !before_ids.contains(id))
            || after_manifest.len() != after_ids.len()
            || after_manifest.keys().any(|id| !after_ids.contains(id))
        {
            return Err(invalid("manifest identities do not cover prepared rows"));
        }
        for id in &before_ids {
            let raw = exact_prepared_item(d1_tx, kind, id, limits, &mut *read_bytes)?
                .ok_or_else(|| invalid("manifest row disappeared"))?;
            if before_manifest.get(id) != Some(&digest(raw.as_bytes())) {
                return Err(invalid("manifest row digest or read budget"));
            }
        }
        for id in &after_ids {
            let raw = exact_prepared_item(after_tx, kind, id, limits, &mut *read_bytes)?
                .ok_or_else(|| invalid("manifest row disappeared"))?;
            if after_manifest.get(id) != Some(&digest(raw.as_bytes())) {
                return Err(invalid("manifest row digest or read budget"));
            }
        }
        for id in before_ids.union(&after_ids) {
            let before = before_manifest.get(id);
            let after = after_manifest.get(id);
            if before != after {
                add_change(
                    &mut changes,
                    kind,
                    id,
                    None,
                    after.map(String::as_str),
                    limits.prepared.max_changes,
                    retained,
                )?;
            }
        }
    }

    // Catch-up may also move equal-digest rows within a complete search tie.
    // Those addresses are derived from the successor's total source order,
    // never from a caller-provided ID list.
    for (kind, plural) in [("node", "nodes"), ("relation", "relations")] {
        let mut statement = d1_tx
            .prepare("SELECT CASE WHEN typeof(id_lower)='text' AND length(CAST(id_lower AS BLOB))<=?2 THEN id_lower END,count(*),length(CAST(json_array(CASE WHEN typeof(id_lower)='text' AND length(CAST(id_lower AS BLOB))<=?2 THEN id_lower END,count(*)) AS BLOB)) FROM knowledge_search_documents WHERE kind=?1 GROUP BY id_lower HAVING count(*)>1 ORDER BY id_lower LIMIT ?3")
            .map_err(|error| error.to_string())?;
        let mut groups = statement
            .query(params![
                plural,
                limits.prepared.max_metadata_bytes as i64,
                limits.prepared.max_changes as i64 + 1
            ])
            .map_err(|error| error.to_string())?;
        let mut group_count = 0usize;
        while let Some(row) = groups.next().map_err(|error| error.to_string())? {
            if group_count >= limits.prepared.max_changes {
                return Err(invalid("catch-up search tie group budget"));
            }
            account_json_row(
                row,
                2,
                limits.prepared.max_row_bytes,
                read_bytes,
                "catch-up search tie group row byte budget",
            )?;
            let lower: Option<String> = row.get(0).map_err(|error| error.to_string())?;
            let lower = lower.ok_or_else(|| invalid("catch-up search tie key size"))?;
            let count: i64 = row.get(1).map_err(|error| error.to_string())?;
            retained.text(&lower)?;
            group_count += 1;
            if count <= 1 || count as usize > limits.prepared.max_changes {
                return Err(invalid("catch-up search tie member budget"));
            }
            let mut old_statement = d1_tx
                .prepare("SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=4096 THEN id END,length(CAST(json_array(CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=4096 THEN id END) AS BLOB)) FROM knowledge_search_documents WHERE kind=?1 AND id_lower=?2 ORDER BY position LIMIT ?3")
                .map_err(|error| error.to_string())?;
            let mut old_rows = old_statement
                .query(params![
                    plural,
                    lower,
                    limits.prepared.max_changes as i64 + 1
                ])
                .map_err(|error| error.to_string())?;
            let mut old_ids = Vec::new();
            read_bytes.charge(2)?;
            while let Some(row) = old_rows.next().map_err(|error| error.to_string())? {
                if old_ids.len() >= limits.prepared.max_changes {
                    return Err(invalid("catch-up search tie member budget"));
                }
                if !old_ids.is_empty() {
                    read_bytes.charge(1)?;
                }
                account_json_row(
                    row,
                    1,
                    limits.prepared.max_row_bytes,
                    read_bytes,
                    "catch-up search tie member byte budget",
                )?;
                let id: Option<String> = row.get(0).map_err(|error| error.to_string())?;
                let id = id.ok_or_else(|| invalid("catch-up search tie member identity size"))?;
                retained.text(&id)?;
                old_ids.push(id);
            }
            if old_ids.len() > limits.prepared.max_changes {
                return Err(invalid("catch-up search tie member budget"));
            }
            let mut ranked = Vec::new();
            for id in &old_ids {
                let order: Option<i64> = after_tx
                    .query_row(
                        "SELECT source_order FROM prepared_documents WHERE kind=?1 AND id=?2 LIMIT 2",
                        params![kind, id],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(|error| error.to_string())?;
                if let Some(order) = order {
                    ranked.push((order, retained.text_copy(id)?));
                }
            }
            ranked.sort();
            let successor = ranked.into_iter().map(|(_, id)| id).collect::<Vec<_>>();
            let retained_ids = old_ids
                .iter()
                .filter(|id| successor.contains(id))
                .map(|id| retained.text_copy(id))
                .collect::<Result<Vec<_>, _>>()?;
            if successor != retained_ids {
                for id in successor {
                    let raw = exact_prepared_item(after_tx, kind, &id, limits, &mut *read_bytes)?
                        .ok_or_else(|| invalid("catch-up reordered row absent"))?;
                    if !changes.contains_key(&(kind.to_owned(), id.clone())) {
                        let after_digest = digest(raw.as_bytes());
                        add_change(
                            &mut changes,
                            kind,
                            &id,
                            Some("update"),
                            Some(&after_digest),
                            limits.prepared.max_changes,
                            retained,
                        )?;
                    }
                }
            }
        }
    }
    let _ = (after_source, expected_revision);
    Ok((changes, manifest_rows_scanned))
}

fn run_prepared_transition(
    request: &Value,
    operation: &str,
    request_schema: &str,
    frame_budget: &mut Option<SnapshotFrameBudget>,
    limits: Limits,
    stdout: &mut dyn Write,
) -> Result<(), String> {
    let d1_path = input_path(string(request, "d1_database")?)?;
    let after_path = input_path(string(request, "after_prepared_database")?)?;
    let expected_revision = string(request, "expected_d1_revision")?;
    let before_prepared_path = if request["before_prepared_database"].is_null() {
        None
    } else {
        Some(input_path(string(request, "before_prepared_database")?)?)
    };
    let after_binding_value = foundation(
        &request["after_binding"],
        limits.prepared.max_metadata_bytes,
    )?;
    let before_binding_value = if request["before_binding"].is_null() {
        None
    } else {
        Some(foundation(
            &request["before_binding"],
            limits.prepared.max_metadata_bytes,
        )?)
    };
    let before_source_inputs = if operation == "prepared-catchup" {
        let raw = string(request, "before_source_inputs_json")?.as_bytes();
        Some(PreparedSourceInputs::parse(raw, limits.prepared).map_err(|error| error.to_string())?)
    } else {
        None
    };
    if operation == "prepared-catchup" {
        if before_prepared_path.is_some() || before_binding_value.is_some() {
            return Err(invalid("catch-up request carries a prepared predecessor"));
        }
    } else if operation == "prepared-delta" || operation == "source-navigation-delta" {
        if before_prepared_path.is_none()
            || before_binding_value.is_none()
            || !request["before_source_inputs_json"].is_null()
        {
            return Err(invalid("prepared transition predecessor fields"));
        }
    } else {
        return Err(invalid("private prepared transition operation"));
    }
    if !request["rights_root"].is_null() {
        return Err(invalid(
            "prepared transition does not take an external rights root",
        ));
    }

    let forward = output_path(string(request, "forward_sql")?)?;
    let rollback = if request["rollback_sql"].is_null() {
        None
    } else {
        Some(output_path(string(request, "rollback_sql")?)?)
    };
    let manifest = output_path(string(request, "manifest_json")?)?;
    validate_capture_outputs(&forward, rollback.as_deref(), &manifest)?;
    let vm_steps = limits.pair.max_work_bytes.min(100_000_000).max(100_000);
    let sqlite_value_bytes = limits
        .prepared
        .max_row_bytes
        .max(limits.prepared.max_metadata_bytes);
    let mut d1 = open_selected_snapshot(
        &d1_path,
        vm_steps,
        sqlite_value_bytes,
        request_schema,
        typed_snapshot::Role::D1,
        "d1_database",
        frame_budget,
    )?;
    let mut after_db = open_selected_snapshot(
        &after_path,
        vm_steps,
        sqlite_value_bytes,
        request_schema,
        typed_snapshot::Role::Prepared,
        "after_prepared_database",
        frame_budget,
    )?;
    let mut before_db = before_prepared_path
        .as_ref()
        .map(|path| {
            open_selected_snapshot(
                path,
                vm_steps,
                sqlite_value_bytes,
                request_schema,
                typed_snapshot::Role::Prepared,
                "before_prepared_database",
                frame_budget,
            )
        })
        .transpose()?;
    let d1_tx = d1
        .connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(|error| error.to_string())?;
    let after_tx = after_db
        .connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(|error| error.to_string())?;
    // Split the optional holder once so its connection transaction and file
    // identity can remain borrowed together through final SQL emission.
    let (before_identity, before_tx) = match before_db.as_mut() {
        Some(db) => (
            Some(&db.identity),
            Some(
                db.connection
                    .transaction_with_behavior(TransactionBehavior::Deferred)
                    .map_err(|error| error.to_string())?,
            ),
        ),
        None => (None, None),
    };
    for tx in std::iter::once(&d1_tx)
        .chain(std::iter::once(&after_tx))
        .chain(before_tx.iter())
    {
        let _: i64 = tx
            .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))
            .map_err(|error| error.to_string())?;
    }
    let mut d1_read_bytes = D1ReadBytes::new(limits);
    let after_source =
        prepared_source_inputs_held(&after_tx, &after_binding_value, limits, &mut d1_read_bytes)
            .map_err(|error| error.to_string())?;
    let after_data_revision = foundation_string(&after_binding_value, "data_revision")?;
    let (after_persisted_revision, _, _) =
        parse_meta_accounted(&after_tx, "data_revision", limits, &mut d1_read_bytes)?;
    if after_persisted_revision != json!({"sha256": after_data_revision}) {
        return Err(invalid(
            "prepared successor data revision differs from binding",
        ));
    }
    let after_descriptor =
        prepared_descriptor(&after_tx, limits, &mut d1_read_bytes, after_data_revision)?;
    let (after_reader_top, _, _) = parse_meta_accounted(
        &after_tx,
        "knowledge_reader_top",
        limits,
        &mut d1_read_bytes,
    )?;
    // Prepared state owns its header inside the digest-checked descriptor;
    // knowledge_top is a published D1 metadata key, not a prepared carrier.
    // The descriptor read above already charges the complete header bytes.
    let after_header = after_descriptor
        .get("header")
        .cloned()
        .ok_or_else(|| invalid("prepared successor descriptor header"))?;
    let after_header_raw = compact(&after_header, limits.prepared.max_metadata_bytes)?;
    let (_, _, after_catalog_raw) =
        parse_meta_accounted(&after_tx, "knowledge_catalog", limits, &mut d1_read_bytes)?;
    let (after_lens, _, after_lens_raw) =
        parse_meta_accounted(&after_tx, "knowledge_lens_top", limits, &mut d1_read_bytes)?;
    // The exact held source binding is validated against prepared_source_state.
    // The rendered knowledge_catalog row is a separate owner output whose
    // bytes are checked against its persisted reader digest below.
    if after_reader_top
        .get("read_model_schema")
        .and_then(Value::as_str)
        != Some(PREPARED_SCHEMA)
        || after_reader_top
            .get("source_revision")
            .and_then(Value::as_str)
            != Some(after_source.source_revision())
        || after_header.get("source_revision").and_then(Value::as_str)
            != Some(after_source.source_revision())
        || after_reader_top
            .get("catalog_sha256")
            .and_then(Value::as_str)
            != Some(digest(after_catalog_raw.as_bytes()).as_str())
        || after_reader_top.get("lens_sha256").and_then(Value::as_str)
            != Some(digest(after_lens_raw.as_bytes()).as_str())
    {
        return Err(invalid(
            "prepared successor reader metadata differs from held source",
        ));
    }
    let before_source = if let Some(source) = before_source_inputs {
        source
    } else {
        let before_binding = before_binding_value
            .as_ref()
            .ok_or_else(|| invalid("prepared predecessor binding"))?;
        prepared_source_inputs_held(
            before_tx
                .as_ref()
                .ok_or_else(|| invalid("prepared predecessor snapshot"))?,
            before_binding,
            limits,
            &mut d1_read_bytes,
        )
        .map_err(|error| error.to_string())?
    };
    let before_source_value = source_inputs_value(&before_source)?;
    let after_source_value = source_inputs_value(&after_source)?;
    source_scope_compatible(&before_source_value, &after_source_value)?;
    // Ordinary Prepared pairs may omit navigation entirely. Source-scope
    // compatibility above still rejects adding or removing a selected root.
    let before_nav_sha = before_source
        .roots()
        .get("source-navigation")
        .map(|root| root.snapshot_sha256.clone());
    let after_nav_sha = after_source
        .roots()
        .get("source-navigation")
        .map(|root| root.snapshot_sha256.clone());
    if operation == "source-navigation-delta"
        && (before_nav_sha.is_none() || after_nav_sha.is_none())
    {
        return Err(invalid(
            "source-navigation delta requires selected navigation roots",
        ));
    }
    // Navigation product rights are absent with the product itself. A
    // separately selected rights root remains in the complete source-input
    // inventory and must still be unchanged under source-scope checks.
    let before_rights_sha = before_nav_sha.as_ref().map(|navigation| {
        before_source
            .roots()
            .get("source-navigation-rights")
            .map(|root| root.snapshot_sha256.clone())
            .unwrap_or_else(|| navigation.clone())
    });
    let after_rights_sha = after_nav_sha.as_ref().map(|navigation| {
        after_source
            .roots()
            .get("source-navigation-rights")
            .map(|root| root.snapshot_sha256.clone())
            .unwrap_or_else(|| navigation.clone())
    });

    let (base_top, _, base_top_raw) =
        parse_meta_accounted(&d1_tx, "knowledge_reader_top", limits, &mut d1_read_bytes)?;
    let (base_header, _, _) =
        parse_meta_accounted(&d1_tx, "knowledge_top", limits, &mut d1_read_bytes)?;
    let (base_catalog, _, base_catalog_raw) =
        parse_meta_accounted(&d1_tx, "knowledge_catalog", limits, &mut d1_read_bytes)?;
    let (base_lens, _, base_lens_raw) =
        parse_meta_accounted(&d1_tx, "knowledge_lens_top", limits, &mut d1_read_bytes)?;
    let expected_data_revision: Value = serde_json::from_str(
        &parse_meta_accounted(&d1_tx, "data_revision", limits, &mut d1_read_bytes)?.2,
    )
    .map_err(|_| invalid("D1 data revision metadata"))?;
    if expected_data_revision != json!({"sha256":expected_revision})
        || base_top.get("read_model_schema").and_then(Value::as_str) != Some(D1_SCHEMA)
        || base_top.get("data_revision").and_then(Value::as_str) != Some(expected_revision)
        || base_top.get("source_revision").and_then(Value::as_str)
            != Some(before_source.source_revision())
        || base_header.get("source_revision").and_then(Value::as_str)
            != Some(before_source.source_revision())
        || base_top.get("catalog_sha256").and_then(Value::as_str)
            != Some(digest(base_catalog_raw.as_bytes()).as_str())
        || base_top.get("lens_sha256").and_then(Value::as_str)
            != Some(digest(base_lens_raw.as_bytes()).as_str())
    {
        return Err(invalid("held D1 predecessor source or revision differs"));
    }
    let descriptor_header = after_descriptor
        .get("header")
        .ok_or_else(|| invalid("prepared successor descriptor header"))?;
    let before_descriptor = before_tx
        .as_ref()
        .map(|tx| {
            let binding = before_binding_value
                .as_ref()
                .ok_or_else(|| invalid("prepared predecessor binding"))?;
            prepared_descriptor(
                tx,
                limits,
                &mut d1_read_bytes,
                foundation_string(binding, "data_revision")?,
            )
        })
        .transpose()?;
    let before_data_revision = before_tx
        .as_ref()
        .map(|tx| {
            parse_meta_accounted(tx, "data_revision", limits, &mut d1_read_bytes)
                .map(|(value, _, _)| value)
        })
        .transpose()?;
    let before_header = if let Some(descriptor) = &before_descriptor {
        descriptor
            .get("header")
            .cloned()
            .ok_or_else(|| invalid("prepared predecessor descriptor header"))?
    } else {
        base_header.clone()
    };
    let mut old_profile = before_header.clone();
    let mut new_profile = descriptor_header.clone();
    for value in [&mut old_profile, &mut new_profile] {
        if let Some(object) = value.as_object_mut() {
            object.remove("source_revision");
            object.remove("counts");
        }
    }
    if old_profile != new_profile {
        return Err(invalid(
            "prepared header profile changed; explicit migration required",
        ));
    }
    if operation != "prepared-catchup" {
        let before_binding = request
            .get("before_binding")
            .ok_or_else(|| invalid("prepared predecessor binding"))?;
        let expected_before_revision = json!({
            "sha256": string(before_binding, "data_revision")?
        });
        if before_data_revision.as_ref() != Some(&expected_before_revision) {
            return Err(invalid(
                "prepared predecessor data revision differs from binding",
            ));
        }
        if after_descriptor.get("mode").and_then(Value::as_str) != Some("delta-history")
            || after_descriptor
                .get("parent_data_revision")
                .and_then(Value::as_str)
                != before_binding.get("data_revision").and_then(Value::as_str)
        {
            return Err(invalid(
                "one exact prepared parent/successor transition required",
            ));
        }
        if base_catalog
            != parse_meta_accounted(
                before_tx
                    .as_ref()
                    .ok_or_else(|| invalid("prepared predecessor snapshot"))?,
                "knowledge_catalog",
                limits,
                &mut d1_read_bytes,
            )?
            .0
            || base_lens
                != parse_meta_accounted(
                    before_tx
                        .as_ref()
                        .ok_or_else(|| invalid("prepared predecessor snapshot"))?,
                    "knowledge_lens_top",
                    limits,
                    &mut d1_read_bytes,
                )?
                .0
        {
            return Err(invalid("D1 prepared predecessor catalog/lens differs"));
        }
    }
    let before_top_local = if let Some(tx) = before_tx.as_ref() {
        parse_meta_accounted(tx, "knowledge_reader_top", limits, &mut d1_read_bytes)?.0
    } else {
        base_top.clone()
    };
    let mut expected_base_top = base_top.clone();
    let mut local_base_top = before_top_local;
    for value in [&mut expected_base_top, &mut local_base_top] {
        if let Some(object) = value.as_object_mut() {
            object.remove("read_model_schema");
            object.remove("data_revision");
        }
    }
    if expected_base_top != local_base_top
        || base_catalog
            != parse_meta_accounted(&d1_tx, "knowledge_catalog", limits, &mut d1_read_bytes)?.0
        || base_lens
            != parse_meta_accounted(&d1_tx, "knowledge_lens_top", limits, &mut d1_read_bytes)?.0
            && operation != "prepared-catchup"
    {
        return Err(invalid(
            "D1 and prepared predecessor reader profile differs",
        ));
    }

    let installed_auxiliary =
        auxiliary_stores_accounted(&d1_tx, &base_top_raw, limits, &mut d1_read_bytes)?;
    let mut retained = RetainedBytes::new(limits);
    let (mut changes, manifest_rows_scanned) = selected_manifest(
        &d1_tx,
        &after_tx,
        &after_source,
        &after_descriptor,
        operation,
        expected_revision,
        limits,
        &mut retained,
        &mut d1_read_bytes,
    )?;
    let before_database = before_tx.as_ref().unwrap_or(&d1_tx);
    let mut old_rows = BTreeMap::new();
    let mut new_rows = BTreeMap::new();
    let mut positions_before = BTreeMap::<(String, String), i64>::new();
    let mut positions_after = BTreeMap::<(String, String), i64>::new();

    let mut groups = BTreeSet::<(String, String)>::new();
    let mut source_lower = BTreeMap::<(String, String), (Option<String>, Option<String>)>::new();
    for ((kind, id), change) in &changes {
        let old = exact_prepared_item(before_database, kind, id, limits, &mut d1_read_bytes)?;
        let new = exact_prepared_item(&after_tx, kind, id, limits, &mut d1_read_bytes)?;
        let old_digest = old.as_ref().map(|raw| digest(raw.as_bytes()));
        let new_digest = new.as_ref().map(|raw| digest(raw.as_bytes()));
        match (change.operation.as_deref(), old.as_ref(), new.as_ref()) {
            (Some("insert"), None, Some(_))
                if change.after_digest.as_deref() == new_digest.as_deref() => {}
            (Some("update"), Some(_), Some(_))
                if change.after_digest.as_deref() == new_digest.as_deref() => {}
            (Some("delete"), Some(_), None) if change.after_digest.is_none() => {}
            (None, Some(_), Some(_)) if old_digest != new_digest => {}
            (None, None, Some(_)) | (None, Some(_), None) => {}
            _ => {
                return Err(invalid(
                    "prepared change frame differs from exact row sides",
                ));
            }
        }
        let old_lower = old
            .as_deref()
            .map(|raw| {
                let value: Value =
                    serde_json::from_str(raw).map_err(|_| invalid("prepared node JSON"))?;
                lower_id(
                    value
                        .get("id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| invalid("prepared row id"))?,
                )
            })
            .transpose()?;
        let new_lower = new
            .as_deref()
            .map(|raw| {
                let value: Value =
                    serde_json::from_str(raw).map_err(|_| invalid("prepared node JSON"))?;
                lower_id(
                    value
                        .get("id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| invalid("prepared row id"))?,
                )
            })
            .transpose()?;
        if let Some(lower) = &old_lower {
            retained.key(kind, lower)?;
            groups.insert((kind.clone(), lower.clone()));
        }
        if let Some(lower) = &new_lower {
            retained.key(kind, lower)?;
            groups.insert((kind.clone(), lower.clone()));
        }
        retained.key(kind, id)?;
        if let Some(lower) = &old_lower {
            retained.text(lower)?;
        }
        if let Some(lower) = &new_lower {
            retained.text(lower)?;
        }
        source_lower.insert((kind.clone(), id.clone()), (old_lower, new_lower));
        retained.source_row(kind, id, old.as_deref())?;
        retained.source_row(kind, id, new.as_deref())?;
        old_rows.insert((kind.clone(), id.clone()), old);
        new_rows.insert((kind.clone(), id.clone()), new);
    }

    if !changes.is_empty() {
        read_search_indexes(&d1_tx)?;
    }
    let mut initial_high_water_by_kind = BTreeMap::<String, i64>::new();
    let mut high_water_by_kind = BTreeMap::<String, i64>::new();
    for (kind, lower) in groups {
        let plural = if kind == "node" { "nodes" } else { "relations" };
        if !initial_high_water_by_kind.contains_key(&kind) {
            let high: Option<i64> = d1_tx
                .query_row(
                    "SELECT max(position) FROM knowledge_search_documents WHERE kind=?1",
                    [plural],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            let high = high.unwrap_or(-1);
            if high < -1 || high > 9_007_199_254_740_991_i64 {
                return Err(invalid("D1 search address high-water"));
            }
            retained.text(&kind)?;
            initial_high_water_by_kind.insert(kind.clone(), high);
            high_water_by_kind.insert(kind.clone(), high);
        }
        let mut statement = d1_tx
            .prepare("SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=4096 THEN id END,position,length(CAST(json_quote(CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=4096 THEN id END) AS BLOB))+1+length(CAST(position AS TEXT)) FROM knowledge_search_documents WHERE kind=?1 AND id_lower=?2 ORDER BY position LIMIT ?3")
            .map_err(|error| error.to_string())?;
        let mut member_rows = statement
            .query(params![
                plural,
                lower,
                limits.prepared.max_changes as i64 + 1
            ])
            .map_err(|error| error.to_string())?;
        let mut old_members = Vec::new();
        // The predecessor member map is compact JSON. Charge its braces, then
        // each quoted key/value pair and comma before owning that row's ID.
        d1_read_bytes.charge(2)?;
        while let Some(row) = member_rows.next().map_err(|error| error.to_string())? {
            if old_members.len() >= limits.prepared.max_changes {
                return Err(invalid("complete predecessor search tie member budget"));
            }
            let member_bytes: i64 = row.get(2).map_err(|error| error.to_string())?;
            let member_bytes = usize::try_from(member_bytes)
                .map_err(|_| invalid("predecessor search tie member bytes"))?;
            let member_bytes = member_bytes
                .checked_add(if old_members.is_empty() { 0 } else { 1 })
                .ok_or_else(|| invalid("predecessor search tie member bytes"))?;
            d1_read_bytes.charge(member_bytes)?;
            let id: Option<String> = row.get(0).map_err(|error| error.to_string())?;
            let id = id.ok_or_else(|| invalid("predecessor search tie identity size"))?;
            let position: i64 = row.get(1).map_err(|error| error.to_string())?;
            retained.text(&id)?;
            old_members.push((id, position));
        }
        let mut successor_ids = BTreeSet::new();
        for (id, _) in &old_members {
            let id = retained.text_copy(id)?;
            successor_ids.insert(id);
        }
        for ((selected_kind, selected_id), _) in &changes {
            if selected_kind != &kind {
                continue;
            }
            let (old_lower, new_lower) = source_lower
                .get(&(selected_kind.clone(), selected_id.clone()))
                .ok_or_else(|| invalid("search tie source identity"))?;
            if old_lower.as_deref() == Some(&lower) {
                successor_ids.remove(selected_id);
            }
            if new_lower.as_deref() == Some(&lower) {
                let selected_id = retained.text_copy(selected_id)?;
                successor_ids.insert(selected_id);
                if successor_ids.len() > limits.prepared.max_changes {
                    return Err(invalid("successor search tie member budget"));
                }
            }
        }
        let mut successor_order = Vec::new();
        for id in successor_ids {
            let order: Option<i64> = after_tx
                .query_row(
                    "SELECT source_order FROM prepared_documents WHERE kind=?1 AND id=?2 LIMIT 2",
                    params![kind, id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|error| error.to_string())?;
            let order = order.ok_or_else(|| invalid("successor tie row has no source order"))?;
            let _ = exact_prepared_item(&after_tx, &kind, &id, limits, &mut d1_read_bytes)?
                .ok_or_else(|| invalid("successor tie row has no normalized row"))?;
            successor_order.push((order, id));
        }
        successor_order.sort();
        let successor = successor_order
            .into_iter()
            .map(|(_, id)| id)
            .collect::<Vec<_>>();
        let retained_ids = old_members
            .iter()
            .filter(|(id, _)| successor.contains(id))
            .map(|(id, _)| retained.text_copy(id))
            .collect::<Result<Vec<_>, _>>()?;
        for (id, position) in &old_members {
            retained.key(&kind, id)?;
            positions_before.insert((kind.clone(), id.clone()), *position);
        }
        if successor == retained_ids {
            for (id, position) in &old_members {
                if successor.contains(id) {
                    retained.key(&kind, id)?;
                    positions_after.insert((kind.clone(), id.clone()), *position);
                }
            }
        } else {
            let high = *high_water_by_kind
                .get(&kind)
                .ok_or_else(|| invalid("D1 search address high-water absent"))?;
            let mut position = high;
            if position < -1 || position > 9_007_199_254_740_991_i64 {
                return Err(invalid("D1 search address high-water"));
            }
            for id in successor {
                position = position
                    .checked_add(1)
                    .ok_or_else(|| invalid("D1 search address overflow"))?;
                if position < 0 || position > 9_007_199_254_740_991_i64 {
                    return Err(invalid("D1 search address space exhausted"));
                }
                retained.text(&kind)?;
                positions_after.insert((kind.clone(), id), position);
            }
            retained.text(&kind)?;
            high_water_by_kind.insert(kind.clone(), position);
        }
    }
    let mut affected = BTreeSet::new();
    for (kind, id) in positions_before
        .keys()
        .chain(positions_after.keys())
        .chain(changes.keys())
    {
        retained.key(kind, id)?;
        affected.insert((kind.clone(), id.clone()));
    }
    let mut before_capture = BTreeMap::new();
    let mut after_capture = BTreeMap::new();
    let mut gram_deltas = BTreeMap::<(String, i64, String), i64>::new();
    let mut posting_count = 0usize;
    let mut projected_count = 0u64;
    let repo_root = repository_root()?;
    for (kind, id) in affected {
        let old = match old_rows.get(&(kind.clone(), id.clone())) {
            Some(Some(raw)) => Some(Cow::Borrowed(raw.as_str())),
            Some(None) => None,
            None => {
                let raw =
                    exact_prepared_item(before_database, &kind, &id, limits, &mut d1_read_bytes)?;
                raw.map(Cow::Owned)
            }
        };
        let new = match new_rows.get(&(kind.clone(), id.clone())) {
            Some(Some(raw)) => Some(Cow::Borrowed(raw.as_str())),
            Some(None) => None,
            None => exact_prepared_item(&after_tx, &kind, &id, limits, &mut d1_read_bytes)?
                .map(Cow::Owned),
        };
        let old_position = positions_before.get(&(kind.clone(), id.clone())).copied();
        let new_position = positions_after.get(&(kind.clone(), id.clone())).copied();
        let mut projected_budget = RetainedBytes::new(limits);
        let mut old_projected = BTreeMap::new();
        let mut new_projected = BTreeMap::new();
        if let (Some(raw), Some(position)) = (&old, old_position) {
            old_projected = projected_map(
                project_private_knowledge_row(
                    &kind,
                    position,
                    raw,
                    &repo_root,
                    search_limits(limits)?,
                )
                .map_err(|error| error.to_string())?,
            )?;
            for ((_, key), row) in &old_projected {
                projected_budget.row(key, row)?;
            }
            let aux = expected_aux_rows(&kind, &id, raw, limits, &installed_auxiliary)?;
            merge_projected(&mut old_projected, aux, &mut projected_budget)?;
            verify_and_add_knowledge_side(
                &d1_tx,
                &old_projected,
                &kind,
                &id,
                true,
                limits,
                &installed_auxiliary,
                &mut before_capture,
                &mut retained,
                &mut d1_read_bytes,
            )?;
        } else if old.is_some() {
            return Err(invalid("selected predecessor search address absent"));
        } else {
            let empty = expected_aux_rows(&kind, &id, "{}", limits, &[])?;
            if !empty.is_empty() {
                return Err(invalid("unexpected D1 auxiliary rows"));
            }
            verify_and_add_knowledge_side(
                &d1_tx,
                &BTreeMap::new(),
                &kind,
                &id,
                false,
                limits,
                &installed_auxiliary,
                &mut before_capture,
                &mut retained,
                &mut d1_read_bytes,
            )?;
        }
        if let (Some(raw), Some(position)) = (&new, new_position) {
            new_projected = projected_map(
                project_private_knowledge_row(
                    &kind,
                    position,
                    raw,
                    &repo_root,
                    search_limits(limits)?,
                )
                .map_err(|error| error.to_string())?,
            )?;
            for ((_, key), row) in &new_projected {
                projected_budget.row(key, row)?;
            }
            let aux = expected_aux_rows(&kind, &id, raw, limits, &installed_auxiliary)?;
            merge_projected(&mut new_projected, aux, &mut projected_budget)?;
        } else if new.is_some() {
            return Err(invalid("selected successor search address absent"));
        }
        for ((table, _), row) in &old_projected {
            if *table == D1Table::KnowledgeSearchGrams
                && row.len() == 4
                && row[0]
                    == D1Cell::Text(if kind == "node" {
                        "nodes".into()
                    } else {
                        "relations".into()
                    })
                && row[1] == D1Cell::Integer(3)
            {
                let gram = match &row[2] {
                    D1Cell::Text(value) => value.as_str(),
                    _ => return Err(invalid("search gram type")),
                };
                posting_count = posting_count
                    .checked_add(1)
                    .filter(|count| *count <= limits.max_postings)
                    .ok_or_else(|| invalid("D1 changed posting budget exceeded"))?;
                let plural = if kind == "node" { "nodes" } else { "relations" };
                retained.key(plural, gram)?;
                *gram_deltas
                    .entry((plural.to_owned(), 3, gram.to_owned()))
                    .or_default() -= 1;
            }
        }
        for ((table, _), row) in &new_projected {
            if *table == D1Table::KnowledgeSearchGrams
                && row.len() == 4
                && row[0]
                    == D1Cell::Text(if kind == "node" {
                        "nodes".into()
                    } else {
                        "relations".into()
                    })
                && row[1] == D1Cell::Integer(3)
            {
                let gram = match &row[2] {
                    D1Cell::Text(value) => value.as_str(),
                    _ => return Err(invalid("search gram type")),
                };
                posting_count = posting_count
                    .checked_add(1)
                    .filter(|count| *count <= limits.max_postings)
                    .ok_or_else(|| invalid("D1 changed posting budget exceeded"))?;
                let plural = if kind == "node" { "nodes" } else { "relations" };
                retained.key(plural, gram)?;
                *gram_deltas
                    .entry((plural.to_owned(), 3, gram.to_owned()))
                    .or_default() += 1;
            }
        }
        projected_count =
            projected_count.saturating_add((old_projected.len() + new_projected.len()) as u64);
        if projected_count > limits.prepared.max_mutations {
            return Err(invalid("private projection row budget"));
        }
        merge_projected(&mut after_capture, new_projected, &mut retained)?;
    }
    for ((kind, n, gram), adjustment) in gram_deltas {
        let key = vec![
            D1Cell::Text(kind.clone()),
            D1Cell::Integer(n),
            D1Cell::Text(gram.clone()),
        ];
        let prior = selected_tuple_accounted(
            &d1_tx,
            D1Table::KnowledgeSearchGramStats,
            &key,
            limits,
            &mut d1_read_bytes,
        )?;
        let count = match prior.as_ref() {
            None => 0,
            Some(row) => match row.get(3) {
                Some(D1Cell::Integer(count)) => *count,
                _ => return Err(invalid("D1 search posting count type")),
            },
        };
        let observed: i64 = d1_tx
            .query_row(
                "SELECT COUNT(*) FROM (SELECT 1 FROM knowledge_search_grams WHERE kind=?1 AND n=?2 AND gram=?3 LIMIT ?4)",
                params![kind, n, gram, limits.max_postings as i64 + 1],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if observed < 0 || observed as usize > limits.max_postings {
            return Err(invalid("D1 selected posting count budget"));
        }
        if count != observed {
            return Err(invalid("D1 search posting count metadata differs"));
        }
        if adjustment == 0 {
            continue;
        }
        let successor = count
            .checked_add(adjustment)
            .ok_or_else(|| invalid("D1 search posting count overflow"))?;
        if count < 0 || successor < 0 {
            return Err(invalid("D1 search posting count underflow"));
        }
        if let Some(row) = prior {
            let key = (
                D1Table::KnowledgeSearchGramStats,
                row_key(D1Table::KnowledgeSearchGramStats, &row)?,
            );
            insert_capture_row(&mut before_capture, key, row, &mut retained)?;
        }
        if successor > 0 {
            let row = vec![
                D1Cell::Text(kind),
                D1Cell::Integer(n),
                D1Cell::Text(gram),
                D1Cell::Integer(successor),
            ];
            let key = row_key(D1Table::KnowledgeSearchGramStats, &row)?;
            insert_capture_row(
                &mut after_capture,
                (D1Table::KnowledgeSearchGramStats, key),
                row,
                &mut retained,
            )?;
        }
    }

    let nav_before = before_source
        .roots()
        .get("source-navigation")
        .map(parsed_root)
        .transpose()?;
    let nav_after = after_source
        .roots()
        .get("source-navigation")
        .map(parsed_root)
        .transpose()?;
    let mut nav_top_update = None;
    let mut nav_product = json!({"state":"unchanged","changed_rows":0});
    let mut nav_projection_usage = D1ProjectionAccounting::default();
    if let (Some(nav_before), Some(nav_after)) = (&nav_before, &nav_after)
        && nav_before.snapshot_sha256() != nav_after.snapshot_sha256()
    {
        let (nav_rows, next_top, nav_changed_rows, usage) = navigation_delta_transitions(
            &d1_tx,
            nav_before,
            nav_after,
            limits,
            &mut d1_read_bytes,
        )?;
        merge_projected(&mut before_capture, nav_rows.0, &mut retained)?;
        merge_projected(&mut after_capture, nav_rows.1, &mut retained)?;
        nav_product = json!({
            "state": if next_top.is_some() { "maintained" } else { "unavailable" },
            "changed_rows": nav_changed_rows
        });
        nav_top_update = next_top;
        nav_projection_usage = usage;
    }

    // Compute the single producer-owned lineage identity, then reuse the
    // exact predecessor member order when updating the reader top.
    let before_binding_raw = before_binding_value
        .as_ref()
        .map(|value| compact_foundation(value, limits.prepared.max_metadata_bytes))
        .transpose()?
        .unwrap_or_default();
    let after_binding_raw =
        compact_foundation(&after_binding_value, limits.prepared.max_metadata_bytes)?;
    let mode = if operation == "prepared-catchup" {
        D1PrivatePreparedMode::CatchUp
    } else {
        D1PrivatePreparedMode::Pair
    };
    let before_navigation_sha = before_nav_sha.clone();
    let after_navigation_sha = after_nav_sha.clone();
    let mut spec = D1PrivatePreparedInput {
        predecessor_mode: mode,
        base_d1_revision: expected_revision.to_owned(),
        before_source_revision: before_source.source_revision().to_owned(),
        after_source_revision: after_source.source_revision().to_owned(),
        before_prepared_binding: before_binding_value.as_ref().map(|_| before_binding_raw),
        after_prepared_binding: Some(after_binding_raw),
        before_source_inputs_sha256: Some(before_source.digest().to_owned()),
        after_source_inputs_sha256: Some(after_source.digest().to_owned()),
        before_navigation_sha256: before_navigation_sha,
        after_navigation_sha256: after_navigation_sha,
        before_rights_sha256: before_rights_sha,
        after_rights_sha256: after_rights_sha,
        implementation_sha256: implementation_digest(),
        bootstrap_implementation_sha256: None,
        migration_implementation_sha256: None,
        auxiliary_stores: installed_auxiliary,
        before_reader_top: base_top_raw.clone(),
        after_reader_top: String::new(),
        limits: limits.pair,
    };
    let exact_revision = private_prepared_target_revision(&spec)
        .map_err(|_| invalid("private D1 lineage inputs"))?;
    let next_top_raw = reader_top_raw_from_predecessor(
        &base_top_raw,
        after_source.source_revision(),
        &exact_revision,
        &digest(after_catalog_raw.as_bytes()),
        &digest(after_lens_raw.as_bytes()),
        limits,
    )?;
    spec.after_reader_top = next_top_raw.clone();
    inspect_private_prepared_source_inputs(
        &spec,
        Some(before_source.raw()),
        Some(after_source.raw()),
    )
    .map_err(|error| format!("offline prepared source pair: {error:?}"))?;
    let (prior_top_value, _, _) =
        parse_meta_accounted(&d1_tx, "knowledge_reader_top", limits, &mut d1_read_bytes)?;
    if prior_top_value != base_top {
        return Err(invalid("D1 reader top changed inside held snapshot"));
    }
    let mut metadata_after = vec![
        ("knowledge_top", after_header_raw),
        (
            "knowledge_exploration_top",
            compact_ordered_metadata(
                vec![
                    (
                        "source_revision",
                        Value::String(after_source.source_revision().to_owned()),
                    ),
                    (
                        "authority_boundary",
                        descriptor_header["authority_boundary"].clone(),
                    ),
                ],
                limits.prepared.max_metadata_bytes,
            )?,
        ),
        (
            "knowledge_search_top",
            compact_ordered_metadata(
                vec![
                    ("schema", json!("tos_knowledge_search_read_model_v3")),
                    (
                        "source_revision",
                        Value::String(after_source.source_revision().to_owned()),
                    ),
                    ("ngram_size", json!(3)),
                    (
                        "matching_counts",
                        json!("unknown-until-indexed-page-exhaustion"),
                    ),
                ],
                limits.prepared.max_metadata_bytes,
            )?,
        ),
        ("knowledge_catalog", after_catalog_raw),
        ("knowledge_lens_top", after_lens_raw),
    ];
    if let Some(nav_raw) = nav_top_update.as_ref() {
        let (prior_digest, _, prior_digest_raw) = parse_meta_accounted(
            &d1_tx,
            "source_navigation_header_digest",
            limits,
            &mut d1_read_bytes,
        )?;
        let (_, nav_chunks, prior_nav_raw) =
            parse_meta_accounted(&d1_tx, "source_navigation_top", limits, &mut d1_read_bytes)?;
        if prior_digest != json!({"sha256":digest(prior_nav_raw.as_bytes())}) {
            return Err(invalid("source-navigation header digest differs"));
        }
        if nav_chunks.is_empty() || prior_digest_raw.is_empty() {
            return Err(invalid("source-navigation predecessor metadata absent"));
        }
        metadata_after.push(("source_navigation_top", nav_raw.clone()));
        metadata_after.push((
            "source_navigation_header_digest",
            compact(
                &json!({"sha256":digest(nav_raw.as_bytes())}),
                limits.prepared.max_metadata_bytes,
            )?,
        ));
    }
    for (key, raw) in metadata_after {
        let (_, old_rows, _) = parse_meta_accounted(&d1_tx, key, limits, &mut d1_read_bytes)?;
        capture_transition_rows(
            &mut before_capture,
            &mut after_capture,
            meta_raw_transitions(key, &old_rows, &raw, limits)?,
            &mut retained,
        )?;
    }
    let address_plans = native_address_plans(
        expected_revision,
        &positions_before,
        &positions_after,
        &initial_high_water_by_kind,
        &high_water_by_kind,
        &mut retained,
    )?;
    let transitions = row_transitions(before_capture, after_capture, limits, &mut retained)?;
    d1.identity.verify_selected_file_identity()?;
    after_db.identity.verify_selected_file_identity()?;
    if let Some(identity) = before_identity {
        identity.verify_selected_file_identity()?;
    }
    let mut selected_snapshots = vec![("d1_database", &d1.identity)];
    if let Some(identity) = before_identity {
        selected_snapshots.push(("before_prepared_database", identity));
    }
    selected_snapshots.push(("after_prepared_database", &after_db.identity));
    let snapshot_transport = snapshot_transport_value(request_schema, &selected_snapshots)?;
    let receipt =
        emit_private_prepared_capture(&spec, transitions, &forward, rollback.as_deref(), &manifest)
            .map_err(|error| format!("offline private capture: {error:?}"))?;
    let sql_bytes = receipt
        .forward_bytes
        .checked_add(receipt.rollback_bytes)
        .ok_or_else(|| invalid("private SQL byte count overflow"))?;
    let receipt_schema = match operation {
        "prepared-delta" => "tos_edge_native_prepared_delta_receipt_v1",
        "prepared-catchup" => "tos_edge_native_prepared_catchup_receipt_v1",
        "source-navigation-delta" => "tos_edge_native_source_navigation_delta_receipt_v1",
        _ => return Err(invalid("private transition receipt operation")),
    };
    let lineage_schema = match spec.predecessor_mode {
        D1PrivatePreparedMode::Pair => "tos_prepared_source_d1_delta_v2",
        D1PrivatePreparedMode::CatchUp => "tos_prepared_source_d1_manifest_reconciliation_v1",
        D1PrivatePreparedMode::Bootstrap => "tos_native_navigation_d1_bootstrap_v1",
        D1PrivatePreparedMode::Integrity { .. } => "tos_native_navigation_integrity_migration_v1",
    };
    let before_prepared_pairing_verified = operation != "prepared-catchup";
    let maintained_auxiliary_stores = spec
        .auxiliary_stores
        .iter()
        .map(|store| store.identity().0)
        .collect::<Vec<_>>();
    let forward_output = json!({
        "available": true,
        "sha256": receipt.forward_sha256,
        "bytes": receipt.forward_bytes,
        "base_revision": receipt.base_d1_revision,
        "target_revision": receipt.target_d1_revision,
        "published": true,
        "publication": "private-capture-manifest-last"
    });
    let rollback_output = receipt.rollback_sha256.as_ref().map(|sha256| {
        json!({
            "available": true,
            "sha256": sha256,
            "bytes": receipt.rollback_bytes,
            "base_revision": receipt.target_d1_revision,
            "target_revision": receipt.base_d1_revision,
            "published": true,
            "publication": "private-capture-manifest-last"
        })
    });
    let projection_usage = capture_json_object([
        ("opened_parts", json!(nav_projection_usage.opened_parts)),
        ("stored_bytes", json!(nav_projection_usage.stored_bytes)),
        ("decoded_bytes", json!(nav_projection_usage.decoded_bytes)),
        ("keys", json!(nav_projection_usage.keys)),
        ("changes", json!(nav_projection_usage.changes)),
        ("output_bytes", json!(nav_projection_usage.output_bytes)),
    ]);
    let mut receipt_value = capture_json_object([
        ("schema", json!(receipt_schema)),
        ("operation", json!(operation)),
        ("lineage_schema", json!(lineage_schema)),
        ("base_d1_revision", json!(receipt.base_d1_revision)),
        ("target_d1_revision", json!(receipt.target_d1_revision)),
        ("before_source_revision", json!(spec.before_source_revision)),
        ("after_source_revision", json!(spec.after_source_revision)),
        ("source_revision", json!(after_source.source_revision())),
        ("before_prepared_binding", json!(request["before_binding"])),
        ("after_prepared_binding", json!(request["after_binding"])),
        (
            "before_source_inputs_sha256",
            json!(spec.before_source_inputs_sha256),
        ),
        (
            "after_source_inputs_sha256",
            json!(spec.after_source_inputs_sha256),
        ),
        (
            "before_navigation_sha256",
            json!(spec.before_navigation_sha256),
        ),
        (
            "after_navigation_sha256",
            json!(spec.after_navigation_sha256),
        ),
        ("before_rights_sha256", json!(spec.before_rights_sha256)),
        ("after_rights_sha256", json!(spec.after_rights_sha256)),
        ("implementation_sha256", json!(spec.implementation_sha256)),
        ("manifest_schema", json!(receipt.schema)),
        ("manifest_published_last", json!(true)),
        ("changed_prepared_rows", json!(changes.len())),
        ("address_plans", json!(address_plans)),
        ("source_navigation_product", json!(nav_product)),
        ("projection_usage", projection_usage),
        ("capture_read_bytes", json!(d1_read_bytes.used)),
        ("retained_bytes", json!(retained.used)),
        ("posting_rows_observed", json!(posting_count)),
        ("digest_manifest_rows_scanned", json!(manifest_rows_scanned)),
        (
            "whole_manifest_reconciliation",
            json!(operation == "prepared-catchup"),
        ),
        (
            "maintained_auxiliary_stores",
            json!(maintained_auxiliary_stores),
        ),
        ("forward", forward_output),
        ("rollback", json!(rollback_output)),
        ("forward_sha256", json!(receipt.forward_sha256)),
        ("rollback_sha256", json!(receipt.rollback_sha256)),
        ("forward_sql_bytes", json!(receipt.forward_bytes)),
        ("rollback_sql_bytes", json!(receipt.rollback_bytes)),
        ("sql_bytes", json!(sql_bytes)),
        ("input_transition_rows", json!(receipt.changed_rows)),
        (
            "prepared_source_pairing_verified",
            json!(operation != "prepared-catchup"),
        ),
        ("successor_prepared_source_pairing_verified", json!(true)),
        (
            "predecessor_prepared_source_pairing_verified",
            json!(before_prepared_pairing_verified),
        ),
        (
            "predecessor_source_admission_external",
            json!(operation == "prepared-catchup"),
        ),
        ("held_snapshot_bindings_verified", json!(true)),
        ("source_currentness_verified", json!(false)),
        (
            "selected_pair_owner_admitted",
            json!(receipt.selected_pair_owner_admitted),
        ),
        ("rights_admission", json!(false)),
        ("semantic_acceptance", json!(false)),
        ("d1_applied", json!(receipt.d1_applied)),
        ("consumer_switched", json!(receipt.consumer_switched)),
    ]);
    attach_snapshot_transport(&mut receipt_value, snapshot_transport)?;
    let result = capture_json_object([
        ("schema", json!(result_schema(request_schema))),
        ("operation", json!(operation)),
        ("receipt", receipt_value),
    ]);
    write_json_line(stdout, &result)
}

fn navigation_row_digest_count(db: &Transaction<'_>, cap: u64) -> Result<u64, String> {
    // A digest companion is one part=0 row. Counting physical multipart
    // chunks could conceal a missing companion without performing a row audit.
    let malformed = db.query_row(
        "SELECT 1 FROM edge_meta WHERE key>=?1 AND key<?2 AND (typeof(part)!='integer' OR part!=0) LIMIT 1",
        params!["source_navigation_row_digest:", "source_navigation_row_digest;"],
        |_| Ok(()),
    ).optional().map_err(|error| error.to_string())?.is_some();
    if malformed {
        return Err(invalid("navigation row digest companion part inventory"));
    }
    let row_limit = i64::try_from(cap)
        .map_err(|_| invalid("navigation row digest inventory cap"))?
        .checked_add(1)
        .ok_or_else(|| invalid("navigation row digest inventory cap"))?;
    let count: i64 = db
        .query_row(
            "SELECT count(*) FROM (SELECT 1 FROM edge_meta WHERE key>=?1 AND key<?2 LIMIT ?3)",
            params![
                "source_navigation_row_digest:",
                "source_navigation_row_digest;",
                row_limit
            ],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if count < 0 || count as u64 > cap {
        return Err(invalid(
            "navigation row digest inventory exceeds source count",
        ));
    }
    Ok(count as u64)
}

fn run_source_navigation_integrity(
    request: &Value,
    request_schema: &str,
    frame_budget: &mut Option<SnapshotFrameBudget>,
    header_only: bool,
    limits: Limits,
    stdout: &mut dyn Write,
) -> Result<(), String> {
    let d1_path = input_path(string(request, "d1_database")?)?;
    let expected_revision = string(request, "expected_d1_revision")?;
    let expected_source_revision = string(request, "expected_source_revision")?;
    if expected_revision.len() != 64
        || !expected_revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || expected_source_revision.len() != 64
        || !expected_source_revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid("integrity source or D1 revision digest"));
    }
    let navigation = private_projection_root(&request["navigation_root"])?;
    let rights = private_projection_root(&request["rights_root"])?;
    let forward = output_path(string(request, "forward_sql")?)?;
    let rollback = output_path(string(request, "rollback_sql")?)?;
    let manifest = output_path(string(request, "manifest_json")?)?;
    validate_capture_outputs(&forward, Some(&rollback), &manifest)?;
    let vm_steps = limits.pair.max_work_bytes.min(100_000_000).max(100_000);
    let sqlite_value_bytes = limits
        .prepared
        .max_row_bytes
        .max(limits.prepared.max_metadata_bytes);
    let mut d1 = open_selected_snapshot(
        &d1_path,
        vm_steps,
        sqlite_value_bytes,
        request_schema,
        typed_snapshot::Role::D1,
        "d1_database",
        frame_budget,
    )?;
    let d1_tx = d1
        .connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(|error| error.to_string())?;
    let _: i64 = d1_tx
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;

    let mut read_bytes = D1ReadBytes::new(limits);
    let (base_top, base_top_rows, base_top_raw) =
        parse_meta_accounted(&d1_tx, "knowledge_reader_top", limits, &mut read_bytes)?;
    let (base_revision, base_revision_rows, _) =
        parse_meta_accounted(&d1_tx, "data_revision", limits, &mut read_bytes)?;
    let base_revision_row = base_revision_rows
        .first()
        .and_then(|transition| transition.before.as_ref())
        .ok_or_else(|| invalid("selected D1 revision row"))?;
    if base_top.get("read_model_schema").and_then(Value::as_str) != Some(D1_SCHEMA)
        || base_top.get("source_revision").and_then(Value::as_str) != Some(expected_source_revision)
        || base_top.get("data_revision").and_then(Value::as_str) != Some(expected_revision)
        || base_revision != json!({"sha256": expected_revision})
        || base_revision_rows.len() != 1
        || base_revision_row.get(1) != Some(&D1Cell::Integer(0))
        || base_revision_row
            .get(2)
            .is_none_or(|cell| !matches!(cell, D1Cell::Text(raw) if raw.len() <= 1024))
    {
        return Err(invalid("selected D1 source or revision differs"));
    }
    let (persisted_header, _, persisted_header_raw) =
        parse_meta_accounted(&d1_tx, "source_navigation_top", limits, &mut read_bytes)?;
    let counts =
        validate_navigation_integrity_inputs(&navigation, &rights, &persisted_header, limits)?;
    if meta_exists(&d1_tx, "source_navigation_header_digest")? {
        return Err(invalid(
            "header integrity already present; no blind checksum refresh",
        ));
    }
    let total_rows = counts.values().try_fold(0u64, |total, count| {
        total
            .checked_add(*count)
            .ok_or_else(|| invalid("integrity source row count overflow"))
    })?;
    if header_only {
        if navigation_row_digest_count(&d1_tx, total_rows)? != total_rows {
            return Err(invalid(
                "header-only migration requires existing row companion inventory",
            ));
        }
    } else if navigation_row_digest_count(&d1_tx, 0)? != 0 {
        return Err(invalid(
            "integrity product already present; no blind checksum refresh",
        ));
    }

    let mut retained = RetainedBytes::new(limits);
    let mut before_rows = BTreeMap::new();
    let mut after_rows = BTreeMap::new();
    let mut projection_usage = D1ProjectionAccounting::default();
    let verified_rows = if header_only {
        BTreeMap::new()
    } else {
        read_navigation_integrity_rows(
            &d1_tx,
            &navigation,
            &rights,
            &counts,
            limits,
            &mut retained,
            &mut after_rows,
            &mut projection_usage,
            &mut read_bytes,
        )?
    };
    let source_header_raw = persisted_header_raw;
    let header_digest_raw = compact(
        &json!({"sha256": digest(source_header_raw.as_bytes())}),
        limits.prepared.max_metadata_bytes,
    )?;
    capture_transition_rows(
        &mut before_rows,
        &mut after_rows,
        meta_raw_transitions(
            "source_navigation_header_digest",
            &[],
            &header_digest_raw,
            limits,
        )?,
        &mut retained,
    )?;

    let navigation_sha256 = navigation.snapshot_sha256().to_owned();
    let rights_sha256 = rights.snapshot_sha256().to_owned();
    let source_inputs_raw = compact(
        &json!({
            "source_navigation_sha256": navigation_sha256,
            "rights_sha256": rights_sha256,
        }),
        limits.prepared.max_metadata_bytes,
    )?;
    let source_inputs_sha256 = digest(source_inputs_raw.as_bytes());
    let migration_implementation_sha256 = digest(include_bytes!("edge_offline_capture.rs"));
    let mut spec = D1PrivatePreparedInput {
        predecessor_mode: D1PrivatePreparedMode::Integrity { header_only },
        base_d1_revision: expected_revision.to_owned(),
        before_source_revision: expected_source_revision.to_owned(),
        after_source_revision: expected_source_revision.to_owned(),
        before_prepared_binding: None,
        after_prepared_binding: None,
        before_source_inputs_sha256: None,
        after_source_inputs_sha256: None,
        before_navigation_sha256: Some(navigation_sha256.clone()),
        after_navigation_sha256: Some(navigation_sha256),
        before_rights_sha256: Some(rights_sha256.clone()),
        after_rights_sha256: Some(rights_sha256),
        implementation_sha256: implementation_digest(),
        bootstrap_implementation_sha256: None,
        migration_implementation_sha256: Some(migration_implementation_sha256.clone()),
        auxiliary_stores: auxiliary_stores_accounted(
            &d1_tx,
            &base_top_raw,
            limits,
            &mut read_bytes,
        )?,
        before_reader_top: base_top_raw.clone(),
        after_reader_top: String::new(),
        limits: limits.pair,
    };
    let target_revision = private_prepared_target_revision(&spec)
        .map_err(|_| invalid("integrity D1 lineage inputs"))?;
    let catalog_sha256 = base_top
        .get("catalog_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("selected D1 catalog binding"))?;
    let lens_sha256 = base_top
        .get("lens_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("selected D1 lens binding"))?;
    let next_top_raw = reader_top_raw_from_predecessor(
        &base_top_raw,
        expected_source_revision,
        &target_revision,
        catalog_sha256,
        lens_sha256,
        limits,
    )?;
    spec.after_reader_top = next_top_raw.clone();
    inspect_private_prepared_source_inputs(&spec, None, None)
        .map_err(|error| format!("offline navigation integrity pair: {error:?}"))?;

    let (_, data_revision_rows, _) =
        parse_meta_accounted(&d1_tx, "data_revision", limits, &mut read_bytes)?;
    let target_revision_raw = compact(
        &json!({"sha256": target_revision}),
        limits.prepared.max_metadata_bytes,
    )?;
    for (key, prior, next) in [
        (
            "knowledge_reader_top",
            base_top_rows.as_slice(),
            next_top_raw.as_str(),
        ),
        (
            "data_revision",
            data_revision_rows.as_slice(),
            target_revision_raw.as_str(),
        ),
    ] {
        capture_transition_rows(
            &mut before_rows,
            &mut after_rows,
            meta_raw_transitions(key, prior, next, limits)?,
            &mut retained,
        )?;
    }

    let transitions = row_transitions(before_rows, after_rows, limits, &mut retained)?;
    d1.identity.verify_selected_file_identity()?;
    let snapshot_transport =
        snapshot_transport_value(request_schema, &[("d1_database", &d1.identity)])?;
    let receipt =
        emit_private_prepared_capture(&spec, transitions, &forward, Some(&rollback), &manifest)
            .map_err(|error| format!("offline navigation integrity capture: {error:?}"))?;
    let sql_bytes = receipt
        .forward_bytes
        .checked_add(receipt.rollback_bytes)
        .ok_or_else(|| invalid("integrity SQL byte count overflow"))?;
    let verified_header_counts = json!({
        "nodes": counts["nodes"],
        "edges": counts["edges"],
        "rights": counts["rights"],
    });
    let verified_source_rows = if header_only {
        json!({})
    } else {
        json!(verified_rows)
    };
    let maintained_auxiliary_stores = spec
        .auxiliary_stores
        .iter()
        .map(|store| store.identity().0)
        .collect::<Vec<_>>();
    let forward_output = json!({
        "available": true,
        "sha256": receipt.forward_sha256,
        "bytes": receipt.forward_bytes,
        "base_revision": receipt.base_d1_revision,
        "target_revision": receipt.target_d1_revision,
        "published": true,
        "publication": "private-capture-manifest-last"
    });
    let rollback_output = receipt.rollback_sha256.as_ref().map(|sha256| {
        json!({
            "available": true,
            "sha256": sha256,
            "bytes": receipt.rollback_bytes,
            "base_revision": receipt.target_d1_revision,
            "target_revision": receipt.base_d1_revision,
            "published": true,
            "publication": "private-capture-manifest-last"
        })
    });
    let projection_usage_value = capture_json_object([
        ("opened_parts", json!(projection_usage.opened_parts)),
        ("stored_bytes", json!(projection_usage.stored_bytes)),
        ("decoded_bytes", json!(projection_usage.decoded_bytes)),
        ("keys", json!(projection_usage.keys)),
        ("changes", json!(projection_usage.changes)),
        ("output_bytes", json!(projection_usage.output_bytes)),
    ]);
    let mut receipt_value = capture_json_object([
        (
            "schema",
            json!("tos_edge_native_source_navigation_integrity_receipt_v1"),
        ),
        ("operation", json!("source-navigation-integrity")),
        (
            "lineage_schema",
            json!("tos_native_navigation_integrity_migration_v1"),
        ),
        ("header_only", json!(header_only)),
        ("base_d1_revision", json!(receipt.base_d1_revision)),
        ("target_d1_revision", json!(receipt.target_d1_revision)),
        ("source_revision", json!(expected_source_revision)),
        (
            "source_navigation_sha256",
            json!(navigation.snapshot_sha256()),
        ),
        (
            "source_navigation_header_sha256",
            json!(digest(source_header_raw.as_bytes())),
        ),
        ("rights_sha256", json!(rights.snapshot_sha256())),
        ("source_inputs_sha256", json!(source_inputs_sha256)),
        ("implementation_sha256", json!(spec.implementation_sha256)),
        (
            "migration_implementation_sha256",
            json!(migration_implementation_sha256),
        ),
        ("manifest_schema", json!(receipt.schema)),
        ("manifest_published_last", json!(true)),
        ("changed_rows", json!(receipt.changed_rows)),
        ("verified_source_rows", verified_source_rows),
        ("verified_header_counts", verified_header_counts),
        ("projection_usage", projection_usage_value),
        ("retained_bytes", json!(retained.used)),
        ("capture_read_bytes", json!(read_bytes.used)),
        (
            "maintained_auxiliary_stores",
            json!(maintained_auxiliary_stores),
        ),
        ("forward", forward_output),
        ("rollback", json!(rollback_output)),
        ("forward_sha256", json!(receipt.forward_sha256)),
        ("rollback_sha256", json!(receipt.rollback_sha256)),
        ("forward_sql_bytes", json!(receipt.forward_bytes)),
        ("rollback_sql_bytes", json!(receipt.rollback_bytes)),
        ("sql_bytes", json!(sql_bytes)),
        ("native_rows_changed", json!(0)),
        ("normalized_rows_changed", json!(0)),
        ("source_rights_admission_verified_by_helper", json!(false)),
        ("held_snapshot_bindings_verified", json!(true)),
        ("source_currentness_verified", json!(false)),
        ("selected_pair_owner_admitted", json!(false)),
        ("rights_admission", json!(false)),
        ("semantic_acceptance", json!(false)),
        ("d1_applied", json!(receipt.d1_applied)),
        ("consumer_switched", json!(receipt.consumer_switched)),
    ]);
    attach_snapshot_transport(&mut receipt_value, snapshot_transport)?;
    let result = capture_json_object([
        ("schema", json!(result_schema(request_schema))),
        ("operation", json!("source-navigation-integrity")),
        ("receipt", receipt_value),
    ]);
    write_json_line(stdout, &result)
}

fn run_request(raw: &[u8], stdout: &mut dyn Write, parent_guarded: bool) -> Result<(), String> {
    if raw.is_empty() || raw.len() > REQUEST_BYTES {
        return Err(invalid("capture request byte budget"));
    }
    // Preflight exact duplicate-key, depth, visit and logical-state bounds
    // before building the serde request tree. The Foundation tree is dropped
    // first; the logical state budget is not an RSS promise.
    let preflight_limits = JsonLimits::new(REQUEST_BYTES, 128, REQUEST_JSON_VISITS, 4300)
        .map_err(|error| error.to_string())?;
    drop(
        parse_json_with_state_budget(
            raw,
            JsonMode::PublishedStrict,
            preflight_limits,
            REQUEST_JSON_STATE_BYTES,
        )
        .map_err(|error| error.to_string())?,
    );
    let request: Value =
        serde_json::from_slice(raw).map_err(|_| invalid("capture request JSON"))?;
    let request_schema = string(&request, "schema")?;
    let operation = string(&request, "operation")?;
    if request_schema != REQUEST_SCHEMA && request_schema != TYPED_REQUEST_SCHEMA {
        return Err(invalid("capture request schema"));
    }
    let typed_request = request_schema == TYPED_REQUEST_SCHEMA;
    if typed_request != parent_guarded {
        return Err(invalid("typed capture requires an armed caller process"));
    }
    if operation == "source-navigation-integrity" {
        let mut fields = vec![
            "schema",
            "operation",
            "d1_database",
            "expected_d1_revision",
            "expected_source_revision",
            "navigation_root",
            "rights_root",
            "header_only",
            "forward_sql",
            "rollback_sql",
            "manifest_json",
            "limits",
        ];
        if typed_request {
            fields.push("snapshot_frame_max_bytes");
            fields.push("snapshot_schema_max_allocation_bytes");
        }
        exact(&request, &fields, "integrity capture request fields")?;
    } else {
        let mut fields = vec![
            "schema",
            "operation",
            "d1_database",
            "before_prepared_database",
            "after_prepared_database",
            "expected_d1_revision",
            "before_binding",
            "after_binding",
            "before_source_inputs_json",
            "rights_root",
            "forward_sql",
            "rollback_sql",
            "manifest_json",
            "limits",
        ];
        if typed_request {
            fields.push("snapshot_frame_max_bytes");
            fields.push("snapshot_schema_max_allocation_bytes");
        }
        exact(&request, &fields, "capture request fields")?;
    }
    if !matches!(
        operation,
        "prepared-delta"
            | "prepared-catchup"
            | "source-navigation-bootstrap"
            | "source-navigation-delta"
            | "source-navigation-integrity"
    ) {
        return Err(invalid("unsupported private Edge capture operation"));
    }
    let limits = limits(&request["limits"], operation)?;
    let mut frame_budget = if typed_request {
        Some(preflight_snapshot_frame_budget(&request, operation)?)
    } else {
        None
    };
    if operation == "source-navigation-integrity" {
        let header_only = request["header_only"]
            .as_bool()
            .ok_or_else(|| invalid("integrity header_only must be boolean"))?;
        return run_source_navigation_integrity(
            &request,
            request_schema,
            &mut frame_budget,
            header_only,
            limits,
            stdout,
        );
    }
    if operation != "source-navigation-bootstrap" {
        return run_prepared_transition(
            &request,
            operation,
            request_schema,
            &mut frame_budget,
            limits,
            stdout,
        );
    }
    let d1_path = input_path(string(&request, "d1_database")?)?;
    let after_path = input_path(string(&request, "after_prepared_database")?)?;
    if !request["before_prepared_database"].is_null()
        || !request["before_binding"].is_null()
        || !request["before_source_inputs_json"].is_null()
    {
        return Err(invalid(
            "bootstrap request must not claim a prepared predecessor",
        ));
    }
    let binding = foundation(
        &request["after_binding"],
        limits.prepared.max_metadata_bytes,
    )?;
    let (rights, rights_expected, _rights_trusted) =
        private_trusted_projection_root(&request["rights_root"])?;
    let forward = output_path(string(&request, "forward_sql")?)?;
    let rollback = output_path(string(&request, "rollback_sql")?)?;
    let manifest = output_path(string(&request, "manifest_json")?)?;
    validate_capture_outputs(&forward, Some(&rollback), &manifest)?;
    let vm_steps = limits.pair.max_work_bytes.min(100_000_000).max(100_000);
    let sqlite_value_bytes = limits
        .prepared
        .max_row_bytes
        .max(limits.prepared.max_metadata_bytes);
    let mut d1 = open_selected_snapshot(
        &d1_path,
        vm_steps,
        sqlite_value_bytes,
        request_schema,
        typed_snapshot::Role::D1,
        "d1_database",
        &mut frame_budget,
    )?;
    let mut after_db = open_selected_snapshot(
        &after_path,
        vm_steps,
        sqlite_value_bytes,
        request_schema,
        typed_snapshot::Role::Prepared,
        "after_prepared_database",
        &mut frame_budget,
    )?;
    let d1_tx = d1
        .connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(|error| error.to_string())?;
    let after_tx = after_db
        .connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(|error| error.to_string())?;
    // Force each SQLite snapshot before any retained data is inspected.
    let _: i64 = d1_tx
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let _: i64 = after_tx
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let mut d1_read_bytes = D1ReadBytes::new(limits);
    let after_source =
        prepared_source_inputs_held(&after_tx, &binding, limits, &mut d1_read_bytes)?;
    let binding_revision = foundation_string(&binding, "data_revision")?;
    let (prepared_data_revision, _, _) =
        parse_meta_accounted(&after_tx, "data_revision", limits, &mut d1_read_bytes)?;
    if prepared_data_revision != json!({"sha256": binding_revision}) {
        return Err(invalid(
            "prepared bootstrap data revision differs from binding",
        ));
    }
    let _prepared_descriptor =
        prepared_descriptor(&after_tx, limits, &mut d1_read_bytes, binding_revision)?;
    let nav_root = root_for(&after_source, "source-navigation")?;
    let nav = parsed_root(nav_root)?;
    let nav_root_json: Value =
        serde_json::from_slice(nav.root_bytes()).map_err(|_| invalid("navigation root JSON"))?;
    let nav_collections = nav_root_json
        .get("collections")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("navigation collections"))?;
    if !matches!(
        nav_root_json.get("logical_schema").and_then(Value::as_str),
        Some("tos_agent_source_navigation_rows_v1" | "tos_source_navigation_v1")
    ) || nav_collections.len() != 2
        || !nav_collections.contains_key("nodes")
        || !nav_collections.contains_key("edges")
    {
        return Err(invalid(
            "bootstrap requires exact separate node-edge and rights roots",
        ));
    }
    for (collection, key_field) in [("nodes", "node_id"), ("edges", "edge_id")] {
        let spec = &nav_collections[collection];
        if spec.get("key_field").and_then(Value::as_str) != Some(key_field)
            || spec.get("order_fields") != Some(&json!([key_field]))
        {
            return Err(invalid("bootstrap navigation identity/order profile"));
        }
    }
    let rights_json: Value =
        serde_json::from_slice(rights.root_bytes()).map_err(|_| invalid("rights root JSON"))?;
    let rights_collections = rights_json
        .get("collections")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("rights collections"))?;
    if rights_json.get("logical_schema").and_then(Value::as_str)
        != Some("tos_source_navigation_rights_v1")
        || rights_collections.len() != 1
        || !rights_collections.contains_key("rights")
    {
        return Err(invalid("separate rights projection profile"));
    }
    let rights_spec = &rights_collections["rights"];
    if rights_spec.get("key_field").and_then(Value::as_str) != Some("rights_id")
        || rights_spec.get("order_fields") != Some(&json!(["rights_id"]))
    {
        return Err(invalid("bootstrap rights identity/order profile"));
    }
    let (base_top, _, base_top_raw) =
        parse_meta_accounted(&d1_tx, "knowledge_reader_top", limits, &mut d1_read_bytes)?;
    let (prepared_top, _, _) = parse_meta_accounted(
        &after_tx,
        "knowledge_reader_top",
        limits,
        &mut d1_read_bytes,
    )?;
    let (base_data_revision, _, _) =
        parse_meta_accounted(&d1_tx, "data_revision", limits, &mut d1_read_bytes)?;
    let (base_catalog, _, base_catalog_raw) =
        parse_meta_accounted(&d1_tx, "knowledge_catalog", limits, &mut d1_read_bytes)?;
    let (after_catalog, _, after_catalog_raw) =
        parse_meta_accounted(&after_tx, "knowledge_catalog", limits, &mut d1_read_bytes)?;
    let (base_lens, _, base_lens_raw) =
        parse_meta_accounted(&d1_tx, "knowledge_lens_top", limits, &mut d1_read_bytes)?;
    let (after_lens, _, after_lens_raw) =
        parse_meta_accounted(&after_tx, "knowledge_lens_top", limits, &mut d1_read_bytes)?;
    let expected_revision = string(&request, "expected_d1_revision")?;
    let mut base_top_profile = base_top.clone();
    let mut prepared_top_profile = prepared_top.clone();
    for top in [&mut base_top_profile, &mut prepared_top_profile] {
        if let Some(fields) = top.as_object_mut() {
            fields.remove("read_model_schema");
            fields.remove("data_revision");
        }
    }
    if base_top.get("read_model_schema").and_then(Value::as_str) != Some(D1_SCHEMA)
        || base_top.get("data_revision").and_then(Value::as_str) != Some(expected_revision)
        || base_data_revision != json!({"sha256": expected_revision})
        || base_top.get("catalog_sha256").and_then(Value::as_str)
            != Some(digest(base_catalog_raw.as_bytes()).as_str())
        || base_top.get("lens_sha256").and_then(Value::as_str)
            != Some(digest(base_lens_raw.as_bytes()).as_str())
        || prepared_top
            .get("read_model_schema")
            .and_then(Value::as_str)
            != Some(PREPARED_SCHEMA)
        || prepared_top.get("catalog_sha256").and_then(Value::as_str)
            != Some(digest(after_catalog_raw.as_bytes()).as_str())
        || prepared_top.get("lens_sha256").and_then(Value::as_str)
            != Some(digest(after_lens_raw.as_bytes()).as_str())
        || base_catalog != after_catalog
        || base_lens != after_lens
        || base_top_profile != prepared_top_profile
    {
        return Err(invalid("held D1/prepared predecessor metadata differs"));
    }
    let installed_auxiliary =
        auxiliary_stores_accounted(&d1_tx, &base_top_raw, limits, &mut d1_read_bytes)?;
    let (existing_top, _, _) =
        parse_meta_accounted(&d1_tx, "source_navigation_top", limits, &mut d1_read_bytes)?;
    if existing_top != json!({}) {
        return Err(invalid("native navigation product is not absent"));
    }
    for table in [
        "source_navigation_nodes",
        "source_navigation_node_payload",
        "source_navigation_edges",
        "source_navigation_edge_payload",
        "source_navigation_rights",
        "source_navigation_rights_payload",
    ] {
        let exists: Option<i64> = d1_tx
            .query_row(&format!("SELECT 1 FROM {table} LIMIT 1"), [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|error| error.to_string())?;
        if exists.is_some() {
            return Err(invalid("native navigation tables are not empty"));
        }
    }
    let (mut transitions, header, projection_usage, retained_bytes, header_sha256) =
        bootstrap_transitions(
            &d1_tx,
            &after_source,
            &nav,
            &rights,
            limits,
            &mut d1_read_bytes,
        )?;
    if transitions.len() as u64 > limits.pair.max_transitions {
        return Err(invalid("private D1 transition limit"));
    }
    let before_inputs_sha = after_source.digest().to_owned();
    let source_revision = after_source.source_revision().to_owned();
    let before_binding_raw = compact(
        &request["after_binding"],
        limits.prepared.max_metadata_bytes,
    )?;
    let after_binding_raw = before_binding_raw.clone();
    let nav_sha = nav.snapshot_sha256().to_owned();
    let mut spec = D1PrivatePreparedInput {
        predecessor_mode: D1PrivatePreparedMode::Bootstrap,
        base_d1_revision: expected_revision.to_owned(),
        before_source_revision: source_revision.clone(),
        after_source_revision: source_revision,
        before_prepared_binding: None,
        after_prepared_binding: Some(after_binding_raw),
        before_source_inputs_sha256: Some(before_inputs_sha.clone()),
        after_source_inputs_sha256: Some(before_inputs_sha),
        before_navigation_sha256: Some(nav_sha.clone()),
        after_navigation_sha256: Some(nav_sha),
        before_rights_sha256: Some(rights.snapshot_sha256().to_owned()),
        after_rights_sha256: Some(rights.snapshot_sha256().to_owned()),
        implementation_sha256: implementation_digest(),
        bootstrap_implementation_sha256: Some(digest(include_bytes!("edge_offline_capture.rs"))),
        migration_implementation_sha256: None,
        auxiliary_stores: installed_auxiliary,
        before_reader_top: base_top_raw.clone(),
        after_reader_top: String::new(),
        limits: limits.pair,
    };
    let revision = private_prepared_target_revision(&spec)
        .map_err(|_| invalid("private D1 lineage inputs"))?;
    spec.after_reader_top = reader_top_raw_from_predecessor(
        &base_top_raw,
        after_source.source_revision(),
        &revision,
        &digest(after_catalog_raw.as_bytes()),
        &digest(after_lens_raw.as_bytes()),
        limits,
    )?;
    inspect_private_prepared_source_inputs(&spec, None, Some(after_source.raw()))
        .map_err(|error| format!("offline bootstrap source pair: {error:?}"))?;
    d1.identity.verify_selected_file_identity()?;
    after_db.identity.verify_selected_file_identity()?;
    let snapshot_transport = snapshot_transport_value(
        request_schema,
        &[
            ("d1_database", &d1.identity),
            ("after_prepared_database", &after_db.identity),
        ],
    )?;
    let receipt = emit_private_prepared_capture(
        &spec,
        transitions.drain(..),
        &forward,
        Some(&rollback),
        &manifest,
    )
    .map_err(|error| format!("offline private capture: {error:?}"))?;
    let sql_bytes = receipt
        .forward_bytes
        .checked_add(receipt.rollback_bytes)
        .ok_or_else(|| invalid("bootstrap SQL byte count overflow"))?;
    let maintained_auxiliary_stores = spec
        .auxiliary_stores
        .iter()
        .map(|store| store.identity().0)
        .collect::<Vec<_>>();
    let forward_output = json!({
        "available": true,
        "sha256": receipt.forward_sha256,
        "bytes": receipt.forward_bytes,
        "base_revision": receipt.base_d1_revision,
        "target_revision": receipt.target_d1_revision,
        "published": true,
        "publication": "private-capture-manifest-last"
    });
    let rollback_output = receipt.rollback_sha256.as_ref().map(|sha256| {
        json!({
            "available": true,
            "sha256": sha256,
            "bytes": receipt.rollback_bytes,
            "base_revision": receipt.target_d1_revision,
            "target_revision": receipt.base_d1_revision,
            "published": true,
            "publication": "private-capture-manifest-last"
        })
    });
    let projection_usage_value = capture_json_object([
        ("opened_parts", json!(projection_usage.opened_parts)),
        ("stored_bytes", json!(projection_usage.stored_bytes)),
        ("decoded_bytes", json!(projection_usage.decoded_bytes)),
        ("keys", json!(projection_usage.keys)),
        ("changes", json!(projection_usage.changes)),
        ("output_bytes", json!(projection_usage.output_bytes)),
    ]);
    let mut receipt_value = capture_json_object([
        (
            "schema",
            json!("tos_edge_native_source_navigation_bootstrap_receipt_v1"),
        ),
        ("operation", json!(operation)),
        (
            "lineage_schema",
            json!("tos_native_navigation_d1_bootstrap_v1"),
        ),
        ("base_d1_revision", json!(receipt.base_d1_revision)),
        ("target_d1_revision", json!(receipt.target_d1_revision)),
        ("source_revision", json!(after_source.source_revision())),
        ("prepared_binding", json!(request["after_binding"])),
        ("source_inputs_sha256", json!(after_source.digest())),
        ("source_navigation_sha256", json!(nav.snapshot_sha256())),
        ("rights_sha256", json!(rights.snapshot_sha256())),
        ("implementation_sha256", json!(spec.implementation_sha256)),
        (
            "bootstrap_implementation_sha256",
            json!(spec.bootstrap_implementation_sha256),
        ),
        ("manifest_schema", json!(receipt.schema)),
        ("manifest_published_last", json!(true)),
        ("changed_rows", json!(receipt.changed_rows)),
        ("source_navigation_header", json!(header)),
        ("source_navigation_header_sha256", json!(header_sha256)),
        ("verified_header_counts", json!(header.get("counts"))),
        ("projection_usage", projection_usage_value),
        ("capture_read_bytes", json!(d1_read_bytes.used)),
        ("retained_bytes", json!(retained_bytes)),
        (
            "maintained_auxiliary_stores",
            json!(maintained_auxiliary_stores),
        ),
        ("forward", forward_output),
        ("rollback", json!(rollback_output)),
        ("forward_sha256", json!(receipt.forward_sha256)),
        ("rollback_sha256", json!(receipt.rollback_sha256)),
        ("forward_sql_bytes", json!(receipt.forward_bytes)),
        ("rollback_sql_bytes", json!(receipt.rollback_bytes)),
        ("sql_bytes", json!(sql_bytes)),
        ("prepared_source_pairing_verified", json!(true)),
        ("held_snapshot_bindings_verified", json!(true)),
        ("source_currentness_verified", json!(false)),
        (
            "selected_pair_owner_admitted",
            json!(receipt.selected_pair_owner_admitted),
        ),
        ("rights_admission", json!(false)),
        ("semantic_acceptance", json!(false)),
        ("d1_applied", json!(receipt.d1_applied)),
        ("consumer_switched", json!(receipt.consumer_switched)),
    ]);
    attach_snapshot_transport(&mut receipt_value, snapshot_transport)?;
    let result = capture_json_object([
        ("schema", json!(result_schema(request_schema))),
        ("operation", json!(operation)),
        ("receipt", receipt_value),
    ]);
    write_json_line(stdout, &result)
}

#[cfg(target_os = "linux")]
fn arm_parent_death_signal(expected_parent_pid: libc::pid_t) -> Result<(), String> {
    let own_pid = unsafe { libc::getpid() };
    if expected_parent_pid <= 1 || expected_parent_pid == own_pid {
        return Err(invalid("expected caller process id"));
    }
    if unsafe { libc::getppid() } != expected_parent_pid {
        return Err(invalid("caller process changed before death guard"));
    }
    if unsafe {
        libc::prctl(
            libc::PR_SET_PDEATHSIG,
            libc::SIGKILL,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
        )
    } != 0
    {
        return Err(invalid("caller death guard unavailable"));
    }
    if unsafe { libc::getppid() } != expected_parent_pid {
        return Err(invalid("caller process changed while arming death guard"));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn arm_parent_death_signal(_expected_parent_pid: libc::pid_t) -> Result<(), String> {
    Err(invalid("caller death guard is Linux-only"))
}

fn run(args: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let (request_argument, parent_guarded) = match args {
        [command, request_flag, request_path]
            if command == "edge-offline-capture" && request_flag == "--request" =>
        {
            (request_path, false)
        }
        [
            command,
            parent_flag,
            expected_parent_pid,
            request_flag,
            request_path,
        ] if command == "edge-offline-capture"
            && parent_flag == "--expected-parent-pid"
            && request_flag == "--request" =>
        {
            let expected_parent_pid = expected_parent_pid
                .parse::<libc::pid_t>()
                .map_err(|_| invalid("expected caller process id"))?;
            // The bridge supplies this child-only cue outside the semantic
            // request. Arm it before opening or parsing the request bytes.
            arm_parent_death_signal(expected_parent_pid)?;
            (request_path, true)
        }
        _ => {
            return Err(invalid(
                "usage: tos edge-offline-capture [--expected-parent-pid PID] --request ABS.json",
            ));
        }
    };
    let request_path = input_path(request_argument)?;
    let request_file = tos_fd_open::open_absolute_regular(&request_path, REQUEST_BYTES as u64)
        .map_err(|error| format!("capture request file: {error}"))?;
    let mut raw = Vec::new();
    request_file
        .take(REQUEST_BYTES as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|error| error.to_string())?;
    run_request(&raw, stdout, parent_guarded)
}

pub fn run_if_requested(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    if args.first().is_none_or(|arg| arg != "edge-offline-capture") {
        return None;
    }
    Some(match run(args, stdout) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(stderr, "edge offline capture: {error}");
            2
        }
    })
}
