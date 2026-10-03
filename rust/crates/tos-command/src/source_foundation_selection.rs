//! Bounded, source-cut-backed selection for the source-foundation physical
//! snapshot. This module selects only the exact finite inputs consumed by the
//! existing Discovery, Labs, Goldset, record, and payload owners. It does not
//! observe a filesystem or infer physical facts from source-cut membership.

use super::foundation_capture::FoundationCapturedCut;
use serde_json::Value;
use std::collections::BTreeSet;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath};
use tos_ops_mechanics_plan::route_cards::{RouteResolvedTarget, RouteSources};
use tos_source_store::CorpusCutReader;
use tos_validation::item_rules::ItemRefusal;
use tos_validation::source_foundation_discovery::{
    PhysicalResolvedTargetFacts, SourcePhysicalFacts,
};
use tos_validation::source_foundation_labs::foundation_lab_private_output_refs;

const MAX_PATH_BYTES: usize = 4096;
const MAX_PATH_COMPONENTS: usize = 128;
const PATH_FORMATTING_WORKSPACE: usize = MAX_PATH_BYTES * 3 + 256;
const JSON_VALUE_BYTES_PER_SOURCE_BYTE: usize = 65;
const JSON_PARSE_FIXED_OVERHEAD: usize = 8192;
const SOURCE_HOME: &str = "ToS/source-witnesses/";
const ARTIFACTS: &str = "ToS/source-witnesses/artifacts/";
const COMPOSITES: &str = "ToS/source-witnesses/scholarly-composites/";
const PRIVATE_ROUTE: &str = "ToS/source-witnesses/access-requests/private";
const PRIVATE_ROUTE_CARD: &str = "ToS/source-witnesses/access-requests/private/README.md";
const PRIVATE_HANDOFF: &str =
    "ToS/research-packets/foundation-laboratory-2026-07/private-evidence-handoff.v1.json";
const MANUAL_LEDGER: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/manual-error-ledger.jsonl";
const MANUAL_LEDGER_PROVENANCE: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/provenance.manual-error-ledger.ocr-candidate-review-foundation-v1.jsonl";
const TRANSFER_CROSSWALK: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/naumann-1886-polilov-mysl-1996/transfer-candidate-page-crosswalk.v1.json";
const HIERARCHICAL_TARGET_ROOTS: &[&str] = &[
    "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/expressions/ru-svasyan-mysl-1996/structure/mysl-1996-volume-2-operator-pdf",
    "ToS/source-witnesses/works/friedrich-nietzsche/der-antichrist/expressions/ru-flerova-mysl-1996/structure/mysl-1996-volume-2-operator-pdf",
];
const GOLD_DOCUMENTS: &[&str] = &[
    "transfer-samples.json",
    "retrieval-queries.json",
    "ocr-visual-samples.json",
    "visual-retrieval-plan.v1.json",
    "translation-source-review-plan.v2.json",
    "translation-laboratory-plan.v1.json",
];
const RETRIEVAL_DOCUMENT: &str = "retrieval-queries.json";
const LAB_DOCUMENTS: &[&str] = &[
    "antonovsky-2007-1911-opening-sentence-collation.plan.v1.json",
    "authored-canon-evidence-bridge.plan.v1.json",
];
const ARTIFACT_COMPANIONS: &[&str] = &[
    "source-create-request.json",
    "source-create-receipt.json",
    "source-create-environment.json",
    "source-create-provenance.jsonl",
    "artifact-witness.human-forms.json",
];

#[derive(Clone, Copy)]
enum RepoPathBucket {
    Authored,
    Private,
    PrivatePrefix,
    Payload,
}

/// Caller-selected selector budgets. These are local to the source-derived
/// selector pass; the complete physical and payload ceilings remain separate
/// caller-owned arguments to their existing providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FoundationSelectionLimits {
    pub max_selector_documents: usize,
    pub max_output_entries: usize,
    pub max_private_prefixes: usize,
    pub max_payload_paths: usize,
    pub max_member_bytes: u64,
    pub max_total_read_bytes: u64,
    pub max_state_bytes: usize,
    pub deadline: Instant,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FoundationSelectionCost {
    pub selector_documents: usize,
    pub source_bytes_read: u64,
    pub output_entries: usize,
    pub retained_state_bytes: usize,
    pub peak_state_bytes: usize,
}

/// Exact finite path lists passed to the existing physical and payload
/// snapshot owners. Artifact paths are relative to the separately selected,
/// held artifact-root capability. Payload paths are full ToS-relative paths.
#[derive(Debug, Clone)]
pub(crate) struct FoundationPhysicalSelection {
    limits: FoundationSelectionLimits,
    pub authored_paths: Vec<String>,
    pub private_paths: Vec<String>,
    pub private_prefixes: Vec<String>,
    pub artifact_paths: Vec<String>,
    pub payload_paths: Vec<String>,
    pub resolved_source_directories: std::collections::BTreeMap<String, String>,
    /// Authored-plan-selected private JSON files to read from the held ToS
    /// root before the final physical snapshot, never from the authored cut.
    pub auxiliary_query_documents: Vec<String>,
    /// Bindings emitted by the bounded secondary read and joined against the
    /// final physical snapshot before any owner consumes the observations.
    pub query_content_bindings: Vec<FoundationQueryContentBinding>,
    pub cost: FoundationSelectionCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FoundationQueryContentBinding {
    pub path: String,
    pub sha256: Option<String>,
    pub byte_size: Option<u64>,
}

enum SelectionInput<'a> {
    Cut(&'a CorpusCutReader),
    Candidate(&'a dyn tos_validation::record_biblio_cut::SourceCutInput),
}
impl SelectionInput<'_> {
    fn walk(
        &self,
        limits: FoundationSelectionLimits,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        match self {
            Self::Cut(cut) => {
                for member in cut.current().members() {
                    selection_checkpoint(limits, cancelled)?;
                    visit(member.path.as_str())?;
                }
                Ok(())
            }
            Self::Candidate(input) => {
                input.for_each_current_member_meta(limits.deadline, cancelled, &mut |meta| {
                    visit(meta.path)
                })
            }
        }
    }
    fn present(
        &self,
        path: &str,
        limits: FoundationSelectionLimits,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        match self {
            Self::Cut(cut) => Ok(RelativePath::parse(path)
                .ok()
                .is_some_and(|path| cut.current().member(&path).is_some())),
            Self::Candidate(input) => Ok(matches!(
                input.path_presence(path, limits.deadline, cancelled)?,
                Some(tos_source_store::SourcePresenceV1::File)
            )),
        }
    }
    fn read(
        &self,
        path: &str,
        limits: FoundationSelectionLimits,
        cancelled: &AtomicBool,
        available_state_bytes: usize,
        visit: &mut dyn FnMut(u64, &[u8]) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        match self {
            Self::Cut(cut) => {
                let Ok(path) = RelativePath::parse(path) else {
                    return Ok(());
                };
                let Some(metadata) = cut.current().member(&path) else {
                    return Ok(());
                };
                if metadata.size_bytes > limits.max_member_bytes
                    || usize::try_from(metadata.size_bytes)
                        .ok()
                        .and_then(|n| n.checked_mul(JSON_VALUE_BYTES_PER_SOURCE_BYTE))
                        .and_then(|n| n.checked_add(JSON_PARSE_FIXED_OVERHEAD))
                        .is_none_or(|n| n > available_state_bytes)
                {
                    return Err(ItemRefusal::Budget);
                }
                let member = cut
                    .read_member(
                        cut.current().revision(),
                        &path,
                        limits.max_member_bytes,
                        limits.deadline,
                        cancelled,
                    )
                    .map_err(|_| {
                        ItemRefusal::Source("foundation selector exact-cut read refused".into())
                    })?;
                visit(metadata.size_bytes, &member.raw)
            }
            Self::Candidate(input) => {
                // Narrow the carrier's actual raw allocation BEFORE entering
                // its reader; the decoded Value and retained selector state
                // must fit the same envelope used by the cold carrier.
                let state_member_cap = available_state_bytes
                    .checked_sub(JSON_PARSE_FIXED_OVERHEAD)
                    .ok_or(ItemRefusal::Budget)?
                    / JSON_VALUE_BYTES_PER_SOURCE_BYTE;
                let member_cap = usize::try_from(limits.max_member_bytes)
                    .map_err(|_| ItemRefusal::Budget)?
                    .min(state_member_cap);
                if member_cap == 0 {
                    return Err(ItemRefusal::Budget);
                }
                input.with_current_member(
                    path,
                    member_cap,
                    limits.deadline,
                    cancelled,
                    &mut |meta, raw| visit(meta.size_bytes, raw),
                )
            }
        }
    }
}

struct Builder<'a> {
    limits: FoundationSelectionLimits,
    cancelled: &'a AtomicBool,
    authored_paths: BTreeSet<String>,
    private_paths: BTreeSet<String>,
    private_prefixes: BTreeSet<String>,
    artifact_paths: BTreeSet<String>,
    payload_paths: BTreeSet<String>,
    auxiliary_query_documents: BTreeSet<String>,
    resolved_source_directories: std::collections::BTreeMap<String, String>,
    retained_state_bytes: usize,
    document_state_bytes: usize,
    selector_documents: usize,
    source_bytes_read: u64,
    output_entries: usize,
    peak_state_bytes: usize,
}

impl<'a> Builder<'a> {
    fn new(
        limits: FoundationSelectionLimits,
        cancelled: &'a AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        if limits.max_selector_documents == 0
            || limits.max_output_entries == 0
            || limits.max_private_prefixes == 0
            || limits.max_member_bytes == 0
            || limits.max_total_read_bytes == 0
            || limits.max_state_bytes == 0
        {
            return Err(ItemRefusal::Budget);
        }
        let retained_state_bytes = size_of::<FoundationPhysicalSelection>()
            .checked_add(size_of::<Self>())
            .and_then(|bytes| bytes.checked_add(PATH_FORMATTING_WORKSPACE))
            .ok_or(ItemRefusal::Budget)?;
        if retained_state_bytes > limits.max_state_bytes {
            return Err(ItemRefusal::Budget);
        }
        Ok(Self {
            limits,
            cancelled,
            authored_paths: BTreeSet::new(),
            private_paths: BTreeSet::new(),
            private_prefixes: BTreeSet::new(),
            artifact_paths: BTreeSet::new(),
            payload_paths: BTreeSet::new(),
            auxiliary_query_documents: BTreeSet::new(),
            resolved_source_directories: std::collections::BTreeMap::new(),
            retained_state_bytes,
            document_state_bytes: 0,
            selector_documents: 0,
            source_bytes_read: 0,
            output_entries: 0,
            peak_state_bytes: retained_state_bytes,
        })
    }

    fn checkpoint(&self) -> Result<(), ItemRefusal> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(ItemRefusal::Source(
                "foundation path selector cancelled".into(),
            ));
        }
        if Instant::now() >= self.limits.deadline {
            return Err(ItemRefusal::Deadline);
        }
        Ok(())
    }

    fn reserve_retained(&mut self, bytes: usize) -> Result<(), ItemRefusal> {
        let next = self
            .retained_state_bytes
            .checked_add(bytes)
            .ok_or(ItemRefusal::Budget)?;
        let peak = next
            .checked_add(self.document_state_bytes)
            .filter(|peak| *peak <= self.limits.max_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        self.retained_state_bytes = next;
        self.peak_state_bytes = self.peak_state_bytes.max(peak);
        Ok(())
    }

    fn reserve_output_entry(&mut self, path: &str) -> Result<(), ItemRefusal> {
        let next = self
            .output_entries
            .checked_add(1)
            .filter(|count| *count <= self.limits.max_output_entries)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "foundation-selector-output-entries",
                used: Some(self.output_entries.saturating_add(1) as u64),
                limit: Some(self.limits.max_output_entries as u64),
            })?;
        let row_bytes = path
            .len()
            .checked_add(size_of::<String>())
            .and_then(|bytes| bytes.checked_add(32 * size_of::<usize>()))
            .ok_or(ItemRefusal::Budget)?;
        self.reserve_retained(row_bytes)?;
        self.output_entries = next;
        Ok(())
    }

    fn add_repo_path(
        &mut self,
        bucket: RepoPathBucket,
        path: &str,
        require_file: bool,
    ) -> Result<(), ItemRefusal> {
        self.checkpoint()?;
        if path.len() > MAX_PATH_BYTES
            || path.split('/').count() > MAX_PATH_COMPONENTS
            || RelativePath::parse(path).is_err()
            || !path.starts_with("ToS/")
            || (require_file && path.ends_with('/'))
        {
            return Ok(());
        }
        let contains = match bucket {
            RepoPathBucket::Authored => self.authored_paths.contains(path),
            RepoPathBucket::Private => self.private_paths.contains(path),
            RepoPathBucket::PrivatePrefix => self.private_prefixes.contains(path),
            RepoPathBucket::Payload => self.payload_paths.contains(path),
        };
        if contains {
            return Ok(());
        }
        match bucket {
            RepoPathBucket::PrivatePrefix => {
                if self
                    .private_prefixes
                    .iter()
                    .any(|existing| path_within(path, existing) || path_within(existing, path))
                {
                    return Err(ItemRefusal::Unsupported(
                        "foundation selected private inventory prefixes overlap".into(),
                    ));
                }
                if self.private_prefixes.len() >= self.limits.max_private_prefixes {
                    return Err(ItemRefusal::BudgetCheck {
                        check: "foundation-selector-private-prefixes",
                        used: Some(self.private_prefixes.len().saturating_add(1) as u64),
                        limit: Some(self.limits.max_private_prefixes as u64),
                    });
                }
            }
            RepoPathBucket::Payload
                if self.payload_paths.len() >= self.limits.max_payload_paths =>
            {
                return Err(ItemRefusal::BudgetCheck {
                    check: "foundation-selector-payload-paths",
                    used: Some(self.payload_paths.len().saturating_add(1) as u64),
                    limit: Some(self.limits.max_payload_paths as u64),
                });
            }
            _ => {}
        }
        self.reserve_output_entry(path)?;
        match bucket {
            RepoPathBucket::Authored => &mut self.authored_paths,
            RepoPathBucket::Private => &mut self.private_paths,
            RepoPathBucket::PrivatePrefix => &mut self.private_prefixes,
            RepoPathBucket::Payload => &mut self.payload_paths,
        }
        .insert(path.to_owned());
        Ok(())
    }

    fn add_artifact_path(&mut self, path: &str) -> Result<(), ItemRefusal> {
        self.checkpoint()?;
        if path.len() > MAX_PATH_BYTES
            || path.split('/').count() > MAX_PATH_COMPONENTS
            || RelativePath::parse(path).is_err()
        {
            return Ok(());
        }
        if self.artifact_paths.contains(path) {
            return Ok(());
        }
        self.reserve_output_entry(path)?;
        self.artifact_paths.insert(path.to_owned());
        Ok(())
    }

    fn add_auxiliary_query_document(&mut self, path: &str) -> Result<(), ItemRefusal> {
        self.checkpoint()?;
        if self.auxiliary_query_documents.contains(path) {
            return Ok(());
        }
        self.reserve_output_entry(path)?;
        self.auxiliary_query_documents.insert(path.to_owned());
        Ok(())
    }

    fn add_resolved_directory(&mut self, path: &str, directory: &str) -> Result<(), ItemRefusal> {
        self.checkpoint()?;
        if RelativePath::parse(path).is_err()
            || RelativePath::parse(directory).is_err()
            || !path.starts_with("ToS/")
            || !path_within(path, directory)
        {
            return Ok(());
        }
        if self.resolved_source_directories.contains_key(path) {
            return Ok(());
        }
        self.reserve_output_entry(path)?;
        self.reserve_retained(directory.len() + size_of::<String>())?;
        self.resolved_source_directories
            .insert(path.to_owned(), directory.to_owned());
        Ok(())
    }

    fn read_json(
        &mut self,
        input: &SelectionInput<'_>,
        path: &str,
    ) -> Result<Option<Value>, ItemRefusal> {
        self.checkpoint()?;
        let mut selected = None;
        let mut read_limits = self.limits;
        read_limits.max_member_bytes = read_limits.max_member_bytes.min(
            self.limits
                .max_total_read_bytes
                .checked_sub(self.source_bytes_read)
                .ok_or(ItemRefusal::Budget)?,
        );
        let available_state = self
            .limits
            .max_state_bytes
            .checked_sub(self.retained_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        input.read(
            path,
            read_limits,
            self.cancelled,
            available_state,
            &mut |size_bytes, raw| {
                let metadata_size = size_bytes;
                let next_documents = self
                    .selector_documents
                    .checked_add(1)
                    .filter(|count| *count <= self.limits.max_selector_documents)
                    .ok_or(ItemRefusal::BudgetCheck {
                        check: "foundation-selector-documents",
                        used: Some(self.selector_documents.saturating_add(1) as u64),
                        limit: Some(self.limits.max_selector_documents as u64),
                    })?;
                let size = usize::try_from(metadata_size).map_err(|_| ItemRefusal::Budget)?;
                if metadata_size > self.limits.max_member_bytes {
                    return Err(ItemRefusal::BudgetCheck {
                        check: "foundation-selector-member-bytes",
                        used: Some(metadata_size),
                        limit: Some(self.limits.max_member_bytes),
                    });
                }
                let next_read_bytes = self
                    .source_bytes_read
                    .checked_add(metadata_size)
                    .filter(|bytes| *bytes <= self.limits.max_total_read_bytes)
                    .ok_or(ItemRefusal::BudgetCheck {
                        check: "foundation-selector-total-read-bytes",
                        used: Some(self.source_bytes_read.saturating_add(metadata_size)),
                        limit: Some(self.limits.max_total_read_bytes),
                    })?;
                // Cover the raw member, a conservative serde_json Value tree (including
                // per-container/per-entry nodes), and bounded parser recursion before
                // either the raw bytes or the decoded value are allocated.
                let parse_bound = size
                    .checked_mul(JSON_VALUE_BYTES_PER_SOURCE_BYTE)
                    .and_then(|bytes| bytes.checked_add(JSON_PARSE_FIXED_OVERHEAD))
                    .ok_or(ItemRefusal::Budget)?;
                let peak = self
                    .retained_state_bytes
                    .checked_add(parse_bound)
                    .ok_or(ItemRefusal::Budget)?;
                if peak > self.limits.max_state_bytes {
                    return Err(ItemRefusal::BudgetCheck {
                        check: "foundation-selector-state-bytes",
                        used: Some(peak as u64),
                        limit: Some(self.limits.max_state_bytes as u64),
                    });
                }
                self.checkpoint()?;
                if raw.len() != size || raw.len() as u64 != metadata_size {
                    return Err(ItemRefusal::Source(
                        "foundation selector exact-cut member changed".into(),
                    ));
                }
                self.source_bytes_read = next_read_bytes;
                self.selector_documents = next_documents;
                self.document_state_bytes = parse_bound;
                self.peak_state_bytes = self.peak_state_bytes.max(peak);
                let parsed = serde_json::from_slice::<Value>(raw).ok();
                if parsed.is_some() {
                    // The decoded Value remains live in the caller while exact named
                    // fields are copied into the output lists.
                    self.document_state_bytes = parse_bound;
                } else {
                    self.document_state_bytes = 0;
                }
                selected = parsed;
                Ok(())
            },
        )?;
        Ok(selected)
    }

    fn add_private_ref(&mut self, root: &str, reference: &str) -> Result<(), ItemRefusal> {
        self.checkpoint()?;
        if let Some(path) = private_ref_path(root, reference) {
            self.add_repo_path(RepoPathBucket::Private, &path, true)?;
            if let Some(local_content) = join_repo_path(root, "local-content") {
                self.add_resolved_directory(&path, &local_content)?;
            }
        }
        Ok(())
    }

    fn add_query_content_ref(&mut self, root: &str, reference: &str) -> Result<(), ItemRefusal> {
        self.checkpoint()?;
        let Some(path) = query_content_path(root, reference) else {
            return Ok(());
        };
        self.add_private_ref(root, reference)?;
        self.add_auxiliary_query_document(&path)?;
        Ok(())
    }

    fn add_ref_from_binding(&mut self, binding: Option<&Value>) -> Result<(), ItemRefusal> {
        self.checkpoint()?;
        let Some(reference) = binding
            .and_then(|value| value.get("ref"))
            .and_then(Value::as_str)
        else {
            return Ok(());
        };
        self.add_repo_path(RepoPathBucket::Authored, reference, true)
    }

    fn finish(self) -> FoundationPhysicalSelection {
        FoundationPhysicalSelection {
            limits: self.limits,
            authored_paths: self.authored_paths.into_iter().collect(),
            private_paths: self.private_paths.into_iter().collect(),
            private_prefixes: self.private_prefixes.into_iter().collect(),
            artifact_paths: self.artifact_paths.into_iter().collect(),
            payload_paths: self.payload_paths.into_iter().collect(),
            resolved_source_directories: self.resolved_source_directories,
            auxiliary_query_documents: self.auxiliary_query_documents.into_iter().collect(),
            query_content_bindings: Vec::new(),
            cost: FoundationSelectionCost {
                selector_documents: self.selector_documents,
                source_bytes_read: self.source_bytes_read,
                output_entries: self.output_entries,
                retained_state_bytes: self.retained_state_bytes,
                peak_state_bytes: self.peak_state_bytes,
            },
        }
    }
}

/// Derive bounded physical selectors from the captured current source itself.
/// A missing current document is never converted into an absent physical fact;
/// only the downstream physical snapshot observes existence/type/fixity.
pub(crate) fn select(
    captured: &FoundationCapturedCut,
    limits: FoundationSelectionLimits,
    cancelled: &AtomicBool,
) -> Result<FoundationPhysicalSelection, ItemRefusal> {
    select_input(&SelectionInput::Cut(captured.cut()), limits, cancelled)
}

pub(crate) fn select_candidate(
    input: &dyn tos_validation::record_biblio_cut::SourceCutInput,
    limits: FoundationSelectionLimits,
    cancelled: &AtomicBool,
) -> Result<FoundationPhysicalSelection, ItemRefusal> {
    select_input(&SelectionInput::Candidate(input), limits, cancelled)
}

fn select_input(
    input: &SelectionInput<'_>,
    limits: FoundationSelectionLimits,
    cancelled: &AtomicBool,
) -> Result<FoundationPhysicalSelection, ItemRefusal> {
    let mut builder = Builder::new(limits, cancelled)?;

    builder.add_repo_path(RepoPathBucket::PrivatePrefix, PRIVATE_ROUTE, false)?;
    builder.add_repo_path(RepoPathBucket::Authored, PRIVATE_ROUTE_CARD, true)?;
    builder.add_resolved_directory(PRIVATE_ROUTE_CARD, PRIVATE_ROUTE)?;
    input.walk(limits, cancelled, &mut |path| {
        builder.checkpoint()?;
        if let Some(root) = gold_root(path) {
            let Some(local_content) = join_repo_path(root, "local-content") else {
                return Ok(());
            };
            if !builder.private_prefixes.contains(&local_content) {
                builder.add_repo_path(RepoPathBucket::PrivatePrefix, &local_content, false)?;
                if let Some(route_card) = join_repo_path(&local_content, "README.md") {
                    builder.add_repo_path(RepoPathBucket::Authored, &route_card, true)?;
                    builder.add_resolved_directory(&route_card, &local_content)?;
                }
                // Gold rules have these exact static local-content inputs.
                for reference in [
                    "local-content/semantic-source-observation/initial-v1/work-recurrence-bundle.json",
                    "local-content/translation/research-inputs/za-i-vorrede-1-opening-sentence.v1.json",
                    "local-content/retrieval/queries.v1.json",
                ] {
                    builder.add_private_ref(root, reference)?;
                }
            }
        }
            Ok(())
    })?;

    input.walk(limits, cancelled, &mut |path| {
        builder.checkpoint()?;
        if path.starts_with(ARTIFACTS) && path.ends_with("/artifact-witness.json") {
            builder.add_repo_path(RepoPathBucket::Authored, path, true)?;
            let parent = path
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or("");
            for companion in ARTIFACT_COMPANIONS {
                if let Some(companion_path) = join_repo_path(parent, companion) {
                    builder.add_repo_path(RepoPathBucket::Authored, &companion_path, true)?;
                }
            }
        }
        Ok(())
    })?;

    // Exact fixed authored inputs whose physical bytes/types are directly
    // consumed by the maintained companion and transfer-map predicates.
    for path in [PRIVATE_HANDOFF, TRANSFER_CROSSWALK] {
        if input.present(path, limits, cancelled)? {
            builder.add_repo_path(RepoPathBucket::Authored, path, true)?;
        }
    }
    for root in HIERARCHICAL_TARGET_ROOTS {
        for name in [
            "hierarchical-numbered-unit-page-map.json",
            "transfer-candidate-page-crosswalk.v1.json",
        ] {
            if let Some(path) = join_repo_path(root, name) {
                if input.present(&path, limits, cancelled)? {
                    builder.add_repo_path(RepoPathBucket::Authored, &path, true)?;
                }
            }
        }
    }

    // Read selector documents in exact current membership order, with bounded
    // per-member and aggregate reads. Malformed/non-object content is left to
    // its source owner; fields not represented as strings are not selectors.
    input.walk(limits, cancelled, &mut |path| {
        builder.checkpoint()?;
        if !is_selector_document(path, &builder.private_prefixes, input, limits, cancelled)? {
            return Ok(());
        }
        let Some(value) = builder.read_json(input, path)? else {
            return Ok(());
        };
        if !value.is_object() {
            builder.document_state_bytes = 0;
            return Ok(());
        }

        if path.starts_with(SOURCE_HOME) && path.ends_with("/item.manifest.json") {
            let parent = path
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or("");
            for entry in value
                .get("payload_files")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|entry| entry.is_object())
            {
                builder.checkpoint()?;
                let Some(relative) = entry.get("relative_path").and_then(Value::as_str) else {
                    continue;
                };
                if let Some(full) = join_repo_path(parent, relative) {
                    if safe_payload_path(&full) {
                        builder.add_repo_path(RepoPathBucket::Payload, &full, true)?;
                    }
                }
            }
        }

        if path.starts_with(ARTIFACTS) && path.ends_with("/representation.json") {
            builder.add_repo_path(RepoPathBucket::Authored, path, true)?;
            let parent = path
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or("");
            let payload = value.get("payload").unwrap_or(&Value::Null);
            if let Some(relative) = payload.get("relative_path").and_then(Value::as_str) {
                if let Some(full) = join_repo_path(parent, relative) {
                    if safe_payload_path(&full) {
                        builder.add_repo_path(RepoPathBucket::Payload, &full, true)?;
                    }
                }
            }
            if let Some(reference) = value.get("rights_ref").and_then(Value::as_str) {
                builder.add_repo_path(RepoPathBucket::Authored, reference, true)?;
            }
            if let Some(reference) = value.get("discovery_ref").and_then(Value::as_str) {
                builder.add_repo_path(RepoPathBucket::Authored, reference, true)?;
            }
        }

        if path.starts_with(COMPOSITES) && path.ends_with("/representation.json") {
            builder.add_repo_path(RepoPathBucket::Authored, path, true)?;
            let parent = path
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or("");
            let payload = value.get("payload").unwrap_or(&Value::Null);
            if let Some(relative) = payload.get("relative_path").and_then(Value::as_str) {
                let payload_name = relative.strip_prefix("payload/").unwrap_or("");
                if let Some(full) = join_repo_path(parent, relative) {
                    if !payload_name.is_empty()
                        && !payload_name.contains('/')
                        && safe_payload_path(&full)
                    {
                        builder.add_repo_path(RepoPathBucket::Payload, &full, true)?;
                    }
                }
            }
            if let Some(reference) = value.get("rights_ref").and_then(Value::as_str) {
                builder.add_repo_path(RepoPathBucket::Authored, reference, true)?;
            }
            if let Some(reference) = value.get("discovery_ref").and_then(Value::as_str) {
                builder.add_repo_path(RepoPathBucket::Authored, reference, true)?;
            }
            builder.add_repo_path(
                RepoPathBucket::Authored,
                "ToS/source-witnesses/discovery/provenance.jsonl",
                true,
            )?;
        }

        if path == PRIVATE_HANDOFF {
            let destination = value
                .get("destination")
                .and_then(|destination| destination.get("artifact_path"))
                .and_then(Value::as_str);
            if let Some(destination) = destination {
                builder.add_repo_path(RepoPathBucket::Authored, destination, true)?;
                builder.add_repo_path(RepoPathBucket::Authored, MANUAL_LEDGER, true)?;
                builder.add_repo_path(RepoPathBucket::Authored, MANUAL_LEDGER_PROVENANCE, true)?;
            }
        }

        if path == TRANSFER_CROSSWALK
            || is_hierarchical_target_path(path, "transfer-candidate-page-crosswalk.v1.json")
        {
            if let Some(inputs) = value.get("inputs").and_then(Value::as_object) {
                for binding in inputs.values() {
                    builder.add_ref_from_binding(Some(binding))?;
                }
            }
        }
        if is_hierarchical_target_path(path, "hierarchical-numbered-unit-page-map.json") {
            for field in ["inventory", "work_boundary"] {
                builder.add_ref_from_binding(value.get(field))?;
            }
        }

        if is_artifact_request(path, input, limits, cancelled)? {
            if let Some(bindings) = value.get("source_bindings") {
                for field in ["rights_ref", "discovery_ref", "research_ref"] {
                    builder.add_ref_from_binding(bindings.get(field))?;
                }
            }
        }

        if let Some(root) = gold_root_for_document(path, &builder.private_prefixes) {
            select_gold_document(&mut builder, root, path, &value)?;
            if path.ends_with("antonovsky-2007-1911-opening-sentence-collation.plan.v1.json") {
                for reference in foundation_lab_private_output_refs(Some(&value), None) {
                    builder.checkpoint()?;
                    builder.add_private_ref(root, reference)?;
                }
            }
            if path.ends_with("authored-canon-evidence-bridge.plan.v1.json") {
                for reference in foundation_lab_private_output_refs(None, Some(&value)) {
                    builder.checkpoint()?;
                    builder.add_private_ref(root, reference)?;
                }
            }
        }
        drop(value);
        builder.document_state_bytes = 0;
        Ok(())
    })?;

    Ok(builder.finish())
}

/// Read only the retrieval query JSON files named by exact authored plans.
/// This second selector phase shares the first phase's aggregate byte, state,
/// document and output ceilings. It runs against the caller's already-held
/// ToS root before the one final physical snapshot is observed.
#[cfg(target_os = "linux")]
pub(crate) fn read_auxiliary_query_documents(
    selection: &mut FoundationPhysicalSelection,
    sources: &mut RouteSources,
    limits: FoundationSelectionLimits,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    if selection.limits != limits {
        return Err(ItemRefusal::Unsupported(
            "foundation selector limits changed between phases".into(),
        ));
    }
    let custody = sources.root_custody();
    let mut aggregate_read_bytes =
        usize::try_from(selection.cost.source_bytes_read).map_err(|_| ItemRefusal::Budget)?;
    let maximum_read_bytes =
        usize::try_from(limits.max_total_read_bytes).map_err(|_| ItemRefusal::Budget)?;

    let selected_query_documents = std::mem::take(&mut selection.auxiliary_query_documents);
    for path in &selected_query_documents {
        selection_checkpoint(limits, cancelled)?;
        sources
            .verify_custody(&custody)
            .map_err(|_| source_operation_refusal(limits, cancelled))?;
        let next_document_count = selection
            .cost
            .selector_documents
            .checked_add(1)
            .filter(|count| *count <= limits.max_selector_documents)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "foundation-selector-documents",
                used: Some(selection.cost.selector_documents.saturating_add(1) as u64),
                limit: Some(limits.max_selector_documents as u64),
            })?;

        let Some(root) = gold_root(path) else {
            return Err(ItemRefusal::Source(
                "foundation auxiliary query document has no selected gold root".into(),
            ));
        };
        let local_content = join_repo_path(root, "local-content").ok_or_else(|| {
            ItemRefusal::Source("foundation auxiliary query document path refused".into())
        })?;
        if !path_within(path, &local_content) || path == &local_content {
            return Err(ItemRefusal::Source(
                "foundation auxiliary query document escaped selected local content".into(),
            ));
        }

        let before = sources
            .resolve_selected_target(path, Some(&local_content))
            .map_err(|_| source_operation_refusal(limits, cancelled))?;
        selection_checkpoint(limits, cancelled)?;
        let (target_path, before_metadata, before_topology) = match before {
            RouteResolvedTarget::OutsideSelectedRoot => {
                selection.cost.selector_documents = next_document_count;
                push_query_binding(selection, path, None, None, limits, 0)?;
                continue;
            }
            RouteResolvedTarget::Inside {
                relative_path,
                metadata,
                topology_stamp,
            } => (relative_path, metadata, topology_stamp.to_hex()),
        };
        let Some(metadata) = before_metadata.filter(|metadata| metadata.is_file()) else {
            selection.cost.selector_documents = next_document_count;
            push_query_binding(selection, path, None, None, limits, 0)?;
            continue;
        };

        let observed_size = metadata.len();
        if observed_size > limits.max_member_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "foundation-selector-member-bytes",
                used: Some(observed_size),
                limit: Some(limits.max_member_bytes),
            });
        }
        let size = usize::try_from(observed_size).map_err(|_| ItemRefusal::Budget)?;
        let next_read_bytes = aggregate_read_bytes
            .checked_add(size)
            .filter(|bytes| *bytes <= maximum_read_bytes)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "foundation-selector-total-read-bytes",
                used: Some(aggregate_read_bytes.saturating_add(size) as u64),
                limit: Some(limits.max_total_read_bytes),
            })?;
        let parse_bound = size
            .checked_mul(JSON_VALUE_BYTES_PER_SOURCE_BYTE)
            .and_then(|bytes| bytes.checked_add(JSON_PARSE_FIXED_OVERHEAD))
            .ok_or(ItemRefusal::Budget)?;
        let read_peak = selection
            .cost
            .retained_state_bytes
            .checked_add(parse_bound)
            .ok_or(ItemRefusal::Budget)?;
        if read_peak > limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "foundation-selector-state-bytes",
                used: Some(read_peak as u64),
                limit: Some(limits.max_state_bytes as u64),
            });
        }

        let (raw, read_metadata) = sources
            .bounded_metadata_bytes(
                &target_path,
                size,
                &mut aggregate_read_bytes,
                maximum_read_bytes,
            )
            .map_err(|_| source_operation_refusal(limits, cancelled))?;
        selection_checkpoint(limits, cancelled)?;
        sources
            .verify_custody(&custody)
            .map_err(|_| source_operation_refusal(limits, cancelled))?;
        if raw.len() != size || read_metadata.len() != observed_size {
            return Err(ItemRefusal::Source(
                "foundation auxiliary query document changed during read".into(),
            ));
        }
        let digest = Digest256::of_bytes(&raw).to_hex();
        let parsed = serde_json::from_slice::<Value>(&raw).ok();
        drop(raw);

        let after = sources
            .resolve_selected_target(path, Some(&local_content))
            .map_err(|_| source_operation_refusal(limits, cancelled))?;
        selection_checkpoint(limits, cancelled)?;
        match after {
            RouteResolvedTarget::Inside {
                relative_path,
                metadata: Some(after_metadata),
                topology_stamp,
            } if relative_path == target_path
                && topology_stamp.to_hex() == before_topology
                && same_metadata_identity(&metadata, &after_metadata)
                && same_metadata_identity(&metadata, &read_metadata) => {}
            _ => {
                return Err(ItemRefusal::Source(
                    "foundation auxiliary query document target changed".into(),
                ));
            }
        }
        sources
            .verify_custody(&custody)
            .map_err(|_| source_operation_refusal(limits, cancelled))?;

        aggregate_read_bytes = next_read_bytes;
        selection.cost.selector_documents = next_document_count;
        selection.cost.source_bytes_read =
            u64::try_from(aggregate_read_bytes).map_err(|_| ItemRefusal::Budget)?;
        selection.cost.peak_state_bytes = selection.cost.peak_state_bytes.max(read_peak);

        let retained_document_state = if let Some(value) = parsed.as_ref() {
            add_query_content_paths(selection, root, value, limits, cancelled, parse_bound)?;
            parse_bound
        } else {
            0
        };
        push_query_binding(
            selection,
            path,
            Some(digest),
            Some(observed_size),
            limits,
            retained_document_state,
        )?;
    }
    selection.auxiliary_query_documents = selected_query_documents;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn read_auxiliary_query_documents(
    _selection: &mut FoundationPhysicalSelection,
    _sources: &mut RouteSources,
    _limits: FoundationSelectionLimits,
    _cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    Err(ItemRefusal::Unsupported(
        "foundation auxiliary query source resolution requires Linux".into(),
    ))
}

/// Join every private retrieval document read against the actual final
/// physical snapshot. A missing or unsupported binding is tolerated only
/// when that snapshot does not report a regular file at the selected path.
pub(crate) fn verify_query_content_bindings(
    selection: &FoundationPhysicalSelection,
    physical: &SourcePhysicalFacts,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    selection_checkpoint(selection.limits, cancelled)?;
    if selection.auxiliary_query_documents.len() != selection.query_content_bindings.len() {
        return Err(ItemRefusal::Source(
            "foundation query bindings do not cover the selected documents".into(),
        ));
    }
    for (requested, binding) in selection
        .auxiliary_query_documents
        .iter()
        .zip(&selection.query_content_bindings)
    {
        selection_checkpoint(selection.limits, cancelled)?;
        if requested != &binding.path {
            return Err(ItemRefusal::Source(
                "foundation query bindings do not cover the selected documents".into(),
            ));
        }
        let root = gold_root(&binding.path).ok_or_else(|| {
            ItemRefusal::Source("foundation query binding has no selected gold root".into())
        })?;
        let local_content = join_repo_path(root, "local-content")
            .ok_or_else(|| ItemRefusal::Source("foundation query binding path refused".into()))?;
        let facts = physical.private_paths.get(&binding.path).ok_or_else(|| {
            ItemRefusal::Source("foundation query binding lacks final physical path facts".into())
        })?;

        match (binding.sha256.as_deref(), binding.byte_size) {
            (Some(expected_sha256), Some(expected_size)) => {
                let matches = if facts.symlink {
                    matches!(
                        facts.resolved_target.as_ref(),
                        Some(PhysicalResolvedTargetFacts::InsideSelectedRoot {
                            relative_target,
                            exists: true,
                            regular_file: true,
                            byte_size: Some(actual_size),
                            sha256: Some(actual_sha256),
                            ..
                        }) if path_within(relative_target, &local_content)
                            && *actual_size == expected_size
                            && actual_sha256.as_str() == expected_sha256
                    )
                } else {
                    facts.exists
                        && facts.regular_file
                        && !facts.directory
                        && facts.byte_size == Some(expected_size)
                        && facts.sha256.as_deref() == Some(expected_sha256)
                };
                if !matches {
                    return Err(ItemRefusal::Source(
                        "foundation query binding differs from final physical content".into(),
                    ));
                }
            }
            (None, None) => {
                let direct_regular =
                    facts.exists && facts.regular_file && !facts.symlink && !facts.directory;
                let resolved_regular = matches!(
                    facts.resolved_target.as_ref(),
                    Some(PhysicalResolvedTargetFacts::InsideSelectedRoot {
                        relative_target,
                        exists: true,
                        regular_file: true,
                        ..
                    }) if path_within(relative_target, &local_content)
                );
                if direct_regular || resolved_regular {
                    return Err(ItemRefusal::Source(
                        "foundation query binding omitted a final regular file".into(),
                    ));
                }
            }
            _ => {
                return Err(ItemRefusal::Source(
                    "foundation query binding has incomplete content identity".into(),
                ));
            }
        }
    }
    selection_checkpoint(selection.limits, cancelled)
}

fn selection_checkpoint(
    limits: FoundationSelectionLimits,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(ItemRefusal::Source(
            "foundation path selector cancelled".into(),
        ));
    }
    if Instant::now() >= limits.deadline {
        return Err(ItemRefusal::Deadline);
    }
    Ok(())
}

fn source_operation_refusal(
    limits: FoundationSelectionLimits,
    cancelled: &AtomicBool,
) -> ItemRefusal {
    if cancelled.load(Ordering::Relaxed) {
        ItemRefusal::Source("foundation path selector cancelled".into())
    } else if Instant::now() >= limits.deadline {
        ItemRefusal::Deadline
    } else {
        ItemRefusal::Source("foundation held-source operation refused".into())
    }
}

fn add_query_content_paths(
    selection: &mut FoundationPhysicalSelection,
    gold_root: &str,
    value: &Value,
    limits: FoundationSelectionLimits,
    cancelled: &AtomicBool,
    document_state_bytes: usize,
) -> Result<(), ItemRefusal> {
    for query in value
        .get("queries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        selection_checkpoint(limits, cancelled)?;
        let Some(reference) = query.get("local_content_ref").and_then(Value::as_str) else {
            continue;
        };
        let Some(path) = private_ref_path(gold_root, reference) else {
            continue;
        };
        match selection.private_paths.binary_search(&path) {
            Ok(_) => continue,
            Err(index) => {
                let next_output_entries = selection
                    .cost
                    .output_entries
                    .checked_add(1)
                    .filter(|count| *count <= limits.max_output_entries)
                    .ok_or(ItemRefusal::BudgetCheck {
                        check: "foundation-selector-output-entries",
                        used: Some(selection.cost.output_entries.saturating_add(1) as u64),
                        limit: Some(limits.max_output_entries as u64),
                    })?;
                let row_bytes = path
                    .len()
                    .checked_add(size_of::<String>())
                    .and_then(|bytes| bytes.checked_add(32 * size_of::<usize>()))
                    .ok_or(ItemRefusal::Budget)?;
                let next_retained = selection
                    .cost
                    .retained_state_bytes
                    .checked_add(row_bytes)
                    .ok_or(ItemRefusal::Budget)?;
                let insertion_peak = next_retained
                    .checked_add(document_state_bytes)
                    .ok_or(ItemRefusal::Budget)?;
                if insertion_peak > limits.max_state_bytes {
                    return Err(ItemRefusal::BudgetCheck {
                        check: "foundation-selector-state-bytes",
                        used: Some(insertion_peak as u64),
                        limit: Some(limits.max_state_bytes as u64),
                    });
                }
                selection
                    .private_paths
                    .try_reserve(1)
                    .map_err(|_| ItemRefusal::Budget)?;
                let needs_resolved_scope =
                    !selection.resolved_source_directories.contains_key(&path);
                let resolved_path = needs_resolved_scope.then(|| path.clone());
                selection.private_paths.insert(index, path);
                selection.cost.output_entries = next_output_entries;
                selection.cost.retained_state_bytes = next_retained;
                selection.cost.peak_state_bytes =
                    selection.cost.peak_state_bytes.max(insertion_peak);
                if let Some(resolved_path) = resolved_path {
                    add_resolved_directory_to_selection(
                        selection,
                        resolved_path,
                        gold_root,
                        limits,
                        document_state_bytes,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn push_query_binding(
    selection: &mut FoundationPhysicalSelection,
    path: &str,
    sha256: Option<String>,
    byte_size: Option<u64>,
    limits: FoundationSelectionLimits,
    document_state_bytes: usize,
) -> Result<(), ItemRefusal> {
    let next_output_entries = selection
        .cost
        .output_entries
        .checked_add(1)
        .filter(|count| *count <= limits.max_output_entries)
        .ok_or(ItemRefusal::BudgetCheck {
            check: "foundation-selector-output-entries",
            used: Some(selection.cost.output_entries.saturating_add(1) as u64),
            limit: Some(limits.max_output_entries as u64),
        })?;
    let row_bytes = size_of::<FoundationQueryContentBinding>()
        .checked_add(path.len())
        .and_then(|bytes| bytes.checked_add(sha256.as_ref().map_or(0, String::len)))
        .and_then(|bytes| bytes.checked_add(32 * size_of::<usize>()))
        .ok_or(ItemRefusal::Budget)?;
    let next_retained = selection
        .cost
        .retained_state_bytes
        .checked_add(row_bytes)
        .ok_or(ItemRefusal::Budget)?;
    let peak = next_retained
        .checked_add(document_state_bytes)
        .ok_or(ItemRefusal::Budget)?;
    if peak > limits.max_state_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "foundation-selector-state-bytes",
            used: Some(peak as u64),
            limit: Some(limits.max_state_bytes as u64),
        });
    }
    selection
        .query_content_bindings
        .try_reserve(1)
        .map_err(|_| ItemRefusal::Budget)?;
    selection
        .query_content_bindings
        .push(FoundationQueryContentBinding {
            path: path.to_owned(),
            sha256,
            byte_size,
        });
    selection.cost.output_entries = next_output_entries;
    selection.cost.retained_state_bytes = next_retained;
    selection.cost.peak_state_bytes = selection.cost.peak_state_bytes.max(peak);
    Ok(())
}

fn add_resolved_directory_to_selection(
    selection: &mut FoundationPhysicalSelection,
    path: String,
    gold_root: &str,
    limits: FoundationSelectionLimits,
    document_state_bytes: usize,
) -> Result<(), ItemRefusal> {
    let local_content = join_repo_path(gold_root, "local-content")
        .ok_or_else(|| ItemRefusal::Source("foundation private target scope refused".into()))?;
    if !path_within(&path, &local_content) || path == local_content {
        return Err(ItemRefusal::Source(
            "foundation private target escaped selected local content".into(),
        ));
    }
    if selection.resolved_source_directories.contains_key(&path) {
        return Ok(());
    }
    let next_output_entries = selection
        .cost
        .output_entries
        .checked_add(1)
        .filter(|count| *count <= limits.max_output_entries)
        .ok_or(ItemRefusal::BudgetCheck {
            check: "foundation-selector-output-entries",
            used: Some(selection.cost.output_entries.saturating_add(1) as u64),
            limit: Some(limits.max_output_entries as u64),
        })?;
    let row_bytes = path
        .len()
        .checked_add(size_of::<String>())
        .and_then(|bytes| bytes.checked_add(32 * size_of::<usize>()))
        .and_then(|bytes| bytes.checked_add(local_content.len()))
        .and_then(|bytes| bytes.checked_add(size_of::<String>()))
        .ok_or(ItemRefusal::Budget)?;
    let next_retained = selection
        .cost
        .retained_state_bytes
        .checked_add(row_bytes)
        .ok_or(ItemRefusal::Budget)?;
    let peak = next_retained
        .checked_add(document_state_bytes)
        .ok_or(ItemRefusal::Budget)?;
    if peak > limits.max_state_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "foundation-selector-state-bytes",
            used: Some(peak as u64),
            limit: Some(limits.max_state_bytes as u64),
        });
    }
    selection
        .resolved_source_directories
        .insert(path, local_content);
    selection.cost.output_entries = next_output_entries;
    selection.cost.retained_state_bytes = next_retained;
    selection.cost.peak_state_bytes = selection.cost.peak_state_bytes.max(peak);
    Ok(())
}

#[cfg(target_os = "linux")]
fn same_metadata_identity(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    fn stamp(metadata: &std::fs::Metadata) -> (u64, u64, u64, u32, u64, i64, i64, i64, i64) {
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mode(),
            metadata.nlink(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        )
    }
    stamp(left) == stamp(right)
}

#[cfg(not(target_os = "linux"))]
fn same_metadata_identity(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    left.len() == right.len() && left.is_file() == right.is_file()
}

fn select_gold_document(
    builder: &mut Builder<'_>,
    root: &str,
    document_path: &str,
    value: &Value,
) -> Result<(), ItemRefusal> {
    if value.get("candidate_target_units").is_some() {
        for candidate in value
            .get("candidate_target_units")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            builder.checkpoint()?;
            if let Some(reference) = candidate.get("source_content_ref").and_then(Value::as_str) {
                builder.add_private_ref(root, reference)?;
            }
        }
    }
    if let Some(reference) = value.get("query_content_ref").and_then(Value::as_str) {
        if document_path.ends_with(RETRIEVAL_DOCUMENT) {
            builder.add_query_content_ref(root, reference)?;
        } else {
            builder.add_private_ref(root, reference)?;
        }
    }
    if let Some(reference) = value
        .pointer("/render_specification/render_manifest_ref")
        .and_then(Value::as_str)
    {
        builder.add_private_ref(root, reference)?;
    }
    if let Some(tasks) = value.get("tasks").and_then(Value::as_array) {
        for task in tasks {
            builder.checkpoint()?;
            if let Some(reference) = task.get("local_content_ref").and_then(Value::as_str) {
                builder.add_private_ref(root, reference)?;
            }
        }
    }
    if let Some(queries) = value.get("queries").and_then(Value::as_array) {
        for query in queries {
            builder.checkpoint()?;
            if let Some(reference) = query.get("local_content_ref").and_then(Value::as_str) {
                builder.add_private_ref(root, reference)?;
            }
        }
    }
    if let Some(page_corpus) = value.get("page_image_corpus") {
        if let Some(reference) = page_corpus
            .get("render_artifact_ref")
            .and_then(Value::as_str)
        {
            builder.add_artifact_path(reference)?;
        }
    }
    for control in value
        .get("fixed_controls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        builder.checkpoint()?;
        if let Some(run_ref) = control.get("run_ref").and_then(Value::as_str) {
            if !run_ref.starts_with('/')
                && run_ref.len() <= MAX_PATH_BYTES
                && run_ref.split('/').count() <= MAX_PATH_COMPONENTS
                && RelativePath::parse(run_ref).is_ok()
            {
                builder.add_artifact_path(run_ref)?;
                let parent = run_ref.trim_end_matches('/');
                if let Some(receipt) = join_repo_path(parent, "run.receipt.json") {
                    builder.add_artifact_path(&receipt)?;
                }
            }
        }
    }
    Ok(())
}

fn gold_root(path: &str) -> Option<&str> {
    if path.len() > MAX_PATH_BYTES || !path.starts_with(SOURCE_HOME) {
        return None;
    }
    let mut offset = 0usize;
    let components = path.split('/');
    for component in components {
        if component == "gold-sets" {
            let root_start = offset;
            let remaining = path.get(root_start..)?;
            let tail = remaining.strip_prefix("gold-sets/")?;
            let set_id = tail.split('/').next()?;
            if set_id.is_empty() {
                return None;
            }
            let root_end = root_start
                .checked_add("gold-sets/".len())?
                .checked_add(set_id.len())?;
            return path.get(..root_end);
        }
        offset = offset.checked_add(component.len())?.checked_add(1)?;
    }
    None
}

fn gold_root_for_document<'a>(
    path: &'a str,
    private_prefixes: &BTreeSet<String>,
) -> Option<&'a str> {
    let root = gold_root(path)?;
    let local_content = join_repo_path(root, "local-content")?;
    private_prefixes.contains(&local_content).then_some(root)
}

fn is_selector_document(
    path: &str,
    private_prefixes: &BTreeSet<String>,
    input: &SelectionInput<'_>,
    limits: FoundationSelectionLimits,
    cancelled: &AtomicBool,
) -> Result<bool, ItemRefusal> {
    if path.starts_with(SOURCE_HOME) && path.ends_with("/item.manifest.json") {
        return Ok(true);
    }
    if path.starts_with(ARTIFACTS)
        && (path.ends_with("/artifact-witness.json")
            || path.ends_with("/representation.json")
            || is_artifact_request(path, input, limits, cancelled)?)
    {
        return Ok(true);
    }
    if path.starts_with(COMPOSITES) && path.ends_with("/representation.json") {
        return Ok(true);
    }
    if path == PRIVATE_HANDOFF || path == TRANSFER_CROSSWALK {
        return Ok(true);
    }
    if is_hierarchical_target_path(path, "hierarchical-numbered-unit-page-map.json")
        || is_hierarchical_target_path(path, "transfer-candidate-page-crosswalk.v1.json")
    {
        return Ok(true);
    }
    if let Some(root) = gold_root_for_document(path, private_prefixes) {
        let Some(tail) = path
            .strip_prefix(root)
            .and_then(|tail| tail.strip_prefix('/'))
        else {
            return Ok(false);
        };
        return Ok(!tail.contains('/')
            && (GOLD_DOCUMENTS.contains(&tail) || LAB_DOCUMENTS.contains(&tail)));
    }
    Ok(false)
}

fn is_artifact_request(
    path: &str,
    input: &SelectionInput<'_>,
    limits: FoundationSelectionLimits,
    cancelled: &AtomicBool,
) -> Result<bool, ItemRefusal> {
    if !path.starts_with(ARTIFACTS) || !path.ends_with("/source-create-request.json") {
        return Ok(false);
    }
    let Some((parent, _)) = path.rsplit_once('/') else {
        return Ok(false);
    };
    match join_repo_path(parent, "artifact-witness.json") {
        Some(path) => input.present(&path, limits, cancelled),
        None => Ok(false),
    }
}

fn safe_payload_path(path: &str) -> bool {
    path.len() <= MAX_PATH_BYTES
        && path.split('/').count() <= MAX_PATH_COMPONENTS
        && path.starts_with(SOURCE_HOME)
        && RelativePath::parse(path).is_ok()
}

fn is_hierarchical_target_path(path: &str, basename: &str) -> bool {
    HIERARCHICAL_TARGET_ROOTS
        .iter()
        .any(|root| join_repo_path(root, basename).as_deref() == Some(path))
}

fn join_repo_path(parent: &str, child: &str) -> Option<String> {
    let total_bytes = parent.len().checked_add(1)?.checked_add(child.len())?;
    if total_bytes > MAX_PATH_BYTES
        || parent
            .split('/')
            .count()
            .checked_add(child.split('/').count())?
            > MAX_PATH_COMPONENTS
    {
        return None;
    }
    Some(format!("{parent}/{child}"))
}

fn private_ref_path(root: &str, reference: &str) -> Option<String> {
    if reference.starts_with('/') || reference.contains('\\') {
        return None;
    }
    let path = if reference.starts_with("ToS/") {
        if reference.len() > MAX_PATH_BYTES {
            return None;
        }
        reference.to_owned()
    } else {
        join_repo_path(root, reference)?
    };
    let local_content = join_repo_path(root, "local-content")?;
    let beneath_local_content = path
        .strip_prefix(&local_content)
        .is_some_and(|tail| tail.starts_with('/'));
    (beneath_local_content
        && path.len() <= MAX_PATH_BYTES
        && path.split('/').count() <= MAX_PATH_COMPONENTS
        && RelativePath::parse(&path).is_ok())
    .then_some(path)
}

fn query_content_path(root: &str, reference: &str) -> Option<String> {
    if reference.starts_with('/') || reference.starts_with("ToS/") || reference.contains('\\') {
        return None;
    }
    let path = join_repo_path(root, reference)?;
    let local_content = join_repo_path(root, "local-content")?;
    let beneath_local_content = path
        .strip_prefix(&local_content)
        .is_some_and(|tail| tail.starts_with('/'));
    (beneath_local_content
        && path.split('/').count() <= MAX_PATH_COMPONENTS
        && RelativePath::parse(&path).is_ok())
    .then_some(path)
}

fn path_within(path: &str, root: &str) -> bool {
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|tail| tail.starts_with('/'))
}
