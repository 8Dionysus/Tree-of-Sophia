//! Bounded PostgreSQL/STO transport for one authored source revision.
//!
//! This is deliberately a read epoch, not a current-source generation.  It
//! reconstructs only the immutable cohort identity and the addressed members
//! needed by one owner command; it never performs the complete cold reopen or
//! creates a selected full-corpus snapshot.

use super::*;
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_revisions::ReadonlyRecordFiles;
use postgres::IsolationLevel;
use std::sync::atomic::AtomicBool;

const MAX_CHILDREN: usize = 64;
const MAX_RETAINED_STATE_BYTES: usize = 64 * 1024 * 1024;

/// The bounded initial Agent route treats these five identity namespaces as a
/// single uniqueness domain.  Keep the list with the PG transport so the
/// absence probes and their generation predicates cannot drift apart.
pub(crate) const PG_AGENT_INDEX_KINDS: [&str; 5] = ["metadata", "claim", "event", "anchor", "form"];

#[derive(Clone, Debug)]
pub(crate) struct SourceRevisionSelection {
    pub(crate) cohort: ManagedSourceCohort,
    pub(crate) contract: Digest256,
    pub(crate) epoch: u64,
    pub(crate) source_generation: u64,
    pub(crate) head_seq: u64,
    pub(crate) audit_generation: u64,
    pub(crate) rights_version: u64,
    pub(crate) rule_version: u64,
    pub(crate) job_id: String,
    pub(crate) job_fence: u64,
}

/// Opaque identity of the bounded PostgreSQL read epoch.  This is deliberately
/// separate from `SourceRevision`: the latter names the real bootstrap source
/// revision carried by the semantic `CommandContext`, while this value binds
/// the selected PG domain/fences used for admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PgRecordRevisionIdentity {
    pub(crate) domain_digest: Digest256,
    pub(crate) store_id: [u8; 16],
    pub(crate) bootstrap_revision: SourceRevision,
    pub(crate) bootstrap_membership: SourceMembershipV1,
    pub(crate) cohort_digest: Digest256,
    pub(crate) cohort_epoch: u64,
    pub(crate) cohort_definition: Digest256,
    pub(crate) cohort_generation: u64,
    pub(crate) job_id_digest: Digest256,
    pub(crate) job_fence: u64,
    pub(crate) epoch: u64,
    pub(crate) source_generation: u64,
    pub(crate) head_seq: u64,
    pub(crate) audit_generation: u64,
    pub(crate) rights_version: u64,
    pub(crate) rule_version: u64,
    pub(crate) contract: Digest256,
}

impl PgRecordRevisionIdentity {
    pub(crate) fn from_selection(selection: &SourceRevisionSelection) -> Self {
        let mut cohort_hasher = Digest256Hasher::new();
        part(
            &mut cohort_hasher,
            b"tos-pg-record-revision-cohort-identity-v1",
        );
        part(&mut cohort_hasher, selection.cohort.domain.as_bytes());
        part(&mut cohort_hasher, &selection.cohort.store_id);
        part(
            &mut cohort_hasher,
            selection.cohort.initial_revision.0.as_bytes(),
        );
        part(
            &mut cohort_hasher,
            &selection.cohort.initial_membership.count.to_be_bytes(),
        );
        part(
            &mut cohort_hasher,
            selection.cohort.initial_membership.digest.as_bytes(),
        );
        part(&mut cohort_hasher, &selection.cohort.epoch.to_be_bytes());
        part(&mut cohort_hasher, selection.cohort.definition.as_bytes());
        part(
            &mut cohort_hasher,
            &selection.cohort.generation.to_be_bytes(),
        );
        Self {
            domain_digest: Digest256::of_bytes(selection.cohort.domain.as_bytes()),
            store_id: selection.cohort.store_id,
            bootstrap_revision: selection.cohort.initial_revision,
            bootstrap_membership: selection.cohort.initial_membership,
            cohort_digest: cohort_hasher.finalize(),
            cohort_epoch: selection.cohort.epoch,
            cohort_definition: selection.cohort.definition,
            cohort_generation: selection.cohort.generation,
            job_id_digest: Digest256::of_bytes(selection.job_id.as_bytes()),
            job_fence: selection.job_fence,
            epoch: selection.epoch,
            source_generation: selection.source_generation,
            head_seq: selection.head_seq,
            audit_generation: selection.audit_generation,
            rights_version: selection.rights_version,
            rule_version: selection.rule_version,
            contract: selection.contract,
        }
    }
}

impl SourceRevisionSelection {
    pub(crate) fn identity(&self) -> PgRecordRevisionIdentity {
        PgRecordRevisionIdentity::from_selection(self)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SourceRevisionObservation {
    pub(crate) member: ManagedCurrentMember,
    pub(crate) locator: Digest256,
}

pub(crate) struct SourceRevisionReadSet {
    pub(crate) selection: SourceRevisionSelection,
    pub(crate) reads: SourceReads,
    pub(crate) observations: BTreeMap<String, SourceRevisionObservation>,
    pub(crate) work: SourceRevisionReadWork,
}

#[derive(Clone, Debug)]
pub(crate) struct PgSourceHomeObservation {
    pub(crate) parent: RelativePath,
    pub(crate) children: Vec<(String, bool)>,
    pub(crate) generation: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct PgSourceUniqueIndexObservation {
    pub(crate) kind: String,
    pub(crate) token: String,
    pub(crate) rows: Vec<(String, String)>,
    pub(crate) generation: u64,
}

/// Bounded source transport accounting. These counters describe work actually
/// performed by the selected PG/STO route; they are not a claim that a
/// complete source inventory was observed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SourceRevisionReadWork {
    pub(crate) exact_source_members_read: u64,
    pub(crate) exact_source_bytes_read: u64,
    pub(crate) custody_metadata_bytes_read: u64,
    pub(crate) segment_bytes_read: u64,
    pub(crate) committed_member_rows_examined: u64,
    pub(crate) metadata_member_lookups: u64,
    pub(crate) directory_index_rows_examined: u64,
    pub(crate) unique_index_lookups: u64,
}

pub(crate) struct SourceRevisionReadSession<'a> {
    coordinator: &'a mut DurablePgCoordinator,
    store: &'a SegmentStore,
    selection: SourceRevisionSelection,
    reads: SourceReads,
    observations: BTreeMap<String, SourceRevisionObservation>,
    retained_state_bytes: usize,
    predicate_reads: BTreeSet<(String, String, String)>,
    work: SourceRevisionReadWork,
}

impl SourceRevisionReadSet {
    pub(crate) fn selection(&self) -> &SourceRevisionSelection {
        &self.selection
    }

    pub(crate) fn reads(&self) -> &SourceReads {
        &self.reads
    }

    pub(crate) fn observations(&self) -> &BTreeMap<String, SourceRevisionObservation> {
        &self.observations
    }

    pub(crate) fn work(&self) -> SourceRevisionReadWork {
        self.work
    }
}

impl DurablePgCoordinator {
    /// Execute one bounded source read epoch. The closure may read addressed
    /// members and immediate children only. The final fence check happens
    /// before the readset is returned, so callers cannot accidentally use a
    /// stale PG selection as an admitted source revision.
    pub(crate) fn with_source_revision_session<T>(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
        operation: impl FnOnce(&mut SourceRevisionReadSession<'_>) -> DurableResult<T>,
    ) -> DurableResult<(T, SourceRevisionReadSet)> {
        active(deadline, cancelled)?;
        let selection = self.open_source_revision_selection(
            store, domain, job_id, job_fence, deadline, cancelled,
        )?;
        let mut session = SourceRevisionReadSession {
            coordinator: self,
            store,
            selection: selection.clone(),
            reads: SourceReads {
                reads: vec![PredicateRead::Generation {
                    predicate: PredicateToken {
                        kind: PredicateKind::Range,
                        owner: OWNER.to_owned(),
                        scope: "source-cohort".to_owned(),
                        token: "generation".to_owned(),
                        definition_version: selection.cohort.definition.to_hex(),
                    },
                    observed_generation: selection.source_generation,
                }],
                locators: BTreeMap::new(),
                managed_original: None,
            },
            observations: BTreeMap::new(),
            retained_state_bytes: 0,
            predicate_reads: BTreeSet::new(),
            work: SourceRevisionReadWork::default(),
        };
        let result = operation(&mut session)?;
        session.verify_fresh(deadline, cancelled)?;
        let readset = SourceRevisionReadSet {
            selection: session.selection.clone(),
            reads: session.reads,
            observations: session.observations,
            work: session.work,
        };
        Ok((result, readset))
    }

    /// Reopen only the domain/cohort descriptor needed by an incremental
    /// revision. No complete membership, index, or STO generation is scanned.
    pub(crate) fn open_source_revision_selection(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<SourceRevisionSelection> {
        active(deadline, cancelled)?;
        if domain.is_empty()
            || store.custody_domain() != domain.as_bytes()
            || store.store_id() == [0; 16]
        {
            return Err(DurableError::Conflict(
                "source revision custody domain differs",
            ));
        }
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let row = tx.query_one(
            "SELECT d.*,f.generation AS audit_generation,f.maintenance_state
             FROM cmd2_domain d JOIN cmd2_audit_fence f USING(domain)
             WHERE d.domain=$1",
            &[&domain],
        )?;
        if row.get::<_, String>("maintenance_state") != "normal"
            || !row.get::<_, bool>("rights_allowed")
            || !row.get::<_, bool>("source_complete")
            || row.get::<_, Option<String>>("schema_profile_digest")
                != Some(schema_profile_digest().to_hex())
        {
            return Err(DurableError::Refused(
                "bounded source revision selection is not currently readable",
            ));
        }
        let initial_revision = SourceRevision(parse_hex(
            row.get::<_, Option<String>>("source_revision")
                .ok_or(DurableError::Refused("source cohort revision absent"))?,
        )?);
        let initial_membership = SourceMembershipV1 {
            count: as_u64(
                row.get::<_, Option<i64>>("source_membership_count")
                    .ok_or(DurableError::Corrupt("source membership count absent"))?,
            )?,
            digest: parse_hex(
                row.get::<_, Option<String>>("source_membership_digest")
                    .ok_or(DurableError::Corrupt("source membership digest absent"))?,
            )?,
        };
        let epoch = as_u64(
            row.get::<_, Option<i64>>("source_epoch")
                .ok_or(DurableError::Corrupt("source epoch absent"))?,
        )?;
        let definition_digest = parse_hex(
            row.get::<_, Option<String>>("source_definition_digest")
                .ok_or(DurableError::Corrupt("source definition absent"))?,
        )?;
        if definition_digest != super::definition() {
            return Err(DurableError::Conflict(
                "bounded source revision definition differs",
            ));
        }
        let source_generation = as_u64(
            row.get::<_, Option<i64>>("source_generation")
                .ok_or(DurableError::Refused("source generation absent"))?,
        )?;
        let head_seq = as_u64(row.get("head_seq"))?;
        let audit_generation = as_u64(row.get("audit_generation"))?;
        let contract = parse_hex(row.get::<_, String>("contract_digest"))?;
        let rights_version = as_u64(row.get("rights_version"))?;
        let rule_version = as_u64(row.get("rule_version"))?;
        let job = tx
            .query_opt(
                "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2",
                &[&domain, &job_id],
            )?
            .ok_or(DurableError::Refused(
                "bounded source revision job lease absent",
            ))?;
        if as_u64(job.get(0))? != job_fence {
            return Err(DurableError::Refused(
                "bounded source revision job lease changed",
            ));
        }
        let cohort = ManagedSourceCohort {
            domain: domain.to_owned(),
            store_id: store.store_id(),
            initial_revision,
            initial_membership,
            epoch,
            definition: definition_digest,
            generation: source_generation,
        };
        cohort_matches(&row, &cohort, true)?;
        tx.commit()?;
        Ok(SourceRevisionSelection {
            cohort,
            contract,
            epoch,
            source_generation,
            head_seq,
            audit_generation,
            rights_version,
            rule_version,
            job_id: job_id.to_owned(),
            job_fence,
        })
    }
}

impl SourceRevisionReadSession<'_> {
    pub(crate) fn selection(&self) -> &SourceRevisionSelection {
        &self.selection
    }

    pub(crate) fn work(&self) -> SourceRevisionReadWork {
        self.work
    }

    fn charge_work(counter: &mut u64, amount: u64) -> DurableResult<()> {
        *counter = counter
            .checked_add(amount)
            .ok_or(DurableError::Refused("bounded source read work overflow"))?;
        Ok(())
    }

    fn observe_predicate_generation(
        &mut self,
        kind: PredicateKind,
        scope: &str,
        token: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<u64> {
        active(deadline, cancelled)?;
        if scope.is_empty() || token.is_empty() {
            return Err(DurableError::Invalid(
                "bounded source predicate identity empty",
            ));
        }
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence
             WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0))? != self.selection.audit_generation
        {
            return Err(DurableError::Conflict(
                "bounded source predicate audit fence changed",
            ));
        }
        let gate = tx.query_one(
            "SELECT rights_allowed,source_complete,source_generation,source_epoch,
                    contract_digest,source_definition_digest
             FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        if !gate.get::<_, bool>(0)
            || !gate.get::<_, bool>(1)
            || gate.get::<_, Option<i64>>(2) != Some(as_i64(self.selection.source_generation)?)
            || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.epoch)?)
            || gate.get::<_, String>(4) != self.selection.contract.to_hex()
            || gate.get::<_, Option<String>>(5) != Some(self.selection.cohort.definition.to_hex())
        {
            return Err(DurableError::Conflict(
                "bounded source predicate read epoch changed",
            ));
        }
        let kind_text = kind.as_str();
        let predicate = tx.query_opt(
            "SELECT generation,complete,definition_version FROM cmd2_predicate
             WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5",
            &[
                &self.selection.cohort.domain,
                &kind_text,
                &OWNER,
                &scope,
                &token,
            ],
        )?;
        let generation = predicate
            .as_ref()
            .map(|row| as_u64(row.get(0)))
            .transpose()?
            .unwrap_or(0);
        if predicate.as_ref().is_some_and(|row| {
            !row.get::<_, bool>(1)
                || row.get::<_, String>(2) != self.selection.cohort.definition.to_hex()
        }) {
            return Err(DurableError::Conflict(
                "bounded source predicate definition differs",
            ));
        }
        tx.commit()?;
        self.record_predicate(kind_text, scope, token, generation);
        Ok(generation)
    }

    /// Record and verify that the complete source-home prefix is empty. The
    /// C-collated half-open range is an indexed existence check, not a corpus
    /// directory scan.
    pub(crate) fn observe_home_absence(
        &mut self,
        home: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<PgSourceHomeObservation> {
        active(deadline, cancelled)?;
        let home = home.as_str();
        if !home.starts_with("ToS/") || home.ends_with('/') {
            return Err(DurableError::Invalid("bounded source home path"));
        }
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence
             WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0))? != self.selection.audit_generation
        {
            return Err(DurableError::Conflict(
                "bounded source home audit fence changed",
            ));
        }
        let lower = format!("{home}/");
        let upper = format!("{home}0");
        if tx
            .query_opt(
                "SELECT subject FROM cmd2_current
                 WHERE domain=$1 AND subject=$2
                 LIMIT 1",
                &[&self.selection.cohort.domain, &home],
            )?
            .is_some()
        {
            return Err(DurableError::Conflict(
                "bounded source home is already occupied",
            ));
        }
        if tx
            .query_opt(
                "SELECT subject FROM cmd2_current
                 WHERE domain=$1
                   AND subject COLLATE \"C\" >= $2 COLLATE \"C\"
                   AND subject COLLATE \"C\" < $3 COLLATE \"C\"
                 ORDER BY subject COLLATE \"C\" LIMIT 1",
                &[&self.selection.cohort.domain, &lower, &upper],
            )?
            .is_some()
        {
            return Err(DurableError::Conflict(
                "bounded source home is already occupied",
            ));
        }
        tx.commit()?;
        let generation = self.observe_predicate_generation(
            PredicateKind::Range,
            "source-home",
            home,
            deadline,
            cancelled,
        )?;
        Ok(PgSourceHomeObservation {
            parent: RelativePath::parse(home)
                .map_err(|_| DurableError::Invalid("bounded source home path"))?,
            children: Vec::new(),
            generation,
        })
    }

    /// Record a point unique-identity observation. Only the fixed owner index
    /// kinds are admitted by the bounded Agent-creation route.
    pub(crate) fn observe_unique_index(
        &mut self,
        kind: &str,
        token: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<PgSourceUniqueIndexObservation> {
        if !PG_AGENT_INDEX_KINDS.contains(&kind) {
            return Err(DurableError::Refused(
                "bounded source unique index kind is outside Agent profile",
            ));
        }
        if token.is_empty() {
            return Err(DurableError::Invalid("bounded source unique token empty"));
        }
        active(deadline, cancelled)?;
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence
             WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0))? != self.selection.audit_generation
        {
            return Err(DurableError::Conflict(
                "bounded source unique audit fence changed",
            ));
        }
        let kinds = PG_AGENT_INDEX_KINDS
            .iter()
            .map(|kind| (*kind).to_owned())
            .collect::<Vec<_>>();
        if tx
            .query_opt(
                "SELECT 1 FROM cmd2_source_index
                 WHERE domain=$1 AND kind=ANY($2) AND token=$3 LIMIT 1",
                &[&self.selection.cohort.domain, &kinds, &token],
            )?
            .is_some()
        {
            return Err(DurableError::Conflict(
                "bounded source unique identity is already occupied",
            ));
        }
        tx.commit()?;
        Self::charge_work(&mut self.work.unique_index_lookups, 1)?;
        // A token is unique across the five fixed namespaces.  Record every
        // namespace's generation in the readset so a later write in any one
        // of them invalidates this admission.
        let mut generation = None;
        for observed_kind in PG_AGENT_INDEX_KINDS {
            let observed = self.observe_predicate_generation(
                PredicateKind::Unique,
                observed_kind,
                token,
                deadline,
                cancelled,
            )?;
            if observed_kind == kind {
                generation = Some(observed);
            }
        }
        Ok(PgSourceUniqueIndexObservation {
            kind: kind.to_owned(),
            token: token.to_owned(),
            rows: Vec::new(),
            generation: generation.ok_or(DurableError::Corrupt(
                "bounded source unique generation missing",
            ))?,
        })
    }

    /// Bind the semantic dependency commitment to its own invalidation
    /// predicate. This is intentionally separate from the legacy complete
    /// `source-inventory/all` range used by cold creation.
    pub(crate) fn observe_dependency_commitment(
        &mut self,
        version: u16,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<u64> {
        let scope = match version {
            1 => "source-dependencies-v1",
            2 => "source-dependencies-v2",
            _ => {
                return Err(DurableError::Refused(
                    "bounded source dependency commitment version",
                ));
            }
        };
        self.observe_predicate_generation(
            PredicateKind::Range,
            scope,
            &version.to_string(),
            deadline,
            cancelled,
        )
    }

    fn record_exact_metadata(
        &mut self,
        key: &str,
        custody_revision: u64,
        content_digest: Digest256,
        locator: Digest256,
    ) -> DurableResult<()> {
        if let Some(read) = self.reads.reads.iter().find_map(|read| match read {
            PredicateRead::Exact {
                namespace,
                key: prior,
                expected_version,
                expected_digest,
            } if namespace == "source" && prior == key => Some((expected_version, expected_digest)),
            _ => None,
        }) {
            if read.0 != &Some(custody_revision) || read.1 != &Some(content_digest) {
                return Err(DurableError::Conflict(
                    "bounded source exact read metadata changed",
                ));
            }
        } else {
            self.reads.reads.push(PredicateRead::Exact {
                namespace: "source".to_owned(),
                key: key.to_owned(),
                expected_version: Some(custody_revision),
                expected_digest: Some(content_digest),
            });
        }
        if self
            .reads
            .locators
            .insert(key.to_owned(), locator)
            .is_some_and(|prior| prior != locator)
        {
            return Err(DurableError::Conflict(
                "bounded source exact locator changed",
            ));
        }
        Ok(())
    }

    fn record_exact(
        &mut self,
        member: ManagedCurrentMember,
        locator: Digest256,
    ) -> DurableResult<()> {
        let key = member.path.as_str().to_owned();
        self.record_exact_metadata(
            &key,
            member.custody_revision,
            member.metadata.sha256,
            locator,
        )?;
        self.observations
            .insert(key, SourceRevisionObservation { member, locator });
        Ok(())
    }

    fn record_absent(&mut self, path: &str) {
        if self.reads.reads.iter().any(|read| {
            matches!(read, PredicateRead::Absent { namespace, key } if namespace == "source" && key == path)
        }) {
            return;
        }
        self.reads.reads.push(PredicateRead::Absent {
            namespace: "source".to_owned(),
            key: path.to_owned(),
        });
    }

    fn record_predicate(&mut self, kind: &str, scope: &str, token: &str, generation: u64) {
        let key = (kind.to_owned(), scope.to_owned(), token.to_owned());
        if !self.predicate_reads.insert(key.clone()) {
            return;
        }
        self.reads.reads.push(PredicateRead::Generation {
            predicate: PredicateToken {
                kind: if kind == "unique" {
                    PredicateKind::Unique
                } else {
                    PredicateKind::Range
                },
                owner: OWNER.to_owned(),
                scope: scope.to_owned(),
                token: token.to_owned(),
                definition_version: self.selection.cohort.definition.to_hex(),
            },
            observed_generation: generation,
        });
    }

    fn verify_fresh(&mut self, deadline: Instant, cancelled: &AtomicBool) -> DurableResult<()> {
        active(deadline, cancelled)?;
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence
             WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        let row = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || !row.get::<_, bool>("rights_allowed")
            || !row.get::<_, bool>("source_complete")
            || as_u64(row.get("rights_version"))? != self.selection.rights_version
            || as_u64(row.get("rule_version"))? != self.selection.rule_version
            || as_u64(fence.get(0))? != self.selection.audit_generation
            || as_u64(row.get("head_seq"))? != self.selection.head_seq
            || as_u64(
                row.get::<_, Option<i64>>("source_generation")
                    .ok_or(DurableError::Corrupt("source generation absent"))?,
            )? != self.selection.source_generation
            || as_u64(
                row.get::<_, Option<i64>>("source_epoch")
                    .ok_or(DurableError::Corrupt("source epoch absent"))?,
            )? != self.selection.epoch
            || row.get::<_, String>("contract_digest") != self.selection.contract.to_hex()
            || row.get::<_, Option<String>>("source_definition_digest")
                != Some(self.selection.cohort.definition.to_hex())
        {
            return Err(DurableError::Conflict(
                "bounded source revision read epoch changed",
            ));
        }
        let job = tx.query_one(
            "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2",
            &[&self.selection.cohort.domain, &self.selection.job_id],
        )?;
        if as_u64(job.get(0))? != self.selection.job_fence {
            return Err(DurableError::Conflict(
                "bounded source revision job lease changed",
            ));
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn read_current_member(
        &mut self,
        path: &RelativePath,
        max_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ManagedCurrentMember> {
        active(deadline, cancelled)?;
        if !path.as_str().starts_with("ToS/") {
            return Err(DurableError::Refused(
                "bounded source revision path outside authored namespace",
            ));
        }
        let (row, raw, placement, work) = self.coordinator.read_committed_source_selected(
            self.store,
            &self.selection.cohort.domain,
            path.as_str(),
            None,
            max_bytes,
            deadline,
            cancelled,
            |row| cohort_matches(row, &self.selection.cohort, true),
        )?;
        let (metadata, dependency_claims) = expose_metadata(&row, path)?;
        let locator = metadata_locator_digest(&row);
        let member = ManagedCurrentMember {
            path: path.clone(),
            custody_revision: as_u64(row.get("revision"))?,
            current_generation: self.selection.head_seq,
            commit_seq: as_u64(row.get("commit_seq"))?,
            raw,
            metadata,
            dependency_claims,
            placement,
        };
        Self::charge_work(&mut self.work.exact_source_members_read, 1)?;
        Self::charge_work(
            &mut self.work.exact_source_bytes_read,
            member.raw.len() as u64,
        )?;
        Self::charge_work(
            &mut self.work.custody_metadata_bytes_read,
            work.custody_metadata_bytes_read,
        )?;
        Self::charge_work(&mut self.work.segment_bytes_read, work.segment_bytes_read)?;
        Self::charge_work(
            &mut self.work.committed_member_rows_examined,
            work.committed_member_rows_examined,
        )?;
        self.record_exact(member, locator)?;
        Ok(self
            .observations
            .get(path.as_str())
            .map(|observation| observation.member.clone())
            .ok_or(DurableError::Corrupt("source revision observation absent"))?)
    }

    pub(crate) fn list_immediate_children(
        &mut self,
        parent: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Vec<(String, bool)>> {
        active(deadline, cancelled)?;
        let parent = parent.as_str();
        if !parent.starts_with("ToS/") {
            return Err(DurableError::Refused(
                "bounded source revision directory outside authored namespace",
            ));
        }
        let prefix = format!("{parent}/");
        // The source subject index is byte-ordered under C collation. A
        // half-open prefix range keeps this immediate-child read bounded and
        // avoids a LIKE pattern scan over the whole corpus.
        let upper = format!("{parent}0");
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence
             WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0))? != self.selection.audit_generation
        {
            return Err(DurableError::Conflict(
                "bounded source directory audit fence changed",
            ));
        }
        let gate = tx.query_one(
            "SELECT rights_allowed,source_complete,source_generation,source_epoch,contract_digest,
                    source_definition_digest
             FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        if !gate.get::<_, bool>(0)
            || !gate.get::<_, bool>(1)
            || gate.get::<_, Option<i64>>(2) != Some(as_i64(self.selection.source_generation)?)
            || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.epoch)?)
            || gate.get::<_, String>(4) != self.selection.contract.to_hex()
            || gate.get::<_, Option<String>>(5) != Some(self.selection.cohort.definition.to_hex())
        {
            return Err(DurableError::Conflict(
                "bounded source directory read epoch changed",
            ));
        }
        let predicate = tx.query_opt(
            "SELECT generation,definition_version FROM cmd2_predicate
             WHERE domain=$1 AND kind='range' AND owner=$2 AND scope='source-home' AND token=$3",
            &[&self.selection.cohort.domain, &OWNER, &parent],
        )?;
        let generation = predicate
            .as_ref()
            .map(|row| as_u64(row.get(0)))
            .transpose()?
            .unwrap_or(0);
        if predicate
            .as_ref()
            .is_some_and(|row| row.get::<_, String>(1) != self.selection.cohort.definition.to_hex())
        {
            return Err(DurableError::Conflict(
                "source directory predicate definition differs",
            ));
        }
        let mut children = BTreeMap::new();
        let mut lower = prefix.clone();
        let mut inclusive = true;
        loop {
            active(deadline, cancelled)?;
            let row = tx.query_opt(
                "SELECT subject FROM cmd2_current
                 WHERE domain=$1
                   AND subject COLLATE \"C\" >= $2 COLLATE \"C\"
                   AND ($4 OR subject COLLATE \"C\" > $2 COLLATE \"C\")
                   AND subject COLLATE \"C\" < $3 COLLATE \"C\"
                 ORDER BY subject COLLATE \"C\" LIMIT 1",
                &[&self.selection.cohort.domain, &lower, &upper, &inclusive],
            )?;
            let Some(row) = row else { break };
            Self::charge_work(&mut self.work.directory_index_rows_examined, 1)?;
            let subject: String = row.get("subject");
            let rest = subject
                .strip_prefix(&prefix)
                .ok_or(DurableError::Corrupt("source child prefix differs"))?;
            let (name, directory) = match rest.split_once('/') {
                Some((name, _)) => (name, true),
                None => (rest, false),
            };
            if name.is_empty() {
                return Err(DurableError::Corrupt("source child name empty"));
            }
            if children.contains_key(name) {
                return Err(DurableError::Corrupt(
                    "source child file and directory collide",
                ));
            }
            if children.len() == MAX_CHILDREN {
                return Err(DurableError::Refused(
                    "source revision immediate child budget exceeded",
                ));
            }
            let name = name.to_owned();
            children.insert(name.clone(), directory);
            if directory {
                // Under the source path grammar, the slash that begins a
                // descendant sorts before `0` in C order.  Advancing to this
                // sibling bound skips the whole immediate directory in one
                // indexed seek; a later same-name row is rejected above.
                lower = format!("{prefix}{name}0");
                inclusive = true;
            } else {
                // A file is itself the key.  Advancing to `name0` would lose
                // valid prefixed siblings such as `foo.bar`; strict `>` keeps
                // the next key visible without a descendant probe.
                lower = subject;
                inclusive = false;
            }
        }
        tx.commit()?;
        self.record_predicate("range", "source-home", parent, generation);
        Ok(children.into_iter().collect())
    }
}

impl ReadonlyRecordFiles for SourceRevisionReadSession<'_> {
    fn read(
        &mut self,
        name: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        let path = RelativePath::parse(name)
            .map_err(|_| SourceCommandError::Invalid("bounded source member path"))?;
        match self.read_current_member(&path, max_bytes as u64, deadline, cancelled) {
            Ok(member) => {
                self.retained_state_bytes = self
                    .retained_state_bytes
                    .checked_add(name.len().saturating_mul(4) + member.raw.len())
                    .filter(|bytes| *bytes <= MAX_RETAINED_STATE_BYTES)
                    .ok_or(SourceCommandError::Unsupported(
                        "bounded source revision state budget",
                    ))?;
                Ok(member.raw)
            }
            Err(DurableError::Conflict("current source member absent")) => {
                self.record_absent(name);
                Err(SourceCommandError::Conflict("bounded source member absent"))
            }
            Err(error) => Err(source_command_error(error)),
        }
    }

    fn list_directory(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<(String, bool)>> {
        let path = RelativePath::parse(path)
            .map_err(|_| SourceCommandError::Invalid("bounded source directory path"))?;
        self.list_immediate_children(&path, deadline, cancelled)
            .map_err(source_command_error)
    }

    fn has_file(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<bool> {
        let path = RelativePath::parse(path)
            .map_err(|_| SourceCommandError::Invalid("bounded source member path"))?;
        if !path.as_str().starts_with("ToS/") {
            return Err(SourceCommandError::Denied(
                "bounded source revision path outside authored namespace",
            ));
        }
        active(deadline, cancelled).map_err(source_command_error)?;
        let mut tx = self
            .coordinator
            .client
            .transaction()
            .map_err(|error| source_command_error(DurableError::Database(error)))?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")
            .map_err(|error| source_command_error(DurableError::Database(error)))?;
        let fence = tx
            .query_one(
                "SELECT generation,maintenance_state FROM cmd2_audit_fence
                 WHERE domain=$1 FOR SHARE",
                &[&self.selection.cohort.domain],
            )
            .map_err(|error| source_command_error(DurableError::Database(error)))?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0)).map_err(source_command_error)?
                != self.selection.audit_generation
        {
            return Err(SourceCommandError::Conflict(
                "bounded source presence audit fence changed",
            ));
        }
        let gate = tx
            .query_one(
                "SELECT rights_allowed,source_complete,source_generation,source_epoch,
                        contract_digest,source_definition_digest
                 FROM cmd2_domain WHERE domain=$1 FOR SHARE",
                &[&self.selection.cohort.domain],
            )
            .map_err(|error| source_command_error(DurableError::Database(error)))?;
        if !gate.get::<_, bool>(0)
            || !gate.get::<_, bool>(1)
            || gate.get::<_, Option<i64>>(2)
                != Some(as_i64(self.selection.source_generation).map_err(source_command_error)?)
            || gate.get::<_, Option<i64>>(3)
                != Some(as_i64(self.selection.epoch).map_err(source_command_error)?)
            || gate.get::<_, String>(4) != self.selection.contract.to_hex()
            || gate.get::<_, Option<String>>(5) != Some(self.selection.cohort.definition.to_hex())
        {
            return Err(SourceCommandError::Conflict(
                "bounded source presence read epoch changed",
            ));
        }
        let row = tx
            .query_opt(
                "SELECT * FROM cmd2_current WHERE domain=$1 AND subject=$2",
                &[&self.selection.cohort.domain, &path.as_str()],
            )
            .map_err(|error| source_command_error(DurableError::Database(error)))?;
        Self::charge_work(&mut self.work.metadata_member_lookups, 1)
            .map_err(source_command_error)?;
        let Some(row) = row else {
            tx.commit()
                .map_err(|error| source_command_error(DurableError::Database(error)))?;
            self.record_absent(path.as_str());
            return Ok(false);
        };
        let custody_revision = as_u64(row.get("revision")).map_err(source_command_error)?;
        let content_digest =
            parse_hex(row.get::<_, String>("content_digest")).map_err(source_command_error)?;
        let locator = metadata_locator_digest(&row);
        tx.commit()
            .map_err(|error| source_command_error(DurableError::Database(error)))?;
        self.record_exact_metadata(path.as_str(), custody_revision, content_digest, locator)
            .map_err(source_command_error)?;
        Ok(true)
    }

    fn set_retained_state_bytes(
        &mut self,
        bytes: usize,
        _deadline: Instant,
        _cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if bytes > MAX_RETAINED_STATE_BYTES {
            return Err(SourceCommandError::Unsupported(
                "bounded source revision state budget",
            ));
        }
        self.retained_state_bytes = bytes;
        Ok(())
    }

    fn retained_state_bytes(&self) -> usize {
        self.retained_state_bytes
    }
}

fn source_command_error(error: DurableError) -> SourceCommandError {
    match error {
        DurableError::Source(error) => error,
        DurableError::Conflict(_) => {
            SourceCommandError::Conflict("bounded source revision read changed or is unavailable")
        }
        DurableError::Refused(_) => SourceCommandError::Denied(
            "bounded source revision read is refused by current custody state",
        ),
        DurableError::Invalid(_) | DurableError::Corrupt(_) => {
            SourceCommandError::Invalid("bounded source revision read metadata")
        }
        DurableError::Database(_) | DurableError::Storage(_) | DurableError::Indeterminate(_) => {
            SourceCommandError::Conflict("bounded source revision read custody unavailable")
        }
    }
}
