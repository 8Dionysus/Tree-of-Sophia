//! Shared bounded base assembly for ordinary owner node/edge carriers.
//! These pure mechanics normalize selected registry mappings and preserve raw
//! source records; source adapters still own input completeness and admission.
use crate::knowledge_normalization::{SourceRow, stamp_content_revision};
use crate::knowledge_philosophy_display::{
    full_owner_relation_display, full_philosophy_node_display,
};
use crate::knowledge_source_navigation_node::{ASSERTION_FIELDS, epistemic, normalized_time};
use crate::knowledge_source_navigation_relation::direct_assertion_context;
use crate::{Error, KnowledgeRegistry, QueryVocabulary, Result};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
};
use tos_foundation::Digest256;

#[derive(Clone, Copy, Default)]
pub struct BaseNodeOverrides<'a> {
    pub native_id: Option<&'a str>,
    pub identity_id: Option<&'a str>,
    pub kind_id: Option<&'a str>,
}
#[derive(Clone, Copy)]
pub struct BaseNormalizationLimits {
    pub max_registry_bytes: usize,
    pub max_output_bytes: usize,
}
pub struct KnowledgeBaseNormalizer<'a> {
    registry: &'a KnowledgeRegistry,
    sources: BTreeSet<String>,
    entity_registry_ref: String,
    relation_registry_ref: String,
    entities: BTreeMap<String, Value>,
    relations: BTreeMap<String, Value>,
    limits: BaseNormalizationLimits,
    owner_state: Option<&'a crate::d1_public_capture::CreationState<'a>>,
}
struct CappedBytes {
    bytes: usize,
    cap: usize,
}
impl Write for CappedBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|n| *n <= self.cap)
            .ok_or_else(|| std::io::Error::other("owner base title byte ceiling"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn required<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096 && !s.contains('\0'))
        .ok_or(Error::Invalid("owner base normalized field"))
}
fn text(value: Option<&Value>) -> Option<&str> {
    value?.as_str().map(str::trim).filter(|s| !s.is_empty())
}
fn strings(value: Option<&Value>) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().filter(|s| !s.is_empty()))
        .filter(|s| seen.insert((*s).to_owned()))
        .map(|s| Value::String(s.to_owned()))
        .collect()
}
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().is_some_and(|n| n != 0.),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
        _ => true,
    }
}
fn attrs(item: &Value, relation: bool) -> Result<Map<String, Value>> {
    let mut output = item
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let excluded = if relation {
        &[
            "id",
            "edge_id",
            "from_id",
            "to_id",
            "predicate_id",
            "display",
            "properties",
            "source_ref",
            "source_refs",
            "graph_layers",
            "view_ids",
            "from_source_graph",
            "to_source_graph",
        ][..]
    } else {
        &[
            "id",
            "node_id",
            "label",
            "canonical_label",
            "node_type",
            "node_kind",
            "resource_kind",
            "display",
            "multilingual",
            "properties",
            "source_ref",
            "source_refs",
            "graph_layers",
            "view_ids",
        ][..]
    };
    for (key, value) in item
        .as_object()
        .ok_or(Error::Invalid("owner base source object"))?
    {
        if !excluded.contains(&key.as_str()) {
            output.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    Ok(output)
}
fn layers(item: &Value) -> Vec<Value> {
    let layers = strings(item.get("graph_layers"));
    if layers.is_empty() {
        text(item.get("layer"))
            .map(|s| vec![json!(s)])
            .unwrap_or_default()
    } else {
        layers
    }
}
fn owned_btree_entry(
    state: &crate::d1_public_capture::CreationState<'_>,
    key_bytes: usize,
    key_size: usize,
    value_size: usize,
) -> Result<()> {
    let bytes = key_size
        .checked_add(value_size)
        .and_then(|n| n.checked_mul(11))
        .and_then(|n| n.checked_add(16 * std::mem::size_of::<usize>()))
        .and_then(|n| n.checked_add(key_bytes))
        .ok_or(Error::Budget("owner base registry BTreeMap geometry"))?;
    state.retain(bytes)
}
fn owned_string(
    value: &str,
    state: Option<&crate::d1_public_capture::CreationState<'_>>,
) -> Result<String> {
    if let Some(state) = state {
        state.retain(
            std::mem::size_of::<String>()
                .checked_add(value.len())
                .ok_or(Error::Budget("owner base registry text geometry"))?,
        )?;
    }
    Ok(value.to_owned())
}
fn entries(
    bytes: &[u8],
    cap: usize,
    collection: &str,
    id_field: &str,
    owner_state: Option<&crate::d1_public_capture::CreationState<'_>>,
) -> Result<BTreeMap<String, Value>> {
    let row = SourceRow::parse_scoped_with_optional_owned_state(bytes, cap, owner_state)?;
    let mut entries = BTreeMap::new();
    for entry in row
        .value()
        .get(collection)
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("owner base registry entries"))?
    {
        let borrowed_id = required(entry, id_field)?;
        if let Some(state) = owner_state {
            owned_btree_entry(
                state,
                borrowed_id.len(),
                std::mem::size_of::<String>(),
                std::mem::size_of::<Value>(),
            )?;
        }
        let id = borrowed_id.to_owned();
        let copied = match owner_state {
            Some(state) => state.clone_value(entry)?,
            None => entry.clone(),
        };
        if entries.insert(id, copied).is_some() {
            return Err(Error::Invalid("duplicate owner base registry entry"));
        }
    }
    Ok(entries)
}

enum BoundedSourceRow<'s, 'budget> {
    Owned(crate::knowledge_normalization::OwnedSourceRow<'s, 'budget>),
    Legacy(SourceRow),
}
impl std::ops::Deref for BoundedSourceRow<'_, '_> {
    type Target = SourceRow;
    fn deref(&self) -> &SourceRow {
        match self {
            Self::Owned(row) => row,
            Self::Legacy(row) => row,
        }
    }
}
fn bounded_source_value<'s, 'budget>(
    value: &Value,
    cap: usize,
    owner_state: Option<&'s crate::d1_public_capture::CreationState<'budget>>,
) -> Result<BoundedSourceRow<'s, 'budget>> {
    serde_json::to_writer(&mut CappedBytes { bytes: 0, cap }, value)
        .map_err(|_| Error::Budget("owner base display source bytes"))?;
    if let Some(state) = owner_state {
        state.with_json_encoded(value, cap, |raw| {
            SourceRow::parse_scoped_with_optional_owned_state(raw, cap, Some(state))
                .map(BoundedSourceRow::Owned)
        })
    } else {
        let raw =
            serde_json::to_vec(value).map_err(|_| Error::Invalid("owner base display JSON"))?;
        SourceRow::parse(&raw, cap).map(BoundedSourceRow::Legacy)
    }
}
fn source_epistemic(item: &Value, default_authority: &str) -> Value {
    let mut value = epistemic(item);
    if text(
        item.get("properties")
            .and_then(|p| p.get("authority_posture")),
    )
    .is_none()
        && text(item.get("authority_layer")).is_none()
    {
        value["authority_layer"] = json!(default_authority);
    }
    value
}

// Owned output forecasts are semantic destination envelopes. Each source-tree
// envelope below corresponds to one distinct normalizer destination: attrs,
// complete source_record.payload, ordinary semantics, display, epistemic,
// graph_layers, view_ids, or source_refs. Repeated subset and wrapper copies
// are planned separately from the selected carrier before normalize_* runs.
// These are conservative logical ownership bounds, not RSS measurements.
const NODE_SOURCE_DESTINATION_ENVELOPES: usize = 8;
const FIXED_NODE_SCHEMA_ENVELOPE: usize = 64 * 1024;

fn add_owned_bytes(total: &mut usize, bytes: usize, label: &'static str) -> Result<()> {
    *total = total.checked_add(bytes).ok_or(Error::Budget(label))?;
    Ok(())
}
fn add_source_copy(
    state: &crate::d1_public_capture::CreationState<'_>,
    total: &mut usize,
    value: &Value,
    copies: usize,
    label: &'static str,
) -> Result<()> {
    for _ in 0..copies {
        state.active()?;
        add_owned_bytes(total, state.value_clone_state_upper_bound(value)?, label)?;
    }
    Ok(())
}
fn source_key_count(
    state: &crate::d1_public_capture::CreationState<'_>,
    value: &Value,
    depth: usize,
) -> Result<usize> {
    if depth > 96 {
        return Err(Error::Budget("owned normalized digest key depth"));
    }
    state.charge_work(std::mem::size_of::<Value>())?;
    let mut count = 0usize;
    match value {
        Value::Array(items) => {
            for item in items {
                count = count
                    .checked_add(source_key_count(state, item, depth + 1)?)
                    .ok_or(Error::Budget("owned normalized digest keys"))?;
            }
        }
        Value::Object(fields) => {
            for (key, item) in fields {
                state.charge_work(key.len())?;
                let child = source_key_count(state, item, depth + 1)?;
                count = count
                    .checked_add(1)
                    .and_then(|n| n.checked_add(child))
                    .ok_or(Error::Budget("owned normalized digest keys"))?;
            }
        }
        _ => (),
    }
    Ok(count)
}
fn string_array_count_and_bytes(
    state: &crate::d1_public_capture::CreationState<'_>,
    value: Option<&Value>,
) -> Result<(usize, usize)> {
    let mut count = 0usize;
    let mut bytes = 0usize;
    if let Some(items) = value.and_then(Value::as_array) {
        for item in items {
            state.active()?;
            state.charge_work(std::mem::size_of::<Value>())?;
            if let Some(text) = item.as_str().filter(|text| !text.is_empty()) {
                state.charge_work(text.len())?;
                count = count
                    .checked_add(1)
                    .ok_or(Error::Budget("owned normalized string count"))?;
                bytes = bytes
                    .checked_add(text.len())
                    .ok_or(Error::Budget("owned normalized string bytes"))?;
            }
        }
    }
    Ok((count, bytes))
}
fn string_set_workspace(count: usize) -> Result<usize> {
    count
        .checked_mul(11 * std::mem::size_of::<String>() + 16 * std::mem::size_of::<usize>())
        .ok_or(Error::Budget("owned normalized string set workspace"))
}
fn temporal_object_call_counts(
    state: &crate::d1_public_capture::CreationState<'_>,
    object: &Map<String, Value>,
    depth: usize,
) -> Result<(usize, usize)> {
    state.active()?;
    state.charge_work(std::mem::size_of::<Map<String, Value>>())?;
    if depth > 32 {
        return Err(Error::Budget("navigation temporal nesting"));
    }
    for wrapper in ["temporal", "interval"] {
        state.charge_work(wrapper.len())?;
        if let Some(inner) = object.get(wrapper).and_then(Value::as_object) {
            let (calls, wrappers) = temporal_object_call_counts(state, inner, depth + 1)?;
            return Ok((
                calls
                    .checked_add(1)
                    .ok_or(Error::Budget("owned temporal calls"))?,
                wrappers
                    .checked_add(1)
                    .ok_or(Error::Budget("owned temporal wrappers"))?,
            ));
        }
    }
    let mut calls = 1usize;
    let mut wrappers = 0usize;
    for key in ["start", "end"] {
        state.charge_work(key.len())?;
        if let Some(bound) = object.get(key) {
            let (nested, nested_wrappers) = temporal_call_counts(state, Some(bound), depth + 1)?;
            calls = calls
                .checked_add(nested)
                .ok_or(Error::Budget("owned temporal calls"))?;
            wrappers = wrappers
                .checked_add(nested_wrappers)
                .ok_or(Error::Budget("owned temporal wrappers"))?;
        }
    }
    Ok((calls, wrappers))
}
fn temporal_call_counts(
    state: &crate::d1_public_capture::CreationState<'_>,
    value: Option<&Value>,
    depth: usize,
) -> Result<(usize, usize)> {
    state.active()?;
    state.charge_work(std::mem::size_of::<Value>())?;
    if depth > 32 {
        return Err(Error::Budget("navigation temporal nesting"));
    }
    let Some(value) = value else {
        return Ok((0, 0));
    };
    match value.as_object() {
        Some(object) => temporal_object_call_counts(state, object, depth),
        None => Ok((1, 0)),
    }
}
fn attribute_field_map_geometry(
    state: &crate::d1_public_capture::CreationState<'_>,
    item: &Value,
) -> Result<(usize, usize)> {
    state.active()?;
    state.charge_work(std::mem::size_of::<Value>())?;
    let object = item
        .as_object()
        .ok_or(Error::Invalid("owner base source object"))?;
    let props = item.get("properties").and_then(Value::as_object);
    let mut count = props.map_or(0, Map::len);
    let mut text_bytes = 0usize;
    let add_key = |key: &str, properties: bool, text_bytes: &mut usize| -> Result<()> {
        let escaped_extra = key
            .bytes()
            .filter(|byte| matches!(byte, b'~' | b'/'))
            .count();
        let path_len = (if properties { 12usize } else { 1usize })
            .checked_add(key.len())
            .and_then(|n| n.checked_add(escaped_extra))
            .ok_or(Error::Budget("owner base field-map path"))?;
        let map_key_len = "attributes."
            .len()
            .checked_add(key.len())
            .ok_or(Error::Budget("owner base field-map key"))?;
        add_owned_bytes(
            text_bytes,
            path_len
                .checked_add(map_key_len)
                .ok_or(Error::Budget("owner base field-map text"))?,
            "owner base field-map text",
        )
    };
    if let Some(props) = props {
        for key in props.keys() {
            state.active()?;
            state.charge_work(key.len())?;
            add_key(key, true, &mut text_bytes)?;
        }
    }
    for key in object.keys() {
        state.active()?;
        state.charge_work(key.len())?;
        if ![
            "id",
            "node_id",
            "label",
            "canonical_label",
            "node_type",
            "node_kind",
            "resource_kind",
            "display",
            "multilingual",
            "properties",
            "source_ref",
            "source_refs",
            "graph_layers",
            "view_ids",
        ]
        .contains(&key.as_str())
            && !props.is_some_and(|props| props.contains_key(key))
        {
            count = count
                .checked_add(1)
                .ok_or(Error::Budget("owner base field-map count"))?;
            add_key(key, false, &mut text_bytes)?;
        }
    }
    Ok((count, text_bytes))
}
fn assertion_output_extras(
    state: &crate::d1_public_capture::CreationState<'_>,
    item: &Value,
    total: &mut usize,
    serialized_cap: usize,
) -> Result<(usize, usize)> {
    let Some(outer) = item.as_object() else {
        return Ok((0, 0));
    };
    let props = item.get("properties").and_then(Value::as_object);
    let embedded = props
        .and_then(|props| props.get("source_claim"))
        .and_then(Value::as_object);
    let layers = [Some(outer), props, embedded];
    let prefixes = ["", "/properties", "/properties/source_claim"];
    let mut unique = 0usize;
    let mut occurrences = 0usize;
    let mut duplicates = 0usize;
    let mut pointer_bytes = 0usize;
    for key in ASSERTION_FIELDS {
        let mut seen = false;
        let mut key_occurrences = 0usize;
        for (index, layer) in layers.iter().enumerate() {
            if let Some(value) = layer.and_then(|layer| layer.get(*key)) {
                if !seen {
                    unique = unique
                        .checked_add(1)
                        .ok_or(Error::Budget("assertion output fields"))?;
                } else {
                    duplicates = duplicates
                        .checked_add(1)
                        .ok_or(Error::Budget("assertion output conflicts"))?;
                    // A repeated field may retain both prior and current trees
                    // in one conflict record, in addition to the final field.
                    add_source_copy(state, total, item, 2, "assertion conflict source copies")?;
                }
                seen = true;
                key_occurrences += 1;
                occurrences = occurrences
                    .checked_add(1)
                    .ok_or(Error::Budget("assertion output occurrences"))?;
                let prefix = prefixes[index];
                let pointer = prefix
                    .len()
                    .checked_add(1)
                    .and_then(|n| n.checked_add(key.len()))
                    .ok_or(Error::Budget("assertion source pointer"))?;
                pointer_bytes = pointer_bytes
                    .checked_add(pointer)
                    .ok_or(Error::Budget("assertion source pointer"))?;
                let _ = value;
            }
        }
        if key_occurrences > 0 {
            add_owned_bytes(total, key.len(), "assertion field-map keys")?;
        }
    }
    if occurrences == 0 {
        return Ok((0, 0));
    }
    let fields_map = crate::knowledge_normalization::serde_object_slots_upper(unique)?;
    let context_map = crate::knowledge_normalization::serde_object_slots_upper(6)?;
    let entry_map = crate::knowledge_normalization::serde_object_slots_upper(2)?;
    let conflicts_array = crate::knowledge_normalization::serde_array_slots_upper(duplicates)?;
    let mut wrappers = fields_map
        .checked_add(context_map)
        .and_then(|n| n.checked_add(entry_map))
        .and_then(|n| n.checked_add(conflicts_array))
        .ok_or(Error::Budget("assertion context containers"))?;
    if duplicates > 0 {
        wrappers = wrappers
            .checked_add(
                duplicates
                    .checked_mul(crate::knowledge_normalization::serde_object_slots_upper(3)?)
                    .ok_or(Error::Budget("assertion conflict maps"))?,
            )
            .ok_or(Error::Budget("assertion conflict maps"))?;
        let conflict_text = duplicates
            .checked_mul(2 * 32 + 2 * ASSERTION_FIELDS.iter().map(|k| k.len()).max().unwrap_or(0))
            .ok_or(Error::Budget("assertion conflict text"))?;
        add_owned_bytes(total, conflict_text, "assertion conflict text")?;
    }
    wrappers = wrappers
        .checked_add(
            occurrences
                .checked_mul(crate::knowledge_normalization::serde_object_slots_upper(2)?)
                .ok_or(Error::Budget("assertion entry maps"))?,
        )
        .ok_or(Error::Budget("assertion entry maps"))?;
    add_owned_bytes(total, wrappers, "assertion context containers")?;
    add_owned_bytes(total, pointer_bytes, "assertion source pointers")?;
    // direct_assertion_context serializes once into a bounded temporary Vec.
    add_owned_bytes(
        total,
        serialized_cap,
        "assertion serialized temporary ceiling",
    )?;
    // Its source_refs call coexists with the outer normalized source_refs.
    add_source_copy(state, total, item, 1, "assertion source refs destination")?;
    Ok((occurrences, duplicates))
}

impl<'a> KnowledgeBaseNormalizer<'a> {
    pub fn new(
        registry: &'a KnowledgeRegistry,
        entity_bytes: &[u8],
        relation_bytes: &[u8],
        vocabulary: &QueryVocabulary,
        descriptor_bytes: &[u8],
        limits: BaseNormalizationLimits,
    ) -> Result<Self> {
        Self::new_inner(
            registry,
            entity_bytes,
            relation_bytes,
            vocabulary,
            descriptor_bytes,
            limits,
            None,
        )
    }
    pub fn new_with_owned_state(
        registry: &'a KnowledgeRegistry,
        entity_bytes: &[u8],
        relation_bytes: &[u8],
        vocabulary: &QueryVocabulary,
        descriptor_bytes: &[u8],
        limits: BaseNormalizationLimits,
        state: &'a crate::d1_public_capture::CreationState<'a>,
    ) -> Result<Self> {
        Self::new_inner(
            registry,
            entity_bytes,
            relation_bytes,
            vocabulary,
            descriptor_bytes,
            limits,
            Some(state),
        )
    }
    /// Require the repository and normalizer to share the exact capture owner.
    pub(crate) fn ensure_same_owned_state(
        &self,
        state: &crate::d1_public_capture::CreationState<'_>,
    ) -> Result<()> {
        let owner = self
            .owner_state
            .ok_or(Error::Invalid("owned normalizer state absent"))?;
        let owner_address = std::ptr::from_ref(owner).cast::<()>();
        let state_address = std::ptr::from_ref(state).cast::<()>();
        if owner_address != state_address {
            return Err(Error::Invalid("owned normalizer state mismatch"));
        }
        Ok(())
    }
    fn new_inner(
        registry: &'a KnowledgeRegistry,
        entity_bytes: &[u8],
        relation_bytes: &[u8],
        vocabulary: &QueryVocabulary,
        descriptor_bytes: &[u8],
        limits: BaseNormalizationLimits,
        owner_state: Option<&'a crate::d1_public_capture::CreationState<'a>>,
    ) -> Result<Self> {
        if limits.max_registry_bytes == 0
            || limits.max_registry_bytes > 4 * 1024 * 1024
            || limits.max_output_bytes == 0
            || limits.max_output_bytes > 8 * 1024 * 1024
        {
            return Err(Error::Budget("owner base limits"));
        }
        vocabulary.verify_authored_bytes(descriptor_bytes)?;
        if registry.entity_registry_id != vocabulary.entity_registry_id
            || registry.relation_registry_id != vocabulary.relation_registry_id
        {
            return Err(Error::Invalid("owner base descriptor registry identity"));
        }
        if entity_bytes.len() > limits.max_registry_bytes
            || relation_bytes.len() > limits.max_registry_bytes
            || Digest256::of_bytes(entity_bytes).to_hex() != registry.entity_sha256
            || Digest256::of_bytes(relation_bytes).to_hex() != registry.relation_sha256
        {
            return Err(Error::Invalid("owner base selected registry bytes"));
        }
        let descriptor = SourceRow::parse_scoped_with_optional_owned_state(
            descriptor_bytes,
            1024 * 1024,
            owner_state,
        )?;
        let refs = descriptor
            .value()
            .get("semantic_registry_refs")
            .ok_or(Error::Invalid("owner base registry refs"))?;
        let mut sources = BTreeSet::new();
        for source in &vocabulary.registered_source_ids {
            if let Some(state) = owner_state {
                owned_btree_entry(state, source.len(), std::mem::size_of::<String>(), 0)?;
            }
            sources.insert(source.clone());
        }
        let entity_ref = required(
            refs.get("entity")
                .ok_or(Error::Invalid("owner base entity ref"))?,
            "source_ref",
        )?;
        let relation_ref = required(
            refs.get("relation")
                .ok_or(Error::Invalid("owner base relation ref"))?,
            "source_ref",
        )?;
        let entity_registry_ref = owned_string(entity_ref, owner_state)?;
        let relation_registry_ref = owned_string(relation_ref, owner_state)?;
        let entities = entries(
            entity_bytes,
            limits.max_registry_bytes,
            "types",
            "type_id",
            owner_state,
        )?;
        let relations = entries(
            relation_bytes,
            limits.max_registry_bytes,
            "relations",
            "relation_type_id",
            owner_state,
        )?;
        Ok(Self {
            registry,
            sources,
            entity_registry_ref,
            relation_registry_ref,
            entities,
            relations,
            limits,
            owner_state,
        })
    }

    fn owned_node_output_hold(
        &self,
        source: &SourceRow,
        source_graph: &str,
        canonical: bool,
        overrides: BaseNodeOverrides<'_>,
        state: &'a crate::d1_public_capture::CreationState<'a>,
    ) -> Result<crate::d1_public_capture::CreationStateHold<'a, 'a>> {
        let item = source.value();
        let mut total = FIXED_NODE_SCHEMA_ENVELOPE;
        // The eight separately documented destinations are attributes,
        // complete source_record.payload, ordinary semantics, philosophy
        // display, epistemic, graph_layers, view_ids and source_refs.
        add_source_copy(
            state,
            &mut total,
            item,
            NODE_SOURCE_DESTINATION_ENVELOPES,
            "owner node semantic destinations",
        )?;
        // strings() keeps a source String set while it creates the output
        // Value array; these are extra per-call source-tree envelopes.
        let mut extra_source_envelopes = 0usize;
        if item.get("graph_layers").and_then(Value::as_array).is_some() {
            add_source_copy(state, &mut total, item, 1, "owner node layer set strings")?;
            extra_source_envelopes += 1;
        }
        if item.get("view_ids").and_then(Value::as_array).is_some() {
            add_source_copy(state, &mut total, item, 1, "owner node view set strings")?;
            extra_source_envelopes += 1;
        }
        // source_refs() stores deduplicated Strings before the output Value
        // array copies them, so one additional source-tree envelope covers
        // that simultaneously live set/vector data.
        add_source_copy(state, &mut total, item, 1, "owner source refs set strings")?;
        extra_source_envelopes += 1;
        let props = item.get("properties").and_then(Value::as_object);
        let p = |key: &str| props.and_then(|props| props.get(key));
        let native = overrides
            .native_id
            .or_else(|| text(item.get("node_id")))
            .or_else(|| text(item.get("id")))
            .or_else(|| text(item.get("path")))
            .unwrap_or("unnamed");
        let kind = overrides
            .kind_id
            .or_else(|| text(p("original_node_type")))
            .or_else(|| text(item.get("node_type")))
            .or_else(|| text(item.get("node_kind")))
            .or_else(|| text(item.get("resource_kind")))
            .unwrap_or("knowledge-object");
        let type_id = self.registry.entity(source_graph, kind).type_id;
        let entry = self
            .entities
            .get(type_id)
            .ok_or(Error::Invalid("owner base entity type entry"))?;
        add_source_copy(
            state,
            &mut total,
            entry,
            1,
            "owner node selected registry display",
        )?;
        // Every reachable ancestor is bounded by all selected-registry IDs
        // and parent edges; the existing walk clones one frontier String per
        // emitted parent and stores one BTreeSet key per unique type.
        let entity_count = self.entities.len();
        let mut edge_count = 0usize;
        let mut key_bytes = 0usize;
        let mut edge_bytes = 0usize;
        for (id, entry) in &self.entities {
            state.charge_work(id.len())?;
            key_bytes = key_bytes
                .checked_add(id.len())
                .ok_or(Error::Budget("owner ancestor IDs"))?;
            if let Some(parents) = entry.get("parent_type_ids").and_then(Value::as_array) {
                for parent in parents {
                    if let Some(parent) = parent.as_str() {
                        state.charge_work(parent.len())?;
                        edge_count = edge_count
                            .checked_add(1)
                            .ok_or(Error::Budget("owner ancestor edges"))?;
                        edge_bytes = edge_bytes
                            .checked_add(parent.len())
                            .ok_or(Error::Budget("owner ancestor edge IDs"))?;
                    }
                }
            }
        }
        let node_entry = entity_count
            .checked_mul(11 * std::mem::size_of::<String>() + 16 * std::mem::size_of::<usize>())
            .and_then(|n| n.checked_add(key_bytes))
            .ok_or(Error::Budget("owner ancestor set geometry"))?;
        let frontier_slots = edge_count
            .checked_add(1)
            .and_then(|n| n.checked_mul(6 * std::mem::size_of::<String>()))
            .ok_or(Error::Budget("owner ancestor frontier geometry"))?;
        add_owned_bytes(&mut total, node_entry, "owner ancestor set geometry")?;
        add_owned_bytes(
            &mut total,
            frontier_slots,
            "owner ancestor frontier geometry",
        )?;
        add_owned_bytes(
            &mut total,
            edge_bytes
                .checked_mul(2)
                .and_then(|n| n.checked_add(key_bytes.checked_mul(2)?))
                .and_then(|n| n.checked_add(type_id.len().checked_mul(2)?))
                .ok_or(Error::Budget("owner ancestor string geometry"))?,
            "owner ancestor string geometry",
        )?;
        add_owned_bytes(
            &mut total,
            crate::knowledge_normalization::serde_array_slots_upper(entity_count)?,
            "owner ancestor output array",
        )?;
        let (assertion_occurrences, assertion_conflicts) =
            assertion_output_extras(state, item, &mut total, self.limits.max_output_bytes)?;
        if assertion_occurrences > 0 {
            extra_source_envelopes = extra_source_envelopes
                .checked_add(1)
                .ok_or(Error::Budget("owner assertion source refs"))?;
        }
        extra_source_envelopes = extra_source_envelopes
            .checked_add(
                assertion_conflicts
                    .checked_mul(2)
                    .ok_or(Error::Budget("owner assertion conflict sources"))?,
            )
            .ok_or(Error::Budget("owner assertion conflict sources"))?;
        // normalized_time recursively retains merged temporal maps while
        // descending wrappers, then copies raw snapshots and interval bounds
        // during unwind. Eight source-equivalent copies cover each actual
        // call's common/raw, cloned common map, bounds and parsed endpoint
        // destinations; five more cover each temporal/interval wrapper's
        // inner map, merged context, raw snapshot, wording and interval.
        let time_value = if props.is_some_and(|props| props.contains_key("period")) {
            p("period")
        } else {
            item.get("temporal_context")
        };
        let mut temporal_copies = 0usize;
        if let Some(time) = time_value {
            let (calls, wrappers) = temporal_call_counts(state, Some(time), 0)?;
            temporal_copies = calls
                .checked_mul(8)
                .and_then(|n| n.checked_add(wrappers.checked_mul(5)?))
                .ok_or(Error::Budget("owner temporal copies"))?;
            add_source_copy(
                state,
                &mut total,
                time,
                temporal_copies,
                "owner temporal normalization destinations",
            )?;
            add_owned_bytes(
                &mut total,
                calls
                    .checked_add(wrappers)
                    .and_then(|n| n.checked_mul(4096))
                    .ok_or(Error::Budget("owner temporal maps"))?,
                "owner temporal output maps",
            )?;
        }
        let (attribute_count, field_text) = attribute_field_map_geometry(state, item)?;
        let field_maps = crate::knowledge_normalization::serde_object_slots_upper(attribute_count)?
            .checked_mul(2)
            .ok_or(Error::Budget("owner field-map containers"))?;
        add_owned_bytes(&mut total, field_maps, "owner field-map containers")?;
        add_owned_bytes(&mut total, field_text, "owner field-map text")?;
        let (layer_count, _) = string_array_count_and_bytes(state, item.get("graph_layers"))?;
        let (view_count, _) = string_array_count_and_bytes(state, item.get("view_ids"))?;
        add_owned_bytes(
            &mut total,
            crate::knowledge_normalization::serde_array_slots_upper(layer_count.max(usize::from(
                item.get("layer").and_then(Value::as_str).is_some(),
            )))?,
            "owner graph layer output array",
        )?;
        add_owned_bytes(
            &mut total,
            crate::knowledge_normalization::serde_array_slots_upper(view_count)?,
            "owner view output array",
        )?;
        let mut source_refs = string_array_count_and_bytes(state, item.get("source_refs"))?.0;
        for key in ["source_ref", "source_path", "path", "owner_surface"] {
            if text(item.get(key)).is_some() {
                source_refs = source_refs
                    .checked_add(1)
                    .ok_or(Error::Budget("owner source refs"))?;
            }
        }
        if source_refs == 0 {
            source_refs = 1;
        }
        add_owned_bytes(
            &mut total,
            string_set_workspace(source_refs)?,
            "owner source refs set",
        )?;
        add_owned_bytes(
            &mut total,
            crate::knowledge_normalization::serde_array_slots_upper(source_refs)?,
            "owner source refs output array",
        )?;
        let keys = source_key_count(state, item, 0)?;
        let digest_traversals = 2usize
            .checked_add(
                assertion_conflicts
                    .checked_mul(2)
                    .ok_or(Error::Budget("owner digest traversal count"))?,
            )
            .ok_or(Error::Budget("owner digest traversal count"))?;
        let dynamic_keys = assertion_occurrences
            .checked_mul(4)
            .and_then(|n| n.checked_add(2048))
            .ok_or(Error::Budget("owner normalized digest keys"))?;
        let output_envelopes = NODE_SOURCE_DESTINATION_ENVELOPES
            .checked_add(extra_source_envelopes)
            .and_then(|n| n.checked_add(temporal_copies))
            .ok_or(Error::Budget("owner normalized digest envelopes"))?;
        let output_keys = keys
            .checked_mul(output_envelopes)
            .and_then(|n| n.checked_add(dynamic_keys))
            .ok_or(Error::Budget("owner normalized digest keys"))?;
        let pointer_slots = keys
            .checked_mul(digest_traversals)
            .and_then(|n| n.checked_add(output_keys))
            .and_then(|n| n.checked_mul(std::mem::size_of::<&String>()))
            .ok_or(Error::Budget("owner normalized digest key workspace"))?;
        add_owned_bytes(
            &mut total,
            pointer_slots,
            "owner normalized digest key workspace",
        )?;
        // Exact dynamic identifier/value strings outside the source-tree
        // envelopes, including the root identity and registry references.
        let explicit_entity = [p("record_id"), item.get("record_id"), item.get("node_id")]
            .into_iter()
            .find_map(|v| text(v).filter(|s| s.starts_with("tos.")))
            .or_else(|| native.starts_with("tos.").then_some(native));
        let identity = overrides.identity_id.unwrap_or(native);
        let id_len = source_graph
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_add(identity.len()))
            .ok_or(Error::Budget("owner node identity"))?;
        let entity_len = explicit_entity.map_or(id_len, |entity| entity.len());
        let root_text = id_len
            .checked_add(entity_len)
            .and_then(|n| n.checked_add(native.len()))
            .and_then(|n| n.checked_add(source_graph.len()))
            .and_then(|n| n.checked_add(kind.len()))
            .and_then(|n| n.checked_add(type_id.len()))
            .and_then(|n| n.checked_add(kind.len()))
            .and_then(|n| n.checked_add(self.entity_registry_ref.len()))
            .and_then(|n| n.checked_add(64 + 30))
            .ok_or(Error::Budget("owner node root text"))?;
        add_owned_bytes(&mut total, root_text, "owner node root text")?;
        add_owned_bytes(
            &mut total,
            crate::knowledge_normalization::serde_object_slots_upper(16)?
                .checked_add(16 * std::mem::size_of::<Value>())
                .ok_or(Error::Budget("owner node root map"))?,
            "owner node root map",
        )?;
        let _ = (canonical, assertion_occurrences);
        state.hold(total)
    }
    pub(crate) fn with_normalized_node_owned(
        &self,
        source: &SourceRow,
        source_graph: &str,
        canonical: bool,
        overrides: BaseNodeOverrides<'_>,
        output_byte_cap: usize,
        consume: impl FnOnce(&Value, &[u8]) -> Result<()>,
    ) -> Result<()> {
        if output_byte_cap == 0 || output_byte_cap > self.limits.max_output_bytes {
            return Err(Error::Budget("owner node output callback cap"));
        }
        let state = self
            .owner_state
            .ok_or(Error::Invalid("owned node normalizer state absent"))?;
        let output_hold =
            self.owned_node_output_hold(source, source_graph, canonical, overrides, state)?;
        let value = self.normalize_node_unstamped(source, source_graph, canonical, overrides)?;
        let result = crate::knowledge_normalization::with_content_revision_owned(
            state,
            value,
            output_byte_cap,
            |value, encoded| consume(value, encoded),
        );
        drop(output_hold);
        result
    }

    pub fn normalize_node(
        &self,
        source: &SourceRow,
        source_graph: &str,
        canonical: bool,
        overrides: BaseNodeOverrides<'_>,
    ) -> Result<Value> {
        let mut value =
            self.normalize_node_unstamped(source, source_graph, canonical, overrides)?;
        stamp_content_revision(&mut value, self.limits.max_output_bytes)?;
        Ok(value)
    }

    fn normalize_node_unstamped(
        &self,
        source: &SourceRow,
        source_graph: &str,
        canonical: bool,
        overrides: BaseNodeOverrides<'_>,
    ) -> Result<Value> {
        if !self.sources.contains(source_graph) {
            return Err(Error::Invalid("owner base unregistered source"));
        }
        let default_authority = if canonical { "canon" } else { "derived-export" };
        let item = source.value();
        let native = overrides
            .native_id
            .or_else(|| text(item.get("node_id")))
            .or_else(|| text(item.get("id")))
            .or_else(|| text(item.get("path")))
            .unwrap_or("unnamed");
        let props = item.get("properties").and_then(Value::as_object);
        let p = |key: &str| props.and_then(|props| props.get(key));
        let kind = overrides
            .kind_id
            .or_else(|| text(p("original_node_type")))
            .or_else(|| text(item.get("node_type")))
            .or_else(|| text(item.get("node_kind")))
            .or_else(|| text(item.get("resource_kind")))
            .unwrap_or("knowledge-object");
        let resolved = self.registry.entity(source_graph, kind);
        let type_id = resolved.type_id;
        let entry = self
            .entities
            .get(type_id)
            .ok_or(Error::Invalid("owner base entity type entry"))?;
        let labels = entry
            .get("source_mappings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|mapping| {
                mapping.get("source_graph").and_then(Value::as_str) == Some(source_graph)
                    && mapping.get("source_kind_id").and_then(Value::as_str) == Some(kind)
                    && truthy(mapping.get("labels"))
            })
            .and_then(|mapping| mapping.get("labels"))
            .or_else(|| entry.get("labels"));
        let mut ancestors = BTreeSet::new();
        let mut frontier = vec![type_id.to_owned()];
        while let Some(id) = frontier.pop() {
            if ancestors.insert(id.clone()) {
                let entry = self
                    .entities
                    .get(&id)
                    .ok_or(Error::Invalid("owner base ancestor entry"))?;
                frontier.extend(
                    entry
                        .get("parent_type_ids")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(|v| {
                            v.as_str()
                                .map(str::to_owned)
                                .ok_or(Error::Invalid("owner base parent ID"))
                        })
                        .collect::<Result<Vec<_>>>()?,
                );
            }
        }
        let refs = source.source_refs(&[]);
        let display_owner;
        let display_source = if canonical && props.is_some_and(|p| p.contains_key("node_id")) {
            let properties = props.ok_or(Error::Invalid("canonical retained source"))?;
            if properties.get("node_id") != item.get("node_id")
                || properties.get("node_type") != item.get("node_type")
            {
                return Err(Error::Invalid("canonical retained source identity/type"));
            }
            let mut display_item;
            if let Some(state) = self.owner_state {
                let properties_value = item
                    .get("properties")
                    .ok_or(Error::Invalid("canonical retained properties"))?;
                let one_copy = state.value_clone_state_upper_bound(properties_value)?;
                let outer_map = crate::knowledge_normalization::serde_object_slots_upper(
                    properties
                        .len()
                        .checked_add(1)
                        .ok_or(Error::Budget("canonical retained display map"))?,
                )?;
                let copies = one_copy
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(outer_map))
                    .and_then(|n| n.checked_add(std::mem::size_of::<Value>() + "properties".len()))
                    .ok_or(Error::Budget("canonical retained display geometry"))?;
                state.charge_work(copies)?;
                let _display_hold = state.hold(copies)?;
                display_item = properties.clone();
                display_item.insert("properties".into(), properties_value.clone());
                display_owner = bounded_source_value(
                    &Value::Object(display_item),
                    self.limits.max_output_bytes,
                    Some(state),
                )?;
            } else {
                display_item = properties.clone();
                display_item.insert("properties".into(), Value::Object(properties.clone()));
                display_owner = bounded_source_value(
                    &Value::Object(display_item),
                    self.limits.max_output_bytes,
                    None,
                )?;
            }
            &display_owner
        } else {
            source
        };
        let attributes = attrs(item, false)?;
        let mut semantics = Map::new();
        if let Some(multilingual) = item.get("multilingual").and_then(Value::as_object) {
            semantics.insert(
                "language_context".into(),
                Value::Object(
                    multilingual
                        .iter()
                        .filter(|(k, _)| k.as_str() != "label")
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                ),
            );
        }
        if let Some(context) = direct_assertion_context(source, self.limits.max_output_bytes)? {
            semantics.insert("assertion_contexts".into(), json!([context]));
        }
        if truthy(p("packet_id")) {
            semantics.insert("annotation".into(),json!({"packet_id":p("packet_id"),"packet_version":p("packet_version"),"content_available":p("content_available"),"publication_posture":p("publication_posture")}));
            if kind == "annotation-claim" {
                semantics.insert("claim".into(),json!({"claim_id":p("claim_id"),"claim_version":p("claim_version"),"proposition":p("proposition"),"review_status":p("claim_status"),"contract_ref":"ToS/contracts/semantic-annotation-packet-v2.schema.json"}));
            }
        }
        let period_field = if props.is_some_and(|p| p.contains_key("period")) {
            "properties.period"
        } else {
            "temporal_context"
        };
        if let Some(time) = normalized_time(
            p("period").or_else(|| item.get("temporal_context")),
            period_field,
        )? {
            semantics.insert("time".into(), time);
        }
        semantics.insert("type_ancestors".into(), json!(ancestors));
        let id = format!(
            "{}:{}",
            source_graph,
            overrides.identity_id.unwrap_or(native)
        );
        let entity = [p("record_id"), item.get("record_id"), item.get("node_id")]
            .into_iter()
            .find_map(|v| text(v).filter(|s| s.starts_with("tos.")))
            .or_else(|| native.starts_with("tos.").then_some(native))
            .unwrap_or(&id);
        let mut value = json!({"id":id,"entity_id":entity,"native_id":native,"source_graph":source_graph,"kind_id":kind,"type_id":type_id,
            "type_mapping":{"status":if type_id != self.registry.fallback_entity_type_id() {"mapped"} else {"unmapped"},"source_kind_id":kind,"registry_ref":self.entity_registry_ref},
            "display":full_philosophy_node_display(display_source,kind,labels,entry.get("object_role").and_then(Value::as_str))?,"epistemic":source_epistemic(item,default_authority),"graph_layers":layers(item),"view_ids":strings(item.get("view_ids")),
            "source_refs":refs,"source_record":source.source_record(&attributes)?,"attributes":attributes,"semantics":semantics});
        if canonical
            && !value["display"]["provenance"]["source_title_available"]
                .as_bool()
                .unwrap_or(false)
        {
            if let Some(label) = text(item.get("label")) {
                value["display"]["title"]["default"] = json!(label);
            }
        }
        Ok(value)
    }
    fn owned_relation_output_hold(
        &self,
        source: &SourceRow,
        source_graph: &str,
        identity: Option<&str>,
        left_title: &Value,
        right_title: &Value,
        default_authority: &str,
        state: &'a crate::d1_public_capture::CreationState<'a>,
    ) -> Result<crate::d1_public_capture::CreationStateHold<'a, 'a>> {
        state.active()?;
        let item = source.value();
        let mut total = FIXED_NODE_SCHEMA_ENVELOPE;
        // Exact semantic destinations: attributes, complete source payload,
        // semantics, relation display, epistemic, graph layers, view IDs, refs.
        add_source_copy(
            state,
            &mut total,
            item,
            8,
            "owner relation semantic destinations",
        )?;

        let native = required(item, "edge_id")?;
        let predicate = text(item.get("predicate_id")).unwrap_or("related_to");
        let type_id = self
            .registry
            .relation(source_graph, predicate, "edge")
            .type_id;
        let entry = self
            .relations
            .get(type_id)
            .ok_or(Error::Invalid("owner base relation type entry"))?;
        // The output clones the registry entry and selected mapping, then
        // projects its labels into localized display forms.
        add_source_copy(
            state,
            &mut total,
            entry,
            3,
            "owner relation registry outputs",
        )?;
        let languages = 4usize
            .checked_add(
                entry
                    .get("labels")
                    .and_then(Value::as_object)
                    .map_or(0, Map::len),
            )
            .ok_or(Error::Budget("owner relation display languages"))?;
        add_source_copy(
            state,
            &mut total,
            left_title,
            languages,
            "owner relation localized left title",
        )?;
        add_source_copy(
            state,
            &mut total,
            right_title,
            languages,
            "owner relation localized right title",
        )?;

        let object = item
            .as_object()
            .ok_or(Error::Invalid("owner relation source object"))?;
        let props = item.get("properties").and_then(Value::as_object);
        let excluded = [
            "id",
            "edge_id",
            "from_id",
            "to_id",
            "predicate_id",
            "display",
            "properties",
            "source_ref",
            "source_refs",
            "graph_layers",
            "view_ids",
            "from_source_graph",
            "to_source_graph",
        ];
        let mut fields = props.map_or(0, Map::len);
        let mut field_text = 0usize;
        let mut add_field = |key: &str, in_props: bool| -> Result<()> {
            let escaped = key.bytes().filter(|b| matches!(b, b'~' | b'/')).count();
            let path = (if in_props { 12usize } else { 1usize })
                .checked_add(key.len())
                .and_then(|n| n.checked_add(escaped))
                .ok_or(Error::Budget("owner relation field path"))?;
            let map_key = "attributes."
                .len()
                .checked_add(key.len())
                .ok_or(Error::Budget("owner relation field key"))?;
            field_text = field_text
                .checked_add(
                    path.checked_add(map_key)
                        .ok_or(Error::Budget("owner relation field text"))?,
                )
                .ok_or(Error::Budget("owner relation field text"))?;
            Ok(())
        };
        if let Some(props) = props {
            for key in props.keys() {
                state.active()?;
                state.charge_work(key.len())?;
                add_field(key, true)?;
            }
        }
        for key in object.keys() {
            state.active()?;
            state.charge_work(key.len())?;
            if !excluded.contains(&key.as_str()) && !props.is_some_and(|p| p.contains_key(key)) {
                fields = fields
                    .checked_add(1)
                    .ok_or(Error::Budget("owner relation field count"))?;
                add_field(key, false)?;
            }
        }
        add_owned_bytes(
            &mut total,
            crate::knowledge_normalization::serde_object_slots_upper(fields)?,
            "owner relation field map slots",
        )?;
        add_owned_bytes(&mut total, field_text, "owner relation field map text")?;

        let (layers, _) = string_array_count_and_bytes(state, item.get("graph_layers"))?;
        let (views, _) = string_array_count_and_bytes(state, item.get("view_ids"))?;
        add_owned_bytes(
            &mut total,
            crate::knowledge_normalization::serde_array_slots_upper(layers)?,
            "owner relation layers output",
        )?;
        add_owned_bytes(
            &mut total,
            crate::knowledge_normalization::serde_array_slots_upper(views)?,
            "owner relation views output",
        )?;
        let mut refs = string_array_count_and_bytes(state, item.get("source_refs"))?.0;
        for key in ["source_ref", "source_path", "path", "owner_surface"] {
            if text(item.get(key)).is_some() {
                refs = refs
                    .checked_add(1)
                    .ok_or(Error::Budget("owner relation refs"))?;
            }
        }
        if refs == 0 {
            refs = 1;
        }
        add_source_copy(
            state,
            &mut total,
            item,
            1,
            "owner relation source-ref set strings",
        )?;
        add_owned_bytes(
            &mut total,
            string_set_workspace(refs)?,
            "owner relation source-ref set",
        )?;
        add_owned_bytes(
            &mut total,
            crate::knowledge_normalization::serde_array_slots_upper(refs)?,
            "owner relation source-ref output",
        )?;

        if matches!(
            type_id,
            "tos.relation.has-normalized-place" | "tos.relation.has-normalized-agent"
        ) {
            let keys = if type_id == "tos.relation.has-normalized-place" {
                ["spatial_roles", "spatial_literal_forms"]
            } else {
                ["agent_roles", "agent_literal_forms"]
            };
            for key in keys {
                let (count, _) =
                    string_array_count_and_bytes(state, props.and_then(|p| p.get(key)))?;
                add_owned_bytes(
                    &mut total,
                    string_set_workspace(count)?,
                    "owner relation special set",
                )?;
                add_owned_bytes(
                    &mut total,
                    crate::knowledge_normalization::serde_array_slots_upper(count)?,
                    "owner relation special arrays",
                )?;
            }
            // Each of the two branches owns an intermediate Value vector and
            // a sorted String set before its semantic output is assembled.
            add_source_copy(
                state,
                &mut total,
                item,
                4,
                "owner relation special-set intermediates",
            )?;
        }
        let (assertions, conflicts) =
            assertion_output_extras(state, item, &mut total, self.limits.max_output_bytes)?;
        let source_keys = source_key_count(state, item, 0)?;
        let title_keys = source_key_count(state, left_title, 0)?
            .checked_add(source_key_count(state, right_title, 0)?)
            .ok_or(Error::Budget("owner relation title keys"))?;
        let entry_keys = source_key_count(state, entry, 0)?;
        let dynamic_keys = assertions
            .checked_mul(4)
            .and_then(|n| n.checked_add(2048))
            .ok_or(Error::Budget("owner relation digest keys"))?;
        let key_slots = source_keys
            .checked_mul(
                10 + conflicts
                    .checked_mul(2)
                    .ok_or(Error::Budget("owner relation digest keys"))?,
            )
            .and_then(|n| n.checked_add(dynamic_keys))
            .and_then(|n| {
                n.checked_add(title_keys)
                    .and_then(|x| x.checked_add(entry_keys))
            })
            .and_then(|n| n.checked_mul(std::mem::size_of::<&String>()))
            .ok_or(Error::Budget("owner relation digest workspace"))?;
        add_owned_bytes(&mut total, key_slots, "owner relation digest workspace")?;
        add_owned_bytes(
            &mut total,
            crate::knowledge_normalization::serde_object_slots_upper(
                16usize
                    .checked_add(title_keys)
                    .and_then(|n| n.checked_add(entry_keys))
                    .ok_or(Error::Budget("owner relation object slots"))?,
            ),
            "owner relation object slots",
        )?;
        add_owned_bytes(
            &mut total,
            crate::knowledge_normalization::serde_array_slots_upper(assertions)?,
            "owner relation assertion arrays",
        )?;

        let from_graph = text(item.get("from_source_graph")).unwrap_or(source_graph);
        let from_id = text(item.get("from_id")).unwrap_or("unknown-source");
        let to_graph = text(item.get("to_source_graph")).unwrap_or(source_graph);
        let to_id = text(item.get("to_id")).unwrap_or("unknown-target");
        let identity = identity.unwrap_or(native);
        let texts = [
            source_graph.len(),
            1,
            identity.len(),
            native.len(),
            from_graph.len(),
            1,
            from_id.len(),
            to_graph.len(),
            1,
            to_id.len(),
            predicate.len(),
            type_id.len(),
            self.relation_registry_ref.len(),
            default_authority.len(),
            64,
            64,
            30,
            field_text,
        ];
        let dynamic_text = texts
            .into_iter()
            .try_fold(0usize, |n, v| n.checked_add(v))
            .ok_or(Error::Budget("owner relation output text"))?;
        state.charge_work(dynamic_text)?;
        add_owned_bytes(&mut total, dynamic_text, "owner relation output text")?;
        state.hold(total)
    }

    /// Keep the normalized relation and encoded bytes under one owner until a
    /// synchronous unit callback completes.
    pub(crate) fn with_normalized_relation_owned(
        &self,
        source: &SourceRow,
        source_graph: &str,
        identity: Option<&str>,
        left_title: &Value,
        right_title: &Value,
        default_authority: &str,
        output_byte_cap: usize,
        consume: impl FnOnce(&Value, &[u8]) -> Result<()>,
    ) -> Result<()> {
        if output_byte_cap == 0 || output_byte_cap > self.limits.max_output_bytes {
            return Err(Error::Budget("owner relation output callback cap"));
        }
        let state = self
            .owner_state
            .ok_or(Error::Invalid("owned relation state absent"))?;
        let output_hold = self.owned_relation_output_hold(
            source,
            source_graph,
            identity,
            left_title,
            right_title,
            default_authority,
            state,
        )?;
        let value = self.normalize_relation_unstamped(
            source,
            source_graph,
            identity,
            left_title,
            right_title,
            default_authority,
        )?;
        let result = crate::knowledge_normalization::with_content_revision_owned(
            state,
            value,
            output_byte_cap,
            |value, encoded| consume(value, encoded),
        );
        drop(output_hold);
        result
    }

    /// Parse exact row material under the stored state; only unit work escapes.
    pub(crate) fn with_normalized_relation_raw_owned(
        &self,
        raw: &[u8],
        max_raw_bytes: usize,
        source_graph: &str,
        identity: Option<&str>,
        left_title: &Value,
        right_title: &Value,
        default_authority: &str,
        output_byte_cap: usize,
        consume: impl FnOnce(&Value, &[u8]) -> Result<()>,
    ) -> Result<()> {
        let state = self
            .owner_state
            .ok_or(Error::Invalid("owned relation state absent"))?;
        let source =
            SourceRow::parse_scoped_with_optional_owned_state(raw, max_raw_bytes, Some(state))?;
        self.with_normalized_relation_owned(
            &source,
            source_graph,
            identity,
            left_title,
            right_title,
            default_authority,
            output_byte_cap,
            consume,
        )
    }

    pub fn normalize_relation(
        &self,
        source: &SourceRow,
        source_graph: &str,
        identity: Option<&str>,
        left_title: &Value,
        right_title: &Value,
        default_authority: &str,
    ) -> Result<Value> {
        let mut value = self.normalize_relation_unstamped(
            source,
            source_graph,
            identity,
            left_title,
            right_title,
            default_authority,
        )?;
        stamp_content_revision(&mut value, self.limits.max_output_bytes)?;
        Ok(value)
    }

    fn normalize_relation_unstamped(
        &self,
        source: &SourceRow,
        source_graph: &str,
        identity: Option<&str>,
        left_title: &Value,
        right_title: &Value,
        default_authority: &str,
    ) -> Result<Value> {
        if !self.sources.contains(source_graph) {
            return Err(Error::Invalid("owner base unregistered source"));
        }
        for title in [left_title, right_title] {
            if !title.is_object() {
                return Err(Error::Invalid("owner base title object"));
            }
            serde_json::to_writer(
                &mut CappedBytes {
                    bytes: 0,
                    cap: self.limits.max_output_bytes,
                },
                title,
            )
            .map_err(|_| Error::Budget("owner base title bytes"))?;
        }
        let native = required(source.value(), "edge_id")?;
        let item = source.value();
        let (from, to) = endpoints(item, source_graph)?;
        let predicate = text(item.get("predicate_id")).unwrap_or("related_to");
        let resolved = self.registry.relation(source_graph, predicate, "edge");
        let type_id = resolved.type_id;
        let mut entry = self
            .relations
            .get(type_id)
            .cloned()
            .ok_or(Error::Invalid("owner base relation type entry"))?;
        let mapping = entry
            .get("source_mappings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|mapping| {
                mapping.get("source_graph").and_then(Value::as_str) == Some(source_graph)
                    && mapping.get("source_predicate_id").and_then(Value::as_str) == Some(predicate)
                    && mapping.get("scope").and_then(Value::as_str) == Some("edge")
                    && truthy(mapping.get("labels"))
            })
            .cloned();
        if let Some(mapping) = mapping {
            entry["labels"] = mapping["labels"].clone();
            entry["definition"] = mapping.get("definition").cloned().unwrap_or(Value::Null);
            entry["source_mappings"] = json!([mapping]);
        }
        let attributes = attrs(item, true)?;
        let mut semantics = Map::new();
        if let Some(context) = direct_assertion_context(source, self.limits.max_output_bytes)? {
            semantics.insert("assertion_contexts".into(), json!([context]));
        }
        for (special, key, roles, forms) in [
            (
                "tos.relation.has-normalized-place",
                "space",
                "spatial_roles",
                "spatial_literal_forms",
            ),
            (
                "tos.relation.has-normalized-agent",
                "responsibility",
                "agent_roles",
                "agent_literal_forms",
            ),
        ] {
            if type_id == special {
                let p = item.get("properties");
                let sorted = |key| {
                    strings(p.and_then(|p| p.get(key)))
                        .into_iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect::<BTreeSet<_>>()
                };
                semantics.insert(key.into(),json!({"roles":sorted(roles),"literal_forms":sorted(forms),"normalization_status":"source-declared"}));
            }
        }
        let mut value = json!({"id":format!("{}:{}",source_graph,identity.unwrap_or(native)),"native_id":native,"source_graph":source_graph,
            "from_id":from,"to_id":to,"predicate_id":predicate,"relation_type_id":type_id,
            "predicate_mapping":{"status":if resolved.mapped {"mapped"} else {"unmapped"},"source_predicate_id":predicate,"registry_ref":self.relation_registry_ref},
            "display":full_owner_relation_display(source,predicate,left_title,right_title,&entry)?,"epistemic":source_epistemic(item,default_authority),
            "graph_layers":layers(item),"view_ids":strings(item.get("view_ids")),"source_refs":source.source_refs(&[]),
            "source_record":source.source_record(&attributes)?,"attributes":attributes,"semantics":semantics});
        Ok(value)
    }
}
fn endpoints(item: &Value, source: &str) -> Result<(String, String)> {
    Ok((
        format!(
            "{}:{}",
            text(item.get("from_source_graph")).unwrap_or(source),
            text(item.get("from_id")).unwrap_or("unknown-source")
        ),
        format!(
            "{}:{}",
            text(item.get("to_source_graph")).unwrap_or(source),
            text(item.get("to_id")).unwrap_or("unknown-target")
        ),
    ))
}
