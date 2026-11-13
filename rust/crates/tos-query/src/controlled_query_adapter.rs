//! Controlled knowledge header and catalog responses.

use crate::{
    knowledge_binding::BoundCmpKnowledge,
    search::{SearchError, SearchErrorCode},
};
use tos_compiler::{ControlledKnowledgeModel, Error as CompilerError};
use tos_foundation::{JsonValue, OwnedState};

fn error(code: SearchErrorCode, message: &'static str) -> SearchError {
    SearchError { code, message }
}

pub(crate) fn compiler_query_error(reason: CompilerError) -> SearchError {
    match reason {
        // These owner reasons are static code-owned literals, never source
        // values or parser text. Preserve the cause through the public adapter.
        CompilerError::Budget(reason) => error(SearchErrorCode::BudgetExceeded, reason),
        CompilerError::SqliteVmBudget { .. } => error(
            SearchErrorCode::BudgetExceeded,
            "controlled query owner budget exceeded",
        ),
        CompilerError::Invalid(reason) => error(SearchErrorCode::CorruptSelectedCarrier, reason),
        CompilerError::Sql(rusqlite::Error::SqliteFailure(failure, _))
            if failure.code == rusqlite::ErrorCode::OperationInterrupted =>
        {
            error(
                SearchErrorCode::BudgetExceeded,
                "controlled selected query VM budget exceeded",
            )
        }
        _ => error(
            SearchErrorCode::Unavailable,
            "controlled selected query owner refused",
        ),
    }
}

/// Header-only selected disclosure with the maintained Inspect scope and lease.
/// Body and lease stay inside synchronous delivery under the compiler state hold.
pub fn execute_controlled_knowledge_header_response<'hold, A>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    budget: crate::InspectBudget,
    deliver: impl FnOnce(&[u8]) -> Result<(), SearchError>,
) -> Result<(), SearchError>
where
    A: crate::InspectCurrentAuthority<'hold> + ?Sized,
{
    if budget.max_rows == 0 || budget.max_response_bytes == 0 {
        return Err(error(
            SearchErrorCode::BudgetExceeded,
            "controlled header budget",
        ));
    }
    model
        .check_query_open_vm_admission(budget.max_open_vm_steps)
        .map_err(compiler_query_error)?;
    bound.check_controlled_model(model)?;
    let forecast = authority.disclosure_metadata_state_upper_bound()?;
    let mut outcome = Ok(());
    let frame = std::mem::size_of_val(&deliver)
        .checked_add(std::mem::size_of::<crate::search::CurrentPolicyBinding>())
        .and_then(|n| n.checked_add(std::mem::size_of::<crate::IndexedDisclosureScope>()))
        .and_then(|n| {
            n.checked_add(
                std::mem::size_of::<Option<std::sync::Arc<dyn crate::AbortProbe>>>()
                    + std::mem::size_of::<Result<(), SearchError>>() * 3,
            )
        })
        .ok_or_else(|| {
            error(
                SearchErrorCode::BudgetExceeded,
                "controlled header frame state",
            )
        })?;
    model
        .with_owned_query_workspace(
            forecast
                .checked_add(frame)
                .ok_or(CompilerError::Budget("controlled header policy workspace"))
                .map_err(compiler_query_error)?,
            |model| {
                let policy = authority.policy_binding();
                let scope = authority.disclosure_scope();
                let actual = policy
                    .retained_state_bytes()
                    .map_err(|_| CompilerError::Budget("controlled header policy state"))?
                    .checked_add(
                        crate::knowledge_inspect::scope_owned_state(&scope)
                            .map_err(|_| CompilerError::Budget("controlled header scope state"))?,
                    )
                    .ok_or(CompilerError::Budget("controlled header metadata state"))?;
                if actual > forecast {
                    return Err(CompilerError::Budget("controlled header metadata forecast"));
                }
                outcome = (|| {
                    scope.validate_for(
                        bound,
                        &policy,
                        "tos_knowledge_header",
                        "read_only_public_knowledge_header_v1",
                    )?;
                    authority.check_selected()?;
                    if let Some(proof) = bound.source_basis().managed_source() {
                        authority.authorize_managed_source_current(proof)?;
                    }
                    if let Some(proof) = bound.source_basis().managed_source_v2() {
                        authority.authorize_managed_source_v2_current(proof)?;
                    }
                    let mut delivered = Ok(());
                    model
                        .with_controlled_header(
                            budget.max_payload_bytes.min(budget.max_response_bytes),
                            budget.max_decoded_bytes,
                            budget.max_read_vm_steps,
                            budget.json,
                            |body| {
                                delivered = (|| {
                                    if let Some(reason) =
                                        authority.abort_probe().and_then(|probe| probe.reason())
                                    {
                                        return Err(error(
                                            match reason {
                                                crate::AbortReason::Cancelled => {
                                                    SearchErrorCode::Cancelled
                                                }
                                                crate::AbortReason::DeadlineExceeded => {
                                                    SearchErrorCode::DeadlineExceeded
                                                }
                                            },
                                            "controlled header aborted",
                                        ));
                                    }
                                    authority.check_selected()?;
                                    let mut lease = authority.acquire_disclosure(&scope, &[])?;
                                    lease.recheck()?;
                                    deliver(body)?;
                                    lease.recheck()
                                })();
                                Ok(())
                            },
                        )
                        .map_err(compiler_query_error)?;
                    delivered
                })();
                Ok(())
            },
        )
        .map_err(compiler_query_error)?;
    outcome
}

/// Exact catalog delivery under the original controlled model and an actual
/// Catalog lease. The same authority also supplies its existing owned metadata
/// forecast; no policy/scope clone is allocated before that admission.
pub fn execute_controlled_catalog_response<'hold, A>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    budget: crate::CatalogBudget,
    deliver: impl FnOnce(&[u8]) -> Result<(), crate::CatalogError>,
) -> Result<(), crate::CatalogError>
where
    A: crate::CatalogCurrentAuthority<'hold> + crate::InspectCurrentAuthority<'hold>,
{
    execute_controlled_catalog_payload_response(model, bound, authority, budget, |raw, _| {
        deliver(raw)
    })
}

pub(crate) fn execute_controlled_catalog_payload_response<'hold, A>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    budget: crate::CatalogBudget,
    deliver: impl FnOnce(&[u8], &JsonValue) -> Result<(), crate::CatalogError>,
) -> Result<(), crate::CatalogError>
where
    A: crate::CatalogCurrentAuthority<'hold> + crate::InspectCurrentAuthority<'hold>,
{
    use crate::knowledge_catalog::{CatalogError, CatalogErrorCode};
    use crate::{CatalogCurrentAuthority as C, InspectCurrentAuthority as I};
    let refused = || CatalogError {
        code: CatalogErrorCode::BudgetExceeded,
        message: "controlled catalog owner refused",
    };
    let compiler_error = |e| CatalogError {
        code: match e {
            CompilerError::Budget(_) | CompilerError::SqliteVmBudget { .. } => {
                CatalogErrorCode::BudgetExceeded
            }
            CompilerError::Invalid(_) => CatalogErrorCode::CorruptSelectedCarrier,
            _ => CatalogErrorCode::PolicyBindingUnavailable,
        },
        message: "controlled catalog owner refused",
    };
    model
        .check_query_open_vm_admission(budget.max_open_vm_steps)
        .map_err(compiler_error)?;
    bound
        .check_controlled_model(model)
        .map_err(|_| CatalogError {
            code: CatalogErrorCode::StaleSelection,
            message: "controlled catalog binding differs",
        })?;
    let forecast = I::disclosure_metadata_state_upper_bound(authority)
        .map_err(|_| refused())?
        .checked_add(std::mem::size_of::<crate::CatalogDisclosureScope>())
        .and_then(|n| n.checked_add(std::mem::size_of_val(&deliver)))
        .ok_or_else(refused)?;
    let mut outcome = Ok(());
    model
        .with_owned_query_workspace(forecast, |model| {
            let policy = C::policy_binding(authority);
            let scope = C::disclosure_scope(authority);
            outcome = (|| {
                scope.validate(bound, &policy)?;
                C::check_selected(authority)?;
                if let Some(proof) = bound.source_basis().managed_source() {
                    C::authorize_managed_source_current(authority, proof)?;
                }
                if let Some(proof) = bound.source_basis().managed_source_v2() {
                    C::authorize_managed_source_v2_current(authority, proof)?;
                }
                let mut delivered = Ok(());
                model
                    .with_controlled_catalog(
                        budget.max_packet_bytes,
                        budget.max_decoded_bytes,
                        budget.max_read_vm_steps,
                        budget.json,
                        |body, catalog| {
                            delivered = (|| {
                                bound
                                    .validate_catalog_identity(catalog, budget.json)
                                    .map_err(|_| CatalogError {
                                        code: CatalogErrorCode::CorruptSelectedCarrier,
                                        message: "controlled catalog identity differs",
                                    })?;
                                C::authorize_current(
                                    authority,
                                    bound.selection().catalog_packet_sha256,
                                )?;
                                C::check_selected(authority)?;
                                if let Some(reason) =
                                    C::abort_probe(authority).and_then(|probe| probe.reason())
                                {
                                    return Err(CatalogError {
                                        code: match reason {
                                            crate::AbortReason::Cancelled => {
                                                CatalogErrorCode::Cancelled
                                            }
                                            crate::AbortReason::DeadlineExceeded => {
                                                CatalogErrorCode::DeadlineExceeded
                                            }
                                        },
                                        message: "controlled catalog aborted",
                                    });
                                }
                                let mut lease = C::acquire_disclosure(
                                    authority,
                                    &scope,
                                    bound.selection().catalog_packet_sha256,
                                )?;
                                lease.recheck()?;
                                deliver(body, catalog)?;
                                lease.recheck()
                            })();
                            Ok(())
                        },
                    )
                    .map_err(compiler_error)?;
                delivered
            })();
            Ok(())
        })
        .map_err(compiler_error)?;
    outcome
}
