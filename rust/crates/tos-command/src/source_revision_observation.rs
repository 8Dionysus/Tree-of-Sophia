//! Read-only immediate committed Record proof. The borrowed filesystem and
//! actual corpus mutex remain alive until the caller drops this observation.
use super::*;
use std::cell::Cell;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OriginalRecordBinding {
    pub(crate) raw: Vec<u8>,
    pub(crate) sha256: Digest256,
    pub(crate) subject: JsonValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CurrentRecordBinding {
    pub(crate) raw: Vec<u8>,
    pub(crate) sha256: Digest256,
    pub(crate) subject: JsonValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommittedRecordBinding {
    pub(crate) transaction_id: String,
    pub(crate) record_id: String,
    pub(crate) family: RevisionFamily,
    pub(crate) source_path: RelativePath,
    pub(crate) archive_path: RelativePath,
    pub(crate) original: OriginalRecordBinding,
    pub(crate) current: CurrentRecordBinding,
    pub(crate) manifest_sha256: Digest256,
    pub(crate) original_publication: JsonValue,
    pub(crate) current_publication: JsonValue,
    pub(crate) authorization: JsonValue,
}

pub(crate) struct CommittedRecordObservation<'a> {
    fs: &'a CreationFilesystem,
    ctx: &'a CommandContext,
    current: &'a CorpusCutReader,
    original: &'a CorpusCutReader,
    software: &'a SoftwareCaptureReader,
    components: &'a SoftwareComponentSelectionV1,
    fence: tx::WorkCorpusFence<'a>,
    binding: CommittedRecordBinding,
    original_publication: JsonValue,
    current_publication: JsonValue,
    original_ctx: CommandContext,
    selected_read_bytes: Cell<usize>,
    selected_read_rows: Cell<usize>,
    original_read_bytes: Cell<usize>,
    original_read_rows: Cell<usize>,
}

pub(crate) fn observe_committed<'a>(
    fs: &'a CreationFilesystem,
    ctx: &'a CommandContext,
    current: &'a CorpusCutReader,
    original: &'a CorpusCutReader,
    software: &'a SoftwareCaptureReader,
    components: &'a SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CommittedRecordObservation<'a>> {
    let fence = tx::WorkCorpusFence::hold_existing(fs, deadline, cancelled)?;
    let (binding, original_ctx) = authenticate(
        fs, ctx, current, original, software, components, deadline, cancelled,
    )?;
    fence.verify(deadline, cancelled)?;
    // Catalog publication bindings expose the authenticated journal selection,
    // not the private transaction state or its internal two-field base.
    let publication = |state: &JsonValue| -> SourceCommandResult<JsonValue> {
        Ok(cmd::object(vec![
            ("protocol", cmd::string("tos_selected_source_metadata_v1")),
            ("token", cmd::field(state, "token")?.clone()),
            ("generation", cmd::field(state, "generation")?.clone()),
        ]))
    };
    let original_publication = publication(&binding.original_publication)?;
    let current_publication = publication(&binding.current_publication)?;
    Ok(CommittedRecordObservation {
        fs,
        ctx,
        current,
        original,
        software,
        components,
        fence,
        binding,
        original_publication,
        current_publication,
        original_ctx,
        selected_read_bytes: Cell::new(0),
        selected_read_rows: Cell::new(0),
        original_read_bytes: Cell::new(0),
        original_read_rows: Cell::new(0),
    })
}

impl CommittedRecordObservation<'_> {
    pub(crate) fn transaction_id(&self) -> &str {
        &self.binding.transaction_id
    }
    pub(crate) fn record_id(&self) -> &str {
        &self.binding.record_id
    }
    pub(crate) fn manifest_sha256(&self) -> Digest256 {
        self.binding.manifest_sha256
    }
    pub(crate) fn source_publication(&self) -> &JsonValue {
        &self.current_publication
    }
    pub(crate) fn original_source_publication(&self) -> &JsonValue {
        &self.original_publication
    }
    pub(crate) fn binding(&self) -> &CommittedRecordBinding {
        &self.binding
    }
    pub(crate) fn original_cut(&self) -> &CorpusCutReader {
        self.original
    }
    pub(crate) fn current_cut(&self) -> &CorpusCutReader {
        self.current
    }
    /// Independently authenticated against original_cut, including its actual
    /// revision identity. A historical resolver needs a worker for this cut.
    pub(crate) fn original_context(&self) -> &CommandContext {
        &self.original_ctx
    }
    pub(crate) fn current_context(&self) -> &CommandContext {
        self.ctx
    }

    pub(crate) fn read_current_optional(
        &self,
        path: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<Vec<u8>>> {
        self.verify_read_boundary(path, deadline, cancelled)?;
        if self.current.current().member(path).is_some() {
            return self.read_selected(path, deadline, cancelled).map(Some);
        }
        let rows =
            self.selected_read_rows
                .get()
                .checked_add(1)
                .ok_or(SourceCommandError::Unsupported(
                    "committed Record optional row overflow",
                ))?;
        if rows > 2048 {
            return Err(SourceCommandError::Unsupported(
                "committed Record optional row budget",
            ));
        }
        self.selected_read_rows.set(rows);
        if read(self.fs, path.as_str(), deadline, cancelled)?.is_some() {
            return Err(SourceCommandError::Conflict(
                "committed Record current manifest absence differs",
            ));
        }
        self.verify_read_boundary(path, deadline, cancelled)?;
        Ok(None)
    }

    pub(crate) fn read_original_optional(
        &self,
        path: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<Vec<u8>>> {
        self.verify_read_boundary(path, deadline, cancelled)?;
        if self.original.current().member(path).is_some() {
            return self
                .read_original_selected(path, deadline, cancelled)
                .map(Some);
        }
        let rows =
            self.original_read_rows
                .get()
                .checked_add(1)
                .ok_or(SourceCommandError::Unsupported(
                    "committed Record original optional row overflow",
                ))?;
        if rows > 2048 {
            return Err(SourceCommandError::Unsupported(
                "committed Record original optional row budget",
            ));
        }
        self.original_read_rows.set(rows);
        // Absence is selected from the authenticated complete historical
        // manifest. A later live member cannot rewrite that namespace.
        self.verify_read_boundary(path, deadline, cancelled)?;
        Ok(None)
    }

    fn verify_read_boundary(
        &self,
        path: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        active(deadline, cancelled)?;
        if !path.as_str().starts_with("ToS/") {
            return Err(SourceCommandError::Denied(
                "committed Record optional authored path",
            ));
        }
        self.fence.verify(deadline, cancelled)?;
        self.fs.current_context(self.ctx, deadline, cancelled)?;
        if !snapshot_matches(
            &PublicationSnapshot::select(self.fs, deadline, cancelled)?,
            &self.binding.current_publication,
        )? {
            return Err(SourceCommandError::Conflict(
                "committed Record optional publication changed",
            ));
        }
        Ok(())
    }

    pub(crate) fn verify_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.fence.verify(deadline, cancelled)?;
        let (current, original_ctx) = authenticate(
            self.fs,
            self.ctx,
            self.current,
            self.original,
            self.software,
            self.components,
            deadline,
            cancelled,
        )?;
        if current != self.binding
            || original_ctx.base_revision != self.original_ctx.base_revision
            || original_ctx.files != self.original_ctx.files
            || original_ctx.configuration_raw != self.original_ctx.configuration_raw
            || original_ctx.request_raw != self.original_ctx.request_raw
        {
            return Err(SourceCommandError::Conflict(
                "committed Record observation changed",
            ));
        }
        self.fence.verify(deadline, cancelled)
    }

    /// Only selected authored current-cut members. Preserve exact bytes/order;
    /// each read uses the retained shared mutex and cumulative finite meter.
    pub(crate) fn read_selected(
        &self,
        path: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        active(deadline, cancelled)?;
        self.fence.verify(deadline, cancelled)?;
        self.fs.current_context(self.ctx, deadline, cancelled)?;
        let snapshot = PublicationSnapshot::select(self.fs, deadline, cancelled)?;
        if !snapshot_matches(&snapshot, &self.binding.current_publication)? {
            return Err(SourceCommandError::Conflict(
                "committed Record selected publication changed",
            ));
        }
        if !path.as_str().starts_with("ToS/") {
            return Err(SourceCommandError::Denied(
                "committed Record selected authored path",
            ));
        }
        let member = self
            .current
            .current()
            .member(path)
            .ok_or(SourceCommandError::Denied(
                "committed Record unselected member",
            ))?;
        let size = usize::try_from(member.size_bytes)
            .map_err(|_| SourceCommandError::Unsupported("committed Record selected size"))?;
        let total = self.selected_read_bytes.get().checked_add(size).ok_or(
            SourceCommandError::Unsupported("committed Record selected read overflow"),
        )?;
        let rows =
            self.selected_read_rows
                .get()
                .checked_add(1)
                .ok_or(SourceCommandError::Unsupported(
                    "committed Record selected row overflow",
                ))?;
        if size > 8_388_608 || total > 33_554_432 || rows > 2048 {
            return Err(SourceCommandError::Unsupported(
                "committed Record selected read budget",
            ));
        }
        self.selected_read_bytes.set(total);
        self.selected_read_rows.set(rows);
        let raw = read(self.fs, path.as_str(), deadline, cancelled)?.ok_or(
            SourceCommandError::Conflict("committed Record selected member absent"),
        )?;
        if raw.len() != size || Digest256::of_bytes(&raw) != member.sha256 {
            return Err(SourceCommandError::Conflict(
                "committed Record selected member differs",
            ));
        }
        self.fence.verify(deadline, cancelled)?;
        Ok(raw)
    }

    /// Historical custody reads never compare changed predecessor bytes with
    /// the live path. This namespace has its own finite cumulative meter.
    pub(crate) fn read_original_selected(
        &self,
        path: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        self.verify_read_boundary(path, deadline, cancelled)?;
        let member = self
            .original
            .current()
            .member(path)
            .ok_or(SourceCommandError::Denied(
                "committed Record unselected original member",
            ))?;
        if !path.as_str().starts_with("ToS/") {
            return Err(SourceCommandError::Denied(
                "committed Record original authored path",
            ));
        }
        let size = usize::try_from(member.size_bytes)
            .map_err(|_| SourceCommandError::Unsupported("committed Record original read size"))?;
        let total = self.original_read_bytes.get().checked_add(size).ok_or(
            SourceCommandError::Unsupported("committed Record original read overflow"),
        )?;
        let rows =
            self.original_read_rows
                .get()
                .checked_add(1)
                .ok_or(SourceCommandError::Unsupported(
                    "committed Record original row overflow",
                ))?;
        if size > 8_388_608 || total > 33_554_432 || rows > 2048 {
            return Err(SourceCommandError::Unsupported(
                "committed Record original read budget",
            ));
        }
        self.original_read_bytes.set(total);
        self.original_read_rows.set(rows);
        let selected = self
            .original
            .read_member(
                self.original.current().revision(),
                path,
                8_388_608,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("committed Record original read custody"))?;
        if selected.raw.len() != size || Digest256::of_bytes(&selected.raw) != member.sha256 {
            return Err(SourceCommandError::Conflict(
                "committed Record original read member differs",
            ));
        }
        if path == &self.binding.source_path && selected.raw != self.binding.original.raw {
            return Err(SourceCommandError::Conflict(
                "committed Record original read archive agreement",
            ));
        }
        self.fence.verify(deadline, cancelled)?;
        Ok(selected.raw)
    }
}

fn snapshot_matches(
    snapshot: &PublicationSnapshot,
    state: &JsonValue,
) -> SourceCommandResult<bool> {
    let mut raw = cmd::canonical(state)?;
    raw.push(b'\n');
    Ok(snapshot.member_binding()? == Some((Digest256::of_bytes(&raw), raw.len() as u64)))
}

fn authenticate(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    current: &CorpusCutReader,
    original: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(CommittedRecordBinding, CommandContext)> {
    ctx.check_from_selected_captures(current, software, components, deadline, cancelled)?;
    fs.current_context(ctx, deadline, cancelled)?;
    let (config, family) = revision::configuration(ctx)?;
    let request = cmd::parse(&ctx.request_raw)?;
    if cmd::text(&request, "operation")? != "record.revise" {
        return Err(SourceCommandError::Denied(
            "committed Record exact revise request",
        ));
    }
    if cmd::text(&request, "expected_configuration")? != cmd::record_digest(&config)?.to_prefixed()
        || !cmd::array(&config, "allowed_operations")?
            .iter()
            .any(|v| v.as_str() == Some("record.revise"))
    {
        return Err(SourceCommandError::Denied(
            "committed Record current exact owner grant",
        ));
    }
    if tx::read_pending(fs, deadline, cancelled)?.is_some() {
        return Err(SourceCommandError::Conflict(
            "committed Record publication pending",
        ));
    }
    let transaction = revision::transaction_id(&request)?;
    let (manifest, plan, base_publication, terminal) =
        tx::inspect_committed(fs, &transaction, deadline, cancelled)?;
    if plan.item_path_profile.is_some() || !plan.new_directories.is_empty() || plan.files.len() != 3
    {
        return Err(SourceCommandError::Denied(
            "committed Record exact retained closure",
        ));
    }
    let source = cmd::text(&config, "source_path")?;
    let names = revision::names(source)?;
    let parent = source
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("committed Record parent"))?
        .0;
    let prefix = format!("{parent}/");
    let package_now = package(ctx, &config)?;
    let now_raw = package_now
        .get(&names[0])
        .ok_or(SourceCommandError::Conflict(
            "committed Record current source absent",
        ))?;
    let now = cmd::parse(now_raw)?;
    let now_subject = crate::source_forms::metadata_subject(&now)?;
    let history = revision::history(&package_now, &now)?;
    let receipt = cmd::array(&history, "receipts")?
        .last()
        .ok_or(SourceCommandError::Conflict(
            "committed Record receipt absent",
        ))?;
    if !cmd::same(cmd::field(receipt, "request")?, &request)?
        || !cmd::same(cmd::field(receipt, "source")?, &now_subject)?
    {
        return Err(SourceCommandError::Conflict(
            "committed Record is not immediate exact request successor",
        ));
    }
    let (before, _) = revision::read_archive(ctx, &config, receipt)?;
    let before_raw = before.get(&names[0]).ok_or(SourceCommandError::Conflict(
        "committed Record original source absent",
    ))?;
    let before_record = cmd::parse(before_raw)?;
    let before_subject = crate::source_forms::metadata_subject(&before_record)?;
    if !cmd::same(cmd::field(receipt, "previous_source")?, &before_subject)?
        || cmd::integer(&before_subject, "version")?.checked_add(1)
            != Some(cmd::integer(&now_subject, "version")?)
        || cmd::field(&before_subject, "id")? != cmd::field(&now_subject, "id")?
    {
        return Err(SourceCommandError::Conflict(
            "committed Record original/current identity or version",
        ));
    }
    let mut original_ctx = ctx.clone();
    original_ctx.base_revision = original.current().revision();
    original_ctx
        .files
        .retain(|f| !f.path.as_str().starts_with("ToS/"));
    let mut original_bytes = original_ctx
        .files
        .iter()
        .map(|f| f.raw.len())
        .sum::<usize>();
    for member in original.current().members() {
        active(deadline, cancelled)?;
        let size = usize::try_from(member.size_bytes)
            .map_err(|_| SourceCommandError::Unsupported("committed Record original size"))?;
        original_bytes =
            original_bytes
                .checked_add(size)
                .ok_or(SourceCommandError::Unsupported(
                    "committed Record original bytes overflow",
                ))?;
        if size > 8_388_608 || original_bytes > 33_554_432 || original_ctx.files.len() >= 2048 {
            return Err(SourceCommandError::Unsupported(
                "committed Record original context budget",
            ));
        }
        let selected = original
            .read_member(
                original.current().revision(),
                &member.path,
                8_388_608,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("committed Record original custody"))?;
        original_ctx.files.push(SourceFile {
            path: member.path.clone(),
            raw: selected.raw,
        });
    }
    original_ctx
        .check_from_selected_captures(original, software, components, deadline, cancelled)?;
    if package(&original_ctx, &config)? != before {
        return Err(SourceCommandError::Conflict(
            "committed Record original archive package differs",
        ));
    }
    if !cmd::same(
        &authorization(&original_ctx, &config, family, &before_record)?,
        &plan.authorization,
    )? {
        return Err(SourceCommandError::Denied(
            "committed Record retained owner authorization differs",
        ));
    }
    let expected_paths = names
        .iter()
        .map(|name| format!("{prefix}{name}"))
        .collect::<BTreeSet<_>>();
    if plan
        .files
        .iter()
        .map(|f| f.path.as_str().to_owned())
        .collect::<BTreeSet<_>>()
        != expected_paths
    {
        return Err(SourceCommandError::Denied(
            "committed Record selected path closure",
        ));
    }
    for file in &plan.files {
        let name = file
            .path
            .as_str()
            .strip_prefix(&prefix)
            .ok_or(SourceCommandError::Denied(
                "committed Record selected parent",
            ))?;
        if file.before.as_ref() != before.get(name) || file.after.as_ref() != package_now.get(name)
        {
            return Err(SourceCommandError::Conflict(
                "committed Record retained selected sides differ",
            ));
        }
    }
    let archive = tx::record_revision_archive(
        fs,
        &original_ctx,
        &before_record,
        &before,
        cmd::text(receipt, "previous_revision")?,
        deadline,
        cancelled,
        false,
    )?;
    let snapshot = PublicationSnapshot::select(fs, deadline, cancelled)?;
    if !snapshot_matches(&snapshot, &terminal)? {
        return Err(SourceCommandError::Conflict(
            "committed Record terminal is not current publication",
        ));
    }
    physical::terminal_cut_current(
        fs,
        original,
        &snapshot,
        &plan,
        &base_publication,
        &archive,
        false,
        deadline,
        cancelled,
    )?;
    physical::complete_current_cut(fs, current, &snapshot, deadline, cancelled)?;
    physical::software_current(fs, ctx, deadline, cancelled)?;
    fs.current_context(ctx, deadline, cancelled)?;
    Ok((
        CommittedRecordBinding {
            transaction_id: transaction,
            record_id: cmd::text(&now_subject, "id")?.to_owned(),
            family,
            source_path: RelativePath::parse(source)
                .map_err(|_| SourceCommandError::Invalid("committed Record source path"))?,
            archive_path: RelativePath::parse(cmd::text(receipt, "archive_path")?)
                .map_err(|_| SourceCommandError::Invalid("committed Record archive path"))?,
            original: OriginalRecordBinding {
                raw: before_raw.clone(),
                sha256: Digest256::of_bytes(before_raw),
                subject: before_subject,
            },
            current: CurrentRecordBinding {
                raw: now_raw.clone(),
                sha256: Digest256::of_bytes(now_raw),
                subject: now_subject,
            },
            manifest_sha256: Digest256::from_prefixed(&manifest)
                .map_err(|_| SourceCommandError::Invalid("committed Record manifest digest"))?,
            original_publication: base_publication,
            current_publication: terminal,
            authorization: plan.authorization,
        },
        original_ctx,
    ))
}
