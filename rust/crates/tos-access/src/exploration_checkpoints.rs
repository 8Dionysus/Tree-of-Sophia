//! Disposable process-owned exploration storage. Source/corpus bytes are never
//! persisted here. Random cursors identify opaque QRY states, not authority.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tos_foundation::{Digest256, JsonString, JsonValue};
use tos_query::{
    knowledge_exploration::{
        ExplorationBudget, ExplorationCheckpoint, ExplorationCheckpoints, ExplorationState,
        PreparedExplorationCheckpoint,
    },
    search_v2::{SearchV2Error, SearchV2ErrorCode},
};

#[derive(Clone, Copy)]
pub struct CheckpointLimits {
    pub ttl: Duration,
    pub max_entries: usize,
    /// Canonical encoded residency/admission cap, not parsed allocation or RSS.
    /// State cardinalities and JSON limits belong to the QRY caller budget.
    pub max_encoded_bytes: usize,
}
#[derive(Clone)]
pub struct ProcessExplorationCheckpoints {
    store: Arc<Mutex<Store>>,
    limits: CheckpointLimits,
}
struct Store {
    entries: BTreeMap<String, Entry>,
    busy: BTreeSet<String>,
    reserved_tokens: BTreeSet<String>,
    encoded_bytes: usize,
    reserved_bytes: usize,
}
struct Entry {
    revision: String,
    expires: Instant,
    value: Arc<ExplorationCheckpoint>,
    encoded_bytes: usize,
}
fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn corrupt() -> SearchV2Error {
    error(
        SearchV2ErrorCode::CorruptSelectedCarrier,
        "exploration checkpoint store invariant failed",
    )
}
fn budget() -> SearchV2Error {
    error(
        SearchV2ErrorCode::BudgetExceeded,
        "exploration checkpoint capacity exceeded",
    )
}
fn expired() -> SearchV2Error {
    error(
        SearchV2ErrorCode::CursorExpired,
        "exploration checkpoint expired or unavailable; restart",
    )
}
fn lock(store: &Arc<Mutex<Store>>) -> Result<std::sync::MutexGuard<'_, Store>, SearchV2Error> {
    store.lock().map_err(|_| corrupt())
}
impl ProcessExplorationCheckpoints {
    pub fn limits(&self) -> CheckpointLimits {
        self.limits
    }
    pub fn new(limits: CheckpointLimits) -> Result<Self, SearchV2Error> {
        if limits.ttl.is_zero()
            || limits.max_entries == 0
            || limits.max_entries > 128
            || limits.max_encoded_bytes == 0
            || limits.max_encoded_bytes > 32 * 1024 * 1024
        {
            return Err(error(
                SearchV2ErrorCode::InvalidRequest,
                "invalid process exploration checkpoint limits",
            ));
        }
        Ok(Self {
            store: Arc::new(Mutex::new(Store {
                entries: BTreeMap::new(),
                busy: BTreeSet::new(),
                reserved_tokens: BTreeSet::new(),
                encoded_bytes: 0,
                reserved_bytes: 0,
            })),
            limits,
        })
    }
    /// Expired entries have no authority value. Active preparations retain
    /// their exact input until either atomic commit or rollback.
    fn purge(store: &mut Store, now: Instant) -> Vec<Entry> {
        let tokens = store
            .entries
            .iter()
            .filter(|(token, entry)| entry.expires <= now && !store.busy.contains(*token))
            .map(|(token, _)| token.clone())
            .collect::<Vec<_>>();
        let mut retired = Vec::new();
        for token in tokens {
            if let Some(entry) = store.entries.remove(&token) {
                store.encoded_bytes -= entry.encoded_bytes;
                retired.push(entry);
            }
        }
        retired
    }
}
fn token() -> Result<String, SearchV2Error> {
    let mut entropy = [0u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut entropy))
        .map_err(|_| {
            error(
                SearchV2ErrorCode::Unavailable,
                "exploration cursor entropy unavailable",
            )
        })?;
    Ok(Digest256::of_bytes(&entropy).to_hex())
}
fn set_cursor(packet: &JsonValue, cursor: Option<&str>) -> Result<JsonValue, SearchV2Error> {
    let mut packet = packet.clone();
    let JsonValue::Object(fields) = &mut packet else {
        return Err(corrupt());
    };
    let page = fields
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some("page"))
        .ok_or_else(corrupt)?;
    let JsonValue::Object(fields) = &mut page.1 else {
        return Err(corrupt());
    };
    let value = cursor.map_or(JsonValue::Null, |s| {
        JsonValue::String(JsonString::from_utf8(s))
    });
    if let Some((_, old)) = fields
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some("next_cursor"))
    {
        *old = value
    } else {
        fields.push((JsonString::from_utf8("next_cursor"), value));
    }
    Ok(packet)
}
impl ExplorationCheckpoints for ProcessExplorationCheckpoints {
    fn load(
        &mut self,
        cursor: &str,
        revision: &str,
    ) -> Result<ExplorationCheckpoint, SearchV2Error> {
        let (value, retired) = {
            let mut store = lock(&self.store)?;
            let retired = Self::purge(&mut store, Instant::now());
            // Keep retired ownership outside the lock even on an error.
            let result = (|| {
                let entry = store.entries.get(cursor).ok_or_else(expired)?;
                if entry.revision != revision {
                    return Err(error(
                        SearchV2ErrorCode::StaleContinuation,
                        "exploration snapshot or policy changed; restart",
                    ));
                }
                if store.busy.contains(cursor) {
                    return Err(error(
                        SearchV2ErrorCode::Unavailable,
                        "exploration continuation already being prepared",
                    ));
                }
                Ok(Arc::clone(&entry.value))
            })();
            (result, retired)
        };
        drop(retired);
        let value = value?;
        // The immutable snapshot linearizes at lookup; deep copying cannot
        // block another cursor's lookup/reservation/commit.
        Ok(match value.as_ref() {
            ExplorationCheckpoint::State(state) => ExplorationCheckpoint::State(state.clone()),
            ExplorationCheckpoint::Replay {
                packet,
                packet_sha256,
            } => ExplorationCheckpoint::Replay {
                packet: packet.clone(),
                packet_sha256: *packet_sha256,
            },
        })
    }
    fn prepare(
        &mut self,
        input: Option<&str>,
        revision: &str,
        successor: Option<&ExplorationState>,
        packet: &JsonValue,
        limits: ExplorationBudget,
    ) -> Result<Box<dyn PreparedExplorationCheckpoint>, SearchV2Error> {
        if revision.is_empty()
            || successor.is_some_and(|state| state.snapshot_revision() != revision)
        {
            return Err(corrupt());
        }
        // Entropy and all structural/encoded work occur outside the store lock.
        let candidates = if successor.is_some() {
            (0..4).map(|_| token()).collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
        let (reservation, retired) = {
            let mut store = lock(&self.store)?;
            let retired = Self::purge(&mut store, Instant::now());
            let result = (|| {
                if let Some(input) = input {
                    let entry = store.entries.get(input).ok_or_else(expired)?;
                    if entry.revision != revision {
                        return Err(error(
                            SearchV2ErrorCode::StaleContinuation,
                            "exploration snapshot or policy changed; restart",
                        ));
                    }
                    if store.busy.contains(input) {
                        return Err(error(
                            SearchV2ErrorCode::Unavailable,
                            "exploration continuation already being prepared",
                        ));
                    }
                    if !matches!(entry.value.as_ref(), ExplorationCheckpoint::State(_)) {
                        return Err(corrupt());
                    }
                }
                let next = if successor.is_some() {
                    Some(
                        candidates
                            .into_iter()
                            .find(|candidate| {
                                !store.entries.contains_key(candidate)
                                    && !store.reserved_tokens.contains(candidate)
                            })
                            .ok_or_else(|| {
                                error(
                                    SearchV2ErrorCode::Unavailable,
                                    "exploration cursor collision",
                                )
                            })?,
                    )
                } else {
                    None
                };
                let count_cap = self.limits.max_entries.min(limits.max_checkpoints);
                if store
                    .entries
                    .len()
                    .checked_add(store.reserved_tokens.len())
                    .and_then(|n| n.checked_add(usize::from(next.is_some())))
                    .is_none_or(|n| n > count_cap)
                {
                    return Err(budget());
                }
                let expires = match input {
                    Some(input) => store.entries.get(input).ok_or_else(expired)?.expires,
                    None => Instant::now()
                        .checked_add(self.limits.ttl)
                        .ok_or_else(corrupt)?,
                };
                if let Some(input) = input {
                    store.busy.insert(input.to_owned());
                }
                if let Some(next) = &next {
                    store.reserved_tokens.insert(next.clone());
                }
                Ok(Staged {
                    store: Arc::clone(&self.store),
                    input: input.map(str::to_owned),
                    revision: revision.to_owned(),
                    next,
                    state: None,
                    state_bytes: 0,
                    replay: None,
                    replay_packet: None,
                    response_staged: false,
                    response_cap: limits.read.max_response_bytes,
                    checkpoint_cap: self
                        .limits
                        .max_encoded_bytes
                        .min(limits.max_checkpoint_bytes),
                    replay_bytes: 0,
                    reserved: 0,
                    expires,
                    done: false,
                })
            })();
            (result, retired)
        };
        drop(retired);
        // RAII releases the input/token reservation on every structural/count or
        // byte admission failure, preserving the original input unchanged.
        let mut staged = reservation?;
        let mut json = limits.read.json;
        json.max_bytes = json.max_bytes.min(limits.max_state_bytes);
        staged.state_bytes = successor
            .map(|state| state.encoded_state_count(json))
            .transpose()?
            .unwrap_or(0);
        // Only parsed replay ownership is retained. Its exact encoded length
        // and digest arrive from QRY's one final emission before disclosure.
        if input.is_some() {
            staged.replay_packet = Some(set_cursor(packet, staged.next.as_deref())?);
        }
        let reserved = staged.state_bytes;
        let cap = staged.checkpoint_cap;
        {
            let mut store = lock(&self.store)?;
            // Input and successor tokens have been protected since lookup.
            // Committed inputs and staged successors coexist until commit.
            if store
                .encoded_bytes
                .checked_add(store.reserved_bytes)
                .and_then(|n| n.checked_add(reserved))
                .is_none_or(|n| n > cap)
            {
                return Err(budget());
            }
            store.reserved_bytes += reserved;
            staged.reserved = reserved;
        }
        staged.state = successor
            .cloned()
            .map(|state| Arc::new(ExplorationCheckpoint::State(state)));
        Ok(Box::new(staged))
    }
}

struct Staged {
    store: Arc<Mutex<Store>>,
    input: Option<String>,
    revision: String,
    next: Option<String>,
    state: Option<Arc<ExplorationCheckpoint>>,
    state_bytes: usize,
    replay: Option<Arc<ExplorationCheckpoint>>,
    replay_packet: Option<JsonValue>,
    response_staged: bool,
    response_cap: usize,
    checkpoint_cap: usize,
    replay_bytes: usize,
    reserved: usize,
    expires: Instant,
    done: bool,
}
impl PreparedExplorationCheckpoint for Staged {
    fn next_cursor(&self) -> Option<&str> {
        self.next.as_deref()
    }
    fn stage_response(&mut self, body: &[u8]) -> Result<(), SearchV2Error> {
        if self.done || self.response_staged {
            return Err(corrupt());
        }
        if body.len() > self.response_cap {
            return Err(budget());
        }
        let replay_bytes = if self.input.is_some() { body.len() } else { 0 };
        let digest = if self.input.is_some() {
            Some(Digest256::of_bytes(body))
        } else {
            None
        };
        // Hashing and typed ownership transfer stay outside the global lock.
        {
            let mut store = lock(&self.store)?;
            if store
                .encoded_bytes
                .checked_add(store.reserved_bytes)
                .and_then(|n| n.checked_add(replay_bytes))
                .is_none_or(|n| n > self.checkpoint_cap)
            {
                return Err(budget());
            }
            store.reserved_bytes += replay_bytes;
            self.reserved += replay_bytes;
        }
        if let Some(digest) = digest {
            let packet = self.replay_packet.take().ok_or_else(corrupt)?;
            self.replay = Some(Arc::new(ExplorationCheckpoint::Replay {
                packet,
                packet_sha256: digest,
            }));
        }
        self.replay_bytes = replay_bytes;
        self.response_staged = true;
        Ok(())
    }
    fn commit(&mut self) -> Result<(), SearchV2Error> {
        if self.done || !self.response_staged {
            return Err(corrupt());
        }
        let expires = self.expires;
        let mut store = lock(&self.store)?;
        if self.next.is_some() != self.state.is_some()
            || self.input.is_some() != self.replay.is_some()
            || self.input.as_ref().is_some_and(|input| {
                !store.busy.contains(input) || !store.entries.contains_key(input)
            })
            || self.next.as_ref().is_some_and(|next| {
                !store.reserved_tokens.contains(next) || store.entries.contains_key(next)
            })
            || store.reserved_bytes < self.reserved
        {
            return Err(corrupt());
        }
        let mut retired = None;
        if let Some(input) = &self.input {
            let old = store
                .entries
                .remove(input)
                .expect("held input checked under the same lock");
            store.encoded_bytes -= old.encoded_bytes;
            retired = Some(old);
            store.entries.insert(
                input.clone(),
                Entry {
                    revision: self.revision.clone(),
                    expires,
                    value: self
                        .replay
                        .take()
                        .expect("staged replay admitted before commit"),
                    encoded_bytes: self.replay_bytes,
                },
            );
            store.encoded_bytes += self.replay_bytes;
            store.busy.remove(input);
        }
        if let Some(next) = &self.next {
            let state = self
                .state
                .take()
                .expect("staged successor checked before mutation");
            store.entries.insert(
                next.clone(),
                Entry {
                    revision: self.revision.clone(),
                    expires,
                    value: state,
                    encoded_bytes: self.state_bytes,
                },
            );
            store.encoded_bytes += self.state_bytes;
            store.reserved_tokens.remove(next);
        }
        store.reserved_bytes -= self.reserved;
        self.done = true;
        drop(store);
        drop(retired);
        Ok(())
    }
}
impl Drop for Staged {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        if let Ok(mut store) = self.store.lock() {
            store.reserved_bytes = store.reserved_bytes.saturating_sub(self.reserved);
            if let Some(input) = &self.input {
                store.busy.remove(input);
            }
            if let Some(next) = &self.next {
                store.reserved_tokens.remove(next);
            }
        }
    }
}
