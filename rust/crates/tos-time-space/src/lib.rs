//! Source-pinned time/space domain algebra. No resolver, grant, calendar fallback,
//! registry issuance, persistence, semantic admission or geometry engine is hidden here.
//! Common K0 structural checks do not replace these domain preconditions.
mod binding;
mod budget;
mod interval;
mod order;
mod rational;
mod spatial;
mod temporal_paths;

pub use binding::*;
pub use budget::{DomainError, DomainLimits};
pub use interval::*;
pub use order::*;
pub use rational::Rational;
pub use spatial::*;
pub use temporal_paths::*;
