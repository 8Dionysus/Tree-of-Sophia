//! Full-rule reader over the authenticated authored cut and separately held
//! physical auxiliary namespace. Physical observations never extend the cut.
use crate::source_admission_candidate_records::CandidateRecordsInput;
use crate::source_admission_spooled_candidate::CandidateFence;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, Digest256Hasher, RelativePath};
use tos_ops_mechanics_plan::route_cards::{RouteRootCustody, RouteSourceReadHooks, RouteSources};
use tos_source_store::{CorpusCutReader, is_authored_source_path_v1};
use tos_validation::item_rules::ItemRefusal;
use tos_validation::layer_family_cut::CutLayerPayloadReader;
use tos_validation::layer_family_rules::{LayerFamilySource, LayerPayload};
use tos_validation::record_biblio_cut::SourceCutInputWithIdentity;
use tos_validation::source_cut::CutSchemaExecutor;

/// Exact, explicitly selected historical evidence. This capability cannot
/// supply current authored membership or executable software authority.
/// Usage is monotonic charged I/O and retained logical state, not RSS.
pub(crate) trait FoundationHistoricalEvidence {
    fn selected(&self, path: &str) -> bool;
    fn read(
        &mut self,
        path: &str,
        expected_digest: Option<&str>,
        max_bytes: usize,
        max_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::io::Result<Option<Vec<u8>>>;
    fn physical(
        &mut self,
        path: &str,
        max_read_bytes: u64,
        max_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::io::Result<Option<tos_validation::source_foundation_discovery::PhysicalPathFacts>>;
    fn recheck(
        &mut self,
        max_read_bytes: u64,
        max_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::io::Result<()>;
    fn usage(&self) -> (u64, usize);
    fn shared_runtime_read_bytes_returned(&self) -> u64 {
        0
    }
    fn shared_io_budget_matches(&self, _: &tos_source_store::PinnedSqliteIoBudget) -> bool {
        false
    }
}

pub(crate) struct FoundationRuleReadLimits {
    pub max_member_bytes: usize,
    pub max_read_bytes: u64,
    pub max_auxiliary_paths: usize,
    pub max_auxiliary_state_bytes: usize,
    pub deadline: Instant,
}

/// Successful reads, including the final repeated auxiliary verification.
/// A refused read does not export a completed cost receipt.
pub(crate) struct FoundationRuleReadCost {
    pub bytes_read: u64,
    pub shared_read_bytes_returned: u64,
    pub auxiliary_paths: usize,
    pub auxiliary_state_bytes: usize,
}

/// Retained byte custody for separately selected operands. Moving this out of
/// the rule reader releases its worker/payload borrows without losing the EOF
/// obligations across later catalog and bibliographic workers.
pub(crate) struct FoundationAuxiliaryCustody<'cancel> {
    cancelled: &'cancel AtomicBool,
    root_custody: RouteRootCustody,
    limits: FoundationRuleReadLimits,
    read_bytes: u64,
    original_io: Option<tos_source_store::PinnedSqliteIoBudget>,
    shared_read_bytes_returned: u64,
    auxiliary: BTreeMap<String, Option<(Digest256, u64)>>,
    auxiliary_state_bytes: usize,
}

impl FoundationAuxiliaryCustody<'_> {
    pub(crate) fn deadline(&self) -> Instant {
        self.limits.deadline
    }
    pub(crate) fn retained_state_bytes(&self) -> usize {
        self.auxiliary_state_bytes
    }
    pub(crate) fn bytes_read(&self) -> u64 {
        self.read_bytes
    }
    pub(crate) fn verify_context(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        if !std::ptr::eq(cancelled, self.cancelled) || self.limits.deadline > deadline {
            return Err(ItemRefusal::Source(
                "foundation auxiliary operation context differs".into(),
            ));
        }
        reader_checkpoint(self.limits.deadline, self.cancelled)
    }
    pub(crate) fn recheck(
        &mut self,
        physical: &mut RouteSources,
        cancelled: &AtomicBool,
    ) -> Result<FoundationRuleReadCost, ItemRefusal> {
        let remaining_read_bytes = self
            .limits
            .max_read_bytes
            .checked_sub(self.read_bytes)
            .ok_or(ItemRefusal::Budget)?;
        self.recheck_until_with_limits(
            physical,
            remaining_read_bytes,
            self.limits.max_auxiliary_state_bytes,
            self.limits.deadline,
            cancelled,
        )
    }

    /// Recheck selected auxiliary operands under a caller's remaining whole-
    /// invocation allowance. Both limits narrow monotonically, including on
    /// failure, so a retry cannot recover spent headroom. The original
    /// deadline, cancellation signal, and held-root custody remain exact.
    pub(crate) fn recheck_until_with_limits(
        &mut self,
        physical: &mut RouteSources,
        max_remaining_read_bytes: u64,
        max_auxiliary_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<FoundationRuleReadCost, ItemRefusal> {
        if deadline != self.limits.deadline {
            return Err(ItemRefusal::Source(
                "foundation auxiliary deadline was changed".into(),
            ));
        }
        self.verify_context(deadline, cancelled)?;
        if self.auxiliary_state_bytes > max_auxiliary_state_bytes {
            return Err(ItemRefusal::Budget);
        }
        self.limits.max_auxiliary_state_bytes = self
            .limits
            .max_auxiliary_state_bytes
            .min(max_auxiliary_state_bytes);
        let requested_total = self
            .read_bytes
            .checked_add(max_remaining_read_bytes)
            .ok_or(ItemRefusal::Budget)?;
        self.limits.max_read_bytes = self.limits.max_read_bytes.min(requested_total);
        physical
            .verify_custody(&self.root_custody)
            .map_err(custody)?;
        recheck_auxiliary_inputs(
            physical,
            &self.auxiliary,
            &self.limits,
            &mut self.read_bytes,
            self.original_io.as_ref(),
            &mut self.shared_read_bytes_returned,
            cancelled,
        )?;
        Ok(FoundationRuleReadCost {
            bytes_read: self.read_bytes,
            shared_read_bytes_returned: self.shared_read_bytes_returned,
            auxiliary_paths: self.auxiliary.len(),
            auxiliary_state_bytes: self.auxiliary_state_bytes,
        })
    }
}

fn recheck_auxiliary_inputs(
    physical: &mut RouteSources,
    selected: &BTreeMap<String, Option<(Digest256, u64)>>,
    limits: &FoundationRuleReadLimits,
    read_bytes: &mut u64,
    original_io: Option<&tos_source_store::PinnedSqliteIoBudget>,
    shared_read_bytes_returned: &mut u64,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    for (path, expected) in selected {
        reader_checkpoint(limits.deadline, cancelled)?;
        let present = physical.is_file(path).map_err(custody)?;
        if present != expected.is_some() {
            return Err(ItemRefusal::Source(
                "foundation auxiliary presence changed".into(),
            ));
        }
        if let Some((digest, length)) = expected {
            let remaining = limits
                .max_read_bytes
                .checked_sub(*read_bytes)
                .ok_or(ItemRefusal::Budget)?;
            let cap = limits
                .max_member_bytes
                .min(usize::try_from(remaining).unwrap_or(usize::MAX));
            if *length > cap as u64 {
                return Err(ItemRefusal::Budget);
            }
            let mut read = 0;
            let raw = read_auxiliary_bytes(
                physical,
                path,
                cap,
                &mut read,
                original_io,
                shared_read_bytes_returned,
                limits.deadline,
                cancelled,
            )?;
            *read_bytes = read_bytes
                .checked_add(read as u64)
                .filter(|n| *n <= limits.max_read_bytes)
                .ok_or(ItemRefusal::Budget)?;
            if raw.len() as u64 != *length || Digest256::of_bytes(&raw) != *digest {
                return Err(ItemRefusal::Source(
                    "foundation auxiliary bytes changed".into(),
                ));
            }
        }
        reader_checkpoint(limits.deadline, cancelled)?;
    }
    reader_checkpoint(limits.deadline, cancelled)?;
    physical.verify_root().map_err(custody)?;
    reader_checkpoint(limits.deadline, cancelled)
}

fn reader_checkpoint(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
    if cancelled.load(Ordering::Relaxed) {
        Err(ItemRefusal::Source(
            "foundation validation cancelled".into(),
        ))
    } else if Instant::now() >= deadline {
        Err(ItemRefusal::Deadline)
    } else {
        Ok(())
    }
}

enum FoundationRuleInput<'a> {
    Cut(&'a CorpusCutReader),
    Candidate(&'a dyn SourceCutInputWithIdentity<CandidateFence>),
}

pub(crate) struct FoundationRuleSource<'a, 'cancel> {
    input: FoundationRuleInput<'a>,
    pub physical: &'a mut RouteSources,
    pub schemas: &'a mut dyn CutSchemaExecutor,
    pub payloads: &'a mut dyn CutLayerPayloadReader,
    pub cancelled: &'cancel AtomicBool,
    pub limits: FoundationRuleReadLimits,
    read_bytes: u64,
    original_io: Option<tos_source_store::PinnedSqliteIoBudget>,
    shared_read_bytes_returned: u64,
    auxiliary: BTreeMap<String, Option<(Digest256, u64)>>,
    auxiliary_state_bytes: usize,
    history: Option<&'a mut (dyn FoundationHistoricalEvidence + 'static)>,
}

impl<'a, 'cancel> FoundationRuleSource<'a, 'cancel> {
    pub(crate) fn into_auxiliary_custody(self) -> FoundationAuxiliaryCustody<'cancel> {
        FoundationAuxiliaryCustody {
            cancelled: self.cancelled,
            root_custody: self.physical.root_custody(),
            limits: self.limits,
            read_bytes: self.read_bytes,
            original_io: self.original_io,
            shared_read_bytes_returned: self.shared_read_bytes_returned,
            auxiliary: self.auxiliary,
            auxiliary_state_bytes: self.auxiliary_state_bytes,
        }
    }
    pub(crate) fn cost(&self) -> FoundationRuleReadCost {
        FoundationRuleReadCost {
            bytes_read: self.read_bytes,
            shared_read_bytes_returned: self.shared_read_bytes_returned,
            auxiliary_paths: self.auxiliary.len(),
            auxiliary_state_bytes: self.auxiliary_state_bytes,
        }
    }
    pub(crate) fn new(
        cut: &'a CorpusCutReader,
        physical: &'a mut RouteSources,
        schemas: &'a mut dyn CutSchemaExecutor,
        payloads: &'a mut dyn CutLayerPayloadReader,
        cancelled: &'cancel AtomicBool,
        limits: FoundationRuleReadLimits,
    ) -> Result<Self, ItemRefusal> {
        Self::new_inner(
            FoundationRuleInput::Cut(cut),
            physical,
            schemas,
            payloads,
            cancelled,
            limits,
        )
    }

    /// Borrow the actual candidate input; neither a corpus revision nor a
    /// physical authored-source fallback can be supplied by this route.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_candidate(
        input: &'a CandidateRecordsInput<'_, '_>,
        original_io: &tos_source_store::PinnedSqliteIoBudget,
        physical: &'a mut RouteSources,
        schemas: &'a mut dyn CutSchemaExecutor,
        payloads: &'a mut dyn CutLayerPayloadReader,
        cancelled: &'cancel AtomicBool,
        limits: FoundationRuleReadLimits,
        retained_state_bytes: usize,
        max_callback_state_bytes: usize,
        max_operation_state_bytes: usize,
    ) -> Result<Self, ItemRefusal> {
        // The adapter retains its raw callback bytes while this reader copies
        // the selected operand into the existing LayerFamilySource interface.
        // Reserve that simultaneous copy and the complete auxiliary namespace
        // before the first borrowed callback can allocate.
        if !input.shares_io_budget(original_io) {
            return Err(custody(std::io::Error::other(
                "candidate rule budget differs",
            )));
        }
        input.verify_invocation(limits.deadline, cancelled)?;
        let callback_state = retained_state_bytes
            .checked_add(std::mem::size_of::<Self>())
            .and_then(|bytes| bytes.checked_add(limits.max_auxiliary_state_bytes))
            .and_then(|bytes| bytes.checked_add(limits.max_member_bytes))
            .ok_or(tos_validation::item_budget_origin!())?;
        if callback_state > max_callback_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "candidate rule reader simultaneous callback state",
                used: u64::try_from(callback_state).ok(),
                limit: u64::try_from(max_callback_state_bytes).ok(),
            });
        }
        input.require_callback_state(callback_state, max_operation_state_bytes)?;
        let mut source = Self::new_inner(
            FoundationRuleInput::Candidate(input),
            physical,
            schemas,
            payloads,
            cancelled,
            limits,
        )?;
        source.original_io = Some(original_io.clone());
        Ok(source)
    }

    fn new_inner(
        input: FoundationRuleInput<'a>,
        physical: &'a mut RouteSources,
        schemas: &'a mut dyn CutSchemaExecutor,
        payloads: &'a mut dyn CutLayerPayloadReader,
        cancelled: &'cancel AtomicBool,
        limits: FoundationRuleReadLimits,
    ) -> Result<Self, ItemRefusal> {
        if limits.max_member_bytes == 0
            || limits.max_read_bytes == 0
            || limits.max_auxiliary_paths == 0
            || limits.max_auxiliary_state_bytes == 0
        {
            return Err(tos_validation::item_budget_origin!());
        }
        let selected = Self {
            input,
            physical,
            schemas,
            payloads,
            cancelled,
            limits,
            read_bytes: 0,
            original_io: None,
            shared_read_bytes_returned: 0,
            auxiliary: BTreeMap::new(),
            auxiliary_state_bytes: 0,
            history: None,
        };
        selected.checkpoint(selected.limits.deadline)?;
        Ok(selected)
    }

    pub(crate) fn with_history(
        mut self,
        history: &'a mut (dyn FoundationHistoricalEvidence + 'static),
    ) -> Result<Self, ItemRefusal> {
        // A historical capability may never shadow a proposed authored member.
        match &self.input {
            FoundationRuleInput::Cut(cut) => {
                for member in cut.current().members() {
                    self.checkpoint(self.limits.deadline)?;
                    if history.selected(member.path.as_str()) {
                        return Err(ItemRefusal::Source(
                            "historical input overlaps candidate".into(),
                        ));
                    }
                }
            }
            FoundationRuleInput::Candidate(input) => {
                input.for_each_current_member_meta(
                    self.limits.deadline,
                    self.cancelled,
                    &mut |member| {
                        reader_checkpoint(self.limits.deadline, self.cancelled)?;
                        if history.selected(member.path) {
                            return Err(ItemRefusal::Source(
                                "historical input overlaps candidate".into(),
                            ));
                        }
                        Ok(())
                    },
                )?;
            }
        }
        self.history = Some(history);
        Ok(self)
    }

    fn historical_read(
        &mut self,
        path: &str,
        digest: Option<&str>,
        requested: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        self.checkpoint(deadline)?;
        let cap = self.remaining(requested)?;
        let state = self
            .limits
            .max_auxiliary_state_bytes
            .checked_sub(self.auxiliary_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let history = self.history.as_deref_mut().ok_or(ItemRefusal::Budget)?;
        let shared_before = match self.original_io.as_ref() {
            Some(io) if history.shared_io_budget_matches(io) => {
                history.shared_runtime_read_bytes_returned()
            }
            Some(_) => {
                return Err(ItemRefusal::Source(
                    "historical original IO binding differs".into(),
                ));
            }
            None => 0,
        };
        let before = history.usage();
        let result = history.read(
            path,
            digest,
            cap,
            state,
            deadline.min(self.limits.deadline),
            self.cancelled,
        );
        let after = history.usage();
        if self.original_io.is_some() {
            self.shared_read_bytes_returned = self
                .shared_read_bytes_returned
                .checked_add(
                    history
                        .shared_runtime_read_bytes_returned()
                        .checked_sub(shared_before)
                        .ok_or(ItemRefusal::Budget)?,
                )
                .ok_or(ItemRefusal::Budget)?;
        }
        let delta = after.0.checked_sub(before.0).ok_or(ItemRefusal::Budget)?;
        // Charge observed provider work even if the selected read refuses.
        self.read_bytes = self
            .read_bytes
            .checked_add(delta)
            .filter(|n| *n <= self.limits.max_read_bytes)
            .ok_or(ItemRefusal::Budget)?;
        self.auxiliary_state_bytes = self
            .auxiliary_state_bytes
            .checked_add(after.1.checked_sub(before.1).ok_or(ItemRefusal::Budget)?)
            .filter(|bytes| *bytes <= self.limits.max_auxiliary_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let raw = result.map_err(custody)?;
        if raw.as_ref().is_some_and(|raw| {
            raw.len() > cap
                || digest.is_some_and(|expected| Digest256::of_bytes(raw).to_hex() != expected)
        }) {
            return Err(ItemRefusal::Source(
                "historical input binding differs".into(),
            ));
        }
        self.checkpoint(deadline)?;
        Ok(raw)
    }

    fn remaining(&self, requested: usize) -> Result<usize, ItemRefusal> {
        let remaining = self
            .limits
            .max_read_bytes
            .checked_sub(self.read_bytes)
            .ok_or(ItemRefusal::Budget)?;
        Ok(requested
            .min(self.limits.max_member_bytes)
            .min(usize::try_from(remaining).unwrap_or(usize::MAX)))
    }

    fn auxiliary_read(
        &mut self,
        path: &str,
        requested: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        self.checkpoint(deadline)?;
        let cap = self.remaining(requested)?;
        if !self.auxiliary.contains_key(path) {
            let state = self
                .auxiliary_state_bytes
                .checked_add(path.len())
                .and_then(|n| {
                    n.checked_add(std::mem::size_of::<(String, Option<(Digest256, u64)>)>() + 64)
                })
                .filter(|n| *n <= self.limits.max_auxiliary_state_bytes)
                .ok_or(ItemRefusal::Budget)?;
            if self.auxiliary.len() >= self.limits.max_auxiliary_paths {
                return Err(ItemRefusal::Budget);
            }
            self.auxiliary_state_bytes = state;
        }
        if !self.physical.is_file(path).map_err(custody)? {
            if self.auxiliary.get(path).is_some_and(Option::is_some) {
                return Err(ItemRefusal::Source(
                    "foundation auxiliary input disappeared".into(),
                ));
            }
            if !self.auxiliary.contains_key(path) {
                self.auxiliary.insert(path.to_owned(), None);
            }
            return Ok(None);
        }
        let mut read = 0;
        let raw = read_auxiliary_bytes(
            self.physical,
            path,
            cap,
            &mut read,
            self.original_io.as_ref(),
            &mut self.shared_read_bytes_returned,
            self.limits.deadline,
            self.cancelled,
        )?;
        self.read_bytes = self
            .read_bytes
            .checked_add(read as u64)
            .filter(|n| *n <= self.limits.max_read_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let stamp = Some((Digest256::of_bytes(&raw), raw.len() as u64));
        if self
            .auxiliary
            .get(path)
            .is_some_and(|before| before != &stamp)
        {
            return Err(ItemRefusal::Source(
                "foundation auxiliary bytes changed".into(),
            ));
        }
        if let Some(selected) = self.auxiliary.get_mut(path) {
            *selected = stamp;
        } else {
            self.auxiliary.insert(path.to_owned(), stamp);
        }
        self.checkpoint(deadline)?;
        Ok(Some(raw))
    }

    /// Verify every separately selected software/private-content byte operand.
    /// This repeated physical read belongs in the whole operation envelope.
    pub(crate) fn recheck_auxiliary(&mut self) -> Result<(), ItemRefusal> {
        recheck_auxiliary_inputs(
            self.physical,
            &self.auxiliary,
            &self.limits,
            &mut self.read_bytes,
            self.original_io.as_ref(),
            &mut self.shared_read_bytes_returned,
            self.cancelled,
        )
    }
}

struct RuleSharedReadHooks<'a> {
    original_io: &'a tos_source_store::PinnedSqliteIoBudget,
    returned: &'a mut u64,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl RouteSourceReadHooks for RuleSharedReadHooks<'_> {
    fn before_read(&mut self, requested: u64) -> std::io::Result<()> {
        reader_checkpoint(self.deadline, self.cancelled)
            .map_err(|_| std::io::Error::other("rule read context refused"))?;
        self.original_io
            .charge_read(requested)
            .map_err(|_| std::io::Error::other("rule read permit refused"))?;
        reader_checkpoint(self.deadline, self.cancelled)
            .map_err(|_| std::io::Error::other("rule read context refused"))
    }
    fn read_returned(&mut self, actual: u64) -> std::io::Result<()> {
        self.original_io
            .record_read_returned(actual)
            .map_err(|_| std::io::Error::other("rule read return refused"))?;
        *self.returned = self
            .returned
            .checked_add(actual)
            .ok_or_else(|| std::io::Error::other("rule return overflow"))?;
        reader_checkpoint(self.deadline, self.cancelled)
            .map_err(|_| std::io::Error::other("rule read context refused"))
    }
}
fn read_auxiliary_bytes(
    physical: &mut RouteSources,
    path: &str,
    cap: usize,
    read: &mut usize,
    original_io: Option<&tos_source_store::PinnedSqliteIoBudget>,
    returned: &mut u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, ItemRefusal> {
    match original_io {
        Some(original_io) => physical
            .bounded_metadata_bytes_with_hooks(
                path,
                cap,
                read,
                cap,
                &mut RuleSharedReadHooks {
                    original_io,
                    returned,
                    deadline,
                    cancelled,
                },
            )
            .map(|(raw, _)| raw)
            .map_err(custody),
        None => physical
            .bounded_bytes(path, cap, read, cap)
            .map_err(custody),
    }
}

fn custody(_: std::io::Error) -> ItemRefusal {
    ItemRefusal::Source("foundation held physical input custody refused".into())
}
fn allowed(path: &str) -> Result<(), ItemRefusal> {
    RelativePath::parse(path)
        .map_err(|_| ItemRefusal::Source("foundation relative input path".into()))?;
    if path.starts_with("ToS/") || path.starts_with("scripts/") {
        Ok(())
    } else {
        Err(ItemRefusal::Source(
            "foundation input owner namespace".into(),
        ))
    }
}
fn builder_name(path: &str) -> Option<&str> {
    let name = path.strip_prefix("scripts/")?.strip_suffix(".py")?;
    if name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
    {
        Some(name)
    } else {
        None
    }
}
fn contract_name(path: &str) -> bool {
    let Some(name) = path
        .strip_prefix("ToS/contracts/")
        .and_then(|n| n.strip_suffix(".schema.json"))
    else {
        return false;
    };
    name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

impl LayerFamilySource for FoundationRuleSource<'_, '_> {
    fn record_selection(
        &self,
    ) -> Option<std::sync::Arc<tos_validation::source_record_selection::SourceRecordSelection>>
    {
        match &self.input {
            FoundationRuleInput::Candidate(input) => input.record_selection(),
            FoundationRuleInput::Cut(_) => None,
        }
    }
    fn generated_selection(
        &self,
    ) -> Option<std::sync::Arc<dyn tos_validation::record_biblio_cut::GeneratedSourceSelection>>
    {
        match &self.input {
            FoundationRuleInput::Candidate(input) => input.generated_selection(),
            FoundationRuleInput::Cut(_) => None,
        }
    }
    fn current(
        &mut self,
        path: &str,
        requested: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        self.checkpoint(deadline)?;
        allowed(path)?;
        if !self.selects_required_member(path)? {
            return Ok(None);
        }
        if self
            .record_selection()
            .is_some_and(|selection| !selection.contains_member(path))
        {
            return Ok(None);
        }
        if self
            .history
            .as_deref()
            .is_some_and(|history| history.selected(path))
        {
            let raw = self.historical_read(path, None, requested, deadline)?;
            if let (Some(selection), Some(bytes)) = (self.record_selection(), raw.as_ref()) {
                if selection.contains_member(path) {
                    selection.verify_metadata_member(path, bytes)?;
                }
            }
            return Ok(raw);
        }
        if is_authored_source_path_v1(path) {
            let relative = RelativePath::parse(path)
                .map_err(|_| ItemRefusal::Source("foundation current input path".into()))?;
            let cap = self.remaining(requested)?;
            let raw = match &self.input {
                FoundationRuleInput::Cut(cut) => {
                    let Some(metadata) = cut.current().member(&relative) else {
                        return Ok(None);
                    };
                    if metadata.size_bytes > cap as u64 {
                        return Err(ItemRefusal::Budget);
                    }
                    cut.read_member(
                        cut.current().revision(),
                        &relative,
                        cap as u64,
                        deadline.min(self.limits.deadline),
                        self.cancelled,
                    )
                    .map_err(|_| ItemRefusal::Source("foundation immutable input custody".into()))?
                    .raw
                }
                FoundationRuleInput::Candidate(input) => {
                    let until = deadline.min(self.limits.deadline);
                    if input.path_presence(path, until, self.cancelled)?
                        != Some(tos_source_store::SourcePresenceV1::File)
                    {
                        return Ok(None);
                    }
                    let mut selected = None;
                    input.with_current_member(
                        path,
                        cap,
                        until,
                        self.cancelled,
                        &mut |meta, bytes| {
                            if selected.is_some()
                                || meta.path != path
                                || meta.size_bytes != bytes.len() as u64
                                || bytes.len() > cap
                            {
                                return Err(ItemRefusal::Source(
                                    "foundation candidate member custody".into(),
                                ));
                            }
                            if let Some(selection) = input.record_selection() {
                                if selection.contains_member(path) {
                                    selection.verify_metadata_member(path, bytes)?;
                                }
                            }
                            let mut raw = Vec::new();
                            raw.try_reserve_exact(bytes.len())
                                .map_err(|_| ItemRefusal::Budget)?;
                            if raw.capacity() > cap {
                                return Err(ItemRefusal::Budget);
                            }
                            for chunk in bytes.chunks(64 * 1024) {
                                reader_checkpoint(until, self.cancelled)?;
                                raw.extend_from_slice(chunk);
                            }
                            selected = Some(raw);
                            Ok(())
                        },
                    )?;
                    selected.ok_or_else(|| {
                        ItemRefusal::Source("foundation candidate member callback missing".into())
                    })?
                }
            };
            if matches!(self.input, FoundationRuleInput::Candidate(_)) {
                self.shared_read_bytes_returned = self
                    .shared_read_bytes_returned
                    .checked_add(raw.len() as u64)
                    .ok_or(ItemRefusal::Budget)?;
            }
            self.read_bytes = self
                .read_bytes
                .checked_add(raw.len() as u64)
                .filter(|n| *n <= self.limits.max_read_bytes)
                .ok_or(ItemRefusal::Budget)?;
            Ok(Some(raw))
        } else {
            // Explicit private text/software operands are physical selections;
            // their absence is never inferred from the authored cut.
            self.auxiliary_read(path, requested, deadline)
        }
    }

    fn recorded(
        &mut self,
        path: &str,
        digest: &str,
        requested: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        allowed(path)?;
        if !self.selects_required_member(path)? {
            return Ok(None);
        }
        let Ok(expected) = Digest256::from_hex(digest) else {
            return Ok(None);
        };
        if self
            .history
            .as_deref()
            .is_some_and(|history| history.selected(path))
        {
            return self.historical_read(path, Some(digest), requested, deadline);
        }
        // Historical scripts may be retired while their exact authored
        // captures remain. This cold reader selects no current producer
        // components; current-only components stay in CutProvenanceSource.
        let current = self.current(path, requested, deadline)?;
        if let Some(raw) = current.as_ref() {
            if Digest256::of_bytes(raw) == expected {
                return Ok(current);
            }
        }
        let (archived, schema) = if let Some(builder) = builder_name(path) {
            (
                format!("ToS/research-packets/retained-builder-inputs/{builder}/{digest}.py"),
                false,
            )
        } else if contract_name(path) {
            // Schema retirement is outside the accepted script-only change.
            if current.is_none() {
                return Ok(None);
            }
            (format!("ToS/contracts/history/{digest}.json"), true)
        } else {
            return Ok(None);
        };
        let Some(raw) = self.current(&archived, requested.min(1_048_576), deadline)? else {
            return Ok(None);
        };
        if Digest256::of_bytes(&raw) != expected {
            return Ok(None);
        }
        if schema {
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&raw) else {
                return Ok(None);
            };
            if !value.is_object()
                || value["$id"].as_str()
                    != Some(format!("https://tree-of-sophia.local/{path}").as_str())
            {
                return Ok(None);
            }
        }
        Ok(Some(raw))
    }

    fn schema(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
    ) -> Result<bool, ItemRefusal> {
        self.checkpoint(deadline)?;
        self.schemas.check(
            path,
            raw,
            contract,
            deadline.min(self.limits.deadline),
            self.cancelled,
        )
    }
    fn exists(&mut self, path: &str, _: usize, deadline: Instant) -> Result<bool, ItemRefusal> {
        self.checkpoint(deadline)?;
        allowed(path)?;
        if !self.selects_required_member(path)? {
            return Ok(false);
        }
        let remaining_read = self
            .limits
            .max_read_bytes
            .checked_sub(self.read_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let remaining_state = self
            .limits
            .max_auxiliary_state_bytes
            .checked_sub(self.auxiliary_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        if let Some(history) = self
            .history
            .as_deref_mut()
            .filter(|history| history.selected(path))
        {
            let shared_before = match self.original_io.as_ref() {
                Some(io) if history.shared_io_budget_matches(io) => {
                    history.shared_runtime_read_bytes_returned()
                }
                Some(_) => {
                    return Err(ItemRefusal::Source(
                        "historical original IO binding differs".into(),
                    ));
                }
                None => 0,
            };
            let before = history.usage();
            let result = history.physical(
                path,
                remaining_read,
                remaining_state,
                deadline.min(self.limits.deadline),
                self.cancelled,
            );
            let after = history.usage();
            if self.original_io.is_some() {
                self.shared_read_bytes_returned = self
                    .shared_read_bytes_returned
                    .checked_add(
                        history
                            .shared_runtime_read_bytes_returned()
                            .checked_sub(shared_before)
                            .ok_or(ItemRefusal::Budget)?,
                    )
                    .ok_or(ItemRefusal::Budget)?;
            }
            self.read_bytes = self
                .read_bytes
                .checked_add(after.0.checked_sub(before.0).ok_or(ItemRefusal::Budget)?)
                .filter(|n| *n <= self.limits.max_read_bytes)
                .ok_or(ItemRefusal::Budget)?;
            self.auxiliary_state_bytes = self
                .auxiliary_state_bytes
                .checked_add(after.1.checked_sub(before.1).ok_or(ItemRefusal::Budget)?)
                .filter(|bytes| *bytes <= self.limits.max_auxiliary_state_bytes)
                .ok_or(ItemRefusal::Budget)?;
            self.checkpoint(deadline)?;
            return result
                .map(|facts| facts.is_some_and(|facts| facts.exists))
                .map_err(custody);
        }
        self.physical.exists(path).map_err(custody)
    }
    fn payload(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<LayerPayload, ItemRefusal> {
        self.checkpoint(deadline)?;
        allowed(path)?;
        self.payloads.inspect(
            path,
            max_bytes,
            deadline.min(self.limits.deadline),
            self.cancelled,
        )
    }
    fn cancellation(&self) -> &AtomicBool {
        self.cancelled
    }
    fn generation(&self) -> String {
        match &self.input {
            FoundationRuleInput::Cut(cut) => cut.current().revision().0.to_hex(),
            // Generation is an opaque layer namespace, not SourceRevision.
            // The fixed-size candidate fence includes the complete selection.
            FoundationRuleInput::Candidate(input) => {
                let fence = input.input_identity();
                let mut hash = Digest256Hasher::new();
                hash.update(b"tos-foundation-candidate-layer-generation-v1\0");
                hash.update(fence.batch_sha256.as_bytes());
                match fence.base_revision {
                    Some(revision) => {
                        hash.update(&[1]);
                        hash.update(revision.0.as_bytes());
                    }
                    None => hash.update(&[0]),
                }
                hash.update(fence.validator_sha256.as_bytes());
                hash.update(&fence.membership.count.to_be_bytes());
                hash.update(fence.membership.digest.as_bytes());
                hash.update(&fence.source_bytes.to_be_bytes());
                hash.update(&fence.retirement_count.to_be_bytes());
                hash.update(fence.retirement_digest.as_bytes());
                format!("candidate/{}", hash.finalize().to_hex())
            }
        }
    }
    fn checkpoint(&self, deadline: Instant) -> Result<(), ItemRefusal> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err(ItemRefusal::Source(
                "foundation validation cancelled".into(),
            ))
        } else if Instant::now() >= deadline.min(self.limits.deadline) {
            Err(ItemRefusal::Deadline)
        } else {
            Ok(())
        }
    }
}
