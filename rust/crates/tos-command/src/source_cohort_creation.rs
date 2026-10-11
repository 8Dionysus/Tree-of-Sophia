//! Bounded PostgreSQL/STO transport for one initial native Agent creation.
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

pub(crate) const PG_CREATION: &[u8] = b"tos.source-file.agent-create-pg-v1";
// This is a new bounded semantic commitment, deliberately separate from the
// legacy complete `source-inventory/all` predicate and its v1 namespace.
pub(crate) const COMMITMENT_VERSION: u16 = 2;
pub(crate) const DEPENDENCY_SCOPE: &str = "source-dependencies-v2";

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
                "bounded Agent dependency commitment version",
            ));
        }
        Ok(Self { version, digest })
    }
    pub(crate) fn version(self) -> u16 {
        self.version
    }
    pub(crate) fn digest(self) -> Digest256 {
        self.digest
    }
}

/// Capture owns the concrete sealed prepared type. Implementing this trait is
/// the only adapter boundary it needs; no public forgeable proposal is
/// accepted by registration.
pub(crate) trait PgAgentCreationInput {
    fn context(&self) -> &CommandContext;
    /// The semantic owner keeps the unconsumed plan alongside its sealed
    /// creation payload.  Registration never accepts a caller-built
    /// `PreparedCommand`; it derives the base revision from `context` and
    /// binds every plan field into the durable delta.
    fn plan(&self) -> &CommandPlan;
    fn home(&self) -> &RelativePath;
    fn files(&self) -> &BTreeMap<String, Vec<u8>>;
    fn indexes(&self) -> &IndexRows;
    fn dependency_commitment(&self) -> PgDependencyCommitment;
    fn identity(&self) -> PgRecordRevisionIdentity;
    fn home_observation(&self) -> &PgSourceHomeObservation;
    fn unique_observations(&self) -> &[PgSourceUniqueIndexObservation];
}

/// Exact durable registration state for one bounded Agent creation.
pub(crate) struct PgAgentCreationAttempt {
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
    pub(crate) predicate_keys: BTreeSet<(String, String, String)>,
    pub(crate) writes: Vec<SourceRevisionWrite>,
    pub(crate) identity: Option<PgRecordRevisionIdentity>,
}

fn identity_digest(identity: PgRecordRevisionIdentity) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"tos-pg-agent-creation-read-epoch-v1");
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
) -> DurableResult<Vec<u8>> {
    serde_json::to_vec(&serde_json::json!({
        "profile": "tos-pg-agent-creation-indexes-v2",
        "commitment": [commitment.version, commitment.digest.to_hex()],
        "read_epoch": identity_digest(identity).to_hex(),
        "indexes": indexes.iter().collect::<Vec<_>>(),
    }))
    .map_err(|_| DurableError::Invalid("bounded Agent indexes encode"))
}

fn parse_pg_indexes(raw: &[u8]) -> DurableResult<(IndexRows, PgDependencyCommitment, Digest256)> {
    let value: serde_json::Value = serde_json::from_slice(raw)
        .map_err(|_| DurableError::Corrupt("bounded Agent indexes encoding"))?;
    if value.get("profile").and_then(|v| v.as_str()) != Some("tos-pg-agent-creation-indexes-v2") {
        return Err(DurableError::Corrupt("bounded Agent indexes profile"));
    }
    let commitment = value
        .get("commitment")
        .and_then(|v| v.as_array())
        .filter(|v| v.len() == 2)
        .ok_or(DurableError::Corrupt("bounded Agent commitment encoding"))?;
    let version = commitment[0]
        .as_u64()
        .and_then(|v| u16::try_from(v).ok())
        .ok_or(DurableError::Corrupt("bounded Agent commitment version"))?;
    let digest = Digest256::from_hex(
        commitment[1]
            .as_str()
            .ok_or(DurableError::Corrupt("bounded Agent commitment digest"))?,
    )
    .map_err(|_| DurableError::Corrupt("bounded Agent commitment digest"))?;
    let read_epoch = Digest256::from_hex(
        value
            .get("read_epoch")
            .and_then(|v| v.as_str())
            .ok_or(DurableError::Corrupt("bounded Agent read epoch digest"))?,
    )
    .map_err(|_| DurableError::Corrupt("bounded Agent read epoch digest"))?;
    let rows = value
        .get("indexes")
        .and_then(|v| v.as_array())
        .ok_or(DurableError::Corrupt("bounded Agent indexes rows"))?;
    let mut indexes = IndexRows::new();
    for row in rows {
        let values = row
            .as_array()
            .filter(|values| values.len() == 3)
            .ok_or(DurableError::Corrupt("bounded Agent index row"))?;
        let item = (
            values[0]
                .as_str()
                .ok_or(DurableError::Corrupt("bounded Agent index kind"))?
                .to_owned(),
            values[1]
                .as_str()
                .ok_or(DurableError::Corrupt("bounded Agent index token"))?
                .to_owned(),
            values[2]
                .as_str()
                .ok_or(DurableError::Corrupt("bounded Agent index path"))?
                .to_owned(),
        );
        if (item.0 != "path" && !fixed_index(&item.0))
            || item.1.is_empty()
            || item.2.is_empty()
            || !item.2.starts_with("ToS/")
            || (item.0 == "path" && item.1 != item.2)
        {
            return Err(DurableError::Corrupt("bounded PG Agent index profile"));
        }
        if !indexes.insert(item) {
            return Err(DurableError::Corrupt("duplicate bounded Agent index"));
        }
    }
    Ok((
        indexes,
        PgDependencyCommitment::new(version, digest)?,
        read_epoch,
    ))
}

fn fixed_index(kind: &str) -> bool {
    PG_AGENT_INDEX_KINDS.contains(&kind)
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
            "bounded Agent required predicate read absent",
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
            "bounded Agent source generation read absent",
        ))
}

fn creation_delta(
    input: &dyn PgAgentCreationInput,
    readset: &SourceRevisionReadSet,
    writes: &[SourceRevisionWrite],
    indexes: &IndexRows,
    reads: &SourceReads,
) -> DurableResult<Digest256> {
    let selection = readset.selection();
    let plan = input.plan();
    let commitment = input.dependency_commitment();
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"tos-pg-agent-creation-delta-v1");
    part(&mut hasher, selection.cohort.domain.as_bytes());
    part(&mut hasher, selection.contract.as_bytes());
    part(&mut hasher, &selection.epoch.to_be_bytes());
    part(&mut hasher, &selection.source_generation.to_be_bytes());
    part(&mut hasher, &selection.head_seq.to_be_bytes());
    part(&mut hasher, identity_digest(input.identity()).as_bytes());
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

fn creation_writes(input: &dyn PgAgentCreationInput) -> DurableResult<Vec<SourceRevisionWrite>> {
    let plan = input.plan();
    if plan.operation != "source.create"
        || plan.handler_id != "native-corpus-create"
        || plan.changes.is_empty()
        || plan.changes.len() > MAX_MEMBERS
    {
        return Err(DurableError::Refused(
            "bounded PG creation requires an initial native Agent package",
        ));
    }
    let request = cmd::parse(&input.context().request_raw).map_err(source_error)?;
    let record = cmd::field(&request, "record").map_err(source_error)?;
    if cmd::text(record, "record_type").map_err(source_error)? != "agent"
        || cmd::integer(record, "record_version").map_err(source_error)? != 1
    {
        return Err(DurableError::Refused(
            "bounded PG creation requires Agent v1",
        ));
    }
    let config = cmd::parse(&input.context().configuration_raw).map_err(source_error)?;
    let source_path = cmd::text(&config, "source_path").map_err(source_error)?;
    if source_path != format!("{}/agent.json", input.home().as_str()) {
        return Err(DurableError::Conflict(
            "bounded PG Agent source path differs from selected home",
        ));
    }
    let serialized = input
        .files()
        .get("agent.json")
        .ok_or(DurableError::Corrupt("bounded PG Agent body absent"))?;
    if !cmd::same(record, &cmd::parse(serialized).map_err(source_error)?).map_err(source_error)? {
        return Err(DurableError::Conflict(
            "bounded PG Agent body differs from semantic record",
        ));
    }
    let prefix = format!("{}/", input.home().as_str());
    let mut changes = plan.changes.clone();
    changes.sort_by(|left, right| left.path.cmp(&right.path));
    if changes.windows(2).any(|pair| pair[0].path == pair[1].path)
        || changes.len() != input.files().len()
    {
        return Err(DurableError::Invalid(
            "bounded PG Agent package membership differs",
        ));
    }
    let mut writes = Vec::with_capacity(changes.len());
    for change in changes {
        let subject = change.path.as_str().to_owned();
        let name = subject.strip_prefix(&prefix).ok_or(DurableError::Refused(
            "bounded PG Agent member escapes selected home",
        ))?;
        if name.is_empty() {
            return Err(DurableError::Refused(
                "bounded PG Agent package contains its home directory",
            ));
        }
        let after = change
            .after
            .ok_or(DurableError::Refused("bounded PG creation deletion"))?;
        if change.before.is_some() || input.files().get(name) != Some(&after) {
            return Err(DurableError::Conflict(
                "bounded PG Agent member bytes differ from semantic package",
            ));
        }
        writes.push(SourceRevisionWrite {
            subject,
            expected_predecessor: None,
            proposed_revision: 1,
            exact_bytes: after,
        });
    }
    Ok(writes)
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
            "bounded PG Agent source home is occupied",
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
            "bounded PG Agent source home is occupied",
        ));
    }
    Ok(())
}

/// Derive the maintained structural index and ancestor predicates from the
/// actual package writes.  These are transport-owned facts: the semantic
/// producer supplies only its identity indexes, while every package member
/// still receives the same `path` row and source-home invalidation coverage as
/// the complete creation route.
fn add_derived_path_predicates(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    reads: &mut SourceReads,
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
        let generation = ensure_predicate(tx, domain, &kind, &scope, &token, definition, None)?;
        let predicate_kind = if kind == "unique" {
            PredicateKind::Unique
        } else {
            PredicateKind::Range
        };
        if let Some(observed) = reads.reads.iter().find_map(|read| match read {
            PredicateRead::Generation {
                predicate,
                observed_generation,
            } if predicate.kind == predicate_kind
                && predicate.owner == OWNER
                && predicate.scope == scope
                && predicate.token == token
                && predicate.definition_version == definition.to_hex() =>
            {
                Some(*observed_generation)
            }
            _ => None,
        }) {
            if observed != generation {
                return Err(DurableError::Conflict(
                    "bounded PG derived source predicate changed",
                ));
            }
        } else {
            reads.reads.push(PredicateRead::Generation {
                predicate: PredicateToken {
                    kind: predicate_kind,
                    owner: OWNER.to_owned(),
                    scope,
                    token,
                    definition_version: definition.to_hex(),
                },
                observed_generation: generation,
            });
        }
    }
    Ok(())
}

fn valid_predicate_shape(predicate: &PredicateToken) -> bool {
    if predicate.owner != OWNER || predicate.definition_version != definition().to_hex() {
        return false;
    }
    match (predicate.kind, predicate.scope.as_str()) {
        (PredicateKind::Range, "source-home") => {
            predicate.token.starts_with("ToS/") && !predicate.token.ends_with('/')
        }
        (PredicateKind::Range, DEPENDENCY_SCOPE) => {
            predicate.token == COMMITMENT_VERSION.to_string()
        }
        (PredicateKind::Unique, "path") => {
            predicate.token.starts_with("ToS/") && !predicate.token.ends_with('/')
        }
        (PredicateKind::Unique, scope) => fixed_index(scope) && !predicate.token.is_empty(),
        _ => false,
    }
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
    /// Register one semantic Agent package against the selected bounded PG
    /// read epoch. All source/index absence checks are point or C-range seeks.
    pub(crate) fn register_pg_agent_creation(
        &mut self,
        store: &SegmentStore,
        input: &dyn PgAgentCreationInput,
        readset: &SourceRevisionReadSet,
        prepare_id: &[u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<PgAgentCreationAttempt> {
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
            for kind in PG_AGENT_INDEX_KINDS {
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
        let initial_audit = lock_audit_fence(&mut tx, domain)?;
        if initial_audit != selection.audit_generation {
            return Err(DurableError::Conflict(
                "bounded PG creation audit fence changed before registration",
            ));
        }
        let row = tx.query_one("SELECT * FROM cmd2_domain WHERE domain=$1", &[&domain])?;
        cohort_matches(&row, &selection.cohort, true)?;
        if as_u64(row.get("head_seq"))? != selection.head_seq
            || row.get::<_, Option<i64>>("source_generation")
                != Some(as_i64(selection.source_generation)?)
            || as_u64(row.get("rights_version"))? != selection.rights_version
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
        add_derived_path_predicates(
            &mut tx,
            domain,
            &mut reads,
            &writes,
            selection.cohort.definition,
        )?;
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
        let indexes_encoded = indexes_bytes(&indexes, commitment, input.identity())?;
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
            return Ok(PgAgentCreationAttempt {
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
        let unique_kinds = PG_AGENT_INDEX_KINDS
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
        Ok(PgAgentCreationAttempt {
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
            predicate_keys,
            writes,
            identity: Some(input.identity()),
        })
    }

    /// Reconstruct a ready or committed bounded creation from PG/STO
    /// carriers. The current source graph is not used to reconstruct the
    /// semantic proposal; current rights, schema, lease and owner checks still
    /// run at the final commit fence.
    pub(crate) fn recover_pg_agent_creation_attempt(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        prepare_id: &[u8],
        expected_command_id: &str,
        expected_raw_request_digest: Digest256,
        expected_configuration_raw_digest: Digest256,
        expected_configuration_canonical_digest: Digest256,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<PgAgentCreationAttempt> {
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
        if definition != super::definition() {
            return Err(DurableError::Conflict(
                "bounded PG creation recovery definition differs",
            ));
        }
        let reads = decode_reads(
            &attempt
                .get::<_, Option<Vec<u8>>>("source_reads")
                .ok_or(DurableError::Corrupt("bounded PG creation reads absent"))?,
        )?;
        let (indexes, dependency_commitment, _read_epoch) = parse_pg_indexes(
            &attempt
                .get::<_, Option<Vec<u8>>>("source_indexes")
                .ok_or(DurableError::Corrupt("bounded PG creation indexes absent"))?,
        )?;
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
            if row.get::<_, String>("profile_id").as_bytes() != PG_CREATION {
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
        Ok(PgAgentCreationAttempt {
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
            predicate_keys,
            writes,
            identity: None,
        })
    }

    pub(crate) fn commit_pg_agent_creation(
        &mut self,
        store: &SegmentStore,
        attempt: &PgAgentCreationAttempt,
        contract: Digest256,
        owner_check: &dyn Fn() -> DurableResult<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<(DurableCommitReceipt, DurableTiming)> {
        active(deadline, cancelled)?;
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
                            PG_CREATION,
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
                    PG_CREATION,
                    Some(attempt.delta),
                    Some(&attempt.metadata),
                    Some(attempt.audit_generation),
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
                                PG_CREATION,
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
}

pub(super) fn check_commit_owner(
    tx: &mut postgres::Transaction<'_>,
    request: &CommitShadowAttempt<'_>,
    registered: &postgres::Row,
    members: &[postgres::Row],
    attempt: &PgAgentCreationAttempt,
    owner_check: &dyn Fn() -> DurableResult<()>,
    replayed: bool,
) -> DurableResult<()> {
    owner_check()?;
    let domain = tx.query_one(
        "SELECT * FROM cmd2_domain WHERE domain=$1",
        &[&request.domain],
    )?;
    let expected_indexes = attempt.index_binding.clone();
    let expected_reads = reads_bytes(&attempt.reads)?;
    let empty_projections = projections_bytes(&ProjectionRows::new())?;
    if attempt.domain != request.domain
        || attempt.prepare_id != request.prepare_id
        || attempt.fence != request.attempt_fence
        || attempt.definition != definition()
        || attempt.dependency_commitment.version != COMMITMENT_VERSION
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
        || members
            .iter()
            .any(|member| member.get::<_, String>("profile_id").as_bytes() != PG_CREATION)
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
    if domain.get::<_, Option<i64>>("source_epoch") != Some(as_i64(attempt.epoch)?)
        || domain.get::<_, Option<i64>>("source_generation")
            != Some(as_i64(attempt.source_generation)?)
    {
        return Err(DurableError::Conflict(
            "bounded PG creation source generation changed",
        ));
    }
    let observed = check_predicate_reads(tx, &request.domain, &attempt.reads, attempt.definition)?;
    let unique_kinds = PG_AGENT_INDEX_KINDS
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
    attempt: &PgAgentCreationAttempt,
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
    let mut predicates = attempt.predicate_keys.clone();
    predicates.insert((
        "range".to_owned(),
        DEPENDENCY_SCOPE.to_owned(),
        attempt.dependency_commitment.version.to_string(),
    ));
    // The legacy complete inventory predicate is invalidated for compatibility
    // with old readers, but it is never a prerequisite of this bounded route.
    predicates.insert((
        "range".to_owned(),
        "source-inventory".to_owned(),
        "all".to_owned(),
    ));
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
