mod ordinary_lens;
pub use ordinary_lens::{ControlledLensKeySelection, ControlledLensKeyRow};
mod ordinary_sidecar;
pub use ordinary_sidecar::{ControlledSidecarModel, SearchSidecarAdmissionError};
use std::{fs::File, io::Seek};

use rusqlite::{OptionalExtension, params};
use tos_foundation::{CanonicalProfile, Digest256, JsonLimits, JsonString, JsonValue, OwnedState};

use crate::{
    ColdOpenLimits, Error, ImmutableKnowledgeCustody, KnowledgeSelectedExpectation,
    KnowledgeSourceBasis, NativeSnapshotOwnedReadLoan, QueryVocabulary, Result,
    knowledge_payload_read::{RuntimeKnowledgeReadContext, SelectedPayloadRow},
    knowledge_stage::{self, KnowledgePayloadLayout},
};

/// Compiler-owned selectors over exact normalized carrier rows. These cannot
/// supply a SQL expression, path, connection, state, or source authority.
#[derive(Clone, Copy)]
pub enum ControlledCarrierSelection<'a> {
    AllSources,
    Id { identifier: &'a str, limit: usize },
    NativeId { identifier: &'a str, limit: usize },
    EntityId { identifier: &'a str, limit: usize },
    Incident { ids_json: &'a str, limit: usize },
}

/// Exact Original namespace; collection names come only from owner enums.
#[derive(Clone, Copy)]
pub enum ControlledOriginalCollection {
    Navigation,
    Philosophy(crate::PhilosophyOriginalCollection),
    Corpus(crate::CorpusOriginalCollection),
}

/// Actual receipts borrowed from the authenticated cold owner. A consumer
/// authorizes the exact row against this receipt before any disclosure.
#[derive(Clone, Copy)]
pub enum ControlledOriginalReceipt<'a> {
    Navigation(&'a crate::NavigationOriginalReceipt),
    Philosophy(&'a crate::PhilosophyOriginalReceipt),
    Corpus(&'a crate::CorpusOriginalReceipt),
}

/// The borrowed callback is invoked at most once; no Original payload escapes
/// its admitted raw/parser holds. VM steps include both metadata and row reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct ControlledOriginalRowRead {
    pub ordinal: Option<i64>,
    pub decoded_bytes: u64,
    pub vm_steps: u64,
}

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

/// Dynamic, same-original-state holds for query-owned per-row retention.
/// Each node admits itself and the exact caller-reported heap retained with it.
struct ControlledQueryHoldNode<'state, 'budget> {
    _hold: crate::d1_public_capture::CreationStateHold<'state, 'budget>,
    next: Option<Box<ControlledQueryHoldNode<'state, 'budget>>>,
}

/// Bounded retained query state. It exposes only exact additional holds; the
/// original counters, deadline and aggregate ceilings remain private.
pub struct ControlledQueryHeap<'context, 'state, 'budget> {
    context: &'context RuntimeKnowledgeReadContext<'state, 'budget>,
    state: &'state crate::d1_public_capture::CreationState<'budget>,
    holds: Option<Box<ControlledQueryHoldNode<'state, 'budget>>>,
}
impl ControlledQueryHeap<'_, '_, '_> {
    /// The same original context remains live while a borrowed pure plan runs.
    pub fn check(&self) -> Result<()> { self.context.check() }
    pub fn charge_work(&self, units: usize) -> Result<()> {
        self.context.check()?;
        self.context.charge_work(units)?;
        self.context.check()
    }

    /// Parse while lending this same-state hold owner for any derived copies.
    /// The original parser/tree never escapes; persistent copies require retain.
    pub fn with_owned_query_json<T>(&mut self, raw: &[u8], limits: JsonLimits,
        operation: impl FnOnce(&JsonValue, &mut Self) -> Result<T>) -> Result<T> {
        let context = self.context;
        context.with_foundation_owned_with_limits(raw, limits,
            |value| operation(value, self))
    }

    /// Retain a caller-owned allocation until this hold set is dropped.
    pub fn retain(&mut self, bytes: usize) -> Result<()> {
        let total = bytes
            .checked_add(std::mem::size_of::<ControlledQueryHoldNode<'_, '_>>())
            .ok_or(Error::Budget("controlled query retained heap"))?;
        let hold = self.state.hold(total)?;
        self.holds = Some(Box::new(ControlledQueryHoldNode {
            _hold: hold,
            next: self.holds.take(),
        }));
        Ok(())
    }

    /// Admit one short-lived allocation around a synchronous callback.
    pub fn with_temporary<T>(
        &self,
        bytes: usize,
        operation: impl FnOnce() -> T,
    ) -> Result<T> {
        let _hold = self.state.hold(bytes)?;
        Ok(operation())
    }

    /// Maintained resource text renderer under the same original state,
    /// JSON visits, work and interruption controller as selected parsing.
    /// A returned buffer requires caller persistent admission before use.
    pub fn emit_python_pretty_owned_json(&self, value: &JsonValue,
        mut limits: JsonLimits) -> Result<Vec<u8>> {
        self.context.check()?;
        limits.max_visits = limits.max_visits.min(self.context.remaining_json_visits()?);
        if limits.max_visits == 0 { return Err(Error::Budget("resource renderer visits")); }
        let mut check = || self.context.check().map_err(|_| tos_foundation::FoundationError::new(
            tos_foundation::FoundationErrorCode::BudgetExceeded, "resource original interruption"));
        let mut admit = |bytes: usize, visits: usize| {
            let work = bytes.checked_mul(2).and_then(|n| visits.checked_mul(2)
                .and_then(|v| n.checked_add(v))).ok_or_else(|| tos_foundation::FoundationError::new(
                    tos_foundation::FoundationErrorCode::BudgetExceeded, "resource work overflow"))?;
            self.context.charge_work(work).map_err(|_| tos_foundation::FoundationError::new(
                tos_foundation::FoundationErrorCode::BudgetExceeded, "resource original work"))
        };
        let emitted = tos_foundation::emit_python_pretty_sorted_json_with_state_budget(
            value, limits, self.context.remaining_after_retained(0)?, &mut check, &mut admit);
        let (bytes, visits) = match emitted {
            Ok(result) => result,
            Err(_) => {
                // Failed traversal never restores visits already spent. The
                // writer does not expose partial progress, so consume its
                // admitted original visit ceiling on failure.
                self.context.debit_json_visits(limits.max_visits)?;
                return Err(Error::Budget("resource original renderer state"));
            }
        };
        self.context.debit_json_visits(visits)?;
        self.context.check()?;
        Ok(bytes)
    }

    /// Canonicalize a retained search item under the same original visit,
    /// work, and state ledgers as carrier parsing and response emission.
    pub fn canonicalize_owned_query_json(
        &self,
        value: &JsonValue,
        limits: JsonLimits,
    ) -> Result<Vec<u8>> {
        self.context.canonicalize_foundation_owned_with_limits(
            value,
            CanonicalProfile::SourceRecordDigestV1,
            limits,
        )
    }
}
impl Drop for ControlledQueryHeap<'_, '_, '_> {
    fn drop(&mut self) {
        // Avoid recursively dropping an arbitrarily long row-hold chain.
        let mut next = self.holds.take();
        while let Some(mut node) = next {
            next = node.next.take();
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ControlledLegacySearchScan {
    pub rows: u64,
    pub decoded_bytes: u64,
    pub vm_steps: u64,
}

impl ControlledKnowledgeModel<'_, '_, '_> {
    /// Lend the next exact retained Original row under the existing capture,
    /// receipt, State, Work and VM owners. The callback must retain any derived
    /// allocation in the caller's same-state ControlledQueryHeap.
    pub fn with_controlled_original_row(
        &mut self, collection: ControlledOriginalCollection, after_ordinal: i64,
        max_row_bytes: usize, max_decoded_bytes: u64, max_vm_steps: u64,
        json: JsonLimits,
        consume: impl FnOnce(i64, Digest256, &[u8], &JsonValue) -> Result<()>,
    ) -> Result<ControlledOriginalRowRead> {
        self.with_controlled_original_row_receipt(collection, after_ordinal,
            max_row_bytes, max_decoded_bytes, max_vm_steps, json,
            |_, ordinal, digest, raw, value| consume(ordinal, digest, raw, value))
    }

    pub fn with_controlled_original_row_receipt(
        &mut self,
        collection: ControlledOriginalCollection,
        after_ordinal: i64,
        max_row_bytes: usize,
        max_decoded_bytes: u64,
        max_vm_steps: u64,
        json: JsonLimits,
        consume: impl FnOnce(ControlledOriginalReceipt<'_>, i64, Digest256, &[u8], &JsonValue) -> Result<()>,
    ) -> Result<ControlledOriginalRowRead> {
        self.check_pin()?;
        if max_row_bytes == 0 || max_row_bytes > i64::MAX as usize
            || max_vm_steps == 0 || after_ordinal < -2 {
            return Err(Error::Budget("controlled Original row admission"));
        }
        if self.selection.model_abi != knowledge_stage::KNOWLEDGE_CARRIER_ONCE_MODEL_ABI {
            return Err(Error::Invalid("controlled Original requires CarrierOnce ABI"));
        }
        let receipt = match collection {
            ControlledOriginalCollection::Navigation => ControlledOriginalReceipt::Navigation(
                self.navigation_original.ok_or(Error::Invalid("navigation Original receipt absent"))?),
            ControlledOriginalCollection::Philosophy(_) => ControlledOriginalReceipt::Philosophy(
                self.philosophy_original.ok_or(Error::Invalid("philosophy Original receipt absent"))?),
            ControlledOriginalCollection::Corpus(_) => ControlledOriginalReceipt::Corpus(
                self.corpus_original.ok_or(Error::Invalid("corpus Original receipt absent"))?),
        };
        let (collection_name, metadata_sql, payload_sql) = match collection {
            ControlledOriginalCollection::Navigation => {
                if self.navigation_original.is_none() {
                    return Err(Error::Invalid("controlled navigation Original receipt absent"));
                }
                ("", "SELECT ordinal,packet_len FROM navigation_original_rows WHERE ordinal>?1 ORDER BY ordinal LIMIT 1",
                 "SELECT packet_sha256,packet FROM navigation_original_rows WHERE ordinal=?1 AND ?2='' AND packet_len=?3 AND typeof(packet_sha256)='blob' AND length(packet_sha256)=32 AND typeof(packet)='blob' AND length(packet)=?3")
            }
            ControlledOriginalCollection::Philosophy(selected) => {
                if self.philosophy_original.is_none() || after_ordinal < -1 {
                    return Err(Error::Invalid("controlled philosophy Original receipt or ordinal"));
                }
                (selected.as_str(), "SELECT ordinal,packet_len FROM philosophy_original_rows WHERE ordinal>?1 AND collection=?2 ORDER BY ordinal LIMIT 1",
                 "SELECT packet_sha256,packet FROM philosophy_original_rows LEFT JOIN knowledge_source_carriers USING(packet_sha256,packet_len) WHERE ordinal=?1 AND collection=?2 AND packet_len=?3 AND typeof(packet_sha256)='blob' AND length(packet_sha256)=32 AND typeof(packet)='blob' AND length(packet)=?3")
            }
            ControlledOriginalCollection::Corpus(selected) => {
                if self.corpus_original.is_none() || after_ordinal < -1 {
                    return Err(Error::Invalid("controlled corpus Original receipt or ordinal"));
                }
                (selected.as_str(), "SELECT ordinal,packet_len FROM corpus_original_rows WHERE ordinal>?1 AND collection=?2 ORDER BY ordinal LIMIT 1",
                 "SELECT packet_sha256,packet FROM corpus_original_rows LEFT JOIN knowledge_source_carriers USING(packet_sha256,packet_len) WHERE ordinal=?1 AND collection=?2 AND packet_len=?3 AND typeof(packet_sha256)='blob' AND length(packet_sha256)=32 AND typeof(packet)='blob' AND length(packet)=?3")
            }
        };
        let fixed = tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()
            .checked_add(std::mem::size_of_val(&consume))
            .and_then(|n| n.checked_add(std::mem::size_of::<(i64, i64, Vec<u8>, Vec<u8>)>()))
            .ok_or(Error::Budget("controlled Original frame"))?;
        let _frame = self.context.owned_state().hold(fixed)?;
        self.charge_query_work(metadata_sql.len().checked_add(payload_sql.len())
            .ok_or(Error::Budget("controlled Original SQL work"))?)?;
        let (mut result, vm_steps) = crate::knowledge_payload_read::with_query_vm_window(
            self.context, &self.connection, max_vm_steps, || {
                let next = match collection {
                    ControlledOriginalCollection::Navigation => self.connection.query_row(
                        metadata_sql, [after_ordinal], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))),
                    _ => self.connection.query_row(metadata_sql, params![after_ordinal, collection_name],
                        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))),
                }.optional()?;
                let Some((ordinal, length)) = next else {
                    return Ok(ControlledOriginalRowRead::default());
                };
                let length_usize = usize::try_from(length)
                    .map_err(|_| Error::Invalid("controlled Original row length"))?;
                let decoded = u64::try_from(length_usize).ok().and_then(|n| n.checked_add(32))
                    .ok_or(Error::Budget("controlled Original decoded bytes"))?;
                if length_usize == 0 || length_usize > max_row_bytes || decoded > max_decoded_bytes {
                    return Err(Error::Budget("controlled Original row bytes"));
                }
                let allocation = length_usize.checked_add(32)
                    .ok_or(Error::Budget("controlled Original raw state"))?;
                let _raw_hold = self.context.owned_state().hold(allocation)?;
                let (digest, raw): (Vec<u8>, Vec<u8>) = self.connection.query_row(
                    payload_sql, params![ordinal, collection_name, length],
                    |row| Ok((row.get(0)?, row.get(1)?)))?;
                self.charge_query_work(raw.len())?;
                let computed = Digest256::of_bytes(&raw);
                if raw.len() != length_usize || computed.as_bytes() != digest.as_slice() {
                    return Err(Error::Invalid("controlled Original row digest"));
                }
                self.check_pin()?;
                self.context.with_foundation_owned_with_limits(&raw, json,
                    |value| consume(receipt, ordinal, computed, &raw, value))?;
                self.check_pin()?;
                Ok(ControlledOriginalRowRead { ordinal: Some(ordinal), decoded_bytes: decoded, vm_steps: 0 })
            })?;
        result.vm_steps = vm_steps;
        self.check_pin()?;
        Ok(result)
    }

    /// Lend one descriptor-keyed catalog packet with its selected digest and
    /// parsed identity held inside the original cold state.
    pub fn with_controlled_catalog(
        &mut self, max_packet_bytes: usize, max_decoded_bytes: usize,
        max_vm_steps: u64, json: JsonLimits,
        consume: impl FnOnce(&[u8], &JsonValue) -> Result<()>,
    ) -> Result<()> {
        self.with_controlled_catalog_scan(max_packet_bytes, max_decoded_bytes,
            max_vm_steps, json, consume).map(|_| ())
    }

    pub fn with_controlled_catalog_scan(
        &mut self, max_packet_bytes: usize, max_decoded_bytes: usize,
        max_vm_steps: u64, json: JsonLimits,
        consume: impl FnOnce(&[u8], &JsonValue) -> Result<()>,
    ) -> Result<ControlledLegacySearchScan> {
        self.check_pin()?;
        if max_packet_bytes == 0 || max_packet_bytes > i64::MAX as usize
            || max_decoded_bytes < 32 || max_vm_steps == 0 {
            return Err(Error::Budget("controlled catalog admission"));
        }
        let cap = max_packet_bytes.min(max_decoded_bytes - 32);
        let forecast = cap.checked_add(32 + 64)
            .and_then(|n| n.checked_add(std::mem::size_of_val(&consume)))
            .and_then(|n| n.checked_add(std::mem::size_of::<(String, Vec<u8>, Vec<u8>, i64)>()))
            .and_then(|n| n.checked_add(tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()))
            .ok_or(Error::Budget("controlled catalog workspace"))?;
        let _hold = self.context.owned_state().hold(forecast)?;
        self.context.charge_work(64)?;
        let descriptor = self.selection.vocabulary.descriptor_sha256.to_hex();
        let (row, vm_steps) = crate::knowledge_payload_read::with_query_vm_window(
            self.context, &self.connection, max_vm_steps, || {
                self.connection.query_row(
                    "SELECT packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 END,CASE WHEN typeof(packet)='blob' AND packet_len BETWEEN 1 AND ?2 AND length(packet)=packet_len THEN packet END FROM catalog_index_meta WHERE descriptor_sha256=?1",
                    params![descriptor, cap as i64], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, Option<Vec<u8>>>(1)?,
                            row.get::<_, Option<Vec<u8>>>(2)?))
                    }).map_err(Error::from)
            })?;
        let (length, digest, payload) = row;
        let payload = payload.ok_or(Error::Invalid("controlled catalog payload shape"))?;
        let digest = digest.ok_or(Error::Invalid("controlled catalog digest shape"))?;
        self.context.charge_work(payload.len())?;
        let computed = Digest256::of_bytes(&payload);
        if length <= 0 || payload.len() != length as usize
            || computed.as_bytes() != digest.as_slice()
            || computed != self.selection.catalog_packet_sha256 {
            return Err(Error::Invalid("controlled catalog digest differs"));
        }
        self.check_pin()?;
        self.context.with_foundation_owned_with_limits(&payload, json,
            |catalog| consume(&payload, catalog))?;
        self.check_pin()?;
        Ok(ControlledLegacySearchScan { rows: 1, decoded_bytes: (payload.len() as u64)
            .checked_add(32).ok_or(Error::Budget("controlled catalog decoded count"))?, vm_steps })
    }

    /// Deliver the digest-bound normalized header under the original cold state.
    /// Unit callback keeps both the decoded payload and parser hold in this owner.
    pub fn with_controlled_header(
        &mut self, max_payload_bytes: usize, max_decoded_bytes: u64,
        max_vm_steps: u64, json: JsonLimits,
        consume: impl FnOnce(&[u8]) -> Result<()>,
    ) -> Result<()> {
        self.with_controlled_header_scan(max_payload_bytes, max_decoded_bytes,
            max_vm_steps, json, consume).map(|_| ())
    }

    pub fn with_controlled_header_scan(
        &mut self, max_payload_bytes: usize, max_decoded_bytes: u64,
        max_vm_steps: u64, json: JsonLimits,
        consume: impl FnOnce(&[u8]) -> Result<()>,
    ) -> Result<ControlledLegacySearchScan> {
        self.check_pin()?;
        if max_payload_bytes == 0 || max_payload_bytes > i64::MAX as usize
            || max_decoded_bytes < 40 || max_vm_steps == 0 {
            return Err(Error::Budget("controlled header admission"));
        }
        let cap = max_payload_bytes.min(usize::try_from(max_decoded_bytes - 40)
            .unwrap_or(usize::MAX));
        let forecast = cap.checked_add(32)
            .and_then(|n| n.checked_add(std::mem::size_of_val(&consume)))
            .and_then(|n| n.checked_add(tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()))
            .ok_or(Error::Budget("controlled header workspace"))?;
        let _hold = self.context.owned_state().hold(forecast)?;
        let (payload, vm_steps) = crate::knowledge_payload_read::with_query_vm_window(
            self.context, &self.connection, max_vm_steps, || {
                self.connection.query_row(
                    "SELECT packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 END,CASE WHEN typeof(packet)='blob' AND packet_len BETWEEN 1 AND ?1 AND length(packet)=packet_len THEN packet END FROM graph_header WHERE singleton=1",
                    [cap as i64], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, Option<Vec<u8>>>(1)?,
                            row.get::<_, Option<Vec<u8>>>(2)?))
                    }).map_err(Error::from)
            })?;
        let (length, digest, payload) = payload;
        let payload = payload.ok_or(Error::Invalid("controlled header payload shape"))?;
        let digest = digest.ok_or(Error::Invalid("controlled header digest shape"))?;
        self.context.charge_work(payload.len())?;
        if length <= 0 || payload.len() != length as usize
            || Digest256::of_bytes(&payload).as_bytes() != digest.as_slice() {
            return Err(Error::Invalid("controlled header digest differs"));
        }
        self.check_pin()?;
        self.context.with_foundation_owned_with_limits(&payload, json, |header| {
            if header.as_object().is_none() {
                return Err(Error::Invalid("controlled header object absent"));
            }
            consume(&payload)
        })?;
        self.check_pin()?;
        Ok(ControlledLegacySearchScan { rows: 1,
            decoded_bytes: payload.len() as u64 + 32, vm_steps })
    }

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

/// Same-state parser access for bounded selected response components.
impl<'model, 'state, 'budget> ControlledKnowledgeModel<'model, 'state, 'budget> {
    pub fn new_owned_query_heap(&self) -> ControlledQueryHeap<'model, 'state, 'budget> {
        ControlledQueryHeap {
            context: self.context,
            state: self.context.owned_state(),
            holds: None,
        }
    }

    pub fn with_owned_query_json<T>(
        &self,
        raw: &[u8],
        limits: JsonLimits,
        operation: impl FnOnce(&JsonValue) -> Result<T>,
    ) -> Result<T> {
        self.check_pin()?;
        self.context
            .with_foundation_owned_with_limits(raw, limits, operation)
    }

    pub fn canonicalize_owned_query_json(
        &self,
        value: &JsonValue,
        limits: JsonLimits,
    ) -> Result<Vec<u8>> {
        self.check_pin()?;
        let bytes = self.context.canonicalize_foundation_owned_with_limits(
            value,
            CanonicalProfile::SourceRecordDigestV1,
            limits,
        )?;
        self.check_pin()?;
        Ok(bytes)
    }

    pub fn controlled_incident_count(&mut self, ids_json: &str, max_vm_steps: u64)
        -> Result<(u64, u64)> {
        self.check_pin()?;
        self.charge_query_work(ids_json.len())?;
        let fixed = tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()
            .checked_add(std::mem::size_of::<(&str, i64, u64)>())
            .ok_or(Error::Budget("controlled incident count state"))?;
        let _hold = self.context.owned_state().hold(fixed)?;
        let (count, vm) = crate::knowledge_payload_read::with_query_vm_window(
            self.context, &self.connection, max_vm_steps, || {
                self.connection.query_row("SELECT count(*) FROM knowledge_relations WHERE from_id IN (SELECT value FROM json_each(?1)) OR to_id IN (SELECT value FROM json_each(?1))",
                    [ids_json], |row| row.get::<_, i64>(0)).map_err(Error::from)
            })?;
        self.check_pin()?;
        Ok((u64::try_from(count).map_err(|_| Error::Invalid("controlled incident count"))?, vm))
    }

    /// Visit exact normalized carrier rows in source/position order. No
    /// indexed search-document representation participates.
    pub fn visit_controlled_legacy_search_rows<E>(
        &mut self,
        kind: ControlledSearchKind,
        sources: &[String],
        max_rows: u64,
        max_decoded_bytes: u64,
        max_vm_steps: u64,
        max_payload_bytes: usize,
        max_field_bytes: usize,
        json_limits: JsonLimits,
        heap: &mut ControlledQueryHeap<'model, 'state, 'budget>,
        consume: impl FnMut(
            &str,
            u64,
            Digest256,
            &JsonValue,
            &mut dyn FnMut(usize) -> Result<()>,
            &mut ControlledQueryHeap<'model, 'state, 'budget>,
        ) -> std::result::Result<(), E>,
    ) -> Result<std::result::Result<ControlledLegacySearchScan, E>> {
        self.visit_controlled_carrier_rows(kind, sources, ControlledCarrierSelection::AllSources,
            max_rows, max_decoded_bytes, max_vm_steps, max_payload_bytes,
            max_field_bytes, json_limits, heap, consume)
    }

    pub fn visit_controlled_carrier_rows<E>(
        &mut self,
        kind: ControlledSearchKind,
        sources: &[String],
        selected: ControlledCarrierSelection<'_>,
        max_rows: u64,
        max_decoded_bytes: u64,
        max_vm_steps: u64,
        max_payload_bytes: usize,
        max_field_bytes: usize,
        json_limits: JsonLimits,
        heap: &mut ControlledQueryHeap<'model, 'state, 'budget>,
        mut consume: impl FnMut(
            &str,
            u64,
            Digest256,
            &JsonValue,
            &mut dyn FnMut(usize) -> Result<()>,
            &mut ControlledQueryHeap<'model, 'state, 'budget>,
        ) -> std::result::Result<(), E>,
    ) -> Result<std::result::Result<ControlledLegacySearchScan, E>> {
        self.visit_controlled_carrier_rows_with_size(kind, sources, selected,
            max_rows, max_decoded_bytes, max_vm_steps, max_payload_bytes,
            max_field_bytes, json_limits, heap,
            |source, position, digest, _, value, charge, heap|
                consume(source, position, digest, value, charge, heap))
    }

    pub fn visit_controlled_carrier_rows_with_size<E>(
        &mut self,
        kind: ControlledSearchKind,
        sources: &[String],
        selected: ControlledCarrierSelection<'_>,
        max_rows: u64,
        max_decoded_bytes: u64,
        max_vm_steps: u64,
        max_payload_bytes: usize,
        max_field_bytes: usize,
        json_limits: JsonLimits,
        heap: &mut ControlledQueryHeap<'model, 'state, 'budget>,
        mut consume: impl FnMut(
            &str,
            u64,
            Digest256,
            usize,
            &JsonValue,
            &mut dyn FnMut(usize) -> Result<()>,
            &mut ControlledQueryHeap<'model, 'state, 'budget>,
        ) -> std::result::Result<(), E>,
    ) -> Result<std::result::Result<ControlledLegacySearchScan, E>> {
        self.check_pin()?;
        if self.selection.model_abi != knowledge_stage::KNOWLEDGE_CARRIER_ONCE_MODEL_ABI {
            return Err(Error::Invalid("controlled legacy search requires CarrierOnce ABI"));
        }
        if max_payload_bytes == 0
            || max_field_bytes == 0
            || max_payload_bytes > i64::MAX as usize
            || max_field_bytes > i64::MAX as usize
            || max_rows > i64::MAX as u64
            || max_vm_steps == 0
            || sources.iter().any(|source| source.is_empty() || source.len() > max_field_bytes)
            || sources.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(Error::Budget("controlled legacy source-scan admission"));
        }
        if sources.is_empty() {
            return Ok(Ok(ControlledLegacySearchScan::default()));
        }
        let source_bytes = sources.iter().try_fold(0usize, |sum, source| {
            sum.checked_add(source.len())
                .ok_or(Error::Budget("controlled legacy source filter"))
        })?;
        self.charge_query_work(source_bytes)?;
        let source_filter = JsonValue::Array(
            sources
                .iter()
                .map(|source| JsonValue::String(JsonString::from_utf8(source)))
                .collect(),
        );
        let mut source_limits = JsonLimits::default();
        source_limits.max_bytes = source_bytes
            .checked_mul(6)
            .and_then(|n| n.checked_add(sources.len().saturating_mul(2)))
            .and_then(|n| n.checked_add(2))
            .ok_or(Error::Budget("controlled legacy source filter"))?;
        let encoded_sources =
            self.canonicalize_owned_query_json(&source_filter, source_limits)?;

        let (selector, needle, limit, order) = match selected {
            ControlledCarrierSelection::AllSources => (0i64, "", i64::MAX, "c.source_graph,c.source_order,c.id"),
            ControlledCarrierSelection::Id { identifier, limit } => (1, identifier, i64::try_from(limit).map_err(|_| Error::Budget("controlled carrier lookup limit"))?, "c.source_order,c.id"),
            ControlledCarrierSelection::NativeId { identifier, limit } => (2, identifier, i64::try_from(limit).map_err(|_| Error::Budget("controlled carrier lookup limit"))?, "c.source_order,c.id"),
            ControlledCarrierSelection::EntityId { identifier, limit } if kind == ControlledSearchKind::Nodes => (3, identifier, i64::try_from(limit).map_err(|_| Error::Budget("controlled carrier lookup limit"))?, "c.source_order,c.id"),
            ControlledCarrierSelection::Incident { ids_json, limit } if kind == ControlledSearchKind::Relations => (4, ids_json, i64::try_from(limit).map_err(|_| Error::Budget("controlled carrier lookup limit"))?, "c.id"),
            _ => return Err(Error::Invalid("controlled carrier selector family")),
        };
        if limit < 0 || (selector != 0 && needle.is_empty())
            || (selector != 4 && needle.len() > max_field_bytes)
            || (selector == 4 && needle.len() > json_limits.max_bytes) {
            return Err(Error::Budget("controlled carrier selector admission"));
        }
        let sql = match kind {
            ControlledSearchKind::Nodes => "SELECT
                CASE WHEN typeof(c.id)='text' AND length(CAST(c.id AS BLOB)) BETWEEN 1 AND ?2 THEN c.id END,
                CASE WHEN typeof(c.source_graph)='text' AND length(CAST(c.source_graph AS BLOB)) BETWEEN 1 AND ?2 THEN c.source_graph END,
                CASE WHEN typeof(c.source_order)='integer' AND c.source_order>=0 THEN c.source_order END,
                CASE WHEN typeof(c.payload_len)='integer' AND c.payload_len>=0 AND c.payload_len<=?3 THEN c.payload_len END,
                CASE WHEN typeof(c.payload_sha256)='blob' AND length(c.payload_sha256)=32 THEN c.payload_sha256 END,
                CASE WHEN typeof(c.payload)='blob' AND length(c.payload)<=?3 THEN c.payload END,
                CASE WHEN typeof(c.payload_codec)='integer' THEN c.payload_codec END,
                CASE WHEN typeof(c.source_packet_sha256)='blob' AND length(c.source_packet_sha256)=32 THEN c.source_packet_sha256 END,
                CASE WHEN typeof(s.packet_len)='integer' AND s.packet_len>=1 AND s.packet_len<=?3 THEN s.packet_len END,
                CASE WHEN typeof(s.packet_sha256)='blob' AND length(s.packet_sha256)=32 THEN s.packet_sha256 END,
                CASE WHEN typeof(s.packet)='blob' AND s.packet_len>=1 AND s.packet_len<=?3 AND length(s.packet)=s.packet_len THEN s.packet END
                FROM knowledge_nodes c LEFT JOIN knowledge_source_carriers s ON s.packet_sha256=c.source_packet_sha256
                WHERE c.source_graph IN (SELECT value FROM json_each(?1))
                AND (?4=0 OR (?4=1 AND c.id=?5) OR (?4=2 AND c.native_id=?5) OR (?4=3 AND c.entity_id=?5))
                ORDER BY ORDER_KEY LIMIT ?6",
            ControlledSearchKind::Relations => "SELECT
                CASE WHEN typeof(c.id)='text' AND length(CAST(c.id AS BLOB)) BETWEEN 1 AND ?2 THEN c.id END,
                CASE WHEN typeof(c.source_graph)='text' AND length(CAST(c.source_graph AS BLOB)) BETWEEN 1 AND ?2 THEN c.source_graph END,
                CASE WHEN typeof(c.source_order)='integer' AND c.source_order>=0 THEN c.source_order END,
                CASE WHEN typeof(c.payload_len)='integer' AND c.payload_len>=0 AND c.payload_len<=?3 THEN c.payload_len END,
                CASE WHEN typeof(c.payload_sha256)='blob' AND length(c.payload_sha256)=32 THEN c.payload_sha256 END,
                CASE WHEN typeof(c.payload)='blob' AND length(c.payload)<=?3 THEN c.payload END,
                CASE WHEN typeof(c.payload_codec)='integer' THEN c.payload_codec END,
                CASE WHEN typeof(c.source_packet_sha256)='blob' AND length(c.source_packet_sha256)=32 THEN c.source_packet_sha256 END,
                CASE WHEN typeof(s.packet_len)='integer' AND s.packet_len>=1 AND s.packet_len<=?3 THEN s.packet_len END,
                CASE WHEN typeof(s.packet_sha256)='blob' AND length(s.packet_sha256)=32 THEN s.packet_sha256 END,
                CASE WHEN typeof(s.packet)='blob' AND s.packet_len>=1 AND s.packet_len<=?3 AND length(s.packet)=s.packet_len THEN s.packet END
                FROM knowledge_relations c LEFT JOIN knowledge_source_carriers s ON s.packet_sha256=c.source_packet_sha256
                WHERE c.source_graph IN (SELECT value FROM json_each(?1))
                AND (?4=0 OR (?4=1 AND c.id=?5) OR (?4=2 AND c.native_id=?5) OR (?4=4 AND (c.from_id IN (SELECT value FROM json_each(?5)) OR c.to_id IN (SELECT value FROM json_each(?5)))))
                ORDER BY ORDER_KEY LIMIT ?6",
        };
        if limit == 0 { return Ok(Ok(ControlledLegacySearchScan::default())); }
        let sql_state = sql.len().checked_add(order.len())
            .and_then(|n| n.checked_add(std::mem::size_of::<String>()))
            .ok_or(Error::Budget("controlled carrier SQL state"))?;
        let _sql_hold = self.context.owned_state().hold(sql_state)?;
        self.charge_query_work(needle.len().checked_add(sql.len())
            .ok_or(Error::Budget("controlled carrier selector work"))?)?;
        let sql = sql.replace("ORDER_KEY", order);
        let context = self.context;
        let connection = self.connection;
        let pinned = self.pinned;
        let selection = self.selection;
        let custody = self.custody;
        let layout = KnowledgePayloadLayout::CarrierOnceV1;
        let mut callback_error = None;
        let (scan, vm_steps) = crate::knowledge_payload_read::with_query_vm_window(
            context,
            connection,
            max_vm_steps,
            || {
                let mut statement = connection.prepare_cached(&sql)?;
                let mut rows = statement.query(params![
                    encoded_sources,
                    max_field_bytes as i64,
                    max_payload_bytes as i64, selector, needle, limit
                ])?;
                let mut scan = ControlledLegacySearchScan::default();
                while let Some(row) = rows.next()? {
                    context.check()?;
                    custody.verify(pinned, selection)?;
                    scan.rows = scan
                        .rows
                        .checked_add(1)
                        .ok_or(Error::Budget("controlled legacy row count"))?;
                    if scan.rows > max_rows {
                        return Err(Error::Budget("controlled legacy row count"));
                    }
                    let (
                        Some(id),
                        Some(source_graph),
                        Some(position),
                        Some(logical_len),
                        Some(payload_sha),
                        Some(physical),
                        Some(codec),
                        source_sha,
                        source_len,
                        joined_source_sha,
                        source_packet,
                    ) = (
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<Vec<u8>>>(4)?,
                        row.get::<_, Option<Vec<u8>>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, Option<Vec<u8>>>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                        row.get::<_, Option<Vec<u8>>>(9)?,
                        row.get::<_, Option<Vec<u8>>>(10)?,
                    )
                    else {
                        return Err(Error::Invalid("controlled legacy carrier row shape"));
                    };
                    if position < 0
                        || logical_len <= 0
                        || physical.is_empty()
                        || physical.len() > max_payload_bytes
                        || (source_sha.is_some() != source_packet.is_some())
                        || (source_packet.is_some() != joined_source_sha.is_some())
                        || (source_packet.is_some() != source_len.is_some())
                        || source_len.is_some_and(|length| {
                            length < 0
                                || source_packet
                                    .as_ref()
                                    .is_none_or(|packet| packet.len() as i64 != length)
                        })
                    {
                        return Err(Error::Invalid("controlled legacy carrier row shape"));
                    }
                    let digest = |raw: &[u8]| -> Result<Digest256> {
                        let bytes: [u8; 32] = raw
                            .try_into()
                            .map_err(|_| Error::Invalid("controlled legacy digest width"))?;
                        Ok(Digest256::from_bytes(bytes))
                    };
                    let payload_sha256 = digest(&payload_sha)?;
                    let source_packet_sha256 = source_sha.as_deref().map(digest).transpose()?;
                    let joined_digest = joined_source_sha.as_deref().map(digest).transpose()?;
                    if source_packet_sha256 != joined_digest {
                        return Err(Error::Invalid("controlled legacy source packet join"));
                    }
                    let logical_len = usize::try_from(logical_len)
                        .map_err(|_| Error::Budget("controlled legacy logical length"))?;
                    scan.decoded_bytes = scan
                        .decoded_bytes
                        .checked_add(logical_len as u64)
                        .ok_or(Error::Budget("controlled legacy decoded bytes"))?;
                    if scan.decoded_bytes > max_decoded_bytes {
                        return Err(Error::Budget("controlled legacy decoded bytes"));
                    }
                    let mut limits = json_limits;
                    limits.max_bytes = limits.max_bytes.min(max_payload_bytes);
                    let _decoded = crate::knowledge_payload_read::with_logical_payload_for_verified_layout(
                        context,
                        layout,
                        &SelectedPayloadRow {
                            payload_codec: u8::try_from(codec)
                                .map_err(|_| Error::Invalid("controlled legacy payload codec"))?,
                            physical: &physical,
                            logical_len,
                            logical_sha256: payload_sha256,
                            source_packet_sha256,
                            source_packet: source_packet.as_deref(),
                        },
                        JsonLimits::default(),
                        JsonLimits::default(),
                        max_payload_bytes,
                        |logical, context| {
                            if logical.len() != logical_len {
                                return Err(Error::Invalid("controlled legacy logical length"));
                            }
                            context.charge_work(logical.len())?;
                            context.with_foundation_owned_with_limits(logical, limits, |value| {
                                if value.as_object().is_none()
                                    || value.object_get("id").and_then(JsonValue::as_str)
                                        != Some(id.as_str())
                                    || value.object_get("source_graph").and_then(JsonValue::as_str)
                                        != Some(source_graph.as_str())
                                {
                                    return Err(Error::Invalid("controlled legacy carrier mirrors"));
                                }
                                let mut charge = |bytes: usize| {
                                    context.check()?;
                                    context.charge_work(bytes)?;
                                    context.check()
                                };
                                match consume(
                                    &source_graph,
                                    position as u64,
                                    payload_sha256,
                                    logical.len(),
                                    value,
                                    &mut charge,
                                    heap,
                                ) {
                                    Ok(()) => Ok(()),
                                    Err(reason) => {
                                        callback_error = Some(reason);
                                        Ok(())
                                    }
                                }
                            })
                        },
                    )?;
                    if callback_error.is_some() {
                        break;
                    }
                }
                Ok(scan)
            },
        )?;
        self.check_pin()?;
        let scan = ControlledLegacySearchScan {
            vm_steps,
            ..scan
        };
        if let Some(error) = callback_error {
            return Ok(Err(error));
        }
        Ok(Ok(scan))
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
