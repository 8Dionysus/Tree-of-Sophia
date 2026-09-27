//! Bounded published inspection transport over the shared non-replay plan.
//! Publication selection and authenticated physical reads remain in the host.
use tos_foundation::{
    FoundationErrorCode, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    emit_python_compact_json, parse_json,
};
use tos_query::search_v2::SearchKind;
use tos_query::search_v2::{SearchV2Error, SearchV2ErrorCode};
use tos_query::{InspectBudget, InspectNeed, InspectPlan, InspectRequest};

fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}

pub fn request(raw: &[u8], budget: InspectBudget) -> Result<InspectRequest, SearchV2Error> {
    let document = parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 65536,
            ..budget.json
        },
    )
    .map_err(|_| {
        error(
            SearchV2ErrorCode::InvalidRequest,
            "inspection request JSON invalid",
        )
    })?;
    InspectRequest::from_json(document.root(), budget)
}

pub struct InspectionSession {
    plan: Option<InspectPlan>,
    budget: InspectBudget,
}

impl InspectionSession {
    pub fn new(
        raw: &[u8],
        revision: String,
        top: &[u8],
        budget: InspectBudget,
    ) -> Result<Self, SearchV2Error> {
        let request = request(raw, budget)?;
        let document = parse_json(
            top,
            JsonMode::PublishedStrict,
            JsonLimits {
                max_bytes: 65536,
                ..budget.json
            },
        )
        .map_err(|_| {
            error(
                SearchV2ErrorCode::CorruptSelectedCarrier,
                "inspection publication header invalid",
            )
        })?;
        if document
            .root()
            .object_get("source_revision")
            .and_then(JsonValue::as_str)
            != Some(revision.as_str())
        {
            return Err(error(
                SearchV2ErrorCode::CorruptSelectedCarrier,
                "inspection publication revision differs",
            ));
        }
        let boundary = document
            .root()
            .object_get("authority_boundary")
            .cloned()
            .ok_or_else(|| {
                error(
                    SearchV2ErrorCode::CorruptSelectedCarrier,
                    "inspection publication boundary missing",
                )
            })?;
        Ok(Self {
            plan: Some(InspectPlan::new(request, revision, boundary, budget)?),
            budget,
        })
    }

    pub fn need(&self) -> Result<Option<&InspectNeed>, SearchV2Error> {
        self.plan.as_ref().map(InspectPlan::need).ok_or_else(|| {
            error(
                SearchV2ErrorCode::NonMonotoneProgress,
                "inspection session complete",
            )
        })
    }

    pub fn need_bytes(&self) -> Result<Option<Vec<u8>>, SearchV2Error> {
        let Some(need) = self.need()? else {
            return Ok(None);
        };
        let text = |value: &str| JsonValue::String(JsonString::from_utf8(value));
        let number = |value: usize| {
            JsonValue::Number(JsonNumber {
                kind: JsonNumberKind::Int,
                lexeme: value.to_string(),
            })
        };
        let ids =
            |values: &[String]| JsonValue::Array(values.iter().map(|value| text(value)).collect());
        let fields = match need {
            InspectNeed::Lookup {
                kind,
                selector,
                identifier,
                limit,
            } => vec![
                ("operation", text("lookup")),
                (
                    "kind",
                    text(if *kind == SearchKind::Nodes {
                        "node"
                    } else {
                        "relation"
                    }),
                ),
                ("selector", text(selector)),
                ("identifier", text(identifier)),
                ("limit", number(*limit)),
            ],
            InspectNeed::NodeIncident {
                ids: values,
                relation_limit,
            } => vec![
                ("operation", text("incident")),
                ("ids", ids(values)),
                ("relation_limit", number(*relation_limit)),
            ],
            InspectNeed::RelationEndpoints { ids: values } => {
                vec![("operation", text("endpoints")), ("ids", ids(values))]
            }
        };
        let value = JsonValue::Object(
            fields
                .into_iter()
                .map(|(key, value)| (JsonString::from_utf8(key), value))
                .collect(),
        );
        emit_python_compact_json(&value, self.budget.json)
            .map(Some)
            .map_err(|_| {
                error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "inspection physical need emission refused",
                )
            })
    }

    fn rows(&self, raw: &[u8]) -> Result<Vec<JsonValue>, SearchV2Error> {
        let document =
            parse_json(raw, JsonMode::PublishedStrict, self.budget.json).map_err(|reason| {
                error(
                    if reason.code == FoundationErrorCode::BudgetExceeded {
                        SearchV2ErrorCode::BudgetExceeded
                    } else {
                        SearchV2ErrorCode::CorruptSelectedCarrier
                    },
                    "inspection carrier batch invalid",
                )
            })?;
        match document.root() {
            JsonValue::Array(rows) => Ok(rows.clone()),
            _ => Err(error(
                SearchV2ErrorCode::CorruptSelectedCarrier,
                "inspection carrier batch is not an array",
            )),
        }
    }

    pub fn resume_lookup(&mut self, raw: &[u8]) -> Result<(), SearchV2Error> {
        let rows = self.rows(raw)?;
        self.plan
            .as_mut()
            .ok_or_else(|| {
                error(
                    SearchV2ErrorCode::NonMonotoneProgress,
                    "inspection session complete",
                )
            })?
            .resume_lookup(rows, raw.len() as u64, None)
    }
    pub fn resume_incident(&mut self, total: u64, raw: &[u8]) -> Result<(), SearchV2Error> {
        let rows = self.rows(raw)?;
        self.plan
            .as_mut()
            .ok_or_else(|| {
                error(
                    SearchV2ErrorCode::NonMonotoneProgress,
                    "inspection session complete",
                )
            })?
            .resume_incident(total, rows, raw.len() as u64, None)
    }
    pub fn resume_endpoints(&mut self, raw: &[u8]) -> Result<(), SearchV2Error> {
        let rows = self.rows(raw)?;
        self.plan
            .as_mut()
            .ok_or_else(|| {
                error(
                    SearchV2ErrorCode::NonMonotoneProgress,
                    "inspection session complete",
                )
            })?
            .resume_endpoints(rows, raw.len() as u64, None)
    }
    pub fn finish(&mut self) -> Result<Vec<u8>, SearchV2Error> {
        let packet = self
            .plan
            .take()
            .ok_or_else(|| {
                error(
                    SearchV2ErrorCode::NonMonotoneProgress,
                    "inspection session complete",
                )
            })?
            .into_packet()?;
        emit_python_compact_json(
            &packet,
            JsonLimits {
                max_bytes: self.budget.max_response_bytes as usize,
                ..self.budget.json
            },
        )
        .map_err(|reason| {
            error(
                if reason.code == FoundationErrorCode::BudgetExceeded {
                    SearchV2ErrorCode::BudgetExceeded
                } else {
                    SearchV2ErrorCode::CorruptSelectedCarrier
                },
                "inspection packet emission refused",
            )
        })
    }
}
