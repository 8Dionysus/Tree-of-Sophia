//! Multivalued point index for identities declared by public semantic
//! annotation packets. The table is an owner-derived reservation aid only;
//! packet bytes and their semantic validation remain in ToS source owners.

use crate::{DurableError, DurableResult};
use postgres::Transaction;
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{Digest256, JsonMode, JsonValue, RelativePath};

pub(crate) const PREDICATE_SCOPE: &str = "semantic-annotation-entity";
pub(crate) const MAX_FILES: usize = 1_024;
pub(crate) const MAX_BYTES: usize = 8_388_608;
const MAX_MEMBER_BYTES: usize = 1_048_576;
const MAX_ROWS: usize = 65_536;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct AnnotationEntityOwner {
    pub path: String,
    pub member_digest: Digest256,
}

pub(crate) fn is_packet(path: &str) -> bool {
    let leaf = path.rsplit('/').next().unwrap_or(path);
    path.starts_with("ToS/source-witnesses/")
        && !path.starts_with("ToS/source-witnesses/owner-local/")
        && leaf.starts_with("semantic-annotation")
        && leaf.ends_with(".json")
        && !path
            .split('/')
            .any(|part| matches!(part, "payload" | "local-content" | "catalog"))
}

fn validate_path(path: &str) -> DurableResult<()> {
    if !path.starts_with("ToS/source-witnesses/")
        || path.starts_with("ToS/source-witnesses/owner-local/")
        || RelativePath::parse(path).is_err()
    {
        return Err(DurableError::Corrupt("annotation identity index path"));
    }
    Ok(())
}

fn member_rows(path: &str, bytes: &[u8]) -> DurableResult<BTreeSet<String>> {
    validate_path(path)?;
    if bytes.len() > MAX_MEMBER_BYTES {
        return Err(DurableError::Refused(
            "semantic annotation member exceeds identity index bound",
        ));
    }
    let packet = tos_foundation::parse_json(
        bytes,
        JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::default(),
    )
    .map_err(|_| DurableError::Corrupt("semantic annotation identity packet JSON"))?
    .into_root();
    if packet
        .object_get("schema_version")
        .and_then(JsonValue::as_str)
        != Some("tos_semantic_annotation_packet_v2")
    {
        return Err(DurableError::Corrupt(
            "semantic annotation identity packet version",
        ));
    }
    let entities = packet
        .object_get("entities")
        .and_then(JsonValue::as_array)
        .ok_or(DurableError::Corrupt(
            "semantic annotation identity packet entities",
        ))?;
    let mut rows = BTreeSet::new();
    for entity in entities {
        let id = entity
            .object_get("entity_id")
            .and_then(JsonValue::as_str)
            .ok_or(DurableError::Corrupt("semantic annotation entity identity"))?;
        if id.is_empty() || id.len() > 4_096 {
            return Err(DurableError::Corrupt("semantic annotation entity identity"));
        }
        rows.insert(id.to_owned());
    }
    Ok(rows)
}

fn as_i64(value: usize) -> DurableResult<i64> {
    i64::try_from(value).map_err(|_| DurableError::Refused("annotation index count overflow"))
}

fn prior_member_rows(
    tx: &mut Transaction<'_>,
    domain: &str,
    path: &str,
    definition: Digest256,
) -> DurableResult<BTreeMap<String, Digest256>> {
    let rows = tx.query(
        "SELECT token,member_digest,definition_digest FROM cmd2_source_annotation_identity
         WHERE domain=$1 AND path COLLATE \"C\"=$2 COLLATE \"C\"
         ORDER BY token COLLATE \"C\" LIMIT $3",
        &[&domain, &path, &as_i64(MAX_ROWS + 1)?],
    )?;
    if rows.len() > MAX_ROWS {
        return Err(DurableError::Corrupt(
            "annotation identity member row bound",
        ));
    }
    let mut out = BTreeMap::new();
    for row in rows {
        if row.get::<_, String>(2) != definition.to_hex() {
            return Err(DurableError::Conflict(
                "annotation identity definition differs",
            ));
        }
        let token: String = row.get(0);
        let digest = Digest256::from_hex(&row.get::<_, String>(1))
            .map_err(|_| DurableError::Corrupt("annotation identity member digest"))?;
        if out.insert(token, digest).is_some() {
            return Err(DurableError::Corrupt(
                "duplicate annotation identity member row",
            ));
        }
    }
    Ok(out)
}

fn advance_generations(
    tx: &mut Transaction<'_>,
    domain: &str,
    definition: Digest256,
    tokens: &BTreeSet<String>,
    repair: bool,
) -> DurableResult<()> {
    for token in tokens {
        tx.execute(
            "INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version,generation,complete)
             VALUES($1,'unique',$2,$3,$4,$5,0,true) ON CONFLICT DO NOTHING",
            &[&domain, &crate::source_cohort::OWNER, &PREDICATE_SCOPE, token, &definition.to_hex()],
        )?;
        let update = if repair {
            tx.execute(
                "UPDATE cmd2_predicate SET generation=generation+1,complete=true,definition_version=$5
                 WHERE domain=$1 AND kind='unique' AND owner=$2 AND scope=$3 AND token=$4
                   AND generation<9223372036854775807",
                &[&domain, &crate::source_cohort::OWNER, &PREDICATE_SCOPE, token, &definition.to_hex()],
            )?
        } else {
            let row = tx.query_one(
                "SELECT complete,definition_version,generation FROM cmd2_predicate
                 WHERE domain=$1 AND kind='unique' AND owner=$2 AND scope=$3 AND token=$4 FOR UPDATE",
                &[&domain, &crate::source_cohort::OWNER, &PREDICATE_SCOPE, token],
            )?;
            if !row.get::<_, bool>(0)
                || row.get::<_, String>(1) != definition.to_hex()
                || row.get::<_, i64>(2) >= i64::MAX
            {
                return Err(DurableError::Conflict(
                    "annotation identity predicate changed",
                ));
            }
            tx.execute(
                "UPDATE cmd2_predicate SET generation=generation+1
                 WHERE domain=$1 AND kind='unique' AND owner=$2 AND scope=$3 AND token=$4
                   AND complete AND definition_version=$5 AND generation<9223372036854775807",
                &[
                    &domain,
                    &crate::source_cohort::OWNER,
                    &PREDICATE_SCOPE,
                    token,
                    &definition.to_hex(),
                ],
            )?
        };
        if update != 1 {
            return Err(DurableError::Conflict(
                "annotation identity predicate generation",
            ));
        }
    }
    Ok(())
}

pub(crate) fn replace_current_members<'a, I>(
    tx: &mut Transaction<'_>,
    domain: &str,
    members: I,
    definition: Digest256,
) -> DurableResult<()>
where
    I: IntoIterator<Item = (&'a str, Option<&'a [u8]>)>,
{
    let mut seen = BTreeSet::new();
    let mut changed = BTreeSet::new();
    for (path, bytes) in members {
        if !seen.insert(path.to_owned()) {
            return Err(DurableError::Invalid(
                "duplicate annotation identity replacement path",
            ));
        }
        let old = prior_member_rows(tx, domain, path, definition)?;
        let new = match bytes {
            Some(raw) if is_packet(path) => member_rows(path, raw)?,
            _ => BTreeSet::new(),
        };
        for token in old.keys().chain(new.iter()) {
            if old.contains_key(token) != new.contains(token) {
                changed.insert(token.clone());
            }
        }
        let deleted = tx.execute(
            "DELETE FROM cmd2_source_annotation_identity WHERE domain=$1 AND path COLLATE \"C\"=$2 COLLATE \"C\"",
            &[&domain, &path],
        )?;
        if deleted != old.len() as u64 {
            return Err(DurableError::Conflict(
                "annotation identity contribution changed",
            ));
        }
        if let Some(raw) = bytes.filter(|_| is_packet(path)) {
            let member_digest = Digest256::of_bytes(raw).to_hex();
            for token in new {
                tx.execute(
                    "INSERT INTO cmd2_source_annotation_identity(domain,token,path,member_digest,definition_digest)
                     VALUES($1,$2,$3,$4,$5)",
                    &[&domain, &token, &path, &member_digest, &definition.to_hex()],
                )?;
            }
        }
    }
    advance_generations(tx, domain, definition, &changed, false)
}

fn current_rows(
    files: &BTreeMap<String, Vec<u8>>,
) -> DurableResult<BTreeMap<(String, String), Digest256>> {
    let mut out = BTreeMap::new();
    let mut paths = 0usize;
    let mut bytes = 0usize;
    for (path, raw) in files {
        if !is_packet(path) {
            continue;
        }
        paths += 1;
        bytes = bytes
            .checked_add(raw.len())
            .filter(|total| *total <= MAX_BYTES)
            .ok_or(DurableError::Refused(
                "annotation identity inventory byte bound",
            ))?;
        if paths > MAX_FILES {
            return Err(DurableError::Refused(
                "annotation identity inventory member bound",
            ));
        }
        let digest = Digest256::of_bytes(raw);
        for token in member_rows(path, raw)? {
            if out.len() >= MAX_ROWS || out.insert((token, path.clone()), digest).is_some() {
                return Err(DurableError::Refused(
                    "annotation identity inventory row bound",
                ));
            }
        }
    }
    Ok(out)
}

/// Exact unique-predicate tokens derived from current public annotation
/// packets. The caller adds retained generations from the durable predicate
/// table separately.
pub(crate) fn current_predicate_tokens(
    files: &BTreeMap<String, Vec<u8>>,
) -> DurableResult<BTreeSet<String>> {
    Ok(current_rows(files)?
        .into_keys()
        .map(|(token, _path)| token)
        .collect())
}

pub(crate) fn rebuild_current(
    tx: &mut Transaction<'_>,
    domain: &str,
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<()> {
    let new = current_rows(files)?;
    let old_rows = tx.query(
        "SELECT token,path,member_digest,definition_digest FROM cmd2_source_annotation_identity
         WHERE domain=$1 ORDER BY path COLLATE \"C\",token COLLATE \"C\" LIMIT $2",
        &[&domain, &as_i64(MAX_ROWS + 1)?],
    )?;
    if old_rows.len() > MAX_ROWS {
        return Err(DurableError::Refused(
            "annotation identity stored row bound",
        ));
    }
    let mut old = BTreeMap::new();
    for row in old_rows {
        let token: String = row.get(0);
        let path: String = row.get(1);
        let digest = Digest256::from_hex(&row.get::<_, String>(2))
            .map_err(|_| DurableError::Corrupt("annotation identity stored digest"))?;
        let def: String = row.get(3);
        if old.insert((token, path), (digest, def)).is_some() {
            return Err(DurableError::Corrupt(
                "duplicate annotation identity full row",
            ));
        }
    }
    let mut changed = old
        .keys()
        .map(|(token, _)| token.clone())
        .collect::<BTreeSet<_>>();
    changed.extend(new.keys().map(|(token, _)| token.clone()));
    tx.execute(
        "DELETE FROM cmd2_source_annotation_identity WHERE domain=$1",
        &[&domain],
    )?;
    let def = definition.to_hex();
    for ((token, path), member_digest) in &new {
        tx.execute(
            "INSERT INTO cmd2_source_annotation_identity(domain,token,path,member_digest,definition_digest)
             VALUES($1,$2,$3,$4,$5)",
            &[&domain, token, path, &member_digest.to_hex(), &def],
        )?;
    }
    advance_generations(tx, domain, definition, &changed, true)
}

pub(crate) fn verify_current(
    tx: &mut Transaction<'_>,
    domain: &str,
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<()> {
    let expected = current_rows(files)?;
    let rows = tx.query(
        "SELECT i.token,i.path,i.member_digest,i.definition_digest,c.content_digest
         FROM cmd2_source_annotation_identity i
         LEFT JOIN cmd2_current c ON c.domain=i.domain AND c.subject=i.path
         WHERE i.domain=$1 ORDER BY i.path COLLATE \"C\",i.token COLLATE \"C\" LIMIT $2",
        &[&domain, &as_i64(MAX_ROWS + 1)?],
    )?;
    if rows.len() > MAX_ROWS || rows.len() != expected.len() {
        return Err(DurableError::Conflict(
            "annotation identity index completeness differs",
        ));
    }
    let mut actual = BTreeMap::new();
    for row in rows {
        let token: String = row.get(0);
        let path: String = row.get(1);
        let member = Digest256::from_hex(&row.get::<_, String>(2))
            .map_err(|_| DurableError::Corrupt("annotation identity member digest"))?;
        if row.get::<_, String>(3) != definition.to_hex()
            || row.get::<_, Option<String>>(4).as_deref() != Some(member.to_hex().as_str())
            || actual.insert((token, path), member).is_some()
        {
            return Err(DurableError::Conflict(
                "annotation identity index row differs",
            ));
        }
    }
    if actual != expected {
        return Err(DurableError::Conflict(
            "annotation identity index source differs",
        ));
    }
    Ok(())
}

pub(crate) fn lookup_current_owners(
    tx: &mut Transaction<'_>,
    domain: &str,
    token: &str,
    definition: Digest256,
) -> DurableResult<BTreeSet<AnnotationEntityOwner>> {
    let rows = tx.query(
        "SELECT i.path,i.member_digest,i.definition_digest,c.content_digest
         FROM cmd2_source_annotation_identity i
         LEFT JOIN cmd2_current c ON c.domain=i.domain AND c.subject=i.path
         WHERE i.domain=$1 AND i.token COLLATE \"C\"=$2 COLLATE \"C\"
         ORDER BY i.path COLLATE \"C\" LIMIT $3",
        &[&domain, &token, &as_i64(MAX_FILES + 1)?],
    )?;
    if rows.len() > MAX_FILES {
        return Err(DurableError::Refused(
            "annotation identity point owner bound",
        ));
    }
    let mut owners = BTreeSet::new();
    for row in rows {
        let path: String = row.get(0);
        validate_path(&path)?;
        let digest = Digest256::from_hex(&row.get::<_, String>(1))
            .map_err(|_| DurableError::Corrupt("annotation identity point digest"))?;
        if row.get::<_, String>(2) != definition.to_hex()
            || row.get::<_, Option<String>>(3).as_deref() != Some(digest.to_hex().as_str())
        {
            return Err(DurableError::Conflict(
                "annotation identity point source differs",
            ));
        }
        owners.insert(AnnotationEntityOwner {
            path,
            member_digest: digest,
        });
    }
    Ok(owners)
}
