//! Selected source declarations, with every variable lookup/comparison paid.
use super::{HeldFrames, invalid, unsupported};
use crate::source_command::{SourceCommandError, SourceCommandResult};
use tos_compiler::pwe_native::Kind;
use tos_foundation::JsonValue;

pub(crate) const BINDINGS: &str = "ToS/doctrine/semantic-interchange/pwe-bindings.v1.json";
const SCHEMA: &str = "ToS/contracts/pwe-bindings.schema.json";
const RELATIONS: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const RELATION_SCHEMA: &str = "ToS/contracts/semantic-relation-type-registry.schema.json";
pub(crate) struct DispatchSpec {
    pub predicate: String, pub kind: Kind, pub relation_type_id: String,
    pub schema_version: String, pub schema_path: String, pub assertion_layer: String,
    pub arguments: Vec<String>,
}
pub(crate) struct BindingRegistry { specs: Vec<DispatchSpec> }
impl BindingRegistry {
    pub(crate) fn load(frames: &mut HeldFrames<'_, '_>) -> SourceCommandResult<Self> {
        let bindings = frames.selected_document(BINDINGS, SCHEMA)?;
        let relations = frames.selected_document(RELATIONS, RELATION_SCHEMA)?;
        let schema = frames.native_text(&bindings, "schema_version")?;
        let registry_id = frames.native_text(&bindings, "registry_id")?;
        if !frames.text_equal(schema, "tos_pwe_bindings_v1")? || !frames.text_equal(registry_id, "tos.pwe.bindings")? { return Err(invalid("PWE binding registry identity")); }
        let entries = frames.native_array(&bindings, "bindings")?;
        if entries.len() != 3 { return Err(invalid("PWE registry must declare exactly three account readers")); }
        // Preserve the prior logical registry holder fee; new copies also
        // receive prospective per-holder admission on these same ledgers.
        frames.retain(8192)?;
        frames.retain(3 * std::mem::size_of::<DispatchSpec>())?;
        let mut specs = Vec::new();
        specs.try_reserve_exact(3).map_err(|_| unsupported("PWE dispatch reservation"))?;
        for row in entries {
            let predicate = frames.native_text(row, "predicate")?;
            let value_kind = frames.native_text(row, "value_kind")?;
            let kind = if frames.text_equal(value_kind, "name-assignment")? { Kind::Name }
                else if frames.text_equal(value_kind, "responsibility-account")? { Kind::Responsibility }
                else if frames.text_equal(value_kind, "identity-correspondence")? { Kind::Identity }
                else { return Err(invalid("PWE registered value kind")); };
            frames.inspect_access(0, specs.len())?;
            if !frames.text_equal(kind.outer_predicate(), predicate)? || specs.iter().any(|s: &DispatchSpec| s.kind == kind) { return Err(invalid("PWE registry predicate/kind uniqueness")); }
            let mut selected = None;
            for relation in frames.native_array(&relations, "relations")? {
                for mapping in frames.native_array(relation, "source_mappings")? {
                    let graph = frames.native_get(mapping, "source_graph")?.and_then(JsonValue::as_str);
                    let source_predicate = frames.native_get(mapping, "source_predicate_id")?.and_then(JsonValue::as_str);
                    if match_text(frames, graph, "source-claims")? && match_text(frames, source_predicate, predicate)? {
                        if selected.is_some() { return Err(SourceCommandError::Conflict("PWE source relation mapping ambiguous")); }
                        selected = Some(relation);
                    }
                }
            }
            let relation = selected.ok_or(unsupported("PWE predicate has no actual registered source mapping"))?;
            let relation_id = frames.native_text(relation, "relation_type_id")?;
            let declared_relation_id = frames.native_text(row, "relation_type_id")?;
            if !frames.text_equal(relation_id, declared_relation_id)? { return Err(SourceCommandError::Conflict("PWE selected relation differs from binding registry")); }
            let profile = frames.native_field(relation, "source_claim_profile")?;
            let reader = frames.native_text(profile, "reader")?;
            let profile_version = frames.native_field(profile, "profile_version")?.as_u64();
            let profile_kind = frames.native_text(profile, "value_kind")?;
            if !frames.text_equal(reader, "pwe-exact-basis-v1")? || profile_version != Some(1) || !frames.text_equal(profile_kind, value_kind)? { return Err(unsupported("PWE actual mandatory exact-basis reader differs")); }
            let schema_version = frames.native_text(row, "schema_version")?;
            let schema_path = frames.native_text(row, "schema_ref")?;
            let mut found = false;
            for route in frames.native_array(profile, "schemas")? {
                let actual_version = frames.native_get(route, "schema_version")?.and_then(JsonValue::as_str);
                if match_text(frames, actual_version, schema_version)? {
                    let actual_path = frames.native_text(route, "schema_ref")?;
                    if found || !frames.text_equal(actual_path, schema_path)? { return Err(SourceCommandError::Conflict("PWE selected schema route differs")); }
                    found = true;
                    for dependency in frames.native_array(route, "schema_dependencies")? {
                        let name = dependency.as_str().ok_or(invalid("PWE schema dependency path"))?;
                        if frames.contract_digest(name)?.is_none() { return Err(unsupported("PWE required schema dependency not selected")); }
                    }
                }
            }
            if !found { return Err(unsupported("PWE source schema version not registered")); }
            let layer = frames.native_text(row, "assertion_layer")?;
            let mut layer_found = false;
            for value in frames.native_array(profile, "assertion_layers")? {
                if match_text(frames, value.as_str(), layer)? { layer_found = true; break; }
            }
            if !layer_found { return Err(SourceCommandError::Conflict("PWE registered assertion layer differs")); }
            let mut arguments = Vec::new();
            let declared_arguments = frames.native_array(row, "arguments")?;
            frames.retain(declared_arguments.len().checked_mul(std::mem::size_of::<String>()).ok_or(unsupported("PWE argument vector holder"))?)?;
            arguments.try_reserve_exact(declared_arguments.len()).map_err(|_| unsupported("PWE argument vector reservation"))?;
            for value in declared_arguments {
                let argument = value.as_str().ok_or(invalid("PWE argument field"))?;
                arguments.push(copy_text(frames, argument)?);
            }
            specs.push(DispatchSpec {
                predicate: copy_text(frames, predicate)?, kind,
                relation_type_id: copy_text(frames, relation_id)?,
                schema_version: copy_text(frames, schema_version)?,
                schema_path: copy_text(frames, schema_path)?,
                assertion_layer: copy_text(frames, layer)?, arguments,
            });
        }
        frames.verify()?;
        Ok(Self { specs })
    }
    pub(crate) fn select(&self, kind: Kind) -> SourceCommandResult<&DispatchSpec> {
        // Exactly three enum tags, no variable-length lookup or comparison.
        self.specs.iter().find(|s| s.kind == kind).ok_or(unsupported("PWE account has no selected dispatch"))
    }
}
fn match_text(frames: &mut HeldFrames<'_, '_>, actual: Option<&str>, expected: &str) -> SourceCommandResult<bool> {
    match actual { Some(actual) => frames.text_equal(actual, expected), None => Ok(false) }
}
fn copy_text(frames: &mut HeldFrames<'_, '_>, text: &str) -> SourceCommandResult<String> {
    frames.inspect_access(text.len(), 1)?;
    frames.retain(text.len().checked_add(std::mem::size_of::<String>()).ok_or(unsupported("PWE string holder overflow"))?)?;
    Ok(text.to_owned())
}
