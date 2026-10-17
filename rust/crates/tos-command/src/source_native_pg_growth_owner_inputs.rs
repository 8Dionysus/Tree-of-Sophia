//! Exact catalog and endpoint selection for PG-native Growth owners.
//!
//! These packages select the complete witness catalog through the PG input
//! seam, then bind each addressed owner endpoint to its exact catalog row and
//! bytes. They are intentionally not a synthetic corpus cut or a source-wide
//! report; the native compound core owns the family topology and read closure.

use crate::source_cohort::revision::PgRecordRevisionIdentity;
use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{
    CreationFilesystem, SelectedSourceWitnessCatalog, select_source_witness_catalog_from_input,
};
use crate::source_revisions::ReadonlyRecordFiles;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{JsonValue, RelativePath};
use tos_validation::record_biblio_cut::{
    SourceCutInput, SourceCutInputWithIdentity, SourceCutMemberMeta,
};

const MAX_SELECTED_MEMBER: usize = 8_388_608;

/// One selected catalog plus only the exact owner files required to construct
/// its `CommandContext`. The catalog itself was checked as a complete current
/// topology by `select_source_witness_catalog_from_input`.
pub(crate) struct PgGrowthSelectedInputs {
    pub(crate) identity: PgRecordRevisionIdentity,
    pub(crate) catalog: SelectedSourceWitnessCatalog,
    files: BTreeMap<String, Vec<u8>>,
}

impl PgGrowthSelectedInputs {
    pub(crate) fn select(
        input: &dyn SourceCutInputWithIdentity<PgRecordRevisionIdentity>,
        publication_token: Option<&str>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let catalog = select_source_witness_catalog_from_input(
            input.source_input(),
            publication_token,
            deadline,
            cancelled,
        )?;
        Ok(Self {
            identity: *input.input_identity(),
            catalog,
            files: BTreeMap::new(),
        })
    }

    pub(crate) fn record(
        &mut self,
        input: &dyn SourceCutInput,
        record_type: &str,
        record_id: &str,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let mut matches = self.catalog.entries.iter().filter(|(kind, entry)| {
            kind == record_type
                && entry.object_get("record_id").and_then(JsonValue::as_str) == Some(record_id)
        });
        let entry = matches.next().ok_or(SourceCommandError::Conflict(
            "PG Growth endpoint absent from catalog",
        ))?;
        if matches.next().is_some() || cmd::text(&entry.1, "source_record_ref")? != path {
            return Err(SourceCommandError::Conflict(
                "PG Growth endpoint catalog identity differs",
            ));
        }
        let raw = read_current_member(input, path, MAX_SELECTED_MEMBER, deadline, cancelled)?;
        let record = cmd::parse(&raw)?;
        if cmd::text(&record, "record_type")? != record_type
            || cmd::text(&record, "record_id")? != record_id
            || cmd::text(&entry.1, "record_sha256")? != cmd::record_digest(&record)?.to_hex()
        {
            return Err(SourceCommandError::Conflict(
                "PG Growth endpoint bytes differ from catalog",
            ));
        }
        self.insert_file(path, raw)
    }

    pub(crate) fn absent_record(
        &self,
        input: &dyn SourceCutInput,
        record_id: &str,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let relative = canonical_path(path)?;
        if self.catalog.record_ids.contains(record_id)
            || input
                .path_presence(path, deadline, cancelled)
                .map_err(|_| SourceCommandError::Conflict("PG Growth endpoint absence read"))?
                .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "PG Growth new record identity or path is occupied",
            ));
        }
        let _ = relative;
        Ok(())
    }

    pub(crate) fn absent_claim(&self, claim_id: &str) -> SourceCommandResult<()> {
        if self.catalog.claim_ids.contains(claim_id) || self.catalog.record_ids.contains(claim_id) {
            return Err(SourceCommandError::Conflict(
                "PG Growth new Claim identity is occupied",
            ));
        }
        Ok(())
    }

    /// Select one additional exact authored member needed by the owner core
    /// (for example, its grammar or semantic registries) and retain the bytes
    /// for both CommandContext construction and later PG-session rebinding.
    pub(crate) fn member(
        &mut self,
        input: &dyn SourceCutInput,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        let raw = read_current_member(input, path, MAX_SELECTED_MEMBER, deadline, cancelled)?;
        self.insert_file(path, raw.clone())?;
        Ok(raw)
    }

    fn insert_file(&mut self, path: &str, raw: Vec<u8>) -> SourceCommandResult<()> {
        let relative = canonical_path(path)?;
        if let Some(existing) = self.files.get(path) {
            if existing != &raw {
                return Err(SourceCommandError::Conflict(
                    "PG Growth selected endpoint changed",
                ));
            }
            return Ok(());
        }
        let _ = relative;
        self.files.insert(path.to_owned(), raw);
        Ok(())
    }

    /// Rebind every catalog member and exact owner endpoint selected from the
    /// cut input to the held PostgreSQL read session before capturing its
    /// readset. The complete catalog is part of owner authority, so its
    /// manifest and all routed rows are observed as well as addressed records.
    pub(crate) fn bind_selected_readset(
        &self,
        session: &mut crate::source_cohort::revision::SourceRevisionReadSession<'_>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> crate::DurableResult<()> {
        use tos_foundation::{Digest256, RelativePath};

        if self.identity != session.selection().identity() {
            return Err(crate::DurableError::Conflict(
                "PG Growth selected catalog identity differs from held read session",
            ));
        }
        let mut expected = self.catalog.digests.clone();
        for (path, raw) in &self.files {
            let digest = Digest256::of_bytes(raw).to_hex();
            if expected
                .insert(path.clone(), digest.clone())
                .is_some_and(|prior| prior != digest)
            {
                return Err(crate::DurableError::Conflict(
                    "PG Growth selected member conflicts with catalog closure",
                ));
            }
        }
        for (path, digest) in expected {
            let relative = RelativePath::parse(&path)
                .map_err(|_| crate::DurableError::Invalid("PG Growth selected path"))?;
            if relative.as_str() != path {
                return Err(crate::DurableError::Invalid(
                    "PG Growth noncanonical selected path",
                ));
            }
            let member = session.read_current_member(&relative, 16_777_216, deadline, cancelled)?;
            if Digest256::of_bytes(&member.raw).to_hex() != digest {
                return Err(crate::DurableError::Conflict(
                    "PG Growth selected bytes differ from held read session",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn context(
        &self,
        configuration_raw: &[u8],
        request_raw: &[u8],
        recorded_at: String,
        effective_uid: u32,
        software_files: &[cmd::SourceFile],
    ) -> SourceCommandResult<CommandContext> {
        let mut files = self
            .files
            .iter()
            .map(|(path, raw)| {
                Ok(cmd::SourceFile {
                    path: canonical_path(path)?,
                    raw: raw.clone(),
                })
            })
            .collect::<SourceCommandResult<Vec<_>>>()?;
        for file in software_files {
            if let Some(existing) = files.iter().find(|existing| existing.path == file.path) {
                if existing.raw != file.raw {
                    return Err(SourceCommandError::Conflict(
                        "PG Growth software/source path collision",
                    ));
                }
                continue;
            }
            files.push(file.clone());
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let context = CommandContext {
            base_revision: self.identity.bootstrap_revision,
            configuration_raw: configuration_raw.to_vec(),
            request_raw: request_raw.to_vec(),
            recorded_at,
            effective_uid: u64::from(effective_uid),
            files,
        };
        context.check()?;
        Ok(context)
    }
}

/// Materialize the exact authored PG point reads into the owner context so a
/// local protected-source fence can compare the same bytes before registration.
/// Absence predicates are re-probed and returned for the owner to check against
/// its protected local source root as well.
pub(crate) fn context_with_pg_exact_reads(
    context: &CommandContext,
    session: &mut crate::source_cohort::revision::SourceRevisionReadSession<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(CommandContext, BTreeSet<String>)> {
    use crate::PredicateRead;
    use tos_foundation::Digest256;

    const MAX_EXACT_MEMBERS: usize = 4096;
    const MAX_EXACT_BYTES: usize = 33_554_432;
    let reads = session.reads_snapshot();
    let mut result = context.clone();
    let mut absences = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut total = result
        .files
        .iter()
        .try_fold(0usize, |sum, file| sum.checked_add(file.raw.len()));
    for read in reads.reads {
        match read {
            PredicateRead::Exact {
                namespace,
                key,
                expected_version,
                expected_digest,
            } if namespace == "source" => {
                if !paths.insert(key.clone()) || paths.len() > MAX_EXACT_MEMBERS {
                    return Err(SourceCommandError::Invalid(
                        "PG Growth exact read count bound",
                    ));
                }
                let relative = canonical_path(&key)?;
                let member = session
                    .read_current_member(&relative, MAX_SELECTED_MEMBER as u64, deadline, cancelled)
                    .map_err(|_| SourceCommandError::Conflict("PG Growth exact read changed"))?;
                if expected_version.is_some_and(|revision| revision != member.custody_revision)
                    || expected_digest
                        .is_some_and(|digest| digest != Digest256::of_bytes(&member.raw))
                {
                    return Err(SourceCommandError::Conflict(
                        "PG Growth exact read digest or revision changed",
                    ));
                }
                if let Some(existing) = result.files.iter().find(|file| file.path == relative) {
                    if existing.raw != member.raw {
                        return Err(SourceCommandError::Conflict(
                            "PG Growth context bytes differ from exact read",
                        ));
                    }
                } else {
                    total = total
                        .and_then(|sum| sum.checked_add(member.raw.len()))
                        .filter(|sum| *sum <= MAX_EXACT_BYTES);
                    if total.is_none() {
                        return Err(SourceCommandError::Invalid(
                            "PG Growth exact read byte bound",
                        ));
                    }
                    result.files.push(cmd::SourceFile {
                        path: relative,
                        raw: member.raw,
                    });
                }
            }
            PredicateRead::Absent { namespace, key } if namespace == "source" => {
                if !absences.insert(key.clone()) || absences.len() > MAX_EXACT_MEMBERS {
                    return Err(SourceCommandError::Invalid(
                        "PG Growth exact absence count bound",
                    ));
                }
                let relative = canonical_path(&key)?;
                if session.has_file(&key, deadline, cancelled).map_err(|_| {
                    SourceCommandError::Conflict("PG Growth exact absence probe failed")
                })? {
                    return Err(SourceCommandError::Conflict(
                        "PG Growth exact path absence changed",
                    ));
                }
                if result.files.iter().any(|file| file.path == relative) {
                    return Err(SourceCommandError::Conflict(
                        "PG Growth path is both present and absent",
                    ));
                }
            }
            _ => (),
        }
    }
    result
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    result.check()?;
    Ok((result, absences))
}

/// The exact current Work/Expression/Edition endpoints for one Edition
/// creation attempt, with the future Edition and Claim identities proven free.
impl PgGrowthSelectedInputs {
    pub(crate) fn select_expression_edition(
        input: &dyn SourceCutInputWithIdentity<PgRecordRevisionIdentity>,
        fs: &CreationFilesystem,
        configuration: &JsonValue,
        publication_token: Option<&str>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        crate::source_creation_store::check_expression_edition_configuration(
            fs,
            configuration,
            deadline,
            cancelled,
        )?;
        let work_id = cmd::text(configuration, "work_id")?;
        let work_path = cmd::text(configuration, "work_source_path")?.to_owned();
        let expression_id = cmd::text(configuration, "expression_id")?;
        let expression_path = cmd::text(configuration, "expression_source_path")?.to_owned();
        let edition_id = cmd::text(configuration, "edition_id")?;
        let edition_path = cmd::text(configuration, "edition_source_path")?.to_owned();
        let claim_id = cmd::text(configuration, "claim_id")?;
        let claim_path = format!(
            "{}/source-claims.jsonl",
            edition_path
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .ok_or(SourceCommandError::Invalid(
                    "PG Expression Edition Claim home"
                ),)?
        );
        let mut selected =
            PgGrowthSelectedInputs::select(input, publication_token, deadline, cancelled)?;
        selected.record(
            input.source_input(),
            "work",
            work_id,
            &work_path,
            deadline,
            cancelled,
        )?;
        selected.record(
            input.source_input(),
            "expression",
            expression_id,
            &expression_path,
            deadline,
            cancelled,
        )?;
        selected.absent_record(
            input.source_input(),
            edition_id,
            &edition_path,
            deadline,
            cancelled,
        )?;
        selected.absent_claim(claim_id)?;
        if catalog_event_occupied(
            &selected.catalog,
            cmd::text(configuration, "provenance_event_id")?,
        ) {
            return Err(SourceCommandError::Conflict(
                "PG Expression Edition event identity is occupied",
            ));
        }
        Ok(selected)
    }
}

/// Exact current Expression/Agent endpoints for one responsibility Claim.
impl PgGrowthSelectedInputs {
    pub(crate) fn select_expression_responsibility(
        input: &dyn SourceCutInputWithIdentity<PgRecordRevisionIdentity>,
        fs: &CreationFilesystem,
        configuration: &JsonValue,
        publication_token: Option<&str>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        crate::source_creation_store::check_expression_responsibility_configuration(
            fs,
            configuration,
            deadline,
            cancelled,
        )?;
        let expression_id = cmd::text(configuration, "expression_id")?;
        let expression_path = cmd::text(configuration, "expression_source_path")?.to_owned();
        let agent_id = cmd::text(configuration, "agent_id")?;
        let agent_path = cmd::text(configuration, "agent_source_path")?.to_owned();
        let claim_id = cmd::text(configuration, "claim_id")?;
        let claim_path = cmd::text(configuration, "claim_source_path")?.to_owned();
        let mut selected =
            PgGrowthSelectedInputs::select(input, publication_token, deadline, cancelled)?;
        selected.record(
            input.source_input(),
            "expression",
            expression_id,
            &expression_path,
            deadline,
            cancelled,
        )?;
        selected.record(
            input.source_input(),
            "agent",
            agent_id,
            &agent_path,
            deadline,
            cancelled,
        )?;
        selected.absent_claim(claim_id)?;
        if catalog_event_occupied(
            &selected.catalog,
            cmd::text(configuration, "provenance_event_id")?,
        ) {
            return Err(SourceCommandError::Conflict(
                "PG Expression Responsibility event identity is occupied",
            ));
        }
        Ok(selected)
    }
}

/// Exact current Collection/Work endpoints for one membership Claim.
impl PgGrowthSelectedInputs {
    pub(crate) fn select_collection_membership(
        input: &dyn SourceCutInputWithIdentity<PgRecordRevisionIdentity>,
        fs: &CreationFilesystem,
        configuration: &JsonValue,
        publication_token: Option<&str>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        crate::source_creation_store::check_collection_membership_configuration(
            fs,
            configuration,
            deadline,
            cancelled,
        )?;
        let collection_id = cmd::text(configuration, "collection_id")?;
        let collection_path = cmd::text(configuration, "collection_source_path")?.to_owned();
        let work_id = cmd::text(configuration, "work_id")?;
        let work_path = cmd::text(configuration, "work_source_path")?.to_owned();
        let claim_id = cmd::text(configuration, "claim_id")?;
        let claim_path = cmd::text(configuration, "claim_source_path")?.to_owned();
        let mut selected =
            PgGrowthSelectedInputs::select(input, publication_token, deadline, cancelled)?;
        selected.record(
            input.source_input(),
            "collection",
            collection_id,
            &collection_path,
            deadline,
            cancelled,
        )?;
        selected.record(
            input.source_input(),
            "work",
            work_id,
            &work_path,
            deadline,
            cancelled,
        )?;
        selected.absent_claim(claim_id)?;
        if catalog_event_occupied(
            &selected.catalog,
            cmd::text(configuration, "provenance_event_id")?,
        ) {
            return Err(SourceCommandError::Conflict(
                "PG Collection Membership event identity is occupied",
            ));
        }
        Ok(selected)
    }
}

/// Exact current Work endpoint plus the future Expression/Claim identities
/// for one Work-expression creation. The Work owner keeps request and grant
/// authorization; this bridge binds only the source-side identity closure.
impl PgGrowthSelectedInputs {
    pub(crate) fn select_work_expression(
        input: &dyn SourceCutInputWithIdentity<PgRecordRevisionIdentity>,
        configuration: &JsonValue,
        publication_token: Option<&str>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let work_id = cmd::text(configuration, "work_id")?;
        let work_path = cmd::text(configuration, "work_source_path")?.to_owned();
        let expression_id = cmd::text(configuration, "expression_id")?;
        let expression_path = cmd::text(configuration, "expression_source_path")?.to_owned();
        let claim_id = cmd::text(configuration, "claim_id")?;
        let claim_path = format!(
            "{}/source-claims.jsonl",
            expression_path
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .ok_or(SourceCommandError::Invalid("PG Work Expression Claim home"))?
        );
        let mut selected =
            PgGrowthSelectedInputs::select(input, publication_token, deadline, cancelled)?;
        selected.record(
            input.source_input(),
            "work",
            work_id,
            &work_path,
            deadline,
            cancelled,
        )?;
        selected.absent_record(
            input.source_input(),
            expression_id,
            &expression_path,
            deadline,
            cancelled,
        )?;
        selected.absent_claim(claim_id)?;
        if catalog_event_occupied(
            &selected.catalog,
            cmd::text(configuration, "provenance_event_id")?,
        ) {
            return Err(SourceCommandError::Conflict(
                "PG Work Expression event identity is occupied",
            ));
        }
        Ok(selected)
    }
}

pub(crate) fn read_current_member(
    input: &dyn SourceCutInput,
    path: &str,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let _ = canonical_path(path)?;
    let presence = input
        .path_presence(path, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG Growth selected path observation"))?;
    if presence.is_none() {
        return Err(SourceCommandError::Conflict(
            "PG Growth selected path absent",
        ));
    }
    let mut selected = None;
    input
        .with_current_member(
            path,
            max_bytes.min(MAX_SELECTED_MEMBER),
            deadline,
            cancelled,
            &mut |meta: SourceCutMemberMeta<'_>, raw| {
                if meta.path != path || raw.len() as u64 != meta.size_bytes {
                    return Err(tos_validation::item_rules::ItemRefusal::Source(
                        "PG Growth selected member binding differs".into(),
                    ));
                }
                selected = Some(raw.to_vec());
                Ok(())
            },
        )
        .map_err(|_| SourceCommandError::Conflict("PG Growth selected member read"))?;
    selected.ok_or(SourceCommandError::Conflict(
        "PG Growth selected member absent",
    ))
}

fn canonical_path(path: &str) -> SourceCommandResult<RelativePath> {
    let parsed = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Invalid("PG Growth selected source path"))?;
    if parsed.as_str() != path {
        return Err(SourceCommandError::Invalid(
            "PG Growth noncanonical selected source path",
        ));
    }
    Ok(parsed)
}

fn catalog_event_occupied(catalog: &SelectedSourceWitnessCatalog, event_id: &str) -> bool {
    catalog.entries.iter().any(|(_, entry)| {
        entry
            .object_get("provenance_event_ref")
            .and_then(JsonValue::as_str)
            == Some(event_id)
    })
}
