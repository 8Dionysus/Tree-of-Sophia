//! The maintained owner-local Text and Translation Alignment creators check
//! the complete bounded metadata namespace before assigning opaque IDs. This
//! scan reads metadata only; a stable identity does not grant source or
//! private-content access.

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

pub(crate) const MAX_FILES: usize = 2048;
pub(crate) const MAX_ENTRIES: usize = 32768;
pub(crate) const MAX_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_FILE: usize = 32 * 1024 * 1024;
pub(crate) const MAX_RECORDS: usize = 65536;
pub(crate) const MAX_JSONL_LINE: usize = 1_048_576;
pub(crate) const MAX_POINT_OWNERS: usize = MAX_RECORDS;

/// One exact current public Text identity owner returned by the point index.
/// The member digest is required to distinguish a verified retained ancestor
/// from a later replacement at the same path.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct TextIdentityOwner {
    pub path: String,
    pub member_digest: Digest256,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TextIdentityContribution {
    pub path: String,
    pub token: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PgSourceTextIdentityObservation {
    pub owners: BTreeSet<TextIdentityOwner>,
    pub generation: u64,
}

pub(crate) fn is_candidate(name: &str) -> bool {
    // Schemas and the philosophy producer's future-source backlog describe
    // this namespace; neither owns Text/Anchor record identities. Backlog
    // bytes still belong to the source cohort and its history.
    if name.ends_with(".schema.json") || name == "source-anchor-backlog.jsonl" {
        return false;
    }
    (name.contains("source-text-unit")
        || name.contains("source-text-layer")
        || name.contains("source-anchor")
        || name.contains("translation-alignment"))
        && name.ends_with(".json")
        || name.contains("anchor") && name.ends_with(".jsonl")
        || name.contains("provenance") && name.ends_with(".jsonl")
}

/// Return whether one package member belongs to the native translation
/// Alignment identity family. Alignment packages share the established ID
/// parser and index, but do not index TextLayer, TextUnit, Anchor or event
/// members carried alongside an alignment record.
pub(crate) fn is_alignment_candidate(name: &str) -> bool {
    name.contains("translation-alignment") && name.ends_with(".json") && is_candidate(name)
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
        let text = value
            .as_str()
            .filter(|text| !text.is_empty() && text.len() <= 4096)
            .ok_or(SourceCommandError::Invalid(
                "native Text identity identifier shape",
            ))?;
        owned.insert(text.to_owned());
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
                // Both Anchor schemas allow an absent or null passage_id.
                // Only an actual passage identifier owns an index token.
                if !matches!(value, JsonValue::Null) {
                    add(value)?;
                }
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

/// Extract the existing Text identity namespace from one exact member. This
/// is shared by PG point-index maintenance and private owner packages so the
/// schema-to-ID mapping stays owned here.
pub(crate) fn text_identity_ids_for_member(
    name: &str,
    raw: &[u8],
) -> SourceCommandResult<BTreeSet<String>> {
    if !is_candidate(name) {
        return Err(SourceCommandError::Invalid(
            "native Text identity candidate route",
        ));
    }
    if raw.len() > MAX_FILE {
        return Err(SourceCommandError::Unsupported(
            "native Text identity file byte budget",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut records = 0usize;
    if name.ends_with(".jsonl") {
        for line in raw.split_inclusive(|byte| *byte == b'\n') {
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
            record_ids(&parsed_record(line)?, name, &mut ids)?;
        }
    } else {
        record_ids(&parsed_record(raw)?, name, &mut ids)?;
    }
    Ok(ids)
}

/// Build exact per-member contribution rows for the public point index.
pub(crate) fn text_identity_contributions(
    path: &str,
    raw: &[u8],
) -> SourceCommandResult<Vec<TextIdentityContribution>> {
    if path.is_empty()
        || path.len() > 4096
        || tos_foundation::RelativePath::parse(path).is_err()
        || !path.starts_with("ToS/")
    {
        return Err(SourceCommandError::Invalid("native Text identity route"));
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    let ids = text_identity_ids_for_member(name, raw)?;
    if ids.len() > MAX_POINT_OWNERS {
        return Err(SourceCommandError::Unsupported(
            "native Text identity contribution bound exceeded",
        ));
    }
    Ok(ids
        .into_iter()
        .map(|token| TextIdentityContribution {
            path: path.to_owned(),
            token,
        })
        .collect())
}

/// Canonically commit one bounded set of public point observations. Callers
/// must separately refuse owners outside the exact ancestry allowlist before
/// admitting a new identity; this digest records both the observations and
/// the allowlist under the selected source fence.
pub(crate) fn point_identity_snapshot_digest(
    selection: &crate::source_cohort::revision::SourceRevisionSelection,
    requested: &BTreeSet<String>,
    owners_by_token: &BTreeMap<String, BTreeSet<TextIdentityOwner>>,
    generations_by_token: &BTreeMap<String, u64>,
    ancestry: &BTreeMap<String, Digest256>,
) -> SourceCommandResult<Digest256> {
    if requested.len() > MAX_RECORDS
        || owners_by_token.len() != requested.len()
        || generations_by_token.len() != requested.len()
        || owners_by_token.keys().ne(requested.iter())
        || generations_by_token.keys().ne(requested.iter())
        || ancestry.len() > MAX_FILES
    {
        return Err(SourceCommandError::Invalid(
            "native Text identity point snapshot shape",
        ));
    }
    let mut owner_count = 0usize;
    let mut token_rows = Vec::with_capacity(requested.len());
    for token in requested {
        if token.is_empty() || token.len() > 4096 {
            return Err(SourceCommandError::Invalid(
                "native Text identity point token",
            ));
        }
        let owners = owners_by_token
            .get(token)
            .ok_or(SourceCommandError::Invalid(
                "native Text identity point owners absent",
            ))?;
        owner_count = owner_count
            .checked_add(owners.len())
            .filter(|count| *count <= MAX_POINT_OWNERS)
            .ok_or(SourceCommandError::Unsupported(
                "native Text identity point owner budget",
            ))?;
        let mut owner_rows = Vec::with_capacity(owners.len());
        let mut previous_owner_path = None;
        for owner in owners {
            if owner.path.is_empty()
                || owner.path.len() > 4096
                || tos_foundation::RelativePath::parse(&owner.path).is_err()
                || !owner.path.starts_with("ToS/")
                || owner.path == "ToS/source-witnesses/owner-local"
                || owner.path.starts_with("ToS/source-witnesses/owner-local/")
            {
                return Err(SourceCommandError::Invalid(
                    "native Text identity point owner path",
                ));
            }
            if previous_owner_path == Some(owner.path.as_str()) {
                return Err(SourceCommandError::Invalid(
                    "native Text identity duplicate point owner path",
                ));
            }
            previous_owner_path = Some(owner.path.as_str());
            owner_rows.push(cmd::object(vec![
                ("path", cmd::string(&owner.path)),
                ("member_digest", cmd::string(&owner.member_digest.to_hex())),
                (
                    "ancestry_match",
                    JsonValue::Bool(ancestry.get(&owner.path) == Some(&owner.member_digest)),
                ),
            ]));
        }
        token_rows.push(cmd::object(vec![
            ("token", cmd::string(token)),
            (
                "generation",
                cmd::number(*generations_by_token.get(token).ok_or(
                    SourceCommandError::Invalid("native Text identity point generation absent"),
                )?),
            ),
            ("owners", JsonValue::Array(owner_rows)),
        ]));
    }
    let mut ancestry_rows = Vec::with_capacity(ancestry.len());
    for (path, digest) in ancestry {
        if path.is_empty()
            || path.len() > 4096
            || tos_foundation::RelativePath::parse(path).is_err()
            || !path.starts_with("ToS/")
            || path == "ToS/source-witnesses/owner-local"
            || path.starts_with("ToS/source-witnesses/owner-local/")
        {
            return Err(SourceCommandError::Invalid(
                "native Text identity ancestry path",
            ));
        }
        ancestry_rows.push(cmd::object(vec![
            ("path", cmd::string(path)),
            ("member_digest", cmd::string(&digest.to_hex())),
        ]));
    }
    let identity = selection.identity();
    let value = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_text_identity_point_snapshot_v1"),
        ),
        ("domain", cmd::string(selection.cohort.domain())),
        (
            "domain_digest",
            cmd::string(&identity.domain_digest.to_hex()),
        ),
        (
            "store_id_digest",
            cmd::string(&Digest256::of_bytes(&identity.store_id).to_hex()),
        ),
        (
            "bootstrap_revision",
            cmd::string(&identity.bootstrap_revision.0.to_hex()),
        ),
        (
            "bootstrap_membership_count",
            cmd::number(identity.bootstrap_membership.count),
        ),
        (
            "bootstrap_membership_digest",
            cmd::string(&identity.bootstrap_membership.digest.to_hex()),
        ),
        (
            "cohort_digest",
            cmd::string(&identity.cohort_digest.to_hex()),
        ),
        ("cohort_epoch", cmd::number(identity.cohort_epoch)),
        (
            "cohort_definition",
            cmd::string(&identity.cohort_definition.to_hex()),
        ),
        ("cohort_generation", cmd::number(identity.cohort_generation)),
        (
            "job_id_digest",
            cmd::string(&identity.job_id_digest.to_hex()),
        ),
        ("job_fence", cmd::number(identity.job_fence)),
        ("epoch", cmd::number(identity.epoch)),
        ("source_generation", cmd::number(identity.source_generation)),
        ("head_seq", cmd::number(identity.head_seq)),
        ("audit_generation", cmd::number(identity.audit_generation)),
        ("rights_version", cmd::number(identity.rights_version)),
        ("rule_version", cmd::number(identity.rule_version)),
        ("contract", cmd::string(&identity.contract.to_hex())),
        ("tokens", JsonValue::Array(token_rows)),
        ("ancestry", JsonValue::Array(ancestry_rows)),
    ]);
    let raw = canonical_bytes_v1(
        &value,
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits {
            max_bytes: MAX_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| {
        SourceCommandError::Unsupported("native Text identity point snapshot byte budget")
    })?;
    Ok(Digest256::of_bytes(&raw))
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

/// Use a PG-selected identity inventory when the private owner adapter has
/// installed one; its source and private-generation fences are rechecked by
/// the transactional writer. Filesystem-backed callers retain the established
/// bounded dual-root census.
pub(crate) fn selected_owner_identity_snapshot(
    context: &OwnerTextContext,
    delegated: &[&str],
    exclude: Option<&Path>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Digest256> {
    if let Some(snapshot) = context.pg_identity_snapshot()? {
        active(deadline, cancelled)?;
        let _ = (delegated, exclude);
        Ok(snapshot)
    } else {
        selected_identity_snapshot(context, delegated, exclude, deadline, cancelled)
    }
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

#[cfg(test)]
mod tests {
    use super::{is_alignment_candidate, is_candidate, text_identity_ids_for_member};
    use std::collections::BTreeSet;

    #[test]
    fn bootstrap_contract_identity_candidates_distinguish_contracts_from_owned_records() {
        for name in [
            "native-translation-alignment-record-v1.schema.json",
            "source-anchor-v2.schema.json",
            "source-anchor.schema.json",
            "source-text-layer.schema.json",
            "source-text-unit-packet-v1.schema.json",
            "translation-alignment-packet-v1.schema.json",
            "source-anchor-backlog.jsonl",
        ] {
            assert!(!is_candidate(name));
            assert!(!is_alignment_candidate(name));
            assert!(text_identity_ids_for_member(name, br#"{}"#).is_err());
        }
        for name in [
            "native-translation-alignment.v1.json",
            "source-anchor.v2.json",
            "source-text-layer.v1.json",
            "source-text-unit-packet.v1.json",
            "translation-alignment-packet.v1.json",
            "source-create-provenance.jsonl",
            "source-anchor.jsonl",
        ] {
            assert!(is_candidate(name));
            // Missing schema_version still fails for actual record candidates.
            assert!(text_identity_ids_for_member(name, br#"{}"#).is_err());
        }
    }

    #[test]
    fn bootstrap_contract_anchor_optional_passage_identity_follows_both_schemas() {
        for version in [1, 2] {
            for suffix in ["", ",\"passage_id\":null"] {
                let raw = format!(
                    "{{\"schema_version\":\"tos_source_anchor_v{version}\",\"anchor_id\":\"tos.anchor.a\"{suffix}}}"
                );
                assert_eq!(
                    text_identity_ids_for_member("anchors.jsonl", raw.as_bytes()).unwrap(),
                    BTreeSet::from(["tos.anchor.a".to_owned()])
                );
            }
            let raw = format!(
                "{{\"schema_version\":\"tos_source_anchor_v{version}\",\"anchor_id\":\"tos.anchor.a\",\"passage_id\":\"tos.passage.p\"}}"
            );
            assert_eq!(
                text_identity_ids_for_member("anchors.jsonl", raw.as_bytes()).unwrap(),
                BTreeSet::from(["tos.anchor.a".to_owned(), "tos.passage.p".to_owned()])
            );
            for invalid in ["0", "false", "{}", "[]", "\"\""] {
                let raw = format!(
                    "{{\"schema_version\":\"tos_source_anchor_v{version}\",\"anchor_id\":\"tos.anchor.a\",\"passage_id\":{invalid}}}"
                );
                assert!(text_identity_ids_for_member("anchors.jsonl", raw.as_bytes()).is_err());
            }
        }
    }

    #[test]
    fn alignment_identity_uses_only_translation_alignment_json_members() {
        assert!(is_alignment_candidate(
            "native-translation-alignment.v1.json"
        ));
        assert!(!is_alignment_candidate("source-create-provenance.jsonl"));
        assert!(!is_alignment_candidate(
            "translation-alignment-provenance.jsonl"
        ));
        assert!(!is_alignment_candidate("source-text-unit-packet.v1.json"));

        let ids = text_identity_ids_for_member(
            "native-translation-alignment.v1.json",
            br#"{"schema_version":"tos_native_translation_alignment_record_v1","record_id":"tos.record.translation-alignment-record.r1","alignment_id":"tos.alignment.translation-alignment.a1","claim":{"claim_id":"tos.claim.translation-alignment-claim.c1"}}"#,
        )
        .expect("exact native alignment IDs parse");
        assert_eq!(
            ids,
            BTreeSet::from([
                "tos.alignment.translation-alignment.a1".to_owned(),
                "tos.claim.translation-alignment-claim.c1".to_owned(),
                "tos.record.translation-alignment-record.r1".to_owned(),
            ])
        );
    }
}
