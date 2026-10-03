//! The maintained owner-local Text creators check the complete bounded
//! metadata namespace before assigning opaque IDs. This scan reads metadata
//! only; a stable identity does not grant source or private-content access.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use crate::source_public_text_owner::PublicNativeTextSelection;
use crate::source_text_owner::OwnerTextContext;
use crate::source_text_private_store::TextPackageOwner;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::Metadata;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
};

const MAX_FILES: usize = 2048;
const MAX_ENTRIES: usize = 32768;
const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_FILE: usize = 32 * 1024 * 1024;
const MAX_RECORDS: usize = 65536;
const MAX_JSONL_LINE: usize = 1_048_576;

fn is_candidate(name: &str) -> bool {
    (name.contains("source-text-unit")
        || name.contains("source-text-layer")
        || name.contains("source-anchor")
        || name.contains("translation-alignment"))
        && name.ends_with(".json")
        || name.contains("anchor") && name.ends_with(".jsonl")
        || name.contains("provenance") && name.ends_with(".jsonl")
}

fn same(meta: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        meta.dev(),
        meta.ino(),
        meta.len(),
        meta.mtime(),
        meta.mtime_nsec(),
        meta.ctime(),
        meta.ctime_nsec(),
    )
}

fn directory(path: &Path, uid: u32, private: bool) -> SourceCommandResult<std::fs::File> {
    let fd = tos_fd_open::open_absolute_directory(path)
        .map_err(|_| SourceCommandError::Denied("native Text identity directory unsafe"))?;
    let meta = fd
        .metadata()
        .map_err(|_| SourceCommandError::Denied("native Text identity directory metadata"))?;
    if !meta.is_dir()
        || meta.uid() != uid && (private || meta.uid() != 0)
        || meta.mode() & 0o022 != 0
        || private && meta.mode() & 0o7777 != 0o700
    {
        return Err(SourceCommandError::Denied(
            "native Text identity directory owner or mode",
        ));
    }
    Ok(fd)
}

fn paths(
    context: &impl TextPackageOwner,
    exclude: Option<&Path>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<String>> {
    let roots = context.identity_roots();
    let mut pending: Vec<(&Path, PathBuf, bool)> = roots
        .iter()
        .map(|(root, start, private)| (*root, start.clone(), *private))
        .collect();
    let mut entries = 0usize;
    let mut found = Vec::new();
    while let Some((root, path, private)) = pending.pop() {
        active(deadline, cancelled)?;
        if exclude == Some(path.as_path()) {
            continue;
        }
        let fd = directory(&path, context.account_uid(), private)?;
        let before = fd
            .metadata()
            .map_err(|_| SourceCommandError::Denied("native Text identity directory metadata"))?;
        let children = std::fs::read_dir(format!("/proc/self/fd/{}", fd.as_raw_fd()))
            .map_err(|_| SourceCommandError::Denied("native Text identity enumeration"))?;
        for child in children {
            active(deadline, cancelled)?;
            entries += 1;
            if entries > MAX_ENTRIES {
                return Err(SourceCommandError::Unsupported(
                    "native Text identity entry budget",
                ));
            }
            let name = child
                .map_err(|_| SourceCommandError::Denied("native Text identity entry"))?
                .file_name();
            let name = name
                .to_str()
                .ok_or(SourceCommandError::Invalid("native Text identity name"))?;
            if name.starts_with('.') || matches!(name, "payload" | "local-content" | "catalog") {
                continue;
            }
            let next = path.join(name);
            let metadata = next
                .symlink_metadata()
                .map_err(|_| SourceCommandError::Conflict("native Text identity entry changed"))?;
            if metadata.is_dir() {
                pending.push((root, next, private));
            } else if metadata.file_type().is_symlink() {
                return Err(SourceCommandError::Denied("native Text identity alias"));
            } else if is_candidate(name) {
                if !metadata.is_file() {
                    return Err(SourceCommandError::Denied(
                        "native Text identity metadata type",
                    ));
                }
                let relative = next
                    .strip_prefix(root)
                    .map_err(|_| SourceCommandError::Invalid("native Text identity route"))?;
                let relative = relative
                    .to_str()
                    .ok_or(SourceCommandError::Invalid("native Text identity UTF-8"))?;
                found.push(relative.to_owned());
                if found.len() > MAX_FILES {
                    return Err(SourceCommandError::Unsupported(
                        "native Text identity file budget",
                    ));
                }
            }
        }
        let after = fd
            .metadata()
            .map_err(|_| SourceCommandError::Conflict("native Text identity directory changed"))?;
        let current = directory(&path, context.account_uid(), private)?;
        if same(&before) != same(&after)
            || same(&before)
                != same(&current.metadata().map_err(|_| {
                    SourceCommandError::Conflict("native Text identity directory changed")
                })?)
        {
            return Err(SourceCommandError::Conflict(
                "native Text identity directory changed",
            ));
        }
    }
    found.sort();
    if found.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(SourceCommandError::Conflict(
            "native Text identity duplicate route",
        ));
    }
    Ok(found)
}

fn parsed_record(raw: &[u8]) -> SourceCommandResult<JsonValue> {
    parse_json(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: MAX_FILE,
            ..JsonLimits::default()
        },
    )
    .map(|doc| doc.into_root())
    .map_err(|_| SourceCommandError::Invalid("native Text identity metadata JSON"))
}

fn record_ids(
    record: &JsonValue,
    name: &str,
    owned: &mut BTreeSet<String>,
) -> SourceCommandResult<()> {
    let schema = cmd::text(record, "schema_version")?;
    let mut add = |value: &JsonValue| -> SourceCommandResult<()> {
        if let Some(text) = value.as_str() {
            owned.insert(text.to_owned());
        }
        Ok(())
    };
    match schema {
        "tos_source_text_unit_packet_v1" => {
            add(cmd::field(record, "packet_id")?)?;
            for (group, key) in [
                ("schemes", "scheme_id"),
                ("anchors", "anchor_ref"),
                ("units", "unit_id"),
                ("segmentations", "segmentation_id"),
            ] {
                for entry in cmd::array(record, group)? {
                    add(cmd::field(entry, key)?)?;
                }
            }
        }
        "tos_source_text_layer_v1" => add(cmd::field(record, "layer_id")?)?,
        "tos_native_translation_alignment_record_v1" => {
            add(cmd::field(record, "record_id")?)?;
            add(cmd::field(record, "alignment_id")?)?;
            add(cmd::field(cmd::field(record, "claim")?, "claim_id")?)?;
        }
        "tos_translation_alignment_packet_v1" => {
            add(cmd::field(record, "packet_id")?)?;
            for row in cmd::array(record, "alignments")? {
                add(cmd::field(row, "alignment_id")?)?;
                add(cmd::field(row, "claim_id")?)?;
            }
        }
        "tos_source_anchor_v2" | "tos_source_anchor_v1" => {
            add(cmd::field(record, "anchor_id")?)?;
            if let Some(value) = record.as_object().and_then(|entries| {
                entries
                    .iter()
                    .find(|(key, _)| key.as_str() == Some("passage_id"))
                    .map(|(_, value)| value)
            }) {
                add(value)?;
            }
        }
        _ if name.contains("provenance") => add(cmd::field(record, "event_id")?)?,
        _ => {
            return Err(SourceCommandError::Invalid(
                "native Text identity owner shape",
            ));
        }
    }
    Ok(())
}

/// One complete bounded metadata pass plus a path-membership rewalk. The
/// returned digest is a current inventory fact, not a grant. The owning entry
/// repeats this complete pass at its final selected publication check; the
/// intermediate staging checks cover the current owner and control instead.
pub(crate) fn selected_identity_snapshot(
    context: &OwnerTextContext,
    delegated: &[&str],
    exclude: Option<&Path>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Digest256> {
    selected_identity_snapshot_with_ancestry(context, delegated, exclude, None, deadline, cancelled)
}

/// A native descriptive successor may retain only IDs belonging to its exact
/// immutable predecessor chain. The allow-list is constructed from verified
/// path+bytes+identity+version references by the alignment caller; it does
/// not grant a read or bypass the complete current membership walk.
pub(crate) fn selected_alignment_identity_snapshot(
    context: &OwnerTextContext,
    delegated: &[&str],
    exclude: Option<&Path>,
    ancestry: &BTreeMap<String, Digest256>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Digest256> {
    selected_identity_snapshot_with_ancestry(
        context,
        delegated,
        exclude,
        Some(ancestry),
        deadline,
        cancelled,
    )
}

fn selected_identity_snapshot_with_ancestry(
    context: &impl TextPackageOwner,
    delegated: &[&str],
    exclude: Option<&Path>,
    ancestry: Option<&BTreeMap<String, Digest256>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Digest256> {
    let selected = paths(context, exclude, deadline, cancelled)?;
    let mut remaining = MAX_BYTES;
    let mut inputs = BTreeMap::new();
    let mut owned = BTreeSet::new();
    let mut records = 0usize;
    for reference in &selected {
        active(deadline, cancelled)?;
        let bytes =
            context.identity_read(reference, remaining.min(MAX_FILE), deadline, cancelled)?;
        remaining -= bytes.len();
        inputs.insert(
            reference.as_str(),
            cmd::string(&Digest256::of_bytes(&bytes).to_prefixed()),
        );
        if reference.ends_with(".jsonl") {
            for line in bytes.split_inclusive(|byte| *byte == b'\n') {
                let line = line.strip_suffix(b"\n").unwrap_or(line);
                if line.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                records += 1;
                if records > MAX_RECORDS || line.len() > MAX_JSONL_LINE {
                    return Err(SourceCommandError::Unsupported(
                        "native Text identity record budget",
                    ));
                }
                let mut current = BTreeSet::new();
                record_ids(&parsed_record(line)?, reference, &mut current)?;
                if exclude.is_none()
                    && delegated.iter().any(|id| current.contains(*id))
                    && !ancestry.is_some_and(|chain| {
                        chain.get(reference) == Some(&Digest256::of_bytes(&bytes))
                    })
                {
                    return Err(SourceCommandError::Conflict(
                        "native Text identity occupied",
                    ));
                }
                owned.extend(current);
            }
        } else {
            records += 1;
            if records > MAX_RECORDS {
                return Err(SourceCommandError::Unsupported(
                    "native Text identity record budget",
                ));
            }
            let mut current = BTreeSet::new();
            record_ids(&parsed_record(&bytes)?, reference, &mut current)?;
            if exclude.is_none()
                && delegated.iter().any(|id| current.contains(*id))
                && !ancestry
                    .is_some_and(|chain| chain.get(reference) == Some(&Digest256::of_bytes(&bytes)))
            {
                return Err(SourceCommandError::Conflict(
                    "native Text identity occupied",
                ));
            }
            owned.extend(current);
        }
    }
    if ancestry.is_none() && delegated.iter().any(|id| owned.contains(*id)) {
        return Err(SourceCommandError::Conflict(
            "native Text delegated identity already owned",
        ));
    }
    if selected != paths(context, exclude, deadline, cancelled)? {
        return Err(SourceCommandError::Conflict(
            "native Text identity membership changed",
        ));
    }
    let value = cmd::object(inputs.into_iter().collect());
    let raw = canonical_bytes_v1(
        &value,
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits {
            max_bytes: MAX_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| SourceCommandError::Unsupported("native Text identity snapshot byte budget"))?;
    Ok(Digest256::of_bytes(&raw))
}

pub(crate) fn selected_public_identity_snapshot(
    context: &PublicNativeTextSelection,
    delegated: &[&str],
    exclude: Option<&Path>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Digest256> {
    selected_identity_snapshot_with_ancestry(context, delegated, exclude, None, deadline, cancelled)
}
