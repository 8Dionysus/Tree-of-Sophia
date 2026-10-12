//! Typed reverse dependency rows for native Item adoption and source
//! navigation. Source bytes remain the authority; these tables are bounded,
//! audited point indexes over exact current Claim streams.

use crate::{DurableError, DurableResult};
use postgres::Transaction;
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{Digest256, Digest256Hasher, JsonValue, RelativePath};

pub(crate) const PREDICATE_SCOPE: &str = "claim-dependency-v1";
pub(crate) const NAVIGATION_REFERENCE_SCOPE: &str = "claim-navigation-reference-v1";
pub(crate) const MAX_CLAIM_STREAMS: usize = 4_096;
pub(crate) const MAX_STREAM_BYTES: usize = 1_048_576;
pub(crate) const MAX_TOTAL_CLAIM_BYTES: usize = 268_435_456;
const MAX_INDEX_ROWS: usize = 65_536;
const MAX_PREDICATE_ROWS: usize = MAX_INDEX_ROWS * 2;
pub(crate) const MAX_LOOKUP_ROWS: usize = 8_192;
const MAX_PATH_BYTES: usize = 4_096;
const MAX_ID_BYTES: usize = 4_096;
pub(crate) const MAX_NAVIGATION_REFERENCE_ROWS: usize = 262_144;
pub(crate) const MAX_NAVIGATION_REFERENCE_LOOKUPS: usize = 8_192;
const NAVIGATION_REFERENCE_COMPLETE_MARKER: &str = "claim-navigation-reference-index-complete-v2";
const NAVIGATION_SHARED_SOURCE_PATHS: &[&str] = &[
    "ToS/contracts/source-claim-record.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    "ToS/source-witnesses/catalog/claims.jsonl",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ClaimNavigationReferenceKind {
    TosId,
    SourcePath,
}

impl ClaimNavigationReferenceKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::TosId => "tos-id",
            Self::SourcePath => "source-path",
        }
    }

    fn parse(value: &str) -> DurableResult<Self> {
        match value {
            "tos-id" => Ok(Self::TosId),
            "source-path" => Ok(Self::SourcePath),
            _ => Err(DurableError::Corrupt(
                "Claim navigation reference kind invalid",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum NavigationReferenceBasis {
    ClaimValue,
    OwnerInput,
    SharedSchema,
}

impl NavigationReferenceBasis {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ClaimValue => "claim-value",
            Self::OwnerInput => "owner-input",
            Self::SharedSchema => "shared-schema",
        }
    }

    fn parse(value: &str) -> DurableResult<Self> {
        match value {
            "claim-value" => Ok(Self::ClaimValue),
            "owner-input" => Ok(Self::OwnerInput),
            "shared-schema" => Ok(Self::SharedSchema),
            _ => Err(DurableError::Corrupt(
                "Claim navigation reference basis invalid",
            )),
        }
    }
}

/// Exact JSON-value reference owned by one Claim row. The source stream digest
/// binds all references to the complete current member, not a parsed fragment.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClaimNavigationReference {
    pub(crate) claim_id: String,
    pub(crate) path: String,
    pub(crate) kind: ClaimNavigationReferenceKind,
    pub(crate) endpoint: String,
    pub(crate) member_digest: Digest256,
    pub(crate) size_bytes: u64,
    pub(crate) commit_seq: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct ClaimNavigationReferenceObservation {
    pub(crate) kind: ClaimNavigationReferenceKind,
    pub(crate) endpoint: String,
    pub(crate) rows: BTreeSet<ClaimNavigationReference>,
    pub(crate) predicate_token: String,
    pub(crate) generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NavigationReferenceIndexRow {
    claim_id: String,
    path: String,
    basis: NavigationReferenceBasis,
    kind: ClaimNavigationReferenceKind,
    endpoint: String,
    member_digest: Digest256,
    definition_digest: Digest256,
}

impl NavigationReferenceIndexRow {
    fn token(&self) -> DurableResult<String> {
        navigation_reference_token(self.kind, &self.endpoint)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClaimDependencyDirection {
    Subject,
    Object,
}

impl ClaimDependencyDirection {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Subject => "subject",
            Self::Object => "object",
        }
    }

    const fn endpoint_column(self) -> &'static str {
        match self {
            Self::Subject => "subject_endpoint",
            Self::Object => "object_endpoint",
        }
    }
}

/// Exact current Claim row returned by a typed point lookup. `member_digest`
/// binds the edge to the complete source stream from which it was derived.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ClaimDependencyEdge {
    pub(crate) predicate: String,
    pub(crate) claim_id: String,
    pub(crate) path: String,
    pub(crate) subject_endpoint: String,
    pub(crate) object_endpoint: String,
    pub(crate) member_digest: Digest256,
}

#[derive(Clone, Debug)]
pub(crate) struct ClaimDependencyObservation {
    pub(crate) rows: BTreeSet<ClaimDependencyEdge>,
    pub(crate) predicate_token: String,
    pub(crate) generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndexRow {
    predicate: String,
    claim_id: String,
    path: String,
    subject_endpoint: String,
    object_endpoint: String,
    member_digest: Digest256,
    definition_digest: Digest256,
}

impl IndexRow {
    fn edge(&self) -> ClaimDependencyEdge {
        ClaimDependencyEdge {
            predicate: self.predicate.clone(),
            claim_id: self.claim_id.clone(),
            path: self.path.clone(),
            subject_endpoint: self.subject_endpoint.clone(),
            object_endpoint: self.object_endpoint.clone(),
            member_digest: self.member_digest,
        }
    }

    fn predicate_tokens(&self) -> DurableResult<[String; 2]> {
        Ok([
            predicate_token(
                ClaimDependencyDirection::Subject,
                &self.predicate,
                &self.subject_endpoint,
            )?,
            predicate_token(
                ClaimDependencyDirection::Object,
                &self.predicate,
                &self.object_endpoint,
            )?,
        ])
    }
}

fn as_i64(value: usize) -> DurableResult<i64> {
    i64::try_from(value).map_err(|_| DurableError::Refused("Claim dependency count overflow"))
}

fn digest(value: &str) -> DurableResult<Digest256> {
    Digest256::from_hex(value)
        .map_err(|_| DurableError::Corrupt("Claim dependency index digest invalid"))
}

fn is_indexed_predicate(predicate: &str) -> bool {
    matches!(predicate, "embodied_by" | "exemplified_by")
}

fn validate_public_path(path: &str) -> DurableResult<()> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || !path.starts_with("ToS/")
        || RelativePath::parse(path).is_err()
        || path == "ToS/source-witnesses/owner-local"
        || path.starts_with("ToS/source-witnesses/owner-local/")
    {
        return Err(DurableError::Invalid("Claim dependency source path"));
    }
    Ok(())
}

fn is_claim_stream_path(path: &str) -> bool {
    path.starts_with("ToS/source-witnesses/relations/")
        && path.ends_with("/source-claims.jsonl")
}

fn validate_claim_stream_path(path: &str) -> DurableResult<()> {
    validate_public_path(path)?;
    if !is_claim_stream_path(path) {
        return Err(DurableError::Corrupt(
            "Claim dependency index path is not a public Claim stream",
        ));
    }
    Ok(())
}

fn text(value: &JsonValue, key: &'static str) -> DurableResult<&str> {
    value
        .object_get(key)
        .and_then(JsonValue::as_str)
        .ok_or(DurableError::Corrupt(
            "Claim dependency source field invalid",
        ))
}

fn source_command_error<T>(
    value: crate::source_command::SourceCommandResult<T>,
) -> DurableResult<T> {
    value.map_err(|_| DurableError::Corrupt("Claim dependency source JSON invalid"))
}

fn python_bytes_blank(raw: &[u8]) -> bool {
    raw.iter()
        .all(|byte| matches!(*byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
}

fn member_rows(
    path: &str,
    bytes: &[u8],
    definition: Digest256,
) -> DurableResult<BTreeMap<String, IndexRow>> {
    validate_claim_stream_path(path)?;
    if bytes.len() > MAX_STREAM_BYTES {
        return Err(DurableError::Refused(
            "Claim dependency stream byte bound exceeded",
        ));
    }
    let member_digest = Digest256::of_bytes(bytes);
    let mut rows = BTreeMap::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        if python_bytes_blank(line) {
            continue;
        }
        if line.len() > MAX_STREAM_BYTES {
            return Err(DurableError::Refused(
                "Claim dependency line byte bound exceeded",
            ));
        }
        let claim = source_command_error(crate::source_command::parse(line))?;
        let predicate = text(&claim, "predicate")?;
        if !is_indexed_predicate(predicate) {
            continue;
        }
        let claim_id = text(&claim, "claim_id")?;
        let subject = text(&claim, "subject_ref")?;
        let object = text(&claim, "object")?;
        if claim_id.len() > MAX_ID_BYTES
            || !claim_id.starts_with("tos.claim.")
            || subject.len() > MAX_ID_BYTES
            || !subject.starts_with("tos.")
            || object.len() > MAX_ID_BYTES
            || !object.starts_with("tos.")
        {
            return Err(DurableError::Corrupt(
                "Claim dependency relation identity invalid",
            ));
        }
        let row = IndexRow {
            predicate: predicate.to_owned(),
            claim_id: claim_id.to_owned(),
            path: path.to_owned(),
            subject_endpoint: subject.to_owned(),
            object_endpoint: object.to_owned(),
            member_digest,
            definition_digest: definition,
        };
        if rows.insert(row.claim_id.clone(), row).is_some() {
            return Err(DurableError::Corrupt(
                "duplicate Claim dependency identity in stream",
            ));
        }
        if rows.len() > MAX_INDEX_ROWS {
            return Err(DurableError::Refused(
                "Claim dependency member contribution bound exceeded",
            ));
        }
    }
    Ok(rows)
}

fn supported_tos_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("tos.") else {
        return false;
    };
    let Some((kind, tail)) = rest.split_once('.') else {
        return false;
    };
    if kind.is_empty()
        || !kind.as_bytes()[0].is_ascii_lowercase()
        || !kind
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return false;
    }
    let mut previous_separator = true;
    for byte in tail.bytes() {
        match byte {
            b'a'..=b'z' | b'0'..=b'9' => previous_separator = false,
            b'.' | b'-' if !previous_separator => previous_separator = true,
            _ => return false,
        }
    }
    !tail.is_empty() && !previous_separator
}

fn navigation_reference_token(
    kind: ClaimNavigationReferenceKind,
    endpoint: &str,
) -> DurableResult<String> {
    let supported = match kind {
        ClaimNavigationReferenceKind::TosId => {
            endpoint.len() <= MAX_ID_BYTES && supported_tos_id(endpoint)
        }
        ClaimNavigationReferenceKind::SourcePath => {
            endpoint.len() <= MAX_PATH_BYTES && validate_public_path(endpoint).is_ok()
        }
    };
    if !supported {
        return Err(DurableError::Invalid(
            "Claim navigation reference lookup key",
        ));
    }
    let mut hasher = Digest256Hasher::new();
    for part in [
        b"tos-pg-claim-navigation-reference-v1".as_slice(),
        kind.as_str().as_bytes(),
        endpoint.as_bytes(),
    ] {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    Ok(hasher.finalize().to_prefixed())
}

fn navigation_reference_complete_token() -> DurableResult<String> {
    let mut hasher = Digest256Hasher::new();
    let part = NAVIGATION_REFERENCE_COMPLETE_MARKER.as_bytes();
    hasher.update(&(part.len() as u64).to_be_bytes());
    hasher.update(part);
    Ok(hasher.finalize().to_prefixed())
}

fn collect_navigation_references(
    value: &JsonValue,
    output: &mut BTreeSet<(ClaimNavigationReferenceKind, String)>,
) -> DurableResult<()> {
    match value {
        JsonValue::String(value) => {
            let Some(value) = value.as_str() else {
                return Ok(());
            };
            if value.len() <= MAX_ID_BYTES && supported_tos_id(value) {
                output.insert((ClaimNavigationReferenceKind::TosId, value.to_owned()));
            } else if value.len() <= MAX_PATH_BYTES
                && value.starts_with("ToS/")
                && validate_public_path(value).is_ok()
            {
                output.insert((ClaimNavigationReferenceKind::SourcePath, value.to_owned()));
            }
        }
        JsonValue::Array(values) => {
            for item in values {
                collect_navigation_references(item, output)?;
            }
        }
        JsonValue::Object(values) => {
            for (_, item) in values {
                collect_navigation_references(item, output)?;
            }
        }
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) => {}
    }
    Ok(())
}

/// Index only complete JSON string values that match the supported ToS stable
/// identity or public source-path grammar. This deliberately over-approximates
/// which fields the navigation renderer uses while never searching substrings
/// or inventing references absent from the exact Claim JSON.
fn navigation_reference_rows(
    path: &str,
    bytes: &[u8],
    definition: Digest256,
    shared_schema_paths: &BTreeSet<String>,
) -> DurableResult<BTreeSet<NavigationReferenceIndexRow>> {
    validate_claim_stream_path(path)?;
    if bytes.len() > MAX_STREAM_BYTES {
        return Err(DurableError::Refused(
            "Claim navigation reference stream byte bound exceeded",
        ));
    }
    let member_digest = Digest256::of_bytes(bytes);
    let mut result = BTreeSet::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        if python_bytes_blank(line) {
            continue;
        }
        if line.len() > MAX_STREAM_BYTES {
            return Err(DurableError::Refused(
                "Claim navigation reference line byte bound exceeded",
            ));
        }
        let claim = source_command_error(crate::source_command::parse(line))?;
        let claim_id = text(&claim, "claim_id")?;
        if claim_id.len() > MAX_ID_BYTES
            || !claim_id.starts_with("tos.claim.")
            || !supported_tos_id(claim_id)
        {
            return Err(DurableError::Corrupt(
                "Claim navigation reference owner identity invalid",
            ));
        }
        let mut claim_references = BTreeSet::new();
        collect_navigation_references(&claim, &mut claim_references)?;
        let mut owner_references = BTreeSet::new();
        // Querying a changed Claim stream returns every owner in that stream.
        owner_references.insert((ClaimNavigationReferenceKind::SourcePath, path.to_owned()));
        for shared_path in NAVIGATION_SHARED_SOURCE_PATHS {
            owner_references.insert((
                ClaimNavigationReferenceKind::SourcePath,
                (*shared_path).to_owned(),
            ));
        }
        let shared_schema_references = shared_schema_paths
            .iter()
            .cloned()
            .map(|path| (ClaimNavigationReferenceKind::SourcePath, path))
            .collect::<BTreeSet<_>>();
        let stream_prefix = path
            .strip_suffix("source-claims.jsonl")
            .ok_or(DurableError::Corrupt("Claim stream suffix invalid"))?;
        let claim_hash = Digest256::of_bytes(claim_id.as_bytes()).to_hex();
        for owned_path in [
            format!("{stream_prefix}{claim_hash}.human-forms.json"),
            format!("{stream_prefix}source-create-provenance.jsonl"),
            format!("{stream_prefix}source-create-receipt.json"),
            format!("{stream_prefix}claim-revision-history.json"),
        ] {
            owner_references.insert((ClaimNavigationReferenceKind::SourcePath, owned_path));
        }
        for (basis, references) in [
            (NavigationReferenceBasis::ClaimValue, claim_references),
            (NavigationReferenceBasis::OwnerInput, owner_references),
            (
                NavigationReferenceBasis::SharedSchema,
                shared_schema_references,
            ),
        ] {
            for (kind, endpoint) in references {
                result.insert(NavigationReferenceIndexRow {
                    claim_id: claim_id.to_owned(),
                    path: path.to_owned(),
                    basis,
                    kind,
                    endpoint,
                    member_digest,
                    definition_digest: definition,
                });
                if result.len() > MAX_NAVIGATION_REFERENCE_ROWS {
                    return Err(DurableError::Refused(
                        "Claim navigation reference member bound exceeded",
                    ));
                }
            }
        }
    }
    Ok(result)
}

fn read_member_rows(
    tx: &mut Transaction<'_>,
    domain: &str,
    path: &str,
    definition: Digest256,
) -> DurableResult<BTreeMap<String, IndexRow>> {
    let limit = as_i64(MAX_INDEX_ROWS + 1)?;
    let rows = tx.query(
        "SELECT predicate,claim_id,path,subject_endpoint,object_endpoint,
                member_digest,definition_digest
         FROM cmd2_source_claim_dependency
         WHERE domain=$1 AND path COLLATE \"C\"=$2 COLLATE \"C\"
         ORDER BY claim_id COLLATE \"C\" LIMIT $3",
        &[&domain, &path, &limit],
    )?;
    if rows.len() > MAX_INDEX_ROWS {
        return Err(DurableError::Corrupt(
            "Claim dependency member index bound exceeded",
        ));
    }
    let expected_definition = definition.to_hex();
    let mut result = BTreeMap::new();
    for row in rows {
        let path: String = row.get("path");
        validate_claim_stream_path(&path)?;
        let index_row = IndexRow {
            predicate: row.get("predicate"),
            claim_id: row.get("claim_id"),
            path,
            subject_endpoint: row.get("subject_endpoint"),
            object_endpoint: row.get("object_endpoint"),
            member_digest: digest(row.get("member_digest"))?,
            definition_digest: digest(row.get("definition_digest"))?,
        };
        validate_index_row(&index_row)?;
        if index_row.definition_digest.to_hex() != expected_definition {
            return Err(DurableError::Conflict(
                "Claim dependency index definition differs",
            ));
        }
        if result
            .insert(index_row.claim_id.clone(), index_row)
            .is_some()
        {
            return Err(DurableError::Corrupt(
                "duplicate Claim dependency member index row",
            ));
        }
    }
    Ok(result)
}

fn read_navigation_member_rows(
    tx: &mut Transaction<'_>,
    domain: &str,
    path: &str,
    definition: Digest256,
) -> DurableResult<BTreeSet<NavigationReferenceIndexRow>> {
    let limit = as_i64(MAX_NAVIGATION_REFERENCE_ROWS + 1)?;
    let rows = tx.query(
        "SELECT claim_id,path,basis,endpoint_kind,endpoint,member_digest,definition_digest
         FROM cmd2_source_claim_navigation_reference
         WHERE domain=$1 AND path COLLATE \"C\"=$2 COLLATE \"C\"
         ORDER BY claim_id COLLATE \"C\",endpoint_kind COLLATE \"C\",endpoint COLLATE \"C\"
         LIMIT $3",
        &[&domain, &path, &limit],
    )?;
    if rows.len() > MAX_NAVIGATION_REFERENCE_ROWS {
        return Err(DurableError::Corrupt(
            "Claim navigation reference member bound exceeded",
        ));
    }
    let expected_definition = definition.to_hex();
    let mut result = BTreeSet::new();
    for row in rows {
        let value = NavigationReferenceIndexRow {
            claim_id: row.get("claim_id"),
            path: row.get("path"),
            basis: NavigationReferenceBasis::parse(row.get("basis"))?,
            kind: ClaimNavigationReferenceKind::parse(row.get("endpoint_kind"))?,
            endpoint: row.get("endpoint"),
            member_digest: digest(row.get("member_digest"))?,
            definition_digest: digest(row.get("definition_digest"))?,
        };
        validate_navigation_reference_row(&value)?;
        if value.definition_digest.to_hex() != expected_definition || value.path != path {
            return Err(DurableError::Conflict(
                "Claim navigation reference definition or owner path differs",
            ));
        }
        if !result.insert(value) {
            return Err(DurableError::Corrupt(
                "duplicate Claim navigation reference member row",
            ));
        }
    }
    Ok(result)
}

fn read_navigation_full_rows(
    tx: &mut Transaction<'_>,
    domain: &str,
) -> DurableResult<BTreeSet<NavigationReferenceIndexRow>> {
    let limit = as_i64(MAX_NAVIGATION_REFERENCE_ROWS + 1)?;
    let rows = tx.query(
        "SELECT claim_id,path,basis,endpoint_kind,endpoint,member_digest,definition_digest
         FROM cmd2_source_claim_navigation_reference
         WHERE domain=$1
         ORDER BY path COLLATE \"C\",claim_id COLLATE \"C\",endpoint_kind COLLATE \"C\",endpoint COLLATE \"C\"
         LIMIT $2",
        &[&domain, &limit],
    )?;
    if rows.len() > MAX_NAVIGATION_REFERENCE_ROWS {
        return Err(DurableError::Refused(
            "Claim navigation reference full index bound exceeded",
        ));
    }
    let mut result = BTreeSet::new();
    for row in rows {
        let value = NavigationReferenceIndexRow {
            claim_id: row.get("claim_id"),
            path: row.get("path"),
            basis: NavigationReferenceBasis::parse(row.get("basis"))?,
            kind: ClaimNavigationReferenceKind::parse(row.get("endpoint_kind"))?,
            endpoint: row.get("endpoint"),
            member_digest: digest(row.get("member_digest"))?,
            definition_digest: digest(row.get("definition_digest"))?,
        };
        validate_navigation_reference_row(&value)?;
        if !result.insert(value) {
            return Err(DurableError::Corrupt(
                "duplicate Claim navigation reference full row",
            ));
        }
    }
    Ok(result)
}

fn validate_navigation_reference_row(row: &NavigationReferenceIndexRow) -> DurableResult<()> {
    validate_claim_stream_path(&row.path)?;
    if row.claim_id.is_empty()
        || row.claim_id.len() > MAX_ID_BYTES
        || !supported_tos_id(&row.claim_id)
        || row.kind == ClaimNavigationReferenceKind::TosId && !supported_tos_id(&row.endpoint)
        || row.kind == ClaimNavigationReferenceKind::SourcePath
            && validate_public_path(&row.endpoint).is_err()
    {
        return Err(DurableError::Corrupt(
            "Claim navigation reference index row invalid",
        ));
    }
    Ok(())
}

fn validate_index_row(row: &IndexRow) -> DurableResult<()> {
    validate_claim_stream_path(&row.path)?;
    if !is_indexed_predicate(&row.predicate)
        || row.claim_id.is_empty()
        || row.claim_id.len() > MAX_ID_BYTES
        || !row.claim_id.starts_with("tos.claim.")
        || row.subject_endpoint.is_empty()
        || row.subject_endpoint.len() > MAX_ID_BYTES
        || !row.subject_endpoint.starts_with("tos.")
        || row.object_endpoint.is_empty()
        || row.object_endpoint.len() > MAX_ID_BYTES
        || !row.object_endpoint.starts_with("tos.")
    {
        return Err(DurableError::Corrupt("Claim dependency index row invalid"));
    }
    Ok(())
}

fn predicate_token(
    direction: ClaimDependencyDirection,
    predicate: &str,
    endpoint: &str,
) -> DurableResult<String> {
    if !is_indexed_predicate(predicate)
        || endpoint.is_empty()
        || endpoint.len() > MAX_ID_BYTES
        || !endpoint.starts_with("tos.")
    {
        return Err(DurableError::Invalid("Claim dependency lookup key"));
    }
    let mut hasher = Digest256Hasher::new();
    for part in [
        b"tos-pg-claim-dependency-v1".as_slice(),
        direction.as_str().as_bytes(),
        predicate.as_bytes(),
        endpoint.as_bytes(),
    ] {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    Ok(hasher.finalize().to_prefixed())
}

fn advance_generations(
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
            &[
                &domain,
                &crate::source_cohort::OWNER,
                &scope,
                token,
                &definition_hex,
            ],
        )?;
        let row = tx.query_one(
            "SELECT generation,complete,definition_version FROM cmd2_predicate
             WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4 FOR UPDATE",
            &[&domain, &crate::source_cohort::OWNER, &scope, token],
        )?;
        let generation: i64 = row.get("generation");
        if generation < 0 || generation >= i64::MAX {
            return Err(DurableError::Refused(
                "Claim dependency predicate generation exhausted",
            ));
        }
        if !repair_incomplete
            && (!row.get::<_, bool>("complete")
                || row.get::<_, String>("definition_version") != definition_hex)
        {
            return Err(DurableError::Refused(
                "Claim dependency predicate is incomplete or has another definition",
            ));
        }
        let changed = if repair_incomplete {
            tx.execute(
                "UPDATE cmd2_predicate SET generation=generation+1,complete=true,definition_version=$5
                 WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4
                   AND generation<9223372036854775807",
                &[
                    &domain,
                    &crate::source_cohort::OWNER,
                    &scope,
                    token,
                    &definition_hex,
                ],
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
        if changed != 1 {
            return Err(DurableError::Conflict(
                "Claim dependency predicate generation changed",
            ));
        }
    }
    Ok(())
}

fn advance_predicate_generations(
    tx: &mut Transaction<'_>,
    domain: &str,
    definition: Digest256,
    tokens: &BTreeSet<String>,
    repair_incomplete: bool,
) -> DurableResult<()> {
    advance_generations(
        tx,
        domain,
        definition,
        PREDICATE_SCOPE,
        tokens,
        repair_incomplete,
    )
}

fn rowset_tokens(rows: &BTreeMap<String, IndexRow>) -> DurableResult<BTreeSet<String>> {
    let mut tokens = BTreeSet::new();
    for row in rows.values() {
        tokens.extend(row.predicate_tokens()?);
    }
    Ok(tokens)
}

fn navigation_rowset_tokens(
    rows: &BTreeSet<NavigationReferenceIndexRow>,
) -> DurableResult<BTreeSet<String>> {
    rows.iter()
        .map(NavigationReferenceIndexRow::token)
        .collect()
}

fn full_navigation_source_rows(
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<BTreeSet<NavigationReferenceIndexRow>> {
    let shared_schema_paths = files
        .keys()
        .filter(|path| {
            path.starts_with("ToS/contracts/") && path.ends_with(".schema.json")
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut claim_streams = 0usize;
    let mut total_bytes = 0usize;
    let mut rows = BTreeSet::new();
    for (path, bytes) in files {
        validate_public_path(path)?;
        if !is_claim_stream_path(path) {
            continue;
        }
        claim_streams += 1;
        if claim_streams > MAX_CLAIM_STREAMS {
            return Err(DurableError::Refused(
                "Claim navigation reference stream count bound exceeded",
            ));
        }
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .filter(|count| *count <= MAX_TOTAL_CLAIM_BYTES)
            .ok_or(DurableError::Refused(
                "Claim navigation reference total byte bound exceeded",
            ))?;
        rows.extend(navigation_reference_rows(
            path,
            bytes,
            definition,
            &shared_schema_paths,
        )?);
        if rows.len() > MAX_NAVIGATION_REFERENCE_ROWS {
            return Err(DurableError::Refused(
                "Claim navigation reference full index bound exceeded",
            ));
        }
    }
    Ok(rows)
}

fn current_schema_paths(
    tx: &mut Transaction<'_>,
    domain: &str,
) -> DurableResult<BTreeSet<String>> {
    let limit = as_i64(8193)?;
    let rows = tx.query(
        "SELECT subject FROM cmd2_current
         WHERE domain=$1 AND subject COLLATE \"C\" >= 'ToS/contracts/' COLLATE \"C\"
           AND subject COLLATE \"C\" < 'ToS/contracts0' COLLATE \"C\"
           AND subject LIKE 'ToS/contracts/%.schema.json'
         ORDER BY subject COLLATE \"C\" LIMIT $2",
        &[&domain, &limit],
    )?;
    if rows.len() > 8192 {
        return Err(DurableError::Refused(
            "Claim navigation shared schema path bound exceeded",
        ));
    }
    let mut result = BTreeSet::new();
    for row in rows {
        let path: String = row.get("subject");
        validate_public_path(&path)?;
        if !path.starts_with("ToS/contracts/") || !path.ends_with(".schema.json") {
            return Err(DurableError::Corrupt(
                "Claim navigation shared schema path differs",
            ));
        }
        result.insert(path);
    }
    Ok(result)
}

fn existing_shared_schema_paths(
    tx: &mut Transaction<'_>,
    domain: &str,
) -> DurableResult<BTreeSet<String>> {
    let limit = as_i64(8193)?;
    let rows = tx.query(
        "SELECT DISTINCT endpoint FROM cmd2_source_claim_navigation_reference
         WHERE domain=$1 AND basis=$2 AND endpoint_kind='source-path'
         ORDER BY endpoint COLLATE \"C\" LIMIT $3",
        &[&domain, &NavigationReferenceBasis::SharedSchema.as_str(), &limit],
    )?;
    if rows.len() > 8192 {
        return Err(DurableError::Refused(
            "Claim navigation indexed schema path bound exceeded",
        ));
    }
    let mut result = BTreeSet::new();
    for row in rows {
        let path: String = row.get("endpoint");
        validate_public_path(&path)?;
        if !path.starts_with("ToS/contracts/") || !path.ends_with(".schema.json") {
            return Err(DurableError::Corrupt(
                "Claim navigation indexed schema path differs",
            ));
        }
        result.insert(path);
    }
    Ok(result)
}

fn current_claim_owner_facts(
    tx: &mut Transaction<'_>,
    domain: &str,
    definition: Digest256,
) -> DurableResult<BTreeMap<(String, String), (Digest256, u64)>> {
    let limit = as_i64(MAX_INDEX_ROWS + 1)?;
    let rows = tx.query(
        "SELECT i.token AS claim_id,i.path,c.content_digest,c.content_length,
                i.definition_digest
         FROM cmd2_source_index i
         JOIN cmd2_current c ON c.domain=i.domain AND c.subject=i.path
         WHERE i.domain=$1 AND i.kind='claim'
           AND i.path COLLATE \"C\" >= 'ToS/source-witnesses/relations/' COLLATE \"C\"
           AND i.path COLLATE \"C\" < 'ToS/source-witnesses/relations0' COLLATE \"C\"
           AND i.path LIKE 'ToS/source-witnesses/relations/%/source-claims.jsonl'
         ORDER BY i.path COLLATE \"C\",i.token COLLATE \"C\" LIMIT $2",
        &[&domain, &limit],
    )?;
    if rows.len() > MAX_INDEX_ROWS {
        return Err(DurableError::Refused(
            "Claim navigation owner enumeration bound exceeded",
        ));
    }
    let expected_definition = definition.to_hex();
    let mut result = BTreeMap::new();
    for row in rows {
        let claim_id: String = row.get("claim_id");
        let path: String = row.get("path");
        validate_claim_stream_path(&path)?;
        if !supported_tos_id(&claim_id) || !claim_id.starts_with("tos.claim.") {
            return Err(DurableError::Corrupt(
                "Claim navigation current owner identity differs",
            ));
        }
        let member_digest = digest(row.get("content_digest"))?;
        let size_bytes = row
            .get::<_, i64>("content_length")
            .try_into()
            .map_err(|_| DurableError::Corrupt("Claim navigation current size invalid"))?;
        if row.get::<_, String>("definition_digest") != expected_definition {
            return Err(DurableError::Conflict(
                "Claim navigation current owner definition differs",
            ));
        }
        if result
            .insert((path, claim_id), (member_digest, size_bytes))
            .is_some()
        {
            return Err(DurableError::Corrupt(
                "duplicate Claim navigation current owner identity",
            ));
        }
    }
    Ok(result)
}

fn replace_shared_schema_references(
    tx: &mut Transaction<'_>,
    domain: &str,
    definition: Digest256,
    old_schema_paths: &BTreeSet<String>,
    current_schema_paths: &BTreeSet<String>,
    changed_tokens: &mut BTreeSet<String>,
) -> DurableResult<()> {
    let added = current_schema_paths
        .difference(old_schema_paths)
        .cloned()
        .collect::<Vec<_>>();
    let removed = old_schema_paths
        .difference(current_schema_paths)
        .cloned()
        .collect::<Vec<_>>();
    if added.is_empty() && removed.is_empty() {
        return Ok(());
    }

    let mut removed_rows = BTreeSet::new();
    for endpoint in &removed {
        let remaining = MAX_NAVIGATION_REFERENCE_ROWS
            .checked_sub(removed_rows.len())
            .ok_or(DurableError::Refused(
                "Claim navigation schema removal row bound exceeded",
            ))?;
        let rows = tx.query(
            "SELECT claim_id,path,basis,endpoint_kind,endpoint,member_digest,definition_digest
             FROM cmd2_source_claim_navigation_reference
             WHERE domain=$1 AND basis=$2 AND endpoint_kind='source-path'
               AND endpoint COLLATE \"C\"=$3 COLLATE \"C\"
             ORDER BY path COLLATE \"C\",claim_id COLLATE \"C\" LIMIT $4",
            &[
                &domain,
                &NavigationReferenceBasis::SharedSchema.as_str(),
                endpoint,
                &as_i64(remaining + 1)?,
            ],
        )?;
        if rows.len() > remaining {
            return Err(DurableError::Refused(
                "Claim navigation schema removal row bound exceeded",
            ));
        }
        for row in rows {
            let value = NavigationReferenceIndexRow {
                claim_id: row.get("claim_id"),
                path: row.get("path"),
                basis: NavigationReferenceBasis::parse(row.get("basis"))?,
                kind: ClaimNavigationReferenceKind::parse(row.get("endpoint_kind"))?,
                endpoint: row.get("endpoint"),
                member_digest: digest(row.get("member_digest"))?,
                definition_digest: digest(row.get("definition_digest"))?,
            };
            validate_navigation_reference_row(&value)?;
            if value.definition_digest != definition || value.endpoint != *endpoint {
                return Err(DurableError::Conflict(
                    "Claim navigation schema removal row differs",
                ));
            }
            changed_tokens.insert(value.token()?);
            removed_rows.insert((value.path, value.claim_id, value.endpoint));
        }
    }

    let current_owners = if added.is_empty() {
        BTreeMap::new()
    } else {
        current_claim_owner_facts(tx, domain, definition)?
    };
    let current_rows: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM cmd2_source_claim_navigation_reference WHERE domain=$1",
            &[&domain],
        )?
        .get(0);
    let current_rows = usize::try_from(current_rows)
        .map_err(|_| DurableError::Corrupt("Claim navigation index row count invalid"))?;
    let rows_after_removal = current_rows
        .checked_sub(removed_rows.len())
        .ok_or(DurableError::Corrupt(
            "Claim navigation schema removal count exceeds index",
        ))?;
    let maximum_additions = added
        .len()
        .checked_mul(current_owners.len())
        .ok_or(DurableError::Refused(
            "Claim navigation schema addition row bound exceeded",
        ))?;
    if rows_after_removal
        .checked_add(maximum_additions)
        .filter(|rows| *rows <= MAX_NAVIGATION_REFERENCE_ROWS)
        .is_none()
    {
        return Err(DurableError::Refused(
            "Claim navigation schema dependency index bound exceeded",
        ));
    }
    for endpoint in &added {
        for ((path, claim_id), (member_digest, _size_bytes)) in &current_owners {
            let row = NavigationReferenceIndexRow {
                claim_id: claim_id.clone(),
                path: path.clone(),
                basis: NavigationReferenceBasis::SharedSchema,
                kind: ClaimNavigationReferenceKind::SourcePath,
                endpoint: endpoint.clone(),
                member_digest: *member_digest,
                definition_digest: definition,
            };
            validate_navigation_reference_row(&row)?;
            let inserted = tx.execute(
                "INSERT INTO cmd2_source_claim_navigation_reference(
                   domain,claim_id,path,basis,endpoint_kind,endpoint,member_digest,definition_digest
                 ) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT DO NOTHING",
                &[
                    &domain,
                    &row.claim_id,
                    &row.path,
                    &row.basis.as_str(),
                    &row.kind.as_str(),
                    &row.endpoint,
                    &row.member_digest.to_hex(),
                    &row.definition_digest.to_hex(),
                ],
            )?;
            if inserted == 1 {
                changed_tokens.insert(row.token()?);
            } else {
                let existing = tx.query_opt(
                    "SELECT member_digest,definition_digest FROM cmd2_source_claim_navigation_reference
                     WHERE domain=$1 AND path=$2 AND claim_id=$3 AND basis=$4
                       AND endpoint_kind=$5 AND endpoint=$6",
                    &[
                        &domain,
                        &row.path,
                        &row.claim_id,
                        &row.basis.as_str(),
                        &row.kind.as_str(),
                        &row.endpoint,
                    ],
                )?;
                let Some(existing) = existing else {
                    return Err(DurableError::Conflict(
                        "Claim navigation schema insertion lost its row",
                    ));
                };
                if digest(existing.get("member_digest"))? != row.member_digest
                    || digest(existing.get("definition_digest"))? != definition
                {
                    return Err(DurableError::Conflict(
                        "Claim navigation schema insertion conflicts with current owner",
                    ));
                }
            }
        }
    }
    for endpoint in &removed {
        let deleted = tx.execute(
            "DELETE FROM cmd2_source_claim_navigation_reference
             WHERE domain=$1 AND basis=$2 AND endpoint_kind='source-path'
               AND endpoint COLLATE \"C\"=$3 COLLATE \"C\"",
            &[
                &domain,
                &NavigationReferenceBasis::SharedSchema.as_str(),
                endpoint,
            ],
        )?;
        if usize::try_from(deleted).ok() != Some(
            removed_rows
                .iter()
                .filter(|(_, _, removed_endpoint)| removed_endpoint == endpoint)
                .count(),
        ) {
            return Err(DurableError::Conflict(
                "Claim navigation schema removal changed during replacement",
            ));
        }
    }
    Ok(())
}

/// Atomically replace only changed Claim-stream contributions and advance the
/// Range predicates for every old/new endpoint tuple affected by the change.
pub(crate) fn replace_current_members<'a, I>(
    tx: &mut Transaction<'_>,
    domain: &str,
    members: I,
    definition: Digest256,
) -> DurableResult<()>
where
    I: IntoIterator<Item = (&'a str, Option<&'a [u8]>)>,
{
    let members = members.into_iter().collect::<Vec<_>>();
    let schema_membership_changed = members.iter().any(|(path, _)| {
        path.starts_with("ToS/contracts/") && path.ends_with(".schema.json")
    });
    let claim_stream_changed = members
        .iter()
        .any(|(path, bytes)| bytes.is_some() && is_claim_stream_path(path));
    require_navigation_completeness_generation(tx, domain, definition)?;
    let shared_schema_paths = if schema_membership_changed || claim_stream_changed {
        current_schema_paths(tx, domain)?
    } else {
        BTreeSet::new()
    };
    let old_shared_schema_paths = if schema_membership_changed {
        existing_shared_schema_paths(tx, domain)?
    } else {
        BTreeSet::new()
    };
    let mut seen_paths = BTreeSet::new();
    let mut changed_tokens = BTreeSet::new();
    let mut changed_navigation_tokens = BTreeSet::new();
    for (path, bytes) in members {
        validate_public_path(path)?;
        if !seen_paths.insert(path.to_owned()) {
            return Err(DurableError::Invalid(
                "duplicate Claim dependency replacement path",
            ));
        }
        let old = read_member_rows(tx, domain, path, definition)?;
        let new = match bytes {
            Some(bytes) if is_claim_stream_path(path) => member_rows(path, bytes, definition)?,
            Some(_) | None => BTreeMap::new(),
        };
        let old_navigation = read_navigation_member_rows(tx, domain, path, definition)?;
        let new_navigation = match bytes {
            Some(bytes) if is_claim_stream_path(path) => {
                navigation_reference_rows(path, bytes, definition, &shared_schema_paths)?
            }
            Some(_) | None => BTreeSet::new(),
        };
        for row in old_navigation.symmetric_difference(&new_navigation) {
            changed_navigation_tokens.insert(row.token()?);
        }
        for claim_id in old.keys().chain(new.keys()).collect::<BTreeSet<_>>() {
            if old.get(claim_id) != new.get(claim_id) {
                if let Some(row) = old.get(claim_id) {
                    changed_tokens.extend(row.predicate_tokens()?);
                }
                if let Some(row) = new.get(claim_id) {
                    changed_tokens.extend(row.predicate_tokens()?);
                }
            }
        }
        let deleted = tx.execute(
            "DELETE FROM cmd2_source_claim_dependency WHERE domain=$1 AND path COLLATE \"C\"=$2 COLLATE \"C\"",
            &[&domain, &path],
        )?;
        if deleted != old.len() as u64 {
            return Err(DurableError::Conflict(
                "Claim dependency member changed during replacement",
            ));
        }
        for row in new.values() {
            tx.execute(
                "INSERT INTO cmd2_source_claim_dependency(
                   domain,predicate,claim_id,path,subject_endpoint,object_endpoint,
                   member_digest,definition_digest
                 ) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
                &[
                    &domain,
                    &row.predicate,
                    &row.claim_id,
                    &row.path,
                    &row.subject_endpoint,
                    &row.object_endpoint,
                    &row.member_digest.to_hex(),
                    &row.definition_digest.to_hex(),
                ],
            )?;
        }
        let deleted_navigation = tx.execute(
            "DELETE FROM cmd2_source_claim_navigation_reference WHERE domain=$1 AND path COLLATE \"C\"=$2 COLLATE \"C\"",
            &[&domain, &path],
        )?;
        if deleted_navigation != old_navigation.len() as u64 {
            return Err(DurableError::Conflict(
                "Claim navigation reference member changed during replacement",
            ));
        }
        for row in &new_navigation {
            tx.execute(
                "INSERT INTO cmd2_source_claim_navigation_reference(
                   domain,claim_id,path,basis,endpoint_kind,endpoint,member_digest,definition_digest
                 ) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
                &[
                    &domain,
                    &row.claim_id,
                    &row.path,
                    &row.basis.as_str(),
                    &row.kind.as_str(),
                    &row.endpoint,
                    &row.member_digest.to_hex(),
                    &row.definition_digest.to_hex(),
                ],
            )?;
        }
    }
    if schema_membership_changed {
        replace_shared_schema_references(
            tx,
            domain,
            definition,
            &old_shared_schema_paths,
            &shared_schema_paths,
            &mut changed_navigation_tokens,
        )?;
    }
    advance_predicate_generations(tx, domain, definition, &changed_tokens, false)?;
    if !changed_navigation_tokens.is_empty() {
        changed_navigation_tokens.insert(navigation_reference_complete_token()?);
        advance_generations(
            tx,
            domain,
            definition,
            NAVIGATION_REFERENCE_SCOPE,
            &changed_navigation_tokens,
            false,
        )?;
    }
    Ok(())
}

fn full_source_rows(
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<BTreeMap<String, IndexRow>> {
    let mut claim_streams = 0usize;
    let mut total_bytes = 0usize;
    let mut rows = BTreeMap::new();
    for (path, bytes) in files {
        validate_public_path(path)?;
        if !is_claim_stream_path(path) {
            continue;
        }
        claim_streams += 1;
        if claim_streams > MAX_CLAIM_STREAMS {
            return Err(DurableError::Refused(
                "Claim dependency stream count bound exceeded",
            ));
        }
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .filter(|count| *count <= MAX_TOTAL_CLAIM_BYTES)
            .ok_or(DurableError::Refused(
                "Claim dependency total byte bound exceeded",
            ))?;
        for (claim_id, row) in member_rows(path, bytes, definition)? {
            if rows.insert(claim_id, row).is_some() {
                return Err(DurableError::Corrupt(
                    "duplicate Claim dependency identity in current source",
                ));
            }
            if rows.len() > MAX_INDEX_ROWS {
                return Err(DurableError::Refused(
                    "Claim dependency full index row bound exceeded",
                ));
            }
        }
    }
    Ok(rows)
}

fn old_full_rows(
    tx: &mut Transaction<'_>,
    domain: &str,
) -> DurableResult<BTreeMap<String, IndexRow>> {
    let limit = as_i64(MAX_INDEX_ROWS + 1)?;
    let rows = tx.query(
        "SELECT predicate,claim_id,path,subject_endpoint,object_endpoint,
                member_digest,definition_digest
         FROM cmd2_source_claim_dependency WHERE domain=$1
         ORDER BY claim_id COLLATE \"C\" LIMIT $2",
        &[&domain, &limit],
    )?;
    if rows.len() > MAX_INDEX_ROWS {
        return Err(DurableError::Refused(
            "Claim dependency full index row bound exceeded",
        ));
    }
    let mut result = BTreeMap::new();
    for row in rows {
        let value = IndexRow {
            predicate: row.get("predicate"),
            claim_id: row.get("claim_id"),
            path: row.get("path"),
            subject_endpoint: row.get("subject_endpoint"),
            object_endpoint: row.get("object_endpoint"),
            member_digest: digest(row.get("member_digest"))?,
            definition_digest: digest(row.get("definition_digest"))?,
        };
        validate_index_row(&value)?;
        if result.insert(value.claim_id.clone(), value).is_some() {
            return Err(DurableError::Corrupt(
                "duplicate Claim dependency full index row",
            ));
        }
    }
    Ok(result)
}

fn old_predicate_tokens(
    tx: &mut Transaction<'_>,
    domain: &str,
    scope: &str,
) -> DurableResult<BTreeSet<String>> {
    let limit = as_i64(MAX_PREDICATE_ROWS + 1)?;
    let rows = tx.query(
        "SELECT token FROM cmd2_predicate
         WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3
         ORDER BY token COLLATE \"C\" LIMIT $4",
        &[&domain, &crate::source_cohort::OWNER, &scope, &limit],
    )?;
    if rows.len() > MAX_PREDICATE_ROWS {
        return Err(DurableError::Refused(
            "Claim dependency predicate rebuild bound exceeded",
        ));
    }
    let mut result = BTreeSet::new();
    for row in rows {
        let token: String = row.get("token");
        if token.len() != 71 || !token.starts_with("sha256:") {
            return Err(DurableError::Corrupt(
                "Claim dependency predicate token invalid",
            ));
        }
        digest(token.strip_prefix("sha256:").unwrap())?;
        result.insert(token);
    }
    Ok(result)
}

fn old_navigation_tokens(
    tx: &mut Transaction<'_>,
    domain: &str,
) -> DurableResult<BTreeSet<String>> {
    let limit = as_i64(MAX_NAVIGATION_REFERENCE_ROWS + 2)?;
    let rows = tx.query(
        "SELECT token FROM cmd2_predicate
         WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3
         ORDER BY token COLLATE \"C\" LIMIT $4",
        &[
            &domain,
            &crate::source_cohort::OWNER,
            &NAVIGATION_REFERENCE_SCOPE,
            &limit,
        ],
    )?;
    if rows.len() > MAX_NAVIGATION_REFERENCE_ROWS + 1 {
        return Err(DurableError::Refused(
            "Claim navigation reference predicate rebuild bound exceeded",
        ));
    }
    rows.into_iter()
        .map(|row| {
            let token: String = row.get("token");
            if token.len() != 71 || !token.starts_with("sha256:") {
                return Err(DurableError::Corrupt(
                    "Claim navigation reference predicate token invalid",
                ));
            }
            digest(token.strip_prefix("sha256:").unwrap())?;
            Ok(token)
        })
        .collect()
}

fn require_navigation_completeness_generation(
    tx: &mut Transaction<'_>,
    domain: &str,
    definition: Digest256,
) -> DurableResult<u64> {
    let token = navigation_reference_complete_token()?;
    let row = tx.query_opt(
        "SELECT generation,complete,definition_version FROM cmd2_predicate
         WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4",
        &[
            &domain,
            &crate::source_cohort::OWNER,
            &NAVIGATION_REFERENCE_SCOPE,
            &token,
        ],
    )?;
    let Some(row) = row else {
        return Err(DurableError::Conflict(
            "Claim navigation reference index completeness marker absent",
        ));
    };
    let generation: i64 = row.get("generation");
    if generation <= 0
        || !row.get::<_, bool>("complete")
        || row.get::<_, String>("definition_version") != definition.to_hex()
    {
        return Err(DurableError::Conflict(
            "Claim navigation reference index completeness marker stale",
        ));
    }
    u64::try_from(generation)
        .map_err(|_| DurableError::Corrupt("Claim navigation reference generation invalid"))
}

/// Rebuild the complete endpoint index only during the selected bootstrap.
/// Ordinary source writes must use `replace_current_members`.
pub(crate) fn rebuild_current(
    tx: &mut Transaction<'_>,
    domain: &str,
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<()> {
    let old = old_full_rows(tx, domain)?;
    let new = full_source_rows(files, definition)?;
    let mut changed_tokens = old_predicate_tokens(tx, domain, PREDICATE_SCOPE)?;
    for claim_id in old.keys().chain(new.keys()).collect::<BTreeSet<_>>() {
        if old.get(claim_id) != new.get(claim_id) {
            if let Some(row) = old.get(claim_id) {
                changed_tokens.extend(row.predicate_tokens()?);
            }
            if let Some(row) = new.get(claim_id) {
                changed_tokens.extend(row.predicate_tokens()?);
            }
        }
    }
    tx.execute(
        "DELETE FROM cmd2_source_claim_dependency WHERE domain=$1",
        &[&domain],
    )?;
    for row in new.values() {
        tx.execute(
            "INSERT INTO cmd2_source_claim_dependency(
               domain,predicate,claim_id,path,subject_endpoint,object_endpoint,
               member_digest,definition_digest
             ) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &domain,
                &row.predicate,
                &row.claim_id,
                &row.path,
                &row.subject_endpoint,
                &row.object_endpoint,
                &row.member_digest.to_hex(),
                &row.definition_digest.to_hex(),
            ],
        )?;
    }
    advance_predicate_generations(tx, domain, definition, &changed_tokens, true)?;

    let old_navigation = read_navigation_full_rows(tx, domain)?;
    let new_navigation = full_navigation_source_rows(files, definition)?;
    let mut changed_navigation_tokens = old_navigation_tokens(tx, domain)?;
    for row in old_navigation.symmetric_difference(&new_navigation) {
        changed_navigation_tokens.insert(row.token()?);
    }
    tx.execute(
        "DELETE FROM cmd2_source_claim_navigation_reference WHERE domain=$1",
        &[&domain],
    )?;
    for row in &new_navigation {
        tx.execute(
            "INSERT INTO cmd2_source_claim_navigation_reference(
               domain,claim_id,path,basis,endpoint_kind,endpoint,member_digest,definition_digest
             ) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &domain,
                &row.claim_id,
                &row.path,
                &row.basis.as_str(),
                &row.kind.as_str(),
                &row.endpoint,
                &row.member_digest.to_hex(),
                &row.definition_digest.to_hex(),
            ],
        )?;
    }
    changed_navigation_tokens.insert(navigation_reference_complete_token()?);
    advance_generations(
        tx,
        domain,
        definition,
        NAVIGATION_REFERENCE_SCOPE,
        &changed_navigation_tokens,
        true,
    )
}

/// Verify exact equality with a complete selected ToS source. This is a cold
/// read-only check; it never repairs a missing or forged index contribution.
pub(crate) fn verify_current(
    tx: &mut Transaction<'_>,
    domain: &str,
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<()> {
    let expected = full_source_rows(files, definition)?;
    let expected_definition = definition.to_hex();
    let row_limit = as_i64(MAX_INDEX_ROWS + 1)?;
    let rows = tx.query(
        "SELECT i.predicate,i.claim_id,i.path,i.subject_endpoint,i.object_endpoint,
                i.member_digest,i.definition_digest,c.content_digest,
                s.path AS identity_path,s.definition_digest AS identity_definition
         FROM cmd2_source_claim_dependency i
         LEFT JOIN cmd2_current c ON c.domain=i.domain AND c.subject=i.path
         LEFT JOIN cmd2_source_index s ON s.domain=i.domain AND s.kind='claim' AND s.token=i.claim_id
         WHERE i.domain=$1 ORDER BY i.claim_id COLLATE \"C\" LIMIT $2",
        &[&domain, &row_limit],
    )?;
    if rows.len() > MAX_INDEX_ROWS {
        return Err(DurableError::Corrupt(
            "Claim dependency cold index row bound exceeded",
        ));
    }
    let mut observed = BTreeMap::new();
    for row in rows {
        let value = IndexRow {
            predicate: row.get("predicate"),
            claim_id: row.get("claim_id"),
            path: row.get("path"),
            subject_endpoint: row.get("subject_endpoint"),
            object_endpoint: row.get("object_endpoint"),
            member_digest: digest(row.get("member_digest"))?,
            definition_digest: digest(row.get("definition_digest"))?,
        };
        validate_index_row(&value)?;
        let current_digest: Option<String> = row.get("content_digest");
        let indexed_path: Option<String> = row.get("identity_path");
        let identity_definition: Option<String> = row.get("identity_definition");
        if value.definition_digest.to_hex() != expected_definition
            || current_digest.as_deref().map(digest).transpose()? != Some(value.member_digest)
            || indexed_path.as_deref() != Some(value.path.as_str())
            || identity_definition.as_deref() != Some(expected_definition.as_str())
        {
            return Err(DurableError::Conflict(
                "Claim dependency cold row definition or current source differs",
            ));
        }
        if observed.insert(value.claim_id.clone(), value).is_some() {
            return Err(DurableError::Corrupt(
                "duplicate Claim dependency cold index row",
            ));
        }
    }
    if observed != expected {
        return Err(DurableError::Conflict(
            "Claim dependency cold rowset differs from current source",
        ));
    }
    let required_tokens = rowset_tokens(&expected)?;
    if tx
        .query_opt(
            "SELECT 1 FROM cmd2_predicate
             WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3
               AND (NOT complete OR definition_version<>$4 OR generation<=0)
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
            "Claim dependency range predicate is incomplete or stale",
        ));
    }
    for token in required_tokens {
        let row = tx.query_opt(
            "SELECT generation,complete,definition_version FROM cmd2_predicate
             WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4",
            &[
                &domain,
                &crate::source_cohort::OWNER,
                &PREDICATE_SCOPE,
                &token,
            ],
        )?;
        let Some(row) = row else {
            return Err(DurableError::Corrupt(
                "Claim dependency owner lacks a range predicate generation",
            ));
        };
        if row.get::<_, i64>("generation") <= 0
            || !row.get::<_, bool>("complete")
            || row.get::<_, String>("definition_version") != expected_definition
        {
            return Err(DurableError::Corrupt(
                "Claim dependency range predicate is incomplete or stale",
            ));
        }
    }

    let expected_navigation = full_navigation_source_rows(files, definition)?;
    let navigation_limit = as_i64(MAX_NAVIGATION_REFERENCE_ROWS + 1)?;
    let navigation_rows = tx.query(
        "SELECT i.claim_id,i.path,i.basis,i.endpoint_kind,i.endpoint,i.member_digest,i.definition_digest,
                c.content_digest,c.content_length,c.commit_seq,
                s.path AS identity_path,s.definition_digest AS identity_definition
         FROM cmd2_source_claim_navigation_reference i
         LEFT JOIN cmd2_current c ON c.domain=i.domain AND c.subject=i.path
         LEFT JOIN cmd2_source_index s ON s.domain=i.domain AND s.kind='claim' AND s.token=i.claim_id
         WHERE i.domain=$1
         ORDER BY i.path COLLATE \"C\",i.claim_id COLLATE \"C\",i.endpoint_kind COLLATE \"C\",i.endpoint COLLATE \"C\"
         LIMIT $2",
        &[&domain, &navigation_limit],
    )?;
    if navigation_rows.len() > MAX_NAVIGATION_REFERENCE_ROWS {
        return Err(DurableError::Corrupt(
            "Claim navigation reference cold index bound exceeded",
        ));
    }
    let mut observed_navigation = BTreeSet::new();
    for row in navigation_rows {
        let value = NavigationReferenceIndexRow {
            claim_id: row.get("claim_id"),
            path: row.get("path"),
            basis: NavigationReferenceBasis::parse(row.get("basis"))?,
            kind: ClaimNavigationReferenceKind::parse(row.get("endpoint_kind"))?,
            endpoint: row.get("endpoint"),
            member_digest: digest(row.get("member_digest"))?,
            definition_digest: digest(row.get("definition_digest"))?,
        };
        validate_navigation_reference_row(&value)?;
        let current_digest: Option<String> = row.get("content_digest");
        let content_length: Option<i64> = row.get("content_length");
        let commit_seq: Option<i64> = row.get("commit_seq");
        let indexed_path: Option<String> = row.get("identity_path");
        let identity_definition: Option<String> = row.get("identity_definition");
        let expected_len = files.get(&value.path).map(|bytes| bytes.len() as u64);
        if value.definition_digest.to_hex() != expected_definition
            || current_digest.as_deref().map(digest).transpose()? != Some(value.member_digest)
            || content_length.and_then(|length| u64::try_from(length).ok()) != expected_len
            || commit_seq.is_none_or(|sequence| sequence <= 0)
            || indexed_path.as_deref() != Some(value.path.as_str())
            || identity_definition.as_deref() != Some(expected_definition.as_str())
        {
            return Err(DurableError::Conflict(
                "Claim navigation reference current owner differs",
            ));
        }
        if !observed_navigation.insert(value) {
            return Err(DurableError::Corrupt(
                "duplicate Claim navigation reference cold row",
            ));
        }
    }
    if observed_navigation != expected_navigation {
        return Err(DurableError::Conflict(
            "Claim navigation reference cold rowset differs from source",
        ));
    }
    let _ = require_navigation_completeness_generation(tx, domain, definition)?;
    if tx
        .query_opt(
            "SELECT 1 FROM cmd2_predicate
             WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3
               AND (NOT complete OR definition_version<>$4 OR generation<=0)
             LIMIT 1",
            &[
                &domain,
                &crate::source_cohort::OWNER,
                &NAVIGATION_REFERENCE_SCOPE,
                &expected_definition,
            ],
        )?
        .is_some()
    {
        return Err(DurableError::Corrupt(
            "Claim navigation reference predicate is incomplete or stale",
        ));
    }
    for token in navigation_rowset_tokens(&expected_navigation)? {
        let row = tx.query_opt(
            "SELECT generation,complete,definition_version FROM cmd2_predicate
             WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4",
            &[
                &domain,
                &crate::source_cohort::OWNER,
                &NAVIGATION_REFERENCE_SCOPE,
                &token,
            ],
        )?;
        let Some(row) = row else {
            return Err(DurableError::Corrupt(
                "Claim navigation reference lacks a predicate generation",
            ));
        };
        if row.get::<_, i64>("generation") <= 0
            || !row.get::<_, bool>("complete")
            || row.get::<_, String>("definition_version") != expected_definition
        {
            return Err(DurableError::Corrupt(
                "Claim navigation reference predicate is incomplete or stale",
            ));
        }
    }
    Ok(())
}

/// Read the exact current reverse set and its predicate generation while the
/// caller holds the source/audit transaction and epoch fences.
pub(crate) fn lookup_current(
    tx: &mut Transaction<'_>,
    domain: &str,
    direction: ClaimDependencyDirection,
    predicate: &str,
    endpoint: &str,
    definition: Digest256,
) -> DurableResult<ClaimDependencyObservation> {
    let token = predicate_token(direction, predicate, endpoint)?;
    let definition_hex = definition.to_hex();
    let generation = tx.query_opt(
        "SELECT generation,complete,definition_version FROM cmd2_predicate
         WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4",
        &[
            &domain,
            &crate::source_cohort::OWNER,
            &PREDICATE_SCOPE,
            &token,
        ],
    )?;
    let generation = match generation {
        Some(row) => {
            let value: i64 = row.get("generation");
            if value < 0
                || !row.get::<_, bool>("complete")
                || row.get::<_, String>("definition_version") != definition_hex
            {
                return Err(DurableError::Conflict(
                    "Claim dependency range predicate is incomplete or stale",
                ));
            }
            value as u64
        }
        None => 0,
    };
    let endpoint_column = direction.endpoint_column();
    let query = format!(
        "SELECT i.predicate,i.claim_id,i.path,i.subject_endpoint,i.object_endpoint,
                i.member_digest,i.definition_digest,c.content_digest,
                s.path AS identity_path,s.definition_digest AS identity_definition
         FROM cmd2_source_claim_dependency i
         LEFT JOIN cmd2_current c ON c.domain=i.domain AND c.subject=i.path
         LEFT JOIN cmd2_source_index s ON s.domain=i.domain AND s.kind='claim' AND s.token=i.claim_id
         WHERE i.domain=$1 AND i.predicate=$2 AND i.{endpoint_column} COLLATE \"C\"=$3 COLLATE \"C\"
         ORDER BY i.path COLLATE \"C\",i.claim_id COLLATE \"C\" LIMIT $4"
    );
    let rows = tx.query(
        &query,
        &[
            &domain,
            &predicate,
            &endpoint,
            &as_i64(MAX_LOOKUP_ROWS + 1)?,
        ],
    )?;
    if rows.len() > MAX_LOOKUP_ROWS {
        return Err(DurableError::Refused(
            "Claim dependency point result bound exceeded",
        ));
    }
    let mut result = BTreeSet::new();
    for row in rows {
        let value = IndexRow {
            predicate: row.get("predicate"),
            claim_id: row.get("claim_id"),
            path: row.get("path"),
            subject_endpoint: row.get("subject_endpoint"),
            object_endpoint: row.get("object_endpoint"),
            member_digest: digest(row.get("member_digest"))?,
            definition_digest: digest(row.get("definition_digest"))?,
        };
        validate_index_row(&value)?;
        let current_digest: Option<String> = row.get("content_digest");
        let indexed_path: Option<String> = row.get("identity_path");
        let identity_definition: Option<String> = row.get("identity_definition");
        if value.definition_digest.to_hex() != definition_hex
            || current_digest.as_deref().map(digest).transpose()? != Some(value.member_digest)
            || indexed_path.as_deref() != Some(value.path.as_str())
            || identity_definition.as_deref() != Some(definition_hex.as_str())
            || (direction == ClaimDependencyDirection::Subject
                && value.subject_endpoint != endpoint)
            || (direction == ClaimDependencyDirection::Object && value.object_endpoint != endpoint)
        {
            return Err(DurableError::Conflict(
                "Claim dependency point row differs from current source",
            ));
        }
        if !result.insert(value.edge()) {
            return Err(DurableError::Corrupt(
                "duplicate Claim dependency point row",
            ));
        }
    }
    if generation == 0 && !result.is_empty() {
        return Err(DurableError::Corrupt(
            "Claim dependency rows lack a range predicate generation",
        ));
    }
    Ok(ClaimDependencyObservation {
        rows: result,
        predicate_token: token,
        generation,
    })
}

/// Read exact current reverse references for one endpoint while the caller
/// holds the source/audit transaction and epoch fences. The returned Range
/// generation is the shared completeness marker; every changed navigation
/// contribution advances it, so an empty endpoint result is still fenced.
pub(crate) fn lookup_current_navigation_references(
    tx: &mut Transaction<'_>,
    domain: &str,
    kind: ClaimNavigationReferenceKind,
    endpoint: &str,
    definition: Digest256,
) -> DurableResult<ClaimNavigationReferenceObservation> {
    let key_token = navigation_reference_token(kind, endpoint)?;
    let definition_hex = definition.to_hex();
    let completeness_generation =
        require_navigation_completeness_generation(tx, domain, definition)?;
    let key_generation = tx.query_opt(
        "SELECT generation,complete,definition_version FROM cmd2_predicate
         WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4",
        &[
            &domain,
            &crate::source_cohort::OWNER,
            &NAVIGATION_REFERENCE_SCOPE,
            &key_token,
        ],
    )?;
    let key_generation = match key_generation {
        Some(row) => {
            let generation: i64 = row.get("generation");
            if generation < 0
                || !row.get::<_, bool>("complete")
                || row.get::<_, String>("definition_version") != definition_hex
            {
                return Err(DurableError::Conflict(
                    "Claim navigation endpoint predicate is incomplete or stale",
                ));
            }
            generation
        }
        None => 0,
    };
    let limit = as_i64(MAX_NAVIGATION_REFERENCE_ROWS + 1)?;
    let rows = tx.query(
        "SELECT i.claim_id,i.path,i.basis,i.endpoint_kind,i.endpoint,i.member_digest,i.definition_digest,
                c.content_digest,c.content_length,c.commit_seq,
                s.path AS identity_path,s.definition_digest AS identity_definition
         FROM cmd2_source_claim_navigation_reference i
         LEFT JOIN cmd2_current c ON c.domain=i.domain AND c.subject=i.path
         LEFT JOIN cmd2_source_index s ON s.domain=i.domain AND s.kind='claim' AND s.token=i.claim_id
         WHERE i.domain=$1 AND i.endpoint_kind=$2 AND i.endpoint COLLATE \"C\"=$3 COLLATE \"C\"
         ORDER BY i.path COLLATE \"C\",i.claim_id COLLATE \"C\" LIMIT $4",
        &[&domain, &kind.as_str(), &endpoint, &limit],
    )?;
    if rows.len() > MAX_NAVIGATION_REFERENCE_ROWS {
        return Err(DurableError::Refused(
            "Claim navigation reference point result bound exceeded",
        ));
    }
    let mut result = BTreeSet::new();
    for row in rows {
        let value = NavigationReferenceIndexRow {
            claim_id: row.get("claim_id"),
            path: row.get("path"),
            basis: NavigationReferenceBasis::parse(row.get("basis"))?,
            kind: ClaimNavigationReferenceKind::parse(row.get("endpoint_kind"))?,
            endpoint: row.get("endpoint"),
            member_digest: digest(row.get("member_digest"))?,
            definition_digest: digest(row.get("definition_digest"))?,
        };
        validate_navigation_reference_row(&value)?;
        let current_digest: Option<String> = row.get("content_digest");
        let content_length: Option<i64> = row.get("content_length");
        let commit_seq: Option<i64> = row.get("commit_seq");
        let indexed_path: Option<String> = row.get("identity_path");
        let identity_definition: Option<String> = row.get("identity_definition");
        let size_bytes = content_length
            .and_then(|length| u64::try_from(length).ok())
            .ok_or(DurableError::Conflict(
                "Claim navigation current stream size absent",
            ))?;
        let commit_seq = commit_seq
            .and_then(|sequence| u64::try_from(sequence).ok())
            .filter(|sequence| *sequence > 0)
            .ok_or(DurableError::Conflict(
                "Claim navigation current stream sequence absent",
            ))?;
        if value.kind != kind
            || value.endpoint != endpoint
            || value.definition_digest.to_hex() != definition_hex
            || current_digest.as_deref().map(digest).transpose()? != Some(value.member_digest)
            || indexed_path.as_deref() != Some(value.path.as_str())
            || identity_definition.as_deref() != Some(definition_hex.as_str())
        {
            return Err(DurableError::Conflict(
                "Claim navigation point row differs from current source",
            ));
        }
        let reference = ClaimNavigationReference {
            claim_id: value.claim_id,
            path: value.path,
            kind,
            endpoint: endpoint.to_owned(),
            member_digest: value.member_digest,
            size_bytes,
            commit_seq,
        };
        result.insert(reference);
    }
    if key_generation == 0 && !result.is_empty() {
        return Err(DurableError::Corrupt(
            "Claim navigation rows lack an endpoint generation",
        ));
    }
    Ok(ClaimNavigationReferenceObservation {
        kind,
        endpoint: endpoint.to_owned(),
        rows: result,
        predicate_token: navigation_reference_complete_token()?,
        generation: completeness_generation,
    })
}
