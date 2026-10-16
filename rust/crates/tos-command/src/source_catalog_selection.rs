//! Bounded raw catalog selection shared by native owner adapters.
use super::work_expression::{catalog_lines, known_catalog_kind, raw_hex};
use super::{CreationFilesystem, active, walk, work_transaction};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{JsonValue, RelativePath};
use tos_validation::item_rules::ItemRefusal;
use tos_validation::record_biblio_cut::{SourceCutInput, SourceCutMemberMeta};
pub(super) struct CatalogSelection {
    pub(super) digests: BTreeMap<String, String>,
    pub(super) record_ids: BTreeSet<String>,
    pub(super) claim_ids: BTreeSet<String>,
    pub(super) entries: Vec<(String, JsonValue)>,
    pub(super) bytes_read: usize,
}
pub(super) fn select_catalog(
    fs: &CreationFilesystem,
    publication_token: Option<&str>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CatalogSelection> {
    let catalog = walk(&fs.root, "ToS/source-witnesses/catalog", fs.uid)?;
    select_catalog_with_reader(
        publication_token,
        |path, cap| {
            let leaf = path
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("catalog member path"))?
                .1;
            work_transaction::read_at(&catalog, leaf, fs.uid, cap, deadline, cancelled)?
                .ok_or(SourceCommandError::Conflict("Work catalog member absent"))
        },
        deadline,
        cancelled,
    )
}

/// Select the same complete, publication-bound catalog through an exact
/// current-source input. The caller's source readset owns currentness; this
/// helper returns the exact dependency digests consumed by the owner.
pub(super) fn select_catalog_from_input(
    input: &dyn SourceCutInput,
    publication_token: Option<&str>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CatalogSelection> {
    select_catalog_with_reader(
        publication_token,
        |path, cap| {
            let parsed = RelativePath::parse(path)
                .map_err(|_| SourceCommandError::Invalid("catalog member path"))?;
            if parsed.as_str() != path {
                return Err(SourceCommandError::Invalid("catalog member path"));
            }
            let mut selected = None;
            input
                .with_current_member(
                    path,
                    cap,
                    deadline,
                    cancelled,
                    &mut |meta: SourceCutMemberMeta<'_>, raw| {
                        if meta.path != path || raw.len() as u64 != meta.size_bytes {
                            return Err(ItemRefusal::Source(
                                "selected catalog member metadata differs".into(),
                            ));
                        }
                        selected = Some(raw.to_vec());
                        Ok(())
                    },
                )
                .map_err(|_| SourceCommandError::Conflict("selected catalog member read"))?;
            selected.ok_or(SourceCommandError::Conflict(
                "selected catalog member absent",
            ))
        },
        deadline,
        cancelled,
    )
}

fn select_catalog_with_reader(
    publication_token: Option<&str>,
    mut read: impl FnMut(&str, usize) -> SourceCommandResult<Vec<u8>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CatalogSelection> {
    active(deadline, cancelled)?;
    let manifest_path = "ToS/source-witnesses/catalog/catalog.manifest.json";
    let manifest_raw = read(manifest_path, 2_097_152)?;
    let manifest = cmd::parse(&manifest_raw)?;
    if cmd::text(&manifest, "schema_version")? != "tos_source_witness_catalog_v3"
        || cmd::text(&manifest, "claim_file")? != "ToS/source-witnesses/catalog/claims.jsonl"
    {
        return Err(SourceCommandError::Unsupported(
            "Work catalog route/version",
        ));
    }
    let record_files = cmd::field(&manifest, "record_files")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("Work catalog record routes"))?;
    if record_files.is_empty() || record_files.len() > 128 {
        return Err(SourceCommandError::Invalid("Work catalog route count"));
    }
    let mut routes = Vec::with_capacity(record_files.len() + 1);
    for (kind, value) in record_files {
        let kind = kind
            .as_str()
            .ok_or(SourceCommandError::Invalid("catalog kind"))?;
        let reference = value
            .as_str()
            .ok_or(SourceCommandError::Invalid("catalog route"))?;
        let prefix = "ToS/source-witnesses/catalog/";
        if !reference.starts_with(prefix) || !known_catalog_kind(kind, &reference[prefix.len()..]) {
            return Err(SourceCommandError::Unsupported(
                "Work catalog undeclared profile route",
            ));
        }
        routes.push((kind.to_owned(), reference.to_owned()));
    }
    routes.sort();
    routes.push((
        "claim".to_owned(),
        "ToS/source-witnesses/catalog/claims.jsonl".to_owned(),
    ));
    let mut digests = BTreeMap::new();
    let mut record_ids = BTreeSet::new();
    let mut claim_ids = BTreeSet::new();
    let mut entries = Vec::new();
    let mut total_bytes = manifest_raw.len();
    let mut count = 0usize;
    for (kind, reference) in routes {
        active(deadline, cancelled)?;
        let raw = read(&reference, 16_777_216)?;
        total_bytes = total_bytes
            .checked_add(raw.len())
            .ok_or(SourceCommandError::Invalid(
                "Work catalog aggregate overflow",
            ))?;
        if total_bytes > 16_777_216 {
            return Err(SourceCommandError::Invalid(
                "Work catalog aggregate byte budget",
            ));
        }
        digests.insert(reference.clone(), raw_hex(&raw));
        for line in catalog_lines(&raw) {
            active(deadline, cancelled)?;
            count += 1;
            if count > 8192 {
                return Err(SourceCommandError::Invalid("Work catalog row budget"));
            }
            let entry = cmd::parse(line)?;
            let id = if kind == "claim" {
                if cmd::text(&entry, "schema_version")?
                    != "tos_source_witness_claim_catalog_entry_v1"
                {
                    return Err(SourceCommandError::Invalid("Work claim catalog schema"));
                }
                cmd::text(&entry, "claim_id")?
            } else {
                if cmd::text(&entry, "schema_version")? != "tos_source_witness_catalog_entry_v1"
                    || cmd::text(&entry, "record_type")? != kind
                {
                    return Err(SourceCommandError::Invalid("Work record catalog schema"));
                }
                cmd::text(&entry, "record_id")?
            };
            if record_ids.contains(id)
                || claim_ids.contains(id)
                || !(if kind == "claim" {
                    &mut claim_ids
                } else {
                    &mut record_ids
                })
                .insert(id.to_owned())
            {
                return Err(SourceCommandError::Conflict(
                    "duplicate Work catalog identity",
                ));
            }
            entries.push((kind.clone(), entry));
        }
    }
    let binding = manifest.object_get("selected_metadata_publication");
    match (binding, publication_token) {
        (None, None) => (),
        (Some(binding), Some(token)) => {
            cmd::exact_keys(binding, &["protocol", "token", "files"])?;
            if cmd::text(binding, "protocol")? != "tos_selected_source_metadata_v1"
                || cmd::text(binding, "token")? != token
            {
                return Err(SourceCommandError::Conflict(
                    "Work catalog publication token",
                ));
            }
            let rows =
                cmd::field(binding, "files")?
                    .as_object()
                    .ok_or(SourceCommandError::Invalid(
                        "Work catalog publication file map",
                    ))?;
            if rows.len() != digests.len()
                || digests.iter().any(|(path, digest)| {
                    rows.iter()
                        .find(|(key, _)| key.as_str() == Some(path.as_str()))
                        .and_then(|(_, value)| value.as_str())
                        != Some(digest.as_str())
                })
            {
                return Err(SourceCommandError::Conflict(
                    "Work catalog publication file closure",
                ));
            }
        }
        _ => {
            return Err(SourceCommandError::Conflict(
                "Work catalog publication binding absent",
            ));
        }
    }
    digests.insert(
        "ToS/source-witnesses/catalog/catalog.manifest.json".to_owned(),
        raw_hex(&manifest_raw),
    );
    Ok(CatalogSelection {
        digests,
        record_ids,
        claim_ids,
        entries,
        bytes_read: total_bytes,
    })
}
