//! Generic reads of the weaker authenticated query_store_v1 carrier.
//! No prepared model, source capture, publication epoch or authority is created.
use super::*;
use crate::knowledge_legacy_search::{LegacySearchIntegerInput, normalize_legacy_search_integer};
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
/// Source registration defaults for the native ordinary QueryStore adapter.
pub fn query_store_indexed_default_sources() -> &'static [&'static str] {
    SOURCES
}
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
fn search_integer(
    value: Option<&Value>,
    default: usize,
    minimum: usize,
    maximum: usize,
    budget: Option<&OriginalStoreBudget<'_>>,
) -> Result<usize> {
    let charge = |bytes| match budget {
        Some(budget) => search_charge(budget, bytes),
        None => Ok(()),
    };
    let normalize = |input| {
        normalize_legacy_search_integer(input, default, minimum, maximum)
            .map_err(|_| owned_err("legacy Search numeric argument"))
    };
    match value {
        None => normalize(LegacySearchIntegerInput::Missing),
        Some(Value::Null) => normalize(LegacySearchIntegerInput::Null),
        Some(Value::Bool(_)) => normalize(LegacySearchIntegerInput::Boolean),
        Some(Value::Number(number)) if number.is_i64() || number.is_u64() => {
            let text = number.to_string();
            charge(text.len())?;
            normalize(LegacySearchIntegerInput::Integer(&text))
        }
        Some(Value::Number(number)) => normalize(LegacySearchIntegerInput::Float(
            number.as_f64().unwrap_or(f64::NAN),
        )),
        Some(Value::String(text)) => {
            charge(text.len())?;
            normalize(LegacySearchIntegerInput::String(text))
        }
        Some(_) => normalize(LegacySearchIntegerInput::Other),
    }
}

impl LegacyStore {
    fn source_revision(&self) -> Result<&str> {
        self.graph_header
            .get("source_revision")
            .and_then(Value::as_str)
            .ok_or_else(|| owned_err("legacy graph source revision absent"))
    }

    pub fn indexed_graph_source_revision(&self) -> Result<Option<&str>> {
        match self.graph_header.get("source_revision") {
            Some(Value::Null) => Ok(None),
            Some(Value::String(value)) => Ok(Some(value)),
            _ => Err(owned_err("legacy graph source revision absent or invalid")),
        }
    }

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
        let targets = crate::source_read_projection::source_read_targets(
            &envelopes,
            self.source_revision()?,
            limits,
        );
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
        if self.owned.is_some() {
            return Err(err(
                "legacy owned store requires original-budget operation API",
            ));
        }

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
                let mut packet = json!({"schema":if relations {"tos_knowledge_relation_packet_v1"} else {"tos_knowledge_node_packet_v1"},"source_revision":self.source_revision()?,"requested_id":identifier,"ambiguous_native_id":key=="native_id" && matches.len()>1,"source_refs":source_refs,"source_read_targets":source_read_targets,"authority_boundary":self.graph_header.get("authority_boundary").cloned().unwrap_or(json!({}))});
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
        let offset = search_integer(args.get("offset"), 0, 0, 100_000, None)?;
        let limit = search_integer(args.get("limit"), 40, 1, 100, None)?;
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
            json!({"schema":"tos_knowledge_search_v1","source_revision":self.source_revision()?,"query":query,"filters":{"sources":sources,"kind_ids":kinds,"predicate_ids":predicates},"page":{"offset":offset,"limit_per_kind":limit},"counts":{"matching_nodes":counts[0],"matching_relations":counts[1],"returned_nodes":nodes.len(),"returned_relations":relations.len()},"nodes":nodes,"relations":relations,"authority_boundary":self.graph_header.get("authority_boundary").cloned().unwrap_or(json!({}))}),
        )
    }
}

// Owned v1 search uses the same normalized rows and ranking rule. Every Rust
// carrier is admitted before allocation; SQLite's copied bound text remains
// inside the one original dedicated heap. Arguments are borrowed caller-owned
// input, already admitted by that caller's ingress owner.
type OwnedHit = ((u8, String, String, String), Value, usize);
fn search_charge(budget: &OriginalStoreBudget<'_>, amount: usize) -> Result<()> {
    let amount = u64::try_from(amount).map_err(owned_err)?;
    atomic_charge(&budget.byte_work, budget.max_byte_work, amount)?;
    atomic_charge(&budget.store_steps, budget.max_store_steps, amount)
}
fn search_text<'s>(
    statement: &'s tos_source_store::PinnedBoundedStatement<'_>,
    column: i32,
    budget: &OriginalStoreBudget<'_>,
    deadline: Instant,
    probe: &dyn AbortProbe,
    held: usize,
) -> Result<&'s str> {
    let mut check = |bytes| {
        budget
            .check(deadline, probe)
            .and_then(|_| search_charge(budget, bytes))
            .map_err(|_| {
                tos_source_store::StoreError::new(
                    tos_source_store::StoreErrorCode::BudgetExceeded,
                    "legacy original text validation work/deadline",
                )
            })
    };
    // The callback target is distinct from its still-live parameter slots.
    (budget.remaining_after_retained)(state_add(held, std::mem::size_of_val(&check))?)?;
    statement
        .text_with_check(column, &mut check)
        .map_err(owned_err)
}

fn search_heap(value: &Value, check: &dyn Fn() -> Result<()>) -> Result<usize> {
    state_add(
        std::mem::size_of::<Value>(),
        serde_clone_storage(value, check)?,
    )
}
fn owned_search_fixed() -> Result<usize> {
    // Actual operation, scan, ranking, filter and response controllers. The
    // recursive JSON/conversion/serializer owners remain their existing owner
    // census, shared with metadata instead of an encoded-byte multiplier.
    let mut size = state_add(
        owned_metadata_controller_state_upper_bound()?,
        state_slots_bytes(34, owned_tree_controller_frame_bytes())?,
    )?;
    for extra in [
        std::mem::size_of::<OwnedBorrowedProbe<'_>>(),
        std::mem::size_of::<OriginalStoreBudget<'_>>(),
        std::mem::size_of::<tos_source_store::PinnedBoundedStatement<'_>>(),
        tos_source_store::PinnedBoundedStatement::text_validation_rust_workspace_upper_bound(),
        std::mem::size_of::<usize>(),
        std::mem::size_of::<(
            &tos_source_store::PinnedBoundedStatement<'_>,
            i32,
            &OriginalStoreBudget<'_>,
            Instant,
            &dyn AbortProbe,
        )>(),
        std::mem::size_of::<[Vec<Value>; 2]>(),
        std::mem::size_of::<Vec<OwnedHit>>(),
        std::mem::size_of::<[Vec<String>; 5]>(),
        std::mem::size_of::<[String; 8]>(),
        std::mem::size_of::<[(&'static str, Value); 11]>(),
        std::mem::size_of::<[Value; 12]>(),
        std::mem::size_of::<[usize; 40]>(),
        std::mem::size_of::<[&Value; 12]>(),
        std::mem::size_of::<[&str; 12]>(),
        std::mem::size_of::<std::str::CharIndices<'_>>(),
        std::mem::size_of::<std::str::Chars<'_>>(),
        std::mem::size_of::<[std::slice::Iter<'_, Value>; 5]>(),
        std::mem::size_of::<[std::slice::Iter<'_, String>; 5]>(),
        tos_foundation::python_lower_unicode16_v1_error_state_upper_bound(),
    ] {
        size = state_add(size, extra)?;
    }
    Ok(size)
}
fn search_lower_counts(
    input: &str,
    cap: usize,
    input_points: usize,
    output_points: usize,
    held: usize,
    budget: &OriginalStoreBudget<'_>,
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> Result<String> {
    if input.len() > cap {
        return Err(owned_err("legacy rank field byte cap"));
    }
    budget.check(deadline, probe)?;
    let available = (budget.remaining_after_retained)(held)?;
    let mut check = || {
        budget
            .check(deadline, probe)
            .and_then(|_| search_charge(budget, 4))
            .map_err(|_| {
                tos_foundation::FoundationError::new(
                    tos_foundation::FoundationErrorCode::BudgetExceeded,
                    "legacy original rank work/deadline",
                )
            })
    };
    tos_foundation::python_lower_unicode16_v1_with_state_budget_and_check(
        input,
        input_points,
        output_points,
        cap,
        available,
        &mut check,
    )
    .map_err(owned_err)
}
fn search_lower(
    input: &str,
    cap: usize,
    held: usize,
    budget: &OriginalStoreBudget<'_>,
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> Result<String> {
    search_lower_counts(input, cap, cap, cap, held, budget, deadline, probe)
}
fn search_strings(
    value: Option<&Value>,
    defaults: bool,
    held: &mut usize,
    budget: &OriginalStoreBudget<'_>,
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> Result<Vec<String>> {
    let values = match value {
        None | Some(Value::Null) => &[][..],
        Some(Value::Array(values)) => values.as_slice(),
        _ => return Err(owned_err("legacy query string filter must be an array")),
    };
    let mut count = 0usize;
    let mut bytes = 0usize;
    for value in values {
        budget.check(deadline, probe)?;
        search_charge(budget, 1)?;
        if let Some(value) = value.as_str().filter(|v| !v.is_empty()) {
            count = state_add(count, 1)?;
            bytes = state_add(bytes, value.len())?;
        }
    }
    let defaulted = defaults && count == 0;
    if defaulted {
        count = SOURCES.len();
        bytes = SOURCES.iter().map(|s| s.len()).sum();
    }
    let allocated = state_add(state_slots::<String>(count)?, bytes)?;
    (budget.remaining_after_retained)(state_add(*held, allocated)?)?;
    search_charge(budget, bytes)?;
    let mut output: Vec<String> = Vec::new();
    output.try_reserve_exact(count).map_err(owned_err)?;
    if output.capacity() != count {
        return Err(owned_err("legacy exact filter capacity"));
    }
    // Complete admitted key comparisons and slot movement before copying any
    // filter String. Exact-capacity sorted insertion needs no hidden sort
    // recursion or allocator and preserves the maintained sorted-unique set.
    search_charge(
        budget,
        count
            .checked_mul(count)
            .and_then(|n| n.checked_mul(bytes.max(std::mem::size_of::<String>())))
            .ok_or_else(|| owned_err("legacy filter ordering work overflow"))?,
    )?;
    let mut add = |value: &str| -> Result<()> {
        budget.check(deadline, probe)?;
        let (mut low, mut high) = (0usize, output.len());
        while low < high {
            budget.check(deadline, probe)?;
            let middle = low + (high - low) / 2;
            match output[middle].as_str().cmp(value) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Ok(()),
            }
        }
        output.insert(low, exact_string(value)?);
        Ok(())
    };
    (budget.remaining_after_retained)(state_add(
        state_add(*held, allocated)?,
        std::mem::size_of_val(&add),
    )?)?;
    if defaulted {
        for value in SOURCES {
            add(value)?;
        }
    } else {
        for value in values {
            if let Some(value) = value.as_str().filter(|s| !s.is_empty()) {
                add(value)?;
            }
        }
    }
    drop(add);
    *held = state_add(*held, allocated)?;
    Ok(output)
}
fn search_display_values(
    value: &Value,
    fields: &[&str],
    held: &mut usize,
    cap: usize,
    budget: &OriginalStoreBudget<'_>,
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> Result<Vec<String>> {
    let display = value.get("display").unwrap_or(&Value::Null);
    let mut count = 0usize;
    for name in fields {
        budget.check(deadline, probe)?;
        search_charge(budget, name.len())?;
        let field = display.get(*name).unwrap_or(&Value::Null);
        if field.is_string() {
            count = state_add(count, 1)?;
        } else if let Some(entries) = field.as_object() {
            for value in entries.values() {
                budget.check(deadline, probe)?;
                search_charge(budget, 1)?;
                if value.is_string() {
                    count = state_add(count, 1)?;
                }
            }
        }
    }
    let slots = state_slots::<String>(count)?;
    (budget.remaining_after_retained)(state_add(*held, slots)?)?;
    let mut output = Vec::new();
    output.try_reserve_exact(count).map_err(owned_err)?;
    if output.capacity() != count {
        return Err(owned_err("legacy exact rank vector capacity"));
    }
    *held = state_add(*held, slots)?;
    let mut bytes = 0usize;
    let mut add = |value: &str| -> Result<()> {
        let lowered = search_lower(value, cap, *held, budget, deadline, probe)?;
        bytes = state_add(bytes, lowered.len())?;
        if bytes > cap {
            return Err(owned_err("legacy rank display byte cap"));
        }
        *held = state_add(*held, lowered.capacity())?;
        output.push(lowered);
        Ok(())
    };
    for name in fields {
        let field = display.get(*name).unwrap_or(&Value::Null);
        if let Some(value) = field.as_str() {
            add(value)?;
        } else if let Some(entries) = field.as_object() {
            for value in entries.values() {
                if let Some(value) = value.as_str() {
                    add(value)?;
                }
            }
        }
    }
    Ok(output)
}
fn search_rank(
    value: &Value,
    needle: &str,
    relations: bool,
    held: usize,
    cap: usize,
    budget: &OriginalStoreBudget<'_>,
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> Result<(u8, String)> {
    let id = search_lower(field(value, "id"), cap, held, budget, deadline, probe)?;
    if needle.is_empty() {
        return Ok((3, id));
    }
    let mut held = state_add(held, id.capacity())?;
    let native = search_lower(
        field(value, "native_id"),
        cap,
        held,
        budget,
        deadline,
        probe,
    )?;
    held = state_add(held, native.capacity())?;
    let primary = search_display_values(
        value,
        if relations { &["label"] } else { &["title"] },
        &mut held,
        cap,
        budget,
        deadline,
        probe,
    )?;
    let visible = search_display_values(
        value,
        if relations {
            &["label", "inverse_label", "statement", "explanation"]
        } else {
            &["title", "kind_label", "summary"]
        },
        &mut held,
        cap,
        budget,
        deadline,
        probe,
    )?;
    // Equality/prefix/substring scans have their own byte work, in addition to
    // the checked Unicode mapping. String::contains uses no retained heap.
    let compared = primary
        .iter()
        .chain(visible.iter())
        .try_fold(state_add(id.len(), native.len())?, |sum, v| {
            state_add(sum, v.len())
        })?;
    let comparisons = state_add(
        compared,
        needle
            .len()
            .checked_mul(primary.len() + visible.len() + 4)
            .ok_or_else(|| owned_err("legacy rank comparison work overflow"))?,
    )?;
    search_charge(
        budget,
        comparisons
            .checked_mul(3)
            .ok_or_else(|| owned_err("legacy rank comparison work overflow"))?,
    )?;
    budget.check(deadline, probe)?;
    Ok((
        crate::knowledge_legacy_search::rank_lowered(&id, &native, &primary, &visible, needle),
        id,
    ))
}
fn search_object<const N: usize>(
    fields: [(&'static str, Value); N],
    held: &mut usize,
    budget: &OriginalStoreBudget<'_>,
) -> Result<Value> {
    let keys = fields
        .iter()
        .try_fold(0usize, |sum, (key, _)| state_add(sum, key.len()))?;
    let nodes = serde_map_nodes_upper_bound(N)?;
    let additional = state_add(keys, nodes)?;
    (budget.remaining_after_retained)(state_add(*held, additional)?)?;
    search_charge(
        budget,
        keys.checked_mul(N + 1)
            .ok_or_else(|| owned_err("legacy packet map work overflow"))?,
    )?;
    let mut map = Map::new();
    for (key, value) in fields {
        map.insert(exact_string(key)?, value);
    }
    *held = state_add(*held, additional)?;
    Ok(Value::Object(map))
}
fn search_string_values(
    values: Vec<String>,
    held: &mut usize,
    budget: &OriginalStoreBudget<'_>,
) -> Result<Vec<Value>> {
    let slots = state_slots::<Value>(values.len())?;
    (budget.remaining_after_retained)(state_add(*held, slots)?)?;
    let mut result = Vec::new();
    result.try_reserve_exact(values.len()).map_err(owned_err)?;
    if result.capacity() != values.len() {
        return Err(owned_err("legacy exact filter value capacity"));
    }
    *held = state_add(*held, slots)?;
    for value in values {
        result.push(Value::String(value));
    }
    Ok(result)
}
impl LegacyStore {
    /// Borrow an already admitted Foundation ingress DTO; no parser/visit grant.
    /// The caller retains that document through this entire operation, with
    /// its parser depth <=96 and all ingress tree/raw storage already admitted.
    pub fn search_packet_from_foundation_with_owned_budget(
        &mut self,
        args: &tos_foundation::JsonValue,
        budget: &OriginalStoreBudget<'_>,
        usage: &mut StoreUsage,
        call_deadline: Instant,
        operation: Arc<dyn AbortProbe>,
        max_output_bytes: usize,
    ) -> Result<Vec<u8>> {
        let result = (|| {
            self.verify_currentness_with_owned_budget(budget)?;
            if call_deadline > self.deadline
                || usage.rows != 0
                || usage.input_bytes != 0
                || usage.json_visits != 0
            {
                return Err(owned_err("legacy original borrowed search call/usage"));
            }
            let probe = OwnedBorrowedProbe {
                original: self.abort.as_ref(),
                operation: operation.as_ref(),
            };
            let check = || {
                budget.check(call_deadline, &probe)?;
                atomic_charge(&budget.byte_work, budget.max_byte_work, 1)?;
                atomic_charge(&budget.store_steps, budget.max_store_steps, 1)
            };
            let fixed = state_add(
                state_slots_bytes(
                    100,
                    state_add(
                        owned_tree_controller_frame_bytes(),
                        std::mem::size_of::<&tos_foundation::JsonValue>()
                            + std::mem::size_of::<std::slice::Iter<'_, tos_foundation::JsonValue>>(
                            )
                            + std::mem::size_of::<
                                std::slice::Iter<
                                    '_,
                                    (tos_foundation::JsonString, tos_foundation::JsonValue),
                                >,
                            >()
                            + std::mem::size_of::<usize>()
                            + std::mem::size_of::<u64>(),
                    )?,
                )?,
                state_add(
                    std::mem::size_of::<tos_foundation::JsonValue>(),
                    state_add(
                        std::mem::size_of::<OriginalStoreBudget<'_>>(),
                        state_add(
                            std::mem::size_of::<OwnedBorrowedProbe<'_>>(),
                            state_add(std::mem::size_of_val(&check), std::mem::size_of::<Value>())?,
                        )?,
                    )?,
                )?,
            )?;
            let carried = std::cell::Cell::new(fixed);
            let remaining = |additional| {
                (budget.remaining_after_retained)(state_add(additional, carried.get())?)
            };
            let fixed = state_add(
                fixed,
                state_add(
                    std::mem::size_of_val(&carried),
                    std::mem::size_of_val(&remaining),
                )?,
            )?;
            carried.set(fixed);
            remaining(self.retained_state_upper_bound()?)?;
            let converted = converted_storage_checked(args, &check)?;
            let work = conversion_work_checked(args, &check)?;
            atomic_charge(&budget.byte_work, budget.max_byte_work, work)?;
            atomic_charge(&budget.store_steps, budget.max_store_steps, work)?;
            carried.set(state_add(fixed, converted)?);
            remaining(self.retained_state_upper_bound()?)?;
            let value = convert_borrowed(args, &check)?;
            let original = OriginalStoreBudget {
                original_sqlite_heap: budget.original_sqlite_heap.clone(),
                remaining_after_retained: &remaining,
                byte_work: budget.byte_work.clone(),
                max_byte_work: budget.max_byte_work,
                sql_vm_steps: budget.sql_vm_steps.clone(),
                max_sql_vm_steps: budget.max_sql_vm_steps,
                store_sql_vm_steps: budget.store_sql_vm_steps.clone(),
                store_steps: budget.store_steps.clone(),
                max_store_steps: budget.max_store_steps,
                json_visits: budget.json_visits.clone(),
                max_json_visits: budget.max_json_visits,
                max_rows_remaining: budget.max_rows_remaining,
                max_input_bytes_remaining: budget.max_input_bytes_remaining,
            };
            self.search_packet_with_owned_budget(
                &value,
                &original,
                usage,
                call_deadline,
                operation.clone(),
                max_output_bytes,
            )
        })();
        self.work = budget.store_steps.load(Ordering::Relaxed);
        if result.is_err() {
            if let Some(owner) = &mut self.owned {
                owner.poisoned = true;
            }
        }
        result
    }
    /// Legacy offset search under the SAME Driver state/work/SQL/JSON owners.
    /// This does not authorize Node/Relation's navigation construction.
    pub fn search_packet_with_owned_budget(
        &mut self,
        args: &Value,
        budget: &OriginalStoreBudget<'_>,
        usage: &mut StoreUsage,
        call_deadline: Instant,
        operation: Arc<dyn AbortProbe>,
        max_output_bytes: usize,
    ) -> Result<Vec<u8>> {
        let result = (|| {
            if max_output_bytes == 0 || max_output_bytes > self.limits.max_json_bytes {
                return Err(owned_err("legacy search output cap"));
            }
            self.verify_currentness_with_owned_budget(budget)?;
            if call_deadline > self.deadline
                || usage.rows != 0
                || usage.input_bytes != 0
                || usage.json_visits != 0
            {
                return Err(owned_err("legacy original search call/usage"));
            }
            self.install_owned_progress(budget, call_deadline, Some(operation.clone()), 0)?;
            let result =
                self.search_owned_inner(args, budget, usage, call_deadline, operation.as_ref());
            let result_state = result.as_ref().map_or(0, |(_, state)| *state);
            let restored = self.install_owned_progress(budget, self.deadline, None, result_state);
            let (value, state) = result?;
            restored?;
            self.packet_from_owned_value_with_controller(
                value,
                state,
                budget,
                usage,
                call_deadline,
                operation.as_ref(),
                max_output_bytes,
                state_add(
                    owned_metadata_controller_state_upper_bound()?,
                    state_slots_bytes(34, owned_tree_controller_frame_bytes())?,
                )?,
            )
        })();
        self.work = budget.store_steps.load(Ordering::Relaxed);
        if result.is_err() {
            if let Some(owner) = &mut self.owned {
                owner.poisoned = true;
            }
        }
        result
    }
    fn search_owned_inner(
        &mut self,
        args: &Value,
        budget: &OriginalStoreBudget<'_>,
        usage: &mut StoreUsage,
        deadline: Instant,
        operation: &dyn AbortProbe,
    ) -> Result<(Value, usize)> {
        let probe = OwnedBorrowedProbe {
            original: self.abort.as_ref(),
            operation,
        };
        budget.check(deadline, &probe)?;
        let mut held = state_add(self.retained_state_upper_bound()?, owned_search_fixed()?)?;
        (budget.remaining_after_retained)(held)?;
        let fields = args
            .as_object()
            .ok_or_else(|| owned_err("legacy search arguments object"))?;
        const ALLOWED: &[&str] = &[
            "query",
            "sources",
            "kind_ids",
            "predicate_ids",
            "offset",
            "limit",
            "mode",
            "cursor",
        ];
        for key in fields.keys() {
            budget.check(deadline, &probe)?;
            search_charge(
                budget,
                key.len()
                    .checked_mul(ALLOWED.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(|| owned_err("legacy argument work overflow"))?,
            )?;
            if !ALLOWED.contains(&key.as_str()) {
                return Err(owned_err("legacy query unknown argument"));
            }
        }
        if args
            .get("mode")
            .is_some_and(|v| v.as_str() != Some("legacy"))
            || args.get("cursor").is_some_and(|v| !v.is_null())
        {
            return Err(owned_err(
                "selected legacy search requires legacy offset mode",
            ));
        }
        let query = args
            .get("query")
            .map(|v| {
                v.as_str()
                    .ok_or_else(|| owned_err("legacy search query string"))
            })
            .transpose()?
            .unwrap_or("");
        search_charge(budget, query.len())?;
        let query = tos_foundation::python_strip_unicode16_v1_with_check(
            query,
            self.limits.max_json_bytes,
            &mut || {
                budget
                    .check(deadline, &probe)
                    .and_then(|_| search_charge(budget, 1))
                    .map_err(|_| {
                        tos_foundation::FoundationError::new(
                            tos_foundation::FoundationErrorCode::BudgetExceeded,
                            "legacy original rank work/deadline",
                        )
                    })
            },
        )
        .map_err(owned_err)?;
        budget.check(deadline, &probe)?;
        let mut query_points = 0usize;
        for _ in query.chars() {
            budget.check(deadline, &probe)?;
            search_charge(budget, 1)?;
            query_points += 1;
            if query_points > 256 {
                return Err(owned_err("legacy search query bound"));
            }
        }
        (budget.remaining_after_retained)(state_add(held, query.len())?)?;
        let query = exact_string(query)?;
        held = state_add(held, query.capacity())?;
        let needle = search_lower_counts(&query, 4096, 256, 1024, held, budget, deadline, &probe)?;
        held = state_add(held, needle.capacity())?;
        let sources = search_strings(
            args.get("sources"),
            true,
            &mut held,
            budget,
            deadline,
            &probe,
        )?;
        if sources.iter().any(|s| !SOURCES.contains(&s.as_str())) {
            return Err(owned_err("unsupported knowledge source"));
        }
        let kinds = search_strings(
            args.get("kind_ids"),
            false,
            &mut held,
            budget,
            deadline,
            &probe,
        )?;
        let predicates = search_strings(
            args.get("predicate_ids"),
            false,
            &mut held,
            budget,
            deadline,
            &probe,
        )?;
        if kinds.len() > 100 || predicates.len() > 100 {
            return Err(owned_err("legacy search filter bound"));
        }
        let offset = search_integer(args.get("offset"), 0, 0, 100_000, Some(budget))?;
        let limit = search_integer(args.get("limit"), 40, 1, 100, Some(budget))?;
        if limit == 0 {
            return Err(owned_err("legacy search limit bound"));
        }
        let keep = state_add(offset, limit)?;
        let mut pages: [Vec<Value>; 2] = [Vec::new(), Vec::new()];
        let mut counts = [0u64; 2];
        for (kind, relations) in [false, true].into_iter().enumerate() {
            budget.check(deadline, &probe)?;
            let slots = state_slots::<OwnedHit>(keep)?;
            (budget.remaining_after_retained)(state_add(held, slots)?)?;
            let mut top: Vec<OwnedHit> = Vec::new();
            top.try_reserve_exact(keep).map_err(owned_err)?;
            if top.capacity() != keep {
                return Err(owned_err("legacy exact ranked vector capacity"));
            }
            let top_base = state_add(held, slots)?;
            let mut top_heap = 0usize;
            let sql = if relations {
                c"SELECT coalesce(source_graph,''),coalesce(predicate_id,''),coalesce(search_text,''),CASE WHEN typeof(payload)='text' AND length(CAST(payload AS BLOB))<=?1 THEN payload ELSE NULL END FROM knowledge_relations ORDER BY id"
            } else {
                c"SELECT coalesce(source_graph,''),coalesce(kind_id,''),coalesce(search_text,''),CASE WHEN typeof(payload)='text' AND length(CAST(payload AS BLOB))<=?1 THEN payload ELSE NULL END FROM knowledge_nodes ORDER BY id"
            };
            let mut statement = self.db.prepare_static_bounded(sql).map_err(owned_err)?;
            statement
                .bind_i64(1, self.limits.max_json_bytes as i64)
                .map_err(owned_err)?;
            while statement.step().map_err(owned_err)? {
                self.verify_currentness_with_owned_budget(budget)?;
                budget.check(deadline, &probe)?;
                usage.rows = usage
                    .rows
                    .checked_add(1)
                    .ok_or_else(|| owned_err("legacy row usage overflow"))?;
                if usage.rows > budget.max_rows_remaining {
                    return Err(owned_err("legacy original remaining rows"));
                }
                self.rows = self
                    .rows
                    .checked_add(1)
                    .filter(|n| *n <= self.limits.max_rows)
                    .ok_or_else(|| owned_err("legacy cumulative rows"))?;
                let filters = if relations { &predicates } else { &kinds };
                let selected_source = search_text(
                    &statement,
                    0,
                    budget,
                    deadline,
                    &probe,
                    state_add(top_base, top_heap)?,
                )?;
                let selected_category = search_text(
                    &statement,
                    1,
                    budget,
                    deadline,
                    &probe,
                    state_add(top_base, top_heap)?,
                )?;
                let selected_search = search_text(
                    &statement,
                    2,
                    budget,
                    deadline,
                    &probe,
                    state_add(top_base, top_heap)?,
                )?;
                let filter_work =
                    sources
                        .iter()
                        .chain(filters.iter())
                        .try_fold(0usize, |sum, s| {
                            state_add(
                                sum,
                                state_add(
                                    s.len(),
                                    state_add(selected_source.len(), selected_category.len())?,
                                )?,
                            )
                        })?;
                search_charge(
                    budget,
                    state_add(filter_work, state_add(selected_search.len(), needle.len())?)?,
                )?;
                if !sources.iter().any(|s| s == selected_source)
                    || !filters.is_empty() && !filters.iter().any(|s| s == selected_category)
                    || !needle.is_empty() && !selected_search.contains(&needle)
                {
                    continue;
                }
                let raw = search_text(
                    &statement,
                    3,
                    budget,
                    deadline,
                    &probe,
                    state_add(top_base, top_heap)?,
                )?;
                usage.input_bytes = usage
                    .input_bytes
                    .checked_add(raw.len() as u64)
                    .ok_or_else(|| owned_err("legacy input usage overflow"))?;
                if usage.input_bytes > budget.max_input_bytes_remaining {
                    return Err(owned_err("legacy original remaining input"));
                }
                self.bytes = self
                    .bytes
                    .checked_add(raw.len() as u64)
                    .filter(|n| *n <= self.limits.max_input_bytes)
                    .ok_or_else(|| owned_err("legacy cumulative input"))?;
                let base = state_add(top_base, top_heap)?;
                let (value, heap) = owned_parse(
                    raw.as_bytes(),
                    self.limits,
                    deadline,
                    &probe,
                    budget,
                    base,
                    usage,
                )?;
                object(&value)?;
                let source = field(&value, "source_graph");
                counts[kind] = counts[kind]
                    .checked_add(1)
                    .ok_or_else(|| owned_err("legacy search count overflow"))?;
                let row_state = state_add(std::mem::size_of::<Value>(), heap)?;
                let rank_base = state_add(base, row_state)?;
                let (rank, id) = search_rank(
                    &value,
                    &needle,
                    relations,
                    rank_base,
                    self.limits.max_json_bytes,
                    budget,
                    deadline,
                    &probe,
                )?;
                let key_heap = state_add(
                    id.capacity(),
                    state_add(source.len(), field(&value, "id").len())?,
                )?;
                (budget.remaining_after_retained)(state_add(rank_base, key_heap)?)?;
                search_charge(budget, key_heap)?;
                let key = (
                    rank,
                    id,
                    exact_string(source)?,
                    exact_string(field(&value, "id"))?,
                );
                search_charge(
                    budget,
                    top.len()
                        .checked_mul(state_add(state_add(key_heap, top_heap)?, 1)?)
                        .ok_or_else(|| owned_err("legacy ranked ordering work overflow"))?,
                )?;
                let position = top.partition_point(|(old, _, _)| old <= &key);
                if position >= keep {
                    continue;
                }
                // Pop before insert; capacity never grows beyond the admitted slots.
                if top.len() == keep {
                    top_heap = top_heap
                        .checked_sub(top.pop().unwrap().2)
                        .ok_or_else(|| owned_err("legacy ranked retained subtraction"))?;
                }
                let cost = state_add(heap, key_heap)?;
                top_heap = state_add(top_heap, cost)?;
                top.insert(position, (key, value, cost));
            }
            drop(statement);
            let length = top.len().saturating_sub(offset);
            let page_slots = state_slots::<Value>(length)?;
            (budget.remaining_after_retained)(state_add(
                state_add(top_base, top_heap)?,
                page_slots,
            )?)?;
            let mut page = Vec::new();
            page.try_reserve_exact(length).map_err(owned_err)?;
            if page.capacity() != length {
                return Err(owned_err("legacy exact page capacity"));
            }
            for (_, value, _) in top.into_iter().skip(offset) {
                page.push(value);
            }
            search_charge(budget, top_heap)?;
            let page_heap = page.iter().try_fold(page_slots, |sum, v| {
                state_add(
                    sum,
                    serde_clone_storage(v, &|| budget.check(deadline, &probe))?,
                )
            })?;
            held = state_add(held, page_heap)?;
            pages[kind] = page;
        }
        let empty_authority = Value::Object(Map::new());
        let authority = self
            .graph_header
            .get("authority_boundary")
            .unwrap_or(&empty_authority);
        let authority_planning = self
            .owned
            .as_ref()
            .ok_or_else(|| owned_err("legacy owned search owner absent"))?
            .metadata_storage
            .checked_mul(2)
            .ok_or_else(|| owned_err("legacy authority planning work overflow"))?;
        search_charge(budget, authority_planning)?;
        let authority_state = search_heap(authority, &|| budget.check(deadline, &probe))?;
        (budget.remaining_after_retained)(state_add(held, authority_state)?)?;
        search_charge(
            budget,
            serde_clone_work(authority, &|| budget.check(deadline, &probe))? as usize,
        )?;
        let authority = clone_serde_owned(authority, &|| budget.check(deadline, &probe))?;
        held = state_add(held, authority_state)?;
        // Source cut and exploration owner binding are distinct identities.
        let source_binding = self.source_revision()?;
        (budget.remaining_after_retained)(state_add(
            held,
            state_add(source_binding.len(), "tos_knowledge_search_v1".len())?,
        )?)?;
        search_charge(
            budget,
            state_add(source_binding.len(), "tos_knowledge_search_v1".len())?,
        )?;
        budget.check(deadline, &probe)?;
        // Keep both response strings admitted while the numeric token
        // reservation is added; the revision copy occurs after that admission.
        held = state_add(
            held,
            state_add(source_binding.len(), "tos_knowledge_search_v1".len())?,
        )?;
        // serde_json arbitrary_precision keeps decimal token strings even for
        // these six bounded unsigned output numbers. u64 max needs 20 bytes.
        let number_bytes = 6usize
            .checked_mul(20)
            .ok_or_else(|| owned_err("legacy output number state overflow"))?;
        (budget.remaining_after_retained)(state_add(held, number_bytes)?)?;
        search_charge(budget, number_bytes)?;
        held = state_add(held, number_bytes)?;
        let source_revision = exact_string(source_binding)?;
        if source_revision.capacity() != source_binding.len() {
            return Err(owned_err("legacy exact revision capacity"));
        }
        let filters = search_object(
            [
                (
                    "sources",
                    Value::Array(search_string_values(sources, &mut held, budget)?),
                ),
                (
                    "kind_ids",
                    Value::Array(search_string_values(kinds, &mut held, budget)?),
                ),
                (
                    "predicate_ids",
                    Value::Array(search_string_values(predicates, &mut held, budget)?),
                ),
            ],
            &mut held,
            budget,
        )?;
        let page = search_object(
            [
                ("offset", Value::from(offset)),
                ("limit_per_kind", Value::from(limit)),
            ],
            &mut held,
            budget,
        )?;
        let count = search_object(
            [
                ("matching_nodes", Value::from(counts[0])),
                ("matching_relations", Value::from(counts[1])),
                ("returned_nodes", Value::from(pages[0].len())),
                ("returned_relations", Value::from(pages[1].len())),
            ],
            &mut held,
            budget,
        )?;
        let [nodes, relations] = pages;
        let packet = search_object(
            [
                (
                    "schema",
                    Value::String(exact_string("tos_knowledge_search_v1")?),
                ),
                ("source_revision", Value::String(source_revision)),
                ("query", Value::String(query)),
                ("filters", filters),
                ("page", page),
                ("counts", count),
                ("nodes", Value::Array(nodes)),
                ("relations", Value::Array(relations)),
                ("authority_boundary", authority),
            ],
            &mut held,
            budget,
        )?;
        self.verify_currentness_with_owned_budget(budget)?;
        budget.check(deadline, &probe)?;
        search_charge(budget, held)?;
        let packet_state = search_heap(&packet, &|| budget.check(deadline, &probe))?;
        Ok((packet, packet_state))
    }
}
// Proposed additive QRY owner fragment for source_diagnostic_legacy.rs.
// It uses the held LegacyStore connection and the existing owned budget/currentness
// helpers. The caller supplies an already normalized request/filter sets and the
// selected Reference max_verify_chars value. This intentionally contains no
// WholeRoot/CMP source-cut or disclosure-policy authority.

const QUERY_STORE_INDEXED_MAX_CANDIDATES: u64 = 50_000;

#[derive(Debug)]
struct OwnedQueryStoreRankedRow {
    order: (u8, String, u64),
    payload: Value,
    payload_retained: usize,
    retained: usize,
}

#[derive(Debug)]
struct OwnedQueryStorePage {
    rows: Vec<OwnedQueryStoreRankedRow>,
    candidate_rows: u64,
    verified_chars: u64,
    has_more: bool,
    next_after: Option<(u8, String, u64)>,
    sql_pages: u64,
    retained: usize,
}

// QueryStore's compiler emits a complete contentless FTS5 trigram table. Keep
// all AND terms in the MATCH expression; exact substring matching follows in
// SQL, just as QueryStore.ranked_page does. Repeated grams are harmless and
// avoid a second deduplication allocation.
fn owned_query_store_fts_expression(
    needle: &str,
    held: usize,
    budget: &OriginalStoreBudget<'_>,
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> Result<(String, usize)> {
    let count = needle.chars().count();
    if count < 3 || needle.contains('\0') {
        return Err(owned_err(
            "indexed QueryStore requires a non-NUL three-character query",
        ));
    }
    let terms = count - 2;
    let capacity = terms
        .checked_mul(14)
        .and_then(|n| n.checked_add(terms.saturating_sub(1).checked_mul(5)?))
        .ok_or_else(|| owned_err("indexed QueryStore FTS expression bound"))?;
    let offsets_bytes = state_slots::<(usize, char)>(count)?;
    (budget.remaining_after_retained)(state_add(held, state_add(capacity, offsets_bytes)?)?)?;
    search_charge(budget, count)?;
    let mut offsets = Vec::new();
    offsets.try_reserve_exact(count).map_err(owned_err)?;
    if offsets.capacity() != count {
        return Err(owned_err("indexed QueryStore exact FTS offsets"));
    }
    offsets.extend(needle.char_indices());
    let mut expression = String::new();
    expression.try_reserve_exact(capacity).map_err(owned_err)?;
    if expression.capacity() != capacity {
        return Err(owned_err("indexed QueryStore exact FTS expression"));
    }
    for index in 0..terms {
        budget.check(deadline, probe)?;
        if index != 0 {
            expression.push_str(" AND ");
        }
        expression.push('"');
        for (_, character) in &offsets[index..index + 3] {
            budget.check(deadline, probe)?;
            if *character == '"' {
                expression.push_str("\"\"");
            } else {
                expression.push(*character);
            }
        }
        expression.push('"');
    }
    let retained = state_add(expression.capacity(), offsets_bytes)?;
    Ok((expression, retained))
}

fn owned_query_store_json_strings(
    values: &[String],
    held: usize,
    budget: &OriginalStoreBudget<'_>,
) -> Result<(String, usize)> {
    // serde_json escapes each input byte by at most six bytes; this is a
    // pre-allocation ceiling, then we retain the actual String capacity.
    let capacity = values.iter().try_fold(2usize, |sum, value| {
        value
            .len()
            .checked_mul(6)
            .and_then(|bytes| sum.checked_add(bytes))
            .and_then(|n| n.checked_add(3))
            .ok_or_else(|| owned_err("indexed QueryStore filter JSON bound"))
    })?;
    (budget.remaining_after_retained)(state_add(held, capacity)?)?;
    search_charge(budget, capacity)?;
    let mut encoded = Vec::new();
    encoded.try_reserve_exact(capacity).map_err(owned_err)?;
    if encoded.capacity() != capacity {
        return Err(owned_err("indexed QueryStore exact filter JSON capacity"));
    }
    serde_json::to_writer(&mut encoded, values).map_err(owned_err)?;
    if encoded.len() > capacity {
        return Err(owned_err(
            "indexed QueryStore filter JSON exceeds admission",
        ));
    }
    let encoded = String::from_utf8(encoded).map_err(owned_err)?;
    let retained = encoded.capacity();
    Ok((encoded, retained))
}

impl LegacyStore {
    /// One genuine QueryStore indexed kind page. Candidate caps are applied
    /// after source/kind filters and to the complete all-grams FTS candidate
    /// set, before exact substring verification. max_verify_chars is per-kind,
    /// matching the two independent Reference ranked_page calls.
    fn ranked_query_store_kind_page_owned(
        &mut self,
        relations: bool,
        sources_json: &str,
        filters_json: &str,
        fts_expression: &str,
        needle: &str,
        after: Option<&(u8, String, u64)>,
        page_size: usize,
        max_verify_chars: u64,
        budget: &OriginalStoreBudget<'_>,
        usage: &mut StoreUsage,
        deadline: Instant,
        probe: &dyn AbortProbe,
        held: usize,
    ) -> Result<OwnedQueryStorePage> {
        if page_size == 0 || page_size > 100 || max_verify_chars == 0 || !self.search_indexed_fts5 {
            return Err(owned_err(
                "selected QueryStore has no admitted indexed page",
            ));
        }
        self.verify_currentness_with_owned_budget(budget)?;

        let (preflight_sql, page_sql) = if relations {
            (
                c"SELECT count(*),coalesce(sum(n),0),count(n) FROM (SELECT CASE WHEN typeof(search_text)='text' THEN length(search_text) END AS n FROM knowledge_relations WHERE source_graph IN (SELECT value FROM json_each(?1)) AND (?2='[]' OR predicate_id IN (SELECT value FROM json_each(?2))) AND rowid IN (SELECT rowid FROM knowledge_relations_trigram WHERE knowledge_relations_trigram MATCH ?3) LIMIT ?4)",
                c"SELECT rowid-1,CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=?5 THEN id END,CASE WHEN typeof(source_graph)='text' AND length(CAST(source_graph AS BLOB))<=?5 THEN source_graph END,CASE WHEN typeof(predicate_id)='text' AND length(CAST(predicate_id AS BLOB))<=?5 THEN predicate_id END,CASE WHEN typeof(payload)='text' AND length(CAST(payload AS BLOB))<=?5 THEN payload END FROM knowledge_relations WHERE source_graph IN (SELECT value FROM json_each(?1)) AND (?2='[]' OR predicate_id IN (SELECT value FROM json_each(?2))) AND rowid IN (SELECT rowid FROM knowledge_relations_trigram WHERE knowledge_relations_trigram MATCH ?3) AND instr(search_text,?4)>0 ORDER BY rowid",
            )
        } else {
            (
                c"SELECT count(*),coalesce(sum(n),0),count(n) FROM (SELECT CASE WHEN typeof(search_text)='text' THEN length(search_text) END AS n FROM knowledge_nodes WHERE source_graph IN (SELECT value FROM json_each(?1)) AND (?2='[]' OR kind_id IN (SELECT value FROM json_each(?2))) AND rowid IN (SELECT rowid FROM knowledge_nodes_trigram WHERE knowledge_nodes_trigram MATCH ?3) LIMIT ?4)",
                c"SELECT rowid-1,CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=?5 THEN id END,CASE WHEN typeof(source_graph)='text' AND length(CAST(source_graph AS BLOB))<=?5 THEN source_graph END,CASE WHEN typeof(kind_id)='text' AND length(CAST(kind_id AS BLOB))<=?5 THEN kind_id END,CASE WHEN typeof(payload)='text' AND length(CAST(payload AS BLOB))<=?5 THEN payload END FROM knowledge_nodes WHERE source_graph IN (SELECT value FROM json_each(?1)) AND (?2='[]' OR kind_id IN (SELECT value FROM json_each(?2))) AND rowid IN (SELECT rowid FROM knowledge_nodes_trigram WHERE knowledge_nodes_trigram MATCH ?3) AND instr(search_text,?4)>0 ORDER BY rowid",
            )
        };

        // Reference QueryStore.ranked_page uses LIMIT max_candidates+1 for
        // preflight. Aggregate transfer remains one SQL row; original SQLite
        // VM/work counters stay installed by the caller on this same connection.
        let mut preflight = self
            .db
            .prepare_static_bounded(preflight_sql)
            .map_err(owned_err)?;
        preflight.bind_text(1, sources_json).map_err(owned_err)?;
        preflight.bind_text(2, filters_json).map_err(owned_err)?;
        preflight.bind_text(3, fts_expression).map_err(owned_err)?;
        preflight
            .bind_i64(4, (QUERY_STORE_INDEXED_MAX_CANDIDATES + 1) as i64)
            .map_err(owned_err)?;
        if !preflight.step().map_err(owned_err)? {
            return Err(owned_err("indexed QueryStore preflight row absent"));
        }
        let candidate_rows = preflight.unsigned_integer(0).map_err(owned_err)?;
        let verified_chars = preflight.unsigned_integer(1).map_err(owned_err)?;
        let valid_text_rows = preflight.unsigned_integer(2).map_err(owned_err)?;
        if preflight.step().map_err(owned_err)? {
            return Err(owned_err(
                "indexed QueryStore preflight returned extra rows",
            ));
        }
        drop(preflight);
        if candidate_rows > QUERY_STORE_INDEXED_MAX_CANDIDATES
            || valid_text_rows != candidate_rows
            || verified_chars > max_verify_chars
        {
            return Err(owned_err(
                "indexed QueryStore candidate/verification budget exceeded",
            ));
        }
        usage.rows = usage
            .rows
            .checked_add(candidate_rows)
            .filter(|rows| *rows <= budget.max_rows_remaining)
            .ok_or_else(|| owned_err("indexed QueryStore original remaining rows"))?;
        self.rows = self
            .rows
            .checked_add(candidate_rows)
            .filter(|rows| *rows <= self.limits.max_rows)
            .ok_or_else(|| owned_err("indexed QueryStore cumulative rows"))?;

        let keep = page_size
            .checked_add(1)
            .ok_or_else(|| owned_err("indexed QueryStore page rows"))?;
        let slots = state_slots::<OwnedQueryStoreRankedRow>(keep)?;
        (budget.remaining_after_retained)(state_add(held, slots)?)?;
        let mut top: Vec<OwnedQueryStoreRankedRow> = Vec::new();
        top.try_reserve_exact(keep).map_err(owned_err)?;
        if top.capacity() != keep {
            return Err(owned_err("indexed QueryStore exact rank vector"));
        }
        let mut top_heap = 0usize;
        let mut statement = self
            .db
            .prepare_static_bounded(page_sql)
            .map_err(owned_err)?;
        statement.bind_text(1, sources_json).map_err(owned_err)?;
        statement.bind_text(2, filters_json).map_err(owned_err)?;
        statement.bind_text(3, fts_expression).map_err(owned_err)?;
        statement.bind_text(4, needle).map_err(owned_err)?;
        statement
            .bind_i64(5, self.limits.max_json_bytes as i64)
            .map_err(owned_err)?;
        while statement.step().map_err(owned_err)? {
            self.verify_currentness_with_owned_budget(budget)?;
            budget.check(deadline, probe)?;
            let position = statement.unsigned_integer(0).map_err(owned_err)?;
            let selected_id = search_text(
                &statement,
                1,
                budget,
                deadline,
                probe,
                state_add(held, state_add(slots, top_heap)?)?,
            )?;
            let selected_source = search_text(
                &statement,
                2,
                budget,
                deadline,
                probe,
                state_add(held, state_add(slots, top_heap)?)?,
            )?;
            let selected_category = search_text(
                &statement,
                3,
                budget,
                deadline,
                probe,
                state_add(held, state_add(slots, top_heap)?)?,
            )?;
            let raw = search_text(
                &statement,
                4,
                budget,
                deadline,
                probe,
                state_add(held, state_add(slots, top_heap)?)?,
            )?;
            usage.input_bytes = usage
                .input_bytes
                .checked_add(raw.len() as u64)
                .filter(|bytes| *bytes <= budget.max_input_bytes_remaining)
                .ok_or_else(|| owned_err("indexed QueryStore original input bytes"))?;
            self.bytes = self
                .bytes
                .checked_add(raw.len() as u64)
                .filter(|bytes| *bytes <= self.limits.max_input_bytes)
                .ok_or_else(|| owned_err("indexed QueryStore cumulative input bytes"))?;
            let base = state_add(held, state_add(slots, top_heap)?)?;
            let (value, heap) = owned_parse(
                raw.as_bytes(),
                self.limits,
                deadline,
                probe,
                budget,
                base,
                usage,
            )?;
            object(&value)?;
            if field(&value, "id") != selected_id
                || field(&value, "source_graph") != selected_source
                || field(&value, if relations { "predicate_id" } else { "kind_id" })
                    != selected_category
            {
                return Err(owned_err("indexed QueryStore row/payload identity differs"));
            }
            let (rank, lower_id) = search_rank(
                &value,
                needle,
                relations,
                state_add(base, heap)?,
                self.limits.max_json_bytes,
                budget,
                deadline,
                probe,
            )?;
            let key_heap = state_add(
                lower_id.capacity(),
                std::mem::size_of::<(u8, String, u64)>(),
            )?;
            (budget.remaining_after_retained)(state_add(
                state_add(base, state_add(heap, key_heap)?)?,
                std::mem::size_of_val(&lower_id),
            )?)?;
            let key = (rank, lower_id, position);
            if after.is_some_and(|previous| {
                (key.0, key.1.as_str(), key.2) <= (previous.0, previous.1.as_str(), previous.2)
            }) {
                continue;
            }
            search_charge(
                budget,
                top.len()
                    .checked_mul(state_add(key_heap, top_heap)?)
                    .ok_or_else(|| owned_err("indexed QueryStore rank order work"))?,
            )?;
            let insertion = top.partition_point(|old| old.order <= key);
            if insertion >= keep {
                continue;
            }
            if top.len() == keep {
                top_heap = top_heap
                    .checked_sub(top.pop().unwrap().retained)
                    .ok_or_else(|| owned_err("indexed QueryStore rank state subtraction"))?;
            }
            let retained = state_add(heap, key_heap)?;
            top_heap = state_add(top_heap, retained)?;
            top.insert(
                insertion,
                OwnedQueryStoreRankedRow {
                    order: key,
                    payload: value,
                    payload_retained: heap,
                    retained,
                },
            );
        }
        drop(statement);
        let has_more = top.len() > page_size;
        let selected = top.len().min(page_size);
        let (next_after, next_after_retained) = if has_more {
            let key = &top[selected - 1].order;
            let requested = state_add(std::mem::size_of::<(u8, String, u64)>(), key.1.len())?;
            (budget.remaining_after_retained)(state_add(
                state_add(held, slots)?,
                state_add(top_heap, requested)?,
            )?)?;
            let lower_id = exact_string(&key.1)?;
            let retained = state_add(
                std::mem::size_of::<(u8, String, u64)>(),
                lower_id.capacity(),
            )?;
            (budget.remaining_after_retained)(state_add(
                state_add(held, slots)?,
                state_add(top_heap, retained)?,
            )?)?;
            (Some((key.0, lower_id, key.2)), retained)
        } else {
            (None, 0)
        };
        let result_slots = state_slots::<OwnedQueryStoreRankedRow>(selected)?;
        (budget.remaining_after_retained)(state_add(
            state_add(held, slots)?,
            state_add(state_add(top_heap, result_slots)?, next_after_retained)?,
        )?)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(selected).map_err(owned_err)?;
        if rows.capacity() != selected {
            return Err(owned_err("indexed QueryStore exact selected rows"));
        }
        for row in top.drain(..selected) {
            rows.push(row);
        }
        Ok(OwnedQueryStorePage {
            rows,
            candidate_rows,
            verified_chars,
            has_more,
            next_after,
            sql_pages: 2,
            retained: state_add(state_add(top_heap, result_slots)?, next_after_retained)?,
        })
    }
}

/// Normalized input for the ordinary Reference-compatible QueryStore route.
/// The caller owns Foundation parsing and supplies the ordinary public filter
/// arrays; this QRY owner revalidates and canonicalizes them before SQL.
pub struct QueryStoreIndexedSearchRequest<'a> {
    pub query: &'a str,
    pub sources: &'a [String],
    pub kind_ids: &'a [String],
    pub predicate_ids: &'a [String],
    pub limit_per_kind: usize,
}

/// Child cursor values decoded from the Reference outer envelope. The child
/// strings still bind the exact selected Store and graph-header cut here.
pub struct QueryStoreIndexedContinuation<'a> {
    pub cursor_present: bool,
    pub nodes: Option<&'a str>,
    pub relations: Option<&'a str>,
    pub nodes_exhausted: bool,
    pub relations_exhausted: bool,
}

pub struct QueryStoreIndexedFilters {
    pub sources: Vec<String>,
    pub kind_ids: Vec<String>,
    pub predicate_ids: Vec<String>,
}

pub struct QueryStoreIndexedKindPage {
    pub rows: Vec<Value>,
    pub candidate_rows: u64,
    pub verified_chars: u64,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    pub sql_pages: u64,
    pub retained_bytes: usize,
}

/// Actual weak-store data only. This carries the authenticated Store revision
/// and graph-header source revision; it does not claim a CMP selection/cut.
pub struct QueryStoreIndexedPage {
    pub store_revision: String,
    pub source_revision: Option<String>,
    pub normalized_query: String,
    pub filters: QueryStoreIndexedFilters,
    pub nodes: QueryStoreIndexedKindPage,
    pub relations: QueryStoreIndexedKindPage,
    pub has_more: bool,
    pub retained_bytes: usize,
}

const QUERY_STORE_INDEXED_CURSOR_SCHEMA: &str = "tos_knowledge_search_cursor_v1";
const QUERY_STORE_INDEXED_CURSOR_BACKEND: &str = "compiled-fts5-v1";
const QUERY_STORE_INDEXED_CURSOR_MAX_BYTES: usize = 2048;
const QUERY_STORE_INDEXED_CURSOR_TTL_SECONDS: u64 = 900;
const QUERY_STORE_INDEXED_FILTER_FIELD_BYTES: usize = 6 * 1024;

fn indexed_store_filter(
    values: &[String],
    allow_empty: bool,
    total_values: &mut usize,
    field_bytes: &mut usize,
    held: usize,
    budget: &OriginalStoreBudget<'_>,
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> Result<(Vec<String>, usize)> {
    *total_values = total_values
        .checked_add(values.len())
        .filter(|count| *count <= 100)
        .ok_or_else(|| owned_err("indexed QueryStore filter count"))?;
    let slots = state_slots::<String>(values.len())?;
    let mut bytes = 0usize;
    for value in values {
        budget.check(deadline, probe)?;
        search_charge(
            budget,
            value
                .len()
                .checked_add(1)
                .ok_or_else(|| owned_err("indexed QueryStore filter work"))?,
        )?;
        if (!allow_empty && value.is_empty()) || value.chars().count() > 256 {
            return Err(owned_err("indexed QueryStore filter value bound"));
        }
        bytes = state_add(bytes, value.len())?;
    }
    *field_bytes = state_add(*field_bytes, bytes)?;
    if *field_bytes > QUERY_STORE_INDEXED_FILTER_FIELD_BYTES {
        return Err(owned_err("indexed QueryStore filter byte bound"));
    }
    let allocation = state_add(slots, bytes)?;
    (budget.remaining_after_retained)(state_add(held, allocation)?)?;
    let mut levels = 0usize;
    let mut width = 1usize;
    while width < values.len() {
        width = width
            .checked_mul(2)
            .ok_or_else(|| owned_err("indexed QueryStore filter ordering work"))?;
        levels = levels
            .checked_add(1)
            .ok_or_else(|| owned_err("indexed QueryStore filter ordering work"))?;
    }
    let comparisons = values
        .len()
        .checked_mul(levels)
        .and_then(|n| n.checked_mul(bytes.max(1)))
        .ok_or_else(|| owned_err("indexed QueryStore filter ordering work"))?;
    search_charge(budget, comparisons)?;
    let mut output = Vec::new();
    output.try_reserve_exact(values.len()).map_err(owned_err)?;
    if output.capacity() != values.len() {
        return Err(owned_err("indexed QueryStore exact filter capacity"));
    }
    for value in values {
        budget.check(deadline, probe)?;
        output.push(exact_string(value)?);
    }
    output.sort_unstable();
    output.dedup();
    let output_bytes = output
        .iter()
        .try_fold(0usize, |sum, value| state_add(sum, value.capacity()))?;
    Ok((output, state_add(slots, output_bytes)?))
}

fn indexed_store_query(
    value: &str,
    held: usize,
    budget: &OriginalStoreBudget<'_>,
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> Result<(String, usize)> {
    let mut code_points = 0usize;
    for _ in value.chars() {
        budget.check(deadline, probe)?;
        search_charge(budget, 1)?;
        code_points += 1;
        if code_points > 256 {
            return Err(owned_err("indexed QueryStore query length"));
        }
    }
    search_charge(budget, value.len())?;
    (budget.remaining_after_retained)(state_add(held, value.len())?)?;
    search_charge(budget, value.len())?;
    let stripped = tos_foundation::python_strip_unicode16_v1(value, 256).map_err(owned_err)?;
    // Strip borrows the request bytes. Admit only the live slice controller;
    // the lower owner accounts its own allocations and the retained output.
    let stripped_state = std::mem::size_of_val(&stripped);
    (budget.remaining_after_retained)(state_add(held, stripped_state)?)?;
    let query = search_lower_counts(
        stripped,
        1024,
        256,
        1024,
        state_add(held, stripped_state)?,
        budget,
        deadline,
        probe,
    )?;
    if query.chars().count() < 3 || query.contains('\0') {
        return Err(owned_err(
            "indexed QueryStore requires a non-NUL three-character query",
        ));
    }
    let retained = query.capacity();
    Ok((query, retained))
}

fn query_store_filter_digest(
    sources: &[String],
    kind_ids: &[String],
    predicate_ids: &[String],
    held: usize,
    budget: &OriginalStoreBudget<'_>,
) -> Result<String> {
    let bytes = sources
        .iter()
        .chain(kind_ids)
        .chain(predicate_ids)
        .try_fold(512usize, |sum, value| {
            value
                .len()
                .checked_mul(6)
                .and_then(|bytes| sum.checked_add(bytes))
                .and_then(|n| n.checked_add(3))
                .ok_or_else(|| owned_err("indexed QueryStore cursor digest bound"))
        })?;
    // serde_json's Vec writer may retain up to twice its final length while
    // growing. Admit that temporary capacity as well as the small map nodes.
    let allocation = state_add(
        bytes
            .checked_mul(2)
            .ok_or_else(|| owned_err("indexed QueryStore cursor digest allocation"))?,
        512,
    )?;
    (budget.remaining_after_retained)(state_add(held, allocation)?)?;
    search_charge(budget, bytes)?;
    let mut value = std::collections::BTreeMap::new();
    value.insert("sources", sources);
    value.insert("kind_ids", kind_ids);
    value.insert("predicate_ids", predicate_ids);
    let raw = serde_json::to_vec(&value).map_err(owned_err)?;
    if raw.len() > bytes {
        return Err(owned_err(
            "indexed QueryStore cursor digest exceeds admission",
        ));
    }
    let digest = tos_foundation::Digest256::of_bytes(&raw).to_hex();
    Ok(digest)
}

fn query_store_cursor_now() -> Result<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| owned_err("indexed QueryStore cursor clock"))
}

fn query_store_cursor_decode(
    token: &str,
    kind: &str,
    store_revision: &str,
    source_revision: Option<&str>,
    query: &str,
    filters_digest: &str,
    held: usize,
    budget: &OriginalStoreBudget<'_>,
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> Result<(u8, String, u64)> {
    if token.is_empty() || token.len() > QUERY_STORE_INDEXED_CURSOR_MAX_BYTES {
        return Err(owned_err("indexed QueryStore cursor length"));
    }
    budget.check(deadline, probe)?;
    search_charge(budget, token.len())?;
    // The decoded Value tree, its owned strings, and the base64 buffer coexist
    // until the bound is checked. The raw token is capped at 2 KiB, but its
    // map nodes and allocator slack need their own admission.
    let reservation = QUERY_STORE_INDEXED_CURSOR_MAX_BYTES
        .checked_mul(4)
        .ok_or_else(|| owned_err("indexed QueryStore cursor decode admission"))?;
    (budget.remaining_after_retained)(state_add(held, reservation)?)?;
    search_charge(budget, reservation)?;
    // The maintained Python URL-safe decoder accepts ordinary '=' padding;
    // emit unpadded tokens but accept that same spelling on input.
    let padding = token.bytes().rev().take_while(|byte| *byte == b'=').count();
    if padding > 2 || token[..token.len() - padding].contains('=') {
        return Err(owned_err("indexed QueryStore cursor padding"));
    }
    let unpadded = &token[..token.len() - padding];
    if padding != 0 && (token.len() % 4 != 0 || unpadded.len() % 4 == 0) {
        return Err(owned_err("indexed QueryStore cursor padding"));
    }
    let raw = crate::compressed_search_state::base64_decode(unpadded).map_err(owned_err)?;
    if raw.len() > QUERY_STORE_INDEXED_CURSOR_MAX_BYTES {
        return Err(owned_err("indexed QueryStore cursor decoded length"));
    }
    let value: Value = serde_json::from_slice(&raw).map_err(owned_err)?;
    let fields = value
        .as_object()
        .ok_or_else(|| owned_err("indexed QueryStore cursor object"))?;
    const KEYS: &[&str] = &[
        "schema",
        "backend",
        "kind",
        "store_revision",
        "source_revision",
        "query",
        "filters_digest",
        "after",
        "expires_at",
    ];
    if fields.len() != KEYS.len() || fields.keys().any(|key| !KEYS.contains(&key.as_str())) {
        return Err(owned_err("indexed QueryStore cursor members"));
    }
    if field(&value, "schema") != QUERY_STORE_INDEXED_CURSOR_SCHEMA
        || field(&value, "backend") != QUERY_STORE_INDEXED_CURSOR_BACKEND
        || field(&value, "kind") != kind
    {
        return Err(owned_err("indexed QueryStore cursor schema/backend"));
    }
    let source_revision_matches = match (value.get("source_revision"), source_revision) {
        (Some(Value::Null), None) => true,
        (Some(Value::String(actual)), Some(expected)) => actual.as_str() == expected,
        _ => false,
    };
    if field(&value, "store_revision") != store_revision || !source_revision_matches {
        return Err(owned_err("indexed QueryStore cursor snapshot changed"));
    }
    if field(&value, "query") != query || field(&value, "filters_digest") != filters_digest {
        return Err(owned_err(
            "indexed QueryStore cursor query or filters changed",
        ));
    }
    let expires = value
        .get("expires_at")
        .and_then(Value::as_u64)
        .ok_or_else(|| owned_err("indexed QueryStore cursor expiry"))?;
    if expires < query_store_cursor_now()? {
        return Err(owned_err("indexed QueryStore cursor expired"));
    }
    let after = value
        .get("after")
        .and_then(Value::as_array)
        .filter(|after| after.len() == 3)
        .ok_or_else(|| owned_err("indexed QueryStore cursor position"))?;
    let rank = after[0]
        .as_u64()
        .filter(|rank| *rank <= 3)
        .ok_or_else(|| owned_err("indexed QueryStore cursor rank"))? as u8;
    let id = after[1]
        .as_str()
        .ok_or_else(|| owned_err("indexed QueryStore cursor id"))?;
    let position = after[2]
        .as_u64()
        .filter(|position| *position <= i64::MAX as u64)
        .ok_or_else(|| owned_err("indexed QueryStore cursor row position"))?;
    let decode_held = state_add(held, reservation)?;
    let normalized_id =
        search_lower_counts(id, 8192, 2048, 8192, decode_held, budget, deadline, probe)?;
    if normalized_id != id {
        return Err(owned_err("indexed QueryStore cursor id is not lowercase"));
    }
    let copy_state = state_add(
        state_add(normalized_id.capacity(), id.len())?,
        std::mem::size_of::<(u8, String, u64)>(),
    )?;
    (budget.remaining_after_retained)(state_add(decode_held, copy_state)?)?;
    let id = exact_string(id)?;
    Ok((rank, id, position))
}

fn query_store_cursor_json_string_size(value: &str) -> Result<usize> {
    let mut size = 2usize; // surrounding quotes
    for character in value.chars() {
        let bytes = match character {
            '"' | '\\' | '\u{0008}' | '\u{000c}' | '\n' | '\r' | '\t' => 2,
            '\u{0000}'..='\u{001f}' => 6,
            _ => character.len_utf8(),
        };
        size = state_add(size, bytes)?;
    }
    Ok(size)
}

fn query_store_cursor_raw_bound(
    kind: &str,
    store_revision: &str,
    source_revision: Option<&str>,
    query: &str,
    filters_digest: &str,
    after: &(u8, String, u64),
    expires: u64,
) -> Result<usize> {
    // Exact compact-json size in sorted-key order, including JSON escaping.
    // Checking this before cloning any binding strings prevents an oversized
    // graph header or row id from creating an unbounded cursor allocation.
    let pairs = [
        (
            "after",
            state_add(
                5, // [, rank, two commas, and ]
                state_add(
                    query_store_cursor_json_string_size(&after.1)?,
                    after.2.to_string().len(),
                )?,
            )?,
        ),
        (
            "backend",
            query_store_cursor_json_string_size(QUERY_STORE_INDEXED_CURSOR_BACKEND)?,
        ),
        ("expires_at", expires.to_string().len()),
        (
            "filters_digest",
            query_store_cursor_json_string_size(filters_digest)?,
        ),
        ("kind", query_store_cursor_json_string_size(kind)?),
        ("query", query_store_cursor_json_string_size(query)?),
        (
            "schema",
            query_store_cursor_json_string_size(QUERY_STORE_INDEXED_CURSOR_SCHEMA)?,
        ),
        (
            "source_revision",
            source_revision.map_or(Ok(4), query_store_cursor_json_string_size)?,
        ),
        (
            "store_revision",
            query_store_cursor_json_string_size(store_revision)?,
        ),
    ];
    let mut size = 2usize; // object braces
    for (index, (key, value)) in pairs.iter().enumerate() {
        if index != 0 {
            size = state_add(size, 1)?;
        }
        size = state_add(
            size,
            state_add(
                query_store_cursor_json_string_size(key)?,
                state_add(1, *value)?,
            )?,
        )?;
    }
    Ok(size)
}

fn query_store_cursor_encode(
    kind: &str,
    store_revision: &str,
    source_revision: Option<&str>,
    query: &str,
    filters_digest: &str,
    after: &(u8, String, u64),
    held: usize,
    budget: &OriginalStoreBudget<'_>,
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> Result<String> {
    budget.check(deadline, probe)?;
    let expires = query_store_cursor_now()?
        .checked_add(QUERY_STORE_INDEXED_CURSOR_TTL_SECONDS)
        .ok_or_else(|| owned_err("indexed QueryStore cursor expiry overflow"))?;
    let raw_bound = query_store_cursor_raw_bound(
        kind,
        store_revision,
        source_revision,
        query,
        filters_digest,
        after,
        expires,
    )?;
    // 1,536 raw bytes encode to the 2 KiB Python token limit without padding.
    const MAX_CURSOR_RAW_BYTES: usize = QUERY_STORE_INDEXED_CURSOR_MAX_BYTES * 3 / 4;
    if raw_bound > MAX_CURSOR_RAW_BYTES {
        return Err(owned_err(
            "indexed QueryStore cursor exceeds Reference bound",
        ));
    }
    let reservation = QUERY_STORE_INDEXED_CURSOR_MAX_BYTES
        .checked_mul(4)
        .ok_or_else(|| owned_err("indexed QueryStore cursor allocation bound"))?;
    (budget.remaining_after_retained)(state_add(held, reservation)?)?;
    search_charge(budget, reservation)?;
    let mut value = std::collections::BTreeMap::new();
    value.insert("schema", Value::from(QUERY_STORE_INDEXED_CURSOR_SCHEMA));
    value.insert("backend", Value::from(QUERY_STORE_INDEXED_CURSOR_BACKEND));
    value.insert("kind", Value::from(kind));
    value.insert("store_revision", Value::from(store_revision));
    value.insert(
        "source_revision",
        source_revision.map_or(Value::Null, Value::from),
    );
    value.insert("query", Value::from(query));
    value.insert("filters_digest", Value::from(filters_digest));
    value.insert(
        "after",
        Value::Array(vec![
            Value::from(after.0),
            Value::from(after.1.as_str()),
            Value::from(after.2),
        ]),
    );
    value.insert("expires_at", Value::from(expires));
    let mut raw = Vec::new();
    raw.try_reserve_exact(raw_bound).map_err(owned_err)?;
    if raw.capacity() != raw_bound {
        return Err(owned_err("indexed QueryStore exact cursor capacity"));
    }
    serde_json::to_writer(&mut raw, &value).map_err(owned_err)?;
    if raw.len() > raw_bound || raw.len() > MAX_CURSOR_RAW_BYTES {
        return Err(owned_err(
            "indexed QueryStore cursor exceeds Reference bound",
        ));
    }
    let token = crate::compressed_search_state::base64_encode(&raw);
    if token.len() > QUERY_STORE_INDEXED_CURSOR_MAX_BYTES {
        return Err(owned_err(
            "indexed QueryStore cursor exceeds Reference bound",
        ));
    }
    Ok(token)
}

fn finish_query_store_kind_page(
    page: OwnedQueryStorePage,
    kind: &str,
    store_revision: &str,
    source_revision: Option<&str>,
    query: &str,
    filters_digest: &str,
    held: usize,
    budget: &OriginalStoreBudget<'_>,
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> Result<QueryStoreIndexedKindPage> {
    let next_cursor = if page.has_more {
        let after = page
            .next_after
            .as_ref()
            .ok_or_else(|| owned_err("indexed QueryStore continuation absent"))?;
        Some(query_store_cursor_encode(
            kind,
            store_revision,
            source_revision,
            query,
            filters_digest,
            after,
            state_add(held, page.retained)?,
            budget,
            deadline,
            probe,
        )?)
    } else {
        None
    };
    let row_slots = state_slots::<Value>(page.rows.len())?;
    let cursor_heap = next_cursor.as_ref().map_or(0, String::capacity);
    (budget.remaining_after_retained)(state_add(
        state_add(held, page.retained)?,
        state_add(row_slots, cursor_heap)?,
    )?)?;
    let payload_heap = page
        .rows
        .iter()
        .try_fold(0usize, |sum, row| state_add(sum, row.payload_retained))?;
    let retained = state_add(state_add(row_slots, payload_heap)?, cursor_heap)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(page.rows.len()).map_err(owned_err)?;
    if rows.capacity() != page.rows.len() {
        return Err(owned_err("indexed QueryStore exact response rows"));
    }
    for row in page.rows {
        rows.push(row.payload);
    }
    Ok(QueryStoreIndexedKindPage {
        rows,
        candidate_rows: page.candidate_rows,
        verified_chars: page.verified_chars,
        has_more: page.has_more,
        next_cursor,
        sql_pages: page.sql_pages,
        retained_bytes: retained,
    })
}

impl LegacyStore {
    /// Execute both globally ranked QueryStore kind pages on this exact held
    /// authenticated Store and original counter/deadline owner.
    pub fn indexed_query_store_page_with_owned_budget(
        &mut self,
        request: &QueryStoreIndexedSearchRequest<'_>,
        continuation: &QueryStoreIndexedContinuation<'_>,
        max_verify_chars: u64,
        budget: &OriginalStoreBudget<'_>,
        usage: &mut StoreUsage,
        call_deadline: Instant,
        operation: Arc<dyn AbortProbe>,
    ) -> Result<QueryStoreIndexedPage> {
        let result = (|| {
            self.verify_currentness_with_owned_budget(budget)?;
            if !self.supports_indexed_fts5() {
                return Err(owned_err(
                    "selected QueryStore has no admitted indexed FTS5 page",
                ));
            }
            if max_verify_chars == 0
                || !(1..=100).contains(&request.limit_per_kind)
                || call_deadline > self.deadline
                || usage.rows != 0
                || usage.input_bytes != 0
                || usage.json_visits != 0
            {
                return Err(owned_err("indexed QueryStore owner/request unavailable"));
            }
            if continuation.cursor_present {
                if continuation.nodes_exhausted == continuation.nodes.is_some()
                    || continuation.relations_exhausted == continuation.relations.is_some()
                {
                    return Err(owned_err("indexed Reference child cursor state"));
                }
            } else if continuation.nodes.is_some()
                || continuation.relations.is_some()
                || continuation.nodes_exhausted
                || continuation.relations_exhausted
            {
                return Err(owned_err("indexed first-page cursor state"));
            }
            self.install_owned_progress(budget, call_deadline, Some(operation.clone()), 0)?;
            let original_abort = self.abort.clone();
            let probe = OwnedBorrowedProbe {
                original: original_abort.as_ref(),
                operation: operation.as_ref(),
            };
            let mut result = (|| {
                let mut field_bytes = request.query.len();
                if field_bytes > QUERY_STORE_INDEXED_FILTER_FIELD_BYTES {
                    return Err(owned_err("indexed QueryStore query byte bound"));
                }
                let mut query_points = 0usize;
                for _ in request.query.chars() {
                    budget.check(call_deadline, &probe)?;
                    search_charge(budget, 1)?;
                    query_points += 1;
                    if query_points > 256 {
                        return Err(owned_err("indexed QueryStore query codepoint bound"));
                    }
                }
                let fixed = state_add(
                    self.retained_state_upper_bound()?,
                    state_add(
                        std::mem::size_of::<QueryStoreIndexedSearchRequest<'_>>(),
                        state_add(
                            std::mem::size_of::<QueryStoreIndexedContinuation<'_>>(),
                            std::mem::size_of::<OwnedBorrowedProbe<'_>>(),
                        )?,
                    )?,
                )?;
                (budget.remaining_after_retained)(fixed)?;
                let mut held = fixed;
                let (normalized_query, query_heap) =
                    indexed_store_query(request.query, held, budget, call_deadline, &probe)?;
                held = state_add(held, query_heap)?;
                let mut total_values = 0usize;
                let (sources, sources_heap) = if request.sources.is_empty() {
                    let default_slots = state_slots::<String>(SOURCES.len())?;
                    let default_bytes = SOURCES
                        .iter()
                        .try_fold(0usize, |sum, value| state_add(sum, value.len()))?;
                    (budget.remaining_after_retained)(state_add(
                        held,
                        state_add(default_slots, default_bytes)?,
                    )?)?;
                    search_charge(budget, state_add(default_slots, default_bytes)?)?;
                    let mut defaults = Vec::new();
                    defaults
                        .try_reserve_exact(SOURCES.len())
                        .map_err(owned_err)?;
                    if defaults.capacity() != SOURCES.len() {
                        return Err(owned_err("indexed QueryStore exact default sources"));
                    }
                    for source in SOURCES {
                        defaults.push(exact_string(source)?);
                    }
                    let default_heap = state_add(
                        state_slots::<String>(defaults.capacity())?,
                        defaults
                            .iter()
                            .try_fold(0usize, |sum, value| state_add(sum, value.capacity()))?,
                    )?;
                    let (sources, output_heap) = indexed_store_filter(
                        &defaults,
                        false,
                        &mut total_values,
                        &mut field_bytes,
                        state_add(held, default_heap)?,
                        budget,
                        call_deadline,
                        &probe,
                    )?;
                    (sources, output_heap)
                } else {
                    indexed_store_filter(
                        request.sources,
                        false,
                        &mut total_values,
                        &mut field_bytes,
                        held,
                        budget,
                        call_deadline,
                        &probe,
                    )?
                };
                if sources
                    .iter()
                    .any(|source| !SOURCES.contains(&source.as_str()))
                {
                    return Err(owned_err("unsupported indexed QueryStore source"));
                }
                held = state_add(held, sources_heap)?;
                let (kind_ids, kinds_heap) = indexed_store_filter(
                    request.kind_ids,
                    true,
                    &mut total_values,
                    &mut field_bytes,
                    held,
                    budget,
                    call_deadline,
                    &probe,
                )?;
                held = state_add(held, kinds_heap)?;
                let (predicate_ids, predicates_heap) = indexed_store_filter(
                    request.predicate_ids,
                    true,
                    &mut total_values,
                    &mut field_bytes,
                    held,
                    budget,
                    call_deadline,
                    &probe,
                )?;
                held = state_add(held, predicates_heap)?;
                let (source_json, source_json_heap) =
                    owned_query_store_json_strings(&sources, held, budget)?;
                held = state_add(held, source_json_heap)?;
                let (kind_json, kind_json_heap) =
                    owned_query_store_json_strings(&kind_ids, held, budget)?;
                held = state_add(held, kind_json_heap)?;
                let (predicate_json, predicate_json_heap) =
                    owned_query_store_json_strings(&predicate_ids, held, budget)?;
                held = state_add(held, predicate_json_heap)?;
                let filter_digest =
                    query_store_filter_digest(&sources, &kind_ids, &predicate_ids, held, budget)?;
                let digest_heap = filter_digest.capacity();
                held = state_add(held, digest_heap)?;
                let (fts_expression, fts_heap) = owned_query_store_fts_expression(
                    &normalized_query,
                    held,
                    budget,
                    call_deadline,
                    &probe,
                )?;
                held = state_add(held, fts_heap)?;
                let source_revision_value = self.indexed_graph_source_revision()?;
                let store_revision_value = self.revision.as_str();
                if store_revision_value.is_empty() {
                    return Err(owned_err("indexed QueryStore source revision absent"));
                }
                let revisions_heap = state_add(
                    state_add(
                        source_revision_value.map_or(0, str::len),
                        store_revision_value.len(),
                    )?,
                    std::mem::size_of::<Option<String>>() + std::mem::size_of::<String>(),
                )?;
                (budget.remaining_after_retained)(state_add(held, revisions_heap)?)?;
                held = state_add(held, revisions_heap)?;
                let source_revision = source_revision_value.map(exact_string).transpose()?;
                let store_revision = exact_string(store_revision_value)?;
                let nodes_after = if continuation.nodes_exhausted {
                    None
                } else if let Some(token) = continuation.nodes {
                    Some(query_store_cursor_decode(
                        token,
                        "nodes",
                        &store_revision,
                        source_revision.as_deref(),
                        &normalized_query,
                        &filter_digest,
                        held,
                        budget,
                        call_deadline,
                        &probe,
                    )?)
                } else {
                    None
                };
                if let Some(after) = &nodes_after {
                    held = state_add(
                        held,
                        state_add(std::mem::size_of_val(after), after.1.capacity())?,
                    )?;
                }
                let relations_after = if continuation.relations_exhausted {
                    None
                } else if let Some(token) = continuation.relations {
                    Some(query_store_cursor_decode(
                        token,
                        "relations",
                        &store_revision,
                        source_revision.as_deref(),
                        &normalized_query,
                        &filter_digest,
                        held,
                        budget,
                        call_deadline,
                        &probe,
                    )?)
                } else {
                    None
                };
                if let Some(after) = &relations_after {
                    held = state_add(
                        held,
                        state_add(std::mem::size_of_val(after), after.1.capacity())?,
                    )?;
                }
                let nodes = if continuation.nodes_exhausted {
                    OwnedQueryStorePage {
                        rows: Vec::new(),
                        candidate_rows: 0,
                        verified_chars: 0,
                        has_more: false,
                        next_after: None,
                        sql_pages: 0,
                        retained: 0,
                    }
                } else {
                    self.ranked_query_store_kind_page_owned(
                        false,
                        &source_json,
                        &kind_json,
                        &fts_expression,
                        &normalized_query,
                        nodes_after.as_ref(),
                        request.limit_per_kind,
                        max_verify_chars,
                        budget,
                        usage,
                        call_deadline,
                        &probe,
                        held,
                    )?
                };
                let node_data = finish_query_store_kind_page(
                    nodes,
                    "nodes",
                    &store_revision,
                    source_revision.as_deref(),
                    &normalized_query,
                    &filter_digest,
                    held,
                    budget,
                    call_deadline,
                    &probe,
                )?;
                held = state_add(held, node_data.retained_bytes)?;
                let relations = if continuation.relations_exhausted {
                    OwnedQueryStorePage {
                        rows: Vec::new(),
                        candidate_rows: 0,
                        verified_chars: 0,
                        has_more: false,
                        next_after: None,
                        sql_pages: 0,
                        retained: 0,
                    }
                } else {
                    self.ranked_query_store_kind_page_owned(
                        true,
                        &source_json,
                        &predicate_json,
                        &fts_expression,
                        &normalized_query,
                        relations_after.as_ref(),
                        request.limit_per_kind,
                        max_verify_chars,
                        budget,
                        usage,
                        call_deadline,
                        &probe,
                        held,
                    )?
                };
                let relation_data = finish_query_store_kind_page(
                    relations,
                    "relations",
                    &store_revision,
                    source_revision.as_deref(),
                    &normalized_query,
                    &filter_digest,
                    held,
                    budget,
                    call_deadline,
                    &probe,
                )?;
                held = state_add(held, relation_data.retained_bytes)?;
                let page_fixed = state_add(
                    std::mem::size_of::<QueryStoreIndexedPage>(),
                    state_add(
                        std::mem::size_of::<QueryStoreIndexedFilters>(),
                        state_add(
                            std::mem::size_of::<QueryStoreIndexedKindPage>() * 2,
                            std::mem::size_of::<String>() * 3,
                        )?,
                    )?,
                )?;
                let metadata_heap = state_add(
                    state_add(
                        source_revision.as_ref().map_or(0, String::len),
                        store_revision.len(),
                    )?,
                    state_add(
                        normalized_query.capacity(),
                        state_add(
                            state_slots::<String>(sources.capacity())?,
                            state_add(
                                sources.iter().try_fold(0usize, |sum, value| {
                                    state_add(sum, value.capacity())
                                })?,
                                state_add(
                                    state_slots::<String>(kind_ids.capacity())?,
                                    state_add(
                                        kind_ids.iter().try_fold(0usize, |sum, value| {
                                            state_add(sum, value.capacity())
                                        })?,
                                        state_add(
                                            state_slots::<String>(predicate_ids.capacity())?,
                                            predicate_ids
                                                .iter()
                                                .try_fold(0usize, |sum, value| {
                                                    state_add(sum, value.capacity())
                                                })?,
                                        )?,
                                    )?,
                                )?,
                            )?,
                        )?,
                    )?,
                )?;
                let page_retained = state_add(
                    state_add(page_fixed, metadata_heap)?,
                    state_add(node_data.retained_bytes, relation_data.retained_bytes)?,
                )?;
                // The callback accounts for the whole live owner, not only the
                // returned page. `held` includes the Store, temporaries and both
                // kind pages; reserve the response's fixed state alongside them.
                (budget.remaining_after_retained)(state_add(held, page_fixed)?)?;
                Ok(QueryStoreIndexedPage {
                    store_revision,
                    source_revision,
                    normalized_query,
                    filters: QueryStoreIndexedFilters {
                        sources,
                        kind_ids,
                        predicate_ids,
                    },
                    has_more: node_data.has_more || relation_data.has_more,
                    nodes: node_data,
                    relations: relation_data,
                    retained_bytes: page_retained,
                })
            })();
            let result_state = result.as_ref().map_or(0, |page| page.retained_bytes);
            let restored = self.install_owned_progress(budget, self.deadline, None, result_state);
            let page = result?;
            restored?;
            budget.check(call_deadline, &probe)?;
            self.verify_currentness_with_owned_budget(budget)?;
            budget.check(call_deadline, &probe)?;
            Ok(page)
        })();
        self.work = budget.store_steps.load(Ordering::Relaxed);
        if result.is_err() {
            if let Some(owner) = &mut self.owned {
                owner.poisoned = true;
            }
        }
        result
    }
}
