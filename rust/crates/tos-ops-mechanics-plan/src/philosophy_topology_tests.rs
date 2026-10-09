use super::*;
use serde_json::json;
fn root() -> std::path::PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap().to_path_buf() }
fn context<'a>(root:&Path,cancel:&'a AtomicI32)->Context<'a> { Context { source:RouteSources::new(root).unwrap(),cancel,issues:Vec::new(),issue_bytes:0 } }
#[test]
fn planting_schema_preserves_namespaces_and_refuses_false_promotions() {
    let schema:Value=serde_json::from_str(include_str!("../../../../ToS/contracts/philosophy-source-planting.schema.json")).unwrap();
    let validator=jsonschema::options().with_draft(jsonschema::Draft::Draft202012).should_validate_formats(true).offline().build(&schema).unwrap();
    let root=root();let base="ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/proto-cuneiform-accounting-ontologies/sources/plantings";
    let load=|name:&str|->Value {serde_json::from_slice(&std::fs::read(root.join(format!("{base}/{name}/source-planting.json"))).unwrap()).unwrap()};
    let plain=load("cdli-p000015");let composite=load("dcclt-q000023");let biblio=load("atu2-green-sign-list");let work=load("atu3-lexical-lists");let primary=load("cdlb-2021-6-quantitative-sign-use");let control=load("born-kelley-2021-tribute-control");
    for value in [&plain,&composite,&biblio,&work,&primary,&control]{assert!(validator.is_valid(value),"{:?}",validator.iter_errors(value).map(|e|e.to_string()).collect::<Vec<_>>());}
    assert_eq!(primary["source_witness"],control["source_witness"]);assert_eq!(BTreeSet::from([primary["source_backlog_anchor"]["line"].as_u64().unwrap(),control["source_backlog_anchor"]["line"].as_u64().unwrap()]),BTreeSet::from([16,22]));
    assert_eq!(primary["authority"]["source_text_admitted"],false);assert_eq!(control["authority"]["semantic_status"],"not_started");
    for id in ["A12","T2-39","T3-01","T4-01"] {let mut changed=plain.clone();changed["atlas_row_id"]=json!(id);changed["dossier_id"]=json!(id);assert_eq!(validator.is_valid(&changed),id!="T4-01");}
    for (mut changed,pointer,value) in [(plain.clone(),"/authority/graph_status",json!("promoted")),(plain,"/authority/human_task_created",json!(true)),(composite,"/authority/source_text_admitted",json!(true))] {*changed.pointer_mut(pointer).unwrap()=value;assert!(!validator.is_valid(&changed));}
    let mut changed=biblio;changed["source_witness"].as_object_mut().unwrap().remove("membership_claim_ref");assert!(!validator.is_valid(&changed));
    let mut changed=work;changed["source_witness"]["container_id"]=json!("tos.collection.proto-cuneiform.unsupported-wrapper");assert!(!validator.is_valid(&changed));
}
#[test]
fn planting_atlas_membership_requires_exact_current_row_and_branch() {
    let root=root();let cancel=AtomicI32::new(0);
    let paths=["table-i","table-ii","table-iii"].map(|table|format!("ToS/philosophy/atlas/master-tables/{table}/rows.jsonl"));
    let mut planting=json!({"atlas_row_id":"T2-39","dossier_id":"T2-39","branch_path":"ToS/philosophy/eras/medieval-worlds/regions/sri-lanka/traditions/pali-scholastic-commentarial-handoff"});
    let mut c=context(&root,&cancel);atlas_membership(&mut c,&planting,"fixture",&paths).unwrap();assert!(c.issues.is_empty(),"{:?}",c.issues);
    for (id,needle) in [("T2-99","planting atlas row must resolve exactly once"),("A12","planting atlas row does not belong to the exact branch")] {
        planting["atlas_row_id"]=json!(id);planting["dossier_id"]=json!(id);let mut c=context(&root,&cancel);atlas_membership(&mut c,&planting,"fixture",&paths).unwrap();assert!(c.issues.iter().any(|(_,m)|m.contains(needle)),"{:?}",c.issues);
    }
}
