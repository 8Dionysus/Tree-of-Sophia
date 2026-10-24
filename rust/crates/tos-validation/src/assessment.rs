//! Bounded native application of the maintained knowledge-assessment rules.
//!
//! These inputs are observations supplied by the protected owner reader. This
//! module authenticates nobody, reads no journal, and issues no permission.
//! In particular a rendered `can_use` is the deterministic current view of
//! authored judgments, not a publication/admission token. CMD must reconstruct
//! and reevaluate its protected snapshot under its current source/config/head
//! fence. Source grounding, native content custody and history authenticity
//! cannot be established by assessment prose or by this pure calculation.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::time::Instant;

use serde_json::{Value, json};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonNumberKind, JsonValue,
    SourceRevision, canonical_bytes_v1, canonical_raw_bytes_v1, parse_json,
    python_casefold_unicode16_v1, python_strip_unicode16_v1,
};

use crate::FormatProfile;
use crate::executor::BatchBudget;
use crate::item_rules::ItemRefusal;
use crate::retirement_rules::observed_instant_order;
use crate::source_cut::{
    CandidateCutWorkerSchemaExecutor, CutPreparedSchemaExecutionBinding, CutSchemaCheck,
    CutSchemaExecutor, CutWorkerSchemaExecutor,
};


pub const MAX_ASSESSMENTS: usize = 1024;
pub const MAX_RECORD_BYTES: usize = 1_048_576;
pub const MAX_REQUIRED_ADMISSIONS: usize = 64;

/// Raw `{id,version,payload,origin_id}` owner envelopes. Payload record
/// digests are derived here using the existing Python record profile.
#[derive(Debug, Clone)]
pub struct AssessmentRecordInput {
    pub envelope: Vec<u8>,
}

/// The principal and execution binding are supplied by the authenticated
/// adapter; the original committed scope comes from its verified batch.
#[derive(Debug, Clone)]
pub struct AssessmentSubmissionInput {
    pub assessment: Vec<u8>,
    pub principal_id: String,
    pub execution_profile: Vec<u8>,
    pub committed_scope: Option<Vec<u8>>,
}

/// Current source-layer comparison observations supplied by the selected
/// native owner adapter after the exact private source comparison and its
/// currentness recheck. These values are never accepted from a command
/// request or read as caller-provided readiness/eligibility from configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssessmentLayerQualityObservation {
    /// The selected source contract requires an exact source comparison.
    pub source_comparison_required: bool,
    /// That required comparison is present in the current selected source cut.
    pub source_comparison_present: bool,
    /// Current exact source comparison and layer policy permit positive use.
    /// The native adapter derives this; assessment prose and request fields do
    /// not grant it.
    pub positive_use_allowed: bool,
}

#[derive(Debug, Clone)]
pub struct AssessmentReadInput {
    /// Exact immutable source cut when the legacy complete-cut adapter owns
    /// selection. Candidate-input adapters leave this absent and bind their
    /// opaque read epoch through `source_input_identity`.
    pub source_revision: Option<SourceRevision>,
    /// Digest of the exact source identity used by this calculation. Legacy
    /// cut adapters use the revision digest; candidate adapters provide the
    /// digest of their typed source-selection identity.
    pub source_input_identity: Digest256,
    pub policy: AssessmentRecordInput,
    pub authorities: Vec<AssessmentRecordInput>,
    pub competencies: Vec<AssessmentRecordInput>,
    pub records: Vec<AssessmentRecordInput>,
    /// Independently resolved authored source bindings; inline owner records
    /// cannot shadow their IDs. Native adapters remain separately identified.
    pub source_records: Vec<AssessmentRecordInput>,
    pub native_records: Vec<AssessmentRecordInput>,
    pub source_route: AssessmentSourceRoute,
    pub subject_id: String,
    /// Actual configured scope including its exact `record` binding. It is
    /// selected by the protected owner configuration, never by a reviewer.
    pub configured_scope: Vec<u8>,
    /// Exact refs selected by the actual source-owner dependency resolver.
    pub required_source_refs: Vec<Vec<u8>>,
    /// Existing owner-derived quality basis envelopes, with their actual
    /// `can_use`/`limits` observations. A caller cannot substitute a bool.
    pub required_admission_bases: Vec<AssessmentRecordInput>,
    /// Present only for the selected native LayerQuality adapter after exact
    /// comparison and currentness recheck. Never supplied by the command
    /// request/configuration or inferred from assessment prose.
    pub layer_quality: Option<AssessmentLayerQualityObservation>,
    /// Content-ready is derived from the separate `native_records` and their
    /// entire native_binding/content_verified observations. The private owner
    /// resolver must produce these after an actual content read; authored or
    /// configured payloads cannot populate that vector as read observations.
    pub reviews: Vec<AssessmentSubmissionInput>,
    pub trusted_history: Vec<AssessmentSubmissionInput>,
    pub observed_now: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssessmentSourceRoute {
    SelectedSource,
    SourceBoundClaim,
    /// A selected native adapter supplies current layer/comparison
    /// observations and derived quality bases. It does not obtain eligibility
    /// from assessment prose or command-request booleans.
    LayerQuality,
}

#[derive(Debug, Clone, Copy)]
pub struct AssessmentLimits {
    pub max_input_bytes: usize,
    pub max_work: usize,
    pub batch: BatchBudget,
    pub deadline: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssessmentRefusal {
    InvalidInput(String),
    Unsupported(String),
    Budget,
    Cancelled,
    Deadline,
    Schema(ItemRefusal),
}

/// Truthful schema/source binding for one mechanics report. Candidate input
/// evaluations retain their opaque identity digest without presenting a
/// bootstrap revision as the current source revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssessmentSchemaBindingV1 {
    SourceCut {
        source_revision: SourceRevision,
        worker: CutPreparedSchemaExecutionBinding,
    },
    SelectedInput {
        source_identity: Digest256,
        worker: CutPreparedSchemaExecutionBinding,
    },
}

/// Private construction ensures this result always comes from the native
/// calculation. It is deliberately not usable as an authorization token.
#[derive(Debug, Clone)]
pub struct AssessmentMechanicsReport {
    admission: Value,
    scope: Value,
    required_sources: Vec<Value>,
    required_admissions: Vec<Value>,
    source_read_ready: bool,
    source_read_required: bool,
    input_sha256: Digest256,
    source_revision: Option<SourceRevision>,
    source_input_identity: Digest256,
    schema_binding: AssessmentSchemaBindingV1,
    observed_now: String,
    work_used: usize,
    input_bytes_used: usize,
}

/// Static refusal from the C01 private-report retained-state certificate.
/// This is not an assessment, source-access decision, or allocator/RSS meter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AssessmentRetainedFootprintRefusal {
    UnsupportedShape,
    Overflow,
    OwnedLimit,
    VisitLimit,
    Workspace,
}

enum C01ReportFrame<'a> {
    Node {
        value: &'a Value,
        depth: usize,
    },
    Array {
        iter: std::slice::Iter<'a, Value>,
        depth: usize,
    },
    Object {
        iter: serde_json::map::Iter<'a>,
        depth: usize,
    },
}

#[derive(Clone, Copy)]
enum C01ReportAction {
    Pop,
    Replace,
    Push,
}

// Persistent traversal/action state only. External activation and expression
// values are separately admitted below; unused in-header bytes cannot pay them.
struct C01ReportHeader<'report, 'visit> {
    visit: &'visit mut dyn FnMut(usize) -> bool,
    max_owned: usize,
    total: usize,
    field: usize,
    top: usize,
    index: usize,
    depth: usize,
    length: usize,
    amount: usize,
    buckets: usize,
    bucket_upper: usize,
    node: Option<&'report Value>,
    pair: Option<(&'report String, &'report Value)>,
    action: C01ReportAction,
}

struct C01ReportWorkspace<'report, 'visit> {
    frames: [Option<C01ReportFrame<'report>>; 9],
    replacement: Option<C01ReportFrame<'report>>,
    header: C01ReportHeader<'report, 'visit>,
}

type C01FootprintResult<T> = std::result::Result<T, AssessmentRetainedFootprintRefusal>;

impl C01ReportHeader<'_, '_> {
    fn step(&mut self, units: usize) -> C01FootprintResult<()> {
        if !(self.visit)(units) {
            return Err(AssessmentRetainedFootprintRefusal::VisitLimit);
        }
        Ok(())
    }

    fn add(&mut self, bytes: usize) -> C01FootprintResult<()> {
        self.step(1)?;
        self.amount = bytes;
        self.total = self
            .total
            .checked_add(self.amount)
            .ok_or(AssessmentRetainedFootprintRefusal::Overflow)?;
        self.step(1)?;
        if self.total > self.max_owned {
            return Err(AssessmentRetainedFootprintRefusal::OwnedLimit);
        }
        Ok(())
    }

    fn value_slots(&mut self, capacity: usize) -> C01FootprintResult<()> {
        self.step(1)?;
        self.amount = capacity
            .checked_mul(std::mem::size_of::<Value>())
            .ok_or(AssessmentRetainedFootprintRefusal::Overflow)?;
        self.add(self.amount)
    }

    fn object_slots(&mut self, entries: usize) -> C01FootprintResult<()> {
        // ONLY this private report's fresh parse/json!/serialization/clone
        // constructors qualify. M includes initial hints and clone-source
        // ceilings; none of those constructors clears/reserves an older map.
        // This is not a capacity certificate for arbitrary external Values.
        self.step(1)?;
        if entries == 0 {
            return Ok(());
        }
        self.step(1)?;
        self.buckets = entries
            .checked_mul(2)
            .ok_or(AssessmentRetainedFootprintRefusal::Overflow)?;
        self.step(1)?;
        self.buckets = self
            .buckets
            .max(4)
            .checked_next_power_of_two()
            .ok_or(AssessmentRetainedFootprintRefusal::Overflow)?;
        self.step(1)?;
        self.amount = self
            .buckets
            .checked_mul(self.bucket_upper)
            .ok_or(AssessmentRetainedFootprintRefusal::Overflow)?;
        self.add(self.amount)?;
        self.step(1)?;
        self.amount = self
            .buckets
            .checked_mul(8)
            .and_then(|n| n.checked_add(15))
            .ok_or(AssessmentRetainedFootprintRefusal::Overflow)?;
        self.step(1)?;
        self.amount &= !15;
        self.add(self.amount)?;
        self.add(self.buckets)?;
        self.add(16)
    }
}

impl C01ReportWorkspace<'_, '_> {
    fn walk(&mut self) -> C01FootprintResult<()> {
        loop {
            self.header.step(1)?;
            if self.header.top == 0 {
                return Ok(());
            }
            self.header.step(1)?;
            self.header.index = self
                .header
                .top
                .checked_sub(1)
                .ok_or(AssessmentRetainedFootprintRefusal::Overflow)?;
            self.header.step(1)?;
            match self.frames[self.header.index]
                .as_mut()
                .ok_or(AssessmentRetainedFootprintRefusal::UnsupportedShape)?
            {
                C01ReportFrame::Node { value, depth } => {
                    self.header.step(1)?; // Value, before its tag or children
                    self.header.depth = *depth;
                    self.header.action = C01ReportAction::Pop;
                    match *value {
                        Value::Null | Value::Bool(_) => {}
                        Value::String(text) => {
                            self.header.step(1)?;
                            self.header.add(text.capacity())?;
                        }
                        Value::Number(number) => {
                            self.header.step(1)?;
                            self.header.length = number.as_str().len();
                            self.header.step(1)?;
                            if self.header.length != 1 {
                                return Err(AssessmentRetainedFootprintRefusal::UnsupportedShape);
                            }
                            self.header.step(1)?;
                            self.header.amount = self
                                .header
                                .length
                                .checked_mul(3)
                                .ok_or(AssessmentRetainedFootprintRefusal::Overflow)?;
                            self.header.step(self.header.amount)?;
                            if !matches!(number.as_str(), "1" | "2" | "3") {
                                return Err(AssessmentRetainedFootprintRefusal::UnsupportedShape);
                            }
                            // Pinned canonical U64/itoa + fresh clone/serialize
                            // ref versions. NOT arbitrary Number::as_str capacity.
                            self.header.add(16)?;
                        }
                        Value::Array(values) => {
                            self.header.step(1)?;
                            self.header.value_slots(values.capacity())?;
                            self.replacement = Some(C01ReportFrame::Array {
                                iter: values.iter(),
                                depth: *depth,
                            });
                            self.header.action = C01ReportAction::Replace;
                        }
                        Value::Object(values) => {
                            self.header.step(1)?;
                            self.header.length = values.len();
                            self.header.object_slots(self.header.length)?;
                            self.replacement = Some(C01ReportFrame::Object {
                                iter: values.iter(),
                                depth: *depth,
                            });
                            self.header.action = C01ReportAction::Replace;
                        }
                    }
                }
                C01ReportFrame::Array { iter, depth } => {
                    self.header.step(1)?; // includes next(None)
                    self.header.node = iter.next();
                    self.header.step(1)?;
                    self.header.action = if self.header.node.is_some() {
                        self.header.step(1)?;
                        self.header.depth = depth
                            .checked_add(1)
                            .ok_or(AssessmentRetainedFootprintRefusal::Overflow)?;
                        C01ReportAction::Push
                    } else {
                        C01ReportAction::Pop
                    };
                }
                C01ReportFrame::Object { iter, depth } => {
                    self.header.step(1)?; // includes next(None)
                    self.header.pair = iter.next();
                    self.header.step(1)?;
                    if let Some((key, value)) = self.header.pair {
                        self.header.step(1)?; // key backing, no key lookup/clone
                        self.header.add(key.capacity())?;
                        self.header.node = Some(value);
                        self.header.step(1)?;
                        self.header.depth = depth
                            .checked_add(1)
                            .ok_or(AssessmentRetainedFootprintRefusal::Overflow)?;
                        self.header.action = C01ReportAction::Push;
                    } else {
                        self.header.action = C01ReportAction::Pop;
                    }
                }
            }
            self.header.step(1)?;
            match self.header.action {
                C01ReportAction::Pop => {
                    self.replacement = self.frames[self.header.index].take();
                    self.replacement = None;
                    self.header.top = self.header.index;
                }
                C01ReportAction::Replace => {
                    self.frames[self.header.index] = self.replacement.take();
                }
                C01ReportAction::Push => {
                    self.header.step(1)?;
                    if self.header.depth > 8 || self.header.top >= self.frames.len() {
                        return Err(AssessmentRetainedFootprintRefusal::UnsupportedShape);
                    }
                    self.header.step(1)?;
                    self.replacement = Some(C01ReportFrame::Node {
                        value: self
                            .header
                            .node
                            .take()
                            .ok_or(AssessmentRetainedFootprintRefusal::UnsupportedShape)?,
                        depth: self.header.depth,
                    });
                    self.frames[self.header.top] = self.replacement.take();
                    self.header.step(1)?;
                    self.header.top = self
                        .header
                        .top
                        .checked_add(1)
                        .ok_or(AssessmentRetainedFootprintRefusal::Overflow)?;
                }
            }
        }
    }
}

impl AssessmentMechanicsReport {
    /// Conservative retained capacity of this privately constructed mechanics
    /// report for the pinned C01 command graph. This neither authenticates the
    /// report nor changes current admission. Unsupported layouts/shapes refuse.
    ///
    /// BEFORE calling, the command owner must reserve 2816 logical workspace
    /// bytes and a visit subaccount from its SAME adapter payer. The callback
    /// consumes that subaccount (false is immediately VisitLimit); it must keep
    /// its own first error. No workspace/result/visit credit is refunded here.
    /// This counts Self once; an enclosing result subtracts that inline Self
    /// size before adding the report's backing. No allocator/RSS claim is made.
    pub fn c01_retained_footprint_with_visit(
        &self,
        max_owned: usize,
        visit: &mut dyn FnMut(usize) -> bool,
    ) -> std::result::Result<usize, AssessmentRetainedFootprintRefusal> {
        if !visit(1) {
            // report root
            return Err(AssessmentRetainedFootprintRefusal::VisitLimit);
        }
        if !visit(32) {
            // paid prefix/layout/representation/refusal guards
            return Err(AssessmentRetainedFootprintRefusal::VisitLimit);
        }
        if max_owned == 0 {
            return Err(AssessmentRetainedFootprintRefusal::OwnedLimit);
        }
        // Pinned serde_json1.0.151 preserve_order: Map wraps IndexMap (72
        // bytes), unlike the BTree representation. Cargo.lock and the selected
        // command feature graph are part of this certificate, not a new flag.
        if !cfg!(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        )) || std::mem::size_of::<usize>() != 8
            || std::mem::size_of::<String>() != 24
            || std::mem::size_of::<Vec<Value>>() != 24
            || std::mem::size_of::<serde_json::Map<String, Value>>() != 72
            || std::mem::size_of::<serde_json::Number>() != 24
            || std::mem::size_of::<Value>() > 128
            || std::mem::size_of::<Self>() > 1024
            || std::mem::align_of::<String>() > 8
            || std::mem::align_of::<Value>() > 8
            || std::mem::align_of::<Self>() > 8
        {
            return Err(AssessmentRetainedFootprintRefusal::UnsupportedShape);
        }
        // Pay the additional outside-workspace inventory/layout/arithmetic
        // guards BEFORE computing it or constructing any traversal state.
        if !visit(64) {
            return Err(AssessmentRetainedFootprintRefusal::VisitLimit);
        }
        // Conservative simultaneous SOURCE-value inventory (not machine stack):
        // four Option<Frame> construction/take/replacement/move temporaries;
        // one complete Header initializer; eight usize slots for outer inputs
        // and return; sixteen for ALL nonrecursive helper activation arguments;
        // sixteen for ALL pattern borrows (including pair/key/value); thirty-two
        // Option<usize> arithmetic temporaries (27 checked expressions total);
        // eight Result<usize, E> slots for nested call/?/residual values (maximum
        // six active method levels); four fat str borrows; thirty-two remaining
        // scalar/closure/guard/iterator-result slots. These SUM all alternatives
        // in each category rather than assuming optimizer reuse. Iterators are
        // bounded by the four whole-frame slots. Header fields, the nine frame
        // array and replacement already belong to Workspace; moving their owned
        // state creates no second logical owned array. No compiler-stack/RSS or
        // callee callback implementation workspace is certified here.
        let outside_upper = std::mem::size_of::<Option<C01ReportFrame<'_>>>()
            .checked_mul(4)
            .and_then(|n| n.checked_add(std::mem::size_of::<C01ReportHeader<'_, '_>>()))
            .and_then(|n| n.checked_add(8 * std::mem::size_of::<usize>()))
            .and_then(|n| n.checked_add(16 * std::mem::size_of::<usize>()))
            .and_then(|n| n.checked_add(16 * std::mem::size_of::<usize>()))
            .and_then(|n| n.checked_add(32 * std::mem::size_of::<Option<usize>>()))
            .and_then(|n| n.checked_add(8 * std::mem::size_of::<C01FootprintResult<usize>>()))
            .and_then(|n| n.checked_add(4 * std::mem::size_of::<&str>()))
            .and_then(|n| n.checked_add(32 * std::mem::size_of::<usize>()))
            .ok_or(AssessmentRetainedFootprintRefusal::Workspace)?;
        // Hard guards precede BOTH frames and complete header construction.
        if std::mem::size_of::<Option<C01ReportFrame<'_>>>() > 256
            || std::mem::align_of::<Option<C01ReportFrame<'_>>>() > 16
            || std::mem::size_of::<C01ReportHeader<'_, '_>>() > 256
            || std::mem::align_of::<C01ReportHeader<'_, '_>>() > 16
            || std::mem::size_of::<C01ReportWorkspace<'_, '_>>()
                .checked_add(outside_upper)
                .ok_or(AssessmentRetainedFootprintRefusal::Workspace)?
                > 2816
        {
            return Err(AssessmentRetainedFootprintRefusal::Workspace);
        }
        let mut counter = C01ReportWorkspace {
            frames: std::array::from_fn(|_| None),
            replacement: None,
            header: C01ReportHeader {
                visit,
                max_owned,
                total: 0,
                field: 0,
                top: 0,
                index: 0,
                depth: 0,
                length: 0,
                amount: 0,
                buckets: 0,
                bucket_upper: 0,
                node: None,
                pair: None,
                action: C01ReportAction::Pop,
            },
        };
        counter.header.step(1)?;
        counter.header.bucket_upper = 8usize
            .checked_add(24)
            .and_then(|n| n.checked_add(std::mem::size_of::<Value>()))
            .and_then(|n| n.checked_add(32))
            .and_then(|n| n.checked_add(7))
            .ok_or(AssessmentRetainedFootprintRefusal::Overflow)?
            & !7;
        counter.header.add(std::mem::size_of::<Self>())?;
        loop {
            counter.header.step(1)?;
            if counter.header.field == 13 {
                return Ok(counter.header.total);
            }
            counter.header.step(1)?; // each private field before examination
            match counter.header.field {
                0 => {
                    counter.replacement = Some(C01ReportFrame::Node {
                        value: &self.admission,
                        depth: 0,
                    });
                }
                1 => {
                    counter.replacement = Some(C01ReportFrame::Node {
                        value: &self.scope,
                        depth: 0,
                    });
                }
                2 => {
                    counter
                        .header
                        .value_slots(self.required_sources.capacity())?;
                    counter.replacement = Some(C01ReportFrame::Array {
                        iter: self.required_sources.iter(),
                        depth: 0,
                    });
                }
                3 => {
                    counter
                        .header
                        .value_slots(self.required_admissions.capacity())?;
                    counter.replacement = Some(C01ReportFrame::Array {
                        iter: self.required_admissions.iter(),
                        depth: 0,
                    });
                }
                // bools, digests, Option<SourceRevision>, Copy schema-binding,
                // and both usize counters are fully inline in Self.
                4..=9 | 11..=12 => {}
                10 => counter.header.add(self.observed_now.capacity())?,
                _ => return Err(AssessmentRetainedFootprintRefusal::UnsupportedShape),
            }
            counter.header.step(1)?;
            if counter.replacement.is_some() {
                counter.frames[0] = counter.replacement.take();
                counter.header.top = 1;
                counter.walk()?;
            }
            counter.header.step(1)?;
            counter.header.field = counter
                .header
                .field
                .checked_add(1)
                .ok_or(AssessmentRetainedFootprintRefusal::Overflow)?;
        }
    }

    pub fn current_admission(&self) -> &Value {
        &self.admission
    }
    pub fn scope(&self) -> &Value {
        &self.scope
    }
    pub fn required_sources(&self) -> &[Value] {
        &self.required_sources
    }
    pub fn required_admissions(&self) -> &[Value] {
        &self.required_admissions
    }
    pub fn source_read_ready(&self) -> bool {
        self.source_read_ready
    }
    /// The maintained describe response omits `source_read` entirely when
    /// no native binding is required. In particular Sign must not turn the
    /// vacuous true readiness of an empty closure into a present read gate.
    pub fn source_read_required(&self) -> bool {
        self.source_read_required
    }
    pub fn input_sha256(&self) -> Digest256 {
        self.input_sha256
    }
    pub fn source_revision(&self) -> Option<SourceRevision> {
        self.source_revision
    }
    pub fn source_input_identity(&self) -> Digest256 {
        self.source_input_identity
    }
    pub fn schema_binding(&self) -> AssessmentSchemaBindingV1 {
        self.schema_binding
    }
    pub fn observed_now(&self) -> &str {
        &self.observed_now
    }
    /// Consumption by this exact evaluation. A protected native caller can
    /// subtract it from one operation-wide budget before evaluating another
    /// subject; reports do not reset the caller's shared budget.
    pub fn work_used(&self) -> usize {
        self.work_used
    }
    pub fn input_bytes_used(&self) -> usize {
        self.input_bytes_used
    }
}

type Result<T> = std::result::Result<T, AssessmentRefusal>;

struct Work<'a> {
    limits: AssessmentLimits,
    cancelled: &'a AtomicBool,
    bytes: usize,
    work: usize,
}
impl Work<'_> {
    fn tick(&mut self, count: usize) -> Result<()> {
        if self.cancelled.load(AtomicOrdering::Relaxed) {
            return Err(AssessmentRefusal::Cancelled);
        }
        if Instant::now() >= self.limits.deadline {
            return Err(AssessmentRefusal::Deadline);
        }
        self.work = self
            .work
            .checked_add(count)
            .filter(|n| *n <= self.limits.max_work)
            .ok_or(AssessmentRefusal::Budget)?;
        Ok(())
    }
    fn raw(&mut self, raw: &[u8]) -> Result<Value> {
        self.charge(raw.len())?;
        decoded(raw)
    }
    fn charge(&mut self, bytes: usize) -> Result<()> {
        self.tick(1)?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_input_bytes)
            .ok_or(AssessmentRefusal::Budget)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Record {
    id: String,
    version: Value,
    body: Value,
    canonical: Vec<u8>,
    origin: Option<String>,
    reference: Value,
}
impl Record {
    fn read(input: &AssessmentRecordInput, work: &mut Work<'_>) -> Result<Self> {
        let envelope = work.raw(&input.envelope)?;
        let Some(fields) = envelope.as_object() else {
            return invalid("assessment owner envelope must be an object");
        };
        if fields.len() != 4
            || ["id", "version", "payload", "origin_id"]
                .iter()
                .any(|key| !fields.contains_key(*key))
        {
            return invalid("assessment owner envelope fields differ from maintained contract");
        }
        let id = text(&envelope, "id")?.to_owned();
        if stripped(&id)?.is_empty() {
            return invalid("record.id must be nonempty");
        }
        let original =
            parse_json(&input.envelope, JsonMode::PublishedStrict, raw_limits()).map_err(codec)?;
        let version = original
            .root()
            .object_get("version")
            .ok_or_else(|| bad("record.version missing"))?;
        match version {
            JsonValue::Number(n)
                if n.kind == JsonNumberKind::Int
                    && !n.lexeme.starts_with('-')
                    && n.lexeme != "0" =>
            {
                ()
            }
            _ => return invalid("record.version must be a positive integer"),
        }
        let payload = original
            .root()
            .object_get("payload")
            .ok_or_else(|| bad("record.payload missing"))?;
        if payload.as_object().is_none() {
            return invalid("record.payload must be a JSON object");
        }
        let canonical = canonical_bytes_v1(
            payload,
            CanonicalProfile::SourceRecordDigestV1,
            JsonLimits::default(),
        )
        .map_err(codec)?;
        if canonical.len() > MAX_RECORD_BYTES {
            return Err(AssessmentRefusal::Budget);
        }
        let body: Value = serde_json::from_slice(&canonical)
            .map_err(|_| unsupported("native scalar representation"))?;
        let version = field(&envelope, "version")?.clone();
        let origin = match envelope.get("origin_id") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) if !stripped(s)?.is_empty() => Some(s.clone()),
            _ => return invalid("record.origin_id must be nonempty when present"),
        };
        let reference = json!({"id":id,"version":version,"digest":Digest256::of_bytes(&canonical).to_prefixed()});
        Ok(Self {
            id,
            version,
            body,
            canonical,
            origin,
            reference,
        })
    }
    fn assessment(id: &str, body: &Value) -> Result<Value> {
        let canonical = canonical(body)?;
        if canonical.len() > MAX_RECORD_BYTES {
            return Err(AssessmentRefusal::Budget);
        }
        Ok(json!({"id":id,"version":1,"digest":Digest256::of_bytes(&canonical).to_prefixed()}))
    }
}

#[derive(Clone)]
struct Submission {
    body: Value,
    principal: String,
    executor: Value,
    scope: Option<Value>,
    canonical: Vec<u8>,
    schema_valid: bool,
}
impl Submission {
    fn read(input: &AssessmentSubmissionInput, work: &mut Work<'_>) -> Result<Self> {
        work.charge(input.principal_id.len())?;
        let body = work.raw(&input.assessment)?;
        if !body.is_object() {
            return invalid("submission assessment must be an object");
        }
        let executor = work.raw(&input.execution_profile)?;
        let scope = input
            .committed_scope
            .as_ref()
            .map(|raw| work.raw(raw))
            .transpose()?;
        if let Some(scope) = &scope {
            validate_scope(scope, false)?;
        }
        let canonical = canonical(&body)?;
        Ok(Self {
            body,
            principal: input.principal_id.clone(),
            executor,
            scope,
            canonical,
            schema_valid: false,
        })
    }
}

struct Context {
    subject: Record,
    scope: Value,
    languages: Vec<String>,
    required: Vec<Record>,
    admissions: Vec<Record>,
    source_ready: bool,
    positive: bool,
}
struct Engine {
    policy: Record,
    authorities: BTreeMap<String, Record>,
    competencies: BTreeMap<String, Record>,
    records: BTreeMap<String, Record>,
    profiles: BTreeMap<String, Value>,
}

/// Current-view calculation on exact raw owner inputs and current selected
/// grammar. No weaker/schema-free adapter is accepted. The protected caller
/// must preserve this input binding and reevaluate while holding its fences.
pub fn evaluate_current_assessment(
    input: &AssessmentReadInput,
    schemas: &mut CutWorkerSchemaExecutor,
    limits: AssessmentLimits,
    cancelled: &AtomicBool,
) -> Result<AssessmentMechanicsReport> {
    let Some(source_revision) = input.source_revision else {
        return invalid("assessment complete-cut source revision is absent");
    };
    if schemas.selected_source_revision() != Some(source_revision)
        || input.source_input_identity != source_revision.0
    {
        return invalid(
            "assessment source/schema cut revision differs from selected current source",
        );
    }
    let worker = CutSchemaExecutor::prepared_execution_binding(schemas)
        .ok_or_else(|| unsupported("assessment schema worker binding is unavailable"))?;
    evaluate_current_assessment_inner(
        input,
        schemas,
        AssessmentSchemaBindingV1::SourceCut {
            source_revision,
            worker,
        },
        limits,
        cancelled,
    )
}

/// Evaluate the maintained assessment rules against an exact selected-input
/// identity. This uses the same asserted schema worker and mechanics as the
/// complete-cut route while leaving the caller's identity opaque.
pub fn evaluate_current_assessment_selected<I: Copy + Eq>(
    input: &AssessmentReadInput,
    input_identity: &I,
    input_identity_digest: Digest256,
    schemas: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: AssessmentLimits,
    cancelled: &AtomicBool,
) -> Result<AssessmentMechanicsReport> {
    if input.source_revision.is_some()
        || input.source_input_identity != input_identity_digest
        || schemas.input_identity() != input_identity
    {
        return invalid("assessment candidate source identity differs from selected worker");
    }
    let worker = CutSchemaExecutor::prepared_execution_binding(schemas)
        .ok_or_else(|| unsupported("assessment candidate worker binding is unavailable"))?;
    evaluate_current_assessment_inner(
        input,
        schemas,
        AssessmentSchemaBindingV1::SelectedInput {
            source_identity: input_identity_digest,
            worker,
        },
        limits,
        cancelled,
    )
}

fn evaluate_current_assessment_inner<S: CutSchemaExecutor>(
    input: &AssessmentReadInput,
    schemas: &mut S,
    schema_binding: AssessmentSchemaBindingV1,
    limits: AssessmentLimits,
    cancelled: &AtomicBool,
) -> Result<AssessmentMechanicsReport> {
    if limits.max_input_bytes == 0 || limits.max_work == 0 {
        return Err(AssessmentRefusal::Budget);
    }
    if schemas.schema_profile() != Some(FormatProfile::AssertedSourceCandidateV1) {
        return Err(unsupported(
            "assessment requires asserted source grammar formats",
        ));
    }
    if input.authorities.len() > MAX_ASSESSMENTS
        || input.competencies.len() > MAX_ASSESSMENTS
        || input
            .records
            .len()
            .checked_add(input.source_records.len())
            .and_then(|n| n.checked_add(input.native_records.len()))
            .is_none_or(|n| n > MAX_ASSESSMENTS)
        || input.required_source_refs.len() > MAX_ASSESSMENTS
        || input.required_admission_bases.len() > MAX_REQUIRED_ADMISSIONS
        || input
            .reviews
            .len()
            .checked_add(input.trusted_history.len())
            .is_none_or(|n| n > MAX_ASSESSMENTS)
    {
        return Err(AssessmentRefusal::Budget);
    }
    let mut work = Work {
        limits,
        cancelled,
        bytes: 0,
        work: 0,
    };
    match (input.source_route, input.layer_quality) {
        (AssessmentSourceRoute::LayerQuality, None) => {
            return Err(unsupported(
                "layer quality requires selected native comparison observations",
            ));
        }
        (
            AssessmentSourceRoute::SelectedSource | AssessmentSourceRoute::SourceBoundClaim,
            Some(_),
        ) => {
            return invalid("layer quality observation is outside the selected source route");
        }
        _ => (),
    }
    compare_time(&input.observed_now, &input.observed_now)?;
    work.charge(input.subject_id.len())?;
    work.charge(input.observed_now.len())?;
    let policy = Record::read(&input.policy, &mut work)?;
    let mut checks = vec![schema_check(&policy.id, &policy.canonical, "-policy")];
    let mut authorities = BTreeMap::new();
    let mut competencies = BTreeMap::new();
    let mut records = BTreeMap::new();
    for raw in &input.records {
        insert(&mut records, Record::read(raw, &mut work)?)?;
    }
    let inline_ids: BTreeSet<String> = records.keys().cloned().collect();
    let mut sourced = BTreeMap::new();
    let mut native = BTreeMap::new();
    for raw in &input.source_records {
        let record = Record::read(raw, &mut work)?;
        if inline_ids.contains(&record.id) {
            return invalid("inline owner record shadows selected source");
        }
        insert(&mut sourced, record.clone())?;
        insert(&mut records, record)?;
    }
    for raw in &input.native_records {
        let record = Record::read(raw, &mut work)?;
        if inline_ids.contains(&record.id) {
            return invalid("inline owner record shadows selected native source");
        }
        insert(&mut sourced, record.clone())?;
        insert(&mut native, record.clone())?;
        insert(&mut records, record)?;
    }
    insert(&mut records, policy.clone())?;
    for (inputs, target, suffix) in [
        (&input.authorities, &mut authorities, "-authority"),
        (&input.competencies, &mut competencies, "-competence"),
    ] {
        for raw in inputs {
            let record = Record::read(raw, &mut work)?;
            checks.push(schema_check(&record.id, &record.canonical, suffix));
            insert(target, record.clone())?;
            insert(&mut records, record)?;
        }
    }
    let owner_checks = checks.len();
    let mut history = input
        .trusted_history
        .iter()
        .map(|s| Submission::read(s, &mut work))
        .collect::<Result<Vec<_>>>()?;
    let mut reviews = input
        .reviews
        .iter()
        .map(|s| Submission::read(s, &mut work))
        .collect::<Result<Vec<_>>>()?;
    let mut submission_checks = Vec::new();
    for (ordinal, s) in history.iter().chain(&reviews).enumerate() {
        if s.canonical.len() > MAX_RECORD_BYTES {
            submission_checks.push(None);
        } else {
            submission_checks.push(Some(checks.len()));
            checks.push(schema_check(
                &format!("assessment:{ordinal}"),
                &s.canonical,
                "",
            ));
        }
    }
    let outcomes = check_all(schemas, &checks, &mut work)?;
    if outcomes[..owner_checks].iter().any(|valid| !valid) {
        return invalid("trusted assessment policy/authority/competence schema");
    }
    for (s, index) in history
        .iter_mut()
        .chain(&mut reviews)
        .zip(submission_checks)
    {
        s.schema_valid = index.is_some_and(|index| outcomes[index]);
    }
    for (name, collection) in [
        ("policy", vec![&policy]),
        ("authority", authorities.values().collect()),
        ("competence", competencies.values().collect()),
    ] {
        for record in collection {
            work.tick(1)?;
            if text(&record.body, &format!("{name}_id"))? != record.id
                || !py_equal(
                    field(&record.body, &format!("{name}_version"))?,
                    &record.version,
                )?
            {
                return invalid("trusted assessment envelope disagrees with its record");
            }
            if name != "policy"
                && compare_time(
                    text(&record.body, "valid_from")?,
                    text(&record.body, "valid_until")?,
                )? != Ordering::Less
            {
                return invalid("empty or reversed grant validity interval");
            }
        }
    }
    let mut profiles = BTreeMap::new();
    for profile in array(&policy.body, "profiles")? {
        work.tick(1)?;
        let id = text(profile, "profile_id")?.to_owned();
        if profiles.insert(id, profile.clone()).is_some() {
            return invalid("duplicate assessment profile");
        }
        if integer_cmp(
            field(profile, "min_independence_groups")?,
            field(profile, "min_reviewers")?,
        )? == Ordering::Greater
        {
            return invalid("independence groups exceed required reviewers");
        }
    }
    let scope = work.raw(&input.configured_scope)?;
    validate_scope(&scope, true)?;
    let subject = records
        .get(&input.subject_id)
        .cloned()
        .ok_or_else(|| bad("current subject missing"))?;
    if canonical(field(&scope, "record")?)? != canonical(&subject.reference)? {
        return invalid("configured subject is stale");
    }
    let mut required_index = BTreeMap::new();
    for raw in &input.required_source_refs {
        let reference = work.raw(raw)?;
        let record = resolve(&reference, &records)?
            .cloned()
            .ok_or_else(|| bad("required current source missing or stale"))?;
        if record.id == subject.id {
            return invalid("required source is the subject");
        }
        insert(&mut required_index, record)?;
    }
    let mut admission_index = BTreeMap::new();
    for raw in &input.required_admission_bases {
        let record = Record::read(raw, &mut work)?;
        if record.id == subject.id {
            return invalid("required admission is the subject");
        }
        if !field(&record.body, "can_use")?.is_boolean() {
            return invalid("quality basis eligibility must be an explicit boolean observation");
        }
        for limit in strings(&record.body, "limits")? {
            if stripped(&limit)?.is_empty() {
                return invalid("empty quality basis limit");
            }
        }
        // The basis must already be in the independently supplied current
        // engine snapshot. A dependency envelope cannot inject itself there.
        if resolve(&record.reference, &records)?.is_none() {
            return invalid("required quality basis absent from current owner records");
        }
        insert(&mut admission_index, record)?;
    }
    let required: Vec<Record> = required_index.into_values().collect();
    if input.source_route == AssessmentSourceRoute::SourceBoundClaim
        && !sourced.contains_key(&subject.id)
    {
        return invalid("source-bound Claim absent from selected source bindings");
    }
    if sourced.contains_key(&subject.id) {
        source_owned_scope(&subject, &scope)?;
    }
    if scope.get("form_language_context").is_some() {
        if !sourced.contains_key(&subject.id)
            || subject.body.get("schema_version").and_then(Value::as_str)
                != Some("tos_human_form_v1")
        {
            return invalid("form linguistic context is outside a selected source-form scope");
        }
        // Describe retains this authored form context. Its language grammar
        // is checked by the existing materialization route when consumed;
        // imposing that assertion here would strengthen the maintained read.
    }
    let (native_source_read_required, native_source_ready) = native_source_ready(
        &subject,
        &required,
        &sourced,
        &native,
        input.source_route,
        &mut work,
    )?;
    let (source_read_required, source_ready, positive) = match input.source_route {
        AssessmentSourceRoute::SelectedSource | AssessmentSourceRoute::SourceBoundClaim => {
            (native_source_read_required, native_source_ready, true)
        }
        AssessmentSourceRoute::LayerQuality => {
            let observation = input.layer_quality.ok_or_else(|| {
                unsupported("layer quality requires selected native comparison observations")
            })?;
            let comparison_ready =
                !observation.source_comparison_required || observation.source_comparison_present;
            (
                native_source_read_required || observation.source_comparison_required,
                native_source_ready && comparison_ready,
                observation.positive_use_allowed,
            )
        }
    };
    let context = Context {
        languages: strings(&scope, "languages")?,
        subject,
        scope,
        required,
        admissions: admission_index.into_values().collect(),
        source_ready,
        positive,
    };
    let engine = Engine {
        policy,
        authorities,
        competencies,
        records,
        profiles,
    };
    let admission =
        engine.evaluate(&context, &reviews, &history, &input.observed_now, &mut work)?;
    work.tick(1)?;
    let input_binding = input_binding(input);
    work.tick(1)?;
    Ok(AssessmentMechanicsReport {
        admission,
        scope: context.scope.clone(),
        required_sources: context
            .required
            .iter()
            .map(|r| r.reference.clone())
            .collect(),
        required_admissions: context
            .admissions
            .iter()
            .map(|r| r.reference.clone())
            .collect(),
        source_read_ready: context.source_ready,
        source_read_required,
        input_sha256: input_binding,
        source_revision: input.source_revision,
        source_input_identity: input.source_input_identity,
        schema_binding,
        observed_now: input.observed_now.clone(),
        work_used: work.work,
        input_bytes_used: work.bytes,
    })
}

fn schema_check(path: &str, raw: &[u8], suffix: &str) -> CutSchemaCheck {
    CutSchemaCheck {
        path: path.to_owned(),
        raw: raw.to_vec(),
        contract: format!("ToS/contracts/knowledge-assessment{suffix}.schema.json"),
    }
}

fn check_all<S: CutSchemaExecutor>(
    schemas: &mut S,
    checks: &[CutSchemaCheck],
    work: &mut Work<'_>,
) -> Result<Vec<bool>> {
    let budget = work.limits.batch;
    if budget.max_units == 0
        || budget.max_units > BatchBudget::MAX_UNITS
        || budget.max_total_raw_bytes == 0
        || budget.max_total_raw_bytes > BatchBudget::MAX_RAW_BYTES
    {
        return Err(AssessmentRefusal::Budget);
    }
    let mut results = Vec::with_capacity(checks.len());
    let mut start = 0;
    while start < checks.len() {
        work.tick(1)?;
        let mut end = start;
        let mut bytes = 0usize;
        while end < checks.len() && end - start < budget.max_units {
            let length = checks[end].raw.len();
            if length > MAX_RECORD_BYTES || length > budget.max_total_raw_bytes {
                return Err(AssessmentRefusal::Budget);
            }
            if length > budget.max_total_raw_bytes.saturating_sub(bytes) {
                break;
            }
            bytes += length;
            end += 1;
        }
        if end == start {
            return Err(AssessmentRefusal::Budget);
        }
        let batch = schemas
            .check_batch(
                &checks[start..end],
                budget,
                work.limits.deadline,
                work.cancelled,
            )
            .map_err(AssessmentRefusal::Schema)?;
        if batch.len() != end - start {
            return Err(unsupported("incomplete assessment schema coverage"));
        }
        results.extend(batch);
        start = end;
        work.tick(1)?;
    }
    Ok(results)
}

fn source_owned_scope(record: &Record, scope: &Value) -> Result<()> {
    let body = &record.body;
    if body.get("claim_id").is_some() && body.get("claim_version").is_some() {
        if field(scope, "assertion_layer")? != body.get("assertion_layer").unwrap_or(&Value::Null)
            || field(scope, "maker_id")?
                != body
                    .get("maker")
                    .and_then(|maker| maker.get("agent_ref"))
                    .unwrap_or(&Value::Null)
        {
            return invalid("configured scope disagrees with source-owned Claim layer or maker");
        }
        if matches!(
            body.get("predicate").and_then(Value::as_str),
            Some("identity_transition_proposal" | "subject_identity_transition_proposal")
        ) && (text(scope, "risk")? != "high" || text(scope, "requested_use")? != "research")
        {
            return invalid("identity proposal requires high-risk research assessment");
        }
    } else if body.get("schema_version").and_then(Value::as_str) == Some("tos_human_form_v1")
        && (text(scope, "assertion_layer")? != "human_projection"
            || scope.get("maker_id") != body.get("creator_id"))
    {
        return invalid("configured scope disagrees with source-owned form layer or maker");
    }
    Ok(())
}

fn native_source_ready(
    subject: &Record,
    required: &[Record],
    sourced: &BTreeMap<String, Record>,
    native: &BTreeMap<String, Record>,
    route: AssessmentSourceRoute,
    work: &mut Work<'_>,
) -> Result<(bool, bool)> {
    let rows: Vec<&Record> = if route == AssessmentSourceRoute::SourceBoundClaim {
        required.iter().collect()
    } else {
        sourced.values().collect()
    };
    let mut bindings: Vec<&Value> = rows
        .iter()
        .filter_map(|row| row.body.get("native_text_binding"))
        .collect();
    if subject.body.get("native_text_binding").is_some()
        || subject.body.get("schema_version").and_then(Value::as_str)
            == Some("tos_occurrence_description_record_v1")
    {
        bindings.push(
            subject
                .body
                .get("native_text_binding")
                .unwrap_or(&Value::Null),
        );
    }
    let required = !bindings.is_empty();
    let mut ready = true;
    for binding in bindings {
        work.tick(1)?;
        if !binding.is_object() {
            ready = false;
            continue;
        }
        let selected = canonical(binding)?;
        let mut found = false;
        for record in native.values() {
            work.tick(1)?;
            if record.body.get("content_verified").and_then(Value::as_bool) == Some(true)
                && canonical(record.body.get("native_binding").unwrap_or(&Value::Null))? == selected
            {
                found = true;
            }
        }
        ready &= found;
    }
    Ok((required, ready))
}

fn validate_scope(scope: &Value, configured: bool) -> Result<()> {
    let Some(object) = scope.as_object() else {
        return invalid("assessment scope must be an object");
    };
    let fields = if configured {
        vec![
            "record",
            "assertion_layer",
            "risk",
            "languages",
            "maker_id",
            "requested_use",
            "access_allowed",
        ]
    } else {
        vec![
            "assertion_layer",
            "risk",
            "languages",
            "maker_id",
            "requested_use",
        ]
    };
    if fields.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| {
            !fields.contains(&key.as_str()) && !(configured && key == "form_language_context")
        })
    {
        return invalid("assessment scope has missing or unrecognized fields");
    }
    if configured && !field(scope, "access_allowed")?.is_boolean() {
        return invalid("configured access observation must be boolean");
    }
    for key in ["assertion_layer", "risk", "maker_id", "requested_use"] {
        if text(scope, key)?.is_empty() {
            return invalid("assessment scope is incomplete");
        }
    }
    let languages = strings(scope, "languages")?;
    if languages.is_empty() || languages.iter().any(String::is_empty) {
        return invalid("assessment scope languages are incomplete");
    }
    Ok(())
}

fn scope_matches(scope: &Value, context: &Context) -> Result<bool> {
    for key in ["assertion_layer", "risk", "maker_id", "requested_use"] {
        if field(scope, key)? != field(&context.scope, key)? {
            return Ok(false);
        }
    }
    Ok(folded(&strings(scope, "languages")?)? == folded(&context.languages)?)
}
fn submission_scope_matches(s: &Submission, context: &Context) -> Result<bool> {
    s.scope
        .as_ref()
        .map(|scope| scope_matches(scope, context))
        .transpose()
        .map(|matches| matches.unwrap_or(true))
}
fn folded(values: &[String]) -> Result<BTreeSet<String>> {
    values
        .iter()
        .map(|value| {
            python_casefold_unicode16_v1(
                value,
                MAX_RECORD_BYTES,
                MAX_RECORD_BYTES,
                MAX_RECORD_BYTES,
            )
            .map_err(codec)
        })
        .collect()
}
fn languages_fit(required: &[String], allowed: &[String], work: &mut Work<'_>) -> Result<bool> {
    work.tick(
        required
            .len()
            .checked_add(allowed.len())
            .ok_or(AssessmentRefusal::Budget)?,
    )?;
    let accepted = folded(allowed)?;
    if accepted.contains("*") {
        return Ok(true);
    }
    Ok(folded(required)?.is_subset(&accepted))
}
fn stripped(value: &str) -> Result<&str> {
    python_strip_unicode16_v1(value, MAX_RECORD_BYTES).map_err(codec)
}

fn insert(index: &mut BTreeMap<String, Record>, record: Record) -> Result<()> {
    if index.get(&record.id).is_some_and(|old| old != &record) {
        return invalid("snapshot has conflicting current records");
    }
    index.insert(record.id.clone(), record);
    Ok(())
}
fn resolve<'a>(
    reference: &Value,
    records: &'a BTreeMap<String, Record>,
) -> Result<Option<&'a Record>> {
    let id = reference
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("exact reference lacks string id"))?;
    records
        .get(id)
        .map(|record| py_equal(&record.reference, reference).map(|same| same.then_some(record)))
        .transpose()
        .map(Option::flatten)
}

fn support_count(records: &[&Record], work: &mut Work<'_>) -> Result<usize> {
    let mut components: Vec<BTreeSet<String>> = Vec::new();
    for record in records {
        work.tick(1)?;
        let Some(origin) = &record.origin else {
            continue;
        };
        let mut keys: BTreeSet<String> = [
            format!("origin:{origin}"),
            format!("bytes:{}", text(&record.reference, "digest")?),
        ]
        .into_iter()
        .collect();
        let mut separate = Vec::new();
        for component in components {
            work.tick(1)?;
            if component.is_disjoint(&keys) {
                separate.push(component);
            } else {
                keys.extend(component);
            }
        }
        separate.push(keys);
        components = separate;
    }
    Ok(components.len())
}

fn independent_seats(
    options: &BTreeMap<String, BTreeSet<String>>,
    work: &mut Work<'_>,
) -> Result<usize> {
    let mut actor_group = BTreeMap::<String, String>::new();
    let mut group_actor = BTreeMap::<String, String>::new();
    for actor in options.keys() {
        let mut queue = VecDeque::from([actor.clone()]);
        let mut seen = BTreeSet::from([actor.clone()]);
        let mut parent = BTreeMap::<String, String>::new();
        let mut found = false;
        while let Some(current) = queue.pop_front() {
            for group in &options[&current] {
                work.tick(1)?;
                if parent.contains_key(group) {
                    continue;
                }
                parent.insert(group.clone(), current.clone());
                if !group_actor.contains_key(group) {
                    let mut step = Some(group.clone());
                    while let Some(group) = step {
                        work.tick(1)?;
                        let owner = parent[&group].clone();
                        let previous = actor_group.insert(owner.clone(), group.clone());
                        group_actor.insert(group, owner);
                        step = previous;
                    }
                    found = true;
                    break;
                }
                let next = group_actor[group].clone();
                if seen.insert(next.clone()) {
                    queue.push_back(next);
                }
            }
            if found {
                break;
            }
        }
    }
    Ok(actor_group.len())
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| bad(&format!("missing assessment field {key}")))
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    field(value, key)?
        .as_str()
        .ok_or_else(|| bad(&format!("assessment field {key} must be string")))
}
fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    field(value, key)?
        .as_array()
        .ok_or_else(|| bad(&format!("assessment field {key} must be array")))
}
fn strings(value: &Value, key: &str) -> Result<Vec<String>> {
    array(value, key)?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| bad("expected string array"))
        })
        .collect()
}
fn boolean(value: &Value, key: &str) -> Result<bool> {
    field(value, key)?
        .as_bool()
        .ok_or_else(|| bad("expected boolean observation"))
}
fn contains_text(value: &Value, key: &str, needle: &str) -> Result<bool> {
    Ok(strings(value, key)?.iter().any(|item| item == needle))
}
fn contains_value(value: &Value, key: &str, needle: &Value) -> Result<bool> {
    for item in array(value, key)? {
        if py_equal(item, needle)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn is_positive(decision: &str) -> bool {
    matches!(decision, "admit" | "admit-with-limits")
}

fn raw_limits() -> JsonLimits {
    JsonLimits {
        max_bytes: 8 * MAX_RECORD_BYTES,
        ..JsonLimits::default()
    }
}
fn decoded(raw: &[u8]) -> Result<Value> {
    let canonical =
        canonical_raw_bytes_v1(raw, CanonicalProfile::SourceRecordDigestV1, raw_limits())
            .map_err(codec)?;
    serde_json::from_slice(&canonical).map_err(|_| unsupported("native scalar representation"))
}
fn canonical(value: &Value) -> Result<Vec<u8>> {
    let raw = serde_json::to_vec(value).map_err(|_| bad("non-JSON assessment value"))?;
    canonical_raw_bytes_v1(&raw, CanonicalProfile::SourceRecordDigestV1, raw_limits())
        .map_err(codec)
}
fn compare_time(left: &str, right: &str) -> Result<Ordering> {
    observed_instant_order(left, right).map_err(|error| match error {
        crate::retirement_rules::ObservedDateTimeError::Budget => AssessmentRefusal::Budget,
        crate::retirement_rules::ObservedDateTimeError::Invalid => {
            bad("assessment instant requires a valid aware timestamp")
        }
    })
}
fn bad(message: &str) -> AssessmentRefusal {
    AssessmentRefusal::InvalidInput(message.to_owned())
}
fn unsupported(message: &str) -> AssessmentRefusal {
    AssessmentRefusal::Unsupported(message.to_owned())
}
fn invalid<T>(message: &str) -> Result<T> {
    Err(bad(message))
}
fn codec(error: tos_foundation::FoundationError) -> AssessmentRefusal {
    use tos_foundation::FoundationErrorCode as Code;
    match error.code {
        Code::BudgetExceeded => AssessmentRefusal::Budget,
        Code::InvalidUnicodeScalar | Code::UnsupportedFormat | Code::UnsupportedCanonicalNumber => {
            unsupported(&error.to_string())
        }
        _ => bad(&error.to_string()),
    }
}

/// Length-framed raw inputs, including trusted history purpose and native
/// source classification. This is an observation binding, not a grant.
fn input_binding(input: &AssessmentReadInput) -> Digest256 {
    fn push(hash: &mut Digest256Hasher, raw: &[u8]) {
        hash.update(&(raw.len() as u64).to_be_bytes());
        hash.update(raw);
    }
    fn records(hash: &mut Digest256Hasher, inputs: &[AssessmentRecordInput]) {
        hash.update(&(inputs.len() as u64).to_be_bytes());
        for input in inputs {
            push(hash, &input.envelope);
        }
    }
    fn submissions(hash: &mut Digest256Hasher, inputs: &[AssessmentSubmissionInput]) {
        hash.update(&(inputs.len() as u64).to_be_bytes());
        for input in inputs {
            push(hash, &input.assessment);
            push(hash, input.principal_id.as_bytes());
            push(hash, &input.execution_profile);
            hash.update(&[u8::from(input.committed_scope.is_some())]);
            if let Some(scope) = &input.committed_scope {
                push(hash, scope);
            }
        }
    }
    let mut hash = Digest256Hasher::new();
    hash.update(match input.source_route {
        AssessmentSourceRoute::LayerQuality => b"tos-current-assessment-input-v2\0",
        AssessmentSourceRoute::SelectedSource | AssessmentSourceRoute::SourceBoundClaim => {
            b"tos-current-assessment-input-v1\0"
        }
    });
    hash.update(input.source_input_identity.as_bytes());
    push(&mut hash, &input.policy.envelope);
    records(&mut hash, &input.authorities);
    records(&mut hash, &input.competencies);
    records(&mut hash, &input.records);
    records(&mut hash, &input.source_records);
    records(&mut hash, &input.native_records);
    hash.update(&[match input.source_route {
        AssessmentSourceRoute::SelectedSource => 0,
        AssessmentSourceRoute::SourceBoundClaim => 1,
        AssessmentSourceRoute::LayerQuality => 2,
    }]);
    push(&mut hash, input.subject_id.as_bytes());
    push(&mut hash, &input.configured_scope);
    hash.update(&(input.required_source_refs.len() as u64).to_be_bytes());
    for reference in &input.required_source_refs {
        push(&mut hash, reference);
    }
    records(&mut hash, &input.required_admission_bases);
    if let Some(observation) = input.layer_quality {
        hash.update(&[
            u8::from(observation.source_comparison_required),
            u8::from(observation.source_comparison_present),
            u8::from(observation.positive_use_allowed),
        ]);
    }
    submissions(&mut hash, &input.reviews);
    submissions(&mut hash, &input.trusted_history);
    push(&mut hash, input.observed_now.as_bytes());
    hash.finalize()
}

// Python's exact-ref dictionary equality is numeric equality (1 == 1.0),
// whereas evidence membership intentionally uses canonical bytes (1 != 1.0
// there). Do not round large owner integers through f64 to implement either.
pub(crate) fn py_equal(left: &Value, right: &Value) -> Result<bool> {
    match (left, right) {
        (Value::Number(_) | Value::Bool(_), Value::Number(_) | Value::Bool(_)) => {
            let li = integer_key(left)?;
            let ri = integer_key(right)?;
            match (li, ri) {
                (Some(a), Some(b)) => Ok(a == b),
                (None, None) => Ok(floating(left)? == floating(right)?),
                _ => Ok(false),
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (left, right) in a.iter().zip(b) {
                if !py_equal(left, right)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (Value::Object(a), Value::Object(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (key, left) in a {
                let Some(right) = b.get(key) else {
                    return Ok(false);
                };
                if !py_equal(left, right)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(left == right),
    }
}
fn floating(value: &Value) -> Result<f64> {
    let value = match value {
        Value::Bool(b) => return Ok(if *b { 1.0 } else { 0.0 }),
        Value::Number(number) => number
            .to_string()
            .parse::<f64>()
            .map_err(|_| unsupported("floating representation"))?,
        _ => return invalid("expected numeric value"),
    };
    if !value.is_finite() {
        return Err(unsupported("nonfinite numeric observation"));
    }
    Ok(value)
}
fn integer_key(value: &Value) -> Result<Option<String>> {
    let lexeme = match value {
        Value::Bool(value) => return Ok(Some(if *value { "1" } else { "0" }.to_owned())),
        Value::Number(number) => number.to_string(),
        _ => return invalid("expected numeric value"),
    };
    if !lexeme.bytes().any(|ch| matches!(ch, b'.' | b'e' | b'E')) {
        return Ok(Some(if lexeme == "-0" {
            "0".to_owned()
        } else {
            lexeme
        }));
    }
    let value = floating(value)?;
    if value.fract() != 0.0 {
        return Ok(None);
    }
    if value == 0.0 {
        return Ok(Some("0".to_owned()));
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mut mantissa = bits & ((1u64 << 52) - 1);
    let power = if exponent == 0 {
        -1074
    } else {
        mantissa |= 1u64 << 52;
        exponent - 1023 - 52
    };
    let mut digits: Vec<u8>;
    if power < 0 {
        let shift = (-power) as u32;
        if shift >= 64 || mantissa & ((1u64 << shift) - 1) != 0 {
            return Ok(None);
        }
        digits = (mantissa >> shift)
            .to_string()
            .bytes()
            .map(|byte| byte - b'0')
            .collect();
    } else {
        digits = mantissa
            .to_string()
            .bytes()
            .map(|byte| byte - b'0')
            .collect();
        for _ in 0..power {
            let mut carry = 0u8;
            for digit in digits.iter_mut().rev() {
                let next = *digit * 2 + carry;
                *digit = next % 10;
                carry = next / 10;
            }
            if carry != 0 {
                digits.insert(0, carry);
            }
        }
    }
    let mut out = String::with_capacity(digits.len() + 1);
    if bits >> 63 != 0 {
        out.push('-');
    }
    for digit in digits {
        out.push(char::from(b'0' + digit));
    }
    Ok(Some(out))
}
fn integer_cmp(left: &Value, right: &Value) -> Result<Ordering> {
    let left = integer_key(left)?.ok_or_else(|| bad("expected integer threshold"))?;
    let right = integer_key(right)?.ok_or_else(|| bad("expected integer threshold"))?;
    let negative_left = left.starts_with('-');
    let negative_right = right.starts_with('-');
    if negative_left != negative_right {
        return Ok(if negative_left {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }
    let l = left.trim_start_matches('-');
    let r = right.trim_start_matches('-');
    let order = l.len().cmp(&r.len()).then_with(|| l.cmp(r));
    Ok(if negative_left {
        order.reverse()
    } else {
        order
    })
}
fn meets(count: usize, threshold: &Value) -> Result<bool> {
    Ok(integer_cmp(&json!(count), threshold)? != Ordering::Less)
}

impl Engine {
    fn qualify(
        &self,
        submission: &Submission,
        context: &Context,
        now: &str,
        work: &mut Work<'_>,
    ) -> Result<Vec<String>> {
        work.tick(1)?;
        if !submission.schema_valid {
            return Ok(vec!["assessment.schema".into()]);
        }
        let a = &submission.body;
        let mut reasons = BTreeSet::new();
        macro_rules! reason {
            ($condition:expr,$name:expr) => {
                if $condition {
                    reasons.insert($name.to_owned());
                }
            };
        }
        reason!(
            !py_equal(field(a, "subject")?, &context.subject.reference)?
                || resolve(field(a, "subject")?, &self.records)?.is_none(),
            "subject.stale-or-mismatched"
        );
        reason!(
            !py_equal(field(a, "policy")?, &self.policy.reference)?,
            "policy.stale-or-mismatched"
        );
        let reviewer = field(a, "reviewer")?;
        reason!(
            submission.principal != text(reviewer, "actor_id")?,
            "reviewer.authentication"
        );
        reason!(
            context.scope.get("access_allowed").and_then(Value::as_bool) != Some(true),
            "subject.access-denied"
        );
        reason!(!context.source_ready, "subject.exact-source-unverified");
        reason!(
            submission
                .scope
                .as_ref()
                .map(|scope| scope_matches(scope, context))
                .transpose()?
                .is_some_and(|matches| !matches),
            "assessment.committed-scope-mismatch"
        );
        reason!(
            context.languages.is_empty() || text(&context.scope, "maker_id")?.is_empty(),
            "subject.scope-incomplete"
        );
        let issued = text(a, "issued_at")?;
        reason!(
            compare_time(issued, now)? == Ordering::Greater,
            "assessment.future"
        );
        let profile = self.profiles.get(text(a, "profile_id")?);
        let grant = resolve(field(a, "authority")?, &self.authorities)?;
        let competence = resolve(field(a, "competence")?, &self.competencies)?;
        reason!(profile.is_none(), "profile.unregistered");
        reason!(grant.is_none(), "authority.stale-or-missing");
        reason!(competence.is_none(), "competence.stale-or-missing");
        let (Some(profile), Some(grant), Some(competence)) = (profile, grant, competence) else {
            return Ok(reasons.into_iter().collect());
        };
        let authority = &grant.body;
        let calibration = &competence.body;
        let mut languages = context.languages.clone();
        languages.push(text(a, "language")?.into());
        let layer = text(&context.scope, "assertion_layer")?;
        let risk = text(&context.scope, "risk")?;
        let use_ = text(&context.scope, "requested_use")?;
        let actor = text(reviewer, "actor_id")?;
        reason!(
            !contains_text(profile, "assertion_layers", layer)?
                || !contains_text(profile, "risk_tiers", risk)?
                || !contains_text(profile, "uses", use_)?
                || !contains_text(profile, "reviewer_kinds", text(reviewer, "kind")?)?
                || !languages_fit(&languages, &strings(profile, "languages")?, work)?,
            "profile.outside-scope"
        );
        reason!(
            !boolean(profile, "allow_self_review")? && text(&context.scope, "maker_id")? == actor,
            "reviewer.self-review"
        );
        reason!(
            text(authority, "actor_id")? != actor
                || text(authority, "actor_kind")? != text(reviewer, "kind")?
                || !py_equal(field(authority, "policy")?, &self.policy.reference)?
                || !contains_value(authority, "competence_refs", field(a, "competence")?)?
                || !contains_text(authority, "decisions", text(a, "decision")?)?
                || !contains_text(authority, "uses", use_)?
                || !strings(authority, "subject_prefixes")?
                    .iter()
                    .any(|prefix| context.subject.id.starts_with(prefix)),
            "authority.binding-or-scope"
        );
        for (prefix, record, state) in [
            ("authority", authority, "active"),
            ("competence", calibration, "verified"),
        ] {
            work.tick(1)?;
            reason!(
                text(record, "state")? != state,
                &format!("{prefix}.inactive")
            );
            reason!(
                compare_time(text(record, "valid_from")?, issued)? == Ordering::Greater
                    || compare_time(issued, now)? == Ordering::Greater
                    || compare_time(now, text(record, "valid_until")?)? != Ordering::Less,
                &format!("{prefix}.outside-validity")
            );
            reason!(
                text(record, "actor_id")? != actor
                    || !contains_text(record, "profile_ids", text(a, "profile_id")?)?
                    || !contains_text(record, "assertion_layers", layer)?
                    || !languages_fit(&languages, &strings(record, "languages")?, work)?,
                &format!("{prefix}.outside-scope")
            );
        }
        let mut evidence_stale = false;
        for reference in array(calibration, "evidence_refs")? {
            work.tick(1)?;
            evidence_stale |= resolve(reference, &self.records)?.is_none();
        }
        reason!(evidence_stale, "competence.evidence-stale");
        let method = field(a, "method")?;
        let executor = resolve(&submission.executor, &self.records)?;
        let qualified_execution = if let Some(executor) = executor {
            py_equal(field(method, "execution_profile")?, &submission.executor)?
                && contains_value(calibration, "execution_profiles", &submission.executor)?
                && py_equal(
                    field(method, "procedure_ref")?,
                    executor.body.get("procedure_ref").unwrap_or(&Value::Null),
                )?
                && py_equal(
                    field(method, "model_ref")?,
                    executor.body.get("model_ref").unwrap_or(&Value::Null),
                )?
        } else {
            false
        };
        reason!(!qualified_execution, "method.unqualified-execution");
        let evidence = array(a, "evidence")?;
        let mut evidence_refs = BTreeSet::new();
        for item in evidence {
            work.tick(1)?;
            evidence_refs.insert(canonical(field(item, "record")?)?);
        }
        for dependency in context.required.iter().chain(&context.admissions) {
            work.tick(1)?;
            reason!(
                resolve(&dependency.reference, &self.records)?.is_none(),
                "source-dependency.stale-or-missing"
            );
            reason!(
                !evidence_refs.contains(&canonical(&dependency.reference)?),
                "evidence.required-source-omitted"
            );
        }
        let positive = is_positive(text(a, "decision")?);
        reason!(
            positive
                && (!context.positive
                    || context.admissions.iter().any(|r| r
                        .body
                        .get("can_use")
                        .and_then(Value::as_bool)
                        != Some(true))),
            "source-quality.not-admitted"
        );
        let mut supporting = Vec::new();
        for item in evidence {
            work.tick(1)?;
            match resolve(field(item, "record")?, &self.records)? {
                None => {
                    reasons.insert("evidence.stale-or-missing".into());
                }
                Some(record) if text(item, "stance")? == "supports" => {
                    reason!(record.id == context.subject.id, "evidence.circular-support");
                    reason!(record.origin.is_none(), "evidence.origin-missing");
                    supporting.push(record);
                }
                _ => (),
            }
        }
        if positive {
            reason!(
                !meets(
                    support_count(&supporting, work)?,
                    field(profile, "min_supporting_origins")?
                )?,
                "evidence.insufficient-origins"
            );
            reason!(
                boolean(profile, "require_counterevidence_search")?
                    && text(field(a, "counterevidence_search")?, "status")? != "searched",
                "evidence.countersearch-required"
            );
            if use_ == manual_authoring::USE {
                if let Some(reason) = manual_authoring::qualify(context, a, work)? {
                    reasons.insert(reason);
                }
            }
        }
        Ok(reasons.into_iter().collect())
    }

    fn evaluate(
        &self,
        context: &Context,
        reviews: &[Submission],
        trusted_history: &[Submission],
        now: &str,
        work: &mut Work<'_>,
    ) -> Result<Value> {
        let mut history = BTreeMap::<String, &Submission>::new();
        let mut refs = BTreeMap::<String, Value>::new();
        for s in trusted_history {
            work.tick(1)?;
            let a = &s.body;
            if !s.schema_valid {
                return invalid("trusted history assessment schema");
            }
            if text(field(a, "subject")?, "id")? != context.subject.id
                || text(field(a, "reviewer")?, "actor_id")? != s.principal
                || !py_equal(
                    field(field(a, "method")?, "execution_profile")?,
                    &s.executor,
                )?
                || compare_time(text(a, "issued_at")?, now)? == Ordering::Greater
            {
                return invalid("trusted history has an inconsistent binding");
            }
            let id = text(a, "assessment_id")?.to_owned();
            let reference = Record::assessment(&id, a)?;
            if refs
                .get(&id)
                .map(|r| py_equal(r, &reference))
                .transpose()?
                .is_some_and(|same| !same)
            {
                return invalid("trusted history identity collision");
            }
            history.insert(id.clone(), s);
            refs.insert(id, reference);
        }
        let mut permanent = BTreeSet::new();
        for s in history.values() {
            for reference in array(&s.body, "supersedes")? {
                work.tick(1)?;
                let target = text(reference, "id")?;
                let Some(previous) = history.get(target) else {
                    return invalid("trusted history missing exact supersession target");
                };
                if !refs
                    .get(target)
                    .map(|r| py_equal(r, reference))
                    .transpose()?
                    .unwrap_or(false)
                {
                    return invalid("trusted history missing exact supersession target");
                }
                if submission_scope_matches(s, context)?
                    && submission_scope_matches(previous, context)?
                {
                    permanent.insert(target.to_owned());
                }
            }
        }
        let mut grouped = BTreeMap::<String, Vec<&Submission>>::new();
        for s in trusted_history.iter().chain(reviews) {
            let id = s
                .body
                .get("assessment_id")
                .and_then(Value::as_str)
                .unwrap_or("<invalid-id>");
            grouped.entry(id.to_owned()).or_default().push(s);
        }
        let mut qualified = BTreeMap::<String, &Submission>::new();
        let mut invalids = BTreeMap::<String, Vec<String>>::new();
        for (id, variants) in grouped {
            work.tick(1)?;
            let mut signatures = BTreeSet::new();
            for s in &variants {
                work.tick(1)?;
                signatures.insert(canonical(&json!({"assessment":s.body,"principal_id":s.principal,"execution_profile":s.executor}))?);
            }
            let failure = if signatures.iter().any(|s| s.len() > MAX_RECORD_BYTES) {
                Some("assessment.size-limit")
            } else if signatures.len() != 1 {
                Some("assessment.identity-collision")
            } else {
                None
            };
            if let Some(failure) = failure {
                invalids.insert(id, vec![failure.into()]);
                continue;
            }
            let s = variants[0];
            let reasons = self.qualify(s, context, now, work)?;
            if !reasons.is_empty() {
                invalids.insert(id, reasons);
            } else {
                refs.insert(id.clone(), Record::assessment(&id, &s.body)?);
                qualified.insert(id, s);
            }
        }
        let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
        for (id, s) in &qualified {
            let deps = dependencies.entry(id.clone()).or_default();
            for reference in array(&s.body, "supersedes")? {
                work.tick(1)?;
                let target = text(reference, "id")?;
                let previous = qualified.get(target).or_else(|| history.get(target));
                let exact_target = refs
                    .get(target)
                    .map(|r| py_equal(r, reference))
                    .transpose()?
                    .unwrap_or(false);
                let valid = if let Some(previous) = previous {
                    exact_target
                        && target != id
                        && py_equal(
                            field(&previous.body, "subject")?,
                            field(&s.body, "subject")?,
                        )?
                        && compare_time(
                            text(&previous.body, "issued_at")?,
                            text(&s.body, "issued_at")?,
                        )? != Ordering::Greater
                } else {
                    false
                };
                if !valid {
                    invalids
                        .entry(id.clone())
                        .or_default()
                        .push("supersession.invalid-target".into());
                    continue;
                }
                let previous = previous.expect("checked target");
                if !submission_scope_matches(previous, context)? {
                    invalids
                        .entry(id.clone())
                        .or_default()
                        .push("supersession.outside-committed-scope".into());
                }
                let authority = &self.authorities[text(field(&s.body, "authority")?, "id")?].body;
                if previous.principal != s.principal && !boolean(authority, "can_supersede_others")?
                {
                    invalids
                        .entry(id.clone())
                        .or_default()
                        .push("supersession.unauthorized".into());
                }
                deps.insert(target.to_owned());
            }
        }
        let mut remaining: BTreeSet<String> = qualified
            .keys()
            .filter(|id| !invalids.contains_key(*id))
            .cloned()
            .collect();
        let mut ordered = Vec::new();
        while !remaining.is_empty() {
            work.tick(1)?;
            let mut children = BTreeSet::new();
            for id in &remaining {
                for target in &dependencies[id] {
                    work.tick(1)?;
                    if invalids.contains_key(target) && !history.contains_key(target) {
                        children.insert(id.clone());
                    }
                }
            }
            for id in children {
                remaining.remove(&id);
                invalids.insert(id, vec!["supersession.invalid-target".into()]);
            }
            let mut ready = Vec::new();
            for id in &remaining {
                let mut waiting = false;
                for target in &dependencies[id] {
                    work.tick(1)?;
                    waiting |= remaining.contains(target);
                }
                if !waiting {
                    ready.push(id.clone());
                }
            }
            if ready.is_empty() {
                for id in &remaining {
                    invalids.insert(
                        id.clone(),
                        vec!["supersession.cycle-or-invalid-ancestor".into()],
                    );
                }
                break;
            }
            for id in ready {
                remaining.remove(&id);
                ordered.push(id);
            }
        }
        let mut superseded = permanent;
        for id in &ordered {
            superseded.extend(dependencies[id].iter().cloned());
        }
        let active: Vec<&Submission> = ordered
            .iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|id| !superseded.contains(*id))
            .map(|id| qualified[id])
            .collect();
        let judgments: Vec<&Submission> = active
            .iter()
            .copied()
            .filter(|s| s.body["decision"] != "withdraw")
            .collect();
        let positives: Vec<&Submission> = judgments
            .iter()
            .copied()
            .filter(|s| is_positive(s.body["decision"].as_str().unwrap_or("")))
            .collect();
        let negatives = judgments.iter().any(|s| s.body["decision"] == "reject");
        let disputes = judgments.iter().any(|s| s.body["decision"] == "dispute");
        let mut quorum = false;
        for (id, profile) in &self.profiles {
            let mut options = BTreeMap::<String, BTreeSet<String>>::new();
            for s in &positives {
                work.tick(1)?;
                if text(&s.body, "profile_id")? == id {
                    let group = text(
                        &self.authorities[text(field(&s.body, "authority")?, "id")?].body,
                        "independence_group",
                    )?;
                    options
                        .entry(s.principal.clone())
                        .or_default()
                        .insert(group.to_owned());
                }
            }
            if meets(options.len(), field(profile, "min_reviewers")?)?
                && meets(
                    independent_seats(&options, work)?,
                    field(profile, "min_independence_groups")?,
                )?
            {
                quorum = true;
            }
        }
        let mut limits = BTreeSet::new();
        let mut kinds = BTreeSet::new();
        for s in &judgments {
            work.tick(1)?;
            limits.extend(strings(&s.body, "limits")?);
            kinds.insert(text(field(&s.body, "reviewer")?, "kind")?.to_owned());
        }
        for dependency in &context.admissions {
            limits.extend(strings(&dependency.body, "limits")?);
        }
        let status = if disputes || (!positives.is_empty() && negatives) {
            "disputed"
        } else if negatives {
            "rejected"
        } else if quorum {
            if limits.is_empty() {
                "admitted"
            } else {
                "admitted-with-limits"
            }
        } else if !judgments.is_empty() {
            "deferred"
        } else {
            "unreviewed"
        };
        let active_refs: Vec<Value> = active
            .iter()
            .map(|s| refs[s.body["assessment_id"].as_str().expect("qualified id")].clone())
            .collect();
        let superseded_refs: Vec<Value> = superseded.iter().map(|id| refs[id].clone()).collect();
        let invalids:Vec<Value> = invalids.into_iter().map(|(id,reasons)| json!({"assessment_id":id,"reasons":reasons.into_iter().collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>()})).collect();
        Ok(
            json!({"schema_version":"tos_knowledge_admission_v1","subject":context.subject.reference,
            "policy":self.policy.reference,"use":text(&context.scope,"requested_use")?,"status":status,
            "can_use":matches!(status,"admitted"|"admitted-with-limits"),"is_semantic_evaluation":false,
            "reviewer_kinds":kinds.into_iter().collect::<Vec<_>>(),"assessment_refs":active_refs,
            "superseded_assessment_refs":superseded_refs,"invalid_assessments":invalids,"limits":limits.into_iter().collect::<Vec<_>>() }),
        )
    }
}

// Kept inside this already fingerprinted owner source: every existing source
// grounding rule binds assessment.rs. No unbound executable child file.
mod manual_authoring {
    // Use-specific manual-authoring admission over already selected owner records.
    // No source reader, grant, completeness flag, identity decision or truth oracle.
    // Legacy uses and negative/deferred/withdrawal judgments retain their contract.

    use super::{AssessmentRefusal, Context, Record, Result, Work, invalid};
    use serde_json::Value;

    pub(super) const USE: &str = "registry-manual-authoring-v1";
    const VERSION: &str = "tos_manual_authoring_basis_v1";

    // New comparisons must be admitted BEFORE reading key/string contents. Values
    // are borrowed from the original bounded owner snapshot; no cloned source tree.
    fn equal(a: &str, b: &str, work: &mut Work<'_>) -> Result<bool> {
        work.tick(a.len().checked_add(b.len()).ok_or(AssessmentRefusal::Budget)?)?;
        Ok(a == b)
    }
    fn get<'a>(value: &'a Value, key: &str, work: &mut Work<'_>) -> Result<&'a Value> {
        optional(value, key, work)?.ok_or_else(|| super::bad("manual-authoring.required-field-missing"))
    }
    fn optional<'a>(value: &'a Value, key: &str, work: &mut Work<'_>) -> Result<Option<&'a Value>> {
        work.tick(1)?;
        let Some(object) = value.as_object() else {
            return invalid("manual-authoring.object-required");
        };
        for (name, value) in object {
            work.tick(1)?;
            if equal(name, key, work)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }
    fn closed(value: &Value, keys: &[&str], work: &mut Work<'_>) -> Result<()> {
        work.tick(1)?;
        let Some(object) = value.as_object() else {
            return invalid("manual-authoring.object-required");
        };
        if object.len() != keys.len() {
            return invalid("manual-authoring.closed-fields");
        }
        for key in keys {
            get(value, key, work)?;
        }
        Ok(())
    }
    fn text<'a>(value: &'a Value, work: &mut Work<'_>) -> Result<&'a str> {
        work.tick(1)?;
        let Some(value) = value.as_str() else {
            return invalid("manual-authoring.text-required");
        };
        work.tick(value.len())?;
        if value.trim().is_empty() {
            return invalid("manual-authoring.empty-rationale-or-locator");
        }
        Ok(value)
    }
    fn items<'a>(value: &'a Value, nonempty: bool, work: &mut Work<'_>) -> Result<&'a [Value]> {
        work.tick(1)?;
        let Some(values) = value.as_array() else {
            return invalid("manual-authoring.array-required");
        };
        // Same finite upper bound as the existing assessment evidence envelope;
        // this is a resource ceiling, never a source quorum or sufficiency rule.
        if values.len() > 256 || nonempty && values.is_empty() {
            return invalid("manual-authoring.array-shape");
        }
        Ok(values)
    }
    fn numeric_equal(a: &Value, b: &Value, work: &mut Work<'_>) -> Result<bool> {
        let (Some(a_number), Some(b_number)) = (a.as_number(), b.as_number()) else {
            return invalid("manual-authoring.numeric-reference");
        };
        let a_raw = a_number.as_str();
        let b_raw = b_number.as_str();
        let bytes = a_raw.len().checked_add(b_raw.len()).ok_or(AssessmentRefusal::Budget)?;
        work.tick(bytes.checked_mul(8).ok_or(AssessmentRefusal::Budget)?)?;
        let a_int = !a_raw.bytes().any(|b| matches!(b, b'.' | b'e' | b'E'));
        let b_int = !b_raw.bytes().any(|b| matches!(b, b'.' | b'e' | b'E'));
        if a_int && b_int {
            return equal(a_raw, b_raw, work);
        }
        // Preserve the maintained exact-reference numeric equality, including
        // 1 == 1.0; do not impose a new u64 bound or round large integer versions.
        // Float-to-integer expansion is bounded by IEEE binary64: at most309
        // decimal digits and971 doubling passes. Each pass visits/copies at
        // most320digits twice. Derive each bound from the actual binary exponent
        // BEFORE the legacy helper; small integral floats do not pay the
        // maximum-size reservation. Token parse/copy passes were paid above.
        for (number, integer) in [(a_number, a_int), (b_number, b_int)] {
            if !integer {
                let value = number.as_f64().ok_or_else(|| super::unsupported("manual-authoring.numeric-representation"))?;
                if !value.is_finite() {
                    return Err(super::unsupported("manual-authoring.nonfinite-number"));
                }
                let exponent = ((value.to_bits() >> 52) & 0x7ff) as i32;
                let passes = (exponent - 1023 - 52).max(0) as usize;
                work.tick((passes + 1) * 640 + 64)?;
            }
        }
        super::py_equal(a, b)
    }
    fn exact<'a>(value: &'a Value, work: &mut Work<'_>) -> Result<(&'a str, &'a Value, &'a str)> {
        closed(value, &["id", "version", "digest"], work)?;
        let id = text(get(value, "id", work)?, work)?;
        let version = get(value, "version", work)?;
        let Some(number) = version.as_number() else {
            return invalid("manual-authoring.exact-reference-version");
        };
        let lexeme = number.as_str();
        work.tick(lexeme.len().checked_mul(4).ok_or(AssessmentRefusal::Budget)?)?;
        let integral = if lexeme.bytes().all(|b| b.is_ascii_digit()) {
            lexeme != "0"
        } else {
            // ExactRef uses the maintained schema/Python numeric interpretation.
            // The actual selected owner record still supplies a positive Int.
            number.as_f64().is_some_and(|n| n.is_finite() && n > 0.0 && n.fract() == 0.0)
        };
        let digest = text(get(value, "digest", work)?, work)?;
        work.tick(digest.len())?;
        if !integral
            || digest.len() != 71
            || !digest.starts_with("sha256:")
            || !digest.as_bytes()[7..].iter().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
        {
            return invalid("manual-authoring.exact-reference");
        }
        Ok((id, version, digest))
    }
    fn same_reference(a: &Value, b: &Value, work: &mut Work<'_>) -> Result<bool> {
        let a = exact(a, work)?;
        let b = exact(b, work)?;
        Ok(equal(a.0, b.0, work)? && numeric_equal(a.1, b.1, work)? && equal(a.2, b.2, work)?)
    }
    fn selected<'a>(reference: &Value, context: &'a Context, work: &mut Work<'_>) -> Result<&'a Record> {
        exact(reference, work)?;
        for record in &context.required {
            work.tick(1)?;
            if same_reference(reference, &record.reference, work)? {
                return Ok(record);
            }
        }
        invalid("manual-authoring.evidence-outside-owner-required-closure")
    }
    fn citation<'a>(value: &Value, context: &'a Context, assessment: &Value, work: &mut Work<'_>) -> Result<&'a Record> {
        closed(value, &["record", "locator"], work)?;
        let reference = get(value, "record", work)?;
        let locator = text(get(value, "locator", work)?, work)?;
        let selected = selected(reference, context, work)?;
        work.tick(locator.len())?;
        let pointer = locator.strip_prefix("json-pointer:")
            .filter(|pointer| pointer.starts_with('/'))
            .ok_or_else(|| super::bad("manual-authoring.locator-adapter-not-installed"))?;
        // This first registered adapter reads an actual field of the exact
        // selected JSON record. Matching two submitted free strings is not
        // evidence that their locator exists. Other representation locators
        // require their own source-owner adapter; no guessed page/cell fallback.
        pointer_at(&selected.body, pointer, work)?;
        for evidence in items(get(assessment, "evidence", work)?, false, work)? {
            work.tick(1)?;
            if same_reference(reference, get(evidence, "record", work)?, work)?
                && equal(locator, text(get(evidence, "locator", work)?, work)?, work)?
            {
                return Ok(selected);
            }
        }
        invalid("manual-authoring.exact-citation-not-assessed")
    }

    fn segment_equal(key: &str, encoded: &str, work: &mut Work<'_>) -> Result<bool> {
        work.tick(key.len().checked_add(encoded.len()).ok_or(AssessmentRefusal::Budget)?)?;
        let mut key = key.bytes();
        let mut encoded = encoded.bytes();
        while let Some(byte) = encoded.next() {
            let decoded = if byte == b'~' {
                match encoded.next() {
                    Some(b'0') => b'~',
                    Some(b'1') => b'/',
                    _ => return invalid("manual-authoring.value-pointer-escape"),
                }
            } else {
                byte
            };
            if key.next() != Some(decoded) {
                return Ok(false);
            }
        }
        Ok(key.next().is_none())
    }
    fn value_at<'a>(claim: &'a Value, pointer: &str, work: &mut Work<'_>) -> Result<&'a Value> {
        work.tick(pointer.len())?;
        if pointer != "/object" && !pointer.starts_with("/object/") {
            return invalid("manual-authoring.question-must-be-claim-local");
        }
        pointer_at(claim, pointer, work)
    }
    fn pointer_at<'a>(root: &'a Value, pointer: &str, work: &mut Work<'_>) -> Result<&'a Value> {
        work.tick(pointer.len())?;
        if !pointer.starts_with('/') { return invalid("manual-authoring.value-pointer-root"); }
        let mut value = root;
        for segment in pointer.split('/').skip(1) {
            work.tick(1)?;
            value = match value {
                Value::Object(object) => {
                    let mut found = None;
                    for (key, child) in object {
                        work.tick(1)?;
                        if segment_equal(key, segment, work)? {
                            found = Some(child);
                            break;
                        }
                    }
                    found.ok_or_else(|| super::bad("manual-authoring.value-pointer-missing"))?
                }
                Value::Array(array) => {
                    work.tick(segment.len())?;
                    if segment.is_empty()
                        || segment.len() > 1 && segment.starts_with('0')
                        || !segment.bytes().all(|b| b.is_ascii_digit())
                    {
                        return invalid("manual-authoring.value-pointer-index");
                    }
                    let index = segment.parse::<usize>().map_err(|_| AssessmentRefusal::Budget)?;
                    array.get(index).ok_or_else(|| super::bad("manual-authoring.value-pointer-missing"))?
                }
                _ => return invalid("manual-authoring.value-pointer-missing"),
            };
        }
        Ok(value)
    }
    fn values_equal(a: &Value, b: &Value, work: &mut Work<'_>) -> Result<bool> {
        work.tick(1)?;
        match (a, b) {
            (Value::String(a), Value::String(b)) => equal(a, b, work),
            (Value::Number(_), Value::Number(_)) => {
                numeric_equal(a, b, work)
            }
            (Value::Bool(a), Value::Bool(b)) => Ok(a == b),
            (Value::Null, Value::Null) => Ok(true),
            (Value::Array(a), Value::Array(b)) => {
                if a.len() != b.len() { return Ok(false); }
                for (a, b) in a.iter().zip(b) {
                    if !values_equal(a, b, work)? { return Ok(false); }
                }
                Ok(true)
            }
            (Value::Object(a), Value::Object(b)) => {
                if a.len() != b.len() { return Ok(false); }
                for (key, a) in a {
                    let mut found = None;
                    for (other, b) in b {
                        work.tick(1)?;
                        if equal(key, other, work)? { found = Some(b); break; }
                    }
                    let Some(b) = found else { return Ok(false); };
                    if !values_equal(a, b, work)? { return Ok(false); }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn check(context: &Context, assessment: &Value, work: &mut Work<'_>) -> Result<()> {
        let claim = &context.subject.body;
        if !equal(text(get(claim, "claim_id", work)?, work)?, &context.subject.id, work)?
            || !numeric_equal(get(claim, "claim_version", work)?, &context.subject.version, work)?
        {
            return invalid("manual-authoring.claim-envelope-mismatch");
        }
        let basis = get(get(claim, "qualifiers", work)?, "manual_authoring", work)?;
        let version = text(get(basis, "schema_version", work)?, work)?;
        if !equal(version, VERSION, work)? { return invalid("manual-authoring.basis-version"); }
        let question = get(basis, "question", work)?;
        closed(question, &["subject", "predicate", "value_path"], work)?;
        let subject = get(question, "subject", work)?;
        let subject_record = selected(subject, context, work)?;
        if !equal(&subject_record.id, text(get(claim, "subject_ref", work)?, work)?, work)?
            || !equal(text(get(question, "predicate", work)?, work)?, text(get(claim, "predicate", work)?, work)?, work)?
        { return invalid("manual-authoring.question-mismatch"); }
        let pointer = text(get(question, "value_path", work)?, work)?;
        value_at(claim, pointer, work)?;
        let finding = text(get(basis, "finding", work)?, work)?;
        let status = text(get(claim, "epistemic_status", work)?, work)?;
        // New manual-use qualification cannot label an explicitly uncertain source
        // assertion determinate. Reported source-unknown is still a scoped report;
        // its actual meaning/adequacy is the competent reviewer's judgment.
        let mut source_unresolved = equal(status, "uncertain", work)? || equal(status, "disputed", work)?;
        if let Some(polarity) = optional(claim, "polarity", work)? {
            source_unresolved |= equal(text(polarity, work)?, "unknown", work)?;
        }
        if equal(finding, "determinate", work)? {
            closed(basis, &["schema_version", "question", "finding"], work)?;
            if source_unresolved { return invalid("manual-authoring.unresolved-source-as-determinate"); }
            return Ok(());
        }
        if !equal(finding, "unresolved", work)? { return invalid("manual-authoring.finding"); }
        closed(basis, &["schema_version", "question", "finding", "grounds"], work)?;
        let grounds = get(basis, "grounds", work)?;
        closed(grounds, &["reason", "rationale", "read_extent", "alternatives", "residual", "resolution", "details"], work)?;
        text(get(grounds, "rationale", work)?, work)?;
        text(get(grounds, "residual", work)?, work)?;
        let resolution = get(grounds, "resolution", work)?;
        closed(resolution, &["kind", "statement"], work)?;
        let kind = text(get(resolution, "kind", work)?, work)?;
        if !equal(kind, "discriminator", work)? && !equal(kind, "objective-boundary", work)? {
            return invalid("manual-authoring.resolution-kind");
        }
        text(get(resolution, "statement", work)?, work)?;
        let extent = get(grounds, "read_extent", work)?;
        closed(extent, &["sources", "coverage_basis", "exclusions"], work)?;
        citation(get(extent, "coverage_basis", work)?, context, assessment, work)?;
        for source in items(get(extent, "sources", work)?, true, work)? {
            work.tick(1)?;
            citation(source, context, assessment, work)?;
        }
        for excluded in items(get(extent, "exclusions", work)?, false, work)? {
            work.tick(1)?;
            text(excluded, work)?;
        }
        let alternatives = items(get(grounds, "alternatives", work)?, false, work)?;
        let reason = text(get(grounds, "reason", work)?, work)?;
        let details = get(grounds, "details", work)?;
        if equal(reason, "source-contradiction", work)? {
            closed(details, &["criterion", "scope", "incompatibility"], work)?;
            citation(get(details, "criterion", work)?, context, assessment, work)?;
            citation(get(details, "scope", work)?, context, assessment, work)?;
            text(get(details, "incompatibility", work)?, work)?;
            if alternatives.len() < 2 { return invalid("manual-authoring.conflict-needs-alternatives"); }
            // Nonidentical source values are a necessary structural condition,
            // not a proof of incompatibility (coauthors/ranges may coexist).
            // The exact criterion/scope and competent assessment own that
            // substantive judgment; this gate creates no universal logic.
            let mut different = false;
            for (i, alternative) in alternatives.iter().enumerate() {
                work.tick(1)?;
                let a = citation(alternative, context, assessment, work)?;
                if !equal(text(get(&a.body, "claim_id", work)?, work)?, &a.id, work)?
                    || !numeric_equal(get(&a.body, "claim_version", work)?, &a.version, work)?
                    || !equal(text(get(&a.body, "subject_ref", work)?, work)?, &subject_record.id, work)?
                    || !equal(text(get(&a.body, "predicate", work)?, work)?, text(get(claim, "predicate", work)?, work)?, work)?
                { return invalid("manual-authoring.alternative-question-mismatch"); }
                let av = value_at(&a.body, pointer, work)?;
                for other in &alternatives[..i] {
                    work.tick(1)?;
                    let b = citation(other, context, assessment, work)?;
                    if same_reference(&a.reference, &b.reference, work)? {
                        return invalid("manual-authoring.repeated-alternative");
                    }
                    different |= !values_equal(av, value_at(&b.body, pointer, work)?, work)?;
                }
            }
            if !different { return invalid("manual-authoring.conflict-with-identical-values"); }
        } else if equal(reason, "scoped-information-absence", work)? {
            closed(details, &["missing_at", "extent_justification"], work)?;
            if !equal(text(get(details, "missing_at", work)?, work)?, pointer, work)? {
                return invalid("manual-authoring.absence-question-mismatch");
            }
            text(get(details, "extent_justification", work)?, work)?;
            for alternative in alternatives { citation(alternative, context, assessment, work)?; }
        } else if equal(reason, "unestablished-identity", work)? {
            // Criterion/comparison evidence belongs to this reason, not to a
            // universal name/string equality heuristic or an invented native role.
            closed(details, &["identity_level", "criterion", "comparisons", "non_joins"], work)?;
            text(get(details, "identity_level", work)?, work)?;
            citation(get(details, "criterion", work)?, context, assessment, work)?;
            for comparison in items(get(details, "comparisons", work)?, true, work)? {
                work.tick(1)?;
                citation(comparison, context, assessment, work)?;
            }
            for non_join in items(get(details, "non_joins", work)?, false, work)? {
                work.tick(1)?;
                citation(non_join, context, assessment, work)?;
            }
            for candidate in alternatives { citation(candidate, context, assessment, work)?; }
        } else { return invalid("manual-authoring.pending-or-unsupported-reason"); }
        Ok(())
    }

    pub(super) fn qualify(context: &Context, assessment: &Value, work: &mut Work<'_>) -> Result<Option<String>> {
        // Only Engine::qualify calls this, for the exact protected requested use
        // and positive decisions. Existing use branches incur no new traversal.
        match check(context, assessment, work) {
            Ok(()) => Ok(None),
            Err(AssessmentRefusal::InvalidInput(reason)) => Ok(Some(reason)),
            Err(error) => Err(error),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::assessment::{AssessmentLimits, AssessmentRecordInput};
        use crate::executor::BatchBudget;
        use serde_json::json;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::{Duration, Instant};

        fn record(id: &str, body: Value, work: &mut Work<'_>) -> Record {
            // Pure value fixtures have no native IDs, authority/grant/cut or
            // complete/readiness evidence. They cannot enter a source schema.
            Record::read(&AssessmentRecordInput {
                envelope: serde_json::to_vec(&json!({"id":id,"version":1,
                    "payload":body,"origin_id":"fixture:origin"})).unwrap(),
            }, work).unwrap()
        }
        fn cite(record: &Record) -> Value {
            json!({"record":record.reference,"locator":"json-pointer:/evidence"})
        }
        #[test]
        fn reason_specific_basis_refuses_foreign_empty_pending_and_spent_prefixes() {
            let cancelled = AtomicBool::new(false);
            let mut work = Work {
                limits: AssessmentLimits {
                    max_input_bytes: 1_048_576,
                    max_work: 20_000_000,
                    batch: BatchBudget::laboratory(),
                    deadline: Instant::now() + Duration::from_secs(30),
                },
                cancelled: &cancelled,
                bytes: 0,
                work: 0,
            };
            let metadata = record("fixture:work", json!({"record_type":"work","evidence":"Actual source field."}), &mut work);
            let scope = record("fixture:extent", json!({"scope":"bounded source field","evidence":"Declared extent."}), &mut work);
            let criterion = record("fixture:criterion", json!({"criterion":"source-owned","evidence":"Comparison criterion."}), &mut work);
            let a = record("fixture:a", json!({"claim_id":"fixture:a","claim_version":1,"subject_ref":"fixture:work",
                "predicate":"reported_date","object":{"date":"A"},"evidence":"Source reports A."}), &mut work);
            let b = record("fixture:b", json!({"claim_id":"fixture:b","claim_version":1,"subject_ref":"fixture:work",
                "predicate":"reported_date","object":{"date":"B"},"evidence":"Source reports B."}), &mut work);
            let mut body = json!({"claim_id":"fixture:target","claim_version":1,"subject_ref":"fixture:work",
                "predicate":"reported_date","object":{"date":"source reports no date"},
                "epistemic_status":"uncertain","qualifiers":{"manual_authoring":{
                    "schema_version":VERSION,"finding":"unresolved",
                    "question":{"subject":metadata.reference,"predicate":"reported_date",
                        "value_path":"/object/date"},
                    "grounds":{"reason":"scoped-information-absence",
                        "rationale":"The declared source field omits a date.",
                        "residual":"Historical date not established in this source extent.",
                        "resolution":{"kind":"objective-boundary",
                            "statement":"The selected source supplies no date evidence."},
                        "read_extent":{"sources":[cite(&metadata)],"coverage_basis":cite(&scope),
                            "exclusions":["Other sources have not been examined."]},
                        "alternatives":[],"details":{"missing_at":"/object/date",
                            "extent_justification":"Extent is limited to this source field."}}}}});
            let evidence = json!({"evidence":[cite(&metadata),cite(&scope),cite(&criterion),cite(&a),cite(&b)]});
            let mut context = Context {
                subject: record("fixture:target", body.clone(), &mut work),
                scope: json!({}), languages: Vec::new(),
                required: vec![metadata, scope, criterion, a, b], admissions: Vec::new(),
                source_ready: false, positive: false,
            };
            assert!(check(&context, &evidence, &mut work).is_ok());
            body["qualifiers"]["manual_authoring"]["grounds"]["rationale"] = json!(" \n\t");
            context.subject = record("fixture:target", body.clone(), &mut work);
            let spent = work.work;
            assert_eq!(qualify(&context, &evidence, &mut work).unwrap().as_deref(), Some("manual-authoring.empty-rationale-or-locator"));
            assert!(work.work > spent);
            body["qualifiers"]["manual_authoring"]["grounds"]["rationale"] = json!("Bounded source report.");
            body["qualifiers"]["manual_authoring"]["grounds"]["reason"] = json!("not-yet-reviewed");
            context.subject = record("fixture:target", body.clone(), &mut work);
            assert!(check(&context, &evidence, &mut work).is_err());
            body["qualifiers"]["manual_authoring"]["grounds"]["reason"] = json!("scoped-information-absence");
            body["qualifiers"]["manual_authoring"]["grounds"]["read_extent"]["sources"][0]["locator"] = json!("json-pointer:/absent");
            context.subject = record("fixture:target", body.clone(), &mut work);
            let mut irrelevant = evidence.clone();
            irrelevant["evidence"][0]["locator"] = json!("json-pointer:/absent");
            assert!(check(&context, &irrelevant, &mut work).is_err());
            body["qualifiers"]["manual_authoring"]["grounds"]["read_extent"]["sources"][0]["locator"] = json!("json-pointer:/evidence");
            let grounds = &mut body["qualifiers"]["manual_authoring"]["grounds"];
            grounds["reason"] = json!("source-contradiction");
            grounds["details"] = json!({"criterion":cite(&context.required[2]),
                "scope":cite(&context.required[1]),"incompatibility":"Qualified source comparison is required."});
            grounds["alternatives"] = json!([cite(&context.required[3]),cite(&context.required[4])]);
            context.subject = record("fixture:target", body.clone(), &mut work);
            assert!(check(&context, &evidence, &mut work).is_ok());
            body["qualifiers"]["manual_authoring"]["grounds"]["alternatives"] = json!([cite(&context.required[3]),cite(&context.required[3])]);
            context.subject = record("fixture:target", body.clone(), &mut work);
            assert!(check(&context, &evidence, &mut work).is_err());
            let grounds = &mut body["qualifiers"]["manual_authoring"]["grounds"];
            grounds["reason"] = json!("unestablished-identity");
            grounds["alternatives"] = json!([]);
            grounds["details"] = json!({"identity_level":"fixture:work-level",
                "criterion":cite(&context.required[2]),"comparisons":[cite(&context.required[3])],"non_joins":[]});
            context.subject = record("fixture:target", body, &mut work);
            assert!(check(&context, &evidence, &mut work).is_ok());
            let mut determinate = context.subject.body.clone();
            determinate["epistemic_status"] = json!("reported");
            let question = determinate["qualifiers"]["manual_authoring"]["question"].clone();
            determinate["qualifiers"]["manual_authoring"] = json!({
                "schema_version":VERSION,"finding":"determinate","question":question});
            context.subject = record("fixture:target", determinate.clone(), &mut work);
            assert!(check(&context, &evidence, &mut work).is_ok());
            determinate["polarity"] = json!("unknown");
            context.subject = record("fixture:target", determinate, &mut work);
            assert!(check(&context, &evidence, &mut work).is_err());
            let mut floating = context.required[0].reference.clone();
            floating["version"] = json!(1.0);
            assert!(same_reference(&floating, &context.required[0].reference, &mut work).unwrap());
            work.limits.max_work = work.work + 2;
            let key = "p".repeat(8192) + "A";
            let query = "p".repeat(8192) + "B";
            let value = Value::Object(serde_json::Map::from_iter([(key, json!("value"))]));
            assert_eq!(get(&value, &query, &mut work), Err(AssessmentRefusal::Budget));
            assert_eq!(work.work, work.limits.max_work);
            cancelled.store(true, Ordering::Relaxed);
            assert!(matches!(qualify(&context, &evidence, &mut work), Err(AssessmentRefusal::Cancelled)));
        }
    }
}
