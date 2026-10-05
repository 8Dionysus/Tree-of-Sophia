//! Maintained root-entry readmodel kernel, schema and orientation checks.
use crate::kag_corpus_export::{self, VerifiedExport};
use crate::route_cards::{self, RouteSources};
use serde_json::Value;
use std::{
    io,
    path::Path,
    sync::atomic::{AtomicI32, Ordering},
};
use tos_foundation::{JsonLimits, JsonMode, parse_json};
pub type Issue = (String, String);
pub const OUTPUT: &str = "ToS/derived-exports/root_entry_map.min.json";
const SCHEMA: &str = "ToS/contracts/root-entry-map.schema.json";
pub const SOURCE: &str = "scripts/root_entry_map.source.json";
fn tick(s: &RouteSources, cancel: &AtomicI32) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) != 0 {
        return Err(io::Error::other("root-entry-map operation cancelled"));
    }
    s.check()
}
fn parse(raw: &[u8]) -> io::Result<Value> {
    parse_json(raw, JsonMode::RequestLastWins, JsonLimits::default()).map_err(|e| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            format!("root-entry-map finite JSON profile: {e:?}"),
        )
    })?;
    serde_json::from_slice(raw).map_err(io::Error::other)
}
fn resolve(
    s: &mut RouteSources,
    value: &str,
    export: Option<&VerifiedExport>,
    cancel: &AtomicI32,
) -> io::Result<()> {
    if value == kag_corpus_export::CAPSULE {
        if let Some(export) = export {
            return export.check(s, cancel);
        }
    }
    if !s.exists(value)? {
        return Err(io::Error::other(format!("missing ref target '{value}'")));
    }
    Ok(())
}
fn low_context(
    s: &mut RouteSources,
    value: &str,
    location: &str,
    export: Option<&VerifiedExport>,
    cancel: &AtomicI32,
) -> io::Result<()> {
    let (path, anchor) = value.split_once('#').unwrap_or((value, ""));
    if ["src/", "scripts/"].iter().any(|p| path.starts_with(p)) {
        return Err(io::Error::other(format!(
            "{location} must not point to implementation path '{value}'"
        )));
    }
    resolve(s, path, export, cancel)?;
    if !anchor.is_empty()
        && Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .is_none_or(|e| !e.eq_ignore_ascii_case("md"))
    {
        return Err(io::Error::other(format!(
            "{location} may only use anchors for markdown refs"
        )));
    }
    Ok(())
}
fn schema(s: &mut RouteSources, payload: &Value, cancel: &AtomicI32) -> io::Result<()> {
    tick(s, cancel)?;
    let value = parse(&s.bytes(SCHEMA)?)?;
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(false)
        .offline()
        .build(&value)
        .map_err(|e| io::Error::other(format!("root-entry-map schema compilation: {e}")))?;
    tick(s, cancel)?;
    // Validate the live owner schema, rather than a handwritten subset. No schema
    // or source admission is inferred from structural success.
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    enum Part {
        Index(usize),
        Property(String),
    }
    let mut first: Option<(Vec<Part>, String, String)> = None;
    for error in validator.iter_errors(payload) {
        tick(s, cancel)?;
        let mut path = String::new();
        let parts = error
            .instance_path()
            .segments()
            .map(|segment| match segment {
                jsonschema::paths::LocationSegment::Index(index) => {
                    path.push_str(&format!("[{index}]"));
                    Part::Index(index)
                }
                jsonschema::paths::LocationSegment::Property(name) => {
                    if !path.is_empty() {
                        path.push('.');
                    }
                    path.push_str(&name);
                    Part::Property(name.into_owned())
                }
            })
            .collect::<Vec<_>>();
        let message = error.to_string();
        if path.len() + message.len() > 1_048_576 {
            return Err(io::Error::other(
                "root-entry-map schema diagnostic exceeds bound",
            ));
        }
        if first.as_ref().is_none_or(|(order, _, _)| parts < *order) {
            first = Some((parts, path, message));
        }
    }
    if let Some((_, path, message)) = first {
        return Err(io::Error::other(if path.is_empty() {
            format!("schema violation: {message}")
        } else {
            format!("schema violation at '{path}': {message}")
        }));
    }
    tick(s, cancel)
}
fn build_payload_verified(
    _root: &Path,
    s: &mut RouteSources,
    export: Option<&VerifiedExport>,
    cancel: &AtomicI32,
) -> io::Result<Value> {
    tick(s, cancel)?;
    let payload = parse(&s.bytes(SOURCE)?)?;
    // The authored declaration is untrusted until the current owner schema passes.
    // Validate before route field indexing or unwraps, then resolve held source refs.
    schema(s, &payload, cancel)?;
    for key in [
        "schema_ref",
        "authority_ref",
        "public_root_ref",
        "current_tiny_entry_ref",
        "export_ref",
    ] {
        low_context(
            s,
            payload[key]
                .as_str()
                .ok_or_else(|| io::Error::other("canonical root-entry-map field missing"))?,
            &format!("surface.{key}"),
            export,
            cancel,
        )?;
    }
    for reference in payload["validation_refs"]
        .as_array()
        .ok_or_else(|| io::Error::other("canonical validation_refs missing"))?
    {
        resolve(
            s,
            reference
                .as_str()
                .ok_or_else(|| io::Error::other("canonical ref not string"))?,
            export,
            cancel,
        )?;
    }
    for route in payload["routes"]
        .as_array()
        .ok_or_else(|| io::Error::other("canonical routes missing"))?
    {
        tick(s, cancel)?;
        let id = route["route_id"].as_str().unwrap();
        low_context(
            s,
            route["surface_ref"].as_str().unwrap(),
            &format!("route:{id}.surface_ref"),
            export,
            cancel,
        )?;
        for reference in route["verification_refs"].as_array().unwrap() {
            low_context(
                s,
                reference.as_str().unwrap(),
                &format!("route:{id}.verification_refs"),
                export,
                cancel,
            )?;
        }
    }
    tick(s, cancel)?;
    Ok(payload)
}
pub fn render(payload: &Value) -> io::Result<String> {
    let mut result = serde_json::to_string(payload).map_err(io::Error::other)?;
    if result.len() > 1_048_576 {
        return Err(io::Error::other("root-entry-map output exceeds bound"));
    }
    result.push('\n');
    Ok(result)
}
fn validate_inner(
    root: &Path,
    s: &mut RouteSources,
    export: Option<&VerifiedExport>,
    cancel: &AtomicI32,
) -> io::Result<()> {
    let expected = build_payload_verified(root, s, export, cancel)?;
    let current = parse(&s.bytes(OUTPUT)?)?;
    schema(s, &current, cancel)?;
    if current != expected {
        return Err(io::Error::other(format!(
            "{OUTPUT} does not match the canonical rebuild"
        )));
    }
    // Exact canonical equality makes every field/type/identity check below
    // meaningful without accepting dynamic shell commands or extra routes.
    for key in [
        "schema_ref",
        "authority_ref",
        "public_root_ref",
        "current_tiny_entry_ref",
        "export_ref",
    ] {
        resolve(s, current[key].as_str().unwrap(), export, cancel)?;
    }
    for reference in current["validation_refs"].as_array().unwrap() {
        resolve(s, reference.as_str().unwrap(), export, cancel)?;
    }
    let identity = &current["artifact_identity"];
    for key in ["authority_ref", "contract_version"] {
        low_context(
            s,
            identity[key].as_str().unwrap(),
            &format!("artifact_identity.{key}"),
            export,
            cancel,
        )?;
    }
    for command in identity["verification"].as_array().unwrap() {
        // The exact canonical command strings contain no quotes/backslashes. Unlike
        // shlex over untrusted strings, this never interprets caller-supplied syntax.
        for part in command.as_str().unwrap().split_whitespace().skip(1) {
            if part.ends_with(".py") && !part.starts_with('-') {
                resolve(s, part, export, cancel)?;
            }
        }
    }
    let mut ids = std::collections::BTreeSet::new();
    for route in current["routes"].as_array().unwrap() {
        tick(s, cancel)?;
        let id = route["route_id"].as_str().unwrap();
        if !ids.insert(id) {
            return Err(io::Error::other(format!(
                "{OUTPUT} has duplicate route_id '{id}'"
            )));
        }
        low_context(
            s,
            route["surface_ref"].as_str().unwrap(),
            &format!("route:{id}.surface_ref"),
            export,
            cancel,
        )?;
        for reference in route["verification_refs"].as_array().unwrap() {
            low_context(
                s,
                reference.as_str().unwrap(),
                &format!("route:{id}.verification_refs"),
                export,
                cancel,
            )?;
        }
    }
    if ["current-tiny-entry", "tree-first-model", "bounded-export"]
        .iter()
        .any(|id| !ids.contains(id))
    {
        return Err(io::Error::other(format!(
            "{OUTPUT} is missing core root-entry routes"
        )));
    }
    tick(s, cancel)
}
pub fn validate_verified_export(
    root: &Path,
    s: &mut RouteSources,
    export: Option<&VerifiedExport>,
    cancel: &AtomicI32,
) -> io::Result<Vec<Issue>> {
    if let Some(export) = export {
        export.bind_repo(s, cancel)?;
    }
    match validate_inner(root, s, export, cancel) {
        Ok(()) => {
            if let Some(export) = export {
                export.check(s, cancel)?;
            }
            Ok(Vec::new())
        }
        // Unsupported IO/path/JSON budget and cancellation stay operation failures;
        // ordinary authored/ref/schema/currentness defects remain ordered issues.
        Err(error)
            if cancel.load(Ordering::Relaxed) != 0
                || error.kind() == io::ErrorKind::Unsupported =>
        {
            Err(error)
        }
        Err(error) => {
            s.check()?;
            let message = error.to_string();
            if message.len() > 1_048_576 {
                return Err(io::Error::other("root-entry-map diagnostic exceeds bound"));
            }
            Ok(vec![(OUTPUT.into(), message)])
        }
    }
}
fn build_verified(
    root: &Path,
    s: &mut RouteSources,
    export: Option<&VerifiedExport>,
    cancel: &AtomicI32,
    check: bool,
) -> io::Result<bool> {
    if let Some(export) = export {
        export.bind_repo(s, cancel)?;
    }
    let rendered = render(&build_payload_verified(root, s, export, cancel)?)?;
    tick(s, cancel)?;
    // Refuse an input changed during preparation before touching the canonical
    // owner output. The final check still refuses later concurrent substitution;
    // that error grants no admission even though canonical bytes may be written.
    if let Some(export) = export {
        export.check(s, cancel)?;
    }
    if check {
        // Python read_text universal-newline comparison, exactly like the maintained
        // builder. RouteSources text retains that read profile with bounded custody.
        Ok(s.text(OUTPUT)?.as_deref() == Some(rendered.as_str()))
    } else {
        route_cards::write_output(root, Path::new(OUTPUT), &rendered)?;
        tick(s, cancel)?;
        Ok(true)
    }
}
/// Optional runtime export input resolves only the unchanged canonical export_ref.
/// It does not rewrite the authored URI/schema or copy capsule bytes to checkout.
pub fn validate_with_export(
    root: &Path,
    s: &mut RouteSources,
    path: Option<&Path>,
    cancel: &AtomicI32,
) -> io::Result<Vec<Issue>> {
    let export = path
        .map(|p| kag_corpus_export::verify_with_sources(p, s, cancel))
        .transpose()?;
    validate_verified_export(root, s, export.as_ref(), cancel)
}
pub fn build_with_export(
    root: &Path,
    s: &mut RouteSources,
    path: Option<&Path>,
    cancel: &AtomicI32,
    check: bool,
) -> io::Result<bool> {
    let export = path
        .map(|p| kag_corpus_export::verify_with_sources(p, s, cancel))
        .transpose()?;
    let result = build_verified(root, s, export.as_ref(), cancel, check)?;
    if let Some(export) = &export {
        export.check(s, cancel)?;
    }
    Ok(result)
}
pub fn validate(root: &Path, s: &mut RouteSources, cancel: &AtomicI32) -> io::Result<Vec<Issue>> {
    validate_with_export(root, s, None, cancel)
}
pub fn build(
    root: &Path,
    s: &mut RouteSources,
    cancel: &AtomicI32,
    check: bool,
) -> io::Result<bool> {
    build_with_export(root, s, None, cancel, check)
}
pub fn build_payload(root: &Path, s: &mut RouteSources, cancel: &AtomicI32) -> io::Result<Value> {
    build_payload_verified(root, s, None, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_schema_ref_parity_and_builder_repair() {
        let root = std::env::temp_dir().join(format!(
            "tos-root-entry-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let write = |path: &str, text: &str| {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        let source = include_str!("../../../../scripts/root_entry_map.source.json");
        let p = parse(source.as_bytes()).unwrap();
        for key in [
            "schema_ref",
            "authority_ref",
            "public_root_ref",
            "current_tiny_entry_ref",
            "export_ref",
        ] {
            write(p[key].as_str().unwrap(), "owned public fixture");
        }
        for reference in p["validation_refs"].as_array().unwrap() {
            write(reference.as_str().unwrap(), "fixture");
        }
        for route in p["routes"].as_array().unwrap() {
            write(route["surface_ref"].as_str().unwrap(), "fixture");
            for reference in route["verification_refs"].as_array().unwrap() {
                write(reference.as_str().unwrap(), "fixture");
            }
        }
        write(
            p["artifact_identity"]["authority_ref"].as_str().unwrap(),
            "owner",
        );
        write(
            SCHEMA,
            include_str!("../../../../ToS/contracts/root-entry-map.schema.json"),
        );
        write(SOURCE, source);
        let cancel = AtomicI32::new(0);
        assert!(
            build(
                &root,
                &mut RouteSources::new(&root).unwrap(),
                &cancel,
                false
            )
            .unwrap()
        );
        assert!(build(&root, &mut RouteSources::new(&root).unwrap(), &cancel, true).unwrap());
        assert!(
            validate(&root, &mut RouteSources::new(&root).unwrap(), &cancel)
                .unwrap()
                .is_empty()
        );
        // The declaration, rather than a compiled duplicate, owns route wording.
        let mut authored = p.clone();
        authored["routes"][0]["need"] = Value::String("edited authored entry need".into());
        write(SOURCE, &render(&authored).unwrap());
        assert_eq!(
            build_payload(&root, &mut RouteSources::new(&root).unwrap(), &cancel).unwrap(),
            authored
        );
        authored["routes"][0]
            .as_object_mut()
            .unwrap()
            .remove("surface_ref");
        write(SOURCE, &render(&authored).unwrap());
        assert!(
            build_payload(&root, &mut RouteSources::new(&root).unwrap(), &cancel)
                .unwrap_err()
                .to_string()
                .contains("schema violation")
        );
        write(SOURCE, source);
        let mut changed = p.clone();
        changed["routes"][0]["route_id"] = Value::String("drift".into());
        write(OUTPUT, &render(&changed).unwrap());
        assert!(!build(&root, &mut RouteSources::new(&root).unwrap(), &cancel, true).unwrap());
        assert!(
            validate(&root, &mut RouteSources::new(&root).unwrap(), &cancel).unwrap()[0]
                .1
                .contains("canonical rebuild")
        );
        changed["extra"] = Value::Bool(true);
        write(OUTPUT, &render(&changed).unwrap());
        assert!(
            validate(&root, &mut RouteSources::new(&root).unwrap(), &cancel).unwrap()[0]
                .1
                .contains("schema violation")
        );
        std::fs::remove_file(root.join("BOUNDARIES.md")).unwrap();
        assert!(
            build_payload(&root, &mut RouteSources::new(&root).unwrap(), &cancel)
                .unwrap_err()
                .to_string()
                .contains("missing ref target")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn canonical_compact_render_and_route_ids() {
        let source = include_str!("../../../../scripts/root_entry_map.source.json");
        let p = parse(source.as_bytes()).unwrap();
        assert_eq!(render(&p).unwrap(), source);
        assert_eq!(p["routes"].as_array().unwrap().len(), 3);
        assert_eq!(p["routes"][0]["route_id"], "current-tiny-entry");
        assert_eq!(
            p["artifact_identity"]["producer"],
            "scripts/build_root_entry_map.py from scripts/root_entry_map_common.py"
        );
    }
}
