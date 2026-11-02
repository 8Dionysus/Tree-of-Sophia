//! Curation-selected alignment of numbered candidates and reviewed page returns.
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use tos_source_store::execution::Execution;

const EXISTING_CANDIDATE_LIMIT: usize = 4096;

#[derive(Clone, Debug)]
pub struct Candidate {
    pub page: u64,
    pub token: String,
    pub raw: String,
}

pub struct Matches {
    pub pages: BTreeMap<String, u64>,
    pub raw: BTreeMap<String, String>,
    pub skipped: Vec<String>,
}

fn array<'a>(value: &'a Value, name: &str) -> Result<&'a Vec<Value>, String> {
    value
        .as_array()
        .ok_or_else(|| format!("{name} must be an array"))
}
fn string<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("{name} must be a string"))
}
fn unsigned(value: &Value, name: &str) -> Result<u64, String> {
    value
        .as_u64()
        .ok_or_else(|| format!("{name} must be an unsigned integer"))
}
fn float(value: &Value, name: &str) -> Result<f64, String> {
    value
        .as_f64()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .ok_or_else(|| format!("{name} must be a nonnegative finite number"))
}

fn distance(expected: &str, observed: &str, substitutions: &[(char, String)], cost: f64) -> f64 {
    let observed = observed.chars().collect::<Vec<_>>();
    let mut previous = (0..=observed.len()).map(|n| n as f64).collect::<Vec<_>>();
    for (row, a) in expected.chars().enumerate() {
        let mut current = vec![(row + 1) as f64];
        for (col, b) in observed.iter().enumerate() {
            let substitution = if a == *b {
                0.0
            } else if substitutions
                .iter()
                .any(|(digit, values)| *digit == a && values.contains(*b))
            {
                cost
            } else {
                1.0
            };
            current.push(
                (previous[col + 1] + 1.0)
                    .min(current[col] + 1.0)
                    .min(previous[col] + substitution),
            );
        }
        previous = current;
    }
    previous[observed.len()]
}

/// Align an ordered candidate stream using the selected weighted profile.
pub fn weighted(
    ctx: &Execution,
    recipe: &Value,
    keys: &[String],
    candidates: &[Candidate],
) -> Result<Matches, String> {
    if candidates.len() > EXISTING_CANDIDATE_LIMIT
        || keys.len() as u64
            != unsigned(&recipe["unit_sequence"]["expected_count"], "expected count")?
    {
        return Err("number alignment dimensions".into());
    }
    let profile = &recipe["candidate_profile"];
    if profile["kind"] != "abbyy"
        || profile["alignment"] != "weighted-ordered"
        || profile["visit_order"] != "row-major"
    {
        return Err("weighted alignment profile required".into());
    }
    let replacement = profile["replace_on_equal"]
        .as_bool()
        .ok_or("tie rule required")?;
    let skip_key = float(&profile["skipped_key_cost"], "skipped key cost")?;
    let skip_candidate = float(&profile["skipped_candidate_cost"], "skipped candidate cost")?;
    let substitution_cost = float(&profile["substitution_cost"], "substitution cost")?;
    let maximum_cost = float(&profile["maximum_match_cost"], "maximum match cost")?;
    let suffix = string(&recipe["unit_sequence"]["repeat_suffix"], "repeat suffix")?;
    let mut substitutions = Vec::new();
    for rule in array(&recipe["ocr_substitutions"], "OCR substitutions")? {
        let digit = string(&rule["digit"], "substitution digit")?;
        let mut chars = digit.chars();
        let digit = chars.next().ok_or("empty substitution digit")?;
        if chars.next().is_some() {
            return Err("substitution digit must be one character".into());
        }
        substitutions.push((
            digit,
            string(&rule["allowed_characters"], "substitution characters")?.to_owned(),
        ));
    }
    let order = array(&profile["transition_order"], "transition order")?
        .iter()
        .map(|v| string(v, "transition"))
        .collect::<Result<Vec<_>, _>>()?;
    if order.len() != 3
        || order.iter().copied().collect::<BTreeSet<_>>()
            != BTreeSet::from(["skip_key", "skip_candidate", "match"])
    {
        return Err("invalid transition order".into());
    }
    let width = candidates
        .len()
        .checked_add(1)
        .ok_or("alignment dimensions overflow")?;
    let len = keys
        .len()
        .checked_add(1)
        .and_then(|n| n.checked_mul(width))
        .ok_or("alignment allocation overflow")?;
    let budget = ctx.budget_report();
    let max_work = unsigned(&budget["work_units_max"], "work limit")?;
    let used = unsigned(&budget["charged_work_units"], "charged work")?;
    if u64::try_from(len).map_err(|_| "alignment work overflow")? > max_work.saturating_sub(used) {
        return Err("alignment exceeds remaining work".into());
    }
    let mut scores = Vec::new();
    scores
        .try_reserve_exact(len)
        .map_err(|_| "alignment score allocation failed")?;
    scores.resize(len, f64::INFINITY);
    let mut route = Vec::new();
    route
        .try_reserve_exact(len)
        .map_err(|_| "alignment route allocation failed")?;
    route.resize(len, 0u8);
    scores[0] = 0.0;
    for i in 0..=keys.len() {
        for j in 0..=candidates.len() {
            ctx.tick(1)?;
            let at = i * width + j;
            let score = scores[at];
            if !score.is_finite() {
                continue;
            }
            for action in &order {
                let next = match *action {
                    "skip_key" if i < keys.len() => Some((at + width, score + skip_key, 1)),
                    "skip_candidate" if j < candidates.len() => {
                        Some((at + 1, score + skip_candidate, 2))
                    }
                    "match" if i < keys.len() && j < candidates.len() => {
                        let cost = distance(
                            keys[i].trim_end_matches(suffix),
                            &candidates[j].token,
                            &substitutions,
                            substitution_cost,
                        );
                        (cost <= maximum_cost).then_some((at + width + 1, score + cost, 3))
                    }
                    _ => None,
                };
                if let Some((to, proposed, mark)) = next {
                    if proposed < scores[to] || (replacement && proposed == scores[to]) {
                        scores[to] = proposed;
                        route[to] = mark;
                    }
                }
            }
        }
    }
    let (mut i, mut j) = (keys.len(), candidates.len());
    let mut actions = Vec::new();
    while i > 0 || j > 0 {
        ctx.tick(1)?;
        match route[i * width + j] {
            1 => {
                i -= 1;
                actions.push((false, i, j));
            }
            2 => j -= 1,
            3 => {
                i -= 1;
                j -= 1;
                actions.push((true, i, j));
            }
            _ => return Err("ordered candidate route incomplete".into()),
        }
    }
    actions.reverse();
    let mut pages = BTreeMap::new();
    let mut raw = BTreeMap::new();
    let mut skipped = Vec::new();
    for (matched, i, j) in actions {
        if matched {
            pages.insert(keys[i].clone(), candidates[j].page);
            raw.insert(keys[i].clone(), candidates[j].raw.clone());
        } else {
            skipped.push(keys[i].clone());
        }
    }
    Ok(Matches {
        pages,
        raw,
        skipped,
    })
}

/// Normalize a target token using the selected coordinate-navigation profile.
pub fn normalize_target(recipe: &Value, raw: &str) -> Result<Option<String>, String> {
    let profile = &recipe["candidate_profile"];
    if profile["kind"] != "pdf-bbox" {
        return Err("PDF candidate profile required".into());
    }
    let mut input = raw.trim();
    if profile["trim_surrounding_periods"]
        .as_bool()
        .ok_or("period rule required")?
    {
        input = input.trim_matches('.').trim();
    }
    let mut replacements = BTreeMap::new();
    for rule in array(&profile["normalization"], "normalization")? {
        let from = string(&rule["from"], "normalization source")?;
        let mut chars = from.chars();
        let from = chars.next().ok_or("empty normalization source")?;
        if chars.next().is_some()
            || replacements
                .insert(from, string(&rule["to"], "normalization target")?)
                .is_some()
        {
            return Err("invalid normalization source".into());
        }
    }
    let mut key = String::new();
    for c in input.chars() {
        if let Some(v) = replacements.get(&c) {
            key.push_str(v);
        } else {
            key.push(c);
        }
    }
    let suffix = string(&recipe["unit_sequence"]["repeat_suffix"], "repeat suffix")?;
    let number = key.strip_suffix(suffix).unwrap_or(&key);
    let allow_zero = profile["allow_leading_zero"]
        .as_bool()
        .ok_or("leading zero rule required")?;
    if number.is_empty()
        || (!allow_zero && number.starts_with('0'))
        || !number.bytes().all(|b| b.is_ascii_digit())
    {
        return Ok(None);
    }
    let parsed = match number.parse::<u64>() {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let first = unsigned(&profile["minimum_number"], "minimum number")?;
    let last = unsigned(&profile["maximum_number"], "maximum number")?;
    Ok((first <= parsed && parsed <= last).then_some(key))
}

/// Apply monotonic first-match alignment to an already selected candidate stream.
pub fn monotonic(keys: &[String], candidates: &[Candidate]) -> Result<Matches, String> {
    if candidates.len() > EXISTING_CANDIDATE_LIMIT {
        return Err("candidate count bound".into());
    }
    let positions = keys
        .iter()
        .enumerate()
        .map(|(i, key)| (key.as_str(), i))
        .collect::<BTreeMap<_, _>>();
    if positions.len() != keys.len() {
        return Err("duplicate numbered key".into());
    }
    let (mut next, mut pages, mut raw) = (0, BTreeMap::new(), BTreeMap::new());
    for candidate in candidates {
        if let Some(&index) = positions.get(candidate.token.as_str()) {
            if index < next {
                continue;
            }
            pages.insert(candidate.token.clone(), candidate.page);
            raw.insert(candidate.token.clone(), candidate.raw.clone());
            next = index + 1;
        }
    }
    let skipped = keys
        .iter()
        .filter(|key| !pages.contains_key(*key))
        .cloned()
        .collect();
    Ok(Matches {
        pages,
        raw,
        skipped,
    })
}

/// Require exact retained gap coverage before applying reviewed page returns.
pub fn apply_overrides(
    recipe: &Value,
    keys: &[String],
    mut matched: Matches,
) -> Result<Matches, String> {
    let mut overrides = BTreeMap::new();
    for row in array(&recipe["page_overrides"], "page overrides")? {
        let key = string(&row["key"], "override key")?.to_owned();
        let page = unsigned(&row["page"], "override page")?;
        if overrides.insert(key, page).is_some() {
            return Err("duplicate page override".into());
        }
    }
    if matched.skipped.iter().cloned().collect::<BTreeSet<_>>()
        != overrides.keys().cloned().collect()
    {
        return Err("candidate gaps differ from reviewed overrides".into());
    }
    for (key, page) in overrides {
        matched.pages.insert(key, page);
    }
    if matched.pages.len() != keys.len() {
        return Err("numbered page coverage differs".into());
    }
    let mut previous = 0;
    for key in keys {
        let page = *matched.pages.get(key).ok_or("numbered page missing")?;
        if recipe["page_order"] == "nondecreasing" && page < previous {
            return Err("numbered pages are not ordered".into());
        }
        previous = page;
    }
    Ok(matched)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn weighted_alignment_retains_repeated_keys_and_requires_reviewed_gaps() {
        let root = std::env::current_dir().unwrap();
        let ctx = Execution::new(&root, 30).unwrap();
        let recipe = json!({"unit_sequence":{"expected_count":4,"repeat_suffix":"a"},"candidate_profile":{"kind":"abbyy","alignment":"weighted-ordered","visit_order":"row-major","replace_on_equal":false,"skipped_key_cost":0.92,"skipped_candidate_cost":0.04,"substitution_cost":0.22,"maximum_match_cost":1.55,"transition_order":["skip_key","skip_candidate","match"]},"ocr_substitutions":[{"digit":"1","allowed_characters":"1il"},{"digit":"8","allowed_characters":"8s"}],"page_overrides":[{"key":"66","page":11}],"page_order":"nondecreasing"});
        let keys = ["65", "65a", "66", "67"].map(str::to_owned);
        let candidates =
            [(10, "65"), (11, "65"), (11, "noise"), (12, "67")].map(|(page, token)| Candidate {
                page,
                token: token.into(),
                raw: token.into(),
            });
        let matches = weighted(&ctx, &recipe, &keys, &candidates).unwrap();
        assert_eq!(matches.skipped, ["66"]);
        assert_eq!(
            matches.pages,
            BTreeMap::from([("65".into(), 10), ("65a".into(), 11), ("67".into(), 12)])
        );
        let matches = apply_overrides(&recipe, &keys, matches).unwrap();
        assert_eq!(matches.pages["66"], 11);
        assert!(
            (distance(
                "188",
                "i8s",
                &[('1', "1il".into()), ('8', "8s".into())],
                0.22
            ) - 0.44)
                .abs()
                < 1e-9
        );
    }
    #[test]
    fn weighted_alignment_respects_the_selected_tie_policy() {
        let root = std::env::current_dir().unwrap();
        let ctx = Execution::new(&root, 30).unwrap();
        let mut recipe = json!({"unit_sequence":{"expected_count":1,"repeat_suffix":"a"},"candidate_profile":{"kind":"abbyy","alignment":"weighted-ordered","visit_order":"row-major","replace_on_equal":false,"skipped_key_cost":0.5,"skipped_candidate_cost":0.5,"substitution_cost":0.22,"maximum_match_cost":1.0,"transition_order":["skip_key","skip_candidate","match"]},"ocr_substitutions":[]});
        let keys = ["1".to_owned()];
        let candidates = [Candidate {
            page: 1,
            token: "2".into(),
            raw: "2".into(),
        }];
        assert!(
            weighted(&ctx, &recipe, &keys, &candidates)
                .unwrap()
                .skipped
                .is_empty()
        );
        recipe["candidate_profile"]["replace_on_equal"] = json!(true);
        assert_eq!(
            weighted(&ctx, &recipe, &keys, &candidates).unwrap().skipped,
            ["1"]
        );
    }
    #[test]
    fn target_normalization_and_monotonic_matching_follow_selected_rules() {
        let recipe = json!({"unit_sequence":{"repeat_suffix":"a"},"candidate_profile":{"kind":"pdf-bbox","trim_surrounding_periods":true,"allow_leading_zero":false,"minimum_number":1,"maximum_number":99,"normalization":[{"from":"б","to":"6"},{"from":"а","to":"a"}]},"page_overrides":[{"key":"7","page":11}],"page_order":"nondecreasing"});
        assert_eq!(
            normalize_target(&recipe, " .б. ").unwrap().as_deref(),
            Some("6")
        );
        assert_eq!(normalize_target(&recipe, "06").unwrap(), None);
        let keys = ["6", "6a", "7", "8"].map(str::to_owned);
        let candidates = [(10, "6"), (10, "6"), (11, "6а"), (12, "8")]
            .into_iter()
            .map(|(page, raw)| Candidate {
                page,
                token: normalize_target(&recipe, raw).unwrap().unwrap(),
                raw: raw.into(),
            })
            .collect::<Vec<_>>();
        let matches = monotonic(&keys, &candidates).unwrap();
        assert_eq!(matches.skipped, ["7"]);
        assert_eq!(matches.raw["6a"], "6а");
        assert_eq!(
            apply_overrides(&recipe, &keys, matches).unwrap().pages["7"],
            11
        );
    }
}
