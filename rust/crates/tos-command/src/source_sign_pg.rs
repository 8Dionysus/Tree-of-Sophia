//! Bounded PostgreSQL adapters for the maintained Sign promotion owner.
//!
//! The shared revision writer calls these exact-byte projections after the
//! Sign owner has prepared and validated its package. They derive semantic
//! identity rows from the emitted package; they do not grant promotion.

use crate::source_cohort::revision::PgRecordRevisionIdentity;
use crate::source_cohort::{IndexRows, SourceRevisionWrite};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use crate::source_forms;
use crate::source_native_cli::PgGrowthCurrentInput;
use crate::source_native_cli::private_assessment as assessment;
use crate::source_revisions::{ReadonlyRecordFiles, RecordRead};
use crate::source_sign_native::{NativeReadKind, NativeReadScope, SignNativeRead};
use crate::source_text_owner::OwnerTextContext;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonString, JsonValue, RelativePath};
use tos_validation::record_biblio_cut::SourceCutInputWithIdentity;
use tos_validation::source_cut::CutSchemaExecutor;

type Result<T> = SourceCommandResult<T>;
type Error = SourceCommandError;

const PUBLICATION: &str = "ToS/source-witnesses/.metadata-publication.json";
const ENTITIES: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RELATIONS: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const MAX_NATIVE_ID_TOKENS: usize = 1024;
const SIGN_DESCRIPTION_SCHEMA: &str = "ToS/contracts/sign-description-record.schema.json";

fn map_input_refusal(refusal: tos_validation::item_rules::ItemRefusal) -> Error {
    use tos_validation::item_rules::ItemRefusal;
    match refusal {
        ItemRefusal::Deadline => Error::Denied("Sign selected source deadline"),
        ItemRefusal::Budget | ItemRefusal::BudgetCheck { .. } => {
            Error::Unsupported("Sign selected source input budget")
        }
        ItemRefusal::Source(_) => Error::Conflict("Sign selected source input changed"),
        ItemRefusal::Unsupported(_) => Error::Unsupported("Sign selected source input unavailable"),
        ItemRefusal::Executor(_) => Error::Unsupported("Sign selected source input execution"),
    }
}

fn read_current_input(
    input: &PgGrowthCurrentInput<'_, '_>,
    path: &str,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let relative =
        RelativePath::parse(path).map_err(|_| Error::Denied("Sign selected PG source path"))?;
    if relative.as_str() != path {
        return Err(Error::Denied("Sign selected PG source path"));
    }
    let mut selected = None;
    input
        .source_input()
        .with_current_member(path, max_bytes, deadline, cancelled, &mut |meta, raw| {
            if meta.path != path || selected.is_some() {
                return Err(tos_validation::item_rules::ItemRefusal::Source(
                    "repeated selected PG source member".into(),
                ));
            }
            selected = Some(raw.to_vec());
            Ok(())
        })
        .map_err(map_input_refusal)?;
    selected.ok_or(Error::Conflict("Sign selected PG source member absent"))
}

fn read_optional_current_input(
    input: &PgGrowthCurrentInput<'_, '_>,
    path: &str,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Option<Vec<u8>>> {
    let relative =
        RelativePath::parse(path).map_err(|_| Error::Denied("Sign selected PG source path"))?;
    if relative.as_str() != path {
        return Err(Error::Denied("Sign selected PG source path"));
    }
    let mut selected = None;
    input
        .source_input()
        .with_current_member(path, max_bytes, deadline, cancelled, &mut |meta, raw| {
            if meta.path != path || selected.is_some() {
                return Err(tos_validation::item_rules::ItemRefusal::Source(
                    "repeated selected PG source member".into(),
                ));
            }
            selected = Some(raw.to_vec());
            Ok(())
        })
        .map_err(map_input_refusal)?;
    Ok(selected)
}

struct CandidateProfileRead<I: Copy + Eq> {
    identity: I,
    files: Vec<crate::source_command::SourceFile>,
    schema_source_path: String,
}

impl<I: Copy + Eq> RecordRead for CandidateProfileRead<I> {
    type Identity = I;

    fn files(&self) -> &[crate::source_command::SourceFile] {
        &self.files
    }

    fn input_identity(&self) -> I {
        self.identity
    }

    fn schema_source_path(&self) -> Result<String> {
        Ok(self.schema_source_path.clone())
    }

    fn check(&self) -> Result<()> {
        Ok(())
    }
}

/// Exact current public-source snapshot for PG-backed Sign and private
/// Assessment consumers. Its membership claim is limited to configured
/// selected carriers and typed per-token identity-index lookups.
pub(crate) struct PgOwnerAssessmentPublicSources {
    pub(crate) rows: Vec<JsonValue>,
    pub(crate) native_rows: Vec<JsonValue>,
    pub(crate) native_summaries: Vec<JsonValue>,
    pub(crate) native_contracts: BTreeMap<String, String>,
    pub(crate) claim_dependencies: BTreeMap<String, Vec<JsonValue>>,
    pub(crate) fixity: Vec<JsonValue>,
    pub(crate) identity_snapshots: BTreeMap<String, JsonValue>,
    pub(crate) record_paths: BTreeMap<String, String>,
    pub(crate) form_paths: BTreeMap<String, String>,
    pub(crate) form_sets: BTreeMap<String, JsonValue>,
    pub(crate) selected_members: BTreeMap<String, Vec<u8>>,
    pub(crate) owner_snapshot: Digest256,
    pub(crate) publication: Option<Vec<u8>>,
    publication_selected: bool,
    identity_owners:
        BTreeMap<String, BTreeSet<crate::source_annotation_identity_index::AnnotationEntityOwner>>,
    native_identity_owner_context: Option<String>,
}

impl PgOwnerAssessmentPublicSources {
    pub(crate) fn input_bytes(&self) -> Result<usize> {
        self.selected_members
            .values()
            .try_fold(0usize, |total, raw| {
                total
                    .checked_add(raw.len())
                    .ok_or(Error::Invalid("Sign PG selected source byte overflow"))
            })
    }

    /// Re-read every exact member and every queried native identity key while
    /// the selected PG session remains open. The caller still performs the
    /// enclosing readset's final source-generation fence before commit.
    pub(crate) fn verify_current(
        &self,
        input: &PgGrowthCurrentInput<'_, '_>,
        owner_context: Option<&OwnerTextContext>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        if self.publication_selected {
            let publication =
                read_optional_current_input(input, PUBLICATION, 8192, deadline, cancelled)?;
            if publication != self.publication {
                return Err(Error::Conflict("Sign PG publication state changed"));
            }
        }
        for (path, expected) in &self.selected_members {
            if read_current_input(input, path, expected.len(), deadline, cancelled)? != *expected {
                return Err(Error::Conflict("Sign PG selected public member changed"));
            }
        }
        if let Some(expected) = &self.native_identity_owner_context {
            let context = owner_context.ok_or(Error::Conflict(
                "Sign PG owner text context absent during currentness check",
            ))?;
            let current = context.snapshot(deadline, cancelled)?.to_prefixed();
            if expected != &current {
                return Err(Error::Conflict("Sign PG owner text context changed"));
            }
        }
        if !self.identity_owners.is_empty() {
            let tokens = self
                .identity_owners
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>();
            let current = input
                .lookup_annotation_identity_owners(&tokens, deadline, cancelled)
                .map_err(|_| Error::Conflict("Sign PG native identity index changed"))?;
            if current != self.identity_owners {
                return Err(Error::Conflict("Sign PG native identity owners changed"));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_form_set(
        &mut self,
        path: &str,
        worker: &mut impl tos_validation::source_cut::CutSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        let Some(form_set) = self.form_sets.get(path) else {
            return Err(Error::Denied(
                "private assessment form set is outside selected public inputs",
            ));
        };
        let raw = self
            .selected_members
            .get(path)
            .ok_or(Error::Conflict("Sign PG selected form-set bytes absent"))?;
        let schema_path = "ToS/contracts/human-form-set.schema.json";
        let schema_raw = self
            .selected_members
            .get(schema_path)
            .ok_or(Error::Conflict("Sign PG form-set schema absent"))?;
        if worker.selected_contract_digest(schema_path) != Some(Digest256::of_bytes(schema_raw)) {
            return Err(Error::Conflict("Sign PG form-set worker binding differs"));
        }
        match worker.check(
            path,
            &cmd::canonical(form_set)?,
            schema_path,
            deadline,
            cancelled,
        ) {
            Ok(true) => (),
            Ok(false) => return Err(Error::Invalid("Sign PG form-set schema")),
            Err(reason) => {
                return Err(Error::SchemaExecution {
                    path: path.to_owned(),
                    root: schema_path.to_owned(),
                    reason,
                });
            }
        }
        if cmd::parse(raw)? != *form_set {
            return Err(Error::Conflict("Sign PG selected form-set bytes changed"));
        }
        let subject = cmd::field(form_set, "subject")?;
        tos_validation::source_forms::source_copy_kernel::validate_history(form_set, subject)
            .map_err(|_| Error::Invalid("Sign PG public form-set history"))?;
        Ok(())
    }

    pub(crate) fn candidate_ref(&self, candidate_id: &str) -> Result<JsonValue> {
        let selected = self
            .rows
            .iter()
            .filter(|row| row.object_get("id").and_then(JsonValue::as_str) == Some(candidate_id))
            .collect::<Vec<_>>();
        if selected.len() != 1
            || !selected[0]
                .object_get("payload")
                .is_some_and(|payload| payload.object_get("claim_id").is_some())
        {
            return Err(Error::Denied(
                "Sign PG candidate must be selected authored Claim",
            ));
        }
        super::envelope_ref(selected[0])
    }

    pub(crate) fn required_sources(&self, candidate_id: &str) -> Result<Vec<JsonValue>> {
        let mut all_rows = self.rows.clone();
        all_rows.extend(self.native_rows.clone());
        super::required_sources(
            candidate_id,
            &self.claim_dependencies,
            &all_rows,
            &self.native_summaries,
        )
    }
}

struct PgPublicSelection<'a, 'session, 'store> {
    input: &'a PgGrowthCurrentInput<'session, 'store>,
    owner_context: Option<&'a OwnerTextContext>,
    observed: BTreeMap<String, Vec<u8>>,
    selected_members: BTreeMap<String, Vec<u8>>,
    source_reads: BTreeSet<String>,
    source_bytes: usize,
    native_reads: BTreeSet<String>,
    native_bytes: usize,
    native_raws: BTreeMap<String, Vec<u8>>,
    source_carriers: Vec<String>,
    source_dependencies: BTreeSet<String>,
    identity_owners:
        BTreeMap<String, BTreeSet<crate::source_annotation_identity_index::AnnotationEntityOwner>>,
    identity_snapshot: Option<String>,
    profile_native_inputs: BTreeMap<(String, &'static str), Digest256>,
    native_identity_owner_context: Option<String>,
}

impl<'a, 'session, 'store> PgPublicSelection<'a, 'session, 'store> {
    fn new(
        input: &'a PgGrowthCurrentInput<'session, 'store>,
        owner_context: Option<&'a OwnerTextContext>,
    ) -> Self {
        Self {
            input,
            owner_context,
            observed: BTreeMap::new(),
            selected_members: BTreeMap::new(),
            source_reads: BTreeSet::new(),
            source_bytes: 0,
            native_reads: BTreeSet::new(),
            native_bytes: 0,
            native_raws: BTreeMap::new(),
            source_carriers: Vec::new(),
            source_dependencies: BTreeSet::new(),
            identity_owners: BTreeMap::new(),
            identity_snapshot: None,
            profile_native_inputs: BTreeMap::new(),
            native_identity_owner_context: None,
        }
    }

    fn authored(
        &mut self,
        name: &str,
        cap: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        if let Some(raw) = self.observed.get(name) {
            if raw.len() > cap {
                return Err(Error::Invalid("Sign PG selected source byte budget"));
            }
            return Ok(raw.clone());
        }
        let raw = read_current_input(self.input, name, cap, deadline, cancelled)?;
        self.observed.insert(name.to_owned(), raw.clone());
        self.selected_members.insert(name.to_owned(), raw.clone());
        Ok(raw)
    }

    fn source(
        &mut self,
        name: &str,
        cap: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        let allowance = if self.source_reads.contains(name) {
            cap
        } else {
            cap.min(MAX_SELECTED_SOURCE_BYTES.saturating_sub(self.source_bytes))
        };
        let raw = self.authored(name, allowance, deadline, cancelled)?;
        if self.source_reads.insert(name.to_owned()) {
            self.source_bytes = self
                .source_bytes
                .checked_add(raw.len())
                .filter(|bytes| *bytes <= MAX_SELECTED_SOURCE_BYTES)
                .ok_or(Error::Unsupported("Sign PG public source byte budget"))?;
        }
        Ok(raw)
    }

    fn candidate_schema(
        &mut self,
        worker: &mut impl tos_validation::source_cut::CutSchemaExecutor,
        name: &str,
        value: &JsonValue,
        deadline: Instant,
        cancelled: &AtomicBool,
        semantic_dependency: bool,
    ) -> Result<()> {
        let was_dependency = self.source_dependencies.contains(name);
        let result = (|| {
            let raw = self.source(name, 1_048_576, deadline, cancelled)?;
            if worker.selected_contract_digest(name) != Some(Digest256::of_bytes(&raw)) {
                return Err(Error::Conflict("Sign PG candidate schema binding differs"));
            }
            match worker.check(name, &cmd::canonical(value)?, name, deadline, cancelled) {
                Ok(true) => Ok(()),
                Ok(false) => Err(Error::Invalid("Sign PG selected source violates schema")),
                Err(reason) => Err(Error::SchemaExecution {
                    path: name.to_owned(),
                    root: name.to_owned(),
                    reason,
                }),
            }
        })();
        if !semantic_dependency && !was_dependency {
            self.source_dependencies.remove(name);
        }
        result
    }

    fn observe_native_identity(
        &mut self,
        id: &str,
        worker: &mut impl tos_validation::source_cut::CutSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        if self.identity_owners.contains_key(id) {
            return Ok(());
        }
        let token = id.to_owned();
        if self.identity_owners.len() >= MAX_NATIVE_ID_TOKENS {
            return Err(Error::Unsupported("Sign PG native identity query count"));
        }
        let result = self
            .input
            .lookup_annotation_identity_owners(
                &BTreeSet::from([token.clone()]),
                deadline,
                cancelled,
            )
            .map_err(|_| Error::Conflict("Sign PG annotation identity index unavailable"))?;
        let owners = result.get(&token).cloned().unwrap_or_default();
        for owner in &owners {
            let raw = self.source(&owner.path, 1_048_576, deadline, cancelled)?;
            if Digest256::of_bytes(&raw) != owner.member_digest {
                return Err(Error::Conflict("Sign PG annotation owner digest differs"));
            }
            let packet = cmd::parse(&raw)?;
            if cmd::text(&packet, "schema_version")? != "tos_semantic_annotation_packet_v2"
                || !cmd::array(&packet, "entities")?.iter().any(|entity| {
                    entity.object_get("entity_id").and_then(JsonValue::as_str) == Some(id)
                })
            {
                return Err(Error::Conflict(
                    "Sign PG annotation identity index is stale",
                ));
            }
            self.candidate_schema(
                worker,
                "ToS/contracts/semantic-annotation-packet-v2.schema.json",
                &packet,
                deadline,
                cancelled,
                true,
            )?;
        }
        self.identity_owners.insert(token, owners);
        let snapshot = JsonValue::Object(
            self.identity_owners
                .iter()
                .map(|(identity, owners)| {
                    let packets = JsonValue::Object(
                        owners
                            .iter()
                            .map(|owner| {
                                (
                                    JsonString::from_utf8(&owner.path),
                                    cmd::string(&owner.member_digest.to_hex()),
                                )
                            })
                            .collect(),
                    );
                    (JsonString::from_utf8(identity), packets)
                })
                .collect(),
        );
        self.identity_snapshot = Some(crate::source_revisions::python_ascii_digest(&snapshot)?);
        Ok(())
    }

    fn verify_current(&self, deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
        for (path, raw) in &self.selected_members {
            if read_current_input(self.input, path, raw.len(), deadline, cancelled)? != *raw {
                return Err(Error::Conflict("Sign PG selected public source changed"));
            }
        }
        if !self.identity_owners.is_empty() {
            let tokens = self
                .identity_owners
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>();
            let current = self
                .input
                .lookup_annotation_identity_owners(&tokens, deadline, cancelled)
                .map_err(|_| Error::Conflict("Sign PG annotation identity index changed"))?;
            if current != self.identity_owners {
                return Err(Error::Conflict(
                    "Sign PG annotation identity owners changed",
                ));
            }
        }
        if let Some(context) = self.owner_context {
            let snapshot = context.snapshot(deadline, cancelled)?.to_prefixed();
            if self
                .native_identity_owner_context
                .as_ref()
                .is_some_and(|previous| previous != &snapshot)
            {
                return Err(Error::Conflict("Sign PG owner context changed"));
            }
        }
        Ok(())
    }
}

impl SignNativeRead for PgPublicSelection<'_, '_, '_> {
    fn read(
        &mut self,
        reference: &str,
        kind: NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        let relative = RelativePath::parse(reference)
            .map_err(|_| Error::Denied("Sign PG native source path"))?;
        let parts = relative.as_str().split('/').collect::<Vec<_>>();
        let is_content = kind == NativeReadKind::Content;
        let prefix = match kind {
            NativeReadKind::Schema => "ToS/contracts/",
            NativeReadKind::Support => "ToS/",
            _ => "ToS/source-witnesses/",
        };
        if relative.as_str() != reference
            || !reference.starts_with(prefix)
            || reference.starts_with("ToS/source-witnesses/owner-local/") && !is_content
            || parts.contains(&"catalog")
            || !is_content
                && parts
                    .iter()
                    .any(|part| matches!(*part, "payload" | "local-content"))
        {
            return Err(Error::Denied("Sign PG native source namespace"));
        }
        if let Some(previous) = self.native_raws.get(reference) {
            if previous.len() > max_bytes {
                return Err(Error::Invalid("Sign PG native dependency byte budget"));
            }
            return Ok(previous.clone());
        }
        if self.native_reads.len() >= 128 {
            return Err(Error::Unsupported("Sign PG native dependency count"));
        }
        let remaining = (16 * 1_048_576usize).saturating_sub(self.native_bytes);
        let cap = max_bytes.min(remaining);
        let owner_local = match self.owner_context {
            Some(context) => context.owner_local(reference)?,
            None if reference.starts_with("ToS/source-witnesses/owner-local/") => {
                return Err(Error::Denied("Sign PG owner-local context absent"));
            }
            None => false,
        };
        let bytes = if owner_local {
            let context = self
                .owner_context
                .ok_or(Error::Denied("Sign PG owner-local read"))?;
            context.read(reference, cap, deadline, cancelled)?
        } else {
            let bytes = self.authored(reference, cap, deadline, cancelled)?;
            if let Some(context) = self.owner_context {
                context.extend_pg_selected_members(BTreeMap::from([(
                    reference.to_owned(),
                    bytes.clone(),
                )]))?;
            }
            bytes
        };
        self.native_bytes = self
            .native_bytes
            .checked_add(bytes.len())
            .filter(|total| *total <= 16 * 1_048_576)
            .ok_or(Error::Unsupported("Sign PG native byte budget"))?;
        self.native_reads.insert(reference.to_owned());
        self.native_raws.insert(reference.to_owned(), bytes.clone());
        Ok(bytes)
    }

    fn verify_current(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
        PgPublicSelection::verify_current(self, deadline, cancelled)
    }

    fn owner_local(&self, reference: &str) -> Result<bool> {
        match self.owner_context {
            Some(context) => context.owner_local(reference),
            None if reference.starts_with("ToS/source-witnesses/owner-local/") => {
                Err(Error::Denied("Sign PG owner-local context absent"))
            }
            None => Ok(false),
        }
    }

    fn owner_context_snapshot(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<String>> {
        self.owner_context
            .map(|context| {
                context
                    .snapshot(deadline, cancelled)
                    .map(|digest| digest.to_prefixed())
            })
            .transpose()
    }
}

const SIGN_SCHEMA_ROOTS: &[&str] = &[
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/claim-packet.schema.json",
    "ToS/contracts/source-claim-record.schema.json",
    "ToS/contracts/historical-claim.schema.json",
    "ToS/contracts/historical-record.schema.json",
    SIGN_DESCRIPTION_SCHEMA,
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
    "ToS/contracts/knowledge-assessment.schema.json",
    "ToS/contracts/knowledge-assessment-batch.schema.json",
    "ToS/contracts/knowledge-assessment-policy.schema.json",
    "ToS/contracts/knowledge-assessment-authority.schema.json",
    "ToS/contracts/knowledge-assessment-competence.schema.json",
    "ToS/contracts/owner-local-source-context.schema.json",
    "ToS/contracts/native-text-layer-quality-basis.schema.json",
    "ToS/contracts/native-text-unit-assessment-subject.schema.json",
    "ToS/contracts/native-text-unit-binding.schema.json",
    "ToS/contracts/native-text-layer-binding.schema.json",
    "ToS/contracts/source-text-unit-packet-v1.schema.json",
    "ToS/contracts/source-text-layer.schema.json",
    "ToS/contracts/source-anchor-v2.schema.json",
    "ToS/contracts/source-item-manifest.schema.json",
    "ToS/contracts/rights-record.schema.json",
    "ToS/contracts/semantic-annotation-packet-v2.schema.json",
];

const MAX_SELECTED_SOURCE_BYTES: usize = 8 * 1_048_576;

/// Parse the narrow unique candidate key from one exact current Sign member.
/// Streamed bootstrap and package-based commits share this owner parser so a
/// byte-level format drift cannot produce different candidate namespaces.
pub(crate) fn candidate_id_from_current_bytes(
    path: &str,
    raw: &[u8],
) -> SourceCommandResult<String> {
    let relative = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Denied("Sign candidate source path"))?;
    if relative.as_str() != path
        || !path.starts_with("ToS/source-witnesses/")
        || path.starts_with("ToS/source-witnesses/owner-local/")
        || !path.ends_with("/sign.json")
    {
        return Err(SourceCommandError::Denied("Sign candidate source path"));
    }
    let record = cmd::parse(raw)?;
    if cmd::text(&record, "schema_version")? != "tos_sign_description_record_v1"
        || cmd::text(&record, "record_type")? != "sign"
        || !crate::source_revisions::valid_id(cmd::text(&record, "record_id")?, "tos.sign.", false)
    {
        return Err(SourceCommandError::Invalid(
            "Sign candidate index owner profile",
        ));
    }
    let candidate_id = cmd::text(
        cmd::field(cmd::field(&record, "promotion_basis")?, "candidate")?,
        "id",
    )?;
    if !crate::source_revisions::valid_id(candidate_id, "tos.claim.", false) {
        return Err(SourceCommandError::Invalid("Sign candidate index identity"));
    }
    Ok(candidate_id.to_owned())
}

/// Exact bounded output-identity probes captured by the selected PG input.
/// The source readset retains each queried index generation through commit.
#[derive(Clone, Debug)]
pub(crate) struct PgSignCreationIdentityObservations {
    pub(crate) sign_id: String,
    pub(crate) annotation_identity_owners:
        BTreeSet<crate::source_annotation_identity_index::AnnotationEntityOwner>,
    pub(crate) candidate_id: String,
    pub(crate) prior_candidate_path: Option<RelativePath>,
}

pub(crate) fn observe_sign_creation_absences(
    input: &PgGrowthCurrentInput<'_, '_>,
    sign_id: &str,
    candidate_id: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PgSignCreationIdentityObservations> {
    if !crate::source_revisions::valid_id(sign_id, "tos.sign.", false)
        || !crate::source_revisions::valid_id(candidate_id, "tos.claim.", false)
    {
        return Err(Error::Invalid("Sign PG output identity token"));
    }
    let tokens = BTreeSet::from([sign_id.to_owned()]);
    let owners = input
        .lookup_annotation_identity_owners(&tokens, deadline, cancelled)
        .map_err(|_| Error::Conflict("Sign PG annotation identity index changed"))?;
    let annotation_identity_owners = owners
        .get(sign_id)
        .cloned()
        .ok_or(Error::Conflict("Sign PG annotation identity key absent"))?;
    if !annotation_identity_owners.is_empty() {
        return Err(Error::Conflict(
            "Sign PG semantic annotation identity already owned",
        ));
    }
    let prior_candidate_path = input
        .lookup_sign_promotion_candidate(candidate_id, deadline, cancelled)
        .map_err(|_| Error::Conflict("Sign PG candidate index changed"))?;
    if prior_candidate_path.is_some() {
        return Err(Error::Conflict(
            "Sign PG promotion candidate already issued",
        ));
    }
    Ok(PgSignCreationIdentityObservations {
        sign_id: sign_id.to_owned(),
        annotation_identity_owners,
        candidate_id: candidate_id.to_owned(),
        prior_candidate_path,
    })
}

/// The exact immutable assessment decision already stored inside a ready Sign
/// package. A PG retry may validate these typed facts under the held current
/// assessment-history guard without creating a fresh Sign proposal.
#[derive(Clone, Debug)]
pub(crate) struct PgSignRecoverableBasis {
    pub(crate) candidate: JsonValue,
    pub(crate) policy: JsonValue,
    pub(crate) required_sources: Vec<JsonValue>,
    pub(crate) assessment_refs: Vec<JsonValue>,
    pub(crate) owner_snapshot: Digest256,
    pub(crate) journal_revision: Digest256,
    pub(crate) status: String,
    pub(crate) limits: JsonValue,
    pub(crate) basis_digest: Digest256,
}

fn validate_sign_basis_ref(value: &JsonValue, candidate: bool) -> Result<()> {
    cmd::exact_keys(value, &["id", "version", "digest"])?;
    let id = cmd::text(value, "id")?;
    if id.is_empty()
        || tos_foundation::python_strip_unicode16_v1(id, 1_048_576)
            .map_err(|_| Error::Invalid("Sign PG basis reference budget"))?
            .is_empty()
        || cmd::integer(value, "version")? == 0
    {
        return Err(Error::Invalid("Sign PG basis reference identity"));
    }
    if candidate && !crate::source_revisions::valid_id(id, "tos.claim.", false) {
        return Err(Error::Invalid("Sign PG basis candidate identity"));
    }
    Digest256::from_prefixed(cmd::text(value, "digest")?)
        .map_err(|_| Error::Invalid("Sign PG basis reference digest"))?;
    Ok(())
}

impl PgSignRecoverableBasis {
    pub(crate) fn from_writes(writes: &[SourceRevisionWrite]) -> Result<Self> {
        let sign_writes = writes
            .iter()
            .filter(|write| write.subject.ends_with("/sign.json"))
            .collect::<Vec<_>>();
        let sign_write = match sign_writes.as_slice() {
            [write] => *write,
            _ => {
                return Err(Error::Invalid(
                    "Sign PG recoverable package sign.json count",
                ));
            }
        };
        let candidate_id =
            candidate_id_from_current_bytes(&sign_write.subject, &sign_write.exact_bytes)?;
        let record = cmd::parse(&sign_write.exact_bytes)?;
        let basis = cmd::field(&record, "promotion_basis")?;
        cmd::exact_keys(
            basis,
            &[
                "schema_version",
                "candidate",
                "policy",
                "required_sources",
                "assessment_refs",
                "owner_snapshot",
                "journal_revision",
                "status",
                "use",
                "limits",
                "grants_current_use",
            ],
        )?;
        if cmd::text(basis, "schema_version")? != "tos_sign_promotion_basis_v1"
            || cmd::text(basis, "use")? != "sign-promotion"
            || cmd::field(basis, "grants_current_use")? != &JsonValue::Bool(false)
        {
            return Err(Error::Invalid("Sign PG recoverable basis profile"));
        }
        let candidate = cmd::field(basis, "candidate")?.clone();
        validate_sign_basis_ref(&candidate, true)?;
        if cmd::text(&candidate, "id")? != candidate_id {
            return Err(Error::Conflict("Sign PG candidate index and basis differ"));
        }
        let policy = cmd::field(basis, "policy")?.clone();
        validate_sign_basis_ref(&policy, false)?;
        let required_sources = cmd::array(basis, "required_sources")?.to_vec();
        let assessment_refs = cmd::array(basis, "assessment_refs")?.to_vec();
        if required_sources.is_empty() || assessment_refs.is_empty() {
            return Err(Error::Invalid(
                "Sign PG recoverable basis references absent",
            ));
        }
        for reference in &required_sources {
            validate_sign_basis_ref(reference, false)?;
        }
        for reference in &assessment_refs {
            validate_sign_basis_ref(reference, false)?;
        }
        for refs in [&required_sources, &assessment_refs] {
            let mut seen = BTreeSet::new();
            for reference in refs {
                if !seen.insert(cmd::canonical(reference)?) {
                    return Err(Error::Invalid(
                        "Sign PG recoverable basis reference duplicate",
                    ));
                }
            }
        }
        let owner_snapshot = Digest256::from_prefixed(cmd::text(basis, "owner_snapshot")?)
            .map_err(|_| Error::Invalid("Sign PG recoverable owner snapshot digest"))?;
        let journal_revision = Digest256::from_hex(cmd::text(basis, "journal_revision")?)
            .map_err(|_| Error::Invalid("Sign PG recoverable journal revision"))?;
        let status = cmd::text(basis, "status")?.to_owned();
        if !matches!(status.as_str(), "admitted" | "admitted-with-limits") {
            return Err(Error::Invalid("Sign PG recoverable basis admission status"));
        }
        let limits = cmd::field(basis, "limits")?.clone();
        if limits.as_object().is_none() {
            return Err(Error::Invalid("Sign PG recoverable basis limits"));
        }
        Ok(Self {
            candidate,
            policy,
            required_sources,
            assessment_refs,
            owner_snapshot,
            journal_revision,
            status,
            limits,
            basis_digest: cmd::record_digest(basis)?,
        })
    }
}

/// Produce the maintained Sign describe/prepare view from the Assessment
/// report computed under the caller's selected PG history guard. This helper
/// creates no source package or admission authority.
pub(crate) fn promotion_view_from_report(
    candidate_ref: &JsonValue,
    required_sources: &[JsonValue],
    owner_snapshot: Digest256,
    history: &crate::source_assessment_journal::AssessmentHistory,
    report: &tos_validation::assessment::AssessmentMechanicsReport,
    require_ready: bool,
) -> Result<JsonValue> {
    let admission = super::decoded(report.current_admission())?;
    if !cmd::same(cmd::field(&admission, "subject")?, candidate_ref)? {
        return Err(Error::Conflict("Sign PG evaluated subject differs"));
    }
    let report_sources = report
        .required_sources()
        .iter()
        .map(super::decoded)
        .collect::<Result<Vec<_>>>()?;
    if !cmd::same(
        &JsonValue::Array(report_sources.clone()),
        &JsonValue::Array(required_sources.to_vec()),
    )? {
        return Err(Error::Conflict("Sign PG evaluated source closure differs"));
    }
    let ready = cmd::text(&admission, "use")? == "sign-promotion"
        && admission.object_get("can_use") == Some(&JsonValue::Bool(true))
        && matches!(
            cmd::text(&admission, "status")?,
            "admitted" | "admitted-with-limits"
        )
        && !cmd::array(&admission, "assessment_refs")?.is_empty()
        && history.head.is_some()
        && !report_sources.is_empty()
        && report.source_read_required()
        && report.source_read_ready();
    if require_ready && !ready {
        return Err(Error::Denied(
            "Sign PG current qualified assessment and exact source reading required",
        ));
    }
    let basis = if ready {
        let journal_revision = history
            .head
            .as_ref()
            .ok_or(Error::Invalid("Sign PG Assessment history head absent"))?;
        Digest256::from_hex(journal_revision)
            .map_err(|_| Error::Invalid("Sign PG Assessment history revision"))?;
        cmd::object(vec![
            ("schema_version", cmd::string("tos_sign_promotion_basis_v1")),
            ("candidate", candidate_ref.clone()),
            ("policy", cmd::field(&admission, "policy")?.clone()),
            ("required_sources", JsonValue::Array(report_sources)),
            (
                "assessment_refs",
                cmd::field(&admission, "assessment_refs")?.clone(),
            ),
            ("owner_snapshot", cmd::string(&owner_snapshot.to_prefixed())),
            ("journal_revision", cmd::string(journal_revision)),
            ("status", cmd::field(&admission, "status")?.clone()),
            ("use", cmd::string("sign-promotion")),
            ("limits", cmd::field(&admission, "limits")?.clone()),
            ("grants_current_use", JsonValue::Bool(false)),
        ])
    } else {
        JsonValue::Null
    };
    Ok(cmd::object(vec![
        ("eligible", JsonValue::Bool(ready)),
        ("basis", basis),
        ("current_admission", admission),
        ("grants_issuance_authority", JsonValue::Bool(false)),
    ]))
}

/// Evaluate the maintained source-neutral Assessment plan against histories
/// already selected under the caller's PG guard, then project the exact
/// Sign-promotion view. The caller owns private source selection and the
/// held-history callback; this helper keeps the Sign subject and opaque PG
/// identity bound while reusing the shared Assessment evaluator.
pub(crate) fn evaluate_promotion_view_selected_pg(
    plan: &assessment::AssessmentEvaluationPlan<'_>,
    histories: &BTreeMap<String, crate::source_assessment_journal::AssessmentHistory>,
    public_sources: &PgOwnerAssessmentPublicSources,
    expected_candidate_id: &str,
    observed_now: &str,
    identity: PgRecordRevisionIdentity,
    worker: &mut tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<
        PgRecordRevisionIdentity,
    >,
    budget: &mut assessment::OperationBudget,
    require_ready: bool,
    cancelled: &AtomicBool,
) -> Result<(JsonValue, assessment::AssessmentEvaluationResult)> {
    let candidate_id = plan.request.subject_id.as_str();
    if candidate_id != expected_candidate_id
        || worker.input_identity() != &identity
        || plan.source_route != tos_validation::assessment::AssessmentSourceRoute::SourceBoundClaim
    {
        return Err(Error::Conflict(
            "Sign PG Assessment candidate identity or route differs",
        ));
    }
    let candidate_ref = public_sources.candidate_ref(candidate_id)?;
    let required_sources = public_sources.required_sources(candidate_id)?;
    let mut evaluate =
        |input: &tos_validation::assessment::AssessmentReadInput,
         worker: &mut tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<
            PgRecordRevisionIdentity,
        >,
         limits: tos_validation::assessment::AssessmentLimits,
         cancelled: &AtomicBool| {
            tos_validation::assessment::evaluate_current_assessment_selected(
                input,
                &identity,
                identity.digest(),
                worker,
                limits,
                cancelled,
            )
            .map_err(map_selected_assessment_refusal)
        };
    let evaluation = assessment::evaluate_assessment_histories(
        plan,
        histories,
        observed_now,
        None,
        identity.digest(),
        worker,
        budget,
        cancelled,
        &mut evaluate,
    )?;
    let history = histories.get(candidate_id).ok_or(Error::Conflict(
        "Sign PG selected Assessment history absent",
    ))?;
    let view = promotion_view_from_report(
        &candidate_ref,
        &required_sources,
        public_sources.owner_snapshot,
        history,
        &evaluation.current_report,
        require_ready,
    )?;
    Ok((view, evaluation))
}

fn map_selected_assessment_refusal(
    refusal: tos_validation::assessment::AssessmentRefusal,
) -> Error {
    use tos_validation::assessment::AssessmentRefusal;
    match refusal {
        AssessmentRefusal::InvalidInput(_) => Error::Invalid("Sign PG Assessment input"),
        AssessmentRefusal::Budget
        | AssessmentRefusal::Schema(
            tos_validation::item_rules::ItemRefusal::Budget
            | tos_validation::item_rules::ItemRefusal::BudgetCheck { .. },
        ) => Error::Unsupported("Sign PG Assessment budget"),
        AssessmentRefusal::Cancelled
        | AssessmentRefusal::Deadline
        | AssessmentRefusal::Schema(tos_validation::item_rules::ItemRefusal::Deadline) => {
            Error::Denied("Sign PG Assessment cancelled or expired")
        }
        AssessmentRefusal::Unsupported(_)
        | AssessmentRefusal::Schema(
            tos_validation::item_rules::ItemRefusal::Source(_)
            | tos_validation::item_rules::ItemRefusal::Unsupported(_)
            | tos_validation::item_rules::ItemRefusal::Executor(_),
        ) => Error::Unsupported("Sign PG Assessment schema or execution"),
    }
}

/// Revalidate a retained ready basis under the current selected PG source
/// snapshot and the exact Assessment history head. This deliberately avoids
/// reevaluation: a recoverable Sign retry reuses its stored decision bytes.
pub(crate) fn verify_recoverable_basis_current(
    basis: &PgSignRecoverableBasis,
    history: &crate::source_assessment_journal::AssessmentHistory,
    public_sources: &PgOwnerAssessmentPublicSources,
    assessment_configuration: &JsonValue,
) -> Result<()> {
    let candidate_id = cmd::text(&basis.candidate, "id")?;
    let candidate_ref = public_sources.candidate_ref(candidate_id)?;
    let required_sources = public_sources.required_sources(candidate_id)?;
    let policy = owner_policy_ref(cmd::field(assessment_configuration, "policy")?)?;
    let current_head = history.head.as_deref().ok_or(Error::Conflict(
        "Sign PG recoverable Assessment history absent",
    ))?;
    let current_head = Digest256::from_hex(current_head)
        .map_err(|_| Error::Invalid("Sign PG recoverable Assessment head"))?;
    if !history.contains_exact_assessment_refs(&basis.assessment_refs)?
        || current_head != basis.journal_revision
        || basis.owner_snapshot != public_sources.owner_snapshot
        || !cmd::same(&basis.candidate, &candidate_ref)?
        || !cmd::same(&basis.policy, &policy)?
        || !cmd::same(
            &JsonValue::Array(basis.required_sources.clone()),
            &JsonValue::Array(required_sources),
        )?
    {
        return Err(Error::Conflict(
            "Sign PG retained ready basis no longer matches current source or Assessment head",
        ));
    }
    Ok(())
}

fn owner_policy_ref(value: &JsonValue) -> Result<JsonValue> {
    let mut keys = vec!["id", "version", "payload"];
    if value.object_get("origin_id").is_some() {
        keys.push("origin_id");
    }
    cmd::exact_keys(value, &keys)?;
    Ok(cmd::object(vec![
        ("id", cmd::field(value, "id")?.clone()),
        ("version", cmd::field(value, "version")?.clone()),
        (
            "digest",
            cmd::string(&cmd::record_digest(cmd::field(value, "payload")?)?.to_prefixed()),
        ),
    ]))
}

/// Determine the asserted schema closure from the Sign owner's exact selected
/// source carriers and the maintained v2-v6 Assessment/native route. No whole
/// corpus schema enumeration is used; only configured source paths and their
/// declared current profile routes enter the closure.
pub(crate) fn candidate_schema_paths_from_config(
    readset: &mut impl crate::source_revisions::ReadonlyRecordFiles,
    config: &JsonValue,
    deadline: std::time::Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> SourceCommandResult<Vec<String>> {
    let entities_raw = readset.read(super::ENTITIES, 1_048_576, deadline, cancelled)?;
    let relations_raw = readset.read(super::RELATIONS, 1_048_576, deadline, cancelled)?;
    let entities = cmd::parse(&entities_raw)?;
    let relations = cmd::parse(&relations_raw)?;
    let mut roots = SIGN_SCHEMA_ROOTS
        .iter()
        .map(|root| (*root).to_owned())
        .collect::<BTreeSet<_>>();
    let mut selected_source_bytes =
        entities_raw
            .len()
            .checked_add(relations_raw.len())
            .ok_or(SourceCommandError::Invalid(
                "Sign schema source byte overflow",
            ))?;

    let bindings = cmd::array(config, "source_records")?;
    if bindings.len() > 1024 {
        return Err(SourceCommandError::Invalid(
            "Sign configured source selection count",
        ));
    }
    let mut paths = BTreeSet::new();
    for binding in bindings {
        cmd::exact_keys(binding, &["path", "record_id", "origin_id"])?;
        let path = cmd::text(binding, "path")?;
        let relative = RelativePath::parse(path)
            .map_err(|_| SourceCommandError::Denied("Sign selected source path"))?;
        let parts = relative.as_str().split('/').collect::<Vec<_>>();
        if !path.starts_with("ToS/source-witnesses/")
            || path.starts_with("ToS/source-witnesses/owner-local/")
            || parts
                .iter()
                .any(|part| matches!(*part, "payload" | "local-content" | "catalog"))
            || !path.ends_with(".json") && !path.ends_with(".jsonl")
        {
            return Err(SourceCommandError::Denied(
                "Sign public selected metadata carrier",
            ));
        }
        paths.insert(path.to_owned());
    }

    for path in paths {
        let raw = readset.read(&path, 8 * 1_048_576, deadline, cancelled)?;
        selected_source_bytes = selected_source_bytes
            .checked_add(raw.len())
            .filter(|bytes| *bytes <= MAX_SELECTED_SOURCE_BYTES)
            .ok_or(SourceCommandError::Unsupported(
                "Sign selected source schema discovery byte budget",
            ))?;
        let values = selected_carrier_values(&path, &raw)?;
        for value in values {
            let schema_version = value
                .object_get("schema_version")
                .and_then(JsonValue::as_str)
                .unwrap_or_default();
            match schema_version {
                "tos_historical_claim_v1" => {
                    roots.insert("ToS/contracts/historical-claim.schema.json".into());
                }
                "tos_human_form_v1" => {
                    roots.insert("ToS/contracts/human-form.schema.json".into());
                }
                "tos_corpus_record_v1" => {
                    roots.insert("ToS/contracts/corpus-record.schema.json".into());
                }
                "tos_historical_record_v1" => {
                    roots.insert("ToS/contracts/historical-record.schema.json".into());
                }
                _ if value.object_get("claim_id").is_some()
                    && path.ends_with("/source-claims.jsonl") =>
                {
                    append_relation_schema_roots(&relations, &value, &mut roots)?;
                }
                _ if value.object_get("record_type").is_some() => {
                    append_entity_schema_roots(&entities, &value, &mut roots)?;
                }
                _ => (),
            }
            if roots.len() > tos_validation::SchemaBackendProbe::MAX_RESOURCES {
                return Err(SourceCommandError::Unsupported(
                    "Sign selected schema root budget",
                ));
            }
        }
    }

    let roots = roots.into_iter().collect::<Vec<_>>();
    let borrowed = roots.iter().map(String::as_str).collect::<Vec<_>>();
    let closure = crate::source_revisions::collect_readonly_schema_files(
        readset, &borrowed, deadline, cancelled,
    )?;
    if closure.is_empty() || closure.len() > tos_validation::SchemaBackendProbe::MAX_RESOURCES {
        return Err(SourceCommandError::Unsupported(
            "Sign selected schema closure budget",
        ));
    }
    Ok(closure
        .into_iter()
        .map(|file| file.path.as_str().to_owned())
        .collect())
}

fn record_read<I: Copy + Eq>(
    reader: &PgPublicSelection<'_, '_, '_>,
    identity: I,
    path: &str,
) -> Result<CandidateProfileRead<I>> {
    let files = reader
        .observed
        .iter()
        .map(|(name, raw)| {
            Ok(crate::source_command::SourceFile {
                path: RelativePath::parse(name)
                    .map_err(|_| Error::Invalid("Sign PG selected context path"))?,
                raw: raw.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(CandidateProfileRead {
        identity,
        files,
        schema_source_path: path.to_owned(),
    })
}

fn validate_profile_pg(
    reader: &mut PgPublicSelection<'_, '_, '_>,
    input: &PgGrowthCurrentInput<'_, '_>,
    worker: &mut tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<
        PgRecordRevisionIdentity,
    >,
    name: &str,
    body: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    for path in [
        ENTITIES,
        "ToS/contracts/semantic-entity-type-registry.schema.json",
        "ToS/contracts/corpus-record.schema.json",
    ] {
        reader.source_dependencies.insert(path.to_owned());
        reader.source(path, 1_048_576, deadline, cancelled)?;
    }
    let identity = *input.input_identity();
    let ctx = record_read(reader, identity, name)?;
    let registry = crate::source_revisions::validate_source_profile_registry_candidate(
        worker, deadline, cancelled, &ctx,
    )?;
    let kind = cmd::text(body, "record_type")?;
    let entries = cmd::array(&registry, "types")?
        .iter()
        .filter(|entry| {
            entry
                .object_get("source_record_profile")
                .and_then(|profile| profile.object_get("record_type"))
                .and_then(JsonValue::as_str)
                == Some(kind)
        })
        .collect::<Vec<_>>();
    if entries.len() != 1 {
        return Err(Error::Unsupported(
            "Sign PG selected metadata profile route",
        ));
    }
    let entry = entries[0];
    let profile = cmd::field(entry, "source_record_profile")?;
    let source_path = name;
    let (_, basename) = source_path
        .rsplit_once('/')
        .ok_or(Error::Invalid("Sign PG selected metadata basename"))?;
    if basename != cmd::text(profile, "source_basename")?
        || cmd::text(body, "record_type")? != kind
        || !crate::source_revisions::valid_id(
            cmd::text(body, "record_id")?,
            cmd::text(profile, "id_prefix")?,
            false,
        )
        || !matches!(
            cmd::text(body, "visibility")?,
            "public" | "public_metadata_only"
        )
        || !matches!(
            cmd::text(body, "identity_status")?,
            "provisional" | "verified" | "disputed" | "superseded"
        )
        || tos_foundation::python_strip_unicode16_v1(cmd::text(body, "preferred_label")?, 1_048_576)
            .map_err(|_| Error::Invalid("Sign PG preferred label budget"))?
            .is_empty()
        || cmd::integer(body, "record_version")? == 0
    {
        return Err(Error::Denied("Sign PG public profile identity or metadata"));
    }
    if kind == "composite"
        && (!source_path.starts_with("ToS/source-witnesses/scholarly-composites/")
            || source_path.split('/').count() < 7)
    {
        return Err(Error::Denied("Sign PG retained composite profile path"));
    }

    if let Some(binding) = body.object_get("native_text_binding") {
        if cmd::text(profile, "native_binding_adapter").ok() != Some("source-text-unit-v1") {
            return Err(Error::Denied(
                "Sign PG profile has no native binding adapter",
            ));
        }
        let resolved = crate::source_sign_native::resolve_binding(
            reader,
            worker,
            binding,
            NativeReadScope::MetadataOnly,
            deadline,
            cancelled,
        )?;
        for input in resolved.inputs {
            let raw = reader.source(&input.reference, 1_048_576, deadline, cancelled)?;
            if Digest256::of_bytes(&raw) != input.raw_sha256 {
                return Err(Error::Conflict("Sign PG profile native snapshot differs"));
            }
            let key = (input.reference, input.category);
            if reader
                .profile_native_inputs
                .insert(key, input.raw_sha256)
                .is_some_and(|previous| previous != input.raw_sha256)
            {
                return Err(Error::Conflict("Sign PG profile native dependency changed"));
            }
        }
        reader
            .source_dependencies
            .extend(resolved.schema_digests.into_keys());
    } else if profile.object_get("native_binding_adapter").is_some() {
        return Err(Error::Invalid("Sign PG native profile binding absent"));
    }

    if ["occurrence", "lexeme", "sense", "sign", "concept"].contains(&kind) {
        reader.observe_native_identity(
            cmd::text(body, "record_id")?,
            worker,
            deadline,
            cancelled,
        )?;
    }

    let route = cmd::array(profile, "schemas")?
        .iter()
        .filter(|route| route.object_get("schema_version") == body.object_get("schema_version"))
        .collect::<Vec<_>>();
    if route.len() != 1 {
        return Err(Error::Unsupported(
            "Sign PG exact source profile schema route",
        ));
    }
    let route = route[0];
    let root = cmd::text(route, "schema_ref")?;
    let mut resources = vec!["ToS/contracts/corpus-record.schema.json".to_owned()];
    for dependency in cmd::array(route, "schema_dependencies")? {
        resources.push(
            dependency
                .as_str()
                .ok_or(Error::Invalid("Sign PG profile schema dependency"))?
                .to_owned(),
        );
    }
    resources.push(root.to_owned());
    let shared = "ToS/contracts/source-metadata-record.schema.json";
    if !resources.iter().any(|resource| resource == shared) {
        return Err(Error::Unsupported(
            "Sign PG profile shared metadata contract absent",
        ));
    }
    resources.insert(
        0,
        "ToS/contracts/semantic-entity-type-registry.schema.json".into(),
    );
    resources.insert(0, ENTITIES.into());
    let mut seen = BTreeSet::new();
    resources.retain(|path| seen.insert(path.clone()));
    if resources.len() > 128 {
        return Err(Error::Unsupported("Sign PG profile contract count"));
    }
    for path in &resources {
        reader.source_dependencies.insert(path.clone());
        let raw = reader.source(path, 1_048_576, deadline, cancelled)?;
        if worker.selected_contract_digest(path) != Some(Digest256::of_bytes(&raw)) {
            return Err(Error::Conflict("Sign PG profile worker resource differs"));
        }
    }
    let ctx = record_read(reader, identity, name)?;
    crate::source_revisions::schema_candidate(
        worker, deadline, cancelled, &ctx, &resources, root, body,
    )?;
    crate::source_revisions::schema_candidate(
        worker, deadline, cancelled, &ctx, &resources, shared, body,
    )?;
    Ok(())
}

fn local_claim_pg(
    reader: &mut PgPublicSelection<'_, '_, '_>,
    input: &PgGrowthCurrentInput<'_, '_>,
    worker: &mut tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<
        PgRecordRevisionIdentity,
    >,
    claim: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    let raw = cmd::canonical(claim)?;
    let report = tos_validation::record_rules::validate_source_claim_from_asserted_selected_input(
        input,
        &raw,
        worker,
        tos_validation::item_rules::ItemLimits {
            max_member_bytes: 1_048_576,
            max_total_bytes: 8 * 1_048_576,
            max_state_bytes: 8 * 1_048_576,
            max_issues: 128,
            deadline,
        },
        cancelled,
    )
    .map_err(|_| Error::Unsupported("Sign PG source Claim local profile execution"))?;
    if report.input_identity != *input.input_identity()
        || report.source_input_sha256 != Digest256::of_bytes(&raw)
        || report.dependency_digests.len() > 128
        || !report.issues.is_empty()
    {
        return Err(Error::Invalid("Sign PG selected Claim local profile"));
    }
    for (name, digest) in report.dependency_digests {
        reader.source_dependencies.insert(name.clone());
        let raw = reader.source(&name, 1_048_576, deadline, cancelled)?;
        if Digest256::of_bytes(&raw) != digest {
            return Err(Error::Conflict("Sign PG Claim selected resource digest"));
        }
    }
    Ok(())
}

fn source_envelopes_pg(
    reader: &mut PgPublicSelection<'_, '_, '_>,
    input: &PgGrowthCurrentInput<'_, '_>,
    config: &JsonValue,
    worker: &mut tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<
        PgRecordRevisionIdentity,
    >,
    deadline: Instant,
    cancelled: &AtomicBool,
    generic_schema_dependencies: bool,
) -> Result<Vec<JsonValue>> {
    let bindings = cmd::array(config, "source_records")?;
    if bindings.len() > 1024 {
        return Err(Error::Invalid("Sign PG configured source selection count"));
    }
    let mut selected_ids = BTreeSet::new();
    let mut files = BTreeMap::<String, BTreeMap<String, JsonValue>>::new();
    let mut rows = Vec::new();
    for binding in bindings {
        active(deadline, cancelled)?;
        cmd::exact_keys(binding, &["path", "record_id", "origin_id"])?;
        let id = cmd::text(binding, "record_id")?;
        if id.is_empty() || !selected_ids.insert(id) {
            return Err(Error::Invalid("Sign PG distinct selected source identity"));
        }
        let name = cmd::text(binding, "path")?;
        let path =
            RelativePath::parse(name).map_err(|_| Error::Denied("Sign PG selected source path"))?;
        let parts = path.as_str().split('/').collect::<Vec<_>>();
        if path.as_str() != name
            || !name.starts_with("ToS/source-witnesses/")
            || name.starts_with("ToS/source-witnesses/owner-local/")
            || parts
                .iter()
                .any(|part| matches!(*part, "payload" | "local-content" | "catalog"))
            || !name.ends_with(".json") && !name.ends_with(".jsonl")
        {
            return Err(Error::Denied("Sign PG public selected metadata carrier"));
        }
        if !files.contains_key(name) {
            reader.source_carriers.push(name.to_owned());
            let raw = reader.source(name, 8 * 1_048_576, deadline, cancelled)?;
            let requested = bindings
                .iter()
                .filter(|candidate| {
                    candidate.object_get("path").and_then(JsonValue::as_str) == Some(name)
                })
                .filter_map(|candidate| {
                    candidate
                        .object_get("record_id")
                        .and_then(JsonValue::as_str)
                })
                .collect::<BTreeSet<_>>();
            let mut seen = BTreeSet::new();
            let mut indexed = BTreeMap::new();
            for value in selected_carrier_values(name, &raw)? {
                active(deadline, cancelled)?;
                let identity = match value
                    .object_get("schema_version")
                    .and_then(JsonValue::as_str)
                {
                    Some("tos_claim_packet_v1" | "tos_historical_claim_v1") => {
                        Some(("claim_id", "claim_version"))
                    }
                    Some("tos_corpus_record_v1" | "tos_historical_record_v1") => {
                        Some(("record_id", "record_version"))
                    }
                    Some("tos_human_form_v1") => Some(("form_id", "form_version")),
                    _ if parts.last() == Some(&"source-claims.jsonl")
                        && value.object_get("claim_id").is_some() =>
                    {
                        Some(("claim_id", "claim_version"))
                    }
                    _ if value
                        .object_get("record_type")
                        .and_then(JsonValue::as_str)
                        .is_some_and(|kind| {
                            parts
                                .last()
                                .is_some_and(|leaf| **leaf == format!("{kind}.json"))
                        }) =>
                    {
                        Some(("record_id", "record_version"))
                    }
                    _ => None,
                };
                let Some((key, version)) = identity else {
                    continue;
                };
                if value.object_get("claim_id").is_some() {
                    if parts.last() == Some(&"source-claims.jsonl") {
                        reader.source(RELATIONS, 1_048_576, deadline, cancelled)?;
                        let relations = cmd::parse(
                            reader
                                .observed
                                .get(RELATIONS)
                                .ok_or(Error::Conflict("Sign PG relation registry absent"))?,
                        )?;
                        let known = cmd::array(&relations, "relations")?.iter().any(|relation| {
                            relation
                                .object_get("source_mappings")
                                .and_then(JsonValue::as_array)
                                .is_some_and(|mappings| {
                                    mappings.iter().any(|mapping| {
                                        mapping
                                            .object_get("source_graph")
                                            .and_then(JsonValue::as_str)
                                            == Some("source-claims")
                                            && mapping
                                                .object_get("scope")
                                                .and_then(JsonValue::as_str)
                                                == Some("claim-predicate")
                                            && mapping.object_get("source_predicate_id")
                                                == value.object_get("predicate")
                                    })
                                })
                                && relation
                                    .object_get("source_claim_profile")
                                    .and_then(|profile| profile.object_get("schemas"))
                                    .and_then(JsonValue::as_array)
                                    .is_some_and(|routes| {
                                        routes.iter().any(|route| {
                                            route.object_get("schema_version")
                                                == value.object_get("schema_version")
                                        })
                                    })
                        });
                        if !known {
                            continue;
                        }
                        local_claim_pg(reader, input, worker, &value, deadline, cancelled)?;
                    } else if cmd::text(&value, "schema_version")? == "tos_historical_claim_v1" {
                        reader.candidate_schema(
                            worker,
                            "ToS/contracts/historical-claim.schema.json",
                            &value,
                            deadline,
                            cancelled,
                            true,
                        )?;
                    } else if cmd::text(&value, "schema_version")? != "tos_claim_packet_v1" {
                        continue;
                    }
                } else if value.object_get("form_id").is_some() {
                    reader.candidate_schema(
                        worker,
                        "ToS/contracts/human-form.schema.json",
                        &value,
                        deadline,
                        cancelled,
                        generic_schema_dependencies,
                    )?;
                } else if cmd::text(&value, "schema_version")? == "tos_corpus_record_v1" {
                    reader.candidate_schema(
                        worker,
                        "ToS/contracts/corpus-record.schema.json",
                        &value,
                        deadline,
                        cancelled,
                        generic_schema_dependencies,
                    )?;
                } else if cmd::text(&value, "schema_version")? == "tos_historical_record_v1" {
                    reader.candidate_schema(
                        worker,
                        "ToS/contracts/historical-record.schema.json",
                        &value,
                        deadline,
                        cancelled,
                        true,
                    )?;
                } else {
                    validate_profile_pg(reader, input, worker, name, &value, deadline, cancelled)?;
                }
                let identity = cmd::text(&value, key)?.to_owned();
                if tos_foundation::python_strip_unicode16_v1(&identity, 1_048_576)
                    .map_err(|_| Error::Invalid("Sign PG source identity budget"))?
                    .is_empty()
                    || cmd::canonical(&value)?.len() > 1_048_576
                    || cmd::integer(&value, version)? == 0
                    || !seen.insert(identity.clone())
                {
                    return Err(Error::Invalid("Sign PG current source duplicate/version"));
                }
                if !name.ends_with(".jsonl") || requested.contains(identity.as_str()) {
                    indexed.insert(identity, value);
                }
            }
            files.insert(name.to_owned(), indexed);
        }
        let body = files[name].get(id).ok_or(Error::Invalid(
            "Sign PG selected record absent or unsupported",
        ))?;
        if body.object_get("claim_id").is_some() || body.object_get("visibility").is_some() {
            if !matches!(
                cmd::text(body, "visibility")?,
                "public" | "public_metadata_only"
            ) {
                return Err(Error::Denied("Sign PG source metadata visibility"));
            }
        }
        let (key, version) = if body.object_get("claim_id").is_some() {
            ("claim_id", "claim_version")
        } else if body.object_get("form_id").is_some() {
            ("form_id", "form_version")
        } else {
            ("record_id", "record_version")
        };
        rows.push(super::envelope(
            body,
            key,
            version,
            cmd::field(binding, "origin_id")?,
        )?);
    }
    Ok(rows)
}

fn native_envelopes_pg(
    reader: &mut PgPublicSelection<'_, '_, '_>,
    config: &JsonValue,
    worker: &mut tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<
        PgRecordRevisionIdentity,
    >,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<super::NativeSelection> {
    let Some(selections) = config.object_get("native_text_units") else {
        return Ok(super::NativeSelection {
            rows: Vec::new(),
            summaries: Vec::new(),
            snapshots: Vec::new(),
            contracts: BTreeMap::new(),
        });
    };
    let selections = selections
        .as_array()
        .ok_or(Error::Invalid("Sign PG native selections list"))?;
    if selections.len() > 64 {
        return Err(Error::Invalid("Sign PG native selection budget"));
    }
    let subjects = cmd::field(config, "subjects")?;
    let mut selected = BTreeSet::new();
    // Verify every explicit grant before resolving a binding that could read
    // content. The resolver independently checks the exact source binding.
    for selection in selections {
        active(deadline, cancelled)?;
        cmd::exact_keys(selection, &["binding", "origin_id", "read_scope"])?;
        let id = cmd::text(cmd::field(selection, "binding")?, "unit_id")?;
        if !selected.insert(id) {
            return Err(Error::Invalid("Sign PG repeated native selected unit"));
        }
        if !matches!(
            cmd::text(selection, "read_scope")?,
            "metadata_only" | "exact_public" | "exact_owner_local"
        ) {
            return Err(Error::Invalid("Sign PG native read scope"));
        }
        let scope = cmd::field(subjects, id)?;
        cmd::exact_keys(
            scope,
            &[
                "record",
                "assertion_layer",
                "risk",
                "languages",
                "maker_id",
                "requested_use",
                "access_allowed",
            ],
        )?;
        if scope.object_get("access_allowed") != Some(&JsonValue::Bool(true)) {
            return Err(Error::Denied("Sign PG native unit owner access"));
        }
        if cmd::text(selection, "read_scope")? == "exact_owner_local"
            && reader.owner_context.is_none()
        {
            return Err(Error::Denied("Sign PG owner-local context absent"));
        }
    }

    let mut rows = Vec::new();
    let mut summaries = Vec::new();
    let mut snapshots = Vec::new();
    let mut contracts = BTreeMap::new();
    for selection in selections {
        active(deadline, cancelled)?;
        let binding = cmd::field(selection, "binding")?;
        let scope = match cmd::text(selection, "read_scope")? {
            "metadata_only" => NativeReadScope::MetadataOnly,
            "exact_public" => NativeReadScope::PublicContent,
            "exact_owner_local" => NativeReadScope::ExactOwnerLocal,
            _ => unreachable!(),
        };
        let adapted = if scope == NativeReadScope::ExactOwnerLocal {
            crate::source_sign_native::resolve_owner_assessment(
                reader,
                worker,
                binding,
                cmd::text(selection, "origin_id")?,
                scope,
                deadline,
                cancelled,
            )?
        } else {
            super::resolve_assessment(
                reader,
                worker,
                binding,
                cmd::text(selection, "origin_id")?,
                scope,
                deadline,
                cancelled,
            )?
        };
        contracts.extend(
            adapted
                .schema_digests
                .iter()
                .map(|(path, digest)| (path.clone(), digest.to_prefixed())),
        );
        let unit = &adapted.records[0];
        let layer = &adapted.records[1];
        let packet = cmd::field(cmd::field(unit, "payload")?, "packet")?;
        let segment = cmd::array(packet, "segmentations")?
            .iter()
            .find(|candidate| {
                candidate.object_get("segmentation_id") == binding.object_get("segmentation_id")
            })
            .ok_or(Error::Invalid("Sign PG selected native segmentation"))?;
        let configured = cmd::field(subjects, cmd::text(unit, "id")?)?;
        if !matches!(
            cmd::text(configured, "assertion_layer")?,
            "textual_observation" | "linguistic_analysis"
        ) || cmd::text(configured, "maker_id")?
            != cmd::text(cmd::field(segment, "maker")?, "agent_ref")?
            || !cmd::same(
                cmd::field(configured, "languages")?,
                &JsonValue::Array(vec![cmd::field(&adapted.summary, "language")?.clone()]),
            )?
        {
            return Err(Error::Denied(
                "Sign PG native configured unit scope differs from actual source",
            ));
        }
        let mut summary = adapted.summary;
        cmd::set(&mut summary, "record", super::envelope_ref(unit)?)?;
        cmd::set(&mut summary, "evidence_record", super::envelope_ref(layer)?)?;
        cmd::set(
            &mut summary,
            "read_scope",
            cmd::field(selection, "read_scope")?.clone(),
        )?;
        for row in adapted.records {
            if let Some(prior) = rows
                .iter()
                .find(|prior: &&JsonValue| prior.object_get("id") == row.object_get("id"))
            {
                if !cmd::same(prior, &row)? {
                    return Err(Error::Invalid(
                        "Sign PG native identity/body/origin differs",
                    ));
                }
            } else {
                rows.push(row);
            }
        }
        summaries.push(summary);
        snapshots.push(cmd::string(&adapted.input_snapshot));
    }
    Ok(super::NativeSelection {
        rows,
        summaries,
        snapshots,
        contracts,
    })
}

/// Rebuild the exact bounded public source inputs used by current Sign and
/// private Assessment evaluation. All source bytes and index probes are read
/// through the caller's held PG session; this is not a corpus membership
/// claim and never synthesizes a `SourceRevision`.
pub(crate) fn select_owner_assessment_public_sources_pg(
    input: &PgGrowthCurrentInput<'_, '_>,
    config: &JsonValue,
    worker: &mut tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<
        PgRecordRevisionIdentity,
    >,
    owner_context: Option<&OwnerTextContext>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PgOwnerAssessmentPublicSources> {
    active(deadline, cancelled)?;
    let identity = *input.input_identity();
    if worker.input_identity() != &identity {
        return Err(Error::Conflict(
            "Sign PG schema worker input identity differs",
        ));
    }
    super::require_assessment_candidate_profile(worker)?;
    let inline = cmd::text(config, "schema_version")? == "tos_local_assessment_owner_v1";
    let publication = if inline {
        None
    } else {
        let current = read_optional_current_input(input, PUBLICATION, 8192, deadline, cancelled)?;
        if let Some(raw) = &current {
            let value = cmd::parse(raw)?;
            crate::source_revisions::state(&value)?;
            if cmd::text(&value, "phase")? != "ready" {
                return Err(Error::Conflict("Sign PG source publication pending"));
            }
        }
        current
    };
    let mut reader = PgPublicSelection::new(input, owner_context);
    let rows = if inline {
        Vec::new()
    } else {
        source_envelopes_pg(
            &mut reader,
            input,
            config,
            worker,
            deadline,
            cancelled,
            false,
        )?
    };
    let source_bindings = if inline {
        &[][..]
    } else {
        cmd::array(config, "source_records")?
    };
    let has_claims = rows.iter().any(|row| {
        row.object_get("payload")
            .and_then(|payload| payload.object_get("claim_id"))
            .is_some()
    });
    let mut claim_dependencies = if has_claims {
        reader.source(ENTITIES, 1_048_576, deadline, cancelled)?;
        reader.source(RELATIONS, 1_048_576, deadline, cancelled)?;
        reader
            .source_dependencies
            .extend([ENTITIES.to_owned(), RELATIONS.to_owned()]);
        let entities = cmd::parse(
            reader
                .observed
                .get(ENTITIES)
                .ok_or(Error::Conflict("Sign PG entity registry absent"))?,
        )?;
        let relations = cmd::parse(
            reader
                .observed
                .get(RELATIONS)
                .ok_or(Error::Conflict("Sign PG relation registry absent"))?,
        )?;
        super::claim_ground_refs_from_registries(&rows, config, &entities, &relations)?
    } else {
        BTreeMap::new()
    };

    let mut record_paths = BTreeMap::new();
    let mut form_paths = BTreeMap::new();
    let mut form_sets = BTreeMap::new();
    let selected_rows = rows
        .iter()
        .map(|row| Ok((cmd::text(row, "id")?.to_owned(), row)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    for binding in source_bindings {
        let path = cmd::text(binding, "path")?.to_owned();
        let source_id = cmd::text(binding, "record_id")?.to_owned();
        if record_paths
            .insert(source_id.clone(), path.clone())
            .is_some()
        {
            return Err(Error::Invalid(
                "Sign PG public source identity path duplicate",
            ));
        }
        let row = selected_rows
            .get(&source_id)
            .ok_or(Error::Invalid("Sign PG public selected source row absent"))?;
        if cmd::text(cmd::field(row, "payload")?, "schema_version")? != "tos_human_form_v1" {
            continue;
        }
        let raw = reader
            .observed
            .get(&path)
            .ok_or(Error::Invalid("Sign PG public form bytes absent"))?;
        let package = cmd::parse(raw)?;
        if cmd::text(&package, "schema_version")? == "tos_human_form_set_v1" {
            form_paths.insert(source_id, path.clone());
            if let Some(previous) = form_sets.get(&path) {
                if previous != &package {
                    return Err(Error::Conflict("Sign PG selected form-set bytes differ"));
                }
            } else {
                form_sets.insert(path, package);
            }
        }
    }
    if !form_sets.is_empty() {
        reader.source(
            "ToS/contracts/human-form-set.schema.json",
            1_048_576,
            deadline,
            cancelled,
        )?;
    }
    for form in rows.iter().filter(|row| {
        row.object_get("payload")
            .and_then(|payload| payload.object_get("schema_version"))
            .and_then(JsonValue::as_str)
            == Some("tos_human_form_v1")
    }) {
        let payload = cmd::field(form, "payload")?;
        let Some(parent_ref) = payload.object_get("subject").filter(|reference| {
            reference
                .object_get("id")
                .and_then(JsonValue::as_str)
                .is_some()
        }) else {
            continue;
        };
        let parent_id = cmd::text(parent_ref, "id")?;
        let Some(parent_dependencies) = claim_dependencies.get(parent_id).cloned() else {
            continue;
        };
        let parent = rows
            .iter()
            .find(|row| row.object_get("id").and_then(JsonValue::as_str) == Some(parent_id))
            .ok_or(Error::Conflict("Sign PG form Claim parent is not selected"))?;
        if !cmd::same(&super::envelope_ref(parent)?, parent_ref)? {
            return Err(Error::Conflict(
                "Sign PG form Claim parent snapshot differs",
            ));
        }
        let mut closure = vec![parent_ref.clone()];
        closure.extend(parent_dependencies);
        claim_dependencies.insert(cmd::text(form, "id")?.to_owned(), closure);
    }

    // Match the source-cut adapter: v3 owns the public native-unit grammar.
    // Private v5/v6 units have already been selected and validated by their
    // confidential adapter; reinterpreting them here duplicates that read
    // and applies the wrong grant grammar.
    let native = if cmd::text(config, "schema_version")? == "tos_local_assessment_owner_v3" {
        native_envelopes_pg(&mut reader, config, worker, deadline, cancelled)?
    } else {
        super::NativeSelection {
            rows: Vec::new(),
            summaries: Vec::new(),
            snapshots: Vec::new(),
            contracts: BTreeMap::new(),
        }
    };
    let native_contracts = native.contracts.clone();
    let mut all_rows = rows.clone();
    for row in &native.rows {
        if all_rows
            .iter()
            .any(|prior| prior.object_get("id") == row.object_get("id"))
        {
            return Err(Error::Invalid(
                "Sign PG native evidence shadows selected source",
            ));
        }
        all_rows.push(row.clone());
    }
    let mut fixity = Vec::new();
    for name in reader
        .source_carriers
        .iter()
        .chain(reader.source_dependencies.iter())
    {
        let raw = reader
            .observed
            .get(name)
            .ok_or(Error::Conflict("Sign PG source snapshot dependency absent"))?;
        fixity.push(cmd::object(vec![
            ("path", cmd::string(name)),
            (
                "digest",
                cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
            ),
        ]));
    }
    let mut snapshot = cmd::object(vec![
        ("configuration", config.clone()),
        ("source_files", JsonValue::Array(fixity.clone())),
        ("resolved_records", JsonValue::Array(all_rows)),
        (
            "native_text_snapshots",
            JsonValue::Array(native.snapshots.clone()),
        ),
    ]);
    if !claim_dependencies.is_empty() {
        cmd::set(
            &mut snapshot,
            "public_claim_dependencies",
            cmd::object(
                claim_dependencies
                    .iter()
                    .map(|(id, refs)| (id.as_str(), JsonValue::Array(refs.clone())))
                    .collect(),
            ),
        )?;
    }
    if let Some(identity_snapshot) = &reader.identity_snapshot {
        cmd::set(
            &mut snapshot,
            "native_semantic_identity_snapshot",
            cmd::string(identity_snapshot),
        )?;
    }
    let mut identity_snapshots = BTreeMap::new();
    if !native.snapshots.is_empty() {
        identity_snapshots.insert(
            "native_text_snapshots".to_owned(),
            JsonValue::Array(native.snapshots),
        );
    }
    if let Some(identity_snapshot) = &reader.identity_snapshot {
        identity_snapshots.insert(
            "native_semantic_identity_snapshot".to_owned(),
            cmd::string(identity_snapshot),
        );
    }
    if !reader.profile_native_inputs.is_empty() {
        let tuples = JsonValue::Array(
            reader
                .profile_native_inputs
                .iter()
                .map(|((path, category), digest)| {
                    JsonValue::Array(vec![
                        cmd::string(path),
                        cmd::string(category),
                        cmd::string(&digest.to_hex()),
                    ])
                })
                .collect(),
        );
        let binding_snapshot = crate::source_revisions::python_ascii_digest(&tuples)?;
        cmd::set(
            &mut snapshot,
            "native_text_binding_snapshot",
            cmd::string(&binding_snapshot),
        )?;
        identity_snapshots.insert(
            "native_text_binding_snapshot".to_owned(),
            cmd::string(&binding_snapshot),
        );
    }
    let uses_owner_local_native = config
        .object_get("native_text_units")
        .and_then(JsonValue::as_array)
        .is_some_and(|selections| {
            selections.iter().any(|selection| {
                selection
                    .object_get("read_scope")
                    .and_then(JsonValue::as_str)
                    == Some("exact_owner_local")
            })
        });
    let native_identity_owner_context = if uses_owner_local_native {
        let context_snapshot = reader
            .owner_context_snapshot(deadline, cancelled)?
            .ok_or(Error::Denied("Sign PG owner text context absent"))?;
        reader.native_identity_owner_context = Some(context_snapshot.clone());
        identity_snapshots.insert(
            "native_identity_owner_context".to_owned(),
            cmd::string(&context_snapshot),
        );
        Some(context_snapshot)
    } else {
        None
    };
    if let Some(context_snapshot) = &native_identity_owner_context {
        cmd::set(
            &mut snapshot,
            "native_identity_owner_context",
            cmd::string(context_snapshot),
        )?;
    }
    let owner_snapshot = cmd::record_digest(&snapshot)?;
    reader.verify_current(deadline, cancelled)?;
    let public_sources = PgOwnerAssessmentPublicSources {
        rows,
        native_rows: native.rows,
        native_summaries: native.summaries,
        native_contracts,
        claim_dependencies,
        fixity,
        identity_snapshots,
        record_paths,
        form_paths,
        form_sets,
        selected_members: reader.selected_members.clone(),
        owner_snapshot,
        publication,
        publication_selected: !inline,
        identity_owners: reader.identity_owners.clone(),
        native_identity_owner_context,
    };
    public_sources.verify_current(input, owner_context, deadline, cancelled)?;
    Ok(public_sources)
}

fn selected_carrier_values(path: &str, raw: &[u8]) -> SourceCommandResult<Vec<JsonValue>> {
    if path.ends_with(".jsonl") {
        return raw
            .split(|byte| *byte == b'\n')
            .filter(|line| line.iter().any(|byte| !byte.is_ascii_whitespace()))
            .map(|line| {
                if line.len() > 1_048_576 {
                    return Err(SourceCommandError::Invalid(
                        "Sign selected source row byte budget",
                    ));
                }
                cmd::parse(line)
            })
            .collect();
    }
    let value = cmd::parse(raw)?;
    if value
        .object_get("schema_version")
        .and_then(JsonValue::as_str)
        == Some("tos_human_form_set_v1")
    {
        return Ok(cmd::array(&value, "forms")?.to_vec());
    }
    Ok(vec![value])
}

fn append_entity_schema_roots(
    entities: &JsonValue,
    value: &JsonValue,
    roots: &mut BTreeSet<String>,
) -> SourceCommandResult<()> {
    let kind = cmd::text(value, "record_type")?;
    let entries = cmd::array(entities, "types")?
        .iter()
        .filter(|entry| {
            entry
                .object_get("source_record_profile")
                .and_then(|profile| profile.object_get("record_type"))
                .and_then(JsonValue::as_str)
                == Some(kind)
        })
        .collect::<Vec<_>>();
    if entries.len() > 1 {
        return Err(SourceCommandError::Invalid(
            "Sign selected source profile ambiguity",
        ));
    }
    let Some(entry) = entries.first() else {
        return Ok(());
    };
    let profile = cmd::field(entry, "source_record_profile")?;
    if let Some(route) = cmd::array(profile, "schemas")?
        .iter()
        .find(|route| route.object_get("schema_version") == value.object_get("schema_version"))
    {
        roots.insert(cmd::text(route, "schema_ref")?.to_owned());
        for dependency in cmd::array(route, "schema_dependencies")? {
            roots.insert(
                dependency
                    .as_str()
                    .ok_or(SourceCommandError::Invalid(
                        "Sign source profile schema dependency",
                    ))?
                    .to_owned(),
            );
        }
    }
    Ok(())
}

fn append_relation_schema_roots(
    relations: &JsonValue,
    value: &JsonValue,
    roots: &mut BTreeSet<String>,
) -> SourceCommandResult<()> {
    let predicate = value.object_get("predicate");
    let routes = cmd::array(relations, "relations")?
        .iter()
        .filter(|relation| {
            relation
                .object_get("source_mappings")
                .and_then(JsonValue::as_array)
                .is_some_and(|mappings| {
                    mappings.iter().any(|mapping| {
                        mapping
                            .object_get("source_graph")
                            .and_then(JsonValue::as_str)
                            == Some("source-claims")
                            && mapping.object_get("scope").and_then(JsonValue::as_str)
                                == Some("claim-predicate")
                            && mapping.object_get("source_predicate_id") == predicate
                    })
                })
        })
        .collect::<Vec<_>>();
    if routes.len() > 1 {
        return Err(SourceCommandError::Invalid(
            "Sign selected Claim profile ambiguity",
        ));
    }
    let Some(profile) = routes
        .first()
        .and_then(|entry| entry.object_get("source_claim_profile"))
    else {
        return Ok(());
    };
    if let Some(route) = cmd::array(profile, "schemas")?
        .iter()
        .find(|route| route.object_get("schema_version") == value.object_get("schema_version"))
    {
        roots.insert(cmd::text(route, "schema_ref")?.to_owned());
        for dependency in cmd::array(route, "schema_dependencies")? {
            roots.insert(
                dependency
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("Sign Claim schema dependency"))?
                    .to_owned(),
            );
        }
    }
    Ok(())
}

/// Derive the Sign-owned semantic identity rows from the exact files that will
/// be inserted. Structural path rows remain the shared writer's responsibility.
pub(crate) fn expected_semantic_indexes_from_writes(
    writes: &[SourceRevisionWrite],
) -> SourceCommandResult<IndexRows> {
    if writes.is_empty() || writes.len() > 40 {
        return Err(SourceCommandError::Invalid("Sign PG package write count"));
    }

    let mut subjects = BTreeSet::new();
    for write in writes {
        if !subjects.insert(write.subject.as_str())
            || write.expected_predecessor.is_some()
            || write.proposed_revision == 0
        {
            return Err(SourceCommandError::Conflict(
                "Sign PG package must contain unique source-home insertions",
            ));
        }
    }

    let sign_writes = writes
        .iter()
        .filter(|write| write.subject.ends_with("/sign.json"))
        .collect::<Vec<_>>();
    let sign_write = match sign_writes.as_slice() {
        [write] => *write,
        _ => {
            return Err(SourceCommandError::Invalid(
                "Sign PG package requires one sign.json",
            ));
        }
    };
    let home = sign_write
        .subject
        .strip_suffix("/sign.json")
        .filter(|home| {
            home.starts_with("ToS/source-witnesses/")
                && !home.starts_with("ToS/source-witnesses/owner-local/")
        })
        .ok_or(SourceCommandError::Denied("Sign PG source home"))?;
    RelativePath::parse(&sign_write.subject)
        .map_err(|_| SourceCommandError::Denied("Sign PG source path"))?;

    let form_path = format!("{home}/sign.human-forms.json");
    let event_path = format!("{home}/source-create-provenance.jsonl");
    let form_write = writes
        .iter()
        .find(|write| write.subject == form_path)
        .ok_or(SourceCommandError::Invalid("Sign PG form output absent"))?;
    let event_write = writes
        .iter()
        .find(|write| write.subject == event_path)
        .ok_or(SourceCommandError::Invalid(
            "Sign PG provenance output absent",
        ))?;
    if writes.iter().any(|write| {
        !write.subject.starts_with(&format!("{home}/"))
            || write.subject == home
            || write.subject.ends_with("/sign.json") && write.subject != sign_write.subject
    }) {
        return Err(SourceCommandError::Denied(
            "Sign PG package escapes its exact source home",
        ));
    }

    let sign = cmd::parse(&sign_write.exact_bytes)?;
    let sign_id = cmd::text(&sign, "record_id")?.to_owned();
    if !crate::source_revisions::valid_id(&sign_id, "tos.sign.", false) {
        return Err(SourceCommandError::Invalid("Sign PG record identity"));
    }
    let candidate_id =
        candidate_id_from_current_bytes(&sign_write.subject, &sign_write.exact_bytes)?;

    let forms = cmd::parse(&form_write.exact_bytes)?;
    if cmd::text(cmd::field(&forms, "subject")?, "id")? != sign_id.as_str() {
        return Err(SourceCommandError::Conflict(
            "Sign PG form subject differs from Sign identity",
        ));
    }
    source_forms::apply_form_changes(Some(&forms), cmd::field(&forms, "subject")?, &[])?;
    let mut form_ids = BTreeSet::new();
    for section in ["forms", "prior_forms"] {
        for form in cmd::array(&forms, section)? {
            let form_id = cmd::text(form, "form_id")?.to_owned();
            if form_id.is_empty() || !form_ids.insert(form_id.clone()) {
                return Err(SourceCommandError::Invalid(
                    "Sign PG repeated form identity",
                ));
            }
        }
    }

    let event = cmd::parse(&event_write.exact_bytes)?;
    let event_id = cmd::text(&event, "event_id")?.to_owned();
    if event_id.is_empty() {
        return Err(SourceCommandError::Invalid("Sign PG event identity"));
    }

    let mut indexes = IndexRows::new();
    indexes.insert(("metadata".into(), sign_id, sign_write.subject.clone()));
    indexes.insert(("event".into(), event_id, event_write.subject.clone()));
    for form_id in form_ids {
        indexes.insert(("form".into(), form_id, form_write.subject.clone()));
    }
    indexes.insert((
        "sign-candidate".into(),
        candidate_id,
        sign_write.subject.clone(),
    ));
    Ok(indexes)
}

/// The new Sign record is itself the only new Sign inventory contribution.
/// Its public profile and promotion basis have already been checked by the
/// owner preparer; this function rebinds the contribution to the exact output
/// bytes consumed by the common writer.
pub(crate) fn expected_inventory_contributions_from_writes(
    context: &crate::source_command::CommandContext,
    _proposal: &crate::source_command::PreparedCommand,
    writes: &[SourceRevisionWrite],
) -> SourceCommandResult<BTreeMap<String, JsonValue>> {
    let indexes = expected_semantic_indexes_from_writes(writes)?;
    let sign_path = indexes
        .iter()
        .find(|(kind, _, path)| kind == "metadata" && path.ends_with("/sign.json"))
        .map(|(_, _, path)| path.clone())
        .ok_or(SourceCommandError::Invalid("Sign PG metadata index absent"))?;
    let write = writes
        .iter()
        .find(|write| write.subject == sign_path)
        .ok_or(SourceCommandError::Invalid("Sign PG output bytes absent"))?;
    let record = cmd::parse(&write.exact_bytes)?;
    let _subject = source_forms::metadata_subject(&record)?;
    let mut base_profiles = BTreeMap::new();
    let mut full_profiles = BTreeMap::new();
    for file in &context.files {
        let path = file.path.as_str();
        let digest = Digest256::of_bytes(&file.raw).to_hex();
        if path == "ToS/doctrine/semantic-interchange/entity-types.v1.json"
            || path == "ToS/contracts/semantic-entity-type-registry.schema.json"
        {
            base_profiles.insert(path.to_owned(), digest.clone());
            full_profiles.insert(path.to_owned(), digest);
        } else if path.starts_with("ToS/contracts/") && path.ends_with(".schema.json") {
            full_profiles.insert(path.to_owned(), digest);
        }
    }
    if base_profiles.len() != 2 {
        return Err(SourceCommandError::Invalid(
            "Sign PG source profile input closure",
        ));
    }
    if !full_profiles.contains_key(SIGN_DESCRIPTION_SCHEMA) {
        return Err(SourceCommandError::Invalid(
            "Sign PG retained description schema closure",
        ));
    }
    let profiles = |items: BTreeMap<String, String>| {
        JsonValue::Object(
            items
                .into_iter()
                .map(|(path, digest)| (JsonString::from_utf8(&path), cmd::string(&digest)))
                .collect(),
        )
    };
    let base_profiles = profiles(base_profiles);
    let full_profiles = profiles(full_profiles);
    let mut sign_rows = Vec::new();
    sign_rows.push(crate::source_claims::pg_catalogue_record_value(
        &record,
        &sign_path,
        Some(SIGN_DESCRIPTION_SCHEMA),
    )?);
    let event_path = write
        .subject
        .strip_suffix("sign.json")
        .map(|home| format!("{home}source-create-provenance.jsonl"))
        .ok_or(SourceCommandError::Invalid("Sign PG event path"))?;
    let event_write = writes
        .iter()
        .find(|write| write.subject == event_path)
        .ok_or(SourceCommandError::Invalid("Sign PG event bytes absent"))?;
    let mut events = BTreeMap::<String, JsonValue>::new();
    for (line, raw) in event_write
        .exact_bytes
        .split(|byte| *byte == b'\n')
        .enumerate()
    {
        if raw.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let event = cmd::parse(raw)?;
        let id = cmd::text(&event, "event_id")?.to_owned();
        let digest = cmd::record_digest(&event)?.to_hex();
        if events
            .insert(
                id,
                cmd::object(vec![
                    ("payload", event),
                    ("source_ref", cmd::string(&event_path)),
                    ("source_line", cmd::number((line + 1) as u64)),
                    ("source_sha256", cmd::string(&digest)),
                ]),
            )
            .is_some()
        {
            return Err(SourceCommandError::Invalid("Sign PG repeated event"));
        }
    }
    if events.is_empty() {
        return Err(SourceCommandError::Invalid("Sign PG event stream empty"));
    }
    let event_rows = JsonValue::Object(
        events
            .into_iter()
            .map(|(id, row)| (JsonString::from_utf8(&id), row))
            .collect(),
    );
    let mut contributions = BTreeMap::new();
    let mut seen_forms = BTreeSet::new();
    for item in writes {
        let path = &item.subject;
        let mut records = BTreeMap::<String, JsonValue>::new();
        for kind in crate::source_claims::MIXED_INVENTORY_RECORD_KINDS {
            let rows = if *kind == "sign" && path == &sign_path {
                sign_rows.clone()
            } else {
                Vec::new()
            };
            records.insert((*kind).to_owned(), JsonValue::Array(rows));
        }
        let form = if path.ends_with(".human-forms.json") {
            let set = cmd::parse(&item.exact_bytes)?;
            let mut form_ids = Vec::new();
            for section in ["forms", "prior_forms"] {
                for row in cmd::array(&set, section)? {
                    let id = cmd::text(row, "form_id")?.to_owned();
                    if !seen_forms.insert(id.clone()) {
                        return Err(SourceCommandError::Invalid("Sign PG form identity repeats"));
                    }
                    form_ids.push(cmd::string(&id));
                }
            }
            cmd::object(vec![
                (
                    "raw_sha256",
                    cmd::string(&Digest256::of_bytes(&item.exact_bytes).to_prefixed()),
                ),
                (
                    "subject_id",
                    cmd::string(cmd::text(cmd::field(&set, "subject")?, "id")?),
                ),
                ("form_ids", JsonValue::Array(form_ids)),
            ])
        } else {
            JsonValue::Null
        };
        let member_profiles = if path == &sign_path || form != JsonValue::Null {
            full_profiles.clone()
        } else {
            base_profiles.clone()
        };
        let owner = cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_pg_source_inventory_member_contribution_v1"),
            ),
            (
                "records",
                JsonValue::Object(
                    records
                        .into_iter()
                        .map(|(kind, rows)| (JsonString::from_utf8(&kind), rows))
                        .collect(),
                ),
            ),
            ("source_profiles", member_profiles),
            (
                "events",
                if path == &event_path {
                    event_rows.clone()
                } else {
                    JsonValue::Object(Vec::new())
                },
            ),
            ("anchors", JsonValue::Object(Vec::new())),
            ("form", form),
            ("claim_owner", JsonValue::Null),
        ]);
        contributions.insert(path.clone(), owner);
    }
    Ok(contributions)
}
