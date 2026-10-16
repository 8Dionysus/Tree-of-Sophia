use super::*;
use serde_json::{Value, json};
use std::time::Duration;
fn xhtml(body: &str) -> Vec<u8> {
    format!("<html xmlns=\"{XHTML}\"><head/><body>{body}</body></html>").into_bytes()
}
fn selector(value: &str, scheme: &str) -> JsonValue {
    cmd::parse(
        &serde_json::to_vec(&json!({"type":"structural","scheme":scheme,"value":value})).unwrap(),
    )
    .unwrap()
}
fn extract(raw: &[u8], value: &str, scheme: &str) -> SourceCommandResult<String> {
    let selected = selector(value, scheme);
    let policy = cmd::parse(POLICY.as_bytes()).unwrap();
    extract_xhtml_text(
        raw,
        validate_extraction_profile(&selected, &policy)?,
        Instant::now() + Duration::from_secs(10),
        &AtomicBool::new(false),
    )
}
fn ordinal(raw: &[u8]) -> SourceCommandResult<String> {
    extract(raw, "p:1", "tos.xhtml.element-ordinal.v1")
}
#[test]
fn exact_unicode_selection_declarations_and_markup_boundaries() {
    assert_eq!(
        ordinal(&xhtml(
            "<p> \n cafe\u{301} <em>word</em>\t<br/>next \n</p>ROOT-TAIL<p>other</p>"
        ))
        .unwrap(),
        " \n cafe\u{301} word\t\nnext \n"
    );
    let raw = xhtml("<section><p>one</p></section><p xmlns=\"urn:foreign\">foreign</p><p>two</p>");
    assert_eq!(
        extract(&raw, "p:2", "tos.xhtml.element-ordinal.v1").unwrap(),
        "two"
    );
    assert!(extract(&raw, "p:3", "tos.xhtml.element-ordinal.v1").is_err());
    for (attribute, text) in [("id", "alpha"), ("xml:id", "beta")] {
        assert_eq!(
            extract(
                &xhtml(&format!("<p {attribute}=\"selected\">{text}</p>")),
                "selected",
                "tos.xhtml.element-id.v1"
            )
            .unwrap(),
            text
        );
    }
    for body in [
        "<p>none</p>",
        "<p id=\"selected\">one</p><p xml:id=\"selected\">two</p>",
    ] {
        assert!(extract(&xhtml(body), "selected", "tos.xhtml.element-id.v1").is_err());
    }
    let mut raw = b"\xef\xbb\xbf<?xml version='1.0' encoding='UTF-8'?>".to_vec();
    raw.extend(xhtml("<p>A<!--omitted-->B</p>"));
    assert_eq!(ordinal(&raw).unwrap(), "AB");
    let room = MAX_MARKUP_TOKEN_BYTES - "<p data=\"\">".len();
    let raw = xhtml(&format!("<p data=\"{}\">A</p>", "=".repeat(room)));
    assert_eq!(ordinal(&raw).unwrap(), "A");
    let attributes = (0..MAX_ATTRIBUTES)
        .map(|n| format!("a{n}='> = \"'"))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(
        ordinal(&xhtml(&format!("<p {attributes}>B</p>"))).unwrap(),
        "B"
    );
    let fake = format!(
        "<fake {}>",
        (0..MAX_ATTRIBUTES + 1)
            .map(|n| format!("a{n}=\"x\""))
            .collect::<Vec<_>>()
            .join(" ")
    );
    assert_eq!(
        ordinal(&xhtml(&format!("<p>A<!--{fake}-->B</p>"))).unwrap(),
        "AB"
    );
    assert_eq!(
        ordinal(&xhtml(&format!("<p>A<![CDATA[{fake}]]>B</p>"))).unwrap(),
        format!("A{fake}B")
    );
    let text = "x".repeat(MAX_MARKUP_TOKEN_BYTES + 1);
    assert_eq!(
        ordinal(&xhtml(&format!("<p><![CDATA[{text}]]></p>"))).unwrap(),
        text
    );
}
#[test]
fn parser_refuses_repair_unsupported_markup_and_unbounded_inputs_without_disclosure() {
    for body in [
        "<p>A\rB</p>",
        "<p>A\r\nB</p>",
        "<p>A&amp;B</p>",
        "<p>A&#32;B</p>",
        "<p>A&#x20;B</p>",
        "<p>A&nbsp;B</p>",
        "<p>A<script>PRIVATE-CANARY</script></p>",
        "<p>A<style>PRIVATE-CANARY</style></p>",
        "<p>A<img alt=\"private\"/></p>",
        "<p><div>block</div></p>",
        "<p><em xmlns=\"urn:foreign\">B</em></p>",
        "<p/>",
        "<p><br>bad</br></p>",
    ] {
        let error = ordinal(&xhtml(body)).unwrap_err();
        assert!(!format!("{error:?}").contains("PRIVATE-CANARY"));
    }
    for prefix in [
        "<!DOCTYPE html SYSTEM \"file:///PRIVATE-CANARY\">",
        "<!DOCTYPE html [<!ENTITY x SYSTEM \"https://invalid.test\">]>",
        "<?xml-stylesheet href=\"https://invalid.test\"?>",
        "<?other private?>",
        "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>",
        "<?xml version=\"1.1\"?>",
    ] {
        let mut raw = prefix.as_bytes().to_vec();
        raw.extend(xhtml("<p>A</p>"));
        assert!(ordinal(&raw).is_err());
    }
    assert!(ordinal(b"<html xmlns='urn:wrong'><p>A</p></html>").is_err());
    assert!(ordinal(b"<html xmlns='http://www.w3.org/1999/xhtml'><p>unfinished").is_err());
    let mut bad = xhtml("<p>A</p>");
    bad[0] = 255;
    assert!(ordinal(&bad).is_err());
    for value in [
        ">".repeat(MAX_MARKUP_TOKEN_BYTES),
        "é".repeat(MAX_MARKUP_TOKEN_BYTES / 2),
    ] {
        let raw = xhtml(&format!("<p data=\"{value}\">A</p>"));
        assert!(
            markup_preflight(
                &raw,
                Instant::now() + Duration::from_secs(2),
                &AtomicBool::new(false)
            )
            .is_err()
        );
    }
    for prefix in ["a", "xmlns:a"] {
        let attrs = (0..MAX_ATTRIBUTES + 1)
            .map(|n| format!("{prefix}{n}=\"urn:test\""))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            markup_preflight(
                &xhtml(&format!("<p {attrs}>A</p>")),
                Instant::now() + Duration::from_secs(2),
                &AtomicBool::new(false)
            )
            .is_err()
        );
    }
    for body in [
        format!("<p><!--{}-->A</p>", "x".repeat(MAX_MARKUP_TOKEN_BYTES + 1)),
        "<p><!--unfinished".into(),
        "<p><![CDATA[unfinished".into(),
    ] {
        assert!(ordinal(&xhtml(&body)).is_err());
    }
    assert!(ordinal(&vec![b' '; MAX_MEMBER_BYTES + 1]).is_err());
    assert!(
        ordinal(&xhtml(&format!(
            "<p>{}A{}</p>",
            "<em>".repeat(MAX_DEPTH),
            "</em>".repeat(MAX_DEPTH)
        )))
        .is_err()
    );
    assert!(
        ordinal(&xhtml(&format!(
            "<p>A</p>{}",
            "<p>B</p>".repeat(MAX_ELEMENTS)
        )))
        .is_err()
    );
    let raw = xhtml("<p>A</p>");
    assert!(
        extract_xhtml_text(
            &raw,
            XhtmlSelector::Ordinal {
                local: "p",
                ordinal: 1
            },
            Instant::now(),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(
        extract_xhtml_text(
            &raw,
            XhtmlSelector::Ordinal {
                local: "p",
                ordinal: 1
            },
            Instant::now() + Duration::from_secs(1),
            &AtomicBool::new(true)
        )
        .is_err()
    );
}
#[test]
fn extraction_profile_is_exact_and_never_guesses_selectors_or_policy() {
    let policy = cmd::parse(POLICY.as_bytes()).unwrap();
    for value in ["p:0", "p:-1", "p:01", "p:999999", "p:1[evil]", "body:1"] {
        assert!(
            validate_extraction_profile(&selector(value, "tos.xhtml.element-ordinal.v1"), &policy)
                .is_err()
        );
    }
    for (value, scheme) in [
        ("//p", "xpath"),
        ("p", "css"),
        (" a ", "tos.xhtml.element-id.v1"),
    ] {
        assert!(validate_extraction_profile(&selector(value, scheme), &policy).is_err());
    }
    let extra=cmd::parse(br#"{"type":"structural","scheme":"tos.xhtml.element-ordinal.v1","value":"p:1","extra":true}"#).unwrap();
    assert!(validate_extraction_profile(&extra, &policy).is_err());
    let mut changed: Value = serde_json::from_str(POLICY).unwrap();
    changed["unicode_normalization"] = json!("NFC");
    let changed = cmd::parse(&serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(
        validate_extraction_profile(&selector("p:1", "tos.xhtml.element-ordinal.v1"), &changed)
            .is_err()
    );
}
