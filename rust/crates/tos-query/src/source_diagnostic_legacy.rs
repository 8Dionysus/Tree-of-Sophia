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
        let offset = integer(args, "offset", 0, 100000)?;
        let limit = integer(args, "limit", 40, 100)?;
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
        (budget.remaining_after_retained)(state_add(
            held,
            state_add(self.revision.len(), "tos_knowledge_search_v1".len())?,
        )?)?;
        search_charge(
            budget,
            state_add(self.revision.len(), "tos_knowledge_search_v1".len())?,
        )?;
        budget.check(deadline, &probe)?;
        // Keep both response strings admitted while the numeric token
        // reservation is added; the revision copy occurs after that admission.
        held = state_add(
            held,
            state_add(self.revision.len(), "tos_knowledge_search_v1".len())?,
        )?;
        // serde_json arbitrary_precision keeps decimal token strings even for
        // these six bounded unsigned output numbers. u64 max needs 20 bytes.
        let number_bytes = 6usize
            .checked_mul(20)
            .ok_or_else(|| owned_err("legacy output number state overflow"))?;
        (budget.remaining_after_retained)(state_add(held, number_bytes)?)?;
        search_charge(budget, number_bytes)?;
        held = state_add(held, number_bytes)?;
        let source_revision = exact_string(&self.revision)?;
        if source_revision.capacity() != self.revision.len() {
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
