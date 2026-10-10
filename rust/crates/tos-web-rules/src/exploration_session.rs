//! Exact published exploration transport. Domain rules and the one suspended
//! algorithm live in tos-query; checkpoints and D1 custody stay in the host.
use std::rc::Rc;
use tos_foundation::{
    FoundationErrorCode, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    emit_python_compact_json, parse_json,
};
use tos_query::exploration_plan::{
    ExplorationInput, ExplorationNeed, ExplorationOutput, ExplorationPlan, ExplorationReply,
    PublishedExplorationBudget,
};
use tos_query::knowledge_exploration::{
    decode_published_exploration_state, published_exploration_snapshot, set_exploration_cursor,
};
use tos_query::search_v2::{SearchKind, SearchV2Error, SearchV2ErrorCode};
use tos_query::{AbortProbe, AbortReason};

fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn corrupt(message: &'static str) -> SearchV2Error {
    error(SearchV2ErrorCode::CorruptSelectedCarrier, message)
}
fn writer_error(reason: tos_foundation::FoundationError, message: &'static str) -> SearchV2Error {
    error(
        if reason.code == FoundationErrorCode::BudgetExceeded {
            SearchV2ErrorCode::BudgetExceeded
        } else {
            SearchV2ErrorCode::CorruptSelectedCarrier
        },
        message,
    )
}
fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn strings(values: &[String]) -> JsonValue {
    JsonValue::Array(values.iter().map(|value| text(value)).collect())
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}
struct CooperativeHostAbort;
impl AbortProbe for CooperativeHostAbort {
    fn reason(&self) -> Option<AbortReason> {
        None
    }
}

fn parse(raw: &[u8], mode: JsonMode, limits: JsonLimits) -> Result<JsonValue, SearchV2Error> {
    parse_json(raw, mode, limits)
        .map(|document| document.into_root())
        .map_err(|_| corrupt("exploration transport JSON invalid"))
}
fn ids(raw: &[u8], budget: PublishedExplorationBudget) -> Result<Vec<String>, SearchV2Error> {
    let value = parse(raw, JsonMode::PublishedStrict, metadata_limits(budget)?)?;
    let JsonValue::Array(items) = value else {
        return Err(corrupt("exploration ID transport not array"));
    };
    items
        .into_iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| corrupt("exploration ID transport invalid"))
        })
        .collect()
}
fn metadata_limits(budget: PublishedExplorationBudget) -> Result<JsonLimits, SearchV2Error> {
    // Only derived exact strings/IDs enter this bridge. JSON escaping can
    // expand a physically admitted source string by at most six bytes/unit.
    let max_bytes = usize::try_from(budget.exploration.read.max_decoded_bytes)
        .ok()
        .and_then(|n| n.checked_mul(6))
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(|| corrupt("exploration metadata limit overflow"))?;
    Ok(JsonLimits {
        max_bytes,
        ..budget.exploration.read.json
    })
}
fn rows(
    raw: &[u8],
    sizes: &[u32],
    budget: PublishedExplorationBudget,
) -> Result<Vec<JsonValue>, SearchV2Error> {
    let read = budget.exploration.read;
    if sizes.len() as u64 > read.max_rows
        || sizes
            .iter()
            .any(|n| *n == 0 || *n as usize > read.max_payload_bytes)
    {
        return Err(corrupt("exploration original row sizes invalid"));
    }
    let total = sizes
        .iter()
        .try_fold(0usize, |sum, n| sum.checked_add(*n as usize))
        .ok_or_else(|| corrupt("exploration row size overflow"))?;
    if total as u64 > read.max_decoded_bytes {
        return Err(error(
            SearchV2ErrorCode::BudgetExceeded,
            "exploration row source bytes exceeded",
        ));
    }
    let framed = total
        .checked_add(2 + sizes.len().saturating_sub(1))
        .ok_or_else(|| corrupt("exploration row framing overflow"))?;
    if raw.len() != framed {
        return Err(corrupt("exploration row sizes differ from raw array"));
    }
    let limits = JsonLimits {
        max_bytes: framed,
        max_depth: read
            .json
            .max_depth
            .checked_add(1)
            .ok_or_else(|| corrupt("exploration row depth overflow"))?,
        max_visits: read
            .json
            .max_visits
            .checked_mul(sizes.len())
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| corrupt("exploration row visits overflow"))?,
        ..read.json
    };
    let value = parse(raw, JsonMode::PublishedStrict, limits)?;
    let JsonValue::Array(items) = value else {
        return Err(corrupt("exploration raw rows not array"));
    };
    if items.len() != sizes.len() {
        return Err(corrupt("exploration raw row count differs"));
    }
    Ok(items)
}

pub struct ExplorationSession {
    plan: Option<ExplorationPlan>,
    output: Option<ExplorationOutput>,
    budget: PublishedExplorationBudget,
}
impl ExplorationSession {
    pub fn new(
        request: &[u8],
        revision: &str,
        data_revision: &str,
        epoch: u64,
        top: &[u8],
        state: &[u8],
        budget: PublishedExplorationBudget,
    ) -> Result<Self, SearchV2Error> {
        let request = parse_json(
            request,
            JsonMode::RequestLastWins,
            JsonLimits {
                max_bytes: 65536,
                ..budget.exploration.read.json
            },
        )
        .map_err(|_| {
            error(
                SearchV2ErrorCode::InvalidRequest,
                "exploration request JSON invalid",
            )
        })?
        .into_root();
        let cursor =
            tos_query::knowledge_exploration::validate_published_exploration_request(&request)?;
        if cursor.is_some() == state.is_empty() {
            return Err(corrupt(
                "exploration checkpoint presence differs from request",
            ));
        }
        let input = if state.is_empty() {
            ExplorationInput::Start(request)
        } else {
            let snapshot =
                published_exploration_snapshot(data_revision, epoch, budget.exploration.read.json)?;
            ExplorationInput::Continue(decode_published_exploration_state(
                state,
                &snapshot,
                budget.exploration,
            )?)
        };
        let top = parse(
            top,
            JsonMode::PublishedStrict,
            JsonLimits {
                max_bytes: 65536,
                ..budget.exploration.read.json
            },
        )?;
        let boundary = top
            .object_get("authority_boundary")
            .filter(|value| value.as_object().is_some())
            .cloned()
            .ok_or_else(|| corrupt("exploration publication boundary missing"))?;
        let plan = ExplorationPlan::published(
            input,
            revision,
            data_revision,
            epoch,
            boundary,
            budget,
            Rc::new(CooperativeHostAbort),
        )?;
        Ok(Self {
            plan: Some(plan),
            output: None,
            budget,
        })
    }
    pub fn need_bytes(&mut self) -> Result<Option<Vec<u8>>, SearchV2Error> {
        if self.output.is_some() {
            return Ok(None);
        }
        let plan = self
            .plan
            .as_mut()
            .ok_or_else(|| corrupt("exploration session consumed"))?;
        if plan.advance()? {
            self.output = Some(self.plan.take().unwrap().finish()?);
            return Ok(None);
        }
        let need = plan
            .need()
            .ok_or_else(|| corrupt("exploration suspension lacks read"))?;
        let kind = |kind: SearchKind| {
            text(if kind == SearchKind::Nodes {
                "node"
            } else {
                "relation"
            })
        };
        let value = match &*need {
            ExplorationNeed::Rows {
                kind: row_kind,
                ids,
                allow_missing,
                allow_ambiguous,
            } => object(vec![
                ("operation", text("rows")),
                ("kind", kind(*row_kind)),
                ("ids", strings(ids)),
                ("allow_missing", JsonValue::Bool(*allow_missing)),
                ("allow_ambiguous", JsonValue::Bool(*allow_ambiguous)),
            ]),
            ExplorationNeed::Focus {
                field,
                identifier,
                sources,
                source_priority,
                limit,
            } => object(vec![
                ("operation", text("focus")),
                ("field", text(field)),
                ("id", text(identifier)),
                ("sources", strings(sources)),
                (
                    "source_priority",
                    JsonValue::Array(
                        source_priority
                            .iter()
                            .map(|(source, priority)| {
                                JsonValue::Array(vec![text(source), number(*priority)])
                            })
                            .collect(),
                    ),
                ),
                ("limit", number(*limit as u64)),
            ]),
            ExplorationNeed::IdentityPage {
                node_id,
                entity_id,
                declared_prefix,
                expanded_entities,
                after,
                sources,
                limit,
            } => object(vec![
                ("operation", text("identity")),
                ("node_id", text(node_id)),
                (
                    "entity_id",
                    entity_id.as_deref().map_or(JsonValue::Null, text),
                ),
                (
                    "declared_prefix",
                    declared_prefix.map_or(JsonValue::Null, text),
                ),
                ("expanded_entities", strings(expanded_entities)),
                ("after", text(after)),
                ("sources", strings(sources)),
                ("limit", number(*limit as u64)),
            ]),
            ExplorationNeed::AdjacencyPage {
                node_id,
                after,
                limit,
            } => object(vec![
                ("operation", text("adjacency")),
                ("node_id", text(node_id)),
                ("after", text(after)),
                ("limit", number(*limit as u64)),
            ]),
        };
        emit_python_compact_json(&value, metadata_limits(self.budget)?)
            .map(Some)
            .map_err(|reason| writer_error(reason, "exploration read metadata emission failed"))
    }
    pub fn resume_rows(
        &mut self,
        raw: &[u8],
        sizes: &[u32],
        ambiguous: &[u8],
    ) -> Result<(), SearchV2Error> {
        let need = self
            .plan
            .as_ref()
            .and_then(ExplorationPlan::need)
            .ok_or_else(|| corrupt("exploration row reply lacks need"))?;
        let ExplorationNeed::Rows { ids: requested, .. } = &*need else {
            return Err(corrupt("exploration row reply has wrong phase"));
        };
        if sizes.len() > requested.len() {
            return Err(corrupt("exploration row reply count exceeds need"));
        }
        let reply = ExplorationReply::Rows {
            rows: rows(raw, sizes, self.budget)?,
            raw_bytes: sizes.iter().map(|n| *n as usize).collect(),
            ambiguous_ids: ids(ambiguous, self.budget)?,
        };
        self.plan
            .as_mut()
            .ok_or_else(|| corrupt("exploration session consumed"))?
            .resume(reply)
    }
    pub fn resume_focus(
        &mut self,
        matched: usize,
        raw: &[u8],
        sizes: &[u32],
    ) -> Result<(), SearchV2Error> {
        let need = self
            .plan
            .as_ref()
            .and_then(ExplorationPlan::need)
            .ok_or_else(|| corrupt("exploration focus reply lacks need"))?;
        let ExplorationNeed::Focus { limit, .. } = &*need else {
            return Err(corrupt("exploration focus reply has wrong phase"));
        };
        if matched > *limit || sizes.len() > matched {
            return Err(corrupt("exploration focus reply count exceeds need"));
        }
        let reply = ExplorationReply::Focus {
            matched,
            rows: rows(raw, sizes, self.budget)?,
            raw_bytes: sizes.iter().map(|n| *n as usize).collect(),
        };
        self.plan
            .as_mut()
            .ok_or_else(|| corrupt("exploration session consumed"))?
            .resume(reply)
    }
    pub fn resume_ids(&mut self, raw: &[u8]) -> Result<(), SearchV2Error> {
        self.plan
            .as_mut()
            .ok_or_else(|| corrupt("exploration session consumed"))?
            .resume(ExplorationReply::Ids(ids(raw, self.budget)?))
    }
    pub fn paused(&self) -> Result<bool, SearchV2Error> {
        let output = self
            .output
            .as_ref()
            .ok_or_else(|| corrupt("exploration page incomplete"))?;
        Ok(output
            .packet
            .object_get("status")
            .and_then(JsonValue::as_str)
            == Some("paused"))
    }
    pub fn state(&self) -> Result<Vec<u8>, SearchV2Error> {
        self.output
            .as_ref()
            .ok_or_else(|| corrupt("exploration page incomplete"))?
            .state
            .encoded_state(JsonLimits {
                max_bytes: self.budget.exploration.max_state_bytes,
                ..self.budget.exploration.read.json
            })
    }
    pub fn finish(&mut self, cursor: Option<&str>) -> Result<Vec<u8>, SearchV2Error> {
        let mut output = self
            .output
            .take()
            .ok_or_else(|| corrupt("exploration page incomplete or consumed"))?;
        set_exploration_cursor(&mut output.packet, cursor)?;
        emit_python_compact_json(
            &output.packet,
            JsonLimits {
                max_bytes: self.budget.exploration.read.max_response_bytes,
                ..self.budget.exploration.read.json
            },
        )
        .map_err(|reason| writer_error(reason, "exploration response emission failed"))
    }
}
