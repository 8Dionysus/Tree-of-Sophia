//! One fresh, privately streamed catalog render for source admission.
//!
//! The output tree is temporary candidate material under an already selected
//! isolated root. It preserves the renderer's exact relative paths and exact
//! caller-supplied manifest bytes; it does not select a catalog or grant
//! admission. Only the two row bindings consumed by `source_index` are retained
//! from the same addressed-row callbacks.

use crate::source_admission_index::{FreshIndexRowsWriter, FreshRows};
use crate::source_command::SourceCommandError;
use crate::source_creation_store::{
    DisposableCatalogTree, DisposableCatalogTreeCost, DisposableCatalogTreeLimits,
    IsolatedCreationRoot,
};
use serde_json::{Map, Value};
use std::mem::size_of;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_compiler::source_witness_catalog::{SourceCatalogLimits, SourceCatalogSink};
use tos_compiler::{Error, Result};

/// Cost belongs to this fresh sink only. It does not include the compiler's
/// independently budgeted stage or a later native inventory/map merge.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FreshCatalogCandidateCost {
    pub(crate) output_files: usize,
    pub(crate) output_bytes: usize,
    pub(crate) readback_bytes: usize,
    pub(crate) created_inodes: usize,
    pub(crate) addressed_rows: u64,
    pub(crate) record_rows: u64,
    pub(crate) claim_rows: u64,
    pub(crate) slot_rows: u64,
    pub(crate) fresh_rows_retained_state_upper_bound_bytes: usize,
    pub(crate) peak_state_upper_bound_bytes: usize,
    pub(crate) manifest_sha256: tos_foundation::Digest256,
}

/// A successful renderer result remains in a disposable, identity-checked tree
/// for as long as this value is retained. Dropping it removes only that tree.
pub(crate) struct FreshCatalogCandidate<'a> {
    tree: DisposableCatalogTree<'a>,
    rows: FreshRows,
    cost: FreshCatalogCandidateCost,
}

impl FreshCatalogCandidate<'_> {
    pub(crate) fn root_path(&self) -> &std::path::Path {
        self.tree.root_path()
    }

    pub(crate) fn fresh_rows(&self) -> &FreshRows {
        &self.rows
    }

    pub(crate) fn fresh_rows_mut(&mut self) -> &mut FreshRows {
        &mut self.rows
    }

    pub(crate) fn cost(&self) -> FreshCatalogCandidateCost {
        self.cost
    }

    pub(crate) fn eof_verified(&self) -> bool {
        self.tree.eof_verified()
    }
}

/// A bounded SourceCatalogSink that writes each callback directly into the
/// caller's private isolated root and captures the exact rows used by FND.
pub(crate) struct FreshCatalogSink<'root, 'manifest, 'cancel, 'rows> {
    tree: DisposableCatalogTree<'root>,
    source_limits: SourceCatalogLimits,
    max_addressed_rows: u64,
    exact_manifest_raw: &'manifest [u8],
    deadline: Instant,
    cancelled: &'cancel AtomicBool,
    rows: FreshRows,
    rows_writer: Option<&'rows mut dyn FreshIndexRowsWriter>,
    record_rows: u64,
    claim_rows: u64,
    slot_rows: u64,
    addressed_rows: u64,
    fixed_external_state_bytes: usize,
    retained_pair_state_bytes: usize,
    manifest_seen: bool,
}

impl<'root, 'manifest, 'cancel, 'rows> FreshCatalogSink<'root, 'manifest, 'cancel, 'rows> {
    pub(crate) fn new(
        isolated: &'root IsolatedCreationRoot,
        tree_limits: DisposableCatalogTreeLimits,
        source_limits: SourceCatalogLimits,
        exact_manifest_raw: &'manifest [u8],
        deadline: Instant,
        cancelled: &'cancel AtomicBool,
    ) -> Result<Self> {
        Self::new_inner(
            isolated,
            tree_limits,
            source_limits,
            exact_manifest_raw,
            deadline,
            cancelled,
            None,
        )
    }

    /// Candidate-specific route that sends addressed identity pairs directly
    /// to the caller's already-reserved native row store. The writer carries no
    /// admission authority; native validation still consumes and verifies it.
    pub(crate) fn new_with_index_rows(
        isolated: &'root IsolatedCreationRoot,
        tree_limits: DisposableCatalogTreeLimits,
        source_limits: SourceCatalogLimits,
        exact_manifest_raw: &'manifest [u8],
        deadline: Instant,
        cancelled: &'cancel AtomicBool,
        rows_writer: &'rows mut dyn FreshIndexRowsWriter,
    ) -> Result<Self> {
        Self::new_inner(
            isolated,
            tree_limits,
            source_limits,
            exact_manifest_raw,
            deadline,
            cancelled,
            Some(rows_writer),
        )
    }

    fn new_inner(
        isolated: &'root IsolatedCreationRoot,
        tree_limits: DisposableCatalogTreeLimits,
        source_limits: SourceCatalogLimits,
        exact_manifest_raw: &'manifest [u8],
        deadline: Instant,
        cancelled: &'cancel AtomicBool,
        rows_writer: Option<&'rows mut dyn FreshIndexRowsWriter>,
    ) -> Result<Self> {
        source_limits.validate()?;
        let max_addressed_rows = source_limits
            .max_rows
            .checked_mul(3)
            .ok_or(Error::Budget("fresh catalog addressed row count"))?;
        if max_addressed_rows == 0 {
            return Err(Error::Budget("fresh catalog addressed row count"));
        }
        let sink_fixed = size_of::<Self>()
            .checked_sub(size_of::<DisposableCatalogTree<'root>>())
            .and_then(|n| n.checked_add(exact_manifest_raw.len()))
            .ok_or(Error::Budget("fresh catalog retained row state"))?;
        if DisposableCatalogTree::<'root>::minimum_state_upper_bound()
            .and_then(|n| n.checked_add(sink_fixed))
            .is_none_or(|n| n > tree_limits.max_state_bytes)
        {
            return Err(Error::Budget("fresh catalog retained row state"));
        }
        let tree =
            DisposableCatalogTree::create_in_child(isolated, tree_limits, deadline, cancelled)
                .map_err(map_source_error)?;
        let mut sink = Self {
            tree,
            source_limits,
            max_addressed_rows,
            exact_manifest_raw,
            deadline,
            cancelled,
            rows: FreshRows {
                records: Vec::new(),
                claims: Vec::new(),
                native_semantic: Default::default(),
            },
            rows_writer,
            record_rows: 0,
            claim_rows: 0,
            slot_rows: 0,
            addressed_rows: 0,
            fixed_external_state_bytes: sink_fixed,
            retained_pair_state_bytes: 0,
            manifest_seen: false,
        };
        sink.tree
            .set_external_state(sink.fixed_external_state_bytes)
            .map_err(map_source_error)?;
        Ok(sink)
    }

    fn external_state_bytes(&self) -> Result<usize> {
        self.fixed_external_state_bytes
            .checked_add(self.retained_pair_state_bytes)
            .ok_or(Error::Budget("fresh catalog retained row state"))
    }

    fn row_workspace(&self, raw: &[u8]) -> Result<usize> {
        if raw.len() > self.source_limits.max_output_row_bytes {
            return Err(Error::Budget("fresh catalog addressed row bytes"));
        }
        raw.len()
            .checked_mul(128)
            .and_then(|n| n.checked_add(raw.len()))
            .and_then(|n| n.checked_add(512))
            .ok_or(Error::Budget("fresh catalog decoded row state"))
    }

    fn parse_index_pair(&mut self, collection: &str, raw: &[u8], retain: bool) -> Result<Value> {
        let workspace = self.row_workspace(raw)?;
        let retained = self.external_state_bytes()?;
        self.tree
            .check_external_peak(retained, workspace)
            .map_err(map_source_error)?;

        // Addressed rows are fresh compiler output, not persisted corpus JSON.
        // The source compiler has already applied the selected output-row cap;
        // this single bounded decode is only to move the two index bindings.
        let mut decoded: Value = serde_json::from_slice(raw)
            .map_err(|_| Error::Invalid("fresh catalog addressed row JSON"))?;
        let row = decoded
            .as_object_mut()
            .ok_or(Error::Invalid("fresh catalog addressed row object"))?;
        let (id_field, ref_field, entry_ref_field) = match collection {
            "records" => ("record_id", "source_record_ref", "source_record_ref"),
            "claims" => ("claim_id", "source_claim_file_ref", "source_claim_file_ref"),
            _ => return Err(Error::Invalid("fresh catalog addressed row collection")),
        };
        let id = row
            .remove(id_field)
            .filter(Value::is_string)
            .ok_or(Error::Invalid("fresh catalog addressed row identity"))?;
        let mut entry = row
            .remove("entry")
            .ok_or(Error::Invalid("fresh catalog addressed row entry"))?;
        let entry = entry
            .as_object_mut()
            .ok_or(Error::Invalid("fresh catalog addressed row entry"))?;
        if entry
            .get(id_field)
            .and_then(Value::as_str)
            .is_none_or(|entry_id| id.as_str() != Some(entry_id))
        {
            return Err(Error::Invalid("fresh catalog addressed identity binding"));
        }
        let source_ref = entry
            .remove(entry_ref_field)
            .filter(Value::is_string)
            .ok_or(Error::Invalid("fresh catalog addressed source reference"))?;
        let pair_state = pair_state_upper_bound(&id, &source_ref)?;
        self.tree
            .check_external_peak(retained, workspace)
            .map_err(map_source_error)?;
        if retain {
            self.tree
                .check_external_peak(
                    retained
                        .checked_add(pair_state)
                        .ok_or(Error::Budget("fresh catalog row state"))?,
                    workspace,
                )
                .map_err(map_source_error)?;
        } else {
            self.tree
                .check_external_peak(retained, workspace)
                .map_err(map_source_error)?;
        }
        let mut pair = Map::new();
        pair.insert(id_field.to_owned(), id);
        pair.insert(ref_field.to_owned(), source_ref);
        let pair = Value::Object(pair);
        if retain {
            self.retained_pair_state_bytes = self
                .retained_pair_state_bytes
                .checked_add(pair_state)
                .ok_or(Error::Budget("fresh catalog retained row state"))?;
            self.tree
                .set_external_state(self.external_state_bytes()?)
                .map_err(map_source_error)?;
        }
        Ok(pair)
    }

    fn increment_category(&mut self, category: &str) -> Result<()> {
        {
            let counter = match category {
                "records" => &mut self.record_rows,
                "claims" => &mut self.claim_rows,
                "slots" => &mut self.slot_rows,
                _ => return Err(Error::Invalid("fresh catalog row category")),
            };
            *counter = counter
                .checked_add(1)
                .filter(|count| *count <= self.source_limits.max_rows)
                .ok_or(Error::Budget("fresh catalog addressed category rows"))?;
        }
        self.addressed_rows = self
            .addressed_rows
            .checked_add(1)
            .filter(|count| *count <= self.max_addressed_rows)
            .ok_or(Error::Budget("fresh catalog addressed total rows"))?;
        Ok(())
    }

    fn check_manifest_binding(&mut self, rendered: &Value) -> Result<()> {
        self.tree
            .check_active(self.deadline, self.cancelled)
            .map_err(map_source_error)?;
        let workspace = self.row_workspace(self.exact_manifest_raw)?;
        self.tree
            .check_external_peak(self.external_state_bytes()?, workspace)
            .map_err(map_source_error)?;
        let published: Value = serde_json::from_slice(self.exact_manifest_raw)
            .map_err(|_| Error::Invalid("fresh catalog published manifest JSON"))?;
        let published_fields = published
            .as_object()
            .ok_or(Error::Invalid("fresh catalog published manifest object"))?;
        let rendered_fields = rendered
            .as_object()
            .ok_or(Error::Invalid("fresh catalog rendered manifest object"))?;
        let has_extended_record_profile = rendered
            .get("record_files")
            .and_then(Value::as_object)
            .is_some_and(|files| {
                files.keys().any(|kind| {
                    ![
                        "agent",
                        "place",
                        "organization",
                        "work",
                        "expression",
                        "edition",
                        "collection",
                        "item",
                        "link",
                    ]
                    .contains(&kind.as_str())
                })
            });
        if rendered_fields
            .iter()
            .any(|(name, value)| published_fields.get(name) != Some(value))
            || published_fields.iter().any(|(name, value)| {
                !rendered_fields.contains_key(name)
                    && name != "selected_metadata_publication"
                    && !(name == "extension_schema_refs"
                        && has_extended_record_profile
                        && value.as_array().is_some_and(Vec::is_empty))
            })
        {
            return Err(Error::Invalid(
                "fresh catalog published manifest differs from rendered seal",
            ));
        }
        self.tree
            .check_active(self.deadline, self.cancelled)
            .map_err(map_source_error)?;
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<FreshCatalogCandidate<'root>> {
        if !self.manifest_seen {
            return Err(Error::Invalid("fresh catalog renderer manifest absent"));
        }
        let tree_cost = self
            .tree
            .finish(self.deadline, self.cancelled)
            .map_err(map_source_error)?;
        if !self.tree.eof_verified() {
            return Err(Error::PreparedUnsupported(
                "fresh catalog output EOF custody incomplete",
            ));
        }
        let manifest_sha256 = self
            .tree
            .manifest_sha256()
            .ok_or(Error::Invalid("fresh catalog manifest custody absent"))?;
        let cost = FreshCatalogCandidateCost {
            output_files: tree_cost.output_files,
            output_bytes: tree_cost.output_bytes,
            readback_bytes: tree_cost.readback_bytes,
            created_inodes: tree_cost.created_inodes,
            addressed_rows: self.addressed_rows,
            record_rows: self.record_rows,
            claim_rows: self.claim_rows,
            slot_rows: self.slot_rows,
            fresh_rows_retained_state_upper_bound_bytes: size_of::<FreshRows>()
                .checked_add(self.retained_pair_state_bytes)
                .ok_or(Error::Budget("fresh catalog retained row state"))?,
            peak_state_upper_bound_bytes: tree_cost.peak_state_upper_bound_bytes,
            manifest_sha256,
        };
        Ok(FreshCatalogCandidate {
            tree: self.tree,
            rows: self.rows,
            cost,
        })
    }
}

impl SourceCatalogSink for FreshCatalogSink<'_, '_, '_, '_> {
    fn begin_file(&mut self, source_ref: &str) -> Result<()> {
        self.tree
            .begin_file(source_ref, self.deadline, self.cancelled)
            .map_err(map_source_error)
    }

    fn file_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.tree
            .file_bytes(bytes, self.deadline, self.cancelled)
            .map_err(map_source_error)
    }

    fn end_file(&mut self, source_ref: &str, sha256: &str) -> Result<()> {
        self.tree
            .end_file(source_ref, sha256, self.deadline, self.cancelled)
            .map_err(map_source_error)
    }

    fn addressed_row(&mut self, collection: &str, raw: &[u8]) -> Result<()> {
        self.increment_category(collection)?;
        match collection {
            "records" => {
                let retain = self.rows_writer.is_none();
                let pair = self.parse_index_pair(collection, raw, retain)?;
                if self.rows_writer.is_some() {
                    let id = pair
                        .get("record_id")
                        .and_then(Value::as_str)
                        .ok_or(Error::Invalid("fresh catalog record identity"))?;
                    let source_ref = pair
                        .get("source_record_ref")
                        .and_then(Value::as_str)
                        .ok_or(Error::Invalid("fresh catalog record source reference"))?;
                    let pair_state = pair_state_upper_bound(
                        pair.get("record_id")
                            .ok_or(Error::Invalid("fresh catalog record identity"))?,
                        pair.get("source_record_ref")
                            .ok_or(Error::Invalid("fresh catalog record source reference"))?,
                    )?;
                    self.tree
                        .check_external_peak(self.external_state_bytes()?, pair_state)
                        .map_err(map_source_error)?;
                    let writer = self
                        .rows_writer
                        .as_deref_mut()
                        .ok_or(Error::Invalid("fresh catalog row spool absent"))?;
                    writer.push_record(id, source_ref).map_err(Error::Io)?;
                } else {
                    self.rows.records.push(pair);
                }
            }
            "claims" => {
                let retain = self.rows_writer.is_none();
                let pair = self.parse_index_pair(collection, raw, retain)?;
                if self.rows_writer.is_some() {
                    let id = pair
                        .get("claim_id")
                        .and_then(Value::as_str)
                        .ok_or(Error::Invalid("fresh catalog claim identity"))?;
                    let source_ref = pair
                        .get("source_claim_file_ref")
                        .and_then(Value::as_str)
                        .ok_or(Error::Invalid("fresh catalog claim source reference"))?;
                    let pair_state = pair_state_upper_bound(
                        pair.get("claim_id")
                            .ok_or(Error::Invalid("fresh catalog claim identity"))?,
                        pair.get("source_claim_file_ref")
                            .ok_or(Error::Invalid("fresh catalog claim source reference"))?,
                    )?;
                    self.tree
                        .check_external_peak(self.external_state_bytes()?, pair_state)
                        .map_err(map_source_error)?;
                    let writer = self
                        .rows_writer
                        .as_deref_mut()
                        .ok_or(Error::Invalid("fresh catalog row spool absent"))?;
                    writer.push_claim(id, source_ref).map_err(Error::Io)?;
                } else {
                    self.rows.claims.push(pair);
                }
            }
            "slots" => {
                if raw.len() > self.source_limits.max_output_row_bytes {
                    return Err(Error::Budget("fresh catalog addressed slot bytes"));
                }
                self.tree
                    .check_external_peak(self.external_state_bytes()?, raw.len())
                    .map_err(map_source_error)?;
            }
            _ => return Err(Error::Invalid("fresh catalog row category")),
        }
        Ok(())
    }

    fn manifest(&mut self, manifest: &Value) -> Result<()> {
        if self.manifest_seen {
            return Err(Error::Invalid("fresh catalog renderer manifest repeated"));
        }
        self.check_manifest_binding(manifest)?;
        let files = manifest
            .get("record_files")
            .and_then(Value::as_object)
            .ok_or(Error::Invalid("fresh catalog manifest record routes"))?;
        let claim_file = manifest
            .get("claim_file")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("fresh catalog manifest claim route"))?;
        let expected_files = files
            .len()
            .checked_add(1)
            .ok_or(Error::Budget("fresh catalog manifest route count"))?;
        if self.tree.output_file_count() != expected_files || !self.tree.has_file(claim_file) {
            return Err(Error::Invalid(
                "fresh catalog rendered file routes differ from manifest",
            ));
        }
        let route_state = files
            .len()
            .checked_mul(size_of::<&str>())
            .and_then(|n| n.checked_add(size_of::<Vec<&str>>()))
            .ok_or(Error::Budget("fresh catalog manifest route state"))?;
        self.tree
            .check_external_peak(self.external_state_bytes()?, route_state)
            .map_err(map_source_error)?;
        let mut record_routes = Vec::new();
        record_routes
            .try_reserve_exact(files.len())
            .map_err(|_| Error::Budget("fresh catalog manifest route state"))?;
        for path in files.values() {
            self.tree
                .check_active(self.deadline, self.cancelled)
                .map_err(map_source_error)?;
            let path = path
                .as_str()
                .filter(|path| self.tree.has_file(path))
                .ok_or(Error::Invalid("fresh catalog rendered record route absent"))?;
            record_routes.push(path);
        }
        record_routes.sort_unstable();
        if record_routes.windows(2).any(|pair| pair[0] == pair[1])
            || record_routes.binary_search(&claim_file).is_ok()
            || !self.tree.has_file(claim_file)
        {
            return Err(Error::Invalid(
                "fresh catalog rendered file routes are not one-to-one",
            ));
        }
        self.tree
            .check_active(self.deadline, self.cancelled)
            .map_err(map_source_error)?;
        self.tree
            .write_manifest(self.exact_manifest_raw, self.deadline, self.cancelled)
            .map_err(map_source_error)?;
        self.manifest_seen = true;
        Ok(())
    }
}

fn pair_state_upper_bound(id: &Value, source_ref: &Value) -> Result<usize> {
    let id_bytes = id
        .as_str()
        .ok_or(Error::Invalid("fresh catalog pair identity type"))?
        .len();
    let ref_bytes = source_ref
        .as_str()
        .ok_or(Error::Invalid("fresh catalog pair reference type"))?
        .len();
    size_of::<Value>()
        .checked_add(2 * size_of::<(String, Value)>())
        .and_then(|n| n.checked_add(8 * size_of::<usize>()))
        .and_then(|n| n.checked_add(id_bytes))
        .and_then(|n| n.checked_add(ref_bytes))
        .and_then(|n| n.checked_add("record_id".len().max("claim_id".len())))
        .and_then(|n| n.checked_add("source_record_ref".len().max("source_claim_file_ref".len())))
        .ok_or(Error::Budget("fresh catalog pair state overflow"))
}

fn map_source_error(error: SourceCommandError) -> Error {
    match error {
        SourceCommandError::Invalid(reason) => Error::Invalid(reason),
        SourceCommandError::Conflict(reason) => Error::PreparedUnsupported(reason),
        SourceCommandError::Denied(reason) => Error::PreparedUnsupported(reason),
        SourceCommandError::DeniedWithReason(reason) => Error::Source(reason),
        SourceCommandError::Unsupported(reason) => Error::Budget(reason),
        SourceCommandError::SchemaExecution { .. } => {
            Error::PreparedUnsupported("fresh catalog schema capture incomplete")
        }
        SourceCommandError::MissingProductionAdmission => {
            Error::PreparedUnsupported("fresh catalog admission unavailable")
        }
    }
}
