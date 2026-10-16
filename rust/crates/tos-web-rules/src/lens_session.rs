//! Concrete published lens/focus/stored entry parsing. The shared query core
//! owns shape normalization; actual verified metadata binds properties later.
use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json};
use tos_query::{
    knowledge_focus::normalize_published_focus_request,
    knowledge_lens_spec::{normalize_published_lens_request, validate_stored_lens_identifier},
    search_v2::{SearchV2Error, SearchV2ErrorCode},
};

fn request_document(raw: &[u8], json: JsonLimits) -> Result<JsonValue, SearchV2Error> {
    parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 65536,
            ..json
        },
    )
    .map(|document| document.into_root())
    .map_err(|_| SearchV2Error {
        code: SearchV2ErrorCode::InvalidRequest,
        message: "published lens request JSON invalid",
    })
}

pub fn request(raw: &[u8], operation: &str, json: JsonLimits) -> Result<JsonValue, SearchV2Error> {
    let value = request_document(raw, json)?;
    match operation {
        "compile" => normalize_published_lens_request(&value),
        "focus" => normalize_published_focus_request(&value),
        "stored" => {
            validate_stored_lens_identifier(&value)?;
            Ok(value)
        }
        _ => Err(SearchV2Error {
            code: SearchV2ErrorCode::InvalidRequest,
            message: "invalid published lens entry",
        }),
    }
}

use tos_foundation::{
    FoundationErrorCode, JsonNumber, JsonNumberKind, JsonString, emit_python_compact_json,
};
use tos_query::{
    knowledge_lens_spec::stored_lens_spec,
    lens_plan::{
        LensCandidate, LensCandidateCursor, LensCandidateIndex, LensCandidatePage, LensEligibility,
        LensEndpoint, LensHeader, LensHeaderQuery, LensNeed, LensPlan, LensReply,
        LensRepresentation, LensStores, PublishedLensBudget,
    },
    search_v2::SearchKind,
};
fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn number(value: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}
fn strings(values: &[String]) -> JsonValue {
    JsonValue::Array(values.iter().map(|value| text(value)).collect())
}
fn kind(value: SearchKind) -> JsonValue {
    text(if value == SearchKind::Nodes {
        "node"
    } else {
        "relation"
    })
}
fn corrupt(message: &'static str) -> SearchV2Error {
    error(SearchV2ErrorCode::CorruptSelectedCarrier, message)
}
fn parse(raw: &[u8], max_bytes: usize, json: JsonLimits) -> Result<JsonValue, SearchV2Error> {
    parse_json(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits { max_bytes, ..json },
    )
    .map(|document| document.into_root())
    .map_err(|_| corrupt("published lens transport JSON invalid"))
}
fn string(value: &JsonValue) -> Result<String, SearchV2Error> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| corrupt("published lens transport string invalid"))
}
fn field<'a>(value: &'a JsonValue, key: &str) -> Result<&'a JsonValue, SearchV2Error> {
    value
        .object_get(key)
        .ok_or_else(|| corrupt("published lens transport field missing"))
}
fn string_list(value: &JsonValue) -> Result<Vec<String>, SearchV2Error> {
    value
        .as_array()
        .ok_or_else(|| corrupt("published lens transport list invalid"))?
        .iter()
        .map(string)
        .collect()
}
fn header_query(query: &LensHeaderQuery) -> JsonValue {
    let mut fields = vec![
        ("kind", kind(query.kind)),
        ("sources", strings(&query.sources)),
        ("predicate_ids", strings(&query.predicate_ids)),
        ("excluded_predicates", strings(&query.excluded_predicates)),
        (
            "excluded_relation_types",
            strings(&query.excluded_relation_types),
        ),
    ];
    if let Some(cells) = &query.dimensions {
        fields.push((
            "dimensions",
            JsonValue::Array(cells.iter().map(|cell| strings(cell)).collect()),
        ));
    }
    if let Some(membership) = &query.membership {
        fields.push((
            "membership",
            object(vec![
                ("all", JsonValue::Bool(membership.all)),
                (
                    "terms",
                    JsonValue::Array(
                        membership
                            .terms
                            .iter()
                            .map(|term| {
                                object(vec![
                                    ("field", text(&term.field)),
                                    ("all", JsonValue::Bool(term.all)),
                                    ("values", strings(&term.values)),
                                ])
                            })
                            .collect(),
                    ),
                ),
                (
                    "drivers",
                    JsonValue::Array(
                        membership
                            .drivers
                            .iter()
                            .map(|(field, value)| JsonValue::Array(vec![text(field), text(value)]))
                            .collect(),
                    ),
                ),
            ]),
        ));
    }
    if let Some(endpoint) = &query.endpoint {
        let (side, id) = match endpoint {
            LensEndpoint::From(id) => ("from", id),
            LensEndpoint::To(id) => ("to", id),
        };
        fields.push((
            "endpoint",
            object(vec![("side", text(side)), ("id", text(id))]),
        ));
    }
    if let Some(eligible) = &query.eligible {
        let value = match eligible {
            LensEligibility::Both {
                basis,
                traversed,
                pair_index,
            } => object(vec![
                ("policy", text("both")),
                ("basis", strings(basis)),
                ("traversed", strings(traversed)),
                ("pair_index", JsonValue::Bool(*pair_index)),
            ]),
            LensEligibility::Either { basis, traversed } => object(vec![
                ("policy", text("either")),
                ("basis", strings(basis)),
                ("traversed", strings(traversed)),
            ]),
        };
        fields.push(("eligible", value));
    }
    object(fields)
}

pub struct LensSession {
    // Published plans own their normalized spec, metadata and authority.
    plan: LensPlan<'static>,
    budget: PublishedLensBudget,
}
impl LensSession {
    pub fn new(
        raw: &[u8],
        operation: &str,
        revision: &str,
        top: &[u8],
        metadata: &[u8],
        catalog: &[u8],
        publication: &[u8],
        budget: PublishedLensBudget,
    ) -> Result<Self, SearchV2Error> {
        // Shape validation already ran before D1. Compile passes its owned raw
        // request directly to the metadata-bound shared normalizer; focus
        // requires the shared typed focus-to-spec constructor.
        let entry = if operation == "compile" {
            request_document(raw, budget.lens.inspect.json)?
        } else {
            request(raw, operation, budget.lens.inspect.json)?
        };
        let spec = if operation == "stored" {
            let identifier = validate_stored_lens_identifier(&entry)?;
            let catalog = parse(catalog, 8 * 1024 * 1024, budget.lens.inspect.json)?;
            stored_lens_spec(&catalog, &identifier)?
        } else {
            entry
        };
        let top = parse(top, 65536, budget.lens.inspect.json)?;
        if top
            .object_get("source_revision")
            .and_then(JsonValue::as_str)
            != Some(revision)
        {
            return Err(corrupt("published lens revision differs"));
        }
        let metadata = parse(metadata, 1048576, budget.lens.inspect.json)?;
        let publication = parse(publication, 65536, budget.lens.inspect.json)?;
        Ok(Self {
            plan: LensPlan::published(
                &spec,
                &metadata,
                &top,
                revision,
                Some(&publication),
                budget,
            )?,
            budget,
        })
    }
    fn transport_limits(&self) -> Result<JsonLimits, SearchV2Error> {
        // JSON escaping of already admitted D1 text can use six bytes for one
        // control code point. This bounds transport encoding, not source bytes.
        let max_bytes = usize::try_from(self.budget.lens.inspect.max_decoded_bytes)
            .ok()
            .and_then(|n| n.checked_mul(6))
            .ok_or_else(|| {
                error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "lens transport byte limit overflow",
                )
            })?;
        Ok(JsonLimits {
            max_bytes,
            ..self.budget.lens.inspect.json
        })
    }
    pub fn need_bytes(&mut self) -> Result<Option<Vec<u8>>, SearchV2Error> {
        if self.plan.advance()? {
            return Ok(None);
        }
        let need = self
            .plan
            .need()
            .ok_or_else(|| corrupt("lens continuation need missing"))?;
        let value = match need.as_ref() {
            LensNeed::Auxiliary {
                compact,
                membership,
            } => object(vec![
                ("operation", text("auxiliary")),
                ("compact", JsonValue::Bool(*compact)),
                ("membership", JsonValue::Bool(*membership)),
            ]),
            LensNeed::ExactRows {
                kind: row_kind,
                ids,
                representation,
            } => object(vec![
                ("operation", text("rows")),
                ("kind", kind(*row_kind)),
                ("ids", strings(ids)),
                (
                    "representation",
                    text(match representation {
                        LensRepresentation::Full => "full",
                        LensRepresentation::CoveredCompact => "covered_compact",
                    }),
                ),
            ]),
            LensNeed::CandidateIds {
                kind: row_kind,
                sources,
                identities,
                index,
                after,
                limit,
            } => {
                let mut fields = vec![
                    ("operation", text("candidates")),
                    ("kind", kind(*row_kind)),
                    ("sources", strings(sources)),
                    ("limit", number(*limit)),
                    (
                        "identities",
                        JsonValue::Array(
                            identities
                                .iter()
                                .map(|group| {
                                    object(vec![
                                        ("all", JsonValue::Bool(group.all)),
                                        (
                                            "terms",
                                            JsonValue::Array(
                                                group
                                                    .terms
                                                    .iter()
                                                    .map(|term| {
                                                        object(vec![
                                                            ("field", text(&term.field)),
                                                            ("values", strings(&term.values)),
                                                        ])
                                                    })
                                                    .collect(),
                                            ),
                                        ),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                    (
                        "index",
                        match index {
                            LensCandidateIndex::Source => object(vec![("kind", text("source"))]),
                            LensCandidateIndex::Union => object(vec![("kind", text("union"))]),
                            LensCandidateIndex::Identity(field) => {
                                object(vec![("kind", text("identity")), ("field", text(field))])
                            }
                        },
                    ),
                ];
                if let Some(cursor) = after {
                    match cursor {
                        LensCandidateCursor::Id(id) => fields.push((
                            "after",
                            object(vec![("kind", text("id")), ("id", text(id))]),
                        )),
                        LensCandidateCursor::SourceOrder { .. } => {
                            return Err(corrupt("native candidate cursor reached published lens"));
                        }
                    }
                }
                object(fields)
            }
            LensNeed::FocusIds {
                field,
                identifier,
                sources,
                source_priority,
                limit,
            } => object(vec![
                ("operation", text("focus")),
                ("field", text(field)),
                ("identifier", text(identifier)),
                ("sources", strings(sources)),
                ("source_priority", strings(source_priority)),
                ("limit", number(*limit)),
            ]),
            LensNeed::IncidentIds {
                identifier,
                after,
                limit,
            } => object(vec![
                ("operation", text("incident")),
                ("identifier", text(identifier)),
                ("after", text(after)),
                ("limit", number(*limit)),
            ]),
            LensNeed::OrderedHeaders {
                query,
                after,
                limit,
            } => {
                let mut fields = vec![
                    ("operation", text("ordered")),
                    ("query", header_query(query)),
                    ("limit", number(*limit)),
                ];
                if let Some((key, id)) = after {
                    fields.push(("after", JsonValue::Array(vec![text(key), text(id)])));
                }
                object(fields)
            }
            LensNeed::EntityAliasIds {
                entities,
                sources,
                exclude,
                limit,
            } => object(vec![
                ("operation", text("aliases")),
                ("entities", strings(entities)),
                ("sources", strings(sources)),
                ("exclude", strings(exclude)),
                ("limit", number(*limit)),
            ]),
            LensNeed::NodeSources { ids } => {
                object(vec![("operation", text("sources")), ("ids", strings(ids))])
            }
            LensNeed::Count { query } => object(vec![
                ("operation", text("count")),
                ("query", header_query(query)),
            ]),
            LensNeed::LookupRows { .. } | LensNeed::IdentityIds { .. } => {
                return Err(corrupt("native read reached published lens"));
            }
        };
        emit_python_compact_json(&value, self.transport_limits()?)
            .map(Some)
            .map_err(|_| {
                error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "lens need emission refused",
                )
            })
    }
    pub fn resume_rows(&mut self, raw: &[u8], sizes: &[u32]) -> Result<(), SearchV2Error> {
        let row_limit = usize::try_from(self.budget.lens.inspect.max_rows)
            .map_err(|_| corrupt("lens row limit invalid"))?;
        if sizes.len() > row_limit
            || sizes.iter().any(|size| {
                *size == 0 || *size as usize > self.budget.lens.inspect.max_payload_bytes
            })
        {
            return Err(corrupt("lens row byte vector invalid"));
        }
        let total = sizes
            .iter()
            .try_fold(0usize, |sum, size| sum.checked_add(*size as usize))
            .ok_or_else(|| corrupt("lens row bytes overflow"))?;
        if total as u64 > self.budget.lens.inspect.max_decoded_bytes {
            return Err(error(
                SearchV2ErrorCode::BudgetExceeded,
                "lens row byte budget",
            ));
        }
        let expected = total
            .checked_add(2 + sizes.len().saturating_sub(1))
            .ok_or_else(|| corrupt("lens row framing overflow"))?;
        if raw.len() != expected {
            return Err(corrupt("lens row byte vector differs from transport"));
        }
        let visits = self
            .budget
            .lens
            .inspect
            .json
            .max_visits
            .checked_mul(sizes.len())
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| corrupt("lens row visit limit overflow"))?;
        let value = parse_json(
            raw,
            JsonMode::PublishedStrict,
            JsonLimits {
                max_bytes: expected,
                max_depth: self
                    .budget
                    .lens
                    .inspect
                    .json
                    .max_depth
                    .checked_add(1)
                    .ok_or_else(|| corrupt("lens row depth limit overflow"))?,
                max_visits: visits,
                ..self.budget.lens.inspect.json
            },
        )
        .map_err(|_| corrupt("lens row array invalid"))?;
        let JsonValue::Array(rows) = value.into_root() else {
            return Err(corrupt("lens rows are not an array"));
        };
        if rows.len() != sizes.len() {
            return Err(corrupt("lens row vector length differs"));
        }
        self.plan.resume(LensReply::Rows {
            rows,
            raw_bytes: sizes.iter().map(|size| *size as usize).collect(),
        })
    }
    fn reply(&self, raw: &[u8]) -> Result<JsonValue, SearchV2Error> {
        parse(
            raw,
            self.transport_limits()?.max_bytes,
            self.budget.lens.inspect.json,
        )
    }
    pub fn resume_candidates(&mut self, raw: &[u8]) -> Result<(), SearchV2Error> {
        let value = self.reply(raw)?;
        let rows = value
            .as_array()
            .ok_or_else(|| corrupt("lens candidates are not an array"))?
            .iter()
            .map(|row| {
                Ok(LensCandidate {
                    id: string(field(row, "id")?)?,
                    source: None,
                    position: None,
                })
            })
            .collect::<Result<_, SearchV2Error>>()?;
        self.plan
            .resume(LensReply::Candidates(LensCandidatePage { rows }))
    }
    pub fn resume_ids(&mut self, raw: &[u8]) -> Result<(), SearchV2Error> {
        let value = self.reply(raw)?;
        self.plan.resume(LensReply::Ids(string_list(&value)?))
    }
    pub fn resume_headers(&mut self, raw: &[u8]) -> Result<(), SearchV2Error> {
        let value = self.reply(raw)?;
        let rows = value
            .as_array()
            .ok_or_else(|| corrupt("lens headers are not an array"))?
            .iter()
            .map(|row| {
                Ok(LensHeader {
                    id: string(field(row, "id")?)?,
                    sort_key: string(field(row, "sort_key")?)?,
                    from_id: string(field(row, "from_id")?)?,
                    to_id: string(field(row, "to_id")?)?,
                })
            })
            .collect::<Result<_, SearchV2Error>>()?;
        self.plan.resume(LensReply::Headers(rows))
    }
    pub fn resume_sources(&mut self, raw: &[u8]) -> Result<(), SearchV2Error> {
        let value = self.reply(raw)?;
        let rows = value
            .as_array()
            .ok_or_else(|| corrupt("lens sources are not an array"))?
            .iter()
            .map(|row| {
                let values = row
                    .as_array()
                    .filter(|values| values.len() == 2)
                    .ok_or_else(|| corrupt("lens source pair invalid"))?;
                Ok((string(&values[0])?, string(&values[1])?))
            })
            .collect::<Result<_, SearchV2Error>>()?;
        self.plan.resume(LensReply::Sources(rows))
    }
    pub fn resume_count(&mut self, count: u64) -> Result<(), SearchV2Error> {
        self.plan.resume(LensReply::Count(count))
    }
    pub fn resume_stores(&mut self, compact: bool, membership: bool) -> Result<(), SearchV2Error> {
        self.plan.resume(LensReply::Stores(LensStores {
            compact,
            membership,
        }))
    }
    pub fn finish(&mut self) -> Result<Vec<u8>, SearchV2Error> {
        let packet = self.plan.finish()?;
        emit_python_compact_json(
            &packet,
            JsonLimits {
                max_bytes: self.budget.lens.inspect.max_response_bytes,
                ..self.budget.lens.inspect.json
            },
        )
        .map_err(|reason| {
            error(
                if reason.code == FoundationErrorCode::BudgetExceeded {
                    SearchV2ErrorCode::BudgetExceeded
                } else {
                    SearchV2ErrorCode::CorruptSelectedCarrier
                },
                "lens packet emission refused",
            )
        })
    }
}
