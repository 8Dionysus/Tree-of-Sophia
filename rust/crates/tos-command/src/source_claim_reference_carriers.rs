//! Fixed-slot exported Claim ABI, matching knowledge._validate_reference_claim_carriers.
//! Called inside the bounded candidate normalizer after raw carriers are parsed.
//! This checks structural projections, never source/history or semantic admission.
use super::validation_digest;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use tos_compiler::{Error, KnowledgeRegistry, Result};

const INVALID: &str = "incomplete or inconsistent reference-value Claim carriers";
fn require(ok: bool) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid(INVALID))
    }
}
fn text<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}
fn array<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}
fn strings(v: &Value) -> Result<Vec<&str>> {
    v.as_array()
        .ok_or(Error::Invalid(INVALID))?
        .iter()
        .map(|x| x.as_str().ok_or(Error::Invalid(INVALID)))
        .collect()
}
fn is_a<'a>(id: &'a str, allowed: &[&str], entries: &BTreeMap<&'a str, &'a Value>) -> bool {
    let mut pending = vec![id];
    let mut seen = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if allowed.contains(&id) {
            return true;
        }
        if seen.insert(id) {
            if let Some(entry) = entries.get(id) {
                pending.extend(
                    array(entry, "parent_type_ids")
                        .iter()
                        .filter_map(Value::as_str),
                );
            }
        }
    }
    false
}
fn exact_keys(v: &Value, keys: &[&str]) -> bool {
    v.as_object()
        .is_some_and(|m| m.len() == keys.len() && keys.iter().all(|k| m.contains_key(*k)))
}
fn reference_ids(v: &Value) -> Result<Vec<&str>> {
    let values = v.as_array().ok_or(Error::Invalid(INVALID))?;
    require((1..=8).contains(&values.len()))?;
    values
        .iter()
        .map(|v| {
            require(exact_keys(v, &["id", "version", "digest"]))?;
            let id = text(v, "id").ok_or(Error::Invalid(INVALID))?;
            require(
                v["version"]
                    .as_u64()
                    .is_some_and(|n| (1..=9_007_199_254_740_991).contains(&n)),
            )?;
            let digest = text(v, "digest").ok_or(Error::Invalid(INVALID))?;
            require(digest.strip_prefix("sha256:").is_some_and(|s| {
                s.len() == 64
                    && s.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            }))?;
            Ok(id)
        })
        .collect()
}

/// `raw` includes retained carriers; `selected_ids` chooses the candidate Claim
/// owners to validate. Edges/traces must be the complete admitted cohorts, not
/// only their changed rows. All collections share the caller's input budget.
pub(super) fn validate(
    raw: &BTreeMap<String, Value>,
    selected_ids: &[String],
    traces: &[Value],
    edges: &[Value],
    normalized: &BTreeMap<String, Value>,
    registry: &KnowledgeRegistry,
    entity_registry: &Value,
    relation_registry: &Value,
    cap: usize,
) -> Result<()> {
    let entities: BTreeMap<_, _> = array(entity_registry, "types")
        .iter()
        .filter_map(|v| text(v, "type_id").map(|id| (id, v)))
        .collect();
    let policies: BTreeMap<_, _> = array(relation_registry, "relations")
        .iter()
        .filter_map(|v| text(v, "relation_type_id").map(|id| (id, v)))
        .collect();
    let mut by_claim: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    let mut from: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    let mut claimed: BTreeMap<&str, usize> = BTreeMap::new();
    for trace in traces {
        if let Some(id) = text(trace, "claim_node_id") {
            by_claim.entry(id).or_default().push(trace);
        }
    }
    for edge in edges {
        if text(edge, "edge_kind") == Some("has_value_member") {
            if let Some(id) = text(edge, "from_id") {
                from.entry(id).or_default().push(edge);
            }
            if let Some(id) = text(edge, "claim_ref") {
                *claimed.entry(id).or_default() += 1;
            }
        }
    }
    let selected: BTreeSet<_> = selected_ids.iter().map(String::as_str).collect();
    for (native, node) in raw {
        if text(node, "node_kind") == Some("claim")
            && !selected.contains(format!("source-claims:{native}").as_str())
        {
            continue;
        }
        let properties = &node["properties"];
        let source = &properties["source_claim"];
        let trace_rows = by_claim
            .get(native.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let predicates: Vec<_> = std::iter::once(text(source, "predicate"))
            .chain(std::iter::once(text(properties, "predicate")))
            .chain(trace_rows.iter().map(|v| text(v, "predicate")))
            .collect();
        let profiles: Vec<_> = predicates
            .iter()
            .map(|p| {
                p.and_then(|p| {
                    policies.get(
                        registry
                            .relation("source-claims", p, "claim-predicate")
                            .type_id,
                    )
                })
                .map(|p| &p["source_claim_profile"])
                .unwrap_or(&Value::Null)
            })
            .collect();
        let supported = |p: &Value| {
            matches!(
                text(p, "reader"),
                Some(
                    "structured-reference-value-v1"
                        | "identity-transition-v1"
                        | "identity-transition-v2"
                )
            )
        };
        if !profiles.iter().any(|p| supported(p)) {
            continue;
        }
        require(
            text(node, "node_kind") == Some("claim")
                && trace_rows.len() == 1
                && text(source, "claim_id").is_some()
                && predicates.iter().all(|p| *p == text(source, "predicate"))
                && supported(profiles[0]),
        )?;
        let reader = text(profiles[0], "reader").unwrap();
        let identity = reader.starts_with("identity-transition-");
        let semantic = reader == "identity-transition-v2";
        let trace = trace_rows[0];
        let source_id = text(source, "claim_id").unwrap();
        let value = &source["object"];
        let members = strings(&value["members"])?;
        let rules = &profiles[0]["object_reference_set"];
        let (min, max, subject_member) = if identity {
            (3, 9, true)
        } else {
            (
                rules["min_items"].as_u64().ok_or(Error::Invalid(INVALID))? as usize,
                rules["max_items"].as_u64().ok_or(Error::Invalid(INVALID))? as usize,
                rules["subject_is_member"]
                    .as_bool()
                    .ok_or(Error::Invalid(INVALID))?,
            )
        };
        let unique: BTreeSet<_> = members.iter().copied().collect();
        require(
            members.len() >= min
                && members.len() <= max
                && members.iter().all(|m| !m.is_empty())
                && unique.len() == members.len()
                && !unique.contains(source_id)
                && (!subject_member
                    || text(source, "subject_ref").is_some_and(|s| unique.contains(s))),
        )?;
        if identity {
            require(
                text(source, "predicate")
                    == Some(if semantic {
                        "subject_identity_transition_proposal"
                    } else {
                        "identity_transition_proposal"
                    })
                    && text(source, "schema_version")
                        == Some(if semantic {
                            "tos_subject_identity_transition_claim_v1"
                        } else {
                            "tos_source_identity_transition_claim_v1"
                        })
                    && text(value, "kind") == Some("identity-transition-proposal")
                    && text(source, "assertion_layer") == Some("identity_assertion"),
            )?;
            let left = reference_ids(&value["predecessors"])?;
            let right = reference_ids(&value["successors"])?;
            let union: BTreeSet<_> = left.iter().chain(&right).copied().collect();
            require(
                union.len() == left.len() + right.len()
                    && union == unique
                    && text(source, "subject_ref").is_some_and(|s| left.contains(&s))
                    && (text(value, "operation") == Some("merge")
                        && left.len() >= 2
                        && right.len() == 1
                        || text(value, "operation") == Some("split")
                            && left.len() == 1
                            && right.len() >= 2),
            )?;
            let mapping = value["mapping"].as_array().ok_or(Error::Invalid(INVALID))?;
            require(mapping.len() == left.len() * right.len())?;
            let mut pairs = BTreeSet::new();
            for edge in mapping {
                require(exact_keys(edge, &["predecessor", "successor"]))?;
                pairs.insert((
                    text(edge, "predecessor").ok_or(Error::Invalid(INVALID))?,
                    text(edge, "successor").ok_or(Error::Invalid(INVALID))?,
                ));
            }
            require(
                pairs
                    == left
                        .iter()
                        .flat_map(|a| right.iter().map(move |b| (*a, *b)))
                        .collect(),
            )?;
        }
        let expected: BTreeSet<_> = members.iter().map(|m| format!("identity:{m}")).collect();
        let declared = strings(&trace["value_member_node_ids"])?;
        let literal = text(trace, "object_node_id")
            .and_then(|id| raw.get(id))
            .ok_or(Error::Invalid(INVALID))?;
        require(
            text(trace, "claim_ref") == Some(source_id)
                && text(properties, "claim_ref") == Some(source_id)
                && text(source, "subject_ref").is_some_and(|s| {
                    text(trace, "subject_node_id") == Some(format!("identity:{s}").as_str())
                })
                && declared.len() == expected.len()
                && declared.iter().copied().collect::<BTreeSet<_>>()
                    == expected.iter().map(String::as_str).collect()
                && text(literal, "node_kind") == Some("literal"),
        )?;
        let digest = validation_digest(value, cap)?;
        require(
            validation_digest(&properties["object"], cap)? == digest
                && validation_digest(&literal["properties"]["value"], cap)? == digest,
        )?;
        let member_edges = from.get(native.as_str()).map(Vec::as_slice).unwrap_or(&[]);
        require(
            member_edges.len() == expected.len()
                && claimed.get(source_id) == Some(&expected.len())
                && trace["edge_ids"].is_array(),
        )?;
        let mut targets = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for edge in member_edges {
            let id = text(edge, "edge_id").ok_or(Error::Invalid(INVALID))?;
            let target = text(edge, "to_id").ok_or(Error::Invalid(INVALID))?;
            require(
                text(edge, "claim_ref") == Some(source_id)
                    && expected.contains(target)
                    && array(trace, "edge_ids")
                        .iter()
                        .any(|v| v.as_str() == Some(id)),
            )?;
            targets.insert(target);
            ids.insert(id);
        }
        require(targets.len() == expected.len() && ids.len() == expected.len())?;
        let allowed = if identity {
            if semantic {
                vec!["tos.entity.identity", "tos.entity.semantic-object"]
            } else {
                vec!["tos.entity.identity"]
            }
        } else {
            strings(&rules["member_type_ids"])?
        };
        for member in members {
            let id = format!("identity:{member}");
            let carrier = raw.get(&id).ok_or(Error::Invalid(INVALID))?;
            let attributes = &carrier["properties"];
            let node = normalized
                .get(&format!("source-claims:{id}"))
                .ok_or(Error::Invalid(INVALID))?;
            let type_id = text(node, "type_id").ok_or(Error::Invalid(INVALID))?;
            require(
                text(carrier, "node_kind") == Some("identity")
                    && text(attributes, "identity_ref") == Some(member)
                    && is_a(type_id, &allowed, &entities),
            )?;
            if identity {
                let entry = entities.get(type_id).ok_or(Error::Invalid(INVALID))?;
                require(entry["abstract"] == Value::Bool(false))?;
                if text(entry, "object_role") == Some("identity") {
                    continue;
                }
                let profile = &entry["source_record_profile"];
                let record = &attributes["source_record"];
                let kind = text(record, "record_type").ok_or(Error::Invalid(INVALID))?;
                let source_ref = text(carrier, "source_ref").ok_or(Error::Invalid(INVALID))?;
                require(
                    semantic
                        && text(entry, "object_role") == Some("semantic")
                        && profile.is_object()
                        && record.is_object()
                        && text(profile, "reader") == Some("semantic-metadata-v1")
                        && text(profile, "identity_proposal_adapter")
                            == Some("exact-semantic-metadata-v1")
                        && text(profile, "graph_layer") == Some("source-profile")
                        && !["claim", "literal", "temporal-assertion"].contains(&kind)
                        && text(profile, "record_type") == Some(kind)
                        && text(profile, "id_prefix") == Some(format!("tos.{kind}.").as_str())
                        && text(profile, "source_basename")
                            == Some(format!("{kind}.json").as_str())
                        && text(record, "record_id") == Some(member)
                        && member.starts_with(&format!("tos.{kind}."))
                        && matches!(
                            text(record, "visibility"),
                            Some("public" | "public_metadata_only")
                        )
                        && array(profile, "schemas")
                            .iter()
                            .filter(|r| r["schema_version"] == record["schema_version"])
                            .count()
                            == 1
                        && !source_ref.contains(['\\', '\0'])
                        && source_ref.starts_with("ToS/source-witnesses/")
                        && source_ref.rsplit('/').next() == Some(format!("{kind}.json").as_str())
                        && source_ref.split('/').all(|p| {
                            !p.is_empty()
                                && !p.starts_with('.')
                                && ![
                                    "catalog",
                                    "payload",
                                    "private",
                                    "local-content",
                                    "owner-local",
                                ]
                                .contains(&p)
                        })
                        && ["source-claims", "source-navigation"].iter().all(|graph| {
                            array(entry, "source_mappings")
                                .iter()
                                .filter(|m| {
                                    text(m, "source_graph") == Some(*graph)
                                        && text(m, "source_kind_id") == Some(kind)
                                })
                                .count()
                                == 1
                        }),
                )?;
            }
        }
    }
    Ok(())
}
