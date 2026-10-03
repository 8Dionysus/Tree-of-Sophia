//! Evidence Lens joins original projection context to one authenticated,
//! declared release member. Receipt custody does not confer evidence authority.
use super::*;
use crate::knowledge_lens_spec::{py_string, truthy};
const CHALLENGES: [&str; 3] = ["contested_by", "polemicizes_with", "uncertain_relation"];
pub const EVIDENCE_OPERATION: &str = "tos.epistemic.inspect";
#[derive(Clone, Debug)]
pub enum EvidenceMode {
    Philosophy,
    Corpus,
}
#[derive(Clone, Debug)]
pub struct EvidenceRequest {
    pub mode: EvidenceMode,
    pub item_id: String,
    pub view_id: Option<String>,
    pub limit: usize,
}
impl EvidenceRequest {
    pub fn intended_use(&self) -> &'static str {
        match self.mode {
            EvidenceMode::Philosophy => PHILOSOPHY_INTENDED_USE,
            EvidenceMode::Corpus => crate::corpus_read::CORPUS_INTENDED_USE,
        }
    }
}
fn id(value: &JsonValue) -> &str {
    if truthy(get(value, "node_id")) {
        s(get(value, "node_id"))
    } else {
        s(get(value, "edge_id"))
    }
}
fn unique<'a>(rows: impl IntoIterator<Item = &'a JsonValue>, key: &str) -> JsonValue {
    texts(
        rows.into_iter()
            .filter_map(|v| get(v, key).as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
    )
}
fn chosen<'a>(value: &'a JsonValue, key: &str, fallback: &str) -> &'a JsonValue {
    let v = get(value, key);
    if truthy(v) { v } else { get(value, fallback) }
}
pub(super) fn phi_context<'a>(
    graph: &Graph<'a>,
    base_nodes: &'a [JsonValue],
    base_edges: &'a [JsonValue],
    request: &EvidenceRequest,
    w: &mut Work<'_>,
) -> Result<JsonValue, SearchV2Error> {
    let selection = graph
        .nodes
        .iter()
        .chain(graph.edges.iter())
        .copied()
        .find(|v| id(v) == request.item_id)
        .ok_or_else(unknown)?;
    let (nodes, edges) = if let Some(view) = request.view_id.as_deref().filter(|v| !v.is_empty()) {
        let view = graph.view(view)?;
        let rows = graph.view_rows(view, base_nodes, base_edges);
        if !rows
            .0
            .iter()
            .chain(rows.1.iter())
            .any(|v| id(v) == request.item_id)
        {
            return Err(unknown());
        }
        rows
    } else {
        (graph.nodes.clone(), graph.edges.clone())
    };
    context(selection, &nodes, &edges, request, true, w)
}
fn context(
    selection: &JsonValue,
    nodes: &[&JsonValue],
    edges: &[&JsonValue],
    request: &EvidenceRequest,
    phi: bool,
    w: &mut Work<'_>,
) -> Result<JsonValue, SearchV2Error> {
    let node = truthy(get(selection, "node_id"));
    let endpoints = if node {
        BTreeSet::from([request.item_id.as_str()])
    } else {
        BTreeSet::from([s(get(selection, "from_id")), s(get(selection, "to_id"))])
    };
    let mut candidates = Vec::new();
    for edge in edges {
        w.step()?;
        if (!node && id(edge) == request.item_id)
            || endpoints.contains(s(get(edge, "from_id")))
            || endpoints.contains(s(get(edge, "to_id")))
        {
            candidates.push(*edge);
        }
    }
    candidates.sort_by_key(|v| (id(v) != request.item_id, id(v)));
    let available = if phi {
        candidates
            .iter()
            .copied()
            .filter(|v| CHALLENGES.contains(&s(get(v, "predicate_id"))))
            .collect::<Vec<_>>()
    } else {
        vec![]
    };
    let capacity = request.limit
        - usize::from(phi && !node && !CHALLENGES.contains(&s(get(selection, "predicate_id"))));
    let challenges = available.iter().copied().take(capacity).collect::<Vec<_>>();
    let challenge_ids = challenges.iter().map(|v| id(v)).collect::<BTreeSet<_>>();
    let contexts = candidates
        .iter()
        .copied()
        .filter(|v| !challenge_ids.contains(id(v)))
        .take(request.limit - challenges.len())
        .collect::<Vec<_>>();
    let mut related = BTreeSet::new();
    for edge in challenges.iter().chain(contexts.iter()) {
        for endpoint in [s(get(edge, "from_id")), s(get(edge, "to_id"))] {
            if !endpoint.is_empty() {
                related.insert(endpoint);
            }
        }
    }
    if node {
        related.remove(request.item_id.as_str());
    }
    let neighbors = nodes
        .iter()
        .copied()
        .filter(|v| related.contains(id(v)))
        .collect::<Vec<_>>();
    let surrounding = challenges
        .iter()
        .chain(contexts.iter())
        .chain(neighbors.iter())
        .copied()
        .collect::<Vec<_>>();
    let properties = surrounding
        .iter()
        .copied()
        .filter(|v| id(v) != request.item_id)
        .map(|v| get(v, "properties"))
        .filter(|v| v.as_object().is_some())
        .collect::<Vec<_>>();
    let posture = if phi {
        let p = get(selection, "properties");
        object(vec![
            ("authority_posture", get(p, "authority_posture").clone()),
            ("canon_status", get(p, "canon_status").clone()),
            ("review_posture", get(p, "review_posture").clone()),
            (
                "confidence",
                chosen(p, "confidence", "master_confidence").clone(),
            ),
            ("priority", get(p, "priority").clone()),
            (
                "claim_evidence_closed",
                JsonValue::Bool(get(p, "claim_evidence_closed") == &JsonValue::Bool(true)),
            ),
        ])
    } else {
        object(vec![
            (
                "authority_posture",
                get(selection, "authority_layer").clone(),
            ),
            ("canon_status", get(selection, "status").clone()),
            ("review_posture", JsonValue::Null),
            ("confidence", get(selection, "confidence").clone()),
            ("priority", JsonValue::Null),
            ("claim_evidence_closed", JsonValue::Bool(false)),
        ])
    };
    let field = if phi {
        let confidences = properties
            .iter()
            .map(|v| chosen(v, "confidence", "master_confidence"))
            .filter(|v| truthy(v))
            .map(py_string)
            .collect::<BTreeSet<_>>();
        object(vec![
            (
                "authority_postures",
                unique(properties.iter().copied(), "authority_posture"),
            ),
            (
                "canon_statuses",
                unique(properties.iter().copied(), "canon_status"),
            ),
            (
                "review_postures",
                unique(properties.iter().copied(), "review_posture"),
            ),
            ("confidence_values", texts(confidences)),
        ])
    } else {
        object(vec![
            (
                "authority_postures",
                unique(contexts.iter().copied(), "authority_layer"),
            ),
            ("canon_statuses", unique(contexts.iter().copied(), "status")),
            ("review_postures", values([])),
            (
                "confidence_values",
                unique(contexts.iter().copied(), "confidence"),
            ),
        ])
    };
    let coverage = object(vec![
        ("posture", text("partial")),
        (
            "challenge_state",
            text(if challenges.len() < available.len() {
                "projected_signals_truncated"
            } else if !available.is_empty() {
                "projected_signals"
            } else {
                "none_in_projection_scope"
            }),
        ),
        ("available_challenge_relations", number(available.len())),
        ("returned_challenge_relations", number(challenges.len())),
        (
            "missing_surfaces",
            if phi {
                texts(
                    [
                        "claim-level support and counterevidence",
                        "source-visible review decisions",
                        "rights and publication decisions",
                    ]
                    .map(str::to_owned),
                )
            } else {
                texts(["curated Evidence Lens scene lookup pending".to_owned()])
            },
        ),
    ]);
    Ok(object(vec![
        ("selection", selection.clone()),
        ("challenge_relations", copies(challenges)),
        ("context_relations", copies(contexts)),
        ("neighbor_nodes", copies(neighbors)),
        ("selection_posture", posture),
        ("field_posture", field),
        ("coverage", coverage),
        (
            "source_refs",
            source_refs(std::iter::once(selection).chain(surrounding)),
        ),
    ]))
}
fn join(
    evidence: &JsonValue,
    context: &JsonValue,
    request: &EvidenceRequest,
    w: &mut Work<'_>,
) -> Result<JsonValue, SearchV2Error> {
    let mode = match request.mode {
        EvidenceMode::Philosophy => "philosophy",
        EvidenceMode::Corpus => "corpus",
    };
    let mut scene = &JsonValue::Null;
    for candidate in objs(get(evidence, "scenes")) {
        w.step()?;
        if objs(get(candidate, "selections")).iter().any(|route| {
            s(get(route, "mode")) == mode
                && arr(get(route, "item_ids"))
                    .iter()
                    .any(|id| id.as_str() == Some(request.item_id.as_str()))
        }) {
            scene = candidate;
            break;
        }
    }
    let selection = get(context, "selection");
    let (finding, finding_ru, posture, conclusion, routes, gaps, gaps_ru, anchors) = if scene
        .as_object()
        .is_some()
    {
        let finding = if truthy(get(scene, "finding")) {
            py_string(get(scene, "finding"))
        } else {
            String::new()
        };
        let finding_ru = if truthy(get(scene, "finding_ru")) {
            py_string(get(scene, "finding_ru"))
        } else {
            finding.clone()
        };
        (
            finding,
            finding_ru,
            if truthy(get(scene, "posture")) {
                py_string(get(scene, "posture"))
            } else {
                String::new()
            },
            scene
                .object_get("conclusion")
                .filter(|v| truthy(v))
                .cloned()
                .unwrap_or_else(|| object(vec![])),
            copies(objs(get(scene, "routes"))),
            texts(arr(get(scene, "gaps")).iter().map(py_string)),
            scene
                .object_get("gaps_ru")
                .map(|v| texts(arr(v).iter().map(py_string)))
                .unwrap_or_else(|| texts(arr(get(scene, "gaps")).iter().map(py_string))),
            copies(objs(get(scene, "source_anchors"))),
        )
    } else {
        (
            "No curated Evidence Lens route is published for this selection.".into(),
            "Для выбранного объекта ещё не опубликован курируемый маршрут Evidence Lens.".into(),
            "projection-only".into(),
            object(vec![
                ("can_conclude", JsonValue::Bool(false)),
                (
                    "canon_membership",
                    JsonValue::Bool(s(get(selection, "authority_layer")) == "canon"),
                ),
                ("claim_evidence_closed", JsonValue::Bool(false)),
                (
                    "allowed",
                    texts([
                        "inspect the projection context and its source-return references".into(),
                    ]),
                ),
                (
                    "allowed_ru",
                    texts([
                        "исследовать контекст проекции и её ссылки возврата к источникам".into(),
                    ]),
                ),
                (
                    "not_allowed",
                    texts(["infer evidence closure from projection membership".into()]),
                ),
                (
                    "not_allowed_ru",
                    texts(["выводить доказательную замкнутость из присутствия в проекции".into()]),
                ),
            ]),
            values([]),
            texts(["curated source, review, rights, and claim/evidence routes".into()]),
            texts(["курируемые маршруты к source, review, rights и claim/evidence".into()]),
            values([]),
        )
    };
    let mut counts = BTreeMap::<String, usize>::new();
    for route in arr(&routes) {
        w.step()?;
        let kind = if truthy(get(route, "route_kind")) {
            py_string(get(route, "route_kind"))
        } else {
            "other".into()
        };
        *counts.entry(kind).or_default() += 1;
    }
    let route_counts = JsonValue::Object(
        counts
            .into_iter()
            .map(|(k, n)| (JsonString::from_utf8(&k), number(n)))
            .collect(),
    );
    let refs = arr(get(context, "source_refs"))
        .iter()
        .chain(arr(get(scene, "source_refs")))
        .filter_map(JsonValue::as_str)
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let coverage = if scene.as_object().is_some() {
        replace(
            get(context, "coverage"),
            &[],
            vec![
                ("missing_surfaces", gaps.clone()),
                ("posture", text("curated-route")),
            ],
        )
    } else {
        get(context, "coverage").clone()
    };
    let summary = object(vec![
        ("selection", text(&request.item_id)),
        ("finding", text(&finding)),
        ("finding_ru", text(&finding_ru)),
        ("posture", text(&posture)),
        (
            "can_conclude",
            JsonValue::Bool(get(&conclusion, "can_conclude") == &JsonValue::Bool(true)),
        ),
        (
            "canon_membership",
            JsonValue::Bool(get(&conclusion, "canon_membership") == &JsonValue::Bool(true)),
        ),
        (
            "claim_evidence_closed",
            JsonValue::Bool(get(&conclusion, "claim_evidence_closed") == &JsonValue::Bool(true)),
        ),
        ("route_counts", route_counts),
        ("gap_count", number(arr(&gaps).len())),
        ("page_updated", JsonValue::Bool(true)),
        (
            "next_actions",
            texts([
                "inspect the full route cards on the page".into(),
                "open the referenced owner surface before making a stronger claim".into(),
            ]),
        ),
    ]);
    Ok(object(vec![
        ("schema", text("tos_evidence_lens_packet_v1")),
        ("mode", text(mode)),
        ("item_id", text(&request.item_id)),
        (
            "view_id",
            request
                .view_id
                .as_deref()
                .map(text)
                .unwrap_or(JsonValue::Null),
        ),
        ("selection", selection.clone()),
        ("scene", scene.clone()),
        ("finding", text(&finding)),
        ("finding_ru", text(&finding_ru)),
        ("posture", text(&posture)),
        ("conclusion", conclusion),
        ("source_anchors", anchors.clone()),
        ("routes", routes.clone()),
        ("gaps", gaps.clone()),
        ("gaps_ru", gaps_ru),
        (
            "challenge_relations",
            get(context, "challenge_relations").clone(),
        ),
        (
            "context_relations",
            get(context, "context_relations").clone(),
        ),
        ("neighbor_nodes", get(context, "neighbor_nodes").clone()),
        (
            "selection_posture",
            get(context, "selection_posture").clone(),
        ),
        ("field_posture", get(context, "field_posture").clone()),
        ("coverage", coverage),
        (
            "counts",
            object(vec![
                ("routes", number(arr(&routes).len())),
                ("source_anchors", number(arr(&anchors).len())),
                ("gaps", number(arr(&gaps).len())),
                (
                    "challenge_relations",
                    number(arr(get(context, "challenge_relations")).len()),
                ),
                (
                    "context_relations",
                    number(arr(get(context, "context_relations")).len()),
                ),
            ]),
        ),
        ("source_refs", texts(refs)),
        (
            "authority_boundary",
            metadata(evidence, "authority_boundary"),
        ),
        (
            "authority_note",
            get(get(evidence, "authority_boundary"), "note")
                .as_str()
                .map(text)
                .unwrap_or_else(|| text("")),
        ),
        ("agent_summary", summary),
    ]))
}
pub fn execute_selected_evidence<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    request: &EvidenceRequest,
    evidence_raw: &[u8],
    corpus_context: Option<&crate::corpus_read::CorpusReadContext>,
    budget: PhilosophyReadBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    if request.item_id.is_empty()
        || request.item_id.len() > budget.inspect.max_field_bytes
        || request
            .view_id
            .as_ref()
            .is_some_and(|v| v.len() > budget.inspect.max_field_bytes)
        || !(1..=200).contains(&request.limit)
    {
        return Err(invalid());
    }
    if evidence_raw.is_empty()
        || evidence_raw.len() > budget.inspect.max_payload_bytes
        || evidence_raw.len() as u64 > budget.max_work_steps
    {
        return Err(failure(
            SearchV2ErrorCode::BudgetExceeded,
            "evidence projection work/byte budget",
        ));
    }
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        EVIDENCE_OPERATION,
        request.intended_use(),
        budget.inspect,
        |read| {
            read.external_projection_bytes(evidence_raw.len())?;
            let document = parse_json(evidence_raw, JsonMode::PublishedStrict, budget.inspect.json)
                .map_err(|_| {
                    failure(
                        SearchV2ErrorCode::CorruptSelectedCarrier,
                        "invalid evidence projection",
                    )
                })?;
            let evidence = document.root();
            if s(get(evidence, "schema_version")) != "tos_epistemic_evidence_projection_v1" {
                return Err(failure(
                    SearchV2ErrorCode::CorruptSelectedCarrier,
                    "evidence projection schema",
                ));
            }
            let remaining = budget.max_work_steps - evidence_raw.len() as u64;

            match request.mode {
                EvidenceMode::Philosophy => {
                    let receipt = bound_original_receipt(read, bound)?;
                    let mut header =
                        original_rows(read, &receipt, PhilosophyOriginalCollection::Header, 1)?;
                    let nodes = original_rows(
                        read,
                        &receipt,
                        PhilosophyOriginalCollection::Nodes,
                        receipt.nodes,
                    )?;
                    let edges = original_rows(
                        read,
                        &receipt,
                        PhilosophyOriginalCollection::Edges,
                        receipt.edges,
                    )?;
                    let mut interrupt = || read.check_interrupt();
                    let mut w = Work {
                        remaining,
                        interrupt: &mut interrupt,
                    };
                    let header = header.remove(0);
                    let graph = Graph::new(&header, &nodes, &edges, &mut w)?;
                    let packet_context = phi_context(&graph, &nodes, &edges, request, &mut w)?;
                    join(evidence, &packet_context, request, &mut w)
                }
                EvidenceMode::Corpus => {
                    if request
                        .view_id
                        .as_deref()
                        .filter(|v| !v.is_empty())
                        .is_some_and(|v| v != "route-graph")
                    {
                        return Err(unknown());
                    }
                    let (graph, left) = crate::corpus_read::graph_for_evidence(
                        read,
                        bound,
                        corpus_context.ok_or_else(invalid)?,
                        remaining,
                    )?;
                    let nodes = objs(get(&graph, "nodes"));
                    let edges = objs(get(&graph, "edges"));
                    let selection = nodes
                        .iter()
                        .chain(edges.iter())
                        .copied()
                        .find(|v| id(v) == request.item_id)
                        .ok_or_else(unknown)?;
                    let mut interrupt = || read.check_interrupt();
                    let mut w = Work {
                        remaining: left,
                        interrupt: &mut interrupt,
                    };
                    let packet_context =
                        context(selection, &nodes, &edges, request, false, &mut w)?;
                    join(evidence, &packet_context, request, &mut w)
                }
            }
        },
    )
}
