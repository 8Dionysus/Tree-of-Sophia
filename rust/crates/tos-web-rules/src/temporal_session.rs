//! Private replay continuation over the shared temporal core. No selection,
//! carrier integrity, visibility, or disclosure authority is established here.
use std::collections::BTreeMap;
use tos_foundation::{JsonLimits, JsonMode, JsonValue, emit_value_preserved_json, parse_json};
use tos_query::{
    compare_temporal_operands,
    search_v2::{SearchV2Error, SearchV2ErrorCode},
};

#[derive(Clone, Copy, Debug)]
pub struct TemporalSessionBudget {
    pub json: JsonLimits,
    pub max_source_bytes: usize,
    pub max_replay_bytes: usize,
    pub max_output_bytes: usize,
}

#[derive(Debug)]
pub enum TemporalSessionStep {
    Need(String),
    Complete(Vec<u8>),
}

#[derive(Clone, Copy, Debug)]
pub struct TemporalSessionWork {
    pub replay_executions: usize,
    pub retained_source_bytes: usize,
    pub replayed_input_bytes: usize,
    pub exact_lookups: usize,
}

/// A request-local session must only receive exact, verified carriers from one
/// selected model and current policy scope. Host changes to either invalidate
/// and dispose the session; a completed packet still requires a held lease.
pub struct TemporalSession {
    revision: String,
    profile: String,
    request: JsonValue,
    request_bytes: usize,
    rows: BTreeMap<String, Vec<JsonValue>>,
    pending: Option<String>,
    budget: TemporalSessionBudget,
    source_bytes: usize,
    replay_bytes: usize,
    executions: usize,
    terminal: bool,
}

fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn charge(used: &mut usize, amount: usize, maximum: usize) -> Result<(), SearchV2Error> {
    *used = used
        .checked_add(amount)
        .filter(|v| *v <= maximum)
        .ok_or_else(|| {
            error(
                SearchV2ErrorCode::BudgetExceeded,
                "temporal replay admission exhausted",
            )
        })?;
    Ok(())
}

impl TemporalSession {
    pub fn work(&self) -> TemporalSessionWork {
        TemporalSessionWork {
            replay_executions: self.executions,
            retained_source_bytes: self.source_bytes,
            replayed_input_bytes: self.replay_bytes,
            exact_lookups: self.rows.len(),
        }
    }
    pub fn new(
        revision: String,
        profile: String,
        raw: &[u8],
        budget: TemporalSessionBudget,
    ) -> Result<Self, SearchV2Error> {
        let request = parse_json(raw, JsonMode::RequestLastWins, budget.json)
            .map_err(|_| {
                error(
                    SearchV2ErrorCode::InvalidRequest,
                    "temporal request JSON invalid",
                )
            })?
            .root()
            .clone();
        if profile.is_empty()
            || [
                budget.max_source_bytes,
                budget.max_replay_bytes,
                budget.max_output_bytes,
            ]
            .contains(&0)
        {
            return Err(error(
                SearchV2ErrorCode::Unavailable,
                "temporal replay admission unavailable",
            ));
        }
        Ok(Self {
            revision,
            profile,
            request,
            request_bytes: raw.len(),
            rows: BTreeMap::new(),
            pending: None,
            budget,
            source_bytes: 0,
            replay_bytes: 0,
            executions: 0,
            terminal: false,
        })
    }

    pub fn advance(&mut self) -> Result<TemporalSessionStep, SearchV2Error> {
        if self.terminal || self.pending.is_some() {
            return Err(error(
                SearchV2ErrorCode::NonMonotoneProgress,
                "temporal continuation is not advanceable",
            ));
        }
        let result = self.execute();
        if result.is_err() {
            self.terminal = true;
        }
        result
    }

    fn execute(&mut self) -> Result<TemporalSessionStep, SearchV2Error> {
        // Two operands can each read Claim, temporal value and documentary
        // subject. Seven executions cover six distinct needs and finalization.
        self.executions += 1;
        if self.executions > 7 {
            return Err(error(
                SearchV2ErrorCode::BudgetExceeded,
                "temporal replay execution cap",
            ));
        }
        charge(
            &mut self.replay_bytes,
            self.request_bytes
                .checked_add(self.source_bytes)
                .ok_or_else(|| {
                    error(
                        SearchV2ErrorCode::BudgetExceeded,
                        "temporal replay byte overflow",
                    )
                })?,
            self.budget.max_replay_bytes,
        )?;
        // JsonLimits bound each parser/canonical operation, while the replay
        // count and retained input cap bound repetition. Aggregate canonical
        // bytes and CPU instructions are not measured by this core API.
        let mut need = None;
        let result = compare_temporal_operands(
            &self.revision,
            &self.request,
            &self.profile,
            |id| {
                if let Some(rows) = self.rows.get(id) {
                    return Ok(rows.clone());
                }
                need = Some(id.to_owned());
                Err(error(
                    SearchV2ErrorCode::Unavailable,
                    "temporal exact carrier needed",
                ))
            },
            self.budget.json,
        );
        if let Some(id) = need {
            if self.rows.len() >= 6 {
                return Err(error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "temporal exact lookup cap",
                ));
            }
            self.pending = Some(id.clone());
            return Ok(TemporalSessionStep::Need(id));
        }
        let packet = result?;
        let bytes = emit_value_preserved_json(
            &packet,
            JsonLimits {
                max_bytes: self.budget.max_output_bytes,
                ..self.budget.json
            },
        )
        .map_err(|_| {
            error(
                SearchV2ErrorCode::BudgetExceeded,
                "temporal packet output cap",
            )
        })?;
        self.terminal = true;
        Ok(TemporalSessionStep::Complete(bytes))
    }

    /// None attests exact absence; Some is the full retained published carrier,
    /// never an object reserialized by JS. Only the outstanding need can resume.
    pub fn provide(&mut self, id: &str, raw: Option<&[u8]>) -> Result<(), SearchV2Error> {
        let result = self.accept(id, raw);
        if result.is_err() {
            self.terminal = true;
            self.pending = None;
        }
        result
    }

    fn accept(&mut self, id: &str, raw: Option<&[u8]>) -> Result<(), SearchV2Error> {
        if self.terminal || self.pending.as_deref() != Some(id) || self.rows.contains_key(id) {
            return Err(error(
                SearchV2ErrorCode::NonMonotoneProgress,
                "temporal carrier response differs from outstanding need",
            ));
        }
        let rows = match raw {
            None => vec![],
            Some(raw) => {
                charge(
                    &mut self.source_bytes,
                    raw.len(),
                    self.budget.max_source_bytes,
                )?;
                let value = parse_json(raw, JsonMode::PublishedStrict, self.budget.json)
                    .map_err(|_| {
                        error(
                            SearchV2ErrorCode::CorruptSelectedCarrier,
                            "temporal carrier JSON invalid",
                        )
                    })?
                    .root()
                    .clone();
                if value.object_get("id").and_then(JsonValue::as_str) != Some(id) {
                    return Err(error(
                        SearchV2ErrorCode::CorruptSelectedCarrier,
                        "temporal exact carrier identity differs",
                    ));
                }
                vec![value]
            }
        };
        self.rows.insert(id.to_owned(), rows);
        self.pending = None;
        Ok(())
    }
}
