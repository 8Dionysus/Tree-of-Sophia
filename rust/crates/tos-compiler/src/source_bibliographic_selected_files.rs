//! Exact bounded source bytes selected by an owner outside a whole-corpus cut.
//!
//! PG-backed event readers use this carrier after selecting each path from the
//! addressed event closure. It records explicit absence observations separately
//! from paths that were never selected; it does not claim a `SourceRevision`
//! or discover source membership.

use crate::{Error, Result};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::RelativePath;

const MAX_FILES: usize = 4096;
const MAX_BYTES: usize = 64 * 1024 * 1024;

/// Caller-selected exact files and absence observations for one bounded
/// operation. The caller owns the PG snapshot and must supply the complete
/// addressed readset; this type only prevents later resolution from treating
/// an unselected path as absent.
pub struct SelectedBibliographicFilesV1 {
    files: BTreeMap<String, Vec<u8>>,
    absent: BTreeSet<String>,
    max_files: usize,
    max_bytes: usize,
    bytes: usize,
}

impl SelectedBibliographicFilesV1 {
    /// Construct an immutable exact set. Every path must be canonical and
    /// unique across present and absent observations. Limits include absence
    /// witnesses in the path count and present payloads in the byte count.
    pub fn new(
        files: BTreeMap<String, Vec<u8>>,
        absent_paths: BTreeSet<String>,
        max_files: usize,
        max_bytes: usize,
    ) -> Result<Self> {
        if max_files == 0
            || max_files > MAX_FILES
            || max_bytes == 0
            || max_bytes > MAX_BYTES
            || files
                .len()
                .checked_add(absent_paths.len())
                .is_none_or(|count| count > max_files)
        {
            return Err(Error::Budget("selected bibliographic file set limits"));
        }
        let mut bytes = 0usize;
        for (path, raw) in &files {
            validate_path(path)?;
            bytes = bytes
                .checked_add(raw.len())
                .filter(|total| *total <= max_bytes)
                .ok_or(Error::Budget("selected bibliographic file set bytes"))?;
        }
        for path in &absent_paths {
            validate_path(path)?;
            if files.contains_key(path) {
                return Err(Error::Invalid(
                    "selected bibliographic path both present and absent",
                ));
            }
        }
        Ok(Self {
            files,
            absent: absent_paths,
            max_files,
            max_bytes,
            bytes,
        })
    }

    /// Return selected bytes, failing if the path was not part of this exact
    /// input. This does not silently reinterpret an omitted path as missing.
    pub fn required(&self, path: &str) -> Result<&[u8]> {
        validate_path(path)?;
        self.files
            .get(path)
            .map(Vec::as_slice)
            .ok_or(Error::Invalid(
                "required selected bibliographic file absent",
            ))
    }

    /// Return exact selected bytes or an explicit absence observation. An
    /// unknown path is an incomplete readset and refuses resolution.
    pub fn optional(&self, path: &str) -> Result<Option<&[u8]>> {
        validate_path(path)?;
        if let Some(raw) = self.files.get(path) {
            return Ok(Some(raw));
        }
        if self.absent.contains(path) {
            return Ok(None);
        }
        Err(Error::Invalid(
            "optional selected bibliographic path was not observed",
        ))
    }

    /// Whether an exact path was observed, present or absent.
    pub fn observed(&self, path: &str) -> Result<bool> {
        validate_path(path)?;
        Ok(self.files.contains_key(path) || self.absent.contains(path))
    }

    /// All selected present paths in canonical order. Callers use this only
    /// to bind their independently bounded PG prefix listing to the exact
    /// files supplied here.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    /// All selected present and absent paths, in canonical order. Consumers
    /// use this to bind the complete PG readset, including absence witnesses,
    /// into an event dependency manifest.
    pub fn observed_paths(&self) -> Vec<&str> {
        let mut paths = self
            .files
            .keys()
            .chain(self.absent.iter())
            .map(String::as_str)
            .collect::<Vec<_>>();
        paths.sort_unstable();
        paths
    }

    /// Resolve a flat source package selected by the PG owner. The package
    /// must be non-empty; nested and duplicate basenames are rejected.
    pub fn package(
        &self,
        prefix: &str,
        max_members: usize,
        max_package_bytes: usize,
    ) -> Result<BTreeMap<String, &[u8]>> {
        validate_path(prefix)?;
        if max_members == 0
            || max_members > self.max_files
            || max_package_bytes == 0
            || max_package_bytes > self.max_bytes
        {
            return Err(Error::Budget("selected bibliographic package limits"));
        }
        let prefix = format!("{prefix}/");
        let mut result = BTreeMap::new();
        let mut bytes = 0usize;
        for (path, raw) in self.files.range(prefix.clone()..) {
            let Some(relative) = path.strip_prefix(&prefix) else {
                break;
            };
            if relative.contains('/') || relative.is_empty() {
                return Err(Error::Invalid("selected bibliographic package is not flat"));
            }
            if result.len() >= max_members || result.contains_key(relative) {
                return Err(Error::Budget("selected bibliographic package member count"));
            }
            bytes = bytes
                .checked_add(raw.len())
                .filter(|total| *total <= max_package_bytes)
                .ok_or(Error::Budget("selected bibliographic package bytes"))?;
            result.insert(relative.to_owned(), raw.as_slice());
        }
        if result.is_empty() {
            return Err(Error::Invalid("selected bibliographic package is empty"));
        }
        Ok(result)
    }

    pub fn observed_path_count(&self) -> usize {
        self.files.len() + self.absent.len()
    }

    pub fn observed_bytes(&self) -> usize {
        self.bytes
    }
}

fn validate_path(path: &str) -> Result<()> {
    let parsed = RelativePath::parse(path)
        .map_err(|_| Error::Invalid("selected bibliographic source path"))?;
    if parsed.as_str() != path {
        return Err(Error::Invalid(
            "selected bibliographic source path is not canonical",
        ));
    }
    Ok(())
}
