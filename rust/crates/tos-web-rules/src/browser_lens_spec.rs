//! Fixed browser-owned lens recipes. Opaque identity/depth slots stay in the
//! host and are never serialized, normalized or assigned source authority.
use wasm_bindgen::prelude::*;
const FOCUS: &str = r#"{"schema_version":"tos_lens_spec_v1","lens_id":"sophia-observatory-focus","language":"ru","detail":"compact","explain":true,"seed":{"focus_node_id":null},"node_query":{"enabled":false},"traversal":{"depth":1,"direction":"either","profile":"overview"},"limits":{"nodes":40,"relations":80,"groups":8}}"#;
#[wasm_bindgen]
pub fn focus_spec_descriptor_wasm_v1() -> String {
    FOCUS.into()
}
#[wasm_bindgen]
pub fn relation_spec_descriptor_wasm_v1() -> String {
    let focus = FOCUS.replace("\"depth\":1", "\"depth\":0");
    format!(
        r#"{{"focus":{focus},"node_query":{{"enabled":true,"filters":[{{"field":"id","op":"in","value":[null,null]}}]}},"profile":"all","relation_query":{{"filters":[{{"field":"id","op":"eq","value":null}}]}},"limits":{{"nodes":2,"relations":1,"groups":2}}}}"#
    )
}
