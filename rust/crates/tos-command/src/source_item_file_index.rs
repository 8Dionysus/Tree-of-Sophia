//! Typed current Item-manifest File, Rights, and Edition associations for
//! bounded Item adoption. Payload bytes stay in protected custody; this
//! audited index contains only manifest identity and exact source membership.
use crate::{DurableError, DurableResult};
use postgres::Transaction;
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{Digest256, Digest256Hasher, JsonValue, RelativePath};

pub(crate) const PREDICATE_SCOPE: &str = "item-file-v1";
pub(crate) const RIGHTS_PREDICATE_SCOPE: &str = "item-rights-v1";
pub(crate) const EDITION_PREDICATE_SCOPE: &str = "item-edition-v1";
pub(crate) const MAX_MANIFEST_COUNT: usize = 4_096;
pub(crate) const MAX_ASSIGNMENT_COUNT: usize = 65_536;
pub(crate) const MAX_ASSIGNMENTS_PER_MANIFEST: usize = 4_096;
pub(crate) const MAX_MANIFEST_BYTES: usize = 2_097_152;
pub(crate) const MAX_TOTAL_MANIFEST_BYTES: usize = 33_554_432;
const MAX_PATH_BYTES: usize = 4_096;
const MAX_ID_BYTES: usize = 4_096;
pub(crate) const MAX_LOOKUP_ROWS: usize = 8_192;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ItemFileAssignment {
    pub(crate) item_id: String,
    pub(crate) manifest_path: String,
    pub(crate) payload_ordinal: usize,
    pub(crate) file_id: String,
    pub(crate) member_digest: Digest256,
    pub(crate) rights_id: String,
    pub(crate) rights_digest: Digest256,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ItemRightsAssignment {
    pub(crate) item_id: String,
    pub(crate) manifest_path: String,
    pub(crate) manifest_digest: Digest256,
    pub(crate) rights_id: String,
    pub(crate) rights_digest: Digest256,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ItemEditionAssignment {
    pub(crate) item_id: String,
    pub(crate) manifest_path: String,
    pub(crate) manifest_digest: Digest256,
}

#[derive(Clone, Debug)]
pub(crate) struct ItemFileObservation {
    pub(crate) rows: BTreeSet<ItemFileAssignment>,
    pub(crate) predicate_token: String,
    pub(crate) generation: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct ItemRightsObservation {
    pub(crate) rows: BTreeSet<ItemRightsAssignment>,
    pub(crate) predicate_token: String,
    pub(crate) generation: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct ItemEditionObservation {
    pub(crate) rows: BTreeSet<ItemEditionAssignment>,
    pub(crate) predicate_token: String,
    pub(crate) generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndexRow {
    item_id: String,
    manifest_path: String,
    edition_id: String,
    payload_ordinal: usize,
    file_id: String,
    member_digest: Digest256,
    rights_id: String,
    rights_digest: Digest256,
    definition_digest: Digest256,
}

impl IndexRow {
    fn assignment(&self) -> ItemFileAssignment {
        ItemFileAssignment {
            item_id: self.item_id.clone(),
            manifest_path: self.manifest_path.clone(),
            payload_ordinal: self.payload_ordinal,
            file_id: self.file_id.clone(),
            member_digest: self.member_digest,
            rights_id: self.rights_id.clone(),
            rights_digest: self.rights_digest,
        }
    }
}

fn as_i64(value: usize) -> DurableResult<i64> {
    i64::try_from(value).map_err(|_| DurableError::Refused("Item File index count overflow"))
}

fn digest(value: &str) -> DurableResult<Digest256> {
    Digest256::from_hex(value).map_err(|_| DurableError::Corrupt("Item File index digest invalid"))
}

fn parse(raw: &[u8]) -> DurableResult<JsonValue> {
    crate::source_command::parse(raw)
        .map_err(|_| DurableError::Corrupt("Item File source JSON invalid"))
}

fn text<'a>(value: &'a JsonValue, key: &str) -> DurableResult<&'a str> {
    value
        .object_get(key)
        .and_then(JsonValue::as_str)
        .ok_or(DurableError::Corrupt("Item File source field invalid"))
}

pub(crate) fn is_candidate_manifest_path(path: &str) -> bool {
    let parts = path.split('/').collect::<Vec<_>>();
    parts.len() >= 7
        && parts[0] == "ToS"
        && parts[1] == "source-witnesses"
        && parts[parts.len() - 3] == "items"
        && !parts[parts.len() - 2].is_empty()
        && parts[parts.len() - 1] == "item.manifest.json"
        && !parts[2..parts.len() - 2].contains(&"owner-local")
        && !parts.contains(&".record-revisions")
}

pub(crate) fn is_candidate_item_record_path(path: &str) -> bool {
    let parts = path.split('/').collect::<Vec<_>>();
    parts.len() >= 7
        && parts[0] == "ToS"
        && parts[1] == "source-witnesses"
        && parts[parts.len() - 3] == "items"
        && !parts[parts.len() - 2].is_empty()
        && parts[parts.len() - 1] == "item.json"
        && !parts[2..parts.len() - 2].contains(&"owner-local")
        && !parts.contains(&".record-revisions")
}

pub(crate) fn is_candidate_item_rights_path(path: &str) -> bool {
    let parts = path.split('/').collect::<Vec<_>>();
    parts.len() >= 7
        && parts[0] == "ToS"
        && parts[1] == "source-witnesses"
        && parts[parts.len() - 3] == "items"
        && !parts[parts.len() - 2].is_empty()
        && parts[parts.len() - 1] == "rights.json"
        && !parts[2..parts.len() - 2].contains(&"owner-local")
        && !parts.contains(&".record-revisions")
}

fn validate_public_path(path: &str) -> DurableResult<()> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || !path.starts_with("ToS/")
        || RelativePath::parse(path).is_err()
        || path == "ToS/source-witnesses/owner-local"
        || path.starts_with("ToS/source-witnesses/owner-local/")
    {
        return Err(DurableError::Invalid("Item File source path"));
    }
    Ok(())
}

fn item_record_path(manifest_path: &str) -> DurableResult<String> {
    let home = manifest_path
        .strip_suffix("/item.manifest.json")
        .ok_or(DurableError::Corrupt("Item File manifest path invalid"))?;
    Ok(format!("{home}/item.json"))
}

fn rights_record_path(manifest_path: &str) -> DurableResult<String> {
    let home = manifest_path
        .strip_suffix("/item.manifest.json")
        .ok_or(DurableError::Corrupt("Item File manifest path invalid"))?;
    Ok(format!("{home}/rights.json"))
}

fn validate_item_manifest(
    path: &str,
    bytes: &[u8],
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<BTreeMap<(String, usize), IndexRow>> {
    validate_public_path(path)?;
    if !is_candidate_manifest_path(path) || bytes.len() > MAX_MANIFEST_BYTES {
        return Err(DurableError::Refused(
            "Item File manifest path or size bound exceeded",
        ));
    }
    let manifest = parse(bytes)?;
    if text(&manifest, "schema_version")? != "tos_source_item_manifest_v1" {
        return Err(DurableError::Corrupt("Item File manifest schema differs"));
    }
    let item_id = text(&manifest, "item_id")?;
    if item_id.len() > MAX_ID_BYTES || !item_id.starts_with("tos.item.") {
        return Err(DurableError::Corrupt(
            "Item File manifest Item identity invalid",
        ));
    }
    let edition_id = text(&manifest, "embodiment_ref")?;
    if edition_id.len() > MAX_ID_BYTES || !edition_id.starts_with("tos.edition.") {
        return Err(DurableError::Corrupt(
            "Item File manifest Edition identity invalid",
        ));
    }
    manifest_record(files, path, item_id)?;
    let home = path
        .strip_suffix("/item.manifest.json")
        .ok_or(DurableError::Corrupt("Item File manifest path invalid"))?;
    let expected_rights = format!("{home}/rights.json");
    if text(&manifest, "rights_ref")? != expected_rights {
        return Err(DurableError::Corrupt("Item rights locator differs"));
    }
    let rights_raw = files
        .get(&expected_rights)
        .ok_or(DurableError::Corrupt("Item rights source absent"))?;
    if rights_raw.len() > MAX_MANIFEST_BYTES {
        return Err(DurableError::Refused(
            "Item rights member byte bound exceeded",
        ));
    }
    let (rights_id, rights_digest) = parse_rights_member(rights_raw)?;
    let payload_files = manifest
        .object_get("payload_files")
        .and_then(JsonValue::as_array)
        .ok_or(DurableError::Corrupt(
            "Item File manifest assignments invalid",
        ))?;
    if payload_files.is_empty() || payload_files.len() > MAX_ASSIGNMENTS_PER_MANIFEST {
        return Err(DurableError::Refused(
            "Item File manifest assignment bound exceeded",
        ));
    }
    let member_digest = Digest256::of_bytes(bytes);
    let mut rows = BTreeMap::new();
    for (ordinal, assignment) in payload_files.iter().enumerate() {
        let file_id = text(assignment, "file_id")?;
        if file_id.len() > MAX_ID_BYTES || !file_id.starts_with("tos.file.") {
            return Err(DurableError::Corrupt("Item File identity invalid"));
        }
        let row = IndexRow {
            item_id: item_id.to_owned(),
            manifest_path: path.to_owned(),
            edition_id: edition_id.to_owned(),
            payload_ordinal: ordinal,
            file_id: file_id.to_owned(),
            member_digest,
            rights_id: rights_id.clone(),
            rights_digest,
            definition_digest: definition,
        };
        if rows.insert((path.to_owned(), ordinal), row).is_some() {
            return Err(DurableError::Corrupt("duplicate Item File ordinal"));
        }
    }
    Ok(rows)
}

fn manifest_record(
    files: &BTreeMap<String, Vec<u8>>,
    manifest_path: &str,
    item_id: &str,
) -> DurableResult<()> {
    let record_path = item_record_path(manifest_path)?;
    let bytes = files
        .get(&record_path)
        .ok_or(DurableError::Corrupt("Item File source record absent"))?;
    let record = parse(bytes)?;
    if text(&record, "record_type")? != "item"
        || text(&record, "record_id")? != item_id
        || text(&record, "item_manifest_ref")? != manifest_path
    {
        return Err(DurableError::Corrupt(
            "Item File source record binding differs",
        ));
    }
    Ok(())
}

fn full_source_rows(
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<BTreeMap<(String, usize), IndexRow>> {
    let mut manifests = 0usize;
    let mut total_manifest_bytes = 0usize;
    let mut rows = BTreeMap::new();
    let mut identities = BTreeMap::<String, String>::new();
    for (path, bytes) in files {
        validate_public_path(path)?;
        if !is_candidate_manifest_path(path) {
            continue;
        }
        manifests += 1;
        if manifests > MAX_MANIFEST_COUNT {
            return Err(DurableError::Refused(
                "Item File manifest count bound exceeded",
            ));
        }
        total_manifest_bytes = total_manifest_bytes
            .checked_add(bytes.len())
            .filter(|count| *count <= MAX_TOTAL_MANIFEST_BYTES)
            .ok_or(DurableError::Refused(
                "Item File aggregate manifest byte bound exceeded",
            ))?;
        let member_rows = validate_item_manifest(path, bytes, files, definition)?;
        let item_id = member_rows
            .values()
            .next()
            .map(|row| row.item_id.as_str())
            .ok_or(DurableError::Corrupt(
                "Item File manifest has no assignment",
            ))?;
        manifest_record(files, path, item_id)?;
        if identities
            .insert(item_id.to_owned(), path.to_owned())
            .is_some()
        {
            return Err(DurableError::Corrupt("duplicate Item File Item identity"));
        }
        for (key, row) in member_rows {
            if rows.insert(key, row).is_some() {
                return Err(DurableError::Corrupt("duplicate Item File index row"));
            }
            if rows.len() > MAX_ASSIGNMENT_COUNT {
                return Err(DurableError::Refused(
                    "Item File full index row bound exceeded",
                ));
            }
        }
    }
    Ok(rows)
}

fn read_manifest_rows(
    tx: &mut Transaction<'_>,
    domain: &str,
    path: &str,
    definition: Digest256,
) -> DurableResult<BTreeMap<(String, usize), IndexRow>> {
    let limit = as_i64(MAX_ASSIGNMENTS_PER_MANIFEST + 1)?;
    let rows = tx.query(
        "SELECT item_id,manifest_path,edition_id,payload_ordinal,file_id,member_digest,
                rights_id,rights_digest,definition_digest
         FROM cmd2_source_item_file_assignment
         WHERE domain=$1 AND manifest_path COLLATE \"C\"=$2 COLLATE \"C\"
         ORDER BY payload_ordinal LIMIT $3",
        &[&domain, &path, &limit],
    )?;
    if rows.len() > MAX_ASSIGNMENTS_PER_MANIFEST {
        return Err(DurableError::Corrupt(
            "Item File member index bound exceeded",
        ));
    }
    let expected_definition = definition.to_hex();
    let mut result = BTreeMap::new();
    for row in rows {
        let index_row = IndexRow {
            item_id: row.get("item_id"),
            manifest_path: row.get("manifest_path"),
            edition_id: row.get("edition_id"),
            payload_ordinal: usize::try_from(row.get::<_, i32>("payload_ordinal"))
                .map_err(|_| DurableError::Corrupt("Item File ordinal invalid"))?,
            file_id: row.get("file_id"),
            member_digest: digest(row.get("member_digest"))?,
            rights_id: row.get("rights_id"),
            rights_digest: digest(row.get("rights_digest"))?,
            definition_digest: digest(row.get("definition_digest"))?,
        };
        validate_index_row(&index_row)?;
        if index_row.definition_digest.to_hex() != expected_definition {
            return Err(DurableError::Conflict("Item File index definition differs"));
        }
        let key = (index_row.manifest_path.clone(), index_row.payload_ordinal);
        if result.insert(key, index_row).is_some() {
            return Err(DurableError::Corrupt(
                "duplicate Item File member index row",
            ));
        }
    }
    Ok(result)
}

fn validate_index_row(row: &IndexRow) -> DurableResult<()> {
    validate_public_path(&row.manifest_path)?;
    if !is_candidate_manifest_path(&row.manifest_path)
        || row.item_id.is_empty()
        || row.item_id.len() > MAX_ID_BYTES
        || !row.item_id.starts_with("tos.item.")
        || row.edition_id.is_empty()
        || row.edition_id.len() > MAX_ID_BYTES
        || !row.edition_id.starts_with("tos.edition.")
        || row.file_id.is_empty()
        || row.file_id.len() > MAX_ID_BYTES
        || !row.file_id.starts_with("tos.file.")
        || row.rights_id.is_empty()
        || row.rights_id.len() > MAX_ID_BYTES
        || !row.rights_id.starts_with("tos.rights.")
        || row.payload_ordinal > i32::MAX as usize
    {
        return Err(DurableError::Corrupt("Item File index row invalid"));
    }
    Ok(())
}

fn predicate_token(file_id: &str) -> DurableResult<String> {
    if file_id.is_empty() || file_id.len() > MAX_ID_BYTES || !file_id.starts_with("tos.file.") {
        return Err(DurableError::Invalid("Item File lookup identity"));
    }
    let mut hasher = Digest256Hasher::new();
    let label = b"tos-pg-item-file-assignment-v1";
    hasher.update(&(label.len() as u64).to_be_bytes());
    hasher.update(label);
    hasher.update(&(file_id.len() as u64).to_be_bytes());
    hasher.update(file_id.as_bytes());
    Ok(hasher.finalize().to_prefixed())
}

fn advance_predicate_generations(
    tx: &mut Transaction<'_>,
    domain: &str,
    definition: Digest256,
    scope: &str,
    tokens: &BTreeSet<String>,
    repair_incomplete: bool,
) -> DurableResult<()> {
    let definition_hex = definition.to_hex();
    for token in tokens {
        tx.execute(
            "INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version,generation,complete)
             VALUES($1,'range',$2,$3,$4,$5,0,true) ON CONFLICT DO NOTHING",
            &[&domain,&crate::source_cohort::OWNER,&scope,token,&definition_hex],
        )?;
        let row = tx.query_one(
            "SELECT generation,complete,definition_version FROM cmd2_predicate
             WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4 FOR UPDATE",
            &[&domain, &crate::source_cohort::OWNER, &scope, token],
        )?;
        let generation: i64 = row.get("generation");
        if generation < 0 || generation >= i64::MAX {
            return Err(DurableError::Refused(
                "Item File predicate generation exhausted",
            ));
        }
        if !repair_incomplete
            && (!row.get::<_, bool>("complete")
                || row.get::<_, String>("definition_version") != definition_hex)
        {
            return Err(DurableError::Refused(
                "Item File predicate is incomplete or has another definition",
            ));
        }
        let updated = if repair_incomplete {
            tx.execute(
                "UPDATE cmd2_predicate SET generation=generation+1,complete=true,definition_version=$5
                 WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4
                   AND generation<9223372036854775807",
                &[&domain,&crate::source_cohort::OWNER,&scope,token,&definition_hex],
            )?
        } else {
            tx.execute(
                "UPDATE cmd2_predicate SET generation=generation+1
                 WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4
                   AND complete AND definition_version=$5 AND generation<9223372036854775807",
                &[
                    &domain,
                    &crate::source_cohort::OWNER,
                    &scope,
                    token,
                    &definition_hex,
                ],
            )?
        };
        if updated != 1 {
            return Err(DurableError::Conflict(
                "Item File predicate generation changed",
            ));
        }
    }
    Ok(())
}

fn file_rowset_tokens(
    rows: &BTreeMap<(String, usize), IndexRow>,
) -> DurableResult<BTreeSet<String>> {
    rows.values()
        .map(|row| predicate_token(&row.file_id))
        .collect()
}

fn rights_predicate_token(rights_id: &str) -> DurableResult<String> {
    if rights_id.is_empty()
        || rights_id.len() > MAX_ID_BYTES
        || !rights_id.starts_with("tos.rights.")
    {
        return Err(DurableError::Invalid("Item rights lookup identity"));
    }
    let mut hasher = Digest256Hasher::new();
    let label = b"tos-pg-item-rights-assignment-v1";
    hasher.update(&(label.len() as u64).to_be_bytes());
    hasher.update(label);
    hasher.update(&(rights_id.len() as u64).to_be_bytes());
    hasher.update(rights_id.as_bytes());
    Ok(hasher.finalize().to_prefixed())
}

fn edition_predicate_token(edition_id: &str) -> DurableResult<String> {
    if edition_id.is_empty()
        || edition_id.len() > MAX_ID_BYTES
        || !edition_id.starts_with("tos.edition.")
    {
        return Err(DurableError::Invalid("Item Edition lookup identity"));
    }
    let mut hasher = Digest256Hasher::new();
    let label = b"tos-pg-item-edition-assignment-v1";
    hasher.update(&(label.len() as u64).to_be_bytes());
    hasher.update(label);
    hasher.update(&(edition_id.len() as u64).to_be_bytes());
    hasher.update(edition_id.as_bytes());
    Ok(hasher.finalize().to_prefixed())
}

fn rights_rowset_tokens(
    rows: &BTreeMap<(String, usize), IndexRow>,
) -> DurableResult<BTreeSet<String>> {
    rows.values()
        .map(|row| rights_predicate_token(&row.rights_id))
        .collect()
}

fn edition_rowset_tokens(
    rows: &BTreeMap<(String, usize), IndexRow>,
) -> DurableResult<BTreeSet<String>> {
    rows.values()
        .map(|row| edition_predicate_token(&row.edition_id))
        .collect()
}

/// Exact current range-predicate tokens derived from the selected Item
/// manifests and their bound source members. Retained generations are added
/// by the streamed source comparison separately.
pub(crate) fn current_predicate_tokens(
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<[BTreeSet<String>; 3]> {
    let rows = full_source_rows(files, definition)?;
    Ok([
        file_rowset_tokens(&rows)?,
        rights_rowset_tokens(&rows)?,
        edition_rowset_tokens(&rows)?,
    ])
}

fn parse_rights_member(bytes: &[u8]) -> DurableResult<(String, Digest256)> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(DurableError::Refused(
            "Item rights member byte bound exceeded",
        ));
    }
    let rights = parse(bytes)?;
    if text(&rights, "schema_version")? != "tos_rights_record_v1" {
        return Err(DurableError::Corrupt("Item rights schema differs"));
    }
    let rights_id = text(&rights, "rights_id")?;
    if rights_id.len() > MAX_ID_BYTES || !rights_id.starts_with("tos.rights.") {
        return Err(DurableError::Corrupt("Item rights identity invalid"));
    }
    Ok((rights_id.to_owned(), Digest256::of_bytes(bytes)))
}

fn manifest_rows_for_replacement(
    path: &str,
    bytes: &[u8],
    item_record: Option<&[u8]>,
    rights_member: Option<&[u8]>,
    old: &BTreeMap<(String, usize), IndexRow>,
    definition: Digest256,
) -> DurableResult<BTreeMap<(String, usize), IndexRow>> {
    validate_public_path(path)?;
    if !is_candidate_manifest_path(path) || bytes.len() > MAX_MANIFEST_BYTES {
        return Err(DurableError::Refused(
            "Item File manifest path or size bound exceeded",
        ));
    }
    let manifest = parse(bytes)?;
    if text(&manifest, "schema_version")? != "tos_source_item_manifest_v1" {
        return Err(DurableError::Corrupt("Item File manifest schema differs"));
    }
    let item_id = text(&manifest, "item_id")?;
    if item_id.len() > MAX_ID_BYTES || !item_id.starts_with("tos.item.") {
        return Err(DurableError::Corrupt(
            "Item File manifest Item identity invalid",
        ));
    }
    let edition_id = text(&manifest, "embodiment_ref")?;
    if edition_id.len() > MAX_ID_BYTES || !edition_id.starts_with("tos.edition.") {
        return Err(DurableError::Corrupt(
            "Item File manifest Edition identity invalid",
        ));
    }
    if let Some(record) = item_record {
        let record = parse(record)?;
        if text(&record, "record_type")? != "item"
            || text(&record, "record_id")? != item_id
            || text(&record, "item_manifest_ref")? != path
        {
            return Err(DurableError::Corrupt(
                "Item File source record binding differs",
            ));
        }
    } else if old.values().next().is_none_or(|row| row.item_id != item_id) {
        return Err(DurableError::Corrupt(
            "Item File source record absent or changed",
        ));
    }
    let expected_rights = rights_record_path(path)?;
    if text(&manifest, "rights_ref")? != expected_rights {
        return Err(DurableError::Corrupt("Item rights locator differs"));
    }
    let (rights_id, rights_digest) = match rights_member {
        Some(raw) => parse_rights_member(raw)?,
        None => {
            let row = old
                .values()
                .next()
                .ok_or(DurableError::Corrupt("Item rights source absent"))?;
            (row.rights_id.clone(), row.rights_digest)
        }
    };
    let payload_files = manifest
        .object_get("payload_files")
        .and_then(JsonValue::as_array)
        .ok_or(DurableError::Corrupt(
            "Item File manifest assignments invalid",
        ))?;
    if payload_files.is_empty() || payload_files.len() > MAX_ASSIGNMENTS_PER_MANIFEST {
        return Err(DurableError::Refused(
            "Item File manifest assignment bound exceeded",
        ));
    }
    let member_digest = Digest256::of_bytes(bytes);
    let mut rows = BTreeMap::new();
    for (ordinal, assignment) in payload_files.iter().enumerate() {
        let file_id = text(assignment, "file_id")?;
        if file_id.len() > MAX_ID_BYTES || !file_id.starts_with("tos.file.") {
            return Err(DurableError::Corrupt("Item File identity invalid"));
        }
        let row = IndexRow {
            item_id: item_id.to_owned(),
            manifest_path: path.to_owned(),
            edition_id: edition_id.to_owned(),
            payload_ordinal: ordinal,
            file_id: file_id.to_owned(),
            member_digest,
            rights_id: rights_id.clone(),
            rights_digest,
            definition_digest: definition,
        };
        if rows.insert((path.to_owned(), ordinal), row).is_some() {
            return Err(DurableError::Corrupt("duplicate Item File ordinal"));
        }
    }
    Ok(rows)
}

fn affected_manifest_from_member(path: &str) -> Option<String> {
    if is_candidate_manifest_path(path) {
        return Some(path.to_owned());
    }
    for suffix in ["/item.json", "/rights.json"] {
        if (is_candidate_item_record_path(path) && suffix == "/item.json")
            || (is_candidate_item_rights_path(path) && suffix == "/rights.json")
        {
            return path
                .strip_suffix(suffix)
                .map(|home| format!("{home}/item.manifest.json"));
        }
    }
    None
}

fn old_predicate_tokens_for_scope(
    tx: &mut Transaction<'_>,
    domain: &str,
    scope: &str,
) -> DurableResult<BTreeSet<String>> {
    let limit = as_i64(MAX_ASSIGNMENT_COUNT + 1)?;
    let rows = tx.query(
        "SELECT token FROM cmd2_predicate
         WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3
         ORDER BY token COLLATE \"C\" LIMIT $4",
        &[&domain, &crate::source_cohort::OWNER, &scope, &limit],
    )?;
    if rows.len() > MAX_ASSIGNMENT_COUNT {
        return Err(DurableError::Refused(
            "Item assignment predicate rebuild bound exceeded",
        ));
    }
    let mut result = BTreeSet::new();
    for row in rows {
        let token: String = row.get("token");
        if token.len() != 71 || !token.starts_with("sha256:") {
            return Err(DurableError::Corrupt(
                "Item assignment predicate token invalid",
            ));
        }
        digest(token.strip_prefix("sha256:").unwrap())?;
        result.insert(token);
    }
    Ok(result)
}

fn advance_changed_row_predicates(
    tx: &mut Transaction<'_>,
    domain: &str,
    definition: Digest256,
    old: &BTreeMap<(String, usize), IndexRow>,
    new: &BTreeMap<(String, usize), IndexRow>,
    repair_incomplete: bool,
) -> DurableResult<()> {
    let mut changed_files = BTreeSet::new();
    let mut changed_rights = BTreeSet::new();
    let mut changed_editions = BTreeSet::new();
    for key in old.keys().chain(new.keys()).collect::<BTreeSet<_>>() {
        if old.get(key) != new.get(key) {
            for row in [old.get(key), new.get(key)].into_iter().flatten() {
                changed_files.insert(predicate_token(&row.file_id)?);
                changed_rights.insert(rights_predicate_token(&row.rights_id)?);
                changed_editions.insert(edition_predicate_token(&row.edition_id)?);
            }
        }
    }
    advance_predicate_generations(
        tx,
        domain,
        definition,
        EDITION_PREDICATE_SCOPE,
        &changed_editions,
        repair_incomplete,
    )?;
    advance_predicate_generations(
        tx,
        domain,
        definition,
        PREDICATE_SCOPE,
        &changed_files,
        repair_incomplete,
    )?;
    advance_predicate_generations(
        tx,
        domain,
        definition,
        RIGHTS_PREDICATE_SCOPE,
        &changed_rights,
        repair_incomplete,
    )
}

/// Atomically replace only changed Item-manifest assignments and advance each
/// affected File-ID range generation under the caller's held source/audit fence.
pub(crate) fn replace_current_members<'a, I>(
    tx: &mut Transaction<'_>,
    domain: &str,
    members: I,
    definition: Digest256,
) -> DurableResult<()>
where
    I: IntoIterator<Item = (&'a str, Option<&'a [u8]>)>,
{
    let mut changes = BTreeMap::<String, Option<Vec<u8>>>::new();
    let mut affected = BTreeSet::new();
    for (path, bytes) in members {
        validate_public_path(path)?;
        if changes
            .insert(path.to_owned(), bytes.map(<[u8]>::to_vec))
            .is_some()
        {
            return Err(DurableError::Invalid(
                "duplicate Item File replacement path",
            ));
        }
        if let Some(manifest_path) = affected_manifest_from_member(path) {
            affected.insert(manifest_path);
        }
    }
    let mut old_all = BTreeMap::new();
    let mut new_all = BTreeMap::new();
    for manifest_path in affected {
        let old = read_manifest_rows(tx, domain, &manifest_path, definition)?;
        let manifest_changed = changes.contains_key(&manifest_path);
        let item_path = item_record_path(&manifest_path)?;
        let rights_path = rights_record_path(&manifest_path)?;
        let item_change = changes.get(&item_path);
        let rights_change = changes.get(&rights_path);
        if item_change.is_some_and(Option::is_none) || rights_change.is_some_and(Option::is_none) {
            return Err(DurableError::Corrupt(
                "Item record or rights member removed while package is present",
            ));
        }
        let new = if manifest_changed {
            match changes.get(&manifest_path).and_then(Option::as_deref) {
                None => {
                    if !old.is_empty() && item_change.is_some_and(Option::is_some)
                        || !old.is_empty() && rights_change.is_some_and(Option::is_some)
                    {
                        return Err(DurableError::Corrupt(
                            "Item package member deleted with manifest",
                        ));
                    }
                    BTreeMap::new()
                }
                Some(manifest_bytes) => manifest_rows_for_replacement(
                    &manifest_path,
                    manifest_bytes,
                    item_change.and_then(Option::as_deref),
                    rights_change.and_then(Option::as_deref),
                    &old,
                    definition,
                )?,
            }
        } else if let Some(item_bytes) = item_change {
            let Some(item_bytes) = item_bytes.as_deref() else {
                return Err(DurableError::Corrupt(
                    "Item record removed while manifest remains",
                ));
            };
            let row = old
                .values()
                .next()
                .ok_or(DurableError::Corrupt("Item manifest absent"))?;
            let record = parse(item_bytes)?;
            if text(&record, "record_type")? != "item"
                || text(&record, "record_id")? != row.item_id
                || text(&record, "item_manifest_ref")? != manifest_path
            {
                return Err(DurableError::Corrupt(
                    "Item record replacement binding differs",
                ));
            }
            old.clone()
        } else if let Some(rights_bytes) = rights_change {
            let rights_bytes = rights_bytes.as_deref().ok_or(DurableError::Corrupt(
                "Item rights record removed while manifest remains",
            ))?;
            let (rights_id, rights_digest) = parse_rights_member(rights_bytes)?;
            old.iter()
                .map(|(key, row)| {
                    let mut updated = row.clone();
                    updated.rights_id = rights_id.clone();
                    updated.rights_digest = rights_digest;
                    (key.clone(), updated)
                })
                .collect()
        } else {
            old.clone()
        };
        if old != new {
            old_all.extend(old.clone());
            new_all.extend(new.clone());
            let deleted = tx.execute(
                "DELETE FROM cmd2_source_item_file_assignment
                 WHERE domain=$1 AND manifest_path COLLATE \"C\"=$2 COLLATE \"C\"",
                &[&domain, &manifest_path],
            )?;
            if deleted != old.len() as u64 {
                return Err(DurableError::Conflict(
                    "Item File member changed during replacement",
                ));
            }
            for row in new.values() {
                tx.execute(
                    "INSERT INTO cmd2_source_item_file_assignment(
                       domain,item_id,manifest_path,edition_id,payload_ordinal,file_id,member_digest,
                       rights_id,rights_digest,definition_digest
                     ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
                    &[
                        &domain,
                        &row.item_id,
                        &row.manifest_path,
                        &row.edition_id,
                        &(row.payload_ordinal as i32),
                        &row.file_id,
                        &row.member_digest.to_hex(),
                        &row.rights_id,
                        &row.rights_digest.to_hex(),
                        &row.definition_digest.to_hex(),
                    ],
                )?;
            }
        }
    }
    advance_changed_row_predicates(tx, domain, definition, &old_all, &new_all, false)
}

fn old_full_rows(
    tx: &mut Transaction<'_>,
    domain: &str,
) -> DurableResult<BTreeMap<(String, usize), IndexRow>> {
    let limit = as_i64(MAX_ASSIGNMENT_COUNT + 1)?;
    let rows = tx.query(
        "SELECT item_id,manifest_path,edition_id,payload_ordinal,file_id,member_digest,
                rights_id,rights_digest,definition_digest
         FROM cmd2_source_item_file_assignment WHERE domain=$1
         ORDER BY manifest_path COLLATE \"C\",payload_ordinal LIMIT $2",
        &[&domain, &limit],
    )?;
    if rows.len() > MAX_ASSIGNMENT_COUNT {
        return Err(DurableError::Refused(
            "Item File full index row bound exceeded",
        ));
    }
    let mut result = BTreeMap::new();
    for row in rows {
        let value = IndexRow {
            item_id: row.get("item_id"),
            manifest_path: row.get("manifest_path"),
            edition_id: row.get("edition_id"),
            payload_ordinal: usize::try_from(row.get::<_, i32>("payload_ordinal"))
                .map_err(|_| DurableError::Corrupt("Item File ordinal invalid"))?,
            file_id: row.get("file_id"),
            member_digest: digest(row.get("member_digest"))?,
            rights_id: row.get("rights_id"),
            rights_digest: digest(row.get("rights_digest"))?,
            definition_digest: digest(row.get("definition_digest"))?,
        };
        validate_index_row(&value)?;
        let key = (value.manifest_path.clone(), value.payload_ordinal);
        if result.insert(key, value).is_some() {
            return Err(DurableError::Corrupt("duplicate Item File full index row"));
        }
    }
    Ok(result)
}

/// Rebuild the complete Item assignment index only during source bootstrap.
pub(crate) fn rebuild_current(
    tx: &mut Transaction<'_>,
    domain: &str,
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<()> {
    let old = old_full_rows(tx, domain)?;
    let new = full_source_rows(files, definition)?;
    let mut changed_editions = old_predicate_tokens_for_scope(tx, domain, EDITION_PREDICATE_SCOPE)?;
    let mut changed_files = old_predicate_tokens_for_scope(tx, domain, PREDICATE_SCOPE)?;
    let mut changed_rights = old_predicate_tokens_for_scope(tx, domain, RIGHTS_PREDICATE_SCOPE)?;
    for key in old.keys().chain(new.keys()).collect::<BTreeSet<_>>() {
        if old.get(key) != new.get(key) {
            if let Some(row) = old.get(key) {
                changed_files.insert(predicate_token(&row.file_id)?);
                changed_rights.insert(rights_predicate_token(&row.rights_id)?);
                changed_editions.insert(edition_predicate_token(&row.edition_id)?);
            }
            if let Some(row) = new.get(key) {
                changed_files.insert(predicate_token(&row.file_id)?);
                changed_rights.insert(rights_predicate_token(&row.rights_id)?);
                changed_editions.insert(edition_predicate_token(&row.edition_id)?);
            }
        }
    }
    tx.execute(
        "DELETE FROM cmd2_source_item_file_assignment WHERE domain=$1",
        &[&domain],
    )?;
    for row in new.values() {
        tx.execute(
            "INSERT INTO cmd2_source_item_file_assignment(
               domain,item_id,manifest_path,edition_id,payload_ordinal,file_id,member_digest,
               rights_id,rights_digest,definition_digest
             ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            &[
                &domain,
                &row.item_id,
                &row.manifest_path,
                &row.edition_id,
                &(row.payload_ordinal as i32),
                &row.file_id,
                &row.member_digest.to_hex(),
                &row.rights_id,
                &row.rights_digest.to_hex(),
                &row.definition_digest.to_hex(),
            ],
        )?;
    }
    advance_predicate_generations(
        tx,
        domain,
        definition,
        EDITION_PREDICATE_SCOPE,
        &changed_editions,
        true,
    )?;
    advance_predicate_generations(
        tx,
        domain,
        definition,
        PREDICATE_SCOPE,
        &changed_files,
        true,
    )?;
    advance_predicate_generations(
        tx,
        domain,
        definition,
        RIGHTS_PREDICATE_SCOPE,
        &changed_rights,
        true,
    )
}

/// Read-only cold exact rowset and current-source verification.
/// Completeness may be invalidated by maintenance; only the full cold owner
/// may re-enable it after all row, definition and generation checks succeed.
pub(crate) fn verify_for_reassessment(
    tx: &mut Transaction<'_>,
    domain: &str,
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<()> {
    let expected = full_source_rows(files, definition)?;
    let definition_hex = definition.to_hex();
    let limit = as_i64(MAX_ASSIGNMENT_COUNT + 1)?;
    let rows = tx.query(
        "SELECT i.item_id,i.manifest_path,i.edition_id,i.payload_ordinal,i.file_id,
                i.member_digest,i.rights_id,i.rights_digest,i.definition_digest,
                c.content_digest,r.content_digest AS rights_content_digest,
                s.path AS identity_path,s.definition_digest AS identity_definition
         FROM cmd2_source_item_file_assignment i
         LEFT JOIN cmd2_current c ON c.domain=i.domain AND c.subject=i.manifest_path
         LEFT JOIN cmd2_current r ON r.domain=i.domain AND
           r.subject=regexp_replace(i.manifest_path,'/item\\.manifest\\.json$','/rights.json')
         LEFT JOIN cmd2_source_index s ON s.domain=i.domain AND s.kind='metadata' AND s.token=i.item_id
         WHERE i.domain=$1 ORDER BY i.manifest_path COLLATE \"C\",i.payload_ordinal LIMIT $2",
        &[&domain,&limit],
    )?;
    if rows.len() > MAX_ASSIGNMENT_COUNT {
        return Err(DurableError::Corrupt(
            "Item File cold index row bound exceeded",
        ));
    }
    let mut observed = BTreeMap::new();
    for row in rows {
        let value = IndexRow {
            item_id: row.get("item_id"),
            manifest_path: row.get("manifest_path"),
            edition_id: row.get("edition_id"),
            payload_ordinal: usize::try_from(row.get::<_, i32>("payload_ordinal"))
                .map_err(|_| DurableError::Corrupt("Item File ordinal invalid"))?,
            file_id: row.get("file_id"),
            member_digest: digest(row.get("member_digest"))?,
            rights_id: row.get("rights_id"),
            rights_digest: digest(row.get("rights_digest"))?,
            definition_digest: digest(row.get("definition_digest"))?,
        };
        validate_index_row(&value)?;
        let current_digest: Option<String> = row.get("content_digest");
        let rights_content_digest: Option<String> = row.get("rights_content_digest");
        let identity_path: Option<String> = row.get("identity_path");
        let identity_definition: Option<String> = row.get("identity_definition");
        if value.definition_digest.to_hex() != definition_hex
            || current_digest.as_deref().map(digest).transpose()? != Some(value.member_digest)
            || rights_content_digest.as_deref().map(digest).transpose()?
                != Some(value.rights_digest)
            || identity_path.as_deref() != Some(item_record_path(&value.manifest_path)?.as_str())
            || identity_definition.as_deref() != Some(definition_hex.as_str())
        {
            return Err(DurableError::Conflict(
                "Item File cold row definition or source differs",
            ));
        }
        let key = (value.manifest_path.clone(), value.payload_ordinal);
        if observed.insert(key, value).is_some() {
            return Err(DurableError::Corrupt("duplicate Item File cold index row"));
        }
    }
    if observed != expected {
        return Err(DurableError::Conflict(
            "Item File cold rowset differs from current source",
        ));
    }
    let tokens = file_rowset_tokens(&expected)?;
    let rights_tokens = rights_rowset_tokens(&expected)?;
    let edition_tokens = edition_rowset_tokens(&expected)?;
    if tx
        .query_opt(
            "SELECT 1 FROM cmd2_predicate
         WHERE domain=$1 AND kind='range' AND owner=$2 AND scope IN ($3,$4,$5)
           AND (definition_version<>$6 OR generation<=0) LIMIT 1",
            &[
                &domain,
                &crate::source_cohort::OWNER,
                &PREDICATE_SCOPE,
                &RIGHTS_PREDICATE_SCOPE,
                &EDITION_PREDICATE_SCOPE,
                &definition_hex,
            ],
        )?
        .is_some()
    {
        return Err(DurableError::Corrupt(
            "Item File range predicate is incomplete or stale",
        ));
    }
    for (scope, tokens) in [
        (PREDICATE_SCOPE, tokens),
        (RIGHTS_PREDICATE_SCOPE, rights_tokens),
        (EDITION_PREDICATE_SCOPE, edition_tokens),
    ] {
        for token in tokens {
            let row = tx
                .query_opt(
                    "SELECT generation,complete,definition_version FROM cmd2_predicate
                     WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4",
                    &[&domain, &crate::source_cohort::OWNER, &scope, &token],
                )?
                .ok_or(DurableError::Corrupt("Item File range generation absent"))?;
            if row.get::<_, i64>("generation") <= 0
                || row.get::<_, String>("definition_version") != definition_hex
            {
                return Err(DurableError::Corrupt(
                    "Item File range predicate is incomplete or stale",
                ));
            }
        }
    }
    Ok(())
}

/// Read an exact File-ID assignment set and Range generation under the
/// caller's transaction, source epoch, and audit fence.
pub(crate) fn lookup_current(
    tx: &mut Transaction<'_>,
    domain: &str,
    file_id: &str,
    definition: Digest256,
) -> DurableResult<ItemFileObservation> {
    let token = predicate_token(file_id)?;
    let definition_hex = definition.to_hex();
    let generation = match tx.query_opt(
        "SELECT generation,complete,definition_version FROM cmd2_predicate
         WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4",
        &[
            &domain,
            &crate::source_cohort::OWNER,
            &PREDICATE_SCOPE,
            &token,
        ],
    )? {
        Some(row) => {
            let value: i64 = row.get("generation");
            if value < 0
                || !row.get::<_, bool>("complete")
                || row.get::<_, String>("definition_version") != definition_hex
            {
                return Err(DurableError::Conflict(
                    "Item File range predicate is incomplete or stale",
                ));
            }
            value as u64
        }
        None => 0,
    };
    let limit = as_i64(MAX_LOOKUP_ROWS + 1)?;
    let rows = tx.query(
        "SELECT i.item_id,i.manifest_path,i.edition_id,i.payload_ordinal,i.file_id,
                i.member_digest,i.rights_id,i.rights_digest,i.definition_digest,
                c.content_digest,r.content_digest AS rights_content_digest,
                s.path AS identity_path,s.definition_digest AS identity_definition
         FROM cmd2_source_item_file_assignment i
         LEFT JOIN cmd2_current c ON c.domain=i.domain AND c.subject=i.manifest_path
         LEFT JOIN cmd2_current r ON r.domain=i.domain AND
           r.subject=regexp_replace(i.manifest_path,'/item\\.manifest\\.json$','/rights.json')
         LEFT JOIN cmd2_source_index s ON s.domain=i.domain AND s.kind='metadata' AND s.token=i.item_id
         WHERE i.domain=$1 AND i.file_id COLLATE \"C\"=$2 COLLATE \"C\"
         ORDER BY i.manifest_path COLLATE \"C\",i.payload_ordinal LIMIT $3",
        &[&domain,&file_id,&limit],
    )?;
    if rows.len() > MAX_LOOKUP_ROWS {
        return Err(DurableError::Refused(
            "Item File point result bound exceeded",
        ));
    }
    let mut observed = BTreeSet::new();
    for row in rows {
        let value = IndexRow {
            item_id: row.get("item_id"),
            manifest_path: row.get("manifest_path"),
            edition_id: row.get("edition_id"),
            payload_ordinal: usize::try_from(row.get::<_, i32>("payload_ordinal"))
                .map_err(|_| DurableError::Corrupt("Item File ordinal invalid"))?,
            file_id: row.get("file_id"),
            member_digest: digest(row.get("member_digest"))?,
            rights_id: row.get("rights_id"),
            rights_digest: digest(row.get("rights_digest"))?,
            definition_digest: digest(row.get("definition_digest"))?,
        };
        validate_index_row(&value)?;
        let current_digest: Option<String> = row.get("content_digest");
        let rights_content_digest: Option<String> = row.get("rights_content_digest");
        let identity_path: Option<String> = row.get("identity_path");
        let identity_definition: Option<String> = row.get("identity_definition");
        if value.file_id != file_id
            || value.definition_digest.to_hex() != definition_hex
            || current_digest.as_deref().map(digest).transpose()? != Some(value.member_digest)
            || rights_content_digest.as_deref().map(digest).transpose()?
                != Some(value.rights_digest)
            || identity_path.as_deref() != Some(item_record_path(&value.manifest_path)?.as_str())
            || identity_definition.as_deref() != Some(definition_hex.as_str())
        {
            return Err(DurableError::Conflict(
                "Item File point row differs from current source",
            ));
        }
        if !observed.insert(value.assignment()) {
            return Err(DurableError::Corrupt("duplicate Item File point row"));
        }
    }
    if generation == 0 && !observed.is_empty() {
        return Err(DurableError::Corrupt(
            "Item File rows lack a Range generation",
        ));
    }
    Ok(ItemFileObservation {
        rows: observed,
        predicate_token: token,
        generation,
    })
}

/// Read the exact set of Item manifests assigned to a rights identity and its
/// Range generation under the caller's source/audit fences.
pub(crate) fn lookup_rights_current(
    tx: &mut Transaction<'_>,
    domain: &str,
    rights_id: &str,
    definition: Digest256,
) -> DurableResult<ItemRightsObservation> {
    let token = rights_predicate_token(rights_id)?;
    let definition_hex = definition.to_hex();
    let generation = match tx.query_opt(
        "SELECT generation,complete,definition_version FROM cmd2_predicate
         WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4",
        &[
            &domain,
            &crate::source_cohort::OWNER,
            &RIGHTS_PREDICATE_SCOPE,
            &token,
        ],
    )? {
        Some(row) => {
            let value: i64 = row.get("generation");
            if value < 0
                || !row.get::<_, bool>("complete")
                || row.get::<_, String>("definition_version") != definition_hex
            {
                return Err(DurableError::Conflict(
                    "Item rights range predicate is incomplete or stale",
                ));
            }
            value as u64
        }
        None => 0,
    };
    let limit = as_i64(MAX_MANIFEST_COUNT + 1)?;
    let rows = tx.query(
        "SELECT i.item_id,i.manifest_path,i.member_digest,i.rights_id,
                i.rights_digest,i.definition_digest,c.content_digest,
                r.content_digest AS rights_content_digest,
                s.path AS identity_path,s.definition_digest AS identity_definition
         FROM (
           SELECT DISTINCT domain,item_id,manifest_path,member_digest,rights_id,
                  rights_digest,definition_digest
           FROM cmd2_source_item_file_assignment
           WHERE domain=$1 AND rights_id COLLATE \"C\"=$2 COLLATE \"C\"
         ) i
         LEFT JOIN cmd2_current c ON c.domain=i.domain AND c.subject=i.manifest_path
         LEFT JOIN cmd2_current r ON r.domain=i.domain AND
           r.subject=regexp_replace(i.manifest_path,'/item\\.manifest\\.json$','/rights.json')
         LEFT JOIN cmd2_source_index s ON s.domain=i.domain AND s.kind='metadata' AND s.token=i.item_id
         ORDER BY i.manifest_path COLLATE \"C\" LIMIT $3",
        &[&domain, &rights_id, &limit],
    )?;
    if rows.len() > MAX_MANIFEST_COUNT {
        return Err(DurableError::Refused(
            "Item rights point result bound exceeded",
        ));
    }
    let mut observed = BTreeSet::new();
    for row in rows {
        let value = ItemRightsAssignment {
            item_id: row.get("item_id"),
            manifest_path: row.get("manifest_path"),
            manifest_digest: digest(row.get("member_digest"))?,
            rights_id: row.get("rights_id"),
            rights_digest: digest(row.get("rights_digest"))?,
        };
        validate_public_path(&value.manifest_path)?;
        let current_digest: Option<String> = row.get("content_digest");
        let rights_content_digest: Option<String> = row.get("rights_content_digest");
        let identity_path: Option<String> = row.get("identity_path");
        let identity_definition: Option<String> = row.get("identity_definition");
        let row_definition: String = row.get("definition_digest");
        if value.item_id.is_empty()
            || value.item_id.len() > MAX_ID_BYTES
            || !value.item_id.starts_with("tos.item.")
            || !is_candidate_manifest_path(&value.manifest_path)
            || value.rights_id != rights_id
            || row_definition != definition_hex
            || current_digest.as_deref().map(digest).transpose()? != Some(value.manifest_digest)
            || rights_content_digest.as_deref().map(digest).transpose()?
                != Some(value.rights_digest)
            || identity_path.as_deref() != Some(item_record_path(&value.manifest_path)?.as_str())
            || identity_definition.as_deref() != Some(definition_hex.as_str())
        {
            return Err(DurableError::Conflict(
                "Item rights point row differs from current source",
            ));
        }
        observed.insert(value);
    }
    if generation == 0 && !observed.is_empty() {
        return Err(DurableError::Corrupt(
            "Item rights rows lack a Range generation",
        ));
    }
    Ok(ItemRightsObservation {
        rows: observed,
        predicate_token: token,
        generation,
    })
}

/// Read every current Item manifest naming one Edition and its Range
/// generation. Payload ordinals preserve the source index's completeness, but
/// callers receive one exact manifest binding per Item. The Item manifest
/// schema requires at least one payload, and both the bootstrap and ordinary
/// replacement paths enforce that rule before indexing.
pub(crate) fn lookup_edition_current(
    tx: &mut Transaction<'_>,
    domain: &str,
    edition_id: &str,
    definition: Digest256,
) -> DurableResult<ItemEditionObservation> {
    let token = edition_predicate_token(edition_id)?;
    let definition_hex = definition.to_hex();
    let generation = match tx.query_opt(
        "SELECT generation,complete,definition_version FROM cmd2_predicate
         WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4",
        &[
            &domain,
            &crate::source_cohort::OWNER,
            &EDITION_PREDICATE_SCOPE,
            &token,
        ],
    )? {
        Some(row) => {
            let value: i64 = row.get("generation");
            if value < 0
                || !row.get::<_, bool>("complete")
                || row.get::<_, String>("definition_version") != definition_hex
            {
                return Err(DurableError::Conflict(
                    "Item Edition range predicate is incomplete or stale",
                ));
            }
            value as u64
        }
        None => 0,
    };
    let limit = as_i64(MAX_MANIFEST_COUNT + 1)?;
    let rows = tx.query(
        "SELECT i.item_id,i.manifest_path,i.edition_id,i.member_digest,
                i.definition_digest,COUNT(*) AS assignment_count,c.content_digest,
                r.content_digest AS item_content_digest,
                s.path AS identity_path,s.definition_digest AS identity_definition
         FROM cmd2_source_item_file_assignment i
         LEFT JOIN cmd2_current c ON c.domain=i.domain AND c.subject=i.manifest_path
         LEFT JOIN cmd2_current r ON r.domain=i.domain AND
           r.subject=regexp_replace(i.manifest_path,'/item\\.manifest\\.json$','/item.json')
         LEFT JOIN cmd2_source_index s ON s.domain=i.domain AND s.kind='metadata' AND s.token=i.item_id
         WHERE i.domain=$1 AND i.edition_id COLLATE \"C\"=$2 COLLATE \"C\"
         GROUP BY i.item_id,i.manifest_path,i.edition_id,i.member_digest,
                  i.definition_digest,c.content_digest,r.content_digest,
                  s.path,s.definition_digest
         ORDER BY i.manifest_path COLLATE \"C\" LIMIT $3",
        &[&domain, &edition_id, &limit],
    )?;
    if rows.len() > MAX_MANIFEST_COUNT {
        return Err(DurableError::Refused(
            "Item Edition point result bound exceeded",
        ));
    }
    let mut observed = BTreeSet::new();
    let mut observed_ids = BTreeSet::new();
    for row in rows {
        let value = ItemEditionAssignment {
            item_id: row.get("item_id"),
            manifest_path: row.get("manifest_path"),
            manifest_digest: digest(row.get("member_digest"))?,
        };
        validate_public_path(&value.manifest_path)?;
        let row_edition: String = row.get("edition_id");
        let row_definition: String = row.get("definition_digest");
        let assignment_count: i64 = row.get("assignment_count");
        let current_digest: Option<String> = row.get("content_digest");
        let item_content_digest: Option<String> = row.get("item_content_digest");
        let identity_path: Option<String> = row.get("identity_path");
        let identity_definition: Option<String> = row.get("identity_definition");
        if value.item_id.is_empty()
            || value.item_id.len() > MAX_ID_BYTES
            || !value.item_id.starts_with("tos.item.")
            || !is_candidate_manifest_path(&value.manifest_path)
            || row_edition != edition_id
            || row_definition != definition_hex
            || assignment_count <= 0
            || assignment_count > MAX_ASSIGNMENTS_PER_MANIFEST as i64
            || current_digest.as_deref().map(digest).transpose()? != Some(value.manifest_digest)
            || item_content_digest.as_deref().is_none()
            || identity_path.as_deref() != Some(item_record_path(&value.manifest_path)?.as_str())
            || identity_definition.as_deref() != Some(definition_hex.as_str())
        {
            return Err(DurableError::Conflict(
                "Item Edition point row differs from current source",
            ));
        }
        if !observed_ids.insert(value.item_id.clone()) || !observed.insert(value.clone()) {
            return Err(DurableError::Corrupt("duplicate Item Edition point row"));
        }
    }
    if generation == 0 && !observed.is_empty() {
        return Err(DurableError::Corrupt(
            "Item Edition rows lack a Range generation",
        ));
    }
    Ok(ItemEditionObservation {
        rows: observed,
        predicate_token: token,
        generation,
    })
}
