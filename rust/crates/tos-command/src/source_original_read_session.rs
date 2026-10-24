//! Original Assessment Session around the explicit immutable-v1 read ports.
//! This resource adapter grants no source/rights permission, opens no root,
//! performs no callback I/O and never falls back to a legacy or PG reader.
use super::OriginalAssessmentPayer;
use crate::source_command::{SourceCommandError, SourceCommandResult};
use std::{fs::File, mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_compiler::pwe_native::{Error, Session};
use tos_foundation::{RelativePath, SourceRevision};
use tos_source_store::{
    CorpusCutReader, CorpusDescriptor, OriginalReadAdmission, OriginalReadKind, OriginalReadResult,
    OriginalReadStage, RetiredSourceMemberV1, SourceMemberV1, StoreError,
};

/// The caller retains its actual selected cut and original payer throughout.
/// Whole cut/manifest loading and outer protected source fences are separate;
/// this finite v1 compatibility reader is not the addressed PG growth route.
pub(crate) struct OriginalImmutableReads<'loan, 'session, 'budget, 'cancel> {
    cut: &'loan CorpusCutReader,
    session: &'loan mut Session<'session>,
    payer: &'loan OriginalAssessmentPayer<'budget, 'cancel>,
    deadline: Instant,
    cancelled: &'cancel AtomicBool,
}

fn plus(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b).ok_or(Error::ResourceLimit)
}
fn times(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_mul(b).ok_or(Error::ResourceLimit)
}

/// Rust1.98.1 RawVec::grow_amortized, Vec<u8>::extend_from_slice and
/// Vec<String>::push. Global records the requested layout, not allocator RSS.
/// The already-paid old backing stays live; reserve the WHOLE new backing
/// before growth and pay its possible old-capacity copy separately.
fn growth(
    length: usize,
    capacity: usize,
    additional: usize,
    element: usize,
) -> Result<(usize, usize), Error> {
    let needed = plus(length, additional)?;
    if length > capacity {
        return Err(Error::ResourceLimit);
    }
    if needed <= capacity {
        return Ok((0, 0));
    }
    let minimum = if element == 1 { 8 } else { 4 };
    let new_capacity = times(capacity, 2)?.max(needed).max(minimum);
    Ok((times(new_capacity, element)?, times(capacity, element)?))
}

impl<'loan, 'session, 'budget, 'cancel> OriginalImmutableReads<'loan, 'session, 'budget, 'cancel> {
    pub(crate) fn new(
        cut: &'loan CorpusCutReader,
        session: &'loan mut Session<'session>,
        payer: &'loan OriginalAssessmentPayer<'budget, 'cancel>,
        deadline: Instant,
        cancelled: &'cancel AtomicBool,
    ) -> SourceCommandResult<Self> {
        if !session.has_source_payer(payer)
            || deadline > payer.deadline
            || !std::ptr::eq(cancelled, payer.cancelled)
        {
            return Err(SourceCommandError::Conflict(
                "immutable read Session/payer/cutoff selection differs",
            ));
        }
        // Additive bootstrap BEFORE Self and the first source-store event.
        // Fixed logical native call/control/error holders, not an RSS theorem.
        let fixed = size_of::<Self>()
            .checked_add(8 * size_of::<OriginalReadStage>())
            .and_then(|n| n.checked_add(8 * size_of::<StoreError>()))
            .and_then(|n| n.checked_add(8 * size_of::<SourceCommandError>()))
            .and_then(|n| n.checked_add(4 * size_of::<SourceMemberV1>()))
            .and_then(|n| n.checked_add(4 * size_of::<RetiredSourceMemberV1>()))
            .and_then(|n| n.checked_add(4 * size_of::<CorpusDescriptor>()))
            .and_then(|n| n.checked_add(8 * size_of::<std::fs::Metadata>()))
            .and_then(|n| n.checked_add(8 * size_of::<File>()))
            .and_then(|n| n.checked_add(64 * size_of::<usize>()))
            .ok_or(SourceCommandError::Unsupported(
                "immutable read bootstrap holders",
            ))?;
        session
            .retain_source_state(fixed)
            .map_err(|e| payer.error(e))?;
        session
            .admit_source_prefix(0, 32)
            .map_err(|e| payer.error(e))?;
        Ok(Self {
            cut,
            session,
            payer,
            deadline,
            cancelled,
        })
    }

    pub(crate) fn member(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        max_bytes: u64,
    ) -> OriginalReadResult<SourceMemberV1, SourceCommandError> {
        // Copy only held references/clock before the short mutable admission
        // loan. The exact source port owns the actual FD and every read slice.
        let cut = self.cut;
        cut.read_member_with_original_admission(
            revision,
            path,
            max_bytes,
            self.deadline,
            self.cancelled,
            self,
        )
    }

    pub(crate) fn retirement(
        &mut self,
        revision: SourceRevision,
        index: usize,
        max_bytes: u64,
    ) -> OriginalReadResult<RetiredSourceMemberV1, SourceCommandError> {
        let cut = self.cut;
        cut.read_retirement_with_original_admission(
            revision,
            index,
            max_bytes,
            self.deadline,
            self.cancelled,
            self,
        )
    }

    fn prefix(&mut self, bytes: usize, visits: usize, holders: usize) -> Result<(), Error> {
        // Never refund a successful earlier prefix if a later component fails.
        self.session.retain_source_state(holders)?;
        self.session.admit_source_prefix(bytes, visits)
    }

    fn price(&mut self, stage: OriginalReadStage) -> Result<(), Error> {
        use OriginalReadStage::*;
        // One fixed logical dispatch/active/arithmetic/callback enclosure per
        // stage; size/capacity-dependent work/holders are additive below.
        self.prefix(0, 32, 0)?;
        match stage {
            Bootstrap {
                kind,
                entries,
                key_bytes,
            } => match kind {
                OriginalReadKind::Member | OriginalReadKind::Retirement => {
                    // The outer cut search compares actual revision digests,
                    // including retirement's key_bytes=0, before any path lookup.
                    self.prefix(times(entries, 2 * size_of::<SourceRevision>())?, entries, 0)
                }
                OriginalReadKind::SelectedObject => {
                    // check_snapshot's Arc identity, member lookup and complete
                    // descriptor revision/digest/size/mode comparisons.
                    let scalar = 2 * size_of::<usize>()
                        + 4 * size_of::<SourceRevision>()
                        + 2 * size_of::<u64>()
                        + 2 * size_of::<u32>();
                    let keys = times(entries, times(2, plus(key_bytes, 1)?)?)?;
                    self.prefix(plus(scalar, keys)?, plus(entries, 5)?, 0)
                }
                OriginalReadKind::RetirementObject => {
                    // This inner route only checks the held snapshot's Arc;
                    // it does not inspect every selected member or path.
                    self.prefix(2 * size_of::<usize>(), 1, 0)
                }
            },
            Lookup { entries, key_bytes } => {
                // Both operands can contribute the query-length common prefix
                // and the first mismatching boundary. This is a complete upper
                // count, not a selected top-N subset of the member map.
                self.prefix(times(entries, times(2, plus(key_bytes, 1)?)?)?, entries, 0)
            }
            Descriptor { path_bytes } => self.prefix(path_bytes, 1, path_bytes),
            Open { .. } => {
                // Digest hex is exactly64 bytes; secure_open checks one leaf,
                // openat2 and opened regular-file metadata. Its fixed native
                // frames are in bootstrap, with this finite path buffer quote.
                self.prefix(256, 8, 256)
            }
            Metadata => self.prefix(0, 4, 0),
            Workspace {
                scratch_bytes,
                hash_state_bytes,
                stage_inline_bytes,
                ..
            } => self.prefix(
                0,
                4,
                plus(scratch_bytes, plus(hash_state_bytes, stage_inline_bytes)?)?,
            ),
            Read { requested_bytes } => self.prefix(requested_bytes, 1, 0),
            Inspect { .. } => self.prefix(0, 8, 0),
            Hash { bytes } => self.prefix(bytes, 1, 0),
            Append {
                length,
                capacity,
                additional,
            } => {
                let (allocation, copy) = growth(length, capacity, additional, 1)?;
                self.prefix(plus(copy, additional)?, 1, allocation)
            }
            Finalize { .. } => self.prefix(256, 4, 0),
            ReturnMember {
                path_bytes,
                identity_entries,
            } => self.prefix(
                plus(
                    times(identity_entries, times(2, plus(path_bytes, 1)?)?)?,
                    path_bytes,
                )?,
                4,
                plus(
                    path_bytes,
                    plus(
                        size_of::<SourceMemberV1>(),
                        size_of::<OriginalReadResult<SourceMemberV1, SourceCommandError>>(),
                    )?,
                )?,
            ),
            ReturnId {
                id_bytes,
                length,
                capacity,
            } => {
                let (allocation, copy) = growth(length, capacity, 1, size_of::<String>())?;
                self.prefix(plus(copy, id_bytes)?, 2, plus(allocation, id_bytes)?)
            }
            ReturnRetirement {
                path_bytes,
                event_ref_bytes,
            } => {
                let bytes = plus(path_bytes, event_ref_bytes)?;
                // Source and separate event RawVecs remain paid/live here.
                // Pay a result header for EVERY return; the constructor's
                // fixed call workspace is not credit for arbitrary later calls.
                let carriers = plus(
                    size_of::<RetiredSourceMemberV1>(),
                    size_of::<OriginalReadResult<RetiredSourceMemberV1, SourceCommandError>>(),
                )?;
                self.prefix(bytes, 2, plus(bytes, carriers)?)
            }
        }
    }
}

impl OriginalReadAdmission for OriginalImmutableReads<'_, '_, '_, '_> {
    type Error = SourceCommandError;

    fn before_original_read(
        &mut self,
        _descriptor: Option<&File>,
        stage: OriginalReadStage,
    ) -> SourceCommandResult<()> {
        // No File method is called here: no offset changes, read, seek, dup,
        // retention or I/O. The original store alone owns the descriptor.
        self.price(stage).map_err(|error| self.payer.error(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_native_cli::private_assessment::OperationBudget;
    use crate::source_native_cli::private_assessment::original_payer::with_original_pwe_session;
    use serde_json::{Value, json};
    use std::{fs, path::Path, time::Duration};
    use tos_compiler::pwe_native::Limits;
    use tos_foundation::{Digest256, JsonLimits};
    use tos_source_store::{
        CorpusReader, CutReadLimits, OriginalReadError, ReadLimits, StoreErrorCode,
    };

    fn copied_tree(from: &Path, to: &Path) {
        fs::create_dir(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copied_tree(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    fn manifest_bytes(value: &Value) -> Vec<u8> {
        // Same ASCII test-manifest derivation as existing source_cut_cases.
        // The original fixture, object bytes and its oracle stay unchanged.
        let mut sorted = value.clone();
        sorted.sort_all_objects();
        let mut raw = serde_json::to_vec(&sorted).unwrap();
        raw.push(b'\n');
        raw
    }

    #[test]
    fn current_archive_and_retirement_share_original_session_and_refusal_prefix() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("store");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/conformance/rust/corpus-v1/store");
        copied_tree(&fixture, &root);
        // Fixture setup/cut loading is OUTSIDE the read invocation. This test
        // certifies the admitted selected-object reads, not whole bootstrap or
        // source/rights/semantic admission. No production data root is chosen.
        let mut previous = None;
        let mut revisions = Vec::new();
        for original in [
            "21e26ca0a4ccca2e99eb67a561a061894c852c6cd5b02006e4592673773419e1",
            "dd5782f4226f3a0000deae0040fae06c3d12cd609769da8564a8e035fc18c971",
        ] {
            let mut manifest: Value = serde_json::from_slice(
                &fs::read(root.join("revisions").join(original).join("snapshot.json")).unwrap(),
            )
            .unwrap();
            let rename = |name: &str| format!("ToS/source-witnesses/fixture/{name}");
            for row in manifest["files"].as_array_mut().unwrap() {
                row["path"] = json!(rename(row["path"].as_str().unwrap()));
            }
            for path in manifest["identities"].as_object_mut().unwrap().values_mut() {
                *path = json!(rename(path.as_str().unwrap()));
            }
            manifest["dependencies"] = Value::Object(
                manifest["dependencies"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(name, refs)| (rename(name), refs.clone()))
                    .collect(),
            );
            manifest["base_revision"] = previous.map(Value::String).unwrap_or(Value::Null);
            manifest.as_object_mut().unwrap().remove("revision");
            let revision = Digest256::of_bytes(&manifest_bytes(&manifest));
            manifest["revision"] = json!(revision.to_hex());
            let home = root.join("revisions").join(revision.to_hex());
            fs::create_dir(home.clone()).unwrap();
            fs::write(home.join("snapshot.json"), manifest_bytes(&manifest)).unwrap();
            previous = Some(revision.to_hex());
            revisions.push(SourceRevision(revision));
        }
        let retained = revisions[0];
        let current = revisions[1];
        let mut manifest: Value = serde_json::from_slice(
            &fs::read(
                root.join("revisions")
                    .join(current.0.to_hex())
                    .join("snapshot.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let old_digest = "1bb1a59e45683de29cf3c0437d7c68001bde5aebb085244fb366a37b4681402a";
        manifest["retirements"] = json!([{
            "path":"ToS/source-witnesses/fixture/retired-alpha.txt","sha256":old_digest,
            "event_ref":"ToS/source-witnesses/fixture/retirement-event.json",
            "event_sha256":"1ff13cf9bf3038c3c0d15db65de831c3c47d1ce9cebeb8cd80ee79bf812fbece",
            "event_size_bytes":20
        }]);
        manifest.as_object_mut().unwrap().remove("revision");
        let current = SourceRevision(Digest256::of_bytes(&manifest_bytes(&manifest)));
        manifest["revision"] = json!(current.0.to_hex());
        let home = root.join("revisions").join(current.0.to_hex());
        fs::create_dir(&home).unwrap();
        fs::write(home.join("snapshot.json"), manifest_bytes(&manifest)).unwrap();
        let cancel = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(30);
        let reader = CorpusReader::open_existing(
            &root,
            ReadLimits {
                max_manifest_bytes: 8192,
                max_manifest_entries: 32,
                max_selected_object_bytes: 1024,
                json: JsonLimits::default(),
            },
        )
        .unwrap();
        let cut = reader
            .open_source_cut(
                current,
                CutReadLimits {
                    max_revisions: 2,
                    max_members: 5,
                    max_total_bytes: 73,
                    max_member_bytes: 20,
                },
                deadline,
                &cancel,
            )
            .unwrap();
        let mut operation = OperationBudget::new(
            &json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}),
            deadline,
        )
        .unwrap();
        let limits = Limits {
            max_input_bytes: 1_048_576,
            max_nodes: 8192,
            max_work: 1_048_576,
            max_refs: 64,
            max_depth: 16,
            max_items: 64,
            max_state_bytes: 8_388_608,
            deadline,
        };
        let result: SourceCommandResult<()> =
            with_original_pwe_session(&mut operation, limits, &cancel, |payer, session| {
                let mut read =
                    OriginalImmutableReads::new(&cut, session, payer, deadline, &cancel)?;
                let alpha =
                    RelativePath::parse("ToS/source-witnesses/fixture/records/alpha.txt").unwrap();
                assert_eq!(
                    read.member(current, &alpha, 20).unwrap().raw,
                    b"A\xc3\xa9+\xce\xa9\r\n"
                );
                assert_eq!(
                    read.member(retained, &alpha, 20).unwrap().raw,
                    b"A\xc3\xa9\r\n"
                );
                let retired = read.retirement(current, 0, 20).unwrap();
                assert_eq!(retired.raw, b"A\xc3\xa9\r\n");
                assert_eq!(retired.event_raw, b"new in revision two\n");
                let paid = read.session.charges();
                assert!(
                    paid.4 >= 4 * 1_048_576,
                    "both retirement objects keep their workspace charges"
                );
                fs::write(root.join("objects").join(old_digest), b"wrong").unwrap();
                assert!(matches!(
                    read.member(retained, &alpha, 20),
                    Err(OriginalReadError::Store(StoreError {
                        code: StoreErrorCode::CorruptSelectedObject,
                        ..
                    }))
                ));
                assert_eq!(
                    read.member(current, &alpha, 20).unwrap().raw,
                    b"A\xc3\xa9+\xce\xa9\r\n"
                );
                assert!(read.session.charges().0 > paid.0);
                // Exhaust this SAME original Session; no new allowance or current
                // fallback replaces an admitted-read prefix after the refusal.
                let rest = read
                    .session
                    .source_allowance()
                    .map_err(|e| payer.error(e))?
                    .state_bytes;
                read.session
                    .retain_source_state(rest)
                    .map_err(|e| payer.error(e))?;
                let spent = read.session.charges();
                let refusal = match read.member(current, &alpha, 20) {
                    Err(OriginalReadError::Admission(error)) => error,
                    _ => panic!("exhausted original Session must refuse before another read"),
                };
                assert_eq!(read.session.charges().4, spent.4);
                assert!(read.session.charges().0 >= spent.0);
                Err(refusal)
            });
        assert!(matches!(
            result,
            Err(SourceCommandError::Unsupported(
                "PWE Assessment Session exhausted"
            ))
        ));
    }
}
