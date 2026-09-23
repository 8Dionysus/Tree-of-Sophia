//! Source-only v1 research workspace transition contract. A host owns storage,
//! clocks, listeners and page commands; this rule owns validated state changes.
//!
//! Requests and responses are JSON DTOs so native and WASM consume the same
//! wire packet. The caller supplies normalized snake_case command values and
//! an explicit `created_at` when staging a proposal.

use std::collections::HashSet;
use tos_foundation::{
    emit_value_preserved_json, parse_json, JsonLimits, JsonMode, JsonNumber, JsonNumberKind,
    JsonString, JsonValue,
};

use crate::workspace_proposal::workspace_proposal_digest_v1;

const SCHEMA: &str = "tos_research_workspace_session_v1";
const ABI: &str = "tos_research_workspace_transition_v1";
const MAX_BYTES: usize = 16_000_000;
const MAX_PACKET_UNITS: usize = 1_000_000;
const MAX_REVISION: u64 = 1_000_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceMachineErrorCode {
    InvalidRequest,
    InvalidPacket,
    InvalidCommand,
    Conflict,
    StaleRevision,
    OutputBudget,
}
impl WorkspaceMachineErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::InvalidPacket => "invalid_packet",
            Self::InvalidCommand => "invalid_command",
            Self::Conflict => "conflict",
            Self::StaleRevision => "stale_revision",
            Self::OutputBudget => "output_budget",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceMachineError {
    pub code: WorkspaceMachineErrorCode,
}
fn err(code: WorkspaceMachineErrorCode) -> WorkspaceMachineError {
    WorkspaceMachineError { code }
}
fn invalid_packet() -> WorkspaceMachineError {
    err(WorkspaceMachineErrorCode::InvalidPacket)
}
fn invalid_command() -> WorkspaceMachineError {
    err(WorkspaceMachineErrorCode::InvalidCommand)
}
fn get<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    value.object_get(key)
}
fn str_field<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    get(value, key)?.as_str()
}
fn int_field(value: &JsonValue, key: &str, max: u64) -> Option<u64> {
    get(value, key)?.as_u64().filter(|item| *item <= max)
}
fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn integer(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn object(items: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        items
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}
fn set(value: &mut JsonValue, name: &str, replacement: JsonValue) {
    let JsonValue::Object(entries) = value else {
        unreachable!()
    };
    if let Some((_, current)) = entries
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some(name))
    {
        *current = replacement;
    } else {
        entries.push((JsonString::from_utf8(name), replacement));
    }
}
fn array_mut<'a>(value: &'a mut JsonValue, name: &str) -> &'a mut Vec<JsonValue> {
    let JsonValue::Object(entries) = value else {
        unreachable!()
    };
    let (_, JsonValue::Array(items)) = entries
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some(name))
        .expect("validated packet collection")
    else {
        unreachable!()
    };
    items
}
fn exact(value: &JsonValue, required: &[&str], optional: &[&str]) -> bool {
    let Some(entries) = value.as_object() else {
        return false;
    };
    required.iter().all(|key| get(value, key).is_some())
        && entries.iter().all(|(key, _)| {
            key.as_str()
                .is_some_and(|name| required.contains(&name) || optional.contains(&name))
        })
}
fn js_trim(ch: char) -> bool {
    matches!(ch, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}'
        | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}'
        | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
fn bounded(value: &JsonValue, key: &str, max: usize) -> bool {
    str_field(value, key).is_some_and(|item| {
        !item.is_empty() && item.trim_matches(js_trim) == item && item.encode_utf16().count() <= max
    })
}
fn optional_bounded(value: &JsonValue, key: &str, max: usize) -> bool {
    get(value, key).is_none() || bounded(value, key, max)
}
fn unique_strings(value: &JsonValue, key: &str, max_items: usize, max_len: usize) -> bool {
    let Some(items) = get(value, key).and_then(JsonValue::as_array) else {
        return false;
    };
    if items.len() > max_items {
        return false;
    }
    let mut seen = HashSet::new();
    items.iter().all(|item| {
        item.as_str().is_some_and(|word| {
            !word.is_empty()
                && word.trim_matches(js_trim) == word
                && word.encode_utf16().count() <= max_len
                && seen.insert(word)
        })
    })
}
fn unique_ids(items: &[JsonValue]) -> bool {
    let mut seen = HashSet::new();
    items
        .iter()
        .all(|item| str_field(item, "id").is_some_and(|id| seen.insert(id)))
}
fn collection<'a>(
    packet: &'a JsonValue,
    name: &str,
    max: usize,
) -> Result<&'a [JsonValue], WorkspaceMachineError> {
    let items = get(packet, name)
        .and_then(JsonValue::as_array)
        .ok_or_else(invalid_packet)?;
    if items.len() > max {
        return Err(invalid_packet());
    }
    Ok(items)
}
fn validate_selection(value: &JsonValue) -> bool {
    value.is_null()
        || exact(value, &["id", "kind"], &["label"])
            && bounded(value, "id", 256)
            && optional_bounded(value, "label", 512)
            && matches!(
                str_field(value, "kind"),
                Some("node" | "edge" | "cluster" | "item")
            )
}
fn validate_hypothesis(value: &JsonValue) -> bool {
    let Some(posture) = get(value, "posture") else {
        return false;
    };
    exact(
        value,
        &["id", "title", "body", "posture"],
        &["target_id", "from_id", "to_id", "predicate_label"],
    ) && bounded(value, "id", 256)
        && bounded(value, "title", 512)
        && bounded(value, "body", 4000)
        && optional_bounded(value, "target_id", 256)
        && optional_bounded(value, "from_id", 256)
        && optional_bounded(value, "to_id", 256)
        && optional_bounded(value, "predicate_label", 512)
        && get(value, "from_id").is_some() == get(value, "to_id").is_some()
        && exact(
            posture,
            &["session_hypothesis", "source", "reviewed", "canon"],
            &[],
        )
        && get(posture, "session_hypothesis").and_then(JsonValue::as_bool) == Some(true)
        && ["source", "reviewed", "canon"]
            .iter()
            .all(|key| get(posture, key).and_then(JsonValue::as_bool) == Some(false))
}
fn validate_route(value: &JsonValue) -> bool {
    exact(
        value,
        &["id", "label", "from_id", "to_id", "node_ids", "edge_ids"],
        &[],
    ) && bounded(value, "id", 256)
        && bounded(value, "label", 512)
        && bounded(value, "from_id", 256)
        && bounded(value, "to_id", 256)
        && unique_strings(value, "node_ids", 512, 256)
        && unique_strings(value, "edge_ids", 512, 256)
}
fn validate_note(value: &JsonValue) -> bool {
    exact(value, &["id", "body"], &["target_id"])
        && bounded(value, "id", 256)
        && bounded(value, "body", 4000)
        && optional_bounded(value, "target_id", 256)
}
fn validate_journal(value: &JsonValue, previous: u64) -> Option<u64> {
    if !exact(value, &["sequence", "action"], &["target_id", "detail"])
        || !bounded(value, "action", 256)
        || !optional_bounded(value, "target_id", 256)
    {
        return None;
    }
    let sequence = int_field(value, "sequence", MAX_REVISION)?;
    if sequence <= previous {
        return None;
    }
    if let Some(detail) = get(value, "detail") {
        let entries = detail.as_object()?;
        if entries.len() > 32
            || entries.iter().any(|(key, item)| {
                key.as_str().is_none_or(|name| {
                    name.is_empty()
                        || name.trim_matches(js_trim) != name
                        || name.encode_utf16().count() > 256
                }) || !matches!(
                    item,
                    JsonValue::Null
                        | JsonValue::Bool(_)
                        | JsonValue::String(_)
                        | JsonValue::Number(_)
                )
                    || matches!(item, JsonValue::Number(number) if !number.lexeme.parse::<f64>().is_ok_and(f64::is_finite))
            })
        {
            return None;
        }
    }
    Some(sequence)
}
fn stable(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Array(items) => JsonValue::Array(items.iter().map(stable).collect()),
        JsonValue::Object(entries) => {
            let mut output: Vec<_> = entries
                .iter()
                .map(|(key, item)| (key.clone(), stable(item)))
                .collect();
            output.sort_by(|(left, _), (right, _)| left.units().cmp(right.units()));
            JsonValue::Object(output)
        }
        _ => value.clone(),
    }
}
fn emit(value: &JsonValue) -> Result<Vec<u8>, WorkspaceMachineError> {
    emit_value_preserved_json(
        &stable(value),
        JsonLimits {
            max_bytes: MAX_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| err(WorkspaceMachineErrorCode::OutputBudget))
}
fn proposal_request(
    proposal: &JsonValue,
    hypothesis_ids: &[JsonValue],
    operation: &str,
    revision: u64,
    existing: &[JsonValue],
) -> JsonValue {
    object(vec![
        ("operation", text(operation)),
        ("proposal", proposal.clone()),
        ("hypothesis_ids", JsonValue::Array(hypothesis_ids.to_vec())),
        ("current_revision", integer(revision)),
        ("existing_proposal_ids", JsonValue::Array(existing.to_vec())),
    ])
}
fn validate_packet(mut packet: JsonValue) -> Result<JsonValue, WorkspaceMachineError> {
    let required = [
        "schema",
        "version",
        "session_id",
        "revision",
        "selected_lens",
        "excluded_edge_ids",
        "route_snapshots",
        "hypotheses",
        "notes",
        "journal",
    ];
    if !exact(&packet, &required, &["proposals"])
        || str_field(&packet, "schema") != Some(SCHEMA)
        || int_field(&packet, "version", 1) != Some(1)
        || !bounded(&packet, "session_id", 256)
        || int_field(&packet, "revision", MAX_REVISION).is_none()
        || !get(&packet, "selected_lens").is_some_and(validate_selection)
        || !unique_strings(&packet, "excluded_edge_ids", 256, 256)
    {
        return Err(invalid_packet());
    }
    if get(&packet, "proposals").is_none() {
        set(&mut packet, "proposals", JsonValue::Array(Vec::new()));
    }
    let routes = collection(&packet, "route_snapshots", 256)?;
    let hypotheses = collection(&packet, "hypotheses", 256)?;
    let proposals = collection(&packet, "proposals", 256)?;
    let notes = collection(&packet, "notes", 256)?;
    let journal = collection(&packet, "journal", 512)?;
    if !routes.iter().all(validate_route)
        || !hypotheses.iter().all(validate_hypothesis)
        || !notes.iter().all(validate_note)
        || !unique_ids(routes)
        || !unique_ids(hypotheses)
        || !unique_ids(proposals)
        || !unique_ids(notes)
    {
        return Err(invalid_packet());
    }
    let ids: Vec<_> = hypotheses
        .iter()
        .map(|item| text(str_field(item, "id").unwrap()))
        .collect();
    for proposal in proposals {
        let request = proposal_request(proposal, &ids, "verify", 0, &[]);
        let bytes = emit(&request)?;
        workspace_proposal_digest_v1(&bytes).map_err(|_| invalid_packet())?;
    }
    let mut previous = 0;
    for entry in journal {
        previous = validate_journal(entry, previous).ok_or_else(invalid_packet)?;
    }
    Ok(packet)
}

#[derive(Clone)]
struct Machine {
    state: JsonValue,
    undo: Vec<JsonValue>,
    redo: Vec<JsonValue>,
    history_limit: usize,
}
impl Machine {
    fn parse(value: &JsonValue) -> Result<Self, WorkspaceMachineError> {
        if !exact(value, &["state", "undo", "redo", "history_limit"], &[]) {
            return Err(invalid_packet());
        }
        let limit = int_field(value, "history_limit", 100)
            .filter(|value| *value >= 1)
            .ok_or_else(invalid_packet)? as usize;
        let undo = collection(value, "undo", limit)?
            .iter()
            .cloned()
            .map(validate_packet)
            .collect::<Result<Vec<_>, _>>()?;
        let redo = collection(value, "redo", limit)?
            .iter()
            .cloned()
            .map(validate_packet)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            state: validate_packet(get(value, "state").unwrap().clone())?,
            undo,
            redo,
            history_limit: limit,
        })
    }
    fn dto(&self) -> JsonValue {
        object(vec![
            ("state", self.state.clone()),
            ("undo", JsonValue::Array(self.undo.clone())),
            ("redo", JsonValue::Array(self.redo.clone())),
            ("history_limit", integer(self.history_limit as u64)),
        ])
    }
    fn revision(&self) -> u64 {
        int_field(&self.state, "revision", MAX_REVISION).unwrap()
    }
    fn push(stack: &mut Vec<JsonValue>, state: JsonValue, limit: usize) {
        stack.push(state);
        if stack.len() > limit {
            stack.remove(0);
        }
    }
    fn journal(
        state: &mut JsonValue,
        action: &str,
        target: Option<&str>,
    ) -> Result<(), WorkspaceMachineError> {
        let entries = array_mut(state, "journal");
        let last = entries
            .last()
            .and_then(|item| int_field(item, "sequence", MAX_REVISION))
            .unwrap_or(0);
        if last >= MAX_REVISION {
            return Err(invalid_packet());
        }
        let mut entry = object(vec![
            ("sequence", integer(last + 1)),
            ("action", text(action)),
        ]);
        if let Some(target) = target {
            set(&mut entry, "target_id", text(target));
        }
        entries.push(entry);
        if entries.len() > 512 {
            entries.remove(0);
        }
        Ok(())
    }
    fn commit(
        &mut self,
        next: JsonValue,
        action: &str,
        target: Option<&str>,
    ) -> Result<(), WorkspaceMachineError> {
        let mut next = next;
        set(
            &mut next,
            "revision",
            integer(self.revision().saturating_add(1).min(MAX_REVISION)),
        );
        Self::journal(&mut next, action, target)?;
        let next = validate_packet(next)?;
        Self::push(&mut self.undo, self.state.clone(), self.history_limit);
        self.state = next;
        self.redo.clear();
        Ok(())
    }
    fn transition(&mut self, undo: bool) -> Result<bool, WorkspaceMachineError> {
        let revision = self.revision().saturating_add(1).min(MAX_REVISION);
        let source = if undo { &mut self.undo } else { &mut self.redo };
        let Some(mut target) = source.pop() else {
            return Ok(false);
        };
        set(&mut target, "revision", integer(revision));
        Self::journal(
            &mut target,
            if undo {
                "workspace.undo"
            } else {
                "workspace.redo"
            },
            None,
        )?;
        let target = validate_packet(target)?;
        if undo {
            Self::push(&mut self.redo, self.state.clone(), self.history_limit);
        } else {
            Self::push(&mut self.undo, self.state.clone(), self.history_limit);
        }
        self.state = target;
        Ok(true)
    }
}

fn command_id<'a>(command: &'a JsonValue, name: &str) -> Result<&'a str, WorkspaceMachineError> {
    if bounded(command, name, 256) {
        Ok(str_field(command, name).unwrap())
    } else {
        Err(invalid_command())
    }
}
fn command_apply(
    machine: &mut Machine,
    command: &JsonValue,
) -> Result<JsonValue, WorkspaceMachineError> {
    let action = str_field(command, "kind").ok_or_else(invalid_command)?;
    let argument = match action {
        "lens.select" => "selection",
        "edge.exclude" | "edge.include" => "edge_id",
        "hypothesis.add" => "hypothesis",
        "hypothesis.remove" | "route.remove" | "note.remove" => "id",
        "route.snapshot" => "route",
        "note.add" | "note.update" => "note",
        "proposal.stage" => "proposal",
        _ => return Err(invalid_command()),
    };
    if !exact(command, &["kind", argument], &[]) {
        return Err(invalid_command());
    }
    let mut next = machine.state.clone();
    let mut target: Option<String> = None;
    let mut changed = true;
    let mut result = JsonValue::Null;
    match action {
        "lens.select" => {
            let lens = get(command, "selection").ok_or_else(invalid_command)?;
            if !validate_selection(lens) {
                return Err(invalid_command());
            }
            target = str_field(lens, "id").map(str::to_owned);
            set(&mut next, "selected_lens", lens.clone());
        }
        "edge.exclude" | "edge.include" => {
            let id = command_id(command, "edge_id")?.to_owned();
            target = Some(id.clone());
            let items = array_mut(&mut next, "excluded_edge_ids");
            let index = items
                .iter()
                .position(|item| item.as_str() == Some(id.as_str()));
            if action == "edge.exclude" {
                if index.is_some() {
                    changed = false
                } else {
                    items.push(text(&id));
                }
            } else if let Some(index) = index {
                items.remove(index);
            } else {
                changed = false
            }
        }
        "hypothesis.add" => {
            let item = get(command, "hypothesis").ok_or_else(invalid_command)?;
            if !validate_hypothesis(item) {
                return Err(invalid_command());
            }
            let id = str_field(item, "id").unwrap().to_owned();
            let items = array_mut(&mut next, "hypotheses");
            if items
                .iter()
                .any(|existing| str_field(existing, "id") == Some(id.as_str()))
            {
                return Err(err(WorkspaceMachineErrorCode::Conflict));
            }
            items.push(item.clone());
            target = Some(id);
            result = item.clone();
        }
        "hypothesis.remove" => {
            let id = command_id(command, "id")?.to_owned();
            let proposals = get(&next, "proposals")
                .and_then(JsonValue::as_array)
                .unwrap();
            if proposals
                .iter()
                .any(|item| str_field(item, "parent_hypothesis_id") == Some(id.as_str()))
            {
                return Err(err(WorkspaceMachineErrorCode::Conflict));
            }
            let items = array_mut(&mut next, "hypotheses");
            let before = items.len();
            items.retain(|item| str_field(item, "id") != Some(id.as_str()));
            changed = items.len() != before;
            target = Some(id);
        }
        "route.snapshot" => {
            let item = get(command, "route").ok_or_else(invalid_command)?;
            if !validate_route(item) {
                return Err(invalid_command());
            }
            let id = str_field(item, "id").unwrap().to_owned();
            let items = array_mut(&mut next, "route_snapshots");
            if let Some(index) = items
                .iter()
                .position(|existing| str_field(existing, "id") == Some(id.as_str()))
            {
                items[index] = item.clone();
            } else {
                items.push(item.clone());
            }
            target = Some(id);
        }
        "route.remove" => {
            let id = command_id(command, "id")?.to_owned();
            let items = array_mut(&mut next, "route_snapshots");
            let before = items.len();
            items.retain(|item| str_field(item, "id") != Some(id.as_str()));
            changed = items.len() != before;
            target = Some(id);
        }
        "note.add" | "note.update" => {
            let item = get(command, "note").ok_or_else(invalid_command)?;
            if !validate_note(item) {
                return Err(invalid_command());
            }
            let id = str_field(item, "id").unwrap().to_owned();
            let items = array_mut(&mut next, "notes");
            let index = items
                .iter()
                .position(|existing| str_field(existing, "id") == Some(id.as_str()));
            match (action, index) {
                ("note.add", None) => items.push(item.clone()),
                ("note.update", Some(index)) => items[index] = item.clone(),
                _ => return Err(err(WorkspaceMachineErrorCode::Conflict)),
            }
            target = Some(id);
        }
        "note.remove" => {
            let id = command_id(command, "id")?.to_owned();
            let items = array_mut(&mut next, "notes");
            let before = items.len();
            items.retain(|item| str_field(item, "id") != Some(id.as_str()));
            changed = items.len() != before;
            target = Some(id);
        }
        "proposal.stage" => {
            let item = get(command, "proposal").ok_or_else(invalid_command)?;
            let ids: Vec<_> = get(&next, "hypotheses")
                .and_then(JsonValue::as_array)
                .unwrap()
                .iter()
                .map(|item| text(str_field(item, "id").unwrap()))
                .collect();
            let existing: Vec<_> = get(&next, "proposals")
                .and_then(JsonValue::as_array)
                .unwrap()
                .iter()
                .map(|item| text(str_field(item, "id").unwrap()))
                .collect();
            let request = proposal_request(item, &ids, "stage", machine.revision(), &existing);
            let digest =
                workspace_proposal_digest_v1(&emit(&request)?).map_err(|error| {
                    match error.code.as_str() {
                        "stale_revision" => err(WorkspaceMachineErrorCode::StaleRevision),
                        "missing_parent" | "duplicate_proposal" => {
                            err(WorkspaceMachineErrorCode::Conflict)
                        }
                        _ => invalid_command(),
                    }
                })?;
            let mut signed = item.clone();
            set(&mut signed, "digest", text(&digest));
            target = str_field(item, "id").map(str::to_owned);
            array_mut(&mut next, "proposals").push(signed.clone());
            result = signed;
        }
        _ => unreachable!(),
    }
    if changed {
        machine.commit(next, action, target.as_deref())?;
    }
    Ok(object(vec![
        ("changed", JsonValue::Bool(changed)),
        ("value", result),
    ]))
}

fn empty_packet(session_id: &str) -> JsonValue {
    object(vec![
        ("schema", text(SCHEMA)),
        ("version", integer(1)),
        ("session_id", text(session_id)),
        ("revision", integer(0)),
        ("selected_lens", JsonValue::Null),
        ("excluded_edge_ids", JsonValue::Array(Vec::new())),
        ("route_snapshots", JsonValue::Array(Vec::new())),
        ("hypotheses", JsonValue::Array(Vec::new())),
        ("proposals", JsonValue::Array(Vec::new())),
        ("notes", JsonValue::Array(Vec::new())),
        ("journal", JsonValue::Array(Vec::new())),
    ])
}
fn summary(machine: &Machine) -> JsonValue {
    let count = |key| {
        get(&machine.state, key)
            .and_then(JsonValue::as_array)
            .unwrap()
            .len() as u64
    };
    object(vec![
        ("schema", text("tos_research_workspace_summary_v1")),
        (
            "session_id",
            get(&machine.state, "session_id").unwrap().clone(),
        ),
        ("revision", integer(machine.revision())),
        ("hypothesis_count", integer(count("hypotheses"))),
        ("proposal_count", integer(count("proposals"))),
        ("excluded_edge_count", integer(count("excluded_edge_ids"))),
        ("comparison_count", integer(count("route_snapshots"))),
        ("note_count", integer(count("notes"))),
        ("journal_count", integer(count("journal"))),
        ("can_undo", JsonValue::Bool(!machine.undo.is_empty())),
        ("can_redo", JsonValue::Bool(!machine.redo.is_empty())),
    ])
}
fn comparable_routes_ready(machine: &Machine) -> bool {
    let routes = get(&machine.state, "route_snapshots")
        .and_then(JsonValue::as_array)
        .unwrap();
    let mut seen = HashSet::new();
    routes.iter().any(|route| {
        let pair = (
            str_field(route, "from_id").unwrap(),
            str_field(route, "to_id").unwrap(),
        );
        !seen.insert(pair)
    })
}

/// Execute one deterministic workspace operation. Import and all commands are
/// atomic from the host's perspective: errors return no replacement machine.
/// The host persists and notifies only after accepting a successful response.
pub fn workspace_transition_v1(raw: &[u8]) -> Result<Vec<u8>, WorkspaceMachineError> {
    let request = parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: MAX_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| err(WorkspaceMachineErrorCode::InvalidRequest))?;
    let request = request.root();
    if str_field(request, "schema") != Some(ABI) {
        return Err(err(WorkspaceMachineErrorCode::InvalidRequest));
    }
    let operation = str_field(request, "operation")
        .ok_or_else(|| err(WorkspaceMachineErrorCode::InvalidRequest))?;
    let mut machine;
    let value;
    match operation {
        "create" => {
            if !exact(
                request,
                &["schema", "operation", "session_id"],
                &["history_limit"],
            ) || !bounded(request, "session_id", 256)
            {
                return Err(err(WorkspaceMachineErrorCode::InvalidRequest));
            }
            let limit = match get(request, "history_limit") {
                None => 50,
                Some(_) => int_field(request, "history_limit", 100)
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| err(WorkspaceMachineErrorCode::InvalidRequest))?
                    as usize,
            };
            machine = Machine {
                state: empty_packet(str_field(request, "session_id").unwrap()),
                undo: Vec::new(),
                redo: Vec::new(),
                history_limit: limit,
            };
            value = JsonValue::Null;
        }
        "import" => {
            if !exact(request, &["schema", "operation", "machine", "packet"], &[]) {
                return Err(err(WorkspaceMachineErrorCode::InvalidRequest));
            }
            machine = Machine::parse(get(request, "machine").unwrap())?;
            let packet = str_field(request, "packet").ok_or_else(invalid_packet)?;
            if packet.encode_utf16().count() > MAX_PACKET_UNITS {
                return Err(invalid_packet());
            }
            let parsed = parse_json(
                packet.as_bytes(),
                JsonMode::RequestLastWins,
                JsonLimits {
                    max_bytes: MAX_BYTES,
                    ..JsonLimits::default()
                },
            )
            .map_err(|_| invalid_packet())?;
            machine.state = validate_packet(parsed.into_root())?;
            machine.undo.clear();
            machine.redo.clear();
            value = JsonValue::Bool(true);
        }
        "state"
        | "summary"
        | "comparable_routes_ready"
        | "export"
        | "apply"
        | "undo"
        | "redo"
        | "clear_history" => {
            if !exact(
                request,
                &["schema", "operation", "machine"],
                if operation == "apply" {
                    &["command"]
                } else {
                    &[]
                },
            ) {
                return Err(err(WorkspaceMachineErrorCode::InvalidRequest));
            }
            machine = Machine::parse(get(request, "machine").unwrap())?;
            value = match operation {
                "state" => machine.state.clone(),
                "summary" => summary(&machine),
                "comparable_routes_ready" => JsonValue::Bool(comparable_routes_ready(&machine)),
                "export" => {
                    text(&String::from_utf8(emit(&machine.state)?).map_err(|_| invalid_packet())?)
                }
                "apply" => command_apply(
                    &mut machine,
                    get(request, "command").ok_or_else(invalid_command)?,
                )?,
                "undo" => JsonValue::Bool(machine.transition(true)?),
                "redo" => JsonValue::Bool(machine.transition(false)?),
                "clear_history" => {
                    machine.undo.clear();
                    machine.redo.clear();
                    JsonValue::Null
                }
                _ => unreachable!(),
            };
        }
        _ => return Err(err(WorkspaceMachineErrorCode::InvalidRequest)),
    }
    emit(&object(vec![
        ("schema", text(ABI)),
        ("machine", machine.dto()),
        ("value", value),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(request: &str) -> JsonValue {
        let bytes = workspace_transition_v1(request.as_bytes()).unwrap();
        parse_json(&bytes, JsonMode::RequestLastWins, JsonLimits::default())
            .unwrap()
            .into_root()
    }
    #[test]
    fn create_matches_ts_packet_and_summary() {
        let result = run(
            r#"{"schema":"tos_research_workspace_transition_v1","operation":"create","session_id":"demo","history_limit":2}"#,
        );
        let machine = get(&result, "machine").unwrap();
        assert_eq!(
            str_field(get(machine, "state").unwrap(), "session_id"),
            Some("demo")
        );
        assert_eq!(
            int_field(get(machine, "state").unwrap(), "revision", MAX_REVISION),
            Some(0)
        );
        assert_eq!(int_field(machine, "history_limit", 100), Some(2));
    }
    #[test]
    fn invalid_import_never_returns_a_replacement_state() {
        let created = run(
            r#"{"schema":"tos_research_workspace_transition_v1","operation":"create","session_id":"demo"}"#,
        );
        let request = object(vec![
            ("schema", text(ABI)),
            ("operation", text("import")),
            ("machine", get(&created, "machine").unwrap().clone()),
            ("packet", text("{\"schema\":\"wrong\"}")),
        ]);
        assert_eq!(
            workspace_transition_v1(&emit(&request).unwrap())
                .unwrap_err()
                .code,
            WorkspaceMachineErrorCode::InvalidPacket
        );
    }

    fn next(machine: &JsonValue, operation: &str, command: Option<JsonValue>) -> JsonValue {
        let mut request = object(vec![
            ("schema", text(ABI)),
            ("operation", text(operation)),
            ("machine", machine.clone()),
        ]);
        if let Some(command) = command {
            set(&mut request, "command", command);
        }
        parse_json(
            &workspace_transition_v1(&emit(&request).unwrap()).unwrap(),
            JsonMode::RequestLastWins,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root()
    }

    #[test]
    fn edge_note_undo_redo_match_current_ts_oracle() {
        let created = run(
            r#"{"schema":"tos_research_workspace_transition_v1","operation":"create","session_id":"oracle","history_limit":2}"#,
        );
        let excluded = next(
            get(&created, "machine").unwrap(),
            "apply",
            Some(object(vec![
                ("kind", text("edge.exclude")),
                ("edge_id", text("edge:a")),
            ])),
        );
        let noted = next(
            get(&excluded, "machine").unwrap(),
            "apply",
            Some(object(vec![
                ("kind", text("note.add")),
                (
                    "note",
                    object(vec![("id", text("note:a")), ("body", text("First"))]),
                ),
            ])),
        );
        let undone = next(get(&noted, "machine").unwrap(), "undo", None);
        let redone = next(get(&undone, "machine").unwrap(), "redo", None);
        let exported = next(get(&redone, "machine").unwrap(), "export", None);
        assert_eq!(
            get(&exported, "value").and_then(JsonValue::as_str),
            Some(
                r#"{"excluded_edge_ids":["edge:a"],"hypotheses":[],"journal":[{"action":"edge.exclude","sequence":1,"target_id":"edge:a"},{"action":"note.add","sequence":2,"target_id":"note:a"},{"action":"workspace.redo","sequence":3}],"notes":[{"body":"First","id":"note:a"}],"proposals":[],"revision":4,"route_snapshots":[],"schema":"tos_research_workspace_session_v1","selected_lens":null,"session_id":"oracle","version":1}"#
            )
        );
        assert_eq!(
            get(&redone, "value").and_then(JsonValue::as_bool),
            Some(true)
        );
        let summary = next(get(&redone, "machine").unwrap(), "summary", None);
        let value = get(&summary, "value").unwrap();
        assert_eq!(int_field(value, "revision", MAX_REVISION), Some(4));
        assert_eq!(int_field(value, "journal_count", 512), Some(3));
        assert_eq!(
            get(value, "can_redo").and_then(JsonValue::as_bool),
            Some(false)
        );
    }

    #[test]
    fn no_op_edge_preserves_machine() {
        let created = run(
            r#"{"schema":"tos_research_workspace_transition_v1","operation":"create","session_id":"oracle"}"#,
        );
        let first = next(
            get(&created, "machine").unwrap(),
            "apply",
            Some(object(vec![
                ("kind", text("edge.exclude")),
                ("edge_id", text("edge:a")),
            ])),
        );
        let repeated = next(
            get(&first, "machine").unwrap(),
            "apply",
            Some(object(vec![
                ("kind", text("edge.exclude")),
                ("edge_id", text("edge:a")),
            ])),
        );
        assert_eq!(get(&first, "machine"), get(&repeated, "machine"));
        assert_eq!(
            get(get(&repeated, "value").unwrap(), "changed").and_then(JsonValue::as_bool),
            Some(false)
        );
    }

    #[test]
    fn proposal_stage_matches_ts_digest_and_rejects_stale_revision() {
        let created = run(
            r#"{"schema":"tos_research_workspace_transition_v1","operation":"create","session_id":"proposal"}"#,
        );
        let hypothesis = object(vec![
            ("id", text("hyp:one")),
            ("title", text("Working")),
            ("body", text("A reading.")),
            (
                "posture",
                object(vec![
                    ("session_hypothesis", JsonValue::Bool(true)),
                    ("source", JsonValue::Bool(false)),
                    ("reviewed", JsonValue::Bool(false)),
                    ("canon", JsonValue::Bool(false)),
                ]),
            ),
        ]);
        let added = next(
            get(&created, "machine").unwrap(),
            "apply",
            Some(object(vec![
                ("kind", text("hypothesis.add")),
                ("hypothesis", hypothesis),
            ])),
        );
        let proposal = parse_json(
            br#"{"id":"proposal:one","kind":"interpretation","parent_hypothesis_id":"hyp:one","target_id":"edge:one","statement":"Another reading.","source_refs":["source:one"],"evidence_refs":["evidence:one"],"confidence_posture":{"value":"low","meaning":"maker_declared_uncertainty_not_truth_probability"},"actor_origin":"agent","base_page_revision":7,"base_workspace_revision":1,"data_fingerprint":"sha256:fixture","created_at":"2026-09-23T12:00:00.000Z","local_only":true,"review_status":"pending_review","review_requirement":"human_or_authorized_agent","canon":false}"#,
            JsonMode::RequestLastWins, JsonLimits::default(),
        ).unwrap().into_root();
        let staged = next(
            get(&added, "machine").unwrap(),
            "apply",
            Some(object(vec![
                ("kind", text("proposal.stage")),
                ("proposal", proposal.clone()),
            ])),
        );
        assert_eq!(
            str_field(
                get(get(&staged, "value").unwrap(), "value").unwrap(),
                "digest"
            ),
            Some("fnv1a64:602af812971bbce6")
        );
        assert_eq!(
            int_field(
                get(get(&staged, "machine").unwrap(), "state").unwrap(),
                "revision",
                MAX_REVISION
            ),
            Some(2)
        );
        let stale = object(vec![
            ("schema", text(ABI)),
            ("operation", text("apply")),
            ("machine", get(&staged, "machine").unwrap().clone()),
            (
                "command",
                object(vec![
                    ("kind", text("proposal.stage")),
                    ("proposal", proposal),
                ]),
            ),
        ]);
        assert_eq!(
            workspace_transition_v1(&emit(&stale).unwrap())
                .unwrap_err()
                .code,
            WorkspaceMachineErrorCode::StaleRevision
        );
    }
}
