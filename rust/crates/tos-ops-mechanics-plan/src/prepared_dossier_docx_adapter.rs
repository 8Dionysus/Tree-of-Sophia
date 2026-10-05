//! Adapter draft binding readiness to the native owner DOCX parser.
//!
//! This file assumes the source peer's public path:
//! `tos_compiler::source_philosophy_dossier_docx`.

use serde_json::Value;
use tos_compiler::source_philosophy_dossier_docx::{parse_docx, validate_identity_and_headers};

use crate::prepared_dossier_readiness::{DocxContentIssue, PreparedDossierContentValidator};

#[derive(Clone, Copy, Debug, Default)]
pub struct NativePreparedDossierContentValidator;

impl PreparedDossierContentValidator for NativePreparedDossierContentValidator {
    fn validate(
        &self,
        table_id: &str,
        dossier_id: &str,
        raw_docx: &[u8],
        master_row: &Value,
        route: Option<&Value>,
        blocked: Option<&Value>,
        work_tick: &mut dyn FnMut(u64) -> Result<(), String>,
    ) -> Result<(), Vec<DocxContentIssue>> {
        self.validate_and_retain(
            table_id, dossier_id, raw_docx, master_row, route, blocked, work_tick,
        )
        .map(|_| ())
    }

    fn validate_and_retain(
        &self,
        table_id: &str,
        dossier_id: &str,
        raw_docx: &[u8],
        master_row: &Value,
        route: Option<&Value>,
        blocked: Option<&Value>,
        work_tick: &mut dyn FnMut(u64) -> Result<(), String>,
    ) -> Result<
        Option<tos_compiler::source_philosophy_dossier_docx::DocxDocument>,
        Vec<DocxContentIssue>,
    > {
        let docx = parse_docx(raw_docx, work_tick).map_err(|message| {
            vec![DocxContentIssue {
                code: "docx_parse_error".to_owned(),
                message,
                blocking: true,
            }]
        })?;
        validate_identity_and_headers(
            &docx, table_id, dossier_id, master_row, route, blocked, work_tick,
        )
        .map(|_validation| Some(docx))
        .map_err(|issues| {
            issues
                .into_iter()
                .map(|issue| DocxContentIssue {
                    code: issue.code,
                    message: issue.message,
                    blocking: issue.blocking,
                })
                .collect()
        })
    }
}
