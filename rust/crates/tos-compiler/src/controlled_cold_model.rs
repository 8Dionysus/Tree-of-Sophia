use std::{fs::File, io::Seek};

use rusqlite::{OptionalExtension, params};
use tos_foundation::{Digest256, JsonLimits, OwnedState};

use crate::{
    ColdOpenLimits, Error, ImmutableKnowledgeCustody, KnowledgeSelectedExpectation,
    KnowledgeSourceBasis, NativeSnapshotOwnedReadLoan, QueryVocabulary, Result,
    knowledge_payload_read::{RuntimeKnowledgeReadContext, SelectedPayloadRow},
    knowledge_stage::{self, KnowledgePayloadLayout},
};

/// Only the two maintained indexed-v2 table families can enter QRY.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlledSearchKind {
    Nodes,
    Relations,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ControlledGramStat {
    pub postings: Option<u64>,
    pub vm_steps: u64,
    pub rows: u64,
    pub decoded_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct ControlledPostingPage {
    pub positions: Vec<u64>,
    pub exhausted: bool,
    pub vm_steps: u64,
    pub rows: u64,
    pub decoded_bytes: u64,
}

/// One candidate after exact logical payload verification. The row contains
/// no source path, physical connection, custody object, or packet reference.
#[derive(Clone, Debug)]
pub struct ControlledSearchCandidate {
    pub position: u64,
    pub id: String,
    pub source_graph: String,
    pub kind_id: String,
    pub predicate_id: String,
    pub id_lower: String,
    pub native_id_lower: String,
    pub identity_values: String,
    pub visible_values: String,
    pub document_chars: u64,
    pub document_digest: Digest256,
    pub payload_sha256: Digest256,
    pub payload: Vec<u8>,
    pub vm_steps: u64,
    pub rows: u64,
    pub decoded_bytes: u64,
}

fn navigation_original_retained_bytes(
    receipt: &crate::NavigationOriginalReceipt,
) -> Result<usize> {
    [
        receipt.profile.capacity(),
        receipt.descriptor_sha256.capacity(),
        receipt.source_cut.capacity(),
        receipt.membership_root.capacity(),
        receipt.source_graph.capacity(),
        receipt.node_input_root_sha256.capacity(),
        receipt.edge_input_root_sha256.capacity(),
        receipt.header_sha256.capacity(),
        receipt.rights_root_sha256.capacity(),
        receipt.component_root_sha256.capacity(),
        receipt.member_index_root_sha256.capacity(),
    ]
    .into_iter()
    .try_fold(std::mem::size_of_val(receipt), |sum, capacity| {
        sum.checked_add(capacity)
            .ok_or(Error::Budget("controlled Navigation Original heap"))
    })
}

/// Synchronous authenticated selected model. No legacy Verified wrapper,
/// raw connection accessor, Deref, warm fork, or public budget constructor.
/// Every reference remains inside cold/receipt/basis/issuer owner scopes.
pub struct ControlledKnowledgeModel<'model, 'state, 'budget> {
    connection: &'model tos_source_store::PinnedSqliteConnection,
    pinned: &'model File,
    selection: &'model KnowledgeSelectedExpectation,
    custody: &'model dyn ImmutableKnowledgeCustody,
    source_basis: &'model KnowledgeSourceBasis,
    navigation_original: Option<&'model crate::NavigationOriginalReceipt>,
    philosophy_original: Option<&'model crate::PhilosophyOriginalReceipt>,
    corpus_original: Option<&'model crate::CorpusOriginalReceipt>,
    identity: &'model crate::d1_public_capture::ControlledCaptureIdentity,
    open_vm_steps: u64,
    context: &'model RuntimeKnowledgeReadContext<'state, 'budget>,
}

impl ControlledKnowledgeModel<'_, '_, '_> {
    /// Recheck the exact held file, selected expectation and original state.
    /// A pathname or newly opened model is never accepted here.
    pub fn check_pin(&self) -> Result<()> {
        self.context.check()?;
        self.custody.verify(self.pinned, self.selection)?;
        self.context.check()
    }
    /// Charge request decoding/ranking/disclosure work on the same original
    /// CreationState as the writer and cold admission, without exposing a
    /// mutable counter or changing its aggregate ceiling.
    pub fn charge_query_work(&self, bytes: usize) -> Result<()> {
        self.context.check()?;
        self.context.charge_work(bytes)?;
        self.context.check()
    }

    /// The first selected-connection schema/configuration receipt is measured
    /// under the same cold progress hook. QRY checks its advertised startup
    /// ceiling against that observed value; later SQL uses the same counter.
    pub fn check_query_open_vm_admission(&self, maximum: u64) -> Result<()> {
        if maximum == 0 || self.open_vm_steps > maximum {
            return Err(Error::Budget("controlled query open VM admission"));
        }
        self.check_pin()
    }
    pub fn selection(&self) -> &KnowledgeSelectedExpectation {
        self.selection
    }
    pub fn source_basis(&self) -> &KnowledgeSourceBasis {
        self.source_basis
    }
    pub fn search_index_profile(&self) -> &'static str {
        crate::knowledge_search::SEARCH_PROFILE
    }
    pub fn navigation_original_receipt(&self) -> Option<&crate::NavigationOriginalReceipt> {
        self.navigation_original
    }
    pub fn philosophy_original_receipt(&self) -> Option<&crate::PhilosophyOriginalReceipt> {
        self.philosophy_original
    }
    pub fn corpus_original_receipt(&self) -> Option<&crate::CorpusOriginalReceipt> {
        self.corpus_original
    }

    /// Same-owner admission around Query-owned allocation. The caller supplies
    /// only a forecast computed by the maintained Query adapter from its actual
    /// request geometry; this method exposes neither counters nor a new grant.
    pub fn with_owned_query_workspace(
        &mut self,
        forecast_bytes: usize,
        operation: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<()> {
        if forecast_bytes == 0 {
            return Err(Error::Budget("controlled query workspace forecast"));
        }
        let context = self.context;
        context.check()?;
        // Copy the authentic context reference before the mutable model borrow;
        // the state hold remains live through the unit-only callback.
        let result = context.with_reserved_state(forecast_bytes, || operation(self));
        context.check()?;
        result
    }

    /// Bind one authored descriptor while its parse tree and Bound owner stay
    /// under a compiler-derived reservation. The callback returns unit so no
    /// descriptor view or hidden workspace owner can escape this hold.
    pub fn with_owned_binding_workspace(
        &mut self,
        vocabulary: &QueryVocabulary,
        descriptor: &[u8],
        operation: impl for<'value> FnOnce(&mut Self, &'value tos_foundation::JsonValue) -> Result<()>,
    ) -> Result<()> {
        let forecast = self.controlled_binding_workspace_upper_bound(vocabulary, descriptor)?;
        if forecast == 0 {
            return Err(Error::Budget("controlled binding workspace forecast"));
        }
        let context = self.context;
        context.check()?;
        let result = context.with_reserved_state(forecast, || {
            self.verify_authored_vocabulary(vocabulary, descriptor)?;
            let limits = JsonLimits::new(descriptor.len(), 64, 100_000, 4096)
                .map_err(|_| Error::Budget("controlled binding JSON limits"))?;
            context.with_foundation_owned_with_limits(descriptor, limits, |value| {
                operation(self, value)
            })
        });
        context.check()?;
        result
    }

    /// Verify and account the authored vocabulary against the original state.
    /// This deliberately calls the state's parser, not the unmetered fixture
    /// binder used by the static VerifiedKnowledgeModel route.
    pub fn verify_authored_vocabulary(
        &self,
        vocabulary: &QueryVocabulary,
        descriptor: &[u8],
    ) -> Result<()> {
        self.context.check()?;
        vocabulary.verify_authored_bytes_with_owned_state(descriptor, self.context.owned_state())?;
        self.context.check()
    }

    /// Conservative pre-bind forecast derived from the actual descriptor,
    /// vocabulary and authenticated selected metadata. Descriptor parser visit
    /// maxima are included because vocabulary verification and retained binding
    /// overlap while their parse trees are live.
    pub fn controlled_binding_workspace_upper_bound(
        &self,
        vocabulary: &QueryVocabulary,
        descriptor: &[u8],
    ) -> Result<usize> {
        use tos_foundation::OwnedState;
        if descriptor.is_empty() || descriptor.len() > 4 * 1024 * 1024 {
            return Err(Error::Budget("controlled query descriptor size"));
        }
        let selected = self
            .selection
            .owned_heap_bytes()
            .map_err(|_| Error::Budget("controlled selected retained state"))?;
        let basis = self
            .source_basis
            .owned_heap_bytes()
            .map_err(|_| Error::Budget("controlled source-basis retained state"))?;
        let vocab = vocabulary.query_delivery_heap_bytes()?;
        let descriptor_bytes = descriptor
            .len()
            .checked_mul(16)
            .ok_or(Error::Budget("controlled binding descriptor forecast"))?;
        let descriptor_slots = 100_000usize
            .checked_mul(std::mem::size_of::<(tos_foundation::JsonString, tos_foundation::JsonValue)>())
            .ok_or(Error::Budget("controlled binding parser forecast"))?;
        let retained = std::mem::size_of::<Self>()
            .checked_add(selected)
            .and_then(|n| n.checked_add(basis))
            .and_then(|n| n.checked_add(vocab))
            .and_then(|n| n.checked_add(descriptor_bytes))
            .and_then(|n| n.checked_add(descriptor_slots))
            .and_then(|n| n.checked_add(descriptor.len()))
            .ok_or(Error::Budget("controlled binding forecast overflow"))?;
        Ok(retained)
    }

    pub fn gram_stat(
        &mut self,
        kind: ControlledSearchKind,
        gram: &str,
        max_vm_steps: u64,
        max_rows: u64,
        max_decoded_bytes: u64,
    ) -> Result<ControlledGramStat> {
        self.check_pin()?;
        if gram.chars().count() != 3
            || gram.len() > 12
            || max_vm_steps == 0
            || max_rows == 0
            || max_decoded_bytes < 8
        {
            return Err(Error::Budget("controlled gram-stat admission"));
        }
        let fixed = std::mem::size_of::<(
            &Self,
            ControlledSearchKind,
            &str,
            u64,
            u64,
            u64,
            Option<i64>,
            Result<Option<Option<i64>>>,
            Result<ControlledGramStat>,
        )>()
        .checked_add(tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound())
        .ok_or(Error::Budget("controlled gram-stat frame"))?;
        let _hold = self.context.owned_state().hold(fixed)?;
        self.context.charge_work(gram.len().checked_add(16).ok_or(Error::Budget("controlled gram-stat work"))?)?;
        let table_kind = match kind {
            ControlledSearchKind::Nodes => "nodes",
            ControlledSearchKind::Relations => "relations",
        };
        let (selected, vm_steps) = crate::knowledge_payload_read::with_query_vm_window(
            self.context,
            &self.connection,
            max_vm_steps,
            || {
                self.connection
                    .query_row(
                        "SELECT CASE WHEN typeof(postings)='integer' THEN postings END FROM search_gram_stats WHERE kind=?1 AND n=3 AND gram=?2",
                        params![table_kind, gram.as_bytes()],
                        |row| row.get::<_, Option<i64>>(0),
                    )
                    .optional()
                    .map_err(Error::from)
            },
        )?;

        let selected = selected.flatten();
        if selected.is_some_and(|postings| postings < 0) || max_rows < 1 {
            return Err(Error::Invalid("controlled gram-stat row shape"));
        }
        self.check_pin()?;
        let rows = u64::from(selected.is_some());
        Ok(ControlledGramStat {
            postings: selected.map(|postings| postings as u64),
            vm_steps,
            rows,
            decoded_bytes: rows * 8,
        })
    }

    pub fn seek_postings(
        &mut self,
        kind: ControlledSearchKind,
        gram: &str,
        after: Option<u64>,
        max_rows: usize,
        max_vm_steps: u64,
        max_decoded_bytes: u64,
    ) -> Result<ControlledPostingPage> {
        use crate::{MAX_POSTING_DELTA_BYTES, MAX_POSTINGS_PER_BLOCK, decode_posting_block};
        self.check_pin()?;
        if gram.chars().count() != 3
            || gram.len() > 12
            || max_rows == 0
            || max_rows > 1024
            || max_vm_steps == 0
            || (max_rows as u64).checked_mul(8).is_none_or(|bytes| bytes > max_decoded_bytes)
            || after.is_some_and(|position| position > i64::MAX as u64)
        {
            return Err(Error::Budget("controlled posting-page admission"));
        }
        let workspace = max_decoded_bytes
            .checked_add((max_rows as u64).checked_mul(16).ok_or(Error::Budget("posting result hold"))?)
            .and_then(|n| n.checked_add(MAX_POSTING_DELTA_BYTES as u64))
            .and_then(|n| usize::try_from(n).ok())
            .and_then(|n| n.checked_add(std::mem::size_of::<(
                &Self, ControlledSearchKind, &str, Option<u64>, usize, u64, u64,
                Vec<u64>, bool, u64, u64,
            )>()))
            .ok_or(Error::Budget("controlled posting-page workspace"))?;
        let _hold = self.context.owned_state().hold(workspace)?;
        self.context.check()?;
        let table_kind = match kind {
            ControlledSearchKind::Nodes => "nodes",
            ControlledSearchKind::Relations => "relations",
        };
        let sql = "SELECT CASE WHEN typeof(first_position)='integer' THEN first_position END, \
                          CASE WHEN typeof(last_position)='integer' THEN last_position END, \
                          CASE WHEN typeof(postings)='integer' THEN postings END, \
                          CASE WHEN typeof(deltas)='blob' THEN 1 ELSE 0 END, length(deltas), \
                          CASE WHEN typeof(deltas)='blob' AND length(deltas)<=?5 THEN deltas END \
                   FROM search_posting_blocks \
                   WHERE kind=?1 AND n=3 AND gram=?2 AND last_position>?3 \
                   ORDER BY last_position LIMIT ?4";
        let (page, vm_steps) =
            crate::knowledge_payload_read::with_query_vm_window(
                self.context,
                &self.connection,
                max_vm_steps,
                || {
                    let mut statement = self.connection.prepare_cached(sql)?;
                    let mut rows = statement.query(params![
                        table_kind,
                        gram.as_bytes(),
                        after.map_or(-1, |value| value as i64),
                        max_rows as i64,
                        max_decoded_bytes.min(MAX_POSTING_DELTA_BYTES as u64) as i64,
                    ])?;
                    let mut positions = Vec::with_capacity(max_rows);
                    let mut decoded_bytes = 0u64;
                    let mut previous_block_last = None;
                    let mut exhausted = true;
                    while let Some(row) = rows.next()? {
                        self.context.check()?;
                        let first: i64 = row.get::<_, Option<i64>>(0)?.ok_or(Error::Invalid("controlled posting first position"))?;
                        let last: i64 = row.get::<_, Option<i64>>(1)?.ok_or(Error::Invalid("controlled posting last position"))?;
                        let postings: i64 = row.get::<_, Option<i64>>(2)?.ok_or(Error::Invalid("controlled posting count"))?;
                        let delta_is_blob: i64 = row.get(3)?;
                        let delta_len: i64 = row.get::<_, Option<i64>>(4)?.ok_or(Error::Invalid("controlled posting length"))?;
                        if first < 0 || last < first || postings < 1
                            || postings > MAX_POSTINGS_PER_BLOCK as i64
                            || previous_block_last.is_some_and(|prior| first <= prior)
                            || after.is_some_and(|prior| last as u64 <= prior)
                            || delta_is_blob != 1 || delta_len < 0
                            || delta_len as u64 > MAX_POSTING_DELTA_BYTES as u64
                        {
                            return Err(Error::Invalid("controlled posting block shape"));
                        }
                        let block_bytes = 40u64
                            .checked_add(delta_len as u64)
                            .and_then(|n| n.checked_add((postings as u64).checked_mul(8)?))
                            .ok_or(Error::Budget("controlled posting block charge"))?;
                        decoded_bytes = decoded_bytes.checked_add(block_bytes)
                            .ok_or(Error::Budget("controlled posting byte charge"))?;
                        if decoded_bytes > max_decoded_bytes {
                            return Err(Error::Budget("controlled posting decoded bytes"));
                        }
                        self.context.charge_work(usize::try_from(block_bytes).map_err(|_| Error::Budget("controlled posting work"))?)?;
                        let deltas: Vec<u8> = row.get::<_, Option<Vec<u8>>>(5)?
                            .ok_or(Error::Invalid("controlled posting deltas"))?;
                        let block = decode_posting_block(first as u64, last as u64, postings as u16, &deltas)?;
                        previous_block_last = Some(last);
                        for position in block {
                            if after.is_some_and(|prior| position <= prior) { continue; }
                            positions.push(position);
                            if positions.len() == max_rows { exhausted = false; break; }
                        }
                        if !exhausted { break; }
                    }
                    Ok((positions, exhausted, decoded_bytes))
                },
            )?;
        let (positions, exhausted, decoded_bytes) = page;
        self.check_pin()?;
        let rows = positions.len() as u64;
        Ok(ControlledPostingPage { positions, exhausted, vm_steps, rows, decoded_bytes })
    }

    pub fn exact_candidate(
        &mut self,
        kind: ControlledSearchKind,
        position: u64,
        max_vm_steps: u64,
        max_decoded_bytes: u64,
        max_payload_bytes: usize,
        max_field_bytes: usize,
        max_document_chars: u64,
    ) -> Result<ControlledSearchCandidate> {
        self.check_pin()?;
        let field_floor = (max_field_bytes as u64)
            .checked_mul(8).ok_or(Error::Budget("controlled candidate field cap"))?;
        let row_floor = field_floor
            .checked_add((max_payload_bytes as u64).checked_mul(2).ok_or(Error::Budget("controlled candidate payload cap"))?)
            .and_then(|n| n.checked_add(256))
            .ok_or(Error::Budget("controlled candidate row cap"))?;
        if position > i64::MAX as u64 || max_vm_steps == 0 || max_payload_bytes == 0
            || max_field_bytes == 0 || max_document_chars == 0
            || max_decoded_bytes < row_floor
            || max_payload_bytes > i64::MAX as usize
            || max_field_bytes > i64::MAX as usize
            || max_document_chars > i64::MAX as u64
        {
            return Err(Error::Budget("controlled candidate admission"));
        }
        let source_cap = max_payload_bytes as i64;
        let field_cap = max_field_bytes as i64;
        let char_cap = max_document_chars as i64;
        let (table_kind, table) = match kind {
            ControlledSearchKind::Nodes => ("nodes", "knowledge_nodes"),
            ControlledSearchKind::Relations => ("relations", "knowledge_relations"),
        };
        if self.selection.model_abi != knowledge_stage::KNOWLEDGE_CARRIER_ONCE_MODEL_ABI {
            return Err(Error::Invalid("controlled search requires CarrierOnce model ABI"));
        }
        let layout = KnowledgePayloadLayout::CarrierOnceV1;
        let sql_inline = match kind {
            ControlledSearchKind::Nodes => "SELECT\
                 CASE WHEN typeof(d.id)='text' AND length(CAST(d.id AS BLOB))<=?3 THEN d.id END,\
                 CASE WHEN typeof(d.source_graph)='text' AND length(CAST(d.source_graph AS BLOB))<=?3 THEN d.source_graph END,\
                 CASE WHEN typeof(d.kind_id)='text' AND length(CAST(d.kind_id AS BLOB))<=?3 THEN d.kind_id END,\
                 CASE WHEN typeof(d.predicate_id)='text' AND length(CAST(d.predicate_id AS BLOB))<=?3 THEN d.predicate_id END,\
                 CASE WHEN typeof(d.id_lower)='text' AND length(CAST(d.id_lower AS BLOB))<=?3 THEN d.id_lower END,\
                 CASE WHEN typeof(d.native_id_lower)='text' AND length(CAST(d.native_id_lower AS BLOB))<=?3 THEN d.native_id_lower END,\
                 CASE WHEN typeof(d.identity_values)='text' AND length(CAST(d.identity_values AS BLOB))<=?3 THEN d.identity_values END,\
                 CASE WHEN typeof(d.visible_values)='text' AND length(CAST(d.visible_values AS BLOB))<=?3 THEN d.visible_values END,\
                 CASE WHEN typeof(d.document_chars)='integer' AND d.document_chars>=0 AND d.document_chars<=?5 THEN d.document_chars END,\
                 CASE WHEN typeof(d.document_digest)='blob' AND length(d.document_digest)=32 THEN d.document_digest END,\
                 CASE WHEN typeof(c.payload_len)='integer' AND c.payload_len>=0 AND c.payload_len<=?4 THEN c.payload_len END,\
                 CASE WHEN typeof(c.payload_sha256)='blob' AND length(c.payload_sha256)=32 THEN c.payload_sha256 END,\
                 CASE WHEN typeof(c.payload)='blob' AND c.payload_len>=0 AND c.payload_len<=?4 AND length(c.payload)=c.payload_len THEN c.payload END,\
                 0,NULL,NULL,NULL,NULL FROM search_documents d JOIN knowledge_nodes c ON c.source_order=d.position WHERE d.kind=?1 AND d.position=?2",
            ControlledSearchKind::Relations => "SELECT\
                 CASE WHEN typeof(d.id)='text' AND length(CAST(d.id AS BLOB))<=?3 THEN d.id END,\
                 CASE WHEN typeof(d.source_graph)='text' AND length(CAST(d.source_graph AS BLOB))<=?3 THEN d.source_graph END,\
                 CASE WHEN typeof(d.kind_id)='text' AND length(CAST(d.kind_id AS BLOB))<=?3 THEN d.kind_id END,CASE WHEN typeof(d.predicate_id)='text' AND length(CAST(d.predicate_id AS BLOB))<=?3 THEN d.predicate_id END,\
                 CASE WHEN typeof(d.id_lower)='text' AND length(CAST(d.id_lower AS BLOB))<=?3 THEN d.id_lower END,\
                 CASE WHEN typeof(d.native_id_lower)='text' AND length(CAST(d.native_id_lower AS BLOB))<=?3 THEN d.native_id_lower END,\
                 CASE WHEN typeof(d.identity_values)='text' AND length(CAST(d.identity_values AS BLOB))<=?3 THEN d.identity_values END,\
                 CASE WHEN typeof(d.visible_values)='text' AND length(CAST(d.visible_values AS BLOB))<=?3 THEN d.visible_values END,\
                 CASE WHEN typeof(d.document_chars)='integer' AND d.document_chars>=0 AND d.document_chars<=?5 THEN d.document_chars END,\
                 CASE WHEN typeof(d.document_digest)='blob' AND length(d.document_digest)=32 THEN d.document_digest END,\
                 CASE WHEN typeof(c.payload_len)='integer' AND c.payload_len>=0 AND c.payload_len<=?4 THEN c.payload_len END,\
                 CASE WHEN typeof(c.payload_sha256)='blob' AND length(c.payload_sha256)=32 THEN c.payload_sha256 END,\
                 CASE WHEN typeof(c.payload)='blob' AND c.payload_len>=0 AND c.payload_len<=?4 AND length(c.payload)=c.payload_len THEN c.payload END,\
                 0,NULL,NULL,NULL,NULL FROM search_documents d JOIN knowledge_relations c ON c.source_order=d.position WHERE d.kind=?1 AND d.position=?2",
        };
        let sql_carrier = match kind {
            ControlledSearchKind::Nodes => "SELECT\
                 CASE WHEN typeof(d.id)='text' AND length(CAST(d.id AS BLOB))<=?3 THEN d.id END,\
                 CASE WHEN typeof(d.source_graph)='text' AND length(CAST(d.source_graph AS BLOB))<=?3 THEN d.source_graph END,\
                 CASE WHEN typeof(d.kind_id)='text' AND length(CAST(d.kind_id AS BLOB))<=?3 THEN d.kind_id END,\
                 CASE WHEN typeof(d.predicate_id)='text' AND length(CAST(d.predicate_id AS BLOB))<=?3 THEN d.predicate_id END,\
                 CASE WHEN typeof(d.id_lower)='text' AND length(CAST(d.id_lower AS BLOB))<=?3 THEN d.id_lower END,\
                 CASE WHEN typeof(d.native_id_lower)='text' AND length(CAST(d.native_id_lower AS BLOB))<=?3 THEN d.native_id_lower END,\
                 CASE WHEN typeof(d.identity_values)='text' AND length(CAST(d.identity_values AS BLOB))<=?3 THEN d.identity_values END,\
                 CASE WHEN typeof(d.visible_values)='text' AND length(CAST(d.visible_values AS BLOB))<=?3 THEN d.visible_values END,\
                 CASE WHEN typeof(d.document_chars)='integer' AND d.document_chars>=0 AND d.document_chars<=?5 THEN d.document_chars END,\
                 CASE WHEN typeof(d.document_digest)='blob' AND length(d.document_digest)=32 THEN d.document_digest END,\
                 CASE WHEN typeof(c.payload_len)='integer' AND c.payload_len>=0 AND c.payload_len<=?4 THEN c.payload_len END,\
                 CASE WHEN typeof(c.payload_sha256)='blob' AND length(c.payload_sha256)=32 THEN c.payload_sha256 END,\
                 CASE WHEN typeof(c.payload)='blob' AND length(c.payload)<=?4 THEN c.payload END,\
                 CASE WHEN typeof(c.payload_codec)='integer' THEN c.payload_codec END,\
                 CASE WHEN typeof(c.source_packet_sha256)='blob' AND length(c.source_packet_sha256)=32 THEN c.source_packet_sha256 END,\
                 CASE WHEN typeof(s.packet_len)='integer' AND s.packet_len>=0 AND s.packet_len<=?4 THEN s.packet_len END,\
                 CASE WHEN typeof(s.packet_sha256)='blob' AND length(s.packet_sha256)=32 THEN s.packet_sha256 END,\
                 CASE WHEN typeof(s.packet)='blob' AND s.packet_len>=0 AND s.packet_len<=?4 AND length(s.packet)=s.packet_len THEN s.packet END\
                 FROM search_documents d JOIN knowledge_nodes c ON c.source_order=d.position\
                 LEFT JOIN knowledge_source_carriers s ON s.packet_sha256=c.source_packet_sha256\
                 WHERE d.kind=?1 AND d.position=?2",
            ControlledSearchKind::Relations => "SELECT\
                 CASE WHEN typeof(d.id)='text' AND length(CAST(d.id AS BLOB))<=?3 THEN d.id END,\
                 CASE WHEN typeof(d.source_graph)='text' AND length(CAST(d.source_graph AS BLOB))<=?3 THEN d.source_graph END,\
                 CASE WHEN typeof(d.kind_id)='text' AND length(CAST(d.kind_id AS BLOB))<=?3 THEN d.kind_id END,CASE WHEN typeof(d.predicate_id)='text' AND length(CAST(d.predicate_id AS BLOB))<=?3 THEN d.predicate_id END,\
                 CASE WHEN typeof(d.id_lower)='text' AND length(CAST(d.id_lower AS BLOB))<=?3 THEN d.id_lower END,\
                 CASE WHEN typeof(d.native_id_lower)='text' AND length(CAST(d.native_id_lower AS BLOB))<=?3 THEN d.native_id_lower END,\
                 CASE WHEN typeof(d.identity_values)='text' AND length(CAST(d.identity_values AS BLOB))<=?3 THEN d.identity_values END,\
                 CASE WHEN typeof(d.visible_values)='text' AND length(CAST(d.visible_values AS BLOB))<=?3 THEN d.visible_values END,\
                 CASE WHEN typeof(d.document_chars)='integer' AND d.document_chars>=0 AND d.document_chars<=?5 THEN d.document_chars END,\
                 CASE WHEN typeof(d.document_digest)='blob' AND length(d.document_digest)=32 THEN d.document_digest END,\
                 CASE WHEN typeof(c.payload_len)='integer' AND c.payload_len>=0 AND c.payload_len<=?4 THEN c.payload_len END,\
                 CASE WHEN typeof(c.payload_sha256)='blob' AND length(c.payload_sha256)=32 THEN c.payload_sha256 END,\
                 CASE WHEN typeof(c.payload)='blob' AND length(c.payload)<=?4 THEN c.payload END,\
                 CASE WHEN typeof(c.payload_codec)='integer' THEN c.payload_codec END,\
                 CASE WHEN typeof(c.source_packet_sha256)='blob' AND length(c.source_packet_sha256)=32 THEN c.source_packet_sha256 END,\
                 CASE WHEN typeof(s.packet_len)='integer' AND s.packet_len>=0 AND s.packet_len<=?4 THEN s.packet_len END,\
                 CASE WHEN typeof(s.packet_sha256)='blob' AND length(s.packet_sha256)=32 THEN s.packet_sha256 END,\
                 CASE WHEN typeof(s.packet)='blob' AND s.packet_len>=0 AND s.packet_len<=?4 AND length(s.packet)=s.packet_len THEN s.packet END\
                 FROM search_documents d JOIN knowledge_relations c ON c.source_order=d.position\
                 LEFT JOIN knowledge_source_carriers s ON s.packet_sha256=c.source_packet_sha256\
                 WHERE d.kind=?1 AND d.position=?2",
        };
        let sql = if layout == KnowledgePayloadLayout::CarrierOnceV1 { sql_carrier } else { sql_inline };
        let required_hold = usize::try_from(max_decoded_bytes)
            .map_err(|_| Error::Budget("controlled candidate workspace"))?
            .checked_add(std::mem::size_of::<ControlledSearchCandidate>())
            .and_then(|n| n.checked_add(field_floor as usize))
            .ok_or(Error::Budget("controlled candidate workspace"))?;
        let _hold = self.context.owned_state().hold(required_hold)?;
        // Admit the SQL-owned row, both possible payload carriers, and the
        // candidate-copy work before rusqlite transfers any dynamic column.
        self.context.charge_work(usize::try_from(row_floor).map_err(|_| Error::Budget("controlled candidate work"))?)?;
        let (row_result, vm_steps) = crate::knowledge_payload_read::with_query_vm_window(
            self.context,
            &self.connection,
            max_vm_steps,
            || {
                let result = self.connection.query_row(
                    sql,
                    params![table_kind, position as i64, field_cap, source_cap, char_cap],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, Option<String>>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, Option<String>>(6)?,
                            row.get::<_, Option<String>>(7)?,
                            row.get::<_, Option<i64>>(8)?,
                            row.get::<_, Option<Vec<u8>>>(9)?,
                            row.get::<_, Option<i64>>(10)?,
                            row.get::<_, Option<Vec<u8>>>(11)?,
                            row.get::<_, Option<Vec<u8>>>(12)?,
                            row.get::<_, Option<i64>>(13)?,
                            row.get::<_, Option<Vec<u8>>>(14)?,
                            row.get::<_, Option<i64>>(15)?,
                            row.get::<_, Option<Vec<u8>>>(16)?,
                            row.get::<_, Option<Vec<u8>>>(17)?,
                        ))
                    },
                ).optional()?;
                let Some(row) = result else { return Err(Error::Invalid("controlled posting candidate absent")); };
                Ok(row)
            },
        )?;
        let row = row_result;
        let (
            Some(id), Some(source_graph), Some(kind_id), Some(predicate_id),
            Some(id_lower), Some(native_id_lower), Some(identity_values), Some(visible_values),
            Some(document_chars), Some(document_digest), Some(logical_len), Some(payload_digest),
            Some(physical), Some(codec), source_digest, source_len, packet_digest, packet,
        ) = row else {
            return Err(Error::Budget("controlled candidate SQL pretransfer cap"));
        };
        if logical_len < 0 || document_chars < 0 || physical.is_empty()
            || physical.len() > max_payload_bytes
            || (source_digest.is_some() != packet.is_some())
            || (packet.is_some() != packet_digest.is_some())
            || (packet.is_some() != source_len.is_some())
        {
            return Err(Error::Invalid("controlled candidate row shape"));
        }
        let digest = |raw: &[u8]| -> Result<Digest256> {
            let bytes: [u8; 32] = raw.try_into().map_err(|_| Error::Invalid("controlled candidate digest width"))?;
            Ok(Digest256::from_bytes(bytes))
        };
        let document_digest = digest(&document_digest)?;
        let payload_sha256 = digest(&payload_digest)?;
        let source_packet_sha256 = source_digest.as_deref().map(digest).transpose()?;
        let source_packet_sha256_joined = packet_digest.as_deref().map(digest).transpose()?;
        if source_packet_sha256 != source_packet_sha256_joined
            || packet.as_ref().zip(source_len).is_some_and(|(packet, len)| len < 0 || packet.len() as i64 != len)
        {
            return Err(Error::Invalid("controlled candidate source packet join"));
        }
        let physical_len = physical.len();
        let physical_bytes = physical;
        let source_packet_bytes = packet;
        let payload = crate::knowledge_payload_read::with_logical_payload_for_verified_layout(
            self.context,
            layout,
            &SelectedPayloadRow {
                payload_codec: u8::try_from(codec).map_err(|_| Error::Invalid("controlled candidate codec"))?,
                physical: &physical_bytes,
                logical_len: usize::try_from(logical_len).map_err(|_| Error::Budget("controlled candidate logical length"))?,
                logical_sha256: payload_sha256,
                source_packet_sha256,
                source_packet: source_packet_bytes.as_deref(),
            },
            JsonLimits::default(),
            JsonLimits::default(),
            max_payload_bytes,
            |logical, context| {
                if logical.len() > max_payload_bytes {
                    return Err(Error::Budget("controlled logical candidate payload"));
                }
                context.charge_work(logical.len())?;
                Ok(logical.to_vec())
            },
        )?;
        if payload.len() != usize::try_from(logical_len).map_err(|_| Error::Budget("controlled candidate logical length"))?
            || physical_len > max_payload_bytes
        {
            return Err(Error::Invalid("controlled candidate logical payload length"));
        }
        let decoded_bytes = [
            id.len(), source_graph.len(), kind_id.len(), predicate_id.len(), id_lower.len(),
            native_id_lower.len(), identity_values.len(), visible_values.len(), document_digest.as_bytes().len(),
            payload_sha256.as_bytes().len(), payload.len(),
        ].into_iter().try_fold(16u64, |sum, size| sum.checked_add(size as u64))
            .ok_or(Error::Budget("controlled candidate decoded bytes"))?;
        if decoded_bytes > max_decoded_bytes {
            return Err(Error::Budget("controlled candidate decoded bytes"));
        }
        self.check_pin()?;
        Ok(ControlledSearchCandidate {
            position,
            id,
            source_graph,
            kind_id,
            predicate_id,
            id_lower,
            native_id_lower,
            identity_values,
            visible_values,
            document_chars: document_chars as u64,
            document_digest,
            payload_sha256,
            payload,
            vm_steps,
            rows: 1,
            decoded_bytes,
        })
    }
}

/// Incoming file/expectation/custody owners and once-process VFS state are
/// already admitted by the authentic Completed/Driver owner. Source/currentness
/// custody operations retain their separate original owner charging law.
pub(crate) fn with_controlled_selected_knowledge_model<'state, 'budget>(
    pinned: &mut File,
    expected: &KnowledgeSelectedExpectation,
    custody: &dyn ImmutableKnowledgeCustody,
    limits: ColdOpenLimits,
    issued_loan: &NativeSnapshotOwnedReadLoan<'state, 'budget>,
    context: &RuntimeKnowledgeReadContext<'state, 'budget>,
    consume: impl FnOnce(&mut ControlledKnowledgeModel<'_, 'state, 'budget>) -> Result<()>,
) -> Result<()> {
    let state = context.owned_state();
    let remaining = |additional: usize| {
        context.remaining_after_retained(additional).map_err(|_| {
            tos_source_store::StoreError::new(
                tos_source_store::StoreErrorCode::BudgetExceeded,
                "controlled cold connection state",
            )
        })
    };
    let fixed = std::mem::size_of::<(
        &mut File,
        &KnowledgeSelectedExpectation,
        &dyn ImmutableKnowledgeCustody,
        ColdOpenLimits,
        &NativeSnapshotOwnedReadLoan<'_, '_>,
        &RuntimeKnowledgeReadContext<'state, 'budget>,
        &crate::d1_public_capture::CreationState<'budget>,
        ControlledKnowledgeModel<'_, 'state, 'budget>,
        crate::d1_public_capture::ControlledCaptureIdentity,
        u64,
        crate::knowledge_payload_read::RuntimeKnowledgeSqlHook<'_, 'state, 'budget>,
        crate::d1_public_capture::CreationStateHold<'_, '_>,
        std::fs::Metadata,
        std::io::Result<std::fs::Metadata>,
        std::io::Result<()>,
        Result<()>,
        Result<()>,
        Result<bool>,
        Result<crate::d1_public_capture::ControlledCaptureIdentity>,
        tos_source_store::PinnedSqliteConnection,
        Result<tos_source_store::PinnedSqliteConnection>,
        std::result::Result<tos_source_store::PinnedSqliteConnection, tos_source_store::StoreError>,
        Result<crate::knowledge_payload_read::RuntimeKnowledgeSqlHook<'_, 'state, 'budget>>,
        Result<Option<crate::PhilosophyOriginalReceipt>>,
        Result<Option<crate::CorpusOriginalReceipt>>,
        Result<Option<crate::NavigationOriginalReceipt>>,
        Option<crate::PhilosophyOriginalReceipt>,
        Option<crate::CorpusOriginalReceipt>,
        Option<crate::NavigationOriginalReceipt>,
        Result<(Digest256, Digest256)>,
        Digest256,
        Digest256,
        knowledge_stage::KnowledgePayloadLayout,
        u64,
        u64,
        usize,
        Option<usize>,
        Result<usize>,
        Result<(KnowledgeSourceBasis, crate::NavigationOriginalReceipt, crate::PhilosophyOriginalReceipt, crate::CorpusOriginalReceipt)>,
        Result<(
            crate::knowledge_payload_read::RuntimeKnowledgeSqlHook<'_, 'state, 'budget>,
            (KnowledgeSourceBasis, crate::NavigationOriginalReceipt, crate::PhilosophyOriginalReceipt, crate::CorpusOriginalReceipt),
            u64,
            usize,
            usize,
            crate::d1_public_capture::CreationStateHold<'state, 'budget>,
        )>,
    )>()
    .checked_add(std::mem::size_of_val(&consume))
    .and_then(|n| n.checked_add(std::mem::size_of_val(&remaining)))
    .ok_or(Error::Budget("controlled cold model frame"))?;
    let _frame = state.hold(fixed)?;
    context.check()?;
    if expected.model_abi != knowledge_stage::KNOWLEDGE_CARRIER_ONCE_MODEL_ABI
        || expected.navigation_original_root_sha256.is_none()
        || expected.philosophy_original_root_sha256.is_none()
        || expected.corpus_original_root_sha256.is_none()
    {
        return Err(Error::Invalid("controlled Whole read requires CarrierOnce and all three Original roots"));
    }
    crate::knowledge_selected::validate(expected, limits)?;
    let identity = issued_loan.capture().controlled_identity(state)?;
    if !issued_loan.capture().matches_controlled_identity(&identity)? {
        return Err(Error::Invalid("controlled cold capture identity"));
    }
    let metadata = pinned.metadata()?;
    if !metadata.is_file() || metadata.len() != expected.model_size_bytes {
        return Err(Error::Invalid("knowledge selected held-file type or size"));
    }
    custody.verify(pinned, expected)?;
    custody.verify_cold_resources(limits)?;
    let size = crate::knowledge_selected::digest_selected_file_with_owned_context(
        pinned, expected, limits, context,
    )?;
    if size != expected.model_size_bytes {
        return Err(Error::Invalid("knowledge selected held-file size"));
    }
    pinned.rewind()?;
    context.check()?;
    let retained = tos_source_store::PinnedSqliteConnection::immutable_retained_rust_state_upper_bound()
        .checked_add(RuntimeKnowledgeReadContext::sql_callback_retained_state_bytes())
        .ok_or(Error::Budget("controlled cold connection state"))?;
    let connection_hold = state.hold(retained)?;
    let db = match tos_source_store::PinnedSqliteConnection::open_readonly_immutable_with_state(
        pinned, &remaining,
    ) {
        Ok(db) => db,
        Err(error) => {
            context.check()?;
            return Err(if error.code == tos_source_store::StoreErrorCode::BudgetExceeded {
                Error::Budget("controlled cold selected SQLite open")
            } else {
                Error::Invalid("controlled cold selected SQLite open")
            });
        }
    };
    let operation: Result<_> = (|| {
        let cold_hook = context.install_operation_sql_controller(&db, limits.max_vm_steps)?;
        // The cold verifier returns these exact typed roots/receipts into the
        // model callback. Reserve their maximum bounded owners before the
        // graph header and Original rows are decoded; per-helper parse holds
        // cover their temporary trees, this hold covers the escaping values.
        let escaping_basis = std::mem::size_of::<KnowledgeSourceBasis>()
            .checked_add(limits.max_row_bytes)
            .ok_or(Error::Budget("controlled source-basis retained bytes"))?;
        let navigation_receipt = std::mem::size_of::<crate::NavigationOriginalReceipt>()
            .checked_add(8 * 4096 + 4096)
            .ok_or(Error::Budget("controlled Navigation Original retained bytes"))?;
        let escaping_outputs = state.hold(
            escaping_basis
                .checked_add(navigation_receipt)
                .ok_or(Error::Budget("controlled cold output retained bytes"))?,
        )?;
        let open_vm_start = state.sql_vm_counter().load(std::sync::atomic::Ordering::Acquire);
        crate::knowledge_selected::configure_selected_sql_with_owned_context(&db, limits, context)?;
        let open_vm_steps = state
            .sql_vm_counter()
            .load(std::sync::atomic::Ordering::Acquire)
            .saturating_sub(open_vm_start);
        crate::knowledge_selected::verify_integrity_with_owned_context(&db, context)?;
        let layout = KnowledgePayloadLayout::CarrierOnceV1;
        crate::knowledge_selected::verify_schema_with_layout(&db, layout, Some(state))?;
        crate::knowledge_selected::check_native_metadata_with_owned_context(
            &db, expected, limits.max_metadata_bytes, context,
        )?;
        let (node_root, relation_root) = crate::knowledge_selected::verify_core_and_scope_with_owned_context(
            &db, expected, limits, layout, context,
        )?;
        crate::knowledge_selected::verify_search_with_owned_context(&db, expected, limits, context)?;
        let verified = crate::knowledge_selected::verify_native_graph_root_with_owned_context(
            &db,
            expected,
            limits,
            node_root,
            relation_root,
            context,
            |source_basis| {
                crate::knowledge_selected::verify_catalog_with_owned_context(&db, expected, limits, context)?;
                let mut work = 0u64;
                let philosophy_original = crate::knowledge_philosophy_original::verify_with_owned_state(
                    &db, expected, limits, &mut work, Some(state),
                )?;
                let corpus_original = crate::knowledge_corpus_original::verify_with_owned_state(
                    &db, expected, limits, &mut work, Some(state),
                )?;
                let navigation_original = crate::knowledge_navigation_original::verify_with_owned_state(
                    &db, expected, limits, context,
                )?;
                let (Some(navigation_original), Some(philosophy_original), Some(corpus_original)) =
                    (navigation_original, philosophy_original, corpus_original)
                else {
                    return Err(Error::Invalid("controlled model requires all three Original receipts"));
                };
                Ok((source_basis.clone(), navigation_original, philosophy_original, corpus_original))
            },
        )?;
        // Return from the verifier only after its statement/parser owners have
        // dropped; the transition then ends the ColdOpenLimits delta without
        // changing the shared counters or operation cutoff.
        Ok((
            cold_hook,
            verified,
            open_vm_steps,
            escaping_basis,
            navigation_receipt,
            escaping_outputs,
        ))
    })();
    let operation = match operation {
        Err(error) => Err(error),
        Ok((cold_hook, verified, open_vm_steps, escaping_basis, navigation_receipt, _escaping_outputs)) => {
            let post_cold = (|| {
                context.check()?;
                custody.verify_cold_resources(limits)?;
                custody.verify(pinned, expected)?;
                if !issued_loan.capture().matches_controlled_identity(&identity)? {
                    return Err(Error::Invalid("controlled cold capture identity changed"));
                }
                drop(cold_hook);
                issued_loan.capture().finish_owned_operation_phase_limits()?;
                let query_hook = context.install_operation_sql_controller(&db, state.sql_vm_limit())?;
                let (source_basis, navigation_original, philosophy_original, corpus_original) = verified;
                if source_basis.owned_heap_bytes()
                    .map_err(|_| Error::Budget("controlled source-basis retained state"))? > escaping_basis
                    || navigation_original_retained_bytes(&navigation_original)? > navigation_receipt
                {
                    return Err(Error::Budget("controlled cold receipt retained bytes"));
                }
        let mut model = ControlledKnowledgeModel {
            connection: &db,
            pinned,
            selection: expected,
            custody,
            source_basis: &source_basis,
            navigation_original: Some(&navigation_original),
            philosophy_original: Some(&philosophy_original),
            corpus_original: Some(&corpus_original),
            identity: &identity,
            open_vm_steps,
            context,
        };
        let query = consume(&mut model);
        drop(model);
        context.check()?;
        custody.verify_cold_resources(limits)?;
        custody.verify(pinned, expected)?;
        if !issued_loan.capture().matches_controlled_identity(&identity)? {
            return Err(Error::Invalid("controlled cold final capture identity"));
        }
        drop(query_hook);
        query
            })();
            post_cold
        }
    };
    // Statements, model borrows and both SQL callbacks are gone before close.
    let close = db.close();
    drop(connection_hold);
    match close {
        Ok(()) => operation,
        Err((still_open, close)) => {
            drop(still_open);
            match operation {
                Ok(()) => Err(Error::ControlledColdClose { operation: None, close }),
                Err(operation) => Err(Error::ControlledColdClose {
                    operation: Some(crate::ColdOperationFailure::from(operation)),
                    close,
                }),
            }
        }
    }
}
