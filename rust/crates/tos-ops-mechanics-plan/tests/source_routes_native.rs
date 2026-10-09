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
fn invoke(root:&Path, flag:&str) -> Output {
    let exe=std::env::var_os("TOS_SOURCE_ROUTES_TEST_EXECUTABLE").unwrap_or_else(||env!("CARGO_BIN_EXE_tos-ops-mechanics-plan").into());
    let out=Command::new("/usr/bin/timeout").args(["--kill-after=2","45"]).arg(exe)
        .arg("--repo-root").arg(root).args(["--python","/no-python-executable",flag])
        .env_clear().env("PATH","/usr/bin").output().unwrap();
    assert!(out.stdout.len()+out.stderr.len()<2*1024*1024);assert_ne!(out.status.code(),Some(124));out
}
fn text(out:&Output)->String {format!("{}{}",String::from_utf8_lossy(&out.stdout),String::from_utf8_lossy(&out.stderr))}
#[test]
fn current_source_routes_run_natively_and_preserve_authority_bounds() {
    let root=Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap();let mut failures=Vec::new();
    for flag in ["--tiny-entry-validate","--lived-witness-validate","--intake-pack-validate","--tree-node-validate"] {
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
