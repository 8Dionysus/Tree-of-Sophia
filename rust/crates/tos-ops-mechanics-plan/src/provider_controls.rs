//! Bounded independent KAG provider controls, subordinate to the source template.
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
};

pub const MAX_BYTES: usize = 64 * 1024;
pub const PATHS: [&str; 9] = [
    "kag/AGENTS.md",
    "kag/README.md",
    "kag/edges/source-return.json",
    "kag/indexes/source-routes.json",
    "kag/manifest.json",
    "kag/nodes/export-route.json",
    "kag/nodes/source-export.json",
    "kag/projections/source-return.json",
    "kag/receipts/publication-route.json",
];
#[derive(Debug, Serialize)]
pub struct Entry {
    pub path: &'static str,
    pub sha256: String,
    pub size_bytes: usize,
}
fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
fn absolute(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}
// Check every component, including intermediate directories: a resolved-equal
// path alone could conceal a linked route whose target happens to be itself.
fn unlinked(path: &Path, missing: bool) -> io::Result<()> {
    let path = absolute(path)?;
    let mut current = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir | Component::CurDir => {
                return Err(invalid("provider path must be explicit and unlinked"));
            }
            _ => current.push(part.as_os_str()),
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(invalid("linked provider path"));
            }
            Ok(_) => (),
            Err(e) if missing && e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
fn read(path: &Path) -> io::Result<Vec<u8>> {
    unlinked(path, false)?;
    if !fs::symlink_metadata(path)?.is_file() {
        return Err(invalid("non-regular provider file"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_BYTES as u64 {
        return Err(invalid(
            "provider file exceeds its bounded byte limit or is non-regular",
        ));
    }
    let mut raw = Vec::new();
    file.take(MAX_BYTES as u64 + 1).read_to_end(&mut raw)?;
    if raw.len() > MAX_BYTES {
        return Err(invalid("provider file exceeds its bounded byte limit"));
    }
    Ok(raw)
}
fn parse(raw: &[u8]) -> io::Result<JsonValue> {
    let limits = JsonLimits {
        max_bytes: MAX_BYTES,
        ..JsonLimits::default()
    };
    Ok(parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| invalid(e.to_string()))?
        .root()
        .clone())
}
fn canonical(value: &JsonValue) -> io::Result<Vec<u8>> {
    canonical_bytes_v1(
        value,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::default(),
    )
    .map_err(|e| invalid(e.to_string()))
}
fn keys(value: &JsonValue, expected: &[&str]) -> bool {
    value.as_object().is_some_and(|items| {
        items.len() == expected.len()
            && items
                .iter()
                .all(|(key, _)| key.as_str().is_some_and(|s| expected.contains(&s)))
    })
}
// Python str.strip also treats U+001C..U+001F as whitespace.
fn blank(value: &str) -> bool {
    value
        .chars()
        .all(|c| c.is_whitespace() || ('\u{001c}'..='\u{001f}').contains(&c))
}
pub fn template(path: &Path) -> io::Result<Vec<(&'static str, Vec<u8>)>> {
    let value = parse(&read(path)?)?;
    canonical(&value)?; // Finite and UTF-8 encodable throughout, before contract selection.
    if !keys(&value, &["schema_version", "files"])
        || value
            .object_get("schema_version")
            .and_then(JsonValue::as_str)
            != Some("tos_kag_provider_template_v1")
    {
        return Err(invalid("provider template has an unexpected file contract"));
    }
    let files = value
        .object_get("files")
        .ok_or_else(|| invalid("missing provider files"))?;
    if !keys(files, &PATHS) {
        return Err(invalid("provider template has an unexpected file contract"));
    }
    let mut output = Vec::with_capacity(PATHS.len());
    let mut total = 0;
    for path in PATHS {
        let value = files
            .object_get(path)
            .ok_or_else(|| invalid("missing provider file"))?;
        let raw = if path.ends_with(".md") {
            let text = value
                .as_str()
                .filter(|s| !blank(s))
                .ok_or_else(|| invalid("provider route card must contain text"))?;
            text.as_bytes().to_vec()
        } else {
            if value.as_object().is_none() {
                return Err(invalid("provider control must be a JSON object"));
            }
            canonical(value)?
        };
        total += raw.len();
        if total > MAX_BYTES {
            return Err(invalid("provider control closure exceeds its byte limit"));
        }
        output.push((path, raw));
    }
    Ok(output)
}
pub fn verify(root: &Path, entries: &JsonValue) -> io::Result<()> {
    let entries = entries
        .as_array()
        .ok_or_else(|| invalid("provider control membership differs"))?;
    if entries.len() != PATHS.len()
        || entries
            .iter()
            .zip(PATHS)
            .any(|(entry, path)| entry.object_get("path").and_then(JsonValue::as_str) != Some(path))
    {
        return Err(invalid("provider control membership differs"));
    }
    for (entry, path) in entries.iter().zip(PATHS) {
        let raw = read(&root.join(path))?;
        let size = entry.object_get("size_bytes").and_then(JsonValue::as_u64);
        let digest = entry.object_get("sha256").and_then(JsonValue::as_str);
        if size != Some(raw.len() as u64)
            || digest != Some(Digest256::of_bytes(&raw).to_hex().as_str())
        {
            return Err(invalid(format!(
                "consumer changed a provider control: {path}"
            )));
        }
    }
    Ok(())
}
pub fn verify_request(root: &Path, raw: &[u8]) -> io::Result<()> {
    verify(root, &parse(raw)?)
}
pub fn materialize(root: &Path, template_path: &Path) -> io::Result<Vec<Entry>> {
    let root = absolute(root)?;
    unlinked(&root, false)?;
    if !fs::symlink_metadata(&root)?.is_dir() || fs::canonicalize(&root)? != root {
        return Err(invalid(
            "provider output must be an explicit regular directory",
        ));
    }
    let files = template(template_path)?;
    let mut entries = Vec::with_capacity(PATHS.len());
    for (relative, raw) in &files {
        let path = root.join(relative);
        unlinked(&path, true)?;
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
            Ok(_) => return Err(invalid("provider control output must be new")),
        }
        fs::create_dir_all(path.parent().ok_or_else(|| invalid("missing parent"))?)?;
        unlinked(path.parent().unwrap(), false)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        options.open(&path)?.write_all(raw)?;
        entries.push(Entry {
            path: relative,
            sha256: Digest256::of_bytes(raw).to_hex(),
            size_bytes: raw.len(),
        });
    }
    let raw = serde_json::to_vec(&entries).map_err(|e| invalid(e.to_string()))?;
    verify_request(&root, &raw)?;
    if files != template(template_path)? {
        return Err(invalid("provider template changed during materialization"));
    }
    Ok(entries)
}
