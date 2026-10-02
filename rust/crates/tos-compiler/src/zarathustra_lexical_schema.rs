//! Adapter to the existing FND diagnostics-v2 executor; no alternate schema law.
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
