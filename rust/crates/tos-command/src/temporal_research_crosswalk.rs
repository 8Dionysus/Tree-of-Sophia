//! Private C01 interface accounting. No converter, permission or transport.
//! Adapter-owned requested capacity only; delegated kernel transients are not
//! represented as V11-bounded. Every row is finite and spent without refund.
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use std::mem::{align_of, size_of, size_of_val};
use std::path::PathBuf;
use tos_foundation::{JsonString, JsonValue, RelativePath};

macro_rules! c01_each {
    ($counter:expr, $input:expr, $pattern:pat, $body:block) => {{
        $counter.visit(4)?;
        let mut cursor = ($input).into_iter();
        loop {
            $counter.visit(4)?;
            let Some($pattern) = cursor.next() else {
                break;
            };
            $body
        }
    }};
}
pub(crate) use c01_each;

#[derive(Clone, Copy)]
#[repr(usize)]
pub(crate) enum C01Row {
    Pure,
    Owner,
    Context,
    Sources,
    ReadBatch,
    Plan,
    History,
    Result,
    AdapterNew,
    PublicOutput,
}
const CALLS: [u8; 10] = [6, 1, 1, 1, 2, 2, 2, 2, 1, 1];
const OWNED: [usize; 10] = [
    524288, 524288, 2097152, 4194304, 65536, 1048576, 262144, 131072, 1048576, 524288,
];
const VISITS: [usize; 10] = [
    32768, 32768, 2048, 12288, 1024, 32768, 40960, 8192, 262144, 98304,
];
const COMMAND_WORKSPACE: usize = 17408;
const REPORT_WORKSPACE: usize = 2816;

/// Same-thread, one-invocation first-visit-fault latch. The sink must be a
/// trusted synchronous no-payload reporter; it must not panic or replace errors.
pub struct C01VisitDiagnosticForConformance {
    first_visit_fault: Cell<Option<C01VisitFaultForConformance>>,
    sink: fn(&C01VisitFaultForConformance),
    used: Cell<bool>,
}
#[derive(Clone, Copy)]
pub struct C01VisitFaultForConformance {
    row: u8,
    ordinal: u8,
    remaining: usize,
    requested: usize,
    site: &'static std::panic::Location<'static>,
}
impl C01VisitDiagnosticForConformance {
    pub fn new(sink: fn(&C01VisitFaultForConformance)) -> Self {
        Self {
            first_visit_fault: Cell::new(None),
            sink,
            used: Cell::new(false),
        }
    }
    pub fn first_visit_fault(&self) -> Option<C01VisitFaultForConformance> {
        self.first_visit_fault.get()
    }
}
impl C01VisitFaultForConformance {
    pub fn row(&self) -> u8 {
        self.row
    }
    pub fn ordinal(&self) -> u8 {
        self.ordinal
    }
    pub fn remaining(&self) -> usize {
        self.remaining
    }
    pub fn requested(&self) -> usize {
        self.requested
    }
    pub fn line(&self) -> u32 {
        self.site.line()
    }
    pub fn column(&self) -> u32 {
        self.site.column()
    }
    /// Closed source-unit tag only. Never disclose a compiler path.
    /// Rendering work is caller-owned, separate from capture/dispatch's 16 units.
    pub fn source_unit(&self) -> u8 {
        let bytes = self.site.file().as_bytes();
        let mut start = bytes.len();
        for _ in 0..65 {
            let Some(previous) = start.checked_sub(1) else {
                break;
            };
            if matches!(bytes[previous], b'/' | b'\\') {
                break;
            }
            start = previous;
        }
        let name = &bytes[start..];
        if name.len() > 64 {
            return 0;
        }
        match name {
            b"temporal_research_crosswalk.rs" => 1,
            b"source_assessment_journal.rs" => 2,
            b"source_native_private_assessment_cli.rs" => 3,
            b"source_sign.rs" => 4,
            _ => 0,
        }
    }
}

pub(crate) struct C01AdapterPayer<'d> {
    owned_remaining: usize,
    visits_remaining: usize,
    calls: [u8; 10],
    diagnostic: Option<&'d C01VisitDiagnosticForConformance>,
}
impl<'d> C01AdapterPayer<'d> {
    pub(crate) fn new(diagnostic: Option<&'d C01VisitDiagnosticForConformance>) -> Self {
        Self {
            owned_remaining: 16777216,
            visits_remaining: 1000000,
            calls: CALLS,
            diagnostic,
        }
    }
    pub(crate) fn reserve(&mut self, row: C01Row) -> Result<C01Admission<'d>> {
        let i = row as usize;
        let workspace = if matches!(row, C01Row::Pure) {
            0
        } else {
            COMMAND_WORKSPACE
                + if matches!(row, C01Row::Result) {
                    REPORT_WORKSPACE
                } else {
                    0
                }
        };
        let bytes = OWNED[i]
            .checked_add(workspace)
            .ok_or(Error::Invalid("C01 reservation overflow"))?;
        let owned_remaining = self
            .owned_remaining
            .checked_sub(bytes)
            .ok_or(Error::Invalid("C01 adapter owned budget"))?;
        let visits_remaining = self
            .visits_remaining
            .checked_sub(VISITS[i])
            .ok_or(Error::Invalid("C01 adapter visit budget"))?;
        let calls = self.calls[i]
            .checked_sub(1)
            .ok_or(Error::Invalid("C01 named row exhausted"))?;
        let ordinal = CALLS[i]
            .checked_sub(calls)
            .ok_or(Error::Invalid("C01 named row exhausted"))?;
        // Validate the WHOLE reservation before mutating any parent field.
        self.owned_remaining = owned_remaining;
        self.visits_remaining = visits_remaining;
        self.calls[i] = calls;
        let mut admission = C01Admission {
            owned_limit: OWNED[i],
            visits_remaining: VISITS[i],
            first_error: None,
            diagnostic: self.diagnostic,
            row: i as u8,
            ordinal,
        };
        // SAME 128: 112 fixed control/layout + 16 first-fault capture/dispatch.
        admission.visit(128)?;
        layout()?;
        Ok(admission)
    }
}
pub(crate) struct C01Admission<'d> {
    owned_limit: usize,
    visits_remaining: usize,
    first_error: Option<Error>,
    diagnostic: Option<&'d C01VisitDiagnosticForConformance>,
    row: u8,
    ordinal: u8,
}
impl C01Admission<'_> {
    #[track_caller]
    pub(crate) fn visit(&mut self, units: usize) -> Result<()> {
        let Some(remaining) = self.visits_remaining.checked_sub(units) else {
            // This fixed branch is prepaid by reservation128, never the failed
            // debit, Owner's remainder, or callback first_error. Latch BEFORE IO.
            if let Some(diagnostic) = self.diagnostic {
                if diagnostic.first_visit_fault.get().is_none() {
                    let fault = C01VisitFaultForConformance {
                        row: self.row,
                        ordinal: self.ordinal,
                        remaining: self.visits_remaining,
                        requested: units,
                        site: std::panic::Location::caller(),
                    };
                    diagnostic.first_visit_fault.set(Some(fault));
                    (diagnostic.sink)(&fault);
                }
            }
            return Err(Error::Invalid("C01 returned visit budget"));
        };
        self.visits_remaining = remaining;
        Ok(())
    }
    #[track_caller]
    pub(crate) fn callback(&mut self, units: usize) -> bool {
        if self.first_error.is_some() {
            return false;
        }
        match self.visit(units) {
            Ok(()) => true,
            Err(error) => {
                self.first_error = Some(error);
                false
            }
        }
    }
    pub(crate) fn bridge(&mut self, fallback: Error) -> Error {
        self.first_error.take().unwrap_or(fallback)
    }
    pub(crate) fn field<'v>(&mut self, value: &'v JsonValue, name: &str) -> Result<&'v JsonValue> {
        self.visit(8)?;
        self.visit(
            name.len()
                .checked_add(1)
                .ok_or(Error::Invalid("C01 field work overflow"))?,
        )?;
        if !name.is_ascii() {
            return Err(Error::Unsupported("C01 field name profile"));
        }
        let JsonValue::Object(entries) = value else {
            return Err(Error::Invalid("C01 expected object"));
        };
        c01_each!(self, entries, (key, value), {
            self.visit(8)?;
            if key.units().len() != name.len() {
                continue;
            }
            let mut equal = true;
            c01_each!(self, key.units().iter().zip(name.bytes()), (unit, byte), {
                self.visit(4)?;
                if *unit != u16::from(byte) {
                    equal = false;
                    break;
                }
            });
            if equal {
                return Ok(value);
            }
        });
        Err(Error::Invalid("C01 required field absent"))
    }
    pub(crate) fn public_v2(&mut self, config: &JsonValue) -> Result<()> {
        let schema = self.field(config, "schema_version")?;
        self.visit(30)?;
        if schema.as_str() != Some("tos_local_assessment_owner_v2") {
            return Err(Error::Unsupported("C01 public owner V2 profile"));
        }
        self.six_envelopes(config)?;
        let sources = self.field(config, "source_records")?;
        self.visit(8)?;
        if !matches!(sources, JsonValue::Array(rows) if rows.len() == 1) {
            return Err(Error::Unsupported("C01 one-source profile"));
        }
        let subjects = self.field(config, "subjects")?;
        self.visit(8)?;
        if !matches!(subjects, JsonValue::Object(rows) if rows.len() == 1) {
            return Err(Error::Unsupported("C01 one-subject profile"));
        }
        Ok(())
    }
    pub(crate) fn six_envelopes(&mut self, config: &JsonValue) -> Result<()> {
        let policy = self.field(config, "policy")?;
        let authorities = self.field(config, "authorities")?;
        let competencies = self.field(config, "competencies")?;
        let records = self.field(config, "records")?;
        // Pay the borrowed shape/length guards, checked sum and comparisons
        // before examining them. Field scans have their own cumulative debit.
        self.visit(64)?;
        let (JsonValue::Object(_), JsonValue::Array(a), JsonValue::Array(c), JsonValue::Array(r)) =
            (policy, authorities, competencies, records)
        else {
            return Err(Error::Unsupported("C01 six-envelope profile"));
        };
        let total = 1usize
            .checked_add(a.len())
            .and_then(|n| n.checked_add(c.len()))
            .and_then(|n| n.checked_add(r.len()));
        if a.len() != 1 || c.len() != 1 || r.len() != 3 || total != Some(6) {
            return Err(Error::Unsupported("C01 six-envelope profile"));
        }
        Ok(())
    }
    pub(crate) fn owned_limit(&self) -> usize {
        self.owned_limit
    }
    pub(crate) fn error<T>(&mut self, error: Error) -> Result<T> {
        {
            let mut c = C01Counter::new(self)?;
            c.command_error::<T>(&error)?;
        }
        Err(error)
    }
}

enum Frame<'a> {
    Node(&'a JsonValue, usize),
    Array(std::slice::Iter<'a, JsonValue>, usize),
    Object(std::slice::Iter<'a, (JsonString, JsonValue)>, usize),
}
pub(crate) struct C01Counter<'v, 'a, 'd> {
    admission: &'a mut C01Admission<'d>,
    owned: usize,
    frames: [Option<Frame<'v>>; 65],
}
fn layout() -> Result<()> {
    let stack = size_of::<[Option<Frame<'static>>; 65]>();
    let header = size_of::<C01Counter<'static, 'static, 'static>>()
        .checked_sub(stack)
        .and_then(|n| n.checked_add(size_of::<C01Admission<'static>>()))
        .and_then(|n| n.checked_add(size_of::<tos_foundation::FoundationError>()));
    let outside = size_of::<Option<Frame<'static>>>()
        .checked_mul(4)
        .and_then(|n| n.checked_add(header?))
        .and_then(|n| n.checked_add(128)) // checked outer iterator envelope
        .and_then(|n| n.checked_add(128 * size_of::<usize>())) // all owner/utility argument, destructure, action and scalar borrows
        .and_then(|n| n.checked_add(64 * size_of::<Option<usize>>())) // checked arithmetic/method guard intermediates
        .and_then(|n| n.checked_add(16 * size_of::<Result<usize>>())) // shallow fallible utility call/return intermediates
        .and_then(|n| n.checked_add(4 * size_of::<Error>())) // map/bridge/static-error return intermediates
        .and_then(|n| n.checked_add(64)); // exact37-byte ordinary String error, at most one
    let total = outside.and_then(|n| n.checked_add(stack));
    if !cfg!(target_arch = "x86_64")
        || size_of::<usize>() != 8
        || size_of::<String>() != 24
        || size_of::<Vec<u8>>() != 24
        || size_of::<JsonValue>() > 48
        || size_of::<(JsonString, JsonValue)>() > 96
        || size_of::<Result<JsonValue>>() > 96
        || align_of::<JsonValue>() > 8
        || size_of::<Option<Frame<'static>>>() > 128
        || align_of::<Option<Frame<'static>>>() > 16
        || header.is_none_or(|n| n > 320)
        || total.is_none_or(|n| n > COMMAND_WORKSPACE)
    {
        return Err(Error::Unsupported("C01 retained layout certificate"));
    }
    Ok(())
}
impl<'v, 'a, 'd> C01Counter<'v, 'a, 'd> {
    #[track_caller]
    pub(crate) fn new(admission: &'a mut C01Admission<'d>) -> Result<Self> {
        admission.visit(128)?;
        layout()?;
        Ok(Self {
            admission,
            owned: 0,
            frames: std::array::from_fn(|_| None),
        })
    }
    #[track_caller]
    pub(crate) fn visit(&mut self, n: usize) -> Result<()> {
        self.admission.visit(n)
    }
    pub(crate) fn add(&mut self, n: usize) -> Result<()> {
        // 8: ledger call, owned/limit metadata, checked add, filter/bound,
        // failure carrier, assignment and return. No later postcharge.
        self.visit(8)?;
        self.owned = self
            .owned
            .checked_add(n)
            .filter(|n| *n <= self.admission.owned_limit)
            .ok_or(Error::Invalid("C01 returned owned budget"))?;
        Ok(())
    }
    fn inline<T: Copy>(&mut self, _value: &T) -> Result<()> {
        self.visit(4)
    }
    pub(crate) fn root<T>(&mut self) -> Result<()> {
        self.visit(4)?;
        self.add(size_of::<T>())
    }
    pub(crate) fn slots<T>(&mut self, capacity: usize) -> Result<()> {
        self.visit(8)?;
        self.add(
            capacity
                .checked_mul(size_of::<T>())
                .ok_or(Error::Invalid("C01 slot overflow"))?,
        )
    }
    pub(crate) fn string(&mut self, value: &String) -> Result<()> {
        self.visit(4)?;
        self.add(value.capacity())
    }
    pub(crate) fn bytes(&mut self, value: &Vec<u8>) -> Result<()> {
        self.visit(4)?;
        self.add(value.capacity())
    }
    pub(crate) fn path(&mut self, value: &PathBuf) -> Result<()> {
        self.visit(4)?;
        self.add(value.capacity())
    }
    pub(crate) fn relative_path(&mut self, value: &RelativePath) -> Result<()> {
        // Immutable fresh parse/to_owned or derived Clone: pinned String clone
        // capacity certificate, NOT a generic String length-as-capacity rule.
        self.visit(4)?;
        self.add(value.as_str().len())
    }
    fn json_string(&mut self, value: &JsonString) -> Result<()> {
        self.visit(16)?;
        // Exact one-error literal (37 bytes) has a prepaid64-byte workspace.
        let bytes = value
            .retained_storage_bytes()
            .map_err(|_| Error::Invalid("C01 JSON string footprint overflow"))?;
        self.add(bytes)
    }
    pub(crate) fn btree<K: 'v, V: 'v>(&mut self, len: usize) -> Result<()> {
        // 64 before ALL iterator/layout/align guards, eight checked arithmetic
        // steps, rounding, node-count/product and their refusal branches.
        // add() pays its own distinct 8 units; neither resets the row account.
        self.visit(64)?;
        // Outer iterator + two nested slice iterators fit the declared128
        // bytes within the fixed448-byte header; check BEFORE constructing it.
        if size_of::<std::collections::btree_map::Iter<'v, K, V>>()
            + 2 * size_of::<std::slice::Iter<'v, JsonValue>>()
            > 128
        {
            return Err(Error::Unsupported("C01 outer iterator workspace"));
        }
        let align = align_of::<K>().max(align_of::<V>()).max(8);
        if align > 8 {
            return Err(Error::Unsupported("C01 BTree alignment certificate"));
        }
        let node = size_of::<K>()
            .checked_add(size_of::<V>())
            .and_then(|n| n.checked_mul(11))
            .and_then(|n| n.checked_add(13 * 8 + 4))
            .and_then(|n| n.checked_add(8 * align))
            .and_then(|n| n.checked_add(align - 1))
            .map(|n| n / align * align)
            .ok_or(Error::Invalid("C01 BTree layout overflow"))?;
        self.add(
            len.checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .and_then(|n| n.checked_mul(node))
                .ok_or(Error::Invalid("C01 BTree node overflow"))?,
        )
    }
    pub(crate) fn strings(&mut self, values: &Vec<String>) -> Result<()> {
        self.visit(4)?;
        self.slots::<String>(values.capacity())?;
        c01_each!(self, values, value, {
            self.visit(1)?;
            self.string(value)?;
        });
        self.visit(1)
    }
    pub(crate) fn json_vec(&mut self, values: &'v Vec<JsonValue>) -> Result<()> {
        self.visit(4)?;
        self.slots::<JsonValue>(values.capacity())?;
        c01_each!(self, values, value, {
            self.visit(8)?;
            self.json(value)?;
        });
        self.visit(1)
    }
    pub(crate) fn json(&mut self, root: &'v JsonValue) -> Result<()> {
        self.visit(8)?;
        self.frames[0] = Some(Frame::Node(root, 0));
        let mut top = 0usize;
        loop {
            self.visit(8)?;
            let frame = self.frames[top]
                .take()
                .ok_or(Error::Invalid("C01 counter frame state"))?;
            self.visit(8)?;
            match frame {
                Frame::Node(value, depth) => match value {
                    JsonValue::Null | JsonValue::Bool(_) => {}
                    JsonValue::Number(value) => {
                        self.visit(1)?;
                        self.add(value.lexeme.capacity())?;
                    }
                    JsonValue::String(value) => self.json_string(value)?,
                    JsonValue::Array(values) => {
                        self.slots::<JsonValue>(values.capacity())?;
                        self.frames[top] = Some(Frame::Array(values.iter(), depth));
                        continue;
                    }
                    JsonValue::Object(values) => {
                        self.slots::<(JsonString, JsonValue)>(values.capacity())?;
                        self.frames[top] = Some(Frame::Object(values.iter(), depth));
                        continue;
                    }
                },
                Frame::Array(mut iter, depth) => {
                    self.visit(8)?;
                    if let Some(value) = iter.next() {
                        self.visit(8)?;
                        if depth >= 64 {
                            return Err(Error::Unsupported("C01 returned JSON depth"));
                        }
                        self.frames[top] = Some(Frame::Array(iter, depth));
                        top += 1;
                        self.frames[top] = Some(Frame::Node(value, depth + 1));
                        continue;
                    }
                }
                Frame::Object(mut iter, depth) => {
                    self.visit(8)?;
                    if let Some((key, value)) = iter.next() {
                        self.json_string(key)?;
                        self.visit(8)?;
                        if depth >= 64 {
                            return Err(Error::Unsupported("C01 returned JSON depth"));
                        }
                        self.frames[top] = Some(Frame::Object(iter, depth));
                        top += 1;
                        self.frames[top] = Some(Frame::Node(value, depth + 1));
                        continue;
                    }
                }
            }
            if top == 0 {
                return Ok(());
            }
            top -= 1;
        }
    }
    pub(crate) fn command_error<T>(&mut self, error: &Error) -> Result<()> {
        self.root::<Result<T>>()?;
        self.visit(8)?;
        match error {
            Error::DeniedWithReason(value) => self.string(value)?,
            Error::SchemaExecution { path, root, reason } => {
                self.string(path)?;
                self.string(root)?;
                self.visit(8)?;
                use tos_validation::item_rules::ItemRefusal;
                match reason {
                    ItemRefusal::Source(value) | ItemRefusal::Unsupported(value) => {
                        self.string(value)?
                    }
                    ItemRefusal::Executor(value) => {
                        self.add(size_of_val(value.as_ref()))?;
                        let tos_validation::item_rules::ItemExecutorRefusal {
                            stage,
                            reason,
                            exchange,
                            quota,
                            batch_completed_count,
                        } = value.as_ref();
                        self.inline(stage)?;
                        self.inline(reason)?;
                        self.inline(exchange)?;
                        self.inline(quota)?;
                        self.inline(batch_completed_count)?;
                    }
                    ItemRefusal::Budget
                    | ItemRefusal::BudgetCheck { .. }
                    | ItemRefusal::Deadline => {}
                }
            }
            Error::Invalid(_)
            | Error::InvalidAt { .. }
            | Error::Conflict(_)
            | Error::Denied(_)
            | Error::Unsupported(_)
            | Error::MissingProductionAdmission => {}
        }
        Ok(())
    }
    pub(crate) fn report(
        &mut self,
        report: &tos_validation::assessment::AssessmentMechanicsReport,
    ) -> Result<()> {
        // 32 also prepays the terminal static-refusal dispatch and inline
        // subtraction BEFORE the callback can exhaust visits. Its FIRST
        // original admission error still wins over any fallback mapping.
        self.visit(32)?;
        let limit = self.admission.owned_limit;
        let measured = {
            let admission = &mut *self.admission;
            report.c01_retained_footprint_with_visit(limit, &mut |n| admission.callback(n))
        };
        let size = match measured {
            Ok(size) => size,
            Err(reason) => {
                use tos_validation::assessment::AssessmentRetainedFootprintRefusal as R;
                let error = match reason {
                    R::UnsupportedShape => Error::Unsupported("C01 retained report shape"),
                    R::Overflow => Error::Invalid("C01 retained report overflow"),
                    R::OwnedLimit => Error::Invalid("C01 retained report owned budget"),
                    R::VisitLimit => Error::Invalid("C01 retained report visit budget"),
                    R::Workspace => Error::Unsupported("C01 retained report workspace"),
                };
                return Err(self.admission.bridge(error));
            }
        };
        let backing = size
            .checked_sub(size_of::<
                tos_validation::assessment::AssessmentMechanicsReport,
            >())
            .ok_or(Error::Invalid("C01 report inline footprint"))?;
        self.add(backing)
    }
}

// Held research composition. All default/interface definitions above remain
// unchanged. This entry does not append judgments or create source records.
use crate::source_assessment_journal::{
    AssessmentHistory, AssessmentJournalReadBatch, C01OwnerReadLimits, ExactJournalHead,
    ProtectedAssessmentJournal,
};
use crate::source_command::{self as command, CommandContext};
use crate::source_native_cli::private_assessment::{
    self as assessment, AssessmentBorrowedPlanInputs, OperationBudget, OwnerVersion, ParsedRequest,
};
use crate::source_sign::OwnerAssessmentPublicSources;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::temporal::{TemporalBudget, TemporalError};
use tos_foundation::{
    CanonicalProfile, Digest256, ExactRecordRef, FoundationError, FoundationErrorCode, JsonLimits,
    SourceRevision,
};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::assessment::{AssessmentMechanicsReport, evaluate_current_assessment};
use tos_validation::executor::{ExecutorBudget, VerifiedWorkerImageHandle};
use tos_validation::source_cut::{CutWorkerLimits, CutWorkerSchemaExecutor};

pub struct C01ResearchBindingForConformance<'a> {
    pub configuration_path: &'a Path,
    pub public_root: &'a Path,
    pub cut: &'a CorpusCutReader,
    pub software: &'a SoftwareCaptureReader,
    pub components: &'a SoftwareComponentSelectionV1,
    pub image: &'a VerifiedWorkerImageHandle,
    pub source_revision: SourceRevision,
    pub current_root: &'a ExactRecordRef,
    pub predecessor_root: &'a ExactRecordRef,
    pub predecessor_path: &'a RelativePath,
    pub current_request_raw: &'a [u8],
    pub retained_request_raw: &'a [u8],
    pub expected_j1_head: Digest256,
    pub visit_diagnostic: Option<&'a C01VisitDiagnosticForConformance>,
    pub conversion: &'a JsonValue,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ProtocolState {
    Ready,
    RetainedRefused,
    PositiveComplete,
    WithdrawalRefused,
    Failed,
}

// Subaccounts of ONE prepaid row. They are never replenished on drop/error.
struct BuildDebit<'d> {
    admission: C01Admission<'d>,
    remaining: [usize; 4],
}
impl<'d> BuildDebit<'d> {
    fn new(payer: &mut C01AdapterPayer<'d>, output: bool) -> Result<Self> {
        let admission = payer.reserve(if output {
            C01Row::PublicOutput
        } else {
            C01Row::AdapterNew
        })?;
        Ok(Self {
            admission,
            remaining: if output {
                [16384, 262144, 196608, 49152]
            } else {
                [32768, 262144, 655360, 98304]
            },
        })
    }
    #[track_caller]
    fn visit(&mut self, n: usize) -> Result<()> {
        self.admission.visit(n)
    }
    #[track_caller]
    fn own(&mut self, part: usize, n: usize) -> Result<()> {
        self.visit(8)?;
        let value = self
            .remaining
            .get(part)
            .copied()
            .ok_or(Error::Invalid("C01 construction row"))?
            .checked_sub(n)
            .ok_or(Error::Invalid("C01 construction owned budget"))?;
        self.remaining[part] = value;
        Ok(())
    }
    fn field<'a>(&mut self, v: &'a JsonValue, k: &str) -> Result<&'a JsonValue> {
        self.admission.field(v, k)
    }
    fn text<'a>(&mut self, v: &'a JsonValue, k: &str) -> Result<&'a str> {
        self.visit(8)?;
        self.field(v, k)?
            .as_str()
            .ok_or(Error::Invalid("C01 UTF8 field"))
    }
    fn copy(&mut self, part: usize, value: &JsonValue) -> Result<JsonValue> {
        let bytes = {
            let mut c = C01Counter::new(&mut self.admission)?;
            c.root::<Result<JsonValue>>()?;
            c.json(value)?;
            c.owned
        };
        self.own(part, bytes)?;
        // Copy work follows the bounded source traversal plus its actual owned
        // buffers. No lookup-key allocation or recursive sizing walk is used.
        self.visit(bytes)?;
        Ok(value.clone())
    }
    fn string(&mut self, part: usize, value: &str) -> Result<JsonValue> {
        self.visit(64)?;
        let bytes = value
            .len()
            .checked_mul(8)
            .and_then(|n| n.checked_add(64))
            .ok_or(Error::Invalid("C01 string capacity"))?;
        self.own(part, bytes)?;
        self.visit(value.len())?;
        let v = JsonString::from_utf8(value);
        // Pinned fresh UTF16 collect/geometric growth + UTF8 String certificate.
        if v.retained_storage_bytes()
            .map_err(|_| Error::Invalid("C01 string storage"))?
            > bytes
        {
            return Err(Error::Unsupported("C01 fresh string capacity certificate"));
        }
        Ok(JsonValue::String(v))
    }
    fn object<const N: usize>(
        &mut self,
        part: usize,
        entries: [(&str, JsonValue); N],
    ) -> Result<JsonValue> {
        self.visit(64)?;
        if N > 16 {
            return Err(Error::Unsupported("C01 fixed object width"));
        }
        self.own(
            part,
            N.checked_mul(size_of::<(JsonString, JsonValue)>())
                .ok_or(Error::Invalid("C01 object capacity"))?,
        )?;
        let mut result = Vec::with_capacity(N);
        for (key, value) in entries {
            self.visit(8)?;
            let JsonValue::String(key) = self.string(part, key)? else {
                return Err(Error::Invalid("C01 key state"));
            };
            result.push((key, value));
        }
        if result.capacity() != N {
            return Err(Error::Unsupported("C01 fixed Vec certificate"));
        }
        Ok(JsonValue::Object(result))
    }
    fn number(&mut self, part: usize, value: u64) -> Result<JsonValue> {
        self.visit(64)?;
        self.own(part, 64)?;
        let result = command::number(value);
        if !matches!(&result,JsonValue::Number(n) if n.lexeme.capacity()<=64) {
            return Err(Error::Unsupported("C01 fresh number certificate"));
        }
        Ok(result)
    }
    fn reference(&mut self, part: usize, r: &ExactRecordRef) -> Result<JsonValue> {
        self.visit(64)?;
        // Pinned to_prefixed also owns a temporary 64-byte hex String.
        // Pay both that temporary and the <=128-byte final String up front.
        self.own(part, 256)?;
        let digest = r.digest.to_prefixed();
        if digest.capacity() > 128 {
            return Err(Error::Unsupported("C01 digest capacity certificate"));
        }
        let id = self.string(part, r.id.as_str())?;
        let version = self.number(part, r.version.get())?;
        let hash = self.string(part, &digest)?;
        self.object(part, [("id", id), ("version", version), ("digest", hash)])
    }
    fn error<T>(&mut self, error: Error) -> Result<T> {
        // Static first errors have no descendants; their complete inline
        // carriers are already in the prepaid error/workspace inventory.
        if matches!(
            &error,
            Error::Invalid(_)
                | Error::InvalidAt { .. }
                | Error::Conflict(_)
                | Error::Denied(_)
                | Error::Unsupported(_)
                | Error::MissingProductionAdmission
        ) {
            return Err(error);
        }
        let bytes = {
            let mut c = C01Counter::new(&mut self.admission)?;
            c.command_error::<T>(&error)?;
            c.owned
        };
        if bytes > self.remaining[3] {
            return Err(Error::Invalid("C01 construction error budget"));
        }
        // The whole error reservation was prepaid, not acquired after failure.
        Err(error)
    }
    fn request(&mut self, raw: &[u8]) -> Result<ParsedRequest> {
        self.own(1, 131072)?;
        let r = match assessment::parse_request(raw, OwnerVersion::V2) {
            Ok(v) => v,
            Err(e) => return self.error(e),
        };
        {
            let mut c = C01Counter::new(&mut self.admission)?;
            // EXACT seven fields plus the complete result carrier.
            let ParsedRequest {
                operation,
                subject_id,
                expected_subject,
                expected_snapshot,
                command_id,
                expected_revision,
                assessments,
            } = &r;
            c.root::<Result<ParsedRequest>>()?;
            c.string(operation)?;
            c.string(subject_id)?;
            c.visit(64)?;
            if let Some(v) = expected_subject {
                c.json(v)?;
            }
            if let Some(v) = expected_snapshot {
                c.string(v)?;
            }
            if let Some(v) = command_id {
                c.string(v)?;
            }
            if let Some(Some(v)) = expected_revision {
                c.string(v)?;
            }
            c.json_vec(assessments)?;
            if c.owned > 131072 {
                return Err(Error::Invalid("C01 request retained budget"));
            }
        }
        self.visit(64)?;
        if r.operation != "inspect"
            || r.command_id.is_some()
            || r.expected_revision.is_some()
            || !r.assessments.is_empty()
            || r.expected_subject.is_none()
            || r.expected_snapshot.is_none()
        {
            return Err(Error::Unsupported("C01 inspect request profile"));
        }
        Ok(r)
    }
    fn canonical(
        &mut self,
        value: &JsonValue,
        output: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        self.visit(64)?;
        let cap = if output { 65536 } else { 16384 };
        // Output's part2 = state65536 + two maximum65536 buffers.
        self.own(2, if output { 196608 } else { 86016 })?;
        let first = Cell::new(None);
        let mut check = || match crate::source_creation_store::active(deadline, cancelled) {
            Ok(()) => Ok(()),
            Err(e) => {
                latch(&first, e);
                Err(callback_error())
            }
        };
        let mut admit = |bytes: usize, visits: usize| {
            let Some(n) = bytes.checked_add(visits) else {
                latch(&first, Error::Invalid("C01 canonical work overflow"));
                return Err(callback_error());
            };
            match self.admission.visit(n) {
                Ok(()) => Ok(()),
                Err(e) => {
                    latch(&first, e);
                    Err(callback_error())
                }
            }
        };
        let written = tos_foundation::canonical_bytes_v1_with_state_budget_and_visits_and_admission(
            value,
            CanonicalProfile::SourceCommandInputV1,
            JsonLimits {
                max_bytes: cap,
                max_depth: 32,
                max_visits: 32768,
                max_integer_digits: 20,
            },
            65536,
            &mut check,
            &mut admit,
        );
        if let Some(e) = first.take() {
            return Err(e);
        }
        let (raw, _) = match written {
            Ok(v) => v,
            Err(e) => {
                if e.detail.capacity() > self.remaining[3] {
                    return Err(Error::Invalid("C01 encoder error budget"));
                }
                return Err(Error::Invalid("C01 canonical encoding"));
            }
        };
        if raw.len() > cap || raw.capacity() > cap {
            return Err(Error::Invalid("C01 wire capacity"));
        }
        Ok(raw)
    }
}
fn latch(slot: &Cell<Option<Error>>, e: Error) {
    let old = slot.take();
    slot.set(old.or(Some(e)));
}
fn callback_error() -> FoundationError {
    FoundationError {
        code: FoundationErrorCode::BudgetExceeded,
        byte_offset: None,
        detail: String::new(),
    }
}

pub struct C01ResearchSessionForConformance<'w> {
    binding: C01ResearchBindingForConformance<'w>,
    owner: ProtectedAssessmentJournal,
    context: Option<CommandContext>,
    sources: Option<OwnerAssessmentPublicSources<'w>>,
    operation: OperationBudget,
    payer: C01AdapterPayer<'w>,
    build: BuildDebit<'w>,
    requests: [ParsedRequest; 2],
    read: C01OwnerReadLimits,
    state: ProtocolState,
    anchored: bool,
    j1_ref: Option<ExactRecordRef>,
    deadline: Instant,
    cancelled: &'w AtomicBool,
    worker: Option<CutWorkerSchemaExecutor>,
    pure: [Option<JsonValue>; 6],
    payloads: [Option<JsonValue>; 2],
    active_input: Option<JsonValue>,
    payload_wire: [Option<Vec<u8>>; 2],
}
#[derive(PartialEq, Eq)]
enum DisclosureState {
    Unused,
    Consumed,
    Failed,
}
pub struct C01ResearchFinalReadForConformance<'a> {
    body: &'a [u8],
    state: DisclosureState,
    guard: &'a mut dyn FnMut() -> Result<()>,
}
impl C01ResearchFinalReadForConformance<'_> {
    pub fn final_read(&mut self, flush: &mut dyn FnMut(&[u8]) -> Result<()>) -> Result<()> {
        if self.state != DisclosureState::Unused {
            return Err(Error::Invalid("C01 final read repeated"));
        }
        self.state = DisclosureState::Failed;
        (self.guard)()?;
        flush(self.body)?;
        self.state = DisclosureState::Consumed;
        Ok(())
    }
}
struct ClosedPublicHeader<'a> {
    current: &'a ExactRecordRef,
    predecessor: &'a ExactRecordRef,
    policy: &'a JsonValue,
    revision: SourceRevision,
    head: Digest256,
    refs: &'a serde_json::Value,
    rule: &'a JsonValue,
    fixity: &'a JsonValue,
    results: [&'a JsonValue; 3],
}

fn ref_matches(b: &mut BuildDebit<'_>, value: &JsonValue, r: &ExactRecordRef) -> Result<bool> {
    b.visit(64)?;
    let JsonValue::Object(fields) = value else {
        return Ok(false);
    };
    if fields.len() != 3 {
        return Ok(false);
    }
    let id = b.text(value, "id")?;
    let version = b.field(value, "version")?.as_u64();
    let digest = b.text(value, "digest")?;
    b.visit(
        id.len()
            .checked_add(digest.len())
            .ok_or(Error::Invalid("C01 reference work"))?,
    )?;
    if id.len() > 256 || digest.len() != 71 {
        return Ok(false);
    }
    Ok(id == r.id.as_str()
        && version == Some(r.version.get())
        && Digest256::from_prefixed(digest).ok() == Some(r.digest))
}
fn exact_object(b: &mut BuildDebit<'_>, value: &JsonValue, keys: &[&str]) -> Result<()> {
    b.visit(64)?;
    let JsonValue::Object(fields) = value else {
        return Err(Error::Invalid("C01 expected object"));
    };
    if fields.len() != keys.len() {
        return Err(Error::Invalid("C01 exact object fields"));
    }
    for key in keys {
        b.visit(4)?;
        b.field(value, key)?;
    }
    Ok(())
}
fn new_workspace(
    binding: &C01ResearchBindingForConformance<'_>,
    b: &mut BuildDebit<'_>,
) -> Result<()> {
    b.visit(256)?;
    b.own(0, 32768)?;
    let mut n = 0usize;
    macro_rules! add {
        ($v:expr) => {
            n = n
                .checked_add($v)
                .ok_or(Error::Invalid("C01 outer workspace overflow"))?;
        };
    }
    add!(size_of::<C01ResearchSessionForConformance<'static>>());
    add!(size_of::<C01ResearchBindingForConformance<'static>>());
    add!(size_of::<OperationBudget>());
    add!(size_of::<C01AdapterPayer<'static>>());
    add!(size_of::<[Option<C01Admission<'static>>; 4]>());
    add!(size_of::<[Result<ParsedRequest>; 2]>());
    add!(size_of::<[Result<JsonValue>; 4]>());
    add!(size_of::<[Error; 8]>());
    add!(size_of::<[FoundationError; 4]>());
    add!(size_of::<[TemporalError; 4]>());
    add!(size_of::<[ExecutorBudget; 2]>());
    add!(size_of::<[CutWorkerLimits; 2]>());
    add!(size_of::<[Option<(u8, usize, &JsonValue)>; 65]>());
    add!(128 * size_of::<usize>());
    add!(size_of::<[Option<usize>; 64]>());
    add!(size_of::<[Result<usize>; 32]>());
    add!(8 * size_of::<&mut dyn FnMut(&[u8]) -> Result<()>>());
    add!(size_of::<[Instant; 4]>());
    add!(size_of::<[Digest256; 4]>());
    add!(size_of::<[SourceRevision; 2]>());
    // Explicit fixed-object by-value input/move carriers, in addition to the
    // frozen outside scalar envelope. Stricter check, no workspace increase.
    add!(2 * size_of::<[(&str, JsonValue); 16]>());
    add!(512);
    if n > 32768
        || !cfg!(all(target_os = "linux", target_arch = "x86_64"))
        || align_of::<C01ResearchSessionForConformance<'static>>() > 16
        || align_of::<ParsedRequest>() > 16
        || size_of::<usize>() != 8
    {
        return Err(Error::Unsupported("C01 outer workspace certificate"));
    }
    // Fixed diagnostic subinventory inside the SAME outer32768, before arming.
    // 64 + 3*48 + 2*8 + 8 + 32*8 = 488 <=512; no heap/path storage.
    if size_of::<C01VisitFaultForConformance>() > 40
        || size_of::<Option<C01VisitFaultForConformance>>() > 48
        || size_of::<C01VisitDiagnosticForConformance>() > 64
        || align_of::<C01VisitFaultForConformance>() > 8
        || align_of::<C01VisitDiagnosticForConformance>() > 8
        || size_of::<&std::panic::Location<'static>>() != 8
        || size_of::<fn(&C01VisitFaultForConformance)>() != 8
    {
        return Err(Error::Unsupported("C01 diagnostic workspace certificate"));
    }
    if let Some(diagnostic) = binding.visit_diagnostic {
        if diagnostic.used.replace(true) || diagnostic.first_visit_fault.get().is_some() {
            return Err(Error::Invalid("C01 diagnostic already used"));
        }
    }
    b.visit(128)?;
    let mut intake = binding
        .current_request_raw
        .len()
        .checked_add(binding.retained_request_raw.len())
        .ok_or(Error::Invalid("C01 intake budget"))?;
    for s in [
        binding.configuration_path.as_os_str().as_encoded_bytes(),
        binding.public_root.as_os_str().as_encoded_bytes(),
        binding.predecessor_path.as_str().as_bytes(),
        binding.current_root.id.as_str().as_bytes(),
        binding.predecessor_root.id.as_str().as_bytes(),
    ] {
        b.visit(8)?;
        intake = intake
            .checked_add(s.len())
            .ok_or(Error::Invalid("C01 intake budget"))?;
    }
    b.visit(binding.current_root.id.as_str().len())?;
    b.visit(binding.predecessor_root.id.as_str().len())?;
    if intake > 8192
        || binding.current_root.id.as_str().len() > 256
        || binding.predecessor_root.id.as_str().len() > 256
        || !binding.configuration_path.is_absolute()
        || !binding.public_root.is_absolute()
        || binding.current_root.id != binding.predecessor_root.id
        || binding.current_root.version.get() != 2
        || binding.predecessor_root.version.get() != 1
        || binding.source_revision != binding.cut.current().revision()
    {
        return Err(Error::Invalid("C01 closed binding profile"));
    }
    exact_object(
        b,
        binding.conversion,
        &[
            "current_position",
            "pinned_position",
            "out_of_coverage_position",
            "target_trs",
        ],
    )?;
    for (k, n) in [
        ("current_position", 10),
        ("pinned_position", 10),
        ("out_of_coverage_position", 13),
    ] {
        b.visit(64)?;
        let p = b.field(binding.conversion, k)?;
        exact_object(b, p, &["value", "trs_ref", "unit", "origin", "resolution"])?;
        let value = b.field(p, "value")?;
        // The borrowed caller DOM is not a parser-state payment. Refuse an
        // arbitrary numeric lexeme before invoking its scalar parser.
        if !matches!(value,JsonValue::Number(v) if v.lexeme.len()==2)
            || value.as_u64() != Some(n)
            || b.text(p, "resolution")? != "exact"
        {
            return Err(Error::Unsupported("C01 exact finite coordinate profile"));
        }
        for field in ["unit", "origin"] {
            b.visit(8)?;
            let s = b.text(p, field)?;
            if s.is_empty() || s.len() > 256 {
                return Err(Error::Invalid("C01 axis text"));
            }
        }
        let trs = b.field(p, "trs_ref")?;
        axis(b, trs)?;
    }
    let target = b.field(binding.conversion, "target_trs")?;
    axis(b, target)?;
    Ok(())
}
fn axis(b: &mut BuildDebit<'_>, v: &JsonValue) -> Result<()> {
    exact_object(b, v, &["id", "version"])?;
    for k in ["id", "version"] {
        b.visit(8)?;
        let s = b.text(v, k)?;
        if s.is_empty() || s.len() > 256 {
            return Err(Error::Invalid("C01 versioned TRS"));
        }
    }
    Ok(())
}
fn operation(b: &mut BuildDebit<'_>, deadline: Instant) -> Result<OperationBudget> {
    b.visit(128)?;
    b.own(2, 4096)?;
    // Fresh two-object, three-key serde construction. Pinned preserve_order
    // graph: M<=2 per map, B<=4 each, typed slots/strings/numbers fit4096.
    if size_of::<serde_json::Value>() > 128 {
        return Err(Error::Unsupported("C01 budget input layout"));
    }
    let input = serde_json::json!({"budgets":{"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824u64}});
    let mut v = OperationBudget::new(&input, deadline)?;
    v.lower_remaining(33554432, 4194304)?;
    Ok(v)
}
pub fn with_research_crosswalk_for_conformance<'w>(
    binding: C01ResearchBindingForConformance<'w>,
    deadline: Instant,
    cancelled: &'w AtomicBool,
    run: &mut dyn FnMut(&mut C01ResearchSessionForConformance<'w>) -> Result<()>,
) -> Result<()> {
    let mut payer = C01AdapterPayer::new(binding.visit_diagnostic);
    let mut build = BuildDebit::new(&mut payer, false)?;
    new_workspace(&binding, &mut build)?;
    crate::source_creation_store::active(deadline, cancelled)?;
    let mut operation = operation(&mut build, deadline)?;
    operation.charge_bytes(binding.current_request_raw.len())?;
    operation.charge_bytes(binding.retained_request_raw.len())?;
    let current = build.request(binding.current_request_raw)?;
    let retained = build.request(binding.retained_request_raw)?;
    build.visit(
        128usize
            .checked_add(current.subject_id.len())
            .and_then(|n| n.checked_add(retained.subject_id.len()))
            .and_then(|n| n.checked_add(4 * 71))
            .and_then(|n| n.checked_add(current.expected_snapshot.as_ref().map_or(0, String::len)))
            .and_then(|n| n.checked_add(retained.expected_snapshot.as_ref().map_or(0, String::len)))
            .ok_or(Error::Invalid("C01 request comparison work"))?,
    )?;
    if current.subject_id != binding.current_root.id.as_str()
        || retained.subject_id != current.subject_id
        || current.expected_snapshot != retained.expected_snapshot
        || !ref_matches(
            &mut build,
            current
                .expected_subject
                .as_ref()
                .ok_or(Error::Invalid("C01 current subject"))?,
            binding.current_root,
        )?
        || !ref_matches(
            &mut build,
            retained
                .expected_subject
                .as_ref()
                .ok_or(Error::Invalid("C01 retained subject"))?,
            binding.predecessor_root,
        )?
    {
        return Err(Error::Invalid("C01 immutable request pair"));
    }
    let read = C01OwnerReadLimits {
        config_bytes: 32768,
        batch_bytes: 8192,
        history_bytes: 16384,
        batches: 2,
        events: 2,
    };
    let owner = ProtectedAssessmentJournal::select_c01_owner_with_return_limit(
        binding.configuration_path,
        binding.public_root,
        &read,
        &mut payer,
        deadline,
        cancelled,
    )?;
    operation.charge_bytes(owner.configuration_raw().len())?;
    assessment::preflight_configuration(owner.configuration(), OwnerVersion::V2)?;
    if OwnerVersion::from_config(owner.configuration())? != OwnerVersion::V2 {
        return Err(Error::Unsupported("C01 public V2 owner"));
    }
    let subjects = build.field(owner.configuration(), "subjects")?;
    let JsonValue::Object(subjects) = subjects else {
        return Err(Error::Invalid("C01 owner subjects"));
    };
    build.visit(64)?;
    if subjects.len() != 1 {
        return Err(Error::Conflict("C01 configured full subject"));
    }
    let configured_record = build.field(&subjects[0].1, "record")?;
    build.visit(current.subject_id.len())?;
    if subjects[0].0.as_str() != Some(current.subject_id.as_str())
        || !ref_matches(&mut build, configured_record, binding.current_root)?
    {
        return Err(Error::Conflict("C01 configured full subject"));
    }
    let scope = &subjects[0].1;
    build.visit(128)?;
    if build.text(scope, "assertion_layer")? != "scholarly_report"
        || build.text(scope, "requested_use")? != "research"
        || build.text(scope, "risk")? != "low"
        || build.field(scope, "access_allowed")? != &JsonValue::Bool(true)
    {
        return Err(Error::Unsupported("C01 source-attributed research scope"));
    }
    let languages = build.field(scope, "languages")?;
    if !matches!(languages,JsonValue::Array(v) if v.len()==1 && v[0].as_str()==Some("en")) {
        return Err(Error::Unsupported("C01 research language profile"));
    }
    let mut session = C01ResearchSessionForConformance {
        binding,
        owner,
        context: None,
        sources: None,
        operation,
        payer,
        build,
        requests: [current, retained],
        read,
        state: ProtocolState::Ready,
        anchored: false,
        j1_ref: None,
        deadline,
        cancelled,
        worker: None,
        pure: std::array::from_fn(|_| None),
        payloads: std::array::from_fn(|_| None),
        active_input: None,
        payload_wire: std::array::from_fn(|_| None),
    };
    let result = run(&mut session);
    if let Err(e) = result {
        return session.build.error(e);
    }
    session.build.visit(8)?;
    if session.state != ProtocolState::WithdrawalRefused
        || !session.anchored
        || session.worker.is_some()
    {
        return Err(Error::Invalid("C01 protocol incomplete"));
    }
    Ok(())
}
struct CurrentGuard<'g, 'o, 'w> {
    binding: &'g C01ResearchBindingForConformance<'w>,
    owner: &'g ProtectedAssessmentJournal,
    context: &'g CommandContext,
    sources: &'g mut OwnerAssessmentPublicSources<'w>,
    history: &'g AssessmentHistory,
    batch: &'g AssessmentJournalReadBatch<'o>,
    read: &'g C01OwnerReadLimits,
    deadline: Instant,
    cancelled: &'g AtomicBool,
}
impl CurrentGuard<'_, '_, '_> {
    fn check(&mut self) -> Result<()> {
        crate::source_creation_store::active(self.deadline, self.cancelled)?;
        self.context.check_from_selected_captures(
            self.binding.cut,
            self.binding.software,
            self.binding.components,
            self.deadline,
            self.cancelled,
        )?;
        self.owner
            .verify_current_with_read_limits(self.read, self.deadline, self.cancelled)?;
        self.sources.verify_current(self.deadline, self.cancelled)?;
        self.history.verify_current_with_read_limits(
            self.batch,
            self.read,
            self.deadline,
            self.cancelled,
        )?;
        self.owner
            .verify_current_with_read_limits(self.read, self.deadline, self.cancelled)
    }
}
fn worker(
    binding: &C01ResearchBindingForConformance<'_>,
    b: &mut BuildDebit<'_>,
    op: &mut OperationBudget,
    seconds: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CutWorkerSchemaExecutor> {
    b.visit(128)?;
    op.lower_worker_resources(seconds, 1073741824)?;
    let mut budget = ExecutorBudget::laboratory();
    budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    budget.cpu_seconds = seconds;
    budget.address_space_bytes = 1073741824;
    b.own(3, 4096)?; // first-error interface, before delegated construction
    match CutWorkerSchemaExecutor::from_cut_with_image(
        binding.cut,
        tos_validation::FormatProfile::AssertedSourceCandidateV1,
        binding.image,
        budget,
        CutWorkerLimits {
            max_receipts: 64,
            max_receipt_bytes: 131072,
        },
        deadline,
        cancelled,
    ) {
        Ok(w) => Ok(w),
        Err(reason) => {
            let e = Error::SchemaExecution {
                path: "assessment/C01".to_owned(),
                root: "selected-source".to_owned(),
                reason,
            };
            b.error(e)
        }
    }
}
fn observed(b: &mut BuildDebit<'_>) -> Result<String> {
    b.visit(128)?;
    b.own(2, 128)?;
    let s = crate::source_serialization::instant()?;
    if s.capacity() > 128 {
        return Err(Error::Unsupported("C01 observation capacity certificate"));
    }
    Ok(s)
}
fn selected_raw(
    binding: &C01ResearchBindingForConformance<'_>,
    path: &RelativePath,
    b: &mut BuildDebit<'_>,
    op: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    b.visit(128)?;
    let meta = binding
        .cut
        .current()
        .member(path)
        .ok_or(Error::Conflict("C01 source member absent"))?;
    if meta.size_bytes > 8192 {
        return Err(Error::Invalid("C01 source wrapper byte limit"));
    }
    let mut forecast = size_of::<Result<tos_source_store::SourceMemberV1>>()
        .checked_add(16384)
        .and_then(|n| n.checked_add(path.as_str().len()))
        .ok_or(Error::Invalid("C01 member forecast"))?;
    let mut count = 0usize;
    b.visit(8)?;
    let mut ids = binding.cut.current().indexed_ids_for_path(path);
    loop {
        b.visit(8)?;
        let Some(id) = ids.next() else {
            break;
        };
        count = count
            .checked_add(1)
            .ok_or(Error::Invalid("C01 member IDs"))?;
        forecast = forecast
            .checked_add(id.len())
            .ok_or(Error::Invalid("C01 member IDs"))?;
    }
    b.visit(64)?;
    let slots = count
        .checked_mul(2)
        .map(|n| n.max(4))
        .and_then(usize::checked_next_power_of_two)
        .and_then(|n| n.checked_mul(size_of::<String>()))
        .ok_or(Error::Invalid("C01 member IDs"))?;
    forecast = forecast
        .checked_add(slots)
        .ok_or(Error::Invalid("C01 member forecast"))?;
    b.own(2, forecast)?;
    op.charge_bytes(
        usize::try_from(meta.size_bytes).map_err(|_| Error::Invalid("C01 member length"))?,
    )?;
    let member = binding
        .cut
        .read_member(binding.source_revision, path, 8192, deadline, cancelled)
        .map_err(|_| Error::Conflict("C01 selected member custody"))?;
    b.visit(64)?;
    let mut actual = size_of_val(&member)
        .checked_add(member.path.as_str().len())
        .and_then(|n| n.checked_add(member.raw.capacity()))
        .and_then(|n| {
            n.checked_add(
                member
                    .stable_ids
                    .capacity()
                    .checked_mul(size_of::<String>())?,
            )
        })
        .ok_or(Error::Invalid("C01 member retained size"))?;
    c01_each!(b, &member.stable_ids, id, {
        b.visit(8)?;
        actual = actual
            .checked_add(id.capacity())
            .ok_or(Error::Invalid("C01 member retained size"))?;
    });
    b.visit(member.raw.len())?;
    b.visit(path.as_str().len())?;
    if actual > forecast
        || member.revision != binding.source_revision
        || member.path != *path
        || member.raw.len() as u64 != meta.size_bytes
        || Digest256::of_bytes(&member.raw) != meta.sha256
    {
        return Err(Error::Conflict("C01 selected member fixity differs"));
    }
    Ok(member.raw)
}
fn digest_text(b: &mut BuildDebit<'_>, part: usize, d: Digest256, bare: bool) -> Result<JsonValue> {
    b.visit(256)?;
    b.own(part, 256)?;
    let s = if bare { d.to_hex() } else { d.to_prefixed() };
    if s.capacity() > 128 {
        return Err(Error::Unsupported("C01 digest capacity certificate"));
    }
    b.string(part, &s)
}
fn raw_fixity(b: &mut BuildDebit<'_>, part: usize, path: &str, raw: &[u8]) -> Result<JsonValue> {
    b.visit(64)?;
    b.visit(raw.len())?;
    let p = b.string(part, path)?;
    let h = digest_text(b, part, Digest256::of_bytes(raw), false)?;
    let size = b.number(
        part,
        u64::try_from(raw.len()).map_err(|_| Error::Invalid("C01 raw size"))?,
    )?;
    b.object(part, [("path", p), ("sha256", h), ("size_bytes", size)])
}
fn stored_input(
    b: &mut BuildDebit<'_>,
    binding: &C01ResearchBindingForConformance<'_>,
    root: &ExactRecordRef,
    path: &str,
    raw: &[u8],
) -> Result<JsonValue> {
    let fixed = raw_fixity(b, 2, path, raw)?;
    let requested_fixed = b.copy(2, &fixed)?;
    let reference = b.reference(2, root)?;
    let requested = b.copy(2, &reference)?;
    let rev = digest_text(b, 2, binding.source_revision.0, false)?;
    let expected = b.copy(2, &rev)?;
    b.visit(raw.len())?;
    let raw_text = std::str::from_utf8(raw).map_err(|_| Error::Invalid("C01 source UTF8"))?;
    let raw_value = b.string(2, raw_text)?;
    let env = b.object(
        2,
        [
            ("source_record_ref", reference),
            ("raw_member_ref", fixed),
            ("source_revision", rev),
            ("record_bytes", raw_value),
        ],
    )?;
    let schema = b.string(2, "tos_stored_crosswalk_decode_request_v1")?;
    b.object(
        2,
        [
            ("schema_version", schema),
            ("record", env),
            ("requested_source_record_ref", requested),
            ("requested_raw_member_ref", requested_fixed),
            ("expected_source_revision", expected),
        ],
    )
}
fn run_pure(
    payer: &mut C01AdapterPayer<'_>,
    op: &mut OperationBudget,
    kind: u8,
    input: &JsonValue,
) -> Result<JsonValue> {
    let mut a = payer.reserve(C01Row::Pure)?;
    // Same finite owner remainder; no per-pure or per-session reset.
    op.charge_bytes(16384)?;
    op.charge_work(32768)?;
    let context = JsonValue::Object(Vec::new());
    let limits = TemporalBudget {
        max_expression_bytes: 16384,
        max_members: 512,
        max_pairings: 512,
    };
    let result = match kind {
        0 => tos_foundation::temporal::stored_crosswalk::decode_stored_crosswalk_record(
            input, &context, limits,
        ),
        1 => tos_foundation::temporal::crosswalk_records::prepare_crosswalk_version(
            input, &context, limits,
        ),
        2 => tos_foundation::temporal::crosswalk_records::convert_crosswalk_record(
            input, &context, limits,
        ),
        _ => return Err(Error::Invalid("C01 pure dispatch")),
    }
    .map_err(|e| match e {
        TemporalError::MalformedRequest(_) => Error::Invalid("C01 temporal request"),
        TemporalError::ResourceExhausted(_) => Error::Invalid("C01 temporal budget"),
    })?;
    {
        let mut c = C01Counter::new(&mut a)?;
        c.root::<Result<JsonValue>>()?;
        c.json(&result)?;
    }
    Ok(result)
}
fn require_status(b: &mut BuildDebit<'_>, v: &JsonValue, status: &str) -> Result<()> {
    b.visit(64)?;
    if b.text(v, "status")? != status {
        return Err(Error::Unsupported("C01 native result status"));
    }
    Ok(())
}
fn payload(
    b: &mut BuildDebit<'_>,
    semantic: &JsonValue,
    predecessor: JsonValue,
) -> Result<JsonValue> {
    let source_field = b_field(b, semantic, "source_trs")?;
    let source = b.copy(2, source_field)?;
    let target_field = b_field(b, semantic, "target_trs")?;
    let target = b.copy(2, target_field)?;
    let profile_field = b_field(b, semantic, "rule_profile")?;
    let profile = b.copy(2, profile_field)?;
    let rules_field = b_field(b, semantic, "rules_version")?;
    let rules = b.copy(2, rules_field)?;
    let applicability_field = b_field(b, semantic, "applicability")?;
    let applicability = b.copy(2, applicability_field)?;
    let rows_field = b_field(b, semantic, "rows")?;
    let rows = b.copy(2, rows_field)?;
    let provenance_field = b_field(b, semantic, "provenance")?;
    let provenance = b.copy(2, provenance_field)?;
    b.object(
        2,
        [
            ("source_trs", source),
            ("target_trs", target),
            ("rule_profile", profile),
            ("rules_version", rules),
            ("applicability", applicability),
            ("rows", rows),
            ("predecessor", predecessor),
            ("provenance", provenance),
        ],
    )
}
fn b_field<'a>(b: &mut BuildDebit<'_>, v: &'a JsonValue, key: &str) -> Result<&'a JsonValue> {
    b.field(v, key)
}
fn payload_env(
    b: &mut BuildDebit<'_>,
    binding: &C01ResearchBindingForConformance<'_>,
    root: &ExactRecordRef,
    raw: &[u8],
) -> Result<JsonValue> {
    b.visit(raw.len())?;
    b.visit(256)?;
    b.own(2, 256)?;
    let hash = Digest256::of_bytes(raw).to_prefixed();
    if hash.capacity() > 128 {
        return Err(Error::Unsupported("C01 payload digest capacity"));
    }
    let id = b.string(2, root.id.as_str())?;
    let version = b.number(2, root.version.get())?;
    let digest = b.string(2, &hash)?;
    let reference = b.object(2, [("id", id), ("version", version), ("digest", digest)])?;
    let rev = digest_text(b, 2, binding.source_revision.0, false)?;
    let text = b.string(
        2,
        std::str::from_utf8(raw).map_err(|_| Error::Invalid("C01 payload UTF8"))?,
    )?;
    b.object(
        2,
        [
            ("record_ref", reference),
            ("source_revision", rev),
            ("record_bytes", text),
        ],
    )
}
fn conversion_input(
    b: &mut BuildDebit<'_>,
    binding: &C01ResearchBindingForConformance<'_>,
    env: &JsonValue,
    position: &JsonValue,
) -> Result<JsonValue> {
    let reference = b_field(b, env, "record_ref")?;
    let requested = b.copy(2, reference)?;
    let record = b.copy(2, env)?;
    let expected = digest_text(b, 2, binding.source_revision.0, false)?;
    let point = b.copy(2, position)?;
    let target_field = b_field(b, binding.conversion, "target_trs")?;
    let target = b.copy(2, target_field)?;
    b.object(
        2,
        [
            ("record", record),
            ("requested_ref", requested),
            ("expected_source_revision", expected),
            ("position", point),
            ("target_trs", target),
        ],
    )
}

fn bridge(
    binding: &C01ResearchBindingForConformance<'_>,
    sources: &OwnerAssessmentPublicSources<'_>,
    b: &mut BuildDebit<'_>,
    payer: &mut C01AdapterPayer<'_>,
    op: &mut OperationBudget,
    pure: &mut [Option<JsonValue>; 6],
    payloads: &mut [Option<JsonValue>; 2],
    wire: &mut [Option<Vec<u8>>; 2],
    active: &mut Option<JsonValue>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<JsonValue> {
    b.visit(128)?;
    if sources.rows.len() != 1 {
        return Err(Error::Conflict("C01 one actual selected source"));
    }
    let row = &sources.rows[0];
    b.visit(binding.current_root.id.as_str().len())?;
    let name = sources
        .record_paths
        .get(binding.current_root.id.as_str())
        .ok_or(Error::Conflict("C01 selected source path"))?;
    b.own(
        2,
        name.len()
            .checked_add(128)
            .ok_or(Error::Invalid("C01 member path capacity"))?,
    )?;
    b.visit(name.len())?;
    let current_path =
        RelativePath::parse(name).map_err(|_| Error::Invalid("C01 source relative path"))?;
    let current_raw = selected_raw(binding, &current_path, b, op, deadline, cancelled)?;
    let predecessor_raw = selected_raw(
        binding,
        binding.predecessor_path,
        b,
        op,
        deadline,
        cancelled,
    )?;
    *active = Some(stored_input(
        b,
        binding,
        binding.current_root,
        current_path.as_str(),
        &current_raw,
    )?);
    pure[0] = Some(run_pure(
        payer,
        op,
        0,
        active
            .as_ref()
            .ok_or(Error::Invalid("C01 active decoder"))?,
    )?);
    *active = None;
    *active = Some(stored_input(
        b,
        binding,
        binding.predecessor_root,
        binding.predecessor_path.as_str(),
        &predecessor_raw,
    )?);
    pure[1] = Some(run_pure(
        payer,
        op,
        0,
        active
            .as_ref()
            .ok_or(Error::Invalid("C01 active decoder"))?,
    )?);
    *active = None;
    let d2 = pure[0]
        .as_ref()
        .ok_or(Error::Invalid("C01 current decode"))?;
    let d1 = pure[1]
        .as_ref()
        .ok_or(Error::Invalid("C01 predecessor decode"))?;
    for (decoded, root) in [(d2, binding.current_root), (d1, binding.predecessor_root)] {
        b.visit(64)?;
        require_status(b, decoded, "DecodedCrosswalkSourceWrapper")?;
        let r = b.field(decoded, "source_record_ref")?;
        if !ref_matches(b, r, root)?
            || b.text(decoded, "authentication")? != "SuppliedBytesOnly"
            || b.text(decoded, "conversion_policy_admission")? != "NotEstablished"
            || b.field(decoded, "writer_performed")? != &JsonValue::Bool(false)
        {
            return Err(Error::Conflict("C01 stored source qualification"));
        }
    }
    // Full canonical-root join is already performed by maintained Plan
    // select_current_scope over THIS complete row, then independently by the
    // stored decoder over the actual W raw bytes. Both pin the SAME typed
    // current_root. Do not replace either full check with a subset or perform
    // an unmetered recursive Eq/duplicate canonical verifier here.
    b.visit(
        128usize
            .checked_add(binding.current_root.id.as_str().len())
            .ok_or(Error::Invalid("C01 selected envelope join work"))?,
    )?;
    if b.text(row, "id")? != binding.current_root.id.as_str()
        || b.field(row, "version")?.as_u64() != Some(binding.current_root.version.get())
    {
        return Err(Error::Conflict("C01 complete selected source root"));
    }
    let sem2 = b.field(d2, "semantic_content")?;
    let sem1 = b.field(d1, "semantic_content")?;
    let prior = b.field(sem2, "predecessor_source_record_ref")?;
    if !ref_matches(b, prior, binding.predecessor_root)?
        || !b.field(sem1, "predecessor_source_record_ref")?.is_null()
    {
        return Err(Error::Conflict("C01 anchored source predecessor"));
    }
    payloads[0] = Some(payload(b, sem1, JsonValue::Null)?);
    wire[0] = Some(
        b.canonical(
            payloads[0]
                .as_ref()
                .ok_or(Error::Invalid("C01 P1 payload"))?,
            false,
            deadline,
            cancelled,
        )?,
    );
    let env1 = payload_env(
        b,
        binding,
        binding.predecessor_root,
        wire[0].as_deref().ok_or(Error::Invalid("C01 P1 wire"))?,
    )?;
    let predecessor_field = b_field(b, &env1, "record_ref")?;
    let predecessor = b.copy(2, predecessor_field)?;
    payloads[1] = Some(payload(b, sem2, predecessor)?);
    wire[1] = Some(
        b.canonical(
            payloads[1]
                .as_ref()
                .ok_or(Error::Invalid("C01 P2 payload"))?,
            false,
            deadline,
            cancelled,
        )?,
    );
    let env2 = payload_env(
        b,
        binding,
        binding.current_root,
        wire[1].as_deref().ok_or(Error::Invalid("C01 P2 wire"))?,
    )?;
    let record = b.copy(2, &env2)?;
    let requested_field = b_field(b, &env2, "record_ref")?;
    let requested = b.copy(2, requested_field)?;
    let expected = digest_text(b, 2, binding.source_revision.0, false)?;
    let prior = b.copy(2, &env1)?;
    *active = Some(b.object(
        2,
        [
            ("record", record),
            ("requested_ref", requested),
            ("expected_source_revision", expected),
            ("predecessor_record", prior),
        ],
    )?);
    pure[2] = Some(run_pure(
        payer,
        op,
        1,
        active.as_ref().ok_or(Error::Invalid("C01 prepare input"))?,
    )?);
    *active = None;
    require_status(
        b,
        pure[2]
            .as_ref()
            .ok_or(Error::Invalid("C01 prepared result"))?,
        "PreparedCrosswalkVersionCandidate",
    )?;
    for (slot, env, key, status) in [
        (3, &env2, "current_position", "Unique"),
        (4, &env1, "pinned_position", "Unique"),
        (5, &env1, "out_of_coverage_position", "OutOfCoverage"),
    ] {
        b.visit(64)?;
        let position = b.field(binding.conversion, key)?;
        *active = Some(conversion_input(b, binding, env, position)?);
        pure[slot] = Some(run_pure(
            payer,
            op,
            2,
            active
                .as_ref()
                .ok_or(Error::Invalid("C01 conversion input"))?,
        )?);
        *active = None;
        let result = pure[slot]
            .as_ref()
            .ok_or(Error::Invalid("C01 conversion result"))?;
        require_status(b, result, status)?;
        if slot != 5 {
            let p = b.field(result, "position")?;
            let payload_index = if slot == 3 { 1 } else { 0 };
            let selected_payload = payloads[payload_index]
                .as_ref()
                .ok_or(Error::Invalid("C01 mapped source payload"))?;
            let declared_rows = b.field(selected_payload, "rows")?;
            let declared_value = b.field(declared_rows, "10")?;
            b.visit(64)?; // bounded integer lexemes, not an invented oracle
            if b.field(p, "value")? != declared_value
                || !matches!(b.field(result,"precision_loss")?,JsonValue::Array(v) if v.is_empty())
            {
                return Err(Error::Unsupported("C01 exact finite outcome"));
            }
        }
    }
    let current = raw_fixity(b, 2, current_path.as_str(), &current_raw)?;
    let predecessor = raw_fixity(b, 2, binding.predecessor_path.as_str(), &predecessor_raw)?;
    b.object(2, [("current", current), ("predecessor", predecessor)])
}
fn serde_ref_matches(
    b: &mut BuildDebit<'_>,
    v: &serde_json::Value,
    r: &ExactRecordRef,
) -> Result<bool> {
    b.visit(128)?;
    let Some(m) = v.as_object() else {
        return Ok(false);
    };
    if m.len() != 3 {
        return Ok(false);
    }
    let id = m.get("id").and_then(serde_json::Value::as_str);
    let ver = m.get("version").and_then(serde_json::Value::as_u64);
    let digest = m.get("digest").and_then(serde_json::Value::as_str);
    let Some(digest) = digest else {
        return Ok(false);
    };
    if digest.len() != 71 {
        return Ok(false);
    }
    b.visit(71 + r.id.as_str().len())?;
    Ok(id == Some(r.id.as_str())
        && ver == Some(r.version.get())
        && Digest256::from_prefixed(digest).ok() == Some(r.digest))
}
fn report_ref(b: &mut BuildDebit<'_>, v: &serde_json::Value) -> Result<ExactRecordRef> {
    b.visit(128)?;
    let m = v
        .as_object()
        .ok_or(Error::Invalid("C01 report reference"))?;
    if m.len() != 3 {
        return Err(Error::Invalid("C01 report reference fields"));
    }
    let id = m
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or(Error::Invalid("C01 report ID"))?;
    let ver = m
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .ok_or(Error::Invalid("C01 report version"))?;
    let hash = m
        .get("digest")
        .and_then(serde_json::Value::as_str)
        .ok_or(Error::Invalid("C01 report digest"))?;
    if id.len() > 256 || hash.len() != 71 || !(1..=3).contains(&ver) {
        return Err(Error::Unsupported("C01 report ref profile"));
    }
    b.own(
        2,
        id.len()
            .checked_add(256)
            .ok_or(Error::Invalid("C01 report reference capacity"))?,
    )?;
    b.visit(id.len() + 71)?;
    Ok(ExactRecordRef {
        id: tos_foundation::StableId::parse(id).map_err(|_| Error::Invalid("C01 report ID"))?,
        version: tos_foundation::RecordVersion::new(ver)
            .map_err(|_| Error::Invalid("C01 report version"))?,
        digest: Digest256::from_prefixed(hash).map_err(|_| Error::Invalid("C01 report digest"))?,
    })
}
fn report_current(
    b: &mut BuildDebit<'_>,
    report: &AssessmentMechanicsReport,
    root: &ExactRecordRef,
    positive: bool,
) -> Result<()> {
    b.visit(128)?;
    let a = report.current_admission();
    let subject = a
        .get("subject")
        .ok_or(Error::Denied("C01 report subject absent"))?;
    if !serde_ref_matches(b, subject, root)?
        || a.get("use").and_then(serde_json::Value::as_str) != Some("research")
        || !a
            .get("limits")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|v| v.is_empty())
        || !a
            .get("invalid_assessments")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|v| v.is_empty())
    {
        return Err(Error::Denied("C01 exact current research scope"));
    }
    let status = if positive { "admitted" } else { "unreviewed" };
    if a.get("schema_version").and_then(serde_json::Value::as_str)
        != Some("tos_knowledge_admission_v1")
        || a.get("status").and_then(serde_json::Value::as_str) != Some(status)
        || a.get("can_use").and_then(serde_json::Value::as_bool) != Some(positive)
    {
        return Err(Error::Denied("C01 current research admission"));
    }
    Ok(())
}

fn output_layout(b: &mut BuildDebit<'_>) -> Result<()> {
    b.visit(256)?;
    b.own(0, 16384)?;
    let mut n = 0usize;
    macro_rules! add {
        ($v:expr) => {
            n = n
                .checked_add($v)
                .ok_or(Error::Invalid("C01 output workspace overflow"))?;
        };
    }
    add!(size_of::<C01ResearchFinalReadForConformance<'static>>());
    add!(size_of::<ClosedPublicHeader<'static>>());
    add!(size_of::<[Result<JsonValue>; 4]>());
    add!(size_of::<[Error; 4]>());
    add!(size_of::<[FoundationError; 2]>());
    add!(size_of::<[JsonValue; 16]>());
    add!(size_of::<[JsonString; 32]>());
    add!(64 * size_of::<usize>());
    add!(size_of::<[Option<usize>; 32]>());
    add!(size_of::<[Result<usize>; 16]>());
    add!(4 * size_of::<&mut dyn FnMut(&[u8]) -> Result<()>>());
    add!(size_of::<[Instant; 2]>());
    add!(2 * size_of::<[(&str, JsonValue); 16]>());
    if n > 16384 || size_of::<CurrentGuard<'static, 'static, 'static>>() > 128 * size_of::<usize>()
    {
        return Err(Error::Unsupported("C01 output workspace certificate"));
    }
    Ok(())
}
fn output_ref(b: &mut BuildDebit<'_>, v: &serde_json::Value) -> Result<JsonValue> {
    b.visit(128)?;
    let m = v
        .as_object()
        .ok_or(Error::Invalid("C01 public assessment reference"))?;
    if m.len() != 3 {
        return Err(Error::Invalid("C01 public assessment reference fields"));
    }
    let id = m
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or(Error::Invalid("C01 public reference ID"))?;
    let version = m
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .ok_or(Error::Invalid("C01 public reference version"))?;
    let digest = m
        .get("digest")
        .and_then(serde_json::Value::as_str)
        .ok_or(Error::Invalid("C01 public reference digest"))?;
    if id.len() > 256 || !(1..=3).contains(&version) || digest.len() != 71 {
        return Err(Error::Unsupported("C01 public reference profile"));
    }
    let id = b.string(1, id)?;
    let ver = b.number(1, version)?;
    let digest = b.string(1, digest)?;
    b.object(1, [("id", id), ("version", ver), ("digest", digest)])
}
fn tagged(
    b: &mut BuildDebit<'_>,
    root: &ExactRecordRef,
    scope: &str,
    result: &JsonValue,
) -> Result<JsonValue> {
    let label = b.string(1, scope)?;
    let reference = b.reference(1, root)?;
    let native = b.copy(1, result)?;
    b.object(
        1,
        [
            ("scope", label),
            ("source_ref", reference),
            ("native_result", native),
        ],
    )
}
fn public_body(
    binding: &C01ResearchBindingForConformance<'_>,
    report: &AssessmentMechanicsReport,
    head: Digest256,
    fixity: &JsonValue,
    rule: &JsonValue,
    pure: &[Option<JsonValue>; 6],
    out: &mut BuildDebit<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    output_layout(out)?;
    out.visit(128)?;
    let admission = report.current_admission();
    let policy = admission
        .get("policy")
        .ok_or(Error::Denied("C01 report policy"))?;
    let refs = admission
        .get("assessment_refs")
        .and_then(serde_json::Value::as_array)
        .ok_or(Error::Denied("C01 report assessment basis"))?;
    if refs.len() != 1 {
        return Err(Error::Denied("C01 one current assessment basis"));
    }
    let policy = output_ref(out, policy)?;
    out.own(1, size_of::<JsonValue>())?;
    let mut basis = Vec::with_capacity(1);
    basis.push(output_ref(out, &refs[0])?);
    if basis.capacity() != 1 {
        return Err(Error::Unsupported("C01 basis Vec certificate"));
    }
    let head = digest_text(out, 1, head, true)?;
    let journal = out.object(
        1,
        [("head", head), ("assessment_refs", JsonValue::Array(basis))],
    )?;
    let source_trs_field = b_field(out, rule, "source_trs")?;
    let source_trs = out.copy(1, source_trs_field)?;
    let target_trs_field = b_field(out, rule, "target_trs")?;
    let target_trs = out.copy(1, target_trs_field)?;
    let app = out.field(rule, "applicability")?;
    let unit_field = b_field(out, app, "unit")?;
    let unit = out.copy(1, unit_field)?;
    let origin_field = b_field(out, app, "origin")?;
    let origin = out.copy(1, origin_field)?;
    let version_field = b_field(out, rule, "rules_version")?;
    let version = out.copy(1, version_field)?;
    let rule = out.object(
        1,
        [
            ("source_trs", source_trs),
            ("target_trs", target_trs),
            ("unit", unit),
            ("origin", origin),
            ("rules_version", version),
        ],
    )?;
    let fixity = out.copy(1, fixity)?;
    let current = tagged(
        out,
        binding.current_root,
        "CurrentSyntheticResearch",
        pure[3]
            .as_ref()
            .ok_or(Error::Invalid("C01 current native result"))?,
    )?;
    let pinned = tagged(
        out,
        binding.predecessor_root,
        "PinnedSourceReproductionOnly",
        pure[4]
            .as_ref()
            .ok_or(Error::Invalid("C01 pinned native result"))?,
    )?;
    let outside = tagged(
        out,
        binding.predecessor_root,
        "PinnedSourceOutOfCoverageOnly",
        pure[5]
            .as_ref()
            .ok_or(Error::Invalid("C01 outside native result"))?,
    )?;
    let schema = out.string(1, "tos_held_research_crosswalk_v1")?;
    let scope = out.string(1, "SyntheticSourceAttributedFiniteRuleResearch")?;
    let current_ref = out.reference(1, binding.current_root)?;
    let revision = digest_text(out, 1, binding.source_revision.0, false)?;
    let body = out.object(
        1,
        [
            ("schema_version", schema),
            ("scope", scope),
            ("current_source_ref", current_ref),
            ("source_revision", revision),
            ("policy_ref", policy),
            ("journal_basis", journal),
            ("rule", rule),
            ("source_fixity", fixity),
            ("current", current),
            ("pinned", pinned),
            ("out_of_coverage", outside),
        ],
    )?;
    out.canonical(&body, true, deadline, cancelled)
}
fn inputs<'a>(
    owner: &'a ProtectedAssessmentJournal,
    sources: &'a OwnerAssessmentPublicSources<'_>,
    request: &'a ParsedRequest,
    empty_refs: &'a BTreeMap<String, Vec<JsonValue>>,
    empty_languages: &'a BTreeMap<String, Vec<String>>,
) -> Result<AssessmentBorrowedPlanInputs<'a>> {
    let snapshot = request
        .expected_snapshot
        .as_deref()
        .ok_or(Error::Invalid("C01 request snapshot"))?;
    Ok(AssessmentBorrowedPlanInputs {
        config: owner.configuration(),
        configuration_raw: owner.configuration_raw(),
        request,
        version: OwnerVersion::V2,
        public_rows: &sources.rows,
        private_rows: &[],
        public_native_rows: &[],
        private_native_rows: &[],
        public_fixity: &sources.fixity,
        public_identity_snapshots: &sources.identity_snapshots,
        public_claim_dependencies: &sources.claim_dependencies,
        // Inert ONLY under exact config-derived V2 + all private/native/layer
        // guards. owner_snapshot reads this parameter solely for is_private().
        private_source_snapshot: snapshot,
        private_native_snapshots: &[],
        private_required_source_refs: empty_refs,
        private_required_languages: empty_languages,
        native_summaries: &[],
        layers: None,
        layer_preflight: None,
    })
}
fn history_map(
    b: &mut BuildDebit<'_>,
    key: &str,
    history: AssessmentHistory,
) -> Result<BTreeMap<String, AssessmentHistory>> {
    b.visit(128)?;
    let bytes = {
        let mut c = C01Counter::new(&mut b.admission)?;
        c.root::<BTreeMap<String, AssessmentHistory>>()?;
        c.btree::<String, AssessmentHistory>(1)?;
        c.owned
    };
    b.own(
        2,
        bytes
            .checked_add(key.len())
            .ok_or(Error::Invalid("C01 history map capacity"))?,
    )?;
    b.visit(key.len())?;
    let mut result = BTreeMap::new();
    result.insert(key.to_owned(), history);
    Ok(result)
}
fn evaluate(
    plan: &assessment::AssessmentEvaluationPlan<'_>,
    histories: &BTreeMap<String, AssessmentHistory>,
    observed: &str,
    binding: &C01ResearchBindingForConformance<'_>,
    worker: &mut CutWorkerSchemaExecutor,
    operation: &mut OperationBudget,
    payer: &mut C01AdapterPayer<'_>,
    cancelled: &AtomicBool,
) -> Result<assessment::AssessmentEvaluationResult> {
    assessment::evaluate_assessment_histories_c01_with_return_limit(
        plan,
        histories,
        observed,
        Some(binding.source_revision),
        binding.source_revision.0,
        worker,
        operation,
        payer,
        cancelled,
        &mut |input, schemas, limits, cancelled| {
            evaluate_current_assessment(input, schemas, limits, cancelled)
                .map_err(assessment::map_assessment_refusal)
        },
    )
}
fn one_event<'a>(
    b: &mut BuildDebit<'_>,
    history: &'a AssessmentHistory,
    index: usize,
    decision: &str,
) -> Result<&'a JsonValue> {
    b.visit(128)?;
    let batch = history
        .batches
        .get(index)
        .ok_or(Error::Conflict("C01 actual history batch"))?;
    let events = b
        .field(batch, "events")?
        .as_array()
        .ok_or(Error::Conflict("C01 actual journal events"))?;
    if events.len() != 1 {
        return Err(Error::Conflict("C01 exact journal event count"));
    }
    let event = b.field(&events[0], "assessment")?;
    if b.text(event, "decision")? != decision {
        return Err(Error::Conflict("C01 actual journal decision"));
    }
    b.visit(64)?;
    if b.text(event, "profile_id")? != "source-observation"
        || b.text(event, "language")? != "en"
        || !matches!(b.field(event,"limits")?,JsonValue::Array(v) if v.is_empty())
    {
        return Err(Error::Unsupported("C01 journal research profile"));
    }
    Ok(event)
}

impl C01ResearchSessionForConformance<'_> {
    pub fn attempt_retained_as_current(&mut self) -> Result<()> {
        self.build.visit(64)?;
        if self.state != ProtocolState::Ready {
            return Err(Error::Invalid("C01 retained attempt sequence"));
        }
        self.state = ProtocolState::Failed;
        crate::source_creation_store::active(self.deadline, self.cancelled)?;
        // The outer protected selection already checked the exact configured
        // R2 root and immutable request pair. This is PROVISIONAL until the
        // later current snapshot and BOTH actual stored decoders anchor it.
        self.state = ProtocolState::RetainedRefused;
        Err(Error::Conflict("C01 retained version is not current"))
    }
    pub fn with_current_conversion(
        &mut self,
        callback: &mut dyn FnMut(&mut C01ResearchFinalReadForConformance<'_>) -> Result<()>,
    ) -> Result<()> {
        self.build.visit(64)?;
        if self.state != ProtocolState::RetainedRefused {
            return Err(Error::Invalid("C01 positive attempt sequence"));
        }
        self.state = ProtocolState::Failed;
        let result = positive(self, callback);
        match result {
            Ok(()) => {
                self.state = ProtocolState::PositiveComplete;
                Ok(())
            }
            Err(e) => {
                // Failure cannot disclose or reset. Preserve the first operation
                // error; cleanup FINAL errors cannot replace it.
                if let Some(mut w) = self.worker.take() {
                    let _ =
                        crate::source_sign::finish_worker(&mut w, self.deadline, self.cancelled);
                }
                self.build.error(e)
            }
        }
    }
    pub fn attempt_after_withdrawal(&mut self, expected_j2_head: Digest256) -> Result<()> {
        self.build.visit(64)?;
        if self.state != ProtocolState::PositiveComplete || !self.anchored {
            return Err(Error::Invalid("C01 withdrawal attempt sequence"));
        }
        self.state = ProtocolState::Failed;
        match withdrawal(self, expected_j2_head) {
            Ok(()) => {
                self.state = ProtocolState::WithdrawalRefused;
                Err(Error::Denied("C01 current research admission withdrawn"))
            }
            Err(e) => {
                if let Some(mut w) = self.worker.take() {
                    let _ =
                        crate::source_sign::finish_worker(&mut w, self.deadline, self.cancelled);
                }
                self.build.error(e)
            }
        }
    }
}
fn positive(
    session: &mut C01ResearchSessionForConformance<'_>,
    callback: &mut dyn FnMut(&mut C01ResearchFinalReadForConformance<'_>) -> Result<()>,
) -> Result<()> {
    let C01ResearchSessionForConformance {
        binding,
        owner,
        context,
        sources,
        operation,
        payer,
        build,
        requests,
        read,
        anchored,
        j1_ref,
        deadline,
        cancelled,
        worker: live,
        pure,
        payloads,
        active_input,
        payload_wire,
        ..
    } = session;
    *live = Some(worker(binding, build, operation, 2, *deadline, cancelled)?);
    let w = live.as_mut().ok_or(Error::Invalid("C01 initial worker"))?;
    *context = Some(assessment::selected_c01_command_context_with_return_limit(
        binding.cut,
        owner.configuration_raw(),
        binding.current_request_raw,
        operation,
        payer,
        *deadline,
        cancelled,
    )?);
    let ctx = context
        .as_ref()
        .ok_or(Error::Invalid("C01 selected Context"))?;
    // MANDATORY initial DRIVER check: the builder does not perform this.
    ctx.check_from_selected_captures(
        binding.cut,
        binding.software,
        binding.components,
        *deadline,
        cancelled,
    )?;
    owner.verify_current_with_read_limits(read, *deadline, cancelled)?;
    *sources = Some(
        crate::source_sign::select_owner_assessment_public_sources_with_return_limit(
            binding.public_root,
            binding.cut,
            ctx,
            owner.configuration(),
            w,
            payer,
            *deadline,
            cancelled,
        )?,
    );
    let selected = sources
        .as_mut()
        .ok_or(Error::Invalid("C01 selected sources"))?;
    operation.charge_bytes(selected.input_bytes()?)?;
    let empty_refs = BTreeMap::new();
    let empty_languages = BTreeMap::new();
    let plan = assessment::resolve_and_plan_c01_with_return_limit(
        inputs(owner, selected, &requests[0], &empty_refs, &empty_languages)?,
        operation,
        payer,
        *deadline,
        cancelled,
    )?;
    build.visit(128)?;
    build.visit(requests[0].subject_id.len())?;
    build.visit(
        requests[0]
            .expected_snapshot
            .as_ref()
            .map_or(0, String::len),
    )?;
    build.visit(
        requests[1]
            .expected_snapshot
            .as_ref()
            .map_or(0, String::len),
    )?;
    if plan.lock_subjects.len() != 1
        || plan.lock_subjects[0] != requests[0].subject_id
        || plan.base_snapshot
            != requests[0]
                .expected_snapshot
                .as_deref()
                .ok_or(Error::Invalid("C01 current snapshot"))?
        || plan.base_snapshot
            != requests[1]
                .expected_snapshot
                .as_deref()
                .ok_or(Error::Invalid("C01 retained snapshot"))?
    {
        return Err(Error::Conflict("C01 actual shared request snapshot"));
    }
    let batch = owner.read_only_subjects_c01_with_return_limit(
        &plan.lock_subjects,
        read,
        payer,
        *deadline,
        cancelled,
    )?;
    build.visit(64)?;
    build.own(2, 64)?;
    let head = binding.expected_j1_head.to_hex();
    if head.capacity() != 64 {
        return Err(Error::Unsupported("C01 head capacity certificate"));
    }
    let history = batch.read_c01_history_with_return_limit(
        &requests[0].subject_id,
        ctx,
        w,
        ExactJournalHead::Some(&head),
        read,
        payer,
        *deadline,
        cancelled,
    )?;
    let histories = history_map(build, &requests[0].subject_id, history)?;
    let now = observed(build)?;
    let result = evaluate(
        &plan, &histories, &now, binding, w, operation, payer, cancelled,
    )?;
    drop(plan); // required before whole-source mutable rechecks
    report_current(build, &result.current_report, binding.current_root, true)?;
    build.visit(128)?;
    let a = result.current_report.current_admission();
    let refs = a
        .get("assessment_refs")
        .and_then(serde_json::Value::as_array)
        .ok_or(Error::Denied("C01 actual admitted basis"))?;
    if refs.len() != 1 {
        return Err(Error::Denied("C01 exact admitted basis count"));
    }
    let actual_j1 = report_ref(build, &refs[0])?;
    let history = histories
        .get(&requests[0].subject_id)
        .ok_or(Error::Invalid("C01 held history"))?;
    if history.batches.len() != 1 || history.submissions.len() != 1 {
        return Err(Error::Conflict("C01 actual J1 history shape"));
    }
    let event = one_event(build, history, 0, "admit")?;
    build.visit(actual_j1.id.as_str().len())?;
    if build.text(event, "assessment_id")? != actual_j1.id.as_str() || actual_j1.version.get() != 1
    {
        return Err(Error::Conflict("C01 actual J1 assessment basis"));
    }
    let event_subject = build.field(event, "subject")?;
    if !ref_matches(build, event_subject, binding.current_root)? {
        return Err(Error::Conflict("C01 J1 exact subject"));
    }
    let fixity = bridge(
        binding,
        selected,
        build,
        payer,
        operation,
        pure,
        payloads,
        payload_wire,
        active_input,
        *deadline,
        cancelled,
    )?;
    // Provisional early conflict is now anchored by actual current snapshot,
    // complete selected packet, raw fixity and BOTH maintained stored decoders.
    *anchored = true;
    *j1_ref = Some(actual_j1);
    let mut output = BuildDebit::new(payer, true)?;
    let rule = payloads[1]
        .as_ref()
        .ok_or(Error::Invalid("C01 actual P2"))?;
    let body = public_body(
        binding,
        &result.current_report,
        binding.expected_j1_head,
        &fixity,
        rule,
        pure,
        &mut output,
        *deadline,
        cancelled,
    )?;
    let mut completed = live
        .take()
        .ok_or(Error::Invalid("C01 finish initial worker"))?;
    crate::source_sign::finish_worker(&mut completed, *deadline, cancelled)?;
    drop(completed); // literal one-live-worker boundary before disclosure
    output.visit(128)?;
    // Pay the bounded top-level cleanup before disclosure. The six pure
    // results, two bridge payloads, two wire buffers and active input are
    // attempt-local ownership, never permission or storage for J2 reuse.
    build.visit(64)?;
    let mut current = CurrentGuard {
        binding,
        owner,
        context: ctx,
        sources: selected,
        history,
        batch: &batch,
        read,
        deadline: *deadline,
        cancelled,
    };
    current.check()?;
    let mut check = || current.check();
    let mut final_read = C01ResearchFinalReadForConformance {
        body: &body,
        state: DisclosureState::Unused,
        guard: &mut check,
    };
    callback(&mut final_read)?;
    if final_read.state != DisclosureState::Consumed {
        return Err(Error::Invalid("C01 required final read absent"));
    }
    drop(final_read);
    drop(check);
    current.check()?;
    drop(current);
    for value in pure.iter_mut() {
        *value = None;
    }
    for value in payloads.iter_mut() {
        *value = None;
    }
    for value in payload_wire.iter_mut() {
        *value = None;
    }
    *active_input = None;
    // All batch/history/report/body/guard holders drop on return before the
    // actual native J2 append. Only immutable source/owner context and the
    // exact J1 lineage reference survive; no debit is refunded.
    Ok(())
}
fn withdrawal(session: &mut C01ResearchSessionForConformance<'_>, head: Digest256) -> Result<()> {
    let C01ResearchSessionForConformance {
        binding,
        owner,
        context,
        sources,
        operation,
        payer,
        build,
        requests,
        read,
        j1_ref,
        deadline,
        cancelled,
        worker: live,
        ..
    } = session;
    build.visit(128)?;
    if head == binding.expected_j1_head {
        return Err(Error::Conflict("C01 withdrawal head did not change"));
    }
    let actual_j1 = j1_ref
        .as_ref()
        .ok_or(Error::Invalid("C01 previous actual J1 basis"))?;
    let ctx = context
        .as_ref()
        .ok_or(Error::Invalid("C01 retained full Context"))?;
    let selected = sources
        .as_mut()
        .ok_or(Error::Invalid("C01 retained public sources"))?;
    *live = Some(worker(binding, build, operation, 1, *deadline, cancelled)?);
    let w = live.as_mut().ok_or(Error::Invalid("C01 second worker"))?;
    let empty_refs = BTreeMap::new();
    let empty_languages = BTreeMap::new();
    let plan = assessment::resolve_and_plan_c01_with_return_limit(
        inputs(owner, selected, &requests[0], &empty_refs, &empty_languages)?,
        operation,
        payer,
        *deadline,
        cancelled,
    )?;
    build.visit(64)?;
    build.visit(requests[0].subject_id.len())?;
    if plan.lock_subjects.len() != 1 || plan.lock_subjects[0] != requests[0].subject_id {
        return Err(Error::Conflict("C01 current withdrawal subject scope"));
    }
    let batch = owner.read_only_subjects_c01_with_return_limit(
        &plan.lock_subjects,
        read,
        payer,
        *deadline,
        cancelled,
    )?;
    build.visit(64)?;
    build.own(2, 64)?;
    let exact = head.to_hex();
    if exact.capacity() != 64 {
        return Err(Error::Unsupported("C01 second head capacity certificate"));
    }
    let history = batch.read_c01_history_with_return_limit(
        &requests[0].subject_id,
        ctx,
        w,
        ExactJournalHead::Some(&exact),
        read,
        payer,
        *deadline,
        cancelled,
    )?;
    let histories = history_map(build, &requests[0].subject_id, history)?;
    let now = observed(build)?; // actual post-J2 observation, never old clock reuse
    let result = evaluate(
        &plan, &histories, &now, binding, w, operation, payer, cancelled,
    )?;
    drop(plan);
    report_current(build, &result.current_report, binding.current_root, false)?;
    let history = histories
        .get(&requests[0].subject_id)
        .ok_or(Error::Invalid("C01 J2 held history"))?;
    build.visit(128)?;
    if history.batches.len() != 2 || history.submissions.len() != 2 {
        return Err(Error::Conflict("C01 J1 J2 history count"));
    }
    let old = one_event(build, history, 0, "admit")?;
    build.visit(actual_j1.id.as_str().len())?;
    if build.text(old, "assessment_id")? != actual_j1.id.as_str() {
        return Err(Error::Conflict("C01 retained J1 identity"));
    }
    let event = one_event(build, history, 1, "withdraw")?;
    let subject = build.field(event, "subject")?;
    if !ref_matches(build, subject, binding.current_root)? {
        return Err(Error::Conflict("C01 J2 exact subject"));
    }
    let supersedes = build
        .field(event, "supersedes")?
        .as_array()
        .ok_or(Error::Conflict("C01 J2 supersedes"))?;
    if supersedes.len() != 1 || !ref_matches(build, &supersedes[0], actual_j1)? {
        return Err(Error::Conflict("C01 J2 actual J1 supersedes"));
    }
    let admission = result.current_report.current_admission();
    let superseded = admission
        .get("superseded_assessment_refs")
        .and_then(serde_json::Value::as_array)
        .ok_or(Error::Denied("C01 actual superseded basis"))?;
    if superseded.len() != 1 || !serde_ref_matches(build, &superseded[0], actual_j1)? {
        return Err(Error::Denied("C01 full current withdrawal basis"));
    }
    // Valid withdrawal can remain active while superseding the old admit.
    let active = admission
        .get("assessment_refs")
        .and_then(serde_json::Value::as_array)
        .ok_or(Error::Denied("C01 active withdrawal basis"))?;
    if active.len() != 1 {
        return Err(Error::Denied("C01 exact active withdrawal basis"));
    }
    let current_j2 = report_ref(build, &active[0])?;
    build.visit(current_j2.id.as_str().len())?;
    if build.text(event, "assessment_id")? != current_j2.id.as_str()
        || current_j2.version.get() != 1
    {
        return Err(Error::Denied("C01 actual J2 assessment identity"));
    }
    let mut current = CurrentGuard {
        binding,
        owner,
        context: ctx,
        sources: selected,
        history,
        batch: &batch,
        read,
        deadline: *deadline,
        cancelled,
    };
    current.check()?;
    drop(current);
    let mut completed = live
        .take()
        .ok_or(Error::Invalid("C01 finish withdrawal worker"))?;
    crate::source_sign::finish_worker(&mut completed, *deadline, cancelled)?;
    drop(completed);
    // Return actual Denied only after caller method records this FULL refusal.
    // No pure operation, output row or successful callback occurs after J2.
    Ok(())
}
