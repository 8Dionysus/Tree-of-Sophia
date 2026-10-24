//! Verify one selected immutable object without scanning the object namespace.

use std::io::{self, Read, Write};

use tos_foundation::{Digest256, Digest256Hasher};

use crate::error::{Result, StoreError, StoreErrorCode};
use crate::secure_open::StoreRoot;

const BLOCK_BYTES: usize = 1024 * 1024;

pub(crate) fn verify_selected_object(
    root: &StoreRoot,
    sha256: Digest256,
    expected_size: u64,
    max_bytes: u64,
    sink: &mut impl Write,
) -> Result<u64> {
    verify_digest_object(root, sha256, Some(expected_size), max_bytes, sink)
}

/// The v1 retirement ledger omits retired object length. Use the opened
/// object's observed length only as a bounded read target, never as identity;
/// the exact ledger digest still authenticates every returned byte.
pub(crate) fn verify_digest_object(
    root: &StoreRoot,
    sha256: Digest256,
    expected_size: Option<u64>,
    max_bytes: u64,
    sink: &mut impl Write,
) -> Result<u64> {
    if expected_size.is_some_and(|size| size > max_bytes) {
        return Err(StoreError::new(
            StoreErrorCode::BudgetExceeded,
            "selected object exceeds read limit",
        ));
    }
    let mut file = root.open_object(&sha256.to_hex()).map_err(|error| {
        if error.code == StoreErrorCode::Io
            && error
                .source
                .as_ref()
                .is_some_and(|source| source.kind() == io::ErrorKind::NotFound)
        {
            StoreError::new(
                StoreErrorCode::CorruptSelectedObject,
                "selected object is absent",
            )
        } else {
            error
        }
    })?;
    let metadata = file
        .metadata()
        .map_err(|error| StoreError::io("cannot stat opened selected object", error))?;
    if expected_size.is_some_and(|size| metadata.len() != size) {
        return Err(StoreError::new(
            StoreErrorCode::CorruptSelectedObject,
            "selected object size differs",
        ));
    }
    let expected_size = expected_size.unwrap_or(metadata.len());
    if expected_size > max_bytes {
        return Err(StoreError::new(
            StoreErrorCode::BudgetExceeded,
            "retained object exceeds read limit",
        ));
    }
    let mut digest = Digest256Hasher::new();
    let mut actual = 0u64;
    let mut block = [0u8; BLOCK_BYTES];
    loop {
        // Read one byte beyond the smaller bound to detect growth after stat.
        let remaining = max_bytes.min(expected_size).saturating_sub(actual);
        let request = usize::try_from(remaining.saturating_add(1).min(BLOCK_BYTES as u64))
            .expect("request never exceeds BLOCK_BYTES");
        let count = file
            .read(&mut block[..request])
            .map_err(|error| StoreError::io("cannot read selected object", error))?;
        if count == 0 {
            break;
        }
        actual = actual.saturating_add(count as u64);
        if actual > max_bytes || actual > expected_size {
            return Err(StoreError::new(
                StoreErrorCode::CorruptSelectedObject,
                "selected object grew during read",
            ));
        }
        digest.update(&block[..count]);
        sink.write_all(&block[..count])
            .map_err(|error| StoreError::io("cannot write private selected-object stage", error))?;
    }
    if actual != expected_size || digest.finalize() != sha256 {
        return Err(StoreError::new(
            StoreErrorCode::CorruptSelectedObject,
            "selected object digest or size differs",
        ));
    }
    Ok(actual)
}

pub(crate) fn verify_without_copy(
    root: &StoreRoot,
    sha256: Digest256,
    expected_size: u64,
    max_bytes: u64,
) -> Result<()> {
    verify_selected_object(root, sha256, expected_size, max_bytes, &mut io::sink()).map(|_| ())
}

/// Resource-only operation labels. They never select source or confer authority.
#[derive(Clone, Copy, Debug)]
pub enum OriginalReadKind {
    Member,
    Retirement,
    SelectedObject,
    RetirementObject,
}

/// Prospective work/holder facts supplied by the maintained implementation.
/// Requested I/O bytes are not returned bytes, allocation, or an RSS limit.
#[derive(Clone, Copy, Debug)]
pub enum OriginalReadStage {
    Bootstrap {
        kind: OriginalReadKind,
        entries: usize,
        key_bytes: usize,
    },
    Lookup {
        entries: usize,
        key_bytes: usize,
    },
    Descriptor {
        path_bytes: usize,
    },
    Open {
        expected_size: Option<u64>,
        max_bytes: u64,
    },
    Metadata,
    Workspace {
        expected_size: u64,
        scratch_bytes: usize,
        hash_state_bytes: usize,
        stage_inline_bytes: usize,
    },
    Read {
        requested_bytes: usize,
    },
    Inspect {
        returned_bytes: usize,
    },
    Hash {
        bytes: usize,
    },
    Append {
        length: usize,
        capacity: usize,
        additional: usize,
    },
    Finalize {
        actual: u64,
        expected_size: u64,
    },
    ReturnMember {
        path_bytes: usize,
        identity_entries: usize,
    },
    ReturnId {
        id_bytes: usize,
        length: usize,
        capacity: usize,
    },
    ReturnRetirement {
        path_bytes: usize,
        event_ref_bytes: usize,
    },
}

/// Lends the ORIGINAL caller payer/session. No implementation may perform I/O,
/// seek/clone/replace the FD, reenter a reader, decode source, or issue authority.
/// The caller must prepay bootstrap/carriers before invoking the admitted API.
/// Successful charges remain spent on EOF and every later error; no refund.
pub trait OriginalReadAdmission {
    type Error;
    fn before_original_read(
        &mut self,
        descriptor: Option<&std::fs::File>,
        stage: OriginalReadStage,
    ) -> std::result::Result<(), Self::Error>;
}

/// Keeps the original caller error by value, without a side cell, clone or
/// conversion into a generic store refusal. Store errors retain their source.
#[derive(Debug)]
pub enum OriginalReadError<E> {
    Store(StoreError),
    Admission(E),
}
pub type OriginalReadResult<T, E> = std::result::Result<T, OriginalReadError<E>>;
impl<E> From<StoreError> for OriginalReadError<E> {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

pub(crate) fn admit_original<A: OriginalReadAdmission + ?Sized>(
    admission: &mut A,
    descriptor: Option<&std::fs::File>,
    stage: OriginalReadStage,
    deadline: std::time::Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> OriginalReadResult<(), A::Error> {
    original_active(deadline, cancelled)?;
    admission
        .before_original_read(descriptor, stage)
        .map_err(OriginalReadError::Admission)?;
    original_active(deadline, cancelled).map_err(OriginalReadError::Store)
}

fn original_active(
    deadline: std::time::Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) || std::time::Instant::now() >= deadline
    {
        Err(StoreError::new(
            StoreErrorCode::BudgetExceeded,
            "source read cancelled or expired",
        ))
    } else {
        Ok(())
    }
}

/// Closed native stage: cannot be supplied or replaced by an external caller.
/// One mutable admission is lent sequentially; it is never captured by a sink.
struct OriginalStage {
    raw: Vec<u8>,
}
impl OriginalStage {
    fn append<A: OriginalReadAdmission + ?Sized>(
        &mut self,
        descriptor: &std::fs::File,
        bytes: &[u8],
        admission: &mut A,
        deadline: std::time::Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> OriginalReadResult<(), A::Error> {
        admit_original(
            admission,
            Some(descriptor),
            OriginalReadStage::Append {
                length: self.raw.len(),
                capacity: self.raw.capacity(),
                additional: bytes.len(),
            },
            deadline,
            cancelled,
        )?;
        self.raw.extend_from_slice(bytes);
        Ok(())
    }
}

/// Exact same object namespace and digest/size semantics as the legacy reader.
/// Every returned Vec is private until the complete digest check succeeds.
pub(crate) fn read_digest_object_with_original_admission<A: OriginalReadAdmission + ?Sized>(
    root: &StoreRoot,
    sha256: Digest256,
    expected_size: Option<u64>,
    max_bytes: u64,
    admission: &mut A,
    deadline: std::time::Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> OriginalReadResult<Vec<u8>, A::Error> {
    // Keep the original cap refusal before the object-open path.
    if expected_size.is_some_and(|size| size > max_bytes) {
        return Err(StoreError::new(
            StoreErrorCode::BudgetExceeded,
            "selected object exceeds read limit",
        )
        .into());
    }
    admit_original(
        admission,
        None,
        OriginalReadStage::Open {
            expected_size,
            max_bytes,
        },
        deadline,
        cancelled,
    )?;
    let file = root.open_object(&sha256.to_hex()).map_err(|error| {
        if error.code == StoreErrorCode::Io
            && error
                .source
                .as_ref()
                .is_some_and(|source| source.kind() == io::ErrorKind::NotFound)
        {
            StoreError::new(
                StoreErrorCode::CorruptSelectedObject,
                "selected object is absent",
            )
        } else {
            error
        }
    })?;
    admit_original(
        admission,
        Some(&file),
        OriginalReadStage::Metadata,
        deadline,
        cancelled,
    )?;
    let metadata = file
        .metadata()
        .map_err(|error| StoreError::io("cannot stat opened selected object", error))?;
    if expected_size.is_some_and(|size| metadata.len() != size) {
        return Err(StoreError::new(
            StoreErrorCode::CorruptSelectedObject,
            "selected object size differs",
        )
        .into());
    }
    let expected_size = expected_size.unwrap_or(metadata.len());
    if expected_size > max_bytes {
        return Err(StoreError::new(
            StoreErrorCode::BudgetExceeded,
            "retained object exceeds read limit",
        )
        .into());
    }
    // This event precedes Vec, hasher and the full scratch, not merely payload.
    admit_original(
        admission,
        Some(&file),
        OriginalReadStage::Workspace {
            expected_size,
            scratch_bytes: BLOCK_BYTES,
            hash_state_bytes: std::mem::size_of::<Digest256Hasher>(),
            stage_inline_bytes: std::mem::size_of::<OriginalStage>(),
        },
        deadline,
        cancelled,
    )?;
    let mut stage = OriginalStage { raw: Vec::new() };
    // The production reader is this SAME held File, never another path/FD.
    let mut reader = &file;
    verify_opened_original(
        &file,
        &mut reader,
        sha256,
        expected_size,
        max_bytes,
        &mut stage,
        admission,
        deadline,
        cancelled,
    )?;
    Ok(stage.raw)
}

/// Private Read seam permits deterministic short completions in unit tests.
/// Production has exactly one caller above: reader=&the_same_held_File.
#[allow(clippy::too_many_arguments)]
fn verify_opened_original<A: OriginalReadAdmission + ?Sized>(
    file: &std::fs::File,
    reader: &mut impl Read,
    sha256: Digest256,
    expected_size: u64,
    max_bytes: u64,
    stage: &mut OriginalStage,
    admission: &mut A,
    deadline: std::time::Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> OriginalReadResult<u64, A::Error> {
    let mut digest = Digest256Hasher::new();
    let mut actual = 0u64;
    let mut block = [0u8; BLOCK_BYTES];
    loop {
        let remaining = max_bytes.min(expected_size).saturating_sub(actual);
        let request = usize::try_from(remaining.saturating_add(1).min(BLOCK_BYTES as u64))
            .expect("request never exceeds BLOCK_BYTES");
        admit_original(
            admission,
            Some(file),
            OriginalReadStage::Read {
                requested_bytes: request,
            },
            deadline,
            cancelled,
        )?;
        let count = reader
            .read(&mut block[..request])
            .map_err(|error| StoreError::io("cannot read selected object", error))?;
        admit_original(
            admission,
            Some(file),
            OriginalReadStage::Inspect {
                returned_bytes: count,
            },
            deadline,
            cancelled,
        )?;
        if count == 0 {
            break;
        }
        actual = actual.saturating_add(count as u64);
        if actual > max_bytes || actual > expected_size {
            return Err(StoreError::new(
                StoreErrorCode::CorruptSelectedObject,
                "selected object grew during read",
            )
            .into());
        }
        admit_original(
            admission,
            Some(file),
            OriginalReadStage::Hash { bytes: count },
            deadline,
            cancelled,
        )?;
        digest.update(&block[..count]);
        stage.append(file, &block[..count], admission, deadline, cancelled)?;
    }
    admit_original(
        admission,
        Some(file),
        OriginalReadStage::Finalize {
            actual,
            expected_size,
        },
        deadline,
        cancelled,
    )?;
    if actual != expected_size || digest.finalize() != sha256 {
        return Err(StoreError::new(
            StoreErrorCode::CorruptSelectedObject,
            "selected object digest or size differs",
        )
        .into());
    }
    Ok(actual)
}

#[cfg(all(test, target_os = "linux"))]
mod original_admission_tests {
    use super::*;
    use std::fs::{File, OpenOptions};
    use std::os::fd::AsRawFd;
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};

    // Synthetic protocol trace, NOT Common's production currency or a fit
    // certificate. One object is lent through every actual event without reset.
    struct Trace {
        fd: i32,
        remaining: u64,
        spent: u64,
        events: Vec<OriginalReadStage>,
        requests: Vec<usize>,
    }
    impl OriginalReadAdmission for Trace {
        type Error = StoreError;
        fn before_original_read(
            &mut self,
            file: Option<&File>,
            stage: OriginalReadStage,
        ) -> Result<()> {
            if let Some(file) = file {
                assert_eq!(file.as_raw_fd(), self.fd);
            }
            self.events.push(stage);
            let units = match stage {
                OriginalReadStage::Read { requested_bytes } => requested_bytes as u64,
                _ => 1,
            };
            self.remaining = self.remaining.checked_sub(units).ok_or_else(|| {
                StoreError::new(
                    StoreErrorCode::BudgetExceeded,
                    "fixture original payer exhausted",
                )
            })?;
            self.spent += units;
            if let OriginalReadStage::Read { requested_bytes } = stage {
                self.requests.push(requested_bytes);
            }
            Ok(())
        }
    }
    // A controlled short-completion/race seam around the SAME real File. It is
    // private to this module; production passes &File directly, with no shim.
    struct GrowingFile<'a> {
        file: &'a File,
        grow_once: Option<&'a File>,
    }
    impl Read for GrowingFile<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if let Some(writer) = self.grow_once.take() {
                let mut writer = writer;
                writer.write_all(b"x")?;
                writer.flush()?;
            }
            let mut file = self.file;
            file.read(buf)
        }
    }
    fn fixture() -> tempfile::NamedTempFile {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"abc").unwrap();
        tmp.flush().unwrap();
        tmp
    }
    fn control(file: &File, remaining: u64) -> Trace {
        Trace {
            fd: file.as_raw_fd(),
            remaining,
            spent: 0,
            events: Vec::new(),
            requests: Vec::new(),
        }
    }
    fn workspace(file: &File, payer: &mut Trace, deadline: Instant, cancel: &AtomicBool) {
        admit_original(
            payer,
            Some(file),
            OriginalReadStage::Workspace {
                expected_size: 3,
                scratch_bytes: BLOCK_BYTES,
                hash_state_bytes: std::mem::size_of::<Digest256Hasher>(),
                stage_inline_bytes: std::mem::size_of::<OriginalStage>(),
            },
            deadline,
            cancel,
        )
        .unwrap();
    }
    #[test]
    fn original_admission_short_read_requests_are_prospective_and_include_eof() {
        // Deterministic REAL short File::read completions: the producer writes
        // exactly one byte then waits for an acknowledgement AFTER read returns.
        // The request slice is never clipped. This tests the private byte loop,
        // not StoreRoot's regular-object namespace/authority fence.
        let (input, mut output) = std::os::unix::net::UnixStream::pair().unwrap();
        let file = File::from(std::os::fd::OwnedFd::from(input));
        let (ack, next) = std::sync::mpsc::sync_channel(0);
        let producer = std::thread::spawn(move || {
            for byte in b"abc" {
                output.write_all(&[*byte]).unwrap();
                next.recv().unwrap();
            }
        });
        struct AckFile<'a> {
            file: &'a File,
            ack: std::sync::mpsc::SyncSender<()>,
        }
        impl Read for AckFile<'_> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                let mut file = self.file;
                let count = file.read(buf)?;
                if count != 0 {
                    self.ack
                        .send(())
                        .map_err(|_| io::Error::other("fixture producer disconnected"))?;
                }
                Ok(count)
            }
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        let cancel = AtomicBool::new(false);
        let mut payer = control(&file, 1000);
        workspace(&file, &mut payer, deadline, &cancel);
        let mut stage = OriginalStage { raw: Vec::new() };
        let mut reader = AckFile { file: &file, ack };
        assert_eq!(
            verify_opened_original(
                &file,
                &mut reader,
                Digest256::of_bytes(b"abc"),
                3,
                3,
                &mut stage,
                &mut payer,
                deadline,
                &cancel
            )
            .unwrap(),
            3
        );
        producer.join().unwrap();
        assert_eq!(stage.raw, b"abc");
        assert_eq!(payer.requests, [4, 3, 2, 1]);
        assert_eq!(payer.requests.iter().sum::<usize>(), 10);
        assert!(payer.events.windows(2).any(|p| matches!(
            p,
            [
                OriginalReadStage::Read { requested_bytes: 1 },
                OriginalReadStage::Inspect { returned_bytes: 0 }
            ]
        )));
        assert!(matches!(
            payer.events.last(),
            Some(OriginalReadStage::Finalize {
                actual: 3,
                expected_size: 3
            })
        ));
    }
    #[test]
    fn original_admission_growth_is_inspected_before_hash_or_append() {
        let tmp = fixture();
        let file = File::open(tmp.path()).unwrap();
        let writer = OpenOptions::new().append(true).open(tmp.path()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let cancel = AtomicBool::new(false);
        let mut payer = control(&file, 1000);
        workspace(&file, &mut payer, deadline, &cancel);
        let mut stage = OriginalStage { raw: Vec::new() };
        let mut reader = GrowingFile {
            file: &file,
            grow_once: Some(&writer),
        };
        let error = verify_opened_original(
            &file,
            &mut reader,
            Digest256::of_bytes(b"abc"),
            3,
            3,
            &mut stage,
            &mut payer,
            deadline,
            &cancel,
        )
        .unwrap_err();
        let OriginalReadError::Store(error) = error else {
            panic!("growth must retain the original store refusal")
        };
        assert_eq!(error.code, StoreErrorCode::CorruptSelectedObject);
        assert_eq!(error.detail, "selected object grew during read");
        assert_eq!(payer.requests, [4]);
        assert!(stage.raw.is_empty());
        assert!(!payer.events.iter().any(|e| matches!(
            e,
            OriginalReadStage::Hash { .. } | OriginalReadStage::Append { .. }
        )));
        assert!(matches!(
            payer.events.last(),
            Some(OriginalReadStage::Inspect { returned_bytes: 4 })
        ));
    }
    #[test]
    fn original_admission_exhaustion_keeps_first_error_and_prior_spent() {
        for (budget, spent, read) in [(3, 1, false), (7, 7, true)] {
            let tmp = fixture();
            let file = File::open(tmp.path()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            let cancel = AtomicBool::new(false);
            let mut payer = control(&file, budget);
            workspace(&file, &mut payer, deadline, &cancel);
            let mut stage = OriginalStage { raw: Vec::new() };
            let mut reader = &file;
            let error = verify_opened_original(
                &file,
                &mut reader,
                Digest256::of_bytes(b"abc"),
                3,
                3,
                &mut stage,
                &mut payer,
                deadline,
                &cancel,
            )
            .unwrap_err();
            let OriginalReadError::Admission(error) = error else {
                panic!("caller error must remain the original typed value")
            };
            assert_eq!(error.code, StoreErrorCode::BudgetExceeded);
            assert_eq!(error.detail, "fixture original payer exhausted");
            assert_eq!(payer.spent, spent);
            assert_eq!(!payer.requests.is_empty(), read);
            assert!(stage.raw.is_empty());
            assert!(
                !payer
                    .events
                    .iter()
                    .any(|e| matches!(e, OriginalReadStage::Finalize { .. }))
            );
        }
    }
    #[test]
    fn original_admission_real_fd_read_error_is_paid_not_relabelled() {
        let tmp = fixture();
        let file = OpenOptions::new().write(true).open(tmp.path()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let cancel = AtomicBool::new(false);
        let mut payer = control(&file, 1000);
        workspace(&file, &mut payer, deadline, &cancel);
        let mut stage = OriginalStage { raw: Vec::new() };
        let mut reader = &file;
        let error = verify_opened_original(
            &file,
            &mut reader,
            Digest256::of_bytes(b"abc"),
            3,
            3,
            &mut stage,
            &mut payer,
            deadline,
            &cancel,
        )
        .unwrap_err();
        let OriginalReadError::Store(error) = error else {
            panic!("physical I/O error must not become a payer error")
        };
        assert_eq!(error.code, StoreErrorCode::Io);
        assert_eq!(error.detail, "cannot read selected object");
        assert_eq!(
            error.source.unwrap().raw_os_error(),
            Some(rustix::io::Errno::BADF.raw_os_error())
        );
        assert_eq!(payer.requests, [4]);
        assert_eq!(payer.spent, 5);
        assert!(stage.raw.is_empty());
        assert!(
            !payer
                .events
                .iter()
                .any(|e| matches!(e, OriginalReadStage::Inspect { .. }))
        );
    }
}
