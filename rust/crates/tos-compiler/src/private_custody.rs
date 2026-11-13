//! Private custody verifies identity and available bytes, not absent content.
use std::{collections::BTreeSet, io::ErrorKind, path::Path};

use serde_json::Value;
use tos_source_store::execution::Execution;

/// Verify one exact family and return the number of files not hydrated.
pub fn verify_family(
    ctx: &Execution,
    manifest_path: &str,
    expected_names: &[&str],
) -> Result<usize, String> {
    Ok(verify_family_missing(ctx, manifest_path, expected_names)?.len())
}

/// As above, returning the exact absent basenames for bounded partial checks.
pub fn verify_family_missing(
    ctx: &Execution,
    manifest_path: &str,
    expected_names: &[&str],
) -> Result<Vec<String>, String> {
    ctx.tick(1)?;
    let manifest = Path::new(manifest_path);
    let parent = manifest.parent().ok_or("custody manifest parent missing")?;
    let mut directory =
        tos_fd_open::reopen_directory(ctx.root_directory()).map_err(|e| e.to_string())?;
    for part in parent.components() {
        let std::path::Component::Normal(name) = part else {
            return Err("invalid custody manifest path".into());
        };
        directory = tos_fd_open::open_directory_at(&directory, Path::new(name))
            .map_err(|e| e.to_string())?;
    }
    let mut manifest_file = ctx.source_file(manifest_path, 1024 * 1024)?;
    let before = manifest_file.metadata().map_err(|e| e.to_string())?;
    let raw = ctx.read_file(&mut manifest_file, 1024 * 1024)?;
    ctx.verify_file_unchanged(&manifest_file, &before)?;
    let value: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    if value["schema_version"] != "tos_r2_manifest_v1" {
        return Err(format!("invalid private custody manifest: {manifest_path}"));
    }
    let expected: BTreeSet<_> = expected_names.iter().copied().collect();
    if expected.len() != expected_names.len() || expected.is_empty() {
        return Err("invalid custody expected inventory".into());
    }
    let entries = value["entries"]
        .as_array()
        .ok_or("custody entries required")?;
    let mut seen = BTreeSet::new();
    let mut missing = Vec::new();
    for entry in entries {
        ctx.tick(1)?;
        let path = entry["path"].as_str().ok_or("custody path required")?;
        let path = Path::new(path);
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("custody filename required")?;
        let sha = entry["sha256"].as_str().ok_or("custody digest required")?;
        let size = entry["bytes"].as_u64().ok_or("custody size required")?;
        if path.parent() != Some(parent)
            || !expected.contains(name)
            || !seen.insert(name.to_owned())
            || !tos_validation::private_custody::valid_entry(entry)
        {
            return Err(format!("invalid private custody entry: {}", path.display()));
        }
        let mut file = match tos_fd_open::open_regular_at(&directory, Path::new(name)) {
            Ok(file) => file,
            Err(error)
                if error
                    .source
                    .as_ref()
                    .is_some_and(|e| e.kind() == ErrorKind::NotFound) =>
            {
                missing.push(name.to_owned());
                continue;
            }
            Err(error) => return Err(format!("private custody {}: {error}", path.display())),
        };
        let before = file.metadata().map_err(|e| e.to_string())?;
        if before.len() != size || ctx.hash_file(&mut file, size)? != sha {
            return Err(format!(
                "private custody size or SHA-256 mismatch: {}",
                path.display()
            ));
        }
        ctx.verify_file_unchanged(&file, &before)?;
    }
    if seen.len() != expected.len() {
        return Err(format!(
            "private custody inventory incomplete: {manifest_path}"
        ));
    }
    ctx.check()?;
    Ok(missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tos_foundation::Digest256;

    #[test]
    fn missing_present_corrupt_and_incomplete_inventory() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("family")).unwrap();
        let hash = Digest256::of_bytes(b"content").to_hex();
        let manifest = serde_json::json!({"schema_version":"tos_r2_manifest_v1","entries":[{
            "path":"family/a", "sha256":hash, "bytes":7, "bucket":"tos-source-bytes",
            "access":"private", "key":format!("sha256/{hash}"), "source_revision":"a".repeat(40)
        }]});
        fs::write(
            dir.path().join("family/r2-manifest.v1.json"),
            manifest.to_string(),
        )
        .unwrap();
        let ctx = Execution::new(dir.path(), 30).unwrap();
        assert_eq!(
            verify_family(&ctx, "family/r2-manifest.v1.json", &["a"]).unwrap(),
            1
        );
        fs::write(dir.path().join("family/a"), b"content").unwrap();
        assert_eq!(
            verify_family(&ctx, "family/r2-manifest.v1.json", &["a"]).unwrap(),
            0
        );
        fs::write(dir.path().join("family/a"), b"corrupt").unwrap();
        assert!(verify_family(&ctx, "family/r2-manifest.v1.json", &["a"]).is_err());
        assert!(verify_family(&ctx, "family/r2-manifest.v1.json", &["a", "b"]).is_err());
        fs::remove_file(dir.path().join("family/a")).unwrap();
        std::os::unix::fs::symlink("missing", dir.path().join("family/a")).unwrap();
        assert!(verify_family(&ctx, "family/r2-manifest.v1.json", &["a"]).is_err());
    }
}
