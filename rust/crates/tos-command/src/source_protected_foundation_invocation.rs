//! Host-side custody of one explicitly selected foundation invocation.
//!
//! This type does not issue a stage capability or run an invocation. The stage
//! launcher must bind its eventual transport to this exact held selection.

use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{active, protected_configuration_parents};
use crate::source_text_owner::{
    HeldOwnerFile, normalized_absolute, read_absolute, select_held_file, verify_held_file_identity,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::Digest256;

const INVOCATION_BYTES: usize = 1_048_576;

/// Retains the original named file and its private parent in the host namespace.
/// The initial and terminal reads both revisit the original protected path.
pub struct ProtectedFoundationInvocation {
    path: PathBuf,
    uid: u32,
    held: HeldOwnerFile,
    raw: Vec<u8>,
    phase: Phase,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Selected,
    InitialRead,
    Finished,
}

impl ProtectedFoundationInvocation {
    /// Selection occurs before the private-stage user namespace is entered.
    pub fn select(
        path: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        active(deadline, cancelled)?;
        let selected = path.to_str().ok_or(SourceCommandError::Invalid(
            "protected invocation path UTF-8",
        ))?;
        let path = normalized_absolute(selected)?;
        let uid = rustix::process::getuid().as_raw();
        if rustix::process::geteuid().as_raw() != uid {
            return Err(SourceCommandError::Denied(
                "foundation setuid invocation refused",
            ));
        }
        let (held, raw) =
            select_held_file(&path, uid, true, INVOCATION_BYTES, deadline, cancelled)?;
        if raw.is_empty() {
            return Err(SourceCommandError::Invalid("empty foundation invocation"));
        }
        Ok(Self {
            path,
            uid,
            held,
            raw,
            phase: Phase::Selected,
        })
    }

    /// Exact path, selected bytes and digest are metadata for the typed issuer.
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn digest(&self) -> Digest256 {
        Digest256::of_bytes(&self.raw)
    }

    fn current(&self, deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
        active(deadline, cancelled)?;
        if rustix::process::getuid().as_raw() != self.uid
            || rustix::process::geteuid().as_raw() != self.uid
        {
            return Err(SourceCommandError::Denied(
                "foundation invocation account changed",
            ));
        }
        // HeldOwnerFile rechecks its parent and file; this additional walk is
        // essential because ancestors above that parent may have changed.
        protected_configuration_parents(&self.path, self.uid)?;
        verify_held_file_identity(&self.held, self.uid, deadline, cancelled)?;
        let now = read_absolute(
            &self.path,
            self.uid,
            true,
            INVOCATION_BYTES,
            deadline,
            cancelled,
        )?;
        verify_held_file_identity(&self.held, self.uid, deadline, cancelled)?;
        if now != self.raw {
            return Err(SourceCommandError::Conflict(
                "foundation invocation changed",
            ));
        }
        Ok(())
    }

    /// Serve the only permitted initial read after host-side currentness checks.
    pub fn initial_read(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        if self.phase != Phase::Selected {
            return Err(SourceCommandError::Denied(
                "foundation invocation custody sequence",
            ));
        }
        self.current(deadline, cancelled)?;
        self.phase = Phase::InitialRead;
        Ok(self.raw.clone())
    }

    /// Serve the only permitted terminal check; no source path is accepted.
    pub fn verify_before_source_eof(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if self.phase != Phase::InitialRead {
            return Err(SourceCommandError::Denied(
                "foundation invocation custody sequence",
            ));
        }
        self.current(deadline, cancelled)?;
        self.phase = Phase::Finished;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::time::Duration;

    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(5)
    }
    fn fixture() -> (tempfile::TempDir, PathBuf, AtomicBool) {
        let root = tempfile::tempdir().unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = root.path().join("invocation.json");
        std::fs::write(&path, b"{\"selected\":true}\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        (root, path, AtomicBool::new(false))
    }

    #[test]
    fn exact_custody_requires_initial_then_one_terminal_check() {
        let (_root, path, cancelled) = fixture();
        let mut held =
            ProtectedFoundationInvocation::select(&path, deadline(), &cancelled).unwrap();
        assert!(
            held.verify_before_source_eof(deadline(), &cancelled)
                .is_err()
        );
        assert_eq!(
            held.initial_read(deadline(), &cancelled).unwrap(),
            b"{\"selected\":true}\n"
        );
        assert!(held.initial_read(deadline(), &cancelled).is_err());
        held.verify_before_source_eof(deadline(), &cancelled)
            .unwrap();
        assert!(
            held.verify_before_source_eof(deadline(), &cancelled)
                .is_err()
        );
    }

    #[test]
    fn changed_or_replaced_invocation_fails_terminal_check() {
        let (_root, path, cancelled) = fixture();
        let mut held =
            ProtectedFoundationInvocation::select(&path, deadline(), &cancelled).unwrap();
        held.initial_read(deadline(), &cancelled).unwrap();
        std::fs::write(&path, b"changed").unwrap();
        assert!(
            held.verify_before_source_eof(deadline(), &cancelled)
                .is_err()
        );
    }

    #[test]
    fn identical_bytes_in_replaced_inode_fail_terminal_check() {
        let (root, path, cancelled) = fixture();
        let mut held =
            ProtectedFoundationInvocation::select(&path, deadline(), &cancelled).unwrap();
        held.initial_read(deadline(), &cancelled).unwrap();
        let replacement = root.path().join("replacement");
        std::fs::write(&replacement, b"{\"selected\":true}\n").unwrap();
        std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::rename(replacement, path).unwrap();
        assert!(
            held.verify_before_source_eof(deadline(), &cancelled)
                .is_err()
        );
    }

    #[test]
    fn foreign_owner_and_unsafe_file_mode_are_refused() {
        let (_root, path, cancelled) = fixture();
        let uid = rustix::process::getuid().as_raw();
        let other = uid.checked_add(1).unwrap();
        assert!(
            select_held_file(&path, other, true, INVOCATION_BYTES, deadline(), &cancelled).is_err()
        );
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        assert!(ProtectedFoundationInvocation::select(&path, deadline(), &cancelled).is_err());
    }

    #[test]
    fn symlink_and_writable_ancestor_are_refused() {
        let (root, path, cancelled) = fixture();
        let alias = root.path().join("link.json");
        symlink(&path, &alias).unwrap();
        assert!(ProtectedFoundationInvocation::select(&alias, deadline(), &cancelled).is_err());
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o770)).unwrap();
        assert!(ProtectedFoundationInvocation::select(&path, deadline(), &cancelled).is_err());
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[test]
    fn ancestor_becoming_writable_fails_terminal_check() {
        let (root, path, cancelled) = fixture();
        let mut held =
            ProtectedFoundationInvocation::select(&path, deadline(), &cancelled).unwrap();
        held.initial_read(deadline(), &cancelled).unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o770)).unwrap();
        assert!(
            held.verify_before_source_eof(deadline(), &cancelled)
                .is_err()
        );
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
}
