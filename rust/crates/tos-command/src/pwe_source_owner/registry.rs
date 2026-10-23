//! Actual selected registration, not Kind::outer_predicate as a registration.
use super::{HeldFrames,invalid,unsupported};
use crate::source_command::{self as cmd,SourceCommandError,SourceCommandResult};
use tos_compiler::pwe_native::Kind;
use tos_foundation::JsonValue;

pub(crate) const BINDINGS:&str="ToS/doctrine/semantic-interchange/pwe-bindings.v1.json";
const SCHEMA:&str="ToS/contracts/pwe-bindings.schema.json";
const RELATIONS:&str="ToS/doctrine/semantic-interchange/relation-types.v1.json";
const RELATION_SCHEMA:&str="ToS/contracts/semantic-relation-type-registry.schema.json";
pub(crate) struct DispatchSpec {
    pub predicate:String,pub kind:Kind,pub relation_type_id:String,
    pub schema_version:String,pub schema_path:String,pub assertion_layer:String,
    pub arguments:Vec<String>,
}
pub(crate) struct BindingRegistry { specs:Vec<DispatchSpec> }
impl BindingRegistry {
    pub(crate) fn load(frames:&mut HeldFrames<'_,'_>)->SourceCommandResult<Self> {
        let bindings=frames.selected_document(BINDINGS,SCHEMA)?;
        let relations=frames.selected_document(RELATIONS,RELATION_SCHEMA)?;
        if cmd::text(&bindings,"schema_version")?!="tos_pwe_bindings_v1"
            || cmd::text(&bindings,"registry_id")?!="tos.pwe.bindings" {
            return Err(invalid("PWE binding registry identity"));
        }
        let entries=cmd::array(&bindings,"bindings")?;
        if entries.len()!=3{return Err(invalid("PWE registry must declare exactly three account readers"));}
        let mut specs=Vec::new();specs.try_reserve_exact(3).map_err(|_|unsupported("PWE dispatch reservation"))?;
        for row in entries {
            let predicate=cmd::text(row,"predicate")?;
            let kind=match cmd::text(row,"value_kind")? {
                "name-assignment"=>Kind::Name,"responsibility-account"=>Kind::Responsibility,
                "identity-correspondence"=>Kind::Identity,_=>return Err(invalid("PWE registered value kind")),
            };
            if kind.outer_predicate()!=predicate || specs.iter().any(|s:&DispatchSpec|s.kind==kind) {
                return Err(invalid("PWE registry predicate/kind uniqueness"));
            }
            let mut selected=None;
            for relation in cmd::array(&relations,"relations")? {
                for mapping in cmd::array(relation,"source_mappings")? {
                    if mapping.object_get("source_graph").and_then(JsonValue::as_str)==Some("source-claims")
                        && mapping.object_get("source_predicate_id").and_then(JsonValue::as_str)==Some(predicate) {
                        if selected.is_some(){return Err(SourceCommandError::Conflict("PWE source relation mapping ambiguous"));}
                        selected=Some(relation);
                    }
                }
            }
            let relation=selected.ok_or(unsupported("PWE predicate has no actual registered source mapping"))?;
            if cmd::text(relation,"relation_type_id")?!=cmd::text(row,"relation_type_id")? {
                return Err(SourceCommandError::Conflict("PWE selected relation differs from binding registry"));
            }
            let profile=cmd::field(relation,"source_claim_profile")?;
            if cmd::text(profile,"reader")?!="pwe-exact-basis-v1"
                || cmd::field(profile,"profile_version")?.as_u64()!=Some(1)
                || cmd::text(profile,"value_kind")?!=cmd::text(row,"value_kind")? {
                return Err(unsupported("PWE actual mandatory exact-basis reader differs"));
            }
            let schema_version=cmd::text(row,"schema_version")?;let schema_path=cmd::text(row,"schema_ref")?;
            let routes=cmd::array(profile,"schemas")?;
            let mut found=false;
            for route in routes {if route.object_get("schema_version").and_then(JsonValue::as_str)==Some(schema_version) {
                if found || cmd::text(route,"schema_ref")?!=schema_path{return Err(SourceCommandError::Conflict("PWE selected schema route differs"));}
                found=true;
                for dependency in cmd::array(route,"schema_dependencies")? {
                    let name=dependency.as_str().ok_or(invalid("PWE schema dependency path"))?;
                    if frames.worker.selected_contract_digest(name).is_none(){return Err(unsupported("PWE required schema dependency not selected"));}
                }
            }}
            if !found{return Err(unsupported("PWE source schema version not registered"));}
            let layer=cmd::text(row,"assertion_layer")?;
            if !cmd::array(profile,"assertion_layers")?.iter().any(|v|v.as_str()==Some(layer)) {
                return Err(SourceCommandError::Conflict("PWE registered assertion layer differs"));
            }
            let arguments=cmd::array(row,"arguments")?.iter().map(|x|x.as_str().map(str::to_owned).ok_or(invalid("PWE argument field")))
                .collect::<SourceCommandResult<Vec<_>>>()?;
            specs.push(DispatchSpec{predicate:predicate.to_owned(),kind,relation_type_id:cmd::text(row,"relation_type_id")?.to_owned(),
                schema_version:schema_version.to_owned(),schema_path:schema_path.to_owned(),assertion_layer:layer.to_owned(),arguments});
        }
        // Only the bounded three-row selection is retained. Large input
        // registries stay transient under the existing parser workspace cap.
        frames.retain(8192)?;frames.verify()?;Ok(Self{specs})
    }
    pub(crate) fn select(&self,kind:Kind)->SourceCommandResult<&DispatchSpec> {
        self.specs.iter().find(|s|s.kind==kind).ok_or(unsupported("PWE account has no selected dispatch"))
    }
}

use tos_validation::source_cut::CutSchemaExecutor;
