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
