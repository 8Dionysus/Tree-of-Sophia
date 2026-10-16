//! Observes an already committed genuine initial metadata creation package.
//! This is a reader: its retained source mutex spans the prepared transaction.
use super::*;
use crate::source_creation::CreationFamily;
use std::cell::Cell;

pub(crate) struct CommittedMetadataCreationObservation<'a> {
    fs: &'a CreationFilesystem,
    package: &'a SerializedCreation,
    original: &'a CorpusCutReader,
    current: &'a CorpusCutReader,
    context: &'a cmd::CommandContext,
    software: &'a SoftwareCaptureReader,
    components: &'a SoftwareComponentSelectionV1,
    fence: work_transaction::WorkCorpusFence<'a>,
    publication: work_transaction::PublicationSnapshot,
    bytes: Cell<usize>,
    rows: Cell<usize>,
}
impl<'a> CommittedMetadataCreationObservation<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn select(
        fs: &'a CreationFilesystem,
        package: &'a SerializedCreation,
        original: &'a CorpusCutReader,
        current: &'a CorpusCutReader,
        context: &'a cmd::CommandContext,
        software: &'a SoftwareCaptureReader,
        components: &'a SoftwareComponentSelectionV1,
        expected_receipt: Digest256,
        expected_request: Digest256,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        if !matches!(
            package.prepared().family(),
            CreationFamily::PublicProfile | CreationFamily::CorpusV1 | CreationFamily::CorpusV2
        ) {
            return Err(SourceCommandError::Denied(
                "initial metadata creation family",
            ));
        }
        let request = cmd::parse(&package.prepared().context().request_raw)?;
        if cmd::text(&request, "operation")? != "source.create"
            || Digest256::of_bytes(&cmd::canonical(&request)?) != expected_request
            || cmd::field(&request, "expected_revision")? != &JsonValue::Null
            || cmd::field(&request, "expected_source")? != &JsonValue::Null
        {
            return Err(SourceCommandError::Conflict(
                "initial metadata request differs",
            ));
        }
        let record = cmd::field(&request, "record")?;
        if cmd::integer(record, "record_version")? != 1 {
            return Err(SourceCommandError::Conflict("initial metadata version"));
        }
        let receipt = package
            .prepared()
            .files()
            .get("source-create-receipt.json")
            .ok_or(SourceCommandError::Conflict(
                "initial metadata receipt missing",
            ))?;
        if Digest256::of_bytes(receipt) != expected_receipt {
            return Err(SourceCommandError::Conflict(
                "initial metadata receipt selection differs",
            ));
        }
        let receipt_value = cmd::parse(receipt)?;
        if cmd::field(&receipt_value, "grants_admission")? != &JsonValue::Bool(false) {
            return Err(SourceCommandError::Denied(
                "initial metadata admission ceiling",
            ));
        }
        let fence = work_transaction::WorkCorpusFence::hold_existing(fs, deadline, cancelled)?;
        let publication = work_transaction::PublicationSnapshot::select(fs, deadline, cancelled)?;
        let result = Self {
            fs,
            package,
            original,
            current,
            context,
            software,
            components,
            fence,
            publication,
            bytes: Cell::new(0),
            rows: Cell::new(0),
        };
        result.verify_current(deadline, cancelled)?;
        Ok(result)
    }
    pub(crate) fn context(&self) -> &cmd::CommandContext {
        self.context
    }
    pub(crate) fn cut(&self) -> &CorpusCutReader {
        self.current
    }
    pub(crate) fn package(&self) -> &SerializedCreation {
        self.package
    }
    pub(crate) fn publication(&self) -> JsonValue {
        cmd::object(vec![
            ("protocol", cmd::string("tos_selected_source_metadata_v1")),
            (
                "token",
                self.publication
                    .token
                    .as_ref()
                    .map_or(JsonValue::Null, |s| cmd::string(s)),
            ),
            ("generation", cmd::number(self.publication.generation)),
        ])
    }
    pub(crate) fn verify_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.fence.verify(deadline, cancelled)?;
        self.package
            .prepared()
            .context()
            .check_from_selected_captures(
                self.original,
                self.software,
                self.components,
                deadline,
                cancelled,
            )?;
        self.context.check_from_selected_captures(
            self.current,
            self.software,
            self.components,
            deadline,
            cancelled,
        )?;
        if self.context.configuration_raw != self.package.prepared().context().configuration_raw {
            return Err(SourceCommandError::Conflict(
                "initial metadata current configuration differs",
            ));
        }
        self.fs.current(self.package, deadline, cancelled)?;
        self.fs
            .reselect(self.package, self.original, None, true, deadline, cancelled)?;
        self.fs
            .reselect_components(self.software, self.components, deadline, cancelled)?;
        self.publication
            .verify_current(self.fs, deadline, cancelled)?;
        // Complete initial-package reader refuses extra history or sidecar files.
        let retained = self
            .fs
            .read_creation_retained(self.package.prepared(), deadline, cancelled)?
            .ok_or(SourceCommandError::Conflict(
                "initial metadata committed package absent",
            ))?;
        if &retained != self.package.prepared().files() {
            return Err(SourceCommandError::Conflict(
                "initial metadata committed package differs",
            ));
        }
        for (name, raw) in &retained {
            let path = RelativePath::parse(&format!(
                "{}/{}",
                self.package.prepared().home().as_str(),
                name
            ))
            .map_err(|_| SourceCommandError::Invalid("initial metadata package path"))?;
            let captured = self
                .current
                .read_member(
                    self.current.current().revision(),
                    &path,
                    8_388_608,
                    deadline,
                    cancelled,
                )
                .map_err(|_| {
                    SourceCommandError::Conflict("initial metadata current cut package absent")
                })?;
            if &captured.raw != raw {
                return Err(SourceCommandError::Conflict(
                    "initial metadata current cut package differs",
                ));
            }
        }
        self.fence.verify(deadline, cancelled)
    }
    pub(crate) fn read_selected(
        &self,
        path: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        self.fence.verify(deadline, cancelled)?;
        self.publication
            .verify_current(self.fs, deadline, cancelled)?;
        self.fs.current_context(self.context, deadline, cancelled)?;
        if path
            .as_str()
            .split('/')
            .any(|p| ["payload", "local-content", "owner-local"].contains(&p))
        {
            return Err(SourceCommandError::Denied(
                "Metadata private source namespace",
            ));
        }
        let expected_size = if path.as_str().starts_with("ToS/") {
            self.current
                .current()
                .member(path)
                .ok_or(SourceCommandError::Conflict(
                    "Metadata selected member absent",
                ))?
                .size_bytes
        } else {
            self.components
                .members()
                .find(|m| &m.path == path)
                .ok_or(SourceCommandError::Conflict(
                    "Metadata selected software member absent",
                ))?
                .size_bytes
        };
        let size = usize::try_from(expected_size)
            .map_err(|_| SourceCommandError::Invalid("Metadata selected size"))?;
        let bytes = self
            .bytes
            .get()
            .checked_add(size)
            .filter(|n| *n <= 33_554_432)
            .ok_or(SourceCommandError::Invalid("metadata read bytes"))?;
        let rows = self
            .rows
            .get()
            .checked_add(1)
            .filter(|n| *n <= 2048)
            .ok_or(SourceCommandError::Invalid("metadata read rows"))?;
        if size > 8_388_608 {
            return Err(SourceCommandError::Invalid("metadata read member budget"));
        }
        self.bytes.set(bytes);
        self.rows.set(rows);
        let captured = if path.as_str().starts_with("ToS/") {
            self.current
                .read_member(
                    self.current.current().revision(),
                    path,
                    8_388_608,
                    deadline,
                    cancelled,
                )
                .map_err(|_| SourceCommandError::Conflict("metadata selected custody"))?
                .raw
        } else {
            self.software
                .read_selected_component(self.components, path, 8_388_608, deadline, cancelled)
                .map_err(|_| SourceCommandError::Conflict("metadata software custody"))?
        };
        if captured.len() != size {
            return Err(SourceCommandError::Conflict(
                "Metadata selected size differs",
            ));
        }
        let (parent, leaf) = path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("metadata selected parent"))?;
        let directory = walk(&self.fs.root, parent, self.fs.uid)?;
        let mut file = tos_fd_open::open_regular_at(&directory, Path::new(leaf))
            .map_err(|_| SourceCommandError::Conflict("metadata current file absent"))?;
        let signature = stamp(&owned(&file, self.fs.uid, false)?);
        let live = raw(&mut file, 8_388_608, deadline, cancelled)?;
        let fresh = tos_fd_open::open_regular_at(&directory, Path::new(leaf))
            .map_err(|_| SourceCommandError::Conflict("metadata current path replaced"))?;
        if signature != stamp(&owned(&file, self.fs.uid, false)?)
            || signature != stamp(&owned(&fresh, self.fs.uid, false)?)
            || live != captured
        {
            return Err(SourceCommandError::Conflict(
                "metadata current selected bytes changed",
            ));
        }
        self.fence.verify(deadline, cancelled)?;
        Ok(captured)
    }
}
