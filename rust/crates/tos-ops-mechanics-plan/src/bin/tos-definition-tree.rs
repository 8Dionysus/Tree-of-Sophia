#[path = "../definition_tree.rs"]
mod definition_tree;
use std::path::Path;
fn run() -> definition_tree::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["split", entity, relation, output] => definition_tree::split(Path::new(entity), Path::new(relation), Path::new(output)),
        ["check-migration", tree] => definition_tree::assemble(Path::new(tree))?.check_migration(),
        ["check-compatibility", tree, output] => definition_tree::assemble(Path::new(tree))?.check_compatibility(Path::new(output)),
        ["check", tree] => { let a = definition_tree::assemble(Path::new(tree))?; println!("{}", a.evidence()?); Ok(()) },
        ["assemble", tree, output] => definition_tree::assemble(Path::new(tree))?.write_candidate(Path::new(output), false),
        ["check-generated", tree, output] => definition_tree::assemble(Path::new(tree))?.check_generated(Path::new(output)),
        _ => Err(definition_tree::TreeError { code: "Usage", detail: "tos-definition-tree split ENTITY RELATION NEW_TREE | check TREE | check-migration TREE | check-compatibility TREE REGISTRY_DIR | assemble TREE NEW_CANDIDATE_DIR | check-generated TREE REGISTRY_DIR".into() }),
    }
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
