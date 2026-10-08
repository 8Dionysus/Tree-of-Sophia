//! Lived-witness schema, exact-body binding and private route mechanics only.
//! Human authorship, consent, memory, context and meaning remain unvalidated.
use crate::executor::{Limits, capture_ci_git};
use crate::route_cards::{RouteSources, sha256_bytes};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    io,
    path::Path,
    sync::atomic::{AtomicI32, Ordering},
    time::Duration,
};
use tos_foundation::{JsonLimits, JsonMode, parse_json};

pub type Issue = (String, String);
const SCHEMA: &str = "ToS/contracts/lived-witness-packet.schema.json";
const ROUTE: &str = "ToS/zarathustra/lived-witness";
const REQUIRED: &[(&str, &[&str])] = &[
    (
        "AGENTS.md",
        &[
            "explicit author request",
            "AI output",
            "never lived witness",
            "separate permissions",
            "cannot confirm",
        ],
    ),
    (
        "README.md",
        &[
            "private-by-default",
            "I am ready to record a lived witness",
            "No record is created",
        ],
    ),
    (
        "CAPTURE_PROTOCOL.md",
        &[
            "no testimony captured",
            "the time or interval of the remembered experience",
            "the recording time of this version",
            "All except local storage begin `not-granted`",
            "separate `claim-packet`",
            "Structural validation is insufficient",
        ],
    ),
    (
        "CAPTURE_FORM.md",
        &[
            "one block at a time",
            "Keep the raw answer",
            "Without explicit confirmation",
            "Default answer for every item below is `not-granted`",
        ],
    ),
    (
        "local-content/README.md",
        &[
            "ignored by Git",
            "mode `0600`",
            "No testimony has been created",
        ],
    ),
];
fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
fn tick(sources: &RouteSources, cancel: &AtomicI32) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) != 0 {
        return Err(invalid("lived-witness validation cancelled"));
    }
    sources.check()
}
fn parse(raw: &[u8]) -> io::Result<Value> {
    if raw.len() > 1_048_576 {
        return Err(invalid("lived-witness JSON byte bound"));
    }
    parse_json(raw, JsonMode::RequestLastWins, JsonLimits::default())
        .map_err(|e| invalid(format!("lived-witness finite JSON profile: {e:?}")))?;
    serde_json::from_slice(raw).map_err(io::Error::other)
}
pub fn synthetic_draft_packet() -> Value {
    serde_json::from_str(include_str!(
        "../tests/fixtures/lived-witness.synthetic.json"
    ))
    .expect("compiled synthetic schema fixture")
}
pub fn validate_packet_mechanics(packet: &Value, schema: &Value) -> io::Result<Vec<String>> {
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(true)
        .offline()
        .build(schema)
        .map_err(|e| invalid(format!("lived-witness schema load/check failed: {e}")))?;
    let mut issues = Vec::new();
    let mut bytes = 0usize;
    for error in validator.iter_errors(packet) {
        let path = error.instance_path().to_string();
        let message = format!("{}: {error}", if path.is_empty() { "root" } else { &path });
        bytes = bytes
            .checked_add(message.len())
            .ok_or_else(|| invalid("lived-witness issue accounting"))?;
        if issues.len() >= 4096 || bytes > 1_048_576 {
            return Err(invalid("lived-witness issue bound"));
        }
        issues.push(message);
    }
    if let (Some(body), Some(declared)) = (
        packet["testimony"]["body"].as_str(),
        packet["testimony"]["body_sha256"].as_str(),
    ) {
        if body.len() > 1_048_576 {
            return Err(invalid("lived-witness body byte bound"));
        }
        if sha256_bytes(body.as_bytes()) != declared {
            issues.push("testimony.body_sha256 does not match the exact UTF-8 body".into());
        }
    }
    if packet["record_status"] == "author-confirmed" {
        if let (Some(body), Some(reviewed)) = (
            packet["testimony"]["body_sha256"].as_str(),
            packet["author_review"]["reviewed_body_sha256"].as_str(),
        ) {
            if body != reviewed {
                issues.push(
                    "author_review.reviewed_body_sha256 does not bind the exact testimony body"
                        .into(),
                );
            }
        }
    }
    Ok(issues)
}
fn git(
    root: &Path,
    sources: &RouteSources,
    args: &[&str],
    limits: Limits,
    cancel: &AtomicI32,
) -> io::Result<(i32, Vec<u8>)> {
    tick(sources, cancel)?;
    sources.verify_root()?;
    let mut argv = vec!["/usr/bin/env".into()];
    let mut names = 0usize;
    for (name, _) in std::env::vars_os() {
        if name.as_encoded_bytes().starts_with(b"GIT_TRACE") {
            names = names
                .checked_add(name.len())
                .filter(|n| *n <= 4096)
                .ok_or_else(|| invalid("lived-witness Git trace bound"))?;
            if argv.len() >= 256 {
                return Err(invalid("lived-witness Git environment bound"));
            }
            argv.push("-u".into());
            argv.push(
                name.into_string()
                    .map_err(|_| invalid("lived-witness Git trace name"))?,
            );
        }
    }
    argv.extend(
        [
            "--",
            "GIT_NO_LAZY_FETCH=1",
            "GIT_OPTIONAL_LOCKS=0",
            "/usr/bin/git",
            "--no-pager",
            "--no-optional-locks",
            "--no-replace-objects",
            "--literal-pathspecs",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "trace2.eventTarget=0",
            "-c",
            "trace2.perfTarget=0",
            "-c",
            "trace2.normalTarget=0",
        ]
        .map(str::to_owned),
    );
    argv.extend(args.iter().map(|s| (*s).to_owned()));
    let wall = sources
        .remaining_time()?
        .min(limits.command_wall)
        .min(Duration::from_secs(30));
    let (code, stdout, _) = capture_ci_git(
        root,
        argv,
        Limits {
            command_wall: wall,
            lane_wall: wall,
            cleanup_grace: limits.cleanup_grace.min(Duration::from_secs(1)),
            output_bytes: limits.output_bytes.min(65536),
        },
        cancel,
    )?;
    tick(sources, cancel)?;
    sources.verify_root()?;
    if !matches!(code, 0 | 1) {
        return Err(invalid("lived-witness Git boundary query failed"));
    }
    Ok((code, stdout))
}
pub fn validate(
    root: &Path,
    sources: &mut RouteSources,
    limits: Limits,
    cancel: &AtomicI32,
) -> io::Result<Vec<Issue>> {
    let mut issues = Vec::new();
    for path in REQUIRED
        .iter()
        .map(|(p, _)| format!("{ROUTE}/{p}"))
        .chain([SCHEMA.into()])
    {
        tick(sources, cancel)?;
        if !sources.is_file(&path)? {
            issues.push((path, "required lived-witness route file is missing".into()));
        }
    }
    if !issues.is_empty() {
        return Ok(issues);
    }
    for (relative, tokens) in REQUIRED {
        tick(sources, cancel)?;
        let path = format!("{ROUTE}/{relative}");
        let text = sources
            .text(&path)?
            .ok_or_else(|| invalid("lived-witness route file disappeared"))?;
        for token in *tokens {
            if !text.contains(token) {
                issues.push((path.clone(), format!("missing boundary token: {token}")));
            }
        }
    }
    let schema = parse(&sources.bounded_bytes(SCHEMA, 1_048_576, &mut 0, 1_048_576)?)?;
    tick(sources, cancel)?;
    for issue in validate_packet_mechanics(&synthetic_draft_packet(), &schema)? {
        issues.push((SCHEMA.into(), format!("synthetic fixture {issue}")));
    }
    for (path, expected, message) in [
        (
            format!("{ROUTE}/local-content/tos.lived-witness.validator-probe/packet.json"),
            true,
            "private lived-witness packet path is not ignored",
        ),
        (
            format!("{ROUTE}/local-content/README.md"),
            false,
            "local-content boundary README is unexpectedly ignored",
        ),
        (
            format!("{ROUTE}/CAPTURE_FORM.md"),
            false,
            "lived-witness route form is unexpectedly ignored",
        ),
    ] {
        let (code, _) = git(
            root,
            sources,
            &["check-ignore", "--no-index", "--quiet", "--", &path],
            limits,
            cancel,
        )?;
        if (code == 0) != expected {
            issues.push((".gitignore".into(), message.into()));
        }
    }
    let local = format!("{ROUTE}/local-content");
    let (code, tracked) = git(
        root,
        sources,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            &local,
        ],
        limits,
        cancel,
    )?;
    if code != 0 {
        return Err(invalid("lived-witness tracked-boundary query failed"));
    }
    let tracked: BTreeSet<&[u8]> = tracked
        .split(|v| *v == 0)
        .filter(|v| !v.is_empty())
        .collect();
    let expected = format!("{local}/README.md");
    if tracked != BTreeSet::from([expected.as_bytes()]) {
        // The private names themselves are unnecessary to diagnose this boundary.
        issues.push((
            local,
            format!(
                "tracked private-content surface must contain only README.md; found {} entries",
                tracked.len()
            ),
        ));
    }
    tick(sources, cancel)?;
    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lived_witness_keeps_body_review_authorship_permissions_and_format_checks() {
        let schema: Value = serde_json::from_str(include_str!(
            "../../../../ToS/contracts/lived-witness-packet.schema.json"
        ))
        .unwrap();
        let draft = synthetic_draft_packet();
        assert!(
            validate_packet_mechanics(&draft, &schema)
                .unwrap()
                .is_empty()
        );
        let mut body = draft.clone();
        body["testimony"]["body"] = "Changed after hashing.".into();
        assert!(
            validate_packet_mechanics(&body, &schema)
                .unwrap()
                .iter()
                .any(|s| s.contains("exact UTF-8 body"))
        );
        for (pointer, value) in [
            ("/author/agent_kind", Value::from("model")),
            ("/capture/silent_transformation_allowed", Value::from(true)),
            ("/record_status", Value::from("author-confirmed")),
            ("/capture/recorded_at", Value::from("not-a-time")),
        ] {
            let mut packet = draft.clone();
            *packet.pointer_mut(pointer).unwrap() = value;
            assert!(
                !validate_packet_mechanics(&packet, &schema)
                    .unwrap()
                    .is_empty(),
                "{pointer}"
            );
        }
        let mut packet = draft.clone();
        packet["use_permissions"]
            .as_object_mut()
            .unwrap()
            .remove("model_training");
        assert!(
            !validate_packet_mechanics(&packet, &schema)
                .unwrap()
                .is_empty()
        );
        let mut confirmed = draft;
        confirmed["record_status"] = "author-confirmed".into();
        confirmed["author_review"] = serde_json::json!({"author_confirmed": true, "confirmed_at": "2026-08-08T00:05:00Z", "confirmation_method": "explicit-text-confirmation", "reviewed_body_sha256": confirmed["testimony"]["body_sha256"], "review_note": null});
        assert!(
            validate_packet_mechanics(&confirmed, &schema)
                .unwrap()
                .is_empty()
        );
        confirmed["author_review"]["reviewed_body_sha256"] = "0".repeat(64).into();
        assert!(
            validate_packet_mechanics(&confirmed, &schema)
                .unwrap()
                .iter()
                .any(|s| s.contains("does not bind the exact testimony body"))
        );
    }
}
