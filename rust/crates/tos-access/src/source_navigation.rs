//! Native selected PostgreSQL source-navigation transport adapter.
//!
//! Requests carry only the source node and bounded traversal parameters. CMD,
//! policy, the retained model, store, and PostgreSQL bindings come from the
//! explicitly selected protected owner configuration.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use tos_query::{AbortProbe, Budget, DisclosureLease, SourceDescendRequest};

use crate::common::SourceDescendDelivery;
use crate::{AccessError, AccessErrorCode, AccessExecutor, AccessProfile, Params, PreparedPacket};

#[derive(Clone)]
pub struct SelectedPgSourceNavigation {
    selection_path: PathBuf,
    profile: AccessProfile,
}

impl SelectedPgSourceNavigation {
    pub fn new(selection_path: &Path, profile: AccessProfile) -> Result<Self, AccessError> {
        if !selection_path.is_absolute() {
            return Err(AccessError::new(
                AccessErrorCode::InvalidRequest,
                "source-navigation selection requires an absolute path",
            ));
        }
        tos_command::source_navigation_cli::SelectedSourceNavigation::new(selection_path).map_err(
            |_| {
                AccessError::new(
                    AccessErrorCode::InvalidRequest,
                    "source-navigation selection path is invalid",
                )
            },
        )?;
        Ok(Self {
            selection_path: selection_path.to_owned(),
            profile,
        })
    }

    pub(crate) fn deliver(
        &self,
        params: Params,
        probe: Arc<dyn AbortProbe>,
        delivery: &mut SourceDescendDelivery<'_>,
    ) -> Result<(), AccessError> {
        crate::knowledge::check_abort(&probe)?;
        if params.node_id.len() > 4_096 {
            return Err(AccessError::new(
                AccessErrorCode::InvalidRequest,
                "source-navigation node identifier exceeds byte budget",
            ));
        }
        let now = Instant::now();
        let timeout = self
            .profile
            .query_timeout
            .unwrap_or(Duration::from_secs(15))
            .min(Duration::from_secs(60));
        let deadline = now.checked_add(timeout).ok_or_else(|| {
            AccessError::new(
                AccessErrorCode::DeadlineExceeded,
                "source-navigation deadline unavailable",
            )
        })?;
        let request = SourceDescendRequest {
            node_id: params.node_id,
            max_depth: params.max_depth,
            limit: params.limit,
            at_least_commit_seq: params.at_least_commit_seq,
        };
        let budget = Budget {
            max_probes: 100_000,
            max_rows: 10_000,
            max_bytes: 16 * 1024 * 1024,
            max_cpu_steps: 500_000,
            max_edges: 2_400,
            max_response_bytes: self.profile.max_response_bytes,
            max_request_bytes: self.profile.max_request_bytes,
            page_rows: 64,
            json: self.profile.json_limits(),
        };
        let selected =
            tos_command::source_navigation_cli::SelectedSourceNavigation::new(&self.selection_path)
                .map_err(|_| {
                    AccessError::new(
                        AccessErrorCode::InvalidRequest,
                        "source-navigation selection path is invalid",
                    )
                })?;
        match tos_command::source_navigation_cli::with_selected_source_navigation(
            &selected,
            request,
            budget,
            deadline,
            Arc::clone(&probe),
            |result| {
                let result = result
                    .map(|packet| {
                        let (body, lease) = packet.into_parts();
                        PreparedPacket {
                            body,
                            fence: Box::new(SourceNavigationFence(lease)),
                        }
                    })
                    .map_err(AccessError::from);
                delivery(result)
            },
        ) {
            Ok(tos_command::source_navigation_cli::SelectedSourceNavigationRead::Ready(Ok(()))) => {
                Ok(())
            }
            Ok(tos_command::source_navigation_cli::SelectedSourceNavigationRead::Ready(Err(
                error,
            ))) => Err(error),
            Ok(tos_command::source_navigation_cli::SelectedSourceNavigationRead::NotReady) => {
                delivery(Err(AccessError::new(
                    AccessErrorCode::PublicationPending,
                    "selected source-navigation consumer is pending",
                )))
            }
            Err(error) => delivery(Err(map_selection_error(error))),
        }
    }
}

struct SourceNavigationFence<'hold>(Box<dyn DisclosureLease + 'hold>);

impl crate::DisclosureFence for SourceNavigationFence<'_> {
    fn recheck(&mut self) -> Result<(), AccessError> {
        self.0.recheck().map_err(Into::into)
    }
}

fn map_selection_error(
    error: tos_command::source_navigation_cli::SourceNavigationSelectionError,
) -> AccessError {
    use tos_command::{
        DurableError, source_command::SourceCommandError,
        source_managed_selection::ManagedSelectionError,
        source_navigation_cli::SourceNavigationSelectionError,
    };
    match error {
        SourceNavigationSelectionError::Invalid(_) => AccessError::new(
            AccessErrorCode::Unavailable,
            "selected source-navigation owner is unavailable",
        ),
        SourceNavigationSelectionError::Managed(ManagedSelectionError::Durable(
            DurableError::Invalid(_),
        ))
        | SourceNavigationSelectionError::Managed(ManagedSelectionError::Source(
            SourceCommandError::Invalid(_),
        )) => AccessError::new(
            AccessErrorCode::InvalidRequest,
            "selected source-navigation owner configuration is invalid",
        ),
        SourceNavigationSelectionError::Managed(ManagedSelectionError::Durable(
            DurableError::Conflict(_),
        ))
        | SourceNavigationSelectionError::Managed(ManagedSelectionError::Source(
            SourceCommandError::Conflict(_),
        )) => AccessError::new(
            AccessErrorCode::StaleSelection,
            "selected source-navigation owner changed",
        ),
        SourceNavigationSelectionError::Managed(ManagedSelectionError::Durable(
            DurableError::Corrupt(_) | DurableError::Storage(_),
        ))
        | SourceNavigationSelectionError::Managed(ManagedSelectionError::Compiler(_)) => {
            AccessError::new(
                AccessErrorCode::CorruptSelectedCarrier,
                "selected source-navigation carrier is corrupt",
            )
        }
        SourceNavigationSelectionError::Managed(ManagedSelectionError::Source(
            SourceCommandError::Denied(_) | SourceCommandError::DeniedWithReason(_),
        )) => AccessError::new(
            AccessErrorCode::PolicyDenied,
            "selected source-navigation owner denied access",
        ),
        SourceNavigationSelectionError::Managed(_) => AccessError::new(
            AccessErrorCode::Unavailable,
            "selected source-navigation owner is unavailable",
        ),
    }
}

/// Standalone source-navigation selection for the ordinary native CLI/HTTP/MCP
/// routes when no managed release is selected.
pub struct SourceNavigationOnlyExecutor {
    selected: SelectedPgSourceNavigation,
}

impl SourceNavigationOnlyExecutor {
    pub fn new(selected: SelectedPgSourceNavigation) -> Self {
        Self { selected }
    }
}

impl AccessExecutor for SourceNavigationOnlyExecutor {
    fn source_descend_available(&self) -> bool {
        true
    }

    fn source_descend(
        &self,
        _: Params,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        Err(AccessError::new(
            AccessErrorCode::Unavailable,
            "source navigation requires scoped delivery",
        ))
    }

    fn source_descend_with_delivery(
        &self,
        request: Params,
        probe: Arc<dyn AbortProbe>,
        delivery: &mut SourceDescendDelivery<'_>,
    ) -> Result<(), AccessError> {
        self.selected.deliver(request, probe, delivery)
    }
}
