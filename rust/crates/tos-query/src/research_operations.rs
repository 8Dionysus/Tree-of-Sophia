//! Small, reproducible research operators; labels and evidence remain supplied by their owner.
use serde_json::{Value, json};

pub fn execute(request: &Value) -> Result<Value, String> {
    let operation = request
        .get("operation")
        .and_then(Value::as_str)
        .ok_or("operation required")?;
    for name in ["scope", "versions"] {
        required(request, name)?;
    }
    let input = required(request, "input")?;
    if operation == "signed_support"
        && request.get("scope") != input.get("claim").and_then(|c| c.get("scope"))
    {
        return Err("request scope must match claim scope".into());
    }
    let result = match operation {
        "signed_support" => signed_support(input)?,
        "reading_distance" | "commitment_order" => readings(operation, input)?,
        _ => {
            let (result, explanation) = other_operation(operation, input)?;
            return Ok(
                json!({"request": request, "rule_version": "tos-research-operations@1", "result": result, "explanation": explanation}),
            );
        }
    };
    Ok(
        json!({"request":request,"rule_version":"tos-research-operations@1", "explanation":result["explanation"], "result":result}),
    )
}

fn signed_support(input: &Value) -> Result<Value, String> {
    let claim = input.get("claim").ok_or("claim required")?;
    let id = claim
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or("claim.id required")?;
    let version = claim
        .get("version")
        .filter(|x| !x.is_null())
        .ok_or("claim.version required")?;
    let scope = claim
        .get("scope")
        .filter(|x| !x.is_null())
        .ok_or("claim.scope required")?;
    let evidence = input
        .get("evidence")
        .and_then(Value::as_array)
        .ok_or("evidence array required")?;
    let mut positive = Vec::new();
    let mut negative = Vec::new();
    for item in evidence {
        if item.get("claim_id").and_then(Value::as_str) != Some(id)
            || item.get("claim_version") != Some(version)
            || item.get("scope") != Some(scope)
        {
            return Err("evidence must address the exact claim, version and scope".into());
        }
        if item
            .get("ref")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Err("evidence ref required".into());
        }
        match item.get("sign").and_then(Value::as_str) {
            Some("positive") => positive.push(item.clone()),
            Some("negative") => negative.push(item.clone()),
            _ => return Err("evidence sign must be positive or negative".into()),
        }
    }
    let beta = [!positive.is_empty(), !negative.is_empty()];
    let status = match beta {
        [false, false] => "no_evidence",
        [true, false] => "support",
        [false, true] => "refutation",
        _ => "both",
    };
    Ok(
        json!({"claim":claim,"positive":positive,"negative":negative,"beta":beta,"status":status,
        "explanation":"Summary of explicitly classified evidence for this exact claim and scope; it is not a judgment of truth or source acceptance."}),
    )
}

fn readings(operation: &str, input: &Value) -> Result<Value, String> {
    let left = input.get("left").ok_or("left required")?;
    let right = input.get("right").ok_or("right required")?;
    for profile in [left, right] {
        if profile
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .is_none()
            || profile
                .get("version")
                .filter(|v| !v.is_null() && *v != "")
                .is_none()
        {
            return Err("profile id and version required".into());
        }
    }
    let lv = left
        .get("axes_version")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or("left axes_version required")?;
    let rv = right
        .get("axes_version")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or("right axes_version required")?;
    if lv != rv {
        return Err("axes versions differ; supply an explicit mapping before comparison".into());
    }
    let axes = input
        .get("axes")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or("axes required")?;
    let l = left
        .get("values")
        .and_then(Value::as_array)
        .ok_or("left values required")?;
    let r = right
        .get("values")
        .and_then(Value::as_array)
        .ok_or("right values required")?;
    if l.len() != axes.len() || r.len() != axes.len() {
        return Err("each profile must match the axes".into());
    }
    let mut missing = Vec::new();
    let mut different = Vec::new();
    let mut only_left = Vec::new();
    let mut only_right = Vec::new();
    let mut common = Vec::new();
    for (i, axis) in axes.iter().enumerate() {
        if axis
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Err("axis id required".into());
        }
        let valid = |v: &Value| v.is_null() || v.as_u64() == Some(0) || v.as_u64() == Some(1);
        if !valid(&l[i]) || !valid(&r[i]) {
            return Err("profile values must be 0, 1 or null".into());
        }
        if l[i].is_null() {
            missing.push(json!({"side":"left","axis":axis}));
        }
        if r[i].is_null() {
            missing.push(json!({"side":"right","axis":axis}));
        }
        if !l[i].is_null() && !r[i].is_null() {
            if l[i] != r[i] {
                different.push(json!({"axis":axis,"left":l[i],"right":r[i]}));
            }
            match (l[i].as_u64(), r[i].as_u64()) {
                (Some(1), Some(1)) => common.push(axis.clone()),
                (Some(1), Some(0)) => only_left.push(axis.clone()),
                (Some(0), Some(1)) => only_right.push(axis.clone()),
                _ => (),
            }
        }
    }
    if !missing.is_empty() {
        return Ok(
            json!({"status":"incomplete","missing":missing,"explanation":"Null is unannotated, not zero; complete the profiles before comparing."}),
        );
    }
    if operation == "reading_distance" {
        let h = different.len();
        Ok(
            json!({"status":"complete","hamming":h,"euclidean":(h as f64).sqrt(),"differences":different,"explanation":"Equal-weight binary coordinates: Hamming counts differences and Euclidean is the square root of that count."}),
        )
    } else {
        let relation = match (only_left.is_empty(), only_right.is_empty()) {
            (true, true) => "equal",
            (true, false) => "left_subset",
            (false, true) => "right_subset",
            (false, false) => "incomparable",
        };
        Ok(
            json!({"status":"complete","relation":relation,"common":common,"only_left":only_left,"only_right":only_right,"explanation":"Order is set inclusion of explicitly present commitments; it is not textual entailment."}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profiles(a: Value, b: Value) -> Value {
        json!({"axes":[{"id":"f1"},{"id":"f2"},{"id":"f3"},{"id":"f4"}],"left":{"id":"a","version":"1","axes_version":"v1","values":a},"right":{"id":"b","version":"1","axes_version":"v1","values":b}})
    }
    #[test]
    fn specified_readings_and_unknown() {
        let v = profiles(json!([1, 1, 0, 0]), json!([1, 0, 1, 0]));
        let d = readings("reading_distance", &v).unwrap();
        assert_eq!(d["hamming"], 2);
        assert!((d["euclidean"].as_f64().unwrap() - 2_f64.sqrt()).abs() < 1e-12);
        assert_eq!(
            readings("commitment_order", &v).unwrap()["relation"],
            "incomparable"
        );
        let v = profiles(json!([1, 0, 0, 0]), json!([1, 1, 0, 0]));
        assert_eq!(
            readings("commitment_order", &v).unwrap()["relation"],
            "left_subset"
        );
        let v = profiles(json!([1, null, 0, 0]), json!([1, 1, 0, 0]));
        assert_eq!(
            readings("reading_distance", &v).unwrap()["status"],
            "incomplete"
        );
    }
    #[test]
    fn authored_case_preserves_separate_p_and_q() {
        let case: Value = serde_json::from_str(include_str!(
            "../../../../bonsai/zarathustra/research-operations.json"
        ))
        .unwrap();
        let claims = case["claims"].as_array().unwrap();
        for (claim, expected) in claims
            .iter()
            .zip([json!([true, false]), json!([false, true])])
        {
            let request = json!({"operation":"signed_support", "scope":claim["scope"], "versions":{"case":case["schema_version"]}, "input":{"claim":claim,"evidence":claim["evidence"]}});
            assert_eq!(execute(&request).unwrap()["result"]["beta"], expected);
        }
        let mut input =
            json!({"axes":case["axes"],"left":case["profiles"][0],"right":case["profiles"][1]});
        assert_eq!(readings("reading_distance", &input).unwrap()["hamming"], 2);
        input["right"]["axes_version"] = json!("another-version");
        assert!(readings("reading_distance", &input).is_err());
    }
    #[test]
    fn signed_evidence_cannot_cross_claim_or_scope() {
        let mut v = json!({"claim":{"id":"p","version":1,"scope":"s"},"evidence":[{"claim_id":"p","claim_version":1,"scope":"s","ref":"r","sign":"positive"}]});
        assert_eq!(signed_support(&v).unwrap()["beta"], json!([true, false]));
        v["evidence"][0]["claim_id"] = json!("q");
        assert!(signed_support(&v).is_err());
    }
}

fn required<'a>(value: &'a Value, name: &str) -> Result<&'a Value, String> {
    value
        .get(name)
        .filter(|x| !x.is_null() && *x != "")
        .ok_or_else(|| format!("{name} required"))
}
fn array<'a>(value: &'a Value, name: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(name)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{name} must be an array"))
}
fn unique(items: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    for item in items {
        if !out.contains(item) {
            out.push(item.clone());
        }
    }
    out
}
fn other_operation(operation: &str, input: &Value) -> Result<(Value, &'static str), String> {
    match operation {
        "distribution" => {
            let items = array(input, "items")?;
            let witness = required(input, "witness")?;
            let kind = required(input, "search_kind")?;
            let matches: Vec<&Value> = items
                .iter()
                .filter(|item| item.get("matches") == Some(&Value::Bool(true)))
                .collect();
            let mut groups = serde_json::Map::new();
            for item in &matches {
                let key = item
                    .get("group")
                    .filter(|x| !x.is_null())
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let count = groups.get(key).and_then(Value::as_u64).unwrap_or(0) + 1;
                groups.insert(key.into(), json!(count));
            }
            let locations: Vec<Value> = matches
                .iter()
                .map(|x| x.get("location").cloned().unwrap_or(Value::Null))
                .collect();
            let unlocated = locations
                .iter()
                .filter(|x| x.is_null() || *x == "" || *x == &Value::Bool(false) || *x == &json!(0))
                .count();
            Ok((
                json!({"witness":witness,"search_kind":kind,"denominator":items.len(),"count":matches.len(),"groups":groups,"locations":locations,"unlocated":unlocated}),
                "Count of explicitly matched items in the supplied selection; denominator is the number of supplied items.",
            ))
        }
        "directed_path" => {
            let edges = array(input, "edges")?;
            let allowed = array(input, "allowed_types")?;
            let start = required(input, "from")?;
            let target = required(input, "to")?;
            let mut queue = vec![start.clone()];
            let mut previous: Vec<(Value, Value)> = Vec::new();
            let mut i = 0;
            while i < queue.len() && !queue.contains(target) {
                let here = queue[i].clone();
                for edge in edges {
                    let Some(to) = edge.get("to") else { continue };
                    if edge.get("from") != Some(&here)
                        || !edge.get("type").is_some_and(|v| allowed.contains(v))
                        || queue.contains(to)
                    {
                        continue;
                    }
                    queue.push(to.clone());
                    previous.push((to.clone(), edge.clone()));
                }
                i += 1;
            }
            let found = queue.contains(target);
            let mut path = Vec::new();
            if found {
                let mut at = target;
                while at != start {
                    let (_, edge) = previous
                        .iter()
                        .find(|(to, _)| to == at)
                        .ok_or("broken path")?;
                    path.push(edge.clone());
                    at = required(edge, "from")?;
                }
                path.reverse();
            }
            Ok((
                json!({"found":found,"visited_nodes":queue.len(),"transitions":path.len(),"edges":path}),
                "Shortest directed path over supplied edges of the allowed types; each returned edge retains its own provenance.",
            ))
        }
        "common_grounds" => {
            let left = unique(array(input, "left")?);
            let right = unique(array(input, "right")?);
            let common: Vec<Value> = left.iter().filter(|v| right.contains(v)).cloned().collect();
            let only_left: Vec<Value> = left
                .iter()
                .filter(|v| !right.contains(v))
                .cloned()
                .collect();
            let only_right: Vec<Value> = right
                .iter()
                .filter(|v| !left.contains(v))
                .cloned()
                .collect();
            let distance = if left.is_empty() || right.is_empty() {
                None
            } else {
                Some(1.0 - common.len() as f64 / (left.len() + right.len() - common.len()) as f64)
            };
            Ok((
                json!({"common":common,"only_left":only_left,"only_right":only_right,"jaccard_distance":distance}),
                "Exact identity intersection and differences; Jaccard distance is unknown when either selection is empty.",
            ))
        }
        "interpretation_comparison" => {
            let axes = array(input, "axes")?;
            let left = required(input, "left")?;
            let right = required(input, "right")?;
            let features: Vec<Value> = axes
                .iter()
                .map(|axis| {
                    let id = axis.get("id").and_then(Value::as_str).unwrap_or("");
                    let a = left
                        .get(id)
                        .filter(|v| !v.is_null())
                        .unwrap_or(&Value::Null);
                    let b = right
                        .get(id)
                        .filter(|v| !v.is_null())
                        .unwrap_or(&Value::Null);
                    let status = if a.is_null() || b.is_null() {
                        "unknown"
                    } else if a == b {
                        "same"
                    } else {
                        "different"
                    };
                    json!({"axis":axis,"left":a,"right":b,"status":status})
                })
                .collect();
            Ok((
                json!({"features":features}),
                "Comparison of declared features; missing values are unknown and do not imply absence.",
            ))
        }
        "argument_structure" => {
            let steps = array(input, "steps")?;
            let empty = Vec::new();
            let objections = if input.get("objections").is_none_or(Value::is_null) {
                &empty
            } else {
                array(input, "objections")?
            };
            let unattached: Vec<&Value> = objections
                .iter()
                .filter(|o| !steps.iter().any(|s| s.get("id") == o.get("target_step")))
                .collect();
            Ok((
                json!({"steps":steps,"objections":objections,"unattached_objections":unattached}),
                "Supplied premises, conditional transitions and conclusions with objections addressed to their exact steps; no inference is added.",
            ))
        }
        "dependency_impact" => {
            let edges = array(input, "dependencies")?;
            let start = required(input, "changed")?;
            let mut queue = vec![start.clone()];
            let mut paths = Vec::new();
            let mut i = 0;
            while i < queue.len() {
                let here = queue[i].clone();
                for edge in edges {
                    if edge.get("from") != Some(&here) {
                        continue;
                    }
                    paths.push(edge.clone());
                    if let Some(to) = edge.get("to") {
                        if !queue.contains(to) {
                            queue.push(to.clone());
                        }
                    }
                }
                i += 1;
            }
            Ok((
                json!({"changed":start,"affected":&queue[1..],"dependencies":paths}),
                "Reachable dependants along declared dependency direction; this identifies impact, not automatic invalidation.",
            ))
        }
        "time_place" => {
            let left = required(input, "left")?;
            let right = required(input, "right")?;
            fn middle(v: &Value) -> Option<f64> {
                let span = v.get("span")?;
                Some((span.get("from")?.as_f64()? + span.get("to")?.as_f64()?) / 2.0)
            }
            fn point(v: &Value) -> Option<(f64, f64)> {
                let a = v.get("point")?.as_array()?;
                if a.len() != 2 {
                    return None;
                }
                let x = a[0].as_f64()?;
                let y = a[1].as_f64()?;
                if x.abs() > 180.0 || y.abs() > 90.0 {
                    return None;
                }
                Some((x.to_radians(), y.to_radians()))
            }
            let time = middle(left).zip(middle(right)).map(|(a, b)| (a - b).abs());
            let distance = point(left).zip(point(right)).map(|((l1, f1), (l2, f2))| {
                let h = ((f2 - f1) / 2.0).sin().powi(2)
                    + f1.cos() * f2.cos() * ((l2 - l1) / 2.0).sin().powi(2);
                2.0 * 6371.0088 * h.sqrt().min(1.0).asin()
            });
            Ok((
                json!({"left":left,"right":right,"time_midpoint_distance_years":time,"place_distance_km":distance}),
                "Absolute difference between supplied interval midpoints in years and great-circle distance in kilometres; interval bounds, uncertainty and source stay in the inputs.",
            ))
        }
        _ => Err(format!("Unknown research operation: {operation}")),
    }
}
