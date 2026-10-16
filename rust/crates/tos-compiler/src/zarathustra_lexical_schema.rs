//! Adapters to the shared validation owner for cut and explicit-file inputs.
use crate::zarathustra_lexical::{LexicalSchema, Result};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use tos_foundation::Digest256;
use tos_source_store::CorpusCutReader;
use tos_validation::{
    FormatProfile,
    executor::{
        BatchBudget, BatchStreamBudget, ExactWorkerIdentity, ExecutorBudget,
        SharedSchemaWorkerQuota,
    },
    source_cut::{
        CutSchemaDiagnosticsLimits, CutSchemaExecutor, CutWorkerLimits, CutWorkerSchemaExecutor,
        LegacySelectedDiagnosticsLimits, cut_schema_preparation_state_upper_bound,
    },
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
    pub max_wire_bytes: u64,
    pub max_state_bytes: usize,
    pub max_preparation_bytes: usize,
    pub max_report_bytes: usize,
    pub max_issues: usize,
    pub max_cpu_seconds: u64,
    pub address_space_bytes: u64,
    pub max_unit_seconds: u64,
}
pub struct LexicalSchemaExecutor<'a> {
    executor: CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    controller_bytes: usize,
    preparation_bytes: usize,
    setup_state_bytes: usize,
    units: u64,
    max_units: u64,
    finished: bool,
}
impl<'a> LexicalSchemaExecutor<'a> {
    pub fn new(
        cut: &CorpusCutReader,
        worker: &Path,
        worker_sha: Digest256,
        l: LexicalSchemaLimits,
        setup_state_bytes: usize,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Result<Self> {
        if l.max_instance_bytes == 0
            || l.max_instance_bytes > 32 * 1024 * 1024
            || l.max_visits == 0
            || l.parser_state_bytes == 0
            || l.conversion_state_bytes == 0
            || l.max_units == 0
            || l.max_units > 1024
            || l.max_state_bytes == 0
            || l.max_report_bytes == 0
            || l.max_issues == 0
            || l.max_unit_seconds == 0
        {
            return Err("lexical schema declaration".into());
        }
        let wall = deadline
            .checked_duration_since(Instant::now())
            .ok_or("lexical schema deadline")?;
        let unit = Duration::from_secs(l.max_unit_seconds).min(wall);
        // The existing scalar executor admits at most 60 CPU seconds per child.
        // Preserve the separately declared cumulative quota across all children.
        let child_cpu_seconds = l.max_cpu_seconds.min(60);
        let budget = ExecutorBudget {
            execution_wall: unit,
            cleanup_grace: Duration::from_millis(200),
            cpu_seconds: child_cpu_seconds,
            address_space_bytes: l.address_space_bytes,
        };
        let preparation_bytes =
            cut_schema_preparation_state_upper_bound(cut, worker, deadline, cancelled)
                .map_err(|e| format!("lexical preparation bound: {e:?}"))?;
        let remaining_preparation = l
            .max_preparation_bytes
            .checked_sub(setup_state_bytes)
            .ok_or("lexical setup exceeds declared preparation state")?;
        if preparation_bytes > remaining_preparation {
            return Err("lexical schema preparation exceeds declared state".into());
        }
        let mut executor = CutWorkerSchemaExecutor::from_cut(
            cut,
            FormatProfile::LegacyPythonObserved20260923,
            ExactWorkerIdentity {
                absolute_path: worker.into(),
                sha256: worker_sha,
            },
            budget,
            CutWorkerLimits {
                max_receipts: l.max_units,
                max_receipt_bytes: l.max_report_bytes,
            },
            deadline,
            cancelled,
        )
        .map_err(|e| format!("lexical schema closure: {e:?}"))?;
        // Diagnostics are admitted against this selected whole operation envelope.
        executor
            .set_operation_budget(BatchStreamBudget {
                batch: BatchBudget {
                    total_execution_wall: wall,
                    startup_wall: unit,
                    per_unit_wall: unit,
                    cleanup_grace: Duration::from_millis(200),
                    cpu_seconds: child_cpu_seconds,
                    address_space_bytes: l.address_space_bytes,
                    max_units: 1,
                    max_total_raw_bytes: l.max_instance_bytes,
                },
                max_chunks: l.max_units as u64,
                max_total_units: l.max_units as u64,
                max_total_raw_bytes: l.max_raw_bytes,
                total_execution_wall: wall,
                operation_cpu_seconds: l.max_cpu_seconds,
                operation_address_space_bytes: l.address_space_bytes,
                max_total_wire_bytes: l.max_wire_bytes,
                max_distinct_selectors: l.max_units,
            })
            .map_err(|e| format!("lexical operation budget: {e:?}"))?;
        executor
            .enable_diagnostics_v2(CutSchemaDiagnosticsLimits {
                max_total_issues: l.max_issues,
                max_total_report_bytes: l.max_report_bytes,
                max_total_state_bytes: l.max_state_bytes,
            })
            .map_err(|e| format!("lexical diagnostics: {e:?}"))?;
        executor
            .set_diagnostics_v2_legacy_selected_limits(LegacySelectedDiagnosticsLimits {
                max_instance_bytes: l.max_instance_bytes,
                max_visits: l.max_visits,
                parser_state_bytes: l.parser_state_bytes,
                conversion_state_bytes: l.conversion_state_bytes,
            })
            .map_err(|e| format!("lexical instance profile: {e:?}"))?;
        let quota = SharedSchemaWorkerQuota::new(
            l.max_cpu_seconds
                .checked_mul(1_000_000)
                .ok_or("lexical CPU overflow")?,
            l.max_wire_bytes,
            l.max_units as u64,
        )
        .map_err(|e| format!("lexical quota: {e:?}"))?;
        executor
            .set_shared_schema_worker_quota(quota)
            .map_err(|e| format!("lexical shared quota: {e:?}"))?;
        let controller_bytes = executor
            .diagnostics_v2_controller_state_upper_bound(l.max_instance_bytes, 4096)
            .map_err(|e| format!("lexical controller bound: {e:?}"))?;
        if controller_bytes > l.max_state_bytes {
            return Err("lexical schema controller exceeds declared state".into());
        }
        executor
            .set_diagnostics_v2_controller_state_cap(controller_bytes)
            .map_err(|e| format!("lexical controller cap: {e:?}"))?;
        Ok(Self {
            executor,
            deadline,
            cancelled,
            controller_bytes,
            preparation_bytes,
            setup_state_bytes,
            units: 0,
            max_units: l.max_units as u64,
            finished: false,
        })
    }
    pub fn finish(&mut self) -> Result<()> {
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
        let c = self
            .executor
            .diagnostics_v2_cumulative_cost()
            .map_err(|e| format!("lexical schema cost incomplete: {e:?}"))?;
        Ok(
            json!({"profile":"tos_schema_diagnostics_v2","controller_state_upper_bound":self.controller_bytes,"preparation_state_upper_bound":self.preparation_bytes,"already_live_setup_state_upper_bound":self.setup_state_bytes,"completed_exchanges":c.completed_exchanges(),"schema_resource_bytes":c.schema_resource_bytes(),"request_bytes":c.request_bytes(),"response_bytes":c.response_bytes(),"worker_cpu_micros":c.worker_cpu_micros()}),
        )
    }
}
impl LexicalSchema for LexicalSchemaExecutor<'_> {
    fn check(&mut self, contract: &str, raw: &[u8]) -> Result<()> {
        if self.finished {
            return Err("lexical schema finalized".into());
        }
        self.units = self
            .units
            .checked_add(1)
            .filter(|n| *n <= self.max_units)
            .ok_or("lexical schema units")?;
        let d = self
            .executor
            .check_diagnostics_v2_legacy_selected(
                &format!("lexical-private-candidate-unit-{}", self.units),
                raw,
                contract,
                self.deadline,
                self.cancelled,
            )
            .map_err(|e| format!("lexical schema execution refused: {e:?}"))?;
        if d.is_valid() {
            Ok(())
        } else if d.is_invalid() {
            Err(format!("lexical schema invalid: {contract}"))
        } else {
            Err(format!("lexical schema incomplete/unsupported: {contract}"))
        }
    }
}
