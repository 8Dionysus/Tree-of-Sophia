//! Shared original navigation decoding, rights and reference rules for the
//! PostgreSQL projection bootstrap. Source admission and disclosure stay with
//! the actual source owner and current read lease.

use crate::durable_adapter::durable_projection::{
    DurableNavigationBootstrapRowKindV1, DurableNavigationBootstrapRowV1,
    DurableNavigationEventLimits,
};
use crate::{DurableError, DurableResult};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{JsonLimits, JsonMode, RelativePath, parse_json};

const MAX_BOOTSTRAP_PATH_BYTES: usize = 4096;
const MAX_BOOTSTRAP_ID_BYTES: usize = 256;
// A relation identifier can compose two record identities and a predicate.
// Its byte contract is separate from the identities of its owner/endpoints.
pub(crate) const MAX_NAVIGATION_EDGE_ID_BYTES: usize = 512;

fn parse_carrier(
    raw: &[u8],
    limits: DurableNavigationEventLimits,
) -> DurableResult<tos_foundation::JsonValue> {
    parse_json(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: limits.max_row_bytes,
            max_depth: 64,
            max_visits: 300_000,
            max_integer_digits: 4_300,
        },
    )
    .map(|document| document.into_root())
    .map_err(|_| DurableError::Corrupt("selected navigation carrier JSON"))
}

/// Decode one exact carrier using indexed metadata lookups. The in-memory
/// capture and persisted bootstrap share these owner and visibility rules.
pub(crate) fn decode_original_carrier(
    kind: DurableNavigationBootstrapRowKindV1,
    id: String,
    raw: Vec<u8>,
    limits: DurableNavigationEventLimits,
    mut node_visible: impl FnMut(&str) -> DurableResult<Option<bool>>,
    mut current_owner: impl FnMut(&str) -> DurableResult<Option<String>>,
) -> DurableResult<(DurableNavigationBootstrapRowV1, BTreeSet<String>)> {
    let max_id_bytes = if kind == DurableNavigationBootstrapRowKindV1::Edge {
        MAX_NAVIGATION_EDGE_ID_BYTES
    } else {
        MAX_BOOTSTRAP_ID_BYTES
    };
    if id.is_empty() || id.len() > max_id_bytes {
        return Err(DurableError::Refused(
            "navigation bootstrap carrier ID size",
        ));
    }
    let value = parse_carrier(&raw, limits)?;
    let visible = match kind {
        DurableNavigationBootstrapRowKindV1::Node => Some(tos_compiler::visible_node(&value)),
        DurableNavigationBootstrapRowKindV1::Edge => {
            let mut endpoint = |field| {
                let id = value.object_get(field).and_then(|v| v.as_str()).ok_or(
                    DurableError::Corrupt("navigation original edge endpoint absent"),
                )?;
                node_visible(id)?.ok_or(DurableError::Corrupt(
                    "navigation original edge endpoint absent",
                ))
            };
            let from_visible = endpoint("from_id")?;
            let to_visible = endpoint("to_id")?;
            Some(from_visible && to_visible)
        }
        _ => None,
    };
    let id_field = match kind {
        DurableNavigationBootstrapRowKindV1::Node => "node_id",
        DurableNavigationBootstrapRowKindV1::Edge => "edge_id",
        DurableNavigationBootstrapRowKindV1::Right => "rights_id",
        DurableNavigationBootstrapRowKindV1::Diagnostic => {
            return Err(DurableError::Invalid("unexpected diagnostic carrier table"));
        }
    };
    if value.object_get(id_field).and_then(|value| value.as_str()) != Some(id.as_str()) {
        return Err(DurableError::Corrupt("selected navigation carrier key"));
    }
    let mut refs = BTreeSet::new();
    let mut explicit_owner = None;
    if let Some(path) = value
        .object_get("source_ref")
        .and_then(|value| value.as_str())
    {
        let path = checked_path(path)?.into_string();
        explicit_owner = Some(path.clone());
        refs.insert(path);
    }
    if let Some(path) = value.object_get("path").and_then(|value| value.as_str()) {
        let path = checked_path(path)?.into_string();
        explicit_owner.get_or_insert_with(|| path.clone());
        refs.insert(path);
    }
    let source_refs = value
        .object_get("source_refs")
        .map(|value| {
            value
                .as_array()
                .ok_or(DurableError::Corrupt("selected navigation source refs"))
        })
        .transpose()?
        .unwrap_or(&[]);
    let bindings = value
        .object_get("source_ref_bindings")
        .map(|value| {
            value
                .as_object()
                .ok_or(DurableError::Corrupt("selected navigation source bindings"))
        })
        .transpose()?
        .unwrap_or(&[]);
    let bindings = checked_source_reference_bindings(
        bindings
            .iter()
            .map(|(key, value)| {
                Ok((
                    key.as_str().ok_or(DurableError::Corrupt(
                        "selected navigation source binding key",
                    ))?,
                    value.as_str(),
                ))
            })
            .collect::<DurableResult<Vec<_>>>()?,
        |reference| source_refs.iter().any(|v| v.as_str() == Some(reference)),
    )?;
    {
        let paths = source_refs;
        for path in paths {
            let path = path
                .as_str()
                .ok_or(DurableError::Corrupt("selected navigation source ref"))?;
            if has_uri_scheme(path) {
                continue;
            }
            let source_path = bindings.get(path).map(String::as_str).unwrap_or(path);
            if let Ok(path) = checked_path(source_path) {
                refs.insert(path.into_string());
            } else {
                return Err(DurableError::Corrupt("selected navigation source ref"));
            }
        }
    }
    if refs.is_empty() {
        return Err(DurableError::Conflict(
            "selected navigation row has no source owner",
        ));
    }
    let historical_record_id = value
        .object_get("properties")
        .and_then(|properties| properties.object_get("record_version_view"))
        .and_then(|view| view.object_get("record_ref"))
        .and_then(|reference| reference.object_get("id"))
        .and_then(|id| id.as_str());
    let historical_edge_owner = (value.object_get("edge_kind").and_then(|v| v.as_str())
        == Some("exact_historical_record_reference"))
    .then(|| value.object_get("from_id").and_then(|v| v.as_str()))
    .flatten();
    let owner_record_id = value
        .object_get("record_id")
        .and_then(|value| value.as_str())
        .or_else(|| {
            value
                .object_get("record")
                .and_then(|record| record.object_get("record_id"))
                .and_then(|value| value.as_str())
        })
        .or_else(|| {
            value
                .object_get("source_record")
                .and_then(|record| record.object_get("record_id"))
                .and_then(|value| value.as_str())
        })
        .or(historical_record_id)
        .or(historical_edge_owner)
        .filter(|record_id| !record_id.is_empty() && record_id.len() <= MAX_BOOTSTRAP_ID_BYTES)
        .unwrap_or(&id)
        .to_owned();
    // `source_refs` are dependency evidence and may include citations;
    // they are not row ownership. A carrier without an explicit producer
    // path is assigned only when it has exactly one internal dependency.
    // A historical reference belongs to its current record node; the archive
    // is an additional dependency, not an alternative owner of that edge.
    if explicit_owner.is_none() {
        if let Some(record_id) = historical_edge_owner {
            let owner_path = current_owner(record_id)?.ok_or(DurableError::Corrupt(
                "navigation history source owner absent",
            ))?;
            if !refs.contains(&owner_path) {
                return Err(DurableError::Corrupt(
                    "navigation history source owner differs",
                ));
            }
            explicit_owner = Some(owner_path);
        }
    }
    let owner_path = match explicit_owner {
        Some(owner_path) => owner_path,
        None if refs.len() == 1 => refs.iter().next().unwrap().clone(),
        None => {
            return Err(DurableError::Conflict(
                "selected navigation row ownership is ambiguous",
            ));
        }
    };
    if refs.len() > limits.max_dependencies {
        return Err(DurableError::Refused(
            "navigation bootstrap carrier dependencies",
        ));
    }
    Ok((
        DurableNavigationBootstrapRowV1 {
            kind,
            id: id.clone(),
            owner_source_path: owner_path,
            owner_record_id: owner_record_id.clone(),
            visible,
            raw,
        },
        refs,
    ))
}

pub(crate) fn bind_historical_record_owner(
    row: &mut DurableNavigationBootstrapRowV1,
    limits: DurableNavigationEventLimits,
    mut current_owner: impl FnMut(&str) -> DurableResult<Option<String>>,
) -> DurableResult<()> {
    if row.kind != DurableNavigationBootstrapRowKindV1::Node {
        return Ok(());
    }
    let value = parse_carrier(&row.raw, limits)?;
    let Some(view) = value
        .object_get("properties")
        .and_then(|properties| properties.object_get("record_version_view"))
    else {
        return Ok(());
    };
    let id = view
        .object_get("record_ref")
        .and_then(|reference| reference.object_get("id"))
        .and_then(|id| id.as_str())
        .ok_or(DurableError::Corrupt(
            "navigation history record identity absent",
        ))?;
    let owner_path = current_owner(id)?
        .filter(|_| id == row.owner_record_id)
        .ok_or(DurableError::Corrupt(
            "navigation historical record owner absent",
        ))?;
    let declared_source = view
        .object_get("provenance")
        .and_then(|provenance| provenance.object_get("source"))
        .and_then(|source| source.object_get("source_ref"))
        .and_then(|path| path.as_str());
    if let Some(path) = declared_source {
        if checked_path(path)?.as_str() != owner_path.as_str() {
            return Err(DurableError::Corrupt(
                "navigation historical record source differs",
            ));
        }
    } else if row.owner_source_path != owner_path {
        return Err(DurableError::Corrupt(
            "navigation historical archive has no record source",
        ));
    }
    row.owner_source_path = owner_path;
    Ok(())
}

/// The exact Claim ownership rule, independent of the storage adapter. Targets
/// may be shared; an alternative/superseded Claim does not inherit the owner.
pub(crate) fn claim_contribution(
    row: &DurableNavigationBootstrapRowV1,
    limits: DurableNavigationEventLimits,
) -> DurableResult<Option<((String, String), BTreeSet<String>)>> {
    if row.kind != DurableNavigationBootstrapRowKindV1::Edge {
        return Ok(None);
    }
    let value = parse_carrier(&row.raw, limits)?;
    let field = |key| value.object_get(key).and_then(|v| v.as_str());
    let (Some(claim), Some(path)) = (field("claim_ref"), field("source_claim_file_ref")) else {
        return Ok(None);
    };
    if claim.is_empty() || claim.len() > MAX_BOOTSTRAP_ID_BYTES {
        return Err(DurableError::Corrupt("navigation Claim owner identity"));
    }
    let owner = (checked_path(path)?.into_string(), claim.to_owned());
    let root = format!("claim:{claim}");
    let subject_navigation = field("edge_kind") == Some("claim_subject_navigation")
        && field("predicate_id") == Some("has_source_claim")
        && field("to_id") == Some(root.as_str());
    if !subject_navigation && field("from_id") != Some(root.as_str()) {
        return Err(DurableError::Corrupt("navigation Claim edge owner differs"));
    }
    let target = field("to_id").ok_or(DurableError::Corrupt("navigation Claim target"))?;
    let mut targets = BTreeSet::from([root]);
    if !subject_navigation && !target.starts_with("claim:") {
        targets.insert(target.to_owned());
    }
    Ok(Some((owner, targets)))
}

pub(crate) fn maker_identity(
    row: &DurableNavigationBootstrapRowV1,
    limits: DurableNavigationEventLimits,
) -> DurableResult<Option<String>> {
    if row.kind != DurableNavigationBootstrapRowKindV1::Node {
        return Ok(None);
    }
    let value = parse_carrier(&row.raw, limits)?;
    Ok(
        (value.object_get("node_kind").and_then(|v| v.as_str()) == Some("maker"))
            .then(|| {
                value
                    .object_get("properties")
                    .and_then(|p| p.object_get("identity_node_id"))
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            })
            .flatten(),
    )
}

fn checked_path(value: &str) -> DurableResult<RelativePath> {
    if value.len() > MAX_BOOTSTRAP_PATH_BYTES {
        return Err(DurableError::Refused(
            "navigation bootstrap source path size",
        ));
    }
    RelativePath::parse(value.split_once('#').map_or(value, |(path, _)| path))
        .map_err(|_| DurableError::Invalid("navigation bootstrap source path"))
}

/// A stable evidence reference stays in the carrier's source_refs. The source
/// producer may additionally bind it to the exact catalog member that owns the
/// referenced record. Both bootstrap capture and sealing use this contract;
/// a binding cannot add an unrelated reference, redirect a URI, or hide a path.
pub(crate) fn checked_source_reference_bindings<'a>(
    bindings: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
    has_reference: impl Fn(&str) -> bool,
) -> DurableResult<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for (reference, source) in bindings {
        let source = source.ok_or(DurableError::Invalid("navigation source binding path"))?;
        let path = checked_path(source)?;
        if reference.is_empty()
            || reference.len() > MAX_BOOTSTRAP_PATH_BYTES
            || reference.contains('/')
            || reference.contains('#')
            || reference.contains('\0')
            || has_uri_scheme(reference)
            || !has_reference(reference)
            || path.as_str() != source
            || result
                .insert(reference.to_owned(), path.into_string())
                .is_some()
        {
            return Err(DurableError::Invalid("navigation source reference binding"));
        }
    }
    Ok(result)
}

fn has_uri_scheme(value: &str) -> bool {
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphabetic() || (index > 0 && b"+-.".contains(&byte))
        })
}

#[cfg(test)]
pub(crate) fn normalize_original_fixture(
    mut input: Vec<(DurableNavigationBootstrapRowKindV1, String, Vec<u8>)>,
) -> DurableResult<Vec<DurableNavigationBootstrapRowV1>> {
    input.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    input.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1 && a.2 == b.2);
    let limits = DurableNavigationEventLimits::default();
    let mut rows = Vec::new();
    let mut paths = BTreeSet::new();
    let mut bytes = 0;
    let mut visibility = BTreeMap::new();
    for (kind, id, raw) in input {
        capture_original_carrier(
            kind,
            id,
            raw,
            limits,
            &mut rows,
            &mut paths,
            &mut bytes,
            &mut visibility,
        )?;
    }
    bind_historical_record_owners(&mut rows, limits)?;
    expand_claim_contributions(&mut rows, limits, &mut bytes)?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn navigation_edge_identity_has_its_own_bound_and_indexed_owner_lookup() {
        let limits = DurableNavigationEventLimits::default();
        let path = "ToS/current.json";
        let archive = "ToS/archive.json";
        let id = "e".repeat(MAX_NAVIGATION_EDGE_ID_BYTES);
        let raw = serde_json::to_vec(&json!({
            "edge_id":id, "edge_kind":"exact_historical_record_reference",
            "from_id":"record", "to_id":"history", "source_refs":[path,archive]
        }))
        .unwrap();
        let (row, refs) = decode_original_carrier(
            DurableNavigationBootstrapRowKindV1::Edge,
            id.clone(),
            raw.clone(),
            limits,
            |key| Ok(matches!(key, "record" | "history").then_some(true)),
            |key| Ok((key == "record").then(|| path.to_owned())),
        )
        .unwrap();
        assert_eq!(row.id, id);
        assert_eq!(row.raw, raw);
        assert_eq!(row.owner_record_id, "record");
        assert_eq!(row.owner_source_path, path);
        assert_eq!(refs, BTreeSet::from([path.to_owned(), archive.to_owned()]));
        assert!(
            decode_original_carrier(
                DurableNavigationBootstrapRowKindV1::Edge,
                "e".repeat(MAX_NAVIGATION_EDGE_ID_BYTES + 1),
                raw,
                limits,
                |_| Ok(Some(true)),
                |_| Ok(Some(path.into())),
            )
            .is_err()
        );
        assert!(
            decode_original_carrier(
                DurableNavigationBootstrapRowKindV1::Node,
                "n".repeat(257),
                b"{}".to_vec(),
                limits,
                |_| Ok(None),
                |_| Ok(None),
            )
            .is_err()
        );
    }

    #[test]
    fn navigation_reference_binding_capture_preserves_owner_and_dependencies() {
        let limits = DurableNavigationEventLimits::default();
        let owner = "ToS/source-witnesses/example/responsibility-claims.jsonl";
        let anchor_source = "ToS/source-witnesses/example/anchors.jsonl";
        let anchor = "tos.anchor.example.title-page";
        let row = json!({
            "edge_id":"edge:claim", "from_id":"from", "to_id":"to",
            "edge_kind":"evidence_claim", "record_id":"tos.claim.example.author",
            "source_ref":owner, "source_refs":[owner,anchor,"ToS/research.md"],
            "source_ref_bindings":{anchor:anchor_source}
        });
        let capture = |value: &serde_json::Value| {
            let mut rows = Vec::new();
            let mut paths = BTreeSet::new();
            capture_original_carrier(
                DurableNavigationBootstrapRowKindV1::Edge,
                "edge:claim".into(),
                serde_json::to_vec(value).unwrap(),
                limits,
                &mut rows,
                &mut paths,
                &mut 0,
                &mut BTreeMap::from([("from".into(), true), ("to".into(), true)]),
            )?;
            Ok::<_, DurableError>((rows, paths))
        };
        let (rows, paths) = capture(&row).unwrap();
        assert_eq!(rows[0].owner_source_path, owner);
        assert_eq!(rows[0].owner_record_id, "tos.claim.example.author");
        assert_eq!(
            paths,
            BTreeSet::from([
                owner.to_owned(),
                anchor_source.to_owned(),
                "ToS/research.md".into(),
            ])
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&rows[0].raw).unwrap(),
            row
        );
        let mut missing_owner = row.clone();
        missing_owner.as_object_mut().unwrap().remove("source_ref");
        assert!(matches!(
            capture(&missing_owner),
            Err(DurableError::Conflict(_))
        ));
        let mut unrelated = row.clone();
        unrelated["source_ref_bindings"]["tos.anchor.unrelated"] = json!(anchor_source);
        assert!(capture(&unrelated).is_err());
        let mut redirected_path = row;
        redirected_path["source_ref_bindings"]["ToS/research.md"] = json!(anchor_source);
        assert!(capture(&redirected_path).is_err());
    }

    #[test]
    fn shared_claim_carriers_survive_removal_of_one_contributor() {
        let limits = DurableNavigationEventLimits::default();
        let path = "ToS/source-witnesses/claims/example/source-claims.jsonl";
        let mut rows = Vec::new();
        for (kind, id, value) in [
            (
                DurableNavigationBootstrapRowKindV1::Node,
                "claim:a",
                json!({"node_id":"claim:a"}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Node,
                "claim:b",
                json!({"node_id":"claim:b"}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Node,
                "evidence:shared",
                json!({"node_id":"evidence:shared","properties":{"evidence_ref":"ToS/evidence.md"}}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Edge,
                "edge:a",
                json!({"edge_id":"edge:a","claim_ref":"a","source_claim_file_ref":path,"from_id":"claim:a","to_id":"evidence:shared"}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Edge,
                "edge:b",
                json!({"edge_id":"edge:b","claim_ref":"b","source_claim_file_ref":path,"from_id":"claim:b","to_id":"evidence:shared"}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Edge,
                "alternative:a",
                json!({"edge_id":"alternative:a","claim_ref":"a","source_claim_file_ref":path,"from_id":"claim:a","to_id":"claim:b"}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Edge,
                "subject:b",
                json!({"edge_id":"subject:b","claim_ref":"b","source_claim_file_ref":path,
                    "edge_kind":"claim_subject_navigation","predicate_id":"has_source_claim",
                    "from_id":"tos.work.a","to_id":"claim:b"}),
            ),
        ] {
            rows.push(DurableNavigationBootstrapRowV1 {
                kind,
                id: id.into(),
                owner_source_path: path.into(),
                owner_record_id: id.into(),
                visible: Some(true),
                raw: serde_json::to_vec(&value).unwrap(),
            });
        }
        rows.push(DurableNavigationBootstrapRowV1 {
            kind: DurableNavigationBootstrapRowKindV1::Node,
            id: "tos.work.a".into(),
            owner_source_path: "ToS/source-witnesses/works/a/work.json".into(),
            owner_record_id: "tos.work.a".into(),
            visible: Some(true),
            raw: br#"{"node_id":"tos.work.a"}"#.to_vec(),
        });
        let mut bytes = rows.iter().map(|row| row.raw.len() as u64).sum();
        expand_claim_contributions(&mut rows, limits, &mut bytes).unwrap();
        assert_eq!(
            rows.iter()
                .find(|row| row.id == "tos.work.a")
                .unwrap()
                .owner_record_id,
            "tos.work.a"
        );
        assert_eq!(
            rows.iter()
                .find(|row| row.id == "subject:b")
                .unwrap()
                .owner_record_id,
            "b"
        );
        assert_eq!(
            rows.iter()
                .filter(|row| row.id == "evidence:shared")
                .count(),
            2
        );
        assert_eq!(rows.iter().filter(|row| row.id == "claim:b").count(), 1);
        assert_eq!(
            bytes,
            rows.iter().map(|row| row.raw.len() as u64).sum::<u64>()
        );
        rows.retain(|row| row.owner_record_id != "a");
        assert_eq!(
            rows.iter()
                .map(|row| row.id.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "claim:b",
                "edge:b",
                "evidence:shared",
                "subject:b",
                "tos.work.a"
            ])
        );
        assert!(
            rows.iter()
                .filter(|row| row.id != "tos.work.a")
                .all(|row| row.owner_record_id == "b" && row.owner_source_path == path)
        );
    }

    #[test]
    fn original_history_rows_share_owner_and_fragment_refs_keep_exact_file() {
        let source = "ToS/source-witnesses/works/example/work.json";
        let archive = "ToS/source-witnesses/.record-revisions/example/previous.blob";
        let packets = [
            (
                DurableNavigationBootstrapRowKindV1::Node,
                json!({"node_id":"record-version:one","source_ref":format!("{archive}#L1"),"properties":{"record_version_view":{"record_ref":{"id":"work:example","version":1},"provenance":{"source":{"source_ref":source}}}}}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Node,
                json!({"node_id":"work:example","source_ref":source}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Edge,
                json!({"edge_id":"history:one","edge_kind":"exact_historical_record_reference","from_id":"work:example","to_id":"record-version:one","source_refs":[archive,source]}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Right,
                json!({"rights_id":"rights:example","record_id":"work:example","source_ref":source}),
            ),
        ];
        let mut rows = Vec::new();
        let mut paths = BTreeSet::new();
        let mut charged = 0;
        let mut visibility = BTreeMap::new();
        for (kind, value) in packets {
            let key = match kind {
                DurableNavigationBootstrapRowKindV1::Node => "node_id",
                DurableNavigationBootstrapRowKindV1::Edge => "edge_id",
                _ => "rights_id",
            };
            let raw = serde_json::to_vec(&value).unwrap();
            capture_original_carrier(
                kind,
                value[key].as_str().unwrap().into(),
                raw.clone(),
                DurableNavigationEventLimits::default(),
                &mut rows,
                &mut paths,
                &mut charged,
                &mut visibility,
            )
            .unwrap();
            assert_eq!(rows.last().unwrap().raw, raw);
        }
        let original_raw = rows.iter().map(|row| row.raw.clone()).collect::<Vec<_>>();
        bind_historical_record_owners(&mut rows, DurableNavigationEventLimits::default()).unwrap();
        assert_eq!(
            original_raw,
            rows.iter().map(|row| row.raw.clone()).collect::<Vec<_>>()
        );
        assert_eq!(
            paths,
            BTreeSet::from([source.to_owned(), archive.to_owned()])
        );
        assert!(rows.iter().all(|row| row.owner_record_id == "work:example" && row.owner_source_path == source));
        assert!(rows[..3].iter().all(|row| row.visible == Some(true)));
        assert_eq!(rows[3].visible, None);
        assert_eq!(
            charged,
            rows.iter().map(|row| row.raw.len() as u64).sum::<u64>()
        );
        rows[1].owner_source_path = "ToS/source-witnesses/works/another/work.json".into();
        assert!(
            bind_historical_record_owners(&mut rows, DurableNavigationEventLimits::default())
                .is_err()
        );
    }

    #[test]
    fn hidden_original_node_does_not_hide_missing_endpoint_corruption() {
        let mut rows = Vec::new();
        let mut paths = BTreeSet::new();
        let mut charged = 0;
        let mut visibility = BTreeMap::new();
        let limits = DurableNavigationEventLimits::default();
        let node = br#"{"node_id":"hidden","source_ref":"ToS/source-witnesses/works/example/work.json","properties":{"packet_id":"private"}}"#;
        capture_original_carrier(
            DurableNavigationBootstrapRowKindV1::Node,
            "hidden".into(),
            node.to_vec(),
            limits,
            &mut rows,
            &mut paths,
            &mut charged,
            &mut visibility,
        )
        .unwrap();
        assert_eq!(rows[0].visible, Some(false));
        let edge = br#"{"edge_id":"broken","from_id":"hidden","to_id":"missing","source_refs":["ToS/source-witnesses/works/example/work.json"]}"#;
        assert!(
            capture_original_carrier(
                DurableNavigationBootstrapRowKindV1::Edge,
                "broken".into(),
                edge.to_vec(),
                limits,
                &mut rows,
                &mut paths,
                &mut charged,
                &mut visibility
            )
            .is_err()
        );
        assert_eq!(rows.len(), 1);
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn capture_original_carrier(
    kind: DurableNavigationBootstrapRowKindV1,
    id: String,
    raw: Vec<u8>,
    limits: DurableNavigationEventLimits,
    output: &mut Vec<DurableNavigationBootstrapRowV1>,
    owner_source_paths: &mut BTreeSet<String>,
    charged_bytes: &mut u64,
    node_visibility: &mut BTreeMap<String, bool>,
) -> DurableResult<()> {
    if kind == DurableNavigationBootstrapRowKindV1::Node && node_visibility.contains_key(&id) {
        return Err(DurableError::Corrupt("duplicate original navigation node"));
    }
    let (row, refs) = decode_original_carrier(
        kind,
        id,
        raw,
        limits,
        |id| Ok(node_visibility.get(id).copied()),
        |id| {
            Ok(output
                .iter()
                .find(|row| {
                    row.kind == DurableNavigationBootstrapRowKindV1::Node
                        && row.id == id
                        && row.owner_record_id == id
                })
                .map(|row| row.owner_source_path.clone()))
        },
    )?;
    if kind == DurableNavigationBootstrapRowKindV1::Node {
        node_visibility.insert(row.id.clone(), row.visible.unwrap());
    }
    owner_source_paths.extend(refs);
    if owner_source_paths.len() > limits.max_dependencies {
        return Err(DurableError::Refused(
            "navigation bootstrap dependency source budget",
        ));
    }
    *charged_bytes = charged_bytes
        .checked_add(row.raw.len() as u64)
        .filter(|bytes| *bytes <= limits.max_total_bytes)
        .ok_or(DurableError::Refused("navigation bootstrap byte budget"))?;
    if output.len() >= limits.max_current_rows {
        return Err(DurableError::Refused("navigation bootstrap row budget"));
    }
    output.push(row);
    Ok(())
}

#[cfg(test)]
/// A version node keeps its exact archived source reference, but its view also
/// contains the current record/history provenance. It must therefore be
/// replaced with that record. Resolve ownership after all original nodes are
/// captured: historical node IDs may sort before the current record ID.
fn bind_historical_record_owners(
    rows: &mut [DurableNavigationBootstrapRowV1],
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    let current_owners = rows
        .iter()
        .filter(|row| {
            row.kind == DurableNavigationBootstrapRowKindV1::Node && row.id == row.owner_record_id
        })
        .map(|row| (row.id.clone(), row.owner_source_path.clone()))
        .collect::<BTreeMap<_, _>>();
    for row in rows
        .iter_mut()
        .filter(|row| row.kind == DurableNavigationBootstrapRowKindV1::Node)
    {
        bind_historical_record_owner(row, limits, |id| Ok(current_owners.get(id).cloned()))?;
    }
    Ok(())
}

#[cfg(test)]
/// Restore each reified Claim's contribution to shared carriers. PostgreSQL's
/// existing bootstrap key includes the owner pair; selecting one arbitrary
/// owner would lose surviving contributions when another Claim is replaced.
fn expand_claim_contributions(
    rows: &mut Vec<DurableNavigationBootstrapRowV1>,
    limits: DurableNavigationEventLimits,
    charged_bytes: &mut u64,
) -> DurableResult<()> {
    type Owner = (String, String);
    let mut owners = BTreeMap::<String, BTreeSet<Owner>>::new();
    let mut linked = BTreeMap::<String, String>::new();
    let mut claim_edges = BTreeMap::<String, Owner>::new();
    let local_nodes = rows
        .iter()
        .filter(|row| row.kind == DurableNavigationBootstrapRowKindV1::Node)
        .map(|row| row.id.clone())
        .collect::<BTreeSet<_>>();
    for row in rows.iter() {
        if let Some(identity) = maker_identity(row, limits)? {
            linked.insert(row.id.clone(), identity);
        }
        if let Some((owner, targets)) = claim_contribution(row, limits)? {
            for target in targets {
                owners.entry(target).or_default().insert(owner.clone());
            }
            claim_edges.insert(row.id.clone(), owner);
        }
    }
    for (node, identity) in linked {
        if local_nodes.contains(&identity)
            && let Some(contributors) = owners.get(&node).cloned()
        {
            owners.entry(identity).or_default().extend(contributors);
        }
    }
    let mut expanded = Vec::new();
    for row in std::mem::take(rows) {
        let contributions = match row.kind {
            DurableNavigationBootstrapRowKindV1::Node => owners.remove(&row.id),
            DurableNavigationBootstrapRowKindV1::Edge => claim_edges
                .remove(&row.id)
                .map(|owner| BTreeSet::from([owner])),
            _ => None,
        };
        if let Some(contributions) = contributions {
            for (index, (path, id)) in contributions.into_iter().enumerate() {
                if index > 0 {
                    *charged_bytes = charged_bytes
                        .checked_add(row.raw.len() as u64)
                        .filter(|n| *n <= limits.max_total_bytes)
                        .ok_or(DurableError::Refused("navigation Claim contribution bytes"))?;
                }
                if expanded.len() >= limits.max_current_rows {
                    return Err(DurableError::Refused("navigation Claim contribution rows"));
                }
                expanded.push(DurableNavigationBootstrapRowV1 {
                    kind: row.kind,
                    id: row.id.clone(),
                    owner_source_path: path,
                    owner_record_id: id,
                    visible: row.visible,
                    raw: row.raw.clone(),
                });
            }
        } else {
            if expanded.len() >= limits.max_current_rows {
                return Err(DurableError::Refused(
                    "navigation bootstrap contribution rows",
                ));
            }
            expanded.push(row);
        }
    }
    // A maker's identity may already be represented by the same subject/object,
    // but every retained ownership target must be an actual selected carrier.
    if !owners.is_empty() || !claim_edges.is_empty() {
        return Err(DurableError::Corrupt(
            "navigation Claim contribution carrier absent",
        ));
    }
    *rows = expanded;
    Ok(())
}
