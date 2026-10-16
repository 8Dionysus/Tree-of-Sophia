//! Bounded PostgreSQL/STO transport for one initial native creation.
//!
//! The semantic producer owns the prepared object. This module owns only the
//! durable source transaction: exact readset admission, indexed absence and
//! uniqueness predicates, STO sealing, current/history membership, and the
//! outbox transition. It deliberately has no complete inventory reader and no
//! filesystem source mirror.

use super::*;
use crate::source_cohort::revision::{
    PG_AGENT_INDEX_KINDS, PgRecordRevisionIdentity, PgSourceHomeObservation,
    PgSourceUniqueIndexObservation, SourceRevisionReadSet,
};
use crate::source_command::{self as cmd, CommandContext, CommandPlan};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;

// The source-cohort commit protocol still recognizes this original durable
// profile ID. Keep the wire value stable while the adapter accepts the
// maintained Corpus and Artifact creation owners.
pub(crate) const PG_CORPUS_CREATION_PROFILE: &[u8] = b"tos.source-file.agent-create-pg-v1";
pub(crate) const PG_CREATION: &[u8] = PG_CORPUS_CREATION_PROFILE;
const PG_ARTIFACT_INDEXES_PROFILE: &str = "tos-pg-artifact-creation-indexes-v1";
const PG_ARTIFACT_FAMILY_TAG: &str = crate::source_creation::artifact_pg::ARTIFACT_FAMILY_TAG;
/// New creation-family rows use a distinct profile so existing Corpus and
/// Artifact source-index bytes remain stable across this admission extension.
const PG_FAMILY_INDEXES_PROFILE: &str = "tos-pg-family-creation-indexes-v2";
// This is a new bounded semantic commitment, deliberately separate from the
// legacy complete `source-inventory/all` predicate and its v1 namespace.
pub(crate) const COMMITMENT_VERSION: u16 = 2;
pub(crate) const DEPENDENCY_SCOPE: &str = "source-dependencies-v2";

const PG_CREATION_INDEX_KINDS: [&str; 5] = PG_AGENT_INDEX_KINDS;

/// A semantic dependency commitment is distinct from the legacy complete
/// inventory predicate. It is produced by the bounded semantic owner and is
/// persisted with the durable attempt as an opaque exact binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PgDependencyCommitment {
    version: u16,
    digest: Digest256,
}
impl PgDependencyCommitment {
    pub(crate) fn new(version: u16, digest: Digest256) -> DurableResult<Self> {
        if version == 0 {
            return Err(DurableError::Invalid(
                "bounded Corpus dependency commitment version",
            ));
        }
        Ok(Self { version, digest })
    }
}

/// Capture owns the concrete sealed prepared type. Implementing this trait is
/// the only adapter boundary it needs; no public forgeable proposal is
/// accepted by registration.
pub(crate) trait PgCreationInput {
    fn context(&self) -> &CommandContext;
    /// The semantic owner keeps the unconsumed plan alongside its sealed
    /// creation payload.  Registration never accepts a caller-built
    /// `PreparedCommand`; it derives the base revision from `context` and
    /// binds every plan field into the durable delta.
    fn plan(&self) -> &CommandPlan;
    fn family_tag(&self) -> &str;
    /// New family adapters bind the exact invocation and selected software
    /// capture/components so a ready retry cannot substitute another run.
    fn replay_binding(&self) -> Option<Digest256> {
        None
    }
    fn record_type(&self) -> &str;
    fn home(&self) -> &RelativePath;
    fn files(&self) -> &BTreeMap<String, Vec<u8>>;
    fn indexes(&self) -> &IndexRows;
    fn dependency_commitment(&self) -> PgDependencyCommitment;
    fn identity(&self) -> PgRecordRevisionIdentity;
    fn home_observation(&self) -> &PgSourceHomeObservation;
    fn unique_observations(&self) -> &[PgSourceUniqueIndexObservation];
}

// Keep the first family adapter's internal name available to the existing
// native Corpus CLI while other creation owners use the family-neutral seam.
pub(crate) use self::PgCreationInput as PgCorpusCreationInput;

/// Exact durable registration state for one bounded initial source creation.
pub(crate) struct PgCreationAttempt {
    pub(crate) record_type: String,
    pub(crate) family_tag: Option<String>,
    pub(crate) domain: String,
    pub(crate) prepare_id: Vec<u8>,
    pub(crate) fence: u64,
    pub(crate) command_id: String,
    pub(crate) raw_request_digest: Digest256,
    pub(crate) configuration_raw_digest: Digest256,
    pub(crate) configuration_canonical_digest: Digest256,
    pub(crate) epoch: u64,
    pub(crate) source_generation: u64,
    pub(crate) audit_generation: u64,
    pub(crate) rights_version: u64,
    pub(crate) rule_version: u64,
    pub(crate) job_id: String,
    pub(crate) job_fence: u64,
    pub(crate) contract: Digest256,
    pub(crate) definition: Digest256,
    pub(crate) delta: Digest256,
    pub(crate) reads: SourceReads,
    pub(crate) metadata: SourceMetadata,
    pub(crate) indexes: IndexRows,
    pub(crate) index_binding: Vec<u8>,
    pub(crate) dependency_commitment: PgDependencyCommitment,
    pub(crate) replay_binding: Option<Digest256>,
    pub(crate) predicate_keys: BTreeSet<(String, String, String)>,
    pub(crate) writes: Vec<SourceRevisionWrite>,
    pub(crate) identity: Option<PgRecordRevisionIdentity>,
}

// Existing callers and the source-cohort commit enum keep their narrower
// compatibility names; the durable attempt now carries an admitted creation
// family, including Artifact-v1.
pub(crate) type PgCorpusCreationAttempt = PgCreationAttempt;
pub(crate) type PgAgentCreationAttempt = PgCreationAttempt;

fn identity_digest(
    identity: PgRecordRevisionIdentity,
    record_type: &str,
    family_tag: &str,
) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    let profile: &[u8] = match record_type {
        "agent" => b"tos-pg-agent-creation-read-epoch-v1",
        "artifact" => b"tos-pg-artifact-creation-read-epoch-v1",
        _ if is_pg_family_tag(family_tag) => b"tos-pg-family-creation-read-epoch-v1",
        _ => b"tos-pg-corpus-creation-read-epoch-v1",
    };
    part(&mut hasher, profile);
    if record_type == "artifact" || is_pg_family_tag(family_tag) {
        part(&mut hasher, family_tag.as_bytes());
    }
    if record_type != "agent" {
        part(&mut hasher, record_type.as_bytes());
    }
    part(&mut hasher, identity.domain_digest.as_bytes());
    part(&mut hasher, &identity.store_id);
    part(&mut hasher, identity.bootstrap_revision.0.as_bytes());
    part(
        &mut hasher,
        &identity.bootstrap_membership.count.to_be_bytes(),
    );
    part(&mut hasher, identity.bootstrap_membership.digest.as_bytes());
    part(&mut hasher, identity.cohort_digest.as_bytes());
    part(&mut hasher, &identity.cohort_epoch.to_be_bytes());
    part(&mut hasher, identity.cohort_definition.as_bytes());
    part(&mut hasher, &identity.cohort_generation.to_be_bytes());
    part(&mut hasher, identity.job_id_digest.as_bytes());
    part(&mut hasher, &identity.job_fence.to_be_bytes());
    part(&mut hasher, &identity.epoch.to_be_bytes());
    part(&mut hasher, &identity.source_generation.to_be_bytes());
    part(&mut hasher, &identity.head_seq.to_be_bytes());
    part(&mut hasher, &identity.audit_generation.to_be_bytes());
    part(&mut hasher, &identity.rights_version.to_be_bytes());
    part(&mut hasher, &identity.rule_version.to_be_bytes());
    part(&mut hasher, identity.contract.as_bytes());
    hasher.finalize()
}

fn indexes_bytes(
    indexes: &IndexRows,
    commitment: PgDependencyCommitment,
    identity: PgRecordRevisionIdentity,
    record_type: &str,
    family_tag: &str,
    replay_binding: Option<Digest256>,
) -> DurableResult<Vec<u8>> {
    let rows = indexes.iter().collect::<Vec<_>>();
    let value = match record_type {
        "agent" => serde_json::json!({
            "profile": "tos-pg-agent-creation-indexes-v2",
            "commitment": [commitment.version, commitment.digest.to_hex()],
            "read_epoch": identity_digest(identity, record_type, family_tag).to_hex(),
            "indexes": rows,
        }),
        "artifact" => serde_json::json!({
            "profile": PG_ARTIFACT_INDEXES_PROFILE,
            "record_type": record_type,
            "family_tag": family_tag,
            "commitment": [commitment.version, commitment.digest.to_hex()],
            "read_epoch": identity_digest(identity, record_type, family_tag).to_hex(),
            "indexes": rows,
        }),
        _ if is_pg_family_tag(family_tag) => {
            let replay_binding = replay_binding.ok_or(DurableError::Invalid(
                "bounded PG family replay binding absent",
            ))?;
            serde_json::json!({
                "profile": PG_FAMILY_INDEXES_PROFILE,
                "record_type": record_type,
                "family_tag": family_tag,
                "commitment": [commitment.version, commitment.digest.to_hex()],
                "read_epoch": identity_digest(identity, record_type, family_tag).to_hex(),
                "replay_binding": replay_binding.to_hex(),
                "indexes": rows,
            })
        }
        _ => serde_json::json!({
            "profile": "tos-pg-corpus-creation-indexes-v2",
            "record_type": record_type,
            "commitment": [commitment.version, commitment.digest.to_hex()],
            "read_epoch": identity_digest(identity, record_type, family_tag).to_hex(),
            "indexes": rows,
        }),
    };
    serde_json::to_vec(&value)
        .map_err(|_| DurableError::Invalid("bounded PG creation indexes encode"))
}

fn parse_pg_indexes(
    raw: &[u8],
) -> DurableResult<(
    IndexRows,
    PgDependencyCommitment,
    Digest256,
    String,
    Option<String>,
    Option<Digest256>,
)> {
    let value: serde_json::Value = serde_json::from_slice(raw)
        .map_err(|_| DurableError::Corrupt("bounded PG creation indexes encoding"))?;
    let (record_type, family_tag, replay_binding) =
        match value.get("profile").and_then(|v| v.as_str()) {
            Some("tos-pg-agent-creation-indexes-v2") => ("agent".to_owned(), None, None),
            Some("tos-pg-corpus-creation-indexes-v2") => {
                let record_type = value
                    .get("record_type")
                    .and_then(|v| v.as_str())
                    .ok_or(DurableError::Corrupt("bounded Corpus record type"))?;
                if !["place", "organization", "work", "collection"].contains(&record_type) {
                    return Err(DurableError::Corrupt("bounded Corpus record type"));
                }
                (record_type.to_owned(), None, None)
            }
            Some(PG_ARTIFACT_INDEXES_PROFILE) => {
                let record_type = value
                    .get("record_type")
                    .and_then(|v| v.as_str())
                    .ok_or(DurableError::Corrupt("bounded Artifact record type"))?;
                let family_tag = value
                    .get("family_tag")
                    .and_then(|v| v.as_str())
                    .ok_or(DurableError::Corrupt("bounded Artifact family tag"))?;
                if record_type != "artifact" || family_tag != PG_ARTIFACT_FAMILY_TAG {
                    return Err(DurableError::Corrupt("bounded Artifact indexes family"));
                }
                (record_type.to_owned(), Some(family_tag.to_owned()), None)
            }
            Some(PG_FAMILY_INDEXES_PROFILE) => {
                if serde_json::to_vec(&value).ok().as_deref() != Some(raw) {
                    return Err(DurableError::Corrupt(
                        "bounded PG family creation indexes are noncanonical",
                    ));
                }
                let object = value
                    .as_object()
                    .filter(|object| {
                        object.len() == 7
                            && [
                                "profile",
                                "record_type",
                                "family_tag",
                                "commitment",
                                "read_epoch",
                                "replay_binding",
                                "indexes",
                            ]
                            .iter()
                            .all(|key| object.contains_key(*key))
                    })
                    .ok_or(DurableError::Corrupt(
                        "bounded PG family creation index fields",
                    ))?;
                let record_type = object
                    .get("record_type")
                    .and_then(|value| value.as_str())
                    .filter(|value| !value.is_empty() && value.len() <= 128)
                    .ok_or(DurableError::Corrupt(
                        "bounded PG family creation record type",
                    ))?;
                let family_tag = object
                    .get("family_tag")
                    .and_then(|value| value.as_str())
                    .filter(|tag| is_pg_family_tag(tag))
                    .ok_or(DurableError::Corrupt("bounded PG family creation tag"))?;
                let replay_binding = Digest256::from_hex(
                    object
                        .get("replay_binding")
                        .and_then(|value| value.as_str())
                        .ok_or(DurableError::Corrupt("bounded PG family replay binding"))?,
                )
                .map_err(|_| DurableError::Corrupt("bounded PG family replay binding"))?;
                (
                    record_type.to_owned(),
                    Some(family_tag.to_owned()),
                    Some(replay_binding),
                )
            }
            _ => return Err(DurableError::Corrupt("bounded PG creation indexes profile")),
        };
    let commitment = value
        .get("commitment")
        .and_then(|v| v.as_array())
        .filter(|v| v.len() == 2)
        .ok_or(DurableError::Corrupt("bounded Corpus commitment encoding"))?;
    let version = commitment[0]
        .as_u64()
        .and_then(|v| u16::try_from(v).ok())
        .ok_or(DurableError::Corrupt("bounded Corpus commitment version"))?;
    let digest = Digest256::from_hex(
        commitment[1]
            .as_str()
            .ok_or(DurableError::Corrupt("bounded Corpus commitment digest"))?,
    )
    .map_err(|_| DurableError::Corrupt("bounded Corpus commitment digest"))?;
    let read_epoch = Digest256::from_hex(
        value
            .get("read_epoch")
            .and_then(|v| v.as_str())
            .ok_or(DurableError::Corrupt("bounded Corpus read epoch digest"))?,
    )
    .map_err(|_| DurableError::Corrupt("bounded Corpus read epoch digest"))?;
    let rows = value
        .get("indexes")
        .and_then(|v| v.as_array())
        .ok_or(DurableError::Corrupt("bounded Corpus indexes rows"))?;
    let mut indexes = IndexRows::new();
    for row in rows {
        let values = row
            .as_array()
            .filter(|values| values.len() == 3)
            .ok_or(DurableError::Corrupt("bounded Corpus index row"))?;
        let item = (
            values[0]
                .as_str()
                .ok_or(DurableError::Corrupt("bounded Corpus index kind"))?
                .to_owned(),
            values[1]
                .as_str()
                .ok_or(DurableError::Corrupt("bounded Corpus index token"))?
                .to_owned(),
            values[2]
                .as_str()
                .ok_or(DurableError::Corrupt("bounded Corpus index path"))?
                .to_owned(),
        );
        if (item.0 != "path" && !fixed_index(&item.0))
            || item.1.is_empty()
            || item.2.is_empty()
            || !item.2.starts_with("ToS/")
            || (item.0 == "path" && item.1 != item.2)
        {
            return Err(DurableError::Corrupt("bounded PG Corpus index profile"));
        }
        if !indexes.insert(item) {
            return Err(DurableError::Corrupt("duplicate bounded Corpus index"));
        }
    }
    Ok((
        indexes,
        PgDependencyCommitment::new(version, digest)?,
        read_epoch,
        record_type,
        family_tag,
        replay_binding,
    ))
}

fn fixed_index(kind: &str) -> bool {
    PG_CREATION_INDEX_KINDS.contains(&kind)
}

fn is_pg_family_tag(tag: &str) -> bool {
    matches!(
        tag,
        crate::source_creation::pg_family::PG_HISTORICAL_V1_FAMILY_TAG
            | crate::source_creation::pg_family::PG_HISTORICAL_V2_FAMILY_TAG
            | crate::source_creation::pg_family::PG_PUBLIC_PROFILE_FAMILY_TAG
    )
}

fn source_dependencies(reads: &SourceReads) -> Vec<String> {
    reads
        .reads
        .iter()
        .filter_map(|read| match read {
            PredicateRead::Exact { key, .. } => Some(key.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn required_generation(
    reads: &SourceReads,
    kind: &str,
    scope: &str,
    token: &str,
) -> DurableResult<u64> {
    reads
        .reads
        .iter()
        .find_map(|read| match read {
            PredicateRead::Generation {
                predicate,
                observed_generation,
            } if predicate.kind.as_str() == kind
                && predicate.owner == OWNER
                && predicate.scope == scope
                && predicate.token == token
                && predicate.definition_version == definition().to_hex() =>
            {
                Some(*observed_generation)
            }
            _ => None,
        })
        .ok_or(DurableError::Corrupt(
            "bounded Corpus required predicate read absent",
        ))
}

fn source_generation(reads: &SourceReads) -> DurableResult<u64> {
    reads
        .reads
        .iter()
        .find_map(|read| match read {
            PredicateRead::Generation {
                predicate,
                observed_generation,
            } if predicate.kind == PredicateKind::Range
                && predicate.owner == OWNER
                && predicate.scope == "source-cohort"
                && predicate.token == "generation" =>
            {
                Some(*observed_generation)
            }
            _ => None,
        })
        .ok_or(DurableError::Corrupt(
            "bounded Corpus source generation read absent",
        ))
}

fn creation_delta(
    input: &dyn PgCreationInput,
    readset: &SourceRevisionReadSet,
    writes: &[SourceRevisionWrite],
    indexes: &IndexRows,
    reads: &SourceReads,
) -> DurableResult<Digest256> {
    let selection = readset.selection();
    let plan = input.plan();
    let commitment = input.dependency_commitment();
    let mut hasher = Digest256Hasher::new();
    match input.record_type() {
        "agent" => part(&mut hasher, b"tos-pg-agent-creation-delta-v1"),
        "artifact" => {
            part(&mut hasher, b"tos-pg-artifact-creation-delta-v1");
            part(&mut hasher, input.family_tag().as_bytes());
        }
        _ if is_pg_family_tag(input.family_tag()) => {
            part(&mut hasher, b"tos-pg-family-creation-delta-v2");
            part(&mut hasher, input.family_tag().as_bytes());
            part(&mut hasher, input.record_type().as_bytes());
        }
        record_type => {
            part(&mut hasher, b"tos-pg-corpus-creation-delta-v1");
            part(&mut hasher, record_type.as_bytes());
        }
    }
    if is_pg_family_tag(input.family_tag()) {
        part(
            &mut hasher,
            input
                .replay_binding()
                .ok_or(DurableError::Invalid(
                    "bounded PG family replay binding absent",
                ))?
                .as_bytes(),
        );
    }
    part(&mut hasher, selection.cohort.domain.as_bytes());
    part(&mut hasher, selection.contract.as_bytes());
    part(&mut hasher, &selection.epoch.to_be_bytes());
    part(&mut hasher, &selection.source_generation.to_be_bytes());
    part(&mut hasher, &selection.head_seq.to_be_bytes());
    part(
        &mut hasher,
        identity_digest(input.identity(), input.record_type(), input.family_tag()).as_bytes(),
    );
    part(&mut hasher, &commitment.version.to_be_bytes());
    part(&mut hasher, commitment.digest.as_bytes());
    part(&mut hasher, plan.handler_id.as_bytes());
    part(&mut hasher, plan.operation.as_bytes());
    part(&mut hasher, input.context().base_revision.0.as_bytes());
    part(&mut hasher, plan.request_canonical_sha256.as_bytes());
    part(&mut hasher, plan.configuration_raw_sha256.as_bytes());
    part(&mut hasher, plan.configuration_canonical_sha256.as_bytes());
    part(
        &mut hasher,
        Digest256::of_bytes(&reads_bytes(reads)?).as_bytes(),
    );
    for (kind, token, path) in indexes {
        part(&mut hasher, kind.as_bytes());
        part(&mut hasher, token.as_bytes());
        part(&mut hasher, path.as_bytes());
    }
    for write in writes {
        part(&mut hasher, write.subject.as_bytes());
        part(&mut hasher, &write.proposed_revision.to_be_bytes());
        part(
            &mut hasher,
            Digest256::of_bytes(&write.exact_bytes).as_bytes(),
        );
        part(&mut hasher, &(write.exact_bytes.len() as u64).to_be_bytes());
    }
    Ok(hasher.finalize())
}

fn creation_writes(input: &dyn PgCreationInput) -> DurableResult<Vec<SourceRevisionWrite>> {
    let plan = input.plan();
    if plan.changes.is_empty() || plan.changes.len() > MAX_MEMBERS {
        return Err(DurableError::Refused(
            "bounded PG creation requires an admitted initial source package",
        ));
    }
    let request = cmd::parse(&input.context().request_raw).map_err(source_error)?;
    let record = cmd::field(&request, "record").map_err(source_error)?;
    let config = cmd::parse(&input.context().configuration_raw).map_err(source_error)?;
    let family = crate::source_creation::CreationFamily::parse(
        cmd::text(&config, "schema_version").map_err(source_error)?,
    )
    .map_err(source_error)?;
    if family == crate::source_creation::CreationFamily::Sign {
        return Err(DurableError::Refused(
            "Sign promotion is not a source creation family",
        ));
    }
    let expected_operation = family.operation();
    if plan.operation != expected_operation {
        return Err(DurableError::Refused(
            "bounded PG creation operation differs from family",
        ));
    }
    let (expected_tag, record_filename, forms_filename, record_id) = match family {
        crate::source_creation::CreationFamily::CorpusV1
        | crate::source_creation::CreationFamily::CorpusV2 => {
            let expected_tag = if family == crate::source_creation::CreationFamily::CorpusV1 {
                crate::source_creation::pg::PG_CORPUS_V1_FAMILY_TAG
            } else {
                crate::source_creation::pg::PG_CORPUS_V2_FAMILY_TAG
            };
            if plan.handler_id != family.handler_id() {
                return Err(DurableError::Refused(
                    "bounded PG Corpus creation handler differs",
                ));
            }
            crate::source_creation::initial_corpus_with_schema(
                &config,
                family,
                record,
                |_, _, _| Ok(()),
            )
            .map_err(source_error)?;
            let record_type = cmd::text(record, "record_type").map_err(source_error)?;
            if record_type != input.record_type() {
                return Err(DurableError::Conflict(
                    "bounded PG Corpus record type differs from semantic package",
                ));
            }
            let source_path = cmd::text(&config, "source_path").map_err(source_error)?;
            if source_path != format!("{}/{}.json", input.home().as_str(), record_type) {
                return Err(DurableError::Conflict(
                    "bounded PG Corpus source path differs from selected home/type",
                ));
            }
            (
                expected_tag,
                format!("{record_type}.json"),
                format!("{record_type}.human-forms.json"),
                cmd::text(record, "record_id")
                    .map_err(source_error)?
                    .to_owned(),
            )
        }
        crate::source_creation::CreationFamily::Artifact => {
            if plan.handler_id != family.handler_id()
                || input.record_type() != crate::source_creation::artifact_pg::ARTIFACT_RECORD_TYPE
            {
                return Err(DurableError::Refused(
                    "bounded PG Artifact creation handler or type differs",
                ));
            }
            crate::source_artifact_native::configuration(&config).map_err(source_error)?;
            crate::source_artifact_native::initial_with_schema(
                input.context(),
                &config,
                record,
                |_, _, _| Ok(()),
            )
            .map_err(source_error)?;
            let source_path = cmd::text(&config, "source_path").map_err(source_error)?;
            if source_path != format!("{}/artifact-witness.json", input.home().as_str()) {
                return Err(DurableError::Conflict(
                    "bounded PG Artifact source path differs from selected home",
                ));
            }
            (
                crate::source_creation::artifact_pg::ARTIFACT_FAMILY_TAG,
                "artifact-witness.json".to_owned(),
                "artifact-witness.human-forms.json".to_owned(),
                cmd::text(record, "artifact_id")
                    .map_err(source_error)?
                    .to_owned(),
            )
        }
        crate::source_creation::CreationFamily::HistoricalV1
        | crate::source_creation::CreationFamily::HistoricalV2
        | crate::source_creation::CreationFamily::PublicProfile => {
            if plan.handler_id != family.handler_id() {
                return Err(DurableError::Refused(
                    "bounded PG family creation handler differs",
                ));
            }
            let record_type = cmd::text(record, "record_type").map_err(source_error)?;
            if input.record_type() != record_type {
                return Err(DurableError::Conflict(
                    "bounded PG family record type differs from semantic package",
                ));
            }
            let source_path = cmd::text(&config, "source_path").map_err(source_error)?;
            let home_prefix = format!("{}/", input.home().as_str());
            let record_filename = source_path
                .strip_prefix(&home_prefix)
                .filter(|name| {
                    !name.is_empty()
                        && !name.contains('/')
                        && name.ends_with(".json")
                        && RelativePath::parse(source_path).is_ok()
                })
                .ok_or(DurableError::Conflict(
                    "bounded PG family source path differs from selected home",
                ))?
                .to_owned();
            if family.historical() && record_filename != format!("{record_type}.json") {
                return Err(DurableError::Conflict(
                    "bounded PG Historical source basename differs from record type",
                ));
            }
            let tag = match family {
                crate::source_creation::CreationFamily::HistoricalV1 => {
                    crate::source_creation::pg_family::PG_HISTORICAL_V1_FAMILY_TAG
                }
                crate::source_creation::CreationFamily::HistoricalV2 => {
                    crate::source_creation::pg_family::PG_HISTORICAL_V2_FAMILY_TAG
                }
                crate::source_creation::CreationFamily::PublicProfile => {
                    crate::source_creation::pg_family::PG_PUBLIC_PROFILE_FAMILY_TAG
                }
                _ => {
                    return Err(DurableError::Refused(
                        "bounded PG creation family tag is not admitted",
                    ));
                }
            };
            (
                tag,
                record_filename.clone(),
                format!(
                    "{}.human-forms.json",
                    record_filename.trim_end_matches(".json")
                ),
                cmd::text(record, "record_id")
                    .map_err(source_error)?
                    .to_owned(),
            )
        }
        _ => {
            return Err(DurableError::Refused(
                "bounded PG creation family is not admitted",
            ));
        }
    };
    if input.family_tag() != expected_tag {
        return Err(DurableError::Conflict(
            "bounded PG creation family tag differs from owner",
        ));
    }
    if family == crate::source_creation::CreationFamily::Artifact {
        let expected_members = crate::source_creation::artifact_pg::INITIAL_PACKAGE_FILES
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let actual_members = input.files().keys().cloned().collect::<BTreeSet<_>>();
        if actual_members != expected_members {
            return Err(DurableError::Conflict(
                "bounded PG Artifact package member closure differs",
            ));
        }
    }
    if is_pg_family_tag(expected_tag) {
        let expected_members = match family {
            crate::source_creation::CreationFamily::HistoricalV1 => [
                record_filename.as_str(),
                forms_filename.as_str(),
                "historical-claims.jsonl",
                "source-create-receipt.json",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
            crate::source_creation::CreationFamily::HistoricalV2 => [
                record_filename.as_str(),
                forms_filename.as_str(),
                "historical-claims.jsonl",
                "source-create-request.json",
                "source-create-environment.json",
                "source-create-provenance.jsonl",
                "source-create-receipt.json",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
            crate::source_creation::CreationFamily::PublicProfile => [
                record_filename.as_str(),
                forms_filename.as_str(),
                "source-create-request.json",
                "source-create-environment.json",
                "source-create-provenance.jsonl",
                "source-create-receipt.json",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
            _ => return Err(DurableError::Corrupt("bounded PG family creation tag")),
        };
        if input.files().keys().cloned().collect::<BTreeSet<_>>() != expected_members {
            return Err(DurableError::Conflict(
                "bounded PG family package member closure differs",
            ));
        }
    }
    let serialized = input
        .files()
        .get(&record_filename)
        .ok_or(DurableError::Corrupt("bounded PG creation body absent"))?;
    if !cmd::same(record, &cmd::parse(serialized).map_err(source_error)?).map_err(source_error)? {
        return Err(DurableError::Conflict(
            "bounded PG creation body differs from semantic record",
        ));
    }
    let mut expected_indexes = IndexRows::new();
    expected_indexes.insert((
        "metadata".to_owned(),
        record_id,
        format!("{}/{}", input.home().as_str(), record_filename),
    ));
    if family != crate::source_creation::CreationFamily::HistoricalV1 {
        expected_indexes.insert((
            "event".to_owned(),
            cmd::text(&config, "provenance_event_id")
                .map_err(source_error)?
                .to_owned(),
            format!("{}/source-create-provenance.jsonl", input.home().as_str()),
        ));
    }
    for selection in cmd::array(&request, "forms").map_err(source_error)? {
        expected_indexes.insert((
            "form".to_owned(),
            cmd::text(selection, "form_id")
                .map_err(source_error)?
                .to_owned(),
            format!("{}/{}", input.home().as_str(), forms_filename),
        ));
    }
    if matches!(
        family,
        crate::source_creation::CreationFamily::HistoricalV1
            | crate::source_creation::CreationFamily::HistoricalV2
    ) {
        for claim in cmd::array(&request, "claims").map_err(source_error)? {
            expected_indexes.insert((
                "claim".to_owned(),
                cmd::text(claim, "claim_id")
                    .map_err(source_error)?
                    .to_owned(),
                format!("{}/historical-claims.jsonl", input.home().as_str()),
            ));
        }
    }
    if input.indexes() != &expected_indexes {
        return Err(DurableError::Conflict(
            "bounded PG creation identity indexes differ from owner package",
        ));
    }
    let prefix = format!("{}/", input.home().as_str());
    let mut changes = plan.changes.clone();
    changes.sort_by(|left, right| left.path.cmp(&right.path));
    if changes.windows(2).any(|pair| pair[0].path == pair[1].path)
        || changes.len() != input.files().len()
    {
        return Err(DurableError::Invalid(
            "bounded PG creation package membership differs",
        ));
    }
    let mut writes = Vec::with_capacity(changes.len());
    for change in changes {
        let subject = change.path.as_str().to_owned();
        let name = subject.strip_prefix(&prefix).ok_or(DurableError::Refused(
            "bounded PG creation member escapes selected home",
        ))?;
        if name.is_empty() {
            return Err(DurableError::Refused(
                "bounded PG creation package contains its home directory",
            ));
        }
        let after = change
            .after
            .ok_or(DurableError::Refused("bounded PG creation deletion"))?;
        if change.before.is_some() || input.files().get(name) != Some(&after) {
            return Err(DurableError::Conflict(
                "bounded PG creation member bytes differ from semantic package",
            ));
        }
        writes.push(SourceRevisionWrite {
            subject,
            expected_predecessor: None,
            proposed_revision: 1,
            exact_bytes: after,
        });
    }
    if is_pg_family_tag(expected_tag) {
        let owner_indexes = crate::source_creation::pg_family::recovery_package_contract(
            expected_tag,
            input.record_type(),
            input.home().as_str(),
            &writes,
        )
        .map_err(source_error)?;
        if owner_indexes != expected_indexes || input.indexes() != &owner_indexes {
            return Err(DurableError::Conflict(
                "bounded PG family owner indexes differ from exact package",
            ));
        }
    }
    Ok(writes)
}

fn verify_recovered_corpus_record(
    indexes: &IndexRows,
    record_type: &str,
    writes: &[SourceRevisionWrite],
) -> DurableResult<()> {
    let metadata = indexes
        .iter()
        .filter(|(kind, _, _)| kind == "metadata")
        .collect::<Vec<_>>();
    let events = indexes
        .iter()
        .filter(|(kind, _, _)| kind == "event")
        .collect::<Vec<_>>();
    let forms = indexes
        .iter()
        .filter(|(kind, _, _)| kind == "form")
        .collect::<Vec<_>>();
    if metadata.len() != 1 || events.len() != 1 || forms.is_empty() {
        return Err(DurableError::Corrupt(
            "bounded Corpus recovery identity indexes differ",
        ));
    }
    let (metadata_kind, record_id, source_path) = metadata[0];
    let expected_name = format!("/{record_type}.json");
    let home = source_path
        .strip_suffix(&expected_name)
        .ok_or(DurableError::Corrupt(
            "bounded Corpus recovery source basename differs",
        ))?;
    if metadata_kind != "metadata"
        || !source_path.starts_with("ToS/source-witnesses/")
        || RelativePath::parse(source_path).is_err()
        || !crate::source_revisions::valid_id(record_id, &format!("tos.{record_type}."), false)
        || record_type == "collection"
            && !source_path.starts_with("ToS/source-witnesses/collections/")
        || record_type == "work"
            && source_path.starts_with("ToS/source-witnesses/works/friedrich-nietzsche/")
    {
        return Err(DurableError::Corrupt(
            "bounded Corpus recovery home profile differs",
        ));
    }
    let event_path = format!("{home}/source-create-provenance.jsonl");
    let forms_path = format!("{home}/{record_type}.human-forms.json");
    if events.iter().any(|(_, _, path)| path != &event_path)
        || forms.iter().any(|(_, _, path)| path != &forms_path)
    {
        return Err(DurableError::Corrupt(
            "bounded Corpus recovery event/form homes differ",
        ));
    }
    let source = writes
        .iter()
        .find(|write| write.subject == *source_path)
        .ok_or(DurableError::Corrupt(
            "bounded Corpus recovery record member absent",
        ))?;
    let record: serde_json::Value = serde_json::from_slice(&source.exact_bytes)
        .map_err(|_| DurableError::Corrupt("bounded Corpus recovery record encoding"))?;
    if record.get("record_type").and_then(|value| value.as_str()) != Some(record_type)
        || record.get("record_id").and_then(|value| value.as_str()) != Some(record_id)
    {
        return Err(DurableError::Corrupt(
            "bounded Corpus recovery record identity differs",
        ));
    }
    Ok(())
}

fn verify_recovered_initial_record(
    indexes: &IndexRows,
    record_type: &str,
    family_tag: Option<&str>,
    writes: &[SourceRevisionWrite],
) -> DurableResult<()> {
    if let Some(family_tag) = family_tag.filter(|tag| is_pg_family_tag(tag)) {
        let owner_indexes = indexes
            .iter()
            .filter(|(kind, _, _)| kind != "path")
            .cloned()
            .collect::<IndexRows>();
        let metadata = owner_indexes
            .iter()
            .filter(|(kind, _, _)| kind == "metadata")
            .collect::<Vec<_>>();
        if metadata.len() != 1 {
            return Err(DurableError::Corrupt(
                "bounded PG family recovery metadata identity differs",
            ));
        }
        let source_path = metadata[0].2.as_str();
        let (home, record_filename) = source_path.rsplit_once('/').ok_or(DurableError::Corrupt(
            "bounded PG family recovery metadata path differs",
        ))?;
        if !source_path.starts_with("ToS/")
            || record_filename.is_empty()
            || !record_filename.ends_with(".json")
            || RelativePath::parse(source_path).is_err()
            || RelativePath::parse(home).is_err()
        {
            return Err(DurableError::Corrupt(
                "bounded PG family recovery metadata path differs",
            ));
        }
        let expected_owner_indexes = crate::source_creation::pg_family::recovery_package_contract(
            family_tag,
            record_type,
            home,
            writes,
        )
        .map_err(source_error)?;
        let expected_paths = writes
            .iter()
            .map(|write| {
                (
                    "path".to_owned(),
                    write.subject.clone(),
                    write.subject.clone(),
                )
            })
            .collect::<IndexRows>();
        let actual_paths = indexes
            .iter()
            .filter(|(kind, _, _)| kind == "path")
            .cloned()
            .collect::<IndexRows>();
        if owner_indexes != expected_owner_indexes || actual_paths != expected_paths {
            return Err(DurableError::Corrupt(
                "bounded PG family recovery package/index contract differs",
            ));
        }
        return Ok(());
    }
    if record_type != crate::source_creation::artifact_pg::ARTIFACT_RECORD_TYPE {
        if family_tag.is_some() {
            return Err(DurableError::Corrupt(
                "bounded Corpus recovery unexpectedly has a family tag",
            ));
        }
        return verify_recovered_corpus_record(indexes, record_type, writes);
    }
    if family_tag != Some(crate::source_creation::artifact_pg::ARTIFACT_FAMILY_TAG) {
        return Err(DurableError::Corrupt(
            "bounded Artifact recovery family tag differs",
        ));
    }
    let metadata = indexes
        .iter()
        .filter(|(kind, _, _)| kind == "metadata")
        .collect::<Vec<_>>();
    let events = indexes
        .iter()
        .filter(|(kind, _, _)| kind == "event")
        .collect::<Vec<_>>();
    let forms = indexes
        .iter()
        .filter(|(kind, _, _)| kind == "form")
        .collect::<Vec<_>>();
    if metadata.len() != 1 || events.len() != 1 || forms.is_empty() {
        return Err(DurableError::Corrupt(
            "bounded Artifact recovery identity indexes differ",
        ));
    }
    let metadata_index = metadata[0];
    let event_index = events[0];
    let record_id = metadata_index.1.as_str();
    let source_path = metadata_index.2.as_str();
    let source = writes
        .iter()
        .find(|write| write.subject == source_path)
        .ok_or(DurableError::Corrupt(
            "bounded Artifact recovery record member absent",
        ))?;
    if metadata_index.0 != "metadata"
        || !crate::source_creation::artifact_pg::recovery_source_identity_matches(
            record_type,
            record_id,
            source_path,
            &source.exact_bytes,
        )
    {
        return Err(DurableError::Corrupt(
            "bounded Artifact recovery source identity differs",
        ));
    }
    let home = source_path
        .strip_suffix("/artifact-witness.json")
        .ok_or(DurableError::Corrupt(
            "bounded Artifact recovery source basename differs",
        ))?;
    let expected_members = crate::source_creation::artifact_pg::INITIAL_PACKAGE_FILES
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let package_prefix = format!("{home}/");
    let recovered_members = writes
        .iter()
        .map(|write| {
            write
                .subject
                .strip_prefix(&package_prefix)
                .map(str::to_owned)
                .ok_or(DurableError::Corrupt(
                    "bounded Artifact recovery member escapes home",
                ))
        })
        .collect::<DurableResult<BTreeSet<_>>>()?;
    if recovered_members.len() != writes.len() || recovered_members != expected_members {
        return Err(DurableError::Corrupt(
            "bounded Artifact recovery package member closure differs",
        ));
    }
    let event_path = format!("{home}/source-create-provenance.jsonl");
    let forms_path = format!("{home}/artifact-witness.human-forms.json");
    if event_index.0 != "event"
        || event_index.2 != event_path
        || forms
            .iter()
            .any(|(kind, _, path)| kind != "form" || path != &forms_path)
    {
        return Err(DurableError::Corrupt(
            "bounded Artifact recovery event/form paths differ",
        ));
    }
    let record: serde_json::Value = serde_json::from_slice(&source.exact_bytes)
        .map_err(|_| DurableError::Corrupt("bounded Artifact recovery record encoding"))?;
    if record
        .get("provenance_event_ref")
        .and_then(|value| value.as_str())
        != Some(event_index.1.as_str())
    {
        return Err(DurableError::Corrupt(
            "bounded Artifact recovery event identity differs",
        ));
    }
    let event_write = writes
        .iter()
        .find(|write| write.subject == event_path)
        .ok_or(DurableError::Corrupt(
            "bounded Artifact recovery provenance member absent",
        ))?;
    let event: serde_json::Value = serde_json::from_slice(&event_write.exact_bytes)
        .map_err(|_| DurableError::Corrupt("bounded Artifact recovery provenance encoding"))?;
    if event.get("event_id").and_then(|value| value.as_str()) != Some(event_index.1.as_str()) {
        return Err(DurableError::Corrupt(
            "bounded Artifact recovery provenance identity differs",
        ));
    }
    let forms_write = writes
        .iter()
        .find(|write| write.subject == forms_path)
        .ok_or(DurableError::Corrupt(
            "bounded Artifact recovery forms member absent",
        ))?;
    let form_set: serde_json::Value = serde_json::from_slice(&forms_write.exact_bytes)
        .map_err(|_| DurableError::Corrupt("bounded Artifact recovery forms encoding"))?;
    let form_ids = form_set
        .get("forms")
        .and_then(|value| value.as_array())
        .ok_or(DurableError::Corrupt(
            "bounded Artifact recovery forms rows",
        ))?
        .iter()
        .map(|form| {
            form.get("form_id")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
                .ok_or(DurableError::Corrupt(
                    "bounded Artifact recovery form identity",
                ))
        })
        .collect::<DurableResult<BTreeSet<_>>>()?;
    let indexed_form_ids = forms
        .iter()
        .map(|row| row.1.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    if form_ids != indexed_form_ids {
        return Err(DurableError::Corrupt(
            "bounded Artifact recovery form indexes differ",
        ));
    }
    Ok(())
}

fn ensure_predicate(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    kind: &str,
    scope: &str,
    token: &str,
    definition: Digest256,
    expected: Option<u64>,
) -> DurableResult<u64> {
    tx.execute(
        "INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version,generation,complete)
         VALUES($1,$2,$3,$4,$5,$6,0,true) ON CONFLICT DO NOTHING",
        &[&domain, &kind, &OWNER, &scope, &token, &definition.to_hex()],
    )?;
    let row = tx.query_one(
        "SELECT generation,complete,definition_version FROM cmd2_predicate
         WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5 FOR UPDATE",
        &[&domain, &kind, &OWNER, &scope, &token],
    )?;
    if !row.get::<_, bool>(1) || row.get::<_, String>(2) != definition.to_hex() {
        return Err(DurableError::Refused(
            "bounded PG creation predicate definition incomplete",
        ));
    }
    let generation = as_u64(row.get(0))?;
    if expected.is_some_and(|expected| expected != generation) {
        return Err(DurableError::Conflict(
            "bounded PG creation predicate generation changed",
        ));
    }
    Ok(generation)
}

fn check_home_absent(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    home: &str,
) -> DurableResult<()> {
    let lower = format!("{home}/");
    let upper = format!("{home}0");
    if tx
        .query_opt(
            "SELECT subject FROM cmd2_current
             WHERE domain=$1 AND subject=$2
             LIMIT 1",
            &[&domain, &home],
        )?
        .is_some()
    {
        return Err(DurableError::Conflict(
            "bounded PG Corpus source home is occupied",
        ));
    }
    if tx
        .query_opt(
            "SELECT subject FROM cmd2_current
             WHERE domain=$1
               AND subject COLLATE \"C\" >= $2 COLLATE \"C\"
               AND subject COLLATE \"C\" < $3 COLLATE \"C\"
             ORDER BY subject COLLATE \"C\" LIMIT 1",
            &[&domain, &lower, &upper],
        )?
        .is_some()
    {
        return Err(DurableError::Conflict(
            "bounded PG Corpus source home is occupied",
        ));
    }
    Ok(())
}

/// Derive the maintained structural index and ancestor predicates from the
/// actual package writes.  These are transport-owned facts: the semantic
/// producer supplies only its identity indexes, while every package member
/// still receives the same `path` row and source-home invalidation coverage as
/// the complete creation route.
pub(super) fn add_derived_path_predicates(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    writes: &[SourceRevisionWrite],
    definition: Digest256,
) -> DurableResult<()> {
    let mut predicates = BTreeSet::new();
    for write in writes {
        predicates.insert((
            "unique".to_owned(),
            "path".to_owned(),
            write.subject.clone(),
        ));
        let mut rest = write.subject.as_str();
        while let Some((parent, _)) = rest.rsplit_once('/') {
            predicates.insert((
                "range".to_owned(),
                "source-home".to_owned(),
                parent.to_owned(),
            ));
            rest = parent;
        }
    }
    for (kind, scope, token) in predicates {
        // These rows describe the keys that this package will invalidate at
        // commit. They are write-derived coverage, not read predicates: adding
        // shared ancestors to the caller's readset would make two independent
        // homes conflict merely because both were created below the same ToS
        // prefix. Actual admission predicates come from the bounded transport
        // observations and exact source reads.
        ensure_predicate(tx, domain, &kind, &scope, &token, definition, None)?;
    }
    Ok(())
}

fn valid_predicate_shape(predicate: &PredicateToken) -> bool {
    if predicate.owner != OWNER || predicate.definition_version != definition().to_hex() {
        return false;
    }
    let kind = match predicate.kind {
        PredicateKind::Range => "range",
        PredicateKind::Unique => "unique",
    };
    super::valid_owned_predicate_shape(&predicate.owner, kind, &predicate.scope, &predicate.token)
}

fn check_predicate_reads(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    reads: &SourceReads,
    definition: Digest256,
) -> DurableResult<BTreeMap<(String, String, String), u64>> {
    let mut observed = BTreeMap::new();
    for read in &reads.reads {
        if let PredicateRead::Generation {
            predicate,
            observed_generation,
        } = read
        {
            if predicate.kind == PredicateKind::Range
                && predicate.scope == "source-cohort"
                && predicate.token == "generation"
            {
                continue;
            }
            if !valid_predicate_shape(predicate) {
                return Err(DurableError::Conflict(
                    "bounded PG creation predicate shape differs",
                ));
            }
            let key = (
                predicate.kind.as_str().to_owned(),
                predicate.scope.clone(),
                predicate.token.clone(),
            );
            ensure_predicate(
                tx,
                domain,
                &key.0,
                &key.1,
                &key.2,
                definition,
                Some(*observed_generation),
            )?;
            observed.insert(key, *observed_generation);
        }
    }
    Ok(observed)
}

impl DurablePgCoordinator {
    /// Register one sealed initial source package against the selected bounded
    /// PG read epoch. All source/index absence checks are point or C-range seeks.
    pub(crate) fn register_pg_creation(
        &mut self,
        store: &SegmentStore,
        input: &dyn PgCreationInput,
        readset: &SourceRevisionReadSet,
        prepare_id: &[u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<PgCreationAttempt> {
        active(deadline, cancelled)?;
        if prepare_id.is_empty() {
            return Err(DurableError::Invalid(
                "bounded PG creation prepare ID empty",
            ));
        }
        let selection = readset.selection();
        if store.custody_domain() != selection.cohort.domain.as_bytes()
            || store.store_id() != selection.cohort.store_id
        {
            return Err(DurableError::Conflict(
                "bounded PG creation carrier domain differs",
            ));
        }
        if input.identity() != selection.identity() {
            return Err(DurableError::Conflict(
                "bounded PG creation semantic read epoch differs",
            ));
        }
        let commitment = input.dependency_commitment();
        if commitment.version != COMMITMENT_VERSION {
            return Err(DurableError::Refused(
                "bounded PG creation dependency commitment profile",
            ));
        }
        let replay_binding = input.replay_binding();
        if is_pg_family_tag(input.family_tag()) && replay_binding.is_none() {
            return Err(DurableError::Invalid(
                "bounded PG family replay binding absent",
            ));
        }
        if input.home_observation().parent != *input.home()
            || !input.home_observation().children.is_empty()
            || input.home_observation().generation
                != required_generation(
                    readset.reads(),
                    "range",
                    "source-home",
                    input.home().as_str(),
                )?
        {
            return Err(DurableError::Conflict(
                "bounded PG creation home observation differs",
            ));
        }
        let semantic_indexes = input.indexes();
        if semantic_indexes.is_empty()
            || semantic_indexes.iter().any(|(kind, token, path)| {
                !fixed_index(kind)
                    || token.is_empty()
                    || path.is_empty()
                    || !path.starts_with(&format!("{}/", input.home().as_str()))
            })
        {
            return Err(DurableError::Refused("bounded PG creation index profile"));
        }
        let unique_observations = input.unique_observations();
        let mut index_identities = BTreeSet::new();
        if semantic_indexes
            .iter()
            .any(|(kind, token, _)| !index_identities.insert((kind, token)))
        {
            return Err(DurableError::Refused(
                "bounded PG creation duplicate source identity",
            ));
        }
        let mut observed_identities = BTreeSet::new();
        if unique_observations
            .iter()
            .any(|observation| !observed_identities.insert((&observation.kind, &observation.token)))
        {
            return Err(DurableError::Conflict(
                "bounded PG creation duplicate unique observation",
            ));
        }
        if unique_observations.len() != semantic_indexes.len()
            || semantic_indexes.iter().any(|(kind, token, _)| {
                !unique_observations.iter().any(|observation| {
                    observation.kind == *kind
                        && observation.token == *token
                        && observation.rows.is_empty()
                        && observation.generation
                            == required_generation(readset.reads(), "unique", kind, token)
                                .unwrap_or(u64::MAX)
                })
            })
        {
            return Err(DurableError::Conflict(
                "bounded PG creation unique observations differ",
            ));
        }
        for (_, token, _) in semantic_indexes {
            for kind in PG_CREATION_INDEX_KINDS {
                required_generation(readset.reads(), "unique", kind, token)?;
            }
        }
        let writes = creation_writes(input)?;
        let mut indexes = semantic_indexes.clone();
        for write in &writes {
            indexes.insert((
                "path".to_owned(),
                write.subject.clone(),
                write.subject.clone(),
            ));
        }
        let write_subjects = writes
            .iter()
            .map(|write| write.subject.as_str())
            .collect::<BTreeSet<_>>();
        if indexes
            .iter()
            .any(|(_, _, path)| !write_subjects.contains(path.as_str()))
        {
            return Err(DurableError::Conflict(
                "bounded PG creation index path is not a package member",
            ));
        }
        let mut reads = readset.reads().clone();
        for write in &writes {
            if !reads.reads.iter().any(|read| {
                matches!(read, PredicateRead::Absent { namespace, key } if namespace == "source" && key == &write.subject)
            }) {
                reads.reads.push(PredicateRead::Absent {
                    namespace: "source".to_owned(),
                    key: write.subject.clone(),
                });
            }
        }
        let metadata = writes
            .iter()
            .map(|write| {
                (
                    write.subject.clone(),
                    SourceFileMetadata {
                        mode: 0o644,
                        dependencies: Some(source_dependencies(&reads)),
                    },
                )
            })
            .collect::<SourceMetadata>();
        let source_generation = source_generation(&reads)?;
        let plan = input.plan();
        let request = cmd::parse(&input.context().request_raw).map_err(source_error)?;
        let command_id = cmd::text(&request, "command_id").map_err(source_error)?;
        let raw_request_digest = Digest256::of_bytes(&input.context().request_raw);
        let domain = selection.cohort.domain.as_str();
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let _initial_audit = lock_audit_fence(&mut tx, domain)?;
        let row = tx.query_one("SELECT * FROM cmd2_domain WHERE domain=$1", &[&domain])?;
        cohort_matches(&row, &selection.cohort, true)?;
        if as_u64(row.get("rights_version"))? != selection.rights_version
            || as_u64(row.get("rule_version"))? != selection.rule_version
            || row.get::<_, String>("contract_digest") != selection.contract.to_hex()
        {
            return Err(DurableError::Conflict(
                "bounded PG creation source selection changed",
            ));
        }
        let job = tx.query_one(
            "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2",
            &[&domain, &selection.job_id],
        )?;
        if as_u64(job.get(0))? != selection.job_fence {
            return Err(DurableError::Conflict(
                "bounded PG creation job lease changed",
            ));
        }
        add_derived_path_predicates(&mut tx, domain, &writes, selection.cohort.definition)?;
        let predicate_keys = reads
            .reads
            .iter()
            .filter_map(|read| match read {
                PredicateRead::Generation { predicate, .. }
                    if predicate.scope != "source-cohort" =>
                {
                    Some((
                        predicate.kind.as_str().to_owned(),
                        predicate.scope.clone(),
                        predicate.token.clone(),
                    ))
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let delta = creation_delta(input, readset, &writes, &indexes, &reads)?;
        let record_type = input.record_type().to_owned();
        let family_tag = (record_type == "artifact" || is_pg_family_tag(input.family_tag()))
            .then(|| input.family_tag().to_owned());
        let indexes_encoded = indexes_bytes(
            &indexes,
            commitment,
            input.identity(),
            &record_type,
            input.family_tag(),
            replay_binding,
        )?;
        let reads_encoded = reads_bytes(&reads)?;
        let empty_projections = projections_bytes(&ProjectionRows::new())?;
        let existing = tx.query_opt(
            "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 FOR UPDATE",
            &[&domain, &prepare_id],
        )?;
        if let Some(existing) = existing {
            if existing.get::<_, String>("command_id") != command_id
                || existing.get::<_, String>("raw_request_digest") != raw_request_digest.to_hex()
                || existing.get::<_, String>("delta_digest") != delta.to_hex()
                || existing.get::<_, Option<String>>("source_configuration_raw_digest")
                    != Some(plan.configuration_raw_sha256.to_hex())
                || existing.get::<_, Option<String>>("source_configuration_canonical_digest")
                    != Some(plan.configuration_canonical_sha256.to_hex())
                || existing.get::<_, Option<Vec<u8>>>("source_reads") != Some(reads_encoded.clone())
                || existing.get::<_, Option<Vec<u8>>>("source_indexes")
                    != Some(indexes_encoded.clone())
                || existing.get::<_, Option<Vec<u8>>>("source_projections")
                    != Some(empty_projections.clone())
                || existing.get::<_, String>("state") == "aborted"
            {
                return Err(DurableError::Conflict(
                    "bounded PG creation registration identity differs",
                ));
            }
            let fence = as_u64(existing.get("attempt_fence"))?;
            let audit_generation = lock_audit_fence(&mut tx, domain)?;
            tx.commit()?;
            return Ok(PgCreationAttempt {
                record_type,
                family_tag,
                domain: domain.to_owned(),
                prepare_id: prepare_id.to_vec(),
                fence,
                command_id: command_id.to_owned(),
                raw_request_digest,
                configuration_raw_digest: plan.configuration_raw_sha256,
                configuration_canonical_digest: plan.configuration_canonical_sha256,
                epoch: selection.epoch,
                source_generation,
                audit_generation,
                rights_version: selection.rights_version,
                rule_version: selection.rule_version,
                job_id: selection.job_id.clone(),
                job_fence: selection.job_fence,
                contract: selection.contract,
                definition: selection.cohort.definition,
                delta,
                reads,
                metadata,
                indexes: indexes.clone(),
                index_binding: indexes_encoded.clone(),
                dependency_commitment: commitment,
                replay_binding,
                predicate_keys,
                writes,
                identity: Some(input.identity()),
            });
        }
        check_home_absent(&mut tx, domain, input.home().as_str())?;
        let exact_paths = reads.locators.keys().cloned().collect::<Vec<_>>();
        let exact_current = tx
            .query(
                "SELECT * FROM cmd2_current WHERE domain=$1 AND subject=ANY($2)",
                &[&domain, &exact_paths],
            )?
            .into_iter()
            .map(|row| (row.get::<_, String>("subject"), row))
            .collect::<BTreeMap<_, _>>();
        for read in &reads.reads {
            match read {
                PredicateRead::Exact {
                    key,
                    expected_version,
                    expected_digest,
                    ..
                } => {
                    let row = exact_current.get(key).ok_or(DurableError::Conflict(
                        "bounded PG exact source read absent",
                    ))?;
                    if Some(as_u64(row.get("revision"))?) != *expected_version
                        || Some(parse_hex(row.get("content_digest"))?) != *expected_digest
                        || reads.locators.get(key) != Some(&metadata_locator_digest(row))
                    {
                        return Err(DurableError::Conflict(
                            "bounded PG exact source read changed",
                        ));
                    }
                }
                PredicateRead::Absent { namespace, key } if namespace == "source" => {
                    if tx
                        .query_opt(
                            "SELECT 1 FROM cmd2_current WHERE domain=$1 AND subject=$2",
                            &[&domain, key],
                        )?
                        .is_some()
                    {
                        return Err(DurableError::Conflict("bounded PG source absence changed"));
                    }
                }
                PredicateRead::Generation { .. } | PredicateRead::Absent { .. } => {}
            }
        }
        let observed = check_predicate_reads(&mut tx, domain, &reads, selection.cohort.definition)?;
        let unique_kinds = PG_CREATION_INDEX_KINDS
            .iter()
            .map(|kind| (*kind).to_owned())
            .collect::<Vec<_>>();
        for (kind, token, path) in &indexes {
            let occupied = if kind == "path" {
                tx.query_opt(
                    "SELECT 1 FROM cmd2_source_index
                     WHERE domain=$1 AND kind='path' AND token=$2 LIMIT 1",
                    &[&domain, token],
                )?
            } else {
                tx.query_opt(
                    "SELECT 1 FROM cmd2_source_index
                     WHERE domain=$1 AND kind=ANY($2) AND token=$3 LIMIT 1",
                    &[&domain, &unique_kinds, token],
                )?
            };
            if occupied.is_some() {
                return Err(DurableError::Conflict(
                    "bounded PG source unique identity changed",
                ));
            }
            if !path.starts_with(&format!("{}/", input.home().as_str())) {
                return Err(DurableError::Invalid("bounded PG source index path"));
            }
        }
        let dependency_token = commitment.version.to_string();
        if !observed.contains_key(&(
            "range".to_owned(),
            DEPENDENCY_SCOPE.to_owned(),
            dependency_token.clone(),
        )) {
            return Err(DurableError::Corrupt(
                "bounded PG dependency commitment predicate read absent",
            ));
        }
        tx.execute(
            "INSERT INTO cmd2_attempt(domain,prepare_id,command_id,raw_request_digest,delta_digest,
             state,attempt_fence,source_reads,source_indexes,source_epoch,source_projections,
             source_configuration_raw_digest,source_configuration_canonical_digest)
             VALUES($1,$2,$3,$4,$5,'registered',1,$6,$7,$8,$9,$10,$11)",
            &[
                &domain,
                &prepare_id,
                &command_id,
                &raw_request_digest.to_hex(),
                &delta.to_hex(),
                &reads_encoded,
                &indexes_encoded,
                &as_i64(selection.epoch)?,
                &empty_projections,
                &plan.configuration_raw_sha256.to_hex(),
                &plan.configuration_canonical_sha256.to_hex(),
            ],
        )?;
        let audit_generation = lock_audit_fence(&mut tx, domain)?;
        tx.commit()?;
        Ok(PgCreationAttempt {
            record_type,
            family_tag,
            domain: domain.to_owned(),
            prepare_id: prepare_id.to_vec(),
            fence: 1,
            command_id: command_id.to_owned(),
            raw_request_digest,
            configuration_raw_digest: plan.configuration_raw_sha256,
            configuration_canonical_digest: plan.configuration_canonical_sha256,
            epoch: selection.epoch,
            source_generation,
            audit_generation,
            rights_version: selection.rights_version,
            rule_version: selection.rule_version,
            job_id: selection.job_id.clone(),
            job_fence: selection.job_fence,
            contract: selection.contract,
            definition: selection.cohort.definition,
            delta,
            reads,
            metadata,
            indexes: indexes.clone(),
            index_binding: indexes_encoded.clone(),
            dependency_commitment: commitment,
            replay_binding,
            predicate_keys,
            writes,
            identity: Some(input.identity()),
        })
    }

    /// Reconstruct a ready or committed bounded creation from PG/STO
    /// carriers. The current source graph is not used to reconstruct the
    /// semantic proposal; current rights, schema, lease and owner checks still
    /// run at the final commit fence.
    pub(crate) fn recover_pg_creation_attempt(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        prepare_id: &[u8],
        expected_command_id: &str,
        expected_raw_request_digest: Digest256,
        expected_record_type: &str,
        expected_configuration_raw_digest: Digest256,
        expected_configuration_canonical_digest: Digest256,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<PgCreationAttempt> {
        active(deadline, cancelled)?;
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict(
                "bounded PG creation recovery custody domain differs",
            ));
        }
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT maintenance_state,generation FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        let domain_row = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        if fence.get::<_, String>(0) != "normal"
            || !domain_row.get::<_, bool>("rights_allowed")
            || !domain_row.get::<_, bool>("source_complete")
            || domain_row.get::<_, Option<String>>("schema_profile_digest")
                != Some(schema_profile_digest().to_hex())
        {
            return Err(DurableError::Refused(
                "bounded PG creation recovery source is not readable",
            ));
        }
        let job = tx
            .query_opt(
                "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2",
                &[&domain, &job_id],
            )?
            .ok_or(DurableError::Refused(
                "bounded PG creation recovery job lease absent",
            ))?;
        if as_u64(job.get(0))? != job_fence {
            return Err(DurableError::Conflict(
                "bounded PG creation recovery job lease changed",
            ));
        }
        let attempt = tx
            .query_opt(
                "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 FOR SHARE",
                &[&domain, &prepare_id],
            )?
            .ok_or(DurableError::Refused("bounded PG creation attempt absent"))?;
        let state: String = attempt.get("state");
        if state == "registered" {
            return Err(DurableError::Indeterminate(
                "bounded PG creation was registered but not durably sealed",
            ));
        }
        if state != "ready" && state != "committed" {
            return Err(DurableError::Refused(
                "bounded PG creation attempt is not replayable",
            ));
        }
        let raw_digest = parse_hex(attempt.get::<_, String>("raw_request_digest"))?;
        let config_raw = parse_hex(
            attempt
                .get::<_, Option<String>>("source_configuration_raw_digest")
                .ok_or(DurableError::Corrupt(
                    "bounded PG creation config binding absent",
                ))?,
        )?;
        let config_canonical = parse_hex(
            attempt
                .get::<_, Option<String>>("source_configuration_canonical_digest")
                .ok_or(DurableError::Corrupt(
                    "bounded PG creation config binding absent",
                ))?,
        )?;
        if attempt.get::<_, String>("command_id") != expected_command_id
            || raw_digest != expected_raw_request_digest
            || config_raw != expected_configuration_raw_digest
            || config_canonical != expected_configuration_canonical_digest
        {
            return Err(DurableError::Conflict(
                "bounded PG creation recovery request/config binding differs",
            ));
        }
        let definition = parse_hex(
            domain_row
                .get::<_, Option<String>>("source_definition_digest")
                .ok_or(DurableError::Corrupt(
                    "bounded PG creation definition absent",
                ))?,
        )?;
        super::require_current_source_definition(definition)?;
        let reads = decode_reads(
            &attempt
                .get::<_, Option<Vec<u8>>>("source_reads")
                .ok_or(DurableError::Corrupt("bounded PG creation reads absent"))?,
        )?;
        let (indexes, dependency_commitment, _read_epoch, record_type, family_tag, replay_binding) =
            parse_pg_indexes(
                &attempt
                    .get::<_, Option<Vec<u8>>>("source_indexes")
                    .ok_or(DurableError::Corrupt("bounded PG creation indexes absent"))?,
            )?;
        if record_type != expected_record_type {
            return Err(DurableError::Conflict(
                "bounded PG Corpus recovery record type differs",
            ));
        }
        if dependency_commitment.version != COMMITMENT_VERSION {
            return Err(DurableError::Corrupt(
                "bounded PG creation dependency commitment profile",
            ));
        }
        let members = tx.query(
            "SELECT * FROM cmd2_member WHERE domain=$1 AND prepare_id=$2
             ORDER BY member_slot LIMIT $3",
            &[&domain, &prepare_id, &as_i64((MAX_MEMBERS + 1) as u64)?],
        )?;
        if members.is_empty() || members.len() > MAX_MEMBERS {
            return Err(DurableError::Corrupt(
                "bounded PG creation recovery member count",
            ));
        }
        let persisted = store
            .recover_attempt_fenced(prepare_id, as_u64(attempt.get("attempt_fence"))?, 0)?
            .ok_or(DurableError::Indeterminate(
                "bounded PG creation recovery STO intent absent",
            ))?;
        let receipts = match persisted {
            AttemptRecovery::Sealed { receipts } => receipts,
            _ => {
                return Err(DurableError::Indeterminate(
                    "bounded PG creation recovery STO intent uncertain",
                ));
            }
        };
        if receipts.len() != members.len() {
            return Err(DurableError::Corrupt(
                "bounded PG creation recovery receipt count",
            ));
        }
        let _guard = store.verify_and_hold_fenced(
            prepare_id,
            as_u64(attempt.get("attempt_fence"))?,
            0,
            &receipts,
            verification_budget(receipts.len()),
        )?;
        let mut writes = Vec::with_capacity(members.len());
        let mut metadata = SourceMetadata::new();
        for row in &members {
            if row.get::<_, String>("profile_id").as_bytes() != PG_CORPUS_CREATION_PROFILE {
                return Err(DurableError::Corrupt(
                    "bounded PG creation recovery profile differs",
                ));
            }
            let slot = row.get::<_, i32>("member_slot") as u32;
            let receipt = receipts
                .iter()
                .find(|receipt| receipt.binding().member_slot == slot)
                .ok_or(DurableError::Corrupt(
                    "bounded PG creation recovery receipt slot absent",
                ))?;
            let mut exact_bytes = Vec::new();
            store.read_selected(receipt, 8_388_608, &mut exact_bytes)?;
            if Digest256::of_bytes(&exact_bytes)
                != parse_hex(row.get::<_, String>("content_digest"))?
            {
                return Err(DurableError::Corrupt(
                    "bounded PG creation recovery STO bytes differ",
                ));
            }
            let subject: String = row.get("subject");
            metadata.insert(subject.clone(), row_source_metadata(row)?);
            writes.push(SourceRevisionWrite {
                subject,
                expected_predecessor: None,
                proposed_revision: 1,
                exact_bytes,
            });
        }
        let source_generation = source_generation(&reads)?;
        verify_recovered_initial_record(&indexes, &record_type, family_tag.as_deref(), &writes)?;
        let predicate_keys = reads
            .reads
            .iter()
            .filter_map(|read| match read {
                PredicateRead::Generation { predicate, .. }
                    if predicate.scope != "source-cohort" =>
                {
                    Some((
                        predicate.kind.as_str().to_owned(),
                        predicate.scope.clone(),
                        predicate.token.clone(),
                    ))
                }
                _ => None,
            })
            .collect();
        let audit_generation = as_u64(fence.get(1))?;
        let contract = parse_hex(domain_row.get::<_, String>("contract_digest"))?;
        let rights_version = as_u64(domain_row.get("rights_version"))?;
        let rule_version = as_u64(domain_row.get("rule_version"))?;
        let epoch = as_u64(
            attempt
                .get::<_, Option<i64>>("source_epoch")
                .ok_or(DurableError::Corrupt("bounded PG creation epoch absent"))?,
        )?;
        let delta = parse_hex(attempt.get::<_, String>("delta_digest"))?;
        let fence = as_u64(attempt.get("attempt_fence"))?;
        let command_id = attempt.get::<_, String>("command_id");
        tx.commit()?;
        Ok(PgCreationAttempt {
            record_type,
            family_tag,
            domain: domain.to_owned(),
            prepare_id: prepare_id.to_vec(),
            fence,
            command_id,
            raw_request_digest: raw_digest,
            configuration_raw_digest: config_raw,
            configuration_canonical_digest: config_canonical,
            epoch,
            source_generation,
            audit_generation,
            rights_version,
            rule_version,
            job_id: job_id.to_owned(),
            job_fence,
            contract,
            definition,
            delta,
            reads,
            metadata,
            indexes,
            index_binding: attempt
                .get::<_, Option<Vec<u8>>>("source_indexes")
                .ok_or(DurableError::Corrupt("bounded PG creation indexes absent"))?,
            dependency_commitment,
            replay_binding,
            predicate_keys,
            writes,
            identity: None,
        })
    }

    pub(crate) fn commit_pg_creation(
        &mut self,
        store: &SegmentStore,
        attempt: &PgCreationAttempt,
        contract: Digest256,
        owner_check: &dyn Fn() -> DurableResult<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<(DurableCommitReceipt, DurableTiming)> {
        active(deadline, cancelled)?;
        super::require_current_source_definition(attempt.definition)?;
        if attempt.definition != definition()
            || attempt.contract != contract
            || store.custody_domain() != attempt.domain.as_bytes()
        {
            return Err(DurableError::Conflict(
                "registered bounded PG creation package changed",
            ));
        }
        let identities = attempt
            .writes
            .iter()
            .enumerate()
            .map(|(slot, write)| ShadowWriteIdentity {
                member_slot: slot as u32,
                subject: &write.subject,
                expected_predecessor: None,
                proposed_revision: 1,
                exact_bytes: &write.exact_bytes,
            })
            .collect::<Vec<_>>();
        if identities.is_empty() || identities.len() > MAX_MEMBERS {
            return Err(DurableError::Invalid("bounded PG creation member count"));
        }
        let state = self.resolve_attempt(&attempt.domain, &attempt.prepare_id)?;
        let receipts: Vec<ByteDurabilityReceipt> = match state {
            AttemptResolution::Registered => {
                let members =
                    match store.recover_attempt_fenced(&attempt.prepare_id, attempt.fence, 0)? {
                        None => seal_members(
                            store,
                            &attempt.prepare_id,
                            attempt.fence,
                            PG_CORPUS_CREATION_PROFILE,
                            &identities,
                        )?,
                        Some(AttemptRecovery::Sealed { receipts }) => {
                            members_from_receipts(&identities, receipts)?
                        }
                        _ => {
                            return Err(DurableError::Indeterminate(
                                "bounded PG creation seal retains uncertain custody",
                            ));
                        }
                    };
                self.attach_ready_profile_bound(
                    store,
                    &attempt.domain,
                    &attempt.prepare_id,
                    attempt.fence,
                    &members,
                    PG_CORPUS_CREATION_PROFILE,
                    Some(attempt.delta),
                    Some(&attempt.metadata),
                    None,
                )?;
                members.into_iter().map(|member| member.receipt).collect()
            }
            AttemptResolution::Ready | AttemptResolution::Committed(_) => {
                match store.recover_attempt_fenced(&attempt.prepare_id, attempt.fence, 0)? {
                    Some(AttemptRecovery::Sealed { receipts }) => {
                        let members = members_from_receipts(&identities, receipts)?;
                        for member in &members {
                            check_durable_member(
                                &attempt.domain,
                                &attempt.prepare_id,
                                member,
                                PG_CORPUS_CREATION_PROFILE,
                            )?;
                        }
                        members.into_iter().map(|member| member.receipt).collect()
                    }
                    _ => {
                        return Err(DurableError::Corrupt(
                            "bounded PG creation exact STO intent absent",
                        ));
                    }
                }
            }
            AttemptResolution::Aborted => {
                return Err(DurableError::Refused(
                    "bounded PG creation attempt is aborted",
                ));
            }
        };
        self.commit_durable(
            store,
            &CommitShadowAttempt {
                domain: &attempt.domain,
                prepare_id: &attempt.prepare_id,
                attempt_fence: attempt.fence,
                receipts: &receipts,
                expected_contract_digest: contract,
                expected_rule_version: attempt.rule_version,
                expected_rights_version: attempt.rights_version,
                job_id: &attempt.job_id,
                job_fence: attempt.job_fence,
                full_base_seq: 0,
            },
            CommitMode::PgCreation {
                attempt,
                owner_check,
                deadline,
                cancelled,
            },
        )
    }

    /// Compatibility entry for the existing native Corpus creation caller.
    pub(crate) fn register_pg_corpus_creation(
        &mut self,
        store: &SegmentStore,
        input: &dyn PgCorpusCreationInput,
        readset: &SourceRevisionReadSet,
        prepare_id: &[u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<PgCorpusCreationAttempt> {
        self.register_pg_creation(store, input, readset, prepare_id, deadline, cancelled)
    }

    /// Compatibility entry for the existing native Corpus creation caller.
    pub(crate) fn recover_pg_corpus_creation_attempt(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        prepare_id: &[u8],
        expected_command_id: &str,
        expected_raw_request_digest: Digest256,
        expected_record_type: &str,
        expected_configuration_raw_digest: Digest256,
        expected_configuration_canonical_digest: Digest256,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<PgCorpusCreationAttempt> {
        self.recover_pg_creation_attempt(
            store,
            domain,
            prepare_id,
            expected_command_id,
            expected_raw_request_digest,
            expected_record_type,
            expected_configuration_raw_digest,
            expected_configuration_canonical_digest,
            job_id,
            job_fence,
            deadline,
            cancelled,
        )
    }

    /// Compatibility entry for the existing native Corpus creation caller.
    pub(crate) fn commit_pg_corpus_creation(
        &mut self,
        store: &SegmentStore,
        attempt: &PgCorpusCreationAttempt,
        contract: Digest256,
        owner_check: &dyn Fn() -> DurableResult<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<(DurableCommitReceipt, DurableTiming)> {
        self.commit_pg_creation(store, attempt, contract, owner_check, deadline, cancelled)
    }
}

pub(super) fn check_commit_owner(
    tx: &mut postgres::Transaction<'_>,
    request: &CommitShadowAttempt<'_>,
    registered: &postgres::Row,
    members: &[postgres::Row],
    attempt: &PgCreationAttempt,
    owner_check: &dyn Fn() -> DurableResult<()>,
    replayed: bool,
) -> DurableResult<()> {
    owner_check()?;
    let family_tag_matches = match (attempt.record_type.as_str(), attempt.family_tag.as_deref()) {
        ("artifact", Some(tag)) => tag == crate::source_creation::artifact_pg::ARTIFACT_FAMILY_TAG,
        ("artifact", None) => false,
        (_, None) => true,
        (_, Some(tag)) => is_pg_family_tag(tag),
    };
    if attempt.family_tag.as_deref().is_some_and(is_pg_family_tag) {
        verify_recovered_initial_record(
            &attempt.indexes,
            &attempt.record_type,
            attempt.family_tag.as_deref(),
            &attempt.writes,
        )?;
    }
    let (
        bound_indexes,
        bound_commitment,
        bound_read_epoch,
        bound_record_type,
        bound_family_tag,
        bound_replay_binding,
    ) = parse_pg_indexes(&attempt.index_binding)?;
    if bound_indexes != attempt.indexes
        || bound_commitment != attempt.dependency_commitment
        || bound_record_type != attempt.record_type
        || bound_family_tag != attempt.family_tag
        || bound_replay_binding != attempt.replay_binding
        || attempt.identity.is_some_and(|identity| {
            bound_read_epoch
                != identity_digest(
                    identity,
                    &attempt.record_type,
                    attempt.family_tag.as_deref().unwrap_or(""),
                )
        })
    {
        return Err(DurableError::Corrupt(
            "bounded PG creation in-memory attempt differs from index binding",
        ));
    }
    let domain = tx.query_one(
        "SELECT * FROM cmd2_domain WHERE domain=$1",
        &[&request.domain],
    )?;
    let expected_indexes = attempt.index_binding.clone();
    let expected_reads = reads_bytes(&attempt.reads)?;
    let empty_projections = projections_bytes(&ProjectionRows::new())?;
    super::require_current_source_definition(attempt.definition)?;
    let domain_definition = domain
        .get::<_, Option<String>>("source_definition_digest")
        .ok_or(DurableError::Corrupt(
            "bounded PG creation definition absent",
        ))?;
    super::require_current_source_definition(parse_hex(domain_definition)?)?;
    if attempt.domain != request.domain
        || attempt.prepare_id != request.prepare_id
        || attempt.fence != request.attempt_fence
        || attempt.definition != definition()
        || attempt.dependency_commitment.version != COMMITMENT_VERSION
        || !family_tag_matches
        || !matches!(
            attempt.record_type.as_str(),
            "agent" | "place" | "organization" | "work" | "collection" | "artifact"
        ) && !attempt.family_tag.as_deref().is_some_and(is_pg_family_tag)
        || attempt.contract != request.expected_contract_digest
        || attempt.rule_version != request.expected_rule_version
        || attempt.rights_version != request.expected_rights_version
        || attempt.job_id != request.job_id
        || attempt.job_fence != request.job_fence
        || registered.get::<_, String>("command_id") != attempt.command_id
        || registered.get::<_, String>("raw_request_digest") != attempt.raw_request_digest.to_hex()
        || registered.get::<_, String>("delta_digest") != attempt.delta.to_hex()
        || registered.get::<_, Option<Vec<u8>>>("source_reads") != Some(expected_reads)
        || registered.get::<_, Option<Vec<u8>>>("source_indexes") != Some(expected_indexes)
        || registered.get::<_, Option<Vec<u8>>>("source_projections") != Some(empty_projections)
        || registered.get::<_, Option<i64>>("source_epoch") != Some(as_i64(attempt.epoch)?)
        || domain.get::<_, Option<String>>("source_definition_digest")
            != Some(attempt.definition.to_hex())
        || (!replayed
            && domain.get::<_, Option<i64>>("source_epoch") != Some(as_i64(attempt.epoch)?))
        || !domain.get::<_, bool>("source_complete")
        || as_u64(domain.get("rule_version"))? != request.expected_rule_version
        || as_u64(domain.get("rights_version"))? != request.expected_rights_version
        || domain.get::<_, String>("contract_digest") != request.expected_contract_digest.to_hex()
        || domain.get::<_, Option<String>>("schema_profile_digest")
            != Some(schema_profile_digest().to_hex())
        || members.iter().any(|member| {
            member.get::<_, String>("profile_id").as_bytes() != PG_CORPUS_CREATION_PROFILE
        })
    {
        return Err(DurableError::Conflict(
            "bounded PG creation registered binding changed",
        ));
    }
    let job = tx.query_opt(
        "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2 FOR UPDATE",
        &[&request.domain, &request.job_id],
    )?;
    if job.map(|row| row.get::<_, i64>(0)) != Some(as_i64(request.job_fence)?) {
        return Err(DurableError::Refused(
            "bounded PG creation current job lease changed",
        ));
    }
    let dependencies = source_dependencies(&attempt.reads);
    let write_subjects = attempt
        .writes
        .iter()
        .map(|write| write.subject.as_str())
        .collect::<BTreeSet<_>>();
    for member in members {
        let carrier = row_source_metadata(member)?;
        let expected = attempt
            .metadata
            .get(member.get::<_, String>("subject").as_str())
            .ok_or(DurableError::Corrupt(
                "bounded PG creation carrier metadata absent",
            ))?;
        if carrier != *expected
            || carrier.mode != 0o644
            || carrier.dependencies != Some(dependencies.clone())
        {
            return Err(DurableError::Conflict(
                "bounded PG creation carrier metadata differs",
            ));
        }
    }
    for (kind, _, path) in &attempt.indexes {
        if (kind != "path" && !fixed_index(kind)) || !write_subjects.contains(path.as_str()) {
            return Err(DurableError::Corrupt(
                "bounded PG creation index member binding differs",
            ));
        }
    }
    if replayed {
        return Ok(());
    }
    if domain.get::<_, Option<i64>>("source_epoch") != Some(as_i64(attempt.epoch)?) {
        return Err(DurableError::Conflict(
            "bounded PG creation source epoch changed",
        ));
    }
    let observed = check_predicate_reads(tx, &request.domain, &attempt.reads, attempt.definition)?;
    let unique_kinds = PG_CREATION_INDEX_KINDS
        .iter()
        .map(|kind| (*kind).to_owned())
        .collect::<Vec<_>>();
    for (kind, token, path) in &attempt.indexes {
        let occupied = if kind == "path" {
            tx.query_opt(
                "SELECT 1 FROM cmd2_source_index
                 WHERE domain=$1 AND kind='path' AND token=$2 LIMIT 1",
                &[&request.domain, token],
            )?
        } else {
            tx.query_opt(
                "SELECT 1 FROM cmd2_source_index
                 WHERE domain=$1 AND kind=ANY($2) AND token=$3 LIMIT 1",
                &[&request.domain, &unique_kinds, token],
            )?
        };
        if occupied.is_some() {
            return Err(DurableError::Conflict(
                "bounded PG creation unique identity changed",
            ));
        }
        if !path.starts_with("ToS/") {
            return Err(DurableError::Corrupt("bounded PG creation index path"));
        }
    }
    let home = attempt
        .reads
        .reads
        .iter()
        .find_map(|read| match read {
            PredicateRead::Generation { predicate, .. }
                if predicate.kind == PredicateKind::Range && predicate.scope == "source-home" =>
            {
                Some(predicate.token.as_str())
            }
            _ => None,
        })
        .ok_or(DurableError::Corrupt(
            "bounded PG creation home predicate absent",
        ))?;
    check_home_absent(tx, &request.domain, home)?;
    if observed.is_empty() {
        return Err(DurableError::Corrupt(
            "bounded PG creation predicate readset empty",
        ));
    }
    for read in &attempt.reads.reads {
        match read {
            PredicateRead::Exact {
                key,
                expected_version,
                expected_digest,
                ..
            } => {
                let row = tx
                    .query_opt(
                        "SELECT * FROM cmd2_current WHERE domain=$1 AND subject=$2",
                        &[&request.domain, key],
                    )?
                    .ok_or(DurableError::Conflict(
                        "bounded PG creation exact source read disappeared",
                    ))?;
                if Some(as_u64(row.get("revision"))?) != *expected_version
                    || Some(parse_hex(row.get("content_digest"))?) != *expected_digest
                    || attempt.reads.locators.get(key) != Some(&metadata_locator_digest(&row))
                {
                    return Err(DurableError::Conflict(
                        "bounded PG creation exact source read changed",
                    ));
                }
            }
            PredicateRead::Absent { namespace, key } if namespace == "source" => {
                if tx
                    .query_opt(
                        "SELECT 1 FROM cmd2_current WHERE domain=$1 AND subject=$2",
                        &[&request.domain, key],
                    )?
                    .is_some()
                {
                    return Err(DurableError::Conflict(
                        "bounded PG creation source absence changed",
                    ));
                }
            }
            PredicateRead::Generation { .. } | PredicateRead::Absent { .. } => {}
        }
    }
    Ok(())
}

pub(super) fn apply_source_change(
    tx: &mut postgres::Transaction<'_>,
    request: &CommitShadowAttempt<'_>,
    attempt: &PgCreationAttempt,
    seq: u64,
) -> DurableResult<()> {
    for (kind, token, path) in &attempt.indexes {
        tx.execute(
            "INSERT INTO cmd2_source_index(domain,kind,token,path,definition_digest)
             VALUES($1,$2,$3,$4,$5)",
            &[
                &request.domain,
                kind,
                token,
                path,
                &attempt.definition.to_hex(),
            ],
        )?;
    }
    // Read predicates remain exact admission evidence. Invalidation is derived
    // only from the indexes represented by this package, so two independent
    // creations do not conflict merely because both observed the same epoch.
    let predicates = super::predicates(&attempt.indexes);
    for (kind, scope, token) in predicates {
        ensure_predicate(
            tx,
            &request.domain,
            &kind,
            &scope,
            &token,
            attempt.definition,
            None,
        )?;
        let changed = tx.execute(
            "UPDATE cmd2_predicate SET generation=generation+1
             WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5
               AND complete AND definition_version=$6
               AND generation<9223372036854775807",
            &[
                &request.domain,
                &kind,
                &OWNER,
                &scope,
                &token,
                &attempt.definition.to_hex(),
            ],
        )?;
        if changed != 1 {
            return Err(DurableError::Refused(
                "bounded PG creation predicate invalidation exhausted",
            ));
        }
    }
    tx.execute(
        "UPDATE cmd2_domain SET source_generation=$2,source_projection_digest=NULL,
         selected_generation_digest=NULL,complete_cut_digest=NULL,complete_cut_generation=NULL
         WHERE domain=$1 AND source_complete",
        &[&request.domain, &as_i64(seq)?],
    )?;
    Ok(())
}

pub(super) fn verify_owner_before_outcome(
    owner_check: &dyn Fn() -> DurableResult<()>,
) -> DurableResult<()> {
    owner_check()
}

fn part(hasher: &mut Digest256Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value);
}
