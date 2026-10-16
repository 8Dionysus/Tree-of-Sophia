//! Concrete start/advance continuation for knowledge exploration. The future
//! is request-local; only the separate ID/query state goes into checkpoints.
use crate::knowledge_exploration::{self as rules, ExplorationBudget, ExplorationState};
use crate::knowledge_lens_spec::{LensVocabulary, get, string};
use crate::search_v2::{SearchKind, SearchV2Error, SearchV2ErrorCode};
use crate::{AbortProbe, AbortReason, InspectBudget};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    future::{Future, poll_fn},
    pin::Pin,
    rc::{Rc, Weak},
    task::{Context, Poll, Waker},
};
use tos_foundation::JsonValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExplorationProfile {
    NativeSelected,
    PublishedD1,
}
#[derive(Clone, Debug)]
pub enum ExplorationNeed {
    Rows {
        kind: SearchKind,
        ids: Vec<String>,
        allow_missing: bool,
        allow_ambiguous: bool,
    },
    Focus {
        field: &'static str,
        identifier: String,
        sources: Vec<String>,
        source_priority: Vec<(String, u64)>,
        limit: usize,
    },
    IdentityPage {
        node_id: String,
        entity_id: Option<String>,
        expanded_entities: Vec<String>,
        declared_prefix: Option<&'static str>,
        after: String,
        sources: Vec<String>,
        limit: usize,
    },
    AdjacencyPage {
        node_id: String,
        after: String,
        limit: usize,
    },
}
#[derive(Debug)]
pub enum ExplorationReply {
    Rows {
        rows: Vec<JsonValue>,
        raw_bytes: Vec<usize>,
        ambiguous_ids: Vec<String>,
    },
    Focus {
        matched: usize,
        rows: Vec<JsonValue>,
        raw_bytes: Vec<usize>,
    },
    Ids(Vec<String>),
}
pub enum ExplorationInput {
    Start(JsonValue),
    Continue(ExplorationState),
}
pub struct ExplorationOutput {
    pub state: ExplorationState,
    pub packet: JsonValue,
}
#[derive(Clone, Copy, Debug)]
pub struct PublishedExplorationBudget {
    pub exploration: ExplorationBudget,
    pub max_cache_bytes: usize,
    pub max_cache_entries: usize,
}
fn corrupt(message: &'static str) -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::CorruptSelectedCarrier,
        message,
    }
}
fn exhausted() -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::BudgetExceeded,
        message: "exploration source/state budget exceeded",
    }
}
fn check(probe: &dyn AbortProbe) -> Result<(), SearchV2Error> {
    match probe.reason() {
        Some(AbortReason::Cancelled) => Err(SearchV2Error {
            code: SearchV2ErrorCode::Cancelled,
            message: "exploration cancelled",
        }),
        Some(AbortReason::DeadlineExceeded) => Err(SearchV2Error {
            code: SearchV2ErrorCode::DeadlineExceeded,
            message: "exploration deadline exceeded",
        }),
        None => Ok(()),
    }
}
struct Slot {
    need: Option<Rc<ExplorationNeed>>,
    reply: Option<Result<ExplorationReply, SearchV2Error>>,
    reply_bytes: usize,
    live: Rc<Cell<usize>>,
    budget: InspectBudget,
}
// One parsed-row directory: the LRU owns up to the cache profile, while weak
// entries reuse rows still held by a suspended page or final-selection batch.
// An evicted but live row is not parsed again. Its lexical bytes stay charged
// once until the last Rc drops; this is not an RSS/allocator measurement.
impl Drop for Slot {
    fn drop(&mut self) {
        self.live.set(
            self.live
                .get()
                .checked_sub(self.reply_bytes)
                .expect("pending exploration bytes charged once"),
        );
    }
}
pub(crate) struct RetainedRow {
    value: JsonValue,
    bytes: usize,
    live: Rc<Cell<usize>>,
}
impl std::ops::Deref for RetainedRow {
    type Target = JsonValue;
    fn deref(&self) -> &JsonValue {
        &self.value
    }
}
impl Drop for RetainedRow {
    fn drop(&mut self) {
        self.live.set(
            self.live
                .get()
                .checked_sub(self.bytes)
                .expect("retained exploration bytes charged once"),
        );
    }
}
pub(crate) struct Rows {
    slot: Rc<RefCell<Slot>>,
    cache: BTreeMap<(u8, String), (Weak<RetainedRow>, Option<Rc<RetainedRow>>, u64)>,
    cache_entries: usize,
    cache_bytes: usize,
    tick: u64,
    cache_limit: (usize, usize),
    probe: Rc<dyn AbortProbe>,
    pub(crate) profile: ExplorationProfile,
}
impl Rows {
    pub(crate) fn check(&self) -> Result<(), SearchV2Error> {
        check(self.probe.as_ref())
    }
    async fn read(&mut self, need: ExplorationNeed) -> Result<ExplorationReply, SearchV2Error> {
        self.check()?;
        let mut need = Some(need);
        let slot = self.slot.clone();
        let reply = poll_fn(move |_| {
            let mut slot = slot.borrow_mut();
            if let Some(need) = need.take() {
                if slot.need.is_some() || slot.reply.is_some() || slot.reply_bytes != 0 {
                    return Poll::Ready(Err(corrupt("exploration read slot occupied")));
                }
                slot.need = Some(Rc::new(need));
            }
            slot.reply.take().map_or(Poll::Pending, Poll::Ready)
        })
        .await?;
        self.check()?;
        Ok(reply)
    }
    fn retain(
        &mut self,
        kind: SearchKind,
        value: JsonValue,
        size: usize,
    ) -> Result<Rc<RetainedRow>, SearchV2Error> {
        let id = string(get(&value, "id")).to_owned();
        if value.as_object().is_none() || id.is_empty() {
            return Err(corrupt("exploration row identity invalid"));
        }
        let key = (if kind == SearchKind::Nodes { 0 } else { 1 }, id);
        let live = {
            let mut slot = self.slot.borrow_mut();
            slot.reply_bytes = slot
                .reply_bytes
                .checked_sub(size)
                .ok_or_else(|| corrupt("exploration row byte charge differs"))?;
            slot.live.clone()
        };
        let row = Rc::new(RetainedRow {
            value,
            bytes: size,
            live,
        });
        self.cache
            .retain(|_, (weak, owner, _)| owner.is_some() || weak.strong_count() != 0);
        if size <= self.cache_limit.0 {
            while self.cache_entries != 0
                && (self.cache_entries >= self.cache_limit.1
                    || self.cache_bytes.checked_add(size).ok_or_else(exhausted)?
                        > self.cache_limit.0)
            {
                let key = self
                    .cache
                    .iter()
                    .filter(|(_, (_, owner, _))| owner.is_some())
                    .min_by_key(|(_, (_, _, tick))| *tick)
                    .unwrap()
                    .0
                    .clone();
                let old = self.cache.get_mut(&key).unwrap().1.take().unwrap();
                self.cache_bytes -= old.bytes;
                self.cache_entries -= 1;
            }
            self.cache_bytes = self.cache_bytes.checked_add(size).ok_or_else(exhausted)?;
            self.cache_entries += 1;
        }
        self.tick = self.tick.checked_add(1).ok_or_else(exhausted)?;
        if self
            .cache
            .get(&key)
            .is_some_and(|(weak, _, _)| weak.strong_count() != 0)
        {
            return Err(corrupt("exploration retained row duplicated"));
        }
        self.cache.insert(
            key,
            (
                Rc::downgrade(&row),
                (size <= self.cache_limit.0).then(|| row.clone()),
                self.tick,
            ),
        );
        Ok(row)
    }
    async fn load(
        &mut self,
        kind: SearchKind,
        ids: &[String],
        allow_missing: bool,
        allow_ambiguous: bool,
    ) -> Result<Vec<Rc<RetainedRow>>, SearchV2Error> {
        let tag = if kind == SearchKind::Nodes { 0 } else { 1 };
        let mut result = BTreeMap::new();
        let mut missing = vec![];
        for id in ids {
            self.tick = self.tick.checked_add(1).ok_or_else(exhausted)?;
            if let Some((weak, _, tick)) = self.cache.get_mut(&(tag, id.clone())) {
                *tick = self.tick;
                if let Some(row) = weak.upgrade() {
                    result.insert(id.clone(), row);
                    continue;
                }
            }
            missing.push(id.clone());
        }
        if !missing.is_empty() {
            let reply = self
                .read(ExplorationNeed::Rows {
                    kind,
                    ids: missing.clone(),
                    allow_missing,
                    allow_ambiguous,
                })
                .await?;
            let ExplorationReply::Rows {
                rows,
                raw_bytes,
                ambiguous_ids,
            } = reply
            else {
                return Err(corrupt("exploration rows reply expected"));
            };
            if ambiguous_ids.iter().any(|id| !missing.contains(id))
                || (!allow_ambiguous && !ambiguous_ids.is_empty())
            {
                return Err(corrupt("exploration exact row ambiguous"));
            }
            for (value, size) in rows.into_iter().zip(raw_bytes) {
                let id = string(get(&value, "id")).to_owned();
                if !missing.contains(&id) || result.contains_key(&id) || ambiguous_ids.contains(&id)
                {
                    return Err(corrupt("exploration rows selection differs"));
                }
                result.insert(id, self.retain(kind, value, size)?);
            }
            if !allow_missing && missing.iter().any(|id| !result.contains_key(id)) {
                return Err(corrupt("exploration required row missing"));
            }
        }
        Ok(ids.iter().filter_map(|id| result.remove(id)).collect())
    }
    pub(crate) async fn item(
        &mut self,
        kind: SearchKind,
        id: &str,
    ) -> Result<Option<Rc<RetainedRow>>, SearchV2Error> {
        Ok(self.load(kind, &[id.to_owned()], true, false).await?.pop())
    }
    pub(crate) async fn item_origin(
        &mut self,
        kind: SearchKind,
        id: &str,
    ) -> Result<Option<Rc<RetainedRow>>, SearchV2Error> {
        Ok(self.load(kind, &[id.to_owned()], true, true).await?.pop())
    }
    pub(crate) async fn optional(
        &mut self,
        kind: SearchKind,
        ids: &[String],
    ) -> Result<Vec<Rc<RetainedRow>>, SearchV2Error> {
        self.load(kind, ids, true, false).await
    }
    pub(crate) async fn node(&mut self, id: &str) -> Result<Rc<RetainedRow>, SearchV2Error> {
        self.item(SearchKind::Nodes, id)
            .await?
            .ok_or_else(|| corrupt("exploration exact node missing"))
    }
    pub(crate) async fn relation(&mut self, id: &str) -> Result<Rc<RetainedRow>, SearchV2Error> {
        self.item(SearchKind::Relations, id)
            .await?
            .ok_or_else(|| corrupt("exploration exact relation missing"))
    }
    pub(crate) async fn focus(
        &mut self,
        id: &str,
        sources: &[String],
        vocab: &LensVocabulary,
        max_matches: usize,
    ) -> Result<Rc<RetainedRow>, SearchV2Error> {
        for field in ["id", "entity_id", "native_id"] {
            let limit = if self.profile == ExplorationProfile::PublishedD1 {
                if field == "native_id" { 2 } else { 1 }
            } else if field == "id" {
                1
            } else {
                max_matches.checked_add(1).ok_or_else(exhausted)?
            };
            let reply = self
                .read(ExplorationNeed::Focus {
                    field,
                    identifier: id.into(),
                    sources: sources.to_vec(),
                    source_priority: vocab
                        .carrier_source_priority
                        .iter()
                        .map(|(s, p)| (s.clone(), *p))
                        .collect(),
                    limit,
                })
                .await?;
            let ExplorationReply::Focus {
                matched,
                rows,
                raw_bytes,
            } = reply
            else {
                return Err(corrupt("exploration focus rows expected"));
            };
            if self.profile == ExplorationProfile::NativeSelected && matched > max_matches {
                return Err(exhausted());
            }
            if self.profile == ExplorationProfile::PublishedD1
                && field == "native_id"
                && matched > 1
            {
                return Err(SearchV2Error {
                    code: SearchV2ErrorCode::InvalidRequest,
                    message: "ambiguous exploration native focus",
                });
            }
            if rows.len() > matched
                || (self.profile == ExplorationProfile::PublishedD1 && rows.len() != matched)
            {
                return Err(corrupt("exploration focus cardinality differs"));
            }
            let mut scoped = vec![];
            for (value, size) in rows.into_iter().zip(raw_bytes) {
                if string(get(&value, field)) != id
                    || !sources
                        .iter()
                        .any(|s| s == string(get(&value, "source_graph")))
                {
                    return Err(corrupt("exploration focus selection differs"));
                }
                scoped.push(self.retain(SearchKind::Nodes, value, size)?);
            }
            if scoped.is_empty() {
                continue;
            }
            if field == "native_id" && scoped.len() > 1 {
                return Err(SearchV2Error {
                    code: SearchV2ErrorCode::InvalidRequest,
                    message: "ambiguous exploration native focus",
                });
            }
            if field == "entity_id" {
                scoped
                    .sort_by_key(|node| (vocab.priority(node), string(get(node, "id")).to_owned()));
            }
            return Ok(scoped.remove(0));
        }
        Err(SearchV2Error {
            code: SearchV2ErrorCode::InvalidRequest,
            message: "unknown exploration focus",
        })
    }
    pub(crate) async fn identities(
        &mut self,
        node: &str,
        entity: Option<&str>,
        expanded: &[String],
        sources: &[String],
        after: &str,
        limit: usize,
    ) -> Result<Vec<String>, SearchV2Error> {
        self.ids(
            ExplorationNeed::IdentityPage {
                node_id: node.into(),
                entity_id: entity.map(str::to_owned),
                expanded_entities: expanded.to_vec(),
                declared_prefix: if self.profile == ExplorationProfile::PublishedD1 {
                    Some("tos.")
                } else {
                    None
                },
                sources: sources.to_vec(),
                after: after.into(),
                limit,
            },
            after,
            limit,
        )
        .await
    }
    pub(crate) async fn adjacent(
        &mut self,
        node: &str,
        after: &str,
        limit: usize,
    ) -> Result<Vec<String>, SearchV2Error> {
        self.ids(
            ExplorationNeed::AdjacencyPage {
                node_id: node.into(),
                after: after.into(),
                limit,
            },
            after,
            limit,
        )
        .await
    }
    async fn ids(
        &mut self,
        need: ExplorationNeed,
        after: &str,
        limit: usize,
    ) -> Result<Vec<String>, SearchV2Error> {
        let reply = self.read(need).await?;
        let ExplorationReply::Ids(ids) = reply else {
            return Err(corrupt("exploration IDs reply expected"));
        };
        if ids.len() > limit
            || ids
                .iter()
                .any(|id| id.is_empty() || id.len() > self.slot.borrow().budget.max_field_bytes)
            || ids.first().is_some_and(|id| id.as_str() <= after)
            || ids.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(corrupt("exploration seek IDs invalid"));
        }
        Ok(ids)
    }
    pub(crate) async fn selected(
        &mut self,
        kind: SearchKind,
        ids: &[String],
    ) -> Result<Vec<Rc<RetainedRow>>, SearchV2Error> {
        self.load(kind, ids, false, false).await
    }
}

pub struct ExplorationPlan {
    slot: Rc<RefCell<Slot>>,
    execution: Option<Pin<Box<dyn Future<Output = Result<ExplorationOutput, SearchV2Error>>>>>,
    output: Option<ExplorationOutput>,
    terminal: bool,
    probe: Rc<dyn AbortProbe>,
}
impl ExplorationPlan {
    pub fn published(
        input: ExplorationInput,
        source_revision: &str,
        data_revision: &str,
        epoch: u64,
        authority_boundary: JsonValue,
        budget: PublishedExplorationBudget,
        probe: Rc<dyn AbortProbe>,
    ) -> Result<Self, SearchV2Error> {
        if epoch > 9_007_199_254_740_991
            || !rules::bare(source_revision)
            || !rules::bare(data_revision)
            || authority_boundary.as_object().is_none()
            || budget.max_cache_bytes == 0
            || budget.max_cache_bytes > 2 * 1024 * 1024
            || budget.max_cache_entries == 0
            || budget.max_cache_entries > 64
        {
            return Err(corrupt("exploration publication admission invalid"));
        }
        let snapshot = rules::published_exploration_snapshot(
            data_revision,
            epoch,
            budget.exploration.read.json,
        )?;
        Self::new(
            input,
            source_revision,
            &snapshot,
            authority_boundary,
            LensVocabulary::published_shape(),
            budget.exploration,
            ExplorationProfile::PublishedD1,
            (budget.max_cache_bytes, budget.max_cache_entries),
            probe,
        )
    }
    pub(crate) fn native(
        input: ExplorationInput,
        revision: &str,
        snapshot: &str,
        boundary: JsonValue,
        vocab: LensVocabulary,
        budget: ExplorationBudget,
        probe: Rc<dyn AbortProbe>,
    ) -> Result<Self, SearchV2Error> {
        Self::new(
            input,
            revision,
            snapshot,
            boundary,
            vocab,
            budget,
            ExplorationProfile::NativeSelected,
            (
                usize::try_from(budget.read.max_decoded_bytes).map_err(|_| exhausted())?,
                usize::MAX,
            ),
            probe,
        )
    }
    fn new(
        input: ExplorationInput,
        revision: &str,
        snapshot: &str,
        boundary: JsonValue,
        vocab: LensVocabulary,
        budget: ExplorationBudget,
        profile: ExplorationProfile,
        cache_limit: (usize, usize),
        probe: Rc<dyn AbortProbe>,
    ) -> Result<Self, SearchV2Error> {
        rules::validate_budget(budget)?;
        check(probe.as_ref())?;
        let slot = Rc::new(RefCell::new(Slot {
            need: None,
            reply: None,
            reply_bytes: 0,
            live: Rc::new(Cell::new(0)),
            budget: budget.read,
        }));
        let rows = Rows {
            slot: slot.clone(),
            cache: BTreeMap::new(),
            cache_bytes: 0,
            cache_entries: 0,
            tick: 0,
            cache_limit,
            probe: probe.clone(),
            profile,
        };
        let revision = revision.to_owned();
        let snapshot = snapshot.to_owned();
        let execution = Box::pin(async move {
            let mut rows = rows;
            let mut state = match input {
                ExplorationInput::Start(request) => {
                    rules::start(
                        rules::normalize_for_profile(&request, &vocab, profile)?,
                        &revision,
                        &snapshot,
                        &mut rows,
                        &vocab,
                        budget,
                    )
                    .await?
                }
                ExplorationInput::Continue(state) => {
                    if state.snapshot_revision() != snapshot || state.profile() != profile {
                        return Err(SearchV2Error {
                            code: SearchV2ErrorCode::StaleContinuation,
                            message: "exploration continuation binding changed",
                        });
                    }
                    if state
                        .requested_revision()
                        .is_some_and(|expected| expected != revision)
                    {
                        return Err(corrupt("exploration checkpoint source revision differs"));
                    }
                    state
                }
            };
            let packet =
                rules::advance(&mut state, &mut rows, &vocab, &revision, &boundary, budget).await?;
            // Host consumers perform the one actual state/response emission:
            // published before CAS, native before disclosure/checkpoint commit.
            Ok(ExplorationOutput { state, packet })
        });
        Ok(Self {
            slot,
            execution: Some(execution),
            output: None,
            terminal: false,
            probe,
        })
    }
    /// One step to a concrete need or completion; never repoll without reply.
    pub fn advance(&mut self) -> Result<bool, SearchV2Error> {
        if self.terminal || self.slot.borrow().need.is_some() {
            return Err(corrupt("exploration plan is terminal or awaiting reply"));
        }
        check(self.probe.as_ref())?;
        let result = self
            .execution
            .as_mut()
            .ok_or_else(|| corrupt("exploration future unavailable"))?
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()));
        match result {
            Poll::Pending => {
                if self.slot.borrow().need.is_none() {
                    self.clear();
                    return Err(corrupt("exploration suspension lacks need"));
                }
                Ok(false)
            }
            Poll::Ready(result) => {
                self.clear();
                self.output = Some(result?);
                Ok(true)
            }
        }
    }
    pub fn need(&self) -> Option<Rc<ExplorationNeed>> {
        self.slot.borrow().need.clone()
    }
    pub fn resume(&mut self, reply: ExplorationReply) -> Result<(), SearchV2Error> {
        if self.terminal {
            return Err(corrupt("exploration plan already terminal"));
        }
        check(self.probe.as_ref())?;
        let mut slot = self.slot.borrow_mut();
        let need = slot
            .need
            .as_ref()
            .ok_or_else(|| corrupt("exploration reply lacks need"))?;
        let bytes = match (&**need, &reply) {
            (
                ExplorationNeed::Rows { ids, .. },
                ExplorationReply::Rows {
                    rows,
                    raw_bytes,
                    ambiguous_ids,
                },
            ) if rows.len() <= ids.len() && ambiguous_ids.len() <= ids.len() => {
                row_bytes(rows, raw_bytes, slot.budget)?
            }
            (
                ExplorationNeed::Focus { limit, .. },
                ExplorationReply::Focus {
                    matched,
                    rows,
                    raw_bytes,
                },
            ) if rows.len() <= *limit && *matched <= *limit => {
                row_bytes(rows, raw_bytes, slot.budget)?
            }
            (
                ExplorationNeed::IdentityPage { limit, .. }
                | ExplorationNeed::AdjacencyPage { limit, .. },
                ExplorationReply::Ids(ids),
            ) if ids.len() <= *limit => {
                let bytes = ids.iter().try_fold(0usize, |n, id| {
                    n.checked_add(id.len()).ok_or_else(exhausted)
                })?;
                if bytes as u64 > slot.budget.max_decoded_bytes {
                    return Err(exhausted());
                }
                0
            }
            _ => return Err(corrupt("exploration reply kind/count differs")),
        };
        let live = slot.live.get().checked_add(bytes).ok_or_else(exhausted)?;
        if live as u64 > slot.budget.max_decoded_bytes {
            return Err(exhausted());
        }
        slot.live.set(live);
        slot.reply_bytes = bytes;
        slot.need = None;
        slot.reply = Some(Ok(reply));
        Ok(())
    }
    pub fn finish(mut self) -> Result<ExplorationOutput, SearchV2Error> {
        self.output
            .take()
            .ok_or_else(|| corrupt("exploration plan not complete"))
    }
    fn clear(&mut self) {
        self.execution = None;
        let mut slot = self.slot.borrow_mut();
        slot.need = None;
        slot.reply = None;
        slot.live.set(
            slot.live
                .get()
                .checked_sub(slot.reply_bytes)
                .expect("exploration pending charge"),
        );
        slot.reply_bytes = 0;
        self.terminal = true;
    }
}
impl Drop for ExplorationPlan {
    fn drop(&mut self) {
        self.clear();
    }
}
fn row_bytes(
    rows: &[JsonValue],
    sizes: &[usize],
    budget: InspectBudget,
) -> Result<usize, SearchV2Error> {
    if rows.len() != sizes.len()
        || rows.len() as u64 > budget.max_rows
        || sizes
            .iter()
            .any(|n| *n == 0 || *n > budget.max_payload_bytes)
    {
        return Err(corrupt("exploration original row framing invalid"));
    }
    sizes
        .iter()
        .try_fold(0usize, |n, size| n.checked_add(*size).ok_or_else(exhausted))
}
