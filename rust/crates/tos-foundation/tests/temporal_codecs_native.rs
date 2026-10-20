use tos_foundation::temporal::{TemporalBudget, codecs::capture_tei};
use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json};
fn parse(s: &str) -> JsonValue {
    parse_json(
        s.as_bytes(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .root()
    .clone()
}
fn f<'a>(v: &'a JsonValue, k: &str) -> &'a JsonValue {
    v.object_get(k).unwrap_or(&JsonValue::Null)
}
fn s<'a>(v: &'a JsonValue, k: &str) -> Option<&'a str> {
    f(v, k).as_str()
}
fn input(xml: &str) -> JsonValue {
    let mut v = parse(
        r#"{"tei_version":"P5-4.12.0","normalized_profile":"att.datable.w3c","calendar":"Gregorian","locale":"la","provenance":{"source":"witness@r1"}}"#,
    );
    if let JsonValue::Object(ref mut o) = v {
        o.push((
            tos_foundation::JsonString::from_utf8("xml_fragment"),
            JsonValue::String(tos_foundation::JsonString::from_utf8(xml)),
        ));
    }
    v
}
fn run(xml: &str) -> JsonValue {
    capture_tei(&input(xml), &parse("{}"), TemporalBudget::default()).unwrap()
}
#[test]
fn exact_raw_and_month_granularity() {
    let xml = "<date when='1962-10'>October 1962</date>";
    let i = input(xml);
    let c = parse(r#"{"trs_ref":{"id":"calendar-a","version":"rev-7"}}"#);
    let r = capture_tei(&i, &c, TemporalBudget::default()).unwrap();
    assert_eq!(s(&r, "status"), Some("Complete"));
    assert_eq!(f(&r, "preserved_input"), &i);
    assert_eq!(f(&r, "preserved_context"), &c);
    assert_eq!(s(&r, "preserved_raw"), Some(xml));
    assert_eq!(s(f(&r, "value"), "resolution"), Some("month"));
    assert_eq!(s(f(&r, "value"), "type"), Some("CalendarPosition"));
    assert_eq!(s(f(&r, "value"), "position"), Some("1962-10"));
}
#[test]
fn interval_roles_and_inclusive_granularity_do_not_collapse() {
    // Four role combinations plus month and final leap-day granularity. The
    // same assertions verify inclusion without manufacturing a midnight bound.
    for (xml, kind, start, end, res, last) in [
        (
            "<date notBefore='1344' notAfter='1346'/>",
            "PossiblePositionBounds",
            "lower",
            "upper",
            "year",
            "1346",
        ),
        (
            "<date from='1863-05-28' to='1863-06-01'/>",
            "ActualInterval",
            "start",
            "end",
            "day",
            "1863-06-01",
        ),
        (
            "<date from='1344' notAfter='1346'/>",
            "ActualInterval",
            "start",
            "end",
            "year",
            "1346",
        ),
        (
            "<date notBefore='1344' to='1346'/>",
            "ActualInterval",
            "start",
            "end",
            "year",
            "1346",
        ),
        (
            "<date from='1856-03' to='1858-04'/>",
            "ActualInterval",
            "start",
            "end",
            "month",
            "1858-04",
        ),
        (
            "<date from='2020-02-28' to='2020-02-29'>through last leap day</date>",
            "ActualInterval",
            "start",
            "end",
            "day",
            "2020-02-29",
        ),
    ] {
        let r = run(xml);
        assert_eq!(s(&r, "status"), Some("Complete"));
        let v = f(&r, "value");
        assert_eq!(s(v, "type"), Some(kind));
        for key in [start, end] {
            let bound = f(v, key);
            assert!(!bound.is_null());
            assert_eq!(s(bound, "inclusivity"), Some("Inclusive"));
            assert_eq!(s(bound, "resolution"), Some(res));
            assert!(f(bound, "instant").is_null());
        }
        assert_eq!(s(f(v, end), "position"), Some(last));
        assert!(f(v, "end_exclusive").is_null());
        if xml.contains("from='1344' notAfter") {
            assert_eq!(s(v, "end_certainty"), Some("upper_bound_only"));
        }
        if xml.contains("notBefore='1344' to") {
            assert_eq!(f(f(v, start), "start_bound_only"), &JsonValue::Bool(true));
        }
    }
}
#[test]
fn precision_targets_are_local_and_ids_are_data() {
    for id in ["date001", "renamed-source-λ"] {
        let xml = format!(
            "<date xml:id='{id}' notBefore='0014' notAfter='0064'>text</date><precision target='#{id}' match='@notBefore' precision='high'/><precision target='#{id}' match='@notAfter' precision='low'/>"
        );
        let r = run(&xml);
        assert_eq!(s(&r, "status"), Some("Complete"));
        let v = f(&r, "value");
        assert_eq!(s(f(v, "lower"), "precision"), Some("high"));
        assert_eq!(s(f(v, "upper"), "precision"), Some("low"));
    }
    let r = run(
        "<date xml:id='a' when='1344'/><precision target='#other' match='@when' precision='high'/>",
    );
    assert_eq!(s(&r, "status"), Some("Partial"));
}
#[test]
fn unsupported_xml_never_resolves_external_entities_or_loses_raw() {
    for xml in [
        "<!DOCTYPE date [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><date when='1344'>&x;</date>",
        "<date when='1344' when='1345'/>",
        "<date when='1344'><name>x</name></date>",
        "<date when='1344'>unterminated",
    ] {
        let r = run(xml);
        assert_ne!(s(&r, "status"), Some("Complete"));
        assert_eq!(s(&r, "preserved_raw"), Some(xml));
    }
    let mut b = TemporalBudget::default();
    b.max_expression_bytes = 2;
    assert!(capture_tei(&input("<date/>"), &parse("{}"), b).is_err());
}
#[test]
fn source_calendar_pointers_do_not_reinterpret_normalized_w3c_date() {
    for (xml, source, pointers, position) in [
        (
            "<date calendar='#julian' when='1632-06-06'>27th of May (old style)</date>",
            "#julian",
            r##"["#julian"]"##,
            "1632-06-06",
        ),
        (
            "<date calendar='#gregorian #julian' when='1732-02-22'>Feb 22; Feb 11 old style</date>",
            "#gregorian #julian",
            r##"["#gregorian","#julian"]"##,
            "1732-02-22",
        ),
    ] {
        let r = run(xml);
        assert_eq!(s(&r, "status"), Some("Complete"));
        let v = f(&r, "value");
        assert_eq!(s(v, "source_calendar"), Some(source));
        assert_eq!(f(v, "source_calendar_pointers"), &parse(pointers));
        assert_eq!(s(v, "calendar"), Some("Gregorian"));
        assert_eq!(s(v, "normalized_calendar"), Some("Gregorian"));
        assert_eq!(s(v, "position"), Some(position));
        assert_eq!(
            s(&r, "normalized_profile"),
            Some("TEI-P5-4.12.0:att.datable.w3c")
        );
    }
}
#[test]
fn w3c_year_numbering_and_version_are_explicit() {
    for date in ["-0001", "-0001-02", "-0001-02-29", "0001", "0044-03-15"] {
        let r = run(&format!("<date when='{date}'/>"));
        assert_eq!(s(&r, "status"), Some("Complete"), "{date}");
        assert_eq!(s(f(&r, "value"), "position"), Some(date));
        assert_eq!(s(&r, "year_numbering"), Some("W3C-no-year-zero"));
    }
    for date in ["0000", "-0000", "0000-02-29"] {
        let r = run(&format!("<date when='{date}'/>"));
        assert_eq!(
            s(&r, "reason"),
            Some("YearZeroNotPermittedByW3CProfile"),
            "{date}"
        );
    }
    let r = capture_tei(
        &parse(r#"{"xml_fragment":"<date when='0044'/>","tei_version":"P5-other"}"#),
        &parse("{}"),
        TemporalBudget::default(),
    )
    .unwrap();
    assert_eq!(s(&r, "status"), Some("Unsupported"));
}
#[test]
fn forbidden_attribute_combinations_are_diagnosed_without_precedence() {
    for attrs in [
        "when='0044' from='0044'",
        "when='0044' to='0044'",
        "when='0044' notBefore='0044'",
        "when='0044' notAfter='0044'",
        "from='0044' notBefore='0044'",
        "to='0044' notAfter='0044'",
    ] {
        let r = run(&format!("<date {attrs}/>"));
        assert_eq!(s(&r, "reason"), Some("IncompatibleTeiAttributes"));
    }
}
#[test]
fn normalized_profile_selection_is_explicit_and_version_shorthand_is_bounded() {
    // Keep the original legacy request and the identical-lexical unknown/known
    // pair in one matrix. Unknown profiles cannot inherit a calendar.
    let legacy = parse(r#"{"xml_fragment":"<date when='1344'/>","tei_version":"P5-4.12.0"}"#);
    let xml = "<date when='1632-06-06'/>";
    let known = input(xml);
    let mut unknown = known.clone();
    if let JsonValue::Object(ref mut o) = unknown {
        let (_, value) = o
            .iter_mut()
            .find(|(key, _)| key.as_str() == Some("normalized_profile"))
            .unwrap();
        *value = JsonValue::String(tos_foundation::JsonString::from_utf8("Unknown"));
    }
    for (i, status, raw) in [
        (legacy, "Complete", "<date when='1344'/>"),
        (unknown, "Unsupported", xml),
        (known, "Complete", xml),
    ] {
        let r = capture_tei(&i, &parse("{}"), TemporalBudget::default()).unwrap();
        assert_eq!(s(&r, "status"), Some(status));
        assert_eq!(s(&r, "preserved_raw"), Some(raw));
        if status == "Complete" {
            assert_eq!(s(&r, "normalized_calendar"), Some("Gregorian"));
        } else {
            assert!(f(&r, "normalized_calendar").is_null());
        }
    }
}
