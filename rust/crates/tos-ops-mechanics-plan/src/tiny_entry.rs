//! Maintained tiny-entry mechanics; no textual, semantic or canon admission.
use crate::route_cards::{RouteSources, python_space};
use serde_json::Value;
use std::{
    io,
    path::Path,
    sync::atomic::{AtomicI32, Ordering},
};
use tos_foundation::{FoundationErrorCode, JsonLimits, JsonMode, parse_json};
pub type Issue = (String, String);
const ROUTE_PATH: &str = "ToS/public-compatibility/tos_tiny_entry_route.example.json";
const README_PATH: &str = "README.md";
const CHARTER_PATH: &str = "CHARTER.md";
const ROUTE_DOC_PATH: &str = "ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md";
const KAG_EXPORT_DOC_PATH: &str =
    "mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md";
const CAPSULE_PATH: &str = "ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md";
const KNOWLEDGE_MODEL_PATH: &str = "ToS/doctrine/KNOWLEDGE_MODEL.md";
const REVIEW_CHECKLIST_PATH: &str =
    "mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md";
const SOURCE_NODE_PATH: &str = "ToS/public-compatibility/source_node.example.json";
const CONCEPT_NODE_PATH: &str = "ToS/public-compatibility/concept_node.example.json";
const EXPECTED_ROUTE_ID: &str = "tos-tiny-entry.zarathustra-prologue";
const EXPECTED_NODE_KIND: &str = "source_node";
const EXPECTED_ROOT_SURFACE: &str = "README.md";
const EXPECTED_CAPSULE_SURFACE: &str = "ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md";
const EXPECTED_AUTHORITY_SURFACE: &str = "ToS/public-compatibility/source_node.example.json";
const EXPECTED_BOUNDED_HOP: &str = "ToS/public-compatibility/concept_node.example.json";
const EXPECTED_FALLBACK: &str = "ToS/doctrine/KNOWLEDGE_MODEL.md";
const LEGACY_HOP_FIELD: &str = "lineage_or_context_hop";
const README_ROUTE_REFS: [&str; 5] = [
    "ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md",
    "ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md",
    "ToS/derived-exports/README.md",
    "mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md",
    "VALIDATION.md",
];
const README_BANNED_COMMANDS: [&str; 2] = ["python scripts/", "python -m unittest"];
const ROUTE_DOC_REQUIRED_TOKENS: [&str; 9] = [
    "## Source-first re-entry",
    "README.md",
    "ToS/public-compatibility/tos_tiny_entry_route.example.json",
    "ToS/public-compatibility/source_node.example.json",
    "aoa-sdk",
    "routing control plane",
    "aoa-routing",
    "compatibility namespace",
    "scripts/validate_tiny_entry_route.py",
];
const REVIEW_CHECKLIST_REQUIRED_TOKENS: [&str; 1] = ["scripts/validate_tiny_entry_route.py"];
const KAG_EXPORT_TOOLING_REQUIRED_TOKENS: [&str; 3] = [
    "scripts/build_kag_export.py",
    "python scripts/build_kag_export.py verify",
    "scripts/publish_kag_release.py",
];
const BOUNDARY_REQUIRED_TOKENS: [&str; 5] = [
    "ToS-authored authority",
    "aoa-kag",
    "aoa-sdk routing control plane",
    "aoa-routing compatibility namespace",
    "downstream derived system",
];
const REQUIRED_FILES: [&str; 9] = [
    "README.md",
    "CHARTER.md",
    "ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md",
    "ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md",
    "ToS/doctrine/KNOWLEDGE_MODEL.md",
    "mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md",
    "ToS/public-compatibility/source_node.example.json",
    "ToS/public-compatibility/concept_node.example.json",
    "ToS/public-compatibility/tos_tiny_entry_route.example.json",
];
struct Issues {
    rows: Vec<Issue>,
    bytes: usize,
}
impl Issues {
    fn push(&mut self, path: &str, message: impl Into<String>) -> io::Result<()> {
        let message = message.into();
        self.bytes = self
            .bytes
            .checked_add(path.len() + message.len() + 5)
            .ok_or_else(|| io::Error::other("tiny-entry issue accounting overflow"))?;
        if self.rows.len() >= 4096 || self.bytes > 1_048_576 {
            return Err(io::Error::other("tiny-entry issue bound exceeded"));
        }
        self.rows.push((path.into(), message));
        Ok(())
    }
}
fn tick(s: &RouteSources, cancel: &AtomicI32) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) != 0 {
        return Err(io::Error::other("tiny-entry validation cancelled"));
    }
    s.check()
}
fn normalize(text: &str) -> io::Result<String> {
    let mut result = String::new();
    let mut pending = false;
    for c in text.chars().flat_map(char::to_lowercase) {
        if python_space(c) {
            pending = !result.is_empty();
            continue;
        }
        if result.len() + c.len_utf8() + usize::from(pending) > 24 * 1_048_576 {
            return Err(io::Error::other("tiny-entry normalized text exceeds bound"));
        }
        if pending {
            result.push(' ');
            pending = false;
        }
        result.push(c);
    }
    Ok(result)
}
fn json(s: &mut RouteSources, path: &str, issues: &mut Issues) -> io::Result<Option<Value>> {
    let Some(text) = s.text(path)? else {
        issues.push(path, "missing required file")?;
        return Ok(None);
    };
    match parse_json(
        text.as_bytes(),
        JsonMode::RequestLastWins,
        JsonLimits::default(),
    ) {
        Ok(v) => drop(v),
        Err(e)
            if matches!(
                e.code,
                FoundationErrorCode::BudgetExceeded
                    | FoundationErrorCode::NonfiniteFloat
                    | FoundationErrorCode::InvalidUnicodeScalar
            ) =>
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("tiny-entry finite JSON profile: {e:?}"),
            ));
        }
        Err(e) => {
            issues.push(path, format!("invalid JSON: {e:?}"))?;
            return Ok(None);
        }
    }
    serde_json::from_str(&text)
        .map(Some)
        .map_err(io::Error::other)
}
fn surface(
    s: &mut RouteSources,
    value: &Value,
    location: &str,
    issues: &mut Issues,
) -> io::Result<Option<String>> {
    let Some(value) = value.as_str().filter(|v| !v.is_empty()) else {
        issues.push(
            ROUTE_PATH,
            format!("{location} must be a non-empty repo-relative path"),
        )?;
        return Ok(None);
    };
    if value.contains(':')
        || ["aoa-sdk/", "aoa-routing/", "aoa-kag/"]
            .iter()
            .any(|p| value.starts_with(p))
    {
        issues.push(
            ROUTE_PATH,
            format!(
                "{location} must stay inside Tree-of-Sophia and must not point at downstream repos"
            ),
        )?;
        return Ok(None);
    }
    if Path::new(value)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        issues.push(
            ROUTE_PATH,
            format!("{location} must not escape the repository root"),
        )?;
        return Ok(None);
    }
    // The shared reader's explicitly finite repo-relative/nofollow profile rejects
    // absolute/symlink inputs instead of silently widening the Python path scope.
    if !s.is_file(value)? {
        issues.push(
            ROUTE_PATH,
            format!("{location} target '{value}' is missing"),
        )?;
        return Ok(None);
    }
    Ok(Some(value.into()))
}
fn fixed_surface(
    s: &mut RouteSources,
    payload: &Value,
    key: &str,
    expected: &str,
    issues: &mut Issues,
) -> io::Result<Option<String>> {
    let result = surface(s, &payload[key], key, issues)?;
    if result.as_deref().is_some_and(|v| v != expected) {
        issues.push(ROUTE_PATH, format!("{key} must stay '{expected}'"))?;
    }
    Ok(result)
}
fn tokens(
    s: &mut RouteSources,
    path: &str,
    required: &[&str],
    banned: bool,
    issues: &mut Issues,
    cancel: &AtomicI32,
) -> io::Result<()> {
    tick(s, cancel)?;
    let Some(text) = s.text(path)? else {
        return Ok(());
    };
    let text = normalize(&text)?;
    for token in required {
        tick(s, cancel)?;
        if text.contains(&normalize(token)?) == banned {
            issues.push(
                path,
                if banned {
                    format!("route surface should not carry command text: {token}")
                } else {
                    format!("missing stable route token: {token}")
                },
            )?;
        }
    }
    Ok(())
}
pub fn validate(_root: &Path, s: &mut RouteSources, cancel: &AtomicI32) -> io::Result<Vec<Issue>> {
    let mut issues = Issues {
        rows: Vec::new(),
        bytes: 0,
    };
    tick(s, cancel)?;
    for path in REQUIRED_FILES {
        tick(s, cancel)?;
        if !s.is_file(path)? {
            issues.push(path, "missing required file")?;
        }
    }
    let payload = json(s, ROUTE_PATH, &mut issues)?;
    let source = json(s, SOURCE_NODE_PATH, &mut issues)?;
    if let Some(p) = payload.filter(Value::is_object) {
        if p["route_id"] != EXPECTED_ROUTE_ID {
            issues.push(
                ROUTE_PATH,
                format!("route_id must stay '{EXPECTED_ROUTE_ID}' in the current bounded route"),
            )?;
        }
        fixed_surface(s, &p, "root_surface", EXPECTED_ROOT_SURFACE, &mut issues)?;
        if p["node_kind"] != EXPECTED_NODE_KIND {
            issues.push(
                ROUTE_PATH,
                format!("node_kind must stay '{EXPECTED_NODE_KIND}'"),
            )?;
        }
        if let Some(id) = p["node_id"].as_str().filter(|v| !v.is_empty()) {
            if source
                .as_ref()
                .is_some_and(|v| v.is_object() && v["node_id"] != id)
            {
                issues.push(ROUTE_PATH,"node_id must stay aligned with ToS/public-compatibility/source_node.example.json")?;
            }
        } else {
            issues.push(ROUTE_PATH, "node_id must stay a non-empty string")?;
        }
        let capsule = fixed_surface(
            s,
            &p,
            "capsule_surface",
            EXPECTED_CAPSULE_SURFACE,
            &mut issues,
        )?;
        let authority = fixed_surface(
            s,
            &p,
            "authority_surface",
            EXPECTED_AUTHORITY_SURFACE,
            &mut issues,
        )?;
        let hop = fixed_surface(s, &p, "bounded_hop", EXPECTED_BOUNDED_HOP, &mut issues)?;
        if !p[LEGACY_HOP_FIELD].is_null() {
            let legacy = surface(s, &p[LEGACY_HOP_FIELD], LEGACY_HOP_FIELD, &mut issues)?;
            if let (Some(a), Some(b)) = (&hop, &legacy) {
                if a != b {
                    issues.push(ROUTE_PATH,format!("{LEGACY_HOP_FIELD} must match bounded_hop during the compatibility transition"))?;
                }
            }
        }
        fixed_surface(s, &p, "fallback", EXPECTED_FALLBACK, &mut issues)?;
        if let (Some(a), Some(b)) = (&capsule, &authority) {
            if a == b {
                issues.push(
                    ROUTE_PATH,
                    "capsule_surface and authority_surface must remain distinct",
                )?;
            }
        }
        if let Some(boundary) = p["non_identity_boundary"]
            .as_str()
            .filter(|v| !v.is_empty())
        {
            let text = normalize(boundary)?;
            for token in BOUNDARY_REQUIRED_TOKENS {
                if !text.contains(&normalize(token)?) {
                    issues.push(
                        ROUTE_PATH,
                        format!("non_identity_boundary must contain '{token}'"),
                    )?;
                }
            }
        } else {
            issues.push(
                ROUTE_PATH,
                "non_identity_boundary must stay a non-empty string",
            )?;
        }
    }
    tokens(
        s,
        README_PATH,
        &README_ROUTE_REFS,
        false,
        &mut issues,
        cancel,
    )?;
    tokens(
        s,
        README_PATH,
        &README_BANNED_COMMANDS,
        true,
        &mut issues,
        cancel,
    )?;
    tokens(
        s,
        ROUTE_DOC_PATH,
        &ROUTE_DOC_REQUIRED_TOKENS,
        false,
        &mut issues,
        cancel,
    )?;
    tokens(
        s,
        REVIEW_CHECKLIST_PATH,
        &REVIEW_CHECKLIST_REQUIRED_TOKENS,
        false,
        &mut issues,
        cancel,
    )?;
    tokens(
        s,
        KAG_EXPORT_DOC_PATH,
        &KAG_EXPORT_TOOLING_REQUIRED_TOKENS,
        false,
        &mut issues,
        cancel,
    )?;
    tick(s, cancel)?;
    Ok(issues.rows)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_route_fields_legacy_hop_and_token_guards() {
        let root = std::env::temp_dir().join(format!(
            "tos-tiny-entry-{}-{}",
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
        for path in REQUIRED_FILES {
            write(path, "owned fixture route");
        }
        write(README_PATH, &README_ROUTE_REFS.join("\n"));
        write(ROUTE_DOC_PATH, &ROUTE_DOC_REQUIRED_TOKENS.join("\n"));
        write(
            REVIEW_CHECKLIST_PATH,
            &REVIEW_CHECKLIST_REQUIRED_TOKENS.join("\n"),
        );
        write(
            KAG_EXPORT_DOC_PATH,
            &KAG_EXPORT_TOOLING_REQUIRED_TOKENS.join("\n"),
        );
        write(SOURCE_NODE_PATH, r#"{"node_id":"fixture-node"}"#);
        let mut route = serde_json::json!({"route_id":EXPECTED_ROUTE_ID,"root_surface":EXPECTED_ROOT_SURFACE,"node_kind":EXPECTED_NODE_KIND,"node_id":"fixture-node","capsule_surface":EXPECTED_CAPSULE_SURFACE,"authority_surface":EXPECTED_AUTHORITY_SURFACE,"bounded_hop":EXPECTED_BOUNDED_HOP,"fallback":EXPECTED_FALLBACK,"non_identity_boundary":BOUNDARY_REQUIRED_TOKENS.join(" ")});
        write(ROUTE_PATH, &route.to_string());
        let cancel = AtomicI32::new(0);
        assert!(
            validate(&root, &mut RouteSources::new(&root).unwrap(), &cancel)
                .unwrap()
                .is_empty()
        );
        route["root_surface"] = Value::String(ROUTE_DOC_PATH.into());
        route[LEGACY_HOP_FIELD] = Value::String(ROUTE_DOC_PATH.into());
        route["non_identity_boundary"] = Value::String("aoa-kag".into());
        write(ROUTE_PATH, &route.to_string());
        write(
            README_PATH,
            &format!(
                "{}\npython scripts/validate_tiny_entry_route.py",
                README_ROUTE_REFS.join("\n")
            ),
        );
        let rows = validate(&root, &mut RouteSources::new(&root).unwrap(), &cancel).unwrap();
        assert_eq!(rows[0].1, "root_surface must stay 'README.md'");
        assert!(
            rows.iter()
                .any(|(_, m)| m.contains("must match bounded_hop"))
        );
        assert!(
            rows.iter()
                .any(|(_, m)| m.contains("must contain 'ToS-authored authority'"))
        );
        assert!(
            rows.iter()
                .any(|(_, m)| m.contains("should not carry command text"))
        );
        cancel.store(1, Ordering::Relaxed);
        assert!(validate(&root, &mut RouteSources::new(&root).unwrap(), &cancel).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn python_whitespace_and_stable_route_normalization() {
        assert_eq!(
            normalize(" \u{1c}ToS-AUTHORED\u{a0} authority \r\n").unwrap(),
            "tos-authored authority"
        );
    }
}
