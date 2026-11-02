//! Adapters to the shared validation owner for cut and explicit-file inputs.
use crate::zarathustra_lexical::{LexicalSchema, Result};
use serde_json::{Value, json};
use std::{path::Path, sync::atomic::AtomicBool, time::Instant};
use tos_source_store::CorpusCutReader;
use tos_validation::{
    FormatProfile,
    executor::{LegacySelectedDiagnosticsLimits, schema_diagnostics},
    in_process::InputMode,
    source_cut::{CutSchemaExecutor, InProcessCutSchemaExecutor},
};

/// Read-only metadata checks over explicitly selected files. Source capture and
/// FND strict decoding match the lexical rules; schema evaluation remains with
/// tos-validation, using the same observed format profile as the cut adapter.
/// Current lexical contracts are self-contained. Missing external resources
/// fail closed in the offline backend.
pub struct TrackedLexicalSchema<'a> {
    capture: crate::zarathustra_lexical::LexicalCapture<'a>,
    probes: std::collections::BTreeMap<String, (String, tos_validation::SchemaBackendProbe)>,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    units: usize,
    raw_bytes: usize,
}
impl<'a> TrackedLexicalSchema<'a> {
    pub fn new(root: &Path, deadline: Instant, cancelled: &'a AtomicBool) -> Result<Self> {
        Ok(Self {
            capture: crate::zarathustra_lexical::LexicalCapture::new(
                root,
                crate::zarathustra_lexical::LexicalLimits::maintained(),
            )?,
            probes: Default::default(),
            deadline,
            cancelled,
            units: 0,
            raw_bytes: 0,
        })
    }
    pub fn finish(&self) -> Result<Value> {
        crate::zarathustra_lexical::active(self.deadline, self.cancelled)?;
        self.capture.revalidate()?;
        Ok(json!({
            "engine": "tos-validation::SchemaBackendProbe",
            "format_profile": FormatProfile::LegacyPythonObserved20260923.id(),
            "instances": self.units,
            "instance_bytes": self.raw_bytes,
            "schema_inputs": self.capture.member_digests(),
        }))
    }
}
impl LexicalSchema for TrackedLexicalSchema<'_> {
    fn check(&mut self, contract: &str, raw: &[u8]) -> Result<()> {
        use crate::zarathustra_lexical::{active, parse, text};
        active(self.deadline, self.cancelled)?;
        self.units = self
            .units
            .checked_add(1)
            .ok_or("lexical schema units overflow")?;
        self.raw_bytes = self
            .raw_bytes
            .checked_add(raw.len())
            .ok_or("lexical schema bytes overflow")?;
        if self.units > 1024 || self.raw_bytes > 128 * 1024 * 1024 {
            return Err("lexical schema operation budget".into());
        }
        if !self.probes.contains_key(contract) {
            if self.probes.len() >= 64 {
                return Err("lexical schema resource count".into());
            }
            let schema_raw = self.capture.read(contract)?;
            let schema = parse(
                &schema_raw,
                tos_validation::SchemaBackendProbe::MAX_RESOURCE_BYTES,
            )?;
            let uri = text(&schema, "$id")?.to_owned();
            let probe = tos_validation::SchemaBackendProbe::new(
                [tos_validation::SchemaResource {
                    uri: uri.clone(),
                    raw: schema_raw,
                }],
                FormatProfile::LegacyPythonObserved20260923,
            )
            .map_err(|e| format!("lexical schema preparation: {e:?}"))?;
            self.probes.insert(contract.into(), (uri, probe));
        }
        // This owner already admits projection instances above the generic
        // one-MiB raw-probe ceiling. Decode under its existing 64-MiB, depth and
        // visit limits before using the shared strict-value entry point.
        let instance = parse(raw, 64 * 1024 * 1024)?;
        let (uri, probe) = self
            .probes
            .get(contract)
            .ok_or("lexical schema disappeared")?;
        if !probe
            .is_valid_value(uri, &instance)
            .map_err(|e| format!("lexical schema evaluation: {e:?}"))?
        {
            return Err(format!("instance violates {contract}"));
        }
        active(self.deadline, self.cancelled)
    }
}
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LexicalSchemaLimits {
    pub max_instance_bytes: usize,
    pub max_visits: u32,
    pub parser_state_bytes: u64,
    pub conversion_state_bytes: u64,
    pub max_units: usize,
    pub max_raw_bytes: u64,
    pub max_issues: usize,
    // Accept old deployment profiles until their worker fields have been removed.
    #[serde(default)]
    pub max_wire_bytes: u64,
    #[serde(default)]
    pub max_state_bytes: usize,
    #[serde(default)]
    pub max_preparation_bytes: usize,
    #[serde(default)]
    pub max_report_bytes: usize,
    #[serde(default)]
    pub max_cpu_seconds: u64,
    #[serde(default)]
    pub address_space_bytes: u64,
    #[serde(default)]
    pub max_unit_seconds: u64,
}

pub struct LexicalSchemaExecutor<'a> {
    executor: InProcessCutSchemaExecutor<()>,
    input_mode: InputMode,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    units: u64,
    raw_bytes: u64,
    issues: u64,
    max_units: u64,
    max_raw_bytes: u64,
    max_issues: u64,
    finished: bool,
    failed: bool,
}
impl<'a> LexicalSchemaExecutor<'a> {
    pub fn new(
        cut: &CorpusCutReader,
        l: LexicalSchemaLimits,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Result<Self> {
        if l.max_instance_bytes == 0
            || l.max_instance_bytes > LegacySelectedDiagnosticsLimits::MAX_INSTANCE_BYTES
            || l.max_visits == 0
            || l.max_visits > LegacySelectedDiagnosticsLimits::MAX_VISITS
            || l.parser_state_bytes == 0
            || l.parser_state_bytes > LegacySelectedDiagnosticsLimits::MAX_STATE_BYTES
            || l.conversion_state_bytes == 0
            || l.conversion_state_bytes > LegacySelectedDiagnosticsLimits::MAX_STATE_BYTES
            || l.max_units == 0
            || l.max_units > 1024
            || l.max_raw_bytes == 0
            || l.max_issues == 0
        {
            return Err("lexical schema declaration".into());
        }
        let input_mode = InputMode::LegacyPythonObservedSelected(LegacySelectedDiagnosticsLimits {
            max_instance_bytes: l.max_instance_bytes,
            max_visits: l.max_visits,
            parser_state_bytes: l.parser_state_bytes,
            conversion_state_bytes: l.conversion_state_bytes,
        });
        let executor = InProcessCutSchemaExecutor::from_cut(
            cut,
            FormatProfile::LegacyPythonObserved20260923,
            input_mode,
            deadline,
            cancelled,
        )
        .map_err(|e| format!("lexical schema closure: {e:?}"))?;
        Ok(Self {
            executor,
            input_mode,
            deadline,
            cancelled,
            units: 0,
            raw_bytes: 0,
            issues: 0,
            max_units: l.max_units as u64,
            max_raw_bytes: l.max_raw_bytes,
            max_issues: l.max_issues as u64,
            finished: false,
            failed: false,
        })
    }
    pub fn finish(&mut self) -> Result<()> {
        if self.failed {
            return Err("lexical schema operation failed".into());
        }
        self.executor
            .finish(self.deadline, self.cancelled)
            .map_err(|e| format!("lexical schema finalization: {e:?}"))?;
        self.finished = true;
        Ok(())
    }
    pub fn receipt(&self) -> Result<Value> {
        if !self.finished {
            return Err("lexical schema executor not finalized".into());
        }
        let binding = self.executor.binding();
        Ok(json!({
            "profile": binding.engine,
            "format_profile": binding.schema_profile.id(),
            "schema_set_sha256": binding.schema_set_sha256.to_hex(),
            "input_mode": self.input_mode,
            "instances": self.units,
            "instance_bytes": self.raw_bytes,
        }))
    }
}
impl LexicalSchema for LexicalSchemaExecutor<'_> {
    fn check(&mut self, contract: &str, raw: &[u8]) -> Result<()> {
        if self.finished || self.failed {
            return Err("lexical schema operation is not active".into());
        }
        let result = (|| {
            self.units = self
                .units
                .checked_add(1)
                .filter(|n| *n <= self.max_units)
                .ok_or("lexical schema units")?;
            self.raw_bytes = self
                .raw_bytes
                .checked_add(raw.len() as u64)
                .filter(|n| *n <= self.max_raw_bytes)
                .ok_or("lexical schema bytes")?;
            let d = self
                .executor
                .diagnostic_for(
                    &format!("lexical-private-candidate-unit-{}", self.units),
                    contract,
                    raw,
                    schema_diagnostics::Caps::CURRENT,
                    self.deadline,
                    self.cancelled,
                )
                .map_err(|e| format!("lexical schema execution refused: {e:?}"))?;
            self.issues = self
                .issues
                .checked_add(d.report.total_issue_count)
                .filter(|n| *n <= self.max_issues)
                .ok_or("lexical schema issue limit")?;
            match d.report.status {
                schema_diagnostics::Status::Valid => Ok(()),
                schema_diagnostics::Status::Invalid => {
                    Err(format!("lexical schema invalid: {contract}"))
                }
                _ => Err(format!("lexical schema incomplete/unsupported: {contract}")),
            }
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}
