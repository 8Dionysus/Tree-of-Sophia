//! ToS-owned source-export routes exposed to agents, without consumer custody.
use crate::route_cards::RouteSources;
use serde_json::Value;
use std::io;
type Issue = (String, String);
fn push(out: &mut Vec<Issue>, location: &str, message: impl Into<String>) {
    out.push((location.to_owned(), message.into()));
}
// Validate only ToS-owned export routes. Consumer index families and producer
// checkout verification belong to the consumer, never this source gate.
pub fn source_export_issues(sources: &mut RouteSources, port: &Value) -> io::Result<Vec<Issue>> {
    let mut out = Vec::new();
    let family = &port["generated_family"];
    if port["canonical_owner"] != "Tree-of-Sophia" {
        push(
            &mut out,
            "kag_provider",
            "ToS source export must remain owned by Tree-of-Sophia",
        );
    }
    if family["scope"] != "tos_source_export" {
        return Ok(vec![(
            "kag_provider".into(),
            "ToS owns its source export, not a consumer index family".into(),
        )]);
    }
    for field in [
        "source_contract",
        "owner_route",
        "builder",
        "validation_route",
    ] {
        let Some(route) = family[field].as_str().filter(|v| {
            !v.is_empty()
                && !v.starts_with('/')
                && !v.contains('\\')
                && v.split('/')
                    .all(|part| !matches!(part, "" | "." | ".." | ".git"))
        }) else {
            push(
                &mut out,
                "kag_provider",
                format!("source export {field} must be a safe ToS route"),
            );
            continue;
        };
        if !sources.is_file(route)? {
            push(
                &mut out,
                route,
                format!("source export {field} route is missing"),
            );
        }
    }
    if family["source_contract"] != port["manifest"] || family["owner_route"] != port["local_owner"]
    {
        push(
            &mut out,
            "kag_provider",
            "source export routes differ from the declared ToS port",
        );
    }
    sources.check()?;
    Ok(out)
}
