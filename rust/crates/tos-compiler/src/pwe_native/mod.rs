//! PWE-owned supplied-source consumers. No writer, issuer, merge or completeness grant.
//!
//! Callers retain authenticated source selection, parser allocations and current
//! authority leases. This module checks supplied exact content and the domain
//! mechanics; neither a JSON status nor a K0 wrapper supplies those leases.
mod basis;
mod identity;
mod producer;
mod qualification;
mod source;
#[cfg(test)]
mod tests;

pub use basis::{Error, Limits, PweRef, Result, Session};
pub use identity::{
    AccountObservation, AttributionAnswer, AttributionObservation, AttributionQuestion, Conflict,
    ConsumerOwner, EdgeDisposition, IdentityInput, IdentityView, JointCarrier, JointCarrierOwner,
    JointWorld, Query, SameEdge, ViewBasis, ViewLayer,
};
pub use producer::CheckedPreservedCandidate;
pub use qualification::{BasisSlot, DispatchRow, Kind, PreparedAccount, QualifiedAccount};
pub use source::{CheckedMetadata, RegistryDescriptor, ResolvedBasis, ResolvedSlot, SuppliedExact};

pub const SINGLE_VALUE_SCHEMA: &str = "urn:tos:pwe:application:values:1.1.0";
pub const CONTAINER_SCHEMA: &str = "urn:tos:pwe:application:preserved-candidate:1.0.0";
pub const WRAPPER_SCHEMA: &str = "urn:tos:pwe:application:prepared-values-record:1.0.0";
