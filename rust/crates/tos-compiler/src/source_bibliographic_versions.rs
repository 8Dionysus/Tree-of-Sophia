//! Exact metadata/Claim versions and Collection ordering from the immutable source carrier.
//! Positive staged rows are byte-bound to this independently selected cut;
//! only its complete manifest can establish absence of an adjacent history.
use crate::knowledge_normalization::SourceRow;
use crate::knowledge_stage::KnowledgeStage;
use crate::source_bibliographic::{self as graph, BibliographicForms, BibliographicLimits};
use crate::source_bibliographic_render::{array, digest, encode, text};
use crate::source_bibliographic_selected_files::SelectedBibliographicFilesV1;
use crate::source_witness_catalog::{self as catalog, SourceCatalogValidator};
use crate::{Error, Result};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, RelativePath, SourceRevision,
    canonical_raw_bytes_v1,
};
use tos_source_store::{CorpusCutReader, SourceMembershipV1};

/// The caller selects the revision/membership independently of the cut reader.
/// `stage_source_cut` is the caller's exact job label: it is not interpreted as
/// a revision digest or used as permission to read an unselected revision.
pub struct BibliographicSourceCut<'a> {
    pub cut: &'a CorpusCutReader,
    pub expected_revision: SourceRevision,
    pub expected_membership: SourceMembershipV1,
    pub stage_source_cut: &'a str,
    pub max_read_files: usize,
    pub max_read_bytes: usize,
}
/// One caller-owned monotone read budget shared by streamed record/history
/// resolution. Repeated physical reads remain charged; there is no whole-cut cache.
pub struct StreamedBibliographicReadLedger {
    max_files: u64,
    max_bytes: u64,
    files: u64,
    bytes: u64,
    poisoned: bool,
}
impl StreamedBibliographicReadLedger {
    pub fn new(max_files: u64, max_bytes: u64) -> Result<Self> {
        if max_files == 0 || max_files == u64::MAX || max_bytes == 0 || max_bytes == u64::MAX {
            return Err(Error::Budget("streamed bibliographic read limits"));
        }
        Ok(Self {
            max_files,
            max_bytes,
            files: 0,
            bytes: 0,
            poisoned: false,
        })
    }
    fn charge(&mut self, bytes: u64) -> Result<()> {
        self.charge_observed(1, bytes)
    }
    pub(crate) fn remaining(&self) -> Result<(u64, u64)> {
        if self.poisoned {
            return Err(Error::Invalid(
                "streamed bibliographic read ledger poisoned",
            ));
        }
        Ok((self.max_files - self.files, self.max_bytes - self.bytes))
    }
    pub(crate) fn charge_observed(&mut self, files: u64, bytes: u64) -> Result<()> {
        if self.poisoned {
            return Err(Error::Invalid(
                "streamed bibliographic read ledger poisoned",
            ));
        }
        self.poisoned = true;
        self.files = self
            .files
            .checked_add(files)
            .filter(|n| *n <= self.max_files)
            .ok_or(Error::Budget("streamed bibliographic read files"))?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.max_bytes)
            .ok_or(Error::Budget("streamed bibliographic read bytes"))?;
        self.poisoned = false;
        Ok(())
    }
    pub fn observed_reads(&self) -> (u64, u64) {
        (self.files, self.bytes)
    }
}
pub struct StreamedBibliographicSourceCut<'a> {
    pub cut: &'a tos_source_store::StreamedCorpusCutReaderV1,
    pub expected_revision: SourceRevision,
    pub expected_membership: SourceMembershipV1,
    pub stage_source_cut: &'a str,
    pub read_ledger: &'a std::cell::RefCell<StreamedBibliographicReadLedger>,
    pub max_workspace_bytes: usize,
}
// This erased view is private to the selected-source kernel. Its sole generic
// implementation retains the adapter-owned identity and typed stage receipt.
mod candidate_source_seal {
    pub trait Sealed {}
}
pub(crate) trait ErasedCandidateSource: candidate_source_seal::Sealed {
    fn source(&self) -> &dyn tos_validation::record_biblio_cut::SourceCutInput;
    fn coverage(&self) -> &tos_validation::record_biblio_cut::SourceCutInputCoverage;
    fn verify(&self, stage: &KnowledgeStage<'_>, l: BibliographicLimits) -> Result<()>;
    fn charge(&self, bytes: u64) -> Result<()>;
    fn workspace(&self) -> usize;
    fn deadline(&self, limits: BibliographicLimits) -> std::time::Instant;
    fn cancelled(&self) -> &std::sync::atomic::AtomicBool;
    fn check(&self, l: BibliographicLimits) -> Result<()> {
        if std::time::Instant::now() >= self.deadline(l) {
            return Err(Error::Budget("candidate bibliographic original deadline"));
        }
        if self.cancelled().load(std::sync::atomic::Ordering::Relaxed) {
            return Err(Error::Invalid(
                "candidate bibliographic original cancellation",
            ));
        }
        Ok(())
    }
}
pub(crate) struct CandidateBibliographicSource<'a, 'b, I: Eq + Copy + 'static> {
    pub(crate) input: &'a dyn tos_validation::record_biblio_cut::SourceCutInputWithIdentity<I>,
    pub(crate) binding: &'a crate::knowledge_stage::CandidateValidationBinding<I>,
    pub(crate) read_ledger: &'a std::cell::RefCell<StreamedBibliographicReadLedger>,
    pub(crate) max_workspace_bytes: usize,
    pub(crate) validator: &'a SourceCatalogValidator<'b>,
    pub(crate) original_deadline: std::time::Instant,
}
impl<I: Eq + Copy + 'static> candidate_source_seal::Sealed
    for CandidateBibliographicSource<'_, '_, I>
{
}
impl<I: Eq + Copy + 'static> ErasedCandidateSource for CandidateBibliographicSource<'_, '_, I> {
    fn source(&self) -> &dyn tos_validation::record_biblio_cut::SourceCutInput {
        self.input.source_input()
    }
    fn coverage(&self) -> &tos_validation::record_biblio_cut::SourceCutInputCoverage {
        self.binding.coverage()
    }
    fn verify(&self, stage: &KnowledgeStage<'_>, l: BibliographicLimits) -> Result<()> {
        let mut held = l;
        held.deadline = self.deadline(l);
        check(self.validator, held)?;
        self.validator
            .verify_candidate_schema_binding(self.input.input_identity())?;
        let selected = &stage.candidate_receipt::<I>()?.binding;
        if self.max_workspace_bytes == 0
            || self.input.input_identity() != self.binding.input_identity()
            || selected.input_identity() != self.binding.input_identity()
            || selected.coverage() != self.coverage()
        {
            return Err(Error::Invalid(
                "bibliographic actual candidate input/receipt binding",
            ));
        }
        self.source()
            .verify_current_fence(self.coverage(), self.deadline(l), self.cancelled())
            .map_err(|error| crate::source_bibliographic_source::candidate_input_refusal(error))?;
        self.validator
            .verify_candidate_schema_binding(self.input.input_identity())?;
        check(self.validator, held)
    }
    fn charge(&self, bytes: u64) -> Result<()> {
        self.read_ledger
            .try_borrow_mut()
            .map_err(|_| Error::Invalid("candidate bibliographic read ledger in use"))?
            .charge(bytes)
    }
    fn workspace(&self) -> usize {
        self.max_workspace_bytes
    }
    fn deadline(&self, l: BibliographicLimits) -> std::time::Instant {
        self.original_deadline.min(l.deadline)
    }
    fn cancelled(&self) -> &std::sync::atomic::AtomicBool {
        self.validator.cancelled
    }
}
#[derive(Clone, Copy)]
pub(crate) enum SelectedBibliographicSourceCut<'a, 'b> {
    Resident(&'a BibliographicSourceCut<'b>),
    Streamed(&'a StreamedBibliographicSourceCut<'b>),
    Candidate(&'a dyn ErasedCandidateSource),
}
impl<'a, 'b> SelectedBibliographicSourceCut<'a, 'b> {
    fn verify_selected(self, stage: &KnowledgeStage<'_>, l: BibliographicLimits) -> Result<()> {
        match self {
            Self::Resident(i) => {
                if i.cut.current().revision() != i.expected_revision
                    || stage.input_source_cut()? != i.stage_source_cut
                {
                    return Err(Error::Invalid(
                        "bibliographic independently selected source cut",
                    ));
                }
            }
            Self::Streamed(i) => {
                if i.cut.current_revision() != i.expected_revision
                    || stage.input_source_cut()? != i.stage_source_cut
                {
                    return Err(Error::Invalid(
                        "bibliographic independently selected source cut",
                    ));
                }
            }
            Self::Candidate(i) => i.verify(stage, l)?,
        }
        Ok(())
    }
    pub(crate) fn membership(self) -> SourceMembershipV1 {
        match self {
            Self::Resident(i) => i.expected_membership,
            Self::Streamed(i) => i.expected_membership,
            Self::Candidate(i) => i.coverage().membership(),
        }
    }
    fn binding(self) -> Value {
        match self {
            Self::Resident(i) => {
                json!({"source_revision":i.expected_revision.0.to_hex(), "membership_count":i.expected_membership.count, "membership_sha256":i.expected_membership.digest.to_hex(), "stage_source_cut":i.stage_source_cut})
            }
            Self::Streamed(i) => {
                json!({"source_revision":i.expected_revision.0.to_hex(), "membership_count":i.expected_membership.count, "membership_sha256":i.expected_membership.digest.to_hex(), "stage_source_cut":i.stage_source_cut})
            }
            Self::Candidate(i) => {
                json!({"input_kind":"candidate", "membership_count":i.coverage().membership().count, "membership_sha256":i.coverage().membership().digest.to_hex(), "source_bytes_read":i.coverage().source_bytes_read()})
            }
        }
    }
    fn member_facts(
        self,
        path: &RelativePath,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<Option<(u64, Digest256)>> {
        match self {
            Self::Resident(i) => Ok(i
                .cut
                .current()
                .member(path)
                .map(|m| (m.size_bytes, m.sha256))),
            Self::Streamed(i) => i
                .cut
                .member(i.expected_revision, path)
                .map(|m| m.map(|m| (m.size_bytes, m.sha256)))
                .map_err(|e| Error::Source(e.to_string())),
            Self::Candidate(_) => {
                if !self.present(path, validator, l)? {
                    return Ok(None);
                }
                let raw = self.read(path, validator, l)?;
                Ok(Some((raw.len() as u64, Digest256::of_bytes(&raw))))
            }
        }
    }
    fn present(
        self,
        path: &RelativePath,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<bool> {
        match self {
            Self::Resident(i) => Ok(i.cut.current().member(path).is_some()),
            Self::Streamed(i) => i
                .cut
                .member(i.expected_revision, path)
                .map(|m| m.is_some())
                .map_err(|e| Error::Source(e.to_string())),
            Self::Candidate(i) => i
                .source()
                .path_presence(path.as_str(), i.deadline(l), i.cancelled())
                .map(|v| v == Some(tos_source_store::SourcePresenceV1::File))
                .map_err(|error| {
                    crate::source_bibliographic_source::candidate_input_refusal(error)
                }),
        }
    }
    pub(crate) fn visit_members(
        self,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
        mut visit: impl FnMut(&str, u64, Digest256) -> Result<()>,
    ) -> Result<()> {
        match self {
            Self::Resident(i) => {
                for m in i.cut.current().members() {
                    visit(m.path.as_str(), m.size_bytes, m.sha256)?;
                }
            }
            Self::Streamed(i) => {
                let mut after = None;
                loop {
                    check(validator, l)?;
                    let Some(m) = i
                        .cut
                        .member_after(i.expected_revision, after.as_ref())
                        .map_err(|e| Error::Source(e.to_string()))?
                    else {
                        break;
                    };
                    check(validator, l)?;
                    visit(m.path.as_str(), m.size_bytes, m.sha256)?;
                    after = Some(m.path);
                }
            }
            Self::Candidate(i) => {
                let mut previous = None::<String>;
                let (mut observed_count, mut observed_bytes) = (0u64, 0u64);
                let coverage = i
                    .source()
                    .for_each_current_member(i.deadline(l), i.cancelled(), &mut |meta, raw| {
                        i.charge(raw.len() as u64)
                            .and_then(|()| {
                                i.check(l)?;
                                check(validator, l)?;
                                path(meta.path)?;
                                if previous.as_deref().is_some_and(|p| p >= meta.path) {
                                    return Err(Error::Invalid(
                                        "candidate bibliographic complete membership order",
                                    ));
                                }
                                previous = Some(meta.path.to_owned());
                                observed_count = observed_count.checked_add(1).ok_or(
                                    Error::Budget("candidate bibliographic full EOF member count"),
                                )?;
                                observed_bytes =
                                    observed_bytes.checked_add(raw.len() as u64).ok_or(
                                        Error::Budget("candidate bibliographic full EOF bytes"),
                                    )?;
                                if raw.len() > l.catalog.max_file_bytes {
                                    return Err(Error::Budget(
                                        "candidate bibliographic current member bytes",
                                    ));
                                }
                                if raw.len() as u64 != meta.size_bytes {
                                    return Err(Error::Invalid(
                                        "candidate bibliographic metadata/raw size",
                                    ));
                                }
                                visit(meta.path, meta.size_bytes, Digest256::of_bytes(raw))?;
                                i.check(l)
                            })
                            .map_err(|e| {
                                tos_validation::item_rules::ItemRefusal::Source(e.to_string())
                            })
                    })
                    .map_err(|error| {
                        crate::source_bibliographic_source::candidate_input_refusal(error)
                    })?;
                if &coverage != i.coverage()
                    || observed_count != coverage.member_count()
                    || observed_count != coverage.membership().count
                    || observed_bytes != coverage.source_bytes_read()
                {
                    return Err(Error::Invalid(
                        "candidate bibliographic complete EOF coverage",
                    ));
                }
                i.source()
                    .verify_current_fence(&coverage, i.deadline(l), i.cancelled())
                    .map_err(|error| {
                        crate::source_bibliographic_source::candidate_input_refusal(error)
                    })?;
            }
        }
        Ok(())
    }
    fn visit_prefix(
        self,
        base: &str,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
        mut visit: impl FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        let prefix = format!("{base}/");
        match self {
            Self::Resident(_) => self.visit_members(validator, l, |reference, _, _| {
                if let Some(name) = reference.strip_prefix(&prefix) {
                    visit(name)?;
                }
                Ok(())
            }),
            Self::Streamed(i) => {
                let mut after = Some(path(base)?);
                loop {
                    check(validator, l)?;
                    let Some(member) = i
                        .cut
                        .member_after(i.expected_revision, after.as_ref())
                        .map_err(|e| Error::Source(e.to_string()))?
                    else {
                        break;
                    };
                    check(validator, l)?;
                    let reference = member.path.as_str();
                    if reference < prefix.as_str() {
                        after = Some(member.path);
                        continue;
                    }
                    let Some(name) = reference.strip_prefix(&prefix) else {
                        break;
                    };
                    visit(name)?;
                    after = Some(member.path);
                }
                Ok(())
            }
            Self::Candidate(i) => i
                .source()
                .for_each_current_member_meta(i.deadline(l), i.cancelled(), &mut |meta| {
                    check(validator, l)
                        .and_then(|()| {
                            i.check(l)?;
                            path(meta.path)?;
                            if let Some(name) = meta.path.strip_prefix(&prefix) {
                                visit(name)?;
                            }
                            Ok(())
                        })
                        .map_err(|e| tos_validation::item_rules::ItemRefusal::Source(e.to_string()))
                })
                .map_err(|error| {
                    crate::source_bibliographic_source::candidate_input_refusal(error)
                }),
        }
    }
    pub(crate) fn read(
        self,
        path: &RelativePath,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<Vec<u8>> {
        match self {
            Self::Resident(i) => i
                .cut
                .read_member(
                    i.expected_revision,
                    path,
                    l.catalog.max_file_bytes as u64,
                    l.deadline,
                    validator.cancelled,
                )
                .map(|m| m.raw)
                .map_err(|_| Error::Invalid("bibliographic exact current source member read")),
            Self::Streamed(i) => {
                check(validator, l)?;
                let m = i
                    .cut
                    .member(i.expected_revision, path)
                    .map_err(|e| Error::Source(e.to_string()))?
                    .ok_or(Error::Invalid("streamed bibliographic member absent"))?;
                usize::try_from(m.size_bytes)
                    .ok()
                    .and_then(|n| n.max(8).checked_mul(4))
                    .and_then(|n| n.checked_add(128 * 1024))
                    .filter(|n| *n <= i.max_workspace_bytes)
                    .ok_or(Error::Budget("streamed bibliographic raw workspace"))?;
                i.read_ledger
                    .try_borrow_mut()
                    .map_err(|_| Error::Invalid("streamed bibliographic read ledger in use"))?
                    .charge(m.size_bytes)?;
                let raw = i
                    .cut
                    .read_member(
                        i.expected_revision,
                        path,
                        l.catalog.max_file_bytes as u64,
                        l.deadline,
                        validator.cancelled,
                    )
                    .map_err(|e| Error::Source(e.to_string()))?
                    .raw;
                if raw.len() as u64 != m.size_bytes || Digest256::of_bytes(&raw) != m.sha256 {
                    return Err(Error::Invalid("streamed bibliographic raw binding"));
                }
                check(validator, l)?;
                Ok(raw)
            }
            Self::Candidate(i) => {
                let mut result = None;
                i.source()
                    .with_current_member(
                        path.as_str(),
                        l.catalog.max_file_bytes,
                        i.deadline(l),
                        i.cancelled(),
                        &mut |meta, raw| {
                            check(validator, l)
                                .and_then(|()| {
                                    i.check(l)?;
                                    i.charge(raw.len() as u64)?;
                                    if raw.len() > l.catalog.max_file_bytes {
                                        return Err(Error::Budget(
                                            "candidate bibliographic selected member bytes",
                                        ));
                                    }
                                    if meta.path != path.as_str()
                                        || raw.len() as u64 != meta.size_bytes
                                        || result.is_some()
                                    {
                                        return Err(Error::Invalid(
                                            "candidate bibliographic selected raw member",
                                        ));
                                    }
                                    raw.len()
                                        .max(8)
                                        .checked_mul(4)
                                        .and_then(|n| n.checked_add(128 * 1024))
                                        .filter(|n| *n <= i.workspace())
                                        .ok_or(Error::Budget(
                                            "candidate bibliographic raw workspace",
                                        ))?;
                                    result = Some(raw.to_vec());
                                    Ok(())
                                })
                                .map_err(|e| {
                                    tos_validation::item_rules::ItemRefusal::Source(e.to_string())
                                })
                        },
                    )
                    .map_err(|error| {
                        crate::source_bibliographic_source::candidate_input_refusal(error)
                    })?;
                result.ok_or(Error::Invalid(
                    "candidate bibliographic raw callback absent",
                ))
            }
        }
    }
}
pub(crate) struct Versions<'a, 'b> {
    input: SelectedBibliographicSourceCut<'a, 'b>,
    files: BTreeMap<String, Vec<u8>>,
    bytes: usize,
    catalog_root: String,
    // One exact schema closure for this sealed catalog operation. Source reads
    // and version checks remain per record; the immutable schemas are shared.
    contracts: std::rc::Rc<catalog::Contracts>,
    legacy_catalog_locations: BTreeMap<(String, Option<String>), LegacyCatalogLocations>,
    legacy_catalog_location_bytes: u64,
    catalog_epoch: Option<(
        crate::VersionsCatalogEpochV2,
        tos_segment_store::AuthenticatedTreeLimitsV1,
    )>,
}

/// Shared exact archive verifier used by both full source cuts and event-local
/// selected file sets. Only the read and schema-execution adapters differ.
fn validate_archived_metadata_record<R, C>(
    reference: &str,
    receipt: &Value,
    route: &RecordRoute,
    schema_version: &Value,
    l: BibliographicLimits,
    mut read: R,
    mut check_schema: C,
) -> Result<(Value, Value)>
where
    R: FnMut(&str) -> Result<Vec<u8>>,
    C: FnMut(&str, &str, &[u8]) -> Result<()>,
{
    let previous = &receipt["previous_source"];
    exact_ref(previous, false)?;
    let revision = text(receipt, "previous_revision")?;
    let revision_sha = hash(revision)?;
    let archive = format!(
        "ToS/source-witnesses/.record-revisions/{}-{}",
        Digest256::of_bytes(text(previous, "id")?.as_bytes()).to_hex(),
        revision_sha
    );
    if receipt["archive_path"] != archive {
        return Err(Error::Invalid(
            "bibliographic metadata archive source-derived locator",
        ));
    }
    let manifest_ref = format!("{archive}/manifest.json");
    let manifest_raw = read(&manifest_ref)?;
    let manifest = owned(&manifest_raw, l.catalog.max_row_bytes)?;
    let selected = manifest["schema_version"] == "tos_source_package_archive_v2";
    let mut keys = vec![
        "schema_version",
        "source_path",
        "source",
        "revision",
        "files",
    ];
    if selected {
        keys.push("publication_protocol");
    }
    exact_keys(&manifest, &keys)?;
    if !matches!(
        manifest["schema_version"].as_str(),
        Some("tos_source_package_archive_v1" | "tos_source_package_archive_v2")
    ) || selected && manifest["publication_protocol"] != "tos_selected_source_metadata_v1"
        || manifest["source_path"] != reference
        || manifest["source"] != *previous
        || manifest["revision"] != revision
    {
        return Err(Error::Invalid(
            "bibliographic metadata archive exact manifest binding",
        ));
    }
    let bindings = manifest["files"]
        .as_object()
        .ok_or(Error::Invalid("bibliographic metadata archive file map"))?;
    if bindings.is_empty() || bindings.len() > 64 {
        return Err(Error::Budget("bibliographic metadata archive member count"));
    }
    let basename = reference.rsplit('/').next().unwrap();
    let forms_name = format!("{}.human-forms.json", basename.trim_end_matches(".json"));
    let selected_names = [
        basename,
        forms_name.as_str(),
        "source-revision-history.json",
    ];
    let mut package = serde_json::Map::new();
    let mut declared_bytes = 0u64;
    for (name, binding) in bindings {
        exact_keys(binding, &["blob", "sha256", "bytes"])?;
        if name.is_empty()
            || name.contains(['/', '\\', '\0'])
            || matches!(name.as_str(), "." | "..")
            || selected && !selected_names.contains(&name.as_str())
        {
            return Err(Error::Invalid(
                "bibliographic metadata archive member locator/scope",
            ));
        }
        let sha = hash(text(binding, "sha256")?)?;
        if binding["blob"] != format!("{sha}.blob") {
            return Err(Error::Invalid(
                "bibliographic metadata archive blob locator",
            ));
        }
        let bytes = binding["bytes"]
            .as_u64()
            .filter(|n| *n <= 2 * 1024 * 1024)
            .ok_or(Error::Budget(
                "bibliographic metadata archive declared member bytes",
            ))?;
        declared_bytes = declared_bytes
            .checked_add(bytes)
            .filter(|n| *n <= 8 * 1024 * 1024)
            .ok_or(Error::Budget(
                "bibliographic metadata archive declared package bytes",
            ))?;
        package.insert(
            name.clone(),
            json!({"sha256":binding["sha256"],"bytes":binding["bytes"]}),
        );
    }
    if digest(&Value::Object(package), l.max_claim_cohort_bytes)? != revision_sha {
        return Err(Error::Invalid(
            "bibliographic metadata archive package revision",
        ));
    }
    let binding = bindings.get(basename).ok_or(Error::Invalid(
        "bibliographic archived Collection record absent",
    ))?;
    let blob_ref = format!("{archive}/{}", text(binding, "blob")?);
    let raw = read(&blob_ref)?;
    if raw.len() as u64 != binding["bytes"].as_u64().unwrap_or(u64::MAX)
        || format!("sha256:{}", Digest256::of_bytes(&raw).to_hex()) != text(binding, "sha256")?
    {
        return Err(Error::Invalid(
            "bibliographic archived Collection record raw binding",
        ));
    }
    let record = owned(&raw, l.catalog.max_row_bytes)?;
    check_schema(&blob_ref, &route.schema, &raw)?;
    route.validate_record(&record, previous, schema_version)?;
    if metadata_ref(&record, route, l)? != *previous {
        return Err(Error::Invalid(
            "bibliographic retained Collection exact record/profile",
        ));
    }
    Ok((
        record,
        json!({"source_ref":reference,"record_bytes":raw.len(),"record_sha256":binding["sha256"],
        "archive_blob_ref":blob_ref,"package_revision":revision,"archive_manifest_ref":manifest_ref,
        "archive_manifest_sha256":format!("sha256:{}",Digest256::of_bytes(&manifest_raw).to_hex())}),
    ))
}

fn path(reference: &str) -> Result<RelativePath> {
    RelativePath::parse(reference).map_err(|_| Error::Invalid("bibliographic cut source path"))
}
impl<'a, 'b> Versions<'a, 'b> {
    pub(crate) fn new<B: catalog::CatalogInputBinding>(
        input: &'a BibliographicSourceCut<'b>,
        stage: &mut KnowledgeStage<'_>,
        validator: &SourceCatalogValidator<'_>,
        receipt: &catalog::SourceCatalogReceipt<B>,
        l: BibliographicLimits,
    ) -> Result<Self> {
        Self::new_selected(
            SelectedBibliographicSourceCut::Resident(input),
            stage,
            validator,
            receipt,
            l,
        )
    }
    pub(crate) fn new_streamed<B: catalog::CatalogInputBinding>(
        input: &'a StreamedBibliographicSourceCut<'b>,
        stage: &mut KnowledgeStage<'_>,
        validator: &SourceCatalogValidator<'_>,
        receipt: &catalog::SourceCatalogReceipt<B>,
        l: BibliographicLimits,
    ) -> Result<Self> {
        Self::new_selected(
            SelectedBibliographicSourceCut::Streamed(input),
            stage,
            validator,
            receipt,
            l,
        )
    }
    pub(crate) fn new_selected<B: catalog::CatalogInputBinding>(
        input: SelectedBibliographicSourceCut<'a, 'b>,
        stage: &mut KnowledgeStage<'_>,
        validator: &SourceCatalogValidator<'_>,
        receipt: &catalog::SourceCatalogReceipt<B>,
        l: BibliographicLimits,
    ) -> Result<Self> {
        catalog::verify_catalog(stage, receipt, l.catalog)?;
        input.verify_selected(stage, l)?;
        match input {
            SelectedBibliographicSourceCut::Resident(i) => {
                if i.max_read_files == 0
                    || i.max_read_files > 4096
                    || i.max_read_bytes == 0
                    || i.max_read_bytes > 64 * 1024 * 1024
                {
                    return Err(Error::Invalid(
                        "bibliographic independently selected source cut",
                    ));
                }
                if i.cut
                    .stream(i.expected_revision)
                    .map_err(|_| Error::Invalid("bibliographic current source membership"))?
                    .expectation()
                    != i.expected_membership
                {
                    return Err(Error::Invalid(
                        "bibliographic independent membership expectation",
                    ));
                }
            }
            SelectedBibliographicSourceCut::Streamed(i) => {
                if i.max_workspace_bytes == 0
                    || i.cut
                        .revision(i.expected_revision)
                        .map_err(|e| Error::Source(e.to_string()))?
                        .is_none_or(|r| r.membership != i.expected_membership)
                {
                    return Err(Error::Invalid(
                        "streamed bibliographic independently selected cut",
                    ));
                }
            }
            SelectedBibliographicSourceCut::Candidate(_) => {}
        }
        // Metadata is authenticated by SourceRevision. Complete traversal to
        // EOF does not read original payloads; those are outside this carrier.
        let mut membership = Digest256Hasher::new();
        membership.update(b"tos-val-full-membership-v1\0");
        let mut count = 0u64;
        input.visit_members(validator, l, |reference, size, sha| {
            check(validator, l)?;
            membership.update(&(reference.len() as u64).to_be_bytes());
            membership.update(reference.as_bytes());
            membership.update(&size.to_be_bytes());
            membership.update(sha.as_bytes());
            count = count
                .checked_add(1)
                .filter(|n| *n <= input.membership().count)
                .ok_or(Error::Budget("bibliographic cut membership"))?;
            Ok(())
        })?;
        if (SourceMembershipV1 {
            count,
            digest: membership.finalize(),
        }) != input.membership()
        {
            return Err(Error::Invalid(
                "bibliographic complete metadata membership root",
            ));
        }
        let mut result = Self {
            input,
            files: BTreeMap::new(),
            bytes: 0,
            catalog_root: receipt.row_root_sha256.clone(),
            contracts: std::rc::Rc::new(catalog::prepare_catalog_contracts(
                stage, validator, l.catalog,
            )?),
            legacy_catalog_locations: BTreeMap::new(),
            legacy_catalog_location_bytes: 0,
            catalog_epoch: None,
        };
        // Bind every declared raw input, including selected schemas and native
        // packets, to a member of this exact current source revision.
        for collection in [
            catalog::SOURCE_FILES,
            catalog::CONTRACT_FILES,
            catalog::NATIVE_IDENTITIES,
            catalog::NATIVE_TEXT,
            catalog::BIBLIOGRAPHIC_FILES,
        ] {
            if !stage
                .input_collections()
                .iter()
                .any(|c| c.source_graph == catalog::CATALOG_SOURCE && c.collection == collection)
            {
                continue;
            }
            let mut after = None;
            loop {
                let page =
                    stage.scan_input(catalog::CATALOG_SOURCE, collection, after.as_deref(), 1)?;
                for row in page.rows {
                    check(validator, l)?;
                    let (size_bytes, sha256) = input
                        .member_facts(&path(&row.id)?, validator, l)?
                        .ok_or(Error::Invalid(
                        "bibliographic staged source outside selected current cut",
                    ))?;
                    check(validator, l)?;
                    if size_bytes != row.payload.len() as u64
                        || sha256 != Digest256::of_bytes(&row.payload)
                    {
                        return Err(Error::Invalid(
                            "bibliographic staged source/cut bytes differ",
                        ));
                    }
                }
                after = page.next_id;
                if after.is_none() {
                    break;
                }
            }
        }
        // The read cache starts empty: binding staged bytes reserves no second
        // copy and does not claim the selected source is publicly admissible.
        result.bytes = 0;
        Ok(result)
    }
    pub(crate) fn contracts(&self) -> std::rc::Rc<catalog::Contracts> {
        self.contracts.clone()
    }
    pub(crate) fn binding(&self) -> Value {
        self.input.binding()
    }
    fn optional(
        &mut self,
        reference: &str,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<Option<Vec<u8>>> {
        check(validator, l)?;
        let parsed = path(reference)?;
        if !self.input.present(&parsed, validator, l)? {
            return Ok(None);
        }
        if let SelectedBibliographicSourceCut::Resident(input) = self.input {
            if let Some(raw) = self.files.get(reference) {
                return Ok(Some(raw.clone()));
            }
            if self.files.len() >= input.max_read_files {
                return Err(Error::Budget("bibliographic version source file count"));
            }
        }
        let raw = self.input.read(&parsed, validator, l)?;
        if let SelectedBibliographicSourceCut::Resident(input) = self.input {
            self.bytes = self
                .bytes
                .checked_add(raw.len())
                .filter(|n| *n <= input.max_read_bytes)
                .ok_or(Error::Budget(
                    "bibliographic version aggregate source bytes",
                ))?;
            self.files.insert(reference.into(), raw.clone());
        }
        Ok(Some(raw))
    }

    pub(crate) fn required(
        &mut self,
        reference: &str,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<Vec<u8>> {
        self.optional(reference, validator, l)?
            .ok_or(Error::Invalid("bibliographic exact version source missing"))
    }
    pub(crate) fn ground(
        &mut self,
        stage: &mut KnowledgeStage<'_>,
        claim: &Value,
        validator: &SourceCatalogValidator<'_>,
        entities: &Value,
        forms: &mut dyn BibliographicForms,
        l: BibliographicLimits,
    ) -> Result<Value> {
        let value = &claim["object"];
        let collection =
            self.resolve_record(stage, &value["collection_version"], validator, entities, l)?;
        if collection.record["record_type"] != "collection"
            || collection.record["record_id"] != claim["subject_ref"]
        {
            return Err(Error::Invalid(
                "bibliographic exact Collection basis identity/type",
            ));
        }
        let declared = array(&collection.record, "membership_claim_refs")?;
        let expected = array(value, "members")?;
        let mut members = BTreeSet::new();
        let mut bound = Vec::new();
        let mut inputs = BTreeMap::new();
        bind_files(&collection.provenance, &mut inputs)?;
        for reference in array(value, "membership_versions")? {
            let resolved = self.resolve_claim(stage, reference, validator, forms, l)?;
            let record = &resolved.record;
            let legacy = record["schema_version"] == "tos_claim_packet_v1"
                && record["claim_type"] == "bibliographic";
            let native = record["schema_version"] == "tos_source_relation_claim_v1"
                && record["claim_type"] == "relation";
            let member = record["object"]
                .as_str()
                .ok_or(Error::Invalid("bibliographic exact membership Work"))?;
            let polarity = record
                .get("polarity")
                .and_then(Value::as_str)
                .or(if legacy { Some("positive") } else { None });
            if !(legacy || native)
                || !matches!(
                    record["assertion_layer"].as_str(),
                    Some("bibliographic_assertion" | "scholarly_report")
                )
                || !declared.contains(&reference["id"])
                || record["subject_ref"] != claim["subject_ref"]
                || record["predicate"] != "contains_work"
                || polarity != Some("positive")
                || !expected.contains(&record["object"])
                || !members.insert(member.to_owned())
            {
                return Err(Error::Invalid(
                    "bibliographic distinct positive exact Collection membership",
                ));
            }
            bind_files(&resolved.provenance, &mut inputs)?;
            bound.push(json!({"ref":reference,"provenance":resolved.provenance,"version_status":resolved.version_status}));
        }
        let expected = expected
            .iter()
            .map(|v| {
                v.as_str()
                    .ok_or(Error::Invalid("bibliographic Collection member ref"))
            })
            .collect::<Result<BTreeSet<_>>>()?;
        if members.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected {
            return Err(Error::Invalid("bibliographic exact membership set closure"));
        }
        check(validator, l)?;
        let basis = json!({"collection":{"ref":value["collection_version"],
            "provenance":collection.provenance,"version_status":collection.version_status},
            "memberships":bound,"input_digests":inputs,
            "establishes_membership":false,"grants_admission":false});
        encode(&basis, l.max_claim_cohort_bytes)?;
        Ok(basis)
    }
    /// Explicit V2 reader; the selected epoch commitment is independent of the
    /// record request. V1 constructor/output remain unchanged.
    pub(crate) fn new_with_catalog_epoch(
        input: &'a BibliographicSourceCut<'b>,
        stage: &mut KnowledgeStage<'_>,
        validator: &SourceCatalogValidator<'_>,
        receipt: &catalog::SourceCatalogReceipt,
        l: BibliographicLimits,
        epoch: &crate::VersionsCatalogEpochV2,
        expected_epoch: Digest256,
        limits: tos_segment_store::AuthenticatedTreeLimitsV1,
    ) -> Result<Self> {
        if epoch.commitment() != expected_epoch {
            return Err(Error::Invalid(
                "Versions independently selected epoch differs",
            ));
        }
        epoch.require_catalog(receipt)?;
        let mut versions = Self::new(input, stage, validator, receipt, l)?;
        versions.catalog_epoch = Some((epoch.clone(), limits));
        Ok(versions)
    }

    fn catalog_location(
        &mut self,
        stage: &mut KnowledgeStage<'_>,
        category: &str,
        kind: Option<&str>,
        id: &str,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<CatalogLocation> {
        if let Some((epoch, limits)) = &self.catalog_epoch {
            let row = catalog::catalog_row(stage, category, id, l.catalog)?
                .ok_or(Error::Invalid("Versions selected catalog row absent"))?;
            if kind.is_some_and(|kind| row["entry"]["record_type"] != kind) {
                return Err(Error::Invalid("Versions catalog record family differs"));
            }
            return Ok(CatalogLocation::Epoch(epoch.provenance(
                category,
                id,
                &row["entry"],
                *limits,
                l.deadline,
                validator.cancelled,
            )?));
        }
        check(validator, l)?;
        let key = (category.to_owned(), kind.map(str::to_owned));
        if !self.legacy_catalog_locations.contains_key(&key) {
            // Legacy source-return packets retain their exact JSONL digest and
            // family-relative line. Compute each immutable catalog once, rather
            // than reserializing its whole population for every record version.
            // This is bounded operation state under the selected projection's
            // byte envelope, independent of source-read accounting.
            let key_bytes = (key.0.capacity() + key.1.as_ref().map_or(0, String::capacity)) as u64;
            let remaining = l
                .max_output_bytes
                .checked_sub(self.legacy_catalog_location_bytes)
                .and_then(|bytes| bytes.checked_sub(key_bytes))
                .ok_or(Error::Budget("bibliographic catalog location state"))?;
            let mut after = None;
            let mut index = legacy_catalog_locations(
                || {
                    let Some(key) = catalog::catalog_next(stage, category, after.as_deref())?
                    else {
                        return Ok(None);
                    };
                    let row = catalog::catalog_row(stage, category, &key, l.catalog)?
                        .ok_or(Error::Invalid("bibliographic version catalog disappeared"))?;
                    after = Some(key.clone());
                    Ok(Some((key, row)))
                },
                kind,
                remaining,
                l,
            )?;
            index.retained_bytes += key_bytes;
            self.legacy_catalog_location_bytes += index.retained_bytes;
            self.legacy_catalog_locations.insert(key.clone(), index);
        }
        self.legacy_catalog_locations[&key].locate(id)
    }

    pub(crate) fn verify_catalog_binding<B: catalog::CatalogInputBinding>(
        &self,
        stage: &mut KnowledgeStage<'_>,
        receipt: &catalog::SourceCatalogReceipt<B>,
        l: BibliographicLimits,
    ) -> Result<()> {
        if self.catalog_root != receipt.row_root_sha256 {
            return Err(Error::Invalid(
                "navigation version resolver catalog/cut binding",
            ));
        }
        self.input.verify_selected(stage, l)?;
        catalog::verify_catalog(stage, receipt, l.catalog)
    }
    /// Checked family and locator dispatch, matching MetadataVersionReader.supports.
    pub(crate) fn supports_record(
        &self,
        stage: &mut KnowledgeStage<'_>,
        id: &str,
        entities: &Value,
        l: BibliographicLimits,
    ) -> Result<bool> {
        verify_entities(stage, entities, l)?;
        let row = catalog::catalog_row(stage, "records", id, l.catalog)?
            .ok_or(Error::Invalid("metadata supports catalog identity absent"))?;
        let entry = &row["entry"];
        let kind = text(entry, "record_type")?;
        let reference = text(entry, "source_record_ref")?;
        let native_composite =
            kind == "composite" && reference.ends_with("/composite-witness.json");
        let native = kind == "artifact"
            || kind == "link"
            || native_composite
            || [
                "agent",
                "place",
                "organization",
                "work",
                "expression",
                "edition",
                "collection",
                "item",
            ]
            .contains(&kind);
        if native {
            let mappings = array(entities, "types")?
                .iter()
                .filter(|e| {
                    e.get("source_mappings")
                        .and_then(Value::as_array)
                        .is_some_and(|ms| {
                            ms.iter().any(|m| {
                                m["source_graph"] == "source-navigation"
                                    && m["source_kind_id"] == kind
                            })
                        })
                })
                .count();
            if mappings != 1 {
                return Err(Error::Invalid(
                    "metadata supports native owner registry mapping",
                ));
            }
        }
        let profiles = array(entities, "types")?
            .iter()
            .filter(|e| e["source_record_profile"]["record_type"] == kind)
            .collect::<Vec<_>>();
        if !native && profiles.is_empty() {
            return Ok(false);
        }
        if profiles.len() > 1 {
            return Err(Error::Invalid("metadata supports duplicate owner profile"));
        }
        let basename = if kind == "artifact" {
            "artifact-witness.json".into()
        } else if native_composite {
            "composite-witness.json".into()
        } else if native {
            format!("{kind}.json")
        } else {
            text(&profiles[0]["source_record_profile"], "source_basename")?.to_owned()
        };
        let parts = reference.split('/').collect::<Vec<_>>();
        Ok(parts.len() >= 5
            && !reference.contains(['\\', '\0'])
            && parts[..2] == ["ToS", "source-witnesses"]
            && parts.last() == Some(&basename.as_str())
            && !parts.iter().any(|p| {
                p.is_empty()
                    || p.starts_with('.')
                    || [
                        "catalog",
                        "payload",
                        "private",
                        "local-content",
                        "owner-local",
                    ]
                    .contains(p)
            }))
    }
    /// Exact current catalog/source equality without asserting metadata history.
    pub(crate) fn current_record(
        &mut self,
        stage: &mut KnowledgeStage<'_>,
        id: &str,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<Value> {
        let row = catalog::catalog_row(stage, "records", id, l.catalog)?
            .ok_or(Error::Invalid("navigation current metadata not catalogued"))?;
        let raw = self.required(text(&row["entry"], "source_record_ref")?, validator, l)?;
        if Digest256::of_bytes(&raw).to_hex() != text(&row["source"], "raw_sha256")?
            || raw.len() as u64 != row["source"]["raw_bytes"].as_u64().unwrap_or(u64::MAX)
        {
            return Err(Error::Invalid(
                "navigation current metadata exact source bytes",
            ));
        }
        let record = owned(&raw, l.catalog.max_row_bytes)?;
        if digest(&record, l.catalog.max_row_bytes)? != text(&row["entry"], "record_sha256")? {
            return Err(Error::Invalid(
                "navigation current metadata canonical source binding",
            ));
        }
        Ok(record)
    }
    /// Ordered exact retained metadata references. A successful result has already
    /// checked the entire selected record chain; it does not assess current use.
    pub(crate) fn exact_record_refs(
        &mut self,
        stage: &mut KnowledgeStage<'_>,
        id: &str,
        validator: &SourceCatalogValidator<'_>,
        entities: &Value,
        l: BibliographicLimits,
    ) -> Result<Version> {
        let row = catalog::catalog_row(stage, "records", id, l.catalog)?
            .ok_or(Error::Invalid("metadata version identity not catalogued"))?;
        self.resolve_record(stage, &row["source"]["record_ref"], validator, entities, l)
    }
    pub(crate) fn resolve_record(
        &mut self,
        stage: &mut KnowledgeStage<'_>,
        exact: &Value,
        validator: &SourceCatalogValidator<'_>,
        entities: &Value,
        l: BibliographicLimits,
    ) -> Result<Version> {
        verify_entities(stage, entities, l)?;
        exact_ref(exact, false)?;
        let id = text(exact, "id")?;
        let row = catalog::catalog_row(stage, "records", id, l.catalog)?.ok_or(Error::Invalid(
            "bibliographic exact metadata identity not catalogued",
        ))?;
        let entry = &row["entry"];
        let reference = text(entry, "source_record_ref")?;
        let raw = self.required(reference, validator, l)?;
        let record = owned(&raw, l.catalog.max_row_bytes)?;
        let route = RecordRoute::derive(entry, &record, entities)?;
        let current = metadata_ref(&record, &route, l)?;
        route.validate_record(&record, &current, &record["schema_version"])?;
        if entry["record_sha256"] != text(&current, "digest")?.trim_start_matches("sha256:") {
            return Err(Error::Invalid(
                "bibliographic metadata version current catalog binding",
            ));
        }
        let schema = route.schema.as_str();
        catalog::check_catalog_schema_with_contracts(&self.contracts, validator, schema, "", &raw)?;
        let history_ref = adjacent(reference, "source-revision-history.json")?;
        let history_raw = self.optional(&history_ref, validator, l)?;
        let history = if let Some(raw) = &history_raw {
            owned(raw, l.catalog.max_row_bytes)?
        } else {
            json!({"schema_version":"tos_source_revision_history_v1","record_id":id,"receipts":[]})
        };
        exact_keys(&history, &["schema_version", "record_id", "receipts"])?;
        let receipts = array(&history, "receipts")?;
        if !matches!(
            history["schema_version"].as_str(),
            Some("tos_source_revision_history_v1" | "tos_source_revision_history_v2")
        ) || history["record_id"] != id
            || receipts.len() > 128
            || history_raw.is_some() && receipts.is_empty()
        {
            return Err(Error::Invalid(
                "bibliographic retained Collection history identity/bounds",
            ));
        }
        let mut commands = BTreeSet::new();
        let mut head = None;
        let mut baseline = current.clone();
        let mut selected_record = record.clone();
        let mut selected_source = json!({"source_ref":reference,"record_bytes":raw.len(),
            "record_sha256":format!("sha256:{}",Digest256::of_bytes(&raw).to_hex()),
            "archive_blob_ref":null,"archive_manifest_ref":null,"archive_manifest_sha256":null,"package_revision":null});
        let mut transition = Value::Null;
        let mut found = current == *exact;
        let mut refs = Vec::new();
        for (index, receipt) in receipts.iter().enumerate() {
            check(validator, l)?;
            metadata_receipt(
                receipt,
                &history,
                current.get("id").unwrap(),
                head.as_ref(),
                &mut commands,
                &route,
                l,
            )?;
            refs.push(receipt["previous_source"].clone());
            let (previous, source) = self.archive_record(
                reference,
                receipt,
                validator,
                &route,
                &record["schema_version"],
                l,
            )?;
            if index == 0 {
                baseline = receipt["previous_source"].clone();
            }
            let fields = receipt["request"]["fields"]
                .as_object()
                .ok_or(Error::Invalid("bibliographic retained metadata patch"))?;
            let allowed = if let Some(compound) =
                compound_profile(receipt["request"]["operation"].as_str().unwrap_or(""))
            {
                vec![compound.field]
            } else {
                route.revision_fields()
            };
            if fields.is_empty() || fields.keys().any(|key| !allowed.contains(&key.as_str())) {
                return Err(Error::Invalid(
                    "bibliographic retained Collection metadata field scope",
                ));
            }
            let mut revised = previous
                .as_object()
                .ok_or(Error::Invalid("bibliographic archived Collection object"))?
                .clone();
            for (key, value) in fields {
                revised.insert(key.clone(), value.clone());
            }
            revised.insert(
                "record_version".into(),
                receipt["source"]["version"].clone(),
            );
            let revised = Value::Object(revised);
            route.validate_descriptive_delta(&previous, &revised)?;
            route.validate_record(&revised, &receipt["source"], &record["schema_version"])?;
            if metadata_ref(&revised, &route, l)? != receipt["source"] {
                return Err(Error::Invalid(
                    "bibliographic Collection successor reconstruction",
                ));
            }
            if receipt["previous_source"] == *exact {
                if found {
                    return Err(Error::Invalid(
                        "bibliographic duplicated exact Collection version",
                    ));
                }
                found = true;
                selected_record = previous;
                selected_source = source;
                transition = receipt_transition(receipt)?;
            }
            head = Some(receipt["source"].clone());
        }
        refs.push(current.clone());
        if head.is_some_and(|head| head != current) {
            return Err(Error::Invalid(
                "bibliographic Collection current metadata is not retained history head",
            ));
        }
        if !found {
            return Err(Error::Invalid(
                "bibliographic Collection exact version/digest not retained",
            ));
        }
        let catalog =
            self.catalog_location(stage, "records", Some(&route.kind), id, validator, l)?;
        let provenance = if receipts.is_empty() {
            initial_metadata_provenance(
                &route,
                &record,
                reference,
                &current,
                &catalog,
                selected_source.clone(),
            )
        } else {
            json!({"verification_scope":"selected-record-chain","all_package_bytes_verified":false,
            "catalog":catalog.render(json!({"source_ref":format!("ToS/source-witnesses/catalog/{}",route.catalog_filename),
                "source_record_ref":reference,"current_record_ref":current})),
            "descriptor":route.descriptor(&record["schema_version"]),
            "history":{"source_ref":history_raw.as_ref().map(|_|history_ref),
                "sha256":history_raw.as_ref().map(|raw|format!("sha256:{}",Digest256::of_bytes(raw).to_hex())),"receipt_count":receipts.len(),
                "retained_record_chain_verified":true,"retained_baseline_ref":baseline},
            "source":selected_source,"transition":transition})
        };
        Ok(Version {
            record: selected_record,
            provenance,
            refs,
            current_ref: current.clone(),
            version_status: if current == *exact {
                "current"
            } else {
                "historical"
            },
        })
    }
    fn archive_record(
        &mut self,
        reference: &str,
        receipt: &Value,
        validator: &SourceCatalogValidator<'_>,
        route: &RecordRoute,
        schema_version: &Value,
        l: BibliographicLimits,
    ) -> Result<(Value, Value)> {
        let contracts = self.contracts();
        validate_archived_metadata_record(
            reference,
            receipt,
            route,
            schema_version,
            l,
            |path| self.required(path, validator, l),
            |_path, _schema, raw| {
                catalog::check_catalog_schema_with_contracts(
                    &contracts,
                    validator,
                    &route.schema,
                    "",
                    raw,
                )
            },
        )
    }
    fn package_names(
        &self,
        base: &str,
        max: usize,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<Vec<String>> {
        let mut names = Vec::new();
        self.input.visit_prefix(base, validator, l, |name| {
            check(validator, l)?;
            if names.len() >= max {
                return Err(Error::Budget(
                    "bibliographic selected package member enumeration",
                ));
            }
            names.push(name.to_owned());
            Ok(())
        })?;
        Ok(names)
    }
    fn package(
        &mut self,
        base: &str,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<BTreeMap<String, Vec<u8>>> {
        let prefix = format!("{base}/");
        let names = self.package_names(base, 64, validator, l)?;
        if names.is_empty() || names.len() > 64 {
            return Err(Error::Budget(
                "bibliographic exact Claim package file count",
            ));
        }
        let mut files = BTreeMap::new();
        let mut total = 0usize;
        for name in names {
            let lock = public_form_lock(&name);
            if name.contains(['/', '\\', '\0'])
                || matches!(
                    name.as_str(),
                    "catalog" | "payload" | "private" | "local-content" | "owner-local"
                )
                || name.starts_with('.') && !lock
            {
                return Err(Error::Invalid(
                    "bibliographic exact Claim package non-flat/public member",
                ));
            }
            let raw = self.required(&format!("{prefix}{name}"), validator, l)?;
            if raw.len() > 2 * 1024 * 1024 {
                return Err(Error::Budget("bibliographic public package member bytes"));
            }
            if lock && !raw.is_empty() {
                return Err(Error::Invalid(
                    "bibliographic public writer lock has contents",
                ));
            }
            total = total
                .checked_add(raw.len())
                .filter(|n| *n <= 8 * 1024 * 1024)
                .ok_or(Error::Budget("bibliographic exact Claim package bytes"))?;
            files.insert(name, raw);
        }
        Ok(files)
    }
    fn archive_package(
        &mut self,
        reference: &str,
        receipt: &Value,
        validator: &SourceCatalogValidator<'_>,
        l: BibliographicLimits,
    ) -> Result<(BTreeMap<String, Vec<u8>>, BTreeMap<String, Value>)> {
        let previous = &receipt["previous_source"];
        exact_ref(previous, true)?;
        let revision = text(receipt, "previous_revision")?;
        let archive = format!(
            "ToS/source-witnesses/.record-revisions/{}-{}",
            Digest256::of_bytes(text(previous, "id")?.as_bytes()).to_hex(),
            hash(revision)?
        );
        if receipt["archive_path"] != archive {
            return Err(Error::Invalid(
                "bibliographic Claim archive source-derived locator",
            ));
        }
        let prefix = format!("{archive}/");
        let names = self.package_names(&archive, 65, validator, l)?;
        if names.is_empty() || names.len() > 65 {
            return Err(Error::Budget(
                "bibliographic retained Claim archive member count",
            ));
        }
        let mut blobs = BTreeMap::new();
        let mut total = 0usize;
        for name in names {
            if name != "manifest.json"
                && (name.len() != 69
                    || !name.ends_with(".blob")
                    || !name[..64]
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)))
            {
                return Err(Error::Invalid(
                    "bibliographic Claim archive non-blob member",
                ));
            }
            let raw = self.required(&format!("{prefix}{name}"), validator, l)?;
            if raw.len() > 2 * 1024 * 1024 {
                return Err(Error::Budget("bibliographic archive package member bytes"));
            }
            total = total
                .checked_add(raw.len())
                .filter(|n| *n <= 10 * 1024 * 1024)
                .ok_or(Error::Budget("bibliographic retained Claim archive bytes"))?;
            blobs.insert(name, raw);
        }
        let manifest_raw = blobs.remove("manifest.json").ok_or(Error::Invalid(
            "bibliographic Claim archive manifest absent",
        ))?;
        let manifest = owned(&manifest_raw, l.catalog.max_row_bytes)?;
        let selected = manifest["schema_version"] == "tos_source_package_archive_v2";
        let mut keys = vec![
            "schema_version",
            "source_path",
            "source",
            "revision",
            "files",
        ];
        if selected {
            keys.push("publication_protocol");
        }
        exact_keys(&manifest, &keys)?;
        if !matches!(
            manifest["schema_version"].as_str(),
            Some("tos_source_package_archive_v1" | "tos_source_package_archive_v2")
        ) || selected && manifest["publication_protocol"] != "tos_selected_source_metadata_v1"
            || manifest["source_path"] != reference
            || manifest["source"] != *previous
            || manifest["revision"] != revision
        {
            return Err(Error::Invalid(
                "bibliographic Claim archive exact manifest binding",
            ));
        }
        let bindings = manifest["files"]
            .as_object()
            .filter(|files| !files.is_empty() && files.len() <= 64)
            .ok_or(Error::Invalid("bibliographic Claim archive file bindings"))?;
        let basename = reference.rsplit('/').next().unwrap();
        let forms_name = format!("{}.human-forms.json", basename.trim_end_matches(".jsonl"));
        let selected_names = [basename, forms_name.as_str(), "claim-revision-history.json"];
        let mut files = BTreeMap::new();
        let mut locations = BTreeMap::new();
        let mut used = BTreeSet::new();
        for (name, binding) in bindings {
            exact_keys(binding, &["blob", "sha256", "bytes"])?;
            if name.is_empty()
                || name.contains(['/', '\\', '\0'])
                || matches!(
                    name.as_str(),
                    "." | ".."
                        | "payload"
                        | "owner-local"
                        | "private"
                        | "local-content"
                        | "catalog"
                )
                || name.starts_with('.') && !public_form_lock(name)
                || selected && !selected_names.contains(&name.as_str())
            {
                return Err(Error::Invalid(
                    "bibliographic Claim archive public member scope",
                ));
            }
            let sha = hash(text(binding, "sha256")?)?;
            let blob = format!("{sha}.blob");
            if binding["blob"] != blob {
                return Err(Error::Invalid("bibliographic Claim archive blob binding"));
            }
            let raw = blobs
                .get(&blob)
                .ok_or(Error::Invalid("bibliographic Claim archive blob absent"))?;
            if raw.len() as u64 != binding["bytes"].as_u64().unwrap_or(u64::MAX)
                || format!("sha256:{}", Digest256::of_bytes(raw).to_hex())
                    != text(binding, "sha256")?
            {
                return Err(Error::Invalid(
                    "bibliographic Claim archive raw byte binding",
                ));
            }
            if public_form_lock(name) && !raw.is_empty() {
                return Err(Error::Invalid(
                    "bibliographic archived writer lock contains bytes",
                ));
            }
            files.insert(name.clone(), raw.clone());
            used.insert(blob.clone());
            locations.insert(name.clone(),json!({"archive_path":format!("{prefix}{blob}"),"sha256":binding["sha256"],"bytes":binding["bytes"]}));
        }
        if used != blobs.keys().cloned().collect::<BTreeSet<_>>()
            || package_revision(&files, l)? != revision
        {
            return Err(Error::Invalid(
                "bibliographic Claim archive complete package binding",
            ));
        }
        Ok((files, locations))
    }
    fn native_membership(
        &mut self,
        stage: &mut KnowledgeStage<'_>,
        exact: &Value,
        entry: &Value,
        current: &Value,
        reference: &str,
        files: &BTreeMap<String, Vec<u8>>,
        validator: &SourceCatalogValidator<'_>,
        forms: &mut dyn BibliographicForms,
        l: BibliographicLimits,
    ) -> Result<Version> {
        let basename = reference.rsplit('/').next().unwrap();
        let stream = files.get(basename).ok_or(Error::Invalid(
            "bibliographic native Claim package stream absent",
        ))?;
        let current_records = claim_records(stream, l)?;
        let captured = historical_path(reference);
        if captured {
            for record in current_records.values() {
                historical_claim_shape(&self.contracts, record, validator, l)?;
            }
        }
        let id = text(exact, "id")?;
        let current_record = current_records
            .get(id)
            .ok_or(Error::Invalid("bibliographic current membership missing"))?;
        if record_ref(current_record, true, l)? != *current {
            return Err(Error::Invalid(
                "bibliographic current membership catalog binding",
            ));
        }
        let retained = files.get("claim-revision-history.json");
        let history = if let Some(raw) = retained {
            owned(raw, l.catalog.max_row_bytes)?
        } else {
            json!({"schema_version":"tos_claim_revision_history_v1","source_path":reference,"receipts":[]})
        };
        exact_keys(&history, &["schema_version", "source_path", "receipts"])?;
        let receipts = array(&history, "receipts")?;
        if history["schema_version"] != "tos_claim_revision_history_v1"
            || history["source_path"] != reference
            || receipts.len() > 128
        {
            return Err(Error::Invalid(
                "bibliographic native Claim correction history profile/bounds",
            ));
        }
        if receipts.is_empty()
            && current_records
                .values()
                .any(|record| record["claim_version"] != 1)
        {
            return Err(Error::Invalid(
                "bibliographic noninitial Claim stream missing correction history",
            ));
        }
        let revision = package_revision(files, l)?;
        let current_source = json!({"source_ref":reference,"stream_sha256":format!("sha256:{}",Digest256::of_bytes(stream).to_hex()),
            "stream_bytes":stream.len(),"package_revision":revision,"archive_blob_ref":null,"line":entry["source_claim_line"]});
        let mut chosen = if current == exact {
            Some((
                current_record.clone(),
                current_source,
                Value::Null,
                "current",
            ))
        } else {
            None
        };
        let mut expected: Option<Vec<u8>> = None;
        let mut commands = BTreeSet::new();
        let mut historical = BTreeSet::new();
        let mut refs = Vec::new();
        let mut initial_stream = None;
        for receipt in receipts {
            check(validator, l)?;
            claim_receipt(receipt, &mut commands, l)?;
            let (archived, locations) = self.archive_package(reference, receipt, validator, l)?;
            let before = archived
                .get(basename)
                .ok_or(Error::Invalid("bibliographic archived Claim stream absent"))?;
            let previous_records = claim_records(before, l)?;
            if captured {
                if initial_stream.is_none() {
                    initial_stream = Some(before.clone());
                }
                for name in HISTORICAL_CAPTURE {
                    if archived.get(name) != files.get(name) {
                        return Err(Error::Invalid(
                            "historical Claim correction changed captured creation bytes",
                        ));
                    }
                }
                let fields = receipt["request"]["fields"]
                    .as_object()
                    .ok_or(Error::Invalid(
                        "historical Claim retained correction fields",
                    ))?;
                if fields.len() != 1
                    || !fields.contains_key("qualifiers")
                    || fields["qualifiers"].as_object().is_none_or(|q| {
                        q.keys().any(|k| {
                            ![
                                "statement",
                                "statement_language",
                                "statement_script",
                                "display_fields",
                            ]
                            .contains(&k.as_str())
                        })
                    })
                {
                    return Err(Error::Invalid(
                        "historical Claim retained correction exceeds descriptive scope",
                    ));
                }
            }

            if expected.as_ref().is_some_and(|expected| expected != before)
                || expected.is_none()
                    && previous_records
                        .values()
                        .any(|record| record["claim_version"] != 1)
            {
                return Err(Error::Invalid(
                    "bibliographic complete shared Claim correction sequence",
                ));
            }
            let before_id = text(&receipt["previous_source"], "id")?;
            if before_id == id {
                refs.push(receipt["previous_source"].clone());
            }
            let previous = previous_records.get(before_id).ok_or(Error::Invalid(
                "bibliographic corrected Claim predecessor absent",
            ))?;
            if record_ref(previous, true, l)? != receipt["previous_source"]
                || !historical.insert(encode(
                    &receipt["previous_source"],
                    l.catalog.max_row_bytes,
                )?)
            {
                return Err(Error::Invalid(
                    "bibliographic exact Claim predecessor binding/uniqueness",
                ));
            }
            let revised = advance_claim(previous, &receipt["request"], l)?;
            if record_ref(&revised, true, l)? != receipt["source"] {
                return Err(Error::Invalid(
                    "bibliographic retained Claim successor reconstruction",
                ));
            }
            let formname = claim_form_name(reference, before_id)?;
            let prior = archived
                .get(&formname)
                .map(|raw| owned(raw, l.catalog.max_row_bytes))
                .transpose()?;
            let current_forms = files.get(&formname).ok_or(Error::Invalid(
                "bibliographic corrected Claim current form set absent",
            ))?;
            let current_forms = owned(current_forms, l.catalog.max_row_bytes)?;
            let current_source = current_records.get(before_id).ok_or(Error::Invalid(
                "bibliographic corrected sibling absent from current stream",
            ))?;
            let current_materializations =
                forms.materialize(current_source, &current_forms, 262_144)?;
            encode(&json!(current_materializations), 262_144)?;
            reconstruct_forms(
                &self.contracts,
                &revised,
                prior.as_ref(),
                &current_forms,
                receipt,
                validator,
                forms,
                l,
            )?;
            if receipt["previous_source"] == *exact {
                if chosen.is_some() {
                    return Err(Error::Invalid(
                        "bibliographic duplicated exact retained membership",
                    ));
                }
                let line = claim_line(before, before_id, l)?;
                let location = locations.get(basename).ok_or(Error::Invalid(
                    "bibliographic archived Claim source location",
                ))?;
                let source = json!({"source_ref":reference,"stream_sha256":format!("sha256:{}",Digest256::of_bytes(before).to_hex()),
                    "stream_bytes":before.len(),"package_revision":receipt["previous_revision"],"archive_blob_ref":location["archive_path"],"line":line});
                chosen = Some((
                    previous.clone(),
                    source,
                    receipt_transition(receipt)?,
                    "historical",
                ));
            }
            expected = Some(replace_claim(before, &revised, l)?);
        }
        if expected.is_some_and(|expected| expected != *stream) {
            return Err(Error::Invalid(
                "bibliographic current shared Claim stream not history head",
            ));
        }
        if captured {
            verify_historical_capture(
                reference,
                files,
                initial_stream.as_deref().unwrap_or(stream),
                &current_records,
                receipts,
                &self.contracts,
                validator,
                forms,
                l,
            )?;
        }
        refs.push(current.clone());
        let (record, source, transition, version_status) = chosen.ok_or(Error::Invalid(
            "bibliographic exact membership version/digest not retained",
        ))?;
        let catalog = self.catalog_location(stage, "claims", None, id, validator, l)?;
        let provenance = json!({"catalog":catalog.render(json!({"source_ref":"ToS/source-witnesses/catalog/claims.jsonl",
            "source_claim_file_ref":reference,"source_claim_line":entry["source_claim_line"],"current_record_ref":current,"visibility":entry["visibility"]})),
            "source":source,"history":{"source_ref":adjacent(reference,"claim-revision-history.json")?,
                "sha256":retained.map(|raw|format!("sha256:{}",Digest256::of_bytes(raw).to_hex())),"receipt_count":receipts.len(),"correction_chain_verified":true},
            "transition":transition});
        let mut provenance = provenance;
        if captured {
            provenance["history"]["adapter"] = json!("captured-historical-claim-v1");
            provenance["history"]["record_history_verified"] = json!(false);
        }
        Ok(Version {
            record,
            provenance,
            refs,
            current_ref: current.clone(),
            version_status,
        })
    }
    /// Missing/stale envelopes are emitted only after complete current catalog
    /// absence or a fully verified exact retained Claim reference set. An
    /// unsupported/corrupt carrier remains an explicit compiler refusal.
    pub(crate) fn resolve_claim_view(
        &mut self,
        stage: &mut KnowledgeStage<'_>,
        exact: &Value,
        validator: &SourceCatalogValidator<'_>,
        forms: &mut dyn BibliographicForms,
        l: BibliographicLimits,
    ) -> Result<Value> {
        exact_ref(exact, true)?;
        let id = text(exact, "id")?;
        let base = json!({"status":null,"reason":null,"exact_ref":exact,"version_status":null,"record":null,"record_digest":null,"provenance":null,
            "grants_current_use":false,"performs_assessment":false,"writes_to_source":false});
        let Some(row) = catalog::catalog_row(stage, "claims", id, l.catalog)? else {
            let mut missing = base;
            missing["status"] = json!("missing");
            missing["reason"] = json!("claim-not-in-public-catalog");
            return Ok(missing);
        };
        let current = self.resolve_claim(stage, &row["claim_ref"], validator, forms, l)?;
        if !current.refs.iter().any(|r| r == exact) {
            let present = current
                .refs
                .iter()
                .any(|r| r["version"] == exact["version"]);
            let mut unavailable = base;
            unavailable["status"] = json!(if present { "stale" } else { "missing" });
            unavailable["reason"] = json!(if present {
                "exact-version-digest-mismatch"
            } else {
                "exact-version-not-retained"
            });
            return Ok(unavailable);
        }
        let selected = if current.current_ref == *exact {
            current
        } else {
            self.resolve_claim(stage, exact, validator, forms, l)?
        };
        Ok(
            json!({"status":"available","reason":format!("exact-{}-version",selected.version_status),"exact_ref":exact,"version_status":selected.version_status,
            "record":selected.record,"record_digest":exact["digest"],"provenance":selected.provenance,"grants_current_use":false,"performs_assessment":false,"writes_to_source":false}),
        )
    }
    pub(crate) fn resolve_claim(
        &mut self,
        stage: &mut KnowledgeStage<'_>,
        exact: &Value,
        validator: &SourceCatalogValidator<'_>,
        forms: &mut dyn BibliographicForms,
        l: BibliographicLimits,
    ) -> Result<Version> {
        exact_ref(exact, true)?;
        let id = text(exact, "id")?;
        let row = catalog::catalog_row(stage, "claims", id, l.catalog)?.ok_or(Error::Invalid(
            "bibliographic exact membership not catalogued",
        ))?;
        let entry = &row["entry"];
        let (record, location) = graph::slot(stage, "claim", id, l)?
            .ok_or(Error::Invalid("bibliographic membership exact source slot"))?;
        let current = record_ref(&record, true, l)?;
        let reference = text(entry, "source_claim_file_ref")?;
        let raw = self.required(reference, validator, l)?;
        if Digest256::of_bytes(&raw).to_hex() != text(&location, "file_sha256")?
            || raw.len() as u64 != location["file_bytes"].as_u64().unwrap_or(u64::MAX)
        {
            return Err(Error::Invalid(
                "bibliographic membership exact stream binding",
            ));
        }
        // Legacy membership has no native revision package. Every sibling is
        // checked to keep one selected row from hiding a restricted carrier.
        let parts = reference.split('/').collect::<Vec<_>>();
        let legacy = parts.len() == 6
            && parts[..3] == ["ToS", "source-witnesses", "collections"]
            && parts[5] == "membership-claims.jsonl";
        let history = json!({"source_ref":null,"sha256":null,"receipt_count":0,
            "correction_chain_verified":false,"adapter":"retained-collection-membership-v1"});
        let revision = Value::Null;
        if !legacy {
            if parts.len() < 4
                || parts[5.min(parts.len() - 1)] == "membership-claims.jsonl"
                || !(reference.ends_with("/source-claims.jsonl") || historical_path(reference))
            {
                return Err(Error::Invalid(
                    "bibliographic membership source-family route",
                ));
            }
            let base = reference.rsplit_once('/').unwrap().0;
            let files = self.package(base, validator, l)?;
            return self.native_membership(
                stage, exact, entry, &current, reference, &files, validator, forms, l,
            );
        }
        if current != *exact {
            return Err(Error::Invalid(
                "bibliographic legacy membership exact version not retained",
            ));
        }
        let mut siblings = BTreeSet::new();
        if raw.len() > 1_048_576 {
            return Err(Error::Budget(
                "bibliographic retained membership stream bytes",
            ));
        }
        for bytes in raw
            .split(|b| *b == b'\n')
            .filter(|row| !row.iter().all(u8::is_ascii_whitespace))
        {
            check(validator, l)?;
            let sibling = owned(bytes, l.catalog.max_row_bytes)?;
            exact_ref(&record_ref(&sibling, true, l)?, true)?;
            if !siblings.insert(text(&sibling, "claim_id")?.to_owned())
                || !matches!(
                    sibling["visibility"].as_str(),
                    Some("public" | "public_metadata_only")
                )
                || legacy
                    && (sibling["schema_version"] != "tos_claim_packet_v1"
                        || sibling["claim_type"] != "bibliographic"
                        || sibling["predicate"] != "contains_work"
                        || sibling["assertion_layer"] != "bibliographic_assertion"
                        || sibling.get("polarity").is_some_and(|v| v != "positive")
                        || !text(&sibling, "subject_ref")?.starts_with("tos.collection.")
                        || !text(&sibling, "object")?.starts_with("tos.work."))
                || !legacy && (sibling["claim_type"] != "relation" || sibling["claim_version"] != 1)
            {
                return Err(Error::Invalid(
                    "bibliographic public exact membership stream contract",
                ));
            }
        }
        let catalog = self.catalog_location(stage, "claims", None, id, validator, l)?;
        let provenance = json!({"catalog":catalog.render(json!({"source_ref":"ToS/source-witnesses/catalog/claims.jsonl",
            "source_claim_file_ref":reference,"source_claim_line":entry["source_claim_line"],"current_record_ref":current,"visibility":entry["visibility"]})),
            "source":{"source_ref":reference,"stream_sha256":format!("sha256:{}",Digest256::of_bytes(&raw).to_hex()),
                "stream_bytes":raw.len(),"package_revision":revision,"archive_blob_ref":null,"line":entry["source_claim_line"]},
            "history":history,"transition":null});
        Ok(Version {
            record,
            provenance,
            refs: vec![current.clone()],
            current_ref: current.clone(),
            version_status: "current",
        })
    }
}
/// Owner descriptors selected from the already sealed registry and catalog.
/// This is the maintained MetadataVersionReader dispatch, not an admission API.
pub(crate) fn supplied_metadata_descriptor(
    entry: &Value,
    record: &Value,
    entities: &Value,
) -> Result<Value> {
    let route = RecordRoute::derive(entry, record, entities)?;
    Ok(route.descriptor(&record["schema_version"]))
}

/// Exact pure validation used by the event-local metadata producer. It shares
/// the maintained route dispatch and public-profile checks with `Versions`;
/// source membership and JSON-Schema execution remain with the selected-cut
/// caller.
pub(crate) fn validate_supplied_metadata_record(
    entry: &Value,
    record: &Value,
    entities: &Value,
    max_row_bytes: usize,
) -> Result<()> {
    let route = RecordRoute::derive(entry, record, entities)?;
    let current = json!({
        "id":record[route.identity],
        "version":record["record_version"],
        "digest":format!("sha256:{}", digest(record, max_row_bytes)?)
    });
    route.validate_record(record, &current, &record["schema_version"])?;
    if entry.get("record_id") != current.get("id")
        || entry.get("record_sha256").and_then(Value::as_str)
            != current
                .get("digest")
                .and_then(Value::as_str)
                .and_then(|digest| digest.strip_prefix("sha256:"))
        || entry.get("source_schema_ref").and_then(Value::as_str) != Some(route.schema.as_str())
    {
        return Err(Error::Invalid(
            "bibliographic supplied metadata catalog binding",
        ));
    }
    Ok(())
}

/// Select the maintained schema route for one exact current metadata owner
/// without requiring a pre-existing catalog row. The caller still verifies
/// the row identity and executes the selected schema against the exact bytes.
pub fn selected_metadata_record_schema_path_v1(
    record_type: &str,
    source_path: &str,
    record: &Value,
    entities: &Value,
) -> Result<String> {
    let entry = json!({
        "record_type": record_type,
        "source_record_ref": source_path,
    });
    Ok(RecordRoute::derive_historical(&entry, record, entities)?.schema)
}

/// Validate an available historical payload with the same maintained owner
/// route and public-profile rule as the current metadata record. Historical
/// refs deliberately do not compare against today's catalog digest.
pub(crate) fn validate_supplied_metadata_version(
    entry: &Value,
    reference: &Value,
    record: &Value,
    entities: &Value,
    max_row_bytes: usize,
) -> Result<String> {
    let route = RecordRoute::derive_historical(entry, record, entities)?;
    let expected_digest = format!("sha256:{}", digest(record, max_row_bytes)?);
    if reference.get("digest").and_then(Value::as_str) != Some(expected_digest.as_str()) {
        return Err(Error::Invalid("bibliographic historical metadata digest"));
    }
    route.validate_record(record, reference, &record["schema_version"])?;
    Ok(route.schema)
}

struct RecordRoute {
    kind: String,
    adapter: &'static str,
    identity: &'static str,
    basename: String,
    catalog_filename: String,
    type_id: String,
    schema: String,
}
impl RecordRoute {
    fn derive(entry: &Value, record: &Value, entities: &Value) -> Result<Self> {
        Self::derive_with_schema_binding(entry, record, entities, true)
    }
    fn derive_historical(entry: &Value, record: &Value, entities: &Value) -> Result<Self> {
        Self::derive_with_schema_binding(entry, record, entities, false)
    }
    fn derive_with_schema_binding(
        entry: &Value,
        record: &Value,
        entities: &Value,
        bind_entry_schema: bool,
    ) -> Result<Self> {
        let kind = text(entry, "record_type")?;
        let reference = text(entry, "source_record_ref")?;
        let native_witness = kind == "artifact"
            || kind == "composite" && reference.ends_with("/composite-witness.json");
        let native_corpus = [
            "agent",
            "place",
            "organization",
            "work",
            "expression",
            "edition",
            "collection",
            "item",
        ]
        .contains(&kind);
        let types = array(entities, "types")?;
        let matches = types
            .iter()
            .filter(|e| {
                e.get("source_mappings")
                    .and_then(Value::as_array)
                    .is_some_and(|ms| {
                        ms.iter().any(|m| {
                            m["source_graph"] == "source-navigation" && m["source_kind_id"] == kind
                        })
                    })
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(Error::Invalid(
                "bibliographic metadata owner registry mapping",
            ));
        }
        let owner = matches[0];
        let type_id = text(owner, "type_id")?.to_owned();
        let (adapter, identity, basename, filename, schema) = if native_witness {
            let (identity, basename, filename) = if kind == "artifact" {
                ("artifact_id", "artifact-witness.json", "artifacts.jsonl")
            } else {
                ("composite_id", "composite-witness.json", "composites.jsonl")
            };
            let schema = match (kind, record["schema_version"].as_str()) {
                ("artifact", Some("tos_artifact_source_witness_v1")) => {
                    "ToS/contracts/artifact-source-witness.schema.json"
                }
                ("artifact", Some("tos_artifact_source_witness_v2")) => {
                    "ToS/contracts/artifact-source-witness-v2.schema.json"
                }
                ("composite", Some("tos_scholarly_composite_witness_v1")) => {
                    "ToS/contracts/scholarly-composite-witness.schema.json"
                }
                _ => {
                    return Err(Error::Invalid(
                        "bibliographic native witness exact schema descriptor",
                    ));
                }
            };
            if bind_entry_schema && entry["source_schema_ref"] != schema {
                return Err(Error::Invalid(
                    "bibliographic native witness catalog schema binding",
                ));
            }
            (
                "native-witness",
                identity,
                basename.to_owned(),
                filename.to_owned(),
                schema.to_owned(),
            )
        } else if kind == "link" {
            (
                "native-link",
                "record_id",
                "link.json".into(),
                "links.jsonl".into(),
                "ToS/contracts/source-link.schema.json".into(),
            )
        } else if native_corpus {
            let filename = match kind {
                "agent" => "agents.jsonl",
                "place" => "places.jsonl",
                "organization" => "organizations.jsonl",
                "work" => "works.jsonl",
                "expression" => "expressions.jsonl",
                "edition" => "editions.jsonl",
                "collection" => "collections.jsonl",
                "item" => "items.jsonl",
                _ => unreachable!(),
            };
            (
                "native-corpus",
                "record_id",
                format!("{kind}.json"),
                filename.into(),
                "ToS/contracts/corpus-record.schema.json".into(),
            )
        } else {
            let profile = &owner["source_record_profile"];
            if profile["record_type"] != kind || profile["id_prefix"] != format!("tos.{kind}.") {
                return Err(Error::Invalid(
                    "bibliographic declared metadata owner profile",
                ));
            }
            let schemas = array(profile, "schemas")?
                .iter()
                .filter(|s| s["schema_version"] == record["schema_version"])
                .collect::<Vec<_>>();
            if schemas.len() != 1 {
                return Err(Error::Invalid(
                    "bibliographic metadata selected schema profile",
                ));
            }
            let schema = text(schemas[0], "schema_ref")?;
            if bind_entry_schema && entry["source_schema_ref"] != schema {
                return Err(Error::Invalid(
                    "bibliographic metadata catalog schema binding",
                ));
            }
            (
                "declared-profile",
                "record_id",
                text(profile, "source_basename")?.into(),
                text(profile, "catalog_filename")?.into(),
                schema.into(),
            )
        };
        let parts = reference.split('/').collect::<Vec<_>>();
        if parts.len() < 5
            || reference.contains(['\\', '\0'])
            || parts[..2] != ["ToS", "source-witnesses"]
            || parts.last() != Some(&basename.as_str())
            || parts.iter().any(|p| {
                p.is_empty()
                    || *p == ".."
                    || p.starts_with('.')
                    || [
                        "catalog",
                        "owner-local",
                        "payload",
                        "local-content",
                        "private",
                    ]
                    .contains(p)
            })
            || adapter == "native-link" && parts[2] != "links"
            || adapter == "native-witness"
                && parts[2]
                    != if kind == "artifact" {
                        "artifacts"
                    } else {
                        "scholarly-composites"
                    }
            || adapter == "declared-profile"
                && kind == "composite"
                && (parts.len() < 7 || parts[2] != "scholarly-composites")
        {
            return Err(Error::Invalid(
                "bibliographic metadata exact owner source home",
            ));
        }
        Ok(Self {
            kind: kind.into(),
            adapter,
            identity,
            basename,
            catalog_filename: filename,
            type_id,
            schema,
        })
    }
    fn validate_record(
        &self,
        record: &Value,
        reference: &Value,
        schema_version: &Value,
    ) -> Result<()> {
        if record[self.identity] != reference["id"]
            || !text(reference, "id")?.starts_with(&format!("tos.{}.", self.kind))
            || record["schema_version"] != *schema_version
            || !schema_version.is_string()
            || self.adapter != "native-witness" && record["record_type"] != self.kind
        {
            return Err(Error::Invalid(
                "bibliographic metadata descriptor changed within history",
            ));
        }
        let public = |v: &Value| matches!(v.as_str(), Some("public" | "public_metadata_only"));
        let valid = match self.adapter {
            "native-corpus" => {
                record["schema_version"] == "tos_corpus_record_v1"
                    && record.get("visibility").is_none()
            }
            "native-link" => {
                record["schema_version"] == "tos_source_link_v1"
                    && record.get("visibility").is_none()
            }
            "native-witness" => public(&record["authority"]["visibility"]),
            _ => public(&record["visibility"]),
        };
        if !valid {
            return Err(Error::Invalid(
                "bibliographic metadata public owner profile",
            ));
        }
        Ok(())
    }
    fn revision_fields(&self) -> Vec<&'static str> {
        match self.adapter {
            "native-corpus" => vec!["preferred_label", "notes", "field_languages", "source_refs"],
            "declared-profile" => vec![
                "preferred_label",
                "variant_labels",
                "notes",
                "field_languages",
                "source_refs",
                "extensions",
                "semantic_content",
                "semantic_scope",
            ],
            _ => match self.kind.as_str() {
                "artifact" => vec![
                    "path_identity",
                    "physical_description",
                    "find_context",
                    "bibliography",
                ],
                "composite" => vec!["preferred_label", "editorial_object"],
                "link" => vec![
                    "preferred_label",
                    "variant_labels",
                    "notes",
                    "source_refs",
                    "provider_label",
                ],
                _ => Vec::new(),
            },
        }
    }
    fn validate_descriptive_delta(&self, previous: &Value, revised: &Value) -> Result<()> {
        if self.adapter == "native-witness"
            && self.kind == "artifact"
            && ["basis", "provider_independent"]
                .iter()
                .any(|k| previous["path_identity"][*k] != revised["path_identity"][*k])
        {
            return Err(Error::Invalid(
                "bibliographic artifact physical identity path changed",
            ));
        }
        Ok(())
    }
    fn descriptor(&self, schema_version: &Value) -> Value {
        json!({"adapter":self.adapter,"record_type":self.kind,"profile_type_id":self.type_id,"source_schema_ref":self.schema,
            "source_schema_version":schema_version,"source_scope":"public_metadata_only","record_kind":"subject","identity_field":self.identity,
            "source_basename":self.basename,"schema_version":schema_version,"schema_ref":self.schema,"type_id":self.type_id})
    }
}
/// Exact zero-receipt branch of the maintained metadata Versions provenance.
/// The independent caller must already prove current original bytes and history
/// absence. It grants neither a current source read nor retained availability.
fn initial_metadata_provenance(
    route: &RecordRoute,
    record: &Value,
    reference: &str,
    current: &Value,
    catalog: &CatalogLocation,
    source: Value,
) -> Value {
    json!({"verification_scope":"selected-record-chain","all_package_bytes_verified":false,
        "catalog":catalog.render(json!({"source_ref":format!("ToS/source-witnesses/catalog/{}",route.catalog_filename),
            "source_record_ref":reference,"current_record_ref":current})),
        "descriptor":route.descriptor(&record["schema_version"]),
        "history":{"source_ref":null,"sha256":null,"receipt_count":0,
            "retained_record_chain_verified":true,"retained_baseline_ref":current},
        "source":source,"transition":null})
}
/// Managed creation consumes only the same current initial Agent branch.
/// Actual committed creation/history absence is checked by the CMD issuer;
/// this helper checks the existing registry/route/ref/catalog laws.
pub(crate) fn initial_managed_agent_version(
    entry: &Value,
    record: &Value,
    raw: &[u8],
    entities: &Value,
    catalog: (u64, String),
    l: BibliographicLimits,
) -> Result<Version> {
    initial_managed_agent_version_at(
        entry,
        record,
        raw,
        entities,
        CatalogLocation::Legacy(catalog.0, catalog.1),
        l,
    )
}

/// Genuine V2 epoch identity, never fabricated V1 ordinal or file SHA.
pub(crate) fn initial_managed_agent_version_with_epoch(
    entry: &Value,
    record: &Value,
    raw: &[u8],
    entities: &Value,
    epoch: &crate::VersionsCatalogEpochV2,
    tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
    deadline: std::time::Instant,
    cancelled: &std::sync::atomic::AtomicBool,
    l: BibliographicLimits,
) -> Result<Version> {
    initial_managed_agent_version_with_epoch_and_work(
        entry,
        record,
        raw,
        entities,
        epoch,
        tree_limits,
        deadline,
        cancelled,
        l,
    )
    .map(|(version, _)| version)
}

pub(crate) fn initial_managed_agent_version_with_epoch_and_work(
    entry: &Value,
    record: &Value,
    raw: &[u8],
    entities: &Value,
    epoch: &crate::VersionsCatalogEpochV2,
    tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
    deadline: std::time::Instant,
    cancelled: &std::sync::atomic::AtomicBool,
    l: BibliographicLimits,
) -> Result<(Version, tos_segment_store::AuthenticatedTreeWorkV1)> {
    let (location, work) = epoch.provenance_with_work(
        "records",
        text(entry, "record_id")?,
        entry,
        tree_limits,
        deadline,
        cancelled,
    )?;
    initial_managed_agent_version_at(
        entry,
        record,
        raw,
        entities,
        CatalogLocation::Epoch(location),
        l,
    )
    .map(|version| (version, work))
}

fn initial_managed_agent_version_at(
    entry: &Value,
    record: &Value,
    raw: &[u8],
    entities: &Value,
    catalog: CatalogLocation,
    l: BibliographicLimits,
) -> Result<Version> {
    if entry["record_type"] != "agent" || record["record_version"] != 1 {
        return Err(Error::Invalid(
            "managed successor requires initial Agent creation",
        ));
    }
    let route = RecordRoute::derive(entry, record, entities)?;
    let current = metadata_ref(record, &route, l)?;
    route.validate_record(record, &current, &record["schema_version"])?;
    if entry["record_sha256"] != text(&current, "digest")?.trim_start_matches("sha256:") {
        return Err(Error::Invalid("managed Agent exact catalog source digest"));
    }
    let reference = text(entry, "source_record_ref")?;
    let source = json!({"source_ref":reference,"record_bytes":raw.len(),
        "record_sha256":format!("sha256:{}",Digest256::of_bytes(raw).to_hex()),
        "archive_blob_ref":null,"archive_manifest_ref":null,"archive_manifest_sha256":null,"package_revision":null});
    let provenance =
        initial_metadata_provenance(&route, record, reference, &current, &catalog, source);
    Ok(Version {
        record: record.clone(),
        provenance,
        refs: vec![current.clone()],
        current_ref: current,
        version_status: "current",
    })
}

fn metadata_ref(record: &Value, route: &RecordRoute, l: BibliographicLimits) -> Result<Value> {
    let reference = json!({"id":record[route.identity],"version":record["record_version"],"digest":format!("sha256:{}",digest(record,l.catalog.max_row_bytes)?)});
    exact_ref(&reference, false)?;
    Ok(reference)
}
struct CompoundProfile {
    parent: &'static str,
    field: &'static str,
    child: &'static str,
    predicate: &'static str,
    schema: &'static str,
    extra: &'static [&'static str],
}
fn compound_profile(operation: &str) -> Option<CompoundProfile> {
    let (parent, field, child, predicate, schema, extra) = match operation {
        "collection.work.attach" => (
            "collection",
            "membership_claim_refs",
            "work",
            "contains_work",
            "tos_local_collection_membership_command_v1",
            &["work", "claim", "forms", "claim_forms", "reason"][..],
        ),
        "work.expression.create" => (
            "work",
            "expression_claim_refs",
            "record",
            "has_expression",
            "tos_local_work_expression_command_v1",
            &[
                "record",
                "claim",
                "forms",
                "expression_forms",
                "claim_forms",
                "reason",
            ][..],
        ),
        "expression.responsibility.attach" => (
            "expression",
            "responsibility_claim_refs",
            "agent",
            "translated_by",
            "tos_local_expression_responsibility_command_v1",
            &["agent", "claim", "forms", "claim_forms", "reason"][..],
        ),
        "expression.edition.create" => (
            "expression",
            "embodiment_claim_refs",
            "record",
            "embodied_by",
            "tos_local_expression_edition_command_v1",
            &[
                "record",
                "claim",
                "forms",
                "edition_forms",
                "claim_forms",
                "reason",
            ][..],
        ),
        "item.adopt" => (
            "edition",
            "exemplar_claim_refs",
            "record",
            "exemplified_by",
            "tos_local_item_adoption_command_v1",
            &[
                "record",
                "claim",
                "forms",
                "item_forms",
                "claim_forms",
                "reason",
                "rights",
                "item_kind",
                "inventory",
                "inventory_limitation",
                "fixity_verified_at",
            ][..],
        ),
        _ => return None,
    };
    Some(CompoundProfile {
        parent,
        field,
        child,
        predicate,
        schema,
        extra,
    })
}

pub(crate) struct Version {
    pub(crate) record: Value,
    pub(crate) provenance: Value,
    pub(crate) version_status: &'static str,
    pub(crate) refs: Vec<Value>,
    pub(crate) current_ref: Value,
}

/// One exact selected source/schema pair aligned with a navigation history
/// resolution. Historical paths point to archive blobs, not reconstructed
/// current files.
pub struct SelectedMetadataVersionSourceV1<'a> {
    pub source_path: String,
    pub source_record_bytes: &'a [u8],
    pub schema_path: String,
    pub schema_bytes: &'a [u8],
}

/// Maintained Record history resolution from one exact PG-selected file set.
/// Every retained reference has a real archive record source; the current
/// source and history absence are also witnessed by the same selected set.
pub struct SelectedMetadataHistoryV1<'a> {
    pub current_record: Value,
    pub current_record_bytes: &'a [u8],
    pub source_path: String,
    pub schema_path: String,
    pub schema_bytes: &'a [u8],
    pub history_path: String,
    pub history_bytes: Option<&'a [u8]>,
    pub history: Value,
    pub versions: Vec<(Value, Value)>,
    pub version_sources: Vec<SelectedMetadataVersionSourceV1<'a>>,
}

/// Exact archive manifest reads derivable after current Record and history
/// bytes are selected. Receipt structure and every source-derived archive
/// locator are checked before a caller asks for these members.
pub fn selected_metadata_archive_manifest_paths_v1(
    entry: &Value,
    entities: &Value,
    files: &SelectedBibliographicFilesV1,
    l: BibliographicLimits,
) -> Result<Vec<String>> {
    l.validate()?;
    let source_path = text(entry, "source_record_ref")?;
    let current_raw = files.required(source_path)?;
    let current_record = owned(current_raw, l.catalog.max_row_bytes)?;
    let route = RecordRoute::derive(entry, &current_record, entities)?;
    let current = metadata_ref(&current_record, &route, l)?;
    route.validate_record(&current_record, &current, &current_record["schema_version"])?;
    let history_path = adjacent(source_path, "source-revision-history.json")?;
    let history = match files.optional(&history_path)? {
        Some(raw) => owned(raw, l.catalog.max_row_bytes)?,
        None => {
            json!({"schema_version":"tos_source_revision_history_v1","record_id":entry["record_id"],"receipts":[]})
        }
    };
    exact_keys(&history, &["schema_version", "record_id", "receipts"])?;
    let receipts = array(&history, "receipts")?;
    if !matches!(
        history["schema_version"].as_str(),
        Some("tos_source_revision_history_v1" | "tos_source_revision_history_v2")
    ) || history["record_id"] != entry["record_id"]
        || receipts.len() > 128
        || files.optional(&history_path)?.is_some() && receipts.is_empty()
    {
        return Err(Error::Invalid(
            "selected metadata archive history identity/bounds",
        ));
    }
    let mut commands = BTreeSet::new();
    let mut head = None;
    let mut paths = BTreeSet::new();
    for receipt in receipts {
        metadata_receipt(
            receipt,
            &history,
            current.get("id").ok_or(Error::Invalid(
                "selected metadata archive record identity absent",
            ))?,
            head.as_ref(),
            &mut commands,
            &route,
            l,
        )?;
        let archive = text(receipt, "archive_path")?;
        let manifest = format!("{archive}/manifest.json");
        if !paths.insert(manifest.clone()) {
            return Err(Error::Invalid(
                "selected metadata archive manifest repeated",
            ));
        }
        RelativePath::parse(&manifest)
            .map_err(|_| Error::Invalid("selected metadata archive manifest path"))?;
        head = Some(receipt["source"].clone());
    }
    if head.as_ref().is_some_and(|value| value != &current) {
        return Err(Error::Invalid(
            "selected metadata archive history head differs",
        ));
    }
    if paths.len() > 128 {
        return Err(Error::Budget("selected metadata archive manifest count"));
    }
    Ok(paths.into_iter().collect())
}

/// Derive the one archived Record blob path from an exact selected manifest.
/// Full manifest/package fixity and the RecordRoute are checked again by the
/// shared archive verifier before any history result is returned.
pub fn selected_metadata_archive_record_blob_path_v1(
    manifest_path: &str,
    manifest_raw: &[u8],
    l: BibliographicLimits,
) -> Result<String> {
    selected_metadata_archive_blob_path_v1(manifest_path, manifest_raw, false, l)?.ok_or(
        Error::Invalid("selected metadata archive Record member absent"),
    )
}

/// Select creation-time HumanForms from the same exact archive manifest as
/// the retained Record. Absence is established by the manifest file map.
pub fn selected_metadata_archive_forms_blob_path_v1(
    manifest_path: &str,
    manifest_raw: &[u8],
    l: BibliographicLimits,
) -> Result<Option<String>> {
    selected_metadata_archive_blob_path_v1(manifest_path, manifest_raw, true, l)
}

/// Read only the selected archived forms blob and verify its declared bytes.
/// The complete history resolver separately verifies the package commitment.
pub fn resolve_selected_metadata_archive_forms_v1<'a>(
    manifest_path: &str,
    files: &'a SelectedBibliographicFilesV1,
    l: BibliographicLimits,
) -> Result<Option<(String, &'a [u8])>> {
    let manifest_raw = files.required(manifest_path)?;
    let Some(blob_path) =
        selected_metadata_archive_forms_blob_path_v1(manifest_path, manifest_raw, l)?
    else {
        return Ok(None);
    };
    let manifest = owned(manifest_raw, l.catalog.max_row_bytes)?;
    let source_path = text(&manifest, "source_path")?;
    let basename = source_path
        .rsplit('/')
        .next()
        .ok_or(Error::Invalid("selected metadata archive source basename"))?;
    let forms_name = format!("{}.human-forms.json", basename.trim_end_matches(".json"));
    let binding = &manifest["files"][&forms_name];
    let raw = files.required(&blob_path)?;
    if binding["bytes"].as_u64() != Some(raw.len() as u64)
        || binding["sha256"] != format!("sha256:{}", Digest256::of_bytes(raw).to_hex())
    {
        return Err(Error::Invalid(
            "selected metadata archived forms raw binding",
        ));
    }
    Ok(Some((blob_path, raw)))
}

fn selected_metadata_archive_blob_path_v1(
    manifest_path: &str,
    manifest_raw: &[u8],
    forms: bool,
    l: BibliographicLimits,
) -> Result<Option<String>> {
    l.validate()?;
    let manifest = owned(manifest_raw, l.catalog.max_row_bytes)?;
    let schema = manifest
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("selected metadata archive schema"))?;
    let selected = schema == "tos_source_package_archive_v2";
    let mut keys = vec![
        "schema_version",
        "source_path",
        "source",
        "revision",
        "files",
    ];
    if selected {
        keys.push("publication_protocol");
    }
    exact_keys(&manifest, &keys)?;
    if !matches!(
        schema,
        "tos_source_package_archive_v1" | "tos_source_package_archive_v2"
    ) || selected && manifest["publication_protocol"] != "tos_selected_source_metadata_v1"
    {
        return Err(Error::Invalid("selected metadata archive manifest profile"));
    }
    let source_path = text(&manifest, "source_path")?;
    RelativePath::parse(source_path)
        .map_err(|_| Error::Invalid("selected metadata archive source path"))?;
    let source = &manifest["source"];
    exact_ref(source, false)?;
    let revision = text(&manifest, "revision")?;
    let revision_sha = hash(revision)?;
    let expected_archive = format!(
        "ToS/source-witnesses/.record-revisions/{}-{}",
        Digest256::of_bytes(text(source, "id")?.as_bytes()).to_hex(),
        revision_sha
    );
    if manifest_path != format!("{expected_archive}/manifest.json") {
        return Err(Error::Invalid("selected metadata archive manifest locator"));
    }
    let basename = source_path
        .rsplit('/')
        .next()
        .ok_or(Error::Invalid("selected metadata archive source basename"))?;
    let member_name = if forms {
        format!("{}.human-forms.json", basename.trim_end_matches(".json"))
    } else {
        basename.to_owned()
    };
    let bindings = manifest["files"]
        .as_object()
        .ok_or(Error::Invalid("selected metadata archive file map"))?;
    let Some(binding) = bindings.get(&member_name) else {
        if forms {
            return Ok(None);
        }
        return Err(Error::Invalid(
            "selected metadata archive Record member absent",
        ));
    };
    exact_keys(binding, &["blob", "sha256", "bytes"])?;
    let sha = hash(text(binding, "sha256")?)?;
    let blob = text(binding, "blob")?;
    if blob != format!("{sha}.blob")
        || binding["bytes"]
            .as_u64()
            .is_none_or(|bytes| bytes > 2 * 1024 * 1024)
    {
        return Err(Error::Invalid("selected metadata archive Record binding"));
    }
    let blob_path = format!("{expected_archive}/{blob}");
    RelativePath::parse(&blob_path)
        .map_err(|_| Error::Invalid("selected metadata archive Record blob path"))?;
    Ok(Some(blob_path))
}

/// Resolve the complete current Record chain from exact selected PG bytes.
/// This keeps the maintained RecordRoute, receipt, patch reconstruction, and
/// archive-manifest checks; it never manufactures a whole-corpus revision.
pub fn resolve_selected_metadata_history_v1<'a>(
    entry: &Value,
    entities: &Value,
    files: &'a SelectedBibliographicFilesV1,
    validator: &SourceCatalogValidator<'_>,
    expected_event_revision: Digest256,
    l: BibliographicLimits,
) -> Result<SelectedMetadataHistoryV1<'a>> {
    l.validate()?;
    if validator.selected_event_source_revision()? != expected_event_revision {
        return Err(Error::Invalid(
            "selected metadata history event revision differs",
        ));
    }
    let registry_path = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
    let registry_raw = files.required(registry_path)?;
    let registry = owned(
        registry_raw,
        l.catalog.max_contract_bytes.min(l.catalog.max_row_bytes),
    )?;
    if registry != *entities {
        return Err(Error::Invalid(
            "selected metadata history entity registry differs",
        ));
    }

    let source_path = text(entry, "source_record_ref")?.to_owned();
    let current_raw = files.required(&source_path)?;
    let current_record = owned(current_raw, l.catalog.max_row_bytes)?;
    let route = RecordRoute::derive(entry, &current_record, entities)?;
    let current_ref = metadata_ref(&current_record, &route, l)?;
    route.validate_record(
        &current_record,
        &current_ref,
        &current_record["schema_version"],
    )?;
    if entry["record_sha256"] != text(&current_ref, "digest")?.trim_start_matches("sha256:")
        || catalog::render_catalog_record(
            &current_record,
            &source_path,
            Some(&route.schema),
            l.catalog.max_output_row_bytes,
        )? != *entry
    {
        return Err(Error::Invalid(
            "selected metadata current catalog entry differs",
        ));
    }
    let schema_path = route.schema.clone();
    let schema_bytes = files.required(&schema_path)?;
    validator.check_selected_event_document_exact(
        expected_event_revision,
        &source_path,
        &schema_path,
        schema_bytes,
        current_raw,
    )?;

    let history_path = adjacent(&source_path, "source-revision-history.json")?;
    let history_bytes = files.optional(&history_path)?;
    let history = if let Some(raw) = history_bytes {
        owned(raw, l.catalog.max_row_bytes)?
    } else {
        json!({"schema_version":"tos_source_revision_history_v1","record_id":entry["record_id"],"receipts":[]})
    };
    exact_keys(&history, &["schema_version", "record_id", "receipts"])?;
    let receipts = array(&history, "receipts")?;
    if !matches!(
        history["schema_version"].as_str(),
        Some("tos_source_revision_history_v1" | "tos_source_revision_history_v2")
    ) || history["record_id"] != entry["record_id"]
        || receipts.len() > 128
        || history_bytes.is_some() && receipts.is_empty()
    {
        return Err(Error::Invalid(
            "selected metadata retained history identity/bounds",
        ));
    }

    let mut commands = BTreeSet::new();
    let mut head = None;
    let mut baseline = current_ref.clone();
    let mut retained = Vec::with_capacity(receipts.len());
    let mut refs = Vec::with_capacity(receipts.len() + 1);
    for (index, receipt) in receipts.iter().enumerate() {
        check(validator, l)?;
        metadata_receipt(
            receipt,
            &history,
            current_ref
                .get("id")
                .ok_or(Error::Invalid("selected metadata record identity absent"))?,
            head.as_ref(),
            &mut commands,
            &route,
            l,
        )?;
        let reference = receipt["previous_source"].clone();
        refs.push(reference.clone());
        let (previous, source) = validate_archived_metadata_record(
            &source_path,
            receipt,
            &route,
            &current_record["schema_version"],
            l,
            |path| Ok(files.required(path)?.to_vec()),
            |path, schema, raw| {
                let schema_raw = files.required(schema)?;
                validator.check_selected_event_document_exact(
                    expected_event_revision,
                    path,
                    schema,
                    schema_raw,
                    raw,
                )
            },
        )?;
        if index == 0 {
            baseline = reference.clone();
        }
        let previous_record_path = source["archive_blob_ref"]
            .as_str()
            .ok_or(Error::Invalid("selected metadata archive blob path absent"))?
            .to_owned();
        let previous_raw = files.required(&previous_record_path)?;
        let previous_schema_path = validate_supplied_metadata_version(
            entry,
            &reference,
            &previous,
            entities,
            l.catalog.max_row_bytes,
        )?;
        let previous_schema_bytes = files.required(&previous_schema_path)?;
        validator.check_selected_event_document_exact(
            expected_event_revision,
            &previous_record_path,
            &previous_schema_path,
            previous_schema_bytes,
            previous_raw,
        )?;

        let fields = receipt["request"]["fields"]
            .as_object()
            .ok_or(Error::Invalid("selected metadata retained patch"))?;
        let allowed = if let Some(compound) =
            compound_profile(receipt["request"]["operation"].as_str().unwrap_or(""))
        {
            vec![compound.field]
        } else {
            route.revision_fields()
        };
        if fields.is_empty() || fields.keys().any(|key| !allowed.contains(&key.as_str())) {
            return Err(Error::Invalid("selected metadata retained field scope"));
        }
        let mut revised = previous
            .as_object()
            .ok_or(Error::Invalid("selected metadata archived object"))?
            .clone();
        for (key, value) in fields {
            revised.insert(key.clone(), value.clone());
        }
        revised.insert(
            "record_version".into(),
            receipt["source"]["version"].clone(),
        );
        let revised = Value::Object(revised);
        route.validate_descriptive_delta(&previous, &revised)?;
        route.validate_record(
            &revised,
            &receipt["source"],
            &current_record["schema_version"],
        )?;
        if metadata_ref(&revised, &route, l)? != receipt["source"] {
            return Err(Error::Invalid("selected metadata successor reconstruction"));
        }
        retained.push((
            reference,
            previous,
            source,
            previous_record_path,
            previous_raw,
            previous_schema_path,
            previous_schema_bytes,
            receipt_transition(receipt)?,
        ));
        head = Some(receipt["source"].clone());
    }
    refs.push(current_ref.clone());
    if head.as_ref().is_some_and(|value| value != &current_ref) {
        return Err(Error::Invalid(
            "selected metadata current record is not history head",
        ));
    }

    let history_provenance = json!({
        "source_ref":history_bytes.map(|_| history_path.clone()),
        "sha256":history_bytes.map(|raw| format!("sha256:{}", Digest256::of_bytes(raw).to_hex())),
        "receipt_count":receipts.len(),
        "retained_record_chain_verified":true,
        "retained_baseline_ref":baseline,
    });
    let catalog_provenance = json!({
        "selection":"event-derived-entry-v1",
        "projection_ref":format!("ToS/source-witnesses/catalog/{}", route.catalog_filename),
        "entry_sha256":format!("sha256:{}", digest(entry, l.catalog.max_output_row_bytes)?),
        "event_source_revision":expected_event_revision.to_hex(),
    });
    let source_provenance = json!({
        "source_ref":source_path,
        "record_bytes":current_raw.len(),
        "record_sha256":format!("sha256:{}",Digest256::of_bytes(current_raw).to_hex()),
        "archive_blob_ref":null,"archive_manifest_ref":null,
        "archive_manifest_sha256":null,"package_revision":null,
    });
    let current_provenance = json!({
        "verification_scope":"selected-record-chain",
        "all_package_bytes_verified":false,
        "catalog":catalog_provenance,
        "descriptor":route.descriptor(&current_record["schema_version"]),
        "history":history_provenance,
        "source":source_provenance,
        "transition":null,
    });

    let mut versions = Vec::with_capacity(refs.len());
    let mut version_sources = Vec::with_capacity(refs.len());
    for (
        reference,
        record,
        source,
        source_path,
        raw,
        version_schema_path,
        version_schema_bytes,
        transition,
    ) in retained
    {
        let provenance = json!({
            "verification_scope":"selected-record-chain",
            "all_package_bytes_verified":false,
            "catalog":catalog_provenance,
            "descriptor":route.descriptor(&current_record["schema_version"]),
            "history":history_provenance,
            "source":source,
            "transition":transition,
        });
        versions.push((
            reference.clone(),
            json!({
                "exact_ref":reference,
                "record_digest":reference["digest"],
                "status":"available",
                "reason":"exact-historical-version",
                "version_status":"historical",
                "record":record,
                "provenance":provenance,
                "grants_current_use":false,
                "performs_assessment":false,
                "writes_to_source":false,
            }),
        ));
        version_sources.push(SelectedMetadataVersionSourceV1 {
            source_path,
            source_record_bytes: raw,
            schema_path: version_schema_path,
            schema_bytes: version_schema_bytes,
        });
    }
    versions.push((
        current_ref.clone(),
        json!({
            "exact_ref":current_ref,
            "record_digest":current_ref["digest"],
            "status":"available",
            "reason":"exact-current-version",
            "version_status":"current",
            "record":current_record,
            "provenance":current_provenance,
            "grants_current_use":false,
            "performs_assessment":false,
            "writes_to_source":false,
        }),
    ));
    version_sources.push(SelectedMetadataVersionSourceV1 {
        source_path: source_path.clone(),
        source_record_bytes: current_raw,
        schema_path: schema_path.clone(),
        schema_bytes,
    });
    let current_resolution = &versions
        .last()
        .ok_or(Error::Invalid("selected metadata current version absent"))?
        .1;
    let mut history_view_provenance = current_provenance;
    history_view_provenance
        .as_object_mut()
        .ok_or(Error::Invalid("selected metadata history provenance"))?
        .remove("source");
    history_view_provenance
        .as_object_mut()
        .ok_or(Error::Invalid("selected metadata history provenance"))?
        .remove("transition");
    let history_view = json!({
        "status":"available",
        "reason":"verified-record-references",
        "record_id":entry["record_id"],
        "current_ref":current_ref,
        "refs":refs,
        "provenance":history_view_provenance,
        "grants_current_use":false,
        "performs_assessment":false,
        "writes_to_source":false,
    });
    if current_resolution["record"] != current_record {
        return Err(Error::Invalid("selected metadata current resolver binding"));
    }
    Ok(SelectedMetadataHistoryV1 {
        current_record,
        current_record_bytes: current_raw,
        source_path,
        schema_path,
        schema_bytes,
        history_path,
        history_bytes,
        history: history_view,
        versions,
        version_sources,
    })
}

/// Exact current and selected retained Claim version resolved from one
/// owner-bounded PG readset. The current entry is derived from the selected
/// stream row; retained versions are reconstructed from their exact archive
/// package bytes. Missing and stale refs are returned only after the complete
/// selected endpoint history has been verified.
pub struct SelectedClaimVersionV1<'a> {
    pub status: &'static str,
    pub reason: &'static str,
    pub version_status: Option<&'static str>,
    pub current_record: Value,
    pub current_ref: Value,
    pub refs: Vec<Value>,
    pub record: Option<Value>,
    pub provenance: Option<Value>,
    pub source_path: String,
    pub source_bytes: &'a [u8],
    pub history_path: String,
    pub history_bytes: Option<&'a [u8]>,
    pub schema_path: String,
    pub schema_bytes: &'a [u8],
    pub record_source_path: Option<String>,
    pub record_source_bytes: Option<Vec<u8>>,
    pub record_schema_path: Option<String>,
    pub record_schema_bytes: Option<&'a [u8]>,
    pub forms_path: String,
    pub forms_bytes: Option<&'a [u8]>,
}

const CLAIM_SCHEMA: &str = "ToS/contracts/source-claim-record.schema.json";
const HISTORICAL_CLAIM_SCHEMA: &str = "ToS/contracts/historical-claim.schema.json";
const HUMAN_FORMS_SCHEMA: &str = "ToS/contracts/human-form-set.schema.json";

fn selected_claim_schema_path(source_path: &str) -> Result<&'static str> {
    if selected_claim_stream_path(source_path) {
        Ok(CLAIM_SCHEMA)
    } else if historical_path(source_path) {
        Ok(HISTORICAL_CLAIM_SCHEMA)
    } else {
        Err(Error::Invalid("selected Claim endpoint source path"))
    }
}

fn selected_claim_stream_path(source_path: &str) -> bool {
    source_path.starts_with("ToS/source-witnesses/relations/")
        && source_path.ends_with("/source-claims.jsonl")
        && !source_path.contains("/owner-local/")
        && !source_path.contains("/private/")
        && !source_path.contains("/payload/")
}

fn selected_claim_archive_path(reference: &Value, revision: &str) -> Result<String> {
    exact_ref(reference, true)?;
    Ok(format!(
        "ToS/source-witnesses/.record-revisions/{}-{}",
        Digest256::of_bytes(text(reference, "id")?.as_bytes()).to_hex(),
        hash(revision)?
    ))
}

/// Select the exact manifests required by a Claim endpoint's retained history.
/// The current stream and explicit history presence/absence must already be
/// part of `files`; this helper never searches a corpus or interprets an
/// unselected path as absent.
pub fn selected_claim_archive_manifest_paths_v1(
    entry: &Value,
    files: &SelectedBibliographicFilesV1,
    l: BibliographicLimits,
) -> Result<Vec<String>> {
    l.validate()?;
    let source_path = text(entry, "source_claim_file_ref")?;
    let schema_path = selected_claim_schema_path(source_path)?;
    let schema_bytes = files.required(schema_path)?;
    let stream = files.required(source_path)?;
    let current_records = claim_records(stream, l)?;
    let id = text(entry, "claim_id")?;
    let current_record = current_records
        .get(id)
        .ok_or(Error::Invalid("selected Claim current row absent"))?;
    let current_ref = record_ref(current_record, true, l)?;
    let (line, _) = selected_claim_line_bytes(stream, id, l)?;
    let expected_entry = catalog::render_catalog_claim(
        current_record,
        source_path,
        line,
        Some(schema_path),
        l.catalog.max_output_row_bytes,
    )?;
    if entry["source_claim_line"].as_u64() != Some(line)
        || entry["claim_sha256"]
            != current_ref["digest"]
                .as_str()
                .unwrap_or("")
                .trim_start_matches("sha256:")
        || expected_entry != *entry
        || schema_bytes.is_empty()
    {
        return Err(Error::Invalid(
            "selected Claim catalog entry/source binding",
        ));
    }

    let history_path = adjacent(source_path, "claim-revision-history.json")?;
    let history_bytes = files.optional(&history_path)?;
    let history = if let Some(raw) = history_bytes {
        owned(raw, l.catalog.max_row_bytes)?
    } else {
        json!({"schema_version":"tos_claim_revision_history_v1","source_path":source_path,"receipts":[]})
    };
    exact_keys(&history, &["schema_version", "source_path", "receipts"])?;
    let receipts = array(&history, "receipts")?;
    if history["schema_version"] != "tos_claim_revision_history_v1"
        || history["source_path"] != source_path
        || receipts.len() > 128
        || receipts.is_empty()
            && current_records
                .values()
                .any(|record| record["claim_version"] != 1)
    {
        return Err(Error::Invalid("selected Claim history profile/bounds"));
    }
    let mut commands = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut head = None;
    for receipt in receipts {
        claim_receipt(receipt, &mut commands, l)?;
        let archive = selected_claim_archive_path(
            &receipt["previous_source"],
            text(receipt, "previous_revision")?,
        )?;
        if receipt["archive_path"] != archive {
            return Err(Error::Invalid("selected Claim archive source locator"));
        }
        let manifest_path = format!("{archive}/manifest.json");
        RelativePath::parse(&manifest_path)
            .map_err(|_| Error::Invalid("selected Claim archive manifest path"))?;
        if !paths.insert(manifest_path) {
            return Err(Error::Invalid("selected Claim archive manifest repeated"));
        }
        head = Some(receipt["source"].clone());
    }
    if head.as_ref().is_some_and(|head| head != &current_ref) {
        return Err(Error::Invalid("selected Claim history head differs"));
    }
    Ok(paths.into_iter().collect())
}

/// Select every blob named by the already selected Claim archive manifests.
/// Manifest paths are rechecked against the source-derived archive locator;
/// returned paths are sorted and unique for bounded PG selection.
pub fn selected_claim_archive_blob_paths_v1(
    manifest_paths: &[String],
    files: &SelectedBibliographicFilesV1,
    l: BibliographicLimits,
) -> Result<Vec<String>> {
    l.validate()?;
    if manifest_paths.len() > 128 || manifest_paths.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(Error::Invalid("selected Claim archive manifest path set"));
    }
    let mut paths = BTreeSet::new();
    for manifest_path in manifest_paths {
        let raw = files.required(manifest_path)?;
        let manifest = owned(raw, l.catalog.max_row_bytes)?;
        let schema = text(&manifest, "schema_version")?;
        let selected = schema == "tos_source_package_archive_v2";
        let mut keys = vec![
            "schema_version",
            "source_path",
            "source",
            "revision",
            "files",
        ];
        if selected {
            keys.push("publication_protocol");
        }
        exact_keys(&manifest, &keys)?;
        if !matches!(
            schema,
            "tos_source_package_archive_v1" | "tos_source_package_archive_v2"
        ) || selected && manifest["publication_protocol"] != "tos_selected_source_metadata_v1"
        {
            return Err(Error::Invalid("selected Claim archive manifest profile"));
        }
        let source_path = text(&manifest, "source_path")?;
        selected_claim_schema_path(source_path)?;
        let source = &manifest["source"];
        exact_ref(source, true)?;
        let revision = text(&manifest, "revision")?;
        let archive = selected_claim_archive_path(source, revision)?;
        if manifest_path != &format!("{archive}/manifest.json") {
            return Err(Error::Invalid("selected Claim archive manifest locator"));
        }
        let basename = source_path
            .rsplit('/')
            .next()
            .ok_or(Error::Invalid("selected Claim archive source basename"))?;
        let forms_name = format!("{}.human-forms.json", basename.trim_end_matches(".jsonl"));
        let selected_names = [basename, forms_name.as_str(), "claim-revision-history.json"];
        let bindings = manifest["files"]
            .as_object()
            .filter(|bindings| !bindings.is_empty() && bindings.len() <= 64)
            .ok_or(Error::Invalid("selected Claim archive file bindings"))?;
        for (name, binding) in bindings {
            exact_keys(binding, &["blob", "sha256", "bytes"])?;
            if name.is_empty()
                || name.contains(['/', '\\', '\0'])
                || matches!(
                    name.as_str(),
                    "." | ".."
                        | "payload"
                        | "owner-local"
                        | "private"
                        | "local-content"
                        | "catalog"
                )
                || name.starts_with('.') && !public_form_lock(name)
                || selected && !selected_names.contains(&name.as_str())
            {
                return Err(Error::Invalid("selected Claim archive member scope"));
            }
            let sha = hash(text(binding, "sha256")?)?;
            let blob = format!("{sha}.blob");
            if binding["blob"] != blob
                || binding["bytes"]
                    .as_u64()
                    .is_none_or(|bytes| bytes > 2 * 1024 * 1024)
            {
                return Err(Error::Invalid("selected Claim archive blob binding"));
            }
            let path = format!("{archive}/{blob}");
            RelativePath::parse(&path)
                .map_err(|_| Error::Invalid("selected Claim archive blob path"))?;
            paths.insert(path);
        }
    }
    Ok(paths.into_iter().collect())
}

/// Resolve one exact Claim version from current stream, history, and archive
/// bytes in the caller's PG-selected readset. The source path may be an
/// ordinary current Claim stream or an exact historical Claim endpoint; the
/// latter remains its own owner home throughout resolution.
pub fn resolve_selected_claim_version_v1<'a>(
    entry: &Value,
    exact: &Value,
    files: &'a SelectedBibliographicFilesV1,
    validator: &SourceCatalogValidator<'_>,
    expected_event_revision: Digest256,
    forms: &mut dyn BibliographicForms,
    l: BibliographicLimits,
) -> Result<SelectedClaimVersionV1<'a>> {
    l.validate()?;
    exact_ref(exact, true)?;
    if validator.selected_event_source_revision()? != expected_event_revision {
        return Err(Error::Invalid("selected Claim event revision differs"));
    }
    let source_path = text(entry, "source_claim_file_ref")?.to_owned();
    let schema_path = selected_claim_schema_path(&source_path)?.to_owned();
    let schema_bytes = files.required(&schema_path)?;
    let source_bytes = files.required(&source_path)?;
    let current_records = selected_claim_records(
        &source_path,
        source_bytes,
        &schema_path,
        schema_bytes,
        files,
        validator,
        expected_event_revision,
        l,
    )?;
    let id = text(entry, "claim_id")?;
    if text(exact, "id")? != id {
        return Err(Error::Invalid(
            "selected Claim ref and owner identity differ",
        ));
    }
    let current_record = current_records
        .get(id)
        .ok_or(Error::Invalid("selected Claim current membership absent"))?
        .clone();
    let current_ref = record_ref(&current_record, true, l)?;
    let (current_line, current_line_bytes) = selected_claim_line_bytes(source_bytes, id, l)?;
    let expected_entry = catalog::render_catalog_claim(
        &current_record,
        &source_path,
        current_line,
        Some(&schema_path),
        l.catalog.max_output_row_bytes,
    )?;
    if entry["source_claim_line"].as_u64() != Some(current_line)
        || entry["claim_sha256"]
            != current_ref["digest"]
                .as_str()
                .unwrap_or("")
                .trim_start_matches("sha256:")
        || expected_entry != *entry
    {
        return Err(Error::Invalid(
            "selected Claim entry differs from current source",
        ));
    }

    let history_path = adjacent(&source_path, "claim-revision-history.json")?;
    let history_bytes = files.optional(&history_path)?;
    let history = if let Some(raw) = history_bytes {
        owned(raw, l.catalog.max_row_bytes)?
    } else {
        json!({"schema_version":"tos_claim_revision_history_v1","source_path":source_path,"receipts":[]})
    };
    exact_keys(&history, &["schema_version", "source_path", "receipts"])?;
    let receipts = array(&history, "receipts")?;
    if history["schema_version"] != "tos_claim_revision_history_v1"
        || history["source_path"] != source_path
        || receipts.len() > 128
        || receipts.is_empty()
            && current_records
                .values()
                .any(|record| record["claim_version"] != 1)
    {
        return Err(Error::Invalid("selected Claim history profile/bounds"));
    }
    let mut commands = BTreeSet::new();
    let mut expected_stream: Option<Vec<u8>> = None;
    let mut historical = BTreeSet::new();
    let mut refs = Vec::new();
    let mut initial_stream = None;
    let mut chosen: Option<(Value, Value, Value, &'static str, String, Vec<u8>)> = if current_ref
        == *exact
    {
        Some((
            current_record.clone(),
            json!({"source_ref":source_path,"stream_sha256":format!("sha256:{}",Digest256::of_bytes(source_bytes).to_hex()),
                    "stream_bytes":source_bytes.len(),"package_revision":null,"archive_blob_ref":null,"line":current_line}),
            Value::Null,
            "current",
            source_path.clone(),
            current_line_bytes.to_vec(),
        ))
    } else {
        None
    };

    let manifest_paths = selected_claim_archive_manifest_paths_v1(entry, files, l)?;
    if manifest_paths.len() != receipts.len() {
        return Err(Error::Invalid(
            "selected Claim archive manifest set differs",
        ));
    }
    let mut receipt_manifest_paths = Vec::with_capacity(receipts.len());
    for receipt in receipts {
        check(validator, l)?;
        claim_receipt(receipt, &mut commands, l)?;
        let archive = selected_claim_archive_path(
            &receipt["previous_source"],
            text(receipt, "previous_revision")?,
        )?;
        if receipt["archive_path"] != archive {
            return Err(Error::Invalid("selected Claim archive locator differs"));
        }
        let manifest_path = format!("{archive}/manifest.json");
        receipt_manifest_paths.push(manifest_path.clone());
        let (archived, locations) =
            selected_claim_archive_package(&source_path, receipt, files, l)?;
        let archive_stream_location = locations
            .get(source_path.rsplit('/').next().unwrap())
            .ok_or(Error::Invalid(
                "selected Claim archive stream location absent",
            ))?;
        let before = archived
            .get(source_path.rsplit('/').next().unwrap())
            .ok_or(Error::Invalid("selected Claim archived stream absent"))?;
        let before_blob_path = text(archive_stream_location, "archive_path")?;
        let archived_records = selected_claim_records(
            before_blob_path,
            before,
            &schema_path,
            schema_bytes,
            files,
            validator,
            expected_event_revision,
            l,
        )?;
        if historical_path(&source_path) {
            if initial_stream.is_none() {
                initial_stream = Some(before.clone());
            }
            for name in HISTORICAL_CAPTURE {
                if archived.get(name).map(Vec::as_slice)
                    != files.optional(&format!(
                        "{}/{name}",
                        source_path.rsplit_once('/').unwrap().0
                    ))?
                {
                    return Err(Error::Invalid(
                        "historical Claim capture changed in archive",
                    ));
                }
            }
            let fields = receipt["request"]["fields"]
                .as_object()
                .ok_or(Error::Invalid(
                    "historical Claim retained correction fields",
                ))?;
            if fields.len() != 1
                || !fields.contains_key("qualifiers")
                || fields["qualifiers"].as_object().is_none_or(|values| {
                    values.keys().any(|key| {
                        ![
                            "statement",
                            "statement_language",
                            "statement_script",
                            "display_fields",
                        ]
                        .contains(&key.as_str())
                    })
                })
            {
                return Err(Error::Invalid("historical Claim correction scope"));
            }
        }
        let before_records = archived_records;
        if expected_stream
            .as_ref()
            .is_some_and(|expected| expected != before)
            || expected_stream.is_none()
                && before_records
                    .values()
                    .any(|record| record["claim_version"] != 1)
        {
            return Err(Error::Invalid(
                "selected Claim complete correction sequence",
            ));
        }
        let before_id = text(&receipt["previous_source"], "id")?;
        if before_id == id {
            refs.push(receipt["previous_source"].clone());
        }
        let previous = before_records
            .get(before_id)
            .ok_or(Error::Invalid("selected Claim predecessor absent"))?;
        if record_ref(previous, true, l)? != receipt["previous_source"]
            || !historical.insert(encode(
                &receipt["previous_source"],
                l.catalog.max_row_bytes,
            )?)
        {
            return Err(Error::Invalid(
                "selected Claim predecessor binding/uniqueness",
            ));
        }
        let revised = advance_claim(previous, &receipt["request"], l)?;
        if record_ref(&revised, true, l)? != receipt["source"] {
            return Err(Error::Invalid("selected Claim successor reconstruction"));
        }
        let formname = claim_form_name(&source_path, before_id)?;
        let current_form_path = adjacent(&source_path, &formname)?;
        let current_form_raw = files.optional(&current_form_path)?.ok_or(Error::Invalid(
            "selected corrected Claim current forms absent",
        ))?;
        let form_schema_bytes = files.required(HUMAN_FORMS_SCHEMA)?;
        let current_forms = selected_claim_form(
            &current_form_path,
            current_form_raw,
            form_schema_bytes,
            validator,
            expected_event_revision,
            l,
        )?;
        let current_source = current_records.get(before_id).ok_or(Error::Invalid(
            "selected corrected Claim sibling absent from current stream",
        ))?;
        let current_materializations =
            forms.materialize(current_source, &current_forms, 262_144)?;
        encode(&json!(current_materializations), 262_144)?;

        let prior_raw = archived.get(&formname);
        let prior_form_path = prior_raw.and_then(|_| {
            locations
                .get(&formname)
                .and_then(|location| location["archive_path"].as_str())
        });
        let prior_forms = prior_raw
            .map(|raw| {
                selected_claim_form(
                    prior_form_path.ok_or(Error::Invalid("selected prior Claim forms location"))?,
                    raw,
                    form_schema_bytes,
                    validator,
                    expected_event_revision,
                    l,
                )
            })
            .transpose()?;
        reconstruct_forms_with(
            &revised,
            prior_forms.as_ref(),
            &current_forms,
            receipt,
            validator,
            forms,
            l,
            |raw| {
                let instance = format!(
                    "catalog-generated-instance/{}",
                    Digest256::of_bytes(raw).to_hex()
                );
                validator.check_selected_event_document_exact(
                    expected_event_revision,
                    &instance,
                    HUMAN_FORMS_SCHEMA,
                    form_schema_bytes,
                    raw,
                )
            },
        )?;

        if receipt["previous_source"] == *exact {
            if chosen.is_some() {
                return Err(Error::Invalid("selected Claim exact version duplicated"));
            }
            let (line, line_bytes) = selected_claim_line_bytes(before, before_id, l)?;
            let source = json!({"source_ref":source_path,"stream_sha256":format!("sha256:{}",Digest256::of_bytes(before).to_hex()),
                "stream_bytes":before.len(),"package_revision":receipt["previous_revision"],"archive_blob_ref":before_blob_path,"line":line});
            chosen = Some((
                previous.clone(),
                source,
                receipt_transition(receipt)?,
                "historical",
                before_blob_path.to_owned(),
                line_bytes.to_vec(),
            ));
        }
        expected_stream = Some(replace_claim(before, &revised, l)?);
    }
    if receipt_manifest_paths.iter().collect::<BTreeSet<_>>()
        != manifest_paths.iter().collect::<BTreeSet<_>>()
        || receipt_manifest_paths.len() != manifest_paths.len()
    {
        return Err(Error::Invalid(
            "selected Claim archive manifest order differs",
        ));
    }
    if expected_stream
        .as_ref()
        .is_some_and(|expected| expected.as_slice() != source_bytes)
    {
        return Err(Error::Invalid(
            "selected Claim current stream is not history head",
        ));
    }
    let captured = historical_path(&source_path);
    if captured {
        let prefix = source_path.rsplit_once('/').unwrap().0;
        let package = files.package(
            prefix,
            64.min(files.observed_path_count()),
            files.observed_bytes().max(1).min(8 * 1024 * 1024),
        )?;
        let package = package
            .into_iter()
            .map(|(name, raw)| (name, raw.to_vec()))
            .collect::<BTreeMap<_, _>>();
        for name in HISTORICAL_CAPTURE {
            let path = format!("{prefix}/{name}");
            let _ = files.optional(&path)?;
        }
        let capture_receipt = package.get("source-create-receipt.json");
        if capture_receipt.is_some() {
            let request = owned(
                package
                    .get("source-create-request.json")
                    .ok_or(Error::Invalid(
                        "selected historical Claim origin request absent",
                    ))?,
                l.catalog.max_row_bytes,
            )?;
            for original in array(&request, "claims")? {
                let formpath = adjacent(
                    &source_path,
                    &claim_form_name(&source_path, text(original, "claim_id")?)?,
                )?;
                let _ = files.optional(&formpath)?;
                let lockpath = adjacent(
                    &source_path,
                    &format!(
                        ".{}.writer.lock",
                        claim_form_name(&source_path, text(original, "claim_id")?)?
                    ),
                )?;
                let _ = files.optional(&lockpath)?;
            }
        }
        let human_schema = files.required(HUMAN_FORMS_SCHEMA)?;
        verify_historical_capture_with(
            &source_path,
            &package,
            initial_stream.as_deref().unwrap_or(source_bytes),
            &current_records,
            receipts,
            validator,
            forms,
            l,
            |path, raw| {
                validator.check_selected_event_document_exact(
                    expected_event_revision,
                    path,
                    HUMAN_FORMS_SCHEMA,
                    human_schema,
                    raw,
                )
            },
        )?;
    }

    refs.push(current_ref.clone());
    let same_version = refs
        .iter()
        .any(|reference| reference["version"] == exact["version"]);
    let Some((record, source, transition, version_status, record_source_path, record_source_bytes)) =
        chosen
    else {
        let forms_path = adjacent(&source_path, &claim_form_name(&source_path, id)?)?;
        let forms_bytes = files.optional(&forms_path)?;
        return Ok(SelectedClaimVersionV1 {
            status: if same_version { "stale" } else { "missing" },
            reason: if same_version {
                "exact-version-digest-mismatch"
            } else {
                "exact-version-not-retained"
            },
            version_status: None,
            current_record,
            current_ref,
            refs,
            record: None,
            provenance: None,
            source_path,
            source_bytes,
            history_path,
            history_bytes,
            schema_path,
            schema_bytes,
            record_source_path: None,
            record_source_bytes: None,
            record_schema_path: None,
            record_schema_bytes: None,
            forms_path,
            forms_bytes,
        });
    };
    let mut provenance = json!({
        "verification_scope":"selected-claim-chain",
        "all_package_bytes_verified":false,
        "catalog":{
            "selection":"event-derived-entry-v1",
            "projection_ref":"ToS/source-witnesses/catalog/claims.jsonl",
            "entry_sha256":format!("sha256:{}",digest(entry,l.catalog.max_output_row_bytes)?),
            "event_source_revision":expected_event_revision.to_hex(),
        },
        "source":source,
        "history":{
            "source_ref":history_bytes.map(|_|history_path.clone()),
            "sha256":history_bytes.map(|raw|format!("sha256:{}",Digest256::of_bytes(raw).to_hex())),
            "receipt_count":receipts.len(),
            "correction_chain_verified":true,
        },
        "transition":transition,
    });
    if captured {
        provenance["history"]["adapter"] = json!("captured-historical-claim-v1");
        provenance["history"]["record_history_verified"] = json!(false);
    }
    let forms_path = adjacent(&source_path, &claim_form_name(&source_path, id)?)?;
    let forms_bytes = files.optional(&forms_path)?;
    Ok(SelectedClaimVersionV1 {
        status: "available",
        reason: if version_status == "current" {
            "exact-current-version"
        } else {
            "exact-historical-version"
        },
        version_status: Some(version_status),
        current_record,
        current_ref,
        refs,
        record: Some(record),
        provenance: Some(provenance),
        source_path,
        source_bytes,
        history_path,
        history_bytes,
        schema_path: schema_path.clone(),
        schema_bytes,
        record_source_path: Some(record_source_path),
        record_source_bytes: Some(record_source_bytes),
        record_schema_path: Some(schema_path),
        record_schema_bytes: Some(schema_bytes),
        forms_path,
        forms_bytes,
    })
}

fn selected_claim_records(
    source_path: &str,
    raw: &[u8],
    schema_path: &str,
    schema_bytes: &[u8],
    files: &SelectedBibliographicFilesV1,
    validator: &SourceCatalogValidator<'_>,
    event_revision: Digest256,
    l: BibliographicLimits,
) -> Result<BTreeMap<String, Value>> {
    let records = claim_records(raw, l)?;
    for physical in raw.split(|byte| *byte == b'\n') {
        if physical.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let record = owned(physical, l.catalog.max_row_bytes)?;
        validator.check_selected_event_document_exact(
            event_revision,
            source_path,
            schema_path,
            schema_bytes,
            physical,
        )?;
        if historical_path(source_path) {
            selected_historical_claim_shape(
                source_path,
                &record,
                files,
                validator,
                event_revision,
                l,
            )?;
        }
    }
    Ok(records)
}

fn selected_historical_claim_shape(
    source_path: &str,
    record: &Value,
    files: &SelectedBibliographicFilesV1,
    validator: &SourceCatalogValidator<'_>,
    event_revision: Digest256,
    l: BibliographicLimits,
) -> Result<()> {
    historical_claim_shape_with(record, l, |schema_path, fragment, raw| {
        let schema_bytes = files.required(schema_path)?;
        if fragment.is_empty() {
            validator.check_selected_event_document_exact(
                event_revision,
                source_path,
                schema_path,
                schema_bytes,
                raw,
            )
        } else {
            validator.check_selected_event_document_fragment_exact(
                event_revision,
                source_path,
                schema_path,
                schema_bytes,
                fragment,
                raw,
            )
        }
    })
}

fn selected_claim_line_bytes<'a>(
    raw: &'a [u8],
    id: &str,
    l: BibliographicLimits,
) -> Result<(u64, &'a [u8])> {
    for (index, bytes) in raw.split(|byte| *byte == b'\n').enumerate() {
        if bytes.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        if SourceRow::parse(bytes, l.catalog.max_row_bytes)?.value()["claim_id"] == id {
            return Ok((index as u64 + 1, bytes));
        }
    }
    Err(Error::Invalid("selected Claim source line absent"))
}

fn selected_claim_form(
    path: &str,
    raw: &[u8],
    schema_bytes: &[u8],
    validator: &SourceCatalogValidator<'_>,
    event_revision: Digest256,
    l: BibliographicLimits,
) -> Result<Value> {
    validator.check_selected_event_document_exact(
        event_revision,
        path,
        HUMAN_FORMS_SCHEMA,
        schema_bytes,
        raw,
    )?;
    owned(raw, l.catalog.max_row_bytes)
}

fn selected_claim_archive_manifest(
    manifest_path: &str,
    manifest_raw: &[u8],
    l: BibliographicLimits,
) -> Result<Value> {
    let manifest = owned(manifest_raw, l.catalog.max_row_bytes)?;
    let schema = text(&manifest, "schema_version")?;
    let selected = schema == "tos_source_package_archive_v2";
    let mut keys = vec![
        "schema_version",
        "source_path",
        "source",
        "revision",
        "files",
    ];
    if selected {
        keys.push("publication_protocol");
    }
    exact_keys(&manifest, &keys)?;
    if !matches!(
        schema,
        "tos_source_package_archive_v1" | "tos_source_package_archive_v2"
    ) || selected && manifest["publication_protocol"] != "tos_selected_source_metadata_v1"
    {
        return Err(Error::Invalid("selected Claim archive manifest profile"));
    }
    let source_path = text(&manifest, "source_path")?;
    selected_claim_schema_path(source_path)?;
    let source = &manifest["source"];
    exact_ref(source, true)?;
    let revision = text(&manifest, "revision")?;
    let archive = selected_claim_archive_path(source, revision)?;
    if manifest_path != format!("{archive}/manifest.json") {
        return Err(Error::Invalid("selected Claim archive manifest locator"));
    }
    let bindings = manifest["files"]
        .as_object()
        .filter(|bindings| !bindings.is_empty() && bindings.len() <= 64)
        .ok_or(Error::Invalid("selected Claim archive file bindings"))?;
    for (name, binding) in bindings {
        exact_keys(binding, &["blob", "sha256", "bytes"])?;
        if name.is_empty()
            || name.contains(['/', '\\', '\0'])
            || matches!(
                name.as_str(),
                "." | ".." | "payload" | "owner-local" | "private" | "local-content" | "catalog"
            )
            || name.starts_with('.') && !public_form_lock(name)
        {
            return Err(Error::Invalid("selected Claim archive member scope"));
        }
        let sha = hash(text(binding, "sha256")?)?;
        if binding["blob"] != format!("{sha}.blob")
            || binding["bytes"]
                .as_u64()
                .is_none_or(|bytes| bytes > 2 * 1024 * 1024)
        {
            return Err(Error::Invalid("selected Claim archive blob binding"));
        }
    }
    Ok(manifest)
}

fn selected_claim_archive_package(
    reference: &str,
    receipt: &Value,
    selected: &SelectedBibliographicFilesV1,
    l: BibliographicLimits,
) -> Result<(BTreeMap<String, Vec<u8>>, BTreeMap<String, Value>)> {
    let archive = selected_claim_archive_path(
        &receipt["previous_source"],
        text(receipt, "previous_revision")?,
    )?;
    if text(receipt, "archive_path")? != archive {
        return Err(Error::Invalid(
            "selected Claim archive source-derived locator",
        ));
    }
    let package = selected.package(
        &archive,
        65.min(selected.observed_path_count()),
        selected.observed_bytes().max(1).min(10 * 1024 * 1024),
    )?;
    if package.is_empty() || package.len() > 65 {
        return Err(Error::Budget("selected Claim archive package members"));
    }
    let mut blobs = BTreeMap::new();
    let mut total = 0usize;
    for (name, raw) in package {
        if name != "manifest.json"
            && (name.len() != 69
                || !name.ends_with(".blob")
                || !name[..64]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        {
            return Err(Error::Invalid("selected Claim archive non-blob member"));
        }
        if raw.len() > 2 * 1024 * 1024 {
            return Err(Error::Budget("selected Claim archive member bytes"));
        }
        total = total
            .checked_add(raw.len())
            .filter(|bytes| *bytes <= 10 * 1024 * 1024)
            .ok_or(Error::Budget("selected Claim archive bytes"))?;
        blobs.insert(name.to_owned(), raw.to_vec());
    }
    let manifest_raw = blobs
        .remove("manifest.json")
        .ok_or(Error::Invalid("selected Claim archive manifest absent"))?;
    let manifest_path = format!("{archive}/manifest.json");
    let manifest = selected_claim_archive_manifest(&manifest_path, &manifest_raw, l)?;
    let basename = reference
        .rsplit('/')
        .next()
        .ok_or(Error::Invalid("selected Claim archive source basename"))?;
    let selected_manifest = manifest["schema_version"] == "tos_source_package_archive_v2";
    let mut keys = vec![
        "schema_version",
        "source_path",
        "source",
        "revision",
        "files",
    ];
    if selected_manifest {
        keys.push("publication_protocol");
    }
    exact_keys(&manifest, &keys)?;
    if manifest["source_path"] != reference
        || manifest["source"] != receipt["previous_source"]
        || manifest["revision"] != receipt["previous_revision"]
    {
        return Err(Error::Invalid(
            "selected Claim archive exact manifest binding",
        ));
    }
    let forms_name = format!("{}.human-forms.json", basename.trim_end_matches(".jsonl"));
    let selected_names = [basename, forms_name.as_str(), "claim-revision-history.json"];
    let bindings = manifest["files"]
        .as_object()
        .filter(|bindings| !bindings.is_empty() && bindings.len() <= 64)
        .ok_or(Error::Invalid("selected Claim archive file bindings"))?;
    let mut restored = BTreeMap::new();
    let mut locations = BTreeMap::new();
    let mut used = BTreeSet::new();
    for (name, binding) in bindings {
        exact_keys(binding, &["blob", "sha256", "bytes"])?;
        if name.is_empty()
            || name.contains(['/', '\\', '\0'])
            || matches!(
                name.as_str(),
                "." | ".." | "payload" | "owner-local" | "private" | "local-content" | "catalog"
            )
            || name.starts_with('.') && !public_form_lock(name)
            || selected_manifest && !selected_names.contains(&name.as_str())
        {
            return Err(Error::Invalid("selected Claim archive public member scope"));
        }
        let sha = hash(text(binding, "sha256")?)?;
        let blob = format!("{sha}.blob");
        if binding["blob"] != blob {
            return Err(Error::Invalid("selected Claim archive blob binding"));
        }
        let raw = blobs
            .get(&blob)
            .ok_or(Error::Invalid("selected Claim archive blob absent"))?;
        if raw.len() as u64 != binding["bytes"].as_u64().unwrap_or(u64::MAX)
            || format!("sha256:{}", Digest256::of_bytes(raw).to_hex()) != text(binding, "sha256")?
        {
            return Err(Error::Invalid("selected Claim archive raw byte binding"));
        }
        if public_form_lock(name) && !raw.is_empty() {
            return Err(Error::Invalid(
                "selected Claim archive writer lock contents",
            ));
        }
        let blob_path = format!("{archive}/{blob}");
        restored.insert(name.clone(), raw.clone());
        used.insert(blob.clone());
        locations.insert(
            name.clone(),
            json!({"archive_path":blob_path,"sha256":binding["sha256"],"bytes":binding["bytes"]}),
        );
    }
    if used != blobs.keys().cloned().collect::<BTreeSet<_>>()
        || package_revision(&restored, l)? != text(receipt, "previous_revision")?
    {
        return Err(Error::Invalid(
            "selected Claim archive complete package binding",
        ));
    }
    Ok((restored, locations))
}

fn verify_entities(
    stage: &KnowledgeStage<'_>,
    entities: &Value,
    l: BibliographicLimits,
) -> Result<()> {
    let registry = stage
        .raw_by_id(
            catalog::CATALOG_SOURCE,
            catalog::CONTRACT_FILES,
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        )?
        .ok_or(Error::Invalid(
            "metadata version sealed entity registry absent",
        ))?;
    if owned(
        &registry.payload,
        l.catalog.max_contract_bytes.min(l.catalog.max_row_bytes),
    )? != *entities
    {
        return Err(Error::Invalid(
            "metadata version supplied entity registry differs from sealed source",
        ));
    }
    Ok(())
}
/// Retained JSON uses the same exact scalar transport as current catalog rows.
/// Compare owner canonical bytes before any digest-based lineage reconstruction.
fn owned(raw: &[u8], cap: usize) -> Result<Value> {
    let value = SourceRow::parse(raw, cap)?.value().clone();
    let original = canonical_raw_bytes_v1(
        raw,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::new(cap, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("bibliographic retained scalar limits"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    if original != encode(&value, cap)? {
        return Err(Error::Invalid(
            "bibliographic retained exact scalar transport",
        ));
    }
    Ok(value)
}
fn check(validator: &SourceCatalogValidator<'_>, l: BibliographicLimits) -> Result<()> {
    if std::time::Instant::now() >= l.deadline
        || validator
            .cancelled
            .load(std::sync::atomic::Ordering::Relaxed)
    {
        Err(Error::Budget(
            "bibliographic selected version deadline/cancel",
        ))
    } else {
        Ok(())
    }
}
fn adjacent(reference: &str, name: &str) -> Result<String> {
    Ok(format!(
        "{}/{}",
        reference
            .rsplit_once('/')
            .ok_or(Error::Invalid("bibliographic version source locator"))?
            .0,
        name
    ))
}
fn exact_ref(reference: &Value, claim: bool) -> Result<()> {
    let value = reference
        .as_object()
        .ok_or(Error::Invalid("bibliographic exact version reference"))?;
    let id = text(reference, "id")?;
    let sha = text(reference, "digest")?
        .strip_prefix("sha256:")
        .ok_or(Error::Invalid("bibliographic exact version digest"))?;
    let parts = id
        .strip_prefix("tos.")
        .unwrap_or("")
        .splitn(2, '.')
        .collect::<Vec<_>>();
    let identity_valid = parts.len() == 2
        && !parts[0].is_empty()
        && parts[0]
            .bytes()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase())
        && parts[0]
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        && (parts[0] == "claim") == claim
        && !parts[1].is_empty()
        && parts[1].split(['.', '-']).all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
    if value.len() != 3
        || !identity_valid
        || reference["version"]
            .as_u64()
            .is_none_or(|n| !(1..=9_007_199_254_740_991).contains(&n))
        || sha.len() != 64
        || !sha
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(Error::Invalid(
            "bibliographic exact version reference grammar",
        ));
    }
    Ok(())
}
fn record_ref(record: &Value, claim: bool, l: BibliographicLimits) -> Result<Value> {
    let reference = if claim {
        json!({"id":record["claim_id"],"version":record["claim_version"],
        "digest":format!("sha256:{}",digest(record,l.catalog.max_row_bytes)?)})
    } else {
        json!({"id":record["record_id"],"version":record["record_version"],
        "digest":format!("sha256:{}",digest(record,l.catalog.max_row_bytes)?)})
    };
    exact_ref(&reference, claim)?;
    Ok(reference)
}
enum CatalogLocation {
    Legacy(u64, String),
    Epoch(Value),
}
impl CatalogLocation {
    fn render(&self, mut context: Value) -> Value {
        let object = context
            .as_object_mut()
            .expect("internal catalog provenance object");
        match self {
            Self::Legacy(line, sha) => {
                object.insert("line".into(), json!(line));
                object.insert("sha256".into(), json!(sha));
            }
            Self::Epoch(epoch) => {
                object.extend(
                    epoch
                        .as_object()
                        .expect("constructed V2 provenance object")
                        .clone(),
                );
            }
        }
        context
    }
}

struct LegacyCatalogLocations {
    lines: BTreeMap<String, u64>,
    sha256: String,
    retained_bytes: u64,
}
impl LegacyCatalogLocations {
    fn locate(&self, id: &str) -> Result<CatalogLocation> {
        let line = self
            .lines
            .get(id)
            .ok_or(Error::Invalid("bibliographic exact legacy catalog address"))?;
        Ok(CatalogLocation::Legacy(*line, self.sha256.clone()))
    }
}

fn legacy_catalog_locations(
    mut next: impl FnMut() -> Result<Option<(String, Value)>>,
    kind: Option<&str>,
    max_retained_bytes: u64,
    l: BibliographicLimits,
) -> Result<LegacyCatalogLocations> {
    let mut hash = Digest256Hasher::new();
    let mut lines = BTreeMap::new();
    let mut line = 0u64;
    // Includes the outer map node, bounded category/family key, map roots and
    // digest. Per-key charge conservatively covers sparse BTreeMap nodes.
    let mut retained_bytes = 4096u64;
    if retained_bytes > max_retained_bytes {
        return Err(Error::Budget("bibliographic catalog location state"));
    }
    while let Some((key, row)) = next()? {
        if std::time::Instant::now() >= l.deadline {
            return Err(Error::Budget("bibliographic exact catalog render deadline"));
        }
        if kind.is_none_or(|kind| row["entry"]["record_type"] == kind) {
            retained_bytes = retained_bytes
                .checked_add(key.capacity() as u64)
                .and_then(|bytes| bytes.checked_add(512))
                .filter(|bytes| *bytes <= max_retained_bytes)
                .ok_or(Error::Budget("bibliographic catalog location state"))?;
            line = line
                .checked_add(1)
                .filter(|count| *count <= l.catalog.max_rows)
                .ok_or(Error::Budget("bibliographic catalog location rows"))?;
            hash.update(&encode(&row["entry"], l.catalog.max_output_row_bytes)?);
            hash.update(b"\n");
            if lines.insert(key, line).is_some() {
                return Err(Error::Invalid("bibliographic duplicate catalog address"));
            }
        }
    }
    Ok(LegacyCatalogLocations {
        lines,
        sha256: format!("sha256:{}", hash.finalize().to_hex()),
        retained_bytes,
    })
}

fn bind_files(provenance: &Value, inputs: &mut BTreeMap<String, String>) -> Result<()> {
    let mut bind = |reference: &Value, sha: &Value| -> Result<()> {
        if reference.is_null() || sha.is_null() {
            return Ok(());
        }
        let reference = reference
            .as_str()
            .ok_or(Error::Invalid("bibliographic version provenance ref"))?;
        let sha = sha
            .as_str()
            .ok_or(Error::Invalid("bibliographic version provenance digest"))?
            .strip_prefix("sha256:")
            .ok_or(Error::Invalid("bibliographic version digest prefix"))?;
        if let Some(previous) = inputs.insert(reference.into(), sha.into()) {
            if previous != sha {
                return Err(Error::Invalid(
                    "bibliographic conflicting exact version input",
                ));
            }
        }
        Ok(())
    };
    for section in ["catalog", "history"] {
        bind(
            &provenance[section]["source_ref"],
            &provenance[section]["sha256"],
        )?;
    }
    let source = &provenance["source"];
    let location = source
        .get("archive_blob_ref")
        .filter(|v| !v.is_null())
        .unwrap_or(&source["source_ref"]);
    let sha = source
        .get("record_sha256")
        .filter(|v| !v.is_null())
        .unwrap_or(&source["stream_sha256"]);
    bind(location, sha)?;
    bind(
        &source["archive_manifest_ref"],
        &source["archive_manifest_sha256"],
    )
}

fn exact_keys(value: &Value, expected: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or(Error::Invalid("bibliographic exact retained object"))?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(Error::Invalid("bibliographic exact retained object fields"));
    }
    Ok(())
}

fn hash(value: &str) -> Result<&str> {
    let sha = value
        .strip_prefix("sha256:")
        .ok_or(Error::Invalid("bibliographic retained digest prefix"))?;
    if sha.len() != 64
        || !sha
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(Error::Invalid("bibliographic retained digest grammar"));
    }
    Ok(sha)
}
fn receipt_transition(receipt: &Value) -> Result<Value> {
    let mut transition = serde_json::Map::new();
    for key in [
        "command_id",
        "recorded_at",
        "previous_source",
        "source",
        "request_digest",
    ] {
        transition.insert(
            key.into(),
            receipt
                .get(key)
                .ok_or(Error::Invalid("bibliographic retained transition field"))?
                .clone(),
        );
    }
    Ok(Value::Object(transition))
}
fn metadata_receipt(
    receipt: &Value,
    history: &Value,
    id: &Value,
    head: Option<&Value>,
    commands: &mut BTreeSet<String>,
    route: &RecordRoute,
    l: BibliographicLimits,
) -> Result<()> {
    let selected = receipt.get("publication").is_some();
    let mut keys = vec![
        "command_id",
        "request_digest",
        "principal_id",
        "authority_ref",
        "owner_configuration",
        "recorded_at",
        "reason",
        "previous_source",
        "source",
        "previous_revision",
        "archive_path",
        "dependencies",
        "changed_fields",
        "forms",
        "grants_admission",
        "request",
    ];
    if selected {
        keys.push("publication");
    }
    exact_keys(receipt, &keys)?;
    let request = &receipt["request"];
    let operation = text(request, "operation")?;
    let compound = compound_profile(operation);
    if let Some(profile) = compound {
        let mut request_keys = vec![
            "schema_version",
            "operation",
            "command_id",
            "fields",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
            "expected_publication",
        ];
        request_keys.extend_from_slice(profile.extra);
        exact_keys(request, &request_keys)?;
        if request["schema_version"] != profile.schema
            || route.adapter != "native-corpus"
            || route.kind != profile.parent
            || !selected
        {
            return Err(Error::Invalid(
                "bibliographic compound retained metadata parent profile",
            ));
        }
        encode(request, 1_048_576)?;
        for key in [
            "expected_configuration",
            "expected_revision",
            "expected_dependencies",
        ] {
            hash(text(request, key)?)?;
        }
        if !request["expected_publication"].is_null() {
            hash(text(request, "expected_publication")?)?;
        }
        let command = text(request, "command_id")?;
        let reason = text(request, "reason")?;
        if !(1..=256).contains(&command.chars().count())
            || !(1..=4096).contains(&reason.trim().chars().count())
            || request["claim"]["predicate"] != profile.predicate
            || request["claim"]["subject_ref"] != *id
            || request["claim"]["object"] != request[profile.child]["record_id"]
        {
            return Err(Error::Invalid(
                "bibliographic compound retained exact endpoints",
            ));
        }
        exact_keys(&request["fields"], &[profile.field])?;
        if array(&request["fields"], profile.field)?.last() != Some(&request["claim"]["claim_id"])
            || operation == "work.expression.create" && request["record"]["work_ref"] != *id
            || operation == "expression.edition.create"
                && request["record"]["embodies_expression_refs"] != json!([id])
        {
            return Err(Error::Invalid(
                "bibliographic compound retained parent field append",
            ));
        }
        if operation == "item.adopt" {
            let at = text(request, "fixity_verified_at")?;
            tos_validation::retirement_rules::observed_instant_order(at, at)
                .map_err(|_| Error::Invalid("bibliographic retained Item fixity instant"))?;
            if request["inventory"].is_null() != !request["inventory_limitation"].is_null() {
                return Err(Error::Invalid(
                    "bibliographic retained Item inventory completeness",
                ));
            }
        }
        let transaction = json!({"operation":operation,"command_id":request["command_id"],"owner_configuration":request["expected_configuration"],"request_digest":format!("sha256:{}",digest(request,l.catalog.max_row_bytes)?)});
        if receipt["publication"]["transaction_id"]
            != format!("sha256:{}", digest(&transaction, l.catalog.max_row_bytes)?)
        {
            return Err(Error::Invalid(
                "bibliographic compound retained transaction binding",
            ));
        }
    } else {
        let mut request_keys = vec![
            "schema_version",
            "operation",
            "fields",
            "forms",
            "reason",
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
        ];
        if selected {
            request_keys.push("expected_publication");
        }
        exact_keys(request, &request_keys)?;
        if request["schema_version"] != "tos_local_source_command_v1"
            || request["operation"] != "record.revise"
        {
            return Err(Error::Invalid(
                "bibliographic Collection retained correction request",
            ));
        }
    }
    if selected {
        exact_keys(
            &receipt["publication"],
            &["protocol", "transaction_id", "selected_files"],
        )?;
        let mut selected_files = vec![
            route.basename.clone(),
            format!(
                "{}.human-forms.json",
                route.basename.trim_end_matches(".json")
            ),
            "source-revision-history.json".into(),
        ];
        selected_files.sort();
        if history["schema_version"] != "tos_source_revision_history_v2"
            || receipt["publication"]["protocol"] != "tos_selected_source_metadata_v1"
            || receipt["publication"]["selected_files"] != json!(selected_files)
            || !receipt["publication"]["transaction_id"].is_string()
        {
            return Err(Error::Invalid(
                "bibliographic selected metadata retained publication binding",
            ));
        }
    }
    let instant = text(receipt, "recorded_at")?;
    tos_validation::retirement_rules::observed_instant_order(instant, instant)
        .map_err(|_| Error::Invalid("bibliographic retained correction aware instant"))?;
    exact_ref(&receipt["previous_source"], false)?;
    exact_ref(&receipt["source"], false)?;
    let fields = request["fields"]
        .as_object()
        .ok_or(Error::Invalid("bibliographic retained correction fields"))?;
    let mut changed = fields
        .keys()
        .map(|key| Value::String(key.clone()))
        .collect::<Vec<_>>();
    changed.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    if !commands.insert(text(receipt, "command_id")?.to_owned())
        || receipt["request_digest"]
            != format!("sha256:{}", digest(request, l.catalog.max_row_bytes)?)
        || receipt["changed_fields"] != Value::Array(changed)
        || receipt["grants_admission"] != false
        || receipt["source"]["id"] != *id
        || receipt["previous_source"]["id"] != *id
        || receipt["source"]["version"].as_u64()
            != receipt["previous_source"]["version"]
                .as_u64()
                .and_then(|n| n.checked_add(1))
        || head.is_some_and(|head| receipt["previous_source"] != *head)
    {
        return Err(Error::Invalid(
            "bibliographic retained Collection continuous lineage",
        ));
    }
    for (left, right) in [
        ("command_id", "command_id"),
        ("previous_source", "expected_source"),
        ("previous_revision", "expected_revision"),
        ("owner_configuration", "expected_configuration"),
        ("dependencies", "expected_dependencies"),
        ("reason", "reason"),
    ] {
        if receipt[left] != request[right] {
            return Err(Error::Invalid(
                "bibliographic retained correction exact request binding",
            ));
        }
    }
    Ok(())
}

fn package_revision(files: &BTreeMap<String, Vec<u8>>, l: BibliographicLimits) -> Result<String> {
    let bindings = files
        .iter()
        .map(|(name, raw)| {
            (
                name.clone(),
                json!({
        "sha256":format!("sha256:{}",Digest256::of_bytes(raw).to_hex()),"bytes":raw.len()}),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    Ok(format!(
        "sha256:{}",
        digest(&Value::Object(bindings), l.max_claim_cohort_bytes)?
    ))
}
const HISTORICAL_CAPTURE: [&str; 4] = [
    "source-create-request.json",
    "source-create-receipt.json",
    "source-create-environment.json",
    "source-create-provenance.jsonl",
];
fn historical_path(reference: &str) -> bool {
    let p = reference.split('/').collect::<Vec<_>>();
    p.len() >= 5
        && p[..3] == ["ToS", "source-witnesses", "history"]
        && p.last() == Some(&"historical-claims.jsonl")
        && p[3..p.len() - 1].iter().all(|part| {
            !part.is_empty()
                && part.split(['.', '-']).all(|x| {
                    !x.is_empty()
                        && x.bytes()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                })
        })
}
fn historical_claim_shape(
    contracts: &catalog::Contracts,
    record: &Value,
    validator: &SourceCatalogValidator<'_>,
    l: BibliographicLimits,
) -> Result<()> {
    historical_claim_shape_with(record, l, |schema, fragment, raw| {
        catalog::check_catalog_schema_with_contracts(contracts, validator, schema, fragment, raw)
    })
}

fn historical_claim_shape_with(
    record: &Value,
    l: BibliographicLimits,
    mut check_schema: impl FnMut(&str, &str, &[u8]) -> Result<()>,
) -> Result<()> {
    check_schema(
        "ToS/contracts/historical-claim.schema.json",
        "",
        &encode(record, l.catalog.max_row_bytes)?,
    )?;
    let qualifiers = &record["qualifiers"];
    if qualifiers["display_fields"]["schema_version"] == "tos_claim_display_fields_v1" {
        check_schema(
            "ToS/contracts/claim-display-fields.schema.json",
            "",
            &encode(qualifiers, l.catalog.max_row_bytes)?,
        )?;
    }
    if qualifiers["statement"].as_str().is_some_and(|s| {
        !s.trim_matches(crate::source_bibliographic_unicode::is_python_whitespace)
            .is_empty()
    }) {
        let languages = json!({"notes":{"language":qualifiers["statement_language"],"script":qualifiers["statement_script"]}});
        check_schema(
            "ToS/contracts/corpus-record.schema.json",
            "#/properties/field_languages",
            &encode(&languages, l.catalog.max_row_bytes)?,
        )?;
    }
    Ok(())
}
/// Captured Claim-only origin, matching verify_claim_capture/creation_lineage.
/// Record revision history and current correction authority remain separate.
fn verify_historical_capture(
    reference: &str,
    files: &BTreeMap<String, Vec<u8>>,
    initial: &[u8],
    current: &BTreeMap<String, Value>,
    receipts: &[Value],
    contracts: &catalog::Contracts,
    validator: &SourceCatalogValidator<'_>,
    forms: &mut dyn BibliographicForms,
    l: BibliographicLimits,
) -> Result<()> {
    verify_historical_capture_with(
        reference,
        files,
        initial,
        current,
        receipts,
        validator,
        forms,
        l,
        |_path, raw| {
            catalog::check_catalog_schema_with_contracts(
                contracts,
                validator,
                "ToS/contracts/human-form-set.schema.json",
                "",
                raw,
            )
        },
    )
}

fn verify_historical_capture_with(
    reference: &str,
    files: &BTreeMap<String, Vec<u8>>,
    initial: &[u8],
    current: &BTreeMap<String, Value>,
    receipts: &[Value],
    validator: &SourceCatalogValidator<'_>,
    forms: &mut dyn BibliographicForms,
    l: BibliographicLimits,
    mut check_form_schema: impl FnMut(&str, &[u8]) -> Result<()>,
) -> Result<()> {
    let present = HISTORICAL_CAPTURE
        .iter()
        .filter(|n| files.contains_key(**n))
        .count();
    if present == 0 || present == 1 && files.contains_key("source-create-receipt.json") {
        if files.contains_key("claim-revision-history.json") {
            return Err(Error::Invalid(
                "historical Claim revisions lack captured creation origin",
            ));
        }
        return Ok(());
    }
    if present != HISTORICAL_CAPTURE.len() {
        return Err(Error::Invalid(
            "historical Claim creation capture is partial",
        ));
    }
    let request = owned(
        &files["source-create-request.json"],
        l.catalog.max_row_bytes,
    )?;
    let receipt = owned(
        &files["source-create-receipt.json"],
        l.catalog.max_row_bytes,
    )?;
    exact_keys(
        &request,
        &[
            "schema_version",
            "operation",
            "record",
            "forms",
            "claims",
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
        ],
    )?;
    let record_source_path = text(&receipt, "source_path")?;
    let record_type = text(&request["record"], "record_type")?;
    if request["schema_version"] != "tos_local_source_command_v1"
        || request["operation"] != "historical.create"
        || !request["expected_source"].is_null()
        || !request["expected_revision"].is_null()
        || record_source_path.rsplit_once('/').map(|(p, _)| p)
            != reference.rsplit_once('/').map(|(p, _)| p)
        || record_source_path.rsplit('/').next() != Some(format!("{record_type}.json").as_str())
        || receipt["schema_version"] != "tos_local_historical_create_receipt_v1"
        || receipt["command_id"] != request["command_id"]
        || receipt["request_digest"]
            != format!("sha256:{}", digest(&request, l.catalog.max_row_bytes)?)
        || receipt["owner_configuration"] != request["expected_configuration"]
        || receipt["dependencies"] != request["expected_dependencies"]
        || receipt["source"] != record_ref(&request["record"], false, l)?
        || receipt["grants_admission"] != false
    {
        return Err(Error::Invalid(
            "historical Claim creation capture exact request binding",
        ));
    }
    let originals = array(&request, "claims")?;
    if originals.len() > 4096 {
        return Err(Error::Budget("historical Claim origin count"));
    }
    let mut identities = BTreeSet::new();
    let mut original_bytes = Vec::new();
    for record in originals {
        check(validator, l)?;
        let id = text(record, "claim_id")?;
        if !identities.insert(id.to_owned()) {
            return Err(Error::Invalid("historical Claim creation repeats identity"));
        }
        let raw = encode(record, l.catalog.max_row_bytes)?;
        let needed = original_bytes
            .len()
            .checked_add(raw.len())
            .and_then(|n| n.checked_add(1))
            .filter(|n| *n <= 1_048_576)
            .ok_or(Error::Budget("historical Claim captured original stream"))?;
        original_bytes.reserve(needed - original_bytes.len());
        original_bytes.extend_from_slice(&raw);
        original_bytes.push(b'\n');
        let name = claim_form_name(reference, id)?;
        if let Some(raw) = files.get(&name) {
            let set = owned(raw, l.catalog.max_row_bytes)?;
            check_form_schema(
                &format!("{}/{name}", reference.rsplit_once('/').unwrap().0),
                raw,
            )?;
            if set["subject"]["id"] != id {
                return Err(Error::Invalid(
                    "historical Claim current form set foreign subject",
                ));
            }
            let materialized = forms.materialize(
                current.get(id).ok_or(Error::Invalid(
                    "historical Claim original absent from current stream",
                ))?,
                &set,
                262144,
            )?;
            encode(&json!(materialized), 262144)?;
        }
        if files
            .get(&format!(".{name}.writer.lock"))
            .is_some_and(|raw| !raw.is_empty())
        {
            return Err(Error::Invalid(
                "historical Claim writer lock contains bytes",
            ));
        }
    }
    if original_bytes != initial
        || identities != current.keys().cloned().collect::<BTreeSet<_>>()
        || receipts.iter().any(|r| {
            r["source"]["id"]
                .as_str()
                .is_none_or(|id| !identities.contains(id))
        })
    {
        return Err(Error::Invalid(
            "historical Claim lineage differs from captured original identities/stream",
        ));
    }
    for name in [
        "historical-claims.jsonl",
        "source-create-request.json",
        "source-create-environment.json",
        "source-create-provenance.jsonl",
    ] {
        let raw = if name == "historical-claims.jsonl" {
            initial
        } else {
            files[name].as_slice()
        };
        if receipt["files"][name]
            != json!({"sha256":format!("sha256:{}",Digest256::of_bytes(raw).to_hex()),"bytes":raw.len()})
        {
            return Err(Error::Invalid(
                "historical Claim captured byte fixity differs from creation receipt",
            ));
        }
    }
    Ok(())
}
fn claim_records(raw: &[u8], l: BibliographicLimits) -> Result<BTreeMap<String, Value>> {
    if raw.len() > 1_048_576 {
        return Err(Error::Budget(
            "bibliographic exact version Claim stream bytes",
        ));
    }
    let mut records = BTreeMap::new();
    for bytes in raw
        .split(|b| *b == b'\n')
        .filter(|row| !row.iter().all(u8::is_ascii_whitespace))
    {
        if std::time::Instant::now() >= l.deadline {
            return Err(Error::Budget("bibliographic Claim version parse deadline"));
        }
        let record = owned(bytes, l.catalog.max_row_bytes)?;
        exact_ref(&record_ref(&record, true, l)?, true)?;
        if record["claim_type"] != "relation"
            || !matches!(
                record["visibility"].as_str(),
                Some("public" | "public_metadata_only")
            )
        {
            return Err(Error::Invalid(
                "bibliographic public versioned Claim carrier",
            ));
        }
        if records
            .insert(text(&record, "claim_id")?.into(), record)
            .is_some()
        {
            return Err(Error::Invalid(
                "bibliographic duplicate versioned Claim stream identity",
            ));
        }
    }
    Ok(records)
}
fn claim_line(raw: &[u8], id: &str, l: BibliographicLimits) -> Result<u64> {
    for (index, bytes) in raw.split(|b| *b == b'\n').enumerate() {
        if bytes.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let row = SourceRow::parse(bytes, l.catalog.max_row_bytes)?;
        if row.value()["claim_id"] == id {
            return Ok(index as u64 + 1);
        }
    }
    Err(Error::Invalid(
        "bibliographic selected retained Claim source line",
    ))
}
fn claim_form_name(reference: &str, id: &str) -> Result<String> {
    let basename = reference
        .rsplit('/')
        .next()
        .ok_or(Error::Invalid("bibliographic Claim forms source locator"))?;
    let stem = basename
        .strip_suffix(".jsonl")
        .ok_or(Error::Invalid("bibliographic Claim forms stream suffix"))?;
    Ok(format!(
        "{stem}.{}.human-forms.json",
        Digest256::of_bytes(id.as_bytes()).to_hex()
    ))
}
fn advance_claim(previous: &Value, request: &Value, l: BibliographicLimits) -> Result<Value> {
    let fields = request["fields"]
        .as_object()
        .filter(|fields| !fields.is_empty())
        .ok_or(Error::Invalid(
            "bibliographic retained Claim correction fields",
        ))?;
    let transition = request.get("layer_transition");
    let allowed = if transition.is_some() {
        &["assertion_layer"][..]
    } else {
        &[
            "qualifiers",
            "evidence_refs",
            "counterevidence_refs",
            "alternative_claim_refs",
            "supporting_quotes",
            "epistemic_status",
            "confidence",
            "object",
        ][..]
    };
    if fields.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(Error::Invalid(
            "bibliographic Claim correction changed immutable identity/admission field",
        ));
    }
    if let Some(transition) = transition {
        exact_keys(transition, &["from", "to"])?;
        for key in ["from", "to"] {
            let name = text(transition, key)?;
            if name.is_empty()
                || name.len() > 64
                || !name.as_bytes()[0].is_ascii_lowercase()
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
            {
                return Err(Error::Invalid(
                    "bibliographic exact retained Claim layer transition",
                ));
            }
        }
        if transition["from"] == transition["to"]
            || transition["from"] != previous["assertion_layer"]
            || fields.get("assertion_layer") != transition.get("to")
        {
            return Err(Error::Invalid(
                "bibliographic retained Claim predecessor layer transition",
            ));
        }
    }
    if fields.contains_key("object")
        && (!previous["object"].is_object() || !fields["object"].is_object())
    {
        return Err(Error::Invalid(
            "bibliographic Claim correction cannot change identity endpoint",
        ));
    }
    let mut revised = previous
        .as_object()
        .ok_or(Error::Invalid("bibliographic Claim predecessor object"))?
        .clone();
    for (key, value) in fields {
        let value = if key == "qualifiers" {
            let mut merged = previous
                .get("qualifiers")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            for (key, value) in value
                .as_object()
                .ok_or(Error::Invalid("bibliographic Claim qualifier patch"))?
            {
                merged.insert(key.clone(), value.clone());
            }
            Value::Object(merged)
        } else {
            value.clone()
        };
        revised.insert(key.clone(), value);
    }
    if Value::Object(revised.clone()) == *previous {
        return Err(Error::Invalid(
            "bibliographic retained Claim correction is unchanged",
        ));
    }
    let version = previous["claim_version"]
        .as_u64()
        .and_then(|n| n.checked_add(1))
        .ok_or(Error::Invalid("bibliographic Claim version arithmetic"))?;
    revised.insert("claim_version".into(), json!(version));
    let revised = Value::Object(revised);
    encode(&revised, l.catalog.max_row_bytes)?;
    if matches!(
        previous["predicate"].as_str(),
        Some("identity_transition_proposal" | "subject_identity_transition_proposal")
    ) {
        for field in [
            "kind",
            "operation",
            "members",
            "predecessors",
            "successors",
            "mapping",
            "supersedes_proposal",
        ] {
            if revised["object"].get(field).is_none()
                || previous["object"].get(field) != revised["object"].get(field)
            {
                return Err(Error::Invalid(
                    "bibliographic retained proposal correction changed frozen topology",
                ));
            }
        }
    }
    Ok(revised)
}
fn replace_claim(raw: &[u8], revised: &Value, l: BibliographicLimits) -> Result<Vec<u8>> {
    claim_records(raw, l)?;
    let id = text(revised, "claim_id")?;
    let mut output = Vec::new();
    let mut found = false;
    for physical in raw.split_inclusive(|b| *b == b'\n') {
        let ending = if physical.ends_with(b"\r\n") {
            &b"\r\n"[..]
        } else if physical.ends_with(b"\n") {
            &b"\n"[..]
        } else {
            &b""[..]
        };
        let bytes = &physical[..physical.len() - ending.len()];
        if !bytes.iter().all(u8::is_ascii_whitespace)
            && SourceRow::parse(bytes, l.catalog.max_row_bytes)?.value()["claim_id"] == id
        {
            if found {
                return Err(Error::Invalid(
                    "bibliographic corrected Claim appears twice",
                ));
            }
            output.extend_from_slice(&encode(revised, l.catalog.max_row_bytes)?);
            output.extend_from_slice(ending);
            found = true;
        } else {
            output.extend_from_slice(physical);
        }
        if output.len() > 1_048_576 {
            return Err(Error::Budget("bibliographic corrected Claim stream bytes"));
        }
    }
    if !found {
        return Err(Error::Invalid(
            "bibliographic corrected Claim missing from predecessor stream",
        ));
    }
    Ok(output)
}
fn claim_receipt(
    receipt: &Value,
    commands: &mut BTreeSet<String>,
    l: BibliographicLimits,
) -> Result<()> {
    exact_keys(
        receipt,
        &[
            "command_id",
            "request_digest",
            "principal_id",
            "authority_ref",
            "owner_configuration",
            "recorded_at",
            "reason",
            "previous_source",
            "source",
            "previous_revision",
            "archive_path",
            "dependencies",
            "source_bindings",
            "changed_fields",
            "forms",
            "grants_admission",
            "request",
        ],
    )?;
    let request = &receipt["request"];
    let instant = text(receipt, "recorded_at")?;
    tos_validation::retirement_rules::observed_instant_order(instant, instant)
        .map_err(|_| Error::Invalid("bibliographic retained Claim aware instant"))?;
    exact_ref(&receipt["previous_source"], true)?;
    exact_ref(&receipt["source"], true)?;
    let fields = request["fields"].as_object().ok_or(Error::Invalid(
        "bibliographic retained Claim request fields",
    ))?;
    let mut changed = fields.keys().map(|key| json!(key)).collect::<Vec<_>>();
    changed.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    if !commands.insert(text(receipt, "command_id")?.into())
        || request["operation"] != "claim.revise"
        || receipt["request_digest"]
            != format!("sha256:{}", digest(request, l.catalog.max_row_bytes)?)
        || receipt["changed_fields"] != Value::Array(changed)
        || receipt["grants_admission"] != false
        || receipt["source"]["id"] != receipt["previous_source"]["id"]
        || receipt["source"]["version"].as_u64()
            != receipt["previous_source"]["version"]
                .as_u64()
                .and_then(|n| n.checked_add(1))
    {
        return Err(Error::Invalid(
            "bibliographic native Claim correction receipt",
        ));
    }
    for (left, right) in [
        ("command_id", "command_id"),
        ("previous_source", "expected_source"),
        ("previous_revision", "expected_revision"),
        ("owner_configuration", "expected_configuration"),
        ("dependencies", "expected_dependencies"),
        ("source_bindings", "expected_inputs"),
        ("reason", "reason"),
    ] {
        if receipt[left] != request[right] {
            return Err(Error::Invalid(
                "bibliographic retained Claim request binding",
            ));
        }
    }
    Ok(())
}
fn form_ref(form: &Value, l: BibliographicLimits) -> Result<Value> {
    Ok(
        json!({"id":text(form,"form_id")?,"version":form["form_version"],"digest":format!("sha256:{}",digest(form,l.catalog.max_row_bytes)?)}),
    )
}
fn reconstruct_forms(
    contracts: &catalog::Contracts,
    revised: &Value,
    prior: Option<&Value>,
    current: &Value,
    receipt: &Value,
    validator: &SourceCatalogValidator<'_>,
    forms: &mut dyn BibliographicForms,
    l: BibliographicLimits,
) -> Result<()> {
    reconstruct_forms_with(
        revised,
        prior,
        current,
        receipt,
        validator,
        forms,
        l,
        |raw| {
            catalog::check_catalog_schema_with_contracts(
                contracts,
                validator,
                "ToS/contracts/human-form-set.schema.json",
                "",
                raw,
            )
        },
    )
}

fn reconstruct_forms_with(
    revised: &Value,
    prior: Option<&Value>,
    current: &Value,
    receipt: &Value,
    _validator: &SourceCatalogValidator<'_>,
    forms: &mut dyn BibliographicForms,
    l: BibliographicLimits,
    mut check_form_schema: impl FnMut(&[u8]) -> Result<()>,
) -> Result<()> {
    for set in prior.into_iter().chain(std::iter::once(current)) {
        check_form_schema(&encode(set, l.catalog.max_row_bytes)?)?;
        if set["subject"]["id"] != revised["claim_id"] {
            return Err(Error::Invalid("bibliographic retained form set subject"));
        }
    }
    let selections = &receipt["request"]["forms"];
    let selections_array = selections
        .as_array()
        .filter(|values| (1..=32).contains(&values.len()))
        .ok_or(Error::Invalid("bibliographic retained form selector count"))?;
    let mut selected = BTreeSet::new();
    let mut statement = false;
    for selection in selections_array {
        exact_keys(selection, &["form_id", "field_id"])?;
        if !selected.insert(text(selection, "form_id")?) {
            return Err(Error::Invalid(
                "bibliographic duplicate retained form selector",
            ));
        }
        statement |= selection["field_id"] == "claim.statement";
    }
    if !statement
        || prior.is_some_and(|prior| {
            prior["forms"].as_array().is_some_and(|values| {
                values
                    .iter()
                    .any(|form| !selected.contains(form["form_id"].as_str().unwrap_or("")))
            })
        })
    {
        return Err(Error::Invalid(
            "bibliographic retained correction omitted complete statement/prior form",
        ));
    }
    let result = forms.reconstruct_revision_forms(
        revised,
        prior,
        text(receipt, "principal_id")?,
        selections,
        262_144,
    )?;
    let raw = encode(&result, 262_144)?;
    check_form_schema(&raw)?;
    let subject = record_ref(revised, true, l)?;
    if result["subject"] != subject {
        return Err(Error::Invalid(
            "bibliographic reconstructed form set exact source subject",
        ));
    }
    let returned = array(&result, "forms")?;
    if returned.len() != selected.len() {
        return Err(Error::Invalid(
            "bibliographic reconstructed complete form set",
        ));
    }
    let mut references = Vec::new();
    for selection in selections_array {
        let matching = returned
            .iter()
            .filter(|form| form["form_id"] == selection["form_id"])
            .collect::<Vec<_>>();
        if matching.len() != 1 || matching[0]["subject"] != subject {
            return Err(Error::Invalid(
                "bibliographic reconstructed form identity/source binding",
            ));
        }
        references.push(form_ref(matching[0], l)?);
    }
    if receipt["forms"] != Value::Array(references.clone()) {
        return Err(Error::Invalid(
            "bibliographic retained correction reconstructed form refs",
        ));
    }
    let retained = array(current, "forms")?
        .iter()
        .chain(array(current, "prior_forms")?.iter())
        .map(|form| form_ref(form, l))
        .collect::<Result<Vec<_>>>()?;
    if references
        .iter()
        .any(|reference| !retained.contains(reference))
    {
        return Err(Error::Invalid(
            "bibliographic reconstructed correction forms not retained in current set",
        ));
    }
    Ok(())
}

fn public_form_lock(name: &str) -> bool {
    if matches!(
        name,
        ".historical-event.human-forms.json.writer.lock"
            | ".historical-process.human-forms.json.writer.lock"
            | ".historical-state.human-forms.json.writer.lock"
    ) {
        return true;
    }
    let tail = name
        .strip_prefix(".source-claims.")
        .or_else(|| name.strip_prefix(".historical-claims."));
    tail.and_then(|tail| tail.strip_suffix(".human-forms.json.writer.lock"))
        .is_some_and(|sha| {
            sha.len() == 64
                && sha
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        })
}

#[cfg(test)]
mod oracle {
    use super::*;

    #[test]
    fn legacy_catalog_locations_preserve_exact_family_lines_digest_and_bound() {
        let l = BibliographicLimits {
            catalog: catalog::SourceCatalogLimits {
                max_files: 16,
                max_rows: 16,
                max_file_bytes: 4096,
                max_row_bytes: 4096,
                max_contract_bytes: 4096,
                max_output_row_bytes: 4096,
            },
            max_claim_cohort_rows: 16,
            max_claim_cohort_bytes: 8192,
            max_output_rows: 16,
            max_output_bytes: 8192,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(5),
        };
        let rows = [
            (
                "a".to_owned(),
                json!({"entry":{"label":"Первый","record_type":"work"}}),
            ),
            (
                "b".to_owned(),
                json!({"entry":{"label":"Middle","record_type":"agent"}}),
            ),
            (
                "c".to_owned(),
                json!({"entry":{"label":"Last","record_type":"work"}}),
            ),
        ];
        let build = |kind, cap| {
            let mut rows = rows.clone().into_iter();
            legacy_catalog_locations(|| Ok(rows.next()), kind, cap, l)
        };
        let work = build(Some("work"), 8192).unwrap();
        let expected = concat!(
            "{\"label\":\"Первый\",\"record_type\":\"work\"}\n",
            "{\"label\":\"Last\",\"record_type\":\"work\"}\n"
        );
        let digest = format!(
            "sha256:{}",
            Digest256::of_bytes(expected.as_bytes()).to_hex()
        );
        for (id, expected_line) in [("a", 1), ("c", 2), ("a", 1)] {
            let CatalogLocation::Legacy(line, sha) = work.locate(id).unwrap() else {
                panic!("legacy shape")
            };
            assert_eq!((line, sha), (expected_line, digest.clone()));
        }
        assert!(work.locate("b").is_err());
        let all = build(None, 8192).unwrap();
        let CatalogLocation::Legacy(line, _) = all.locate("c").unwrap() else {
            panic!("legacy shape")
        };
        assert_eq!(line, 3);
        assert!(build(Some("work"), work.retained_bytes).is_ok());
        assert!(build(Some("work"), work.retained_bytes - 1).is_err());
    }

    #[test]
    fn selected_archive_locators_accept_writer_digest_format_and_reject_mismatches() {
        let limits = BibliographicLimits {
            catalog: catalog::SourceCatalogLimits {
                max_files: 128,
                max_rows: 4096,
                max_file_bytes: 2_097_152,
                max_row_bytes: 1_048_576,
                max_contract_bytes: 2_097_152,
                max_output_row_bytes: 1_048_576,
            },
            max_claim_cohort_rows: 1,
            max_claim_cohort_bytes: 1_048_576,
            max_output_rows: 4096,
            max_output_bytes: 8_388_608,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(10),
        };
        let sha = Digest256::of_bytes(b"retained source").to_hex();
        let revision = Digest256::of_bytes(b"retained package").to_hex();
        for selected in [false, true] {
            for (id, source_path, claim) in [
                (
                    "tos.work.fixture",
                    "ToS/source-witnesses/works/fixture/work.json",
                    false,
                ),
                (
                    "tos.claim.fixture",
                    "ToS/source-witnesses/relations/fixture/source-claims.jsonl",
                    true,
                ),
            ] {
                let archive = format!(
                    "ToS/source-witnesses/.record-revisions/{}-{revision}",
                    Digest256::of_bytes(id.as_bytes()).to_hex()
                );
                let path = format!("{archive}/manifest.json");
                let basename = source_path.rsplit('/').next().unwrap();
                let mut manifest = json!({
                    "schema_version": if selected { "tos_source_package_archive_v2" } else { "tos_source_package_archive_v1" },
                    "source_path": source_path,
                    "source": {"id": id, "version": 1, "digest": format!("sha256:{sha}")},
                    "revision": format!("sha256:{revision}"),
                    "files": {basename: {"sha256": format!("sha256:{sha}"), "blob": format!("{sha}.blob"), "bytes": 15}}
                });
                if selected {
                    manifest["publication_protocol"] = json!("tos_selected_source_metadata_v1");
                }
                let resolve = |manifest: &Value| -> Result<Vec<String>> {
                    let raw = serde_json::to_vec(manifest).unwrap();
                    if claim {
                        selected_claim_archive_manifest(&path, &raw, limits)?;
                        let files = SelectedBibliographicFilesV1::new(
                            BTreeMap::from([(path.clone(), raw)]),
                            BTreeSet::new(),
                            1,
                            4096,
                        )?;
                        selected_claim_archive_blob_paths_v1(&[path.clone()], &files, limits)
                    } else {
                        selected_metadata_archive_record_blob_path_v1(&path, &raw, limits)
                            .map(|path| vec![path])
                    }
                };
                assert_eq!(
                    resolve(&manifest).unwrap(),
                    vec![format!("{archive}/{sha}.blob")]
                );
                if !claim {
                    assert!(
                        selected_metadata_archive_forms_blob_path_v1(
                            &path,
                            &serde_json::to_vec(&manifest).unwrap(),
                            limits,
                        )
                        .unwrap()
                        .is_none()
                    );
                    let forms_raw = b"{\"original_forms\":true}\n".to_vec();
                    let forms_sha = Digest256::of_bytes(&forms_raw).to_hex();
                    let forms_name = "work.human-forms.json";
                    let forms_path = format!("{archive}/{forms_sha}.blob");
                    let mut with_forms = manifest.clone();
                    with_forms["files"][forms_name] = json!({
                        "sha256":format!("sha256:{forms_sha}"),
                        "blob":format!("{forms_sha}.blob"), "bytes":forms_raw.len(),
                    });
                    let selected_files = |manifest: &Value, raw: Vec<u8>| {
                        SelectedBibliographicFilesV1::new(
                            BTreeMap::from([
                                (path.clone(), serde_json::to_vec(manifest).unwrap()),
                                (forms_path.clone(), raw),
                            ]),
                            BTreeSet::new(),
                            2,
                            4096,
                        )
                        .unwrap()
                    };
                    let files = selected_files(&with_forms, forms_raw.clone());
                    let (observed_path, observed_raw) =
                        resolve_selected_metadata_archive_forms_v1(&path, &files, limits)
                            .unwrap()
                            .unwrap();
                    assert_eq!(observed_path, forms_path);
                    assert_eq!(observed_raw, forms_raw);
                    let changed = selected_files(&with_forms, b"different forms".to_vec());
                    assert!(
                        resolve_selected_metadata_archive_forms_v1(&path, &changed, limits)
                            .is_err()
                    );
                    with_forms["files"][forms_name]["bytes"] = json!(forms_raw.len() + 1);
                    let wrong_length = selected_files(&with_forms, forms_raw);
                    assert!(
                        resolve_selected_metadata_archive_forms_v1(&path, &wrong_length, limits)
                            .is_err()
                    );
                }
                let mut unprefixed = manifest.clone();
                unprefixed["files"][basename]["sha256"] = json!(sha);
                assert!(resolve(&unprefixed).is_err());
                manifest["files"][basename]["blob"] = json!(format!("{revision}.blob"));
                assert!(resolve(&manifest).is_err());
            }
        }
    }

    #[test]
    fn retained_python_successor_preserves_unselected_sibling_bytes() {
        // Independent maintained claim_revisions._advance/_replace oracle,
        // using the existing transport fixture plus exact physical whitespace.
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../access/tests/fixtures/knowledge-contract/temporal-jenseits-date.json"
        ))
        .unwrap();
        let previous = fixture["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["node_kind"] == "claim")
            .unwrap()["properties"]["source_claim"]
            .clone();
        let mut sibling = previous.clone();
        sibling["claim_id"] = json!("tos.claim.fixture.sibling");
        sibling["extensions"] = json!({"unknown":[false,0,null,"Ω"]});
        let mut raw = b"  ".to_vec();
        raw.extend(encode(&previous, 1_048_576).unwrap());
        raw.extend(b"\r\n\n\t");
        let mut suffix = b"\t".to_vec();
        suffix.extend(encode(&sibling, 1_048_576).unwrap());
        suffix.extend(b" \n");
        raw.extend(&suffix[1..]);
        let limits = BibliographicLimits {
            catalog: catalog::SourceCatalogLimits {
                max_files: 128,
                max_rows: 4096,
                max_file_bytes: 2_097_152,
                max_row_bytes: 1_048_576,
                max_contract_bytes: 2_097_152,
                max_output_row_bytes: 1_048_576,
            },
            max_claim_cohort_rows: 1,
            max_claim_cohort_bytes: 1_048_576,
            max_output_rows: 4096,
            max_output_bytes: 8_388_608,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(10),
        };
        let revised = advance_claim(
            &previous,
            &json!({"fields":{"qualifiers":{"statement":"Исправленная условная датировка."}}}),
            limits,
        )
        .unwrap();
        let result = replace_claim(&raw, &revised, limits).unwrap();
        assert_eq!(
            Digest256::of_bytes(&result).to_hex(),
            "3fe1ab6a4691cd9eee084af315a8b078ca16869323da0e9232df13aa639440f7"
        );
        assert!(result.ends_with(&suffix));
        let mut identity = previous;
        identity["object"] = json!("tos.work.fixture.endpoint");
        assert!(
            advance_claim(
                &identity,
                &json!({"fields":{"object":{"kind":"invented"}}}),
                limits
            )
            .is_err()
        );
    }
    #[test]
    fn v1_catalog_location_preserves_exact_canonical_fields_and_v2_has_no_fake_line() {
        let context = json!({"source_ref":"ToS/source-witnesses/catalog/agents.jsonl",
            "source_record_ref":"ToS/agent.json","current_record_ref":{"id":"tos.agent.a","version":1,"digest":"sha256:original"}});
        let expected = json!({"source_ref":"ToS/source-witnesses/catalog/agents.jsonl",
            "line":7,"sha256":"sha256:whole-jsonl","source_record_ref":"ToS/agent.json",
            "current_record_ref":{"id":"tos.agent.a","version":1,"digest":"sha256:original"}});
        let legacy =
            CatalogLocation::Legacy(7, "sha256:whole-jsonl".into()).render(context.clone());
        assert_eq!(
            encode(&legacy, 4096).unwrap(),
            encode(&expected, 4096).unwrap()
        );
        let epoch = json!({"schema_version":"tos_versions_catalog_reference_v2",
            "epoch":{"root_sha256":"selected-epoch"},"record_id":"tos.agent.a",
            "canonical_entry_sha256":"selected-entry"});
        let rendered = CatalogLocation::Epoch(epoch).render(context);
        assert!(rendered.get("line").is_none());
        assert!(rendered.get("sha256").is_none());
        assert_eq!(rendered["source_record_ref"], expected["source_record_ref"]);
        assert_eq!(
            rendered["current_record_ref"],
            expected["current_record_ref"]
        );
    }
}
