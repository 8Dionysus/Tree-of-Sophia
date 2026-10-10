//! Maintained foundation CLI grammar and output. The installed caller supplies
//! the selected repository root; neither cwd nor the build worktree selects it.
//! Evaluation and resource admission remain the authenticated CMD connector's
//! responsibility. An incomplete evaluation must never reach `write_result`.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use tos_foundation::{
    JsonEmissionProfile, JsonLimits, JsonMode, emit_json_profile, parse_json_with_state_budget,
};
use tos_validation::source_foundation_default_rules::SourceFoundationDefaultRuleScope;
use tos_validation::source_foundation_labs::SourceFoundationLab;
use tos_validation::source_foundation_schema::SourceFoundationSchemaReport;

/// The caller selects normal document findings explicitly; lab controls have
/// separate report/expectation semantics and must not become document errors.
pub(crate) struct SchemaPlacement<'a> {
    pub before_issue: usize,
    pub check_index: usize,
    pub location: &'a str,
    pub contract: &'a str,
    pub prefix: &'a str,
}

/// Move existing owner findings and interleave complete, bound schema findings
/// at the owner's encounter ordinals. `workspace` admits the additional output
/// vector and newly copied schema strings; caller-owned direct/report state is
/// accounted separately. No partial result escapes on refusal.
pub(crate) fn interleave_schema_findings(
    direct: Vec<(String, String)>,
    report: &SourceFoundationSchemaReport,
    selected: &[SchemaPlacement<'_>],
    max_issues: usize,
    max_output_bytes: usize,
    workspace: usize,
) -> Result<Vec<(String, String)>, &'static str> {
    if !report.is_complete() {
        return Err("foundation schema report incomplete");
    }
    let mut count = direct.len();
    let mut output_bytes = 0usize;
    let mut copied_bytes = 0usize;
    for (location, message) in &direct {
        output_bytes = output_bytes
            .checked_add(location.len())
            .and_then(|n| n.checked_add(message.len()))
            .ok_or("foundation issue byte overflow")?;
    }
    let mut prior = None;
    for placement in selected {
        if placement.before_issue > direct.len()
            || prior.is_some_and(|(ordinal, index)| {
                placement.before_issue < ordinal || placement.check_index <= index
            })
        {
            return Err("foundation schema placement order");
        }
        prior = Some((placement.before_issue, placement.check_index));
        let check = report
            .checks
            .get(placement.check_index)
            .ok_or("foundation schema placement absent")?;
        if check.location != placement.location || check.contract != placement.contract {
            return Err("foundation schema placement binding");
        }
        count = count
            .checked_add(check.issues.len())
            .ok_or("foundation issue count overflow")?;
        for issue in &check.issues {
            let bytes = issue
                .location
                .len()
                .checked_add(placement.prefix.len())
                .and_then(|n| n.checked_add(issue.message.len()))
                .ok_or("foundation schema issue byte overflow")?;
            copied_bytes = copied_bytes
                .checked_add(bytes)
                .ok_or("foundation schema issue state overflow")?;
            output_bytes = output_bytes
                .checked_add(bytes)
                .ok_or("foundation issue byte overflow")?;
        }
    }
    let additional_state = count
        .checked_mul(std::mem::size_of::<(String, String)>())
        .and_then(|n| n.checked_add(copied_bytes))
        .ok_or("foundation issue workspace overflow")?;
    if count > max_issues || output_bytes > max_output_bytes || additional_state > workspace {
        return Err("foundation issue limits");
    }
    let mut output = Vec::with_capacity(count);
    let mut direct = direct.into_iter();
    let mut ordinal = 0;
    for placement in selected {
        while ordinal < placement.before_issue {
            output.push(direct.next().ok_or("foundation issue placement absent")?);
            ordinal += 1;
        }
        for issue in &report.checks[placement.check_index].issues {
            let mut message = String::with_capacity(placement.prefix.len() + issue.message.len());
            message.push_str(placement.prefix);
            message.push_str(issue.message);
            output.push((issue.location.clone(), message));
        }
    }
    output.extend(direct);
    Ok(output)
}

/// Same foundation JSON visitor as other maintained sinks, selecting sorted
/// Python pretty output for this actual CLI report. `workspace` is exclusive
/// of the caller-owned Value and includes raw bytes, parser and output state.
pub(crate) fn format_lab_report(
    value: &serde_json::Value,
    limits: JsonLimits,
    workspace: usize,
) -> Result<Vec<u8>, &'static str> {
    struct Count(usize, usize);
    impl Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_add(bytes.len())
                .filter(|n| *n <= self.1)
                .ok_or_else(|| std::io::Error::other("foundation report byte limit"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count(0, limits.max_bytes);
    serde_json::to_writer(&mut count, value).map_err(|_| "foundation report encoding limit")?;
    // The visitor's object-reference vector and duplicate-key set are bounded
    // by visits; allocator/RSS margins belong to the operation RAM envelope.
    let reserved = count
        .0
        .checked_add(limits.max_bytes)
        .and_then(|n| {
            limits
                .max_visits
                .checked_mul(64)
                .and_then(|v| n.checked_add(v))
        })
        .ok_or("foundation report workspace overflow")?;
    let parser_workspace = workspace
        .checked_sub(reserved)
        .ok_or("foundation report workspace limit")?;
    let raw = serde_json::to_vec(value).map_err(|_| "foundation report encoding")?;
    if raw.len() != count.0 {
        return Err("foundation report encoding changed");
    }
    let parsed =
        parse_json_with_state_budget(&raw, JsonMode::PublishedStrict, limits, parser_workspace)
            .map_err(|_| "foundation report parsing limit")?;
    emit_json_profile(
        parsed.root(),
        JsonEmissionProfile::SourceFoundationLabReportV1,
        limits,
    )
    .map(|encoded| encoded.bytes)
    .map_err(|_| "foundation report output limit")
}

/// Software-owner declaration, independent of any immutable candidate grammar.
pub(crate) const VALIDATION_PROFILE_DECLARATION: &str =
    "ToS/doctrine/semantic-interchange/source-validation-profiles.v1.json";
const VALIDATION_PROFILE_BYTES: &[u8] = include_bytes!(
    "../../../../ToS/doctrine/semantic-interchange/source-validation-profiles.v1.json"
);
#[derive(Clone, Copy, Debug)]
pub(crate) struct ValidationProfile {
    pub id: &'static str,
    pub input_scope: &'static str,
    pub declaration_sha256: tos_foundation::Digest256,
    pub scope: SourceFoundationDefaultRuleScope,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidationProfileRegistry<'a> {
    schema_version: &'a str,
    registry_id: &'a str,
    registry_version: u64,
    default_profile: &'a str,
    #[serde(borrow)]
    profiles: Vec<ValidationProfileDeclaration<'a>>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidationProfileDeclaration<'a> {
    profile_id: &'a str,
    input_scope: &'a str,
    #[serde(borrow)]
    required_owners: Vec<&'a str>,
    authority_posture: &'a str,
}
fn select_validation_profile_from_bytes(
    raw: &'static [u8],
    selected: Option<&str>,
) -> Result<ValidationProfile, &'static str> {
    if raw.len() > 65536 {
        return Err("validation profile declaration byte bound");
    }
    let limits = JsonLimits::new(65536, 16, 8192, 20).map_err(|_| "validation profile limits")?;
    parse_json_with_state_budget(raw, JsonMode::PublishedStrict, limits, 1024 * 1024)
        .map_err(|_| "validation profile declaration JSON")?;
    let registry: ValidationProfileRegistry<'static> =
        serde_json::from_slice(raw).map_err(|_| "validation profile declaration shape")?;
    if registry.schema_version != "tos_source_validation_profile_registry_v1"
        || registry.registry_id != "tos.source-validation-profiles"
        || registry.registry_version != 1
        || registry.profiles.is_empty()
        || registry.profiles.len() > 64
    {
        return Err("validation profile registry identity/count");
    }
    const CORE: [&str; 8] = [
        "records",
        "bibliography",
        "rights",
        "review",
        "references",
        "dependency-closure",
        "discovery",
        "closure",
    ];
    let mut ids = std::collections::BTreeSet::new();
    let wanted = selected.unwrap_or(registry.default_profile);
    let mut result = None;
    let mut default_is_full = false;
    for declaration in registry.profiles {
        let id = declaration.profile_id;
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || !ids.insert(id)
            || declaration.authority_posture != "mechanical_validation_only"
        {
            return Err("validation profile ID/posture");
        }
        let scope = match declaration.input_scope {
            "full_audit" => SourceFoundationDefaultRuleScope::FullAudit,
            "selected_source_closure" => SourceFoundationDefaultRuleScope::SelectedSourceClosure,
            "selected_record_closure" => SourceFoundationDefaultRuleScope::SelectedRecordClosure,
            "selected_generated_record_closure" => {
                SourceFoundationDefaultRuleScope::SelectedGeneratedRecordClosure
            }
            _ => return Err("unsupported validation profile input scope"),
        };
        let owners: std::collections::BTreeSet<_> =
            declaration.required_owners.iter().copied().collect();
        let mut expected: std::collections::BTreeSet<_> = CORE.into_iter().collect();
        if scope == SourceFoundationDefaultRuleScope::FullAudit {
            expected.extend(["laboratories", "goldsets"]);
        }
        if owners.len() != declaration.required_owners.len() || owners != expected {
            return Err("validation profile required owner closure");
        }
        if id == registry.default_profile {
            default_is_full = scope == SourceFoundationDefaultRuleScope::FullAudit;
        }
        if id == wanted {
            result = Some(ValidationProfile {
                id,
                input_scope: declaration.input_scope,
                declaration_sha256: tos_foundation::Digest256::of_bytes(raw),
                scope,
            });
        }
    }
    if !default_is_full {
        return Err("validation profile default must be full audit");
    }
    result.ok_or("unknown validation profile ID")
}
pub(crate) fn select_validation_profile(
    selected: Option<&str>,
) -> Result<ValidationProfile, &'static str> {
    select_validation_profile_from_bytes(VALIDATION_PROFILE_BYTES, selected)
}

#[derive(Debug)]
pub(crate) struct FoundationArguments {
    pub repo_root: Option<PathBuf>,
    pub payload_source_root: Option<PathBuf>,
    pub require_local_payloads: bool,
    pub selected_lab: Option<SourceFoundationLab>,
    pub validation_profile: ValidationProfile,
    pub validation_profile_explicit: bool,
    pub record_selection_manifest: Option<PathBuf>,
    pub indexed_input_root: Option<PathBuf>,
    pub help: bool,
}

const FLAGS: [(&str, SourceFoundationLab); 6] = [
    (
        "--source-anchor-v2-lab-only",
        SourceFoundationLab::SourceAnchorV2,
    ),
    (
        "--source-text-layer-lab-only",
        SourceFoundationLab::SourceTextLayer,
    ),
    (
        "--provenance-v2-lab-only",
        SourceFoundationLab::ProvenanceV2,
    ),
    (
        "--semantic-annotation-v2-lab-only",
        SourceFoundationLab::SemanticAnnotationV2,
    ),
    (
        "--translation-alignment-v1-lab-only",
        SourceFoundationLab::TranslationAlignmentV1,
    ),
    (
        "--source-text-unit-v1-lab-only",
        SourceFoundationLab::SourceTextUnitV1,
    ),
];

pub(crate) fn parse_arguments(
    args: &[OsString],
    installed_repo_root: Option<&Path>,
) -> Result<FoundationArguments, &'static str> {
    if args.len() > 64
        || args
            .iter()
            .try_fold(0usize, |sum, a| sum.checked_add(a.len()))
            .is_none_or(|n| n > 32 * 1024)
    {
        return Err("foundation argument limits");
    }
    if installed_repo_root.is_some_and(|root| !root.is_absolute()) {
        return Err("foundation installed root must be absolute");
    }
    let mut parsed = FoundationArguments {
        repo_root: installed_repo_root.map(Path::to_owned),
        payload_source_root: None,
        require_local_payloads: false,
        selected_lab: None,
        validation_profile: select_validation_profile(None)?,
        validation_profile_explicit: false,
        record_selection_manifest: None,
        indexed_input_root: None,
        help: false,
    };
    let mut selected = [false; 6];
    let mut index = 0;
    while index < args.len() {
        let arg = args[index]
            .to_str()
            .ok_or("foundation option must be UTF-8")?;
        match arg {
            "--help" | "-h" => parsed.help = true,
            "--require-local-payloads" => parsed.require_local_payloads = true,
            "--validation-profile" => {
                if parsed.validation_profile_explicit {
                    return Err("validation profile selected more than once");
                }
                index += 1;
                let id = args
                    .get(index)
                    .and_then(|v| v.to_str())
                    .ok_or("validation profile requires UTF-8 ID")?;
                parsed.validation_profile = select_validation_profile(Some(id))?;
                parsed.validation_profile_explicit = true;
            }
            "--record-selection-manifest" => {
                index += 1;
                let path = PathBuf::from(
                    args.get(index)
                        .ok_or("record selection manifest requires a path")?,
                );
                if parsed.record_selection_manifest.replace(path).is_some() {
                    return Err("record selection manifest selected more than once");
                }
            }
            "--indexed-input-root" => {
                index += 1;
                let path = PathBuf::from(
                    args.get(index)
                        .ok_or("indexed input root requires a path")?,
                );
                if parsed.indexed_input_root.replace(path).is_some() {
                    return Err("indexed input root selected more than once");
                }
            }
            "--repo-root" | "--payload-source-root" => {
                index += 1;
                let path = PathBuf::from(
                    args.get(index)
                        .ok_or("foundation path option requires a value")?,
                );
                if arg == "--repo-root" {
                    parsed.repo_root = Some(path);
                } else {
                    parsed.payload_source_root = Some(path);
                }
            }
            _ => {
                if let Some((flag, value)) = arg.split_once('=') {
                    match flag {
                        "--validation-profile" => {
                            if parsed.validation_profile_explicit {
                                return Err("validation profile selected more than once");
                            }
                            parsed.validation_profile = select_validation_profile(Some(value))?;
                            parsed.validation_profile_explicit = true;
                        }
                        "--record-selection-manifest" => {
                            if parsed
                                .record_selection_manifest
                                .replace(PathBuf::from(value))
                                .is_some()
                            {
                                return Err("record selection manifest selected more than once");
                            }
                        }
                        "--indexed-input-root" => {
                            if parsed
                                .indexed_input_root
                                .replace(PathBuf::from(value))
                                .is_some()
                            {
                                return Err("indexed input root selected more than once");
                            }
                        }
                        "--repo-root" => parsed.repo_root = Some(PathBuf::from(value)),
                        "--payload-source-root" => {
                            parsed.payload_source_root = Some(PathBuf::from(value))
                        }
                        _ => return Err("unrecognized foundation option"),
                    }
                } else if let Some(n) = FLAGS.iter().position(|(flag, _)| *flag == arg) {
                    selected[n] = true;
                } else {
                    return Err("unrecognized foundation option");
                }
            }
        }
        index += 1;
    }
    // argparse exposes independent booleans; maintained branch order wins,
    // independently of the order in which flags appeared on the command line.
    parsed.selected_lab = FLAGS
        .iter()
        .enumerate()
        .find_map(|(n, (_, lab))| selected[n].then_some(*lab));
    if parsed.validation_profile_explicit && parsed.selected_lab.is_some() {
        return Err("validation profile is separate from lab-only selection");
    }
    if parsed.record_selection_manifest.is_some()
        != matches!(
            parsed.validation_profile.scope,
            SourceFoundationDefaultRuleScope::SelectedRecordClosure
                | SourceFoundationDefaultRuleScope::SelectedGeneratedRecordClosure
        )
    {
        return Err("selected-record-closure requires its exclusive record selection manifest");
    }
    if parsed
        .record_selection_manifest
        .as_ref()
        .is_some_and(|path| {
            !path.is_absolute()
                || path.components().any(|c| {
                    !matches!(
                        c,
                        std::path::Component::RootDir | std::path::Component::Normal(_)
                    )
                })
        })
    {
        return Err("record selection manifest path must be absolute and normalized");
    }
    if parsed.indexed_input_root.is_some()
        != (parsed.validation_profile.scope
            == SourceFoundationDefaultRuleScope::SelectedGeneratedRecordClosure)
    {
        return Err("selected-generated-record-closure requires its exclusive indexed input root");
    }
    if parsed.indexed_input_root.as_ref().is_some_and(|path| {
        !path.is_absolute()
            || path.components().any(|c| {
                !matches!(
                    c,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )
            })
    }) {
        return Err("indexed input root must be absolute and normalized");
    }
    if !parsed.help && parsed.repo_root.is_none() {
        return Err("foundation requires an explicitly selected --repo-root");
    }
    Ok(parsed)
}

pub(crate) const HELP: &str = "usage: tos-native-owner-command foundation --repo-root PATH --invocation ABSOLUTE_PATH [--require-local-payloads] [--payload-source-root PATH]\n\nThe default route performs the complete source-witness foundation audit and checks generated catalog parity. The protected tos_local_native_foundation_invocation_v1 request selects the exact installed command and schema-worker digests plus finite source, CPU, private-stage, state and output budgets. Run inside the abyss-machine private-tmpfs owner launcher with its sealed ABYSS_STAGE_TICKET_FD and matching ABYSS_STAGE_ROOT. An explicit validation profile or lab-only option is not connected to this whole-operation route.\n";

fn lab_output(lab: SourceFoundationLab) -> Result<(&'static str, &'static str), &'static str> {
    Ok(match lab {
        SourceFoundationLab::SourceAnchorV2 => (
            "Source-anchor v2 laboratory validation failed.",
            "[scope] Synthetic anchor selection, digest and resolution controls.",
        ),
        SourceFoundationLab::SourceTextLayer => (
            "Source-text-layer laboratory validation failed.",
            "[scope] Synthetic text-layer lineage, correction and normalization controls.",
        ),
        SourceFoundationLab::ProvenanceV2 => (
            "Provenance-event-v2 laboratory validation failed.",
            "[scope] Synthetic provenance shape, byte binding and rejection controls.",
        ),
        SourceFoundationLab::SemanticAnnotationV2 => (
            "Semantic identity/annotation v2 laboratory validation failed.",
            "[scope] Synthetic semantic-packet identity, evidence, competition and review-state controls.",
        ),
        SourceFoundationLab::TranslationAlignmentV1 => (
            "Translation-alignment v1 laboratory validation failed.",
            "[scope] Synthetic translation-mapping shape, exact side bindings and acceptance-state controls.",
        ),
        SourceFoundationLab::SourceTextUnitV1 => (
            "Source-text-unit v1 laboratory validation failed.",
            "[scope] Synthetic TextUnit and segmentation identity, anchor, coverage and review-state controls.",
        ),
        _ => return Err("foundation authored bridge is not a lab-only CLI route"),
    })
}

/// Write only a complete accepted evaluation. Lab report bytes are supplied by
/// the bounded Python-compatible report formatter after real diagnostics have
/// filled every result slot. Issues retain their maintained concatenation order.
pub(crate) fn write_result(
    args: &FoundationArguments,
    lab_report: Option<&[u8]>,
    issues: &[(String, String)],
    max_output_bytes: usize,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<i32, &'static str> {
    let (failure, scope) = if let Some(lab) = args.selected_lab {
        let output = lab_output(lab)?;
        let report = lab_report.ok_or("foundation lab report absent")?;
        if !report.ends_with(b"\n") {
            return Err("foundation lab report framing");
        }
        output
    } else {
        if lab_report.is_some() {
            return Err("foundation default has no lab report output");
        }
        (
            "Source-witness foundation validation failed.",
            "[scope] Source identity, reference, fixity and record mechanics. Source-visible assessment, rights, consent, canon and publication retain their owner decisions.",
        )
    };
    let posture = if args.require_local_payloads {
        "required and fixity-checked"
    } else {
        "optional; present bytes fixity-checked"
    };
    let mut output_bytes = lab_report.map_or(0, <[u8]>::len);
    let mut charge = |bytes: usize| -> Result<(), &'static str> {
        output_bytes = output_bytes
            .checked_add(bytes)
            .filter(|n| *n <= max_output_bytes)
            .ok_or("foundation output byte limit")?;
        Ok(())
    };
    charge(0)?;
    if issues.is_empty() {
        if args.selected_lab.is_none() {
            charge("[ok] validated source-witness evidence spine ()\n".len())?;
            charge(posture.len())?;
        }
        charge(scope.len())?;
        charge(1)?;
    } else {
        charge(failure.len())?;
        charge(1)?;
        for (location, message) in issues {
            charge(5)?; // '- ' + ': ' + LF
            charge(location.len())?;
            charge(message.len())?;
        }
    }
    if let Some(report) = lab_report {
        stdout
            .write_all(report)
            .map_err(|_| "foundation stdout failed")?;
    }
    if !issues.is_empty() {
        writeln!(stderr, "{failure}").map_err(|_| "foundation stderr failed")?;
        for (location, message) in issues {
            writeln!(stderr, "- {location}: {message}").map_err(|_| "foundation stderr failed")?;
        }
        return Ok(1);
    }
    if args.selected_lab.is_none() {
        writeln!(
            stdout,
            "[ok] validated source-witness evidence spine ({posture})"
        )
        .map_err(|_| "foundation stdout failed")?;
    }
    writeln!(stdout, "{scope}").map_err(|_| "foundation stdout failed")?;
    Ok(0)
}

/// Count the exact maintained framing with the same formatter, without
/// emitting or retaining output bytes. The caller admits this future output
/// upper bound before its final custody window; it is not observed stdout.
pub(crate) fn measure_result(
    args: &FoundationArguments,
    lab_report: Option<&[u8]>,
    issues: &[(String, String)],
    max_output_bytes: usize,
    deadline: std::time::Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(i32, usize), &'static str> {
    struct Count<'a> {
        bytes: &'a std::cell::Cell<usize>,
        max: usize,
        deadline: std::time::Instant,
        cancelled: &'a std::sync::atomic::AtomicBool,
    }
    impl Write for Count<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.cancelled.load(std::sync::atomic::Ordering::Relaxed)
                || std::time::Instant::now() >= self.deadline
            {
                return Err(std::io::Error::other("foundation output count stopped"));
            }
            let total = self
                .bytes
                .get()
                .checked_add(bytes.len())
                .filter(|total| *total <= self.max)
                .ok_or_else(|| std::io::Error::other("foundation output byte limit"))?;
            self.bytes.set(total);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let bytes = std::cell::Cell::new(0);
    let code = write_result(
        args,
        lab_report,
        issues,
        max_output_bytes,
        &mut Count {
            bytes: &bytes,
            max: max_output_bytes,
            deadline,
            cancelled,
        },
        &mut Count {
            bytes: &bytes,
            max: max_output_bytes,
            deadline,
            cancelled,
        },
    )?;
    Ok((code, bytes.get()))
}

#[cfg(test)]
mod validation_profile_tests {
    use super::*;
    #[test]
    fn explicit_catalog_profiles_preserve_full_default_and_refuse_unknown_or_repeated_selection() {
        let parse = |args: &[&str]| {
            parse_arguments(
                &args.iter().map(OsString::from).collect::<Vec<_>>(),
                Some(Path::new("/selected")),
            )
        };
        let omitted = parse(&[]).unwrap();
        assert_eq!(
            omitted.validation_profile.scope,
            SourceFoundationDefaultRuleScope::FullAudit
        );
        assert!(!omitted.validation_profile_explicit);
        let selected = parse(&["--validation-profile", "selected-source-closure"]).unwrap();
        assert_eq!(
            selected.validation_profile.scope,
            SourceFoundationDefaultRuleScope::SelectedSourceClosure
        );
        assert!(selected.validation_profile_explicit);
        assert!(selected.selected_lab.is_none());
        let records = parse(&[
            "--validation-profile=selected-record-closure",
            "--record-selection-manifest",
            "/selection.json",
        ])
        .unwrap();
        assert_eq!(
            records.validation_profile.scope,
            SourceFoundationDefaultRuleScope::SelectedRecordClosure
        );
        assert_eq!(
            records.record_selection_manifest.as_deref(),
            Some(Path::new("/selection.json"))
        );
        for args in [
            vec!["--validation-profile=selected-record-closure"],
            vec!["--record-selection-manifest=/selection.json"],
            vec![
                "--validation-profile=selected-source-closure",
                "--record-selection-manifest=/selection.json",
            ],
            vec![
                "--validation-profile=selected-record-closure",
                "--record-selection-manifest=relative.json",
            ],
            vec![
                "--validation-profile=selected-record-closure",
                "--record-selection-manifest=/selection.json",
                "--record-selection-manifest=/other.json",
            ],
        ] {
            assert!(parse(&args).is_err());
        }

        let generated = parse(&[
            "--validation-profile=selected-generated-record-closure",
            "--record-selection-manifest=/selection.json",
            "--indexed-input-root=/indexed",
        ])
        .unwrap();
        assert_eq!(
            generated.validation_profile.scope,
            SourceFoundationDefaultRuleScope::SelectedGeneratedRecordClosure
        );
        assert_eq!(
            generated.indexed_input_root.as_deref(),
            Some(Path::new("/indexed"))
        );
        for args in [
            vec![
                "--validation-profile=selected-generated-record-closure",
                "--record-selection-manifest=/selection.json",
            ],
            vec![
                "--validation-profile=selected-generated-record-closure",
                "--indexed-input-root=/indexed",
            ],
            vec![
                "--validation-profile=selected-record-closure",
                "--record-selection-manifest=/selection.json",
                "--indexed-input-root=/indexed",
            ],
            vec![
                "--validation-profile=selected-generated-record-closure",
                "--record-selection-manifest=/selection.json",
                "--indexed-input-root=relative",
            ],
            vec![
                "--validation-profile=selected-generated-record-closure",
                "--record-selection-manifest=/selection.json",
                "--indexed-input-root=/indexed",
                "--indexed-input-root=/other",
            ],
        ] {
            assert!(parse(&args).is_err());
        }
        for args in [
            vec!["--validation-profile=unknown"],
            vec![
                "--validation-profile=full-audit",
                "--validation-profile=full-audit",
            ],
            vec![
                "--validation-profile=selected-source-closure",
                "--source-anchor-v2-lab-only",
            ],
        ] {
            assert!(parse(&args).is_err());
        }
    }
    #[test]
    fn profile_ids_are_catalog_owned_and_exact_declaration_bytes_bound() {
        let original = select_validation_profile(Some("selected-source-closure")).unwrap();
        let alternate: &'static [u8] = Box::leak(
            String::from_utf8(VALIDATION_PROFILE_BYTES.to_vec())
                .unwrap()
                .replace("selected-source-closure", "member-closure-v1")
                .into_bytes()
                .into_boxed_slice(),
        );
        let renamed =
            select_validation_profile_from_bytes(alternate, Some("member-closure-v1")).unwrap();
        assert_eq!(renamed.scope, original.scope);
        assert_ne!(renamed.declaration_sha256, original.declaration_sha256);
        assert!(select_validation_profile_from_bytes(alternate, Some(original.id)).is_err());
        let unsupported: &'static [u8] = Box::leak(
            String::from_utf8(VALIDATION_PROFILE_BYTES.to_vec())
                .unwrap()
                .replace("selected_source_closure", "skip_all")
                .into_bytes()
                .into_boxed_slice(),
        );
        assert!(select_validation_profile_from_bytes(unsupported, None).is_err());
    }
}
