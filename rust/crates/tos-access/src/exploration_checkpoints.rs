//! Disposable process-owned exploration storage. Source/corpus bytes are never
//! persisted here. Random cursors identify opaque QRY states, not authority.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonString, JsonValue, canonical_bytes_v1,
};
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
    value: ExplorationCheckpoint,
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
    fn purge(store: &mut Store, now: Instant) {
        let tokens = store
            .entries
            .iter()
            .filter(|(token, entry)| entry.expires <= now && !store.busy.contains(*token))
            .map(|(token, _)| token.clone())
            .collect::<Vec<_>>();
        for token in tokens {
            if let Some(entry) = store.entries.remove(&token) {
                store.encoded_bytes -= entry.encoded_bytes;
            }
        }
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
        let mut store = lock(&self.store)?;
        Self::purge(&mut store, Instant::now());
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
        Ok(match &entry.value {
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
        let mut json = limits.read.json;
        json.max_bytes = json.max_bytes.min(limits.max_state_bytes);
        let state_bytes = successor
            .map(|state| state.encoded_state(json).map(|bytes| bytes.len()))
            .transpose()?
            .unwrap_or(0);
        let mut store = lock(&self.store)?;
        Self::purge(&mut store, Instant::now());
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
            if !matches!(entry.value, ExplorationCheckpoint::State(_)) {
                return Err(corrupt());
            }
        }
        let next = if successor.is_some() {
            let mut next = None;
            for _ in 0..4 {
                let candidate = token()?;
                if !store.entries.contains_key(&candidate)
                    && !store.reserved_tokens.contains(&candidate)
                {
                    next = Some(candidate);
                    break;
                }
            }
            Some(next.ok_or_else(|| {
                error(
                    SearchV2ErrorCode::Unavailable,
                    "exploration cursor collision",
                )
            })?)
        } else {
            None
        };
        let replay = set_cursor(packet, next.as_deref())?;
        let mut json = limits.read.json;
        json.max_bytes = json.max_bytes.min(limits.read.max_response_bytes);
        let bytes = canonical_bytes_v1(&replay, CanonicalProfile::SourceRecordDigestV1, json)
            .map_err(|_| budget())?;
        let replay_bytes = if input.is_some() { bytes.len() } else { 0 };
        let reserved = state_bytes.checked_add(replay_bytes).ok_or_else(budget)?;
        let cap = self
            .limits
            .max_encoded_bytes
            .min(limits.max_checkpoint_bytes);
        let count_cap = self.limits.max_entries.min(limits.max_checkpoints);
        // Staged clones and committed inputs coexist until commit. Account
        // both, refusing capacity rather than evicting an active continuation.
        if store
            .entries
            .len()
            .checked_add(store.reserved_tokens.len())
            .and_then(|n| n.checked_add(usize::from(next.is_some())))
            .is_none_or(|n| n > count_cap)
            || store
                .encoded_bytes
                .checked_add(store.reserved_bytes)
                .and_then(|n| n.checked_add(reserved))
                .is_none_or(|n| n > cap)
        {
            return Err(budget());
        }
        store.reserved_bytes += reserved;
        if let Some(input) = input {
            store.busy.insert(input.to_owned());
        }
        if let Some(next) = &next {
            store.reserved_tokens.insert(next.clone());
        }
        drop(store);
        Ok(Box::new(Staged {
            store: Arc::clone(&self.store),
            input: input.map(str::to_owned),
            revision: revision.to_owned(),
            next,
            state: successor.cloned(),
            state_bytes,
            replay,
            replay_sha: Digest256::of_bytes(&bytes),
            replay_bytes,
            reserved,
            ttl: self.limits.ttl,
            done: false,
        }))
    }
}
struct Staged {
    store: Arc<Mutex<Store>>,
    input: Option<String>,
    revision: String,
    next: Option<String>,
    state: Option<ExplorationState>,
    state_bytes: usize,
    replay: JsonValue,
    replay_sha: Digest256,
    replay_bytes: usize,
    reserved: usize,
    ttl: Duration,
    done: bool,
}
impl PreparedExplorationCheckpoint for Staged {
    fn next_cursor(&self) -> Option<&str> {
        self.next.as_deref()
    }
    fn commit(&mut self) -> Result<(), SearchV2Error> {
        if self.done {
            return Err(corrupt());
        }
        let expires = Instant::now().checked_add(self.ttl).ok_or_else(corrupt)?;
        let mut store = lock(&self.store)?;
        if self.next.is_some() != self.state.is_some()
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
        if let Some(input) = &self.input {
            let old = store
                .entries
                .remove(input)
                .expect("held input checked under the same lock");
            store.encoded_bytes -= old.encoded_bytes;
            store.entries.insert(
                input.clone(),
                Entry {
                    revision: self.revision.clone(),
                    expires,
                    value: ExplorationCheckpoint::Replay {
                        packet: std::mem::replace(&mut self.replay, JsonValue::Null),
                        packet_sha256: self.replay_sha,
                    },
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
                    value: ExplorationCheckpoint::State(state),
                    encoded_bytes: self.state_bytes,
                },
            );
            store.encoded_bytes += self.state_bytes;
            store.reserved_tokens.remove(next);
        }
        store.reserved_bytes -= self.reserved;
        self.done = true;
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
