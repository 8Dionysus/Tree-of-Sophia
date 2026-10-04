//! Operation-local spill port for packed authenticated-tree cold closure.
//!
//! The tree owner remains responsible for reading and validating physical
//! packs. This port can retain and enumerate observed content addresses, but
//! it cannot supply rows or a coverage receipt.

use tos_foundation::Digest256;

use crate::error::Result;

/// Bounded operation-local inventory used by a full V2 cold verifier.
///
/// Implementations are integrity-sensitive. They must bind themselves to the
/// exact held operation, selected closure, and physical store. Every observed
/// digest must be retained exactly once. `verify_pending` must enumerate every
/// observed digest that has not yet passed the supplied physical verifier,
/// strictly in binary digest order, and mark it verified only after the
/// callback returns success. It must refuse cancellation, deadline, I/O,
/// scratch, and row-limit failures without claiming completion. A caller must
/// not reuse an implementation across different physical store instances,
/// even when copied store metadata and root commitments are equal.
///
/// This is unsafe because the segment-store crate cannot independently prove
/// that an external spill enumerated every row. Implementations must preserve
/// the contract above; lying implementations can omit physical objects from a
/// cold-closure audit.
pub unsafe trait AuthenticatedTreePackSetV2 {
    /// Verify that this spill belongs to the exact selected store closure.
    fn check_binding(
        &self,
        physical_root: (u64, u64),
        store_id: [u8; 16],
        domain_digest: Digest256,
        closure_binding: Digest256,
    ) -> Result<()>;

    /// Retain one digest discovered by the physical tree traversal.
    fn observe(&self, digest: Digest256) -> Result<()>;

    /// Physically verify every distinct pending digest in binary order.
    /// Implementations may mark a row complete only after `verify` succeeds.
    fn verify_pending(&self, verify: &mut dyn FnMut(Digest256) -> Result<()>) -> Result<()>;
}
