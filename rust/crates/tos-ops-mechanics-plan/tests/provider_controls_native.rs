//! Whole closure controls for the parent-owned native product acceptance lane.
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tos_ops_mechanics_plan::provider_controls::{self, PATHS};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "tos-provider-controls-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn template(&self) -> PathBuf {
        let path = self.0.join("template.json");
        fs::write(
            &path,
            include_bytes!("../../../../kag/provider-template.json"),
        )
        .unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn complete_closure_preserves_template_bytes_and_rejects_mutation_or_reuse() {
    let fixture = Fixture::new();
    let template = fixture.template();
    let provider = fixture.0.join("provider");
    fs::create_dir(&provider).unwrap();
    let expected = provider_controls::template(&template).unwrap();
    let before = fs::read(&template).unwrap();
    let entries = provider_controls::materialize(&provider, &template).unwrap();
    assert_eq!(entries.iter().map(|e| e.path).collect::<Vec<_>>(), PATHS);
    for (path, bytes) in expected {
        assert_eq!(fs::read(provider.join(path)).unwrap(), bytes);
    }
    assert_eq!(fs::read(&template).unwrap(), before);
    let raw = serde_json::to_vec(&entries).unwrap();
    provider_controls::verify_request(&provider, &raw).unwrap();
    assert!(provider_controls::materialize(&provider, &template).is_err());
    fs::write(provider.join(PATHS[0]), b"changed").unwrap();
    assert!(provider_controls::verify_request(&provider, &raw).is_err());
}
#[test]
fn duplicate_contract_and_nonfinite_or_blank_template_are_refused() {
    let fixture = Fixture::new();
    let template = fixture.template();
    for raw in [
        br#"{"schema_version":"tos_kag_provider_template_v1","files":{},"files":{}}"#.as_slice(),
        br#"{"schema_version":"tos_kag_provider_template_v1","files":{"x":NaN}}"#,
        br#"[]"#,
    ] {
        fs::write(&template, raw).unwrap();
        assert!(provider_controls::template(&template).is_err());
    }
    let mut value: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../../kag/provider-template.json")).unwrap();
    value["files"][PATHS[0]] = serde_json::json!("\u{001c}\u{001f}\n");
    fs::write(&template, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(provider_controls::template(&template).is_err());
}
#[cfg(unix)]
#[test]
fn linked_provider_parent_and_linked_template_are_refused() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let template = fixture.template();
    let provider = fixture.0.join("provider");
    let elsewhere = fixture.0.join("elsewhere");
    fs::create_dir(&provider).unwrap();
    fs::create_dir(&elsewhere).unwrap();
    symlink(&elsewhere, provider.join("kag")).unwrap();
    assert!(provider_controls::materialize(&provider, &template).is_err());
    assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
    let linked_template = fixture.0.join("linked-template.json");
    symlink(&template, &linked_template).unwrap();
    assert!(provider_controls::template(&linked_template).is_err());
}
