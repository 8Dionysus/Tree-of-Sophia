//! Resource-only original Session adapter for protected physical rereads.
//!
//! The selected owner retains its FD, path and permissions. This adapter
//! opens/clones/seeks/reads no FD in a callback and issues no source authority.
//! Its logical raw-reader holders and requested allocation layouts are not a
//! complete quote for outer owner/path/metadata/driver/activation or allocator
//! RSS. Those remain explicit protected-caller funding obligations.

use super::OriginalAssessmentPayer;
use crate::source_assessment_journal::ProtectedAssessmentPgGrant;
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{CreationReadAdmission, CreationReadStage, ReadLength};
use std::{fs::{File, Metadata}, mem::size_of};
use tos_compiler::pwe_native::{Error, Session};

/// The SAME original payer and cumulative source Session, borrowed only for
/// this physical read. No operation-budget loan is held across foreign I/O.
pub(crate) struct OriginalProtectedReads<'loan, 'pure, 'budget, 'cancel> {
    payer: &'loan OriginalAssessmentPayer<'budget, 'cancel>,
    session: &'loan mut Session<'pure>,
}

impl<'loan, 'pure, 'budget, 'cancel>
    OriginalProtectedReads<'loan, 'pure, 'budget, 'cancel>
{
    pub(crate) fn new(
        payer: &'loan OriginalAssessmentPayer<'budget, 'cancel>,
        session: &'loan mut Session<'pure>,
    ) -> SourceCommandResult<Self> {
        payer.charge_sto_source_measures(session, 0, 0, 0, 0, 0)?;
        // Before Self and the reader's first event. The enum is a single
        // nonrecursive raw-reader argument slot, reused across loop events;
        // the adapter retains neither the event nor the supplied descriptor.
        let fixed = size_of::<Self>()
            .checked_add(size_of::<SourceCommandResult<Self>>())
            .and_then(|n| n.checked_add(size_of::<CreationReadStage>()))
            .and_then(|n| n.checked_add(size_of::<ReadLength>()))
            .and_then(|n| n.checked_add(size_of::<Option<&mut dyn CreationReadAdmission>>()))
            .and_then(|n| n.checked_add(size_of::<SourceCommandResult<()>>()))
            // raw obtains its first metadata BEFORE Begin; those holders
            // therefore belong to this earlier bootstrap, not the callback.
            .and_then(|n| n.checked_add(size_of::<Vec<u8>>()))
            .and_then(|n| n.checked_add(2 * size_of::<Metadata>()))
            .and_then(|n| n.checked_add(size_of::<std::io::Result<usize>>()))
            .and_then(|n| n.checked_add(size_of::<SourceCommandResult<Vec<u8>>>()))
            .and_then(|n| n.checked_add(2 * size_of::<usize>()))
            .ok_or_else(|| payer.error(Error::ResourceLimit))?;
        payer.charge_sto_source_measures(session, 0, fixed, 0, 0, 0)?;
        Ok(Self { payer, session })
    }

    /// Resource exit only; the actual protected owner still owes its current
    /// selection/permission/source exit. Preserve the first body error even
    /// when the same original resource cutoff also refuses.
    pub(crate) fn complete<T>(&mut self, result: SourceCommandResult<T>) -> SourceCommandResult<T> {
        let cutoff = self.payer.charge_sto_source_measures(self.session, 0, 0, 0, 0, 0);
        match (result, cutoff) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
}

impl CreationReadAdmission for OriginalProtectedReads<'_, '_, '_, '_> {
    fn before_creation_read(
        &mut self,
        _descriptor: &File,
        stage: CreationReadStage,
    ) -> SourceCommandResult<()> {
        // The raw owner, not this callback, selects/validates and reads the
        // actual supplied FD immediately after admission. No FD operation,
        // private JSON parse, worker or source dispatch occurs here.
        let (requested, owned, scratch, work) = match stage {
            CreationReadStage::Begin { scratch_bytes, .. } => {
                // The fixed raw slots were paid BEFORE the first metadata.
                // Pay this actual scratch before raw constructs its buffer.
                // Outer path traversal and grant ownership are not included.
                (0, 0, scratch_bytes, 0)
            }
            CreationReadStage::Read { requested_bytes } => {
                // Charge the ACTUAL entire request before syscall, including
                // EOF, I/O Err and each short-read retry. Never infer request
                // size from a cached file length or returned-byte count.
                // admit_source_prefix already spends these requested bytes
                // as input AND work. Do not debit them a second time as an
                // extra byte_work measurement.
                let requested = u64::try_from(requested_bytes)
                    .map_err(|_| self.payer.error(Error::ResourceLimit))?;
                (requested, 0, 0, 0)
            }
            CreationReadStage::Retain { length, capacity, additional } => {
                // Same normative pinned Rust1.98.1 RawVec growth model as the
                // immutable original reader, not a newly invented multiplier.
                // Old backing remains paid; reserve the WHOLE new requested
                // backing before extend and pay old-copy plus incoming copy.
                let (new_owned, copied) = super::immutable_read::growth(
                    length, capacity, additional, size_of::<u8>(),
                ).map_err(|error| self.payer.error(error))?;
                let copied = copied.checked_add(additional)
                    .ok_or_else(|| self.payer.error(Error::ResourceLimit))?;
                let copied = u64::try_from(copied)
                    .map_err(|_| self.payer.error(Error::ResourceLimit))?;
                (0, new_owned, 0, copied)
            }
        };
        // One source event (work), never nodes/refs/input masquerading as work.
        // This is an explicit event currency, not a native instruction bound.
        self.payer.charge_sto_source_measures(self.session, requested, owned, scratch, work, 1)
    }
}

impl crate::source_text_owner::ProtectedContextReadAdmission
    for OriginalProtectedReads<'_, '_, '_, '_>
{
    fn before_protected_context_work(&mut self, bytes: usize) -> SourceCommandResult<()> {
        let work = u64::try_from(bytes).map_err(|_| self.payer.error(Error::ResourceLimit))?;
        self.payer.charge_sto_source_measures(self.session, 0, 0, 0, work, 1)
    }
}

impl OriginalAssessmentPayer<'_, '_> {
    /// Actual protected-grant reread on the SAME source Session. The selected
    /// grant performs its UID/mode/parents/inode/raw checks and uses this
    /// callback for its already-opened FD. This is not yet the whole protected
    /// Assessment operation or permission/semantic/PG role admission.
    pub(crate) fn recheck_current_pg_grant_source(
        &self,
        session: &mut Session<'_>,
        grant: &ProtectedAssessmentPgGrant,
    ) -> SourceCommandResult<()> {
        let (deadline, cancelled) = self.source_cutoff();
        let mut reader = OriginalProtectedReads::new(self, session)?;
        // The owner compares its newly read Vec with this immutable retained
        // raw value. Equal-length comparison can inspect BOTH full operands;
        // a length mismatch is cheaper. Pay that bound before the actual read.
        let comparison = grant.configuration_raw().len().checked_mul(2)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| self.error(Error::ResourceLimit))?;
        self.charge_sto_source_measures(reader.session, 0, 0, 0, comparison, 0)?;
        let result = grant.verify_current_with_read_admission(deadline, cancelled, &mut reader);
        reader.complete(result)
    }

    /// Actual owner-context FD/current-path/root rechecks. This retains all
    /// held-file byte/digest checks and pays real reread/compare/hash work on
    /// the SAME original Session, without a new pinned-I/O wallet. Outer
    /// context selection and complete native path/activation cost remain open.
    pub(crate) fn recheck_owner_context_source(
        &self,
        session: &mut Session<'_>,
        owner: &crate::source_text_owner::OwnerTextContext,
    ) -> SourceCommandResult<()> {
        let (deadline, cancelled) = self.source_cutoff();
        let mut reader = OriginalProtectedReads::new(self, session)?;
        let result = owner.verify_context_base_with_read_admission(deadline, cancelled, &mut reader);
        reader.complete(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_creation_store::raw_with_length_limit_and_admission;
    use crate::source_native_cli::private_assessment::OperationBudget;
    use super::super::with_original_pwe_session;
    use std::{path::PathBuf, sync::atomic::AtomicBool, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};
    use tos_compiler::pwe_native::Limits;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!(
                "tos-original-protected-read-{}-{stamp}", std::process::id(),
            ));
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("input"), b"xy").unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    fn limits(deadline: Instant) -> Limits {
        Limits {
            max_input_bytes: 262_144, max_nodes: 32, max_work: 262_144,
            max_refs: 8, max_depth: 8, max_items: 8,
            max_state_bytes: 1_048_576, deadline,
        }
    }
    fn budget(deadline: Instant) -> OperationBudget {
        OperationBudget::new(&serde_json::json!({"budgets":{
            "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
        }}), deadline).unwrap()
    }
    #[test]
    fn actual_protected_read_prices_full_eof_on_original_session_without_nodes() {
        let fixture = Fixture::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = budget(deadline);
        let original_work = original.work_remaining;
        let limits = limits(deadline);
        with_original_pwe_session(&mut original, limits, &cancelled, |payer, same| {
            let mut file = File::open(fixture.0.join("input")).unwrap();
            let mut reader = OriginalProtectedReads::new(payer, same)?;
            let result = raw_with_length_limit_and_admission(
                &mut file, ReadLength::Exact(2), deadline, &cancelled, Some(&mut reader),
            );
            let raw = reader.complete(result)?;
            assert_eq!(raw, b"xy");
            let spent = reader.session.charges();
            assert_eq!(spent.0, 2 * 65_536, "actual payload request plus actual EOF request");
            assert_eq!(spent.1, 0, "native work is not a node surrogate");
            assert!(spent.2 > spent.0, "copy/event work is additional to requested input work");
            assert_eq!(spent.3, 0);
            assert!(spent.4 >= 65_536 + raw.capacity());
            assert!(reader.session.has_source_payer(payer));
            Ok(())
        }).unwrap();
        assert_eq!(original.work_remaining, original_work - limits.max_work,
            "source work envelope was prepaid once and not refunded/replaced");
    }
    #[test]
    fn actual_protected_read_preserves_io_error_and_paid_request_prefix() {
        let fixture = Fixture::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = budget(deadline);
        let result: SourceCommandResult<()> = with_original_pwe_session(
            &mut original, limits(deadline), &cancelled, |payer, same| {
                // A real write-only descriptor: metadata succeeds, read fails
                // after the full request's admission. This is not a mock I/O error.
                let mut file = std::fs::OpenOptions::new().write(true)
                    .open(fixture.0.join("input")).unwrap();
                let mut reader = OriginalProtectedReads::new(payer, same)?;
                let body = raw_with_length_limit_and_admission(
                    &mut file, ReadLength::Exact(2), deadline, &cancelled, Some(&mut reader),
                ).map(|_| ());
                assert!(matches!(&body, Err(SourceCommandError::Invalid("creation descriptor read"))));
                assert_eq!(reader.session.charges().0, 65_536);
                reader.complete(body)
            },
        );
        assert!(matches!(result, Err(SourceCommandError::Invalid("creation descriptor read"))));
    }
    #[test]
    fn actual_protected_read_denies_eof_overrun_and_keeps_sticky_prefix() {
        let fixture = Fixture::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = budget(deadline);
        let mut limited = limits(deadline);
        limited.max_input_bytes = 65_536;
        let result = with_original_pwe_session(&mut original, limited, &cancelled, |payer, same| {
            let mut file = File::open(fixture.0.join("input")).unwrap();
            let mut reader = OriginalProtectedReads::new(payer, same)?;
            let body = raw_with_length_limit_and_admission(
                &mut file, ReadLength::Exact(2), deadline, &cancelled, Some(&mut reader),
            );
            assert!(body.is_err(), "EOF request is not free or silently truncated");
            assert_eq!(reader.session.charges().0, 65_536);
            let before = reader.session.charges();
            assert!(reader.complete(Ok(())).is_err(), "swallowing mapped refusal cannot renew reader");
            assert_eq!(reader.session.charges(), before, "no reset/refill on refusal");
            // Deliberately swallow: the ORIGINAL payer's outer finish must
            // still deny success, with its same sticky refusal marker.
            drop(body);
            Ok(())
        });
        assert!(result.is_err());
    }
}
