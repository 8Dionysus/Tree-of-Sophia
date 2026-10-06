//! One bounded lens continuation. The native reader and asynchronous published
//! reader supply the same concrete reads; storage custody stays with callers.
//! The compiler retains traversal/path state between needs. No operation is
//! replayed and this is not an executor for other query families.
use crate::{
    inspect_plan::{AbortProbe, AbortReason},
    knowledge_lens::{LensBudget, LensExecutionCounts, finalize_knowledge_lens},
    knowledge_lens_spec::*,
    search_v2::{SearchKind, SearchV2Error, SearchV2ErrorCode},
    source_read_projection::{object, text},
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};
use tos_foundation::{CanonicalProfile, JsonValue, canonical_bytes_v1};

/// The native controlled driver borrows this guard from its original State
/// owner. It admits bounded thin plan frames before construction. Exact row
/// and reply copies are additionally admitted by that same driver/heap before
/// resume; all these holds outlive continuations and final packet delivery.
/// Query VM remains in the controlled reader, never in this pure plan.
pub(crate) trait OriginalLensBudget {
    fn check(&self) -> Result<(), SearchV2Error>;
    fn charge_work(&self, units: usize) -> Result<(), SearchV2Error>;
    fn admit_workspace(&self, bytes: usize) -> Result<(), SearchV2Error>;
    fn canonicalize(
        &self,
        value: &JsonValue,
        limits: tos_foundation::JsonLimits,
    ) -> Result<Vec<u8>, SearchV2Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LensCandidateCursor {
    Id(String),
    SourceOrder { source: String, position: i64 },
}
#[derive(Clone, Debug)]
pub struct LensCandidate {
    pub id: String,
    pub source: Option<String>,
    pub position: Option<i64>,
}
#[derive(Clone, Debug)]
pub struct LensCandidatePage {
    pub rows: Vec<LensCandidate>,
}
#[derive(Clone, Debug)]
pub struct LensIdentityTerm {
    pub field: String,
    pub values: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct LensIdentityGroup {
    pub all: bool,
    pub terms: Vec<LensIdentityTerm>,
}
#[derive(Clone, Debug)]
pub enum LensCandidateIndex {
    Source,
    Identity(String),
    Union,
}
#[derive(Clone, Debug)]
pub struct LensMembershipTerm {
    pub field: String,
    pub all: bool,
    pub values: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct LensMembership {
    pub all: bool,
    pub terms: Vec<LensMembershipTerm>,
    pub drivers: Vec<(String, String)>,
}
#[derive(Clone, Debug)]
pub enum LensEndpoint {
    From(String),
    To(String),
}
#[derive(Clone, Debug)]
pub struct LensHeader {
    pub id: String,
    pub sort_key: String,
    pub from_id: String,
    pub to_id: String,
}
#[derive(Clone, Debug)]
pub enum LensEligibility {
    Both {
        basis: Vec<String>,
        traversed: Vec<String>,
        pair_index: bool,
    },
    Either {
        basis: Vec<String>,
        traversed: Vec<String>,
    },
}
#[derive(Clone, Debug)]
pub struct LensHeaderQuery {
    pub kind: SearchKind,
    pub sources: Vec<String>,
    pub dimensions: Option<Vec<[String; 3]>>,
    pub membership: Option<LensMembership>,
    pub predicate_ids: Vec<String>,
    pub excluded_predicates: Vec<String>,
    pub excluded_relation_types: Vec<String>,
    pub endpoint: Option<LensEndpoint>,
    pub eligible: Option<LensEligibility>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LensRepresentation {
    Full,
    CoveredCompact,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct LensStores {
    pub compact: bool,
    pub membership: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct PublishedLensBudget {
    pub lens: LensBudget,
    pub max_callbacks: usize,
    pub max_sort_bytes: usize,
    pub max_cache_bytes: usize,
    pub max_cache_entries: usize,
}

#[derive(Clone, Debug)]
pub enum LensNeed {
    Auxiliary {
        compact: bool,
        membership: bool,
    },
    ExactRows {
        kind: SearchKind,
        ids: Vec<String>,
        representation: LensRepresentation,
    },
    LookupRows {
        field: String,
        identifier: String,
        limit: usize,
    },
    CandidateIds {
        kind: SearchKind,
        sources: Vec<String>,
        identities: Vec<LensIdentityGroup>,
        index: LensCandidateIndex,
        after: Option<LensCandidateCursor>,
        limit: usize,
    },
    FocusIds {
        field: String,
        identifier: String,
        sources: Vec<String>,
        source_priority: Vec<String>,
        limit: usize,
    },
    IdentityIds {
        identifier: String,
        sources: Vec<String>,
        after: String,
        limit: usize,
    },
    IncidentIds {
        identifier: String,
        after: String,
        limit: usize,
    },
    OrderedHeaders {
        query: LensHeaderQuery,
        after: Option<(String, String)>,
        limit: usize,
    },
    EntityAliasIds {
        entities: Vec<String>,
        sources: Vec<String>,
        exclude: Vec<String>,
        limit: usize,
    },
    NodeSources {
        ids: Vec<String>,
    },
    Count {
        query: LensHeaderQuery,
    },
}
#[derive(Clone, Debug)]
pub enum LensReply {
    Rows {
        rows: Vec<JsonValue>,
        raw_bytes: Vec<usize>,
    },
    Candidates(LensCandidatePage),
    Ids(Vec<String>),
    Headers(Vec<LensHeader>),
    Sources(Vec<(String, String)>),
    Count(u64),
    Stores(LensStores),
}
struct ReadSlot {
    live: Rc<std::cell::Cell<usize>>,
    reply_bytes: usize,
    max_source_bytes: u64,
    max_row_bytes: usize,
    max_rows: u64,
    need: Option<Rc<LensNeed>>,
    reply: Option<Result<LensReply, SearchV2Error>>,
}
#[derive(Clone)]
struct LensRead(Rc<RefCell<ReadSlot>>);
struct PendingRead {
    slot: LensRead,
    need: Option<LensNeed>,
}
impl Future for PendingRead {
    type Output = Result<LensReply, SearchV2Error>;
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(need) = self.need.take() {
            let mut slot = self.slot.0.borrow_mut();
            if slot.need.is_some() || slot.reply.is_some() || slot.reply_bytes != 0 {
                return Poll::Ready(Err(corrupt("lens read slot already occupied")));
            }
            slot.need = Some(Rc::new(need));
        }
        self.slot
            .0
            .borrow_mut()
            .reply
            .take()
            .map_or(Poll::Pending, Poll::Ready)
    }
}
impl LensRead {
    fn transfer_row(&self, size: usize) -> Result<(), SearchV2Error> {
        let mut slot = self.0.borrow_mut();
        slot.reply_bytes = slot
            .reply_bytes
            .checked_sub(size)
            .ok_or_else(|| corrupt("lens row charge differs from reply"))?;
        Ok(())
    }
    fn finish_rows(&self) {
        let mut slot = self.0.borrow_mut();
        slot.live.set(
            slot.live
                .get()
                .checked_sub(slot.reply_bytes)
                .expect("lens reply charged once"),
        );
        slot.reply_bytes = 0;
    }
    fn request(&self, need: LensNeed) -> PendingRead {
        PendingRead {
            slot: self.clone(),
            need: Some(need),
        }
    }
}

/// A plan has at most one outstanding concrete read. A reply is accepted once;
/// the terminal result is returned once. The driver checks its I/O cancellation
/// independently; this plan checks between bounded synchronous domain phases.
pub struct LensPlan<'original> {
    read: LensRead,
    execution: Option<Pin<Box<dyn Future<Output = Result<JsonValue, SearchV2Error>> + 'original>>>,
    output: Option<JsonValue>,
    terminal: bool,
}
fn interrupt(probe: Option<&dyn AbortProbe>) -> Result<(), SearchV2Error> {
    match probe.and_then(AbortProbe::reason) {
        Some(AbortReason::Cancelled) => Err(SearchV2Error {
            code: SearchV2ErrorCode::Cancelled,
            message: "lens cancelled",
        }),
        Some(AbortReason::DeadlineExceeded) => Err(SearchV2Error {
            code: SearchV2ErrorCode::DeadlineExceeded,
            message: "lens deadline exceeded",
        }),
        None => Ok(()),
    }
}

fn default_sort(rules: &JsonValue) -> bool {
    let rules = array(rules);
    rules.len() == 1
        && string(get(&rules[0], "field")) == "id"
        && string(get(&rules[0], "direction")) == "asc"
}
fn compact_covered(spec: &JsonValue) -> bool {
    if string(get(spec, "detail")) != "compact"
        || !string(field(spec, "seed.text_query")).is_empty()
    {
        return false;
    }
    let mut fields: Vec<&str> = array(field(spec, "composition.group_by"))
        .iter()
        .map(string)
        .collect();
    for key in ["composition.sort_nodes", "composition.sort_relations"] {
        fields.extend(
            array(field(spec, key))
                .iter()
                .map(|rule| string(get(rule, "field"))),
        );
    }
    let mut groups = vec![get(spec, "node_query"), get(spec, "relation_query")];
    for path in array(get(spec, "path_query")) {
        for step in array(get(path, "steps")) {
            groups.push(get(step, "node_query"));
            groups.push(get(step, "relation_query"));
        }
    }
    for group in groups {
        for rule in array(get(group, "filters")) {
            let Some(field) = get(rule, "field")
                .as_str()
                .filter(|field| !field.is_empty())
            else {
                return false;
            };
            fields.push(field);
        }
    }
    fields.iter().all(|field| {
        [
            "attributes",
            "source_record",
            "readable_context",
            "semantics.claim.source_canonical_json",
        ]
        .iter()
        .all(|omitted| {
            *field != *omitted
                && !field.starts_with(&format!("{omitted}."))
                && !omitted.starts_with(&format!("{field}."))
        })
    })
}
fn membership(group: &JsonValue) -> Option<LensMembership> {
    if !boolean(get(group, "enabled")) || array(get(group, "filters")).is_empty() {
        return None;
    }
    let mut terms = vec![];
    for rule in array(get(group, "filters")) {
        let field = string(get(rule, "field"));
        let op = string(get(rule, "op"));
        if rule.object_get("_property_binding").is_some()
            || !["view_ids", "graph_layers"].contains(&field)
            || !["eq", "in", "contains"].contains(&op)
        {
            return None;
        }
        let value = get(rule, "value");
        if op == "eq" && value.as_str().is_none() {
            return None;
        }
        let values = value.as_array().unwrap_or(std::slice::from_ref(value));
        if values.is_empty() || values.iter().any(|value| value.as_str().is_none()) {
            return None;
        }
        let values = values
            .iter()
            .map(|value| string(value).to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        terms.push(LensMembershipTerm {
            field: field.to_owned(),
            all: op == "contains",
            values,
        });
    }
    let all = string(get(group, "match")) == "all";
    let mut drivers = BTreeSet::new();
    for term in if all { &terms[..1] } else { &terms[..] } {
        for value in if term.all {
            &term.values[..1]
        } else {
            &term.values[..]
        } {
            drivers.insert((term.field.clone(), value.clone()));
        }
    }
    let predicates = terms.iter().map(|term| term.values.len()).sum::<usize>();
    if predicates > 32 || drivers.len() > 16 || drivers.len() * (3 * predicates + 10) + 6 > 100 {
        return None;
    }
    Some(LensMembership {
        all,
        terms,
        drivers: drivers.into_iter().collect(),
    })
}
#[derive(Clone)]
struct Cell {
    keys: [String; 3],
    count: u64,
}
struct PublishedMetadata {
    nodes: Vec<Cell>,
    relations: Vec<Cell>,
}
impl PublishedMetadata {
    fn parse(value: &JsonValue) -> Result<Self, SearchV2Error> {
        fn cells(value: &JsonValue) -> Result<Vec<Cell>, SearchV2Error> {
            let values = value
                .as_array()
                .filter(|values| values.len() <= 16384)
                .ok_or_else(|| corrupt("invalid published lens histogram"))?;
            let mut cells = vec![];
            let mut previous: Option<[String; 3]> = None;
            for value in values {
                let values = value
                    .as_array()
                    .filter(|values| values.len() == 4)
                    .ok_or_else(|| corrupt("invalid published lens histogram cell"))?;
                let mut keys = vec![];
                for value in &values[..3] {
                    keys.push(
                        value
                            .as_str()
                            .filter(|key| !key.is_empty())
                            .ok_or_else(|| corrupt("invalid published lens histogram key"))?
                            .to_owned(),
                    );
                }
                let keys: [String; 3] = keys
                    .try_into()
                    .map_err(|_| corrupt("invalid published lens histogram key"))?;
                let count = values[3]
                    .as_u64()
                    .filter(|count| *count > 0 && *count <= 9_007_199_254_740_991)
                    .ok_or_else(|| corrupt("invalid published lens histogram count"))?;
                if previous.as_ref().is_some_and(|previous| previous >= &keys) {
                    return Err(corrupt("published lens histogram order differs"));
                }
                previous = Some(keys.clone());
                cells.push(Cell { keys, count });
            }
            Ok(cells)
        }
        Ok(Self {
            nodes: cells(get(value, "node_counts"))?,
            relations: cells(get(value, "relation_counts"))?,
        })
    }
}
struct Execution<'original> {
    original: Option<&'original dyn OriginalLensBudget>,
    read: LensRead,
    vocabulary: LensVocabulary,
    public: JsonValue,
    spec: JsonValue,
    sources: Vec<String>,
    budget: LensBudget,
    published: Option<PublishedMetadata>,
    stores: LensStores,
    available: (u64, u64),
    candidates: usize,
    adjacency: usize,
    path_steps: usize,
    callbacks: usize,
    sort_bytes: usize,
    callback_limit: Option<usize>,
    sort_limit: Option<usize>,
    cache: BTreeMap<(u8, String), (Rc<RetainedRow>, u64)>,
    cache_tick: u64,
    cache_bytes: usize,
    cache_limit: Option<(usize, usize)>,
    live_bytes: Rc<std::cell::Cell<usize>>,
    probe: Option<std::sync::Arc<dyn AbortProbe>>,
}
struct RetainedRow {
    value: JsonValue,
    raw_bytes: usize,
    live: Rc<std::cell::Cell<usize>>,
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
                .checked_sub(self.raw_bytes)
                .expect("retained lens row charged once"),
        );
    }
}
impl Execution<'_> {
    fn scoped(&self, row: &JsonValue) -> bool {
        self.sources
            .iter()
            .any(|source| source == string(get(row, "source_graph")))
    }
    fn candidate(&mut self) -> Result<(), SearchV2Error> {
        self.check()?;
        self.candidates = self.candidates.checked_add(1).ok_or_else(budget)?;
        if self.candidates > self.budget.max_candidates {
            return Err(budget());
        }
        Ok(())
    }
    fn callback(&mut self) -> Result<(), SearchV2Error> {
        self.check()?;
        self.callbacks = self.callbacks.checked_add(1).ok_or_else(budget)?;
        if self
            .callback_limit
            .is_some_and(|limit| self.callbacks > limit)
        {
            return Err(budget());
        }
        Ok(())
    }
    fn group(&mut self, row: &JsonValue, group: &JsonValue) -> Result<bool, SearchV2Error> {
        self.callback()?;
        try_matches_group(row, group, &self.vocabulary)
    }
    async fn rows(
        &mut self,
        kind: SearchKind,
        ids: Vec<String>,
    ) -> Result<Vec<Rc<RetainedRow>>, SearchV2Error> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let tag = if kind == SearchKind::Nodes { 0 } else { 1 };
        let mut result = BTreeMap::new();
        let mut missing = vec![];
        for id in &ids {
            self.cache_tick = self.cache_tick.checked_add(1).ok_or_else(budget)?;
            if let Some((row, tick)) = self.cache.get_mut(&(tag, id.clone())) {
                *tick = self.cache_tick;
                result.insert(id.clone(), row.clone());
            } else {
                missing.push(id.clone());
            }
        }
        if missing.is_empty() {
            return Ok(ids
                .into_iter()
                .filter_map(|id| result.remove(&id))
                .collect());
        }
        let representation = if self.stores.compact {
            LensRepresentation::CoveredCompact
        } else {
            LensRepresentation::Full
        };
        let LensReply::Rows { rows, raw_bytes } = self
            .read
            .request(LensNeed::ExactRows {
                kind,
                ids: missing.clone(),
                representation,
            })
            .await?
        else {
            return Err(corrupt("invalid lens rows reply"));
        };
        if rows.len() != raw_bytes.len() {
            return Err(corrupt("lens lexical size framing differs"));
        }
        let mut seen = BTreeSet::new();
        for row in &rows {
            let id = get(row, "id")
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or_else(|| corrupt("invalid lens row identifier"))?;
            if !missing.iter().any(|expected| expected == id) || !seen.insert(id) {
                return Err(corrupt("lens row closure differs"));
            }
        }
        if self.published.is_some() && rows.len() != missing.len() {
            return Err(corrupt("published lens exact row missing"));
        }
        for (value, size) in rows.into_iter().zip(raw_bytes) {
            let row = self.retain(value, size)?;
            let id = string(get(&row, "id")).to_owned();
            if let Some((byte_limit, entry_limit)) = self.cache_limit {
                if size <= byte_limit {
                    while !self.cache.is_empty()
                        && (self.cache.len() >= entry_limit
                            || self
                                .cache_bytes
                                .checked_add(size)
                                .is_none_or(|bytes| bytes > byte_limit))
                    {
                        let key = self
                            .cache
                            .iter()
                            .min_by_key(|(_, (_, tick))| *tick)
                            .map(|(key, _)| key.clone())
                            .unwrap();
                        let (retired, _) = self.cache.remove(&key).unwrap();
                        self.cache_bytes -= retired.raw_bytes;
                    }
                    self.cache_tick = self.cache_tick.checked_add(1).ok_or_else(budget)?;
                    self.cache_bytes = self.cache_bytes.checked_add(size).ok_or_else(budget)?;
                    self.cache
                        .insert((tag, id.clone()), (row.clone(), self.cache_tick));
                }
            }
            result.insert(id, row);
        }
        self.read.finish_rows();
        Ok(ids
            .into_iter()
            .filter_map(|id| result.remove(&id))
            .collect())
    }
    fn retain(&self, value: JsonValue, size: usize) -> Result<Rc<RetainedRow>, SearchV2Error> {
        if size == 0 || size > self.budget.inspect.max_payload_bytes {
            return Err(budget());
        }
        // The reply slot already admitted the whole incoming batch against
        // retained rows. Transfer its charge, without counting the row twice.
        self.read.transfer_row(size)?;
        Ok(Rc::new(RetainedRow {
            value,
            raw_bytes: size,
            live: self.live_bytes.clone(),
        }))
    }
    async fn item(
        &mut self,
        kind: SearchKind,
        id: &str,
    ) -> Result<Option<Rc<RetainedRow>>, SearchV2Error> {
        let mut rows = self.rows(kind, vec![id.to_owned()]).await?;
        Ok(rows.pop().filter(|row| self.scoped(row)))
    }
    async fn scoped_neighbor(
        &mut self,
        id: &str,
    ) -> Result<Option<Rc<RetainedRow>>, SearchV2Error> {
        if self.published.is_some() {
            let LensReply::Sources(sources) = self
                .read
                .request(LensNeed::NodeSources {
                    ids: vec![id.to_owned()],
                })
                .await?
            else {
                return Err(corrupt("invalid lens node sources reply"));
            };
            if sources.len() > 1 || sources.iter().any(|(node, _)| node != id) {
                return Err(corrupt("lens node sources closure differs"));
            }
            if sources
                .first()
                .is_none_or(|(_, source)| !self.sources.contains(source))
            {
                return Ok(None);
            }
        }
        self.item(SearchKind::Nodes, id).await
    }
    async fn aliases(
        &mut self,
        field: &str,
        identifier: &str,
    ) -> Result<Vec<Rc<RetainedRow>>, SearchV2Error> {
        if self.published.is_some() {
            let mut after = Some(LensCandidateCursor::Id(String::new()));
            let mut out = vec![];
            loop {
                let LensReply::Candidates(page) = self
                    .read
                    .request(LensNeed::CandidateIds {
                        kind: SearchKind::Nodes,
                        sources: self.sources.clone(),
                        identities: vec![LensIdentityGroup {
                            all: true,
                            terms: vec![LensIdentityTerm {
                                field: field.to_owned(),
                                values: vec![identifier.to_owned()],
                            }],
                        }],
                        index: LensCandidateIndex::Identity(field.to_owned()),
                        after: after.clone(),
                        limit: self.budget.block_size,
                    })
                    .await?
                else {
                    return Err(corrupt("invalid lens candidate reply"));
                };
                if page.rows.is_empty() {
                    break;
                }
                self.check_candidates(&page, &after)?;
                for candidate in &page.rows {
                    self.candidate()?;
                    let row = self
                        .item(SearchKind::Nodes, &candidate.id)
                        .await?
                        .ok_or_else(|| corrupt("lens identity closure missing"))?;
                    if string(get(&row, field)) != identifier {
                        return Err(corrupt("lens identity mirror differs"));
                    }
                    out.push(row);
                }
                after = Some(LensCandidateCursor::Id(
                    page.rows.last().unwrap().id.clone(),
                ));
            }
            return Ok(out);
        }
        if field == "entity_id" {
            let mut after = String::new();
            let mut out = vec![];
            loop {
                let LensReply::Ids(ids) = self
                    .read
                    .request(LensNeed::IdentityIds {
                        identifier: identifier.to_owned(),
                        sources: self.sources.clone(),
                        after: after.clone(),
                        limit: self.budget.block_size,
                    })
                    .await?
                else {
                    return Err(corrupt("invalid lens identity reply"));
                };
                self.check_ids(&ids, &after, self.budget.block_size)?;
                if ids.is_empty() {
                    break;
                }
                for id in &ids {
                    self.candidate()?;
                    out.push(
                        self.item(SearchKind::Nodes, id)
                            .await?
                            .ok_or_else(|| corrupt("lens identity closure missing"))?,
                    );
                }
                after = ids.last().unwrap().clone();
            }
            Ok(out)
        } else {
            let LensReply::Rows { rows, raw_bytes } = self
                .read
                .request(LensNeed::LookupRows {
                    field: field.to_owned(),
                    identifier: identifier.to_owned(),
                    limit: self.budget.max_candidates.saturating_add(1),
                })
                .await?
            else {
                return Err(corrupt("invalid lens lookup reply"));
            };
            if rows.len() > self.budget.max_candidates {
                return Err(budget());
            }
            if rows.len() != raw_bytes.len() {
                return Err(corrupt("lens lexical size framing differs"));
            }
            for _ in &rows {
                self.candidate()?;
            }
            let mut retained = vec![];
            for (row, size) in rows.into_iter().zip(raw_bytes) {
                if self.scoped(&row) {
                    retained.push(self.retain(row, size)?);
                }
            }
            self.read.finish_rows();
            Ok(retained)
        }
    }
    fn check_ids(&self, ids: &[String], after: &str, limit: usize) -> Result<(), SearchV2Error> {
        if ids.len() > limit
            || ids.iter().any(|id| {
                id.is_empty()
                    || id.as_str() <= after
                    || id.len() > self.budget.inspect.max_field_bytes
            })
            || ids.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(corrupt("lens ID page order differs"));
        }
        Ok(())
    }
    fn check_candidates(
        &self,
        page: &LensCandidatePage,
        after: &Option<LensCandidateCursor>,
    ) -> Result<(), SearchV2Error> {
        if page.rows.len() > self.budget.block_size {
            return Err(corrupt("lens candidate page exceeds declared limit"));
        }
        let mut previous = after.clone();
        for candidate in &page.rows {
            if candidate.id.is_empty() || candidate.id.len() > self.budget.inspect.max_field_bytes {
                return Err(corrupt("invalid lens candidate identifier"));
            }
            if self.published.is_some() {
                if candidate.position.is_some()
                    || candidate.source.is_some()
                    || previous.as_ref().is_some_and(|previous| !matches!(previous, LensCandidateCursor::Id(id) if id < &candidate.id))
                {
                    return Err(corrupt("published lens candidate order differs"));
                }
                previous = Some(LensCandidateCursor::Id(candidate.id.clone()));
            } else {
                let source = candidate
                    .source
                    .as_ref()
                    .filter(|source| self.sources.contains(source))
                    .ok_or_else(|| corrupt("lens candidate source differs"))?;
                let position = candidate
                    .position
                    .filter(|position| *position >= 0)
                    .ok_or_else(|| corrupt("lens candidate source order differs"))?;
                if let Some(LensCandidateCursor::SourceOrder {
                    source: old_source,
                    position: old_position,
                }) = &previous
                {
                    if (old_source, *old_position) >= (source, position) {
                        return Err(corrupt("lens candidate source order differs"));
                    }
                }
                previous = Some(LensCandidateCursor::SourceOrder {
                    source: source.clone(),
                    position,
                });
            }
        }
        Ok(())
    }
}

struct Scan {
    kind: SearchKind,
    after: Option<LensCandidateCursor>,
    identities: Vec<LensIdentityGroup>,
    index: LensCandidateIndex,
    native_aliases: Option<std::collections::VecDeque<(String, String)>>,
    seen: BTreeSet<String>,
    done: bool,
}
struct KeyedHeader {
    header: LensHeader,
    key: Vec<String>,
}
struct OrderedStream {
    query: Rc<LensHeaderQuery>,
    endpoint: Option<LensEndpoint>,
    after: Option<(String, String)>,
    rows: std::collections::VecDeque<LensHeader>,
    done: bool,
}
impl Execution<'_> {
    fn check(&self) -> Result<(), SearchV2Error> {
        if let Some(original) = self.original {
            original.check()?;
            original.charge_work(1)?;
        }
        interrupt(self.probe.as_deref())
    }
    fn scan(&self, kind: SearchKind) -> Scan {
        let group = get(
            &self.spec,
            if kind == SearchKind::Nodes {
                "node_query"
            } else {
                "relation_query"
            },
        );
        let positive = array(get(group, "filters"))
            .iter()
            .map(|rule| crate::knowledge_lens::identity_selector(rule, kind == SearchKind::Nodes))
            .collect::<Vec<_>>();
        let all = string(get(group, "match")) == "all";
        let group_plan = if all {
            let terms = positive
                .iter()
                .flatten()
                .map(|(field, values)| LensIdentityTerm {
                    field: field.clone(),
                    values: values.clone(),
                })
                .collect::<Vec<_>>();
            (!terms.is_empty()).then_some(LensIdentityGroup { all: true, terms })
        } else if !positive.is_empty() && positive.iter().all(Option::is_some) {
            Some(LensIdentityGroup {
                all: false,
                terms: positive
                    .iter()
                    .flatten()
                    .map(|(field, values)| LensIdentityTerm {
                        field: field.clone(),
                        values: values.clone(),
                    })
                    .collect(),
            })
        } else {
            None
        };
        let seeds = if kind == SearchKind::Nodes {
            array(field(&self.spec, "seed.node_ids"))
        } else {
            &[]
        };
        let seed_plan = (!seeds.is_empty()).then(|| LensIdentityGroup {
            all: false,
            terms: ["id", "native_id", "entity_id"]
                .iter()
                .map(|name| LensIdentityTerm {
                    field: (*name).into(),
                    values: seeds.iter().map(|v| string(v).to_owned()).collect(),
                })
                .collect(),
        });
        let index = match (&seed_plan, &group_plan) {
            (Some(_), None) | (None, Some(LensIdentityGroup { all: false, .. })) => {
                LensCandidateIndex::Union
            }
            (None, Some(group)) => LensCandidateIndex::Identity(group.terms[0].field.clone()),
            _ => LensCandidateIndex::Source,
        };
        let native_aliases = if self.published.is_none() && kind == SearchKind::Nodes {
            let native = if !seeds.is_empty() {
                Some(
                    ["id", "entity_id", "native_id"]
                        .iter()
                        .map(|name| {
                            (
                                (*name).to_owned(),
                                seeds
                                    .iter()
                                    .map(|v| string(v).to_owned())
                                    .collect::<Vec<_>>(),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            } else if all {
                positive.iter().flatten().next().cloned().map(|v| vec![v])
            } else if !positive.is_empty() && positive.iter().all(Option::is_some) {
                Some(positive.iter().flatten().cloned().collect())
            } else {
                None
            };
            native.map(|terms| {
                terms
                    .into_iter()
                    .flat_map(|(name, values)| {
                        values.into_iter().map(move |value| (name.clone(), value))
                    })
                    .collect()
            })
        } else {
            None
        };
        Scan {
            kind,
            after: None,
            identities: seed_plan.into_iter().chain(group_plan).collect(),
            index,
            native_aliases,
            seen: BTreeSet::new(),
            done: false,
        }
    }
    async fn scan_page(&mut self, scan: &mut Scan) -> Result<Vec<Rc<RetainedRow>>, SearchV2Error> {
        self.check()?;
        if scan.done {
            return Ok(vec![]);
        }
        if let Some(aliases) = &mut scan.native_aliases {
            while let Some((name, value)) = aliases.pop_front() {
                let rows = self.aliases(&name, &value).await?;
                let rows = rows
                    .into_iter()
                    .filter(|row| scan.seen.insert(string(get(row, "id")).to_owned()))
                    .collect::<Vec<_>>();
                if !rows.is_empty() {
                    return Ok(rows);
                }
            }
            scan.done = true;
            return Ok(vec![]);
        }
        let LensReply::Candidates(page) = self
            .read
            .request(LensNeed::CandidateIds {
                kind: scan.kind,
                sources: self.sources.clone(),
                identities: if self.published.is_some() {
                    scan.identities.clone()
                } else {
                    vec![]
                },
                index: scan.index.clone(),
                after: scan.after.clone(),
                limit: self.budget.block_size,
            })
            .await?
        else {
            return Err(corrupt("invalid lens candidate reply"));
        };
        self.check_candidates(&page, &scan.after)?;
        if page.rows.is_empty() {
            scan.done = true;
            return Ok(vec![]);
        }
        scan.after = page.rows.last().map(|row| {
            if self.published.is_some() {
                LensCandidateCursor::Id(row.id.clone())
            } else {
                LensCandidateCursor::SourceOrder {
                    source: row.source.clone().expect("validated native source"),
                    position: row.position.expect("validated native position"),
                }
            }
        });
        for _ in &page.rows {
            self.candidate()?;
        }
        let rows = self
            .rows(
                scan.kind,
                page.rows.iter().map(|row| row.id.clone()).collect(),
            )
            .await?;
        if rows.len() != page.rows.len() {
            return Err(corrupt("lens candidate closure missing"));
        }
        for (row, key) in rows.iter().zip(&page.rows) {
            if !self.scoped(row)
                || key
                    .source
                    .as_ref()
                    .is_some_and(|source| source != string(get(row, "source_graph")))
            {
                return Err(corrupt("lens candidate source mirror differs"));
            }
        }
        Ok(rows)
    }
    async fn focus(&mut self, requested: &str) -> Result<Rc<RetainedRow>, SearchV2Error> {
        if self.published.is_some() {
            let mut priority = self.vocabulary.sources.clone();
            priority.sort_by_key(|source| {
                self.vocabulary
                    .carrier_source_priority
                    .get(source)
                    .copied()
                    .unwrap_or(99)
            });
            for name in ["id", "entity_id", "native_id"] {
                let limit = if name == "native_id" { 2 } else { 1 };
                let LensReply::Ids(ids) = self
                    .read
                    .request(LensNeed::FocusIds {
                        field: name.into(),
                        identifier: requested.into(),
                        sources: self.sources.clone(),
                        source_priority: priority.clone(),
                        limit,
                    })
                    .await?
                else {
                    return Err(corrupt("invalid lens focus reply"));
                };
                if ids.len() > limit || ids.iter().any(String::is_empty) {
                    return Err(corrupt("lens focus lookup exceeds declared limit"));
                }
                if ids.len() > 1 {
                    return Err(invalid("ambiguous native lens focus"));
                }
                if let Some(id) = ids.first() {
                    let row = self
                        .item(SearchKind::Nodes, id)
                        .await?
                        .ok_or_else(|| corrupt("lens focus closure missing"))?;
                    if string(get(&row, name)) != requested {
                        return Err(corrupt("lens focus identity mirror differs"));
                    }
                    return Ok(row);
                }
            }
        } else {
            if let Some(exact) = self.item(SearchKind::Nodes, requested).await? {
                return Ok(exact);
            }
            let mut entities = self.aliases("entity_id", requested).await?;
            entities.sort_by(|a, b| {
                (self.vocabulary.priority(a), string(get(a, "id")))
                    .cmp(&(self.vocabulary.priority(b), string(get(b, "id"))))
            });
            if !entities.is_empty() {
                return Ok(entities.remove(0));
            }
            let mut native = self.aliases("native_id", requested).await?;
            if native.len() > 1 {
                return Err(invalid("ambiguous native lens focus"));
            }
            if let Some(row) = native.pop() {
                return Ok(row);
            }
        }
        Err(SearchV2Error {
            code: SearchV2ErrorCode::UnknownIdentifier,
            message: "unknown knowledge lens focus",
        })
    }
    fn walk<'a>(
        &'a mut self,
        current: String,
        index: usize,
        steps: &'a [JsonValue],
        nodes: Vec<JsonValue>,
        relations: Vec<JsonValue>,
        path_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<JsonValue>, SearchV2Error>> + 'a>> {
        Box::pin(async move {
            self.check()?;
            if index == steps.len() {
                return Ok(Some(object(vec![
                    ("path_id", text(path_id)),
                    ("node_ids", JsonValue::Array(nodes)),
                    ("relation_ids", JsonValue::Array(relations)),
                ])));
            }
            let step = &steps[index];
            // Native preserves its prior complete bounded incidence read before
            // evaluating a witness. Published D1 uses addressed pages lazily.
            let mut native = if self.published.is_none() {
                Some(self.native_incident(&current).await?)
            } else {
                None
            };
            let mut after = String::new();
            loop {
                let rows = if let Some(native) = &mut native {
                    if native.is_empty() {
                        return Ok(None);
                    }
                    std::mem::take(native)
                } else {
                    let LensReply::Ids(ids) = self
                        .read
                        .request(LensNeed::IncidentIds {
                            identifier: current.clone(),
                            after: after.clone(),
                            limit: self.budget.block_size,
                        })
                        .await?
                    else {
                        return Err(corrupt("invalid lens incident reply"));
                    };
                    self.check_ids(&ids, &after, self.budget.block_size)?;
                    if ids.is_empty() {
                        return Ok(None);
                    }
                    after = ids.last().expect("nonempty incident page").clone();
                    self.adjacency = self.adjacency.checked_add(ids.len()).ok_or_else(budget)?;
                    if self.adjacency > self.budget.max_adjacency_rows {
                        return Err(budget());
                    }
                    self.rows(SearchKind::Relations, ids).await?
                };
                for relation in rows {
                    if !self.scoped(&relation) {
                        continue;
                    }
                    self.path_steps = self.path_steps.checked_add(1).ok_or_else(budget)?;
                    if self.path_steps > self.budget.max_path_steps {
                        return Err(budget());
                    }
                    let rg = get(step, "relation_query");
                    if !boolean(get(rg, "enabled")) || !self.group(&relation, rg)? {
                        continue;
                    }
                    for neighbor in crate::knowledge_lens::neighbors(
                        &relation,
                        &current,
                        string(get(step, "direction")),
                    ) {
                        let ng = get(step, "node_query");
                        if !boolean(get(ng, "enabled")) {
                            continue;
                        }
                        let Some(node) = self.scoped_neighbor(&neighbor).await? else {
                            continue;
                        };
                        if !self.group(&node, ng)? {
                            continue;
                        }
                        let mut next_nodes = nodes.clone();
                        next_nodes.push(text(&neighbor));
                        let mut next_relations = relations.clone();
                        next_relations.push(get(&relation, "id").clone());
                        if let Some(found) = self
                            .walk(
                                neighbor,
                                index + 1,
                                steps,
                                next_nodes,
                                next_relations,
                                path_id,
                            )
                            .await?
                        {
                            return Ok(Some(found));
                        }
                    }
                }
            }
        })
    }
    async fn native_incident(
        &mut self,
        identifier: &str,
    ) -> Result<Vec<Rc<RetainedRow>>, SearchV2Error> {
        let mut rows = vec![];
        let mut after = String::new();
        loop {
            let LensReply::Ids(ids) = self
                .read
                .request(LensNeed::IncidentIds {
                    identifier: identifier.into(),
                    after: after.clone(),
                    limit: self.budget.block_size,
                })
                .await?
            else {
                return Err(corrupt("invalid native lens incident reply"));
            };
            self.check_ids(&ids, &after, self.budget.block_size)?;
            if ids.is_empty() {
                break;
            }
            after = ids.last().expect("nonempty incident page").clone();
            self.adjacency = self.adjacency.checked_add(ids.len()).ok_or_else(budget)?;
            if self.adjacency > self.budget.max_adjacency_rows {
                return Err(budget());
            }
            let page = self.rows(SearchKind::Relations, ids).await?;
            rows.extend(page.into_iter().filter(|row| self.scoped(row)));
        }
        Ok(rows)
    }
    fn dimensional(&self, kind: SearchKind, group: &JsonValue) -> bool {
        let fields: &[&str] = if kind == SearchKind::Nodes {
            &["source_graph", "kind_id", "type_id"]
        } else {
            &["source_graph", "predicate_id", "relation_type_id"]
        };
        array(get(group, "filters")).iter().all(|rule| {
            rule.object_get("_property_binding").is_none()
                && fields.contains(&string(get(rule, "field")))
        })
    }
    fn scope_cells(&self, kind: SearchKind) -> Vec<Cell> {
        self.published
            .as_ref()
            .map(|metadata| {
                if kind == SearchKind::Nodes {
                    &metadata.nodes
                } else {
                    &metadata.relations
                }
            })
            .into_iter()
            .flatten()
            .filter(|cell| self.sources.contains(&cell.keys[0]))
            .cloned()
            .collect()
    }
    fn header_query(&mut self, kind: SearchKind) -> Result<LensHeaderQuery, SearchV2Error> {
        let group = get(
            &self.spec,
            if kind == SearchKind::Nodes {
                "node_query"
            } else {
                "relation_query"
            },
        )
        .clone();
        let dimensions = if !boolean(get(&group, "enabled")) {
            Some(vec![])
        } else if self.dimensional(kind, &group) && !array(get(&group, "filters")).is_empty() {
            let fields = if kind == SearchKind::Nodes {
                ["source_graph", "kind_id", "type_id"]
            } else {
                ["source_graph", "predicate_id", "relation_type_id"]
            };
            let mut allowed = vec![];
            for cell in self.scope_cells(kind) {
                let item = object(
                    fields
                        .iter()
                        .zip(&cell.keys)
                        .map(|(key, value)| (*key, text(value)))
                        .collect(),
                );
                if self.group(&item, &group)?
                    && (kind == SearchKind::Nodes
                        || crate::knowledge_lens::relation_regime(
                            &item,
                            &self.spec,
                            &self.vocabulary,
                        ))
                {
                    allowed.push(cell.keys);
                }
            }
            Some(allowed)
        } else {
            None
        };
        let overview = string(field(&self.spec, "traversal.profile")) == "overview"
            && kind == SearchKind::Relations;
        Ok(LensHeaderQuery {
            kind,
            sources: self.sources.clone(),
            dimensions,
            membership: if self.stores.membership {
                membership(&group)
            } else {
                None
            },
            predicate_ids: if kind == SearchKind::Relations {
                array(field(&self.spec, "traversal.predicate_ids"))
                    .iter()
                    .map(|v| string(v).to_owned())
                    .collect()
            } else {
                vec![]
            },
            excluded_predicates: if overview {
                self.vocabulary.overview_excluded_predicates.clone()
            } else {
                vec![]
            },
            excluded_relation_types: if overview {
                self.vocabulary.overview_excluded_relation_types.clone()
            } else {
                vec![]
            },
            endpoint: None,
            eligible: None,
        })
    }
    async fn count(&mut self, query: &LensHeaderQuery) -> Result<u64, SearchV2Error> {
        let LensReply::Count(count) = self
            .read
            .request(LensNeed::Count {
                query: query.clone(),
            })
            .await?
        else {
            return Err(corrupt("invalid lens count reply"));
        };
        if count > 9_007_199_254_740_991 {
            return Err(corrupt("lens count exceeds published integer law"));
        }
        Ok(count)
    }
    fn histogram_count(&mut self, kind: SearchKind) -> Result<u64, SearchV2Error> {
        let group = get(
            &self.spec,
            if kind == SearchKind::Nodes {
                "node_query"
            } else {
                "relation_query"
            },
        )
        .clone();
        let fields = if kind == SearchKind::Nodes {
            ["source_graph", "kind_id", "type_id"]
        } else {
            ["source_graph", "predicate_id", "relation_type_id"]
        };
        let mut count = 0u64;
        for cell in self.scope_cells(kind) {
            let item = object(
                fields
                    .iter()
                    .zip(&cell.keys)
                    .map(|(key, value)| (*key, text(value)))
                    .collect(),
            );
            if self.group(&item, &group)?
                && (kind == SearchKind::Nodes
                    || crate::knowledge_lens::relation_regime(&item, &self.spec, &self.vocabulary))
            {
                count = count.checked_add(cell.count).ok_or_else(budget)?;
            }
        }
        Ok(count)
    }
    async fn next_header(
        &mut self,
        stream: &mut OrderedStream,
    ) -> Result<Option<LensHeader>, SearchV2Error> {
        self.check()?;
        if stream.rows.is_empty() && !stream.done {
            let limit = if stream.after.is_none() && stream.endpoint.is_some() {
                1
            } else {
                self.budget.block_size
            };
            let mut query = (*stream.query).clone();
            query.endpoint = stream.endpoint.clone();
            let LensReply::Headers(rows) = self
                .read
                .request(LensNeed::OrderedHeaders {
                    query,
                    after: stream.after.clone(),
                    limit,
                })
                .await?
            else {
                return Err(corrupt("invalid lens header reply"));
            };
            if rows.len() > limit {
                return Err(corrupt("lens header page exceeds limit"));
            }
            let mut previous = stream.after.clone();
            for row in &rows {
                if row.id.is_empty()
                    || [&row.id, &row.sort_key, &row.from_id, &row.to_id]
                        .iter()
                        .any(|field| field.len() > self.budget.inspect.max_field_bytes)
                    || row.sort_key != lower(&row.id)
                    || previous
                        .as_ref()
                        .is_some_and(|(sort, id)| (sort, id) >= (&row.sort_key, &row.id))
                {
                    return Err(corrupt("lens ordered header differs"));
                }
                if let Some(endpoint) = &stream.endpoint {
                    if match endpoint {
                        LensEndpoint::From(id) => &row.from_id != id,
                        LensEndpoint::To(id) => &row.to_id != id,
                    } {
                        return Err(corrupt("lens endpoint header differs"));
                    }
                }
                previous = Some((row.sort_key.clone(), row.id.clone()));
            }
            stream.after = previous;
            stream.done = rows.is_empty();
            stream.rows = rows.into();
        }
        Ok(stream.rows.pop_front())
    }
    fn sort_key(
        &mut self,
        row: &JsonValue,
        rules: &JsonValue,
    ) -> Result<Vec<String>, SearchV2Error> {
        let mut keys = vec![];
        for rule in array(rules) {
            self.callback()?;
            let value = field(row, string(get(rule, "field")));
            let key = lower(&if crate::knowledge_lens::is_truthy(value) {
                py_string(value)
            } else {
                String::new()
            });
            self.sort_bytes = self.sort_bytes.checked_add(key.len()).ok_or_else(budget)?;
            if self.sort_limit.is_some_and(|limit| self.sort_bytes > limit) {
                return Err(budget());
            }
            keys.push(key);
        }
        keys.push(string(get(row, "id")).to_owned());
        Ok(keys)
    }
}
fn compare_keys(left: &[String], right: &[String], rules: &JsonValue) -> std::cmp::Ordering {
    for (index, (left, right)) in left.iter().zip(right).enumerate() {
        let mut order = left.cmp(right);
        if string(get(
            array(rules).get(index).unwrap_or(&JsonValue::Null),
            "direction",
        )) == "desc"
        {
            order = order.reverse();
        }
        if order != std::cmp::Ordering::Equal {
            return order;
        }
    }
    std::cmp::Ordering::Equal
}

struct RelationCursor {
    indices: std::collections::VecDeque<usize>,
    streams: Vec<OrderedStream>,
    heads: Vec<Option<LensHeader>>,
    consumed: Option<usize>,
    previous: Option<String>,
}
impl RelationCursor {
    fn generic(
        headers: &[KeyedHeader],
        frontier: Option<(&BTreeSet<String>, &str)>,
        eligibility: Option<(&BTreeSet<String>, &BTreeSet<String>, &str)>,
    ) -> Self {
        let indices = headers
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                let row = &row.header;
                frontier.is_none_or(|(ids, direction)| {
                    (direction != "incoming" && ids.contains(&row.from_id))
                        || (direction != "outgoing" && ids.contains(&row.to_id))
                }) && eligibility.is_none_or(|(basis, traversed, policy)| {
                    policy == "independent"
                        || traversed.contains(&row.id)
                        || if policy == "both" {
                            basis.contains(&row.from_id) && basis.contains(&row.to_id)
                        } else {
                            basis.contains(&row.from_id) || basis.contains(&row.to_id)
                        }
                })
            })
            .map(|(index, _)| index)
            .collect();
        Self {
            indices,
            streams: vec![],
            heads: vec![],
            consumed: None,
            previous: None,
        }
    }
    fn ordered(query: LensHeaderQuery, frontier: Option<(&BTreeSet<String>, &str)>) -> Self {
        let endpoints = if let Some((ids, direction)) = frontier {
            ids.iter()
                .flat_map(|id| {
                    let mut endpoints = vec![];
                    if direction != "incoming" {
                        endpoints.push(Some(LensEndpoint::From(id.clone())));
                    }
                    if direction != "outgoing" {
                        endpoints.push(Some(LensEndpoint::To(id.clone())));
                    }
                    endpoints
                })
                .collect::<Vec<_>>()
        } else {
            vec![None]
        };
        let query = Rc::new(query);
        let streams = endpoints
            .into_iter()
            .map(|endpoint| OrderedStream {
                query: query.clone(),
                endpoint,
                after: None,
                rows: std::collections::VecDeque::new(),
                done: false,
            })
            .collect::<Vec<_>>();
        Self {
            indices: std::collections::VecDeque::new(),
            heads: vec![None; streams.len()],
            streams,
            consumed: None,
            previous: None,
        }
    }
    async fn next(
        &mut self,
        execution: &mut Execution<'_>,
        generic: Option<&[KeyedHeader]>,
    ) -> Result<Option<LensHeader>, SearchV2Error> {
        if let Some(headers) = generic {
            return Ok(self
                .indices
                .pop_front()
                .map(|index| headers[index].header.clone()));
        }
        for index in 0..self.streams.len() {
            if self.consumed == Some(index)
                || (self.heads[index].is_none() && !self.streams[index].done)
            {
                self.heads[index] = execution.next_header(&mut self.streams[index]).await?;
            }
        }
        self.consumed = None;
        loop {
            let best = self
                .heads
                .iter()
                .enumerate()
                .filter_map(|(index, row)| row.as_ref().map(|row| (index, row)))
                .min_by(|(_, a), (_, b)| (&a.sort_key, &a.id).cmp(&(&b.sort_key, &b.id)))
                .map(|(index, _)| index);
            let Some(best) = best else {
                return Ok(None);
            };
            let row = self.heads[best].take().expect("selected occupied head");
            if self.previous.as_ref() == Some(&row.id) {
                self.heads[best] = execution.next_header(&mut self.streams[best]).await?;
                continue;
            }
            self.previous = Some(row.id.clone());
            self.consumed = Some(best);
            return Ok(Some(row));
        }
    }
}
impl Execution<'_> {
    async fn execute(
        mut self,
        revision: String,
        authority: JsonValue,
        publication: Option<JsonValue>,
    ) -> Result<JsonValue, SearchV2Error> {
        self.check()?;
        if self.published.is_some() {
            let compact = compact_covered(&self.spec);
            let membership = membership(get(&self.spec, "node_query")).is_some()
                || membership(get(&self.spec, "relation_query")).is_some();
            if compact || membership {
                let LensReply::Stores(stores) = self
                    .read
                    .request(LensNeed::Auxiliary {
                        compact,
                        membership,
                    })
                    .await?
                else {
                    return Err(corrupt("invalid lens auxiliary reply"));
                };
                if stores.compact && !compact || stores.membership && !membership {
                    return Err(corrupt("unrequested lens auxiliary store admitted"));
                }
                self.stores = stores;
            }
        }
        let spec = self.spec.clone();
        let node_limit = uint(field(&spec, "limits.nodes"));
        let relation_limit = uint(field(&spec, "limits.relations"));
        let focus = if let Some(id) = field(&spec, "seed.focus_node_id").as_str() {
            Some(self.focus(id).await?)
        } else {
            None
        };
        let mut matched_nodes = 0u64;
        let mut focus_matched = false;
        let mut winners: Vec<(Rc<RetainedRow>, Vec<String>, Vec<JsonValue>)> = vec![];
        let node_group = get(&spec, "node_query");
        if boolean(get(node_group, "enabled")) {
            let simple = self.published.is_some()
                && (self.dimensional(SearchKind::Nodes, node_group)
                    || self.stores.membership && membership(node_group).is_some())
                && array(field(&spec, "seed.node_ids")).is_empty()
                && string(field(&spec, "seed.text_query")).is_empty()
                && array(get(&spec, "path_query")).is_empty()
                && default_sort(field(&spec, "composition.sort_nodes"));
            if simple {
                let query = self.header_query(SearchKind::Nodes)?;
                matched_nodes = if query.membership.is_some() {
                    self.count(&query).await?
                } else {
                    self.histogram_count(SearchKind::Nodes)?
                };
                let expected = matched_nodes.min(node_limit as u64) as usize;
                let mut stream = OrderedStream {
                    query: Rc::new(query),
                    endpoint: None,
                    after: None,
                    rows: std::collections::VecDeque::new(),
                    done: false,
                };
                let mut ids = vec![];
                while ids.len() < expected {
                    let Some(row) = self.next_header(&mut stream).await? else {
                        break;
                    };
                    ids.push(row.id);
                }
                if ids.len() != expected {
                    return Err(corrupt("lens selector/count/order closure missing"));
                }
                if let Some(focus) = &focus {
                    focus_matched = self.group(focus, node_group)?;
                }
                for node in self.rows(SearchKind::Nodes, ids).await? {
                    winners.push((node, vec![], vec![]));
                }
            } else {
                let mut scan = self.scan(SearchKind::Nodes);
                loop {
                    let rows = self.scan_page(&mut scan).await?;
                    if rows.is_empty() {
                        break;
                    }
                    for node in rows {
                        self.check()?;
                        let seeds = array(field(&spec, "seed.node_ids"));
                        if !seeds.is_empty()
                            && !seeds.iter().any(|seed| {
                                ["id", "native_id", "entity_id"]
                                    .iter()
                                    .any(|name| get(&node, name) == seed)
                            })
                        {
                            continue;
                        }
                        let q = lower(string(field(&spec, "seed.text_query")));
                        if !q.is_empty() {
                            self.callback()?;
                            let compact = if let Some(original) = self.original {
                                original.canonicalize(&node, self.budget.inspect.json)?
                            } else {
                                canonical_bytes_v1(
                                    &node,
                                    CanonicalProfile::SourceRecordDigestV1,
                                    self.budget.inspect.json,
                                )
                                .map_err(|_| budget())?
                            };
                            let searchable = String::from_utf8(compact)
                                .map_err(|_| corrupt("lens searchable JSON invalid"))?;
                            if !lower(&crate::knowledge_lens::json_spaces(&searchable)).contains(&q)
                            {
                                continue;
                            }
                        }
                        if !self.group(&node, node_group)? {
                            continue;
                        }
                        let mut proofs = vec![];
                        let mut matched = true;
                        for condition in array(get(&spec, "path_query")) {
                            let id = string(get(&node, "id"));
                            let witness = self
                                .walk(
                                    id.to_owned(),
                                    0,
                                    array(get(condition, "steps")),
                                    vec![text(id)],
                                    vec![],
                                    string(get(condition, "path_id")),
                                )
                                .await?;
                            if witness.is_some()
                                != (string(get(condition, "quantifier")) == "exists")
                            {
                                matched = false;
                                break;
                            }
                            proofs.push(witness.unwrap_or_else(|| {
                                object(vec![
                                    ("path_id", get(condition, "path_id").clone()),
                                    ("absence_in_scope", JsonValue::Bool(true)),
                                ])
                            }));
                        }
                        if !matched {
                            continue;
                        }
                        matched_nodes = matched_nodes.checked_add(1).ok_or_else(budget)?;
                        if focus
                            .as_ref()
                            .is_some_and(|focus| get(focus, "id") == get(&node, "id"))
                        {
                            focus_matched = true;
                        }
                        let rules = field(&spec, "composition.sort_nodes");
                        let key = self.sort_key(&node, rules)?;
                        let place = winners.partition_point(|(_, existing, _)| {
                            compare_keys(existing, &key, rules) != std::cmp::Ordering::Greater
                        });
                        if place < node_limit {
                            winners.insert(place, (node, key, proofs));
                            if winners.len() > node_limit {
                                winners.pop();
                            }
                        }
                    }
                }
            }
        }
        let mut selected: BTreeMap<String, Rc<RetainedRow>> = BTreeMap::new();
        let mut inclusion = object(vec![]);
        let mut frontier = vec![];
        if let Some(row) = &focus {
            let id = string(get(row, "id")).to_owned();
            selected.insert(id.clone(), row.clone());
            frontier.push(id.clone());
            set(&mut inclusion, &id, object(vec![("kind", text("focus"))]));
            if !focus_matched {
                matched_nodes = matched_nodes.checked_add(1).ok_or_else(budget)?;
            }
        }
        for (row, _, proofs) in winners {
            if selected.len() >= node_limit {
                break;
            }
            let id = string(get(&row, "id")).to_owned();
            if !selected.contains_key(&id) {
                frontier.push(id.clone());
                selected.insert(id.clone(), row);
                set(
                    &mut inclusion,
                    &id,
                    object(vec![
                        ("kind", text("selector")),
                        ("path_witnesses", JsonValue::Array(proofs)),
                    ]),
                );
            }
        }
        let relation_group = get(&spec, "relation_query");
        let fast_relations = self.published.is_some()
            && boolean(get(relation_group, "enabled"))
            && (self.dimensional(SearchKind::Relations, relation_group)
                || self.stores.membership && membership(relation_group).is_some())
            && default_sort(field(&spec, "composition.sort_relations"));
        let relation_query = if fast_relations {
            Some(self.header_query(SearchKind::Relations)?)
        } else {
            None
        };
        let mut generic: Option<Vec<KeyedHeader>> =
            if fast_relations { None } else { Some(vec![]) };
        let matched_relations = if let Some(query) = &relation_query {
            if query.membership.is_some() {
                self.count(query).await?
            } else {
                self.histogram_count(SearchKind::Relations)?
            }
        } else {
            if boolean(get(relation_group, "enabled")) {
                let mut scan = self.scan(SearchKind::Relations);
                loop {
                    let rows = self.scan_page(&mut scan).await?;
                    if rows.is_empty() {
                        break;
                    }
                    for row in rows {
                        if crate::knowledge_lens::relation_regime(&row, &spec, &self.vocabulary)
                            && self.group(&row, relation_group)?
                        {
                            let key =
                                self.sort_key(&row, field(&spec, "composition.sort_relations"))?;
                            generic.as_mut().expect("generic relation selection").push(
                                KeyedHeader {
                                    header: LensHeader {
                                        id: string(get(&row, "id")).into(),
                                        from_id: string(get(&row, "from_id")).into(),
                                        to_id: string(get(&row, "to_id")).into(),
                                        sort_key: String::new(),
                                    },
                                    key,
                                },
                            );
                        }
                    }
                }
            }
            let headers = generic.as_mut().expect("generic relation selection");
            headers.sort_by(|a, b| {
                compare_keys(&a.key, &b.key, field(&spec, "composition.sort_relations"))
            });
            headers.len() as u64
        };
        let mut traversed = BTreeSet::new();
        let mut identity_limited = false;
        for depth in 0..uint(field(&spec, "traversal.depth")) {
            self.check()?;
            let mut origins = BTreeMap::new();
            if string(field(&spec, "traversal.profile")) == "overview" {
                for id in frontier.iter().collect::<BTreeSet<_>>() {
                    let entity = string(get(&selected[id], "entity_id"));
                    if self.vocabulary.declared_entity(entity) {
                        origins
                            .entry(entity.to_owned())
                            .or_insert_with(|| (*id).clone());
                    }
                }
            }
            let remaining = node_limit.saturating_sub(selected.len());
            let mut alias_ids = vec![];
            if !origins.is_empty() {
                if self.published.is_some() {
                    let LensReply::Ids(ids) = self
                        .read
                        .request(LensNeed::EntityAliasIds {
                            entities: origins.keys().cloned().collect(),
                            sources: self.sources.clone(),
                            exclude: selected.keys().cloned().collect(),
                            limit: remaining + 1,
                        })
                        .await?
                    else {
                        return Err(corrupt("invalid lens alias reply"));
                    };
                    self.check_ids(&ids, "", remaining + 1)?;
                    if ids.iter().any(|id| selected.contains_key(id)) {
                        return Err(corrupt("excluded lens alias returned"));
                    }
                    identity_limited |= ids.len() > remaining;
                    alias_ids.extend(ids.into_iter().take(remaining));
                } else {
                    let mut ids = BTreeSet::new();
                    for entity in origins.keys() {
                        for row in self.aliases("entity_id", entity).await? {
                            let id = string(get(&row, "id"));
                            if !selected.contains_key(id) {
                                ids.insert(id.to_owned());
                            }
                        }
                    }
                    identity_limited |= ids.len() > remaining;
                    alias_ids.extend(ids.into_iter().take(remaining));
                }
                for row in self.rows(SearchKind::Nodes, alias_ids).await? {
                    let id = string(get(&row, "id")).to_owned();
                    let entity = string(get(&row, "entity_id")).to_owned();
                    let origin = origins
                        .get(&entity)
                        .ok_or_else(|| corrupt("lens alias entity mirror differs"))?;
                    selected.insert(id.clone(), row);
                    frontier.push(id.clone());
                    set(
                        &mut inclusion,
                        &id,
                        object(vec![
                            ("kind", text("identity-carrier")),
                            ("via_node_id", text(origin)),
                            ("entity_id", text(&entity)),
                            ("depth", number(depth)),
                        ]),
                    );
                }
            }
            let current = frontier.iter().cloned().collect::<BTreeSet<_>>();
            let direction = string(field(&spec, "traversal.direction"));
            if self.published.is_none() {
                for id in &frontier {
                    drop(self.native_incident(id).await?);
                }
            }
            let mut cursor = if let Some(headers) = &generic {
                RelationCursor::generic(headers, Some((&current, direction)), None)
            } else {
                RelationCursor::ordered(
                    relation_query.clone().expect("fast relation query"),
                    Some((&current, direction)),
                )
            };
            let mut next = vec![];
            while let Some(relation) = cursor.next(&mut self, generic.as_deref()).await? {
                if self.published.is_some()
                    && selected.len() >= node_limit
                    && traversed.len() >= relation_limit
                {
                    break;
                }
                let mut touched = false;
                let row = object(vec![
                    ("from_id", text(&relation.from_id)),
                    ("to_id", text(&relation.to_id)),
                ]);
                for origin in &frontier {
                    for neighbor in crate::knowledge_lens::neighbors(&row, origin, direction) {
                        touched = true;
                        if !selected.contains_key(&neighbor) && selected.len() < node_limit {
                            if let Some(node) = self.scoped_neighbor(&neighbor).await? {
                                selected.insert(neighbor.clone(), node);
                                next.push(neighbor.clone());
                                set(
                                    &mut inclusion,
                                    &neighbor,
                                    object(vec![
                                        ("kind", text("traversal")),
                                        ("via_node_id", text(origin)),
                                        ("via_relation_id", text(&relation.id)),
                                        ("depth", number(depth + 1)),
                                    ]),
                                );
                            }
                        }
                    }
                }
                if touched && traversed.len() < relation_limit {
                    traversed.insert(relation.id);
                }
            }
            frontier = next;
            if frontier.is_empty() {
                break;
            }
        }
        let basis = selected.keys().cloned().collect::<BTreeSet<_>>();
        let policy = string(field(&spec, "composition.endpoint_policy"));
        let (eligible_count, mut eligible) = if let Some(headers) = &generic {
            let cursor = RelationCursor::generic(headers, None, Some((&basis, &traversed, policy)));
            (cursor.indices.len() as u64, cursor)
        } else {
            let mut query = relation_query.expect("fast relation query");
            if policy != "independent" {
                query.eligible = Some(if policy == "both" {
                    LensEligibility::Both {
                        basis: basis.iter().cloned().collect(),
                        traversed: traversed.iter().cloned().collect(),
                        pair_index: basis.len() <= 64,
                    }
                } else {
                    LensEligibility::Either {
                        basis: basis.iter().cloned().collect(),
                        traversed: traversed.iter().cloned().collect(),
                    }
                });
            }
            let count = if policy == "independent" {
                matched_relations
            } else {
                self.count(&query).await?
            };
            (count, RelationCursor::ordered(query, None))
        };
        let mut relation_ids = vec![];
        let mut examined = 0u64;
        let mut exhausted = true;
        while let Some(relation) = eligible.next(&mut self, generic.as_deref()).await? {
            if relation_ids.len() >= relation_limit {
                exhausted = false;
                break;
            }
            examined = examined.checked_add(1).ok_or_else(budget)?;
            let missing = [&relation.from_id, &relation.to_id]
                .into_iter()
                .filter(|id| !selected.contains_key(*id))
                .cloned()
                .collect::<BTreeSet<_>>();
            if selected.len() + missing.len() > node_limit {
                continue;
            }
            for endpoint in missing {
                if let Some(node) = self.scoped_neighbor(&endpoint).await? {
                    selected.insert(endpoint.clone(), node);
                    set(
                        &mut inclusion,
                        &endpoint,
                        object(vec![
                            ("kind", text("endpoint")),
                            ("via_relation_id", text(&relation.id)),
                        ]),
                    );
                }
            }
            if selected.contains_key(&relation.from_id) && selected.contains_key(&relation.to_id) {
                relation_ids.push(relation.id);
            }
        }
        if self.published.is_some() && exhausted && examined != eligible_count {
            return Err(corrupt("lens eligible/count/order closure incomplete"));
        }
        let expected_relations = relation_ids.len();
        let relations = self.rows(SearchKind::Relations, relation_ids).await?;
        if relations.len() != expected_relations {
            return Err(corrupt("selected lens relation closure absent"));
        }
        if let Some(limit) = self.sort_limit {
            // The maintained finalizer has its own returned-row sort-key cap;
            // it is independent of candidate sorting and does not spend the
            // matcher callback counter a second time.
            let mut bytes = 0usize;
            for (rules, rows) in [
                (
                    field(&spec, "composition.sort_nodes"),
                    selected.values().cloned().collect::<Vec<_>>(),
                ),
                (
                    field(&spec, "composition.sort_relations"),
                    relations.clone(),
                ),
            ] {
                for row in rows {
                    for rule in array(rules) {
                        let value = field(&row, string(get(rule, "field")));
                        let key = lower(&if crate::knowledge_lens::is_truthy(value) {
                            py_string(value)
                        } else {
                            String::new()
                        });
                        bytes = bytes.checked_add(key.len()).ok_or_else(budget)?;
                        if bytes > limit {
                            return Err(budget());
                        }
                    }
                }
            }
        }
        let resolved_focus = focus
            .as_ref()
            .map(|row| object(vec![("id", get(row, "id").clone())]));
        drop(focus);
        self.cache.clear();
        self.cache_bytes = 0;
        let unwrap = |row: Rc<RetainedRow>| {
            // Cache and focus references are gone. Transfer the one retained
            // payload; an accidental extra owner must not cause a hidden full
            // source copy in the terminal phase.
            let mut row =
                Rc::try_unwrap(row).map_err(|_| corrupt("lens terminal row still shared"))?;
            Ok((std::mem::replace(&mut row.value, JsonValue::Null), row))
        };
        let nodes = selected
            .into_values()
            .map(unwrap)
            .collect::<Result<Vec<_>, SearchV2Error>>()?;
        let relations = relations
            .into_iter()
            .map(unwrap)
            .collect::<Result<Vec<_>, SearchV2Error>>()?;
        let (nodes, node_guards): (Vec<_>, Vec<_>) = nodes.into_iter().unzip();
        let (relations, relation_guards): (Vec<_>, Vec<_>) = relations.into_iter().unzip();
        self.check()?;
        let packet = finalize_knowledge_lens(
            &self.public,
            nodes,
            relations,
            &revision,
            &authority,
            LensExecutionCounts {
                available_nodes: self.available.0,
                available_relations: self.available.1,
                matched_nodes,
                matched_relations,
                eligible_relations: eligible_count,
                identity_expansion_limited: identity_limited,
            },
            resolved_focus.as_ref(),
            &inclusion,
            &traversed,
            publication.as_ref(),
            &self.vocabulary,
        )?;
        drop((node_guards, relation_guards));
        Ok(packet)
    }
}

impl<'original> LensPlan<'original> {
    pub fn published(
        value: &JsonValue,
        metadata: &JsonValue,
        top: &JsonValue,
        revision: &str,
        publication: Option<&JsonValue>,
        budget: PublishedLensBudget,
    ) -> Result<Self, SearchV2Error> {
        Self::published_with_abort(value, metadata, top, revision, publication, budget, None)
    }
    pub(crate) fn published_with_abort(
        value: &JsonValue,
        metadata: &JsonValue,
        top: &JsonValue,
        revision: &str,
        publication: Option<&JsonValue>,
        budget: PublishedLensBudget,
        probe: Option<std::sync::Arc<dyn AbortProbe>>,
    ) -> Result<Self, SearchV2Error> {
        let vocabulary = LensVocabulary::from_published_metadata(metadata)?;
        let public = normalize_lens_spec(value, &vocabulary)?;
        let spec = bind_plan_properties(&public, &vocabulary)?;
        let sources = array(get(&spec, "sources"))
            .iter()
            .map(|v| string(v).to_owned())
            .collect::<Vec<_>>();
        let metadata = PublishedMetadata::parse(metadata)?;
        let total = |cells: &[Cell]| {
            cells
                .iter()
                .filter(|cell| sources.contains(&cell.keys[0]))
                .try_fold(0u64, |sum, cell| {
                    sum.checked_add(cell.count)
                        .filter(|sum| *sum <= 9_007_199_254_740_991)
                        .ok_or_else(budget_error)
                })
        };
        let available = (total(&metadata.nodes)?, total(&metadata.relations)?);
        if budget.max_callbacks == 0
            || budget.max_sort_bytes == 0
            || budget.max_cache_bytes == 0
            || budget.max_cache_entries == 0
        {
            return Err(budget_error());
        }
        Self::create(
            public,
            spec,
            vocabulary,
            revision,
            get(top, "authority_boundary").clone(),
            publication.cloned(),
            budget.lens,
            Some(metadata),
            available,
            Some((
                budget.max_callbacks,
                budget.max_sort_bytes,
                budget.max_cache_bytes,
                budget.max_cache_entries,
            )),
            probe,
            None,
        )
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn native(
        public: JsonValue,
        vocabulary: LensVocabulary,
        revision: &str,
        authority: JsonValue,
        publication: JsonValue,
        budget: LensBudget,
        available: (u64, u64),
        probe: Option<std::sync::Arc<dyn AbortProbe>>,
    ) -> Result<Self, SearchV2Error> {
        let spec = bind_plan_properties(&public, &vocabulary)?;
        Self::create(
            public,
            spec,
            vocabulary,
            revision,
            authority,
            Some(publication),
            budget,
            None,
            available,
            None,
            probe,
            None,
        )
    }
    /// Controlled native construction. `workspace_bytes` is the driver's
    /// checked upper bound for its thin traversal/sort/identity frames. The
    /// same driver separately admits actual vocabulary, authenticated row,
    /// reply and packet geometries before their corresponding plan phase.
    /// All admissions stay with the original owner through disclosure.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn native_with_original(
        public: JsonValue,
        vocabulary: LensVocabulary,
        revision: &str,
        authority: JsonValue,
        publication: JsonValue,
        budget: LensBudget,
        available: (u64, u64),
        probe: Option<std::sync::Arc<dyn AbortProbe>>,
        original: &'original dyn OriginalLensBudget,
        workspace_bytes: usize,
    ) -> Result<Self, SearchV2Error> {
        if workspace_bytes == 0 {
            return Err(budget_error());
        }
        original.check()?;
        original.admit_workspace(workspace_bytes)?;
        original.charge_work(workspace_bytes)?;
        let spec = bind_plan_properties(&public, &vocabulary)?;
        Self::create(
            public,
            spec,
            vocabulary,
            revision,
            authority,
            Some(publication),
            budget,
            None,
            available,
            None,
            probe,
            Some(original),
        )
    }

    fn create(
        public: JsonValue,
        spec: JsonValue,
        vocabulary: LensVocabulary,
        revision: &str,
        authority: JsonValue,
        publication: Option<JsonValue>,
        budget: LensBudget,
        published: Option<PublishedMetadata>,
        available: (u64, u64),
        limits: Option<(usize, usize, usize, usize)>,
        probe: Option<std::sync::Arc<dyn AbortProbe>>,
        original: Option<&'original dyn OriginalLensBudget>,
    ) -> Result<Self, SearchV2Error> {
        if budget.max_candidates == 0
            || budget.max_candidates >= i64::MAX as usize
            || budget.max_path_steps == 0
            || budget.max_adjacency_rows == 0
            || budget.block_size == 0
            || budget.block_size as u64 > budget.inspect.max_rows
        {
            return Err(budget_error());
        }
        let live = Rc::new(std::cell::Cell::new(0));
        let read = LensRead(Rc::new(RefCell::new(ReadSlot {
            live: live.clone(),
            reply_bytes: 0,
            max_source_bytes: budget.inspect.max_decoded_bytes,
            max_row_bytes: budget.inspect.max_payload_bytes,
            max_rows: budget.inspect.max_rows,
            need: None,
            reply: None,
        })));
        let sources = array(get(&spec, "sources"))
            .iter()
            .map(|v| string(v).to_owned())
            .collect();
        let execution = Execution {
            original,
            read: read.clone(),
            vocabulary,
            public,
            spec,
            sources,
            budget,
            published,
            stores: LensStores::default(),
            available,
            candidates: 0,
            adjacency: 0,
            path_steps: 0,
            callbacks: 0,
            sort_bytes: 0,
            callback_limit: limits.map(|v| v.0),
            sort_limit: limits.map(|v| v.1),
            cache: BTreeMap::new(),
            cache_tick: 0,
            cache_bytes: 0,
            cache_limit: limits.map(|v| (v.2, v.3)),
            live_bytes: live,
            probe,
        };
        Ok(Self {
            read,
            execution: Some(Box::pin(execution.execute(
                revision.to_owned(),
                authority,
                publication,
            ))),
            output: None,
            terminal: false,
        })
    }
    /// Run one continuation until a concrete read or terminal packet. Never
    /// wait or repoll a pending read; the host must supply exactly one reply.
    pub fn advance(&mut self) -> Result<bool, SearchV2Error> {
        if self.terminal {
            return Err(corrupt("lens plan already terminal"));
        }
        if self.read.0.borrow().need.is_some() {
            return Err(corrupt("lens reply is required before advance"));
        }
        // The execution itself also checks between bounded domain phases.
        let mut context = Context::from_waker(Waker::noop());
        let result = self
            .execution
            .as_mut()
            .ok_or_else(|| corrupt("lens execution unavailable"))?
            .as_mut()
            .poll(&mut context);
        match result {
            Poll::Pending => {
                if self.read.0.borrow().need.is_none() {
                    self.clear();
                    return Err(corrupt("lens continuation lacks concrete need"));
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
    pub fn need(&self) -> Option<Rc<LensNeed>> {
        self.read.0.borrow().need.clone()
    }
    pub fn resume(&mut self, reply: LensReply) -> Result<(), SearchV2Error> {
        self.supply(Ok(reply))
    }
    pub fn fail(&mut self, error: SearchV2Error) -> Result<(), SearchV2Error> {
        self.supply(Err(error))
    }
    fn supply(&mut self, reply: Result<LensReply, SearchV2Error>) -> Result<(), SearchV2Error> {
        if self.terminal {
            return Err(corrupt("lens plan already terminal"));
        }
        let mut slot = self.read.0.borrow_mut();
        if slot.need.is_none() || slot.reply.is_some() {
            drop(slot);
            self.clear();
            return Err(corrupt("lens has no outstanding read"));
        }
        let bytes = match &reply {
            Ok(LensReply::Rows { rows, raw_bytes }) => {
                if rows.len() != raw_bytes.len()
                    || rows.len() as u64 > slot.max_rows
                    || raw_bytes
                        .iter()
                        .any(|bytes| *bytes == 0 || *bytes > slot.max_row_bytes)
                {
                    return Err(corrupt("invalid lens row byte framing"));
                }
                raw_bytes.iter().try_fold(0usize, |sum, bytes| {
                    sum.checked_add(*bytes).ok_or_else(budget)
                })?
            }
            _ => 0,
        };
        let live = slot.live.get().checked_add(bytes).ok_or_else(budget)?;
        if live as u64 > slot.max_source_bytes {
            return Err(budget());
        }
        slot.live.set(live);
        slot.reply_bytes = bytes;
        slot.need = None;
        slot.reply = Some(reply);
        Ok(())
    }
    fn clear(&mut self) {
        self.execution = None;
        let mut slot = self.read.0.borrow_mut();
        slot.need = None;
        slot.reply = None;
        slot.live.set(
            slot.live
                .get()
                .checked_sub(slot.reply_bytes)
                .expect("lens reply charged once"),
        );
        slot.reply_bytes = 0;
        self.terminal = true;
    }
    pub fn finish(&mut self) -> Result<JsonValue, SearchV2Error> {
        if !self.terminal {
            return Err(corrupt("lens plan is not terminal"));
        }
        self.output
            .take()
            .ok_or_else(|| corrupt("lens terminal packet already consumed"))
    }
}
impl Drop for LensPlan<'_> {
    fn drop(&mut self) {
        self.clear();
    }
}
fn budget_error() -> SearchV2Error {
    budget()
}
