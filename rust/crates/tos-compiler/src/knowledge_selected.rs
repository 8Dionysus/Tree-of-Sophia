//! Cold admission of one owner-selected, immutable full knowledge SQLite file.
//! Source admission, immutable inode custody and current disclosure rights are
//! independent owner obligations. No model path is reopened after admission.

use crate::{
    Error, MAX_POSTING_DELTA_BYTES, MAX_POSTINGS_PER_BLOCK, Result, decode_posting_block,
    knowledge_stage, safe_open, stream_digest,
};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    fs::File,
    io::{Read, Seek},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonValue,
    canonical_bytes_v1, parse_json,
};

pub use crate::knowledge_seal::KNOWLEDGE_MODEL_ABI;
// Query vocabulary's semantic primitive profile is distinct from the
// search-index Unicode normalization profile.
pub const KNOWLEDGE_QUERY_PRIMITIVE_PROFILE: &str = "tos-query-primitives-v1";

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedSourceScope {
    pub source_graph: String,
    pub input_role: String,
    pub adapter_profile: String,
    pub node_count: u64,
    pub relation_count: u64,
    /// Independent owner-expected output roots, not read back from this DB.
    pub node_root_sha256: String,
    pub relation_root_sha256: String,
}

/// Every field comes from the independently sealed selection and producer
/// receipt. An SQLite metadata value or selected.json cannot manufacture it.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeSelectedExpectation {
    pub model_sha256: String,
    pub model_size_bytes: u64,
    pub owner_receipt_id: String,
    pub model_abi: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed_source_root_sha256: Option<String>,
    pub descriptor_sha256: String,
    pub descriptor_version: u64,
    pub semantic_primitive_profile: String,
    pub source_cut: String,
    pub through_commit_seq: u64,
    pub membership_root: String,
    pub entity_registry_id: String,
    pub entity_registry_version: String,
    pub entity_registry_sha256: String,
    pub relation_registry_id: String,
    pub relation_registry_version: String,
    pub relation_registry_sha256: String,
    pub graph_root_sha256: String,
    pub navigation_original_root_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub philosophy_original_root_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corpus_original_root_sha256: Option<String>,
    pub catalog_packet_sha256: String,
    pub catalog_index_root_sha256: String,
    pub source_scope_root_sha256: String,
    pub search_index_root_sha256: String,
    pub node_count: u64,
    pub relation_count: u64,
    pub index_generation: String,
    pub route_map_version: String,
    pub reader_abi: String,
    pub authority_boundary: String,
    pub source_scopes: Vec<ExpectedSourceScope>,
    pub complete: bool,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColdOpenLimits {
    pub max_file_bytes: u64,
    pub max_vm_steps: u64,
    pub sqlite_cache_kib: u64,
    /// Per verified table scan. The total scan budget is independently
    /// bounded by `max_work_bytes` and `max_vm_steps`.
    pub max_rows: u64,
    pub max_work_bytes: u64,
    pub max_row_bytes: usize,
    pub max_metadata_bytes: usize,
    pub max_sources: usize,
}

impl ColdOpenLimits {
    /// Maximum metadata envelope accepted by the selected cold reader.
    pub const MAX_METADATA_BYTES: usize = 256 * 1024;
}

/// This owner-held guard must maintain a kernel-enforced immutable custody
/// lease (for example sealed read-only generation or fs-verity) for the full
/// reader lifetime. FD pin blocks path replacement, not in-place same-inode
/// mutation; a metadata-only or no-op implementation is insufficient.
pub trait ImmutableKnowledgeCustody: Send + Sync {
    /// This is a bounded retained-lease/fence check on every warm seek.
    /// The owner must not rehash or rescan the model here: the full SHA is
    /// charged once at cold admission, and QRY budgets SQLite work separately.
    fn verify(&self, pinned: &File, expected: &KnowledgeSelectedExpectation) -> Result<()>;
    /// The host preadmits this cold scan and confines SQLite temp/heap under
    /// independently enforced process/filesystem quotas. Size samples after
    /// a statement cannot cap its peak spill or memory use.
    fn verify_cold_resources(&self, limits: ColdOpenLimits) -> Result<()>;
}

#[derive(Clone)]
enum CustodyRef<'a> {
    Borrowed(&'a dyn ImmutableKnowledgeCustody),
    Owned(Arc<dyn ImmutableKnowledgeCustody>),
}
impl<'a> std::ops::Deref for CustodyRef<'a> {
    type Target = dyn ImmutableKnowledgeCustody + 'a;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(value) => *value,
            Self::Owned(value) => value.as_ref(),
        }
    }
}

pub struct VerifiedKnowledgeModel<'a> {
    connection: tos_source_store::PinnedSqliteConnection,
    pinned: File,
    selection: KnowledgeSelectedExpectation,
    source_basis: crate::KnowledgeSourceBasis,
    navigation_original: Option<crate::NavigationOriginalReceipt>,
    philosophy_original: Option<crate::PhilosophyOriginalReceipt>,
    corpus_original: Option<crate::CorpusOriginalReceipt>,
    custody: CustodyRef<'a>,
    max_cold_vm_steps: u64,
    sqlite_cache_kib: u64,
    open_vm_steps: u64,
    cold_digest_read_bytes: u64,
    cold_validation_charged_bytes: u64,
}

impl<'a> VerifiedKnowledgeModel<'a> {
    /// Logical owned metadata, original sealed model and configured cold cache.
    /// The borrowed custody/source owner is charged by its enclosing callback.
    pub fn retained_state_upper_bound(&self) -> Result<usize> {
        use tos_foundation::{OwnedState, checked_state_add};
        let mut bytes = std::mem::size_of::<Self>();
        macro_rules! charge { ($($field:ident),*) => { $(
            bytes = checked_state_add(bytes, self.$field.owned_heap_bytes()
                .map_err(|_| Error::Budget("verified model retained state"))?)
                .map_err(|_| Error::Budget("verified model retained state"))?;
        )* }; }
        charge!(
            selection,
            source_basis,
            navigation_original,
            philosophy_original,
            corpus_original
        );
        for size in [
            self.selection.model_size_bytes,
            self.sqlite_cache_kib
                .checked_mul(1024)
                .ok_or(Error::Budget("verified model cache state overflow"))?,
        ] {
            bytes = checked_state_add(
                bytes,
                usize::try_from(size).map_err(|_| Error::Budget("verified model state size"))?,
            )
            .map_err(|_| Error::Budget("verified model retained state"))?;
        }
        Ok(bytes)
    }

    /// Exact bounded original carrier under this same immutable selected lease.
    /// Legacy snapshots explicitly refuse; this is not a current rights grant.
    /// Mechanical retained-component presence, never an authority grant.
    pub fn navigation_original_available(&self) -> bool {
        self.navigation_original.is_some()
    }
    pub fn philosophy_original_available(&self) -> bool {
        self.philosophy_original.is_some()
    }
    pub fn corpus_original_available(&self) -> bool {
        self.corpus_original.is_some()
    }
    pub fn corpus_original_receipt(&self) -> Result<&crate::CorpusOriginalReceipt> {
        self.check_pin()?;
        self.corpus_original
            .as_ref()
            .ok_or(Error::Invalid("selected corpus original unavailable"))
    }
    pub fn corpus_original_page_under_caller_budget(
        &self,
        collection: crate::CorpusOriginalCollection,
        selector: &crate::CorpusOriginalSelector,
        after: Option<u64>,
        max_rows: usize,
        max_row_bytes: usize,
        max_page_bytes: u64,
    ) -> Result<crate::CorpusOriginalPage> {
        self.corpus_original_receipt()?;
        let page = crate::knowledge_corpus_original::page_with_layout(
            &self.connection,
            collection,
            selector,
            after,
            max_rows,
            max_row_bytes,
            max_page_bytes,
            selected_payload_layout(&self.selection.model_abi),
        )?;
        self.check_pin()?;
        Ok(page)
    }
    /// Same authenticated ordered original row, with caller-owned logical state.
    pub fn corpus_original_all_row_with_state_budget(
        &self,
        collection: crate::CorpusOriginalCollection,
        after: Option<u64>,
        max_row_bytes: usize,
        max_page_bytes: u64,
        available_state_bytes: usize,
    ) -> Result<Option<crate::CorpusOriginalRow>> {
        self.corpus_original_receipt()?;
        let row = crate::knowledge_corpus_original::all_row_with_state_budget_and_layout(
            &self.connection,
            collection,
            after,
            max_row_bytes,
            max_page_bytes,
            available_state_bytes,
            selected_payload_layout(&self.selection.model_abi),
        )?;
        self.check_pin()?;
        Ok(row)
    }
    /// Bounded GraphViews identity projection under the caller's same VM hook
    /// and selected lease. This does not authorize disclosure of original rows.
    pub fn corpus_original_view_identities_under_caller_budget(
        &self,
        after: Option<u64>,
        max_rows: usize,
        max_id_bytes: usize,
        max_page_bytes: u64,
    ) -> Result<crate::CorpusOriginalViewIdentityPage> {
        let receipt = self.corpus_original_receipt()?;
        let count = receipt
            .collections
            .iter()
            .find(|r| r.collection == crate::CorpusOriginalCollection::GraphViews.as_str())
            .ok_or(Error::Invalid(
                "selected corpus GraphViews receipt unavailable",
            ))?
            .rows;
        let page = crate::knowledge_corpus_original::view_identities(
            &self.connection,
            after,
            max_rows,
            max_id_bytes,
            max_page_bytes,
        )?;
        if page.rows.iter().any(|r| r.ordinal >= count) {
            return Err(Error::Invalid("corpus view identity receipt coverage"));
        }
        self.check_pin()?;
        Ok(page)
    }
    pub fn philosophy_original_receipt(&self) -> Result<&crate::PhilosophyOriginalReceipt> {
        self.check_pin()?;
        self.philosophy_original.as_ref().ok_or(Error::Invalid(
            "selected philosophy original component unavailable",
        ))
    }
    pub fn philosophy_original_page_under_caller_budget(
        &self,
        collection: crate::PhilosophyOriginalCollection,
        after: Option<u64>,
        max_rows: usize,
        max_row_bytes: usize,
        max_page_bytes: u64,
    ) -> Result<crate::PhilosophyOriginalPage> {
        self.philosophy_original_receipt()?;
        let page = crate::knowledge_philosophy_original::page_with_layout(
            &self.connection,
            collection,
            after,
            max_rows,
            max_row_bytes,
            max_page_bytes,
            selected_payload_layout(&self.selection.model_abi),
        )?;
        self.check_pin()?;
        Ok(page)
    }
    pub fn navigation_original_receipt(&self) -> Result<&crate::NavigationOriginalReceipt> {
        self.check_pin()?;
        self.navigation_original.as_ref().ok_or(Error::Invalid(
            "selected navigation original component unavailable",
        ))
    }
    pub fn navigation_original_page(
        &mut self,
        after: Option<i64>,
        max_vm_steps: u64,
        max_rows: usize,
        max_row_bytes: usize,
        max_page_bytes: u64,
    ) -> Result<crate::NavigationOriginalPage> {
        self.navigation_original_receipt()?;
        if max_vm_steps == 0 || max_vm_steps > self.max_cold_vm_steps {
            return Err(Error::Budget("navigation original seek VM budget"));
        }
        let counter = Arc::new(AtomicU64::new(0));
        let used = Arc::clone(&counter);
        self.connection.progress_handler(
            1,
            Some(move || used.fetch_add(1, Ordering::Relaxed).saturating_add(1) >= max_vm_steps),
        );
        let result = crate::knowledge_navigation_original::page(
            &self.connection,
            after,
            max_rows,
            max_row_bytes,
            max_page_bytes,
        );
        self.connection.progress_handler(0, None::<fn() -> bool>);
        self.check_pin()?;
        if counter.load(Ordering::Relaxed) >= max_vm_steps {
            return Err(Error::Budget("navigation original seek VM budget"));
        }
        result.map(|mut page| {
            page.vm_steps = counter.load(Ordering::Relaxed);
            page
        })
    }
    /// Preserve the caller's existing aggregate VM/cancellation/authority hook.
    /// Caller must install and charge that hook for this same selected lease.
    pub fn navigation_original_page_under_caller_budget(
        &self,
        after: Option<i64>,
        max_rows: usize,
        max_row_bytes: usize,
        max_page_bytes: u64,
    ) -> Result<crate::NavigationOriginalPage> {
        self.navigation_original_receipt()?;
        let result = crate::knowledge_navigation_original::page(
            &self.connection,
            after,
            max_rows,
            max_row_bytes,
            max_page_bytes,
        );
        self.check_pin()?;
        result
    }
    /// Original membership index; never infer membership from payload key names.
    /// This accessor preserves the caller's SQLite hook and reports no own VM charge.
    pub fn navigation_original_members_under_caller_budget(
        &self,
        collection: &str,
        after: Option<&str>,
        max_rows: usize,
        max_page_bytes: u64,
    ) -> Result<crate::NavigationOriginalMemberPage> {
        self.navigation_original_receipt()?;
        let result = crate::knowledge_navigation_original::member_page(
            &self.connection,
            collection,
            after,
            max_rows,
            max_page_bytes,
        );
        self.check_pin()?;
        result
    }
    /// Exact original member point seek with the caller's retained VM hook.
    /// The pinned, cold-verified immutable base owns membership/absence.
    pub fn navigation_original_member_under_caller_budget(
        &self,
        collection: &str,
        id: &str,
        max_bytes: u64,
    ) -> Result<Option<crate::NavigationOriginalMember>> {
        self.navigation_original_receipt()?;
        let result = crate::knowledge_navigation_original::member_exact(
            &self.connection,
            collection,
            id,
            max_bytes,
        );
        self.check_pin()?;
        result
    }

    pub fn selection(&self) -> &KnowledgeSelectedExpectation {
        &self.selection
    }
    /// Unicode and 3-gram semantics of the selected search index. The cold
    /// opener verifies the fixed model ABI, complete index root and rows;
    /// the producer seal accepts only this compiled search profile. This is
    /// distinct from the authored query primitive profile in the descriptor.
    pub fn search_index_profile(&self) -> &'static str {
        crate::knowledge_search::SEARCH_PROFILE
    }
    /// The exact revision in the canonical graph header, verified during
    /// cold admission against the selected graph root and file SHA.
    pub fn source_revision(&self) -> Option<&str> {
        self.source_basis.source_revision()
    }
    pub fn source_basis(&self) -> &crate::KnowledgeSourceBasis {
        &self.source_basis
    }
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.connection
    }
    /// Bytes actually consumed by the cold full-file digest. A warm fork
    /// performs no digest admission and reports zero, never repeats its base's
    /// historical cost as newly performed I/O.
    pub fn cold_digest_read_bytes(&self) -> u64 {
        self.cold_digest_read_bytes
    }
    /// Existing logical row/payload work charge from cold validation; excludes
    /// SQLite internal page reads/PRAGMA work (VM steps are reported separately).
    pub fn cold_validation_charged_bytes(&self) -> u64 {
        self.cold_validation_charged_bytes
    }
    pub fn open_vm_steps(&self) -> u64 {
        self.open_vm_steps
    }
    /// Cold transport consumes only the genuinely pinned selected inode.
    /// This is not a writer or a new authority constructor.
    pub(crate) fn read_pinned_chunk(&self, offset: u64, bytes: &mut [u8]) -> Result<()> {
        use std::os::unix::fs::FileExt;
        if offset
            .checked_add(bytes.len() as u64)
            .is_none_or(|end| end > self.selection.model_size_bytes)
        {
            return Err(Error::Budget("selected base transport chunk"));
        }
        self.check_pin()?;
        self.pinned.read_exact_at(bytes, offset)?;
        self.check_pin()
    }

    pub fn check_pin(&self) -> Result<()> {
        let meta = self.pinned.metadata()?;
        if !meta.file_type().is_file() || meta.len() != self.selection.model_size_bytes {
            return Err(Error::Invalid("knowledge selected inode changed"));
        }
        self.custody.verify(&self.pinned, &self.selection)
    }
    /// Only the pinned FD is cloned; SQLite opens `/proc/self/fd/N` under
    /// read-only immutable mode. QRY installs a separate per-query VM cap.
    pub fn fork_reader_with_vm_budget(&self, max_vm_steps: u64) -> Result<Self> {
        if max_vm_steps == 0 || max_vm_steps > self.max_cold_vm_steps {
            return Err(Error::Budget("knowledge warm startup VM steps"));
        }
        self.check_pin()?;
        let pinned = self.pinned.try_clone()?;
        let (connection, counter) = open_sqlite(&pinned, max_vm_steps, self.sqlite_cache_kib)?;
        Ok(Self {
            connection,
            pinned,
            selection: self.selection.clone(),
            source_basis: self.source_basis.clone(),
            navigation_original: self.navigation_original.clone(),
            philosophy_original: self.philosophy_original.clone(),
            corpus_original: self.corpus_original.clone(),
            custody: self.custody.clone(),
            max_cold_vm_steps: max_vm_steps,
            sqlite_cache_kib: self.sqlite_cache_kib,
            open_vm_steps: counter.load(Ordering::Relaxed),
            cold_digest_read_bytes: 0,
            cold_validation_charged_bytes: 0,
        })
    }
}

fn checked_digest(value: &str) -> Result<()> {
    Digest256::from_hex(value).map_err(|_| Error::Invalid("knowledge expected digest"))?;
    Ok(())
}

pub(crate) fn validate(
    expected: &KnowledgeSelectedExpectation,
    limits: ColdOpenLimits,
) -> Result<()> {
    if limits.max_file_bytes == 0
        || limits.max_vm_steps == 0
        || limits.sqlite_cache_kib == 0
        || limits.sqlite_cache_kib > 512 * 1024
        || limits.max_rows == 0
        || limits.max_work_bytes == 0
        || limits.max_row_bytes == 0
        || limits.max_row_bytes > 64 * 1024 * 1024
        || limits.max_metadata_bytes == 0
        || limits.max_metadata_bytes > ColdOpenLimits::MAX_METADATA_BYTES
        || limits.max_sources == 0
        || limits.max_sources > 4096
    {
        return Err(Error::Budget("knowledge cold-open limits"));
    }
    if expected.model_size_bytes == 0
        || expected.model_size_bytes > limits.max_file_bytes
        || !tos_foundation::KNOWLEDGE_POSTINGS_MODEL_ABIS.contains(&expected.model_abi.as_str())
        || !expected.complete
        || expected.source_scopes.is_empty()
        || expected.source_scopes.len() > limits.max_sources
    {
        return Err(Error::Invalid("knowledge selection expectation incomplete"));
    }
    match (
        expected.model_abi.as_str(),
        &expected.navigation_original_root_sha256,
        &expected.philosophy_original_root_sha256,
        &expected.corpus_original_root_sha256,
    ) {
        (KNOWLEDGE_MODEL_ABI, None, None, None) => (),
        (crate::KNOWLEDGE_MANAGED_MODEL_ABI, Some(nav), None, None) => checked_digest(nav)?,
        (crate::KNOWLEDGE_NAVIGATION_MODEL_ABI, Some(nav), None, None) => checked_digest(nav)?,
        (crate::KNOWLEDGE_PHILOSOPHY_MODEL_ABI, nav, Some(phi), None) => {
            checked_digest(phi)?;
            if let Some(nav) = nav {
                checked_digest(nav)?;
            }
        }
        (
            crate::knowledge_stage::KNOWLEDGE_CARRIER_ONCE_MODEL_ABI
                | tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V1,
            Some(nav),
            Some(phi),
            Some(corpus),
        ) => {
            checked_digest(nav)?;
            checked_digest(phi)?;
            checked_digest(corpus)?;
        }
        (crate::KNOWLEDGE_CORPUS_MODEL_ABI, nav, phi, Some(corpus)) => {
            checked_digest(corpus)?;
            if let Some(r) = nav {
                checked_digest(r)?;
            }
            if let Some(r) = phi {
                checked_digest(r)?;
            }
        }
        _ => {
            return Err(Error::Invalid(
                "knowledge independent original component expectation",
            ));
        }
    }
    match (
        &expected.managed_source_root_sha256,
        expected.model_abi.as_str(),
    ) {
        (Some(root), crate::KNOWLEDGE_MANAGED_MODEL_ABI) => checked_digest(root)?,
        (None, abi) if abi != crate::KNOWLEDGE_MANAGED_MODEL_ABI => (),
        _ => return Err(Error::Invalid("knowledge managed source expectation")),
    }
    for text in [
        &expected.owner_receipt_id,
        &expected.semantic_primitive_profile,
        &expected.source_cut,
        &expected.entity_registry_id,
        &expected.entity_registry_version,
        &expected.relation_registry_id,
        &expected.relation_registry_version,
        &expected.index_generation,
        &expected.route_map_version,
        &expected.reader_abi,
        &expected.authority_boundary,
    ] {
        if text.is_empty() || text.len() > limits.max_metadata_bytes {
            return Err(Error::Invalid("knowledge selection field"));
        }
    }
    if expected.semantic_primitive_profile != KNOWLEDGE_QUERY_PRIMITIVE_PROFILE
        || expected.descriptor_version == 0
    {
        return Err(Error::Invalid("knowledge selection profile/version"));
    }
    for digest in [
        &expected.model_sha256,
        &expected.descriptor_sha256,
        &expected.membership_root,
        &expected.entity_registry_sha256,
        &expected.relation_registry_sha256,
        &expected.graph_root_sha256,
        &expected.catalog_packet_sha256,
        &expected.catalog_index_root_sha256,
        &expected.source_scope_root_sha256,
        &expected.search_index_root_sha256,
    ] {
        checked_digest(digest)?;
    }
    let mut previous: Option<&str> = None;
    let mut node_total = 0u64;
    let mut relation_total = 0u64;
    for scope in &expected.source_scopes {
        for text in [
            &scope.source_graph,
            &scope.input_role,
            &scope.adapter_profile,
        ] {
            if text.is_empty() || text.len() > limits.max_metadata_bytes {
                return Err(Error::Invalid("knowledge expected scope field"));
            }
        }
        if previous.is_some_and(|p| p >= scope.source_graph.as_str()) {
            return Err(Error::Invalid("knowledge expected scope order"));
        }
        previous = Some(&scope.source_graph);
        checked_digest(&scope.node_root_sha256)?;
        checked_digest(&scope.relation_root_sha256)?;
        node_total = node_total
            .checked_add(scope.node_count)
            .ok_or(Error::Budget("knowledge expected nodes"))?;
        relation_total = relation_total
            .checked_add(scope.relation_count)
            .ok_or(Error::Budget("knowledge expected relations"))?;
    }
    if node_total != expected.node_count
        || relation_total != expected.relation_count
        || node_total
            .checked_add(relation_total)
            .ok_or(Error::Budget("knowledge rows"))?
            > limits.max_rows
    {
        return Err(Error::Invalid("knowledge expected scope totals"));
    }
    Ok(())
}

fn open_sqlite(
    pinned: &File,
    max_vm_steps: u64,
    cache_kib: u64,
) -> Result<(tos_source_store::PinnedSqliteConnection, Arc<AtomicU64>)> {
    if max_vm_steps == 0 {
        return Err(Error::Budget("knowledge SQLite VM steps"));
    }
    let db = tos_source_store::PinnedSqliteConnection::open_readonly_immutable(pinned)
        .map_err(|error| Error::Source(error.to_string()))?;
    let used = Arc::new(AtomicU64::new(0));
    let callback = Arc::clone(&used);
    db.progress_handler(
        1,
        Some(move || callback.fetch_add(1, Ordering::Relaxed).saturating_add(1) >= max_vm_steps),
    );
    db.execute_batch("PRAGMA query_only=ON; PRAGMA trusted_schema=OFF; PRAGMA temp_store=FILE; PRAGMA mmap_size=0;")?;
    db.pragma_update(None, "cache_size", -(cache_kib as i64))?;
    let effective: i64 = db.query_row("PRAGMA cache_size", [], |r| r.get(0))?;
    if effective != -(cache_kib as i64) {
        return Err(Error::Invalid("knowledge SQLite cache budget"));
    }
    Ok((db, used))
}

/// Charge and hash the exact sealed selected file on the authentic builder
/// state before SQLite opens it. Work is debited before each bounded read so a
/// failed or shortened source never refunds the original owner.
pub(crate) fn digest_selected_file_with_owned_context(
    pinned: &mut File,
    expected: &KnowledgeSelectedExpectation,
    limits: ColdOpenLimits,
    context: &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
) -> Result<u64> {
    use crate::d1_public_capture::{CreationState, CreationStateHold};
    use std::io::SeekFrom;

    let state = context.owned_state();
    let fixed = std::mem::size_of::<(
        &mut File,
        &KnowledgeSelectedExpectation,
        ColdOpenLimits,
        &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
        &CreationState<'_>,
        CreationStateHold<'_, '_>,
        std::fs::Metadata,
        Digest256Hasher,
        [u8; 65_536],
        u64,
        usize,
        Digest256,
        Result<u64>,
    )>();
    let _hold = state.hold(fixed)?;
    if expected.model_size_bytes > limits.max_file_bytes || limits.max_work_bytes == 0 {
        return Err(Error::Budget("controlled selected file digest admission"));
    }
    let metadata = pinned.metadata()?;
    if !metadata.is_file() || metadata.len() != expected.model_size_bytes {
        return Err(Error::Invalid("controlled selected file size"));
    }
    let expected_digest = Digest256::from_hex(&expected.model_sha256)
        .map_err(|_| Error::Invalid("controlled selected file digest expectation"))?;
    pinned.seek(SeekFrom::Start(0))?;
    context.check()?;
    let mut hasher = Digest256Hasher::new();
    let mut buffer = [0u8; 65_536];
    let mut total = 0u64;
    let mut charged = 0u64;
    while total < expected.model_size_bytes {
        context.check()?;
        let remaining = expected
            .model_size_bytes
            .checked_sub(total)
            .ok_or(Error::Budget("controlled selected digest bytes"))?;
        let amount = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|_| Error::Budget("controlled selected digest read size"))?;
        charged = charged
            .checked_add(amount as u64)
            .filter(|next| *next <= limits.max_work_bytes)
            .ok_or(Error::Budget("controlled selected digest work"))?;
        context.charge_work(amount)?;
        let count = pinned.read(&mut buffer[..amount])?;
        if count == 0 {
            return Err(Error::Invalid(
                "controlled selected file truncated during digest",
            ));
        }
        total = total
            .checked_add(count as u64)
            .ok_or(Error::Budget("controlled selected digest bytes"))?;
        hasher.update(&buffer[..count]);
    }
    context.check()?;
    let final_metadata = pinned.metadata()?;
    if !final_metadata.is_file()
        || final_metadata.len() != expected.model_size_bytes
        || total != expected.model_size_bytes
        || hasher.finalize() != expected_digest
    {
        return Err(Error::Invalid("controlled selected file digest mismatch"));
    }
    context.check()?;
    Ok(total)
}

/// Configure the one immutable selected SQLite connection before verification
/// or QRY. This keeps cache/temp/mmap policy with the compiler owner and
/// charges the pragma/update/read sequence on the existing state and VM hook.
pub(crate) fn configure_selected_sql_with_owned_context(
    db: &Connection,
    limits: ColdOpenLimits,
    context: &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
) -> Result<()> {
    use crate::d1_public_capture::CreationStateHold;

    let state = context.owned_state();
    let statement_state =
        tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound()
            .checked_mul(2)
            .ok_or(Error::Budget("controlled selected pragma workspace"))?;
    let fixed = std::mem::size_of::<(
        &Connection,
        ColdOpenLimits,
        &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
        CreationStateHold<'_, '_>,
        i64,
        Result<()>,
    )>()
    .checked_add(statement_state)
    .ok_or(Error::Budget("controlled selected pragma frame"))?;
    let _hold = state.hold(fixed)?;
    if limits.sqlite_cache_kib == 0 || limits.sqlite_cache_kib > i64::MAX as u64 {
        return Err(Error::Budget("controlled selected SQLite cache cap"));
    }
    const PRAGMAS: &str = "PRAGMA query_only=ON; PRAGMA trusted_schema=OFF; PRAGMA temp_store=FILE; PRAGMA mmap_size=0;";
    state.charge_work(PRAGMAS.len())?;
    context.check()?;
    db.execute_batch(PRAGMAS)?;
    context.check()?;
    let cache_kib = i64::try_from(limits.sqlite_cache_kib)
        .map_err(|_| Error::Budget("controlled selected SQLite cache cap"))?;
    state.charge_work(std::mem::size_of::<i64>())?;
    context.check()?;
    db.pragma_update(None, "cache_size", -cache_kib)?;
    with_owned_schema_statement(db, c"PRAGMA cache_size", state, |statement| {
        if !owned_schema_step(statement, state)? {
            return Err(Error::Invalid("controlled selected SQLite cache absent"));
        }
        let effective = statement.integer(0).map_err(owned_schema_sql_error)?;
        if effective != -cache_kib {
            return Err(Error::Invalid("controlled selected SQLite cache budget"));
        }
        context.check()
    })
}

fn metadata_matches(db: &Connection, key: &str, max_bytes: usize, expected: &str) -> Result<bool> {
    let mut statement = db.prepare(
        "SELECT CAST(value AS BLOB) FROM metadata WHERE key=?1 AND typeof(value) IN ('text','blob') AND length(CAST(value AS BLOB))<=?2",
    )?;
    let mut rows = statement.query(params![key, max_bytes as i64])?;
    let row = rows
        .next()?
        .ok_or(Error::Invalid("knowledge metadata absent/oversized"))?;
    let value = match row.get_ref(0)? {
        rusqlite::types::ValueRef::Blob(value) => value,
        _ => return Err(Error::Invalid("knowledge metadata BLOB")),
    };
    let value =
        std::str::from_utf8(value).map_err(|_| Error::Invalid("knowledge metadata UTF-8"))?;
    Ok(value == expected)
}

/// Exact decimal spelling without an owned formatting allocation.
fn decimal_u64(value: u64, storage: &mut [u8; 20]) -> &str {
    let mut value = value;
    let mut start = storage.len();
    loop {
        start -= 1;
        storage[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    // Every emitted byte is an ASCII decimal digit.
    std::str::from_utf8(&storage[start..]).expect("decimal digits")
}

fn check_metadata(
    db: &Connection,
    expected: &KnowledgeSelectedExpectation,
    cap: usize,
) -> Result<()> {
    for (key, value) in [
        ("model_abi", expected.model_abi.as_str()),
        ("descriptor_sha256", expected.descriptor_sha256.as_str()),
        (
            "semantic_primitive_profile",
            expected.semantic_primitive_profile.as_str(),
        ),
        ("source_cut", expected.source_cut.as_str()),
        ("membership_root", expected.membership_root.as_str()),
        ("entity_registry_id", expected.entity_registry_id.as_str()),
        (
            "entity_registry_version",
            expected.entity_registry_version.as_str(),
        ),
        (
            "entity_registry_sha256",
            expected.entity_registry_sha256.as_str(),
        ),
        (
            "relation_registry_id",
            expected.relation_registry_id.as_str(),
        ),
        (
            "relation_registry_version",
            expected.relation_registry_version.as_str(),
        ),
        (
            "relation_registry_sha256",
            expected.relation_registry_sha256.as_str(),
        ),
        ("graph_root_sha256", expected.graph_root_sha256.as_str()),
        (
            "catalog_packet_sha256",
            expected.catalog_packet_sha256.as_str(),
        ),
        (
            "catalog_index_root_sha256",
            expected.catalog_index_root_sha256.as_str(),
        ),
        (
            "source_scope_root_sha256",
            expected.source_scope_root_sha256.as_str(),
        ),
        (
            "search_index_root_sha256",
            expected.search_index_root_sha256.as_str(),
        ),
        ("index_generation", expected.index_generation.as_str()),
        ("route_map_version", expected.route_map_version.as_str()),
        ("reader_abi", expected.reader_abi.as_str()),
        ("authority_boundary", expected.authority_boundary.as_str()),
        ("complete", "true"),
    ] {
        if !metadata_matches(db, key, cap, value)? {
            return Err(Error::Invalid("knowledge metadata binding"));
        }
    }
    let managed: Option<String> = db.query_row(
        "SELECT CASE WHEN typeof(value) IN ('text','blob') AND length(CAST(value AS BLOB))=64 THEN CAST(value AS TEXT) ELSE NULL END FROM metadata WHERE key='managed_source_root_sha256'",
        [], |r| r.get(0)).optional()?;
    if managed != expected.managed_source_root_sha256 {
        return Err(Error::Invalid("knowledge managed source metadata binding"));
    }
    for (key, value) in [
        ("descriptor_version", expected.descriptor_version),
        ("through_commit_seq", expected.through_commit_seq),
        ("node_count", expected.node_count),
        ("relation_count", expected.relation_count),
    ] {
        let mut decimal = [0u8; 20];
        if !metadata_matches(db, key, 32, decimal_u64(value, &mut decimal))? {
            return Err(Error::Invalid("knowledge metadata count/version"));
        }
    }
    Ok(())
}

fn hash_text(hash: &mut Digest256Hasher, text: &str) {
    hash.update(&(text.len() as u64).to_be_bytes());
    hash.update(text.as_bytes());
}
fn hash_item(hash: &mut Digest256Hasher, id: &str, digest: &[u8]) {
    hash_text(hash, id);
    hash.update(digest);
}
fn hash_bytes(hash: &mut Digest256Hasher, bytes: &[u8]) {
    hash.update(&(bytes.len() as u64).to_be_bytes());
    hash.update(bytes);
}
fn charge(work: &mut u64, amount: usize, cap: u64) -> Result<()> {
    *work = work
        .checked_add(amount as u64)
        .ok_or(Error::Budget("knowledge cold work"))?;
    if *work > cap {
        return Err(Error::Budget("knowledge cold work"));
    }
    Ok(())
}

fn nonnegative(value: i64) -> Result<u64> {
    u64::try_from(value).map_err(|_| Error::Invalid("knowledge catalog negative count"))
}

fn catalog_text(row: &rusqlite::Row<'_>, col: usize) -> Result<String> {
    row.get::<_, Option<String>>(col)?
        .ok_or(Error::Budget("knowledge catalog text bytes"))
}

fn owned_metadata_matches(
    db: &Connection,
    key: &str,
    cap: usize,
    expected: &str,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<bool> {
    let _frame = state.hold(std::mem::size_of::<(
        &Connection,
        &str,
        usize,
        &str,
        &crate::d1_public_capture::CreationState<'_>,
        &[u8],
        &str,
        Result<bool>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    if cap > i64::MAX as usize {
        return Err(Error::Budget("metadata addressable cap"));
    }
    with_owned_schema_statement(db,c"SELECT CAST(value AS BLOB) FROM metadata WHERE key=?1 AND typeof(value) IN ('text','blob') AND length(CAST(value AS BLOB))<=?2",state,|statement| {
        state.charge_work(key.len().checked_add(8).ok_or(Error::Budget("metadata bind work"))?)?;
        statement.bind_text(1,key).map_err(owned_schema_sql_error)?;
        statement.bind_i64(2,cap as i64).map_err(owned_schema_sql_error)?;
        if !owned_schema_step(statement,state)? {return Err(Error::Invalid("knowledge metadata absent/oversized"));}
        let rusqlite::types::ValueRef::Blob(value)=statement.value_ref(0).map_err(owned_schema_sql_error)? else {return Err(Error::Invalid("knowledge metadata BLOB"));};
        if value.len()>cap {return Err(Error::Budget("knowledge metadata bytes"));}
        state.charge_work(value.len())?;state.active()?;
        let value=std::str::from_utf8(value).map_err(|_|Error::Invalid("knowledge metadata UTF8"))?;
        owned_schema_equal(state,value.as_bytes(),expected.as_bytes())
    })
}
pub(crate) fn check_native_metadata_with_owned_context(
    db: &Connection,
    expected: &KnowledgeSelectedExpectation,
    cap: usize,
    context: &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
) -> Result<()> {
    let state = context.owned_state();
    let _frame = state.hold(std::mem::size_of::<(
        &Connection,
        &KnowledgeSelectedExpectation,
        usize,
        &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
        &crate::d1_public_capture::CreationState<'_>,
        std::array::IntoIter<(&str, &str), 21>,
        std::array::IntoIter<(&str, u64), 4>,
        [(&str, &str); 21],
        [(&str, u64); 4],
        &str,
        &str,
        u64,
        [u8; 20],
        Result<()>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    if expected.managed_source_root_sha256.is_some() {
        return Err(Error::Invalid("native metadata profile required"));
    }
    for (key, value) in [
        ("model_abi", expected.model_abi.as_str()),
        ("descriptor_sha256", expected.descriptor_sha256.as_str()),
        (
            "semantic_primitive_profile",
            expected.semantic_primitive_profile.as_str(),
        ),
        ("source_cut", expected.source_cut.as_str()),
        ("membership_root", expected.membership_root.as_str()),
        ("entity_registry_id", expected.entity_registry_id.as_str()),
        (
            "entity_registry_version",
            expected.entity_registry_version.as_str(),
        ),
        (
            "entity_registry_sha256",
            expected.entity_registry_sha256.as_str(),
        ),
        (
            "relation_registry_id",
            expected.relation_registry_id.as_str(),
        ),
        (
            "relation_registry_version",
            expected.relation_registry_version.as_str(),
        ),
        (
            "relation_registry_sha256",
            expected.relation_registry_sha256.as_str(),
        ),
        ("graph_root_sha256", expected.graph_root_sha256.as_str()),
        (
            "catalog_packet_sha256",
            expected.catalog_packet_sha256.as_str(),
        ),
        (
            "catalog_index_root_sha256",
            expected.catalog_index_root_sha256.as_str(),
        ),
        (
            "source_scope_root_sha256",
            expected.source_scope_root_sha256.as_str(),
        ),
        (
            "search_index_root_sha256",
            expected.search_index_root_sha256.as_str(),
        ),
        ("index_generation", expected.index_generation.as_str()),
        ("route_map_version", expected.route_map_version.as_str()),
        ("reader_abi", expected.reader_abi.as_str()),
        ("authority_boundary", expected.authority_boundary.as_str()),
        ("complete", "true"),
    ] {
        if !owned_metadata_matches(db, key, cap, value, state)? {
            return Err(Error::Invalid("knowledge metadata binding"));
        }
    }
    with_owned_schema_statement(db,c"SELECT CASE WHEN typeof(value) IN ('text','blob') AND length(CAST(value AS BLOB))=64 THEN CAST(value AS BLOB) ELSE NULL END FROM metadata WHERE key='managed_source_root_sha256'",state,|statement| {
        if owned_schema_step(statement,state)? && !matches!(statement.value_ref(0).map_err(owned_schema_sql_error)?,rusqlite::types::ValueRef::Null) {return Err(Error::Invalid("knowledge managed source metadata binding"));}Ok(())
    })?;
    for (key, value) in [
        ("descriptor_version", expected.descriptor_version),
        ("through_commit_seq", expected.through_commit_seq),
        ("node_count", expected.node_count),
        ("relation_count", expected.relation_count),
    ] {
        let mut decimal = [0u8; 20];
        state.charge_work(20)?;
        state.active()?;
        if !owned_metadata_matches(db, key, 32, decimal_u64(value, &mut decimal), state)? {
            return Err(Error::Invalid("knowledge metadata count/version"));
        }
    }
    state.active()
}
pub(crate) fn verify_integrity_with_owned_context(
    db: &Connection,
    context: &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
) -> Result<()> {
    let state = context.owned_state();
    let _frame = state.hold(std::mem::size_of::<(
        &Connection,
        &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
        &crate::d1_public_capture::CreationState<'_>,
        &[u8],
        Result<()>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    with_owned_schema_statement(db, c"PRAGMA integrity_check", state, |statement| {
        if !owned_schema_step(statement, state)? {
            return Err(Error::Invalid("knowledge integrity missing"));
        }
        let rusqlite::types::ValueRef::Text(first) =
            statement.value_ref(0).map_err(owned_schema_sql_error)?
        else {
            return Err(Error::Invalid("knowledge integrity result type"));
        };
        if first.len() != 2
            || !owned_schema_equal(state, first, b"ok")?
            || owned_schema_step(statement, state)?
        {
            return Err(Error::Invalid("knowledge SQLite integrity"));
        }
        Ok(())
    })
}

fn verify_catalog(
    db: &Connection,
    expected: &KnowledgeSelectedExpectation,
    limits: ColdOpenLimits,
    work: &mut u64,
) -> Result<()> {
    let desc = &expected.descriptor_sha256;
    let one: i64 = db.query_row("SELECT COUNT(*) FROM catalog_index_meta", [], |r| r.get(0))?;
    if one != 1 {
        return Err(Error::Invalid("knowledge catalog meta coverage"));
    }
    let mut statement = db.prepare(
        "SELECT CASE WHEN length(CAST(catalog_packet_sha256 AS BLOB))<=?2 THEN catalog_packet_sha256 ELSE NULL END,
         CASE WHEN length(CAST(index_schema AS BLOB))<=?2 THEN index_schema ELSE NULL END,
         CASE WHEN length(CAST(order_profile AS BLOB))<=?2 THEN order_profile ELSE NULL END,
         source_count,facet_field_count,facet_value_count,route_count,
         CASE WHEN length(CAST(catalog_index_root_sha256 AS BLOB))<=?2 THEN catalog_index_root_sha256 ELSE NULL END,
         packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,
         CASE WHEN typeof(packet)='blob' AND length(packet)<=?3 THEN packet ELSE NULL END
         FROM catalog_index_meta WHERE descriptor_sha256=?1"
    )?;
    let meta = statement
        .query_row(
            params![
                desc,
                limits.max_metadata_bytes as i64,
                limits.max_row_bytes as i64
            ],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, Option<Vec<u8>>>(9)?,
                    r.get::<_, Option<Vec<u8>>>(10)?,
                ))
            },
        )
        .optional()?
        .ok_or(Error::Invalid("knowledge catalog meta missing"))?;
    let (
        catalog_sha,
        schema,
        order_profile,
        sources,
        fields,
        values,
        routes,
        index_root,
        packet_len,
        packet_sha,
        packet,
    ) = meta;
    let catalog_sha = catalog_sha.ok_or(Error::Budget("knowledge catalog SHA text"))?;
    let schema = schema.ok_or(Error::Budget("knowledge catalog schema text"))?;
    let order_profile = order_profile.ok_or(Error::Budget("knowledge catalog order text"))?;
    let index_root = index_root.ok_or(Error::Budget("knowledge catalog index root text"))?;
    let packet_sha = packet_sha.ok_or(Error::Invalid("knowledge catalog packet SHA type"))?;
    let packet = packet.ok_or(Error::Budget("knowledge catalog packet bytes"))?;
    let (sources, fields, values, routes) = (
        nonnegative(sources)?,
        nonnegative(fields)?,
        nonnegative(values)?,
        nonnegative(routes)?,
    );
    if schema != "tos_catalog_index_v1"
        || ![
            crate::knowledge_catalog_index::ORDER_PROFILE,
            crate::knowledge_catalog_index::LEGACY_ORDER_PROFILE,
        ]
        .contains(&order_profile.as_str())
        || catalog_sha != expected.catalog_packet_sha256
        || index_root != expected.catalog_index_root_sha256
        || sources != expected.source_scopes.len() as u64
        || packet_len < 0
        || packet_len as usize != packet.len()
        || packet_sha.as_slice() != Digest256::of_bytes(&packet).as_bytes()
        || catalog_sha != Digest256::of_bytes(&packet).to_hex()
    {
        return Err(Error::Invalid("knowledge catalog packet/meta binding"));
    }
    let legacy_ascii = order_profile == crate::knowledge_catalog_index::LEGACY_ORDER_PROFILE;
    if legacy_ascii
        && expected
            .source_scopes
            .iter()
            .any(|scope| !scope.source_graph.is_ascii())
    {
        return Err(Error::Invalid("legacy catalog ASCII source profile"));
    }
    charge(work, packet.len(), limits.max_work_bytes)?;
    // The catalog owner owns packet interpretation. Cold admission checks
    // syntax and exact packet/index receipts without reimplementing that parser.
    parse_json(
        &packet,
        JsonMode::PublishedStrict,
        JsonLimits::new(limits.max_row_bytes, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("knowledge catalog JSON limits"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    if let Some(expected_root) = &expected.managed_source_root_sha256 {
        let catalog: serde_json::Value = serde_json::from_slice(&packet)
            .map_err(|_| Error::Invalid("managed catalog packet"))?;
        if catalog.get("schema").and_then(serde_json::Value::as_str)
            != Some(crate::managed_source::MANAGED_CATALOG_SCHEMA)
            || catalog.get("source_revision").is_some()
        {
            return Err(Error::Invalid("managed catalog identity fields"));
        }
        let basis: crate::KnowledgeSourceBasis = serde_json::from_value(
            catalog
                .get("source_basis")
                .ok_or(Error::Invalid("managed catalog source basis"))?
                .clone(),
        )
        .map_err(|_| Error::Invalid("managed catalog source basis shape"))?;
        let proof = basis
            .managed_proof()
            .ok_or(Error::Invalid("managed catalog source profile"))?;
        if &proof.root_sha256()? != expected_root {
            return Err(Error::Invalid("managed catalog source root"));
        }
        proof.check_binding(
            &expected.source_cut,
            &expected.membership_root,
            expected.through_commit_seq,
        )?;
    }
    let mut root = Digest256Hasher::new();
    hash_text(&mut root, &schema);
    hash_text(&mut root, &order_profile);
    hash_text(&mut root, desc);
    hash_text(&mut root, &catalog_sha);
    root.update(&packet_sha);
    for n in [
        expected.node_count,
        expected.relation_count,
        fields,
        values,
        routes,
        sources,
    ] {
        root.update(&n.to_be_bytes());
    }
    let mut seen = [0u64; 4];
    let mut field_stmt = db.prepare(
        "SELECT CASE WHEN typeof(domain)='text' AND length(CAST(domain AS BLOB))<=?2 THEN domain ELSE NULL END,
         CASE WHEN typeof(field_id)='text' AND length(CAST(field_id AS BLOB))<=?2 THEN field_id ELSE NULL END,
         value_count,total_count FROM catalog_facet_fields WHERE descriptor_sha256=?1 ORDER BY domain,field_id"
    )?;
    let mut field_rows = field_stmt.query(params![desc, limits.max_metadata_bytes as i64])?;
    while let Some(row) = field_rows.next()? {
        let domain = catalog_text(row, 0)?;
        let field = catalog_text(row, 1)?;
        let value_count = nonnegative(row.get(2)?)?;
        let total_count = nonnegative(row.get(3)?)?;
        if !matches!(domain.as_str(), "node" | "relation") || field.is_empty() {
            return Err(Error::Invalid("knowledge catalog field shape"));
        }
        root.update(b"F");
        hash_text(&mut root, &domain);
        hash_text(&mut root, &field);
        root.update(&value_count.to_be_bytes());
        root.update(&total_count.to_be_bytes());
        charge(work, domain.len() + field.len() + 16, limits.max_work_bytes)?;
        seen[0] = seen[0]
            .checked_add(1)
            .ok_or(Error::Budget("catalog field rows"))?;
        if seen[0] > limits.max_rows {
            return Err(Error::Budget("catalog field rows"));
        }
    }
    let mut facet_stmt = db.prepare(
        "SELECT CASE WHEN typeof(domain)='text' AND length(CAST(domain AS BLOB))<=?2 THEN domain ELSE NULL END,
         CASE WHEN typeof(field_id)='text' AND length(CAST(field_id AS BLOB))<=?2 THEN field_id ELSE NULL END,
         ordinal,CASE WHEN typeof(value_json)='text' AND length(CAST(value_json AS BLOB))<=?3 THEN value_json ELSE NULL END,item_count
         FROM catalog_facets WHERE descriptor_sha256=?1 ORDER BY domain,field_id,ordinal"
    )?;
    let mut facet_rows = facet_stmt.query(params![
        desc,
        limits.max_metadata_bytes as i64,
        limits.max_row_bytes as i64
    ])?;
    let mut previous_field: Option<(String, String)> = None;
    let mut ordinal_for_field = 0u64;
    let mut previous_fold: Option<String> = None;
    while let Some(row) = facet_rows.next()? {
        let domain = catalog_text(row, 0)?;
        let field = catalog_text(row, 1)?;
        let ordinal = nonnegative(row.get(2)?)?;
        let value_json = catalog_text(row, 3)?;
        let item_count = nonnegative(row.get(4)?)?;
        let key = (domain.clone(), field.clone());
        if previous_field.as_ref() != Some(&key) {
            ordinal_for_field = 0;
            previous_fold = None;
            previous_field = Some(key);
        }
        if ordinal != ordinal_for_field || item_count == 0 {
            return Err(Error::Invalid("knowledge catalog facet ordinal/count"));
        }
        let value: String = serde_json::from_str(&value_json)
            .map_err(|_| Error::Invalid("knowledge catalog facet JSON string"))?;
        if legacy_ascii && !value.is_ascii() {
            return Err(Error::Invalid("legacy catalog ASCII facet profile"));
        }
        let folded = tos_foundation::python_casefold_unicode16_v1(
            &value,
            limits.max_row_bytes,
            usize::try_from(limits.max_work_bytes.saturating_sub(*work))
                .map_err(|_| Error::Budget("knowledge casefold addressable bytes"))?,
            usize::try_from(limits.max_work_bytes.saturating_sub(*work))
                .map_err(|_| Error::Budget("knowledge casefold addressable bytes"))?,
        )
        .map_err(|_| Error::Budget("knowledge catalog casefold bytes"))?;
        charge(work, folded.len(), limits.max_work_bytes)?;
        if previous_fold
            .as_ref()
            .is_some_and(|previous| previous > &folded)
        {
            return Err(Error::Invalid("knowledge catalog casefold order"));
        }
        previous_fold = Some(folded);
        ordinal_for_field += 1;
        root.update(b"V");
        hash_text(&mut root, &domain);
        hash_text(&mut root, &field);
        root.update(&ordinal.to_be_bytes());
        hash_text(&mut root, &value_json);
        root.update(&item_count.to_be_bytes());
        charge(
            work,
            domain.len() + field.len() + value_json.len() + 16,
            limits.max_work_bytes,
        )?;
        seen[1] = seen[1]
            .checked_add(1)
            .ok_or(Error::Budget("catalog facet rows"))?;
        if seen[1] > limits.max_rows {
            return Err(Error::Budget("catalog facet rows"));
        }
    }
    let mut route_stmt = db.prepare(
        "SELECT CASE WHEN typeof(route_id)='text' AND length(CAST(route_id AS BLOB))<=?2 THEN route_id ELSE NULL END,
         ordinal,node_count,confirming_relation_count,semantic_confirming_relation_count,
         CASE WHEN typeof(availability)='text' AND length(CAST(availability AS BLOB))<=?2 THEN availability ELSE NULL END,
         CASE WHEN typeof(role_readiness)='text' AND length(CAST(role_readiness AS BLOB))<=?2 THEN role_readiness ELSE NULL END,
         packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,
         CASE WHEN typeof(packet)='blob' AND length(packet)<=?3 THEN packet ELSE NULL END
         FROM catalog_routes WHERE descriptor_sha256=?1 ORDER BY ordinal"
    )?;
    let mut route_rows = route_stmt.query(params![
        desc,
        limits.max_metadata_bytes as i64,
        limits.max_row_bytes as i64
    ])?;
    while let Some(row) = route_rows.next()? {
        let id = catalog_text(row, 0)?;
        let ordinal = nonnegative(row.get(1)?)?;
        let nodes = nonnegative(row.get(2)?)?;
        let confirming = nonnegative(row.get(3)?)?;
        let semantic = nonnegative(row.get(4)?)?;
        let availability = catalog_text(row, 5)?;
        let readiness = catalog_text(row, 6)?;
        let packet_len = nonnegative(row.get(7)?)?;
        let digest = row
            .get::<_, Option<Vec<u8>>>(8)?
            .ok_or(Error::Invalid("catalog route SHA type"))?;
        let route_packet = row
            .get::<_, Option<Vec<u8>>>(9)?
            .ok_or(Error::Budget("catalog route packet bytes"))?;
        if id.is_empty()
            || ordinal != seen[2]
            || nodes > expected.node_count
            || confirming > expected.relation_count
            || semantic > expected.relation_count
            || !matches!(availability.as_str(), "available" | "not_projected")
            || !matches!(
                readiness.as_str(),
                "not_projected" | "kind_only" | "confirmed"
            )
            || packet_len != route_packet.len() as u64
            || digest.as_slice() != Digest256::of_bytes(&route_packet).as_bytes()
        {
            return Err(Error::Invalid("knowledge catalog route closure"));
        }
        root.update(b"R");
        hash_text(&mut root, &id);
        for n in [ordinal, nodes, confirming, semantic] {
            root.update(&n.to_be_bytes());
        }
        hash_text(&mut root, &availability);
        hash_text(&mut root, &readiness);
        root.update(&packet_len.to_be_bytes());
        root.update(&digest);
        charge(
            work,
            id.len() + availability.len() + readiness.len() + route_packet.len() + 32,
            limits.max_work_bytes,
        )?;
        seen[2] = seen[2]
            .checked_add(1)
            .ok_or(Error::Budget("catalog route rows"))?;
        if seen[2] > limits.max_rows {
            return Err(Error::Budget("catalog route rows"));
        }
    }
    let mut source_stmt = db.prepare(
        "SELECT CASE WHEN typeof(source_graph_id)='text' AND length(CAST(source_graph_id AS BLOB))<=?2 THEN source_graph_id ELSE NULL END,node_count,relation_count
         FROM catalog_source_counts WHERE descriptor_sha256=?1 ORDER BY source_graph_id"
    )?;
    let mut source_rows = source_stmt.query(params![desc, limits.max_metadata_bytes as i64])?;
    for scope in &expected.source_scopes {
        let row = source_rows
            .next()?
            .ok_or(Error::Invalid("catalog source omitted"))?;
        let source = catalog_text(row, 0)?;
        let nodes = nonnegative(row.get(1)?)?;
        let relations = nonnegative(row.get(2)?)?;
        if source != scope.source_graph
            || nodes != scope.node_count
            || relations != scope.relation_count
        {
            return Err(Error::Invalid("knowledge catalog source counts"));
        }
        let value_json = serde_json::to_string(&source)
            .map_err(|_| Error::Invalid("knowledge catalog source JSON"))?;
        for (domain, count) in [("node", nodes), ("relation", relations)] {
            let facet: Option<u64> = db.query_row(
                "SELECT item_count FROM catalog_facets WHERE descriptor_sha256=?1 AND domain=?2 AND field_id='source_graph' AND value_json=?3",
                params![desc,domain,&value_json],|r|r.get(0),
            ).optional()?;
            if facet.unwrap_or(0) != count {
                return Err(Error::Invalid("knowledge catalog source facet coverage"));
            }
        }
        root.update(b"S");
        hash_text(&mut root, &source);
        root.update(&nodes.to_be_bytes());
        root.update(&relations.to_be_bytes());
        charge(work, source.len() + 16, limits.max_work_bytes)?;
        seen[3] += 1;
    }
    if source_rows.next()?.is_some()
        || seen != [fields, values, routes, sources]
        || root.finalize().to_hex() != expected.catalog_index_root_sha256
    {
        return Err(Error::Invalid("knowledge catalog index root/coverage"));
    }
    let orphan_facet: Option<i64> = db
        .query_row(
            "SELECT 1 FROM catalog_facets v WHERE NOT EXISTS(
         SELECT 1 FROM catalog_facet_fields f WHERE f.descriptor_sha256=v.descriptor_sha256
         AND f.domain=v.domain AND f.field_id=v.field_id) LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if orphan_facet.is_some() {
        return Err(Error::Invalid("knowledge catalog orphan facet"));
    }
    let broken_field: Option<i64> = db.query_row(
        "SELECT 1 FROM catalog_facet_fields f WHERE
         f.value_count != (SELECT COUNT(*) FROM catalog_facets v WHERE v.descriptor_sha256=f.descriptor_sha256 AND v.domain=f.domain AND v.field_id=f.field_id)
         OR f.total_count != COALESCE((SELECT SUM(item_count) FROM catalog_facets v WHERE v.descriptor_sha256=f.descriptor_sha256 AND v.domain=f.domain AND v.field_id=f.field_id),0)
         LIMIT 1",
        [],|r|r.get(0),
    ).optional()?;
    if broken_field.is_some() {
        return Err(Error::Invalid("knowledge catalog field aggregates"));
    }
    for (table, count) in [
        ("catalog_facet_fields", fields),
        ("catalog_facets", values),
        ("catalog_routes", routes),
        ("catalog_source_counts", sources),
    ] {
        let sql = match table {
            "catalog_facet_fields" => "SELECT COUNT(*) FROM catalog_facet_fields",
            "catalog_facets" => "SELECT COUNT(*) FROM catalog_facets",
            "catalog_routes" => "SELECT COUNT(*) FROM catalog_routes",
            _ => "SELECT COUNT(*) FROM catalog_source_counts",
        };
        let actual: u64 = db.query_row(sql, [], |r| r.get(0))?;
        if actual != count {
            return Err(Error::Invalid("knowledge catalog foreign descriptor row"));
        }
    }
    Ok(())
}

/// Cold admission consumes the predeclared full-file SHA and SQLite VM budget.
/// It never grants current disclosure rights or source-cut authority by itself.
pub fn open_selected_knowledge_model<'a>(
    path: &Path,
    expected: KnowledgeSelectedExpectation,
    custody: &'a dyn ImmutableKnowledgeCustody,
    limits: ColdOpenLimits,
) -> Result<VerifiedKnowledgeModel<'a>> {
    open_selected_inner(path, expected, CustodyRef::Borrowed(custody), limits)
}

/// Open an already-held producer file through the same complete cold verifier
/// as the path-selected entry point. This crate-private seam lets a producer
/// lend a sealed anonymous copy without exposing an arbitrary public-FD API.
pub(crate) fn open_selected_knowledge_model_pinned<'a>(
    pinned: File,
    expected: KnowledgeSelectedExpectation,
    custody: &'a dyn ImmutableKnowledgeCustody,
    limits: ColdOpenLimits,
) -> Result<VerifiedKnowledgeModel<'a>> {
    validate(&expected, limits)?;
    open_selected_pinned_validated(pinned, expected, CustodyRef::Borrowed(custody), limits)
}

/// Retain one admitted selected generation without borrowing an external
/// self-referential holder. Warm forks share this exact owned custody guard.
pub fn open_selected_knowledge_model_owned(
    path: &Path,
    expected: KnowledgeSelectedExpectation,
    custody: Arc<dyn ImmutableKnowledgeCustody>,
    limits: ColdOpenLimits,
) -> Result<VerifiedKnowledgeModel<'static>> {
    open_selected_inner(path, expected, CustodyRef::Owned(custody), limits)
}

// Layout follows the authenticated selected ABI, never table presence. Old
// admitted ABIs retain their exact Inline path; new cold admission is separate.
fn selected_payload_layout(model_abi: &str) -> crate::knowledge_stage::KnowledgePayloadLayout {
    crate::knowledge_stage::KnowledgePayloadLayout::from_model_abi(&model_abi)
}

fn open_selected_inner<'a>(
    path: &Path,
    expected: KnowledgeSelectedExpectation,
    custody: CustodyRef<'a>,
    limits: ColdOpenLimits,
) -> Result<VerifiedKnowledgeModel<'a>> {
    validate(&expected, limits)?;
    if !path.is_absolute() {
        return Err(Error::Invalid("knowledge selected path"));
    }
    let pinned = safe_open::open_regular(path, expected.model_size_bytes)?;
    open_selected_pinned_validated(pinned, expected, custody, limits)
}

fn open_selected_pinned_validated<'a>(
    mut pinned: File,
    expected: KnowledgeSelectedExpectation,
    custody: CustodyRef<'a>,
    limits: ColdOpenLimits,
) -> Result<VerifiedKnowledgeModel<'a>> {
    let metadata = pinned.metadata()?;
    if !metadata.is_file() || metadata.len() != expected.model_size_bytes {
        return Err(Error::Invalid("knowledge selected held-file type or size"));
    }
    custody.verify(&pinned, &expected)?;
    custody.verify_cold_resources(limits)?;
    pinned.rewind()?;
    let (digest, size) = stream_digest(&mut pinned)?;
    if digest != expected.model_sha256 || size != expected.model_size_bytes {
        return Err(Error::Invalid("knowledge selected SHA/size mismatch"));
    }
    pinned.rewind()?;
    custody.verify_cold_resources(limits)?;
    let (db, counter) = open_sqlite(&pinned, limits.max_vm_steps, limits.sqlite_cache_kib)?;
    let mut work = 0u64;
    verify_integrity(&db)?;
    verify_schema(&db)?;
    check_metadata(&db, &expected, limits.max_metadata_bytes)?;
    let (node_root, relation_root) = verify_core_and_scope(&db, &expected, limits, &mut work)?;
    verify_search(&db, &expected, limits, &mut work)?;
    let source_basis =
        verify_graph_root(&db, &expected, limits, &mut work, node_root, relation_root)?;
    verify_catalog(&db, &expected, limits, &mut work)?;
    let navigation_original =
        crate::knowledge_navigation_original::verify(&db, &expected, limits, &mut work)?;
    let philosophy_original =
        crate::knowledge_philosophy_original::verify(&db, &expected, limits, &mut work)?;
    let corpus_original =
        crate::knowledge_corpus_original::verify(&db, &expected, limits, &mut work)?;
    custody.verify_cold_resources(limits)?;
    custody.verify(&pinned, &expected)?;
    Ok(VerifiedKnowledgeModel {
        connection: db,
        pinned,
        selection: expected,
        source_basis,
        navigation_original,
        philosophy_original,
        corpus_original,
        custody,
        max_cold_vm_steps: limits.max_vm_steps,
        sqlite_cache_kib: limits.sqlite_cache_kib,
        open_vm_steps: counter.load(Ordering::Relaxed),
        cold_digest_read_bytes: size,
        cold_validation_charged_bytes: work,
    })
}

fn verify_integrity(db: &Connection) -> Result<()> {
    let mut statement = db.prepare("PRAGMA integrity_check")?;
    let mut rows = statement.query([])?;
    let first: String = rows
        .next()?
        .ok_or(Error::Invalid("knowledge integrity missing"))?
        .get(0)?;
    if first != "ok" || rows.next()?.is_some() {
        return Err(Error::Invalid("knowledge SQLite integrity"));
    }
    Ok(())
}

/// Acyclic graph root over the exact canonical header packet and the two
/// independently rescanned normalized-core roots. No search/catalog/scope or
/// graph root appears in the header itself.
// Native cut header validation uses the same canonical/type/count/root laws
// as verify_graph_root, with authentic borrowed SQL and original owned state.
fn owned_header_field<'a>(
    value: &'a JsonValue,
    name: &str,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<&'a JsonValue> {
    let _frame = state.hold(std::mem::size_of::<(
        &JsonValue,
        &str,
        &crate::d1_public_capture::CreationState<'_>,
        std::slice::Iter<'_, (tos_foundation::JsonString, JsonValue)>,
        &(tos_foundation::JsonString, JsonValue),
        &str,
        Result<&JsonValue>,
    )>())?;
    let fields = value
        .as_object()
        .ok_or(Error::Invalid("knowledge graph header object"))?;
    state.charge_work(name.len())?;
    for (key, value) in fields {
        state.active()?;
        let key = key
            .as_str()
            .ok_or(Error::Invalid("knowledge graph header key"))?;
        state.charge_work(
            key.len()
                .checked_add(name.len())
                .ok_or(Error::Budget("owned header lookup work"))?,
        )?;
        if key == name {
            return Ok(value);
        }
    }
    Err(Error::Invalid("knowledge graph header field"))
}
fn owned_header_keys(
    value: &JsonValue,
    expected: &[&str],
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<()> {
    let _frame = state.hold(std::mem::size_of::<(
        &JsonValue,
        &[&str],
        &crate::d1_public_capture::CreationState<'_>,
        std::iter::Zip<
            std::slice::Iter<'_, (tos_foundation::JsonString, JsonValue)>,
            std::slice::Iter<'_, &str>,
        >,
        &str,
        &str,
        Result<()>,
    )>())?;
    let fields = value
        .as_object()
        .ok_or(Error::Invalid("knowledge graph header object"))?;
    if fields.len() != expected.len() {
        return Err(Error::Invalid("knowledge graph header keys"));
    }
    for ((key, _), name) in fields.iter().zip(expected.iter()) {
        state.active()?;
        let key = key
            .as_str()
            .ok_or(Error::Invalid("knowledge graph header key"))?;
        if !owned_schema_equal(state, key.as_bytes(), name.as_bytes())? {
            return Err(Error::Invalid("knowledge graph header keys"));
        }
    }
    Ok(())
}
fn owned_header_u64(
    value: &JsonValue,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<Option<u64>> {
    let _frame = state.hold(std::mem::size_of::<(
        &JsonValue,
        &crate::d1_public_capture::CreationState<'_>,
        Option<u64>,
        Result<Option<u64>>,
    )>())?;
    if let JsonValue::Number(number) = value {
        state.charge_work(number.lexeme.len())?;
    }
    state.active()?;
    let value = value.as_u64();
    state.active()?;
    Ok(value)
}
/// Same Native v1 graph header semantics; Managed profiles are not claimed.
/// Escaping T ownership must be persistently pre-admitted by the caller under
/// the same state while SQL/header/tree/canonical/basis owners remain live.
pub(crate) fn verify_native_graph_root_with_owned_context<T>(
    db: &Connection,
    expected: &KnowledgeSelectedExpectation,
    limits: ColdOpenLimits,
    node_root: Digest256,
    relation_root: Digest256,
    context: &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
    consume: impl FnOnce(&crate::KnowledgeSourceBasis) -> Result<T>,
) -> Result<T> {
    let state = context.owned_state();
    let fixed = std::mem::size_of::<(
        &Connection,
        &KnowledgeSelectedExpectation,
        ColdOpenLimits,
        Digest256,
        Digest256,
        &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
        &crate::d1_public_capture::CreationState<'_>,
        JsonLimits,
        JsonLimits,
        JsonLimits,
        T,
        i64,
        i64,
        &[u8],
        &[u8],
        Digest256,
        Digest256Hasher,
        std::slice::Chunks<'_, u8>,
        Result<T>,
        Result<T>,
        &JsonValue,
        &JsonValue,
        &JsonValue,
        &JsonValue,
        usize,
        usize,
        std::slice::Iter<'_, (tos_foundation::JsonString, JsonValue)>,
        std::slice::Iter<'_, ExpectedSourceScope>,
        std::array::IntoIter<&str, 4>,
        &str,
        &str,
        Option<u64>,
    )>()
    .checked_add(std::mem::size_of_val(&consume))
    .ok_or(Error::Budget("native graph header state"))?;
    let _frame = state.hold(fixed)?;
    if expected.managed_source_root_sha256.is_some()
        || limits.max_row_bytes == 0
        || limits.max_row_bytes > i64::MAX as usize
    {
        return Err(Error::Invalid("native cut graph header profile"));
    }
    with_owned_schema_statement(db,c"SELECT singleton,packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,CASE WHEN typeof(packet)='blob' AND length(packet)<=?1 THEN packet ELSE NULL END FROM graph_header",state,|statement| {
        statement.bind_i64(1,limits.max_row_bytes as i64).map_err(owned_schema_sql_error)?;
        if !owned_schema_step(statement,state)? {return Err(Error::Invalid("knowledge graph header missing"));}
        let singleton=statement.integer(0).map_err(owned_schema_sql_error)?;
        let packet_len=statement.integer(1).map_err(owned_schema_sql_error)?;
        let rusqlite::types::ValueRef::Blob(packet_sha)=statement.value_ref(2).map_err(owned_schema_sql_error)? else {return Err(Error::Invalid("knowledge graph header digest"));};
        let rusqlite::types::ValueRef::Blob(packet)=statement.value_ref(3).map_err(owned_schema_sql_error)? else {return Err(Error::Budget("knowledge graph header bytes"));};
        if singleton!=1 || packet_len<0 || usize::try_from(packet_len).ok()!=Some(packet.len()) || packet_sha.len()!=32 {return Err(Error::Invalid("knowledge graph header packet"));}
        state.charge_work(packet.len())?;
        let mut hash=Digest256Hasher::new();
        for chunk in packet.chunks(4096) {state.active()?;hash.update(chunk);}
        let actual_packet_sha=hash.finalize();
        if actual_packet_sha.as_bytes().as_slice()!=packet_sha {return Err(Error::Invalid("knowledge graph header packet"));}
        let visits=context.remaining_json_visits()?.min(packet.len().checked_add(1).ok_or(Error::Budget("graph header parser visits"))?);
        let json_limits=JsonLimits::new(limits.max_row_bytes,96,visits,4096).map_err(|_|Error::Budget("knowledge graph header JSON limits"))?;
        let result=context.with_foundation_owned_with_limits(packet,json_limits,|header| {
            let writer_limits=JsonLimits::new(limits.max_row_bytes,96,context.remaining_json_visits()?.min(1_000_000),4096).map_err(|_|Error::Budget("knowledge graph header writer limits"))?;
            let check_header=|canonical: &[u8]| {
                if !owned_schema_equal(state,canonical,packet)? {return Err(Error::Invalid("knowledge graph header canonical bytes"));}
                crate::managed_source::with_native_cut_header_basis_owned(header,state,|basis| {
                    owned_header_keys(header,&["authority_boundary","counts","normalization_binding","query_properties","schema","source_revision"],state)?;
                    let authority=owned_header_field(header,"authority_boundary",state)?;
                    if authority.as_object().is_none() || owned_header_field(header,"query_properties",state)?.as_array().is_none() {return Err(Error::Invalid("knowledge graph header shape"));}
                    let authority_limits=JsonLimits::new(limits.max_row_bytes,96,context.remaining_json_visits()?.min(1_000_000),4096).map_err(|_|Error::Budget("knowledge graph authority writer limits"))?;
                    let check_authority=|raw: &[u8]| {
                        if !owned_schema_equal(state,raw,expected.authority_boundary.as_bytes())? {return Err(Error::Invalid("knowledge graph header shape"));}
                        Ok(())
                    };
                    let _authority_callback=state.hold(std::mem::size_of_val(&check_authority))?;
                    state.with_foundation_canonical_bytes(authority,authority_limits,check_authority)?;
                    let normalization=owned_header_field(header,"normalization_binding",state)?;
                    owned_header_keys(normalization,&["configuration_digest","entity_registry_digest","processor_digest","relation_registry_digest","schema"],state)?;
                    let schema=owned_header_field(normalization,"schema",state)?.as_str().ok_or(Error::Invalid("knowledge normalization profile"))?;
                    if !owned_schema_equal(state,schema.as_bytes(),b"tos_knowledge_graph_normalization_binding_v1")? {return Err(Error::Invalid("knowledge normalization profile"));}
                    for field in ["configuration_digest","entity_registry_digest","processor_digest","relation_registry_digest"] {
                        let digest=owned_header_field(normalization,field,state)?.as_str().ok_or(Error::Invalid("knowledge normalization digest"))?;
                        state.charge_work(digest.len())?;state.active()?;checked_digest(digest)?;
                    }
                    let counts=owned_header_field(header,"counts",state)?;
                    let sources=owned_header_field(counts,"sources",state)?.as_object().ok_or(Error::Invalid("knowledge graph source counts"))?;
                    let mut source_cursor=0usize;
                    for (key,value) in sources {
                        state.active()?;
                        let name=key.as_str().ok_or(Error::Invalid("knowledge graph source key"))?;
                        while source_cursor<expected.source_scopes.len() && expected.source_scopes[source_cursor].node_count==0 {state.charge_work(1)?;state.active()?;source_cursor+=1;}
                        if source_cursor==expected.source_scopes.len() {return Err(Error::Invalid("knowledge graph source count coverage"));}
                        let scope=&expected.source_scopes[source_cursor];
                        if !owned_schema_equal(state,name.as_bytes(),scope.source_graph.as_bytes())? || owned_header_u64(value,state)?!=Some(scope.node_count) {return Err(Error::Invalid("knowledge graph source count coverage"));}
                        source_cursor+=1;
                    }
                    for scope in &expected.source_scopes[source_cursor..] {state.charge_work(1)?;state.active()?;if scope.node_count>0 {return Err(Error::Invalid("knowledge graph source count omitted"));}}
                    if owned_header_u64(owned_header_field(counts,"nodes",state)?,state)?!=Some(expected.node_count)
                        || owned_header_u64(owned_header_field(counts,"relations",state)?,state)?!=Some(expected.relation_count)
                        || owned_header_field(counts,"display_coverage",state)?.as_object().is_none()
                        || owned_header_field(counts,"semantic_mapping",state)?.as_object().is_none() {return Err(Error::Invalid("knowledge graph header counts"));}
                    state.charge_work(8+b"tos-knowledge-graph-root-v1".len()+32+8+8+32+32+64)?;
                    state.active()?;
                    let mut root=Digest256Hasher::new();
                    hash_text(&mut root,"tos-knowledge-graph-root-v1");
                    root.update(actual_packet_sha.as_bytes());root.update(&expected.node_count.to_be_bytes());root.update(&expected.relation_count.to_be_bytes());root.update(node_root.as_bytes());root.update(relation_root.as_bytes());
                    if root.finalize()!=Digest256::from_hex(&expected.graph_root_sha256).map_err(|_|Error::Invalid("knowledge graph root expectation"))? {return Err(Error::Invalid("knowledge graph root mismatch"));}
                    state.active()?;
                    consume(basis)
                })
            };
            let _header_callback=state.hold(std::mem::size_of_val(&check_header))?;
            state.with_foundation_canonical_bytes(header,writer_limits,check_header)
        });
        // Stop immediately on failure; no further SQL may mask the primary error.
        let value=result?;
        // SQL row/statement and escaping output remain admitted through the fence.
        if owned_schema_step(statement,state)? {return Err(Error::Invalid("knowledge graph header packet"));}
        state.active()?;
        Ok(value)
    })
}

fn verify_graph_root(
    db: &Connection,
    expected: &KnowledgeSelectedExpectation,
    limits: ColdOpenLimits,
    work: &mut u64,
    node_root: Digest256,
    relation_root: Digest256,
) -> Result<crate::KnowledgeSourceBasis> {
    let mut statement = db.prepare(
        "SELECT singleton,packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,CASE WHEN typeof(packet)='blob' AND length(packet)<=?1 THEN packet ELSE NULL END FROM graph_header"
    )?;
    let mut rows = statement.query([limits.max_row_bytes as i64])?;
    let row = rows
        .next()?
        .ok_or(Error::Invalid("knowledge graph header missing"))?;
    let singleton: i64 = row.get(0)?;
    let packet_len: i64 = row.get(1)?;
    let packet_sha: Vec<u8> = row
        .get::<_, Option<Vec<u8>>>(2)?
        .ok_or(Error::Invalid("knowledge graph header digest"))?;
    let packet: Vec<u8> = row
        .get::<_, Option<Vec<u8>>>(3)?
        .ok_or(Error::Budget("knowledge graph header bytes"))?;
    if rows.next()?.is_some()
        || singleton != 1
        || packet_len < 0
        || packet_len as usize != packet.len()
        || packet_sha.as_slice() != Digest256::of_bytes(&packet).as_bytes()
    {
        return Err(Error::Invalid("knowledge graph header packet"));
    }
    charge(work, packet.len(), limits.max_work_bytes)?;
    let json_limits = JsonLimits::new(limits.max_row_bytes, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("knowledge graph header JSON limits"))?;
    let parsed = parse_json(&packet, JsonMode::PublishedStrict, json_limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    let canonical = canonical_bytes_v1(
        parsed.root(),
        CanonicalProfile::SourceRecordDigestV1,
        json_limits,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    if canonical != packet {
        return Err(Error::Invalid("knowledge graph header canonical bytes"));
    }
    let header: serde_json::Value = serde_json::from_slice(&packet)
        .map_err(|_| Error::Invalid("knowledge graph typed header"))?;
    let basis = crate::managed_source::header_basis(&header)?;
    let fields = parsed
        .root()
        .as_object()
        .ok_or(Error::Invalid("knowledge graph header object"))?;
    let names: Vec<&str> = fields
        .iter()
        .map(|(key, _)| key.as_str().unwrap_or(""))
        .collect();
    if names
        != [
            "authority_boundary",
            "counts",
            "normalization_binding",
            "query_properties",
            "schema",
            if basis.managed_proof().is_some() {
                "source_basis"
            } else {
                "source_revision"
            },
        ]
    {
        return Err(Error::Invalid("knowledge graph header keys"));
    }
    let required = |name| {
        parsed
            .root()
            .object_get(name)
            .ok_or(Error::Invalid("knowledge graph header field"))
    };
    let schema = required("schema")?
        .as_str()
        .ok_or(Error::Invalid("knowledge graph schema"))?;
    if let Some(proof) = basis.managed_proof() {
        if expected.managed_source_root_sha256.as_deref() != Some(proof.root_sha256()?.as_str()) {
            return Err(Error::Invalid("knowledge managed source root binding"));
        }
        proof.check_binding(
            &expected.source_cut,
            &expected.membership_root,
            expected.through_commit_seq,
        )?;
    } else if expected.managed_source_root_sha256.is_some() {
        return Err(Error::Invalid("knowledge cut and managed root conflict"));
    }
    let authority_value = required("authority_boundary")?;
    if authority_value.as_object().is_none() {
        return Err(Error::Invalid("knowledge graph authority object"));
    }
    let authority = canonical_bytes_v1(
        authority_value,
        CanonicalProfile::SourceRecordDigestV1,
        json_limits,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    if schema
        != if basis.managed_proof().is_some() {
            crate::managed_source::MANAGED_GRAPH_SCHEMA
        } else {
            "tos_knowledge_graph_v1"
        }
        || authority != expected.authority_boundary.as_bytes()
        || required("query_properties")?.as_array().is_none()
    {
        return Err(Error::Invalid("knowledge graph header shape"));
    }
    let normalization = required("normalization_binding")?;
    let norm_fields = normalization
        .as_object()
        .ok_or(Error::Invalid("knowledge normalization object"))?;
    let norm_names: Vec<&str> = norm_fields
        .iter()
        .map(|(key, _)| key.as_str().unwrap_or(""))
        .collect();
    if norm_names
        != [
            "configuration_digest",
            "entity_registry_digest",
            "processor_digest",
            "relation_registry_digest",
            "schema",
        ]
        || normalization
            .object_get("schema")
            .and_then(JsonValue::as_str)
            != Some("tos_knowledge_graph_normalization_binding_v1")
    {
        return Err(Error::Invalid("knowledge normalization profile"));
    }
    for field in [
        "configuration_digest",
        "entity_registry_digest",
        "processor_digest",
        "relation_registry_digest",
    ] {
        let digest = normalization
            .object_get(field)
            .and_then(JsonValue::as_str)
            .ok_or(Error::Invalid("knowledge normalization digest"))?;
        checked_digest(digest)?;
    }
    // These are semantic digests of parsed registries. The expectation's
    // registry SHA fields bind the original registry bytes in metadata; they
    // are intentionally different digests. The exact graph root and selected
    // file SHA bind this canonical header packet.
    let counts = required("counts")?;
    let sources = counts
        .object_get("sources")
        .and_then(JsonValue::as_object)
        .ok_or(Error::Invalid("knowledge graph source counts"))?;
    let mut source_cursor = 0usize;
    for (key, value) in sources {
        let name = key
            .as_str()
            .ok_or(Error::Invalid("knowledge graph source key"))?;
        while source_cursor < expected.source_scopes.len()
            && expected.source_scopes[source_cursor].node_count == 0
        {
            source_cursor += 1;
        }
        if source_cursor == expected.source_scopes.len()
            || name != expected.source_scopes[source_cursor].source_graph
            || value.as_u64() != Some(expected.source_scopes[source_cursor].node_count)
        {
            return Err(Error::Invalid("knowledge graph source count coverage"));
        }
        source_cursor += 1;
    }
    if expected.source_scopes[source_cursor..]
        .iter()
        .any(|scope| scope.node_count > 0)
    {
        return Err(Error::Invalid("knowledge graph source count omitted"));
    }
    if counts.as_object().is_none()
        || counts.object_get("nodes").and_then(JsonValue::as_u64) != Some(expected.node_count)
        || counts.object_get("relations").and_then(JsonValue::as_u64)
            != Some(expected.relation_count)
        || counts
            .object_get("display_coverage")
            .and_then(JsonValue::as_object)
            .is_none()
        || counts
            .object_get("semantic_mapping")
            .and_then(JsonValue::as_object)
            .is_none()
    {
        return Err(Error::Invalid("knowledge graph header counts"));
    }
    let mut hash = Digest256Hasher::new();
    hash_text(&mut hash, "tos-knowledge-graph-root-v1");
    hash.update(&packet_sha);
    hash.update(&expected.node_count.to_be_bytes());
    hash.update(&expected.relation_count.to_be_bytes());
    hash.update(node_root.as_bytes());
    hash.update(relation_root.as_bytes());
    if hash.finalize().to_hex() != expected.graph_root_sha256 {
        return Err(Error::Invalid("knowledge graph root mismatch"));
    }
    Ok(basis)
}

// Following physical verifiers are deliberately explicit. Missing tables,
// columns or roots must not silently fall back to a partial graph.
// Exact sqlite_master.sql bytes produced by the owned table constructors.
// The selected model ABI changes if any DDL changes: column checks alone do
// not exclude a private DEFAULT expression or payload-bearing SQL comment.
const SELECTED_TABLES: [(&str, &str); 13] = [
    (
        "catalog_facet_fields",
        "1b34c315675e270607c5c157ae346c790f473a598a86241a59a64cfcd2ba6eb3",
    ),
    (
        "catalog_facets",
        "6775bdb46634a4d853a5cb8418109a23181d3dcaba4e3ece8a89934fb00e4cc9",
    ),
    (
        "catalog_index_meta",
        "8e140ee923d3673900116b11bcbef1279ee9f0c2221b74abed8584bae1fcbefb",
    ),
    (
        "catalog_routes",
        "d185ce15df7016146c2c8532d8f0912aa0d7eed48fd1fb6fc01b54e630e29072",
    ),
    (
        "catalog_source_counts",
        "d12fe5c97123ab8bacb96d87a8adcd61b0bb5edc3ce44658ed47766ff087643a",
    ),
    (
        "graph_header",
        "5302e4b65d084a575885cfa418d5e632662d5d83b77166f12c3b5a395698a7bb",
    ),
    (
        "knowledge_nodes",
        "34b58af97bfc61e662e2d1c1cb6ed74b9b092dcf763f305102b3d67e0a5f0a19",
    ),
    (
        "knowledge_relations",
        "5470a4a6ca623be58d3df41f31809b224e1d0d1c2f941e19a73b0c42333f079a",
    ),
    (
        "metadata",
        "6c4078f02b0cfa4433e523de5a92045a65d0d0352d4c89924e9fd3718c731e47",
    ),
    (
        "search_documents",
        "0b1b8edf910d7be7f780e1b2777f50055879d43b098724bfcfd9f0e992e2c9cd",
    ),
    (
        "search_gram_stats",
        "92de863c3d48671f36580e57f70252d4c7048f1bc4ae360cf302075a643a14db",
    ),
    (
        "search_posting_blocks",
        "56dc93c046acfec0667d5ee37ea86320d69609fcc8b138243dfd88a50aa9e6ad",
    ),
    (
        "source_scope",
        "7484cf6fb366756db8edc732e4c46ca01800bcdade76c7b6a85e7948b5bfe8f4",
    ),
];

const SELECTED_COLUMN_SPECS: &[(&str, &[&str])] = &[
    ("metadata", &["key:TEXT:1", "value:BLOB:0"]),
    (
        "graph_header",
        &[
            "singleton:INTEGER:1",
            "packet_len:INTEGER:0",
            "packet_sha256:BLOB:0",
            "packet:BLOB:0",
        ],
    ),
    (
        "knowledge_nodes",
        &[
            "id:TEXT:1",
            "source_graph:TEXT:0",
            "native_id:TEXT:0",
            "entity_id:TEXT:0",
            "kind_id:TEXT:0",
            "type_id:TEXT:0",
            "source_order:INTEGER:0",
            "payload_len:INTEGER:0",
            "payload_sha256:BLOB:0",
            "payload:BLOB:0",
        ],
    ),
    (
        "knowledge_relations",
        &[
            "id:TEXT:1",
            "source_graph:TEXT:0",
            "native_id:TEXT:0",
            "from_id:TEXT:0",
            "to_id:TEXT:0",
            "predicate_id:TEXT:0",
            "relation_type_id:TEXT:0",
            "source_order:INTEGER:0",
            "payload_len:INTEGER:0",
            "payload_sha256:BLOB:0",
            "payload:BLOB:0",
        ],
    ),
    (
        "source_scope",
        &[
            "source_graph:TEXT:1",
            "input_role:TEXT:0",
            "adapter_profile:TEXT:0",
            "expected_node_count:INTEGER:0",
            "expected_relation_count:INTEGER:0",
            "node_root_sha256:BLOB:0",
            "relation_root_sha256:BLOB:0",
        ],
    ),
    (
        "search_documents",
        &[
            "kind:TEXT:1",
            "position:INTEGER:2",
            "id:TEXT:0",
            "source_graph:TEXT:0",
            "kind_id:TEXT:0",
            "predicate_id:TEXT:0",
            "id_lower:TEXT:0",
            "native_id_lower:TEXT:0",
            "identity_values:TEXT:0",
            "visible_values:TEXT:0",
            "document_chars:INTEGER:0",
            "document_digest:BLOB:0",
        ],
    ),
    (
        "search_posting_blocks",
        &[
            "kind:TEXT:1",
            "n:INTEGER:2",
            "gram:BLOB:3",
            "last_position:INTEGER:4",
            "first_position:INTEGER:0",
            "postings:INTEGER:0",
            "deltas:BLOB:0",
        ],
    ),
    (
        "search_gram_stats",
        &[
            "kind:TEXT:1",
            "n:INTEGER:2",
            "gram:BLOB:3",
            "postings:INTEGER:0",
        ],
    ),
    (
        "catalog_index_meta",
        &[
            "descriptor_sha256:TEXT:1",
            "catalog_packet_sha256:TEXT:0",
            "index_schema:TEXT:0",
            "order_profile:TEXT:0",
            "source_count:INTEGER:0",
            "facet_field_count:INTEGER:0",
            "facet_value_count:INTEGER:0",
            "route_count:INTEGER:0",
            "catalog_index_root_sha256:TEXT:0",
            "packet_len:INTEGER:0",
            "packet_sha256:BLOB:0",
            "packet:BLOB:0",
        ],
    ),
    (
        "catalog_facet_fields",
        &[
            "descriptor_sha256:TEXT:1",
            "domain:TEXT:2",
            "field_id:TEXT:3",
            "value_count:INTEGER:0",
            "total_count:INTEGER:0",
        ],
    ),
    (
        "catalog_facets",
        &[
            "descriptor_sha256:TEXT:1",
            "domain:TEXT:2",
            "field_id:TEXT:3",
            "ordinal:INTEGER:4",
            "value_json:TEXT:0",
            "item_count:INTEGER:0",
        ],
    ),
    (
        "catalog_routes",
        &[
            "descriptor_sha256:TEXT:1",
            "route_id:TEXT:2",
            "ordinal:INTEGER:0",
            "node_count:INTEGER:0",
            "confirming_relation_count:INTEGER:0",
            "semantic_confirming_relation_count:INTEGER:0",
            "availability:TEXT:0",
            "role_readiness:TEXT:0",
            "packet_len:INTEGER:0",
            "packet_sha256:BLOB:0",
            "packet:BLOB:0",
        ],
    ),
    (
        "catalog_source_counts",
        &[
            "descriptor_sha256:TEXT:1",
            "source_graph_id:TEXT:2",
            "node_count:INTEGER:0",
            "relation_count:INTEGER:0",
        ],
    ),
];

fn verify_selected_table_allowlist(db: &Connection) -> Result<()> {
    // The selected file contains only the read model. In particular, private
    // raw input and intermediate build tables must not survive publication.
    let optional = crate::knowledge_navigation_original::present(db)?;
    let mut expected: Vec<_> = SELECTED_TABLES.iter().map(|(name, _)| *name).collect();
    if optional {
        expected.extend([
            crate::knowledge_navigation_original::META_TABLE,
            crate::knowledge_navigation_original::ROW_TABLE,
            crate::knowledge_navigation_original::MEMBER_TABLE,
        ]);
    }
    if crate::knowledge_philosophy_original::present(db)? {
        expected.extend([
            crate::knowledge_philosophy_original::META_TABLE,
            crate::knowledge_philosophy_original::ROW_TABLE,
        ]);
    }
    if crate::knowledge_corpus_original::present(db)? {
        expected.extend([
            crate::knowledge_corpus_original::META_TABLE,
            crate::knowledge_corpus_original::ROW_TABLE,
        ]);
    }
    expected.sort_unstable();
    let mut table_statement=db.prepare("SELECT CASE WHEN typeof(name)='text' AND length(CAST(name AS BLOB))<=128 THEN name ELSE NULL END FROM sqlite_master WHERE type='table' ORDER BY name")?;
    let mut rows = table_statement.query([])?;
    for name in expected {
        let actual: Option<String> = rows
            .next()?
            .ok_or(Error::Invalid("knowledge table omitted"))?
            .get(0)?;
        if actual.as_deref() != Some(name) {
            return Err(Error::Invalid("knowledge selected table allowlist"));
        }
    }
    if rows.next()?.is_some() {
        return Err(Error::Invalid("knowledge selected extra table"));
    }

    Ok(())
}

pub(crate) fn verify_schema(db: &Connection) -> Result<()> {
    verify_selected_table_allowlist(db)?;
    knowledge_stage::selected_table_closure(db)?;
    if crate::knowledge_navigation_original::present(db)? {
        crate::knowledge_navigation_original::verify_ddl(db)?;
    }
    if crate::knowledge_philosophy_original::present(db)? {
        crate::knowledge_philosophy_original::verify_ddl(db)?;
    }
    if crate::knowledge_corpus_original::present(db)? {
        crate::knowledge_corpus_original::verify_ddl(db)?;
    }
    for (table, expected_ddl_sha256) in SELECTED_TABLES {
        let ddl: Option<Vec<u8>> = db
            .query_row(
                "SELECT CASE WHEN typeof(sql)='text' AND length(CAST(sql AS BLOB))<=65536
                 THEN CAST(sql AS BLOB) ELSE NULL END
                 FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let ddl = ddl.ok_or(Error::Invalid("selected table DDL absent/oversized"))?;
        if Digest256::of_bytes(&ddl).to_hex() != expected_ddl_sha256 {
            return Err(Error::Invalid("selected table DDL differs from model ABI"));
        }
    }
    for (table, columns) in SELECTED_COLUMN_SPECS.iter().copied() {
        let create: Option<String> = db
            .query_row(
                "SELECT CASE WHEN typeof(sql)='text' AND length(CAST(sql AS BLOB))<=65536 THEN sql ELSE NULL END
                 FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        let create = create.ok_or(Error::Invalid("knowledge table DDL absent/oversized"))?;
        if table != "graph_header" && !create.to_ascii_uppercase().contains("WITHOUT ROWID") {
            return Err(Error::Invalid("knowledge table rowid policy"));
        }
        let pragma = format!("PRAGMA table_xinfo({table})");
        let mut statement = db.prepare(&pragma)?;
        let mut rows = statement.query([])?;
        for spec in columns {
            let row = rows
                .next()?
                .ok_or(Error::Invalid("knowledge table column missing"))?;
            let mut parts = spec.split(':');
            let name: String = row.get(1)?;
            let ty: String = row.get(2)?;
            let notnull: i64 = row.get(3)?;
            let pk: i64 = row.get(5)?;
            let hidden: i64 = row.get(6)?;
            let nullable = matches!(
                (table, name.as_str()),
                ("graph_header", "singleton")
                    | ("knowledge_nodes", "native_id" | "entity_id")
                    | ("knowledge_relations", "native_id")
            );
            if name != parts.next().unwrap()
                || ty.to_ascii_uppercase() != parts.next().unwrap()
                || pk.to_string() != parts.next().unwrap()
                || notnull != i64::from(!nullable)
                || hidden != 0
            {
                return Err(Error::Invalid("knowledge table column shape"));
            }
        }
        if rows.next()?.is_some() {
            return Err(Error::Invalid("knowledge table extra column"));
        }
    }
    // These exact index names are used in bounded seek SQL. A missing index
    // must fail rather than turn a query into an unbounded table scan.
    for name in [
        "knowledge_nodes_source_order",
        "knowledge_nodes_kind",
        "knowledge_nodes_entity",
        "knowledge_nodes_native",
        "knowledge_nodes_entity_id",
        "knowledge_relations_native",
        "knowledge_relations_source_order",
        "knowledge_relations_from",
        "knowledge_relations_to",
        "knowledge_relations_from_id",
        "knowledge_relations_to_id",
        "knowledge_relations_predicate",
        "search_document_filter",
    ] {
        let present: Option<i64> = db
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='index' AND name=?1",
                [name],
                |r| r.get(0),
            )
            .optional()?;
        if present != Some(1) {
            return Err(Error::Invalid("knowledge index absent"));
        }
    }
    Ok(())
}

// A borrowed statement remains inside its actual owner; all Rust/native error
// paths are bounded before work. The caller installs the same prepaid SQL hook.
pub(crate) fn with_owned_schema_statement<T>(
    db: &Connection,
    sql: &std::ffi::CStr,
    state: &crate::d1_public_capture::CreationState<'_>,
    consume: impl FnOnce(&mut tos_source_store::PinnedBoundedStatement<'_>) -> Result<T>,
) -> Result<T> {
    let fixed =
        tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound()
            .checked_add(std::mem::size_of::<(
                &Connection,
                &std::ffi::CStr,
                &crate::d1_public_capture::CreationState<'_>,
                Result<T>,
                Result<T>,
            )>())
            .and_then(|n| n.checked_add(std::mem::size_of_val(&consume)))
            .ok_or(Error::Budget("owned schema statement state"))?;
    let _hold = state.hold(fixed)?;
    state.active()?;
    state.charge_work(sql.to_bytes().len())?;
    let mut statement =
        tos_source_store::PinnedBoundedStatement::prepare_on_owned_connection(db, sql)
            .map_err(owned_schema_sql_error)?;
    let result = consume(&mut statement);
    state.active()?;
    result
}
pub(crate) fn owned_schema_sql_error(error: tos_source_store::StoreError) -> Error {
    if error.code == tos_source_store::StoreErrorCode::BudgetExceeded {
        Error::Budget("owned selected schema SQL")
    } else {
        Error::Invalid("owned selected schema SQL")
    }
}
pub(crate) fn owned_schema_step(
    statement: &mut tos_source_store::PinnedBoundedStatement<'_>,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<bool> {
    let _frame = state.hold(std::mem::size_of::<(
        &mut tos_source_store::PinnedBoundedStatement<'_>,
        &crate::d1_public_capture::CreationState<'_>,
        bool,
        Result<bool>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    state.active()?;
    let row = statement.step().map_err(owned_schema_sql_error)?;
    state.active()?;
    Ok(row)
}
pub(crate) fn owned_schema_equal(
    state: &crate::d1_public_capture::CreationState<'_>,
    a: &[u8],
    b: &[u8],
) -> Result<bool> {
    let _frame = state.hold(std::mem::size_of::<(
        &crate::d1_public_capture::CreationState<'_>,
        &[u8],
        &[u8],
        std::iter::Zip<std::slice::Chunks<'_, u8>, std::slice::Chunks<'_, u8>>,
        (&[u8], &[u8]),
        bool,
        Result<bool>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    state.charge_work(
        a.len()
            .checked_add(b.len())
            .ok_or(Error::Budget("schema comparison work"))?,
    )?;
    state.active()?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (left, right) in a.chunks(4096).zip(b.chunks(4096)) {
        state.active()?;
        if left != right {
            return Ok(false);
        }
    }
    state.active()?;
    Ok(true)
}
fn owned_schema_present(
    db: &Connection,
    table: &str,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<bool> {
    with_owned_schema_statement(
        db,
        c"SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
        state,
        |statement| {
            state.charge_work(table.len())?;
            statement
                .bind_text(1, table)
                .map_err(owned_schema_sql_error)?;
            let found = owned_schema_step(statement, state)?;
            if found && owned_schema_step(statement, state)? {
                return Err(Error::Invalid("selected duplicate table"));
            }
            Ok(found)
        },
    )
}
fn owned_schema_ddl(
    db: &Connection,
    table: &str,
    expected_hash: Option<&str>,
    expected_sql: Option<&str>,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<()> {
    let _hold = state.hold(std::mem::size_of::<(
        &Connection,
        &str,
        Option<&str>,
        Option<&str>,
        &crate::d1_public_capture::CreationState<'_>,
        Digest256Hasher,
        Digest256,
        &[u8],
        std::slice::Chunks<'_, u8>,
        Result<()>,
    )>())?;
    with_owned_schema_statement(db, c"SELECT CASE WHEN typeof(sql)='text' AND length(CAST(sql AS BLOB))<=4096 THEN CAST(sql AS BLOB) ELSE NULL END FROM sqlite_master WHERE type='table' AND name=?1", state, |statement| {
        state.charge_work(table.len())?;
        statement.bind_text(1, table).map_err(owned_schema_sql_error)?;
        if !owned_schema_step(statement, state)? { return Err(Error::Invalid("selected DDL omitted")); }
        let rusqlite::types::ValueRef::Blob(raw) = statement.value_ref(0).map_err(owned_schema_sql_error)? else { return Err(Error::Invalid("selected DDL type/size")); };
        if let Some(expected) = expected_sql {
            if !owned_schema_equal(state, raw, expected.as_bytes())? { return Err(Error::Invalid("selected Original DDL differs")); }
        } else if let Some(expected) = expected_hash {
            state.charge_work(raw.len())?;
            let mut hash = Digest256Hasher::new();
            for chunk in raw.chunks(4096) { state.active()?; hash.update(chunk); }
            if hash.finalize() != Digest256::from_hex(expected).map_err(|_| Error::Invalid("selected expected DDL hash"))? {
                return Err(Error::Invalid("selected DDL hash differs"));
            }
        } else { return Err(Error::Invalid("selected DDL expectation absent")); }
        if owned_schema_step(statement, state)? { return Err(Error::Invalid("selected duplicate DDL")); }
        Ok(())
    })
}
fn owned_schema_column_sql(table: &str) -> Result<&'static std::ffi::CStr> {
    Ok(match table {
        "metadata" => c"PRAGMA table_xinfo(metadata)",
        "graph_header" => c"PRAGMA table_xinfo(graph_header)",
        "knowledge_nodes" => c"PRAGMA table_xinfo(knowledge_nodes)",
        "knowledge_relations" => c"PRAGMA table_xinfo(knowledge_relations)",
        "source_scope" => c"PRAGMA table_xinfo(source_scope)",
        "search_documents" => c"PRAGMA table_xinfo(search_documents)",
        "search_posting_blocks" => c"PRAGMA table_xinfo(search_posting_blocks)",
        "search_gram_stats" => c"PRAGMA table_xinfo(search_gram_stats)",
        "catalog_index_meta" => c"PRAGMA table_xinfo(catalog_index_meta)",
        "catalog_facet_fields" => c"PRAGMA table_xinfo(catalog_facet_fields)",
        "catalog_facets" => c"PRAGMA table_xinfo(catalog_facets)",
        "catalog_routes" => c"PRAGMA table_xinfo(catalog_routes)",
        "catalog_source_counts" => c"PRAGMA table_xinfo(catalog_source_counts)",
        "knowledge_source_carriers" => c"PRAGMA table_xinfo(knowledge_source_carriers)",
        _ => return Err(Error::Invalid("owned schema column table")),
    })
}
fn owned_schema_columns(
    db: &Connection,
    table: &str,
    columns: &[&str],
    extra: &[&str],
    layout: knowledge_stage::KnowledgePayloadLayout,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<()> {
    let _hold = state.hold(std::mem::size_of::<(
        &Connection,
        &str,
        &[&str],
        &[&str],
        knowledge_stage::KnowledgePayloadLayout,
        &crate::d1_public_capture::CreationState<'_>,
        std::iter::Chain<std::slice::Iter<'_, &str>, std::slice::Iter<'_, &str>>,
        std::str::Split<'_, char>,
        Option<&str>,
        Option<&str>,
        Option<&str>,
        rusqlite::types::ValueRef<'_>,
        rusqlite::types::ValueRef<'_>,
        i64,
        i64,
        i64,
        bool,
        Result<()>,
    )>())?;
    with_owned_schema_statement(db, owned_schema_column_sql(table)?, state, |statement| {
        for spec in columns.iter().chain(extra.iter()) {
            state.active()?;
            if !owned_schema_step(statement, state)? {
                return Err(Error::Invalid("selected column omitted"));
            }
            let mut parts = spec.split(':');
            let name = parts
                .next()
                .ok_or(Error::Invalid("column expectation name"))?;
            let ty = parts
                .next()
                .ok_or(Error::Invalid("column expectation type"))?;
            let pk = match parts.next() {
                Some("0") => 0,
                Some("1") => 1,
                Some("2") => 2,
                Some("3") => 3,
                Some("4") => 4,
                _ => return Err(Error::Invalid("column expectation PK")),
            };
            let rusqlite::types::ValueRef::Text(raw_name) =
                statement.value_ref(1).map_err(owned_schema_sql_error)?
            else {
                return Err(Error::Invalid("selected column name type"));
            };
            if !owned_schema_equal(state, raw_name, name.as_bytes())? {
                return Err(Error::Invalid("selected column name"));
            }
            let rusqlite::types::ValueRef::Text(raw_ty) =
                statement.value_ref(2).map_err(owned_schema_sql_error)?
            else {
                return Err(Error::Invalid("selected column type"));
            };
            state.charge_work(
                raw_ty
                    .len()
                    .checked_add(ty.len())
                    .ok_or(Error::Budget("column type comparison"))?,
            )?;
            let type_equal = raw_ty.eq_ignore_ascii_case(ty.as_bytes());
            state.active()?;
            let nullable = matches!(
                (table, name),
                ("graph_header", "singleton")
                    | ("knowledge_nodes", "native_id" | "entity_id")
                    | ("knowledge_relations", "native_id")
            ) || layout.uses_carriers()
                && name == "source_packet_sha256"
                && matches!(table, "knowledge_nodes" | "knowledge_relations");
            if !type_equal
                || statement.integer(3).map_err(owned_schema_sql_error)? != i64::from(!nullable)
                || statement.integer(5).map_err(owned_schema_sql_error)? != pk
                || statement.integer(6).map_err(owned_schema_sql_error)? != 0
            {
                return Err(Error::Invalid("selected column shape"));
            }
        }
        if owned_schema_step(statement, state)? {
            return Err(Error::Invalid("selected extra column"));
        }
        Ok(())
    })
}
/// Layout is selected by the authenticated expected ABI, never schema census.
/// Some(state) is mandatory for Carrier and selects this original-owner path.
/// The caller retains the SQLite pool and installs its prepaid SQL controller.
pub(crate) fn verify_schema_with_layout(
    db: &Connection,
    layout: knowledge_stage::KnowledgePayloadLayout,
    state: Option<&crate::d1_public_capture::CreationState<'_>>,
) -> Result<()> {
    let Some(state) = state else {
        return if layout == knowledge_stage::KnowledgePayloadLayout::InlineV1 {
            verify_schema(db)
        } else {
            Err(Error::Invalid("carrier schema requires owned state"))
        };
    };
    type Frame<'a> = (
        &'a Connection,
        knowledge_stage::KnowledgePayloadLayout,
        &'a crate::d1_public_capture::CreationState<'a>,
        [Option<&'static str>; 32],
        [bool; 32],
        usize,
        usize,
        usize,
        bool,
        bool,
        bool,
        Option<usize>,
        std::slice::Iter<'a, (&'a str, &'a str)>,
        std::iter::Enumerate<
            std::iter::Chain<
                std::slice::Iter<'a, (&'a str, &'a str)>,
                std::slice::Iter<'a, (&'a str, &'a str)>,
            >,
        >,
        std::iter::Enumerate<std::slice::Iter<'a, Option<&'static str>>>,
        std::slice::Iter<'a, bool>,
        std::array::IntoIter<(bool, &'a [&'a str]), 3>,
        std::slice::Iter<'a, &'a str>,
        std::array::IntoIter<(&'a str, &'a str), 3>,
        std::iter::Copied<std::slice::Iter<'a, (&'a str, &'a [&'a str])>>,
        Result<()>,
    );
    let _frame = state.hold(std::mem::size_of::<Frame<'_>>())?;
    state.active()?;
    let navigation =
        owned_schema_present(db, crate::knowledge_navigation_original::META_TABLE, state)?;
    let philosophy =
        owned_schema_present(db, crate::knowledge_philosophy_original::META_TABLE, state)?;
    let corpus = owned_schema_present(db, crate::knowledge_corpus_original::META_TABLE, state)?;
    let carrier = layout.uses_carriers();
    let mut tables: [Option<&str>; 32] = [None; 32];
    let mut count = 0usize;
    for (table, _) in SELECTED_TABLES {
        tables[count] = Some(table);
        count += 1;
    }
    if carrier {
        tables[count] = Some("knowledge_source_carriers");
        count += 1;
    }
    for (present, names) in [
        (
            navigation,
            &[
                crate::knowledge_navigation_original::META_TABLE,
                crate::knowledge_navigation_original::ROW_TABLE,
                crate::knowledge_navigation_original::MEMBER_TABLE,
            ][..],
        ),
        (
            philosophy,
            &[
                crate::knowledge_philosophy_original::META_TABLE,
                crate::knowledge_philosophy_original::ROW_TABLE,
            ][..],
        ),
        (
            corpus,
            &[
                crate::knowledge_corpus_original::META_TABLE,
                crate::knowledge_corpus_original::ROW_TABLE,
            ][..],
        ),
    ] {
        if present {
            for name in names {
                if count == tables.len() {
                    return Err(Error::Budget("selected table controller"));
                }
                tables[count] = Some(name);
                count += 1;
            }
        }
    }
    let mut seen = [false; 32];
    with_owned_schema_statement(db, c"SELECT CASE WHEN typeof(name)='text' AND length(CAST(name AS BLOB))<=128 THEN CAST(name AS BLOB) ELSE NULL END FROM sqlite_master WHERE type='table' ORDER BY name", state, |statement| {
        while owned_schema_step(statement, state)? {
            let rusqlite::types::ValueRef::Blob(name) = statement.value_ref(0).map_err(owned_schema_sql_error)? else { return Err(Error::Invalid("selected table name type/bytes")); };
            let mut found = None;
            for (i, expected) in tables[..count].iter().enumerate() {
                if owned_schema_equal(state, name, expected.ok_or(Error::Invalid("selected table controller slot"))?.as_bytes())? { found = Some(i); break; }
            }
            let index = found.ok_or(Error::Invalid("selected extra table"))?;
            if seen[index] { return Err(Error::Invalid("selected duplicate table")); }
            seen[index] = true;
        }
        if seen[..count].iter().any(|value| !value) { return Err(Error::Invalid("selected table omitted")); }
        Ok(())
    })?;
    knowledge_stage::verify_selected_payload_ddl(db, layout, Some(state))?;
    for (table, hash) in SELECTED_TABLES {
        if !(carrier && matches!(table, "knowledge_nodes" | "knowledge_relations")) {
            owned_schema_ddl(db, table, Some(hash), None, state)?;
        }
    }
    if navigation {
        for (name, ddl) in [
            (
                crate::knowledge_navigation_original::META_TABLE,
                crate::knowledge_navigation_original::META_DDL,
            ),
            (
                crate::knowledge_navigation_original::ROW_TABLE,
                crate::knowledge_navigation_original::ROW_DDL,
            ),
            (
                crate::knowledge_navigation_original::MEMBER_TABLE,
                crate::knowledge_navigation_original::MEMBER_DDL,
            ),
        ] {
            owned_schema_ddl(db, name, None, Some(ddl), state)?;
        }
    }
    if philosophy {
        owned_schema_ddl(
            db,
            crate::knowledge_philosophy_original::META_TABLE,
            None,
            Some(crate::knowledge_philosophy_original::META_DDL),
            state,
        )?;
        owned_schema_ddl(
            db,
            crate::knowledge_philosophy_original::ROW_TABLE,
            None,
            Some(if carrier {
                crate::knowledge_philosophy_original::ROW_DDL_CARRIER
            } else {
                crate::knowledge_philosophy_original::ROW_DDL
            }),
            state,
        )?;
    }
    if corpus {
        owned_schema_ddl(
            db,
            crate::knowledge_corpus_original::META_TABLE,
            None,
            Some(crate::knowledge_corpus_original::META_DDL),
            state,
        )?;
        owned_schema_ddl(
            db,
            crate::knowledge_corpus_original::ROW_TABLE,
            None,
            Some(if carrier {
                crate::knowledge_corpus_original::ROW_DDL_CARRIER
            } else {
                crate::knowledge_corpus_original::ROW_DDL
            }),
            state,
        )?;
    }
    for (table, columns) in SELECTED_COLUMN_SPECS.iter().copied() {
        owned_schema_columns(
            db,
            table,
            columns,
            if carrier && matches!(table, "knowledge_nodes" | "knowledge_relations") {
                &["payload_codec:INTEGER:0", "source_packet_sha256:BLOB:0"]
            } else {
                &[]
            },
            layout,
            state,
        )?;
    }
    if carrier {
        owned_schema_columns(
            db,
            "knowledge_source_carriers",
            &[
                "packet_sha256:BLOB:1",
                "packet_len:INTEGER:0",
                "packet:BLOB:0",
            ],
            &[],
            layout,
            state,
        )?;
    }
    let index_count = knowledge_stage::SELECTED_EXPLICIT_INDEXES.len()
        + if corpus {
            crate::knowledge_corpus_original::INDEXES.len()
        } else {
            0
        };
    if index_count > seen.len() {
        return Err(Error::Budget("selected index controller"));
    }
    seen.fill(false);
    with_owned_schema_statement(db,c"SELECT CASE WHEN typeof(name)='text' AND length(CAST(name AS BLOB))<=128 THEN CAST(name AS BLOB) ELSE NULL END, CASE WHEN typeof(sql)='text' AND length(CAST(sql AS BLOB))<=1024 THEN CAST(sql AS BLOB) ELSE NULL END FROM sqlite_master WHERE type='index' AND sql IS NOT NULL ORDER BY name",state,|statement| {
        while owned_schema_step(statement,state)? {
            let rusqlite::types::ValueRef::Blob(name)=statement.value_ref(0).map_err(owned_schema_sql_error)? else {return Err(Error::Invalid("selected index name type/bytes"));};
            let rusqlite::types::ValueRef::Blob(sql)=statement.value_ref(1).map_err(owned_schema_sql_error)? else {return Err(Error::Invalid("selected index SQL type/bytes"));};
            let mut found=None;
            for (i,(expected_name,expected_sql)) in knowledge_stage::SELECTED_EXPLICIT_INDEXES.iter().chain(crate::knowledge_corpus_original::INDEXES[..if corpus {crate::knowledge_corpus_original::INDEXES.len()} else {0}].iter()).enumerate() {
                if owned_schema_equal(state,name,expected_name.as_bytes())? && owned_schema_equal(state,sql,expected_sql.as_bytes())? { found=Some(i);break; }
            }
            let i=found.ok_or(Error::Invalid("selected extra/different index"))?;
            if seen[i] {return Err(Error::Invalid("selected duplicate index"));} seen[i]=true;
        }
        if seen[..index_count].iter().any(|value| !value) {return Err(Error::Invalid("selected index omitted"));}
        Ok(())
    })?;
    with_owned_schema_statement(
        db,
        c"SELECT 1 FROM sqlite_master WHERE type NOT IN ('table','index') LIMIT 1",
        state,
        |statement| {
            if owned_schema_step(statement, state)? {
                return Err(Error::Invalid("selected extra schema object"));
            }
            Ok(())
        },
    )?;
    state.active()
}

fn owned_core_compare(
    state: &crate::d1_public_capture::CreationState<'_>,
    left: &[u8],
    right: &[u8],
) -> Result<std::cmp::Ordering> {
    let _frame = state.hold(std::mem::size_of::<(
        &crate::d1_public_capture::CreationState<'_>,
        &[u8],
        &[u8],
        std::iter::Zip<std::slice::Chunks<'_, u8>, std::slice::Chunks<'_, u8>>,
        (&[u8], &[u8]),
        std::cmp::Ordering,
        Result<std::cmp::Ordering>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    state.charge_work(left.len().min(right.len()))?;
    for (a, b) in left.chunks(4096).zip(right.chunks(4096)) {
        state.active()?;
        let order = a.cmp(b);
        if order != std::cmp::Ordering::Equal {
            return Ok(order);
        }
    }
    state.active()?;
    Ok(left.len().cmp(&right.len()))
}
fn owned_core_hash_item(
    state: &crate::d1_public_capture::CreationState<'_>,
    hash: &mut Digest256Hasher,
    id: &str,
    digest: &[u8],
) -> Result<()> {
    let _frame = state.hold(std::mem::size_of::<(
        &crate::d1_public_capture::CreationState<'_>,
        &mut Digest256Hasher,
        &str,
        &[u8],
        std::slice::Chunks<'_, u8>,
        &[u8],
        Result<()>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    state.charge_work(
        id.len()
            .checked_add(40)
            .ok_or(Error::Budget("core hash work"))?,
    )?;
    hash.update(&(id.len() as u64).to_be_bytes());
    for chunk in id.as_bytes().chunks(4096) {
        state.active()?;
        hash.update(chunk);
    }
    state.active()?;
    hash.update(digest);
    Ok(())
}
/// Destination bytes and its persistent String controller are already held
/// by the caller; this helper holds only its own activation through transfer.
fn owned_core_key_copy(
    state: &crate::d1_public_capture::CreationState<'_>,
    input: &str,
) -> Result<String> {
    let _frame = state.hold(std::mem::size_of::<(
        &crate::d1_public_capture::CreationState<'_>,
        &str,
        String,
        std::str::Chars<'_>,
        char,
        Result<String>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    state.charge_work(input.len())?;
    state.active()?;
    let mut output = String::new();
    output
        .try_reserve_exact(input.len())
        .map_err(|_| Error::Budget("core key allocation"))?;
    for value in input.chars() {
        state.active()?;
        output.push(value);
    }
    state.active()?;
    Ok(output)
}
/// Scans the actual normalized rows under the verified explicit layout. Physical
/// envelope/carrier borrowing survives logical hydration and hash verification;
/// no physical payload is mistaken for the logical normalized packet.
fn scan_core_with_owned_context(
    db: &Connection,
    nodes: bool,
    layout: knowledge_stage::KnowledgePayloadLayout,
    expected: &KnowledgeSelectedExpectation,
    accumulators: &mut [ScopeAcc],
    global: &mut Digest256Hasher,
    limits: ColdOpenLimits,
    context: &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
) -> Result<u64> {
    let state = context.owned_state();
    let fixed = std::mem::size_of::<(
        &Connection,
        &std::ffi::CStr,
        bool,
        knowledge_stage::KnowledgePayloadLayout,
        &KnowledgeSelectedExpectation,
        &mut [ScopeAcc],
        &mut Digest256Hasher,
        ColdOpenLimits,
        &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
        &crate::d1_public_capture::CreationState<'_>,
        u64,
        usize,
        Option<(
            String,
            String,
            crate::d1_public_capture::CreationStateHold<'_, '_>,
        )>,
        &str,
        &str,
        i64,
        i64,
        &[u8],
        &[u8],
        i64,
        Option<Digest256>,
        Option<&[u8]>,
        crate::knowledge_payload_read::SelectedPayloadRow<'_>,
        JsonLimits,
        JsonLimits,
        std::slice::Chunks<'_, u8>,
        Digest256Hasher,
        Digest256,
        usize,
        String,
        String,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
        Result<u64>,
        Result<()>,
        std::cmp::Ordering,
    )>();
    let _fixed = state.hold(fixed)?;
    let sql=match (nodes,layout) {
        (true,knowledge_stage::KnowledgePayloadLayout::InlineV1)=>c"SELECT id,source_graph,source_order,payload_len,payload_sha256,payload,0,NULL,NULL,NULL FROM knowledge_nodes ORDER BY source_order",
        (false,knowledge_stage::KnowledgePayloadLayout::InlineV1)=>c"SELECT id,source_graph,source_order,payload_len,payload_sha256,payload,0,NULL,NULL,NULL FROM knowledge_relations ORDER BY source_order",
        (true,knowledge_stage::KnowledgePayloadLayout::CarrierOnceV1 | knowledge_stage::KnowledgePayloadLayout::CarrierOnceV2)=>c"SELECT n.id,n.source_graph,n.source_order,n.payload_len,n.payload_sha256,n.payload,n.payload_codec,n.source_packet_sha256,c.packet_len,c.packet FROM knowledge_nodes n LEFT JOIN knowledge_source_carriers c ON c.packet_sha256=n.source_packet_sha256 ORDER BY n.source_order",
        (false,knowledge_stage::KnowledgePayloadLayout::CarrierOnceV1 | knowledge_stage::KnowledgePayloadLayout::CarrierOnceV2)=>c"SELECT n.id,n.source_graph,n.source_order,n.payload_len,n.payload_sha256,n.payload,n.payload_codec,n.source_packet_sha256,c.packet_len,c.packet FROM knowledge_relations n LEFT JOIN knowledge_source_carriers c ON c.packet_sha256=n.source_packet_sha256 ORDER BY n.source_order",
    };
    let mut count = 0u64;
    let mut source_index = 0usize;
    let mut previous: Option<(
        String,
        String,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )> = None;
    with_owned_schema_statement(db, sql, state, |statement| {
        while owned_schema_step(statement, state)? {
            let rusqlite::types::ValueRef::Text(id) =
                statement.value_ref(0).map_err(owned_schema_sql_error)?
            else {
                return Err(Error::Invalid("knowledge core ID type"));
            };
            let rusqlite::types::ValueRef::Text(source) =
                statement.value_ref(1).map_err(owned_schema_sql_error)?
            else {
                return Err(Error::Invalid("knowledge core source type"));
            };
            if id.is_empty() || source.is_empty() {
                return Err(Error::Invalid("knowledge core text empty"));
            }
            if id.len() > limits.max_row_bytes || source.len() > limits.max_metadata_bytes {
                return Err(Error::Budget("knowledge core text bytes"));
            }
            state.charge_work(
                id.len()
                    .checked_add(source.len())
                    .ok_or(Error::Budget("core UTF8 work"))?,
            )?;
            state.active()?;
            let id = std::str::from_utf8(id).map_err(|_| Error::Invalid("core ID UTF8"))?;
            let source =
                std::str::from_utf8(source).map_err(|_| Error::Invalid("core source UTF8"))?;
            let order = statement.integer(2).map_err(owned_schema_sql_error)?;
            let logical_len = statement.integer(3).map_err(owned_schema_sql_error)?;
            let rusqlite::types::ValueRef::Blob(digest) =
                statement.value_ref(4).map_err(owned_schema_sql_error)?
            else {
                return Err(Error::Invalid("core digest type"));
            };
            let rusqlite::types::ValueRef::Blob(physical) =
                statement.value_ref(5).map_err(owned_schema_sql_error)?
            else {
                return Err(Error::Invalid("core payload type"));
            };
            if order < 0 || order as u64 != count || logical_len < 0 || digest.len() != 32 {
                return Err(Error::Invalid("knowledge core order/packet"));
            }
            if usize::try_from(logical_len)
                .ok()
                .is_none_or(|n| n > limits.max_row_bytes)
                || physical.len() > layout.physical_bound(limits.max_row_bytes)?
            {
                return Err(Error::Budget("knowledge core payload bytes"));
            }
            if count >= limits.max_rows {
                return Err(Error::Budget("knowledge core rows"));
            }
            if let Some((previous_source, previous_id, _)) = &previous {
                let source_order =
                    owned_core_compare(state, previous_source.as_bytes(), source.as_bytes())?;
                if source_order == std::cmp::Ordering::Greater
                    || (source_order == std::cmp::Ordering::Equal
                        && owned_core_compare(state, previous_id.as_bytes(), id.as_bytes())?
                            != std::cmp::Ordering::Less)
                {
                    return Err(Error::Invalid("knowledge core source order"));
                }
            }
            while source_index < expected.source_scopes.len()
                && owned_core_compare(
                    state,
                    expected.source_scopes[source_index].source_graph.as_bytes(),
                    source.as_bytes(),
                )? == std::cmp::Ordering::Less
            {
                source_index += 1;
            }
            if source_index >= expected.source_scopes.len()
                || !owned_schema_equal(
                    state,
                    expected.source_scopes[source_index].source_graph.as_bytes(),
                    source.as_bytes(),
                )?
            {
                return Err(Error::Invalid("knowledge unregistered core source"));
            }
            let codec = statement.integer(6).map_err(owned_schema_sql_error)?;
            if !(0..=1).contains(&codec) {
                return Err(Error::Invalid("core payload codec"));
            }
            let source_sha = match statement.value_ref(7).map_err(owned_schema_sql_error)? {
                rusqlite::types::ValueRef::Null => None,
                rusqlite::types::ValueRef::Blob(bytes) if bytes.len() == 32 => {
                    Some(Digest256::from_bytes(
                        bytes
                            .try_into()
                            .map_err(|_| Error::Invalid("source digest length"))?,
                    ))
                }
                _ => return Err(Error::Invalid("core source digest type")),
            };
            let source_packet = match statement.value_ref(9).map_err(owned_schema_sql_error)? {
                rusqlite::types::ValueRef::Null => None,
                rusqlite::types::ValueRef::Blob(bytes) if bytes.len() <= layout.physical_bound(limits.max_row_bytes)? => {
                    let length = statement.integer(8).map_err(owned_schema_sql_error)?;
                    let length = usize::try_from(length)
                        .map_err(|_| Error::Invalid("core source packet length"))?;
                    layout.verify_physical_length(bytes, length, limits.max_row_bytes)?;
                    Some(bytes)
                }
                _ => return Err(Error::Budget("core source packet bytes")),
            };
            let row = crate::knowledge_payload_read::SelectedPayloadRow {
                payload_codec: codec as u8,
                physical,
                logical_len: usize::try_from(logical_len)
                    .map_err(|_| Error::Budget("core logical length"))?,
                logical_sha256: Digest256::from_bytes(
                    digest
                        .try_into()
                        .map_err(|_| Error::Invalid("core digest length"))?,
                ),
                source_packet_sha256: source_sha,
                source_packet,
            };
            // Inline verification does not parse JSON; do not demand an unused
            // remaining grammar visit when merely hashing logical bytes.
            let visits = if codec == 0 {
                1
            } else {
                context.remaining_json_visits()?.min(1_000_000)
            };
            let json = JsonLimits::new(limits.max_row_bytes, 96, visits, 4096)
                .map_err(|_| Error::Budget("core codec JSON limits"))?;
            crate::knowledge_payload_read::with_logical_payload_for_verified_layout(
                context,
                layout,
                &row,
                json,
                json,
                limits.max_row_bytes,
                |_, _| Ok(()),
            )?;
            let key_bytes = id
                .len()
                .checked_add(source.len())
                .and_then(|n| n.checked_add(2 * std::mem::size_of::<String>()))
                .ok_or(Error::Budget("core prior key state"))?;
            let key_hold = state.hold(key_bytes)?;
            let owned_id = owned_core_key_copy(state, id)?;
            let owned_source = owned_core_key_copy(state, source)?;
            previous = Some((owned_source, owned_id, key_hold));
            owned_core_hash_item(state, global, id, digest)?;
            let acc = &mut accumulators[source_index];
            if nodes {
                acc.node_count = acc
                    .node_count
                    .checked_add(1)
                    .ok_or(Error::Budget("core node count"))?;
                owned_core_hash_item(state, &mut acc.nodes, id, digest)?;
            } else {
                acc.relation_count = acc
                    .relation_count
                    .checked_add(1)
                    .ok_or(Error::Budget("core relation count"))?;
                owned_core_hash_item(state, &mut acc.relations, id, digest)?;
            }
            count = count
                .checked_add(1)
                .ok_or(Error::Budget("core row count"))?;
        }
        Ok(count)
    })
}

fn owned_scope_hash_text(
    state: &crate::d1_public_capture::CreationState<'_>,
    hash: &mut Digest256Hasher,
    text: &str,
) -> Result<()> {
    let _frame = state.hold(std::mem::size_of::<(
        &crate::d1_public_capture::CreationState<'_>,
        &mut Digest256Hasher,
        &str,
        std::slice::Chunks<'_, u8>,
        &[u8],
        Result<()>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    state.charge_work(
        text.len()
            .checked_add(8)
            .ok_or(Error::Budget("scope text hash work"))?,
    )?;
    hash.update(&(text.len() as u64).to_be_bytes());
    for chunk in text.as_bytes().chunks(4096) {
        state.active()?;
        hash.update(chunk);
    }
    state.active()
}
fn owned_scope_sql_text<'a>(
    statement: &'a tos_source_store::PinnedBoundedStatement<'_>,
    column: usize,
    cap: usize,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<&'a str> {
    let _frame = state.hold(std::mem::size_of::<(
        &tos_source_store::PinnedBoundedStatement<'_>,
        usize,
        usize,
        i32,
        &crate::d1_public_capture::CreationState<'_>,
        &[u8],
        Result<&str>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    let column = i32::try_from(column).map_err(|_| Error::Budget("scope SQL column index"))?;
    let rusqlite::types::ValueRef::Text(bytes) = statement
        .value_ref(column)
        .map_err(owned_schema_sql_error)?
    else {
        return Err(Error::Invalid("scope text type"));
    };
    if bytes.len() > cap {
        return Err(Error::Budget("scope text bytes"));
    }
    state.charge_work(bytes.len())?;
    state.active()?;
    std::str::from_utf8(bytes).map_err(|_| Error::Invalid("scope text UTF8"))
}

/// Same core/scope receipt law as the established cold verifier, using owned
/// state before the accumulator vector and each borrowed SQL scan. Layout is
/// supplied only by the authentic expected model ABI caller.
pub(crate) fn verify_core_and_scope_with_owned_context(
    db: &Connection,
    expected: &KnowledgeSelectedExpectation,
    limits: ColdOpenLimits,
    layout: knowledge_stage::KnowledgePayloadLayout,
    context: &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
) -> Result<(Digest256, Digest256)> {
    let state = context.owned_state();
    let fixed = std::mem::size_of::<(
        &Connection,
        &KnowledgeSelectedExpectation,
        ColdOpenLimits,
        knowledge_stage::KnowledgePayloadLayout,
        &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
        &crate::d1_public_capture::CreationState<'_>,
        Vec<ScopeAcc>,
        Digest256Hasher,
        Digest256Hasher,
        Digest256Hasher,
        Digest256Hasher,
        Digest256,
        Digest256,
        u64,
        u64,
        std::iter::Enumerate<std::slice::Iter<'_, ExpectedSourceScope>>,
        usize,
        &ExpectedSourceScope,
        &ScopeAcc,
        &str,
        &str,
        &str,
        i64,
        i64,
        &[u8],
        &[u8],
        Result<(Digest256, Digest256)>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>();
    let capacity = expected
        .source_scopes
        .len()
        .checked_mul(std::mem::size_of::<ScopeAcc>())
        .ok_or(Error::Budget("scope accumulator bytes"))?;
    let _hold = state.hold(
        fixed
            .checked_add(capacity)
            .ok_or(Error::Budget("scope accumulator controller"))?,
    )?;
    state.charge_work(capacity)?;
    state.active()?;
    let mut accumulators = Vec::new();
    accumulators
        .try_reserve_exact(expected.source_scopes.len())
        .map_err(|_| Error::Budget("scope accumulator allocation"))?;
    if accumulators.capacity() != expected.source_scopes.len() {
        return Err(Error::Budget("scope accumulator exact capacity"));
    }
    for _ in &expected.source_scopes {
        state.active()?;
        accumulators.push(ScopeAcc {
            node_count: 0,
            relation_count: 0,
            nodes: Digest256Hasher::new(),
            relations: Digest256Hasher::new(),
        });
    }
    let mut node_root = Digest256Hasher::new();
    let mut relation_root = Digest256Hasher::new();
    let nodes = scan_core_with_owned_context(
        db,
        true,
        layout,
        expected,
        &mut accumulators,
        &mut node_root,
        limits,
        context,
    )?;
    let relations = scan_core_with_owned_context(
        db,
        false,
        layout,
        expected,
        &mut accumulators,
        &mut relation_root,
        limits,
        context,
    )?;
    if nodes != expected.node_count || relations != expected.relation_count {
        return Err(Error::Invalid("knowledge core count/owner expectation"));
    }
    with_owned_schema_statement(db,c"SELECT 1 FROM knowledge_relations r WHERE NOT EXISTS (SELECT 1 FROM knowledge_nodes n WHERE n.id=r.from_id) OR NOT EXISTS (SELECT 1 FROM knowledge_nodes n WHERE n.id=r.to_id) LIMIT 1",state,|statement| {
        if owned_schema_step(statement,state)? {return Err(Error::Invalid("knowledge relation endpoint closure"));}Ok(())
    })?;
    let mut scope_root = Digest256Hasher::new();
    with_owned_schema_statement(db,c"SELECT source_graph,input_role,adapter_profile,expected_node_count,expected_relation_count,node_root_sha256,relation_root_sha256 FROM source_scope ORDER BY source_graph",state,|statement| {
        for (index,expected_scope) in expected.source_scopes.iter().enumerate() {
            if !owned_schema_step(statement,state)? {return Err(Error::Invalid("knowledge source scope omitted"));}
            let source=owned_scope_sql_text(statement,0,limits.max_metadata_bytes,state)?;
            let role=owned_scope_sql_text(statement,1,limits.max_metadata_bytes,state)?;
            let adapter=owned_scope_sql_text(statement,2,limits.max_metadata_bytes,state)?;
            let node_count=statement.integer(3).map_err(owned_schema_sql_error)?;
            let relation_count=statement.integer(4).map_err(owned_schema_sql_error)?;
            let rusqlite::types::ValueRef::Blob(node_digest)=statement.value_ref(5).map_err(owned_schema_sql_error)? else {return Err(Error::Invalid("scope node digest type"));};
            let rusqlite::types::ValueRef::Blob(relation_digest)=statement.value_ref(6).map_err(owned_schema_sql_error)? else {return Err(Error::Invalid("scope relation digest type"));};
            if node_digest.len()!=32||relation_digest.len()!=32 {return Err(Error::Invalid("scope digest length"));}
            let acc=&accumulators[index];
            state.charge_work(2*std::mem::size_of::<Digest256Hasher>()+128)?;state.active()?;
            let calculated_node=acc.nodes.clone().finalize();let calculated_relation=acc.relations.clone().finalize();
            if !owned_schema_equal(state,source.as_bytes(),expected_scope.source_graph.as_bytes())?
                ||!owned_schema_equal(state,role.as_bytes(),expected_scope.input_role.as_bytes())?
                ||!owned_schema_equal(state,adapter.as_bytes(),expected_scope.adapter_profile.as_bytes())?
                ||node_count<0||relation_count<0||node_count as u64!=expected_scope.node_count||relation_count as u64!=expected_scope.relation_count
                ||acc.node_count!=expected_scope.node_count||acc.relation_count!=expected_scope.relation_count
                ||node_digest!=calculated_node.as_bytes()||relation_digest!=calculated_relation.as_bytes()
                ||Digest256::from_hex(&expected_scope.node_root_sha256).map_err(|_|Error::Invalid("scope node root expectation"))?!=calculated_node
                ||Digest256::from_hex(&expected_scope.relation_root_sha256).map_err(|_|Error::Invalid("scope relation root expectation"))?!=calculated_relation {
                return Err(Error::Invalid("knowledge source scope closure"));
            }
            owned_scope_hash_text(state,&mut scope_root,source)?;
            owned_scope_hash_text(state,&mut scope_root,role)?;
            owned_scope_hash_text(state,&mut scope_root,adapter)?;
            state.charge_work(80)?;state.active()?;
            scope_root.update(&(node_count as u64).to_be_bytes());scope_root.update(&(relation_count as u64).to_be_bytes());scope_root.update(node_digest);scope_root.update(relation_digest);
        }
        if owned_schema_step(statement,state)? {return Err(Error::Invalid("knowledge source scope extra row"));}
        state.charge_work(64)?;state.active()?;
        if scope_root.clone().finalize()!=Digest256::from_hex(&expected.source_scope_root_sha256).map_err(|_|Error::Invalid("scope root expectation"))? {return Err(Error::Invalid("knowledge source scope root"));}
        Ok(())
    })?;
    state.active()?;
    Ok((node_root.finalize(), relation_root.finalize()))
}

struct ScopeAcc {
    node_count: u64,
    relation_count: u64,
    nodes: Digest256Hasher,
    relations: Digest256Hasher,
}

fn scan_core(
    db: &Connection,
    table: &str,
    expected: &KnowledgeSelectedExpectation,
    accumulators: &mut [ScopeAcc],
    global: &mut Digest256Hasher,
    limits: ColdOpenLimits,
    work: &mut u64,
) -> Result<u64> {
    let sql = match table {
        "knowledge_nodes" => {
            "SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=?1 THEN id ELSE NULL END,CASE WHEN typeof(source_graph)='text' AND length(CAST(source_graph AS BLOB))<=?2 THEN source_graph ELSE NULL END,source_order,payload_len,CASE WHEN typeof(payload_sha256)='blob' AND length(payload_sha256)=32 THEN payload_sha256 ELSE NULL END,CASE WHEN typeof(payload)='blob' AND length(payload)<=?1 THEN payload ELSE NULL END FROM knowledge_nodes ORDER BY source_order"
        }
        "knowledge_relations" => {
            "SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=?1 THEN id ELSE NULL END,CASE WHEN typeof(source_graph)='text' AND length(CAST(source_graph AS BLOB))<=?2 THEN source_graph ELSE NULL END,source_order,payload_len,CASE WHEN typeof(payload_sha256)='blob' AND length(payload_sha256)=32 THEN payload_sha256 ELSE NULL END,CASE WHEN typeof(payload)='blob' AND length(payload)<=?1 THEN payload ELSE NULL END FROM knowledge_relations ORDER BY source_order"
        }
        _ => return Err(Error::Invalid("knowledge core table")),
    };
    let mut statement = db.prepare(sql)?;
    let mut rows = statement.query(params![
        limits.max_row_bytes as i64,
        limits.max_metadata_bytes as i64
    ])?;
    let mut count = 0u64;
    let mut source_index = 0usize;
    let mut previous: Option<(String, String)> = None;
    while let Some(row) = rows.next()? {
        let id: String = row
            .get::<_, Option<String>>(0)?
            .ok_or(Error::Budget("knowledge cold core ID bytes"))?;
        let source: String = row
            .get::<_, Option<String>>(1)?
            .ok_or(Error::Budget("knowledge cold core source bytes"))?;
        let order: i64 = row.get(2)?;
        let payload_len: i64 = row.get(3)?;
        let digest: Vec<u8> = row
            .get::<_, Option<Vec<u8>>>(4)?
            .ok_or(Error::Invalid("knowledge cold core digest type/length"))?;
        let payload: Option<Vec<u8>> = row.get(5)?;
        let payload = payload.ok_or(Error::Budget("knowledge cold core payload"))?;
        if id.is_empty()
            || id.len() > limits.max_row_bytes
            || source.is_empty()
            || source.len() > limits.max_metadata_bytes
            || order < 0
            || order as u64 != count
            || payload_len < 0
            || payload_len as usize != payload.len()
            || digest.len() != 32
            || digest.as_slice() != Digest256::of_bytes(&payload).as_bytes()
        {
            return Err(Error::Invalid("knowledge cold core row"));
        }
        let key = (source.clone(), id.clone());
        if previous.as_ref().is_some_and(|p| p >= &key) {
            return Err(Error::Invalid("knowledge cold core source order"));
        }
        previous = Some(key);
        while source_index < expected.source_scopes.len()
            && expected.source_scopes[source_index].source_graph < source
        {
            source_index += 1;
        }
        if source_index == expected.source_scopes.len()
            || expected.source_scopes[source_index].source_graph != source
        {
            return Err(Error::Invalid("knowledge unregistered core source"));
        }
        let acc = &mut accumulators[source_index];
        hash_item(global, &id, &digest);
        if table == "knowledge_nodes" {
            acc.node_count = acc
                .node_count
                .checked_add(1)
                .ok_or(Error::Budget("knowledge core nodes"))?;
            hash_item(&mut acc.nodes, &id, &digest);
        } else {
            acc.relation_count = acc
                .relation_count
                .checked_add(1)
                .ok_or(Error::Budget("knowledge core relations"))?;
            hash_item(&mut acc.relations, &id, &digest);
        }
        count = count
            .checked_add(1)
            .ok_or(Error::Budget("knowledge cold core rows"))?;
        if count > limits.max_rows {
            return Err(Error::Budget("knowledge cold core rows"));
        }
        charge(
            work,
            id.len() + source.len() + digest.len() + payload.len() + 24,
            limits.max_work_bytes,
        )?;
    }
    Ok(count)
}

fn verify_core_and_scope(
    db: &Connection,
    expected: &KnowledgeSelectedExpectation,
    limits: ColdOpenLimits,
    work: &mut u64,
) -> Result<(Digest256, Digest256)> {
    let mut accumulators: Vec<ScopeAcc> = (0..expected.source_scopes.len())
        .map(|_| ScopeAcc {
            node_count: 0,
            relation_count: 0,
            nodes: Digest256Hasher::new(),
            relations: Digest256Hasher::new(),
        })
        .collect();
    let mut node_root = Digest256Hasher::new();
    let mut relation_root = Digest256Hasher::new();
    let nodes = scan_core(
        db,
        "knowledge_nodes",
        expected,
        &mut accumulators,
        &mut node_root,
        limits,
        work,
    )?;
    let relations = scan_core(
        db,
        "knowledge_relations",
        expected,
        &mut accumulators,
        &mut relation_root,
        limits,
        work,
    )?;
    if nodes != expected.node_count || relations != expected.relation_count {
        return Err(Error::Invalid("knowledge core count/owner expectation"));
    }
    let broken: Option<i64> = db.query_row(
        "SELECT 1 FROM knowledge_relations r WHERE NOT EXISTS (SELECT 1 FROM knowledge_nodes n WHERE n.id=r.from_id) OR NOT EXISTS (SELECT 1 FROM knowledge_nodes n WHERE n.id=r.to_id) LIMIT 1",
        [], |r| r.get(0),
    ).optional()?;
    if broken.is_some() {
        return Err(Error::Invalid("knowledge relation endpoint closure"));
    }
    let mut scope_root = Digest256Hasher::new();
    let mut statement = db.prepare("SELECT CASE WHEN typeof(source_graph)='text' AND length(CAST(source_graph AS BLOB))<=?1 THEN source_graph ELSE NULL END,CASE WHEN typeof(input_role)='text' AND length(CAST(input_role AS BLOB))<=?1 THEN input_role ELSE NULL END,CASE WHEN typeof(adapter_profile)='text' AND length(CAST(adapter_profile AS BLOB))<=?1 THEN adapter_profile ELSE NULL END,expected_node_count,expected_relation_count,CASE WHEN typeof(node_root_sha256)='blob' AND length(node_root_sha256)=32 THEN node_root_sha256 ELSE NULL END,CASE WHEN typeof(relation_root_sha256)='blob' AND length(relation_root_sha256)=32 THEN relation_root_sha256 ELSE NULL END FROM source_scope ORDER BY source_graph")?;
    let mut rows = statement.query([limits.max_metadata_bytes as i64])?;
    for (index, expected_scope) in expected.source_scopes.iter().enumerate() {
        let row = rows
            .next()?
            .ok_or(Error::Invalid("knowledge source scope omitted"))?;
        let source: String = row
            .get::<_, Option<String>>(0)?
            .ok_or(Error::Budget("knowledge scope source bytes"))?;
        let role: String = row
            .get::<_, Option<String>>(1)?
            .ok_or(Error::Budget("knowledge scope role bytes"))?;
        let adapter: String = row
            .get::<_, Option<String>>(2)?
            .ok_or(Error::Budget("knowledge scope adapter bytes"))?;
        let node_count: i64 = row.get(3)?;
        let relation_count: i64 = row.get(4)?;
        let node_digest: Vec<u8> = row
            .get::<_, Option<Vec<u8>>>(5)?
            .ok_or(Error::Invalid("knowledge scope node digest"))?;
        let relation_digest: Vec<u8> = row
            .get::<_, Option<Vec<u8>>>(6)?
            .ok_or(Error::Invalid("knowledge scope relation digest"))?;
        let acc = &accumulators[index];
        let calculated_node = acc.nodes.clone().finalize();
        let calculated_relation = acc.relations.clone().finalize();
        if source != expected_scope.source_graph
            || role != expected_scope.input_role
            || adapter != expected_scope.adapter_profile
            || node_count < 0
            || relation_count < 0
            || node_count as u64 != expected_scope.node_count
            || relation_count as u64 != expected_scope.relation_count
            || acc.node_count != expected_scope.node_count
            || acc.relation_count != expected_scope.relation_count
            || node_digest.as_slice() != calculated_node.as_bytes()
            || relation_digest.as_slice() != calculated_relation.as_bytes()
            || expected_scope.node_root_sha256 != calculated_node.to_hex()
            || expected_scope.relation_root_sha256 != calculated_relation.to_hex()
        {
            return Err(Error::Invalid("knowledge source scope closure"));
        }
        hash_text(&mut scope_root, &source);
        hash_text(&mut scope_root, &role);
        hash_text(&mut scope_root, &adapter);
        scope_root.update(&(node_count as u64).to_be_bytes());
        scope_root.update(&(relation_count as u64).to_be_bytes());
        scope_root.update(&node_digest);
        scope_root.update(&relation_digest);
        charge(
            work,
            source.len()
                + role.len()
                + adapter.len()
                + node_digest.len()
                + relation_digest.len()
                + 16,
            limits.max_work_bytes,
        )?;
    }
    if rows.next()?.is_some() || scope_root.finalize().to_hex() != expected.source_scope_root_sha256
    {
        return Err(Error::Invalid("knowledge source scope root/extra row"));
    }
    Ok((node_root.finalize(), relation_root.finalize()))
}
fn hash_sql_row(
    hash: &mut Digest256Hasher,
    row: &rusqlite::Row<'_>,
    work: &mut u64,
    cap: u64,
) -> Result<()> {
    for col in 0..row.as_ref().column_count() {
        match row.get_ref(col)? {
            rusqlite::types::ValueRef::Text(bytes) | rusqlite::types::ValueRef::Blob(bytes) => {
                charge(work, bytes.len() + 8, cap)?;
                hash_bytes(hash, bytes);
            }
            rusqlite::types::ValueRef::Integer(value) => {
                charge(work, 16, cap)?;
                hash_bytes(hash, &value.to_be_bytes());
            }
            _ => return Err(Error::Invalid("knowledge search root value type")),
        }
    }
    Ok(())
}

fn rank_array(raw: &str, limits: ColdOpenLimits) -> Result<()> {
    let json_limits = JsonLimits::new(limits.max_row_bytes, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("knowledge rank JSON limits"))?;
    let parsed = parse_json(raw.as_bytes(), JsonMode::PublishedStrict, json_limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    let array = parsed
        .root()
        .as_array()
        .ok_or(Error::Invalid("knowledge rank array"))?;
    if array
        .iter()
        .any(|value| !matches!(value, JsonValue::String(s) if s.as_str().is_some()))
    {
        return Err(Error::Invalid("knowledge rank array value"));
    }
    Ok(())
}

fn verify_search(
    db: &Connection,
    expected: &KnowledgeSelectedExpectation,
    limits: ColdOpenLimits,
    work: &mut u64,
) -> Result<()> {
    verify_search_inner(db, expected, limits, work, None)
}

pub(crate) fn verify_search_with_owned_context(
    db: &Connection,
    expected: &KnowledgeSelectedExpectation,
    limits: ColdOpenLimits,
    context: &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
) -> Result<()> {
    let state = context.owned_state();
    let retained_rows = limits
        .max_row_bytes
        .checked_mul(8)
        .and_then(|n| n.checked_add(limits.max_metadata_bytes.checked_mul(4)?))
        .and_then(|n| n.checked_add(MAX_POSTING_DELTA_BYTES))
        .and_then(|n| {
            n.checked_add(MAX_POSTINGS_PER_BLOCK.checked_mul(std::mem::size_of::<u64>())?)
        })
        .and_then(|n| n.checked_add(2048))
        .ok_or(Error::Budget("owned Search row workspace"))?;
    let statements =
        tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()
            .checked_mul(6)
            .ok_or(Error::Budget("owned Search statement workspace"))?;
    let frame = std::mem::size_of::<(
        &Connection,
        &KnowledgeSelectedExpectation,
        ColdOpenLimits,
        &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
        &crate::d1_public_capture::CreationState<'_>,
        Digest256Hasher,
        [u64; 2],
        Option<(String, Vec<u8>, u64, u16)>,
        Option<rusqlite::Statement<'_>>,
        Option<rusqlite::Rows<'_>>,
        Result<()>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>()
    .checked_add(std::mem::size_of::<Result<()>>())
    .and_then(|n| n.checked_add(retained_rows))
    .and_then(|n| n.checked_add(statements))
    .ok_or(Error::Budget("owned Search verification frame"))?;
    let _frame = state.hold(frame)?;
    state.active()?;
    let mut work = 0u64;
    verify_search_inner(db, expected, limits, &mut work, Some(context))
}

fn owned_search_charge(
    work: &mut u64,
    amount: usize,
    cap: u64,
    context: Option<&crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>>,
) -> Result<()> {
    charge(work, amount, cap)?;
    if let Some(context) = context {
        context.charge_work(amount)?;
    }
    Ok(())
}

fn owned_hash_sql_row(
    hash: &mut Digest256Hasher,
    row: &rusqlite::Row<'_>,
    work: &mut u64,
    cap: u64,
    context: Option<&crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>>,
) -> Result<()> {
    for col in 0..row.as_ref().column_count() {
        if let Some(context) = context {
            context.check()?;
        }
        match row.get_ref(col)? {
            rusqlite::types::ValueRef::Text(bytes) | rusqlite::types::ValueRef::Blob(bytes) => {
                owned_search_charge(
                    work,
                    bytes
                        .len()
                        .checked_add(8)
                        .ok_or(Error::Budget("Search row charge"))?,
                    cap,
                    context,
                )?;
                hash_bytes(hash, bytes);
            }
            rusqlite::types::ValueRef::Integer(value) => {
                owned_search_charge(work, 16, cap, context)?;
                hash_bytes(hash, &value.to_be_bytes());
            }
            _ => return Err(Error::Invalid("knowledge search root value type")),
        }
    }
    Ok(())
}

fn rank_array_with_owned_context(
    raw: &str,
    limits: ColdOpenLimits,
    context: &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
) -> Result<()> {
    let json_limits = JsonLimits::new(limits.max_row_bytes, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("knowledge rank JSON limits"))?;
    context.with_foundation_owned_with_limits(raw.as_bytes(), json_limits, |root| {
        let array = root
            .as_array()
            .ok_or(Error::Invalid("knowledge rank array"))?;
        for value in array {
            context.check()?;
            let JsonValue::String(text) = value else {
                return Err(Error::Invalid("knowledge rank array value"));
            };
            let text = text
                .as_str()
                .ok_or(Error::Invalid("knowledge rank array string"))?;
            context.charge_work(text.len())?;
        }
        Ok(())
    })
}

fn verify_search_inner(
    db: &Connection,
    expected: &KnowledgeSelectedExpectation,
    limits: ColdOpenLimits,
    work: &mut u64,
    context: Option<&crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>>,
) -> Result<()> {
    let mut root = Digest256Hasher::new();
    hash_bytes(&mut root, b"tos-knowledge-search-posting-blocks-v1");
    hash_bytes(&mut root, &[0]);
    let mut documents = [0u64; 2];
    let mut doc_statement = db.prepare(
        "SELECT CASE WHEN typeof(kind)='text' AND length(CAST(kind AS BLOB))<=16 THEN kind ELSE NULL END,position,
         CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=?1 THEN id ELSE NULL END,
         CASE WHEN typeof(source_graph)='text' AND length(CAST(source_graph AS BLOB))<=?2 THEN source_graph ELSE NULL END,
         CASE WHEN typeof(kind_id)='text' AND length(CAST(kind_id AS BLOB))<=?2 THEN kind_id ELSE NULL END,
         CASE WHEN typeof(predicate_id)='text' AND length(CAST(predicate_id AS BLOB))<=?2 THEN predicate_id ELSE NULL END,
         CASE WHEN typeof(id_lower)='text' AND length(CAST(id_lower AS BLOB))<=?1 THEN id_lower ELSE NULL END,
         CASE WHEN typeof(native_id_lower)='text' AND length(CAST(native_id_lower AS BLOB))<=?1 THEN native_id_lower ELSE NULL END,
         CASE WHEN typeof(identity_values)='text' AND length(CAST(identity_values AS BLOB))<=?1 THEN identity_values ELSE NULL END,
         CASE WHEN typeof(visible_values)='text' AND length(CAST(visible_values AS BLOB))<=?1 THEN visible_values ELSE NULL END,
         document_chars,CASE WHEN typeof(document_digest)='blob' AND length(document_digest)=32 THEN document_digest ELSE NULL END FROM search_documents ORDER BY kind,position"
    )?;
    let mut doc_rows = doc_statement.query(params![
        limits.max_row_bytes as i64,
        limits.max_metadata_bytes as i64
    ])?;
    let mut core_node = db.prepare(
        "SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=?2 THEN id ELSE NULL END,
         CASE WHEN typeof(source_graph)='text' AND length(CAST(source_graph AS BLOB))<=?3 THEN source_graph ELSE NULL END,
         CASE WHEN typeof(kind_id)='text' AND length(CAST(kind_id AS BLOB))<=?2 THEN kind_id ELSE NULL END
         FROM knowledge_nodes WHERE source_order=?1",
    )?;
    let mut core_relation = db.prepare(
        "SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=?2 THEN id ELSE NULL END,
         CASE WHEN typeof(source_graph)='text' AND length(CAST(source_graph AS BLOB))<=?3 THEN source_graph ELSE NULL END,
         CASE WHEN typeof(predicate_id)='text' AND length(CAST(predicate_id AS BLOB))<=?2 THEN predicate_id ELSE NULL END
         FROM knowledge_relations WHERE source_order=?1",
    )?;
    while let Some(row) = doc_rows.next()? {
        if let Some(context) = context {
            context.check()?;
        }
        let kind: String = row
            .get::<_, Option<String>>(0)?
            .ok_or(Error::Invalid("knowledge search kind bytes"))?;
        let k = match kind.as_str() {
            "nodes" => 0,
            "relations" => 1,
            _ => return Err(Error::Invalid("knowledge search document kind")),
        };
        let position: i64 = row.get(1)?;
        if position < 0 || position as u64 != documents[k] {
            return Err(Error::Invalid("knowledge search document ordinal"));
        }
        let id: String = row
            .get::<_, Option<String>>(2)?
            .ok_or(Error::Budget("knowledge search ID bytes"))?;
        let source: String = row
            .get::<_, Option<String>>(3)?
            .ok_or(Error::Budget("knowledge search source bytes"))?;
        let kind_id: String = row
            .get::<_, Option<String>>(4)?
            .ok_or(Error::Budget("knowledge search kind ID bytes"))?;
        let predicate_id: String = row
            .get::<_, Option<String>>(5)?
            .ok_or(Error::Budget("knowledge search predicate ID bytes"))?;
        let id_lower: Option<String> = row.get(6)?;
        let native_lower: Option<String> = row.get(7)?;
        let identity: Option<String> = row.get(8)?;
        let visible: Option<String> = row.get(9)?;
        let chars: i64 = row.get(10)?;
        let digest: Vec<u8> = row
            .get::<_, Option<Vec<u8>>>(11)?
            .ok_or(Error::Invalid("knowledge search digest type/length"))?;
        let (id_lower, native_lower, identity, visible) = (
            id_lower.ok_or(Error::Budget("knowledge search lower ID bytes"))?,
            native_lower.ok_or(Error::Budget("knowledge search native lower bytes"))?,
            identity.ok_or(Error::Budget("knowledge search identity bytes"))?,
            visible.ok_or(Error::Budget("knowledge search visible bytes"))?,
        );
        if id.is_empty()
            || id.len() > limits.max_row_bytes
            || source.is_empty()
            || source.len() > limits.max_metadata_bytes
            || chars < 0
            || chars > 8_000_000
            || digest.len() != 32
            || (k == 0 && !predicate_id.is_empty())
            || (k == 1 && !kind_id.is_empty())
        {
            return Err(Error::Invalid("knowledge search document shape"));
        }
        if let Some(context) = context {
            rank_array_with_owned_context(&identity, limits, context)?;
            rank_array_with_owned_context(&visible, limits, context)?;
        } else {
            rank_array(&identity, limits)?;
            rank_array(&visible, limits)?;
        }
        let (core_id, core_source, core_term): (String, String, String) = if k == 0 {
            core_node.query_row(
                params![
                    position,
                    limits.max_row_bytes as i64,
                    limits.max_metadata_bytes as i64
                ],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?
        } else {
            core_relation.query_row(
                params![
                    position,
                    limits.max_row_bytes as i64,
                    limits.max_metadata_bytes as i64
                ],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?
        };
        if (
            id.as_str(),
            source.as_str(),
            if k == 0 {
                kind_id.as_str()
            } else {
                predicate_id.as_str()
            },
        ) != (core_id.as_str(), core_source.as_str(), core_term.as_str())
        {
            return Err(Error::Invalid("knowledge search/core document mismatch"));
        }
        owned_search_charge(
            work,
            id_lower.len() + native_lower.len() + identity.len() + visible.len(),
            limits.max_work_bytes,
            context,
        )?;
        owned_hash_sql_row(&mut root, row, work, limits.max_work_bytes, context)?;
        documents[k] = documents[k]
            .checked_add(1)
            .ok_or(Error::Budget("knowledge search documents"))?;
        if documents[k] > limits.max_rows {
            return Err(Error::Budget("knowledge search documents"));
        }
    }
    if documents != [expected.node_count, expected.relation_count] {
        return Err(Error::Invalid("knowledge search document coverage"));
    }
    drop(doc_rows);
    drop(doc_statement);
    drop(core_node);
    drop(core_relation);
    hash_bytes(&mut root, &[1]);
    let mut postings = 0u64;
    let mut previous_block: Option<(String, Vec<u8>, u64, u16)> = None;
    let mut gram_statement = db.prepare(
        "SELECT CASE WHEN typeof(kind)='text' AND length(CAST(kind AS BLOB))<=16 THEN kind ELSE NULL END,
         CASE WHEN typeof(n)='integer' THEN n ELSE NULL END,
         CASE WHEN typeof(gram)='blob' AND length(gram)<=12 THEN gram ELSE NULL END,
         CASE WHEN typeof(last_position)='integer' THEN last_position ELSE NULL END,
         CASE WHEN typeof(first_position)='integer' THEN first_position ELSE NULL END,
         CASE WHEN typeof(postings)='integer' THEN postings ELSE NULL END,
         CASE WHEN typeof(deltas)='blob' AND length(deltas)<=?1 THEN deltas ELSE NULL END
         FROM search_posting_blocks ORDER BY kind,n,gram,last_position"
    )?;
    let mut gram_rows = gram_statement.query([MAX_POSTING_DELTA_BYTES as i64])?;
    while let Some(row) = gram_rows.next()? {
        if let Some(context) = context {
            context.check()?;
        }
        let kind: String = row
            .get::<_, Option<String>>(0)?
            .ok_or(Error::Invalid("knowledge gram kind bytes"))?;
        let k = match kind.as_str() {
            "nodes" => 0,
            "relations" => 1,
            _ => return Err(Error::Invalid("knowledge gram kind")),
        };
        let n: i64 = row
            .get::<_, Option<i64>>(1)?
            .ok_or(Error::Invalid("knowledge gram n type"))?;
        let gram: Option<Vec<u8>> = row.get(2)?;
        let gram = gram.ok_or(Error::Budget("knowledge gram bytes"))?;
        let last: i64 = row
            .get::<_, Option<i64>>(3)?
            .ok_or(Error::Invalid("knowledge block last type"))?;
        let first: i64 = row
            .get::<_, Option<i64>>(4)?
            .ok_or(Error::Invalid("knowledge block first type"))?;
        let count: i64 = row
            .get::<_, Option<i64>>(5)?
            .ok_or(Error::Invalid("knowledge block count type"))?;
        let deltas: Vec<u8> = row
            .get::<_, Option<Vec<u8>>>(6)?
            .ok_or(Error::Invalid("knowledge block deltas type/length"))?;
        if n != 3
            || last < 0
            || first < 0
            || count <= 0
            || count > MAX_POSTINGS_PER_BLOCK as i64
            || std::str::from_utf8(&gram).ok().map(|s| s.chars().count()) != Some(3)
        {
            return Err(Error::Invalid("knowledge gram block shape"));
        }
        let positions = decode_posting_block(first as u64, last as u64, count as u16, &deltas)?;
        owned_search_charge(work, positions.len() * 8, limits.max_work_bytes, context)?;
        if positions.iter().any(|position| *position >= documents[k]) {
            return Err(Error::Invalid("knowledge gram block orphan"));
        }
        if let Some((prior_kind, prior_gram, prior_last, prior_count)) = &previous_block {
            if prior_kind == &kind
                && prior_gram == &gram
                && (*prior_count as usize != MAX_POSTINGS_PER_BLOCK || first as u64 <= *prior_last)
            {
                return Err(Error::Invalid("knowledge gram block partition"));
            }
        }
        previous_block = Some((kind, gram, last as u64, count as u16));
        owned_hash_sql_row(&mut root, row, work, limits.max_work_bytes, context)?;
        postings = postings
            .checked_add(count as u64)
            .ok_or(Error::Budget("knowledge postings"))?;
        if postings > limits.max_rows {
            return Err(Error::Budget("knowledge postings"));
        }
    }
    drop(gram_rows);
    drop(gram_statement);
    hash_bytes(&mut root, &[2]);
    let mut aggregate = db.prepare(
        "SELECT CASE WHEN typeof(kind)='text' AND length(CAST(kind AS BLOB))<=16 THEN kind ELSE NULL END,n,CASE WHEN typeof(gram)='blob' AND length(gram)<=12 THEN gram ELSE NULL END,SUM(postings) FROM search_posting_blocks GROUP BY kind,n,gram ORDER BY kind,n,gram",
    )?;
    let mut grouped_rows = aggregate.query([])?;
    let mut stat_statement = db.prepare(
        "SELECT CASE WHEN typeof(kind)='text' AND length(CAST(kind AS BLOB))<=16 THEN kind ELSE NULL END,n,CASE WHEN typeof(gram)='blob' AND length(gram)<=12 THEN gram ELSE NULL END,postings
         FROM search_gram_stats ORDER BY kind,n,gram"
    )?;
    let mut stat_rows = stat_statement.query([])?;
    let mut distinct = 0u64;
    while let Some(row) = stat_rows.next()? {
        if let Some(context) = context {
            context.check()?;
        }
        let grouped = grouped_rows
            .next()?
            .ok_or(Error::Invalid("knowledge gram stats extra"))?;
        let stat_kind: String = row
            .get::<_, Option<String>>(0)?
            .ok_or(Error::Invalid("knowledge gram stat kind bytes"))?;
        let stat_n: i64 = row.get(1)?;
        let stat_gram: Option<Vec<u8>> = row.get(2)?;
        let stat_count: i64 = row.get(3)?;
        let group_kind: String = grouped
            .get::<_, Option<String>>(0)?
            .ok_or(Error::Invalid("knowledge gram group kind bytes"))?;
        let group_n: i64 = grouped.get(1)?;
        let group_gram: Vec<u8> = grouped
            .get::<_, Option<Vec<u8>>>(2)?
            .ok_or(Error::Invalid("knowledge gram group bytes"))?;
        let group_count: i64 = grouped.get(3)?;
        if stat_n != 3
            || stat_count <= 0
            || stat_gram.as_deref() != Some(group_gram.as_slice())
            || stat_kind != group_kind
            || stat_n != group_n
            || stat_count != group_count
        {
            return Err(Error::Invalid("knowledge gram stats mismatch"));
        }
        owned_hash_sql_row(&mut root, row, work, limits.max_work_bytes, context)?;
        distinct = distinct
            .checked_add(1)
            .ok_or(Error::Budget("knowledge gram stats"))?;
        if distinct > limits.max_rows {
            return Err(Error::Budget("knowledge gram stats"));
        }
    }
    if grouped_rows.next()?.is_some()
        || root.finalize().to_hex() != expected.search_index_root_sha256
    {
        return Err(Error::Invalid("knowledge search index root/coverage"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const GRAPH_ROOT: &str = "f69fe8ac5e516d2629699826d6e3bd2ed4b9490a808a4e3b8747645757812757";
    const HEADER: &str = "{\"authority_boundary\":{},\"counts\":{\"display_coverage\":{},\"nodes\":0,\"relations\":0,\"semantic_mapping\":{},\"sources\":{}},\"normalization_binding\":{\"configuration_digest\":\"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\",\"entity_registry_digest\":\"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\",\"processor_digest\":\"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\",\"relation_registry_digest\":\"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\",\"schema\":\"tos_knowledge_graph_normalization_binding_v1\"},\"query_properties\":[],\"schema\":\"tos_knowledge_graph_v1\",\"source_revision\":\"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\"}";

    fn limits() -> ColdOpenLimits {
        ColdOpenLimits {
            max_file_bytes: 8192,
            max_vm_steps: 100_000,
            sqlite_cache_kib: 1024,
            max_rows: 100,
            max_work_bytes: 1_000_000,
            max_row_bytes: 4096,
            max_metadata_bytes: 1024,
            max_sources: 4,
        }
    }
    fn expected() -> KnowledgeSelectedExpectation {
        KnowledgeSelectedExpectation {
            model_sha256: EMPTY.into(),
            model_size_bytes: 4096,
            owner_receipt_id: "owner-r".into(),
            model_abi: KNOWLEDGE_MODEL_ABI.into(),
            managed_source_root_sha256: None,
            descriptor_sha256: EMPTY.into(),
            descriptor_version: 1,
            semantic_primitive_profile: KNOWLEDGE_QUERY_PRIMITIVE_PROFILE.into(),
            source_cut: "cut".into(),
            through_commit_seq: 0,
            membership_root: EMPTY.into(),
            entity_registry_id: "e".into(),
            entity_registry_version: "1".into(),
            entity_registry_sha256: EMPTY.into(),
            relation_registry_id: "r".into(),
            relation_registry_version: "1".into(),
            relation_registry_sha256: EMPTY.into(),
            graph_root_sha256: GRAPH_ROOT.into(),
            navigation_original_root_sha256: None,
            philosophy_original_root_sha256: None,
            corpus_original_root_sha256: None,
            catalog_packet_sha256: EMPTY.into(),
            catalog_index_root_sha256: EMPTY.into(),
            source_scope_root_sha256: EMPTY.into(),
            search_index_root_sha256: EMPTY.into(),
            node_count: 0,
            relation_count: 0,
            index_generation: "g".into(),
            route_map_version: "v".into(),
            reader_abi: "q".into(),
            authority_boundary: "{}".into(),
            source_scopes: vec![ExpectedSourceScope {
                source_graph: "g".into(),
                input_role: "source".into(),
                adapter_profile: "indexed-node-edge-v1".into(),
                node_count: 0,
                relation_count: 0,
                node_root_sha256: EMPTY.into(),
                relation_root_sha256: EMPTY.into(),
            }],
            complete: true,
        }
    }

    #[test]
    fn independent_zero_source_and_limit_expectation() {
        let value = expected();
        assert!(validate(&value, limits()).is_ok());
        let mut unicode = value.clone();
        unicode.source_scopes[0].source_graph = "gé".into();
        assert!(validate(&unicode, limits()).is_ok());
        let mut omitted = value.clone();
        omitted.source_scopes.clear();
        assert!(validate(&omitted, limits()).is_err());
        let mut short = limits();
        short.max_file_bytes = 100;
        assert!(validate(&value, short).is_err());
    }

    #[test]
    fn python_oracle_graph_header_root_and_tamper_refusal() {
        // Independent CPython 3.14 sorted-compact JSON + hashlib/struct
        // oracle: 702 packet bytes, SHA c88b15..., graph root f69fe8...
        assert_eq!(HEADER.len(), 702);
        assert_eq!(
            Digest256::of_bytes(HEADER.as_bytes()).to_hex(),
            "c88b15ba0ab34f69f8682bf831eb2b29ca915d9433035d9e8d14c7b97debe090"
        );
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE graph_header(singleton INTEGER PRIMARY KEY CHECK(singleton=1),packet_len INTEGER NOT NULL,packet_sha256 BLOB NOT NULL,packet BLOB NOT NULL)").unwrap();
        let sha = Digest256::of_bytes(HEADER.as_bytes());
        db.execute(
            "INSERT INTO graph_header VALUES(1,?1,?2,?3)",
            params![
                HEADER.len() as i64,
                sha.as_bytes().as_slice(),
                HEADER.as_bytes()
            ],
        )
        .unwrap();
        let empty = Digest256::of_bytes(b"");
        let mut work = 0;
        assert!(verify_graph_root(&db, &expected(), limits(), &mut work, empty, empty).is_ok());
        // The seal uses semantic registry digests in the header, whereas
        // metadata binds the distinct byte digests of the registry files.
        let mut different_registry_bytes = expected();
        different_registry_bytes.entity_registry_sha256 = "0".repeat(64);
        different_registry_bytes.relation_registry_sha256 = "1".repeat(64);
        assert!(
            verify_graph_root(
                &db,
                &different_registry_bytes,
                limits(),
                &mut work,
                empty,
                empty
            )
            .is_ok()
        );
        db.execute("UPDATE graph_header SET packet_len=packet_len+1", [])
            .unwrap();
        assert!(verify_graph_root(&db, &expected(), limits(), &mut work, empty, empty).is_err());
    }

    #[test]
    fn selected_table_allowlist_refuses_private_input_and_prepare_tables() {
        let db = Connection::open_in_memory().unwrap();
        for (table, _) in SELECTED_TABLES {
            db.execute(&format!("CREATE TABLE {table}(x)"), []).unwrap();
        }
        assert!(verify_selected_table_allowlist(&db).is_ok());
        db.execute_batch(crate::knowledge_navigation_original::META_DDL)
            .unwrap();
        assert!(verify_selected_table_allowlist(&db).is_err());
        db.execute_batch(crate::knowledge_navigation_original::ROW_DDL)
            .unwrap();
        assert!(verify_selected_table_allowlist(&db).is_err());
        db.execute_batch(crate::knowledge_navigation_original::MEMBER_DDL)
            .unwrap();
        assert!(verify_selected_table_allowlist(&db).is_ok());
        crate::knowledge_navigation_original::verify_ddl(&db).unwrap();
        db.execute("DROP TABLE navigation_original_rows", [])
            .unwrap();
        db.execute("CREATE TABLE navigation_original_rows(x)", [])
            .unwrap();
        assert!(crate::knowledge_navigation_original::verify_ddl(&db).is_err());
        db.execute("DROP TABLE navigation_original_rows", [])
            .unwrap();
        db.execute("DROP TABLE navigation_original_members", [])
            .unwrap();
        db.execute("DROP TABLE navigation_original_meta", [])
            .unwrap();
        db.execute("CREATE TABLE raw_records(x)", []).unwrap();
        assert!(verify_selected_table_allowlist(&db).is_err());
        db.execute("DROP TABLE raw_records", []).unwrap();
        db.execute("CREATE TABLE search_pending_grams(x)", [])
            .unwrap();
        assert!(verify_selected_table_allowlist(&db).is_err());
    }

    #[test]
    fn rank_array_rejects_non_strings_and_invalid_json() {
        assert!(rank_array("[\"a\",\"ß\"]", limits()).is_ok());
        assert!(rank_array("[\"a\",4]", limits()).is_err());
        assert!(rank_array("[\"a\",]", limits()).is_err());
    }
}
/// Borrowed cold verifier for the authenticated Native v1 catalog.
/// The caller must pass the once-clipped cold RuntimeKnowledgeReadContext; this
/// helper shares its absolute work, SQL, JSON-visit, heap, cancellation and
/// deadline owners and never initializes or refunds a local counter.
/// This Native cold path charges the same original aggregate phase more
/// precisely and may refuse earlier; it makes no claim of legacy-fit parity.
pub(crate) fn verify_catalog_with_owned_context(
    db: &Connection,
    expected: &KnowledgeSelectedExpectation,
    limits: ColdOpenLimits,
    context: &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
) -> Result<()> {
    let state = context.owned_state();
    let frame = std::mem::size_of::<(
        &Connection,
        &KnowledgeSelectedExpectation,
        ColdOpenLimits,
        &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
        &crate::d1_public_capture::CreationState<'_>,
        &str,
        Digest256Hasher,
        [u64; 4],
        [u64; 4],
        Option<(
            String,
            String,
            crate::d1_public_capture::CreationStateHold<'_, '_>,
            crate::d1_public_capture::CreationStateHold<'_, '_>,
            crate::d1_public_capture::CreationStateHold<'_, '_>,
            crate::d1_public_capture::CreationStateHold<'_, '_>,
        )>,
        Option<(String, crate::d1_public_capture::CreationStateHold<'_, '_>)>,
        std::slice::Iter<'_, ExpectedSourceScope>,
        [i64; 2],
        u64,
        bool,
        Result<(Digest256Hasher, u64, u64, u64, u64, bool)>,
        Result<()>,
        usize,
        [(&'static std::ffi::CStr, u64); 4],
        std::array::IntoIter<(&'static std::ffi::CStr, u64), 4>,
        (&'static std::ffi::CStr, u64),
        std::cmp::Ordering,
        JsonLimits,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>();
    let _frame = state.hold(frame)?;
    state.active()?;
    if expected.managed_source_root_sha256.is_some() {
        return Err(Error::ManagedSourceUnsupported(
            "managed catalog owner parser is required",
        ));
    }
    if limits.max_rows == 0
        || limits.max_sources == 0
        || expected.source_scopes.len() > limits.max_sources
        || limits.max_metadata_bytes == 0
        || limits.max_row_bytes == 0
    {
        return Err(Error::Budget("knowledge catalog cold limits"));
    }
    let descriptor = expected.descriptor_sha256.as_str();
    let metadata_cap = i64::try_from(limits.max_metadata_bytes)
        .map_err(|_| Error::Budget("knowledge catalog metadata cap"))?;
    let packet_cap = i64::try_from(limits.max_row_bytes)
        .map_err(|_| Error::Budget("knowledge catalog packet cap"))?;
    if owned_catalog_count(db, c"SELECT COUNT(*) FROM catalog_index_meta", state)? != 1 {
        return Err(Error::Invalid("knowledge catalog meta coverage"));
    }

    let (mut root, sources, fields, values, routes, legacy_ascii) =
        with_owned_schema_statement(
            db,
            c"SELECT CASE WHEN length(CAST(catalog_packet_sha256 AS BLOB))<=?2 THEN catalog_packet_sha256 ELSE NULL END,
                    CASE WHEN length(CAST(index_schema AS BLOB))<=?2 THEN index_schema ELSE NULL END,
                    CASE WHEN length(CAST(order_profile AS BLOB))<=?2 THEN order_profile ELSE NULL END,
                    source_count,facet_field_count,facet_value_count,route_count,
                    CASE WHEN length(CAST(catalog_index_root_sha256 AS BLOB))<=?2 THEN catalog_index_root_sha256 ELSE NULL END,
                    packet_len,
                    CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,
                    CASE WHEN length(packet)<=?3 THEN packet ELSE NULL END
             FROM catalog_index_meta WHERE descriptor_sha256=?1",
            state,
            |statement| {
                let _callback_frame = state.hold(std::mem::size_of::<(
                    &mut tos_source_store::PinnedBoundedStatement<'_>,
                    &crate::d1_public_capture::CreationState<'_>,
                    &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
                    &KnowledgeSelectedExpectation,
                    ColdOpenLimits,
                    &str,
                    (&str, &str, &str, &str),
                    (u64, u64, u64, u64, u64, u64, u64),
                    (&[u8], &[u8]),
                    Digest256,
                    Digest256Hasher,
                    usize,
                    JsonLimits,
                    std::slice::Iter<'_, ExpectedSourceScope>,
                    &ExpectedSourceScope,
                    [u64; 6],
                    std::array::IntoIter<u64, 6>,
                    u64,
                    [u8; 8],
                    Result<(Digest256Hasher, u64, u64, u64, u64, bool)>,
                    Result<()>,
                    crate::d1_public_capture::CreationStateHold<'_, '_>,
                )>())?;
                state.charge_work(descriptor.len().checked_add(16).ok_or(
                    Error::Budget("knowledge catalog metadata bind work"),
                )?)?;
                statement
                    .bind_text(1, descriptor)
                    .map_err(owned_schema_sql_error)?;
                statement
                    .bind_i64(2, metadata_cap)
                    .map_err(owned_schema_sql_error)?;
                statement
                    .bind_i64(3, packet_cap)
                    .map_err(owned_schema_sql_error)?;
                if !owned_schema_step(statement, state)? {
                    return Err(Error::Invalid("knowledge catalog meta missing"));
                }

                let catalog_sha =
                    owned_catalog_sql_text(statement, 0, limits.max_metadata_bytes, state)?;
                let schema =
                    owned_catalog_sql_text(statement, 1, limits.max_metadata_bytes, state)?;
                let order_profile =
                    owned_catalog_sql_text(statement, 2, limits.max_metadata_bytes, state)?;
                let sources = nonnegative(statement.integer(3).map_err(owned_schema_sql_error)?)?;
                let fields = nonnegative(statement.integer(4).map_err(owned_schema_sql_error)?)?;
                let values = nonnegative(statement.integer(5).map_err(owned_schema_sql_error)?)?;
                let routes = nonnegative(statement.integer(6).map_err(owned_schema_sql_error)?)?;
                let index_root =
                    owned_catalog_sql_text(statement, 7, limits.max_metadata_bytes, state)?;
                let packet_len =
                    nonnegative(statement.integer(8).map_err(owned_schema_sql_error)?)?;
                let rusqlite::types::ValueRef::Blob(packet_sha) =
                    statement.value_ref(9).map_err(owned_schema_sql_error)?
                else {
                    return Err(Error::Invalid("knowledge catalog packet SHA type"));
                };
                let packet = match statement.value_ref(10).map_err(owned_schema_sql_error)? {
                    rusqlite::types::ValueRef::Blob(packet) => packet,
                    rusqlite::types::ValueRef::Null => {
                        return Err(Error::Budget("knowledge catalog packet bytes"));
                    }
                    _ => return Err(Error::Invalid("knowledge catalog packet bytes type")),
                };

                let expected_sources = u64::try_from(expected.source_scopes.len())
                    .map_err(|_| Error::Budget("knowledge catalog source count"))?;
                let packet_bytes = u64::try_from(packet.len())
                    .map_err(|_| Error::Budget("knowledge catalog packet addressability"))?;
                if !owned_schema_equal(state, schema.as_bytes(), b"tos_catalog_index_v1")?
                    || !(owned_schema_equal(
                        state,
                        order_profile.as_bytes(),
                        crate::knowledge_catalog_index::ORDER_PROFILE.as_bytes(),
                    )? || owned_schema_equal(
                        state,
                        order_profile.as_bytes(),
                        crate::knowledge_catalog_index::LEGACY_ORDER_PROFILE.as_bytes(),
                    )?)
                    || !owned_schema_equal(
                        state,
                        catalog_sha.as_bytes(),
                        expected.catalog_packet_sha256.as_bytes(),
                    )?
                    || !owned_schema_equal(
                        state,
                        index_root.as_bytes(),
                        expected.catalog_index_root_sha256.as_bytes(),
                    )?
                    || sources != expected_sources
                    || sources > limits.max_sources as u64
                    || packet_len != packet_bytes
                {
                    return Err(Error::Invalid("knowledge catalog packet/meta binding"));
                }
                let packet_digest = owned_catalog_digest(packet, state)?;
                if !owned_schema_equal(state, packet_sha, packet_digest.as_bytes())?
                    || !owned_catalog_lower_hex_matches(state, &packet_digest, catalog_sha)?
                {
                    return Err(Error::Invalid("knowledge catalog packet digest binding"));
                }

                let legacy_ascii = owned_schema_equal(
                    state,
                    order_profile.as_bytes(),
                    crate::knowledge_catalog_index::LEGACY_ORDER_PROFILE.as_bytes(),
                )?;
                if legacy_ascii {
                    for scope in &expected.source_scopes {
                        if !owned_catalog_is_ascii(scope.source_graph.as_bytes(), state)? {
                            return Err(Error::Invalid("legacy catalog ASCII source profile"));
                        }
                    }
                }

                let visits = context.remaining_json_visits()?.min(1_000_000);
                let packet_limits = JsonLimits::new(
                    limits.max_row_bytes,
                    96,
                    visits,
                    4096,
                )
                .map_err(|_| Error::Budget("knowledge catalog JSON limits"))?;
                let validate_packet_json = |_document: &JsonValue| {
                    let _json_callback_frame = state.hold(std::mem::size_of::<(
                        &JsonValue,
                        &crate::d1_public_capture::CreationState<'_>,
                        Result<()>,
                        crate::d1_public_capture::CreationStateHold<'_, '_>,
                    )>())?;
                    Ok(())
                };
                let _json_operation_hold = state.hold(std::mem::size_of_val(&validate_packet_json))?;
                let context_result = context
                    .with_foundation_owned_with_limits(packet, packet_limits, validate_packet_json)
                    .map_err(|error| match error {
                        Error::Invalid("runtime carrier creation JSON") => {
                            Error::Invalid("knowledge catalog packet JSON")
                        }
                        other => other,
                    });
                drop(_json_operation_hold);
                let _ = context_result?;
                let mut root = Digest256Hasher::new();
                owned_scope_hash_text(state, &mut root, schema)?;
                owned_scope_hash_text(state, &mut root, order_profile)?;
                owned_scope_hash_text(state, &mut root, descriptor)?;
                owned_scope_hash_text(state, &mut root, catalog_sha)?;
                state.charge_work(80)?;
                root.update(packet_sha);
                for count in [
                    expected.node_count,
                    expected.relation_count,
                    fields,
                    values,
                    routes,
                    sources,
                ] {
                    state.active()?;
                    root.update(&count.to_be_bytes());
                }
                if owned_schema_step(statement, state)? {
                    return Err(Error::Invalid("knowledge duplicate catalog meta"));
                }
                Ok((root, sources, fields, values, routes, legacy_ascii))
            },
        )?;

    let mut seen = [0u64; 4];
    with_owned_schema_statement(
        db,
        c"SELECT CASE WHEN length(CAST(domain AS BLOB))<=?2 THEN domain ELSE NULL END,
                 CASE WHEN length(CAST(field_id AS BLOB))<=?2 THEN field_id ELSE NULL END,
                 value_count,total_count
          FROM catalog_facet_fields WHERE descriptor_sha256=?1 ORDER BY domain,field_id",
        state,
        |statement| {
            let _callback_frame = state.hold(std::mem::size_of::<(
                &mut tos_source_store::PinnedBoundedStatement<'_>,
                &crate::d1_public_capture::CreationState<'_>,
                &mut Digest256Hasher,
                &mut [u64; 4],
                &str,
                &str,
                u64,
                u64,
                bool,
                &[&str; 2],
                Result<bool>,
                [u8; 8],
                crate::d1_public_capture::CreationStateHold<'_, '_>,
            )>())?;
            state.charge_work(
                descriptor
                    .len()
                    .checked_add(8)
                    .ok_or(Error::Budget("knowledge catalog fields bind work"))?,
            )?;
            statement
                .bind_text(1, descriptor)
                .map_err(owned_schema_sql_error)?;
            statement
                .bind_i64(2, metadata_cap)
                .map_err(owned_schema_sql_error)?;
            while owned_schema_step(statement, state)? {
                state.charge_work(1)?;
                let domain =
                    owned_catalog_sql_text(statement, 0, limits.max_metadata_bytes, state)?;
                let field = owned_catalog_sql_text(statement, 1, limits.max_metadata_bytes, state)?;
                let value_count =
                    nonnegative(statement.integer(2).map_err(owned_schema_sql_error)?)?;
                let total_count =
                    nonnegative(statement.integer(3).map_err(owned_schema_sql_error)?)?;
                if !owned_catalog_text_equals_any(state, domain, &["node", "relation"])?
                    || field.is_empty()
                {
                    return Err(Error::Invalid("knowledge catalog field shape"));
                }
                state.charge_work(17)?;
                root.update(b"F");
                owned_scope_hash_text(state, &mut root, domain)?;
                owned_scope_hash_text(state, &mut root, field)?;
                root.update(&value_count.to_be_bytes());
                root.update(&total_count.to_be_bytes());
                seen[0] = seen[0]
                    .checked_add(1)
                    .ok_or(Error::Budget("catalog field rows"))?;
                if seen[0] > limits.max_rows {
                    return Err(Error::Budget("catalog field rows"));
                }
            }
            Ok(())
        },
    )?;

    let mut previous_field: Option<(
        String,
        String,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )> = None;
    let mut previous_fold: Option<(String, crate::d1_public_capture::CreationStateHold<'_, '_>)> =
        None;
    let mut ordinal_for_field = 0u64;
    with_owned_schema_statement(
        db,
        c"SELECT CASE WHEN length(CAST(domain AS BLOB))<=?2 THEN domain ELSE NULL END,
                 CASE WHEN length(CAST(field_id AS BLOB))<=?2 THEN field_id ELSE NULL END,
                 ordinal,
                 CASE WHEN length(CAST(value_json AS BLOB))<=?3 THEN value_json ELSE NULL END,
                 item_count
          FROM catalog_facets WHERE descriptor_sha256=?1
          ORDER BY domain,field_id,ordinal",
        state,
        |statement| {
            let _callback_frame = state.hold(std::mem::size_of::<(
                &mut tos_source_store::PinnedBoundedStatement<'_>,
                &crate::d1_public_capture::CreationState<'_>,
                &crate::knowledge_payload_read::RuntimeKnowledgeReadContext<'_, '_>,
                &mut Digest256Hasher,
                &mut [u64; 4],
                &mut Option<(
                    String,
                    String,
                    crate::d1_public_capture::CreationStateHold<'_, '_>,
                    crate::d1_public_capture::CreationStateHold<'_, '_>,
                )>,
                &mut Option<(String, crate::d1_public_capture::CreationStateHold<'_, '_>)>,
                &mut u64,
                (&str, &str, &str),
                (u64, u64),
                (bool, usize, usize, JsonLimits),
                (String, String, String),
                crate::d1_public_capture::CreationStateHold<'_, '_>,
                crate::d1_public_capture::CreationStateHold<'_, '_>,
                crate::d1_public_capture::CreationStateHold<'_, '_>,
                crate::d1_public_capture::CreationStateHold<'_, '_>,
                Result<(
                    String,
                    crate::d1_public_capture::CreationStateHold<'_, '_>,
                    crate::d1_public_capture::CreationStateHold<'_, '_>,
                )>,
                Result<(
                    String,
                    crate::d1_public_capture::CreationStateHold<'_, '_>,
                    crate::d1_public_capture::CreationStateHold<'_, '_>,
                )>,
                Result<String>,
                std::cmp::Ordering,
                Result<()>,
                crate::d1_public_capture::CreationStateHold<'_, '_>,
            )>())?;
            state.charge_work(
                descriptor
                    .len()
                    .checked_add(16)
                    .ok_or(Error::Budget("knowledge catalog facets bind work"))?,
            )?;
            statement
                .bind_text(1, descriptor)
                .map_err(owned_schema_sql_error)?;
            statement
                .bind_i64(2, metadata_cap)
                .map_err(owned_schema_sql_error)?;
            statement
                .bind_i64(3, packet_cap)
                .map_err(owned_schema_sql_error)?;
            while owned_schema_step(statement, state)? {
                state.charge_work(1)?;
                let domain =
                    owned_catalog_sql_text(statement, 0, limits.max_metadata_bytes, state)?;
                let field = owned_catalog_sql_text(statement, 1, limits.max_metadata_bytes, state)?;
                let ordinal = nonnegative(statement.integer(2).map_err(owned_schema_sql_error)?)?;
                let value_json = owned_catalog_sql_text(statement, 3, limits.max_row_bytes, state)?;
                let item_count =
                    nonnegative(statement.integer(4).map_err(owned_schema_sql_error)?)?;
                let group_start = match previous_field.as_ref() {
                    Some((prior_domain, prior_field, _, _, _, _)) => {
                        owned_catalog_compare(state, prior_domain, domain)?
                            != std::cmp::Ordering::Equal
                            || owned_catalog_compare(state, prior_field, field)?
                                != std::cmp::Ordering::Equal
                    }
                    None => true,
                };
                if group_start {
                    ordinal_for_field = 0;
                    previous_fold = None;
                    previous_field = None;
                }
                if ordinal != ordinal_for_field || item_count == 0 {
                    return Err(Error::Invalid("knowledge catalog facet ordinal/count"));
                }
                if group_start {
                    let (domain_copy, domain_hold, domain_capacity_hold) =
                        owned_catalog_key_copy(domain, state)?;
                    let (field_copy, field_hold, field_capacity_hold) =
                        owned_catalog_key_copy(field, state)?;
                    previous_field = Some((
                        domain_copy,
                        field_copy,
                        domain_hold,
                        domain_capacity_hold,
                        field_hold,
                        field_capacity_hold,
                    ));
                }
                ordinal_for_field = ordinal_for_field
                    .checked_add(1)
                    .ok_or(Error::Budget("knowledge catalog facet ordinal"))?;

                let visits = context.remaining_json_visits()?.min(1_000_000);
                let value_limits = JsonLimits::new(limits.max_row_bytes, 96, visits, 4096)
                    .map_err(|_| Error::Budget("knowledge catalog facet JSON limits"))?;
                let fold_cap = usize::try_from(limits.max_work_bytes)
                    .map_err(|_| Error::Budget("knowledge catalog casefold cap"))?;
                let fold_operation = |document: &JsonValue| {
                    let _json_callback_frame = state.hold(std::mem::size_of::<(
                        &JsonValue,
                        &crate::d1_public_capture::CreationState<'_>,
                        &str,
                        bool,
                        usize,
                        Result<String>,
                        crate::d1_public_capture::CreationStateHold<'_, '_>,
                    )>())?;
                    let value = document
                        .as_str()
                        .ok_or(Error::Invalid("knowledge catalog facet JSON string"))?;
                    if legacy_ascii && !owned_catalog_is_ascii(value.as_bytes(), state)? {
                        return Err(Error::Invalid("legacy catalog ASCII facet profile"));
                    }
                    crate::catalog::python_casefold_with_state(value, state, fold_cap)
                };
                let _fold_operation_hold = state.hold(std::mem::size_of_val(&fold_operation))?;
                let context_result = context
                    .with_foundation_owned_with_limits(
                        value_json.as_bytes(),
                        value_limits,
                        fold_operation,
                    )
                    .map_err(|error| match error {
                        Error::Invalid("runtime carrier creation JSON") => {
                            Error::Invalid("knowledge catalog facet JSON string")
                        }
                        other => other,
                    });
                drop(_fold_operation_hold);
                let (folded, folded_hold) = context_result?;
                if !group_start {
                    if let Some((previous, _)) = previous_fold.as_ref() {
                        if owned_catalog_compare(state, previous, &folded)?
                            == std::cmp::Ordering::Greater
                        {
                            return Err(Error::Invalid("knowledge catalog casefold order"));
                        }
                    }
                }
                state.charge_work(17)?;
                root.update(b"V");
                owned_scope_hash_text(state, &mut root, domain)?;
                owned_scope_hash_text(state, &mut root, field)?;
                root.update(&ordinal.to_be_bytes());
                owned_scope_hash_text(state, &mut root, value_json)?;
                root.update(&item_count.to_be_bytes());
                seen[1] = seen[1]
                    .checked_add(1)
                    .ok_or(Error::Budget("catalog facet rows"))?;
                if seen[1] > limits.max_rows {
                    return Err(Error::Budget("catalog facet rows"));
                }
                previous_fold = Some((folded, folded_hold));
            }
            Ok(())
        },
    )?;

    with_owned_schema_statement(
        db,
        c"SELECT CASE WHEN length(CAST(route_id AS BLOB))<=?2 THEN route_id ELSE NULL END,
                 ordinal,node_count,confirming_relation_count,semantic_confirming_relation_count,
                 CASE WHEN length(CAST(availability AS BLOB))<=?2 THEN availability ELSE NULL END,
                 CASE WHEN length(CAST(role_readiness AS BLOB))<=?2 THEN role_readiness ELSE NULL END,
                 packet_len,
                 CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,
                 CASE WHEN length(packet)<=?3 THEN packet ELSE NULL END
          FROM catalog_routes WHERE descriptor_sha256=?1 ORDER BY ordinal",
        state,
        |statement| {
            let _callback_frame = state.hold(std::mem::size_of::<(
                &mut tos_source_store::PinnedBoundedStatement<'_>,
                &crate::d1_public_capture::CreationState<'_>,
                &mut Digest256Hasher,
                &mut [u64; 4],
                &str,
                (&str, &str, &str),
                (u64, u64, u64, u64, u64, u64),
                (&[u8], &[u8]),
                Digest256,
                [u64; 4],
                std::array::IntoIter<u64, 4>,
                u64,
                Result<()>,
                crate::d1_public_capture::CreationStateHold<'_, '_>,
            )>())?;
            state.charge_work(descriptor.len().checked_add(16).ok_or(
                Error::Budget("knowledge catalog route bind work"),
            )?)?;
            statement
                .bind_text(1, descriptor)
                .map_err(owned_schema_sql_error)?;
            statement
                .bind_i64(2, metadata_cap)
                .map_err(owned_schema_sql_error)?;
            statement
                .bind_i64(3, packet_cap)
                .map_err(owned_schema_sql_error)?;
            while owned_schema_step(statement, state)? {
                state.charge_work(1)?;
                let id =
                    owned_catalog_sql_text(statement, 0, limits.max_metadata_bytes, state)?;
                let ordinal =
                    nonnegative(statement.integer(1).map_err(owned_schema_sql_error)?)?;
                let nodes =
                    nonnegative(statement.integer(2).map_err(owned_schema_sql_error)?)?;
                let confirming =
                    nonnegative(statement.integer(3).map_err(owned_schema_sql_error)?)?;
                let semantic =
                    nonnegative(statement.integer(4).map_err(owned_schema_sql_error)?)?;
                let availability =
                    owned_catalog_sql_text(statement, 5, limits.max_metadata_bytes, state)?;
                let readiness =
                    owned_catalog_sql_text(statement, 6, limits.max_metadata_bytes, state)?;
                let packet_len =
                    nonnegative(statement.integer(7).map_err(owned_schema_sql_error)?)?;
                let rusqlite::types::ValueRef::Blob(digest) =
                    statement.value_ref(8).map_err(owned_schema_sql_error)?
                else {
                    return Err(Error::Invalid("catalog route SHA type"));
                };
                let packet = match statement.value_ref(9).map_err(owned_schema_sql_error)? {
                    rusqlite::types::ValueRef::Blob(packet) => packet,
                    rusqlite::types::ValueRef::Null => {
                        return Err(Error::Budget("catalog route packet bytes"));
                    }
                    _ => return Err(Error::Invalid("catalog route packet bytes type")),
                };
                let packet_bytes = u64::try_from(packet.len())
                    .map_err(|_| Error::Budget("catalog route packet length"))?;
                if id.is_empty()
                    || ordinal != seen[2]
                    || nodes > expected.node_count
                    || confirming > expected.relation_count
                    || semantic > expected.relation_count
                    || !owned_catalog_text_equals_any(
                        state,
                        availability,
                        &["available", "not_projected"],
                    )?
                    || !owned_catalog_text_equals_any(
                        state,
                        readiness,
                        &["not_projected", "kind_only", "confirmed"],
                    )?
                    || packet_len != packet_bytes
                {
                    return Err(Error::Invalid("knowledge catalog route closure"));
                }
                let actual_digest = owned_catalog_digest(packet, state)?;
                if !owned_schema_equal(state, digest, actual_digest.as_bytes())? {
                    return Err(Error::Invalid("knowledge catalog route digest"));
                }
                state.charge_work(73)?;
                root.update(b"R");
                owned_scope_hash_text(state, &mut root, id)?;
                for count in [ordinal, nodes, confirming, semantic] {
                    root.update(&count.to_be_bytes());
                }
                owned_scope_hash_text(state, &mut root, availability)?;
                owned_scope_hash_text(state, &mut root, readiness)?;
                root.update(&packet_len.to_be_bytes());
                root.update(digest);
                seen[2] = seen[2]
                    .checked_add(1)
                    .ok_or(Error::Budget("catalog route rows"))?;
                if seen[2] > limits.max_rows {
                    return Err(Error::Budget("catalog route rows"));
                }
            }
            Ok(())
        },
    )?;

    with_owned_schema_statement(
        db,
        c"SELECT CASE WHEN length(CAST(source_graph_id AS BLOB))<=?2 THEN source_graph_id ELSE NULL END,
                 node_count,relation_count
          FROM catalog_source_counts WHERE descriptor_sha256=?1 ORDER BY source_graph_id",
        state,
        |statement| {
            let _callback_frame = state.hold(std::mem::size_of::<(
                &mut tos_source_store::PinnedBoundedStatement<'_>,
                &crate::d1_public_capture::CreationState<'_>,
                &Connection,
                &KnowledgeSelectedExpectation,
                &str,
                std::slice::Iter<'_, ExpectedSourceScope>,
                &ExpectedSourceScope,
                &str,
                (u64, u64),
                usize,
                [(&str, u64); 2],
                std::array::IntoIter<(&str, u64), 2>,
                (&str, u64),
                usize,
                Result<()>,
                Result<&str>,
                crate::d1_public_capture::CreationStateHold<'_, '_>,
            )>())?;
            state.charge_work(descriptor.len().checked_add(8).ok_or(
                Error::Budget("knowledge catalog source bind work"),
            )?)?;
            statement
                .bind_text(1, descriptor)
                .map_err(owned_schema_sql_error)?;
            statement
                .bind_i64(2, metadata_cap)
                .map_err(owned_schema_sql_error)?;
            for scope in &expected.source_scopes {
                if !owned_schema_step(statement, state)? {
                    return Err(Error::Invalid("catalog source omitted"));
                }
                state.charge_work(1)?;
                let source =
                    owned_catalog_sql_text(statement, 0, limits.max_metadata_bytes, state)?;
                let nodes =
                    nonnegative(statement.integer(1).map_err(owned_schema_sql_error)?)?;
                let relations =
                    nonnegative(statement.integer(2).map_err(owned_schema_sql_error)?)?;
                if !owned_schema_equal(
                    state,
                    source.as_bytes(),
                    scope.source_graph.as_bytes(),
                )? || nodes != scope.node_count
                    || relations != scope.relation_count
                {
                    return Err(Error::Invalid("knowledge catalog source counts"));
                }
                if seen[3] >= limits.max_rows {
                    return Err(Error::Budget("catalog source rows"));
                }

                let serialized_cap = scope
                    .source_graph
                    .len()
                    .checked_mul(6)
                    .and_then(|n| n.checked_add(2))
                    .ok_or(Error::Budget("catalog source JSON cap"))?;
                let callback = |wire: &[u8]| -> Result<()> {
                    let _json_callback_frame = state.hold(std::mem::size_of::<(
                        &[u8],
                        &crate::d1_public_capture::CreationState<'_>,
                        &str,
                        [(&str, u64); 2],
                        std::array::IntoIter<(&str, u64), 2>,
                        (&str, u64),
                        &Connection,
                        &str,
                        usize,
                        u64,
                        Result<()>,
                        crate::d1_public_capture::CreationStateHold<'_, '_>,
                    )>())?;
                    let json = owned_catalog_utf8(wire, state)?;
                    for (domain, count) in [("node", nodes), ("relation", relations)] {
                        with_owned_schema_statement(
                            db,
                            c"SELECT item_count FROM catalog_facets WHERE descriptor_sha256=?1 AND domain=?2 AND field_id='source_graph' AND value_json=?3",
                            state,
                            |facet| {
                                let _facet_callback_frame = state.hold(std::mem::size_of::<(
                                    &mut tos_source_store::PinnedBoundedStatement<'_>,
                                    &crate::d1_public_capture::CreationState<'_>,
                                    &str,
                                    &str,
                                    &str,
                                    u64,
                                    usize,
                                    u64,
                                    bool,
                                    Result<()>,
                                    crate::d1_public_capture::CreationStateHold<'_, '_>,
                                )>())?;
                                let bind_work = descriptor
                                    .len()
                                    .checked_add(domain.len())
                                    .and_then(|n| n.checked_add(json.len()))
                                    .ok_or(Error::Budget(
                                        "knowledge catalog source facet bind work",
                                    ))?;
                                state.charge_work(bind_work)?;
                                facet
                                    .bind_text(1, descriptor)
                                    .map_err(owned_schema_sql_error)?;
                                facet
                                    .bind_text(2, domain)
                                    .map_err(owned_schema_sql_error)?;
                                facet
                                    .bind_text(3, json)
                                    .map_err(owned_schema_sql_error)?;
                                if !owned_schema_step(facet, state)? {
                                    if count != 0 {
                                        return Err(Error::Invalid(
                                            "knowledge catalog source facet coverage",
                                        ));
                                    }
                                    return Ok(());
                                }
                                let actual = facet
                                    .unsigned_integer(0)
                                    .map_err(owned_schema_sql_error)?;
                                if owned_schema_step(facet, state)? {
                                    return Err(Error::Invalid(
                                        "knowledge catalog duplicate source facet",
                                    ));
                                }
                                if actual != count {
                                    return Err(Error::Invalid(
                                        "knowledge catalog source facet coverage",
                                    ));
                                }
                                Ok(())
                            },
                        )?;
                    }
                    Ok(())
                };
                let _callback_hold = state.hold(std::mem::size_of_val(&callback))?;
                state.with_json_encoded(&scope.source_graph, serialized_cap, callback)?;

                state.charge_work(17)?;
                root.update(b"S");
                owned_scope_hash_text(state, &mut root, source)?;
                root.update(&nodes.to_be_bytes());
                root.update(&relations.to_be_bytes());
                seen[3] = seen[3]
                    .checked_add(1)
                    .ok_or(Error::Budget("catalog source rows"))?;
            }
            if owned_schema_step(statement, state)? {
                return Err(Error::Invalid("catalog source extra row"));
            }
            Ok(())
        },
    )?;

    if seen != [fields, values, routes, sources] {
        return Err(Error::Invalid("knowledge catalog index coverage"));
    }
    if !owned_catalog_lower_hex_matches(
        state,
        &root.finalize(),
        &expected.catalog_index_root_sha256,
    )? {
        return Err(Error::Invalid("knowledge catalog index root"));
    }
    if owned_catalog_has_row(
        db,
        c"SELECT 1 FROM catalog_facets v WHERE NOT EXISTS(
              SELECT 1 FROM catalog_facet_fields f WHERE f.descriptor_sha256=v.descriptor_sha256
              AND f.domain=v.domain AND f.field_id=v.field_id) LIMIT 1",
        state,
    )? {
        return Err(Error::Invalid("knowledge catalog orphan facet"));
    }
    if owned_catalog_has_row(
        db,
        c"SELECT 1 FROM catalog_facet_fields f WHERE
              f.value_count != (SELECT COUNT(*) FROM catalog_facets v WHERE v.descriptor_sha256=f.descriptor_sha256 AND v.domain=f.domain AND v.field_id=f.field_id)
              OR f.total_count != COALESCE((SELECT SUM(item_count) FROM catalog_facets v WHERE v.descriptor_sha256=f.descriptor_sha256 AND v.domain=f.domain AND v.field_id=f.field_id),0)
              LIMIT 1",
        state,
    )? {
        return Err(Error::Invalid("knowledge catalog field aggregates"));
    }

    for (sql, expected_count) in [
        (c"SELECT COUNT(*) FROM catalog_facet_fields", fields),
        (c"SELECT COUNT(*) FROM catalog_facets", values),
        (c"SELECT COUNT(*) FROM catalog_routes", routes),
        (c"SELECT COUNT(*) FROM catalog_source_counts", sources),
    ] {
        if owned_catalog_count(db, sql, state)? != expected_count {
            return Err(Error::Invalid("knowledge catalog foreign descriptor row"));
        }
    }
    state.active()
}

fn owned_catalog_count(
    db: &Connection,
    sql: &'static std::ffi::CStr,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<u64> {
    with_owned_schema_statement(db, sql, state, |statement| {
        let _callback_frame = state.hold(std::mem::size_of::<(
            &mut tos_source_store::PinnedBoundedStatement<'_>,
            &crate::d1_public_capture::CreationState<'_>,
            u64,
            bool,
            Result<u64>,
            crate::d1_public_capture::CreationStateHold<'_, '_>,
        )>())?;
        if !owned_schema_step(statement, state)? {
            return Err(Error::Invalid("knowledge catalog count missing"));
        }
        let count = nonnegative(statement.integer(0).map_err(owned_schema_sql_error)?)?;
        if owned_schema_step(statement, state)? {
            return Err(Error::Invalid("knowledge catalog count duplicate"));
        }
        Ok(count)
    })
}

fn owned_catalog_has_row(
    db: &Connection,
    sql: &'static std::ffi::CStr,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<bool> {
    with_owned_schema_statement(db, sql, state, |statement| {
        let _callback_frame = state.hold(std::mem::size_of::<(
            &mut tos_source_store::PinnedBoundedStatement<'_>,
            &crate::d1_public_capture::CreationState<'_>,
            bool,
            Result<bool>,
            crate::d1_public_capture::CreationStateHold<'_, '_>,
        )>())?;
        let found = owned_schema_step(statement, state)?;
        if found && owned_schema_step(statement, state)? {
            return Err(Error::Invalid("knowledge catalog existence duplicate"));
        }
        Ok(found)
    })
}

fn owned_catalog_sql_text<'a>(
    statement: &'a tos_source_store::PinnedBoundedStatement<'_>,
    column: i32,
    cap: usize,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<&'a str> {
    let frame = std::mem::size_of::<(
        &'a tos_source_store::PinnedBoundedStatement<'_>,
        i32,
        usize,
        &crate::d1_public_capture::CreationState<'_>,
        &str,
        Result<&str>,
        Result<&[u8]>,
        rusqlite::types::ValueRef<'_>,
        Result<rusqlite::types::ValueRef<'_>>,
        usize,
        tos_source_store::StoreError,
        Result<()>,
        &mut dyn FnMut(usize) -> tos_source_store::Result<()>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>();
    let _frame = state.hold(
        frame
            .checked_add(
                tos_source_store::PinnedBoundedStatement::text_validation_rust_workspace_upper_bound(),
            )
            .ok_or(Error::Budget("knowledge catalog text workspace"))?,
    )?;
    match statement
        .value_ref(column)
        .map_err(owned_schema_sql_error)?
    {
        rusqlite::types::ValueRef::Text(bytes) if bytes.len() <= cap => {}
        rusqlite::types::ValueRef::Null => {
            return Err(Error::Budget("knowledge catalog text bytes"));
        }
        rusqlite::types::ValueRef::Text(_) => {
            return Err(Error::Budget("knowledge catalog text bytes"));
        }
        _ => return Err(Error::Invalid("knowledge catalog text type")),
    }
    let mut check = |bytes| {
        state.charge_work(bytes).map_err(|_| {
            tos_source_store::StoreError::new(
                tos_source_store::StoreErrorCode::BudgetExceeded,
                "knowledge catalog text validation work",
            )
        })
    };
    let _check = state.hold(std::mem::size_of_val(&check))?;
    let text = statement
        .text_with_check(column, &mut check)
        .map_err(owned_schema_sql_error)?;
    Ok(text)
}

fn owned_catalog_text_equals_any(
    state: &crate::d1_public_capture::CreationState<'_>,
    text: &str,
    choices: &[&str],
) -> Result<bool> {
    let _frame = state.hold(std::mem::size_of::<(
        &crate::d1_public_capture::CreationState<'_>,
        &str,
        &[&str],
        std::slice::Iter<'_, &str>,
        Result<bool>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    for choice in choices {
        if owned_schema_equal(state, text.as_bytes(), choice.as_bytes())? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn owned_catalog_key_copy<'state, 'budget>(
    value: &str,
    state: &'state crate::d1_public_capture::CreationState<'budget>,
) -> Result<(
    String,
    crate::d1_public_capture::CreationStateHold<'state, 'budget>,
    crate::d1_public_capture::CreationStateHold<'state, 'budget>,
)> {
    let frame = std::mem::size_of::<(
        &str,
        &crate::d1_public_capture::CreationState<'_>,
        String,
        std::str::Chars<'_>,
        usize,
        usize,
        usize,
        char,
        Result<(
            String,
            crate::d1_public_capture::CreationStateHold<'state, 'budget>,
            crate::d1_public_capture::CreationStateHold<'state, 'budget>,
        )>,
        std::result::Result<(), std::collections::TryReserveError>,
        Option<usize>,
        crate::Result<usize>,
        crate::Result<()>,
        Result<crate::d1_public_capture::CreationStateHold<'state, 'budget>>,
        Result<crate::d1_public_capture::CreationStateHold<'state, 'budget>>,
        crate::d1_public_capture::CreationStateHold<'state, 'budget>,
        crate::d1_public_capture::CreationStateHold<'state, 'budget>,
    )>();
    let _frame = state.hold(frame)?;
    let requested = value.len();
    let hold = state.hold(requested)?;
    state.charge_work(requested)?;
    state.active()?;
    let mut owned = String::new();
    let reserve_result = owned.try_reserve_exact(requested);
    reserve_result.map_err(|_| Error::Budget("catalog previous facet key"))?;
    let actual_capacity = owned.capacity();
    let excess = actual_capacity
        .checked_sub(requested)
        .ok_or(Error::Budget("catalog previous facet key"))?;
    let capacity_hold = state.hold(excess)?;
    for character in value.chars() {
        state.active()?;
        owned.push(character);
    }
    state.active()?;
    Ok((owned, hold, capacity_hold))
}

fn owned_catalog_digest(
    raw: &[u8],
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<Digest256> {
    let _frame = state.hold(std::mem::size_of::<(
        &[u8],
        &crate::d1_public_capture::CreationState<'_>,
        Digest256Hasher,
        std::slice::Chunks<'_, u8>,
        &[u8],
        &[u8],
        Result<Digest256>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    state.charge_work(raw.len())?;
    let mut digest = Digest256Hasher::new();
    for chunk in raw.chunks(4096) {
        state.active()?;
        digest.update(chunk);
    }
    state.active()?;
    Ok(digest.finalize())
}

fn owned_catalog_compare(
    state: &crate::d1_public_capture::CreationState<'_>,
    left: &str,
    right: &str,
) -> Result<std::cmp::Ordering> {
    let _frame = state.hold(std::mem::size_of::<(
        &crate::d1_public_capture::CreationState<'_>,
        &str,
        &str,
        std::slice::Chunks<'_, u8>,
        std::slice::Chunks<'_, u8>,
        std::iter::Zip<std::slice::Chunks<'_, u8>, std::slice::Chunks<'_, u8>>,
        (&[u8], &[u8]),
        usize,
        std::cmp::Ordering,
        Result<std::cmp::Ordering>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    let common = left_bytes.len().min(right_bytes.len());
    state.charge_work(common)?;
    for (left_chunk, right_chunk) in left_bytes[..common]
        .chunks(4096)
        .zip(right_bytes[..common].chunks(4096))
    {
        state.active()?;
        let order = left_chunk.cmp(right_chunk);
        if order != std::cmp::Ordering::Equal {
            return Ok(order);
        }
    }
    state.active()?;
    Ok(left_bytes.len().cmp(&right_bytes.len()))
}

fn owned_catalog_lower_hex_matches(
    state: &crate::d1_public_capture::CreationState<'_>,
    digest: &Digest256,
    text: &str,
) -> Result<bool> {
    let _frame = state.hold(std::mem::size_of::<(
        &crate::d1_public_capture::CreationState<'_>,
        &Digest256,
        &str,
        std::iter::Enumerate<std::slice::Iter<'_, u8>>,
        usize,
        &u8,
        usize,
        Option<u8>,
        Option<u8>,
        u8,
        u8,
        Result<bool>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    if text.len() != 64 {
        return Ok(false);
    }
    fn nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        }
    }
    let bytes = text.as_bytes();
    for (index, expected) in digest.as_bytes().iter().enumerate() {
        state.charge_work(2)?;
        state.active()?;
        let offset = index * 2;
        let Some(high) = nibble(bytes[offset]) else {
            return Ok(false);
        };
        let Some(low) = nibble(bytes[offset + 1]) else {
            return Ok(false);
        };
        if (high << 4) | low != *expected {
            return Ok(false);
        }
    }
    Ok(true)
}

fn owned_catalog_is_ascii(
    raw: &[u8],
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<bool> {
    let _frame = state.hold(std::mem::size_of::<(
        &[u8],
        &crate::d1_public_capture::CreationState<'_>,
        std::slice::Chunks<'_, u8>,
        &[u8],
        Result<bool>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    state.charge_work(raw.len())?;
    for chunk in raw.chunks(4096) {
        state.active()?;
        if !chunk.is_ascii() {
            return Ok(false);
        }
    }
    state.active()?;
    Ok(true)
}

fn owned_catalog_utf8<'a>(
    raw: &'a [u8],
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<&'a str> {
    let _frame = state.hold(std::mem::size_of::<(
        &'a [u8],
        &crate::d1_public_capture::CreationState<'_>,
        usize,
        usize,
        &[u8],
        std::str::Utf8Error,
        Result<&'a str>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
    )>())?;
    let mut at = 0usize;
    while at < raw.len() {
        let end = at.saturating_add(65536).min(raw.len());
        state.charge_work(end - at)?;
        match std::str::from_utf8(&raw[at..end]) {
            Ok(_) => at = end,
            Err(error)
                if error.error_len().is_none() && end < raw.len() && error.valid_up_to() > 0 =>
            {
                at += error.valid_up_to();
            }
            Err(_) => return Err(Error::Invalid("knowledge catalog source JSON UTF8")),
        }
    }
    state.active()?;
    // Every byte was checked in bounded chunks; the borrowed serializer output
    // remains immutable for the full callback.
    Ok(unsafe { std::str::from_utf8_unchecked(raw) })
}
