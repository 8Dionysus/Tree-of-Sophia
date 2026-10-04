//! Generic reads of the weaker authenticated query_store_v1 carrier.
//! No prepared model, source capture, publication epoch or authority is created.
use super::*;
use serde_json::json;
use std::collections::BTreeSet;

const SOURCES: &[&str] = &[
    "philosophy",
    "canon",
    "candidate-intake",
    "source-navigation",
    "source-claims",
    "semantic-interchange",
    "repository",
];
fn field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}
fn strings(value: Option<&Value>) -> Result<Vec<String>> {
    match value {
        None | Some(Value::Null) => Ok(vec![]),
        Some(Value::Array(values)) => Ok(values
            .iter()
            .filter_map(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()),
        _ => Err(err("legacy query string filter must be an array")),
    }
}
fn integer(args: &Value, key: &str, default: usize, max: usize) -> Result<usize> {
    match args.get(key) {
        None => Ok(default),
        Some(v) => v
            .as_u64()
            .filter(|n| *n <= max as u64)
            .map(|n| n as usize)
            .ok_or_else(|| err("legacy query integer bound")),
    }
}
pub(super) fn reserve_raw(bytes: usize, used: &mut usize, cap: usize) -> Result<()> {
    let cost = bytes
        .checked_mul(128)
        .and_then(|n| n.checked_add(1024))
        .ok_or_else(|| err("legacy query state overflow"))?;
    *used = used
        .checked_add(cost)
        .filter(|n| *n <= cap)
        .ok_or_else(|| err("legacy query retained state budget"))?;
    Ok(())
}
pub(super) fn retained(value: &Value, used: &mut usize, cap: usize) -> Result<()> {
    struct Count {
        bytes: usize,
        cap: usize,
    }
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes = self
                .bytes
                .checked_add(bytes.len())
                .filter(|n| *n <= self.cap)
                .ok_or_else(|| std::io::Error::other("legacy query serialization state budget"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let remaining = cap
        .checked_sub(*used)
        .and_then(|n| n.checked_sub(1024))
        .ok_or_else(|| err("legacy query retained state budget"))?;
    let mut count = Count {
        bytes: 0,
        cap: remaining / 128,
    };
    // Count directly into a bounded sink; no complete encoded Vec is allocated.
    serde_json::to_writer(&mut count, value).map_err(err)?;
    reserve_raw(count.bytes, used, cap)
}
impl LegacyStore {
    /// HTTP operation probe is combined with the original owner deadline and
    /// cumulative VM counter; it never replaces or resets those grants.
    pub fn query_call_with_probe(
        &mut self,
        tool: &str,
        args: &Value,
        cap: usize,
        operation: Arc<dyn AbortProbe>,
    ) -> Result<Value> {
        struct Combined {
            original: Arc<dyn AbortProbe>,
            operation: Arc<dyn AbortProbe>,
        }
        impl AbortProbe for Combined {
            fn reason(&self) -> Option<crate::AbortReason> {
                self.original.reason().or_else(|| self.operation.reason())
            }
        }
        let original = self.abort.clone();
        self.abort = Arc::new(Combined {
            original: original.clone(),
            operation,
        });
        self.install_query_progress();
        let result = self.query_call(tool, args, cap);
        self.abort = original;
        self.install_query_progress();
        result
    }
    fn install_query_progress(&self) {
        let vm = self.vm.clone();
        let probe = self.abort.clone();
        let cap = self.limits.max_sql_vm_steps;
        let deadline = self.deadline;
        self.db.progress_handler(
            1,
            Some(move || {
                vm.fetch_add(1, Ordering::Relaxed) >= cap
                    || Instant::now() >= deadline
                    || probe.reason().is_some()
            }),
        );
    }

    fn selected_rows(
        &mut self,
        relations: bool,
        predicate: &str,
        params: &[String],
        limit: Option<usize>,
        state: &mut usize,
        cap: usize,
    ) -> Result<(u64, Vec<Value>)> {
        let mut selected = vec![];
        let count =
            self.visit_knowledge_where(relations, predicate, params, cap / 256, 128, |value| {
                if limit.is_none_or(|limit| selected.len() < limit) {
                    retained(value, state, cap)?;
                    selected.push(value.clone());
                }
                Ok(())
            })?;
        Ok((count, selected))
    }
    fn resolve(
        &mut self,
        relations: bool,
        identifier: &str,
        state: &mut usize,
        cap: usize,
    ) -> Result<(&'static str, Vec<Value>)> {
        for key in if relations {
            &["id", "native_id"][..]
        } else {
            &["id", "entity_id", "native_id"][..]
        } {
            let (_, rows) = self.selected_rows(
                relations,
                &format!("{key}=?2"),
                &[identifier.to_owned()],
                None,
                state,
                cap,
            )?;
            if !rows.is_empty() {
                return Ok((key, rows));
            }
        }
        Err(err("unknown ToS knowledge identifier"))
    }
    fn packet_navigation(&self, items: &[&Value]) -> Result<(Vec<String>, Value)> {
        let mut refs = BTreeSet::new();
        let limits = JsonLimits::new(
            self.limits.max_json_bytes,
            96,
            self.limits.max_work_steps.min(usize::MAX as u64) as usize,
            4300,
        )
        .map_err(err)?;
        let mut envelopes = vec![];
        for item in items {
            refs.extend(strings(item.get("source_refs"))?);
            let raw = serde_json::to_vec(item).map_err(err)?;
            let doc = parse_json(&raw, JsonMode::PublishedStrict, limits).map_err(err)?;
            envelopes.push(doc.root().clone());
        }
        let targets =
            crate::source_read_projection::source_read_targets(&envelopes, &self.revision, limits);
        let raw = tos_foundation::emit_value_preserved_json(&targets, limits).map_err(err)?;
        Ok((
            refs.into_iter().collect(),
            serde_json::from_slice(&raw).map_err(err)?,
        ))
    }
    /// One generic operation stays inside this exact held SQLite/five-input cut.
    /// The caller's original state grant also bounds retained response carriers.
    pub fn query_call(
        &mut self,
        tool: &str,
        args: &Value,
        max_state_bytes: usize,
    ) -> Result<Value> {
        self.verify_currentness()?;
        let fields = args
            .as_object()
            .ok_or_else(|| err("legacy query arguments object"))?;
        if max_state_bytes == 0 {
            return Err(err("legacy query state budget"));
        }
        let allowed: &[&str] = match tool {
            "tos_knowledge_catalog" | "tos_knowledge_header" | "tos_corpus_header" => &[],
            "tos_knowledge_node" => &["node_id", "relation_limit"],
            "tos_knowledge_relation" => &["relation_id"],
            "tos_knowledge_search" => &[
                "query",
                "sources",
                "kind_ids",
                "predicate_ids",
                "offset",
                "limit",
                "mode",
                "cursor",
            ],
            _ => return Err(err("selected legacy QueryStore tool has no native route")),
        };
        if fields.keys().any(|key| !allowed.contains(&key.as_str())) {
            return Err(err("legacy query unknown argument"));
        }
        let mut state = 0usize;
        let result = match tool {
            "tos_knowledge_catalog" => {
                retained(&self.catalog, &mut state, max_state_bytes)?;
                self.catalog.clone()
            }
            "tos_knowledge_header" => {
                retained(&self.graph_header, &mut state, max_state_bytes)?;
                self.graph_header.clone()
            }
            "tos_corpus_header" => self.corpus_header_with_graph_views_bounded(max_state_bytes)?,
            "tos_knowledge_node" | "tos_knowledge_relation" => {
                let relations = tool == "tos_knowledge_relation";
                let identifier = args
                    .get(if relations { "relation_id" } else { "node_id" })
                    .and_then(Value::as_str)
                    .ok_or_else(|| err("legacy query identifier required"))?;
                let identifier = tos_foundation::python_strip_unicode16_v1(
                    identifier,
                    self.limits.max_json_bytes,
                )
                .map_err(err)?;
                if identifier.is_empty() {
                    return Err(err("legacy query identifier required"));
                }
                let (key, matches) =
                    self.resolve(relations, identifier, &mut state, max_state_bytes)?;
                let ids: BTreeSet<String> = if relations {
                    matches
                        .iter()
                        .flat_map(|v| [field(v, "from_id"), field(v, "to_id")])
                        .map(str::to_owned)
                        .collect()
                } else {
                    matches.iter().map(|v| field(v, "id").to_owned()).collect()
                };
                let marks = (0..ids.len())
                    .map(|i| format!("?{}", i + 2))
                    .collect::<Vec<_>>()
                    .join(",");
                let params: Vec<String> = ids.into_iter().collect();
                let predicate = if relations {
                    format!("id IN ({marks})")
                } else {
                    format!("from_id IN ({marks}) OR to_id IN ({marks})")
                };
                let limit = if relations {
                    None
                } else {
                    Some(integer(args, "relation_limit", 200, 1000)?)
                };
                let (count, neighbors) = self.selected_rows(
                    !relations,
                    &predicate,
                    &params,
                    limit,
                    &mut state,
                    max_state_bytes,
                )?;
                // Navigation projection transiently retains a second representation.
                if state > max_state_bytes / 2 {
                    return Err(err("legacy query navigation state budget"));
                }
                let items: Vec<&Value> = matches.iter().chain(neighbors.iter()).collect();
                let (source_refs, source_read_targets) = self.packet_navigation(&items)?;
                let mut packet = json!({"schema":if relations {"tos_knowledge_relation_packet_v1"} else {"tos_knowledge_node_packet_v1"},"source_revision":self.revision,"requested_id":identifier,"ambiguous_native_id":key=="native_id" && matches.len()>1,"source_refs":source_refs,"source_read_targets":source_read_targets,"authority_boundary":self.graph_header.get("authority_boundary").cloned().unwrap_or(json!({}))});
                packet["counts"] = if relations {
                    json!({"matches":matches.len(),"endpoints":neighbors.len()})
                } else {
                    json!({"matches":matches.len(),"related_relations":count,"returned_relations":neighbors.len()})
                };
                if !relations {
                    packet["shared_entity_id"] = json!(key == "entity_id" && matches.len() > 1);
                }
                packet["matches"] = Value::Array(matches);
                packet[if relations {
                    "endpoints"
                } else {
                    "related_relations"
                }] = Value::Array(neighbors);
                packet
            }
            "tos_knowledge_search" => self.search_call(args, max_state_bytes)?,
            _ => unreachable!(),
        };
        self.verify_currentness()?;
        Ok(result)
    }
    fn search_call(&mut self, args: &Value, cap: usize) -> Result<Value> {
        if args
            .get("mode")
            .is_some_and(|v| v.as_str() != Some("legacy"))
            || args.get("cursor").is_some_and(|v| !v.is_null())
        {
            return Err(err("selected legacy search requires legacy offset mode"));
        }
        let query = args
            .get("query")
            .map(|v| v.as_str().ok_or_else(|| err("legacy search query string")))
            .transpose()?
            .unwrap_or("");
        let query = tos_foundation::python_strip_unicode16_v1(query, self.limits.max_json_bytes)
            .map_err(err)?
            .to_owned();
        if query.chars().count() > 256 {
            return Err(err("legacy search query bound"));
        }
        let needle =
            tos_foundation::python_lower_unicode16_v1(&query, 256, 1024, 4096).map_err(err)?;
        let mut sources = strings(args.get("sources"))?;
        if sources.iter().any(|s| !SOURCES.contains(&s.as_str())) {
            return Err(err("unsupported knowledge source"));
        }
        if sources.is_empty() {
            sources = SOURCES.iter().map(|s| s.to_string()).collect();
            sources.sort();
        }
        let kinds = strings(args.get("kind_ids"))?;
        let predicates = strings(args.get("predicate_ids"))?;
        if kinds.len() > 100 || predicates.len() > 100 {
            return Err(err("legacy search filter bound"));
        }
        let offset = integer(args, "offset", 0, 100000)?;
        let limit = integer(args, "limit", 40, 100)?;
        if limit == 0 {
            return Err(err("legacy search limit bound"));
        }
        let keep = offset + limit;
        let mut results = vec![];
        let mut counts = vec![];
        let mut state = 0usize;
        for relations in [false, true] {
            let filters = if relations { &predicates } else { &kinds };
            let mut params = sources.clone();
            let mut clause = format!(
                "source_graph IN ({})",
                (0..params.len())
                    .map(|i| format!("?{}", i + 2))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            if !filters.is_empty() {
                let start = params.len() + 2;
                clause += &format!(
                    " AND {} IN ({})",
                    if relations { "predicate_id" } else { "kind_id" },
                    (0..filters.len())
                        .map(|i| format!("?{}", i + start))
                        .collect::<Vec<_>>()
                        .join(",")
                );
                params.extend(filters.clone());
            }
            if !needle.is_empty() {
                clause += &format!(" AND instr(search_text,?{})>0", params.len() + 2);
                params.push(needle.clone());
            }
            let mut top: Vec<((u8, String, String, String), Value, usize)> = vec![];
            let limits = JsonLimits::new(
                self.limits.max_json_bytes,
                96,
                self.limits.max_work_steps.min(usize::MAX as u64) as usize,
                4300,
            )
            .map_err(err)?;
            let count =
                self.visit_knowledge_where(relations, &clause, &params, cap / 256, 128, |value| {
                    let raw = serde_json::to_vec(value).map_err(err)?;
                    let doc = parse_json(&raw, JsonMode::PublishedStrict, limits).map_err(err)?;
                    let (rank, id) = crate::knowledge_legacy_search::rank(
                        doc.root(),
                        &needle,
                        if relations {
                            crate::search_v2::SearchKind::Relations
                        } else {
                            crate::search_v2::SearchKind::Nodes
                        },
                        limits.max_bytes,
                    )
                    .map_err(err)?;
                    let key = (
                        rank,
                        id,
                        field(value, "source_graph").to_owned(),
                        field(value, "id").to_owned(),
                    );
                    let position = top.partition_point(|(old, _, _)| old <= &key);
                    if position < keep {
                        let before = state;
                        retained(value, &mut state, cap)?;
                        let cost = state - before;
                        top.insert(position, (key, value.clone(), cost));
                        if top.len() > keep {
                            state -= top.pop().unwrap().2;
                        }
                    }
                    Ok(())
                })?;
            counts.push(count);
            let page: Vec<Value> = top
                .into_iter()
                .skip(offset)
                .map(|(_, value, _)| value)
                .collect();
            results.push(page);
        }
        let relations = results.pop().unwrap();
        let nodes = results.pop().unwrap();
        Ok(
            json!({"schema":"tos_knowledge_search_v1","source_revision":self.revision,"query":query,"filters":{"sources":sources,"kind_ids":kinds,"predicate_ids":predicates},"page":{"offset":offset,"limit_per_kind":limit},"counts":{"matching_nodes":counts[0],"matching_relations":counts[1],"returned_nodes":nodes.len(),"returned_relations":relations.len()},"nodes":nodes,"relations":relations,"authority_boundary":self.graph_header.get("authority_boundary").cloned().unwrap_or(json!({}))}),
        )
    }
}
