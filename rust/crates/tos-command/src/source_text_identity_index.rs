//! Normalized current-member contributions for the public Text identity
//! namespace. Authored bytes remain in the source store; this table is a
//! bounded point index, never a source or semantic admission record.

use crate::source_text_identity::{
    MAX_BYTES, MAX_FILES, MAX_POINT_OWNERS, TextIdentityOwner, is_candidate,
    text_identity_contributions,
};
use crate::{DurableError, DurableResult};
use postgres::Transaction;
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{Digest256, RelativePath};

pub(crate) const PREDICATE_SCOPE: &str = "text-identity";
const MAX_INDEX_ROWS: usize = MAX_POINT_OWNERS * 16;

/// Compact complete contribution set for a full assessment. Bodies are parsed
/// one member at a time and discarded; the retained metadata has independent
/// finite row/byte limits. This does not expand private package admission.
#[derive(Default)]
pub(crate) struct CurrentTextIdentityRows {
    rows: BTreeMap<(String, String), Digest256>,
    paths: BTreeSet<String>,
    metadata_bytes: usize,
}
impl CurrentTextIdentityRows {
    pub(crate) fn add_member(&mut self, path: &str, bytes: &[u8]) -> DurableResult<()> {
        validate_public_path(path)?;
        if !is_public_candidate_path(path) {
            return Err(DurableError::Invalid("Text identity candidate source path"));
        }
        if self.paths.len() >= MAX_FILES || !self.paths.insert(path.to_owned()) {
            return Err(DurableError::Refused(
                "Text identity candidate count/uniqueness bound",
            ));
        }
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(path.len())
            .filter(|size| *size <= MAX_BYTES)
            .ok_or(DurableError::Refused(
                "Text identity contribution metadata byte bound",
            ))?;
        for (token, member_digest) in member_rows(path, bytes)? {
            self.metadata_bytes = self
                .metadata_bytes
                .checked_add(path.len() + token.len() + 32)
                .filter(|size| *size <= MAX_BYTES)
                .ok_or(DurableError::Refused(
                    "Text identity contribution metadata byte bound",
                ))?;
            if self.rows.len() >= MAX_INDEX_ROWS
                || self
                    .rows
                    .insert((path.to_owned(), token), member_digest)
                    .is_some()
            {
                return Err(DurableError::Refused(
                    "Text identity contribution row bound",
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn tokens(&self) -> BTreeSet<String> {
        self.rows.keys().map(|(_, token)| token.clone()).collect()
    }
    fn from_files(files: &BTreeMap<String, Vec<u8>>) -> DurableResult<Self> {
        let mut selected = Self::default();
        let mut body_bytes = 0usize;
        for (path, bytes) in files {
            if path.is_empty() || path.len() > 4096 || RelativePath::parse(path).is_err() {
                return Err(DurableError::Invalid("Text identity full source path"));
            }
            if !is_public_candidate_path(path) {
                continue;
            }
            body_bytes = body_bytes
                .checked_add(bytes.len())
                .filter(|n| *n <= MAX_BYTES)
                .ok_or(DurableError::Refused(
                    "Text identity full byte bound exceeded",
                ))?;
            selected.add_member(path, bytes)?;
        }
        Ok(selected)
    }
}

#[cfg(test)]
mod compact_assessment_tests {
    use super::*;

    #[test]
    fn bootstrap_contract_text_identity_stream_retains_rows_instead_of_bodies() {
        let raw = format!(
            r#"{{"schema_version":"tos_source_text_layer_v1","layer_id":"tos.layer.fixture","text":"{}"}}"#,
            "x".repeat(1024 * 1024)
        ).into_bytes();
        let digest = Digest256::of_bytes(&raw);
        let mut rows = CurrentTextIdentityRows::default();
        for index in 0..65 {
            let path = format!("ToS/source-witnesses/text/{index}/source-text-layer.json");
            rows.add_member(&path, &raw).expect("one bounded member");
            assert_eq!(
                rows.rows.get(&(path, "tos.layer.fixture".into())),
                Some(&digest)
            );
        }
        assert!(65 * raw.len() > MAX_BYTES);
        assert_eq!(rows.rows.len(), 65);
        assert_eq!(rows.tokens(), BTreeSet::from(["tos.layer.fixture".into()]));
        assert!(rows.metadata_bytes < 64 * 1024);
        assert!(
            rows.add_member("ToS/source-witnesses/text/0/source-text-layer.json", &raw)
                .is_err()
        );
        assert!(
            CurrentTextIdentityRows::default()
                .add_member(
                    "ToS/source-witnesses/owner-local/hidden/source-text-layer.json",
                    &raw,
                )
                .is_err()
        );
    }
}

fn as_i64(value: usize) -> DurableResult<i64> {
    i64::try_from(value).map_err(|_| DurableError::Refused("Text identity index count overflow"))
}

fn digest(value: &str) -> DurableResult<Digest256> {
    Digest256::from_hex(value)
        .map_err(|_| DurableError::Corrupt("Text identity index digest invalid"))
}

fn member_rows(path: &str, bytes: &[u8]) -> DurableResult<BTreeMap<String, Digest256>> {
    let rows = text_identity_contributions(path, bytes).map_err(DurableError::Source)?;
    if rows.len() > MAX_POINT_OWNERS {
        return Err(DurableError::Refused(
            "Text identity contributions exceed member bound",
        ));
    }
    let member_digest = Digest256::of_bytes(bytes);
    Ok(rows
        .into_iter()
        .map(|row| (row.token, member_digest))
        .collect())
}

fn validate_public_path(path: &str) -> DurableResult<()> {
    if path.is_empty()
        || path.len() > 4096
        || !path.starts_with("ToS/")
        || RelativePath::parse(path).is_err()
    {
        return Err(DurableError::Invalid("Text identity source path"));
    }
    Ok(())
}

fn is_owner_local_path(path: &str) -> bool {
    path == "ToS/source-witnesses/owner-local"
        || path.starts_with("ToS/source-witnesses/owner-local/")
}

fn is_public_candidate_path(path: &str) -> bool {
    path.starts_with("ToS/")
        && !is_owner_local_path(path)
        && is_candidate(path.rsplit('/').next().unwrap_or(path))
}

fn validate_index_path(path: &str) -> DurableResult<()> {
    validate_public_path(path)?;
    if is_owner_local_path(path) || !is_candidate(path.rsplit('/').next().unwrap_or(path)) {
        return Err(DurableError::Corrupt(
            "Text identity index path is not public candidate metadata",
        ));
    }
    Ok(())
}

fn prior_member_rows(
    tx: &mut Transaction<'_>,
    domain: &str,
    path: &str,
    definition: Digest256,
) -> DurableResult<BTreeMap<String, Digest256>> {
    let limit = as_i64(MAX_POINT_OWNERS + 1)?;
    let rows = tx.query(
        "SELECT token,member_digest,definition_digest FROM cmd2_source_text_identity
         WHERE domain=$1 AND path COLLATE \"C\"=$2 COLLATE \"C\"
         ORDER BY token COLLATE \"C\" LIMIT $3",
        &[&domain, &path, &limit],
    )?;
    if rows.len() > MAX_POINT_OWNERS {
        return Err(DurableError::Corrupt(
            "Text identity member contribution bound exceeded",
        ));
    }
    let expected_definition = definition.to_hex();
    let mut result = BTreeMap::new();
    for row in rows {
        let token: String = row.get("token");
        if token.is_empty() || token.len() > 4096 {
            return Err(DurableError::Corrupt("Text identity token invalid"));
        }
        if row.get::<_, String>("definition_digest") != expected_definition {
            return Err(DurableError::Conflict(
                "Text identity index definition differs",
            ));
        }
        validate_index_path(path)?;
        if result
            .insert(token, digest(row.get("member_digest"))?)
            .is_some()
        {
            return Err(DurableError::Corrupt(
                "duplicate Text identity member contribution",
            ));
        }
    }
    Ok(result)
}

fn advance_token_generations(
    tx: &mut Transaction<'_>,
    domain: &str,
    definition: Digest256,
    tokens: &BTreeSet<String>,
    repair_incomplete: bool,
) -> DurableResult<()> {
    let definition_hex = definition.to_hex();
    for token in tokens {
        tx.execute(
            "INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version,generation,complete)
             VALUES($1,'unique',$2,$3,$4,$5,0,true) ON CONFLICT DO NOTHING",
            &[
                &domain,
                &crate::source_cohort::OWNER,
                &PREDICATE_SCOPE,
                token,
                &definition_hex,
            ],
        )?;
        let row = tx.query_one(
            "SELECT generation,complete,definition_version FROM cmd2_predicate
             WHERE domain=$1 AND kind='unique' AND owner=$2 AND scope=$3 AND token=$4 FOR UPDATE",
            &[
                &domain,
                &crate::source_cohort::OWNER,
                &PREDICATE_SCOPE,
                token,
            ],
        )?;
        if row.get::<_, i64>(0) >= i64::MAX {
            return Err(DurableError::Refused(
                "Text identity predicate generation exhausted",
            ));
        }
        if !repair_incomplete
            && (!row.get::<_, bool>(1) || row.get::<_, String>(2) != definition_hex)
        {
            return Err(DurableError::Refused(
                "Text identity predicate is incomplete or has another definition",
            ));
        }
        let changed = if repair_incomplete {
            tx.execute(
                "UPDATE cmd2_predicate SET generation=generation+1,complete=true,definition_version=$5
                 WHERE domain=$1 AND kind='unique' AND owner=$2 AND scope=$3 AND token=$4
                   AND generation<9223372036854775807",
                &[
                    &domain,
                    &crate::source_cohort::OWNER,
                    &PREDICATE_SCOPE,
                    token,
                    &definition_hex,
                ],
            )?
        } else {
            tx.execute(
                "UPDATE cmd2_predicate SET generation=generation+1
                 WHERE domain=$1 AND kind='unique' AND owner=$2 AND scope=$3 AND token=$4
                   AND complete AND definition_version=$5 AND generation<9223372036854775807",
                &[
                    &domain,
                    &crate::source_cohort::OWNER,
                    &PREDICATE_SCOPE,
                    token,
                    &definition_hex,
                ],
            )?
        };
        if changed != 1 {
            return Err(DurableError::Conflict(
                "Text identity predicate generation changed",
            ));
        }
    }
    Ok(())
}

/// Replace only the changed source paths. Old and new `(token,path,digest)`
/// facts are compared before mutation, and every affected token generation is
/// advanced once in the same caller transaction.
pub(crate) fn replace_current_members<'a, I>(
    tx: &mut Transaction<'_>,
    domain: &str,
    members: I,
    definition: Digest256,
) -> DurableResult<()>
where
    I: IntoIterator<Item = (&'a str, Option<&'a [u8]>)>,
{
    let mut seen_paths = BTreeSet::new();
    let mut changed_tokens = BTreeSet::new();
    for (path, bytes) in members {
        validate_public_path(path)?;
        if !seen_paths.insert(path.to_owned()) {
            return Err(DurableError::Invalid(
                "duplicate Text identity replacement path",
            ));
        }
        let old = prior_member_rows(tx, domain, path, definition)?;
        let new = match bytes {
            Some(bytes) if is_public_candidate_path(path) => member_rows(path, bytes)?,
            None => BTreeMap::new(),
            Some(_) => BTreeMap::new(),
        };
        for token in old.keys().chain(new.keys()) {
            if old.get(token) != new.get(token) {
                changed_tokens.insert(token.clone());
            }
        }
        let deleted = tx.execute(
            "DELETE FROM cmd2_source_text_identity WHERE domain=$1 AND path COLLATE \"C\"=$2 COLLATE \"C\"",
            &[&domain, &path],
        )?;
        if deleted != old.len() as u64 {
            return Err(DurableError::Conflict(
                "Text identity member contribution changed during replacement",
            ));
        }
        let member_digest = bytes.map(Digest256::of_bytes).map(|value| value.to_hex());
        if let Some(member_digest) = member_digest {
            let definition_digest = definition.to_hex();
            for token in new.keys() {
                tx.execute(
                    "INSERT INTO cmd2_source_text_identity(domain,token,path,member_digest,definition_digest)
                     VALUES($1,$2,$3,$4,$5)",
                    &[&domain, token, &path, &member_digest, &definition_digest],
                )?;
            }
        }
    }
    advance_token_generations(tx, domain, definition, &changed_tokens, false)
}

/// Rebuild the complete public Text point index during a selected full
/// bootstrap/recovery. Ordinary member writes use `replace_current_members`.
pub(crate) fn rebuild_current(
    tx: &mut Transaction<'_>,
    domain: &str,
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<()> {
    rebuild_current_rows(
        tx,
        domain,
        &CurrentTextIdentityRows::from_files(files)?,
        definition,
    )
}

pub(crate) fn rebuild_current_rows(
    tx: &mut Transaction<'_>,
    domain: &str,
    contributions: &CurrentTextIdentityRows,
    definition: Digest256,
) -> DurableResult<()> {
    let row_limit = as_i64(MAX_INDEX_ROWS + 1)?;
    let old_rows = tx.query(
        "SELECT token,path,member_digest,definition_digest FROM cmd2_source_text_identity
         WHERE domain=$1 ORDER BY path COLLATE \"C\",token COLLATE \"C\" LIMIT $2",
        &[&domain, &row_limit],
    )?;
    if old_rows.len() > MAX_INDEX_ROWS {
        return Err(DurableError::Refused(
            "Text identity full index row bound exceeded",
        ));
    }
    let mut old = BTreeMap::<(String, String), (Digest256, String)>::new();
    for row in old_rows {
        let token: String = row.get("token");
        let path: String = row.get("path");
        validate_public_path(&path)?;
        let value = (
            digest(row.get("member_digest"))?,
            row.get("definition_digest"),
        );
        if old.insert((path, token), value).is_some() {
            return Err(DurableError::Corrupt(
                "duplicate Text identity full index row",
            ));
        }
    }

    let new = &contributions.rows;

    let predicate_limit = as_i64(MAX_INDEX_ROWS + 1)?;
    let predicate_rows = tx.query(
        "SELECT token FROM cmd2_predicate
         WHERE domain=$1 AND kind='unique' AND owner=$2 AND scope=$3
         ORDER BY token COLLATE \"C\" LIMIT $4",
        &[
            &domain,
            &crate::source_cohort::OWNER,
            &PREDICATE_SCOPE,
            &predicate_limit,
        ],
    )?;
    if predicate_rows.len() > MAX_INDEX_ROWS {
        return Err(DurableError::Refused(
            "Text identity predicate rebuild bound exceeded",
        ));
    }
    let mut changed_tokens = predicate_rows
        .into_iter()
        .map(|row| row.get::<_, String>("token"))
        .collect::<BTreeSet<_>>();
    for (key, (old_digest, old_definition)) in &old {
        if new.get(key) != Some(old_digest) || old_definition != &definition.to_hex() {
            changed_tokens.insert(key.1.clone());
        }
    }
    changed_tokens.extend(new.keys().map(|(_, token)| token.clone()));
    if changed_tokens
        .iter()
        .any(|token| token.is_empty() || token.len() > 4096)
    {
        return Err(DurableError::Corrupt(
            "Text identity rebuild predicate token invalid",
        ));
    }
    for key in new.keys() {
        let unchanged = old.get(key).is_some_and(|(old_digest, old_definition)| {
            Some(old_digest) == new.get(key) && old_definition == &definition.to_hex()
        });
        if !unchanged {
            changed_tokens.insert(key.1.clone());
        }
    }
    tx.execute(
        "DELETE FROM cmd2_source_text_identity WHERE domain=$1",
        &[&domain],
    )?;
    let definition_digest = definition.to_hex();
    for ((path, token), member_digest) in new {
        let member_digest = member_digest.to_hex();
        tx.execute(
            "INSERT INTO cmd2_source_text_identity(domain,token,path,member_digest,definition_digest)
             VALUES($1,$2,$3,$4,$5)",
            &[&domain, token, path, &member_digest, &definition_digest],
        )?;
    }
    advance_token_generations(tx, domain, definition, &changed_tokens, true)
}

/// Check that the persisted index exactly matches a complete set of current
/// candidate member bodies. Callers must supply the full current candidate
/// cohort selected by the same source/audit fence; this is intended for cold
/// equality, where an empty persisted table must not stand in for absence.
/// Maintenance may have cleared completeness. Verify rows, definitions and
/// generations before the full cold owner re-enables it under the same fence.
pub(crate) fn verify_for_reassessment(
    tx: &mut Transaction<'_>,
    domain: &str,
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<()> {
    verify_rows_for_reassessment(
        tx,
        domain,
        &CurrentTextIdentityRows::from_files(files)?,
        definition,
    )
}

pub(crate) fn verify_rows_for_reassessment(
    tx: &mut Transaction<'_>,
    domain: &str,
    contributions: &CurrentTextIdentityRows,
    definition: Digest256,
) -> DurableResult<()> {
    let expected = &contributions.rows;

    let row_limit = as_i64(MAX_INDEX_ROWS + 1)?;
    let rows = tx.query(
        "SELECT i.token,i.path,i.member_digest,i.definition_digest,c.content_digest
         FROM cmd2_source_text_identity i
         LEFT JOIN cmd2_current c ON c.domain=i.domain AND c.subject=i.path
         WHERE i.domain=$1
         ORDER BY i.path COLLATE \"C\",i.token COLLATE \"C\" LIMIT $2",
        &[&domain, &row_limit],
    )?;
    if rows.len() > MAX_INDEX_ROWS {
        return Err(DurableError::Corrupt(
            "Text identity full index row bound exceeded",
        ));
    }
    let expected_definition = definition.to_hex();
    let mut observed = BTreeMap::<(String, String), Digest256>::new();
    for row in rows {
        let token: String = row.get("token");
        let path: String = row.get("path");
        validate_index_path(&path)?;
        let member_digest = digest(row.get("member_digest"))?;
        let current_digest =
            row.get::<_, Option<String>>("content_digest")
                .ok_or(DurableError::Corrupt(
                    "Text identity owner is absent from current source",
                ))?;
        if row.get::<_, String>("definition_digest") != expected_definition
            || digest(&current_digest)? != member_digest
        {
            return Err(DurableError::Conflict(
                "Text identity cold row definition or current digest differs",
            ));
        }
        if observed.insert((path, token), member_digest).is_some() {
            return Err(DurableError::Corrupt("duplicate Text identity cold row"));
        }
    }
    if &observed != expected {
        return Err(DurableError::Conflict(
            "Text identity cold rowset differs from current candidates",
        ));
    }
    if tx
        .query_opt(
            "SELECT 1
             FROM cmd2_source_text_identity i
             LEFT JOIN cmd2_predicate p
               ON p.domain=i.domain AND p.kind='unique' AND p.owner=$2
              AND p.scope=$3 AND p.token=i.token
             WHERE i.domain=$1
               AND (p.token IS NULL OR p.definition_version<>$4 OR p.generation<=0)
             LIMIT 1",
            &[
                &domain,
                &crate::source_cohort::OWNER,
                &PREDICATE_SCOPE,
                &expected_definition,
            ],
        )?
        .is_some()
    {
        return Err(DurableError::Corrupt(
            "Text identity owner lacks a complete current predicate generation",
        ));
    }
    Ok(())
}

/// Resolve the bounded current owner set for one batch of requested tokens.
/// The indexed digest is checked against the source store's current locator so
/// stale or forged contribution rows cannot authorize retained ancestry.
pub(crate) fn lookup_current_owners(
    tx: &mut Transaction<'_>,
    domain: &str,
    tokens: &BTreeSet<String>,
    definition: Digest256,
) -> DurableResult<BTreeMap<String, BTreeSet<TextIdentityOwner>>> {
    if tokens.len() > MAX_POINT_OWNERS {
        return Err(DurableError::Refused(
            "Text identity requested token bound exceeded",
        ));
    }
    for token in tokens {
        if token.is_empty() || token.len() > 4096 {
            return Err(DurableError::Invalid("Text identity lookup token"));
        }
    }
    let tokens = tokens.iter().cloned().collect::<Vec<_>>();
    let limit = as_i64(MAX_POINT_OWNERS + 1)?;
    let rows = tx.query(
        "SELECT i.token,i.path,i.member_digest,i.definition_digest,c.content_digest
         FROM cmd2_source_text_identity i
         LEFT JOIN cmd2_current c ON c.domain=i.domain AND c.subject=i.path
         WHERE i.domain=$1 AND i.token=ANY($2)
         ORDER BY i.token COLLATE \"C\",i.path COLLATE \"C\" LIMIT $3",
        &[&domain, &tokens, &limit],
    )?;
    if rows.len() > MAX_POINT_OWNERS {
        return Err(DurableError::Refused(
            "Text identity point result bound exceeded",
        ));
    }
    let expected_definition = definition.to_hex();
    let mut result = tokens
        .into_iter()
        .map(|token| (token, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for row in rows {
        let token: String = row.get("token");
        let path: String = row.get("path");
        validate_index_path(&path)?;
        if row.get::<_, String>("definition_digest") != expected_definition {
            return Err(DurableError::Conflict(
                "Text identity point definition differs",
            ));
        }
        let member_digest = digest(row.get("member_digest"))?;
        let current_digest =
            row.get::<_, Option<String>>("content_digest")
                .ok_or(DurableError::Corrupt(
                    "Text identity owner is absent from current source",
                ))?;
        if digest(&current_digest)? != member_digest {
            return Err(DurableError::Corrupt(
                "Text identity member digest differs from current source",
            ));
        }
        let owners = result
            .get_mut(&token)
            .ok_or(DurableError::Corrupt("Text identity result token absent"))?;
        if !owners.insert(TextIdentityOwner {
            path,
            member_digest,
        }) {
            return Err(DurableError::Corrupt(
                "duplicate Text identity current owner",
            ));
        }
    }
    Ok(result)
}
