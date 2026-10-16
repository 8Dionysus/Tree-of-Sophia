//! Typed reverse dependency rows for native Item adoption and source
//! navigation. Source bytes remain the authority; these tables are bounded,
//! audited point indexes over exact current Claim streams.

use crate::{DurableError, DurableResult};
use postgres::Transaction;
use postgres::fallible_iterator::FallibleIterator;
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{Digest256, Digest256Hasher, JsonValue, RelativePath};

pub(crate) const PREDICATE_SCOPE: &str = "claim-dependency-v1";
pub(crate) const NAVIGATION_REFERENCE_SCOPE: &str = "claim-navigation-reference-v2";
pub(crate) const MAX_CLAIM_STREAMS: usize = 4_096;
pub(crate) const MAX_STREAM_BYTES: usize = 1_048_576;
pub(crate) const MAX_TOTAL_CLAIM_BYTES: usize = 268_435_456;
const MAX_INDEX_ROWS: usize = 65_536;
const MAX_PREDICATE_ROWS: usize = MAX_INDEX_ROWS * 2;
pub(crate) const MAX_LOOKUP_ROWS: usize = 8_192;
pub(crate) const MAX_PATH_BYTES: usize = 4_096;
/// Matches the bounded current shared-schema path query used during updates.
pub(crate) const MAX_SHARED_SCHEMA_PATHS: usize = 8_192;
const MAX_ID_BYTES: usize = 4_096;
pub(crate) const MAX_NAVIGATION_REFERENCE_ROWS: usize = 262_144;
pub(crate) const MAX_NAVIGATION_REFERENCE_LOOKUPS: usize = 8_192;
const NAVIGATION_REFERENCE_COMPLETE_MARKER: &str =
    "claim-navigation-reference-index-complete-v3-intervals";
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
    pub(crate) valid_from_seq: u64,
    pub(crate) valid_to_seq: Option<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct ClaimNavigationReferenceObservation {
    pub(crate) kind: ClaimNavigationReferenceKind,
    pub(crate) endpoint: String,
    pub(crate) rows: BTreeSet<ClaimNavigationReference>,
    pub(crate) predicate_token: String,
    pub(crate) generation: u64,
    pub(crate) completeness_token: String,
    pub(crate) completeness_generation: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
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

fn as_i64(value: impl TryInto<i64>) -> DurableResult<i64> {
    value
        .try_into()
        .map_err(|_| DurableError::Refused("Claim dependency count overflow"))
}

fn as_u64(value: i64) -> DurableResult<u64> {
    u64::try_from(value).map_err(|_| DurableError::Corrupt("Claim navigation sequence invalid"))
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

pub(crate) fn is_claim_stream_path(path: &str) -> bool {
    (path.starts_with("ToS/source-witnesses/relations/")
        && path.ends_with("/source-claims.jsonl")
        && path.split('/').count() == 5)
        || crate::source_creation_store::is_growth_claim_stream_path(path)
        || crate::source_claims::is_managed_item_claim_stream_path(path)
}

pub(crate) fn is_shared_schema_source_path(path: &str) -> bool {
    path.starts_with("ToS/contracts/") && path.ends_with(".schema.json")
}

/// Source members that participate in Claim navigation reconstruction. Claim
/// streams provide row values; selected shared schemas provide exact
/// path-valued endpoints. Keep this selector shared by full and streamed
/// bootstrap/verification routes.
pub(crate) fn is_claim_navigation_source_member(path: &str) -> bool {
    is_claim_stream_path(path) || is_shared_schema_source_path(path)
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

fn text<'a>(value: &'a JsonValue, key: &'static str) -> DurableResult<&'a str> {
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
    owner_files: &BTreeMap<String, Vec<u8>>,
    prior_owner_refs: &BTreeSet<String>,
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
        ] {
            owner_references.insert((ClaimNavigationReferenceKind::SourcePath, owned_path));
        }
        let home = path
            .strip_suffix("/source-claims.jsonl")
            .ok_or(DurableError::Corrupt("Claim stream home invalid"))?;
        let request_path = format!("{home}/source-create-request.json");
        let typed_receipt = owner_files
            .get(&request_path)
            .map(|raw| crate::source_creation_store::pg_growth_claim_receipt_name(path, raw))
            .transpose()?
            .flatten();
        if crate::source_claims::is_managed_item_claim_stream_path(path) {
            // Item adoption owns this Claim stream independently of Growth.
            // Keep dependencies on its receipt and current history even when
            // bootstrap supplies only the exact stream bytes to this index.
            for name in [
                "source-create-request.json",
                "source-create-environment.json",
                "edition-item-receipt.json",
                "source-revision-history.json",
            ] {
                owner_references.insert((
                    ClaimNavigationReferenceKind::SourcePath,
                    format!("{home}/{name}"),
                ));
            }
        } else if let Some(receipt_name) = typed_receipt {
            for name in [
                "source-create-request.json",
                "source-create-environment.json",
                receipt_name,
            ] {
                owner_references.insert((
                    ClaimNavigationReferenceKind::SourcePath,
                    format!("{home}/{name}"),
                ));
            }
            let history = format!("{home}/claim-revision-history.json");
            if owner_files.contains_key(&history) || prior_owner_refs.contains(&history) {
                owner_references.insert((ClaimNavigationReferenceKind::SourcePath, history));
            }
        } else if owner_files.contains_key(&request_path) {
            for name in ["source-create-receipt.json", "claim-revision-history.json"] {
                let path = format!("{home}/{name}");
                if owner_files.contains_key(&path) || prior_owner_refs.contains(&path) {
                    owner_references.insert((ClaimNavigationReferenceKind::SourcePath, path));
                }
            }
        } else if path.starts_with("ToS/source-witnesses/works/")
            || prior_owner_refs.iter().any(|reference| {
                reference.starts_with(&format!("{home}/"))
                    && (reference.ends_with("-receipt.json")
                        || reference.ends_with("/source-create-request.json"))
            })
        {
            for reference in prior_owner_refs {
                if reference.starts_with(&format!("{home}/")) {
                    owner_references
                        .insert((ClaimNavigationReferenceKind::SourcePath, reference.clone()));
                }
            }
        } else {
            // Global Claims preserve their existing request-independent route.
            for name in ["source-create-receipt.json", "claim-revision-history.json"] {
                owner_references.insert((
                    ClaimNavigationReferenceKind::SourcePath,
                    format!("{home}/{name}"),
                ));
            }
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
           AND valid_from_seq IS NOT NULL AND valid_to_seq IS NULL
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

/// Current source-derived range tokens for both Claim-owned indexes. The
/// navigation completeness marker exists even when the selected cut has no
/// Claim stream; older predicate generations are retained by the caller.
pub(crate) fn current_predicate_tokens(
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<(BTreeSet<String>, BTreeSet<String>)> {
    let dependencies = rowset_tokens(&full_source_rows(files, definition)?)?;
    let navigation_rows = full_navigation_source_rows(files, definition)?;
    let mut navigation = navigation_rowset_tokens(&navigation_rows)?;
    navigation.insert(navigation_reference_complete_token()?);
    Ok((dependencies, navigation))
}

fn full_navigation_source_rows(
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<BTreeSet<NavigationReferenceIndexRow>> {
    // Schema bytes are not read by this index; its contribution is the exact
    // selected path set only. Callers may use empty sentinels for these keys.
    let shared_schema_paths = files
        .keys()
        .filter(|path| is_shared_schema_source_path(path))
        .cloned()
        .collect::<BTreeSet<_>>();
    if shared_schema_paths.len() > MAX_SHARED_SCHEMA_PATHS {
        return Err(DurableError::Refused(
            "Claim navigation shared schema path bound exceeded",
        ));
    }
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
            files,
            &BTreeSet::new(),
        )?);
    }
    Ok(rows)
}

fn current_schema_paths(tx: &mut Transaction<'_>, domain: &str) -> DurableResult<BTreeSet<String>> {
    let limit = as_i64(MAX_SHARED_SCHEMA_PATHS + 1)?;
    let rows = tx.query(
        "SELECT subject FROM cmd2_current
         WHERE domain=$1 AND subject COLLATE \"C\" >= 'ToS/contracts/' COLLATE \"C\"
           AND subject COLLATE \"C\" < 'ToS/contracts0' COLLATE \"C\"
           AND subject LIKE 'ToS/contracts/%.schema.json'
         ORDER BY subject COLLATE \"C\" LIMIT $2",
        &[&domain, &limit],
    )?;
    if rows.len() > MAX_SHARED_SCHEMA_PATHS {
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
    let limit = as_i64(MAX_SHARED_SCHEMA_PATHS + 1)?;
    let rows = tx.query(
        "SELECT DISTINCT endpoint FROM cmd2_source_claim_navigation_reference
         WHERE domain=$1 AND basis=$2 AND endpoint_kind='source-path'
           AND valid_from_seq IS NOT NULL AND valid_to_seq IS NULL
         ORDER BY endpoint COLLATE \"C\" LIMIT $3",
        &[
            &domain,
            &NavigationReferenceBasis::SharedSchema.as_str(),
            &limit,
        ],
    )?;
    if rows.len() > MAX_SHARED_SCHEMA_PATHS {
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
           AND (
             (i.path COLLATE \"C\" >= 'ToS/source-witnesses/relations/' COLLATE \"C\"
              AND i.path COLLATE \"C\" < 'ToS/source-witnesses/relations0' COLLATE \"C\"
              AND i.path LIKE 'ToS/source-witnesses/relations/%/source-claims.jsonl')
             OR i.path LIKE 'ToS/source-witnesses/works/%/expressions/%/source-claims.jsonl'
             OR i.path LIKE 'ToS/source-witnesses/works/%/expressions/%/editions/%/source-claims.jsonl'
             OR i.path LIKE 'ToS/source-witnesses/items/%/source-claims.jsonl'
             OR i.path ~ '^ToS/source-witnesses/([^/]+/)*editions/[^/]+/items/[^/]+/source-claims[.]jsonl$'
           )
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
    sequence: u64,
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
            "SELECT claim_id,path,basis,endpoint_kind,endpoint,member_digest,definition_digest,
                    valid_from_seq,valid_to_seq
             FROM cmd2_source_claim_navigation_reference
             WHERE domain=$1 AND basis=$2 AND endpoint_kind='source-path'
               AND endpoint COLLATE \"C\"=$3 COLLATE \"C\"
               AND valid_from_seq IS NOT NULL AND valid_to_seq IS NULL
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
    let maximum_additions =
        added
            .len()
            .checked_mul(current_owners.len())
            .ok_or(DurableError::Refused(
                "Claim navigation schema addition row bound exceeded",
            ))?;
    if maximum_additions > MAX_NAVIGATION_REFERENCE_ROWS {
        return Err(DurableError::Refused(
            "Claim navigation schema addition change-set bound exceeded",
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
            tx.execute(
                "INSERT INTO cmd2_source_claim_navigation_reference(
                   domain,claim_id,path,basis,endpoint_kind,endpoint,member_digest,definition_digest,
                   valid_from_seq,valid_to_seq
                 ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,NULL)",
                &[
                    &domain,
                    &row.claim_id,
                    &row.path,
                    &row.basis.as_str(),
                    &row.kind.as_str(),
                    &row.endpoint,
                    &row.member_digest.to_hex(),
                    &row.definition_digest.to_hex(),
                    &as_i64(sequence)?,
                ],
            )?;
            changed_tokens.insert(row.token()?);
        }
    }
    for (path, claim_id, endpoint) in &removed_rows {
        let closed = tx.execute(
            "UPDATE cmd2_source_claim_navigation_reference SET valid_to_seq=$6
             WHERE domain=$1 AND path=$2 AND claim_id=$3 AND basis=$4
               AND endpoint_kind='source-path' AND endpoint=$5
               AND valid_from_seq IS NOT NULL AND valid_from_seq<$6 AND valid_to_seq IS NULL",
            &[
                &domain,
                path,
                claim_id,
                &NavigationReferenceBasis::SharedSchema.as_str(),
                endpoint,
                &as_i64(sequence)?,
            ],
        )?;
        if closed != 1 {
            return Err(DurableError::Conflict(
                "Claim navigation schema interval changed during replacement",
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
    sequence: u64,
) -> DurableResult<()>
where
    I: IntoIterator<Item = (&'a str, Option<&'a [u8]>)>,
{
    let members = members.into_iter().collect::<Vec<_>>();
    let schema_membership_changed = members
        .iter()
        .any(|(path, _)| path.starts_with("ToS/contracts/") && path.ends_with(".schema.json"));
    let claim_stream_changed = members
        .iter()
        .any(|(path, bytes)| bytes.is_some() && is_claim_stream_path(path));
    require_navigation_completeness_generation(tx, domain, definition)?;
    let domain_row = tx.query_opt(
        "SELECT source_claim_navigation_reference_baseline_seq,head_seq,source_complete
         FROM cmd2_domain WHERE domain=$1 FOR SHARE",
        &[&domain],
    )?;
    let baseline = domain_row
        .as_ref()
        .and_then(|row| row.get::<_, Option<i64>>(0))
        .map(as_u64)
        .transpose()?
        .ok_or(DurableError::Conflict(
            "Claim navigation interval baseline absent",
        ))?;
    let domain_row = domain_row.ok_or(DurableError::Conflict("Claim navigation domain absent"))?;
    let current_head = as_u64(domain_row.get(1))?;
    if !domain_row.get::<_, bool>(2) || sequence <= baseline || sequence <= current_head {
        return Err(DurableError::Conflict(
            "Claim navigation source sequence does not follow its baseline",
        ));
    }
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
    let retained_shared_schema_paths = if schema_membership_changed {
        old_shared_schema_paths
            .intersection(&shared_schema_paths)
            .cloned()
            .collect::<BTreeSet<_>>()
    } else {
        BTreeSet::new()
    };
    let mut seen_paths = BTreeSet::new();
    let mut changed_tokens = BTreeSet::new();
    let mut changed_navigation_tokens = BTreeSet::new();
    let owner_files = members
        .iter()
        .filter_map(|(member_path, member_bytes)| {
            member_bytes.map(|raw| ((*member_path).to_owned(), raw.to_vec()))
        })
        .collect::<BTreeMap<_, _>>();
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
        let member_schema_paths = if schema_membership_changed {
            &retained_shared_schema_paths
        } else {
            &shared_schema_paths
        };
        let prior_owner_refs = old_navigation
            .iter()
            .filter(|row| {
                row.basis == NavigationReferenceBasis::OwnerInput
                    && row.kind == ClaimNavigationReferenceKind::SourcePath
            })
            .map(|row| row.endpoint.clone())
            .collect::<BTreeSet<_>>();
        let new_navigation = match bytes {
            Some(bytes) if is_claim_stream_path(path) => navigation_reference_rows(
                path,
                bytes,
                definition,
                member_schema_paths,
                &owner_files,
                &prior_owner_refs,
            )?,
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
        for row in old_navigation.difference(&new_navigation) {
            let closed = tx.execute(
                "UPDATE cmd2_source_claim_navigation_reference SET valid_to_seq=$8
                 WHERE domain=$1 AND path=$2 AND claim_id=$3 AND basis=$4
                   AND endpoint_kind=$5 AND endpoint=$6 AND member_digest=$7
                   AND valid_from_seq IS NOT NULL AND valid_from_seq<$8 AND valid_to_seq IS NULL",
                &[
                    &domain,
                    &row.path,
                    &row.claim_id,
                    &row.basis.as_str(),
                    &row.kind.as_str(),
                    &row.endpoint,
                    &row.member_digest.to_hex(),
                    &as_i64(sequence)?,
                ],
            )?;
            if closed != 1 {
                return Err(DurableError::Conflict(
                    "Claim navigation reference interval changed during replacement",
                ));
            }
        }
        for row in new_navigation.difference(&old_navigation) {
            tx.execute(
                "INSERT INTO cmd2_source_claim_navigation_reference(
                   domain,claim_id,path,basis,endpoint_kind,endpoint,member_digest,definition_digest,
                   valid_from_seq,valid_to_seq
                 ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,NULL)",
                &[
                    &domain,
                    &row.claim_id,
                    &row.path,
                    &row.basis.as_str(),
                    &row.kind.as_str(),
                    &row.endpoint,
                    &row.member_digest.to_hex(),
                    &row.definition_digest.to_hex(),
                    &as_i64(sequence)?,
                ],
            )?;
        }
    }
    if schema_membership_changed {
        replace_shared_schema_references(
            tx,
            domain,
            definition,
            sequence,
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
    navigation_completeness_generation(tx, domain, definition, true)
}

fn navigation_completeness_generation(
    tx: &mut Transaction<'_>,
    domain: &str,
    definition: Digest256,
    require_complete: bool,
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
        || (require_complete && !row.get::<_, bool>("complete"))
        || row.get::<_, String>("definition_version") != definition.to_hex()
    {
        return Err(DurableError::Conflict(
            "Claim navigation reference index completeness marker stale",
        ));
    }
    u64::try_from(generation)
        .map_err(|_| DurableError::Corrupt("Claim navigation reference generation invalid"))
}

pub(crate) fn navigation_completeness_observation(
    tx: &mut Transaction<'_>,
    domain: &str,
    definition: Digest256,
) -> DurableResult<(String, u64)> {
    Ok((
        navigation_reference_complete_token()?,
        require_navigation_completeness_generation(tx, domain, definition)?,
    ))
}

fn validate_navigation_cut_state(
    baseline: Option<u64>,
    head: u64,
    source_complete: bool,
    require_source_complete: bool,
) -> DurableResult<(u64, u64)> {
    let baseline = baseline.ok_or(DurableError::Conflict(
        "Claim navigation interval baseline absent",
    ))?;
    // Exhaustive owner reassessment also runs after maintenance invalidates
    // readiness at a later head. It verifies every current row and retained
    // interval under the audit fence before publishing completeness again.
    // Public lookups always require aggregate completion.
    if baseline > head || (require_source_complete && !source_complete) {
        return Err(DurableError::Conflict(
            "Claim navigation interval baseline or source completeness differs",
        ));
    }
    Ok((baseline, head))
}

fn navigation_cut(
    tx: &mut Transaction<'_>,
    domain: &str,
    require_source_complete: bool,
) -> DurableResult<(u64, u64)> {
    let row = tx
        .query_opt(
            "SELECT source_claim_navigation_reference_baseline_seq,head_seq,source_complete
         FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?
        .ok_or(DurableError::Conflict("Claim navigation domain absent"))?;
    validate_navigation_cut_state(
        row.get::<_, Option<i64>>(0).map(as_u64).transpose()?,
        as_u64(row.get(1))?,
        row.get(2),
        require_source_complete,
    )
}

/// Rebuild the complete endpoint index only during the selected bootstrap.
/// Ordinary source writes must use `replace_current_members`.
pub(crate) fn rebuild_current(
    tx: &mut Transaction<'_>,
    domain: &str,
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
    baseline_seq: u64,
) -> DurableResult<()> {
    let (head, source_complete, existing_baseline): (u64, bool, Option<u64>) = {
        let row = tx
            .query_opt(
                "SELECT head_seq,source_complete,source_claim_navigation_reference_baseline_seq
             FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
                &[&domain],
            )?
            .ok_or(DurableError::Conflict("Claim navigation domain absent"))?;
        (
            as_u64(row.get(0))?,
            row.get(1),
            row.get::<_, Option<i64>>(2).map(as_u64).transpose()?,
        )
    };
    if head != baseline_seq || source_complete || existing_baseline.is_some() {
        return Err(DurableError::Conflict(
            "Claim navigation rebuild is not at the fresh bootstrap cut",
        ));
    }
    let legacy_rows: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM cmd2_source_claim_navigation_reference WHERE domain=$1",
            &[&domain],
        )?
        .get(0);
    if legacy_rows != 0 {
        return Err(DurableError::Conflict(
            "Claim navigation bootstrap refuses unversioned legacy rows",
        ));
    }
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

    // The preceding fresh-target guard proves there are no stored temporal
    // rows to load. Historical/current intervals are never fabricated here.
    let old_navigation = BTreeSet::new();
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
               domain,claim_id,path,basis,endpoint_kind,endpoint,member_digest,definition_digest,
               valid_from_seq,valid_to_seq
             ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,NULL)",
            &[
                &domain,
                &row.claim_id,
                &row.path,
                &row.basis.as_str(),
                &row.kind.as_str(),
                &row.endpoint,
                &row.member_digest.to_hex(),
                &row.definition_digest.to_hex(),
                &as_i64(baseline_seq)?,
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
    )?;
    tx.execute(
        "UPDATE cmd2_domain SET source_claim_navigation_reference_baseline_seq=$2
         WHERE domain=$1 AND source_claim_navigation_reference_baseline_seq IS NULL
           AND source_complete=false AND head_seq=$2",
        &[&domain, &as_i64(baseline_seq)?],
    )?;
    Ok(())
}

/// Verify exact equality with a complete selected ToS source. This is a cold
/// read-only check; it never repairs a missing or forged index contribution.
/// Cleared readiness is not required for this full reassessment: the cold
/// owner re-enables it only after validating all indexes under the same fence.
pub(crate) fn verify_for_reassessment(
    tx: &mut Transaction<'_>,
    domain: &str,
    files: &BTreeMap<String, Vec<u8>>,
    definition: Digest256,
) -> DurableResult<()> {
    let (baseline_seq, head_seq) = navigation_cut(tx, domain, false)?;
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
               AND (definition_version<>$4 OR generation<=0)
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
            || row.get::<_, String>("definition_version") != expected_definition
        {
            return Err(DurableError::Corrupt(
                "Claim dependency range predicate is incomplete or stale",
            ));
        }
    }

    let expected_navigation = full_navigation_source_rows(files, definition)?;
    {
        let mut intervals = tx.query_raw(
            "SELECT i.claim_id,i.path,i.basis,i.endpoint_kind,i.endpoint,
                    i.member_digest,i.definition_digest,i.valid_from_seq,i.valid_to_seq,
                    h.content_digest AS history_digest,h.commit_seq AS history_commit_seq,
                    h.source_mode AS history_source_mode
             FROM cmd2_source_claim_navigation_reference i
             LEFT JOIN LATERAL (
               SELECT content_digest,commit_seq,source_mode
               FROM cmd2_history h
               WHERE h.domain=i.domain AND h.subject=i.path AND h.commit_seq<=i.valid_from_seq
               ORDER BY h.revision DESC LIMIT 1
             ) h ON TRUE
             WHERE i.domain=$1
             ORDER BY i.path COLLATE \"C\",i.claim_id COLLATE \"C\",i.basis COLLATE \"C\",
                      i.endpoint_kind COLLATE \"C\",i.endpoint COLLATE \"C\",i.valid_from_seq",
            &[&domain],
        )?;
        let mut previous_identity: Option<(
            String,
            String,
            NavigationReferenceBasis,
            ClaimNavigationReferenceKind,
            String,
        )> = None;
        let mut previous_end: Option<u64> = None;
        while let Some(row) = intervals.next()? {
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
            let valid_from = as_u64(row.get("valid_from_seq"))?;
            let valid_to = row
                .get::<_, Option<i64>>("valid_to_seq")
                .map(as_u64)
                .transpose()?;
            let history_digest: Option<String> = row.get("history_digest");
            let history_commit_seq: Option<i64> = row.get("history_commit_seq");
            let history_source_mode: Option<i32> = row.get("history_source_mode");
            let history_digest =
                history_digest
                    .as_deref()
                    .map(digest)
                    .transpose()?
                    .ok_or(DurableError::Corrupt(
                        "Claim navigation interval source history absent",
                    ))?;
            let history_commit_seq =
                history_commit_seq
                    .map(as_u64)
                    .transpose()?
                    .ok_or(DurableError::Corrupt(
                        "Claim navigation interval source sequence absent",
                    ))?;
            if value.definition_digest.to_hex() != expected_definition
                || history_digest != value.member_digest
                || history_commit_seq > valid_from
                || history_source_mode.is_none()
                || valid_from < baseline_seq
                || valid_from > head_seq
                || valid_to.is_some_and(|end| end <= valid_from || end > head_seq)
            {
                return Err(DurableError::Conflict(
                    "Claim navigation interval differs from its source cut",
                ));
            }
            let identity = (
                value.path,
                value.claim_id,
                value.basis,
                value.kind,
                value.endpoint,
            );
            let same_identity = previous_identity.as_ref() == Some(&identity);
            if same_identity {
                if previous_end.is_none_or(|end| end > valid_from) {
                    return Err(DurableError::Corrupt("Claim navigation intervals overlap"));
                }
            } else {
                previous_identity = Some(identity);
            }
            previous_end = valid_to;
        }
    }

    let navigation_limit = as_i64(expected_navigation.len().checked_add(1).ok_or(
        DurableError::Refused("Claim navigation selected row count overflow"),
    )?)?;
    let navigation_rows = tx.query(
        "SELECT i.claim_id,i.path,i.basis,i.endpoint_kind,i.endpoint,i.member_digest,i.definition_digest,
                i.valid_from_seq,
                c.content_digest,c.content_length,c.commit_seq,
                s.path AS identity_path,s.definition_digest AS identity_definition
         FROM cmd2_source_claim_navigation_reference i
         LEFT JOIN cmd2_current c ON c.domain=i.domain AND c.subject=i.path
         LEFT JOIN cmd2_source_index s ON s.domain=i.domain AND s.kind='claim' AND s.token=i.claim_id
         WHERE i.domain=$1 AND i.valid_from_seq IS NOT NULL AND i.valid_to_seq IS NULL
         ORDER BY i.path COLLATE \"C\",i.claim_id COLLATE \"C\",i.endpoint_kind COLLATE \"C\",i.endpoint COLLATE \"C\"
         LIMIT $2",
        &[&domain, &navigation_limit],
    )?;
    if navigation_rows.len() > expected_navigation.len() {
        return Err(DurableError::Conflict(
            "Claim navigation reference cold rowset exceeds selected source",
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
        let valid_from_seq = as_u64(row.get("valid_from_seq"))?;
        let content_length: Option<i64> = row.get("content_length");
        let commit_seq: Option<i64> = row.get("commit_seq");
        let indexed_path: Option<String> = row.get("identity_path");
        let identity_definition: Option<String> = row.get("identity_definition");
        let expected_len = files.get(&value.path).map(|bytes| bytes.len() as u64);
        if value.definition_digest.to_hex() != expected_definition
            || current_digest.as_deref().map(digest).transpose()? != Some(value.member_digest)
            || content_length.and_then(|length| u64::try_from(length).ok()) != expected_len
            || commit_seq.is_none_or(|sequence| sequence <= 0)
            || valid_from_seq < baseline_seq
            || valid_from_seq > head_seq
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
    let _ = navigation_completeness_generation(tx, domain, definition, false)?;
    if tx
        .query_opt(
            "SELECT 1 FROM cmd2_predicate
             WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3
               AND (definition_version<>$4 OR generation<=0)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reassessment_navigation_cut_preserves_baseline_and_public_completion_fence() {
        assert_eq!(
            validate_navigation_cut_state(Some(17), 17, false, false)
                .expect("fresh owner verification may precede aggregate completion"),
            (17, 17),
        );
        assert!(validate_navigation_cut_state(Some(17), 17, false, true).is_err());
        assert_eq!(
            validate_navigation_cut_state(Some(16), 17, false, false)
                .expect("full reassessment retains the original interval baseline"),
            (16, 17),
        );
        assert!(validate_navigation_cut_state(Some(16), 17, false, true).is_err());
        assert!(validate_navigation_cut_state(None, 17, false, false).is_err());
        assert!(validate_navigation_cut_state(Some(18), 17, true, false).is_err());
        assert_eq!(
            validate_navigation_cut_state(Some(17), 20, true, true)
                .expect("retained current lookup keeps its historical baseline"),
            (17, 20),
        );
    }

    #[test]
    fn empty_claim_selection_still_derives_navigation_completeness_marker() {
        let files = BTreeMap::from([(
            "ToS/contracts/source-claim-record.schema.json".to_owned(),
            Vec::new(),
        )]);
        let definition = Digest256::of_bytes(b"definition");
        let (dependency_tokens, navigation_tokens) =
            current_predicate_tokens(&files, definition).expect("selected owner outputs");
        assert!(dependency_tokens.is_empty());
        assert!(
            full_navigation_source_rows(&files, definition)
                .expect("selected navigation rowset")
                .is_empty()
        );
        assert_eq!(
            navigation_tokens,
            BTreeSet::from([navigation_reference_complete_token().expect("completion token")]),
        );
    }

    #[test]
    fn navigation_source_selector_admits_exact_growth_claim_homes_and_typed_receipt_refs() {
        let work_path = "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/source-claims.jsonl";
        let work_request = "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/source-create-request.json";
        let work_environment = "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/source-create-environment.json";
        let work_event = "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/source-create-provenance.jsonl";
        let work_receipt = "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/work-expression-receipt.json";
        let edition_path = "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281/source-claims.jsonl";
        let extra_nested_relation =
            "ToS/source-witnesses/relations/fixture/nested/source-claims.jsonl";
        let item_path = "ToS/source-witnesses/items/fixture/nested/source-claims.jsonl";

        assert!(is_claim_navigation_source_member(work_path));
        assert!(is_claim_navigation_source_member(edition_path));
        assert!(!is_claim_navigation_source_member(extra_nested_relation));
        assert!(!is_claim_navigation_source_member(item_path));

        let files = BTreeMap::from([
            (
                work_path.to_owned(),
                br#"{"claim_id":"tos.claim.fixture"}"#.to_vec(),
            ),
            (
                work_request.to_owned(),
                br#"{"schema_version":"tos_local_work_expression_command_v1","operation":"work.expression.create"}"#.to_vec(),
            ),
            (work_environment.to_owned(), Vec::new()),
            (work_event.to_owned(), Vec::new()),
            (work_receipt.to_owned(), Vec::new()),
        ]);
        let rows = full_navigation_source_rows(&files, Digest256::of_bytes(b"definition"))
            .expect("valid typed Growth Claim route");
        assert!(rows.iter().any(|row| {
            row.path == work_path
                && row.basis == NavigationReferenceBasis::OwnerInput
                && row.endpoint == work_request
        }));
        assert!(rows.iter().any(|row| {
            row.path == work_path
                && row.basis == NavigationReferenceBasis::OwnerInput
                && row.endpoint == work_receipt
        }));
        assert!(!rows.iter().any(|row| {
            row.path == work_path
                && row.basis == NavigationReferenceBasis::OwnerInput
                && row.endpoint.ends_with("/source-create-receipt.json")
        }));
    }

    #[test]
    fn item_exemplar_stream_contributes_reverse_edges_and_navigation_dependencies() {
        let home = "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281/items/acquired-note-utf8-20260910";
        let path = format!("{home}/source-claims.jsonl");
        let bytes = br#"{"claim_id":"tos.claim.item-exemplar","predicate":"exemplified_by","subject_ref":"tos.edition.fixture","object":"tos.item.fixture"}"#.to_vec();
        let files = BTreeMap::from([(path.clone(), bytes.clone())]);
        let definition = Digest256::of_bytes(b"definition");
        let rows = full_source_rows(&files, definition).expect("Item reverse edges");
        assert_eq!(rows.len(), 1);
        let edge = rows.get("tos.claim.item-exemplar").expect("Item exemplar");
        assert_eq!(edge.path, path);
        assert_eq!(edge.subject_endpoint, "tos.edition.fixture");
        assert_eq!(edge.object_endpoint, "tos.item.fixture");
        assert_eq!(edge.member_digest, Digest256::of_bytes(&bytes));
        let navigation =
            full_navigation_source_rows(&files, definition).expect("Item navigation references");
        for name in [
            "source-create-request.json",
            "source-create-environment.json",
            "edition-item-receipt.json",
            "source-revision-history.json",
        ] {
            assert!(navigation.iter().any(|row| {
                row.path == path
                    && row.basis == NavigationReferenceBasis::OwnerInput
                    && row.endpoint == format!("{home}/{name}")
            }));
        }
        assert!(!navigation.iter().any(|row| {
            row.path == path && row.endpoint == format!("{home}/source-create-receipt.json")
        }));
        assert!(is_claim_stream_path(
            "ToS/source-witnesses/items/direct/source-claims.jsonl"
        ));
        assert!(!is_claim_stream_path(&format!(
            "{home}/nested/source-claims.jsonl"
        )));
        assert!(!is_claim_stream_path(
            "ToS/source-witnesses/.record-revisions/item/source-claims.jsonl"
        ));
    }

    #[test]
    fn navigation_source_selector_keeps_claim_streams_and_shared_schemas() {
        let claim_path = "ToS/source-witnesses/relations/fixture/source-claims.jsonl";
        let schema_path = "ToS/contracts/source-claim-record.schema.json";
        let owner_local_claim =
            "ToS/source-witnesses/owner-local/relations/fixture/source-claims.jsonl";
        let non_schema_contract = "ToS/contracts/source-claim-record.json";

        assert!(is_claim_navigation_source_member(claim_path));
        assert!(is_claim_navigation_source_member(schema_path));
        assert!(!is_claim_navigation_source_member(owner_local_claim));
        assert!(!is_claim_navigation_source_member(non_schema_contract));

        let files = BTreeMap::from([
            (
                claim_path.to_owned(),
                br#"{"claim_id":"tos.claim.fixture","object":"ToS/source-witnesses/catalog/example.json"}"#.to_vec(),
            ),
            (schema_path.to_owned(), Vec::new()),
            (non_schema_contract.to_owned(), b"{}".to_vec()),
        ]);
        let rows = full_navigation_source_rows(&files, Digest256::of_bytes(b"definition"))
            .expect("valid Claim stream and shared schema source members");
        assert!(rows.iter().any(|row| {
            row.path == claim_path
                && row.basis == NavigationReferenceBasis::SharedSchema
                && row.endpoint == schema_path
        }));
        assert!(!rows.iter().any(|row| {
            row.basis == NavigationReferenceBasis::SharedSchema
                && row.endpoint == non_schema_contract
        }));
    }
}

/// Read exact reverse Claim references whose half-open interval contains an
/// authored-source cut. The caller must hold the audit/domain share fence;
/// the returned rows are checked against the newest committed source member
/// at that cut, not `cmd2_current`.
pub(crate) fn lookup_navigation_references_at(
    tx: &mut Transaction<'_>,
    domain: &str,
    kind: ClaimNavigationReferenceKind,
    endpoint: &str,
    definition: Digest256,
    as_of_sequence: u64,
    held_source_head_sequence: u64,
) -> DurableResult<ClaimNavigationReferenceObservation> {
    let key_token = navigation_reference_token(kind, endpoint)?;
    let definition_hex = definition.to_hex();
    let (baseline, head) = navigation_cut(tx, domain, true)?;
    if head != held_source_head_sequence
        || as_of_sequence < baseline
        || as_of_sequence > held_source_head_sequence
    {
        return Err(DurableError::Conflict(
            "Claim navigation as-of cut is outside the held source interval",
        ));
    }
    let completeness_token = navigation_reference_complete_token()?;
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
            u64::try_from(generation).map_err(|_| {
                DurableError::Corrupt("Claim navigation endpoint generation invalid")
            })?
        }
        None => 0,
    };
    let as_of = as_i64(as_of_sequence)?;
    let limit = as_i64(MAX_NAVIGATION_REFERENCE_ROWS + 1)?;
    let rows = tx.query(
        "SELECT i.claim_id,i.path,i.basis,i.endpoint_kind,i.endpoint,
                i.member_digest,i.definition_digest,i.valid_from_seq,i.valid_to_seq,
                h.content_digest AS history_digest,h.content_length AS history_length,
                h.commit_seq AS history_commit_seq,h.source_mode AS history_source_mode
         FROM cmd2_source_claim_navigation_reference i
         LEFT JOIN LATERAL (
           SELECT content_digest,content_length,commit_seq,source_mode
           FROM cmd2_history h
           WHERE h.domain=i.domain AND h.subject=i.path AND h.commit_seq<=$4
           ORDER BY h.commit_seq DESC LIMIT 1
         ) h ON TRUE
         WHERE i.domain=$1 AND i.endpoint_kind=$2
           AND i.endpoint COLLATE \"C\"=$3 COLLATE \"C\"
           AND i.valid_from_seq IS NOT NULL AND i.valid_from_seq<=$4
           AND (i.valid_to_seq IS NULL OR $4<i.valid_to_seq)
         ORDER BY i.path COLLATE \"C\",i.claim_id COLLATE \"C\",
                  i.basis COLLATE \"C\",i.valid_from_seq LIMIT $5",
        &[&domain, &kind.as_str(), &endpoint, &as_of, &limit],
    )?;
    if rows.len() > MAX_NAVIGATION_REFERENCE_ROWS {
        return Err(DurableError::Refused(
            "Claim navigation as-of point result bound exceeded",
        ));
    }
    let mut result = BTreeSet::new();
    let mut interval_owners = BTreeSet::new();
    for row in rows {
        let basis = NavigationReferenceBasis::parse(row.get("basis"))?;
        let value = NavigationReferenceIndexRow {
            claim_id: row.get("claim_id"),
            path: row.get("path"),
            basis,
            kind: ClaimNavigationReferenceKind::parse(row.get("endpoint_kind"))?,
            endpoint: row.get("endpoint"),
            member_digest: digest(row.get("member_digest"))?,
            definition_digest: digest(row.get("definition_digest"))?,
        };
        validate_navigation_reference_row(&value)?;
        let valid_from_seq = as_u64(row.get("valid_from_seq"))?;
        let valid_to_seq = row
            .get::<_, Option<i64>>("valid_to_seq")
            .map(as_u64)
            .transpose()?;
        let history_digest: Option<String> = row.get("history_digest");
        let history_length: Option<i64> = row.get("history_length");
        let history_commit_seq: Option<i64> = row.get("history_commit_seq");
        let history_source_mode: Option<i32> = row.get("history_source_mode");
        let member_digest =
            history_digest
                .as_deref()
                .map(digest)
                .transpose()?
                .ok_or(DurableError::Conflict(
                    "Claim navigation as-of source history absent",
                ))?;
        let size_bytes = history_length
            .map(as_u64)
            .transpose()?
            .ok_or(DurableError::Conflict(
                "Claim navigation as-of source size absent",
            ))?;
        let commit_seq = history_commit_seq
            .map(as_u64)
            .transpose()?
            .filter(|sequence| *sequence > 0 && *sequence <= as_of_sequence)
            .ok_or(DurableError::Conflict(
                "Claim navigation as-of source sequence absent",
            ))?;
        if value.kind != kind
            || value.endpoint != endpoint
            || value.definition_digest.to_hex() != definition_hex
            || member_digest != value.member_digest
            || history_source_mode.is_none()
            || valid_from_seq < baseline
            || valid_from_seq > as_of_sequence
            || valid_to_seq.is_some_and(|sequence| {
                sequence <= as_of_sequence || sequence <= valid_from_seq || sequence > head
            })
        {
            return Err(DurableError::Conflict(
                "Claim navigation as-of interval or source history differs",
            ));
        }
        if !interval_owners.insert((
            value.path.clone(),
            value.claim_id.clone(),
            value.basis,
            value.kind,
            value.endpoint.clone(),
        )) {
            return Err(DurableError::Corrupt(
                "overlapping Claim navigation intervals at as-of cut",
            ));
        }
        result.insert(ClaimNavigationReference {
            claim_id: value.claim_id,
            path: value.path,
            kind,
            endpoint: endpoint.to_owned(),
            member_digest: value.member_digest,
            size_bytes,
            commit_seq,
            valid_from_seq,
            valid_to_seq,
        });
    }
    if key_generation == 0 && !result.is_empty() {
        return Err(DurableError::Corrupt(
            "Claim navigation as-of rows lack an endpoint generation",
        ));
    }
    Ok(ClaimNavigationReferenceObservation {
        kind,
        endpoint: endpoint.to_owned(),
        rows: result,
        predicate_token: key_token,
        generation: key_generation,
        completeness_token,
        completeness_generation,
    })
}
