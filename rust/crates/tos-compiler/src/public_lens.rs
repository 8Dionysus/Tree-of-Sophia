use crate::{Error, Result};
use tos_foundation::JsonValue;

pub(crate) fn compact_seed(item: &JsonValue) -> Result<JsonValue> {
    let JsonValue::Object(fields) = item else {
        return Err(Error::Invalid("public compact lens row"));
    };
    let attributes = item
        .object_get("attributes")
        .and_then(JsonValue::as_object)
        .ok_or(Error::Invalid("public compact lens attributes"))?;
    let keep_forms = attributes
        .iter()
        .any(|(key, _)| key.as_str() == Some("human_forms"));
    let mut out = Vec::with_capacity(fields.len());
    for (key, value) in fields {
        match key.as_str() {
            Some("source_record" | "readable_context") => continue,
            Some("attributes") if !keep_forms => {
                out.push((key.clone(), JsonValue::Object(Vec::new())))
            }
            Some("semantics") => {
                let mut semantics = value.clone();
                if let JsonValue::Object(ref mut sem) = semantics {
                    for (name, claim) in sem {
                        if name.as_str() == Some("claim") {
                            if let JsonValue::Object(members) = claim {
                                members.retain(|(name, _)| {
                                    name.as_str() != Some("source_canonical_json")
                                });
                            }
                        }
                    }
                }
                out.push((key.clone(), semantics));
            }
            _ => out.push((key.clone(), value.clone())),
        }
    }
    Ok(JsonValue::Object(out))
}
