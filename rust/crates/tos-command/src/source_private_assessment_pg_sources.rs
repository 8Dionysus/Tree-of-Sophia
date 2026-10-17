//! Exact private-package closure for selected PostgreSQL Assessment sources.
//!
//! The protected Assessment configuration is the only grant for these reads.
//! This module derives package target refs from its explicit selections and
//! never scans the owner-local namespace.

use crate::durable_private_packages::PrivateOwnerPackageHistoryV1;
use crate::source_cohort::revision::PgRecordRevisionIdentity;
use crate::source_command::{
    self as cmd, CommandContext, SourceCommandError, SourceCommandResult, SourceFile,
};
use crate::source_native_cli::PgGrowthCurrentInput;
use crate::source_native_cli::private_owner::profile::{
    PrivateSelectedSourceIdentity, PrivateSelectedSourceInput,
};
use crate::source_revisions::RecordRead;
use crate::source_sign_native::{NativeInput, NativeReadKind, NativeReadScope, SignNativeRead};
use crate::source_text_owner::OwnerTextContext;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{JsonString, JsonValue, RelativePath};
use tos_validation::assessment::{AssessmentRecordInput, MAX_ASSESSMENTS, MAX_RECORD_BYTES};
use tos_validation::record_biblio_cut::{SourceCutInput, SourceCutInputWithIdentity};
use tos_validation::source_cut::CandidateCutWorkerSchemaExecutor;

const MAX_SELECTED_PRIVATE_MEMBERS: usize = 4096;
const MAX_SELECTED_PRIVATE_BYTES: usize = 64 * 1024 * 1024;
const MAX_SELECTED_PUBLIC_MEMBERS: usize = 4096;
const MAX_SELECTED_PUBLIC_BYTES: usize = 64 * 1024 * 1024;
const MAX_PRIVATE_SELECTED_MEMBER_BYTES: usize = 32 * 1024 * 1024;
const MAX_PUBLIC_SELECTED_MEMBER_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
struct PackageHeadMarker {
    revision: u64,
    package_digest: tos_foundation::Digest256,
    private_audit_generation: u64,
    members: BTreeMap<String, (usize, tos_foundation::Digest256)>,
}

/// Exact selected v5/v6 private source state for the Assessment owner. Private
/// package bytes remain internal to the adapter; consumers receive only
/// validated assessment envelopes, references, forms, and digest snapshots.
pub(crate) struct PgPrivateAssessmentSources {
    pub(crate) records: Vec<AssessmentRecordInput>,
    pub(crate) native_records: Vec<AssessmentRecordInput>,
    pub(crate) native_snapshots: Vec<String>,
    pub(crate) native_summaries: Vec<JsonValue>,
    pub(crate) owner_local_sources_snapshot: String,
    pub(crate) required_source_refs: BTreeMap<String, Vec<JsonValue>>,
    pub(crate) required_languages: BTreeMap<String, Vec<String>>,
    pub(crate) form_sets: BTreeMap<String, JsonValue>,
    pub(crate) form_paths: BTreeMap<String, String>,
    pub(crate) source_paths: BTreeMap<String, String>,
    pub(crate) selected_members: BTreeMap<String, Vec<u8>>,
    pub(crate) target_refs: Vec<String>,
    context_snapshot: String,
    private_heads: BTreeMap<String, PackageHeadMarker>,
    public_members: BTreeMap<String, Vec<u8>>,
}

impl PgPrivateAssessmentSources {
    /// Exact source dependency bytes used to bind a private-owner package to
    /// its selected PG read epoch. The returned material stays in-process and
    /// is intended only for the bounded dependency digest; callers must not
    /// serialize it or expose it in command results.
    pub(crate) fn dependency_members(&self) -> Vec<(String, Vec<u8>)> {
        self.selected_members
            .iter()
            .chain(self.public_members.iter())
            .map(|(path, bytes)| (path.clone(), bytes.clone()))
            .collect()
    }

    pub(crate) fn input_bytes(&self) -> SourceCommandResult<usize> {
        self.selected_members
            .values()
            .chain(self.public_members.values())
            .try_fold(0usize, |total, bytes| {
                total
                    .checked_add(bytes.len())
                    .ok_or(SourceCommandError::Invalid(
                        "private assessment PG input byte total overflow",
                    ))
            })
    }

    pub(crate) fn verify_current(
        &self,
        owner: &OwnerTextContext,
        input: &PgGrowthCurrentInput<'_, '_>,
        package_histories: &BTreeMap<String, PrivateOwnerPackageHistoryV1>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if owner.snapshot(deadline, cancelled)?.to_prefixed() != self.context_snapshot {
            return Err(SourceCommandError::Conflict(
                "private assessment PG owner context changed",
            ));
        }
        if package_histories.len() < self.private_heads.len() {
            return Err(SourceCommandError::Conflict(
                "private assessment PG package history set changed",
            ));
        }
        for (target_ref, expected) in &self.private_heads {
            let history = package_histories
                .get(target_ref)
                .ok_or(SourceCommandError::Conflict(
                    "private assessment PG selected package history is absent",
                ))?;
            if history.target_ref != *target_ref
                || history.private_audit_generation != expected.private_audit_generation
            {
                return Err(SourceCommandError::Conflict(
                    "private assessment PG selected package audit changed",
                ));
            }
            let current = history
                .current
                .as_ref()
                .ok_or(SourceCommandError::Conflict(
                    "private assessment PG selected package became absent",
                ))?;
            if current.target_ref != *target_ref
                || current.revision != expected.revision
                || current.package_digest != expected.package_digest
                || current.private_audit_generation != expected.private_audit_generation
                || package_member_markers(&current.members)? != expected.members
            {
                return Err(SourceCommandError::Conflict(
                    "private assessment PG selected package head changed",
                ));
            }
        }
        for (path, expected) in &self.public_members {
            let current = read_selected_public_member(
                input,
                path,
                expected.len().max(1),
                deadline,
                cancelled,
            )?;
            if &current != expected {
                return Err(SourceCommandError::Conflict(
                    "private assessment PG public source member changed",
                ));
            }
        }
        if owner.snapshot(deadline, cancelled)?.to_prefixed() != self.context_snapshot {
            return Err(SourceCommandError::Conflict(
                "private assessment PG owner context changed during verification",
            ));
        }
        Ok(())
    }
}

// A caller's source snapshot binds exact selected package contents and
// revisions. The global private audit generation also advances on journal
// append and unrelated package writes; including it makes a successful append
// invalidate its own retry. Keep that generation in the held currentness
// guards and PackageHeadMarker, outside this stable source fingerprint.
fn source_package_snapshot(
    private_heads: &BTreeMap<String, PackageHeadMarker>,
) -> SourceCommandResult<String> {
    let package_rows = JsonValue::Object(
        private_heads
            .iter()
            .map(|(target, head)| {
                (
                    JsonString::from_utf8(target),
                    cmd::object(vec![
                        ("revision", cmd::number(head.revision)),
                        (
                            "package_digest",
                            cmd::string(&head.package_digest.to_prefixed()),
                        ),
                        (
                            "members",
                            JsonValue::Object(
                                head.members
                                    .iter()
                                    .map(|(name, (size, digest))| {
                                        (
                                            JsonString::from_utf8(name),
                                            cmd::object(vec![
                                                ("size_bytes", cmd::number(*size as u64)),
                                                ("sha256", cmd::string(&digest.to_prefixed())),
                                            ]),
                                        )
                                    })
                                    .collect(),
                            ),
                        ),
                    ]),
                )
            })
            .collect(),
    );
    digest_snapshot(&cmd::object(vec![("targets", package_rows)]))
}

fn package_member_markers(
    members: &BTreeMap<String, Vec<u8>>,
) -> SourceCommandResult<BTreeMap<String, (usize, tos_foundation::Digest256)>> {
    members
        .iter()
        .map(|(name, bytes)| {
            Ok((
                name.clone(),
                (bytes.len(), tos_foundation::Digest256::of_bytes(bytes)),
            ))
        })
        .collect()
}

fn read_selected_public_member(
    input: &PgGrowthCurrentInput<'_, '_>,
    reference: &str,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let relative = RelativePath::parse(reference)
        .map_err(|_| SourceCommandError::Denied("private assessment PG source path"))?;
    if relative.as_str() != reference
        || !reference.starts_with("ToS/")
        || reference.starts_with("ToS/source-witnesses/owner-local/")
        || reference.split('/').any(|part| {
            matches!(part, "payload" | "local-content" | "catalog") || part.starts_with('.')
        })
    {
        return Err(SourceCommandError::Denied(
            "private assessment PG source namespace",
        ));
    }
    let mut selected = None;
    input
        .source_input()
        .with_current_member(
            reference,
            max_bytes.min(MAX_PUBLIC_SELECTED_MEMBER_BYTES),
            deadline,
            cancelled,
            &mut |meta, raw| {
                if meta.path != reference || selected.is_some() {
                    return Err(tos_validation::item_rules::ItemRefusal::Source(
                        "private assessment PG member identity differs".into(),
                    ));
                }
                selected = Some(raw.to_vec());
                Ok(())
            },
        )
        .map_err(|_| {
            SourceCommandError::Conflict("private assessment PG selected source read failed")
        })?;
    selected.ok_or(SourceCommandError::Conflict(
        "private assessment PG selected source member is absent",
    ))
}

/// The Profile/Claim owner kernels accept this narrow transport trait. Public
/// member paths stay reads on the active PG session; private paths are
/// satisfied only from the exact broker histories held by the caller.
struct PgOwnerAssessmentCurrent<'a, 'session, 'store> {
    owner: &'a OwnerTextContext,
    input: &'a PgGrowthCurrentInput<'session, 'store>,
    histories: &'a BTreeMap<String, PrivateOwnerPackageHistoryV1>,
}

impl PrivateSelectedSourceInput for PgOwnerAssessmentCurrent<'_, '_, '_> {
    fn private_selection_identity(&self) -> PrivateSelectedSourceIdentity {
        PrivateSelectedSourceIdentity::PgRecord(*SourceCutInputWithIdentity::input_identity(
            self.input,
        ))
    }

    fn annotation_identity_owners(
        &self,
        tokens: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<
        Option<
            BTreeMap<
                String,
                BTreeSet<crate::source_annotation_identity_index::AnnotationEntityOwner>,
            >,
        >,
        tos_validation::item_rules::ItemRefusal,
    > {
        self.input
            .lookup_annotation_identity_owners(tokens, deadline, cancelled)
            .map(Some)
            .map_err(|_| {
                tos_validation::item_rules::ItemRefusal::Source(
                    "private assessment PG annotation identity lookup failed".into(),
                )
            })
    }

    fn for_each_current_member_path_under(
        &self,
        directory: &str,
        _deadline: Instant,
        _cancelled: &AtomicBool,
        visit: &mut dyn FnMut(&str) -> Result<(), tos_validation::item_rules::ItemRefusal>,
    ) -> Result<
        tos_validation::record_biblio_cut::SourceCutPrefixCoverage,
        tos_validation::item_rules::ItemRefusal,
    > {
        let parsed = RelativePath::parse(directory).map_err(|_| {
            tos_validation::item_rules::ItemRefusal::Source("private assessment path".into())
        })?;
        if parsed.as_str() != directory {
            return Err(tos_validation::item_rules::ItemRefusal::Source(
                "private assessment path is not canonical".into(),
            ));
        }
        if directory.starts_with(self.owner.private_prefix()) {
            let prefix = format!("{directory}/");
            let mut paths = BTreeSet::new();
            for history in self.histories.values() {
                let Some(current) = &history.current else {
                    continue;
                };
                for name in current.members.keys() {
                    let reference = format!("{}/{name}", current.target_ref);
                    if reference.starts_with(&prefix) {
                        paths.insert(reference);
                    }
                }
            }
            let count = u64::try_from(paths.len())
                .map_err(|_| tos_validation::item_rules::ItemRefusal::Budget)?;
            for path in paths {
                visit(&path)?;
            }
            return tos_validation::record_biblio_cut::SourceCutPrefixCoverage::after_verified_prefix_eof(
                directory, count,
            );
        }
        // Public enumeration is deliberately refused. Selected PG operations
        // use exact member and owner-index lookups; a whole ToS prefix scan is
        // not a substitute for the missing point predicate.
        Err(tos_validation::item_rules::ItemRefusal::Unsupported(
            "private assessment PG source input has no public directory walk".into(),
        ))
    }

    fn with_current_member(
        &self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(&str, &[u8]) -> Result<(), tos_validation::item_rules::ItemRefusal>,
    ) -> Result<(), tos_validation::item_rules::ItemRefusal> {
        if self.owner.is_owner_private_reference(path) {
            let target_ref = self.owner.private_package_target_ref(path).map_err(|_| {
                tos_validation::item_rules::ItemRefusal::Source(
                    "private assessment selected package path".into(),
                )
            })?;
            let name = path.rsplit('/').next().ok_or_else(|| {
                tos_validation::item_rules::ItemRefusal::Source(
                    "private assessment selected package member".into(),
                )
            })?;
            let raw = current_package_member(self.histories, &target_ref, name).map_err(|_| {
                tos_validation::item_rules::ItemRefusal::Source(
                    "private assessment selected package member absent".into(),
                )
            })?;
            if raw.len() > max_bytes {
                return Err(tos_validation::item_rules::ItemRefusal::Budget);
            }
            return visit(path, raw);
        }
        SourceCutInput::with_current_member(
            self.input,
            path,
            max_bytes,
            deadline,
            cancelled,
            &mut |meta, raw| visit(meta.path, raw),
        )
    }
}

/// RecordRead adapter that carries the same selected PG identity and adds the
/// exact private package bytes supplied by the held broker histories. It does
/// not turn those packages into authored source revision members.
struct PgOwnerAssessmentRecordRead<'a, 'session, 'store> {
    command: &'a CommandContext,
    current: &'a PgOwnerAssessmentCurrent<'a, 'session, 'store>,
    identity: PgRecordRevisionIdentity,
    files: Vec<SourceFile>,
}

impl RecordRead for PgOwnerAssessmentRecordRead<'_, '_, '_> {
    type Identity = PgRecordRevisionIdentity;

    fn files(&self) -> &[SourceFile] {
        &self.files
    }

    fn input_identity(&self) -> Self::Identity {
        self.identity
    }

    fn effective_uid(&self) -> u64 {
        self.command.effective_uid
    }

    fn schema_source_path(&self) -> SourceCommandResult<String> {
        self.command.schema_source_path()
    }

    fn writer(&self) -> Option<&CommandContext> {
        Some(self.command)
    }

    fn check(&self) -> SourceCommandResult<()> {
        self.command.check()?;
        if *SourceCutInputWithIdentity::input_identity(self.current.input) != self.identity {
            return Err(SourceCommandError::Conflict(
                "private assessment selected source epoch changed",
            ));
        }
        Ok(())
    }
}

fn insert_selected_file(
    files: &mut BTreeMap<String, Vec<u8>>,
    reference: &str,
    raw: &[u8],
) -> SourceCommandResult<()> {
    let path = RelativePath::parse(reference)
        .map_err(|_| SourceCommandError::Invalid("private assessment selected member path"))?;
    if path.as_str() != reference {
        return Err(SourceCommandError::Invalid(
            "private assessment selected member path",
        ));
    }
    if files
        .insert(reference.to_owned(), raw.to_vec())
        .is_some_and(|previous| previous != raw)
    {
        return Err(SourceCommandError::Conflict(
            "private assessment selected member bytes differ",
        ));
    }
    Ok(())
}

fn install_selected_members(
    owner: &OwnerTextContext,
    additions: BTreeMap<String, Vec<u8>>,
) -> SourceCommandResult<()> {
    if additions.is_empty() {
        return Ok(());
    }
    if owner.has_pg_selected_members()? {
        owner.extend_pg_selected_members(additions)
    } else {
        owner.install_pg_selected_members(additions)
    }
}

fn selected_private_package_members(
    owner: &OwnerTextContext,
    package_histories: &BTreeMap<String, PrivateOwnerPackageHistoryV1>,
    targets: &[String],
) -> SourceCommandResult<(
    BTreeMap<String, Vec<u8>>,
    BTreeMap<String, PackageHeadMarker>,
    Option<u64>,
)> {
    let mut members = BTreeMap::new();
    let mut heads = BTreeMap::new();
    let mut common_generation = None;
    let mut total_bytes = 0usize;
    let mut total_members = 0usize;
    for target_ref in targets {
        let history = package_histories
            .get(target_ref)
            .ok_or(SourceCommandError::Conflict(
                "private assessment selected package was not held",
            ))?;
        if history.target_ref != *target_ref {
            return Err(SourceCommandError::Conflict(
                "private assessment selected package target differs",
            ));
        }
        match common_generation {
            None => common_generation = Some(history.private_audit_generation),
            Some(generation) if generation == history.private_audit_generation => (),
            Some(_) => {
                return Err(SourceCommandError::Conflict(
                    "private assessment selected package generations differ",
                ));
            }
        }
        let current = history
            .current
            .as_ref()
            .ok_or(SourceCommandError::Conflict(
                "private assessment selected package is absent",
            ))?;
        if current.target_ref != *target_ref
            || history
                .family
                .is_some_and(|family| family != current.family)
            || current.private_audit_generation != history.private_audit_generation
            || current.members.is_empty()
        {
            return Err(SourceCommandError::Conflict(
                "private assessment selected package head is inconsistent",
            ));
        }
        let mut selected_names = BTreeMap::new();
        for (name, bytes) in &current.members {
            if name.is_empty()
                || name.contains('/')
                || matches!(name.as_str(), "." | "..")
                || name.starts_with('.')
                || bytes.len() > MAX_PRIVATE_SELECTED_MEMBER_BYTES
            {
                return Err(SourceCommandError::Denied(
                    "private assessment selected package member path",
                ));
            }
            let reference = format!("{target_ref}/{name}");
            let parsed = RelativePath::parse(&reference)
                .map_err(|_| SourceCommandError::Denied("private assessment package member"))?;
            if parsed.as_str() != reference || !owner.is_owner_private_reference(&reference) {
                return Err(SourceCommandError::Denied(
                    "private assessment selected package leaves owner store",
                ));
            }
            if members.insert(reference, bytes.clone()).is_some() {
                return Err(SourceCommandError::Conflict(
                    "private assessment selected member path repeats",
                ));
            }
            total_bytes = total_bytes
                .checked_add(bytes.len())
                .filter(|total| *total <= MAX_SELECTED_PRIVATE_BYTES)
                .ok_or(SourceCommandError::Unsupported(
                    "private assessment selected package byte budget",
                ))?;
            selected_names.insert(
                name.clone(),
                (bytes.len(), tos_foundation::Digest256::of_bytes(bytes)),
            );
            total_members = total_members
                .checked_add(1)
                .filter(|count| *count <= MAX_SELECTED_PRIVATE_MEMBERS)
                .ok_or(SourceCommandError::Unsupported(
                    "private assessment selected package member budget",
                ))?;
            if selected_names.len() > MAX_SELECTED_PRIVATE_MEMBERS {
                return Err(SourceCommandError::Unsupported(
                    "private assessment selected package member budget",
                ));
            }
        }
        heads.insert(
            target_ref.clone(),
            PackageHeadMarker {
                revision: current.revision,
                package_digest: current.package_digest,
                private_audit_generation: current.private_audit_generation,
                members: selected_names,
            },
        );
    }
    Ok((members, heads, common_generation))
}

fn add_owner_read(
    owner: &OwnerTextContext,
    input: &PgGrowthCurrentInput<'_, '_>,
    public_members: &mut BTreeMap<String, Vec<u8>>,
    reference: &str,
    max_bytes: usize,
    kind: NativeReadKind,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    if owner.is_owner_private_reference(reference) {
        return owner.read(reference, max_bytes, deadline, cancelled);
    }
    let metadata = reference.starts_with("ToS/source-witnesses/")
        && !reference.starts_with("ToS/source-witnesses/owner-local/")
        && !reference.split('/').any(|part| {
            matches!(part, "catalog" | "local-content")
                || part == "payload" && kind != NativeReadKind::Content
        });
    let schema = reference.starts_with("ToS/contracts/");
    let support = reference.starts_with("ToS/")
        && !reference.starts_with("ToS/source-witnesses/owner-local/")
        && !reference.split('/').any(|part| {
            matches!(part, "catalog" | "local-content")
                || part == "payload" && kind != NativeReadKind::Content
        });
    let allowed = match kind {
        NativeReadKind::Schema => schema,
        NativeReadKind::Metadata => metadata,
        NativeReadKind::Support => support,
        NativeReadKind::Content => false,
    };
    let relative = RelativePath::parse(reference)
        .map_err(|_| SourceCommandError::Denied("private assessment PG dependency path"))?;
    if relative.as_str() != reference || !allowed || max_bytes == 0 {
        return Err(SourceCommandError::Denied(
            "private assessment PG dependency namespace",
        ));
    }
    let raw = if let Some(previous) = public_members.get(reference) {
        previous.clone()
    } else {
        read_selected_public_member(input, reference, max_bytes, deadline, cancelled)?
    };
    if raw.len() > max_bytes {
        return Err(SourceCommandError::Unsupported(
            "private assessment PG dependency byte budget",
        ));
    }
    if public_members
        .insert(reference.to_owned(), raw.clone())
        .is_some_and(|previous| previous != raw)
    {
        return Err(SourceCommandError::Conflict(
            "private assessment PG dependency changed during selection",
        ));
    }
    install_selected_members(owner, BTreeMap::from([(reference.to_owned(), raw.clone())]))?;
    Ok(raw)
}

struct PgOwnerAssessmentReader<'a, 'session, 'store> {
    owner: &'a OwnerTextContext,
    input: &'a PgGrowthCurrentInput<'session, 'store>,
    public_members: BTreeMap<String, Vec<u8>>,
    allow_content: bool,
    context_snapshot: String,
    read_bytes: usize,
}

impl<'a, 'session, 'store> PgOwnerAssessmentReader<'a, 'session, 'store> {
    fn new(
        owner: &'a OwnerTextContext,
        input: &'a PgGrowthCurrentInput<'session, 'store>,
        allow_content: bool,
        context_snapshot: String,
    ) -> Self {
        Self {
            owner,
            input,
            public_members: BTreeMap::new(),
            allow_content,
            context_snapshot,
            read_bytes: 0,
        }
    }

    fn into_public_members(self) -> BTreeMap<String, Vec<u8>> {
        self.public_members
    }
}

impl SignNativeRead for PgOwnerAssessmentReader<'_, '_, '_> {
    fn read(
        &mut self,
        reference: &str,
        kind: NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        if kind == NativeReadKind::Content && !self.allow_content {
            return Err(SourceCommandError::Denied(
                "metadata-only private assessment cannot read source content",
            ));
        }
        let raw = add_owner_read(
            self.owner,
            self.input,
            &mut self.public_members,
            reference,
            max_bytes,
            kind,
            deadline,
            cancelled,
        )?;
        self.read_bytes = self
            .read_bytes
            .checked_add(raw.len())
            .filter(|total| *total <= MAX_SELECTED_PUBLIC_BYTES)
            .ok_or(SourceCommandError::Unsupported(
                "private assessment PG native input budget",
            ))?;
        Ok(raw)
    }

    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        for (reference, expected) in &self.public_members {
            if read_selected_public_member(
                self.input,
                reference,
                expected.len().max(1),
                deadline,
                cancelled,
            )? != *expected
            {
                return Err(SourceCommandError::Conflict(
                    "private assessment PG native input changed",
                ));
            }
        }
        if self.owner.snapshot(deadline, cancelled)?.to_prefixed() != self.context_snapshot {
            return Err(SourceCommandError::Conflict(
                "private assessment PG native owner context changed",
            ));
        }
        Ok(())
    }

    fn owner_local(&self, reference: &str) -> SourceCommandResult<bool> {
        Ok(self.owner.is_owner_private_reference(reference))
    }

    fn owner_context_snapshot(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<String>> {
        Ok(Some(
            self.owner.snapshot(deadline, cancelled)?.to_prefixed(),
        ))
    }
}

const MAX_SOURCE_SELECTIONS: usize = 64;
const MAX_CLAIM_SELECTIONS: usize = 32;
const MAX_SCHEMA_PATHS: usize = 512;

const FIXED_ASSESSMENT_SCHEMAS: &[&str] = &[
    "ToS/contracts/knowledge-assessment.schema.json",
    "ToS/contracts/knowledge-assessment-policy.schema.json",
    "ToS/contracts/knowledge-assessment-authority.schema.json",
    "ToS/contracts/knowledge-assessment-competence.schema.json",
    "ToS/contracts/knowledge-assessment-batch.schema.json",
    "ToS/contracts/native-text-layer-quality-basis.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/owner-local-source-context.schema.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
    "ToS/contracts/source-claim-record.schema.json",
    "ToS/contracts/claim-packet.schema.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/source-metadata-record.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
    "ToS/contracts/native-text-unit-binding.schema.json",
    "ToS/contracts/native-text-unit-assessment-subject.schema.json",
    "ToS/contracts/source-text-unit-packet-v1.schema.json",
    "ToS/contracts/native-text-layer-binding.schema.json",
    "ToS/contracts/source-text-layer.schema.json",
    "ToS/contracts/native-text-layer-comparison.schema.json",
    "ToS/contracts/native-text-layer-derivation-comparison.schema.json",
    "ToS/contracts/native-page-ocr-comparison.schema.json",
    "ToS/contracts/source-anchor-v2.schema.json",
    "ToS/contracts/source-item-manifest.schema.json",
    "ToS/contracts/rights-record.schema.json",
    "ToS/contracts/semantic-annotation-packet-v2.schema.json",
];

fn insert_schema(schemas: &mut BTreeSet<String>, reference: &str) -> SourceCommandResult<()> {
    let path = RelativePath::parse(reference)
        .map_err(|_| SourceCommandError::Invalid("assessment schema path"))?;
    if path.as_str() != reference || !reference.starts_with("ToS/contracts/") {
        return Err(SourceCommandError::Invalid("assessment schema path"));
    }
    schemas.insert(reference.to_owned());
    if schemas.len() > MAX_SCHEMA_PATHS {
        return Err(SourceCommandError::Invalid(
            "private assessment schema path budget",
        ));
    }
    Ok(())
}

fn profile_routes(
    entity_registry: &JsonValue,
    profile_type_id: &str,
    schemas: &mut BTreeSet<String>,
) -> SourceCommandResult<()> {
    let Some(profile) = cmd::array(entity_registry, "types")?
        .iter()
        .find(|row| cmd::text(row, "type_id").ok() == Some(profile_type_id))
        .and_then(|row| row.object_get("source_record_profile"))
    else {
        insert_schema(schemas, "ToS/contracts/corpus-record.schema.json")?;
        insert_schema(schemas, "ToS/contracts/source-metadata-record.schema.json")?;
        return Ok(());
    };
    for route in cmd::array(profile, "schemas")? {
        for dependency in cmd::array(route, "schema_dependencies")? {
            let reference = dependency.as_str().ok_or(SourceCommandError::Invalid(
                "profile schema dependency reference",
            ))?;
            insert_schema(schemas, reference)?;
        }
        insert_schema(schemas, cmd::text(route, "schema_ref")?)?;
    }
    insert_schema(schemas, "ToS/contracts/corpus-record.schema.json")?;
    insert_schema(schemas, "ToS/contracts/source-metadata-record.schema.json")?;
    Ok(())
}

fn relation_routes(
    relation_registry: &JsonValue,
    relation_type_id: &str,
    schemas: &mut BTreeSet<String>,
) -> SourceCommandResult<()> {
    let row = cmd::array(relation_registry, "relations")?
        .iter()
        .find(|row| cmd::text(row, "relation_type_id").ok() == Some(relation_type_id))
        .ok_or(SourceCommandError::Denied(
            "private assessment Claim relation is absent from selected registry",
        ))?;
    if let Some(profile) = row.object_get("source_claim_profile") {
        for route in cmd::array(profile, "schemas")? {
            for dependency in cmd::array(route, "schema_dependencies")? {
                let reference = dependency.as_str().ok_or(SourceCommandError::Invalid(
                    "Claim schema dependency reference",
                ))?;
                insert_schema(schemas, reference)?;
            }
            insert_schema(schemas, cmd::text(route, "schema_ref")?)?;
        }
    } else {
        insert_schema(schemas, "ToS/contracts/source-claim-record.schema.json")?;
    }
    Ok(())
}

fn current_package_member<'a>(
    histories: &'a BTreeMap<String, PrivateOwnerPackageHistoryV1>,
    target_ref: &str,
    member: &str,
) -> SourceCommandResult<&'a [u8]> {
    let history = histories
        .get(target_ref)
        .ok_or(SourceCommandError::Conflict(
            "assessment private source package was not held",
        ))?;
    if history.target_ref != target_ref {
        return Err(SourceCommandError::Conflict(
            "assessment private source target binding differs",
        ));
    }
    let current = history
        .current
        .as_ref()
        .ok_or(SourceCommandError::Conflict(
            "assessment private source package is absent",
        ))?;
    if current.target_ref != target_ref {
        return Err(SourceCommandError::Conflict(
            "assessment private source package head differs",
        ));
    }
    current
        .members
        .get(member)
        .map(Vec::as_slice)
        .ok_or(SourceCommandError::Conflict(
            "assessment selected private source member is absent",
        ))
}

fn selected_package_member<'a>(
    owner: &OwnerTextContext,
    histories: &'a BTreeMap<String, PrivateOwnerPackageHistoryV1>,
    reference: &str,
) -> SourceCommandResult<Option<&'a [u8]>> {
    if !reference.starts_with(owner.private_prefix()) {
        return Ok(None);
    }
    let target_ref = owner.private_package_target_ref(reference)?;
    let member = reference
        .rsplit('/')
        .next()
        .ok_or(SourceCommandError::Invalid(
            "assessment private package member path",
        ))?;
    current_package_member(histories, &target_ref, member).map(Some)
}

pub(crate) fn add_private_target(
    owner: &OwnerTextContext,
    reference: &str,
    targets: &mut BTreeSet<String>,
) -> SourceCommandResult<()> {
    let path = RelativePath::parse(reference)
        .map_err(|_| SourceCommandError::Invalid("assessment selected package path"))?;
    if path.as_str() != reference {
        return Err(SourceCommandError::Invalid(
            "assessment selected package path",
        ));
    }
    let components = path.as_str().split('/').collect::<Vec<_>>();
    if !reference.starts_with("ToS/source-witnesses/")
        || components.iter().any(|part| {
            part.starts_with('.') || matches!(*part, "catalog" | "payload" | "local-content")
        })
    {
        return Err(SourceCommandError::Denied(
            "assessment selected source leaves metadata paths",
        ));
    }
    if reference.starts_with("ToS/source-witnesses/owner-local/")
        && !reference.starts_with(owner.private_prefix())
    {
        return Err(SourceCommandError::Denied(
            "assessment selected source leaves its private owner store",
        ));
    }
    if reference.starts_with(owner.private_prefix()) {
        targets.insert(owner.private_package_target_ref(reference)?);
    }
    Ok(())
}

fn add_required_private_target(
    owner: &OwnerTextContext,
    reference: &str,
    targets: &mut BTreeSet<String>,
    form_set: bool,
    claim_stream: bool,
) -> SourceCommandResult<()> {
    let suffix = if form_set {
        ".human-forms.json"
    } else if claim_stream {
        ".jsonl"
    } else {
        ".json"
    };
    if !reference.starts_with(owner.private_prefix())
        || !reference.ends_with(suffix)
        || (!form_set && reference.ends_with(".human-forms.json"))
    {
        return Err(SourceCommandError::Denied(
            "assessment owner-local source path",
        ));
    }
    let prefix_components = owner
        .private_prefix()
        .trim_end_matches('/')
        .split('/')
        .count();
    let components = reference.split('/').collect::<Vec<_>>();
    if components.len() < prefix_components + 2 {
        return Err(SourceCommandError::Denied(
            "assessment owner-local source package path",
        ));
    }
    add_private_target(owner, reference, targets)
}

fn validate_claim_stream_target(
    owner: &OwnerTextContext,
    reference: &str,
) -> SourceCommandResult<()> {
    let tail = reference
        .strip_prefix(owner.private_prefix())
        .ok_or(SourceCommandError::Denied(
            "private assessment Claim stream must stay in the selected store",
        ))?;
    let parts = tail.split('/').collect::<Vec<_>>();
    let slug = parts.get(1).copied().unwrap_or_default();
    if parts.len() != 3
        || parts[0] != "claims"
        || slug.is_empty()
        || !slug.split(['.', '-']).all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
        || parts[2] != "source-claims.jsonl"
    {
        return Err(SourceCommandError::Denied(
            "private assessment Claim stream target",
        ));
    }
    Ok(())
}

pub(crate) fn add_binding_targets(
    owner: &OwnerTextContext,
    binding: &JsonValue,
    targets: &mut BTreeSet<String>,
) -> SourceCommandResult<()> {
    cmd::exact_keys(
        binding,
        &[
            "schema_version",
            "packet_ref",
            "packet_sha256",
            "packet_id",
            "packet_version",
            "segmentation_id",
            "segmentation_version",
            "unit_id",
            "unit_version",
            "ordered_anchor_refs",
            "text_layer",
            "source_record_refs",
        ],
    )?;
    for reference in [cmd::text(binding, "packet_ref")?].into_iter().chain(
        ["work", "expression", "edition", "item"]
            .into_iter()
            .map(|kind| cmd::text(cmd::field(binding, "source_record_refs")?, kind))
            .collect::<SourceCommandResult<Vec<_>>>()?,
    ) {
        add_private_target(owner, reference, targets)?;
    }
    let layer = cmd::field(binding, "text_layer")?;
    cmd::exact_keys(
        layer,
        &["record_ref", "record_sha256", "layer_id", "layer_version"],
    )?;
    add_private_target(owner, cmd::text(layer, "record_ref")?, targets)
}

/// Derive only the private packages named by the protected v5/v6 Assessment
/// source grants. Public references are intentionally omitted because they
/// remain under the PG source read session.
pub(crate) fn private_source_target_refs(
    owner: &OwnerTextContext,
    configuration: &JsonValue,
) -> SourceCommandResult<Vec<String>> {
    if !matches!(
        cmd::text(configuration, "schema_version")?,
        "tos_local_assessment_owner_v5" | "tos_local_assessment_owner_v6"
    ) {
        return Err(SourceCommandError::Denied(
            "private assessment PG owner v5/v6 required",
        ));
    }
    let mut targets = BTreeSet::new();
    let selections = cmd::array(configuration, "owner_local_source_records")?;
    if selections.len() > MAX_SOURCE_SELECTIONS {
        return Err(SourceCommandError::Invalid(
            "private assessment source selection count budget",
        ));
    }
    for selection in selections {
        cmd::exact_keys(
            selection,
            &[
                "path",
                "record_id",
                "profile_type_id",
                "origin_id",
                "source_access",
                "source_binding",
                "form_ids",
            ],
        )?;
        let path = cmd::text(selection, "path")?;
        add_required_private_target(owner, path, &mut targets, false, false)?;
        if !cmd::array(selection, "form_ids")?.is_empty() {
            add_required_private_target(
                owner,
                &format!("{}.human-forms.json", path.trim_end_matches(".json")),
                &mut targets,
                true,
                false,
            )?;
        }
        let binding = cmd::field(selection, "source_binding")?;
        if !binding.is_null() {
            add_binding_targets(owner, binding, &mut targets)?;
        }
    }

    let units = cmd::array(configuration, "native_text_units")?;
    if units.len() > MAX_SOURCE_SELECTIONS {
        return Err(SourceCommandError::Invalid(
            "private assessment native unit selection count budget",
        ));
    }
    for selection in units {
        cmd::exact_keys(selection, &["binding", "origin_id", "source_access"])?;
        add_binding_targets(owner, cmd::field(selection, "binding")?, &mut targets)?;
    }

    if let Some(claims) = configuration.object_get("owner_local_source_claims") {
        let rows = claims.as_array().ok_or(SourceCommandError::Invalid(
            "private assessment Claim selection array",
        ))?;
        if rows.len() > MAX_CLAIM_SELECTIONS {
            return Err(SourceCommandError::Invalid(
                "private assessment Claim selection count budget",
            ));
        }
        for claim in rows {
            cmd::exact_keys(
                claim,
                &[
                    "path",
                    "claim_id",
                    "relation_type_id",
                    "origin_id",
                    "source_access",
                    "source_records",
                    "native_bindings",
                    "verify_content",
                    "form_ids",
                ],
            )?;
            let path = cmd::text(claim, "path")?;
            validate_claim_stream_target(owner, path)?;
            add_required_private_target(owner, path, &mut targets, false, true)?;
            if !cmd::array(claim, "form_ids")?.is_empty() {
                let parent = path.rsplit_once('/').map(|(parent, _)| parent).ok_or(
                    SourceCommandError::Invalid("private Claim form package path"),
                )?;
                let form_name = format!(
                    "{parent}/source-claims.{}.human-forms.json",
                    tos_foundation::Digest256::of_bytes(cmd::text(claim, "claim_id")?.as_bytes(),)
                        .to_hex()
                );
                add_required_private_target(owner, &form_name, &mut targets, true, false)?;
            }
            for endpoint in cmd::array(claim, "source_records")? {
                cmd::exact_keys(
                    endpoint,
                    &[
                        "path",
                        "record_id",
                        "profile_type_id",
                        "origin_id",
                        "source_access",
                        "source_binding",
                    ],
                )?;
                add_private_target(owner, cmd::text(endpoint, "path")?, &mut targets)?;
                let binding = cmd::field(endpoint, "source_binding")?;
                if !binding.is_null() {
                    add_binding_targets(owner, binding, &mut targets)?;
                }
            }
            for native in cmd::array(claim, "native_bindings")? {
                cmd::exact_keys(native, &["binding", "origin_id", "source_access"])?;
                add_binding_targets(owner, cmd::field(native, "binding")?, &mut targets)?;
            }
        }
    }

    let layers = cmd::array(configuration, "native_text_layers")?;
    if layers.len() > 32 {
        return Err(SourceCommandError::Invalid(
            "private assessment native layer selection count budget",
        ));
    }
    for selection in layers {
        let binding = cmd::field(selection, "binding")?;
        cmd::exact_keys(
            binding,
            &["schema_version", "text_layer", "source_record_refs"],
        )?;
        let layer = cmd::field(binding, "text_layer")?;
        cmd::exact_keys(
            layer,
            &["record_ref", "record_sha256", "layer_id", "layer_version"],
        )?;
        add_private_target(owner, cmd::text(layer, "record_ref")?, &mut targets)?;
        let source_refs = cmd::field(binding, "source_record_refs")?;
        cmd::exact_keys(source_refs, &["work", "expression", "edition", "item"])?;
        for kind in ["work", "expression", "edition", "item"] {
            add_private_target(owner, cmd::text(source_refs, kind)?, &mut targets)?;
        }
    }

    if targets.len()
        > crate::durable_private_packages::MAX_PACKAGE_HISTORY_TARGETS.saturating_sub(1)
    {
        return Err(SourceCommandError::Invalid(
            "private assessment source package target budget",
        ));
    }
    Ok(targets.into_iter().collect())
}

/// Return the schema closure needed before an asserted-identity worker is
/// built. Registry JSON is supplied from the same selected source session;
/// private Claim streams and profile packages come only from the held broker
/// history map named by `private_source_target_refs`.
pub(crate) fn private_source_schema_paths(
    owner: &OwnerTextContext,
    configuration: &JsonValue,
    package_histories: &BTreeMap<String, PrivateOwnerPackageHistoryV1>,
    entity_registry_raw: &[u8],
    relation_registry_raw: &[u8],
) -> SourceCommandResult<Vec<String>> {
    let targets = private_source_target_refs(owner, configuration)?;
    let mut package_generation = None;
    for target_ref in &targets {
        let history = package_histories
            .get(target_ref)
            .ok_or(SourceCommandError::Conflict(
                "assessment private source package was not held",
            ))?;
        if history.target_ref != *target_ref || history.current.is_none() {
            return Err(SourceCommandError::Conflict(
                "assessment private source package is absent or mismatched",
            ));
        }
        match package_generation {
            None => package_generation = Some(history.private_audit_generation),
            Some(generation) if generation == history.private_audit_generation => (),
            Some(_) => {
                return Err(SourceCommandError::Conflict(
                    "assessment private source package generations differ",
                ));
            }
        }
    }
    let entity_registry = cmd::parse(entity_registry_raw)?;
    let relation_registry = cmd::parse(relation_registry_raw)?;
    if cmd::text(&entity_registry, "schema_version")? != "tos_semantic_entity_type_registry_v1"
        || cmd::text(&relation_registry, "schema_version")?
            != "tos_semantic_relation_type_registry_v1"
    {
        return Err(SourceCommandError::Conflict(
            "assessment selected semantic registries have another identity",
        ));
    }
    let mut schemas = BTreeSet::new();
    for reference in FIXED_ASSESSMENT_SCHEMAS {
        insert_schema(&mut schemas, reference)?;
    }

    let selections = cmd::array(configuration, "owner_local_source_records")?;
    for selection in selections {
        profile_routes(
            &entity_registry,
            cmd::text(selection, "profile_type_id")?,
            &mut schemas,
        )?;
    }

    let units = cmd::array(configuration, "native_text_units")?;
    for selection in units {
        let binding = cmd::field(selection, "binding")?;
        let source_refs = cmd::field(binding, "source_record_refs")?;
        for kind in ["work", "expression", "edition", "item"] {
            profile_routes(
                &entity_registry,
                &format!("tos.entity.{kind}"),
                &mut schemas,
            )?;
            let reference = cmd::text(source_refs, kind)?;
            if let Some(raw) = selected_package_member(owner, package_histories, reference)? {
                let value = cmd::parse(raw)?;
                if let Some(profile_type_id) = value.object_get("profile_type_id") {
                    profile_routes(
                        &entity_registry,
                        profile_type_id.as_str().ok_or(SourceCommandError::Invalid(
                            "selected native source profile identity",
                        ))?,
                        &mut schemas,
                    )?;
                }
            }
        }
    }

    if let Some(claims) = configuration.object_get("owner_local_source_claims") {
        for selection in claims.as_array().ok_or(SourceCommandError::Invalid(
            "private assessment Claim selection array",
        ))? {
            let relation = cmd::text(selection, "relation_type_id")?;
            relation_routes(&relation_registry, relation, &mut schemas)?;
            let path = cmd::text(selection, "path")?;
            let raw = selected_package_member(owner, package_histories, path)?.ok_or(
                SourceCommandError::Denied("private assessment Claim stream must be owner-local"),
            )?;
            if raw.len() > 8_388_608 {
                return Err(SourceCommandError::Invalid(
                    "private assessment Claim stream byte budget",
                ));
            }
            let text = std::str::from_utf8(raw).map_err(|_| {
                SourceCommandError::Invalid("private assessment Claim stream UTF-8")
            })?;
            let mut line_count = 0usize;
            for line in text.lines() {
                if line.is_empty() {
                    continue;
                }
                line_count = line_count
                    .checked_add(1)
                    .ok_or(SourceCommandError::Invalid(
                        "private assessment Claim line count overflow",
                    ))?;
                if line_count > 65_536 {
                    return Err(SourceCommandError::Invalid(
                        "private assessment Claim stream line budget",
                    ));
                }
                let claim = cmd::parse(line.as_bytes())?;
                if let Some(relation_id) = claim.object_get("relation_type_id") {
                    let relation_id = relation_id.as_str().ok_or(SourceCommandError::Invalid(
                        "private assessment Claim relation identity",
                    ))?;
                    relation_routes(&relation_registry, relation_id, &mut schemas)?;
                }
                if let Some(source_records) = claim.object_get("source_records") {
                    for source in source_records
                        .as_array()
                        .ok_or(SourceCommandError::Invalid(
                            "private assessment Claim source records",
                        ))?
                    {
                        if let Some(profile_type_id) = source.object_get("profile_type_id") {
                            profile_routes(
                                &entity_registry,
                                profile_type_id.as_str().ok_or(SourceCommandError::Invalid(
                                    "private assessment Claim source profile identity",
                                ))?,
                                &mut schemas,
                            )?;
                        }
                    }
                }
            }
            for endpoint in cmd::array(selection, "source_records")? {
                profile_routes(
                    &entity_registry,
                    cmd::text(endpoint, "profile_type_id")?,
                    &mut schemas,
                )?;
            }
            for native in cmd::array(selection, "native_bindings")? {
                insert_schema(
                    &mut schemas,
                    "ToS/contracts/native-text-unit-binding.schema.json",
                )?;
                insert_schema(
                    &mut schemas,
                    "ToS/contracts/native-text-unit-assessment-subject.schema.json",
                )?;
                let binding = cmd::field(native, "binding")?;
                if let Some(source_refs) = binding.object_get("source_record_refs") {
                    for kind in ["work", "expression", "edition", "item"] {
                        profile_routes(
                            &entity_registry,
                            &format!("tos.entity.{kind}"),
                            &mut schemas,
                        )?;
                        let reference = cmd::text(source_refs, kind)?;
                        if let Some(raw) =
                            selected_package_member(owner, package_histories, reference)?
                        {
                            if let Ok(source) = cmd::parse(raw) {
                                if let Some(profile_type_id) = source.object_get("profile_type_id")
                                {
                                    profile_routes(
                                        &entity_registry,
                                        profile_type_id.as_str().ok_or(
                                            SourceCommandError::Invalid(
                                                "selected native source profile identity",
                                            ),
                                        )?,
                                        &mut schemas,
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let layers = cmd::array(configuration, "native_text_layers")?;
    if !layers.is_empty() {
        for reference in [
            "ToS/contracts/native-text-layer-binding.schema.json",
            "ToS/contracts/source-text-layer.schema.json",
            "ToS/contracts/native-text-layer-comparison.schema.json",
            "ToS/contracts/native-text-layer-derivation-comparison.schema.json",
            "ToS/contracts/native-page-ocr-comparison.schema.json",
            "ToS/contracts/source-anchor-v2.schema.json",
            "ToS/contracts/source-item-manifest.schema.json",
            "ToS/contracts/rights-record.schema.json",
        ] {
            insert_schema(&mut schemas, reference)?;
        }
        for selection in layers {
            let binding = cmd::field(selection, "binding")?;
            for kind in ["work", "expression", "edition", "item"] {
                profile_routes(
                    &entity_registry,
                    &format!("tos.entity.{kind}"),
                    &mut schemas,
                )?;
                let reference = cmd::text(cmd::field(binding, "source_record_refs")?, kind)?;
                if let Some(raw) = selected_package_member(owner, package_histories, reference)? {
                    let value = cmd::parse(raw)?;
                    if let Some(profile_type_id) = value.object_get("profile_type_id") {
                        profile_routes(
                            &entity_registry,
                            profile_type_id.as_str().ok_or(SourceCommandError::Invalid(
                                "selected layer source profile identity",
                            ))?,
                            &mut schemas,
                        )?;
                    }
                }
            }
        }
    }
    Ok(schemas.into_iter().collect())
}

#[derive(Clone)]
struct PgSourceSelection {
    path: String,
    record_id: String,
    profile_type_id: String,
    origin_id: String,
    source_access: JsonValue,
    source_binding: Option<JsonValue>,
    form_ids: Vec<String>,
}

fn source_access(value: &JsonValue) -> SourceCommandResult<String> {
    cmd::exact_keys(value, &["read_scope", "access_allowed", "authority_ref"])?;
    let scope = cmd::text(value, "read_scope")?;
    if !matches!(scope, "metadata_only" | "exact_owner_local")
        || cmd::field(value, "access_allowed")? != &JsonValue::Bool(true)
        || !cmd::nonblank(cmd::text(value, "authority_ref")?)
    {
        return Err(SourceCommandError::Denied(
            "private assessment requires explicit source access",
        ));
    }
    Ok(scope.to_owned())
}

fn context_selection_matches_pg(
    owner: &OwnerTextContext,
    selected_context: &JsonValue,
) -> SourceCommandResult<()> {
    cmd::exact_keys(
        selected_context,
        &[
            "schema_version",
            "store_id",
            "public_root",
            "private_root",
            "private_prefix",
        ],
    )?;
    let prefix = owner.private_prefix();
    let store_id =
        prefix
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .ok_or(SourceCommandError::Denied(
                "private assessment selected store identity",
            ))?;
    if cmd::text(selected_context, "schema_version")? != "tos_owner_local_source_context_v1"
        || cmd::text(selected_context, "store_id")? != store_id
        || cmd::text(selected_context, "public_root")? != owner.public_root().to_str().unwrap_or("")
        || cmd::text(selected_context, "private_root")?
            != owner.private_root().to_str().unwrap_or("")
        || cmd::text(selected_context, "private_prefix")? != prefix
    {
        return Err(SourceCommandError::Conflict(
            "private assessment owner context selection differs",
        ));
    }
    Ok(())
}

fn collect_binding_references(
    binding: &JsonValue,
    references: &mut BTreeSet<String>,
) -> SourceCommandResult<()> {
    references.insert(cmd::text(binding, "packet_ref")?.to_owned());
    let source_refs = cmd::field(binding, "source_record_refs")?;
    for kind in ["work", "expression", "edition", "item"] {
        references.insert(cmd::text(source_refs, kind)?.to_owned());
    }
    references.insert(cmd::text(cmd::field(binding, "text_layer")?, "record_ref")?.to_owned());
    Ok(())
}

fn explicit_public_references(configuration: &JsonValue) -> SourceCommandResult<BTreeSet<String>> {
    let mut references = BTreeSet::new();
    for selection in cmd::array(configuration, "owner_local_source_records")? {
        if let JsonValue::Object(_) = cmd::field(selection, "source_binding")? {
            collect_binding_references(cmd::field(selection, "source_binding")?, &mut references)?;
        }
    }
    for selection in cmd::array(configuration, "native_text_units")? {
        collect_binding_references(cmd::field(selection, "binding")?, &mut references)?;
    }
    if let Some(claims) = configuration.object_get("owner_local_source_claims") {
        for claim in claims.as_array().ok_or(SourceCommandError::Invalid(
            "private assessment Claim selection array",
        ))? {
            for endpoint in cmd::array(claim, "source_records")? {
                references.insert(cmd::text(endpoint, "path")?.to_owned());
                if let JsonValue::Object(_) = cmd::field(endpoint, "source_binding")? {
                    collect_binding_references(
                        cmd::field(endpoint, "source_binding")?,
                        &mut references,
                    )?;
                }
            }
            for native in cmd::array(claim, "native_bindings")? {
                collect_binding_references(cmd::field(native, "binding")?, &mut references)?;
            }
        }
    }
    for selection in cmd::array(configuration, "native_text_layers")? {
        let binding = cmd::field(selection, "binding")?;
        references.insert(cmd::text(cmd::field(binding, "text_layer")?, "record_ref")?.to_owned());
        for kind in ["work", "expression", "edition", "item"] {
            references
                .insert(cmd::text(cmd::field(binding, "source_record_refs")?, kind)?.to_owned());
        }
    }
    Ok(references)
}

fn selected_record_read_files(
    ctx: &CommandContext,
    private_members: &BTreeMap<String, Vec<u8>>,
    public_members: &BTreeMap<String, Vec<u8>>,
) -> SourceCommandResult<Vec<SourceFile>> {
    let mut files = BTreeMap::<String, Vec<u8>>::new();
    for file in &ctx.files {
        insert_selected_file(&mut files, file.path.as_str(), &file.raw)?;
    }
    for (path, raw) in private_members.iter().chain(public_members.iter()) {
        insert_selected_file(&mut files, path, raw)?;
    }
    files
        .into_iter()
        .map(|(path, raw)| {
            Ok(SourceFile {
                path: RelativePath::parse(&path).map_err(|_| {
                    SourceCommandError::Invalid("private assessment selected source path")
                })?,
                raw,
            })
        })
        .collect()
}

fn digest_snapshot(value: &JsonValue) -> SourceCommandResult<String> {
    Ok(tos_foundation::Digest256::of_bytes(&cmd::canonical(value)?).to_prefixed())
}

fn selected_member_bytes(
    current: &PgOwnerAssessmentCurrent<'_, '_, '_>,
    reference: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let mut found = None;
    PrivateSelectedSourceInput::with_current_member(
        current,
        reference,
        MAX_PRIVATE_SELECTED_MEMBER_BYTES,
        deadline,
        cancelled,
        &mut |_, raw| {
            if found.replace(raw.to_vec()).is_some() {
                return Err(tos_validation::item_rules::ItemRefusal::Source(
                    "assessment selected member repeated".into(),
                ));
            }
            Ok(())
        },
    )
    .map_err(|_| SourceCommandError::Conflict("assessment selected member read failed"))?;
    found.ok_or(SourceCommandError::Conflict(
        "assessment selected member is absent",
    ))
}

fn retain_pg_native_resolution(
    owner: &OwnerTextContext,
    input: &PgGrowthCurrentInput<'_, '_>,
    public_members: &mut BTreeMap<String, Vec<u8>>,
    resolved: crate::source_sign_native::ResolvedSignNative,
    allow_content: bool,
    origin_id: &str,
    read_scope: &str,
    supporting_only: bool,
    native_inputs: &mut BTreeMap<(String, String, &'static str), NativeInput>,
    native_snapshots: &mut Vec<String>,
    native_summaries: &mut Vec<JsonValue>,
    native_records: &mut Vec<AssessmentRecordInput>,
    source_dependencies: &mut BTreeMap<String, String>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let content_verified = cmd::field(&resolved.summary, "content_verified")?
        .as_bool()
        .ok_or(SourceCommandError::Invalid(
            "native assessment content status",
        ))?;
    let mut observed_content = false;
    for native in &resolved.inputs {
        let bytes = if owner.is_owner_private_reference(&native.reference) {
            owner.read(
                &native.reference,
                native.raw_size.max(1),
                deadline,
                cancelled,
            )?
        } else {
            public_members
                .get(&native.reference)
                .cloned()
                .ok_or(SourceCommandError::Conflict(
                    "native assessment public bytes were not retained",
                ))?
        };
        if bytes.len() != native.raw_size
            || tos_foundation::Digest256::of_bytes(&bytes) != native.raw_sha256
        {
            return Err(SourceCommandError::Conflict(
                "native assessment input differs from selected bytes",
            ));
        }
        if native.kind == NativeReadKind::Content {
            if !allow_content || !owner.is_owner_private_reference(&native.reference) {
                return Err(SourceCommandError::Denied(
                    "native assessment content is outside exact owner-local scope",
                ));
            }
            observed_content = true;
        }
        let kind = match native.kind {
            NativeReadKind::Metadata => "metadata",
            NativeReadKind::Schema => "schema",
            NativeReadKind::Support => "support",
            NativeReadKind::Content => "content",
        }
        .to_owned();
        let key = (native.reference.clone(), kind, native.category);
        if native_inputs
            .insert(key, native.clone())
            .is_some_and(|previous| previous != *native)
        {
            return Err(SourceCommandError::Conflict(
                "native assessment selected input changed",
            ));
        }
    }
    if content_verified != observed_content {
        return Err(SourceCommandError::Conflict(
            "native assessment content report differs from selected bytes",
        ));
    }
    for (path, digest) in &resolved.schema_digests {
        if source_dependencies
            .insert(path.clone(), digest.to_prefixed())
            .is_some_and(|previous| previous != digest.to_prefixed())
        {
            return Err(SourceCommandError::Conflict(
                "private Assessment schema dependency changed",
            ));
        }
    }
    native_snapshots.push(resolved.input_snapshot);
    let record_refs = resolved
        .records
        .iter()
        .map(|record| {
            Ok(cmd::object(vec![
                ("id", cmd::field(record, "id")?.clone()),
                ("version", cmd::field(record, "version")?.clone()),
                (
                    "digest",
                    cmd::string(&cmd::record_digest(cmd::field(record, "payload")?)?.to_prefixed()),
                ),
            ]))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let mut summary = resolved.summary;
    cmd::set(&mut summary, "origin_id", cmd::string(origin_id))?;
    cmd::set(&mut summary, "read_scope", cmd::string(read_scope))?;
    cmd::set(&mut summary, "record_refs", JsonValue::Array(record_refs))?;
    cmd::set(
        &mut summary,
        "supporting_only",
        JsonValue::Bool(supporting_only),
    )?;
    native_summaries.push(summary);
    for record in resolved.records {
        cmd::exact_keys(&record, &["id", "version", "payload", "origin_id"])?;
        if cmd::canonical(cmd::field(&record, "payload")?)?.len() > MAX_RECORD_BYTES {
            return Err(SourceCommandError::Unsupported(
                "native assessment record budget",
            ));
        }
        native_records.push(AssessmentRecordInput {
            envelope: cmd::canonical(&record)?,
        });
    }
    let _ = input; // all public reads were bound by the supplied reader.
    Ok(())
}

fn envelope(
    id: &str,
    version: &JsonValue,
    payload: &JsonValue,
    origin_id: &str,
) -> SourceCommandResult<AssessmentRecordInput> {
    let number = version
        .as_u64()
        .filter(|value| *value > 0)
        .ok_or(SourceCommandError::Invalid(
            "private assessment source record version",
        ))?;
    if !cmd::nonblank(id) || !cmd::nonblank(origin_id) {
        return Err(SourceCommandError::Invalid(
            "private assessment source record identity",
        ));
    }
    let value = cmd::object(vec![
        ("id", cmd::string(id)),
        ("version", cmd::number(number)),
        ("payload", payload.clone()),
        ("origin_id", cmd::string(origin_id)),
    ]);
    Ok(AssessmentRecordInput {
        envelope: cmd::canonical(&value)?,
    })
}

fn record_reference(record: &JsonValue) -> SourceCommandResult<JsonValue> {
    Ok(cmd::object(vec![
        ("id", cmd::field(record, "id")?.clone()),
        ("version", cmd::field(record, "version")?.clone()),
        (
            "digest",
            cmd::string(&cmd::record_digest(cmd::field(record, "payload")?)?.to_prefixed()),
        ),
    ]))
}

fn source_languages(payload: &JsonValue) -> SourceCommandResult<Vec<String>> {
    let mut languages = BTreeSet::new();
    if let Some(fields) = payload.object_get("field_languages") {
        for (_, field) in fields.as_object().ok_or(SourceCommandError::Invalid(
            "private assessment source field languages",
        ))? {
            if let Some(language) = field.object_get("language").and_then(JsonValue::as_str) {
                if cmd::nonblank(language) {
                    languages.insert(language.to_lowercase());
                }
            }
        }
    }
    for key in ["semantic_scope", "semantic_content", "form_identity"] {
        if let Some(language) = payload
            .object_get(key)
            .and_then(|value| value.object_get("language"))
            .and_then(JsonValue::as_str)
            .filter(|value| cmd::nonblank(value))
        {
            languages.insert(language.to_lowercase());
        }
    }
    Ok(languages.into_iter().collect())
}

/// Select and validate exactly the v5/v6 private Assessment grants against
/// held package histories and the current PG read session. The command context
/// remains bootstrap correspondence; the candidate worker and RecordRead bind
/// the actual PG epoch.
pub(crate) fn select_owner_local_sources_pg(
    owner: &mut OwnerTextContext,
    selected_context: &JsonValue,
    configuration: &JsonValue,
    ctx: &CommandContext,
    input: &PgGrowthCurrentInput<'_, '_>,
    package_histories: &BTreeMap<String, PrivateOwnerPackageHistoryV1>,
    worker: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgPrivateAssessmentSources> {
    if !matches!(
        cmd::text(configuration, "schema_version")?,
        "tos_local_assessment_owner_v5" | "tos_local_assessment_owner_v6"
    ) {
        return Err(SourceCommandError::Denied(
            "private assessment PG owner v5/v6 required",
        ));
    }
    context_selection_matches_pg(owner, selected_context)?;
    let identity = *SourceCutInputWithIdentity::input_identity(input);
    let selected_cut = PgOwnerAssessmentCurrent {
        owner,
        input,
        histories: package_histories,
    };
    let targets = private_source_target_refs(owner, configuration)?;
    let (private_members, private_heads, _private_generation) =
        selected_private_package_members(owner, package_histories, &targets)?;
    if !private_members.is_empty() {
        install_selected_members(owner, private_members.clone())?;
    }
    let mut public_members = BTreeMap::new();
    for reference in explicit_public_references(configuration)? {
        if owner.is_owner_private_reference(&reference) {
            continue;
        }
        // Direct owner bindings may name only public source metadata. Payload
        // content is separately granted and handled by the native reader.
        if !reference.starts_with("ToS/source-witnesses/") {
            continue;
        }
        let raw = read_selected_public_member(
            input,
            &reference,
            MAX_PUBLIC_SELECTED_MEMBER_BYTES,
            deadline,
            cancelled,
        )?;
        insert_selected_file(&mut public_members, &reference, &raw)?;
        install_selected_members(owner, BTreeMap::from([(reference, raw)]))?;
    }
    let mut selected_files = selected_record_read_files(ctx, &private_members, &public_members)?;
    if selected_files.len() > MAX_SELECTED_PUBLIC_MEMBERS
        || selected_files
            .iter()
            .map(|file| file.raw.len())
            .sum::<usize>()
            > MAX_SELECTED_PUBLIC_BYTES
    {
        return Err(SourceCommandError::Unsupported(
            "private assessment PG selected RecordRead budget",
        ));
    }
    // Keep this binding explicit so the exact epoch fences both the selected
    // bytes and every candidate schema check.
    let selected = PgOwnerAssessmentRecordRead {
        command: ctx,
        current: &selected_cut,
        identity,
        files: std::mem::take(&mut selected_files),
    };
    crate::source_native_cli::private_owner::profile::candidate_record_binding_matches(
        ctx,
        &selected_cut,
        &selected,
        worker,
    )?;
    let context_snapshot = owner.snapshot(deadline, cancelled)?.to_prefixed();

    let selection_rows = cmd::object(vec![
        (
            "owner_local_source_records",
            cmd::field(configuration, "owner_local_source_records")?.clone(),
        ),
        (
            "native_text_units",
            cmd::field(configuration, "native_text_units")?.clone(),
        ),
        (
            "owner_local_source_claims",
            configuration
                .object_get("owner_local_source_claims")
                .cloned()
                .unwrap_or(JsonValue::Array(Vec::new())),
        ),
    ]);
    if cmd::canonical(&selection_rows)?.len() > 1_048_576 {
        return Err(SourceCommandError::Unsupported(
            "private assessment PG selection byte budget",
        ));
    }
    let mut selections = Vec::new();
    let mut identities = BTreeSet::new();
    let rows = cmd::array(configuration, "owner_local_source_records")?;
    if rows.len() > MAX_SOURCE_SELECTIONS {
        return Err(SourceCommandError::Unsupported(
            "private assessment PG source count budget",
        ));
    }
    for row in rows {
        let path = cmd::text(row, "path")?.to_owned();
        let id = cmd::text(row, "record_id")?.to_owned();
        if !path.starts_with(owner.private_prefix())
            || !path.ends_with(".json")
            || path.ends_with(".human-forms.json")
            || !identities.insert(id.clone())
        {
            return Err(SourceCommandError::Invalid(
                "private assessment PG source identity or path",
            ));
        }
        let forms = cmd::array(row, "form_ids")?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .map(str::to_owned)
                    .ok_or(SourceCommandError::Invalid(
                        "private assessment PG form identity",
                    ))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?;
        source_access(cmd::field(row, "source_access")?)?;
        selections.push(PgSourceSelection {
            path,
            record_id: id,
            profile_type_id: cmd::text(row, "profile_type_id")?.to_owned(),
            origin_id: cmd::text(row, "origin_id")?.to_owned(),
            source_access: cmd::field(row, "source_access")?.clone(),
            source_binding: match cmd::field(row, "source_binding")? {
                JsonValue::Null => None,
                value @ JsonValue::Object(_) => Some(value.clone()),
                _ => return Err(SourceCommandError::Invalid("private assessment PG binding")),
            },
            form_ids: forms,
        });
    }

    let claim_value = configuration
        .object_get("owner_local_source_claims")
        .cloned()
        .unwrap_or(JsonValue::Array(Vec::new()));
    let claim_selections =
        crate::source_native_cli::private_owner::claim::preflight_assessment_claim_selections(
            &claim_value,
            selected_context,
        )?;
    for claim in &claim_selections {
        if !identities.insert(claim.claim_id.clone())
            || claim
                .form_ids
                .iter()
                .any(|id| !identities.insert(id.clone()))
        {
            return Err(SourceCommandError::Invalid(
                "private assessment source and Claim identities overlap",
            ));
        }
    }
    let native_selections = cmd::array(configuration, "native_text_units")?;
    for native in native_selections {
        cmd::exact_keys(native, &["binding", "origin_id", "source_access"])?;
        source_access(cmd::field(native, "source_access")?)?;
        let unit_id = cmd::text(cmd::field(native, "binding")?, "unit_id")?.to_owned();
        if !identities.insert(unit_id) {
            return Err(SourceCommandError::Invalid(
                "private assessment native identity overlaps selected source",
            ));
        }
    }

    let mut records = Vec::new();
    let mut native_records = Vec::new();
    let mut native_inputs = BTreeMap::<(String, String, &'static str), NativeInput>::new();
    let mut native_snapshots = Vec::new();
    let mut native_summaries = Vec::new();
    let mut profile_snapshots = Vec::new();
    let mut claim_snapshots = Vec::new();
    let mut source_dependencies = BTreeMap::<String, String>::new();
    let mut required_source_refs = BTreeMap::new();
    let mut required_languages = BTreeMap::new();
    let mut form_sets = BTreeMap::new();
    let mut form_paths = BTreeMap::new();
    let mut source_paths = BTreeMap::new();
    for selection in &selections {
        let raw = current_package_member(
            package_histories,
            &owner.private_package_target_ref(&selection.path)?,
            selection.path.rsplit('/').next().unwrap_or_default(),
        )?;
        if raw.len() > MAX_RECORD_BYTES {
            return Err(SourceCommandError::Unsupported(
                "private assessment PG record budget",
            ));
        }
        let validated =
            crate::source_native_cli::private_owner::profile::validate_identity_record_pg(
                &selection.profile_type_id,
                &selection.path,
                &selection.record_id,
                raw,
                ctx,
                &selected_cut,
                &selected,
                worker,
                deadline,
                cancelled,
            )?;
        source_paths.insert(selection.record_id.clone(), selection.path.clone());
        let mut profile_contracts = BTreeMap::new();
        for dependency in &validated.dependencies {
            let dependency_path = dependency.path.as_str().to_owned();
            let dependency_digest = dependency.raw_sha256.to_hex();
            if profile_contracts
                .insert(dependency_path.clone(), dependency_digest.clone())
                .is_some_and(|previous| previous != dependency_digest)
            {
                return Err(SourceCommandError::Conflict(
                    "private assessment profile schema closure differs",
                ));
            }
            let raw = ctx
                .file(&dependency.path)?
                .ok_or(SourceCommandError::Conflict(
                    "private assessment selected profile schema is absent",
                ))?;
            if tos_foundation::Digest256::of_bytes(raw) != dependency.raw_sha256 {
                return Err(SourceCommandError::Conflict(
                    "private assessment profile schema differs from PG selection",
                ));
            }
            source_dependencies.insert(dependency_path, dependency.raw_sha256.to_prefixed());
        }
        let adapter = validated
            .profile
            .object_get("native_binding_adapter")
            .and_then(JsonValue::as_str);
        let native_metadata_snapshot = match adapter {
            None if selection.source_binding.is_none()
                && validated.record.object_get("native_text_binding").is_none()
                && source_access(&selection.source_access)? == "metadata_only" =>
            {
                None
            }
            Some("source-text-unit-v1") => {
                if cmd::text(&validated.record, "record_type")? != "occurrence" {
                    return Err(SourceCommandError::Denied(
                        "private assessment native profile is not an occurrence",
                    ));
                }
                let binding =
                    selection
                        .source_binding
                        .as_ref()
                        .ok_or(SourceCommandError::Denied(
                            "private assessment occurrence binding is absent",
                        ))?;
                if cmd::canonical(cmd::field(&validated.record, "native_text_binding")?)?
                    != cmd::canonical(binding)?
                {
                    return Err(SourceCommandError::Conflict(
                        "private assessment occurrence differs from selected native binding",
                    ));
                }
                let mut reader =
                    PgOwnerAssessmentReader::new(owner, input, false, context_snapshot.clone());
                let resolved = crate::source_sign_native::resolve_owner_assessment(
                    &mut reader,
                    worker,
                    binding,
                    &selection.origin_id,
                    NativeReadScope::MetadataOnly,
                    deadline,
                    cancelled,
                )?;
                if cmd::field(&resolved.summary, "content_verified")? != &JsonValue::Bool(false) {
                    return Err(SourceCommandError::Denied(
                        "private assessment profile resolver disclosed content",
                    ));
                }
                let snapshot = resolved.input_snapshot.clone();
                // Keep the metadata preflight in the profile snapshot, but
                // publish the evidence view requested by this selection. A
                // metadata-only preflight is not a second current TextUnit
                // beside the same binding's exact Claim/Assessment evidence.
                let scope_name = source_access(&selection.source_access)?;
                let allow_content = scope_name == "exact_owner_local";
                let resolved = if allow_content {
                    reader.allow_content = true;
                    crate::source_sign_native::resolve_owner_assessment(
                        &mut reader,
                        worker,
                        binding,
                        &selection.origin_id,
                        NativeReadScope::ExactOwnerLocal,
                        deadline,
                        cancelled,
                    )?
                } else {
                    resolved
                };
                let mut observed = reader.into_public_members();
                for (path, bytes) in observed.clone() {
                    insert_selected_file(&mut public_members, &path, &bytes)?;
                }
                retain_pg_native_resolution(
                    owner,
                    input,
                    &mut observed,
                    resolved,
                    allow_content,
                    &selection.origin_id,
                    &scope_name,
                    true,
                    &mut native_inputs,
                    &mut native_snapshots,
                    &mut native_summaries,
                    &mut native_records,
                    &mut source_dependencies,
                    deadline,
                    cancelled,
                )?;
                Some(snapshot)
            }
            _ => {
                return Err(SourceCommandError::Denied(
                    "private assessment source profile/native binding mismatch",
                ));
            }
        };
        profile_snapshots.push(digest_snapshot(&cmd::object(vec![
            ("context", cmd::string(&context_snapshot)),
            ("source_access", selection.source_access.clone()),
            (
                "source_binding",
                selection.source_binding.clone().unwrap_or(JsonValue::Null),
            ),
            (
                "contracts",
                JsonValue::Object(
                    profile_contracts
                        .into_iter()
                        .map(|(path, digest)| (JsonString::from_utf8(&path), cmd::string(&digest)))
                        .collect(),
                ),
            ),
            (
                "source",
                cmd::string(&tos_foundation::Digest256::of_bytes(raw).to_prefixed()),
            ),
            (
                "native_metadata",
                native_metadata_snapshot
                    .map(|value| cmd::string(&value))
                    .unwrap_or(JsonValue::Null),
            ),
        ]))?);
        let source_envelope = envelope(
            &selection.record_id,
            cmd::field(&validated.record, "record_version")?,
            &validated.record,
            &selection.origin_id,
        )?;
        let source_value = cmd::parse(&source_envelope.envelope)?;
        let source_ref = record_reference(&source_value)?;
        required_source_refs.insert(selection.record_id.clone(), Vec::new());
        let language_set = source_languages(&validated.record)?;
        required_languages.insert(selection.record_id.clone(), language_set.clone());
        records.push(source_envelope);
        for identifier in &selection.form_ids {
            if !identities.insert(identifier.clone()) {
                return Err(SourceCommandError::Invalid(
                    "private assessment PG identity overlaps",
                ));
            }
        }
        if !selection.form_ids.is_empty() {
            let form_path = format!(
                "{}.human-forms.json",
                selection.path.trim_end_matches(".json")
            );
            let form_raw = current_package_member(
                package_histories,
                &owner.private_package_target_ref(&form_path)?,
                form_path.rsplit('/').next().unwrap_or_default(),
            )?;
            let forms = cmd::parse(form_raw)?;
            crate::source_native_cli::private_owner::profile::validate_form_set_pg(
                ctx,
                &selected_cut,
                &selected,
                worker,
                &form_path,
                &forms,
                deadline,
                cancelled,
            )?;
            let subject = crate::source_forms::metadata_subject(&validated.record)?;
            if cmd::canonical(cmd::field(&forms, "subject")?)? != cmd::canonical(&subject)?
                || cmd::canonical(&source_ref)? != cmd::canonical(&subject)?
            {
                return Err(SourceCommandError::Conflict(
                    "private PG form subject changed",
                ));
            }
            tos_validation::source_forms::source_copy_kernel::validate_history(&forms, &subject)
                .map_err(crate::source_forms::form_error)?;
            form_sets.insert(form_path.clone(), forms.clone());
            form_paths.insert(selection.record_id.clone(), form_path.clone());
            for identifier in &selection.form_ids {
                let matching = cmd::array(&forms, "forms")?
                    .iter()
                    .filter(|form| cmd::text(form, "form_id").ok() == Some(identifier.as_str()))
                    .collect::<Vec<_>>();
                if matching.len() != 1 {
                    return Err(SourceCommandError::Invalid(
                        "private PG form is not current",
                    ));
                }
                let form = matching[0];
                required_source_refs.insert(identifier.clone(), vec![source_ref.clone()]);
                let mut languages = language_set.clone();
                languages.extend(source_languages(form)?);
                required_languages.insert(identifier.clone(), languages);
                records.push(envelope(
                    identifier,
                    cmd::field(form, "form_version")?,
                    form,
                    &selection.origin_id,
                )?);
                form_paths.insert(identifier.clone(), form_path.clone());
            }
        }
    }

    let allow_content = claim_selections
        .iter()
        .any(|selection| selection.verify_content)
        || native_selections.iter().any(|row| {
            source_access(cmd::field(row, "source_access").unwrap_or(&JsonValue::Null))
                .is_ok_and(|scope| scope == "exact_owner_local")
        });
    let mut reader =
        PgOwnerAssessmentReader::new(owner, input, allow_content, context_snapshot.clone());
    let claim_sources =
        crate::source_native_cli::private_owner::claim::resolve_assessment_claim_sources_pg(
            owner,
            selected_context,
            &claim_selections,
            ctx,
            &selected_cut,
            &selected,
            worker,
            &mut reader,
            deadline,
            cancelled,
        )?;
    for (reference, raw) in &claim_sources.source_files {
        if selected_member_bytes(&selected_cut, reference, deadline, cancelled)? != *raw {
            return Err(SourceCommandError::Conflict(
                "private Claim source differs from selected PG bytes",
            ));
        }
        if !owner.is_owner_private_reference(reference) {
            insert_selected_file(&mut public_members, reference, raw)?;
        }
    }
    for (path, digest) in &claim_sources.schema_digests {
        if source_dependencies
            .insert(path.clone(), digest.to_prefixed())
            .is_some_and(|previous| previous != digest.to_prefixed())
        {
            return Err(SourceCommandError::Conflict(
                "private Claim schema dependency changed",
            ));
        }
    }
    for native in &claim_sources.native_inputs {
        let raw = selected_member_bytes(&selected_cut, &native.reference, deadline, cancelled)?;
        if raw.len() != native.raw_size
            || tos_foundation::Digest256::of_bytes(&raw) != native.raw_sha256
        {
            return Err(SourceCommandError::Conflict(
                "private Claim native input changed",
            ));
        }
        let kind = match native.kind {
            NativeReadKind::Metadata => "metadata",
            NativeReadKind::Schema => "schema",
            NativeReadKind::Support => "support",
            NativeReadKind::Content => "content",
        }
        .to_owned();
        native_inputs.insert(
            (native.reference.clone(), kind, native.category),
            native.clone(),
        );
    }
    records.extend(
        claim_sources
            .records
            .iter()
            .map(|record| {
                if cmd::canonical(record)
                    .map(|raw| raw.len())
                    .unwrap_or(usize::MAX)
                    > MAX_RECORD_BYTES
                {
                    return Err(SourceCommandError::Unsupported(
                        "private Claim assessment record budget",
                    ));
                }
                Ok(AssessmentRecordInput {
                    envelope: cmd::canonical(record)?,
                })
            })
            .collect::<SourceCommandResult<Vec<_>>>()?,
    );
    native_records.extend(
        claim_sources
            .native_records
            .iter()
            .map(|record| {
                if cmd::canonical(record)
                    .map(|raw| raw.len())
                    .unwrap_or(usize::MAX)
                    > MAX_RECORD_BYTES
                {
                    return Err(SourceCommandError::Unsupported(
                        "private Claim native record budget",
                    ));
                }
                Ok(AssessmentRecordInput {
                    envelope: cmd::canonical(record)?,
                })
            })
            .collect::<SourceCommandResult<Vec<_>>>()?,
    );
    for (identity, refs) in claim_sources.required_source_refs {
        if required_source_refs
            .insert(identity, refs.clone())
            .is_some_and(|previous| previous != refs)
        {
            return Err(SourceCommandError::Conflict(
                "private assessment required-source references collide",
            ));
        }
    }
    for (identity, languages) in claim_sources.required_languages {
        if required_languages
            .insert(identity, languages.clone())
            .is_some_and(|previous| previous != languages)
        {
            return Err(SourceCommandError::Conflict(
                "private assessment source language requirements collide",
            ));
        }
    }
    native_summaries.extend(claim_sources.native_summaries);
    native_snapshots.extend(claim_sources.native_snapshots);
    claim_snapshots.extend(claim_sources.snapshots);
    for (path, forms) in claim_sources.form_sets {
        if form_sets.insert(path, forms).is_some() {
            return Err(SourceCommandError::Conflict(
                "private form set is selected more than once",
            ));
        }
    }
    for (identity, path) in claim_sources.form_paths {
        if form_paths
            .insert(identity, path.clone())
            .is_some_and(|previous| previous != path)
        {
            return Err(SourceCommandError::Conflict(
                "private assessment form paths collide",
            ));
        }
    }
    for (identity, path) in claim_sources.source_paths {
        if source_paths
            .insert(identity, path.clone())
            .is_some_and(|previous| previous != path)
        {
            return Err(SourceCommandError::Conflict(
                "private assessment source paths collide",
            ));
        }
    }

    for selection in native_selections {
        let binding = cmd::field(selection, "binding")?;
        let origin = cmd::text(selection, "origin_id")?;
        let scope_name = source_access(cmd::field(selection, "source_access")?)?;
        let (scope, allow_content) = if scope_name == "exact_owner_local" {
            (NativeReadScope::ExactOwnerLocal, true)
        } else {
            (NativeReadScope::MetadataOnly, false)
        };
        let resolved = crate::source_sign_native::resolve_owner_assessment(
            &mut reader,
            worker,
            binding,
            origin,
            scope,
            deadline,
            cancelled,
        )?;
        let unit_id = cmd::text(binding, "unit_id")?.to_owned();
        let languages = resolved
            .summary
            .object_get("language")
            .and_then(JsonValue::as_str)
            .filter(|value| cmd::nonblank(value))
            .map(|value| vec![value.to_lowercase()])
            .unwrap_or_default();
        retain_pg_native_resolution(
            owner,
            input,
            &mut reader.public_members,
            resolved,
            allow_content,
            origin,
            &scope_name,
            false,
            &mut native_inputs,
            &mut native_snapshots,
            &mut native_summaries,
            &mut native_records,
            &mut source_dependencies,
            deadline,
            cancelled,
        )?;
        required_languages.insert(unit_id, languages);
    }
    // A standalone TextLayer selection also needs its manifest, rights,
    // anchors and retained derivation metadata. Those dependencies must not
    // depend on an unrelated TextUnit happening to populate the PG read set.
    let mut layer_reader =
        PgOwnerAssessmentReader::new(owner, input, false, context_snapshot.clone());
    for selection in cmd::array(configuration, "native_text_layers")? {
        for native in crate::source_sign_native::resolve_owner_text_layer_metadata(
            &mut layer_reader,
            worker,
            cmd::field(selection, "binding")?,
            deadline,
            cancelled,
        )? {
            let kind = match native.kind {
                NativeReadKind::Metadata => "metadata",
                NativeReadKind::Schema => "schema",
                NativeReadKind::Support => "support",
                NativeReadKind::Content => {
                    return Err(SourceCommandError::Denied(
                        "private assessment layer metadata selected content",
                    ));
                }
            };
            let key = (native.reference.clone(), kind.to_owned(), native.category);
            if native_inputs
                .insert(key, native.clone())
                .is_some_and(|previous| previous != native)
            {
                return Err(SourceCommandError::Conflict(
                    "private assessment layer metadata dependency changed",
                ));
            }
        }
    }
    layer_reader.verify_current(deadline, cancelled)?;
    for (path, raw) in layer_reader.into_public_members() {
        insert_selected_file(&mut public_members, &path, &raw)?;
    }
    reader.verify_current(deadline, cancelled)?;
    for (path, raw) in &reader.public_members {
        insert_selected_file(&mut public_members, path, raw)?;
    }
    if owner.snapshot(deadline, cancelled)?.to_prefixed() != context_snapshot {
        return Err(SourceCommandError::Conflict(
            "private assessment owner context changed during PG selection",
        ));
    }
    if records.len().saturating_add(native_records.len()) > MAX_ASSESSMENTS {
        return Err(SourceCommandError::Unsupported(
            "private assessment PG assembled record budget",
        ));
    }
    let package_snapshot = source_package_snapshot(&private_heads)?;
    let claim_rows = JsonValue::Array(
        claim_snapshots
            .iter()
            .map(|value| cmd::string(value))
            .collect(),
    );
    let profile_rows = JsonValue::Array(
        profile_snapshots
            .iter()
            .map(|value| cmd::string(value))
            .collect(),
    );
    let owner_local_sources_snapshot = digest_snapshot(&cmd::object(vec![
        ("context", cmd::string(&context_snapshot)),
        ("profiles", profile_rows),
        ("claims", claim_rows),
        ("packages", cmd::string(&package_snapshot)),
    ]))?;
    Ok(PgPrivateAssessmentSources {
        records,
        native_records,
        native_snapshots,
        native_summaries,
        owner_local_sources_snapshot,
        required_source_refs,
        required_languages,
        form_sets,
        form_paths,
        source_paths,
        selected_members: private_members,
        target_refs: targets,
        context_snapshot,
        private_heads,
        public_members,
    })
}

#[cfg(test)]
mod source_snapshot_tests {
    use super::*;
    use tos_foundation::Digest256;

    #[test]
    fn bootstrap_contract_assessment_snapshot_tracks_sources_not_journal_epoch() {
        let head = PackageHeadMarker {
            revision: 1,
            package_digest: Digest256::of_bytes(b"source package"),
            private_audit_generation: 7,
            members: BTreeMap::from([("source.json".into(), (6, Digest256::of_bytes(b"source")))]),
        };
        let original = BTreeMap::from([("package/source".into(), head)]);
        let expected = source_package_snapshot(&original).unwrap();
        let mut advanced = original.clone();
        advanced
            .get_mut("package/source")
            .unwrap()
            .private_audit_generation += 1;
        assert_eq!(source_package_snapshot(&advanced).unwrap(), expected);
        assert_ne!(advanced, original); // operation-local fence still observes it
        let mut changed = original.clone();
        changed.get_mut("package/source").unwrap().revision += 1;
        assert_ne!(source_package_snapshot(&changed).unwrap(), expected);
        let mut changed = original.clone();
        changed.get_mut("package/source").unwrap().package_digest = Digest256::of_bytes(b"other");
        assert_ne!(source_package_snapshot(&changed).unwrap(), expected);
        let mut changed = original.clone();
        changed
            .get_mut("package/source")
            .unwrap()
            .members
            .insert("source.json".into(), (7, Digest256::of_bytes(b"changed")));
        assert_ne!(source_package_snapshot(&changed).unwrap(), expected);
        let mut changed = original;
        let head = changed.remove("package/source").unwrap();
        changed.insert("package/other".into(), head);
        assert_ne!(source_package_snapshot(&changed).unwrap(), expected);
    }
}
