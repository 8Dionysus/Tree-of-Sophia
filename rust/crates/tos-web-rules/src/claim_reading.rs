//! Compact Claim reading rules. The host supplies strict identity observations
//! and selected metadata, never graph bodies or wording. Source objects stay in JS.
use std::collections::HashSet;
use tos_foundation::{emit_value_preserved_json, parse_json, JsonLimits, JsonMode, JsonValue};
const ERROR: &str = "invalid_claim_reading";
fn get<'a>(v: &'a JsonValue, k: &str) -> &'a JsonValue { v.object_get(k).unwrap_or(&JsonValue::Null) }
fn yes(v: &JsonValue, k: &str) -> bool { matches!(get(v,k), JsonValue::Bool(true)) }
fn text(v: &JsonValue, k: &str, s: &str) -> bool { get(v,k).as_str()==Some(s) }
fn list(v: &JsonValue) -> Result<&[JsonValue], &'static str> {
    let a=v.as_array().ok_or(ERROR)?;
    if a.iter().any(|x| !matches!(x,JsonValue::String(_)) && !yes(x,"hole")) { return Err(ERROR); } Ok(a)
}
fn distinct(a: &[JsonValue]) -> bool {
    let mut words=HashSet::new(); let mut hole=false;
    a.iter().all(|v| match v {
        JsonValue::String(s)=>words.insert(s.units()),
        _ if yes(v,"hole")=>{let fresh=!hole;hole=true;fresh},
        _=>false,
    })
}
fn check(b: bool) -> Result<(), &'static str> { if b {Ok(())} else {Err(ERROR)} }
fn context_declaration(v: &JsonValue) -> Result<(), &'static str> {
    check(yes(v,"reading_object") && (text(v,"mode","claim-with-mandatory-context") || text(v,"mode","claim-with-shared-form-context-v2"))
        && yes(v,"standalone_false") && (text(v,"wording_state","available") || text(v,"wording_state","missing"))
        && yes(v,"context_pointers_exact"))?;
    check(distinct(list(get(v,"relation_context_ids"))?))
}
fn context_source(v: &JsonValue) -> Result<(), &'static str> {
    check(yes(v,"node_found") && yes(v,"revision_equal") && yes(v,"semantics_object") && yes(v,"epistemic_object"))
}
fn closure_path(v: &JsonValue) -> Result<(), &'static str> {
    check(yes(v,"path_object") && yes(v,"path_owned") && yes(v,"id_nonempty") && yes(v,"relation_type_string")
        && yes(v,"middle_equal") && yes(v,"reading_node_equal"))?;
    check(list(get(v,"node_ids"))?.len()==3 && list(get(v,"relation_ids"))?.len()==2)?;
    list(get(v,"detail_relation_ids"))?; Ok(())
}
fn closure_claim(v: &JsonValue) -> Result<(), &'static str> {
    check(yes(v,"claim_object") && text(v,"mapping_status","mapped") && yes(v,"predicate_equal") && yes(v,"subject_equal")
        && yes(v,"object_equal") && yes(v,"endpoints_different"))
}
fn closure_relations(v: &JsonValue) -> Result<Vec<JsonValue>, &'static str> {
    let nodes=list(get(v,"node_ids"))?; let primary=list(get(v,"relation_ids"))?; let detail=list(get(v,"detail_relation_ids"))?;
    check(nodes.len()==3 && primary.len()==2)?;
    let ids: Vec<JsonValue> = primary.iter().chain(detail).cloned().collect();
    let relations=get(v,"relations").as_array().ok_or(ERROR)?;
    check(distinct(&ids) && ids.len()==relations.len() && yes(v,"relation_set_complete"))?;
    for (i,_id) in primary.iter().enumerate() {
        let r=get(v,"primary_selected").as_array().and_then(|a|a.get(i)).ok_or(ERROR)?;
        check(yes(r,"from_claim") && get(r,"to_id")==&nodes[if i==0 {0}else{2}]
            && text(r,"relation_type_id",if i==0 {"tos.relation.has-subject"}else{"tos.relation.has-object"}))?;
    }
    let mut members=Vec::new();
    for (i,_id) in detail.iter().enumerate() {
        let r=get(v,"detail_selected").as_array().and_then(|a|a.get(i)).ok_or(ERROR)?;
        check(yes(r,"from_claim") && (text(r,"relation_type_id","tos.relation.claim-supported-by") || text(r,"relation_type_id","tos.relation.claim-value-member")))?;
        if text(r,"relation_type_id","tos.relation.claim-value-member") {members.push(get(r,"to_id").clone());}
    }
    Ok(members)
}
fn closure_members(v: &JsonValue) -> Result<(), &'static str> {
    let members=get(v,"members").as_array().ok_or(ERROR)?;
    if yes(v,"has_member_declaration") || yes(v,"has_member_edges") {
        let declared=list(get(v,"member_ids"))?;
        check(!declared.is_empty() && declared.iter().all(|x| matches!(x,JsonValue::String(s) if !s.units().is_empty()))
            && distinct(declared) && members.len()==declared.len() && yes(v,"member_count_complete")
            && distinct(&members) && members.iter().all(|x|declared.contains(x)))?;
    }
    Ok(())
}
pub fn validate_claim_reading_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    // Fixed-depth selected metadata adds no new source acceptance quota.
    let limits=JsonLimits {max_bytes:raw.len().max(1), max_visits:raw.len().max(1), max_depth:16, max_integer_digits:32};
    let doc=parse_json(raw,JsonMode::RequestLastWins,limits).map_err(|_|ERROR)?;
    let v=doc.root();
    let output=match get(v,"op").as_str() {
        Some("context-declaration")=>{context_declaration(v)?; JsonValue::Null},
        Some("context-source")=>{context_source(v)?; JsonValue::Null},
        Some("context-relation")=>{check(yes(v,"found"))?; JsonValue::Null},
        Some("closure-path")=>{closure_path(v)?; JsonValue::Null},
        Some("closure-claim")=>{closure_claim(v)?; JsonValue::Null},
        Some("closure-relations")=>JsonValue::Array(closure_relations(v)?),
        Some("closure-members")=>{closure_members(v)?; JsonValue::Null},
        Some("closure-nodes")=>{check(yes(v,"closure_nodes_present"))?; JsonValue::Null},
        Some("path-header")=>{
            check(yes(v,"compact_absent") || (text(v,"scene_schema","tos_knowledge_scene_v1") && yes(v,"compact_object")
                && text(v,"rule","explicit-claim-paths-v1") && text(v,"authority","presentation-only-no-new-assertion")
                && yes(v,"paths_array")))?; JsonValue::Null
        },
        Some("path")=>{
            let matches=get(v,"matches").as_array().ok_or(ERROR)?;
            check(matches.len()<=1)?; matches.first().cloned().unwrap_or(JsonValue::Null)
        },
        Some("wording")=>{
            let shared=text(v,"mode","claim-with-shared-form-context-v2");
            check(!shared || text(v,"selection_schema","tos_human_form_selection_v2"))?;
            if text(v,"wording_state","missing") {check(yes(v,"pointer_null"))?; JsonValue::Null}
            else {
                let path=get(v,"pointer").as_str().ok_or(ERROR)?;
                let mut selected=None;
                for role in ["caption","statement","hover"] {
                    let expected=if shared {format!("/human_form_selection/roles/{role}")} else {format!("/human_form_selection/roles/{role}/packet")};
                    if path==expected {
                        check(shared || text(v,"selection_schema","tos_human_form_selection_v1"))?;
                        check(text(get(v,"role_states"),role,"ready"))?;
                        selected=Some(get(get(v,"role_selectors"),role).clone()); break;
                    }
                }
                match selected {Some(s)=>s,None=>{
                    check(!shared && ["/display_selection/fields/summary","/display_selection/fields/title"].contains(&path)
                        && yes(v,"display_object") && yes(v,"display_available"))?;
                    get(v,"display_selector").clone()
                }}
            }
        },
        _=>return Err(ERROR),
    };
    emit_value_preserved_json(&output,limits).map_err(|_|ERROR)
}
