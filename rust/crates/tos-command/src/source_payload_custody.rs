//! Native source-payload custody compatibility owner.
//!
//! The maintained Python module is a wire adapter. This owner keeps manifest
//! selection, path validation, fixity, no-clobber publication and receipt
//! generation in the same descriptor-based custody substrate used by the
//! acquisition batch route.
use crate::source_acquisition_batch as batch;
use crate::source_serialization;
use serde_json::{Map, Value, json};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs::{self, Metadata};
use std::io::Read;
use std::os::unix::fs::MetadataExt as StdMetadataExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use tos_foundation::{Digest256, Digest256Hasher};

const SOURCE_PREFIX: &str = "ToS/source-witnesses";
const MAX_METADATA: u64 = 16 * 1024 * 1024;
const MAX_PAYLOAD: u64 = 300 * 1024 * 1024;
type Result<T> = std::result::Result<T, String>;

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing text field: {key}"))
}

fn path(value: &Value, key: &str) -> Result<PathBuf> {
    let raw = text(value, key)?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() || path.to_str() != Some(raw) {
        return Err(format!("{key} must be an absolute UTF-8 path"));
    }
    Ok(path)
}

fn path_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| "custody paths must be UTF-8".into())
}

fn optional_path(value: &Value, key: &str) -> Result<Option<PathBuf>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(raw)) => {
            let path = PathBuf::from(raw);
            if !path.is_absolute() || path.to_str() != Some(raw) {
                return Err(format!("{key} must be an absolute UTF-8 path"));
            }
            Ok(Some(path))
        }
        _ => Err(format!("{key} must be a path or null")),
    }
}

fn bool_value(value: &Value, key: &str, default: bool) -> Result<bool> {
    match value.get(key) {
        None => Ok(default),
        Some(Value::Bool(v)) => Ok(*v),
        _ => Err(format!("{key} must be boolean")),
    }
}

fn safe_parts(value: &str, label: &str) -> Result<Vec<String>> {
    batch::safe_ref(value).map_err(|_| format!("unsafe {label}: {value}"))?;
    Ok(value.split('/').map(str::to_owned).collect())
}

fn checked_root(root: &Path, must_exist: bool) -> Result<PathBuf> {
    if !root.is_absolute() {
        return Err("custody roots must be absolute".into());
    }
    let mut current = PathBuf::from("/");
    let components = root.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        match component {
            Component::RootDir => continue,
            Component::Normal(part) => current.push(part),
            Component::CurDir => continue,
            Component::ParentDir => {
                if !current.pop() {
                    return Err("custody root escapes filesystem root".into());
                }
                continue;
            }
            _ => return Err(format!("invalid custody root: {}", root.display())),
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!("symlink in custody root: {}", current.display()));
            }
            Ok(meta) if index + 1 < components.len() && !meta.is_dir() => {
                return Err(format!(
                    "non-directory custody ancestor: {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !must_exist => {}
            Err(error) => {
                return Err(format!(
                    "cannot inspect custody root {}: {error}",
                    root.display()
                ));
            }
        }
    }
    let normalized = current;
    if must_exist {
        let descriptor = tos_fd_open::open_absolute_directory(&normalized).map_err(|error| {
            format!(
                "custody root is not a no-follow directory: {}: {error}",
                normalized.display()
            )
        })?;
        let meta = descriptor.metadata().map_err(|error| error.to_string())?;
        if !meta.is_dir() {
            return Err(format!(
                "custody root is not a directory: {}",
                normalized.display()
            ));
        }
    }
    match fs::canonicalize(&normalized) {
        Ok(canonical) => Ok(canonical),
        Err(error) if !must_exist && error.kind() == std::io::ErrorKind::NotFound => Ok(normalized),
        Err(error) => Err(format!(
            "cannot resolve custody root {}: {error}",
            normalized.display()
        )),
    }
}

fn relative_path_under(root: &Path, target: &Path) -> Result<String> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| format!("payload path is outside custody root: {}", target.display()))?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(
                part.to_str()
                    .ok_or_else(|| "custody path must be UTF-8".to_owned())?,
            ),
            _ => {
                return Err(format!(
                    "payload path is not a safe relative path: {}",
                    target.display()
                ));
            }
        }
    }
    if parts.is_empty() {
        return Err(format!(
            "payload path is not a safe relative path: {}",
            target.display()
        ));
    }
    let reference = parts.join("/");
    batch::safe_ref(&reference).map_err(|_| {
        format!(
            "payload path is not a safe relative path: {}",
            target.display()
        )
    })?;
    Ok(reference)
}

fn checked_child(root: &Path, parts: &[String]) -> Result<PathBuf> {
    let root = checked_root(root, true)?;
    if parts.is_empty() {
        return Ok(root);
    }
    let mut relative = Vec::with_capacity(parts.len());
    for part in parts {
        batch::safe_ref(part).map_err(|_| format!("unsafe custody path component: {part}"))?;
        if part.contains('/') {
            return Err(format!("unsafe custody path component: {part}"));
        }
        relative.push(part.as_str());
    }
    batch::path_under(&root, &relative.join("/"))
}

fn payload_path(root: &Path, item_root_ref: &str, relative_path: &str) -> Result<PathBuf> {
    let item = safe_parts(item_root_ref, "Item root reference")?;
    let relative = safe_parts(relative_path, "payload relative path")?;
    if item.len() < 3 || item[0] != "ToS" || item[1] != "source-witnesses" {
        return Err(format!(
            "Item root is outside {SOURCE_PREFIX}: {item_root_ref}"
        ));
    }
    if relative.first().map(String::as_str) != Some("payload") {
        return Err(format!(
            "payload path must begin with payload/: {relative_path}"
        ));
    }
    let mut parts = item.into_iter().skip(2).collect::<Vec<_>>();
    parts.extend(relative);
    checked_child(root, &parts)
}

fn metadata_path(root: &Path, manifest_ref: &str) -> Result<PathBuf> {
    let parts = safe_parts(manifest_ref, "manifest reference")?;
    checked_child(root, &parts)
}

fn item_root_from_manifest_ref(manifest_ref: &str) -> Result<String> {
    let parts = safe_parts(manifest_ref, "manifest reference")?;
    if parts.last().map(String::as_str) != Some("item.manifest.json") {
        return Err("manifest reference must end in item.manifest.json".into());
    }
    let item = &parts[..parts.len() - 1];
    if item.len() < 3 || item[0] != "ToS" || item[1] != "source-witnesses" {
        return Err("Item manifest is outside ToS/source-witnesses".into());
    }
    Ok(item.join("/"))
}

fn validate_digest_shape(value: Option<&str>, label: &str) -> Result<()> {
    if value.is_some_and(|digest| {
        digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }) {
        return Err(format!("invalid {label}"));
    }
    Ok(())
}

fn valid_item_id(item_id: &str) -> bool {
    item_id.starts_with("tos.item.")
        && item_id.len() > "tos.item.".len()
        && !item_id.chars().any(char::is_whitespace)
        && !item_id.contains(['/', '\\', '\0'])
}

fn integer(value: &Value, label: &str) -> Result<u64> {
    value
        .as_u64()
        .ok_or_else(|| format!("{label} must be a nonnegative integer"))
}

fn entry_value(entry: &Value) -> Result<Value> {
    let item_id = text(entry, "item_id")?;
    let item_root_ref = text(entry, "item_root_ref")?;
    let relative_path = text(entry, "relative_path")?;
    let source_root = path(entry, "source_root")?;
    let file_id = batch::optional_text(entry, "file_id")?;
    let sha256 = batch::optional_text(entry, "sha256")?;
    let manifest_ref = batch::optional_text(entry, "manifest_ref")?;
    let source_label = batch::optional_text(entry, "source_label")?.unwrap_or("source");
    let git_blob_sha1 = batch::optional_text(entry, "git_blob_sha1")?;
    let byte_size = integer(
        entry.get("byte_size").ok_or("missing byte_size")?,
        "byte_size",
    )?;
    if !valid_item_id(item_id) {
        return Err(format!("invalid Item ID: {item_id}"));
    }
    safe_parts(item_root_ref, "Item root reference")?;
    safe_parts(relative_path, "payload relative path")?;
    if let Some(digest) = sha256 {
        validate_digest_shape(Some(digest), "payload sha256")?;
    }
    if let Some(file_id) = file_id {
        if !file_id.starts_with("tos.file.sha256.") {
            return Err("invalid File ID".into());
        }
        if let Some(digest) = sha256 {
            if file_id != format!("tos.file.sha256.{digest}") {
                return Err("payload File ID is not bound to sha256".into());
            }
        }
    }
    let mut result = Map::new();
    result.insert("item_id".into(), json!(item_id));
    result.insert("file_id".into(), json!(file_id));
    result.insert("item_root_ref".into(), json!(item_root_ref));
    result.insert("relative_path".into(), json!(relative_path));
    result.insert("byte_size".into(), json!(byte_size));
    result.insert("sha256".into(), json!(sha256));
    result.insert("source_root".into(), json!(path_string(&source_root)?));
    result.insert("manifest_ref".into(), json!(manifest_ref));
    result.insert("source_label".into(), json!(source_label));
    result.insert("git_blob_sha1".into(), json!(git_blob_sha1));
    Ok(Value::Object(result))
}

fn entries_from_manifest_payload(
    item_id: &str,
    item_root_ref: &str,
    manifest_ref: &str,
    source_root: &Path,
    payload: &Value,
    source_label: &str,
) -> Result<Value> {
    if !valid_item_id(item_id) {
        return Err(format!("invalid Item manifest: {manifest_ref}"));
    }
    let relative_path = text(payload, "relative_path")?;
    safe_parts(relative_path, "payload relative path")?;
    let file_id = text(payload, "file_id")?;
    let byte_size = integer(
        payload
            .get("byte_size")
            .ok_or("missing payload byte_size")?,
        "payload byte_size",
    )?;
    let sha256 = payload.get("sha256").and_then(Value::as_str);
    validate_digest_shape(sha256, "payload sha256")?;
    if sha256.is_none() || file_id != format!("tos.file.sha256.{}", sha256.unwrap()) {
        return Err(format!(
            "payload File ID is not bound to sha256: {manifest_ref}"
        ));
    }
    Ok(json!({
        "item_id": item_id,
        "file_id": file_id,
        "item_root_ref": item_root_ref,
        "relative_path": relative_path,
        "byte_size": byte_size,
        "sha256": sha256,
        "source_root": path_string(source_root)?,
        "manifest_ref": manifest_ref,
        "source_label": source_label,
        "git_blob_sha1": null,
    }))
}

fn read_json(path: &Path, cap: u64) -> Result<Value> {
    let bytes = batch::read_bytes(path, None, false, false, cap)?;
    batch::parse(&bytes)
}

fn entries_from_item_manifest(
    metadata_root: &Path,
    manifest_ref: &str,
    payload_source_root: &Path,
    source_label: &str,
) -> Result<Vec<Value>> {
    let metadata_root = checked_root(metadata_root, true)?;
    let source_root = checked_root(payload_source_root, true)?;
    let path = metadata_path(&metadata_root, manifest_ref)?;
    let manifest = read_json(&path, MAX_METADATA)
        .map_err(|error| format!("cannot load Item manifest: {manifest_ref}: {error}"))?;
    let item_id = text(&manifest, "item_id")?;
    if !valid_item_id(item_id) {
        return Err(format!("invalid Item manifest: {manifest_ref}"));
    }
    let payload_files = manifest
        .get("payload_files")
        .and_then(Value::as_array)
        .filter(|rows| !rows.is_empty())
        .ok_or_else(|| format!("Item manifest payload_files is not a list: {manifest_ref}"))?;
    let item_root_ref = item_root_from_manifest_ref(manifest_ref)?;
    payload_files
        .iter()
        .map(|payload| {
            if !payload.is_object() {
                return Err(format!(
                    "Item manifest payload entry is not an object: {manifest_ref}"
                ));
            }
            entries_from_manifest_payload(
                item_id,
                &item_root_ref,
                manifest_ref,
                &source_root,
                payload,
                source_label,
            )
        })
        .collect()
}

fn entries_from_inventory(
    inventory_path: &Path,
    rows_key: &str,
    source_label: &str,
    only_present: bool,
) -> Result<Vec<Value>> {
    let inventory_path = checked_file_path(inventory_path, "custody inventory")?;
    let inventory = read_json(&inventory_path, MAX_METADATA).map_err(|error| {
        format!(
            "cannot load custody inventory: {}: {error}",
            inventory_path.display()
        )
    })?;
    let rows = inventory
        .get(rows_key)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "inventory does not contain a list at {rows_key}: {}",
                inventory_path.display()
            )
        })?;
    let mut entries = Vec::new();
    for row in rows {
        if !row.is_object() {
            return Err(format!(
                "inventory row is not an object: {}",
                inventory_path.display()
            ));
        }
        if only_present && row.get("source_present") != Some(&Value::Bool(true)) {
            continue;
        }
        let source_root = row
            .get("source_root")
            .and_then(Value::as_str)
            .or_else(|| inventory.get("metadata_root").and_then(Value::as_str))
            .ok_or("inventory row lacks source_root and inventory lacks metadata_root")?;
        let source_root = PathBuf::from(source_root);
        if !source_root.is_absolute() {
            return Err("inventory source_root must be an absolute path".into());
        }
        let source_root = checked_root(&source_root, true)?;
        let manifest_ref = text(row, "manifest_ref")?;
        let item_id = text(row, "item_ref")?;
        if !item_id.starts_with("tos.item.") {
            return Err("inventory item_ref is invalid".into());
        }
        let file_id = text(row, "file_ref")?;
        let digest = text(row, "sha256")?;
        validate_digest_shape(Some(digest), "inventory sha256")?;
        if file_id != format!("tos.file.sha256.{digest}") {
            return Err("inventory file_ref is not bound to sha256".into());
        }
        let relative_ref = text(row, "relative_ref")?;
        let parts = safe_parts(relative_ref, "inventory relative_ref")?;
        if parts.len() < 4 || parts[0] != "ToS" || parts[1] != "source-witnesses" {
            return Err("inventory relative_ref is outside an Item payload".into());
        }
        let payload_index = parts
            .iter()
            .position(|part| part == "payload")
            .ok_or("inventory relative_ref is outside an Item payload")?;
        if payload_index < 3 || payload_index + 1 >= parts.len() {
            return Err("inventory relative_ref is outside an Item payload".into());
        }
        let item_root_ref = parts[..payload_index].join("/");
        let relative_path = parts[payload_index..].join("/");
        let raw_size = row
            .get("byte_size")
            .ok_or("inventory row lacks byte_size")?;
        let byte_size = integer(raw_size, "inventory byte_size")?;
        let source_payload_root = if source_root.join(SOURCE_PREFIX).is_dir() {
            checked_root(&source_root.join(SOURCE_PREFIX), true)?
        } else {
            source_root
        };
        entries.push(json!({
            "item_id": item_id,
            "file_id": file_id,
            "item_root_ref": item_root_ref,
            "relative_path": relative_path,
            "byte_size": byte_size,
            "sha256": digest,
            "source_root": path_string(&source_payload_root)?,
            "manifest_ref": manifest_ref,
            "source_label": source_label,
            "git_blob_sha1": null,
        }));
    }
    Ok(entries)
}

fn checked_file_path(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(format!("invalid {label}: {}", path.display()));
    }
    let meta =
        fs::symlink_metadata(path).map_err(|_| format!("invalid {label}: {}", path.display()))?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(format!("invalid {label}: {}", path.display()));
    }
    // The no-follow reader below remains authoritative for bytes and inode
    // identity. This check supplies the path-shape error used by the API.
    Ok(path.to_owned())
}

fn entries_from_registry_manifest(
    manifest_path: &Path,
    source_root: &Path,
    metadata_root: Option<&Path>,
    source_label: &str,
) -> Result<(Vec<Value>, Vec<Value>)> {
    let manifest_path = checked_file_path(manifest_path, "registry manifest")?;
    let source_root = checked_root(source_root, true)?;
    let metadata_base = checked_root(metadata_root.unwrap_or(&source_root), true)?;
    let manifest = read_json(&manifest_path, MAX_METADATA).map_err(|error| {
        format!(
            "cannot load registry manifest: {}: {error}",
            manifest_path.display()
        )
    })?;
    let targets = manifest
        .get("targets")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "registry manifest lacks targets: {}",
                manifest_path.display()
            )
        })?;
    let manifest_ref = manifest_path
        .strip_prefix(&metadata_base)
        .ok()
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| {
            manifest_path
                .file_name()
                .map(|v| v.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
    let mut entries = Vec::new();
    let mut missing = Vec::new();
    for target in targets {
        let ids = target
            .get("ids")
            .and_then(Value::as_object)
            .ok_or("registry target identity/files are incomplete")?;
        let paths = target
            .get("paths")
            .and_then(Value::as_object)
            .ok_or("registry target identity/files are incomplete")?;
        let item_id = ids
            .get("item")
            .and_then(Value::as_str)
            .ok_or("registry target identity/files are incomplete")?;
        let item_root_ref = paths
            .get("item_root")
            .and_then(Value::as_str)
            .ok_or("registry target identity/files are incomplete")?;
        let files = target
            .get("files")
            .and_then(Value::as_array)
            .ok_or("registry target identity/files are incomplete")?;
        for file in files {
            let basename =
                text(file, "basename").map_err(|_| "registry file identity is incomplete")?;
            safe_parts(basename, "registry basename")?;
            let relative_path = format!("payload/{basename}");
            let candidate = payload_path(&source_root, item_root_ref, &relative_path)?;
            let metadata = fs::symlink_metadata(&candidate);
            let present = metadata
                .as_ref()
                .is_ok_and(|value| !value.file_type().is_symlink() && value.is_file());
            let row = json!({
                "source_label": source_label,
                "manifest_ref": manifest_ref,
                "item_id": item_id,
                "item_root_ref": item_root_ref,
                "relative_path": relative_path,
                "byte_size": file.get("byte_size").cloned().unwrap_or(Value::Null),
                "git_blob_sha1": file.get("git_blob_sha1").cloned().unwrap_or(Value::Null),
                "status": if present { "source_present" } else { "missing" },
            });
            if !present {
                missing.push(row);
                continue;
            }
            let byte_size = integer(
                file.get("byte_size")
                    .ok_or("registry file has invalid byte_size")?,
                "registry byte_size",
            )?;
            let git_blob_sha1 = text(file, "git_blob_sha1")
                .map_err(|_| "registry file has invalid Git blob SHA-1")?;
            if git_blob_sha1.len() != 40 {
                return Err("registry file has invalid Git blob SHA-1".into());
            }
            let item_manifest_ref = format!("{item_root_ref}/item.manifest.json");
            let item_manifest_path = metadata_path(&metadata_base, &item_manifest_ref)?;
            let item_manifest_exists = fs::symlink_metadata(&item_manifest_path)
                .is_ok_and(|meta| meta.is_file() && !meta.file_type().is_symlink());
            let mut file_id = None;
            let mut sha256 = None;
            if item_manifest_exists {
                let bound = entries_from_item_manifest(
                    &metadata_base,
                    &item_manifest_ref,
                    &source_root,
                    source_label,
                )?;
                let matching = bound
                    .iter()
                    .filter(|entry| {
                        entry.get("relative_path").and_then(Value::as_str) == Some(&relative_path)
                    })
                    .collect::<Vec<_>>();
                if matching.len() != 1 {
                    return Err(format!(
                        "registry file is not uniquely present in its Item manifest: {item_manifest_ref}"
                    ));
                }
                if integer(
                    matching[0]
                        .get("byte_size")
                        .ok_or("Item manifest byte_size missing")?,
                    "Item manifest byte_size",
                )? != byte_size
                {
                    return Err(format!(
                        "registry and Item manifest sizes differ: {item_manifest_ref}"
                    ));
                }
                file_id = matching[0].get("file_id").and_then(Value::as_str);
                sha256 = matching[0].get("sha256").and_then(Value::as_str);
            }
            entries.push(json!({
                "item_id": item_id,
                "file_id": file_id,
                "item_root_ref": item_root_ref,
                "relative_path": relative_path,
                "byte_size": byte_size,
                "sha256": sha256,
                "source_root": path_string(&source_root)?,
                "manifest_ref": manifest_ref,
                "source_label": source_label,
                "git_blob_sha1": git_blob_sha1,
            }));
        }
    }
    Ok((entries, missing))
}

fn identity(meta: &Metadata) -> (u64, u64, u64, u32, u32, u64, i64, i64, i64, i64) {
    (
        StdMetadataExt::dev(meta),
        StdMetadataExt::ino(meta),
        StdMetadataExt::len(meta),
        StdMetadataExt::mode(meta),
        StdMetadataExt::uid(meta),
        StdMetadataExt::nlink(meta),
        StdMetadataExt::mtime(meta),
        StdMetadataExt::mtime_nsec(meta),
        StdMetadataExt::ctime(meta),
        StdMetadataExt::ctime_nsec(meta),
    )
}

fn read_file(
    path: &Path,
    expected_mode: Option<u32>,
    expected_owner_uid: Option<u32>,
    require_single_link: bool,
    custody_root: Option<&Path>,
) -> Result<(Vec<u8>, Value)> {
    if !path.is_absolute() {
        return Err("payload path must be absolute".into());
    }
    if let Some(root) = custody_root {
        let root = checked_root(root, true)?;
        return batch::read_bytes_under(
            &root,
            path,
            expected_mode,
            expected_owner_uid,
            require_single_link,
            MAX_PAYLOAD,
        );
    }
    let expected_owner = expected_owner_uid;
    let effective_uid = rustix::process::geteuid().as_raw();
    let bytes = if expected_owner.is_none() || expected_owner == Some(effective_uid) {
        batch::read_bytes(
            path,
            expected_mode,
            expected_owner.is_some(),
            require_single_link,
            MAX_PAYLOAD,
        )?
    } else {
        // The common path uses the shared acquisition FD reader. This narrow
        // compatibility branch retains the historical arbitrary-UID query.
        let mut file = tos_fd_open::open_absolute_regular(path, MAX_PAYLOAD).map_err(|error| {
            format!(
                "cannot open payload without following symlinks: {}: {error}",
                path.display()
            )
        })?;
        let before = file.metadata().map_err(|error| error.to_string())?;
        if expected_mode.is_some_and(|mode| StdMetadataExt::mode(&before) & 0o7777 != mode)
            || expected_owner.is_some_and(|uid| StdMetadataExt::uid(&before) != uid)
            || require_single_link && StdMetadataExt::nlink(&before) != 1
        {
            return Err(format!(
                "payload mode, owner or hard-link conflict: {}",
                path.display()
            ));
        }
        let mut bytes = Vec::new();
        file.by_ref()
            .take(MAX_PAYLOAD + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        let after = file.metadata().map_err(|error| error.to_string())?;
        let current = tos_fd_open::open_absolute_regular(path, MAX_PAYLOAD)
            .map_err(|error| error.to_string())?
            .metadata()
            .map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_PAYLOAD
            || identity(&before) != identity(&after)
            || identity(&after) != identity(&current)
        {
            return Err(format!("file changed during readback: {}", path.display()));
        }
        bytes
    };
    let digest = file_digest(&bytes);
    Ok((bytes, digest))
}

fn file_digest(bytes: &[u8]) -> Value {
    let sha256 = Digest256::of_bytes(bytes)
        .to_prefixed()
        .trim_start_matches("sha256:")
        .to_owned();
    let mut blob = Sha1::new();
    blob.update(format!("blob {}\0", bytes.len()).as_bytes());
    blob.update(bytes);
    json!({
        "byte_size": bytes.len(),
        "sha256": sha256,
        "git_blob_sha1": format!("{:x}", blob.finalize()),
    })
}

fn digest_path(
    path: &Path,
    expected_mode: Option<u32>,
    expected_owner_uid: Option<u32>,
    require_single_link: bool,
    custody_root: Option<&Path>,
) -> Result<Value> {
    if let Some(root) = custody_root {
        let root = checked_root(root, true)?;
        return batch::digest_file_under(
            &root,
            path,
            expected_mode,
            expected_owner_uid,
            require_single_link,
            MAX_PAYLOAD,
        );
    }
    let effective_uid = rustix::process::geteuid().as_raw();
    let result = if expected_owner_uid.is_none() || expected_owner_uid == Some(effective_uid) {
        batch::digest_file(
            path,
            expected_mode,
            expected_owner_uid.is_some(),
            require_single_link,
            MAX_PAYLOAD,
        )?
    } else {
        let (_bytes, digest) = read_file(
            path,
            expected_mode,
            expected_owner_uid,
            require_single_link,
            None,
        )?;
        digest
    };
    Ok(result)
}

fn digest_value(value: &Value) -> Result<Value> {
    Ok(json!({
        "byte_size": integer(value.get("byte_size").ok_or("missing digest byte_size")?, "digest byte_size")?,
        "sha256": text(value, "sha256")?,
        "git_blob_sha1": text(value, "git_blob_sha1")?,
    }))
}

fn digest_equal(left: &Value, right: &Value) -> bool {
    left.get("byte_size") == right.get("byte_size")
        && left.get("sha256") == right.get("sha256")
        && left.get("git_blob_sha1") == right.get("git_blob_sha1")
}

fn read_entry(entry: &Value, source_path: Option<&Path>) -> Result<(PathBuf, Vec<u8>, Value)> {
    let entry = entry_value(entry)?;
    let source_root = path(&entry, "source_root")?;
    let selected_path = match source_path {
        Some(path) => path.to_owned(),
        None => payload_path(
            &source_root,
            text(&entry, "item_root_ref")?,
            text(&entry, "relative_path")?,
        )?,
    };
    let (bytes, digest) = read_file(&selected_path, None, None, false, Some(&source_root))
        .map_err(|error| format!("source payload is invalid: {error}"))?;
    if digest["byte_size"] != entry["byte_size"] {
        return Err(format!(
            "source byte size differs for {}",
            text(&entry, "relative_path")?
        ));
    }
    if entry
        .get("sha256")
        .and_then(Value::as_str)
        .is_some_and(|expected| digest["sha256"] != expected)
    {
        return Err(format!(
            "source SHA-256 differs for {}",
            text(&entry, "relative_path")?
        ));
    }
    if entry
        .get("git_blob_sha1")
        .and_then(Value::as_str)
        .is_some_and(|expected| digest["git_blob_sha1"] != expected)
    {
        return Err(format!(
            "source Git blob SHA-1 differs for {}",
            text(&entry, "relative_path")?
        ));
    }
    if let Some(expected) = entry.get("file_id").and_then(Value::as_str) {
        if expected
            != format!(
                "tos.file.sha256.{}",
                digest["sha256"].as_str().unwrap_or_default()
            )
        {
            return Err(format!(
                "source File ID differs for {}",
                text(&entry, "relative_path")?
            ));
        }
    }
    Ok((selected_path, bytes, digest))
}

fn destination_path(destination_root: &Path, entry: &Value) -> Result<PathBuf> {
    let entry = entry_value(entry)?;
    payload_path(
        destination_root,
        text(&entry, "item_root_ref")?,
        text(&entry, "relative_path")?,
    )
}

fn destination_git_posture(repo_root: &Path, entry: &Value) -> Result<(bool, bool)> {
    let root = checked_root(repo_root, true)?;
    let entry = entry_value(entry)?;
    let relative = format!(
        "{}/{}",
        text(&entry, "item_root_ref")?,
        text(&entry, "relative_path")?
    );
    let ignored = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["check-ignore", "--quiet", "--"])
        .arg(&relative)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("cannot inspect Git ignore posture: {error}"))?
        .success();
    let tracked = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["ls-files", "--error-unmatch", "--"])
        .arg(&relative)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("cannot inspect Git tracking posture: {error}"))?
        .success();
    Ok((ignored, tracked))
}

fn custody_row(
    entry: &Value,
    status: &str,
    digest: Option<&Value>,
    reason: Option<&str>,
) -> Result<Value> {
    let entry = entry_value(entry)?;
    let digest_sha = digest
        .and_then(|value| value.get("sha256"))
        .and_then(Value::as_str);
    let digest_file_id = digest_sha.map(|value| format!("tos.file.sha256.{value}"));
    let mut row = json!({
        "item_id": entry["item_id"],
        "file_id": entry.get("file_id").and_then(Value::as_str).map(str::to_owned).or(digest_file_id),
        "manifest_ref": entry["manifest_ref"],
        "item_root_ref": entry["item_root_ref"],
        "relative_path": entry["relative_path"],
        "destination_ref": format!("{}/{}", entry["item_root_ref"].as_str().unwrap_or_default(), entry["relative_path"].as_str().unwrap_or_default()),
        "source_label": entry["source_label"],
        "expected_byte_size": entry["byte_size"],
        "expected_sha256": entry.get("sha256").and_then(Value::as_str).or(digest_sha),
        "status": status,
    });
    if let Some(git_blob_sha1) = entry.get("git_blob_sha1").and_then(Value::as_str) {
        row["expected_git_blob_sha1"] = json!(git_blob_sha1);
    }
    if let Some(digest) = digest {
        row["byte_size"] = digest_value(digest)?["byte_size"].clone();
        row["sha256"] = json!(text(digest, "sha256")?);
        row["git_blob_sha1"] = json!(text(digest, "git_blob_sha1")?);
    }
    if let Some(reason) = reason {
        if !reason.is_empty() {
            row["reason"] = json!(reason);
        }
    }
    Ok(row)
}

fn deduplicate_entries(entries: &[Value]) -> Result<(Vec<Value>, Vec<Value>)> {
    let mut unique: Vec<Value> = Vec::new();
    let mut duplicates: Vec<Value> = Vec::new();
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    for raw in entries {
        let entry = entry_value(raw)?;
        let item_id = text(&entry, "item_id")?.to_owned();
        let file_id = entry
            .get("file_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "path:{}",
                    text(&entry, "item_root_ref").unwrap_or("").to_owned()
                        + "/"
                        + text(&entry, "relative_path").unwrap_or("")
                )
            });
        let key = (item_id.clone(), file_id.clone());
        if let Some(index) = seen.get(&key).copied() {
            let previous = &unique[index];
            if previous["item_root_ref"] != entry["item_root_ref"]
                || previous["relative_path"] != entry["relative_path"]
                || previous["byte_size"] != entry["byte_size"]
                || previous["sha256"] != entry["sha256"]
            {
                return Err(format!(
                    "Item/File duplicate has conflicting identity: {item_id} / {file_id}"
                ));
            }
            duplicates.push(json!({
                "item_id": item_id,
                "file_id": entry.get("file_id").cloned().unwrap_or(Value::Null),
                "kept_source": previous["source_label"],
                "duplicate_source": entry["source_label"],
                "destination_ref": format!("{}/{}", entry["item_root_ref"].as_str().unwrap_or_default(), entry["relative_path"].as_str().unwrap_or_default()),
                "status": "deduplicated",
            }));
            continue;
        }
        seen.insert(key, unique.len());
        unique.push(entry);
    }
    Ok((unique, duplicates))
}

fn same_file(path: &Path, expected: &Value, posture: bool, root: Option<&Path>) -> bool {
    let (mode, owner, single) = if posture {
        (Some(0o444), Some(rustix::process::geteuid().as_raw()), true)
    } else {
        (None, None, false)
    };
    digest_path(path, mode, owner, single, root)
        .map(|actual| digest_equal(&actual, expected))
        .unwrap_or(false)
}

fn plan_entries(
    raw_entries: &[Value],
    destination_payload_root: Option<&Path>,
    destination_repo_root: Option<&Path>,
) -> Result<(Vec<Value>, Vec<Value>)> {
    let (entries, duplicates) = deduplicate_entries(raw_entries)?;
    let destination_root = destination_payload_root
        .map(|root| checked_root(root, true))
        .transpose()?;
    let destination_repo = destination_repo_root
        .map(|root| checked_root(root, true))
        .transpose()?;
    if destination_repo.is_some() && destination_root.is_none() {
        return Err("destination_repo_root requires destination_payload_root".into());
    }
    let mut rows = Vec::new();
    for entry in entries {
        let (_source, _bytes, digest) = match read_entry(&entry, None) {
            Ok(value) => value,
            Err(reason) => {
                rows.push(custody_row(&entry, "source_invalid", None, Some(&reason))?);
                continue;
            }
        };
        let mut status = "source_verified";
        let mut reason = None;
        if let (Some(destination_root), Some(repo_root)) = (&destination_root, &destination_repo) {
            let (ignored, tracked) = destination_git_posture(repo_root, &entry)?;
            if !ignored || tracked {
                rows.push(custody_row(
                    &entry,
                    "destination_policy_error",
                    Some(&digest),
                    Some("payload path is not ignored or is Git-tracked"),
                )?);
                continue;
            }
            let destination = destination_path(destination_root, &entry)?;
            if fs::symlink_metadata(&destination).is_ok() {
                if fs::symlink_metadata(&destination)
                    .is_ok_and(|meta| meta.file_type().is_symlink())
                {
                    status = "conflict";
                    reason = Some("destination is a symlink");
                } else if same_file(&destination, &digest, true, Some(destination_root)) {
                    status = "already_present";
                } else {
                    status = "conflict";
                    reason = Some("destination has different bytes");
                }
            } else {
                status = "planned_copy";
            }
        } else if let Some(destination_root) = &destination_root {
            let destination = destination_path(destination_root, &entry)?;
            if fs::symlink_metadata(&destination).is_ok() {
                if fs::symlink_metadata(&destination)
                    .is_ok_and(|meta| meta.file_type().is_symlink())
                {
                    status = "conflict";
                    reason = Some("destination is a symlink");
                } else if same_file(&destination, &digest, true, Some(destination_root)) {
                    status = "already_present";
                } else {
                    status = "conflict";
                    reason = Some("destination has different bytes");
                }
            } else {
                status = "planned_copy";
            }
        }
        rows.push(custody_row(&entry, status, Some(&digest), reason)?);
    }
    Ok((rows, duplicates))
}

fn publish_no_clobber(
    destination: &Path,
    bytes: &[u8],
    expected: &Value,
    custody_root: Option<&Path>,
) -> Result<String> {
    if fs::symlink_metadata(destination).is_ok() {
        if fs::symlink_metadata(destination).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Ok("conflict".into());
        }
        return Ok(if same_file(destination, expected, true, custody_root) {
            "already_present"
        } else {
            "conflict"
        }
        .into());
    }
    if !digest_equal(&file_digest(bytes), expected) {
        return Err(format!(
            "source readback differs before publish: {}",
            destination.display()
        ));
    }
    match batch::publish(destination, bytes, 0o444) {
        Ok(status) => Ok(status.to_owned()),
        Err(error) if fs::symlink_metadata(destination).is_ok() => {
            if fs::symlink_metadata(destination).is_ok_and(|meta| meta.file_type().is_symlink()) {
                Ok("conflict".into())
            } else if same_file(destination, expected, true, custody_root) {
                Ok("already_present".into())
            } else {
                let _ = error;
                Ok("conflict".into())
            }
        }
        Err(error) => Err(error),
    }
}

fn copy_entries(
    raw_entries: &[Value],
    destination_payload_root: &Path,
    destination_repo_root: Option<&Path>,
) -> Result<(Vec<Value>, Vec<Value>)> {
    let destination_root = checked_root(destination_payload_root, true)?;
    let (entries, duplicates) = deduplicate_entries(raw_entries)?;
    let destination_repo = destination_repo_root
        .map(|root| checked_root(root, true))
        .transpose()?;
    let mut rows = Vec::new();
    for entry in entries {
        let result = (|| {
            let (source, bytes, digest) = read_entry(&entry, None)?;
            if let Some(repo_root) = &destination_repo {
                let (ignored, tracked) = destination_git_posture(repo_root, &entry)?;
                if !ignored || tracked {
                    return Err("payload path is not ignored or is Git-tracked".into());
                }
            }
            let destination = destination_path(&destination_root, &entry)?;
            let status =
                publish_no_clobber(&destination, &bytes, &digest, Some(&destination_root))?;
            let _ = source;
            Ok::<(String, Value), String>((status, digest))
        })();
        match result {
            Ok((status, digest)) => rows.push(custody_row(&entry, &status, Some(&digest), None)?),
            Err(reason) if reason == "payload path is not ignored or is Git-tracked" => {
                rows.push(custody_row(&entry, "failed", None, Some(&reason))?);
            }
            Err(reason) => rows.push(custody_row(&entry, "failed", None, Some(&reason))?),
        }
    }
    Ok((rows, duplicates))
}

fn contains_absolute_path(value: &Value) -> bool {
    match value {
        Value::String(text) => Path::new(text).is_absolute(),
        Value::Array(values) => values.iter().any(contains_absolute_path),
        Value::Object(values) => values.values().any(contains_absolute_path),
        _ => false,
    }
}

fn write_receipt(request: &Value) -> Result<Value> {
    let destination = path(request, "path")?;
    let operation = text(request, "operation")?;
    let rows = request
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("missing rows")?;
    let duplicates = request
        .get("duplicates")
        .and_then(Value::as_array)
        .ok_or("missing duplicates")?;
    let missing = request
        .get("missing")
        .and_then(Value::as_array)
        .ok_or("missing missing rows")?;
    let inputs = request
        .get("inputs")
        .and_then(Value::as_array)
        .ok_or("missing inputs")?;
    if contains_absolute_path(
        &json!({"rows": rows, "duplicates": duplicates, "missing": missing, "inputs": inputs}),
    ) {
        return Err("custody receipt cannot contain absolute host paths".into());
    }
    let sum_expected = rows
        .iter()
        .filter_map(|row| row.get("expected_byte_size").and_then(Value::as_u64))
        .fold(0u64, u64::saturating_add);
    let sum_copied = rows
        .iter()
        .filter(|row| {
            matches!(
                row.get("status").and_then(Value::as_str),
                Some("copied" | "already_present")
            )
        })
        .filter_map(|row| row.get("byte_size").and_then(Value::as_u64))
        .fold(0u64, u64::saturating_add);
    let count = |status: &str| {
        rows.iter()
            .filter(|row| row.get("status").and_then(Value::as_str) == Some(status))
            .count()
    };
    let failed = rows
        .iter()
        .filter(|row| {
            matches!(
                row.get("status").and_then(Value::as_str),
                Some("failed" | "source_invalid" | "destination_policy_error")
            )
        })
        .count();
    let created_at =
        source_serialization::instant().map_err(|error| format!("clock: {error:?}"))?;
    let receipt = json!({
        "schema_version": "tos.source_payload_custody_receipt.v1",
        "operation": operation,
        "created_at": created_at,
        "inputs": inputs,
        "counts": {
            "unique": rows.len(),
            "copied": count("copied"),
            "already_present": count("already_present"),
            "planned_copy": count("planned_copy"),
            "source_verified": count("source_verified"),
            "conflict": count("conflict"),
            "destination_policy_error": count("destination_policy_error"),
            "failed": failed,
            "missing": missing.len(),
            "deduplicated": duplicates.len(),
        },
        "bytes": {
            "expected_unique": sum_expected,
            "copied_or_present": sum_copied,
        },
        "rows": rows,
        "duplicates": duplicates,
        "missing": missing,
        "authority_boundary": "This receipt records the selected payload custody operation and its mechanical fixity checks.",
    });
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(format!(
            "receipt already exists; choose a new immutable receipt path: {}",
            destination.display()
        ));
    }
    let mut body = serde_json::to_vec_pretty(&receipt).map_err(|error| error.to_string())?;
    body.push(b'\n');
    match batch::publish(&destination, &body, 0o600)? {
        "already_present" => Err(format!(
            "receipt already exists; choose a new immutable receipt path: {}",
            destination.display()
        )),
        _ => Ok(json!({"path": path_string(&destination)?})),
    }
}

fn cli(request: &Value) -> Result<Value> {
    let command = text(request, "command")?;
    let source_root = path(request, "payload_source_root")?;
    let metadata_root = optional_path(request, "metadata_root")?;
    let item_manifests = request
        .get("item_manifests")
        .and_then(Value::as_array)
        .ok_or("missing item_manifests")?;
    let inventories = request
        .get("inventories")
        .and_then(Value::as_array)
        .ok_or("missing inventories")?;
    let rows_key = request
        .get("inventory_rows_key")
        .and_then(Value::as_str)
        .unwrap_or("files");
    let mut entries = Vec::new();
    for manifest_ref in item_manifests {
        let manifest_ref = manifest_ref
            .as_str()
            .ok_or("item manifest ref must be text")?;
        let metadata_root = metadata_root
            .as_deref()
            .ok_or("--metadata-root is required with --item-manifest")?;
        entries.extend(entries_from_item_manifest(
            metadata_root,
            manifest_ref,
            &source_root,
            "item-manifest",
        )?);
    }
    let mut missing = Vec::new();
    for inventory in inventories {
        let inventory_path =
            PathBuf::from(inventory.as_str().ok_or("inventory path must be text")?);
        entries.extend(entries_from_inventory(
            &inventory_path,
            rows_key,
            "inventory",
            false,
        )?);
    }
    if entries.is_empty() {
        return Err("at least one --item-manifest or --inventory is required".into());
    }
    let destination = optional_path(request, "destination_payload_root")?;
    let (rows, duplicates) = if command == "verify" {
        plan_entries(&entries, destination.as_deref(), None)?
    } else if command == "copy" {
        let destination = destination
            .as_deref()
            .ok_or("--destination-payload-root is required for copy")?;
        copy_entries(&entries, destination, None)?
    } else {
        return Err("unsupported custody CLI command".into());
    };
    let receipt = path(request, "receipt")?;
    write_receipt(&json!({
        "path": path_string(&receipt)?,
        "operation": command,
        "rows": rows.clone(),
        "duplicates": duplicates.clone(),
        "missing": missing,
        "inputs": [],
    }))?;
    let rejected = rows.iter().any(|row| {
        matches!(
            row.get("status").and_then(Value::as_str),
            Some("failed" | "source_invalid" | "destination_policy_error" | "conflict")
        )
    });
    Ok(json!({
        "status": "completed",
        "rows": rows.len(),
        "duplicates": duplicates.len(),
        "receipt": receipt,
        "exit_code": if rejected { 1 } else { 0 },
    }))
}

/// Invoke one compatibility operation selected by the typed wire request.
pub fn invoke(request: &Value) -> Result<Value> {
    let operation = text(request, "operation")?;
    match operation {
        "safe_ref" => {
            let reference = text(request, "ref")?;
            batch::safe_ref(reference).map_err(|_| {
                format!("invalid {}", text(request, "label").unwrap_or("reference"))
            })?;
            Ok(json!({"ref": reference}))
        }
        "checked_root" => {
            let root = path(request, "root")?;
            let must_exist = bool_value(request, "must_exist", true)?;
            Ok(json!({"path": path_string(&checked_root(&root, must_exist)?)?}))
        }
        "checked_child" => {
            let root = path(request, "root")?;
            let parts = request
                .get("parts")
                .and_then(Value::as_array)
                .ok_or("parts must be an array")?
                .iter()
                .map(|part| {
                    part.as_str()
                        .map(str::to_owned)
                        .ok_or("path part must be text")
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(json!({"path": path_string(&checked_child(&root, &parts)?)?}))
        }
        "payload_path" => {
            let root = path(request, "payload_source_root")?;
            Ok(
                json!({"path": path_string(&payload_path(&root, text(request, "item_root_ref")?, text(request, "relative_path")?)?)?}),
            )
        }
        "metadata_path" => {
            let root = path(request, "metadata_root")?;
            Ok(
                json!({"path": path_string(&metadata_path(&root, text(request, "manifest_ref")?)?)?}),
            )
        }
        "item_root_from_manifest_ref" => Ok(
            json!({"item_root_ref": item_root_from_manifest_ref(text(request, "manifest_ref")?)?}),
        ),
        "validate_digest_shape" => {
            let value = match request.get("value") {
                None | Some(Value::Null) => None,
                Some(Value::String(value)) => Some(value.as_str()),
                _ => return Err(format!("invalid {}", text(request, "label")?)),
            };
            validate_digest_shape(value, text(request, "label")?)?;
            Ok(json!({}))
        }
        "entry_from_manifest_payload" => {
            let source_root = path(request, "payload_source_root")?;
            let payload = request.get("payload").ok_or("missing payload")?;
            Ok(json!({"entry": entries_from_manifest_payload(
                text(request, "item_id")?,
                text(request, "item_root_ref")?,
                text(request, "manifest_ref")?,
                &source_root,
                payload,
                text(request, "source_label")?,
            )?}))
        }
        "entries_from_item_manifest" => {
            let metadata_root = path(request, "metadata_root")?;
            let source_root = path(request, "payload_source_root")?;
            let rows = entries_from_item_manifest(
                &metadata_root,
                text(request, "manifest_ref")?,
                &source_root,
                text(request, "source_label")?,
            )?;
            Ok(json!({"entries": rows}))
        }
        "entries_from_inventory" => {
            let path = path(request, "inventory_path")?;
            let rows = entries_from_inventory(
                &path,
                text(request, "rows_key")?,
                text(request, "source_label")?,
                bool_value(request, "only_present", false)?,
            )?;
            Ok(json!({"entries": rows}))
        }
        "entries_from_registry_manifest" => {
            let manifest_path = path(request, "manifest_path")?;
            let source_root = path(request, "source_root")?;
            let metadata_root = optional_path(request, "metadata_root")?;
            let (entries, missing) = entries_from_registry_manifest(
                &manifest_path,
                &source_root,
                metadata_root.as_deref(),
                text(request, "source_label")?,
            )?;
            Ok(json!({"entries": entries, "missing": missing}))
        }
        "digest_file" => {
            let path = path(request, "path")?;
            let mode = batch::optional_u64(request, "expected_mode")?
                .map(|mode| u32::try_from(mode).map_err(|_| "expected_mode exceeds u32"))
                .transpose()?;
            let owner = batch::optional_u64(request, "expected_owner_uid")?
                .map(|uid| u32::try_from(uid).map_err(|_| "expected_owner_uid exceeds u32"))
                .transpose()?;
            let single = bool_value(request, "require_single_link", false)?;
            let root = optional_path(request, "custody_root")?;
            Ok(json!({"digest": digest_path(&path, mode, owner, single, root.as_deref())?}))
        }
        "verify_entry" => {
            let entry = request.get("entry").ok_or("missing entry")?;
            let source_path = optional_path(request, "source_path")?;
            let (_path, _bytes, digest) = read_entry(entry, source_path.as_deref())?;
            Ok(json!({"digest": digest}))
        }
        "publish_bytes_no_clobber" => {
            let destination = path(request, "destination")?;
            if !bool_value(request, "fetch_callback", false)? {
                return Err(
                    "publish_bytes_no_clobber requires the bounded caller-byte callback".into(),
                );
            }
            let requested_size = integer(
                request.get("body_size").ok_or("missing body_size")?,
                "body_size",
            )?;
            if requested_size > MAX_PAYLOAD {
                return Err("payload body exceeds custody limit".into());
            }
            let expected = digest_value(request.get("expected").ok_or("missing expected digest")?)?;
            let bytes = batch::fetch_request(&json!({
                "byte_size": requested_size,
                "sha256": expected["sha256"],
            }))?;
            if bytes.len() as u64 != requested_size {
                return Err("caller payload byte size differs".into());
            }
            let root = optional_path(request, "custody_root")?;
            if let Some(root) = &root {
                let root = checked_root(root, true)?;
                let relative = relative_path_under(&root, &destination)?;
                batch::path_under(&root, &relative)?;
            }
            Ok(
                json!({"status": publish_no_clobber(&destination, &bytes, &expected, root.as_deref())?}),
            )
        }
        "destination_path" => {
            let root = path(request, "destination_payload_root")?;
            Ok(
                json!({"path": path_string(&destination_path(&root, request.get("entry").ok_or("missing entry")?)?)?}),
            )
        }
        "destination_git_posture" => {
            let root = path(request, "repo_root")?;
            let (ignored, tracked) =
                destination_git_posture(&root, request.get("entry").ok_or("missing entry")?)?;
            Ok(json!({"ignored": ignored, "tracked": tracked}))
        }
        "custody_row" => {
            let digest = request.get("digest").filter(|value| !value.is_null());
            Ok(json!({"row": custody_row(
                request.get("entry").ok_or("missing entry")?,
                text(request, "status")?,
                digest,
                request.get("reason").and_then(Value::as_str),
            )?}))
        }
        "deduplicate_entries" => {
            let entries = request
                .get("entries")
                .and_then(Value::as_array)
                .ok_or("entries must be an array")?;
            let (entries, duplicates) = deduplicate_entries(entries)?;
            Ok(json!({"entries": entries, "duplicates": duplicates}))
        }
        "plan_entries" => {
            let entries = request
                .get("entries")
                .and_then(Value::as_array)
                .ok_or("entries must be an array")?;
            let destination_root = optional_path(request, "destination_payload_root")?;
            let repo_root = optional_path(request, "destination_repo_root")?;
            let (rows, duplicates) =
                plan_entries(entries, destination_root.as_deref(), repo_root.as_deref())?;
            Ok(json!({"rows": rows, "duplicates": duplicates}))
        }
        "copy_entries" => {
            let entries = request
                .get("entries")
                .and_then(Value::as_array)
                .ok_or("entries must be an array")?;
            let destination_root = path(request, "destination_payload_root")?;
            let repo_root = optional_path(request, "destination_repo_root")?;
            let (rows, duplicates) =
                copy_entries(entries, &destination_root, repo_root.as_deref())?;
            Ok(json!({"rows": rows, "duplicates": duplicates}))
        }
        "write_receipt" => write_receipt(request),
        "cli" => cli(request),
        _ => Err(format!("unsupported custody operation: {operation}")),
    }
}
