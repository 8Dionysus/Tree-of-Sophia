//! Cold admission of one owner-selected, immutable full knowledge SQLite file.
//! Source admission, immutable inode custody and current disclosure rights are
//! independent owner obligations. No model path is reopened after admission.

use crate::{
    Error, MAX_POSTING_DELTA_BYTES, MAX_POSTINGS_PER_BLOCK, Result, decode_posting_block,
    knowledge_stage, safe_open, stream_digest,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::{
    fs::File,
    io::Seek,
    os::fd::AsRawFd,
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
    connection: Connection,
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
}

impl<'a> VerifiedKnowledgeModel<'a> {
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
        let page = crate::knowledge_corpus_original::page(
            &self.connection,
            collection,
            selector,
            after,
            max_rows,
            max_row_bytes,
            max_page_bytes,
        )?;
        self.check_pin()?;
        Ok(page)
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
        let page = crate::knowledge_philosophy_original::page(
            &self.connection,
            collection,
            after,
            max_rows,
            max_row_bytes,
            max_page_bytes,
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
    pub fn open_vm_steps(&self) -> u64 {
        self.open_vm_steps
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
        || limits.max_metadata_bytes > 256 * 1024
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
) -> Result<(Connection, Arc<AtomicU64>)> {
    if max_vm_steps == 0 {
        return Err(Error::Budget("knowledge SQLite VM steps"));
    }
    let uri = format!(
        "file:/proc/self/fd/{}?mode=ro&immutable=1",
        pinned.as_raw_fd()
    );
    let db = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
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

fn metadata(db: &Connection, key: &str, max_bytes: usize) -> Result<String> {
    let bytes: Vec<u8> = db.query_row(
        "SELECT CAST(value AS BLOB) FROM metadata WHERE key=?1 AND typeof(value) IN ('text','blob') AND length(CAST(value AS BLOB))<=?2",
        params![key, max_bytes as i64], |r| r.get(0),
    ).optional()?.ok_or(Error::Invalid("knowledge metadata absent/oversized"))?;
    String::from_utf8(bytes).map_err(|_| Error::Invalid("knowledge metadata UTF-8"))
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
        if metadata(db, key, cap)? != value {
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
        if metadata(db, key, 32)? != value.to_string() {
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
            .managed_source()
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
    let mut pinned = safe_open::open_regular(path, expected.model_size_bytes)?;
    custody.verify(&pinned, &expected)?;
    custody.verify_cold_resources(limits)?;
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
            if basis.managed_source().is_some() {
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
    if let Some(proof) = basis.managed_source() {
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
        != if basis.managed_source().is_some() {
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
    for (table, columns) in [
        ("metadata", &["key:TEXT:1", "value:BLOB:0"][..]),
        (
            "graph_header",
            &[
                "singleton:INTEGER:1",
                "packet_len:INTEGER:0",
                "packet_sha256:BLOB:0",
                "packet:BLOB:0",
            ][..],
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
            ][..],
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
            ][..],
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
            ][..],
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
            ][..],
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
            ][..],
        ),
        (
            "search_gram_stats",
            &[
                "kind:TEXT:1",
                "n:INTEGER:2",
                "gram:BLOB:3",
                "postings:INTEGER:0",
            ][..],
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
            ][..],
        ),
        (
            "catalog_facet_fields",
            &[
                "descriptor_sha256:TEXT:1",
                "domain:TEXT:2",
                "field_id:TEXT:3",
                "value_count:INTEGER:0",
                "total_count:INTEGER:0",
            ][..],
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
            ][..],
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
            ][..],
        ),
        (
            "catalog_source_counts",
            &[
                "descriptor_sha256:TEXT:1",
                "source_graph_id:TEXT:2",
                "node_count:INTEGER:0",
                "relation_count:INTEGER:0",
            ][..],
        ),
    ] {
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
        rank_array(&identity, limits)?;
        rank_array(&visible, limits)?;
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
        charge(
            work,
            id_lower.len() + native_lower.len() + identity.len() + visible.len(),
            limits.max_work_bytes,
        )?;
        hash_sql_row(&mut root, row, work, limits.max_work_bytes)?;
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
        charge(work, positions.len() * 8, limits.max_work_bytes)?;
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
        hash_sql_row(&mut root, row, work, limits.max_work_bytes)?;
        postings = postings
            .checked_add(count as u64)
            .ok_or(Error::Budget("knowledge postings"))?;
        if postings > limits.max_rows {
            return Err(Error::Budget("knowledge postings"));
        }
    }
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
        hash_sql_row(&mut root, row, work, limits.max_work_bytes)?;
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
