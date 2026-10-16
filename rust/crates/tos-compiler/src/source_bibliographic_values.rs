//! Owner-defined fixed structured value mechanics; arbitrary JSON stays inert.
use super::source_bibliographic_render::{array, text};
use crate::{Error, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
pub(crate) fn members<'a>(claim: &'a Value, profile: &Value) -> Result<Vec<&'a str>> {
    let reader = text(profile, "reader")?;
    if matches!(reader, "identity-transition-v1" | "identity-transition-v2") {
        return proposal_members(claim);
    }
    if reader != "structured-reference-value-v1" {
        return Ok(Vec::new());
    }
    let constraint = &profile["object_reference_set"];
    let members = array(&claim["object"], "members")?;
    let min = constraint["min_items"]
        .as_u64()
        .ok_or(Error::Invalid("bibliographic reference-set lower bound"))?;
    let max = constraint["max_items"]
        .as_u64()
        .ok_or(Error::Invalid("bibliographic reference-set upper bound"))?;
    if (members.len() as u64) < min || (members.len() as u64) > max || members.len() > 4096 {
        return Err(Error::Invalid("bibliographic reference-set cardinality"));
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for member in members {
        let id = member
            .as_str()
            .ok_or(Error::Invalid("bibliographic reference-set member"))?;
        if !seen.insert(id) || claim["claim_id"] == id {
            return Err(Error::Invalid(
                "bibliographic reference-set distinct identity",
            ));
        }
        out.push(id);
    }
    if constraint["subject_is_member"] == true && !seen.contains(text(claim, "subject_ref")?) {
        return Err(Error::Invalid(
            "bibliographic reference-set subject membership",
        ));
    }
    if constraint["structure_adapter"] == "scoped-members-v1" {
        validate_order(claim)?
    }
    Ok(out)
}
fn validate_order(claim: &Value) -> Result<()> {
    let value = &claim["object"];
    let members = array(value, "members")?;
    let mut outgoing = BTreeMap::<&str, BTreeSet<&str>>::new();
    let mut indegree = BTreeMap::<&str, u64>::new();
    for member in members {
        let id = member
            .as_str()
            .ok_or(Error::Invalid("bibliographic ordered member identity"))?;
        outgoing.insert(id, BTreeSet::new());
        indegree.insert(id, 0);
    }
    if indegree.contains_key(text(claim, "subject_ref")?) {
        return Err(Error::Invalid("bibliographic scoped self-composition"));
    }
    let order = &value["ordering"];
    let edges = array(order, "precedes")?;
    if order["mode"] == "unordered" && !edges.is_empty() {
        return Err(Error::Invalid("bibliographic unordered precedence"));
    }
    if members.len() > 128 || edges.len() > 8128 {
        return Err(Error::Budget("bibliographic scoped member structure"));
    }
    for pair in edges {
        let pair = pair
            .as_array()
            .filter(|p| p.len() == 2)
            .ok_or(Error::Invalid("bibliographic precedence pair"))?;
        let before = pair[0]
            .as_str()
            .ok_or(Error::Invalid("bibliographic precedence source"))?;
        let after = pair[1]
            .as_str()
            .ok_or(Error::Invalid("bibliographic precedence target"))?;
        if !indegree.contains_key(before) || !indegree.contains_key(after) {
            return Err(Error::Invalid("bibliographic precedence member closure"));
        }
        outgoing
            .get_mut(before)
            .ok_or(Error::Invalid("bibliographic precedence source closure"))?
            .insert(after);
        *indegree
            .get_mut(after)
            .ok_or(Error::Invalid("bibliographic precedence target closure"))? += 1;
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(id, n)| if *n == 0 { Some(*id) } else { None })
        .collect::<Vec<_>>();
    let mut visited = 0;
    while !ready.is_empty() {
        if order["mode"] == "total" && ready.len() != 1 {
            return Err(Error::Invalid("bibliographic incomplete total precedence"));
        }
        let member = ready
            .pop()
            .ok_or(Error::Invalid("bibliographic order ready"))?;
        visited += 1;
        for target in &outgoing[member] {
            let count = indegree
                .get_mut(target)
                .ok_or(Error::Invalid("bibliographic order target"))?;
            *count -= 1;
            if *count == 0 {
                ready.push(target);
            }
        }
    }
    if visited != members.len() {
        return Err(Error::Invalid("bibliographic cyclic scoped order"));
    }
    if value["kind"] == "collection-member-order" {
        let collection = &value["collection_version"];
        let bindings = array(value, "membership_versions")?;
        if !exact_ref(collection, false)
            || collection["id"] != claim["subject_ref"]
            || !text(collection, "id")?.starts_with("tos.collection.")
            || bindings.len() != members.len()
        {
            return Err(Error::Invalid("bibliographic exact Collection order basis"));
        }
        let mut seen = BTreeSet::new();
        for binding in bindings {
            if !exact_ref(binding, true) || !seen.insert(text(binding, "id")?) {
                return Err(Error::Invalid(
                    "bibliographic exact distinct membership basis",
                ));
            }
        }
    }
    Ok(())
}
fn exact_ref(value: &Value, claim: bool) -> bool {
    value.as_object().is_some_and(|o| {
        o.len() == 3
            && o.contains_key("id")
            && o.contains_key("version")
            && o.contains_key("digest")
    }) && value["id"]
        .as_str()
        .is_some_and(|id| id.starts_with(if claim { "tos.claim." } else { "tos." }))
        && value["version"]
            .as_u64()
            .is_some_and(|v| v > 0 && v <= 9_007_199_254_740_991)
        && value["digest"].as_str().is_some_and(|d| {
            d.starts_with("sha256:")
                && d.len() == 71
                && d[7..]
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}
fn proposal_members(claim: &Value) -> Result<Vec<&str>> {
    let value = &claim["object"];
    let predecessors = array(value, "predecessors")?;
    let successors = array(value, "successors")?;
    let members = array(value, "members")?;
    let mapping = array(value, "mapping")?;
    if predecessors.is_empty()
        || predecessors.len() > 8
        || successors.is_empty()
        || successors.len() > 8
        || members.len() < 3
        || members.len() > 9
        || mapping.len() < 2
        || mapping.len() > 8
    {
        return Err(Error::Invalid("bibliographic proposal cardinality"));
    }
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for reference in predecessors.iter().chain(successors) {
        if !exact_ref(reference, false) {
            return Err(Error::Invalid("bibliographic proposal exact participant"));
        }
        let id = text(reference, "id")?;
        if !seen.insert(id) || claim["claim_id"] == id {
            return Err(Error::Invalid(
                "bibliographic distinct proposal participant",
            ));
        }
        result.push(id);
    }
    let declared = members
        .iter()
        .map(|v| {
            v.as_str()
                .ok_or(Error::Invalid("bibliographic proposal member"))
        })
        .collect::<Result<BTreeSet<_>>>()?;
    if declared != seen
        || members.len() != result.len()
        || !predecessors.iter().any(|r| r["id"] == claim["subject_ref"])
    {
        return Err(Error::Invalid(
            "bibliographic proposal member/focal closure",
        ));
    }
    let merge = value["operation"] == "merge" && predecessors.len() >= 2 && successors.len() == 1;
    let split = value["operation"] == "split" && predecessors.len() == 1 && successors.len() >= 2;
    if !merge && !split {
        return Err(Error::Invalid("bibliographic proposal topology"));
    }
    let expected = predecessors
        .iter()
        .flat_map(|old| {
            successors
                .iter()
                .map(move |new| (text(old, "id").unwrap_or(""), text(new, "id").unwrap_or("")))
        })
        .collect::<BTreeSet<_>>();
    let actual = mapping
        .iter()
        .map(|e| Ok((text(e, "predecessor")?, text(e, "successor")?)))
        .collect::<Result<BTreeSet<_>>>()?;
    if actual != expected || mapping.len() != expected.len() {
        return Err(Error::Invalid(
            "bibliographic exact complete proposal mapping",
        ));
    }
    let previous = &value["supersedes_proposal"];
    if !previous.is_null() && (!exact_ref(previous, true) || previous["id"] == claim["claim_id"]) {
        return Err(Error::Invalid("bibliographic predecessor proposal"));
    }
    if claim.get("supersedes_claim_ref").unwrap_or(&Value::Null)
        != if previous.is_null() {
            &Value::Null
        } else {
            &previous["id"]
        }
    {
        return Err(Error::Invalid(
            "bibliographic proposal succession reference",
        ));
    }
    let links = array(value, "unresolved_links")?;
    if links.len() > 32 {
        return Err(Error::Budget("bibliographic proposal unresolved links"));
    }
    for link in links {
        if !exact_ref(&link["claim"], true) || link["claim"]["id"] == claim["claim_id"] {
            return Err(Error::Invalid("bibliographic unresolved exact Claim link"));
        }
    }
    Ok(result)
}
