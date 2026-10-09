//! Retained source routes and mechanics navigation enter through the native CLI.
//! No legacy ToS interpreter or oracle is available to the selected executable.
use std::{fs, path::{Path, PathBuf}, process::{Command, Output}, time::{SystemTime, UNIX_EPOCH}};
use serde_json::{Value, json};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root=Self(std::env::temp_dir().join(format!("tos-source-routes-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())));
        fs::create_dir_all(&root.0).unwrap();
        assert!(Command::new("/usr/bin/git").args(["init","--quiet"]).current_dir(&root.0).status().unwrap().success());root
    }
    fn write(&self,p:&str,text:&str) { let p=self.0.join(p);fs::create_dir_all(p.parent().unwrap()).unwrap();fs::write(p,text).unwrap(); }
    fn track(&self) { assert!(Command::new("/usr/bin/git").args(["add","--","."]).current_dir(&self.0).status().unwrap().success()); }
}
impl Drop for Fixture { fn drop(&mut self){let _=fs::remove_dir_all(&self.0);} }
fn invoke(root:&Path, flag:&str) -> Output {invoke_args(root,&[flag])}
fn invoke_args(root:&Path, args:&[&str]) -> Output {
    let exe=std::env::var_os("TOS_SOURCE_ROUTES_TEST_EXECUTABLE").unwrap_or_else(||env!("CARGO_BIN_EXE_tos-ops-mechanics-plan").into());
    let out=Command::new("/usr/bin/timeout").args(["--kill-after=2","45"]).arg(exe)
        .arg("--repo-root").arg(root).args(["--python","/no-python-executable"]).args(args)
        .env_clear().env("PATH","/usr/bin").output().unwrap();
    assert!(out.stdout.len()+out.stderr.len()<2*1024*1024);assert_ne!(out.status.code(),Some(124));out
}
fn text(out:&Output)->String {format!("{}{}",String::from_utf8_lossy(&out.stdout),String::from_utf8_lossy(&out.stderr))}
#[test]
fn current_source_routes_run_natively_and_preserve_authority_bounds() {
    let root=Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap();let mut failures=Vec::new();
    for flag in ["--tiny-entry-validate","--lived-witness-validate","--intake-pack-validate","--tree-node-validate","--philosophy-topology"] {
        let out=invoke(root,flag);if !out.status.success(){failures.push(format!("{flag}: {}",text(&out)));}
        if flag=="--lived-witness-validate" && out.status.success(){let t=text(&out);assert!(t.contains("structure and private boundary only") && t.contains("remain unvalidated"),"{t}");}
    }
    assert!(failures.is_empty(),"{}",failures.join("\n"));
}
#[test]
fn canonical_identity_duplicates_and_exact_json_refuse_through_native_entry() {
    let f=Fixture::new();
    f.write("ToS/contracts/tos-node-contract.schema.json",include_str!("../../../../ToS/contracts/tos-node-contract.schema.json"));
    let body=include_str!("../../../../ToS/public-compatibility/source_node.example.json");
    let first="ToS/canon/source/a/node.json";let second="ToS/canon/source/b/node.json";f.write(first,body);
    let out=invoke(&f.0,"--tree-node-validate");assert!(out.status.success(),"{}",text(&out));
    f.write(second,body);let out=invoke(&f.0,"--tree-node-validate");assert_eq!(out.status.code(),Some(1));assert!(text(&out).contains("duplicate canonical node_id"));fs::remove_file(f.0.join(second)).unwrap();
    for suffix in ["\"node_id\":\"tos.source.another\"","\"unrecognized\":NaN","\"unrecognized\":Infinity","\"unrecognized\":1e9999"] {
        f.write(first,&format!("{},{} }}",body.trim_end().strip_suffix('}').unwrap(),suffix));
        let out=invoke(&f.0,"--tree-node-validate");assert_eq!(out.status.code(),Some(1));assert!(text(&out).contains("invalid exact node JSON"),"{}",text(&out));
    }
}
fn mechanics_fixture()->(Fixture, Value) {
    let f=Fixture::new();
    f.write("mechanics/AGENTS.md","# Operation owner\n");
    f.write("mechanics/README.md","## Task-to-owner map\n## Progressive disclosure\ntopology.json validation_lanes.json docs/decisions/README.md\n[agon](agon/README.md)\n");
    for p in ["AGENTS.md","PROVENANCE.md","ROADMAP.md","parts/intake/README.md"] {f.write(&format!("mechanics/agon/{p}"),"# Route\n");}
    f.write("mechanics/agon/README.md","[Parts](PARTS.md) [Source](PROVENANCE.md) [Future](ROADMAP.md)\n");
    f.write("mechanics/agon/PARTS.md","[Intake](parts/intake/README.md)\n");
    f.write("docs/validation/script_inventory.json","{\"script_surfaces\":[]}\n");
    f.write("tests/test_inventory.json","{\"tests\":[]}\n");
    let topology=json!({"schema_version":"tos_mechanics_topology_v2","owner_repo":"Tree-of-Sophia","root":"mechanics/","legacy_policy":"pinned-git-history-no-active-legacy","always_on_context_budget":{"surface":"mechanics/README.md","metric":"whitespace_tokens_v1","max_tokens":1000},"packages":[{"slug":"agon","class":"local","status":"active","active_parts":["intake"],"legacy_required":false}],"moved_path_accounting":{},"moved_path_targets":{}});
    f.write("mechanics/topology.json",&topology.to_string());f.track();let out=invoke(&f.0,"--mechanics-topology-validate");assert!(out.status.success(),"{}",text(&out));(f,topology)
}
#[test]
fn mechanics_package_selection_history_and_context_guards_are_native() {
    let (f,mut topology)=mechanics_fixture();
    topology["packages"][0]["legacy_required"]=json!(true);f.write("mechanics/topology.json",&topology.to_string());f.write("mechanics/agon/legacy/README.md","# Retired\n");
    let t=text(&invoke(&f.0,"--mechanics-topology-validate"));assert!(t.contains("must remain false")&&t.contains("retired archives"),"{t}");
    topology["packages"][0]["legacy_required"]=json!(false);topology["always_on_context_budget"]["max_tokens"]=json!(2);f.write("mechanics/topology.json",&topology.to_string());fs::remove_dir_all(f.0.join("mechanics/agon/legacy")).unwrap();
    f.write("mechanics/agon/PARTS.md","<!-- [hidden](parts/intake/README.md) -->\n```md\n[example](parts/intake/README.md)\n```\n");
    let t=text(&invoke(&f.0,"--mechanics-topology-validate"));assert!(t.contains("selection map")&&t.contains("always-on context exceeds"),"{t}");
}
#[test]
fn mechanics_document_routes_preserve_links_fragments_fences_and_tracked_executables() {
    let (f,_)=mechanics_fixture();let doc="mechanics/agon/parts/intake/README.md";
    f.write("mechanics/agon/scripts/run.sh","#!/bin/sh\nexit 0\n");f.track();
    f.write(doc,"Run `../../scripts/run.sh`.\n");let out=invoke(&f.0,"--mechanics-topology-validate");assert!(out.status.success(),"{}",text(&out));
    // The tracked namespace, not an inventory claim, owns executable membership.
    f.write("mechanics/agon/scripts/untracked.sh","#!/bin/sh\nexit 0\n");f.write(doc,"Run `../../scripts/untracked.sh`.\n");
    let t=text(&invoke(&f.0,"--mechanics-topology-validate"));assert!(t.contains("absent from tracked script namespace"),"{t}");
    f.write(doc,"Run `scripts/missing.py`.\n");assert!(text(&invoke(&f.0,"--mechanics-topology-validate")).contains("stale executable reference"));
    f.write(doc,"Run `../../scripts/run.sh`.\n");fs::remove_file(f.0.join("mechanics/agon/scripts/run.sh")).unwrap();
    assert!(text(&invoke(&f.0,"--mechanics-topology-validate")).contains("stale executable reference"));
    f.write("mechanics/agon/target.md","# Present\n");
    for fence in ["```","~~~"] {
        let source=format!("[guide][route]\n[route]: ../../target.md#present\n\n{fence}python\nplan['authorization']['scope']\n[example](missing-example.md)\n[example]: missing-example-definition.md\n[fenced-only]: ../../target.md\n{fence}\n");f.write(doc,&source);
        let out=invoke(&f.0,"--mechanics-topology-validate");assert!(out.status.success(),"{}",text(&out));
        f.write(doc,&(source+"\n[hidden definition][fenced-only]\n[undefined][]\n[broken][broken-route]\n[broken-route]: missing-prose.md\n"));
        let t=text(&invoke(&f.0,"--mechanics-topology-validate"));for expected in ["unresolved reference-style documentation route: fenced-only","unresolved reference-style documentation route: undefined","broken local documentation route: missing-prose.md"]{assert!(t.contains(expected),"{t}");}
    }
    for (body,needle) in [("```sh\npython scripts/missing.py\n```\n","stale executable reference"),("[details](README.md#missing)\n","broken local documentation fragment"),("[outside](../../../../../outside.md)\n","broken local documentation route"),("[guide][route]\n[route]: missing.md\n","broken local documentation route")] {
        f.write(doc,body);let out=invoke(&f.0,"--mechanics-topology-validate");assert_eq!(out.status.code(),Some(1));assert!(text(&out).contains(needle),"{}",text(&out));
    }
}

fn philosophy_fixture()->(Fixture,Value,Value) {
    let f=Fixture::new();for path in ["ToS/philosophy/AGENTS.md","ToS/philosophy/README.md","ToS/research-packets/AGENTS.md","ToS/research-packets/deep-research/philosophy/AGENTS.md"]{f.write(path,"");}
    let packet=json!({"schema_version":"tos_research_packet_v1","path":"ToS/research-packets/deep-research/philosophy","domain_branch":"ToS/philosophy","authority":{"source_status":"not_source_witness","canon_status":"not_canon"},"capture_container":{"page_id":"fixture-page","title":"Fixture Packet"},"branch_child_pages":[{"id":"fixture-child-page","title":"Fixture Child"}]});
    let manifest=json!({"schema_version":"tos_philosophy_topology_v1","branch_id":"philosophy","path":"ToS/philosophy","boundary_routes":{"provisional_extraction":"ToS/candidate-intake","research_packets":"ToS/research-packets","source_witnesses":"ToS/source-witnesses","canon_promotion":"ToS/canon"},"path_component_policy":{"repository_paths_describe":["philosophy branch"],"metadata_only_inputs":["capture titles"]},"mature_branch_shape":["eras/<era>","eras/<era>/regions/<region>","eras/<era>/regions/<region>/traditions/<tradition>"],"promotion_pipeline":["research packet","branch review","local branch skeleton","proposed nodes","proposed relations","relation pack","canon promotion","derived graph/export"],"research_packet_routes":["ToS/research-packets/deep-research/philosophy/research.manifest.json"],"branch_manifests":["ToS/philosophy/trunk/branch.manifest.json"]});
    f.write("ToS/philosophy/philosophy.manifest.json",&manifest.to_string());f.write("ToS/research-packets/deep-research/philosophy/research.manifest.json",&packet.to_string());
    f.write("ToS/philosophy/trunk/branch.manifest.json",&json!({"path":"ToS/philosophy/trunk","branch_id":"philosophy.trunk","role":"fixture trunk"}).to_string());
    let out=invoke(&f.0,"--philosophy-topology");assert!(out.status.success(),"{}",text(&out));(f,manifest,packet)
}
#[test]
fn philosophy_routes_preserve_branch_counts_metadata_names_and_path_boundaries() {
    let (f,manifest,packet)=philosophy_fixture();let manifest_path="ToS/philosophy/philosophy.manifest.json";let packet_path="ToS/research-packets/deep-research/philosophy/research.manifest.json";
    for (field,path,needle) in [
        ("research_packet_routes","ToS/research-packets/../canon/rogue/research.manifest.json","research packet routes must be normalized repo-relative paths under ToS/research-packets"),
        ("graph_view_routes","ToS/philosophy/graph-workbench/rogue.graph.md","graph_view_routes entries must stay under ToS/philosophy/graph-workbench/views"),
        ("graph_view_contracts","ToS/philosophy/graph-workbench/rogue-contract.json","graph_view_contracts entries must stay under ToS/philosophy/graph-workbench/views"),
        ("atlas_routes","ToS/philosophy/rogue-atlas.json","atlas_routes entries must stay under ToS/philosophy/atlas")
    ] {let mut changed=manifest.clone();changed[field]=json!([path]);f.write(manifest_path,&changed.to_string());let out=invoke(&f.0,"--philosophy-topology");assert_eq!(out.status.code(),Some(1));assert!(text(&out).contains(needle),"{}",text(&out));}
    for (title,name) in [("Fixture Child","fixture-child"),("Renamed Child","renamed-child")] {
        let mut changed=manifest.clone();let mut p=packet.clone();p["branch_child_pages"]=json!([{"id":"fixture-child-page","original_title":"Original Child","title":title}]);f.write(packet_path,&p.to_string());
        let path=format!("ToS/philosophy/eras/{name}/branch.manifest.json");changed["branch_manifests"].as_array_mut().unwrap().push(json!(path));f.write(manifest_path,&changed.to_string());f.write(&path,&json!({"path":format!("ToS/philosophy/eras/{name}"),"branch_id":format!("philosophy.{name}"),"role":"fixture"}).to_string());
        let out=invoke(&f.0,"--philosophy-topology");assert_eq!(out.status.code(),Some(1));assert!(text(&out).contains(&format!("metadata-only source label used as path component: {name}")),"{}",text(&out));fs::remove_dir_all(f.0.join(format!("ToS/philosophy/eras/{name}"))).unwrap();
    }
    f.write(manifest_path,&manifest.to_string());f.write(packet_path,&packet.to_string());f.write("ToS/rogue/fixture-child/README.md","");let out=invoke(&f.0,"--philosophy-topology");assert!(text(&out).contains("ToS/rogue/fixture-child: metadata-only source label"),"{}",text(&out));fs::remove_dir_all(f.0.join("ToS/rogue")).unwrap();
    let reference="ToS/philosophy/trunk/sources/plantings/stale/source-planting.json";
    f.write("ToS/philosophy/trunk/branch.manifest.json",&json!({"path":"ToS/philosophy/trunk","branch_id":"philosophy.trunk","role":"fixture trunk","source_planting_refs":[reference],"source_planting_count":1}).to_string());
    f.write("ToS/philosophy/trunk/sources/branch.manifest.json",&json!({"planting_refs":[reference],"planting_count":1}).to_string());let out=invoke(&f.0,"--philosophy-topology");let t=text(&out);assert_eq!(out.status.code(),Some(1));
    for needle in ["source_planting_refs differs from exact planting files","source_planting_count differs from exact planting count","planting_refs differs from exact planting files","planting_count differs from exact planting count"] {assert!(t.contains(needle),"{t}");}
}
#[test]
fn source_anchor_preparation_binds_exact_provider_and_antecedent_without_planting() {
    let root=Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap();
    for (atlas,row,label,line,branch_marker) in [("A12","1","Sefaria: Proverbs / Job / Ecclesiastes",9,""),("T2-39","2","SuttaCentral",14,"pali-scholastic-commentarial-handoff")] {
        let out=invoke_args(root,&["--prepare-source-anchor","--atlas-row",atlas,"--source-table-index","14","--source-row-index",row,"--source-label",label]);assert!(out.status.success(),"{}",text(&out));
        let v:Value=serde_json::from_slice(&out.stdout).unwrap();assert_eq!(v["status"],"prepared-not-planted");assert_eq!(v["source_backlog_anchor"]["line"],line);assert!(v.get("source_witness").is_none());assert!(v["branch_path"].as_str().unwrap().contains(branch_marker));
        for (record,digest) in [("branch_manifest","sha256"),("atlas_source","row_sha256")] {
            let raw=fs::read(root.join(v[record]["path"].as_str().unwrap())).unwrap();
            let raw=if record=="atlas_source"{tos_ops_mechanics_plan::route_cards::splitlines(std::str::from_utf8(&raw).unwrap())[v[record]["line"].as_u64().unwrap() as usize-1].as_bytes().to_vec()}else{raw};
            assert_eq!(v[record][digest],tos_ops_mechanics_plan::route_cards::sha256_bytes(&raw));
        }
        let raw=fs::read(root.join(v["source_backlog_anchor"]["path"].as_str().unwrap())).unwrap();assert_eq!(v["backlog_sha256"],tos_ops_mechanics_plan::route_cards::sha256_bytes(&raw));
    }
    let out=invoke_args(root,&["--prepare-source-anchor","--atlas-row","A12","--source-table-index","14","--source-row-index","1","--source-label","MorphHB"]);assert_eq!(out.status.code(),Some(1));assert!(text(&out).contains("exact atlas, branch or source label"));
    let out=invoke_args(root,&["--prepare-source-anchor","--atlas-row","A12","--source-table-index","0","--source-row-index","1","--source-label","MorphHB"]);assert_eq!(out.status.code(),Some(1));assert!(text(&out).contains("must be positive"));
}
