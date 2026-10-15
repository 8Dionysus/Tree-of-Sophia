//! Exact current-member lineage checks for bounded source-cohort restore.
//!
//! The caller has already completed the cold history/receipt audit and read
//! the exact current STO frame under the domain rights lock. This module
//! classifies that one current member using its retained first history row;
//! it does not reconstruct inventory or weaken the full index comparison.

use super::*;

/// Check that a selected current member is a coherent continuation of its
/// bootstrap member or a controlled source writer. Revision bodies may differ
/// from bootstrap bytes only after the audited custody chain reaches a
/// `REVISION` carrier.
pub(super) fn check_current_source_lineage(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    current: &postgres::Row,
    raw: &[u8],
    original_raw: Option<&[u8]>,
    original_metadata: Option<&SourceFileMetadata>,
) -> DurableResult<()> {
    let path: String = current.get("subject");
    let relative = RelativePath::parse(&path)
        .map_err(|_| DurableError::Corrupt("current source lineage path"))?;
    if !path.starts_with("ToS/") || relative.as_str() != path.as_str() {
        return Err(DurableError::Corrupt(
            "current source lineage escaped authored metadata",
        ));
    }
    if original_raw.is_some() != original_metadata.is_some() {
        return Err(DurableError::Corrupt(
            "bootstrap source lineage evidence is incomplete",
        ));
    }

    let current_revision = as_u64(current.get("revision"))?;
    let current_profile = current.get::<_, String>("profile_id");
    let current_digest = parse_hex(current.get::<_, String>("content_digest"))?;
    let current_length = as_u64(current.get("content_length"))?;
    if Digest256::of_bytes(raw) != current_digest || raw.len() as u64 != current_length {
        return Err(DurableError::Corrupt(
            "current source lineage bytes differ from retained locator",
        ));
    }
    let current_metadata = row_source_metadata(current)?;

    let first = tx
        .query_opt(
            "SELECT h.profile_id,h.content_digest,h.content_length,
                    h.source_mode,h.source_dependencies,h.prepare_id,
                    m.profile_id AS member_profile_id,m.profile_version AS member_profile_version,
                    m.proposed_revision,m.expected_revision,m.expected_digest
             FROM cmd2_history h
             JOIN cmd2_member m
               ON m.domain=h.domain AND m.prepare_id=h.prepare_id
              AND m.member_slot=h.member_slot
             WHERE h.domain=$1 AND h.subject=$2 AND h.revision=1",
            &[&domain, &path],
        )?
        .ok_or(DurableError::Corrupt(
            "current source lineage has no first history member",
        ))?;
    let first_profile: String = first.get("profile_id");
    let first_digest = parse_hex(first.get::<_, String>("content_digest"))?;
    let first_length = as_u64(first.get("content_length"))?;
    let first_metadata = row_source_metadata(&first)?;
    let first_prepare_id: Vec<u8> = first.get("prepare_id");
    if first.get::<_, String>("member_profile_id") != first_profile
        || first.get::<_, String>("member_profile_version") != "1"
        || as_u64(first.get("proposed_revision"))? != 1
        || first.get::<_, Option<i64>>("expected_revision").is_some()
        || first.get::<_, Option<String>>("expected_digest").is_some()
    {
        return Err(DurableError::Corrupt(
            "first source lineage member is not an initial custody root",
        ));
    }
    if (first_profile.as_bytes() == ORIGINAL) != original_raw.is_some() {
        return Err(DurableError::Corrupt(
            "original source lineage root and bootstrap membership differ",
        ));
    }

    if let (Some(bootstrap), Some(bootstrap_metadata)) = (original_raw, original_metadata) {
        if first_profile.as_bytes() != ORIGINAL
            || first_digest != Digest256::of_bytes(bootstrap)
            || first_length != bootstrap.len() as u64
            || first_metadata != *bootstrap_metadata
        {
            return Err(DurableError::Corrupt(
                "bootstrap source lineage root differs",
            ));
        }
    }

    if current_profile.as_bytes() == ORIGINAL {
        if original_raw.is_none()
            || current_revision != 1
            || first_profile.as_bytes() != ORIGINAL
            || current_digest != first_digest
            || current_length != first_length
            || current_metadata != first_metadata
            || original_raw.is_none_or(|bootstrap| raw != bootstrap)
            || original_metadata != Some(&current_metadata)
        {
            return Err(DurableError::Corrupt(
                "original source member differs from bootstrap lineage",
            ));
        }
    } else if current_profile.as_bytes() == CREATION
        || current_profile.as_bytes() == creation::PG_CREATION
    {
        if original_raw.is_some()
            || current_revision != 1
            || current_profile.as_bytes() != first_profile.as_bytes()
            || current_digest != first_digest
            || current_length != first_length
            || current_metadata != first_metadata
            || !controlled_writer_path(&path)
            || !controlled_creation_metadata(&current_metadata)
        {
            return Err(DurableError::Corrupt(
                "created source member differs from its initial lineage",
            ));
        }
    } else if current_profile.as_bytes() == REVISION {
        if !controlled_writer_path(&path) {
            return Err(DurableError::Corrupt(
                "source revision path is outside its controlled writer scope",
            ));
        }
        if current_revision < 2 && first_profile.as_bytes() != REVISION {
            return Err(DurableError::Corrupt(
                "source revision lineage lacks a predecessor",
            ));
        }
        if current_revision == 1
            && (original_raw.is_some()
                || first_profile.as_bytes() != REVISION
                || !is_revision_auxiliary_path(&path))
        {
            return Err(DurableError::Corrupt(
                "initial revision lineage is not a retained auxiliary member",
            ));
        }
        if !controlled_creation_metadata(&current_metadata) {
            return Err(DurableError::Corrupt(
                "source revision carrier metadata is outside its contract",
            ));
        }
        check_revision_read_closure(tx, domain, current, &current_metadata)?;
        if first_profile.as_bytes() == REVISION {
            if first_metadata.mode != 0o644
                || first_metadata.dependencies.is_none()
                || !is_revision_auxiliary_path(&path)
            {
                return Err(DurableError::Corrupt(
                    "initial revision auxiliary carrier metadata differs",
                ));
            }
            check_revision_read_closure_for_prepare(
                tx,
                domain,
                &first_prepare_id,
                &first_metadata,
            )?;
            check_initial_revision_auxiliary_membership(tx, domain, &first_prepare_id, &path)?;
        } else if ![ORIGINAL, CREATION, creation::PG_CREATION]
            .iter()
            .any(|profile| first_profile.as_bytes() == *profile)
        {
            return Err(DurableError::Corrupt(
                "source revision lineage has an unsupported root profile",
            ));
        } else if first_profile.as_bytes() != ORIGINAL
            && !controlled_creation_metadata(&first_metadata)
        {
            return Err(DurableError::Corrupt(
                "source creation lineage carrier metadata is outside its contract",
            ));
        }
    } else {
        return Err(DurableError::Corrupt(
            "current source member has an unsupported lineage profile",
        ));
    }
    Ok(())
}

fn controlled_creation_metadata(metadata: &SourceFileMetadata) -> bool {
    metadata.mode == 0o644 && metadata.dependencies.is_some()
}

fn controlled_writer_path(path: &str) -> bool {
    !path.contains("/payload/")
        && !path.contains("/owner-local/")
        && !path.ends_with("/.metadata-publication.json")
}

fn check_revision_read_closure(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    current: &postgres::Row,
    metadata: &SourceFileMetadata,
) -> DurableResult<()> {
    let prepare_id: Vec<u8> = current.get("prepare_id");
    check_revision_read_closure_for_prepare(tx, domain, &prepare_id, metadata)
}

fn check_revision_read_closure_for_prepare(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    prepare_id: &[u8],
    metadata: &SourceFileMetadata,
) -> DurableResult<()> {
    let attempt = tx
        .query_opt(
            "SELECT source_reads FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2",
            &[&domain, &prepare_id],
        )?
        .ok_or(DurableError::Corrupt(
            "source revision lineage attempt is absent",
        ))?;
    let encoded =
        attempt
            .get::<_, Option<Vec<u8>>>("source_reads")
            .ok_or(DurableError::Corrupt(
                "source revision lineage read closure is absent",
            ))?;
    let reads = decode_reads(&encoded)?;
    let dependencies = reads
        .reads
        .iter()
        .filter_map(|read| match read {
            PredicateRead::Exact { key, .. } => Some(key.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let expected_dependencies = dependencies.into_iter().collect::<Vec<_>>();
    if expected_dependencies.is_empty()
        || metadata.mode != 0o644
        || metadata.dependencies.as_ref() != Some(&expected_dependencies)
    {
        return Err(DurableError::Corrupt(
            "source revision carrier differs from its exact read closure",
        ));
    }
    Ok(())
}

fn check_initial_revision_auxiliary_membership(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    prepare_id: &[u8],
    auxiliary_path: &str,
) -> DurableResult<()> {
    let limit = as_i64((MAX_MEMBERS + 1) as u64)?;
    let members = tx.query(
        "SELECT subject,profile_id,proposed_revision,expected_revision,expected_digest
         FROM cmd2_member WHERE domain=$1 AND prepare_id=$2
         ORDER BY member_slot LIMIT $3",
        &[&domain, &prepare_id, &limit],
    )?;
    if members.is_empty() || members.len() > MAX_MEMBERS {
        return Err(DurableError::Corrupt(
            "initial revision auxiliary member set exceeds custody bound",
        ));
    }
    let auxiliary_parent = auxiliary_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(DurableError::Corrupt(
            "initial revision auxiliary parent is absent",
        ))?;
    let mut target_records = Vec::new();
    for member in &members {
        let subject: String = member.get("subject");
        let profile: String = member.get("profile_id");
        let predecessor = member.get::<_, Option<i64>>("expected_revision");
        let predecessor_digest = member.get::<_, Option<String>>("expected_digest");
        if profile.as_bytes() != REVISION || predecessor.is_none() {
            continue;
        }
        if predecessor_digest.is_none()
            || as_u64(member.get("proposed_revision"))?
                != as_u64(predecessor.expect("checked predecessor"))?
                    .checked_add(1)
                    .ok_or(DurableError::Corrupt(
                        "initial revision auxiliary predecessor overflow",
                    ))?
        {
            return Err(DurableError::Corrupt(
                "initial revision auxiliary target successor differs",
            ));
        }
        let basename = subject.rsplit('/').next().unwrap_or_default();
        if subject.starts_with("ToS/")
            && basename != "source-revision-history.json"
            && !basename.ends_with(".human-forms.json")
            && !is_revision_auxiliary_path(&subject)
        {
            target_records.push(subject);
        }
    }
    if target_records.len() != 1 {
        return Err(DurableError::Corrupt(
            "initial revision auxiliary lacks one exact target successor",
        ));
    }
    if auxiliary_path.ends_with("/source-revision-history.json")
        && target_records[0].rsplit_once('/').map(|(parent, _)| parent) != Some(auxiliary_parent)
    {
        return Err(DurableError::Corrupt(
            "initial revision history and target member parents differ",
        ));
    }
    Ok(())
}

fn is_revision_auxiliary_path(path: &str) -> bool {
    if let Some(rest) = path.strip_prefix("ToS/source-witnesses/.record-revisions/") {
        let mut parts = rest.split('/');
        let Some(directory) = parts.next() else {
            return false;
        };
        let Some(leaf) = parts.next() else {
            return false;
        };
        if parts.next().is_some() {
            return false;
        }
        let mut digests = directory.split('-');
        let (Some(identity), Some(revision), None) =
            (digests.next(), digests.next(), digests.next())
        else {
            return false;
        };
        return is_lower_hex_digest(identity)
            && is_lower_hex_digest(revision)
            && (leaf == "manifest.json"
                || leaf.strip_suffix(".blob").is_some_and(is_lower_hex_digest));
    }
    path.strip_prefix("ToS/source-witnesses/")
        .is_some_and(|rest| {
            rest == "source-revision-history.json"
                || rest.ends_with("/source-revision-history.json")
        })
}

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Refresh an explicitly invalidated projection at the assessed cut. Earlier
/// source history remains immutable; shared-dependency effects use its overlay.
/// Both cold assessors call this while holding the domain/audit snapshot.
pub(super) fn refresh_current_projection(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    current: &postgres::Row,
    projection: &[u8],
    indexed_matches: bool,
    through_seq: u64,
    definition: Digest256,
) -> DurableResult<()> {
    let path: String = current.get("subject");
    let revision: i64 = current.get("revision");
    let commit_seq: i64 = current.get("commit_seq");

    if current
        .get::<_, Option<Vec<u8>>>("inventory_projection")
        .as_deref()
        != Some(projection)
    {
        let changed = tx.execute(
            "UPDATE cmd2_current SET inventory_projection=$5
             WHERE domain=$1 AND subject=$2 AND revision=$3 AND commit_seq=$4",
            &[&domain, &path, &revision, &commit_seq, &projection],
        )?;
        if changed != 1 {
            return Err(DurableError::Conflict("cold current projection changed"));
        }
    }
    if current
        .get::<_, Option<Vec<u8>>>("retained_inventory_projection")
        .as_deref()
        != Some(projection)
    {
        // Shared evidence may be newer than this member's
        // source commit. Publish its projection at the actual
        // verified cut without rewriting earlier history.
        let effective_seq = as_i64(through_seq)?;
        let content_digest: String = current.get("content_digest");
        let definition_digest = definition.to_hex();
        tx.execute(
            "INSERT INTO cmd2_source_inventory_projection(
                domain,subject,effective_seq,source_revision,content_digest,
                definition_digest,inventory_projection)
             VALUES($1,$2,$3,$4,$5,$6,$7)
             ON CONFLICT(domain,subject,effective_seq) DO NOTHING",
            &[
                &domain,
                &path,
                &effective_seq,
                &revision,
                &content_digest,
                &definition_digest,
                &projection,
            ],
        )?;
        let retained = tx.query_one(
            "SELECT source_revision,content_digest,definition_digest,inventory_projection
             FROM cmd2_source_inventory_projection
             WHERE domain=$1 AND subject=$2 AND effective_seq=$3",
            &[&domain, &path, &effective_seq],
        )?;
        if retained.get::<_, i64>(0) != revision
            || retained.get::<_, String>(1) != content_digest
            || retained.get::<_, String>(2) != definition_digest
            || retained.get::<_, Vec<u8>>(3) != projection
        {
            return Err(DurableError::Conflict(
                "cold inventory projection cut already has a different value",
            ));
        }
    }
    if !indexed_matches {
        let changed = tx.execute(
            "UPDATE cmd2_source_index SET inventory_projection=$3
             WHERE domain=$1 AND kind='path' AND token=$2 AND path=$2
               AND definition_digest=$4",
            &[&domain, &path, &projection, &definition.to_hex()],
        )?;
        if changed != 1 {
            return Err(DurableError::Conflict("cold indexed projection changed"));
        }
    }
    Ok(())
}
