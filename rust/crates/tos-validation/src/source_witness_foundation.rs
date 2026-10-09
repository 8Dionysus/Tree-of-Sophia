//! Maintained source-witness foundation mechanics. This result never admits
//! bibliographic truth, textual judgment, rights, canon or publication.
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// One exact content descriptor and every Item-to-File membership. A repeated
/// File does not replace the first descriptor or erase its earlier memberships.
/// This is content relation state, never a filesystem membership certificate.
#[derive(Default)]
pub struct SourceFileMembershipIndex {
    items: BTreeMap<String, BTreeSet<String>>,
    descriptors: BTreeMap<String, [Value; 3]>,
}
impl SourceFileMembershipIndex {
    pub fn add(
        &mut self,
        item_id: &Value,
        file_id: &Value,
        sha256: &Value,
        byte_size: &Value,
        media_type: &Value,
    ) -> Result<Vec<&'static str>, crate::assessment::AssessmentRefusal> {
        let (Some(item), Some(file)) = (item_id.as_str(), file_id.as_str()) else {
            return Ok(Vec::new());
        };
        self.items
            .entry(file.to_owned())
            .or_default()
            .insert(item.to_owned());
        let descriptor = [sha256.clone(), byte_size.clone(), media_type.clone()];
        let mut conflicts = Vec::new();
        if let Some(previous) = self.descriptors.get(file) {
            for ((field, before), after) in ["sha256", "byte_size", "media_type"]
                .into_iter()
                .zip(previous)
                .zip(&descriptor)
            {
                // Maintained Python equality includes numeric 1 == 1.0 == True.
                // Reuse the existing bounded exact-number equality kernel.
                if !crate::assessment::py_equal(before, after)? {
                    conflicts.push(field);
                }
            }
        } else {
            self.descriptors.insert(file.to_owned(), descriptor);
        }
        if sha256
            .as_str()
            .is_some_and(|sha| file != format!("tos.file.sha256.{sha}"))
        {
            conflicts.push("file_id_sha256");
        }
        Ok(conflicts)
    }
    pub fn contains(&self, item_id: &Value, file_id: &Value) -> bool {
        match (item_id.as_str(), file_id.as_str()) {
            (Some(item), Some(file)) => self
                .items
                .get(file)
                .is_some_and(|items| items.contains(item)),
            _ => false,
        }
    }
    pub fn sha256_for(&self, file_id: &Value) -> Option<&Value> {
        file_id
            .as_str()
            .and_then(|file| self.descriptors.get(file))
            .map(|descriptor| &descriptor[0])
    }
}

/// Exact maintained real sentence selector/digest bridge. Source text remains
/// outside this metadata comparison; success grants no textual acceptance.
pub fn opening_sentence_plan_binding_issues(
    plan: &Value,
    source_packet: &Value,
    target_packet: &Value,
    alignment: &Value,
) -> Result<Vec<String>, crate::assessment::AssessmentRefusal> {
    use serde_json::json;
    let mut messages = Vec::new();
    for (label, packet) in [("source", source_packet), ("target", target_packet)] {
        let side_plan = &plan[label];
        let anchor_ref = &plan["opaque_ids"][format!("{label}_sentence_anchor_id")];
        let expected_selector = json!({"type":"text_position", "start":side_plan["sentence_start"],
            "end":side_plan["sentence_end"], "position_unit":"unicode_code_point", "interval":"half_open"});
        let packet_anchor = packet["anchors"].as_array().and_then(|rows| {
            rows.iter()
                .rev()
                .find(|row| row.is_object() && &row["anchor_ref"] == anchor_ref)
        });
        let alignment_rows = alignment[format!("{label}_side")]["anchors"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter(|row| row.is_object())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let alignment_anchor = if alignment_rows.len() == 1 {
            Some(alignment_rows[0])
        } else {
            None
        };
        for (owner, anchor) in [
            ("sentence-unit packet", packet_anchor),
            ("alignment side", alignment_anchor),
        ] {
            let Some(anchor) = anchor else {
                messages.push(format!("{label} {owner} sentence anchor is absent"));
                continue;
            };
            for (actual, expected, message) in [
                (
                    &anchor["anchor_ref"],
                    anchor_ref,
                    "sentence anchor identity drifted",
                ),
                (
                    &anchor["selector"],
                    &expected_selector,
                    "sentence selector drifted from plan",
                ),
                (
                    &anchor["exact_sha256"],
                    &side_plan["sentence_sha256"],
                    "sentence digest drifted from plan",
                ),
                (
                    &anchor["text_layer_ref"],
                    &side_plan["text_layer_ref"],
                    "text-layer ref drifted from plan",
                ),
                (
                    &anchor["text_layer_sha256"],
                    &side_plan["text_layer_sha256"],
                    "text-layer digest drifted from plan",
                ),
                (
                    &anchor["source_return"]["locator_ref"],
                    &side_plan["private_content_ref"],
                    "source-return locator drifted from plan",
                ),
            ] {
                if !crate::assessment::py_equal(actual, expected)? {
                    messages.push(format!("{label} {owner} {message}"));
                }
            }
        }
    }
    Ok(messages)
}

pub fn provision_temporal_issues(activity: &Value) -> Vec<&'static str> {
    if activity["temporal"]["kind"] != "interval" {
        return Vec::new();
    }
    match (
        activity["temporal"]["start"].as_str(),
        activity["temporal"]["end"].as_str(),
    ) {
        (Some(start), Some(end)) if start > end => {
            vec!["provision-activity interval starts after it ends"]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SourceFileMembershipIndex, opening_sentence_plan_binding_issues, provision_temporal_issues,
    };
    use serde_json::{Value, json};

    #[test]
    fn source_file_membership_retains_all_members_and_first_descriptor() {
        let mut index = SourceFileMembershipIndex::default();
        let item_a = json!("tos.item.a");
        let item_b = json!("tos.item.b");
        let item_c = json!("tos.item.c");
        let sha = json!("a".repeat(64));
        let file = json!(format!("tos.file.sha256.{}", sha.as_str().unwrap()));
        let media = json!("text/plain");
        let original_size = json!(1);

        assert!(index
            .add(&item_a, &file, &sha, &original_size, &media)
            .unwrap()
            .is_empty());
        // Maintained Python equality treats an integral JSON float and integer
        // as the same descriptor value.
        assert!(index
            .add(&item_b, &file, &sha, &json!(1.0), &media)
            .unwrap()
            .is_empty());
        assert!(index.contains(&item_a, &file));
        assert!(index.contains(&item_b, &file));
        assert!(!index.contains(&item_c, &file));
        assert_eq!(Some(&sha), index.sha256_for(&file));

        let conflicting_sha = json!("b".repeat(64));
        let conflicts = index
            .add(
                &item_c,
                &file,
                &conflicting_sha,
                &json!(2),
                &json!("application/json"),
            )
            .unwrap();
        assert_eq!(
            vec!["sha256", "byte_size", "media_type", "file_id_sha256"],
            conflicts
        );
        // Conflicting evidence still records the observed relation, but cannot
        // replace the first descriptor used by downstream checks.
        assert!(index.contains(&item_c, &file));
        assert_eq!(Some(&sha), index.sha256_for(&file));
    }

    fn anchor(
        anchor_ref: &str,
        start: u64,
        end: u64,
        sentence_sha256: &str,
        text_layer_ref: &str,
        text_layer_sha256: &str,
        locator_ref: &str,
    ) -> Value {
        json!({
            "anchor_ref": anchor_ref,
            "selector": {
                "type": "text_position",
                "start": start,
                "end": end,
                "position_unit": "unicode_code_point",
                "interval": "half_open"
            },
            "exact_sha256": sentence_sha256,
            "text_layer_ref": text_layer_ref,
            "text_layer_sha256": text_layer_sha256,
            "source_return": {"locator_ref": locator_ref}
        })
    }

    #[test]
    fn opening_sentence_plan_binding_rejects_both_sides_and_owner_views_drift() {
        let plan = json!({
            "source": {
                "sentence_start": 4,
                "sentence_end": 10,
                "sentence_sha256": "source-sentence-sha",
                "text_layer_ref": "source-layer",
                "text_layer_sha256": "source-layer-sha",
                "private_content_ref": "source-locator"
            },
            "target": {
                "sentence_start": 8,
                "sentence_end": 15,
                "sentence_sha256": "target-sentence-sha",
                "text_layer_ref": "target-layer",
                "text_layer_sha256": "target-layer-sha",
                "private_content_ref": "target-locator"
            },
            "opaque_ids": {
                "source_sentence_anchor_id": "source-anchor",
                "target_sentence_anchor_id": "target-anchor"
            }
        });
        let source_anchor = anchor(
            "source-anchor",
            4,
            10,
            "source-sentence-sha",
            "source-layer",
            "source-layer-sha",
            "source-locator",
        );
        let target_anchor = anchor(
            "target-anchor",
            8,
            15,
            "target-sentence-sha",
            "target-layer",
            "target-layer-sha",
            "target-locator",
        );
        let source_packet = json!({"anchors": [source_anchor.clone()]});
        let target_packet = json!({"anchors": [target_anchor.clone()]});
        let alignment = json!({
            "source_side": {"anchors": [source_anchor]},
            "target_side": {"anchors": [target_anchor]}
        });

        assert!(opening_sentence_plan_binding_issues(
            &plan,
            &source_packet,
            &target_packet,
            &alignment
        )
        .unwrap()
        .is_empty());

        let fields = [
            "anchor_ref",
            "selector",
            "exact_sha256",
            "text_layer_ref",
            "text_layer_sha256",
            "locator_ref",
        ];
        for label in ["source", "target"] {
            for owner in ["packet", "alignment"] {
                for field in fields {
                    let mut source_packet = source_packet.clone();
                    let mut target_packet = target_packet.clone();
                    let mut alignment = alignment.clone();
                    let selected_anchor = if owner == "packet" {
                        let packet = if label == "source" {
                            &mut source_packet
                        } else {
                            &mut target_packet
                        };
                        &mut packet["anchors"][0]
                    } else {
                        let side = format!("{label}_side");
                        &mut alignment[side.as_str()]["anchors"][0]
                    };
                    match field {
                        "anchor_ref" => selected_anchor["anchor_ref"] = json!("drifted-anchor"),
                        "selector" => selected_anchor["selector"]["start"] = json!(0),
                        "exact_sha256" => {
                            selected_anchor["exact_sha256"] = json!("drifted-digest")
                        }
                        "text_layer_ref" => {
                            selected_anchor["text_layer_ref"] = json!("drifted-layer")
                        }
                        "text_layer_sha256" => {
                            selected_anchor["text_layer_sha256"] = json!("drifted-layer-digest")
                        }
                        "locator_ref" => {
                            selected_anchor["source_return"]["locator_ref"] =
                                json!("drifted-locator")
                        }
                        _ => unreachable!(),
                    }
                    let issues = opening_sentence_plan_binding_issues(
                        &plan,
                        &source_packet,
                        &target_packet,
                        &alignment,
                    )
                    .unwrap();
                    let owner_name = if owner == "packet" {
                        "sentence-unit packet"
                    } else {
                        "alignment side"
                    };
                    let message = match (owner, field) {
                        ("packet", "anchor_ref") => "sentence anchor is absent",
                        (_, "anchor_ref") => "sentence anchor identity drifted",
                        (_, "selector") => "sentence selector drifted from plan",
                        (_, "exact_sha256") => "sentence digest drifted from plan",
                        (_, "text_layer_ref") => "text-layer ref drifted from plan",
                        (_, "text_layer_sha256") => "text-layer digest drifted from plan",
                        (_, "locator_ref") => "source-return locator drifted from plan",
                        _ => unreachable!(),
                    };
                    assert_eq!(
                        vec![format!("{label} {owner_name} {message}")],
                        issues,
                        "{label} {owner} {field}"
                    );
                }
            }
        }
    }

    #[test]
    fn provision_temporal_interval_rejects_reversed_year_bounds_only() {
        let reversed = json!({
            "temporal": {"kind": "interval", "start": "1909", "end": "1908"}
        });
        assert_eq!(
            vec!["provision-activity interval starts after it ends"],
            provision_temporal_issues(&reversed)
        );
        assert!(provision_temporal_issues(&json!({
            "temporal": {"kind": "interval", "start": "1908", "end": "1909"}
        }))
        .is_empty());
        assert!(provision_temporal_issues(&json!({
            "temporal": {"kind": "date", "value": "1909"}
        }))
        .is_empty());
    }
}
