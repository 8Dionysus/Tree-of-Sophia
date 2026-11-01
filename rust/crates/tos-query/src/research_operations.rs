//! Small, reproducible research operators; labels and evidence remain supplied by their owner.
use serde_json::{Value, json};

pub fn execute(request: &Value) -> Result<Value, String> {
    let operation = request
        .get("operation")
        .and_then(Value::as_str)
        .ok_or("operation required")?;
    let input = request.get("input").ok_or("input required")?;
    if operation == "signed_support"
        && request.get("scope") != input.get("claim").and_then(|c| c.get("scope"))
    {
        return Err("request scope must match claim scope".into());
    }
    let result = match operation {
        "signed_support" => signed_support(input)?,
        "reading_distance" | "commitment_order" => readings(operation, input)?,
        _ => return Err("unknown research operation".into()),
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
