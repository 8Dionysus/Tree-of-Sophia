//! Admitted digest of the fixed private-owner scope recipe. This is context
//! identity only, never a read/write grant or an arbitrary JSON encoder.
//! Valid UTF-8 strings/u64 fields are streamed in the maintained canonical
//! key order, avoiding a second JSON tree, sorting arena or numeric parser.
use super::{OwnerTextContext, ProtectedContextReadAdmission};
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use std::{mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{Digest256, Digest256Hasher};

pub(crate) trait PrivateScopeBindingAdmission: ProtectedContextReadAdmission {
    fn retain_private_scope_binding(&mut self, bytes: usize) -> Result<()>;
    fn inspect_private_scope_binding(&mut self, bytes: usize, events: usize) -> Result<()>;
}

const U64_DIGITS: usize = u64::MAX.ilog10() as usize + 1;
const OUTPUT_CAP: usize = 8_388_608; // cmd::canonical SourceCommandInputV1
const HEX: &[u8; 16] = b"0123456789abcdef";

struct ScopeInput<'a> {
    configuration_path: &'a str,
    configuration_raw: &'a [u8],
    private_prefix: &'a str,
    public_root: &'a str,
    private_root: &'a str,
    public_identity: (u64, u64, u32, u32),
    private_identity: (u64, u64, u32, u32),
    uid: u32,
}
struct Sink<'a> {
    admission: &'a mut dyn PrivateScopeBindingAdmission,
    hash: Digest256Hasher,
    written: usize,
    decimal: [u8; U64_DIGITS],
    escape: [u8; 6],
    hex: [u8; 2],
}
fn fragment(
    admission: &mut dyn PrivateScopeBindingAdmission,
    hash: &mut Digest256Hasher,
    written: &mut usize,
    raw: &[u8],
) -> Result<()> {
    let next = written
        .checked_add(raw.len())
        .filter(|n| *n <= OUTPUT_CAP)
        .ok_or(Error::Invalid("source canonical input"))?;
    admission.inspect_private_scope_binding(raw.len(), 1)?;
    hash.update(raw);
    *written = next;
    Ok(())
}
impl Sink<'_> {
    fn emit(&mut self, raw: &[u8]) -> Result<()> {
        fragment(self.admission, &mut self.hash, &mut self.written, raw)
    }
    fn text(&mut self, value: &str) -> Result<()> {
        // Bound inspection and loop events before iterating valid UTF-8 bytes.
        // Ordinary multibyte UTF-8 is emitted verbatim, like write_string;
        // quotes/backslashes/control ASCII use the same short/lowercase escapes.
        self.admission
            .inspect_private_scope_binding(value.len(), value.len())?;
        self.emit(b"\"")?;
        for byte in value.bytes() {
            match byte {
                b'"' => self.emit(b"\\\"")?,
                b'\\' => self.emit(b"\\\\")?,
                8 => self.emit(b"\\b")?,
                12 => self.emit(b"\\f")?,
                b'\n' => self.emit(b"\\n")?,
                b'\r' => self.emit(b"\\r")?,
                b'\t' => self.emit(b"\\t")?,
                0..=31 => {
                    self.escape[4] = HEX[(byte >> 4) as usize];
                    self.escape[5] = HEX[(byte & 15) as usize];
                    fragment(
                        self.admission,
                        &mut self.hash,
                        &mut self.written,
                        &self.escape,
                    )?;
                }
                _ => self.emit(&[byte])?,
            }
        }
        self.emit(b"\"")
    }
    fn integer(&mut self, mut value: u64) -> Result<()> {
        let mut cursor = U64_DIGITS;
        loop {
            // One actual bounded decimal-digit event; no arbitrary decimal
            // reservation or general Number/Float grammar is introduced.
            self.admission.inspect_private_scope_binding(0, 1)?;
            cursor -= 1;
            self.decimal[cursor] = b'0' + (value % 10) as u8;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        fragment(
            self.admission,
            &mut self.hash,
            &mut self.written,
            &self.decimal[cursor..],
        )
    }
    fn tuple(&mut self, value: (u64, u64, u32, u32)) -> Result<()> {
        self.emit(b"[")?;
        self.integer(value.0)?;
        self.emit(b",")?;
        self.integer(value.1)?;
        self.emit(b",")?;
        self.integer(u64::from(value.2))?;
        self.emit(b",")?;
        self.integer(u64::from(value.3))?;
        self.emit(b"]")
    }
}
fn stream_scope(
    input: &ScopeInput<'_>,
    admission: &mut dyn PrivateScopeBindingAdmission,
) -> Result<Digest256> {
    // Hash initialization/finalization and complete raw configuration hashing
    // are admitted separately. This prices hash operands/events, not native
    // SHA instructions, allocator/RSS, syscall or full activation enclosures.
    admission.inspect_private_scope_binding(0, 1)?;
    let mut configuration_hash = Digest256Hasher::new();
    admission.inspect_private_scope_binding(input.configuration_raw.len(), 1)?;
    configuration_hash.update(input.configuration_raw);
    admission.inspect_private_scope_binding(0, 1)?;
    let configuration = configuration_hash.finalize();
    admission.inspect_private_scope_binding(U64_DIGITS + 6 + 2, 1)?;
    let mut sink = Sink {
        admission,
        hash: Digest256Hasher::new(),
        written: 0,
        decimal: [0; U64_DIGITS],
        escape: *b"\\u0000",
        hex: [0; 2],
    };
    sink.emit(b"{\"configuration_path\":")?;
    sink.text(input.configuration_path)?;
    sink.emit(b",\"configuration_sha256\":\"sha256:")?;
    sink.admission.inspect_private_scope_binding(
        configuration.as_bytes().len(),
        configuration.as_bytes().len(),
    )?;
    for &byte in configuration.as_bytes() {
        sink.hex[0] = HEX[(byte >> 4) as usize];
        sink.hex[1] = HEX[(byte & 15) as usize];
        fragment(sink.admission, &mut sink.hash, &mut sink.written, &sink.hex)?;
    }
    sink.emit(b"\",\"roots\":{\"owner-local-root\":")?;
    sink.tuple(input.private_identity)?;
    sink.emit(b",\"source-contract-root\":")?;
    sink.tuple(input.public_identity)?;
    sink.emit(b"},\"routing\":{\"private_prefix\":")?;
    sink.text(input.private_prefix)?;
    sink.emit(b",\"private_root\":")?;
    sink.text(input.private_root)?;
    sink.emit(b",\"public_root\":")?;
    sink.text(input.public_root)?;
    sink.emit(b"},\"schema_version\":\"tos_owner_local_private_scope_v1\",\"uid\":")?;
    sink.integer(u64::from(input.uid))?;
    sink.emit(b"}")?;
    sink.admission.inspect_private_scope_binding(0, 1)?;
    Ok(sink.hash.finalize())
}
impl OwnerTextContext {
    /// Exact same stable private scope value as private_owner_scope_binding,
    /// with genuine admitted context rechecks on BOTH Result exits. Neither a
    /// schema verdict nor this identity grants Claim/private/current access.
    pub(crate) fn private_owner_scope_binding_with_exact_admission(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
        admission: &mut dyn PrivateScopeBindingAdmission,
    ) -> Result<Digest256> {
        let body = (|| {
            // Before any new helper/input/hash/Result backing is constructed.
            let fixed = size_of::<ScopeInput<'_>>()
                .checked_add(size_of::<Sink<'_>>())
                .and_then(|n| n.checked_add(size_of::<Digest256Hasher>()))
                .and_then(|n| n.checked_add(2 * size_of::<Digest256>()))
                .and_then(|n| n.checked_add(size_of::<Result<Digest256>>()))
                .and_then(|n| n.checked_add(2 * size_of::<Result<()>>()))
                .and_then(|n| n.checked_add(size_of::<usize>()))
                .ok_or(Error::Unsupported("private scope binding holders overflow"))?;
            admission.retain_private_scope_binding(fixed)?;
            self.verify_context_base_using_read_admission(deadline, cancelled, admission, true)?;
            for path in [
                &self.configuration_path,
                &self.public_root,
                &self.private_root,
            ] {
                let bytes = path.as_os_str().as_encoded_bytes().len();
                admission.inspect_private_scope_binding(bytes, 1)?;
            }
            let input = ScopeInput {
                configuration_path: self
                    .configuration_path
                    .to_str()
                    .ok_or(Error::Invalid("owner-local path UTF-8"))?,
                configuration_raw: &self.configuration_raw,
                private_prefix: &self.private_prefix,
                public_root: self
                    .public_root
                    .to_str()
                    .ok_or(Error::Invalid("owner-local root UTF-8"))?,
                private_root: self
                    .private_root
                    .to_str()
                    .ok_or(Error::Invalid("owner-local root UTF-8"))?,
                public_identity: self.public_identity,
                private_identity: self.private_identity,
                uid: self.uid,
            };
            stream_scope(&input, admission)
        })();
        // Attempt the actual FD/root/currentness exit even after hash refusal.
        // A sticky/cancelled original payer may deny physical exit I/O.
        let exited =
            self.verify_context_base_using_read_admission(deadline, cancelled, admission, true);
        match (body, exited) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_command as cmd;
    use crate::source_creation_store::{CreationReadAdmission, CreationReadStage};
    use tos_foundation::JsonValue;
    struct HashMeter {
        bytes: usize,
        events: usize,
        refusal: bool,
    }
    impl CreationReadAdmission for HashMeter {
        fn before_creation_read(&mut self, _: &std::fs::File, _: CreationReadStage) -> Result<()> {
            panic!("hash-byte fixture must not perform physical I/O")
        }
    }
    impl ProtectedContextReadAdmission for HashMeter {
        fn before_protected_context_work(&mut self, _: usize) -> Result<()> {
            Ok(())
        }
    }
    impl PrivateScopeBindingAdmission for HashMeter {
        fn retain_private_scope_binding(&mut self, _: usize) -> Result<()> {
            Ok(())
        }
        fn inspect_private_scope_binding(&mut self, bytes: usize, events: usize) -> Result<()> {
            if self.refusal && self.events > 2 {
                return Err(Error::Invalid("source canonical input"));
            }
            self.bytes += bytes;
            self.events += events;
            Ok(())
        }
    }
    fn legacy(input: &ScopeInput<'_>) -> Digest256 {
        let tuple = |(dev, ino, uid, mode): (u64, u64, u32, u32)| {
            JsonValue::Array(vec![
                cmd::number(dev),
                cmd::number(ino),
                cmd::number(u64::from(uid)),
                cmd::number(u64::from(mode)),
            ])
        };
        let value = cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_owner_local_private_scope_v1"),
            ),
            ("uid", cmd::number(u64::from(input.uid))),
            ("configuration_path", cmd::string(input.configuration_path)),
            (
                "configuration_sha256",
                cmd::string(&Digest256::of_bytes(input.configuration_raw).to_prefixed()),
            ),
            (
                "roots",
                cmd::object(vec![
                    ("source-contract-root", tuple(input.public_identity)),
                    ("owner-local-root", tuple(input.private_identity)),
                ]),
            ),
            (
                "routing",
                cmd::object(vec![
                    ("private_prefix", cmd::string(input.private_prefix)),
                    ("public_root", cmd::string(input.public_root)),
                    ("private_root", cmd::string(input.private_root)),
                ]),
            ),
        ]);
        Digest256::of_bytes(&cmd::canonical(&value).unwrap())
    }
    #[test]
    fn admitted_private_scope_stream_matches_legacy_unicode_control_and_u64_recipe() {
        // Byte-protocol operands only, no OwnerTextContext/grant/issuer fixture.
        for text in [
            "/simple",
            "/π/é/𐐀",
            "\"\\\u{0}\u{1}\u{8}\u{c}\n\r\t\u{1f}\u{7f}",
        ] {
            for number in [0, 1, 9, 10, 99, 100, u32::MAX as u64, u64::MAX] {
                let input = ScopeInput {
                    configuration_path: text,
                    configuration_raw: b"raw config\0",
                    private_prefix: text,
                    public_root: text,
                    private_root: text,
                    public_identity: (number, number, u32::MAX, 0o755),
                    private_identity: (number, number, 0, 0o700),
                    uid: u32::MAX,
                };
                let mut meter = HashMeter {
                    bytes: 0,
                    events: 0,
                    refusal: false,
                };
                assert_eq!(stream_scope(&input, &mut meter).unwrap(), legacy(&input));
                assert!(meter.bytes > input.configuration_raw.len());
            }
        }
    }
    #[test]
    fn private_scope_hash_keeps_foreign_error_and_successful_prefix() {
        let input = ScopeInput {
            configuration_path: "/context",
            configuration_raw: b"raw config",
            private_prefix: "prefix",
            public_root: "/public",
            private_root: "/private",
            public_identity: (1, 2, 3, 4),
            private_identity: (5, 6, 7, 8),
            uid: 3,
        };
        let mut meter = HashMeter {
            bytes: 0,
            events: 0,
            refusal: true,
        };
        assert!(matches!(
            stream_scope(&input, &mut meter),
            Err(Error::Invalid("source canonical input"))
        ));
        assert!(meter.bytes >= input.configuration_raw.len());
        // An admission error colliding with the encoder's own cap message is
        // returned as-is. There is no string-based error remapping/retry.
        assert_eq!(meter.events, 3);
    }
}
