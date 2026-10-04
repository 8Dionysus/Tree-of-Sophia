//! Nested Core owner adapter: same capture/model/context for every admitted frame.
use super::*;
use std::cell::Cell;

pub(super) struct Session {
    pub control: crate::private_stage_run::IssuedConsumerControl,
    pub limits: super::session_transport::Limits,
    pub startup_visits: usize,
}
impl Session {
    pub fn retained_state_upper_bound(&self) -> Result<usize> {
        std::mem::size_of::<Self>()
            .checked_add(
                self.control
                    .retained_state_upper_bound()
                    .map_err(|_| "Core control retained state")?
                    .checked_sub(std::mem::size_of::<
                        crate::private_stage_run::IssuedConsumerControl,
                    >())
                    .ok_or("Core control inline state")?,
            )
            .ok_or("Core session retained state overflow")
    }
}

struct Ledger<'a, 'capture, F> {
    remaining: &'a F,
    reserved: &'a Cell<usize>,
    view: &'a tos_compiler::native_snapshot::CompletedCaptureCarriers<'capture>,
    overhead: usize,
}
impl<F: Fn(usize) -> tos_compiler::Result<usize>> super::session_transport::Workspace
    for Ledger<'_, '_, F>
{
    fn reserve(&mut self, bytes: usize) -> Result<()> {
        let held = self
            .reserved
            .get()
            .checked_add(bytes)
            .ok_or("Core session workspace overflow")?;
        (self.remaining)(
            held.checked_add(self.overhead)
                .ok_or("Core session local owner state overflow")?,
        )
        .map_err(|_| "Core original session workspace allowance")?;
        self.reserved.set(held);
        Ok(())
    }
    fn release(&mut self, bytes: usize) {
        // The transport releases exactly its own once-acquired workspace on every result.
        self.reserved.set(
            self.reserved
                .get()
                .checked_sub(bytes)
                .expect("owned session reservation balance"),
        );
    }
    fn charge_work(&mut self, bytes: u64) -> Result<()> {
        self.view
            .charge_work(bytes)
            .map_err(|_| "Core original session work allowance")
    }
}

// Borrowed literal-key lookup avoids JsonValue::object_get's temporary UTF16 Vec.
fn field<'a>(
    value: &'a tos_foundation::JsonValue,
    name: &str,
) -> Option<&'a tos_foundation::JsonValue> {
    value
        .as_object()?
        .iter()
        .find(|(key, _)| key.as_str() == Some(name))
        .map(|(_, value)| value)
}

/// Exact DTO projection only. URI and tool semantics use their existing native owners.
fn selected_call(
    raw: &[u8],
    sequence: u64,
    request: &Request,
    profile: crate::AccessProfile,
    available: usize,
    remaining_visits: &Cell<usize>,
    view: &tos_compiler::native_snapshot::CompletedCaptureCarriers<'_>,
) -> tos_compiler::Result<(SelectedRootCall<'static>, Instant)> {
    let mut limits = request
        .admission
        .json
        .limits()
        .map_err(tos_compiler::Error::Invalid)?;
    limits.max_visits = limits.max_visits.min(remaining_visits.get());
    view.charge_work(raw.len() as u64)?;
    let doc = tos_foundation::parse_json_with_state_budget(
        raw,
        JsonMode::PublishedStrict,
        limits,
        available,
    )
    .map_err(|_| tos_compiler::Error::Budget("Core session original JSON/state allowance"))?;
    remaining_visits.set(remaining_visits.get().checked_sub(doc.visits()).ok_or(
        tos_compiler::Error::Budget("Core session aggregate JSON visits"),
    )?);
    let root = doc.root();
    let fields = root
        .as_object()
        .ok_or(tos_compiler::Error::Invalid("Core session call object"))?;
    if fields.len() != 5
        || fields.iter().any(|(k, _)| {
            !matches!(
                k.as_str(),
                Some(
                    "schema_version" | "sequence" | "operation" | "arguments" | "work_deadline_ns"
                )
            )
        })
        || field(root, "schema_version").and_then(tos_foundation::JsonValue::as_str)
            != Some("tos_native_core_session_call_v1")
        || field(root, "sequence").and_then(tos_foundation::JsonValue::as_u64) != Some(sequence)
    {
        return Err(tos_compiler::Error::Invalid("Core session strict call DTO"));
    }
    let work = field(root, "work_deadline_ns")
        .and_then(tos_foundation::JsonValue::as_u64)
        .filter(|n| *n > 0 && *n <= request.admission.work_deadline_ns)
        .ok_or(tos_compiler::Error::Invalid(
            "Core session call cutoff extension",
        ))?;
    let deadline = original_cli_deadline(work).map_err(tos_compiler::Error::Invalid)?;
    let args = field(root, "arguments")
        .ok_or(tos_compiler::Error::Invalid("Core session call arguments"))?;
    let (selected, render) = match field(root, "operation")
        .and_then(tos_foundation::JsonValue::as_str)
    {
        Some("tos_native_call") => {
            let fields = args
                .as_object()
                .ok_or(tos_compiler::Error::Invalid("Core session tool DTO"))?;
            if fields.len() != 2
                || fields
                    .iter()
                    .any(|(k, _)| !matches!(k.as_str(), Some("tool" | "arguments")))
                || field(args, "tool").and_then(tos_foundation::JsonValue::as_str)
                    != Some("tos_corpus_graph_views")
            {
                return Err(tos_compiler::Error::Invalid(
                    "Core session tool outside admitted scope",
                ));
            }
            let arguments = field(args, "arguments")
                .ok_or(tos_compiler::Error::Invalid("Core session tool arguments"))?;
            let selected = crate::KnowledgeRequest::from_arguments(
                crate::KnowledgeOperation::CorpusGraphViews,
                arguments,
            )
            .map_err(|_| {
                tos_compiler::Error::Invalid("Core session genuine GraphViews arguments")
            })?;
            if !matches!(
                selected,
                crate::KnowledgeRequest::Corpus(
                    tos_query::corpus_read::CorpusReadRequest::GraphViews
                )
            ) {
                return Err(tos_compiler::Error::Invalid(
                    "Core session selected GraphViews owner",
                ));
            }
            (selected, false)
        }
        Some("tos_native_resource_read") => {
            let fields = args
                .as_object()
                .ok_or(tos_compiler::Error::Invalid("Core session resource DTO"))?;
            if fields.len() != 2
                || fields
                    .iter()
                    .any(|(k, _)| !matches!(k.as_str(), Some("uri" | "render")))
            {
                return Err(tos_compiler::Error::Invalid(
                    "Core session resource argument fields",
                ));
            }
            let uri = field(args, "uri")
                .and_then(tos_foundation::JsonValue::as_str)
                .filter(|uri| *uri == "tos-corpus://graph-views")
                .ok_or(tos_compiler::Error::Invalid(
                    "Core session URI outside admitted scope",
                ))?;
            let selected = crate::mcp_resources::request(uri, profile)
                .map_err(|_| tos_compiler::Error::Invalid("Core session genuine resource owner"))?;
            if !matches!(
                selected,
                crate::KnowledgeRequest::Corpus(
                    tos_query::corpus_read::CorpusReadRequest::GraphViews
                )
            ) {
                return Err(tos_compiler::Error::Invalid(
                    "Core session selected resource owner",
                ));
            }
            let render = field(args, "render")
                .and_then(tos_foundation::JsonValue::as_bool)
                .ok_or(tos_compiler::Error::Invalid(
                    "Core session explicit render flag",
                ))?;
            if render {
                return Err(tos_compiler::Error::Invalid(
                    "Core session rendering unsupported",
                ));
            }
            (selected, false)
        }
        _ => {
            return Err(tos_compiler::Error::Invalid(
                "Core session operation unsupported",
            ));
        }
    };
    // Only this fieldless native request is retained. The decoded frame tree is dropped
    // before GraphViews reservation and execution; no serde Value or second domain parser.
    Ok((SelectedRootCall::Resource(selected, render), deadline))
}

pub(super) fn run_held<'hold, E: crate::ScopedAccessExecutor<'hold> + ?Sized>(
    session: &Session,
    executor: &E,
    request: &Request,
    profile: crate::AccessProfile,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
    view: &tos_compiler::native_snapshot::CompletedCaptureCarriers<'_>,
    source_revision: &str,
    remaining: impl Fn(usize) -> tos_compiler::Result<usize>,
    reserve_query: impl Fn(usize) -> tos_compiler::Result<()>,
) -> tos_compiler::Result<()> {
    session
        .control
        .verify_current()
        .map_err(|_| tos_compiler::Error::Invalid("Core session issued control changed"))?;
    let reserved = Cell::new(0_usize);
    let visits = Cell::new(
        request
            .admission
            .json
            .max_visits
            .checked_sub(session.startup_visits)
            .ok_or(tos_compiler::Error::Budget(
                "Core session startup original JSON visits",
            ))?,
    );
    let mut ledger = Ledger {
        remaining: &remaining,
        reserved: &reserved,
        view,
        overhead: 0,
    };
    let local_state = std::mem::size_of_val(&remaining)
        + std::mem::size_of_val(&reserve_query)
        + std::mem::size_of_val(&profile)
        + std::mem::size_of_val(&ledger)
        + std::mem::size_of_val(&reserved)
        + std::mem::size_of_val(&visits);
    ledger.overhead = local_state;
    let census = |extra: usize| {
        remaining(
            extra
                .checked_add(reserved.get())
                .and_then(|n| n.checked_add(local_state))
                .ok_or(tos_compiler::Error::Budget(
                    "Core session simultaneous state overflow",
                ))?,
        )
    };
    let fence = || {
        active(deadline).map_err(tos_compiler::Error::Invalid)?;
        view.verify_current()?;
        session
            .control
            .verify_current()
            .map_err(|_| tos_compiler::Error::Invalid("Core session actual control fence"))
    };
    super::session_transport::run(session.control.as_fd(),session.limits,deadline,cancelled.as_ref(),&mut ledger,
        |reply| {
            let prefix=br#"{"schema_version":"tos_native_core_session_ready_v1","ok":true,"source_revision":"#;
            let suffix=br#","state_reused":false,"capabilities":[{"operation":"tos_native_call","tool":"tos_corpus_graph_views"},{"operation":"tos_native_resource_read","uri":"tos-corpus://graph-views","render":[false]}]}"#;
            let size=crate::common::json_string_len(source_revision).and_then(|n|n.checked_add(prefix.len())).and_then(|n|n.checked_add(suffix.len()))
                .ok_or("Core session startup size overflow")?;
            let cap=request.http.as_ref().ok_or("Core session profile absent")?.max_startup_receipt_bytes
                .min(session.limits.max_reply_bytes).min(census(0).map_err(|_|"Core session startup state")?);
            let mut out=BoundedOutput::reserved(cap,deadline,size)?;
            out.literal(prefix)?;out.value(&source_revision)?;out.literal(suffix)?;
            view.charge_work(size as u64).map_err(|_|"Core session startup original work")?;
            fence().map_err(|_|"Core session startup fence")?;
            reply.send(super::session_transport::STARTUP,0,&[&out.bytes])?;
            fence().map_err(|_|"Core session startup post fence")
        },
        |sequence,raw,reply| {
            let (call,call_deadline)=selected_call(raw,sequence,request,profile,census(0)
                .map_err(|_|"Core session parser remaining state")?,&visits,view)
                .map_err(|_|"Core session exact call refused")?;
            let call_deadline=call_deadline.min(deadline);
            reply.narrow_deadline(call_deadline)?;
            fence().map_err(|_|"Core session prequery fence")?;
            deliver_selected_root_call(executor,call,request.admission.json.limits().map_err(tos_compiler::Error::Invalid)
                .map_err(|_|"Core session JSON limits")?,profile,call_deadline,cancelled,view,
                census,&reserve_query,
                |bytes|reply.send(super::session_transport::REPLY,sequence,&[bytes])
                    .map_err(tos_compiler::Error::Invalid))
                .map_err(|_|"Core session held query/delivery refused")?;
            fence().map_err(|_|"Core session postquery fence")
        },
        ||fence().map_err(|_|"Core session final source/control fence"),
    ).map_err(tos_compiler::Error::Invalid)
}
