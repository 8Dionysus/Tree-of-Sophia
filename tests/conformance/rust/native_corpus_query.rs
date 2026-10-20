// Private continuation of the existing native corpus producer case.
// This is finite selected projection mechanics, not a production rights issuer.

// Test-owned CurrentDeclared reference. Its inputs are authored files frozen
// before capture/SUT, never a product row/descriptor/materialization. Unsupported
// profiles refuse; a historical golden is not an excuse to copy actual output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReferenceError {
    InvalidBinding,
    UnsupportedSourceProfile,
    DuplicateIdentity,
    Conflict,
    Budget,
}
type NavResult<T> = Result<T, ReferenceError>;

// PRIVATE move bridge: only a fully admitted PublishedStrict JsonDocument may
// enter. It preserves the entire tree, not a projection. The pinned serde map
// first-key protocol is intentionally outside this reference profile.
enum ReferenceMoveFrame {
    Array {
        remaining: std::vec::IntoIter<tos_foundation::JsonValue>,
        output: Vec<serde_json::Value>,
    },
    Object {
        remaining: std::vec::IntoIter<(tos_foundation::JsonString, tos_foundation::JsonValue)>,
        output: serde_json::Map<String, serde_json::Value>,
        pending_key: Option<String>,
    },
}
struct ReferenceMoveState {
    frames: Vec<ReferenceMoveFrame>,
    depth: usize,
    next: Option<tos_foundation::JsonValue>,
    ready: Option<serde_json::Value>,
    available: usize,
}

// These are requested logical construction bounds for the pinned preserve_order
// representation, not allocator usable size, compiler stack or RSS bounds.
fn reference_move_layout() -> NavResult<()> {
    use std::mem::{align_of, size_of};
    if !cfg!(all(target_arch = "x86_64", target_pointer_width = "64"))
        || size_of::<usize>() != 8
        || size_of::<String>() != 24
        || size_of::<Vec<serde_json::Value>>() != 24
        || size_of::<serde_json::Map<String, serde_json::Value>>() != 72
        || size_of::<serde_json::Number>() != 24
        || size_of::<serde_json::Value>() > 128
        || align_of::<usize>() > 8
        || align_of::<String>() > 8
        || align_of::<serde_json::Value>() > 8
        || align_of::<serde_json::Number>() > 8
        || align_of::<serde_json::Map<String, serde_json::Value>>() > 8
    {
        return Err(ReferenceError::UnsupportedSourceProfile);
    }
    Ok(())
}
fn reference_move_workspace() -> NavResult<usize> {
    use std::mem::size_of;
    // Explicit outside holders: document/move carriers; frame/value transitions;
    // scalar parse result; moved keys; checked arithmetic; admission results;
    // helper arguments. No unused scratch array stands in for those holders.
    // Use each holder's largest actual Option/Result/moved spelling rather than
    // assuming an unverified enum niche/layout identity for those transitions.
    size_of::<ReferenceMoveState>()
        .checked_add(reference_stack_workspace::<ReferenceMoveFrame>()?)
        .and_then(|n| {
            n.checked_add(
                size_of::<tos_foundation::JsonDocument>()
                    .max(size_of::<Option<tos_foundation::JsonValue>>())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                size_of::<ReferenceMoveFrame>()
                    .max(size_of::<Option<ReferenceMoveFrame>>())
                    .max(size_of::<NavResult<ReferenceMoveFrame>>())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                size_of::<serde_json::Value>()
                    .max(size_of::<Option<serde_json::Value>>())
                    .max(size_of::<NavResult<serde_json::Value>>())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| n.checked_add(size_of::<Result<serde_json::Number, serde_json::Error>>()))
        .and_then(|n| {
            n.checked_add(
                size_of::<Option<String>>()
                    .max(size_of::<NavResult<String>>())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| n.checked_add(size_of::<Option<usize>>().checked_mul(8)?))
        .and_then(|n| n.checked_add(size_of::<NavResult<()>>().checked_mul(4)?))
        .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(4)?))
        // Implementation inventory amendment: iterator pair/result and pattern
        // move holders; fat key borrows / frame-helper references; scalar
        // parameters outside the checked-arithmetic result slots above. All are
        // prepaid in the SAME remainder, not new workspace or a fresh pool.
        .and_then(|n| {
            n.checked_add(
                size_of::<Option<(tos_foundation::JsonString, tos_foundation::JsonValue)>>()
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| n.checked_add(size_of::<&str>().checked_mul(8)?))
        .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(4)?))
        .ok_or(ReferenceError::Budget)
}
// Explicit actual holders for the two concrete move/scan stack types below.
// This is not a generic public-value or allocator/RSS certificate.
fn reference_stack_workspace<T>() -> NavResult<usize> {
    use std::mem::size_of;
    size_of::<Vec<T>>()
        .checked_mul(2)
        .and_then(|n| {
            n.checked_add(
                size_of::<Result<(), std::collections::TryReserveError>>().checked_mul(2)?,
            )
        })
        .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(8)?))
        .and_then(|n| n.checked_add(size_of::<Option<usize>>().checked_mul(8)?))
        .and_then(|n| n.checked_add(size_of::<&mut Vec<T>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&mut ReferenceLimits<'_>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<NavResult<()>>().checked_mul(2)?))
        .ok_or(ReferenceError::Budget)
}
fn reference_stack_room<T>(
    frames: &mut Vec<T>,
    available: usize,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<()> {
    use std::mem::size_of;
    l.step(1)?;
    let len = frames.len();
    let capacity = frames.capacity();
    let needed = len.checked_add(1).ok_or(ReferenceError::Budget)?;
    if needed > 97 || capacity > 97 {
        return Err(ReferenceError::Budget);
    }
    if len == capacity {
        let next = capacity
            .checked_mul(2)
            .ok_or(ReferenceError::Budget)?
            .max(4)
            .min(97)
            .max(needed);
        let slots = next
            .checked_mul(size_of::<T>())
            .ok_or(ReferenceError::Budget)?;
        let old = capacity
            .checked_mul(size_of::<T>())
            .ok_or(ReferenceError::Budget)?;
        let moved = len
            .checked_mul(size_of::<T>())
            .ok_or(ReferenceError::Budget)?;
        canonical_state_room(old, slots, available)?;
        l.bytes(slots.checked_add(moved).ok_or(ReferenceError::Budget)?)?;
        l.step(1)?;
        frames
            .try_reserve_exact(next.checked_sub(len).ok_or(ReferenceError::Budget)?)
            .map_err(|_| ReferenceError::Budget)?;
        // The pinned requested-capacity law supplies admission BEFORE reserve.
        if frames.capacity() != next {
            return Err(ReferenceError::Budget);
        }
    }
    Ok(())
}
fn reference_move_map_backing(m: usize) -> NavResult<usize> {
    use std::mem::{align_of, size_of};
    if m == 0 {
        return Ok(0);
    }
    // Q bounds IndexMap's hash/key/value Bucket including alignment. Fresh
    // exact-M construction plus exactly M distinct insertions prevents growth.
    let a = align_of::<usize>()
        .max(align_of::<String>())
        .max(align_of::<serde_json::Value>());
    let q = size_of::<usize>()
        .checked_add(size_of::<String>())
        .and_then(|n| n.checked_add(size_of::<serde_json::Value>()))
        .and_then(|n| n.checked_add(a.checked_mul(4)?))
        .and_then(|n| n.checked_add(a.checked_sub(1)?))
        .map(|n| n / a * a)
        .ok_or(ReferenceError::Budget)?;
    let b = m
        .checked_mul(2)
        .map(|n| n.max(4))
        .and_then(usize::checked_next_power_of_two)
        .ok_or(ReferenceError::Budget)?;
    // Hashbrown: aligned usize buckets, B control bytes and one trailing group.
    m.checked_mul(q)
        .and_then(|n| {
            b.checked_mul(size_of::<usize>())
                .and_then(|x| x.checked_add(15))
                .map(|x| x / 16 * 16)
                .and_then(|x| n.checked_add(x))
        })
        .and_then(|n| n.checked_add(b))
        .and_then(|n| n.checked_add(16))
        .ok_or(ReferenceError::Budget)
}
fn reference_move_number(
    number: tos_foundation::JsonNumber,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<serde_json::Value> {
    // Same pinned scalar primitive as serde Value deserialization. The lexeme
    // is a COMPLETE number token validated by the preceding whole strict parse.
    // No f64 cast, unchecked constructor, normalization or arbitrary-string API.
    l.step(1)?;
    reference_number_from_lexeme(&number.lexeme, l).map(serde_json::Value::Number)
}
fn reference_number_from_lexeme(
    lexeme: &str,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<serde_json::Number> {
    let length = lexeme.len();
    let capacity = length
        .max(16)
        .checked_next_power_of_two()
        .ok_or(ReferenceError::Budget)?;
    l.bytes(
        capacity
            .checked_add(length.max(20))
            .and_then(|n| n.checked_add(256))
            .ok_or(ReferenceError::Budget)?,
    )?;
    l.bytes(
        length
            .checked_mul(3)
            .and_then(|n| n.checked_add(32))
            .ok_or(ReferenceError::Budget)?,
    )?;
    lexeme
        .parse::<serde_json::Number>()
        .map_err(|_| ReferenceError::InvalidBinding)
}
fn move_reference_document(
    doc: tos_foundation::JsonDocument,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<serde_json::Value> {
    l.step(1)?;
    if doc.mode() != tos_foundation::JsonMode::PublishedStrict {
        return Err(ReferenceError::InvalidBinding);
    }
    move_reference_value(doc.into_root(), l)
}
fn move_reference_value(
    root: tos_foundation::JsonValue,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<serde_json::Value> {
    use serde_json::Value;
    use tos_foundation::JsonValue as Source;
    reference_move_layout()?;
    l.bytes(reference_move_workspace()?)?; // BEFORE Vec/state/growth holders
    let mut state = ReferenceMoveState {
        frames: Vec::new(),
        depth: 0,
        next: Some(root),
        ready: None,
        available: l.work.left,
    };
    loop {
        l.step(1)?; // transition, including terminal/error transitions
        if let Some(value) = state.next.take() {
            l.step(1)?; // complete source node, before inspecting/moving it
            match value {
                Source::Null => state.ready = Some(Value::Null),
                Source::Bool(value) => state.ready = Some(Value::Bool(value)),
                Source::String(value) => {
                    state.ready = Some(Value::String(
                        value.into_utf8().ok_or(ReferenceError::InvalidBinding)?,
                    ));
                }
                Source::Number(value) => state.ready = Some(reference_move_number(value, l)?),
                Source::Array(values) => {
                    if state.depth == 97 {
                        return Err(ReferenceError::Budget);
                    }
                    reference_stack_room(&mut state.frames, state.available, l)?;
                    l.bytes(
                        values
                            .len()
                            .checked_mul(std::mem::size_of::<Value>())
                            .ok_or(ReferenceError::Budget)?,
                    )?;
                    l.step(1)?; // ordinary frame write; storage/growth bytes remain separate
                    state.frames.push(ReferenceMoveFrame::Array {
                        output: Vec::with_capacity(values.len()),
                        remaining: values.into_iter(),
                    });
                    state.depth += 1;
                }
                Source::Object(values) => {
                    if state.depth == 97 {
                        return Err(ReferenceError::Budget);
                    }
                    if let Some((key, _)) = values.first() {
                        l.step(1)?;
                        let key = key.as_str().ok_or(ReferenceError::InvalidBinding)?;
                        l.bytes(
                            key.len()
                                .min(29)
                                .checked_mul(2)
                                .ok_or(ReferenceError::Budget)?,
                        )?;
                        if key == "$serde_json::private::Number"
                            || key == "$serde_json::private::RawValue"
                        {
                            return Err(ReferenceError::UnsupportedSourceProfile);
                        }
                    }
                    // Keep ALL first-key semantic errors before stack growth.
                    reference_stack_room(&mut state.frames, state.available, l)?;
                    l.bytes(reference_move_map_backing(values.len())?)?;
                    l.step(1)?; // ordinary frame write; storage/growth bytes remain separate
                    state.frames.push(ReferenceMoveFrame::Object {
                        output: serde_json::Map::with_capacity(values.len()),
                        remaining: values.into_iter(),
                        pending_key: None,
                    });
                    state.depth += 1;
                }
            }
            continue;
        }
        if state.ready.is_some() {
            if state.depth == 0 {
                return state.ready.take().ok_or(ReferenceError::InvalidBinding);
            }
            l.step(1)?; // parent insertion, before key hashing / child move
            match state
                .frames
                .get_mut(state.depth - 1)
                .ok_or(ReferenceError::InvalidBinding)?
            {
                ReferenceMoveFrame::Array { output, .. } => {
                    if output.len() == output.capacity() {
                        return Err(ReferenceError::InvalidBinding);
                    }
                    output.push(state.ready.take().ok_or(ReferenceError::InvalidBinding)?);
                }
                ReferenceMoveFrame::Object {
                    output,
                    pending_key,
                    ..
                } => {
                    l.bytes(
                        pending_key
                            .as_ref()
                            .ok_or(ReferenceError::InvalidBinding)?
                            .len()
                            .checked_add(std::mem::size_of::<usize>())
                            .ok_or(ReferenceError::Budget)?,
                    )?;
                    if output
                        .insert(
                            pending_key.take().ok_or(ReferenceError::InvalidBinding)?,
                            state.ready.take().ok_or(ReferenceError::InvalidBinding)?,
                        )
                        .is_some()
                    {
                        return Err(ReferenceError::DuplicateIdentity);
                    }
                }
            }
            continue;
        }
        if state.depth == 0 {
            return Err(ReferenceError::InvalidBinding);
        }
        l.step(1)?; // next child/key or completed frame, before iterator access
        match state
            .frames
            .get_mut(state.depth - 1)
            .ok_or(ReferenceError::InvalidBinding)?
        {
            ReferenceMoveFrame::Array { remaining, .. } => {
                state.next = remaining.next();
            }
            ReferenceMoveFrame::Object {
                remaining,
                pending_key,
                ..
            } => {
                if pending_key.is_some() {
                    return Err(ReferenceError::InvalidBinding);
                }
                if let Some((key, child)) = remaining.next() {
                    l.step(1)?;
                    *pending_key = Some(key.into_utf8().ok_or(ReferenceError::InvalidBinding)?);
                    state.next = Some(child);
                }
            }
        }
        if state.next.is_none() {
            l.step(1)?; // take the finished frame; never clone its output
            l.step(1)?; // ordinary frame write; storage/growth bytes remain separate
            state.depth -= 1;
            state.ready = Some(
                match state.frames.pop().ok_or(ReferenceError::InvalidBinding)? {
                    ReferenceMoveFrame::Array { output, .. } => Value::Array(output),
                    ReferenceMoveFrame::Object {
                        output,
                        pending_key: None,
                        ..
                    } => Value::Object(output),
                    _ => return Err(ReferenceError::InvalidBinding),
                },
            );
        }
    }
}

struct ReferenceLimits<'a> {
    work: &'a mut CurrentReference,
    cancelled: &'a AtomicBool,
    max_files: usize,
    max_input_bytes: usize,
    max_file_bytes: usize,
    max_record_bytes: usize,
    max_claim_rows: usize,
    max_claim_bytes: usize,
    max_rows: usize,
    max_row_bytes: usize,
    max_output_bytes: usize,
}
impl<'a> ReferenceLimits<'a> {
    fn for_case(
        work: &'a mut CurrentReference,
        cancelled: &'a AtomicBool,
        case: &tos_compiler::source_bibliographic::BibliographicLimits,
    ) -> NavResult<Self> {
        Ok(Self {
            work,
            cancelled,
            max_files: usize::try_from(case.catalog.max_files)
                .map_err(|_| ReferenceError::Budget)?,
            max_input_bytes: case.catalog.max_contract_bytes,
            max_file_bytes: case.catalog.max_file_bytes,
            max_record_bytes: case.catalog.max_row_bytes,
            max_claim_rows: case.max_claim_cohort_rows,
            max_claim_bytes: case.max_claim_cohort_bytes,
            max_rows: usize::try_from(case.max_output_rows.min(case.catalog.max_rows))
                .map_err(|_| ReferenceError::Budget)?,
            max_row_bytes: case.catalog.max_output_row_bytes,
            max_output_bytes: usize::try_from(case.max_output_bytes)
                .map_err(|_| ReferenceError::Budget)?,
        })
    }
    fn reborrow(&mut self) -> ReferenceLimits<'_> {
        ReferenceLimits {
            work: &mut *self.work,
            cancelled: self.cancelled,
            max_files: self.max_files,
            max_input_bytes: self.max_input_bytes,
            max_file_bytes: self.max_file_bytes,
            max_record_bytes: self.max_record_bytes,
            max_claim_rows: self.max_claim_rows,
            max_claim_bytes: self.max_claim_bytes,
            max_rows: self.max_rows,
            max_row_bytes: self.max_row_bytes,
            max_output_bytes: self.max_output_bytes,
        }
    }
}
impl ReferenceLimits<'_> {
    #[track_caller]
    fn step(&mut self, n: usize) -> NavResult<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            self.work.note_refusal("cancelled", None, Some(n));
            return Err(ReferenceError::Budget);
        }
        self.work.visit(n).map_err(|_| ReferenceError::Budget)
    }
    #[track_caller]
    fn bytes(&mut self, n: usize) -> NavResult<()> {
        self.step(0)?;
        self.work.charge(n).map_err(|_| ReferenceError::Budget)
    }
    // Charge logical owned headers/text and structural visits BEFORE cloning,
    // serde conversion or output. Not an allocator capacity/RSS certificate.
    fn value_bound(&mut self, v: &serde_json::Value, depth: usize) -> NavResult<usize> {
        use serde_json::Value;
        self.step(1)?;
        if depth > 96 {
            return Err(ReferenceError::Budget);
        }
        self.bytes(128)?;
        let wire = match v {
            Value::Null => 4,
            Value::Bool(_) => 5,
            Value::Number(n) if n.is_i64() || n.is_u64() => 20,
            Value::Number(n) if n.as_f64().is_some_and(f64::is_finite) => 64,
            Value::Number(_) => return Err(ReferenceError::UnsupportedSourceProfile),
            Value::String(s) => {
                let n = s
                    .len()
                    .checked_mul(6)
                    .and_then(|n| n.checked_add(2))
                    .ok_or(ReferenceError::Budget)?;
                self.bytes(n)?;
                n
            }
            Value::Array(a) => {
                let mut n = 2usize;
                for x in a {
                    n = n
                        .checked_add(self.value_bound(x, depth + 1)?)
                        .and_then(|n| n.checked_add(1))
                        .ok_or(ReferenceError::Budget)?;
                }
                n
            }
            Value::Object(o) => {
                let mut n = 2usize;
                for (k, x) in o {
                    self.step(1)?;
                    let key = k
                        .len()
                        .checked_mul(6)
                        .and_then(|n| n.checked_add(4))
                        .ok_or(ReferenceError::Budget)?;
                    self.bytes(key.checked_add(96).ok_or(ReferenceError::Budget)?)?;
                    let child = self.value_bound(x, depth + 1)?;
                    n = n
                        .checked_add(key)
                        .and_then(|n| n.checked_add(child))
                        .ok_or(ReferenceError::Budget)?;
                }
                n
            }
        };
        if wire > self.max_output_bytes {
            return Err(ReferenceError::Budget);
        }
        Ok(wire)
    }
    fn compare_work(&mut self, v: &serde_json::Value, depth: usize) -> NavResult<()> {
        self.step(2)?; // this admission walk and the following whole comparison
        if depth > 96 {
            return Err(ReferenceError::Budget);
        }
        match v {
            serde_json::Value::String(s) => {
                self.bytes(s.len().checked_mul(2).ok_or(ReferenceError::Budget)?)?
            }
            serde_json::Value::Array(a) => {
                for child in a {
                    self.compare_work(child, depth + 1)?;
                }
            }
            serde_json::Value::Object(o) => {
                for (key, child) in o {
                    self.step(2)?;
                    self.bytes(key.len().checked_mul(2).ok_or(ReferenceError::Budget)?)?;
                    self.compare_work(child, depth + 1)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    fn copy(&mut self, v: &serde_json::Value) -> NavResult<serde_json::Value> {
        self.value_bound(v, 0)?;
        Ok(v.clone())
    }
    fn json_limits(&self, bytes: usize) -> NavResult<tos_foundation::JsonLimits> {
        tos_foundation::JsonLimits::new(bytes, 96, self.work.visits_left, 4096)
            .map_err(|_| ReferenceError::Budget)
    }
    fn parse(&mut self, raw: &[u8], ceiling: usize) -> NavResult<serde_json::Value> {
        let doc = self.strict_document(raw, ceiling)?;
        move_reference_document(doc, self)
    }
    fn strict_document(
        &mut self,
        raw: &[u8],
        ceiling: usize,
    ) -> NavResult<tos_foundation::JsonDocument> {
        use tos_foundation::{FoundationError, FoundationErrorCode as Code, JsonMode};
        self.step(1)?;
        if raw.len() > ceiling {
            return Err(ReferenceError::Budget);
        }
        let json = self.json_limits(ceiling)?;
        let available = self.work.left;
        let deadline = self.work.deadline;
        let cancelled = self.cancelled;
        let doc = tos_foundation::parse_json_with_state_budget_and_admission(
            raw,
            JsonMode::PublishedStrict,
            json,
            available,
            &mut || {
                if Instant::now() < deadline && !cancelled.load(Ordering::Relaxed) {
                    Ok(())
                } else {
                    Err(FoundationError::new(
                        Code::BudgetExceeded,
                        "reference cutoff",
                    ))
                }
            },
            &mut |bytes, visits| {
                self.work
                    .charge(bytes)
                    .and_then(|_| self.work.visit(visits))
                    .map_err(|_| FoundationError::new(Code::BudgetExceeded, "reference parse work"))
            },
        )
        .map_err(|e| {
            if e.code == Code::BudgetExceeded {
                ReferenceError::Budget
            } else {
                ReferenceError::InvalidBinding
            }
        })?;
        self.step(doc.visits())?; // retained-state walk, distinct from parser visits
        self.bytes(
            doc.root()
                .retained_storage_bytes()
                .map_err(|_| ReferenceError::Budget)?,
        )?;
        Ok(doc)
    }
    fn retained_root(
        &mut self,
        raw: &[u8],
        ceiling: usize,
    ) -> NavResult<tos_foundation::JsonValue> {
        self.bytes(
            std::mem::size_of::<tos_foundation::JsonDocument>()
                .checked_add(std::mem::size_of::<tos_foundation::JsonValue>())
                .and_then(|n| {
                    n.checked_add(std::mem::size_of::<NavResult<tos_foundation::JsonValue>>())
                })
                .ok_or(ReferenceError::Budget)?,
        )?;
        let doc = self.strict_document(raw, ceiling)?;
        self.step(1)?;
        reference_move_layout()?; // same selected-layout refusal before scalar compatibility
        // Compatibility is WHOLE-document and precedes any requested subtree.
        // Unknown/unselected fields receive the same scalar/first-key checks.
        reference_source_scan(doc.root(), self, true)?;
        Ok(doc.into_root())
    }
    fn canonical_value(&mut self, v: &serde_json::Value) -> NavResult<Vec<u8>> {
        use tos_foundation::{CanonicalProfile, FoundationError, FoundationErrorCode as Code};
        // Caller-owned plan/root and return/move holders coexist with either
        // private workspace; pay them BEFORE constructing the first plan.
        self.bytes(
            std::mem::size_of::<CanonicalPlan>()
                .checked_add(std::mem::size_of::<NavResult<CanonicalPlan>>())
                .and_then(|n| n.checked_add(std::mem::size_of::<tos_foundation::JsonValue>()))
                .and_then(|n| {
                    n.checked_add(std::mem::size_of::<NavResult<tos_foundation::JsonValue>>())
                })
                .and_then(|n| n.checked_add(std::mem::size_of::<&serde_json::Value>()))
                .and_then(|n| n.checked_add(std::mem::size_of::<&mut ReferenceLimits<'_>>()))
                .ok_or(ReferenceError::Budget)?,
        )?;
        let plan = canonical_preflight(v, self)?;
        let root = canonical_construct(v, plan.max_frames, self)?;
        // Same complete retained walk/debit; construction is not a free tree.
        self.step(plan.meta.values)?;
        self.bytes(
            root.retained_storage_bytes()
                .map_err(|_| ReferenceError::Budget)?,
        )?;
        let json = self.json_limits(self.max_output_bytes)?;
        let available = self.work.left;
        let deadline = self.work.deadline;
        let cancelled = self.cancelled;
        let (bytes, _) =
            tos_foundation::canonical_bytes_v1_with_state_budget_and_visits_and_admission(
                &root,
                CanonicalProfile::SourceRecordDigestV1,
                json,
                available,
                &mut || {
                    if Instant::now() < deadline && !cancelled.load(Ordering::Relaxed) {
                        Ok(())
                    } else {
                        Err(FoundationError::new(
                            Code::BudgetExceeded,
                            "reference cutoff",
                        ))
                    }
                },
                &mut |bytes, visits| {
                    self.work
                        .charge(bytes)
                        .and_then(|_| self.work.visit(visits))
                        .map_err(|_| {
                            FoundationError::new(Code::BudgetExceeded, "reference canonical work")
                        })
                },
            )
            .map_err(|_| ReferenceError::Budget)?;
        Ok(bytes)
    }
    fn canonical_value_legacy(&mut self, v: &serde_json::Value) -> NavResult<Vec<u8>> {
        use tos_foundation::{
            CanonicalProfile, FoundationError, FoundationErrorCode as Code, JsonMode,
        };
        let bound = self.value_bound(v, 0)?;
        self.bytes(bound)?;
        let raw = serde_json::to_vec(v).map_err(|_| ReferenceError::InvalidBinding)?;
        let json = self.json_limits(self.max_output_bytes)?;
        let available = self.work.left;
        let deadline = self.work.deadline;
        let cancelled = self.cancelled;
        let doc = tos_foundation::parse_json_with_state_budget_and_admission(
            &raw,
            JsonMode::PublishedStrict,
            json,
            available,
            &mut || {
                if Instant::now() < deadline && !cancelled.load(Ordering::Relaxed) {
                    Ok(())
                } else {
                    Err(FoundationError::new(
                        Code::BudgetExceeded,
                        "reference cutoff",
                    ))
                }
            },
            &mut |bytes, visits| {
                self.work
                    .charge(bytes)
                    .and_then(|_| self.work.visit(visits))
                    .map_err(|_| {
                        FoundationError::new(Code::BudgetExceeded, "reference digest parse work")
                    })
            },
        )
        .map_err(|e| {
            if e.code == Code::BudgetExceeded {
                ReferenceError::Budget
            } else {
                ReferenceError::InvalidBinding
            }
        })?;
        // Retained-state walk and canonical passes share the ORIGINAL remainder.
        self.step(doc.visits())?;
        self.bytes(
            doc.root()
                .retained_storage_bytes()
                .map_err(|_| ReferenceError::Budget)?,
        )?;
        let json = self.json_limits(self.max_output_bytes)?;
        let available = self.work.left;
        let deadline = self.work.deadline;
        let cancelled = self.cancelled;
        let (bytes, _) =
            tos_foundation::canonical_bytes_v1_with_state_budget_and_visits_and_admission(
                doc.root(),
                CanonicalProfile::SourceRecordDigestV1,
                json,
                available,
                &mut || {
                    if Instant::now() < deadline && !cancelled.load(Ordering::Relaxed) {
                        Ok(())
                    } else {
                        Err(FoundationError::new(
                            Code::BudgetExceeded,
                            "reference cutoff",
                        ))
                    }
                },
                &mut |bytes, visits| {
                    self.work
                        .charge(bytes)
                        .and_then(|_| self.work.visit(visits))
                        .map_err(|_| {
                            FoundationError::new(Code::BudgetExceeded, "reference canonical work")
                        })
                },
            )
            .map_err(|_| ReferenceError::Budget)?;
        Ok(bytes)
    }
    fn digest(&mut self, v: &serde_json::Value) -> NavResult<String> {
        let bytes = self.canonical_value(v)?;
        self.bytes(bytes.len().checked_add(64).ok_or(ReferenceError::Budget)?)?;
        Ok(tos_foundation::Digest256::of_bytes(&bytes).to_hex())
    }
    fn fixed(&mut self, fields: usize) -> NavResult<()> {
        self.step(fields)?;
        self.bytes(fields.checked_mul(192).ok_or(ReferenceError::Budget)?)
    }
}
// This is a different CONSTRUCTION, not a cheaper coefficient for value_bound.
// The old conservative subtree guards and the maintained final writer remain.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PreflightPurpose {
    CanonicalConstruction,
    MergeReadGuard,
    VariantWire,
}
// Closed private counter payloads: old modes never own wire-only fields.
// One traversal and all current visits remain; no dynamically boxed callback.
trait CanonicalWireCounts: Copy {
    type Key: Copy;
    fn scalar(input_len: usize) -> Self;
    fn key(input_len: usize) -> Self::Key;
    fn fold(&mut self, child: Self, key: Option<Self::Key>) -> NavResult<()>;
}
#[derive(Clone, Copy)]
struct NoWireCounts;
impl CanonicalWireCounts for NoWireCounts {
    type Key = ();
    fn scalar(_: usize) -> Self {
        Self
    }
    fn key(_: usize) {}
    fn fold(&mut self, _: Self, _: Option<()>) -> NavResult<()> {
        Ok(())
    }
}
#[derive(Clone, Copy)]
struct VariantWireCounts {
    reread: usize,
    keys: usize,
}
impl CanonicalWireCounts for VariantWireCounts {
    type Key = usize;
    fn scalar(input_len: usize) -> Self {
        Self {
            reread: input_len,
            keys: 0,
        }
    }
    fn key(input_len: usize) -> usize {
        input_len
    }
    fn fold(&mut self, child: Self, key: Option<usize>) -> NavResult<()> {
        self.reread = self
            .reread
            .checked_add(child.reread)
            .and_then(|n| n.checked_add(key.map_or(0, |k| k)))
            .ok_or(ReferenceError::Budget)?;
        self.keys = self
            .keys
            .checked_add(child.keys)
            .and_then(|n| n.checked_add(usize::from(key.is_some())))
            .ok_or(ReferenceError::Budget)?;
        Ok(())
    }
}
#[derive(Clone, Copy)]
struct CanonicalMeta<C: CanonicalWireCounts = NoWireCounts> {
    wire: usize,
    raw: usize,
    values: usize,
    extra: C,
}
#[derive(Clone, Copy)]
struct CanonicalKey<C: CanonicalWireCounts = NoWireCounts> {
    wire: usize,
    raw: usize,
    extra: C::Key,
}
struct CanonicalText {
    raw: usize,
    units: usize,
}
fn canonical_text(text: &str, l: &mut ReferenceLimits<'_>) -> NavResult<CanonicalText> {
    let mut raw = 2usize;
    let mut units = 0usize;
    let mut at = 0usize;
    while at < text.len() {
        l.step(1)?;
        let mut end = at
            .checked_add(4096)
            .ok_or(ReferenceError::Budget)?
            .min(text.len());
        l.bytes(4)?; // at most one UTF8 scalar's boundary probes BEFORE reads
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        l.bytes(end - at)?; // BEFORE the complete bounded UTF8 metrics read
        for ch in text[at..end].chars() {
            units = units
                .checked_add(ch.len_utf16())
                .ok_or(ReferenceError::Budget)?;
            let n = match ch {
                '"' | '\\' | '\u{8}' | '\t' | '\n' | '\u{c}' | '\r' => 2,
                '\u{0}'..='\u{1f}' => 6,
                _ => ch.len_utf8(),
            };
            raw = raw.checked_add(n).ok_or(ReferenceError::Budget)?;
        }
        at = end;
    }
    Ok(CanonicalText { raw, units })
}
enum CanonicalPlanFrame<'a, C: CanonicalWireCounts = NoWireCounts> {
    Array {
        remaining: std::slice::Iter<'a, serde_json::Value>,
        meta: CanonicalMeta<C>,
        members: usize,
    },
    Object {
        remaining: serde_json::map::Iter<'a>,
        meta: CanonicalMeta<C>,
        members: usize,
        pending: Option<CanonicalKey<C>>,
    },
}
struct CanonicalPlan<C: CanonicalWireCounts = NoWireCounts> {
    meta: CanonicalMeta<C>,
    max_frames: usize,
}
struct CanonicalPlanState<'a, C: CanonicalWireCounts = NoWireCounts> {
    frames: Vec<CanonicalPlanFrame<'a, C>>,
    next: Option<&'a serde_json::Value>,
    ready: Option<CanonicalMeta<C>>,
    depth: usize,
    max_frames: usize,
    available: usize,
}
fn canonical_plan_workspace<C: CanonicalWireCounts>() -> NavResult<usize> {
    use std::mem::size_of;
    size_of::<CanonicalPlanState<'_, C>>()
        .checked_add(
            size_of::<PreflightPurpose>()
                .checked_mul(2)
                .ok_or(ReferenceError::Budget)?,
        )
        .and_then(|n| n.checked_add(size_of::<bool>().checked_mul(2)?))
        // Purpose wrapper/inner argument borrows coexist; no input clone.
        .and_then(|n| n.checked_add(size_of::<&serde_json::Value>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&mut ReferenceLimits<'_>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<Vec<CanonicalPlanFrame<'_, C>>>().checked_mul(2)?))
        .and_then(|n| {
            n.checked_add(
                size_of::<Result<(), std::collections::TryReserveError>>().checked_mul(2)?,
            )
        })
        .and_then(|n| n.checked_add(size_of::<Option<usize>>().checked_mul(8)?))
        .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(8)?))
        .and_then(|n| {
            n.checked_add(
                size_of::<CanonicalPlan<C>>()
                    .max(size_of::<NavResult<CanonicalPlan<C>>>())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                size_of::<CanonicalPlanFrame<'_, C>>()
                    .max(size_of::<Option<CanonicalPlanFrame<'_, C>>>())
                    .max(size_of::<NavResult<CanonicalPlanFrame<'_, C>>>())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                size_of::<CanonicalMeta<C>>()
                    .max(size_of::<Option<CanonicalMeta<C>>>())
                    .max(size_of::<NavResult<CanonicalMeta<C>>>())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                size_of::<CanonicalText>()
                    .max(size_of::<Option<CanonicalText>>())
                    .max(size_of::<NavResult<CanonicalText>>())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| n.checked_add(size_of::<Option<CanonicalKey<C>>>().checked_mul(2)?))
        .and_then(|n| {
            n.checked_add(size_of::<Option<(&String, &serde_json::Value)>>().checked_mul(2)?)
        })
        .and_then(|n| n.checked_add(size_of::<std::str::Chars<'_>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&str>().checked_mul(8)?))
        .and_then(|n| n.checked_add(size_of::<Option<usize>>().checked_mul(16)?))
        .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(16)?))
        .and_then(|n| n.checked_add(size_of::<char>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<NavResult<()>>().checked_mul(4)?))
        .and_then(|n| n.checked_add(size_of::<Option<f64>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&serde_json::Number>().checked_mul(2)?))
        // Closed trait call overlap: receiver/child/key/scalar arguments and
        // Result forwarding are paid even for the zero-payload old modes.
        .and_then(|n| n.checked_add(size_of::<C>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<C::Key>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<Option<C::Key>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&mut C>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<NavResult<()>>().checked_mul(2)?))
        .ok_or(ReferenceError::Budget)
}
// Only the currently full borrowed-frame stack grows. No source shape pass.
fn canonical_plan_room<C: CanonicalWireCounts>(
    frames: &mut Vec<CanonicalPlanFrame<'_, C>>,
    available: usize,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<()> {
    use std::mem::size_of;
    l.step(1)?;
    let len = frames.len();
    let capacity = frames.capacity();
    let needed = len.checked_add(1).ok_or(ReferenceError::Budget)?;
    if needed > 97 || capacity > 97 {
        return Err(ReferenceError::Budget);
    }
    if len == capacity {
        let next = capacity
            .checked_mul(2)
            .ok_or(ReferenceError::Budget)?
            .max(4)
            .min(97)
            .max(needed);
        let slots = next
            .checked_mul(size_of::<CanonicalPlanFrame<'_, C>>())
            .ok_or(ReferenceError::Budget)?;
        let old = capacity
            .checked_mul(size_of::<CanonicalPlanFrame<'_, C>>())
            .ok_or(ReferenceError::Budget)?;
        let moved = len
            .checked_mul(size_of::<CanonicalPlanFrame<'_, C>>())
            .ok_or(ReferenceError::Budget)?;
        canonical_state_room(old, slots, available)?;
        l.bytes(slots.checked_add(moved).ok_or(ReferenceError::Budget)?)?;
        l.step(1)?;
        frames
            .try_reserve_exact(next.checked_sub(len).ok_or(ReferenceError::Budget)?)
            .map_err(|_| ReferenceError::Budget)?;
        // Pinned requested-capacity law, not an after-allocation admission.
        if frames.capacity() != next {
            return Err(ReferenceError::Budget);
        }
    }
    Ok(())
}
fn canonical_preflight(
    v: &serde_json::Value,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<CanonicalPlan> {
    l.step(1)?;
    l.bytes(std::mem::size_of::<PreflightPurpose>())?; // BEFORE purpose argument
    canonical_preflight_for::<NoWireCounts>(v, l, PreflightPurpose::CanonicalConstruction)
}
// READ guard has no cloned row or serialized wire. Its caller already owns
// the paid complete value (or a live immutable borrow), never a SUT row.
fn merge_read_bound(v: &serde_json::Value, l: &mut ReferenceLimits<'_>) -> NavResult<usize> {
    use std::mem::size_of;
    l.step(1)?;
    l.bytes(
        size_of::<CanonicalPlan>()
            .checked_add(size_of::<PreflightPurpose>())
            .and_then(|n| n.checked_add(size_of::<NavResult<CanonicalPlan>>()))
            .and_then(|n| n.checked_add(size_of::<&serde_json::Value>()))
            .and_then(|n| n.checked_add(size_of::<&mut ReferenceLimits<'_>>()))
            .and_then(|n| n.checked_add(size_of::<NavResult<usize>>()))
            .ok_or(ReferenceError::Budget)?,
    )?;
    Ok(
        canonical_preflight_for::<NoWireCounts>(v, l, PreflightPurpose::MergeReadGuard)?
            .meta
            .wire,
    )
}
fn canonical_preflight_for<C: CanonicalWireCounts>(
    v: &serde_json::Value,
    l: &mut ReferenceLimits<'_>,
    purpose: PreflightPurpose,
) -> NavResult<CanonicalPlan<C>> {
    use serde_json::Value;
    l.step(1)?;
    reference_move_layout()?;
    l.bytes(canonical_plan_workspace::<C>()?)?; // BEFORE Vec/header/transition state
    let mut state = CanonicalPlanState::<C> {
        frames: Vec::new(),
        next: Some(v),
        ready: None,
        depth: 0,
        max_frames: 0,
        available: l.work.left,
    };
    loop {
        l.step(1)?;
        if let Some(value) = state.next.take() {
            if state.depth > 96 {
                return Err(ReferenceError::Budget);
            }
            let meta = match value {
                Value::Null => Some(CanonicalMeta::<C> {
                    wire: 4,
                    raw: 4,
                    values: 1,
                    extra: C::scalar(0),
                }),
                Value::Bool(b) => Some(CanonicalMeta::<C> {
                    wire: 5,
                    raw: if *b { 4 } else { 5 },
                    values: 1,
                    extra: C::scalar(0),
                }),
                Value::Number(n) => {
                    let raw = n.as_str(); // pinned arbitrary_precision serialization spelling
                    l.bytes(raw.len().checked_mul(3).ok_or(ReferenceError::Budget)?)?;
                    l.step(3)?;
                    let wire = if n.is_i64() || n.is_u64() {
                        20
                    } else if n.as_f64().is_some_and(f64::is_finite) {
                        64
                    } else {
                        return Err(ReferenceError::UnsupportedSourceProfile);
                    };
                    l.step(1)?;
                    Some(CanonicalMeta::<C> {
                        wire,
                        raw: raw.len(),
                        values: 1,
                        extra: C::scalar(raw.len()),
                    })
                }
                Value::String(text) => {
                    let wire = text
                        .len()
                        .checked_mul(6)
                        .and_then(|n| n.checked_add(2))
                        .ok_or(ReferenceError::Budget)?;
                    l.step(1)?; // purpose/length metadata, not a fictitious copy
                    let metrics = if purpose != PreflightPurpose::MergeReadGuard {
                        canonical_text(text, l)?
                    } else {
                        CanonicalText { raw: 0, units: 0 }
                    };
                    l.step(1)?;
                    Some(CanonicalMeta::<C> {
                        wire,
                        raw: metrics.raw,
                        values: 1,
                        extra: C::scalar(text.len()),
                    })
                }
                Value::Array(values) => {
                    canonical_plan_room(&mut state.frames, state.available, l)?;
                    l.bytes(std::mem::size_of::<CanonicalPlanFrame<'_, C>>())?;
                    state.frames.push(CanonicalPlanFrame::Array {
                        remaining: values.iter(),
                        meta: CanonicalMeta::<C> {
                            wire: 2,
                            raw: 2,
                            values: 1,
                            extra: C::scalar(0),
                        },
                        members: 0,
                    });
                    state.depth += 1;
                    state.max_frames = state.max_frames.max(state.depth);
                    None
                }
                Value::Object(values) => {
                    canonical_plan_room(&mut state.frames, state.available, l)?;
                    l.bytes(std::mem::size_of::<CanonicalPlanFrame<'_, C>>())?;
                    state.frames.push(CanonicalPlanFrame::Object {
                        remaining: values.iter(),
                        meta: CanonicalMeta::<C> {
                            wire: 2,
                            raw: 2,
                            values: 1,
                            extra: C::scalar(0),
                        },
                        members: 0,
                        pending: None,
                    });
                    state.depth += 1;
                    state.max_frames = state.max_frames.max(state.depth);
                    None
                }
            };
            if let Some(meta) = meta {
                if meta.wire > l.max_output_bytes {
                    return Err(ReferenceError::Budget);
                }
                state.ready = Some(meta);
            }
            continue;
        }
        if let Some(child) = state.ready.take() {
            if state.depth == 0 {
                // Exactly the old ACTUAL whole-wire limit, AFTER all old-order W guards.
                l.step(1)?;
                if purpose == PreflightPurpose::CanonicalConstruction
                    && child.raw > l.max_output_bytes
                {
                    return Err(ReferenceError::Budget);
                }
                return Ok(CanonicalPlan {
                    meta: child,
                    max_frames: state.max_frames,
                });
            }
            let (meta, members, key) = match state
                .frames
                .get_mut(state.depth - 1)
                .ok_or(ReferenceError::InvalidBinding)?
            {
                CanonicalPlanFrame::Array { meta, members, .. } => (meta, members, None),
                CanonicalPlanFrame::Object {
                    meta,
                    members,
                    pending,
                    ..
                } => (
                    meta,
                    members,
                    Some(pending.take().ok_or(ReferenceError::InvalidBinding)?),
                ),
            };
            let (wire_extra, raw_extra) = if let Some(key) = key {
                (
                    key.wire,
                    key.raw.checked_add(1).ok_or(ReferenceError::Budget)?,
                )
            } else {
                (1, 0)
            };
            meta.wire = if key.is_some() {
                meta.wire
                    .checked_add(wire_extra)
                    .and_then(|n| n.checked_add(child.wire))
            } else {
                meta.wire
                    .checked_add(child.wire)
                    .and_then(|n| n.checked_add(1))
            }
            .ok_or(ReferenceError::Budget)?;
            l.step(1)?;
            if purpose != PreflightPurpose::MergeReadGuard {
                meta.raw = meta
                    .raw
                    .checked_add(raw_extra)
                    .and_then(|n| n.checked_add(child.raw))
                    .and_then(|n| n.checked_add(usize::from(*members != 0)))
                    .ok_or(ReferenceError::Budget)?;
            }
            l.step(2)?; // exact-wire input rereads/key count, no extra tree pass
            meta.extra.fold(child.extra, key.map(|k| k.extra))?;
            meta.values = meta
                .values
                .checked_add(child.values)
                .ok_or(ReferenceError::Budget)?;
            *members = members.checked_add(1).ok_or(ReferenceError::Budget)?;
            continue;
        }
        if state.depth == 0 {
            return Err(ReferenceError::InvalidBinding);
        }
        let frame = state
            .frames
            .get_mut(state.depth - 1)
            .ok_or(ReferenceError::InvalidBinding)?;
        state.next = match frame {
            CanonicalPlanFrame::Array { remaining, .. } => remaining.next(),
            CanonicalPlanFrame::Object {
                remaining, pending, ..
            } => {
                if pending.is_some() {
                    return Err(ReferenceError::InvalidBinding);
                }
                if let Some((key, value)) = remaining.next() {
                    l.step(1)?;
                    let wire = key
                        .len()
                        .checked_mul(6)
                        .and_then(|n| n.checked_add(4))
                        .ok_or(ReferenceError::Budget)?;
                    // Preserve old key+header checked arithmetic BEFORE child;
                    // its old clone admission is replaced by actual constructor work.
                    wire.checked_add(96).ok_or(ReferenceError::Budget)?;
                    l.step(1)?; // purpose/length metadata, BEFORE child
                    let metrics = if purpose != PreflightPurpose::MergeReadGuard {
                        canonical_text(key, l)?
                    } else {
                        CanonicalText { raw: 0, units: 0 }
                    };
                    // Key is NOT a serde Value: no new independent key W guard.
                    *pending = Some(CanonicalKey::<C> {
                        wire,
                        raw: metrics.raw,
                        extra: C::key(key.len()),
                    });
                    Some(value)
                } else {
                    None
                }
            }
        };
        if state.next.is_none() {
            l.bytes(std::mem::size_of::<CanonicalPlanFrame<'_, C>>())?;
            state.depth -= 1;
            let meta = match state.frames.pop().ok_or(ReferenceError::InvalidBinding)? {
                CanonicalPlanFrame::Array { meta, .. } => meta,
                CanonicalPlanFrame::Object {
                    meta,
                    pending: None,
                    ..
                } => meta,
                _ => return Err(ReferenceError::InvalidBinding),
            };
            // Same POSTORDER guard, before any next sibling/ancestor completion.
            if meta.wire > l.max_output_bytes {
                return Err(ReferenceError::Budget);
            }
            state.ready = Some(meta);
        }
    }
}

enum CanonicalBuildFrame<'a> {
    Array {
        remaining: std::slice::Iter<'a, serde_json::Value>,
        output: Vec<tos_foundation::JsonValue>,
    },
    Object {
        remaining: serde_json::map::Iter<'a>,
        output: Vec<(tos_foundation::JsonString, tos_foundation::JsonValue)>,
        pending: Option<tos_foundation::JsonString>,
    },
}
struct CanonicalBuildState<'a> {
    frames: Vec<CanonicalBuildFrame<'a>>,
    next: Option<&'a serde_json::Value>,
    ready: Option<tos_foundation::JsonValue>,
    depth: usize,
    held: usize,
    available: usize,
}
fn canonical_build_workspace() -> NavResult<usize> {
    use std::mem::size_of;
    use tos_foundation::{JsonDocument, JsonString, JsonValue as Source};
    size_of::<CanonicalBuildState<'_>>()
        .checked_add(
            size_of::<Vec<CanonicalBuildFrame<'_>>>()
                .checked_mul(2)
                .ok_or(ReferenceError::Budget)?,
        )
        .and_then(|n| n.checked_add(size_of::<Option<usize>>().checked_mul(4)?))
        .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(4)?))
        .and_then(|n| {
            n.checked_add(
                size_of::<CanonicalBuildFrame<'_>>()
                    .max(size_of::<Option<CanonicalBuildFrame<'_>>>())
                    .max(size_of::<NavResult<CanonicalBuildFrame<'_>>>())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                size_of::<Source>()
                    .max(size_of::<Option<Source>>())
                    .max(size_of::<NavResult<Source>>())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                size_of::<JsonString>()
                    .max(size_of::<Option<JsonString>>())
                    .max(size_of::<NavResult<JsonString>>())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                size_of::<JsonDocument>()
                    .max(size_of::<
                        Result<JsonDocument, tos_foundation::FoundationError>,
                    >())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                size_of::<CanonicalText>()
                    .max(size_of::<Option<CanonicalText>>())
                    .max(size_of::<NavResult<CanonicalText>>())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| {
            n.checked_add(size_of::<Option<(&String, &serde_json::Value)>>().checked_mul(2)?)
        })
        .and_then(|n| n.checked_add(size_of::<std::str::EncodeUtf16<'_>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<std::str::Chars<'_>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<[u16; 2]>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&str>().checked_mul(8)?))
        .and_then(|n| n.checked_add(size_of::<Option<usize>>().checked_mul(16)?))
        .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(16)?))
        .and_then(|n| n.checked_add(size_of::<NavResult<()>>().checked_mul(4)?))
        .and_then(|n| n.checked_add(size_of::<tos_foundation::Result<usize>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<tos_foundation::JsonMode>().checked_mul(2)?))
        // Scalar parser limits/result, captured cutoff variables/closures and
        // their trait-object argument holders coexist with the build state.
        .and_then(|n| {
            n.checked_add(
                size_of::<tos_foundation::JsonLimits>()
                    .max(size_of::<NavResult<tos_foundation::JsonLimits>>())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| n.checked_add(size_of::<Instant>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&AtomicBool>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&mut ReferenceLimits<'_>>().checked_mul(2)?))
        .and_then(|n| {
            n.checked_add(
                size_of::<&mut dyn FnMut() -> tos_foundation::Result<()>>().checked_mul(2)?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                size_of::<&mut dyn FnMut(usize, usize) -> tos_foundation::Result<()>>()
                    .checked_mul(2)?,
            )
        })
        .ok_or(ReferenceError::Budget)
}
fn canonical_state_room(held: usize, peak: usize, available: usize) -> NavResult<()> {
    if held.checked_add(peak).ok_or(ReferenceError::Budget)? > available {
        return Err(ReferenceError::Budget);
    }
    Ok(())
}
fn canonical_string(
    text: &str,
    held: &mut usize,
    available: usize,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<tos_foundation::JsonString> {
    let metrics = canonical_text(text, l)?;
    let c = if metrics.units == 0 {
        0
    } else {
        metrics
            .units
            .checked_mul(2)
            .ok_or(ReferenceError::Budget)?
            .max(4)
    };
    let retained = c
        .checked_mul(2)
        .and_then(|n| n.checked_add(text.len()))
        .ok_or(ReferenceError::Budget)?;
    let peak = c
        .checked_mul(3)
        .and_then(|n| n.checked_add(text.len()))
        .ok_or(ReferenceError::Budget)?;
    canonical_state_room(*held, peak, available)?;
    // Pinned collect/doubling law: allocated UTF16 sum<4C; moved bytes<2C;
    // UTF8 allocation L and encoding/copy reads 2L. No generic allocator claim.
    l.bytes(
        c.checked_mul(6)
            .and_then(|n| text.len().checked_mul(3).and_then(|x| n.checked_add(x)))
            .ok_or(ReferenceError::Budget)?,
    )?;
    l.step(3)?;
    let value = tos_foundation::JsonString::from_utf8(text);
    l.step(1)?; // fresh cutoff after the bounded maintained constructor
    if value
        .retained_storage_bytes()
        .map_err(|_| ReferenceError::Budget)?
        > retained
    {
        return Err(ReferenceError::Budget);
    }
    *held = held.checked_add(retained).ok_or(ReferenceError::Budget)?;
    Ok(value)
}
fn canonical_number(
    number: &serde_json::Number,
    held: &mut usize,
    available: usize,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<tos_foundation::JsonValue> {
    use tos_foundation::{
        FoundationError, FoundationErrorCode as Code, JsonMode, JsonValue as Source,
    };
    let raw = number.as_str().as_bytes();
    let capacity = if raw.is_empty() {
        0
    } else {
        raw.len()
            .checked_mul(2)
            .ok_or(ReferenceError::Budget)?
            .max(8)
    };
    canonical_state_room(
        *held,
        capacity.checked_mul(3).ok_or(ReferenceError::Budget)?,
        available,
    )?;
    // Additional prospective numeric String growth allowance, not a raw copy.
    // The maintained parser still charges its COMPLETE own scalar route below.
    l.bytes(capacity)?;
    l.step(1)?;
    let mut json = l.json_limits(l.max_output_bytes)?;
    json.max_depth = 1; // typed numeric COMPONENT; whole tree still checks depth96
    json.max_visits = 1; // every callback remains on SAME aggregate CurrentReference
    let state_available = available
        .checked_sub(*held)
        .and_then(|n| n.checked_sub(capacity.checked_mul(3)?))
        .ok_or(ReferenceError::Budget)?
        .min(l.work.left);
    let deadline = l.work.deadline;
    let cancelled = l.cancelled;
    let doc = tos_foundation::parse_json_with_state_budget_and_admission(
        raw,
        JsonMode::PublishedStrict,
        json,
        state_available,
        &mut || {
            if Instant::now() < deadline && !cancelled.load(Ordering::Relaxed) {
                Ok(())
            } else {
                Err(FoundationError::new(
                    Code::BudgetExceeded,
                    "reference numeric cutoff",
                ))
            }
        },
        &mut |bytes, visits| {
            l.work
                .charge(bytes)
                .and_then(|_| l.work.visit(visits))
                .map_err(|_| FoundationError::new(Code::BudgetExceeded, "reference numeric work"))
        },
    )
    .map_err(|e| {
        if e.code == Code::BudgetExceeded {
            ReferenceError::Budget
        } else {
            ReferenceError::InvalidBinding
        }
    })?;
    let root = doc.into_root();
    match root {
        Source::Number(number) => {
            if number.lexeme.capacity() > capacity {
                return Err(ReferenceError::Budget);
            }
            *held = held.checked_add(capacity).ok_or(ReferenceError::Budget)?;
            Ok(Source::Number(number))
        }
        _ => Err(ReferenceError::InvalidBinding),
    }
}
fn canonical_construct(
    v: &serde_json::Value,
    max_frames: usize,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<tos_foundation::JsonValue> {
    use serde_json::Value;
    use std::mem::size_of;
    use tos_foundation::JsonValue as Source;
    l.step(1)?;
    l.bytes(canonical_build_workspace()?)?; // BEFORE state/Vec/transition holders
    if max_frames > 97 {
        return Err(ReferenceError::Budget);
    }
    l.bytes(
        max_frames
            .checked_mul(size_of::<CanonicalBuildFrame<'_>>())
            .ok_or(ReferenceError::Budget)?,
    )?; // persistent slots BEFORE allocation
    l.step(1)?;
    let frames = Vec::with_capacity(max_frames);
    if frames.capacity() != max_frames {
        return Err(ReferenceError::Budget);
    }
    let mut state = CanonicalBuildState {
        frames,
        next: Some(v),
        ready: None,
        depth: 0,
        held: 0,
        available: l.work.left,
    };
    loop {
        l.step(1)?;
        if let Some(value) = state.next.take() {
            if state.depth > 96 {
                return Err(ReferenceError::Budget);
            }
            let ready = match value {
                Value::Null => Some(Source::Null),
                Value::Bool(v) => Some(Source::Bool(*v)),
                Value::Number(v) => Some(canonical_number(v, &mut state.held, state.available, l)?),
                Value::String(v) => Some(Source::String(canonical_string(
                    v,
                    &mut state.held,
                    state.available,
                    l,
                )?)),
                Value::Array(values) => {
                    if state
                        .frames
                        .len()
                        .checked_add(1)
                        .ok_or(ReferenceError::Budget)?
                        > max_frames
                    {
                        return Err(ReferenceError::Budget);
                    }
                    let bytes = values
                        .len()
                        .checked_mul(size_of::<Source>())
                        .ok_or(ReferenceError::Budget)?;
                    canonical_state_room(state.held, bytes, state.available)?;
                    l.bytes(bytes)?;
                    l.step(1)?;
                    state.held = state
                        .held
                        .checked_add(bytes)
                        .ok_or(ReferenceError::Budget)?;
                    l.bytes(size_of::<CanonicalBuildFrame<'_>>())?;
                    state.frames.push(CanonicalBuildFrame::Array {
                        remaining: values.iter(),
                        output: Vec::with_capacity(values.len()),
                    });
                    state.depth += 1;
                    None
                }
                Value::Object(values) => {
                    if state
                        .frames
                        .len()
                        .checked_add(1)
                        .ok_or(ReferenceError::Budget)?
                        > max_frames
                    {
                        return Err(ReferenceError::Budget);
                    }
                    let bytes = values
                        .len()
                        .checked_mul(size_of::<(tos_foundation::JsonString, Source)>())
                        .ok_or(ReferenceError::Budget)?;
                    canonical_state_room(state.held, bytes, state.available)?;
                    l.bytes(bytes)?;
                    l.step(1)?;
                    state.held = state
                        .held
                        .checked_add(bytes)
                        .ok_or(ReferenceError::Budget)?;
                    l.bytes(size_of::<CanonicalBuildFrame<'_>>())?;
                    state.frames.push(CanonicalBuildFrame::Object {
                        remaining: values.iter(),
                        output: Vec::with_capacity(values.len()),
                        pending: None,
                    });
                    state.depth += 1;
                    None
                }
            };
            state.ready = ready;
            continue;
        }
        if let Some(value) = state.ready.take() {
            if state.depth == 0 {
                return Ok(value);
            }
            match state
                .frames
                .get_mut(state.depth - 1)
                .ok_or(ReferenceError::InvalidBinding)?
            {
                CanonicalBuildFrame::Array { output, .. } => {
                    l.bytes(size_of::<Source>())?;
                    output.push(value);
                }
                CanonicalBuildFrame::Object {
                    output, pending, ..
                } => {
                    l.bytes(size_of::<(tos_foundation::JsonString, Source)>())?;
                    output.push((pending.take().ok_or(ReferenceError::InvalidBinding)?, value));
                }
            }
            continue;
        }
        if state.depth == 0 {
            return Err(ReferenceError::InvalidBinding);
        }
        state.next = match state
            .frames
            .get_mut(state.depth - 1)
            .ok_or(ReferenceError::InvalidBinding)?
        {
            CanonicalBuildFrame::Array { remaining, .. } => remaining.next(),
            CanonicalBuildFrame::Object {
                remaining, pending, ..
            } => {
                if pending.is_some() {
                    return Err(ReferenceError::InvalidBinding);
                }
                if let Some((key, child)) = remaining.next() {
                    *pending = Some(canonical_string(key, &mut state.held, state.available, l)?);
                    Some(child)
                } else {
                    None
                }
            }
        };
        if state.next.is_none() {
            l.bytes(size_of::<CanonicalBuildFrame<'_>>())?;
            state.depth -= 1;
            state.ready = Some(
                match state.frames.pop().ok_or(ReferenceError::InvalidBinding)? {
                    CanonicalBuildFrame::Array { output, .. } => Source::Array(output),
                    CanonicalBuildFrame::Object {
                        output,
                        pending: None,
                        ..
                    } => Source::Object(output),
                    _ => return Err(ReferenceError::InvalidBinding),
                },
            );
        }
    }
}

// A bounded borrowed scan, not a second owned DOM. Every retained root is
// PublishedStrict plus this complete compatibility walk BEFORE any selection.
// It deliberately does not apply canonical-value's finite-number/W checks to
// unselected fields: the old parse facade did not apply those checks either.
struct ReferenceSourceScan<'a> {
    frames: Vec<(&'a tos_foundation::JsonValue, usize)>,
    depth: usize,
    available: usize,
}
fn reference_source_scan(
    root: &tos_foundation::JsonValue,
    l: &mut ReferenceLimits<'_>,
    compatibility: bool,
) -> NavResult<()> {
    use std::mem::size_of;
    use tos_foundation::JsonValue as Source;
    let outside = size_of::<Option<(&Source, usize)>>()
        .checked_mul(2)
        .and_then(|n| n.checked_add(size_of::<&str>().checked_mul(8)?))
        .and_then(|n| n.checked_add(size_of::<Option<usize>>().checked_mul(8)?))
        .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(6)?))
        .and_then(|n| n.checked_add(size_of::<NavResult<()>>().checked_mul(4)?))
        .and_then(|n| n.checked_add(size_of::<Result<serde_json::Number, serde_json::Error>>()))
        .ok_or(ReferenceError::Budget)?;
    l.bytes(
        size_of::<ReferenceSourceScan<'_>>()
            .checked_add(reference_stack_workspace::<(&Source, usize)>()?)
            .and_then(|n| n.checked_add(outside))
            .ok_or(ReferenceError::Budget)?,
    )?;
    let mut state = ReferenceSourceScan {
        frames: Vec::new(),
        depth: 0,
        available: l.work.left,
    };
    // The scan has ROOT and SCALAR frames too, not container-only frames.
    reference_stack_room(&mut state.frames, state.available, l)?;
    l.step(1)?; // ordinary frame write; storage/growth bytes remain separate
    state.frames.push((root, 0));
    state.depth = 1;
    while state.depth != 0 {
        l.step(1)?; // frame/terminal access, including refusal prefixes
        let (value, cursor) = state
            .frames
            .get(state.depth - 1)
            .copied()
            .ok_or(ReferenceError::InvalidBinding)?;
        if cursor == 0 {
            l.step(if compatibility { 1 } else { 3 })?;
            // false: this scan + upcoming retained-storage walk + clone walk.
            match value {
                Source::String(v) => {
                    let text = v.as_str().ok_or(ReferenceError::InvalidBinding)?;
                    l.bytes(if compatibility {
                        text.len()
                    } else {
                        v.units()
                            .len()
                            .checked_mul(2)
                            .and_then(|n| n.checked_add(text.len()))
                            .ok_or(ReferenceError::Budget)?
                    })?;
                }
                Source::Number(v) if compatibility => {
                    l.step(1)?;
                    let _ = reference_number_from_lexeme(&v.lexeme, l)?;
                }
                Source::Number(v) => l.bytes(v.lexeme.len())?,
                Source::Object(v) if compatibility => {
                    if let Some((key, _)) = v.first() {
                        let key = key.as_str().ok_or(ReferenceError::InvalidBinding)?;
                        l.bytes(
                            key.len()
                                .min(29)
                                .checked_mul(2)
                                .ok_or(ReferenceError::Budget)?,
                        )?;
                        if key == "$serde_json::private::Number"
                            || key == "$serde_json::private::RawValue"
                        {
                            return Err(ReferenceError::UnsupportedSourceProfile);
                        }
                    }
                }
                _ => (),
            }
        }
        let child = match value {
            Source::Array(v) => v.get(cursor),
            Source::Object(v) => {
                if let Some((key, child)) = v.get(cursor) {
                    l.step(if compatibility { 1 } else { 3 })?;
                    let text = key.as_str().ok_or(ReferenceError::InvalidBinding)?;
                    l.bytes(if compatibility {
                        text.len()
                    } else {
                        key.units()
                            .len()
                            .checked_mul(2)
                            .and_then(|n| n.checked_add(text.len()))
                            .ok_or(ReferenceError::Budget)?
                    })?;
                    Some(child)
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(child) = child {
            if state.depth == 97 {
                return Err(ReferenceError::Budget);
            }
            let next_cursor = cursor.checked_add(1).ok_or(ReferenceError::Budget)?;
            reference_stack_room(&mut state.frames, state.available, l)?;
            l.step(2)?; // parent update + child push, BEFORE both writes
            *state
                .frames
                .get_mut(state.depth - 1)
                .ok_or(ReferenceError::InvalidBinding)? = (value, next_cursor);
            state.frames.push((child, 0));
            state.depth += 1;
        } else {
            l.step(1)?; // ordinary frame write; storage/growth bytes remain separate
            state.depth -= 1;
            let _ = state.frames.pop().ok_or(ReferenceError::InvalidBinding)?;
        }
    }
    Ok(())
}
// No object_get's temporary UTF-16 allocation. Full bounded linear reads are
// admitted, including absent/malformed-member prefixes, before comparison.
fn reference_source_member<'a>(
    value: &'a tos_foundation::JsonValue,
    name: &str,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<Option<&'a tos_foundation::JsonValue>> {
    l.step(1)?;
    if let Some(entries) = value.as_object() {
        for (key, value) in entries {
            l.step(1)?;
            let key = key.as_str().ok_or(ReferenceError::InvalidBinding)?;
            l.bytes(
                key.len()
                    .checked_add(name.len())
                    .ok_or(ReferenceError::Budget)?,
            )?;
            if key == name {
                return Ok(Some(value));
            }
        }
    }
    Ok(None)
}
fn reference_source_array<'a>(
    value: &'a tos_foundation::JsonValue,
    name: &str,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<&'a [tos_foundation::JsonValue]> {
    reference_source_member(value, name, l)?
        .and_then(tos_foundation::JsonValue::as_array)
        .ok_or(ReferenceError::InvalidBinding)
}
fn reference_source_str<'a>(
    value: &'a tos_foundation::JsonValue,
    name: &str,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<&'a str> {
    reference_source_member(value, name, l)?
        .and_then(tos_foundation::JsonValue::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(ReferenceError::InvalidBinding)
}
fn reference_take_member(
    root: &mut tos_foundation::JsonValue,
    name: &str,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<tos_foundation::JsonValue> {
    l.step(1)?;
    l.bytes(
        std::mem::size_of::<tos_foundation::JsonValue>()
            .checked_mul(2)
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<NavResult<tos_foundation::JsonValue>>())
            })
            .ok_or(ReferenceError::Budget)?,
    )?;
    if let tos_foundation::JsonValue::Object(entries) = root {
        for (key, value) in entries {
            l.step(1)?;
            let key = key.as_str().ok_or(ReferenceError::InvalidBinding)?;
            l.bytes(
                key.len()
                    .checked_add(name.len())
                    .ok_or(ReferenceError::Budget)?,
            )?;
            if key == name {
                return Ok(std::mem::replace(value, tos_foundation::JsonValue::Null));
            }
        }
    }
    Err(ReferenceError::InvalidBinding)
}
fn reference_requested_value(
    value: &tos_foundation::JsonValue,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<serde_json::Value> {
    // Only private callers below borrow fully admitted immutable roots. Clone
    // creates exactly len slots/text, bounded by the already-owned capacities;
    // pay its full retained bound, inline root and read walk BEFORE cloning.
    reference_source_scan(value, l, false)?;
    l.bytes(
        value
            .retained_storage_bytes()
            .map_err(|_| ReferenceError::Budget)?
            .checked_add(std::mem::size_of::<tos_foundation::JsonValue>())
            .ok_or(ReferenceError::Budget)?,
    )?;
    move_reference_value(value.clone(), l)
}
fn nav_str<'a>(v: &'a serde_json::Value, key: &str) -> NavResult<&'a str> {
    v.get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(ReferenceError::InvalidBinding)
}
fn nav_array<'a>(v: &'a serde_json::Value, key: &str) -> NavResult<&'a [serde_json::Value]> {
    v.get(key)
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .ok_or(ReferenceError::InvalidBinding)
}
fn nav_label<'a>(display: Option<&'a str>, preferred: Option<&'a str>, id: &'a str) -> &'a str {
    display
        .or(preferred)
        .filter(|s| !s.is_empty())
        .unwrap_or(id)
}
// Operation-local, successful-result reuse of immutable SOURCE views only.
// No Claim/form value or SUT response is stored here.
struct NavigationRegistryProfile {
    files: usize,
    input: usize,
    file: usize,
    record: usize,
    claim_rows: usize,
    claim_bytes: usize,
    rows: usize,
    row: usize,
    output: usize,
}
impl NavigationRegistryProfile {
    fn matches(&self, l: &mut ReferenceLimits<'_>) -> NavResult<()> {
        l.step(9)?; // all scalar reads/comparisons plus cutoff/cancellation
        if self.files != l.max_files
            || self.input != l.max_input_bytes
            || self.file != l.max_file_bytes
            || self.record != l.max_record_bytes
            || self.claim_rows != l.max_claim_rows
            || self.claim_bytes != l.max_claim_bytes
            || self.rows != l.max_rows
            || self.row != l.max_row_bytes
            || self.output != l.max_output_bytes
        {
            return Err(ReferenceError::UnsupportedSourceProfile);
        }
        Ok(())
    }
}
struct NavigationRegistryView {
    value: serde_json::Value,
    sha: std::cell::OnceCell<String>,
}
impl NavigationRegistryView {
    fn digest<'a>(
        &'a self,
        profile: &NavigationRegistryProfile,
        l: &mut ReferenceLimits<'_>,
    ) -> NavResult<&'a str> {
        profile.matches(l)?;
        l.step(3)?; // cell lookup and successful initialization/read boundary
        if self.sha.get().is_none() {
            // First digest uses the complete paid current canonical route.
            // These additional extents cover cell construction/move holders.
            l.bytes(
                std::mem::size_of::<String>()
                    .checked_mul(2) // returned sha and cell-set argument/move
                    .and_then(|n| n.checked_add(std::mem::size_of::<Result<(), String>>()))
                    .and_then(|n| n.checked_add(std::mem::size_of::<Option<&str>>()))
                    .and_then(|n| n.checked_add(std::mem::size_of::<NavResult<&str>>()))
                    .and_then(|n| n.checked_add(std::mem::size_of::<&str>().checked_mul(4)?))
                    .ok_or(ReferenceError::Budget)?,
            )?;
            let sha = l.digest(&self.value)?;
            // Only a COMPLETE successful old-domain digest is ever retained.
            self.sha
                .set(sha)
                .map_err(|_| ReferenceError::InvalidBinding)?;
        }
        self.sha
            .get()
            .map(String::as_str)
            .ok_or(ReferenceError::InvalidBinding)
    }
}
struct NavigationRegistries {
    entities: tos_foundation::JsonValue,
    relations: tos_foundation::JsonValue,
    type_index: BTreeMap<String, usize>,
    predicate_index: BTreeMap<String, (usize, usize)>,
    projection_profile: NavigationRegistryProfile,
    relation_views: Vec<std::cell::OnceCell<NavigationRegistryView>>,
    template_view: std::cell::OnceCell<NavigationRegistryView>,
}
struct NavigationSource {
    path: String,
    line: Option<usize>,
    value: serde_json::Value,
    sha: String,
}
struct DeclaredNavigationInputs<'a> {
    files: &'a BTreeMap<String, Vec<u8>>,
    registries: &'a NavigationRegistries,
    identities: BTreeMap<String, NavigationSource>,
    claims: Vec<NavigationSource>,
    events: BTreeMap<String, NavigationSource>,
    forms: BTreeMap<String, NavigationSource>,
}
impl NavigationRegistries {
    fn from_files(
        files: &BTreeMap<String, Vec<u8>>,
        l: &mut ReferenceLimits<'_>,
    ) -> NavResult<Self> {
        l.fixed(2)?;
        let entity = files
            .get("ToS/doctrine/semantic-interchange/entity-types.v1.json")
            .ok_or(ReferenceError::InvalidBinding)?;
        let relation = files
            .get("ToS/doctrine/semantic-interchange/relation-types.v1.json")
            .ok_or(ReferenceError::InvalidBinding)?;
        let entities = l.retained_root(entity, l.max_file_bytes)?;
        let relations = l.retained_root(relation, l.max_file_bytes)?;
        let mut type_index = BTreeMap::new();
        let mut predicate_index = BTreeMap::new();
        for (i, t) in reference_source_array(&entities, "types", l)?
            .iter()
            .enumerate()
        {
            l.step(2)?;
            let id = reference_source_str(t, "type_id", l)?;
            l.bytes(id.len().checked_add(128).ok_or(ReferenceError::Budget)?)?;
            if type_index.insert(id.to_owned(), i).is_some() {
                return Err(ReferenceError::DuplicateIdentity);
            }
        }
        let mut relation_ids = std::collections::BTreeSet::new();
        for (i, relation) in reference_source_array(&relations, "relations", l)?
            .iter()
            .enumerate()
        {
            l.step(2)?;
            l.bytes(128)?;
            if !relation_ids.insert(reference_source_str(relation, "relation_type_id", l)?) {
                return Err(ReferenceError::DuplicateIdentity);
            }
            for (j, mapping) in reference_source_array(relation, "source_mappings", l)?
                .iter()
                .enumerate()
            {
                l.step(3)?;
                let graph = reference_source_member(mapping, "source_graph", l)?
                    .and_then(tos_foundation::JsonValue::as_str);
                let scope = reference_source_member(mapping, "scope", l)?
                    .and_then(tos_foundation::JsonValue::as_str);
                if graph == Some("source-claims") && scope == Some("claim-predicate") {
                    let predicate = reference_source_str(mapping, "source_predicate_id", l)?;
                    l.bytes(
                        predicate
                            .len()
                            .checked_add(128)
                            .ok_or(ReferenceError::Budget)?,
                    )?;
                    if predicate_index
                        .insert(predicate.to_owned(), (i, j))
                        .is_some()
                    {
                        return Err(ReferenceError::DuplicateIdentity);
                    }
                }
            }
        }
        let count = reference_source_array(&relations, "relations", l)?.len();
        // Fixed-capacity slots: no payload conversion/digest for unused rows.
        // Pay storage, header/move holders and every slot BEFORE construction.
        l.bytes(
            count
                .checked_mul(std::mem::size_of::<
                    std::cell::OnceCell<NavigationRegistryView>,
                >())
                .and_then(|n| {
                    n.checked_add(
                        std::mem::size_of::<Vec<std::cell::OnceCell<NavigationRegistryView>>>()
                            .checked_mul(2)?,
                    )
                })
                .and_then(|n| {
                    n.checked_add(
                        std::mem::size_of::<std::cell::OnceCell<NavigationRegistryView>>()
                            .checked_mul(3)?,
                    )
                })
                .and_then(|n| {
                    n.checked_add(std::mem::size_of::<NavigationRegistryProfile>().checked_mul(2)?)
                })
                .ok_or(ReferenceError::Budget)?,
        )?;
        l.step(
            count
                .checked_mul(3)
                .and_then(|n| n.checked_add(9))
                .ok_or(ReferenceError::Budget)?,
        )?;
        let projection_profile = NavigationRegistryProfile {
            files: l.max_files,
            input: l.max_input_bytes,
            file: l.max_file_bytes,
            record: l.max_record_bytes,
            claim_rows: l.max_claim_rows,
            claim_bytes: l.max_claim_bytes,
            rows: l.max_rows,
            row: l.max_row_bytes,
            output: l.max_output_bytes,
        };
        let mut relation_views = Vec::with_capacity(count);
        for _ in 0..count {
            relation_views.push(std::cell::OnceCell::new());
        }
        Ok(Self {
            entities,
            relations,
            type_index,
            predicate_index,
            projection_profile,
            relation_views,
            template_view: std::cell::OnceCell::new(),
        })
    }
    fn source_view<'a>(
        &'a self,
        source: &tos_foundation::JsonValue,
        cell: &'a std::cell::OnceCell<NavigationRegistryView>,
        l: &mut ReferenceLimits<'_>,
    ) -> NavResult<&'a NavigationRegistryView> {
        self.projection_profile.matches(l)?;
        l.step(3)?; // lookup, initialization branch and resulting shared view
        if cell.get().is_none() {
            l.bytes(
                std::mem::size_of::<NavigationRegistryView>()
                    .checked_mul(2) // view constructor and cell-set argument/move
                    .and_then(|n| {
                        n.checked_add(std::mem::size_of::<Result<(), NavigationRegistryView>>())
                    })
                    .and_then(|n| n.checked_add(std::mem::size_of::<serde_json::Value>()))
                    .and_then(|n| {
                        n.checked_add(std::mem::size_of::<Option<&NavigationRegistryView>>())
                    })
                    .and_then(|n| {
                        n.checked_add(std::mem::size_of::<NavResult<&NavigationRegistryView>>())
                    })
                    .and_then(|n| {
                        n.checked_add(std::mem::size_of::<&serde_json::Value>().checked_mul(4)?)
                    })
                    .ok_or(ReferenceError::Budget)?,
            )?;
            // FIRST view still executes the complete old clone/bridge route.
            let value = reference_requested_value(source, l)?;
            cell.set(NavigationRegistryView {
                value,
                sha: std::cell::OnceCell::new(),
            })
            .map_err(|_| ReferenceError::InvalidBinding)?;
        }
        cell.get().ok_or(ReferenceError::InvalidBinding)
    }
    fn relation_view(
        &self,
        index: usize,
        l: &mut ReferenceLimits<'_>,
    ) -> NavResult<&NavigationRegistryView> {
        // Preserve the paid SOURCE-index lookup even on a retained-view hit.
        let row = reference_source_array(&self.relations, "relations", l)?
            .get(index)
            .ok_or(ReferenceError::InvalidBinding)?;
        l.step(1)?;
        let cell = self
            .relation_views
            .get(index)
            .ok_or(ReferenceError::InvalidBinding)?;
        self.source_view(row, cell, l)
    }
    fn relation(&self, index: usize, l: &mut ReferenceLimits<'_>) -> NavResult<&serde_json::Value> {
        self.relation_view(index, l).map(|view| &view.value)
    }
    fn template_view(&self, l: &mut ReferenceLimits<'_>) -> NavResult<&NavigationRegistryView> {
        let row = reference_source_member(&self.relations, "claim_navigation_template", l)?
            .ok_or(ReferenceError::InvalidBinding)?;
        self.source_view(row, &self.template_view, l)
    }
}
impl<'a> DeclaredNavigationInputs<'a> {
    fn from_files(
        files: &'a BTreeMap<String, Vec<u8>>,
        registries: &'a NavigationRegistries,
        l: &mut ReferenceLimits<'_>,
    ) -> NavResult<Self> {
        l.step(1)?;
        if files.len() > l.max_files {
            return Err(ReferenceError::Budget);
        }
        let mut total = 0usize;
        for (path, raw) in files {
            l.step(path.len().checked_add(1).ok_or(ReferenceError::Budget)?)?;
            l.bytes(path.len().checked_add(128).ok_or(ReferenceError::Budget)?)?;
            tos_foundation::RelativePath::parse(path)
                .map_err(|_| ReferenceError::InvalidBinding)?;
            total = total.checked_add(raw.len()).ok_or(ReferenceError::Budget)?;
            if raw.len() > l.max_file_bytes || total > l.max_input_bytes {
                return Err(ReferenceError::Budget);
            }
        }
        let mut claim_bytes = 0usize;
        let mut out = Self {
            files,
            registries,
            identities: BTreeMap::new(),
            claims: Vec::new(),
            events: BTreeMap::new(),
            forms: BTreeMap::new(),
        };
        // Independent authored recipe, not discovery through a product catalogue.
        // Archives/canon are H inputs, not active Claim/identity/form declarations.
        for (path, raw) in files {
            l.step(1)?;
            if !path.starts_with("ToS/source-witnesses/") || path.contains("/.record-revisions/") {
                continue;
            }
            let identity = path.ends_with("/agent.json") || path.ends_with("/letter.json");
            let forms = path.ends_with(".human-forms.json");
            let claims = path.ends_with("/source-claims.jsonl");
            let events = path.ends_with("/source-create-provenance.jsonl");
            if !(identity || forms || claims || events) {
                continue;
            }
            let text = std::str::from_utf8(raw).map_err(|_| ReferenceError::InvalidBinding)?;
            l.step(text.len())?;
            let mut add = |line: Option<usize>, raw: &[u8]| -> NavResult<()> {
                l.fixed(8)?;
                if out.identities.len() + out.claims.len() + out.events.len() + out.forms.len()
                    >= l.max_rows
                {
                    return Err(ReferenceError::Budget);
                }
                if claims {
                    claim_bytes = claim_bytes
                        .checked_add(raw.len())
                        .ok_or(ReferenceError::Budget)?;
                    if claim_bytes > l.max_claim_bytes || out.claims.len() >= l.max_claim_rows {
                        return Err(ReferenceError::Budget);
                    }
                }
                let value = l.parse(raw, l.max_record_bytes)?;
                let sha = l.digest(&value)?;
                l.bytes(path.len().checked_add(256).ok_or(ReferenceError::Budget)?)?;
                let source = NavigationSource {
                    path: path.clone(),
                    line,
                    value,
                    sha,
                };
                if identity {
                    let id = nav_str(&source.value, "record_id")?.to_owned();
                    if !matches!(nav_str(&source.value, "record_type")?, "agent" | "letter")
                        || !path
                            .ends_with(&format!("/{}.json", nav_str(&source.value, "record_type")?))
                    {
                        return Err(ReferenceError::UnsupportedSourceProfile);
                    }
                    if out.identities.insert(id, source).is_some() {
                        return Err(ReferenceError::DuplicateIdentity);
                    }
                } else if claims {
                    if source.value["schema_version"] != "tos_source_relation_claim_v1" {
                        return Err(ReferenceError::UnsupportedSourceProfile);
                    }
                    out.claims.push(source);
                } else if events {
                    let id = nav_str(&source.value, "event_id")?.to_owned();
                    if out.events.insert(id, source).is_some() {
                        return Err(ReferenceError::DuplicateIdentity);
                    }
                } else {
                    let id = nav_str(&source.value["subject"], "id")?.to_owned();
                    if out.forms.insert(id, source).is_some() {
                        return Err(ReferenceError::DuplicateIdentity);
                    }
                }
                Ok(())
            };
            if claims || events {
                for (index, line) in text.lines().enumerate() {
                    if !line.trim().is_empty() {
                        add(Some(index + 1), line.as_bytes())?;
                    }
                }
            } else {
                add(None, raw)?;
            }
        }
        Ok(out)
    }
}
fn nav_no_reserved(
    value: &serde_json::Value,
    keys: &[&str],
    l: &mut ReferenceLimits<'_>,
) -> NavResult<()> {
    for key in keys {
        l.step(1)?;
        if value.get(*key).is_some() {
            return Err(ReferenceError::Conflict);
        }
    }
    Ok(())
}
fn nav_exact_ref(
    source: &NavigationSource,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<serde_json::Value> {
    l.fixed(3)?;
    let (id, version) = if source.value.get("claim_id").is_some() {
        ("claim_id", "claim_version")
    } else {
        ("record_id", "record_version")
    };
    let id = nav_str(&source.value, id)?;
    let version = source
        .value
        .get(version)
        .and_then(serde_json::Value::as_u64)
        .filter(|v| *v > 0)
        .ok_or(ReferenceError::InvalidBinding)?;
    l.bytes(id.len().checked_add(64).ok_or(ReferenceError::Budget)?)?;
    Ok(serde_json::json!({"id":id,"version":version,"digest":format!("sha256:{}",source.sha)}))
}
fn nav_pointer<'a>(
    v: &'a serde_json::Value,
    pointer: &str,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<&'a serde_json::Value> {
    l.step(1)?;
    if !pointer.is_empty() {
        if !pointer.starts_with('/') {
            return Err(ReferenceError::InvalidBinding);
        }
        for token in pointer[1..].split('/') {
            l.step(1)?;
            let mut chars = token.chars();
            while let Some(c) = chars.next() {
                l.step(1)?;
                if c == '~' && !matches!(chars.next(), Some('0' | '1')) {
                    return Err(ReferenceError::InvalidBinding);
                }
            }
        }
    }
    v.pointer(pointer).ok_or(ReferenceError::InvalidBinding)
}
fn nav_forms(
    source: &NavigationSource,
    inputs: &DeclaredNavigationInputs<'_>,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<Option<(String, serde_json::Value)>> {
    use serde_json::{Value, json};
    l.step(1)?;
    let subject = nav_exact_ref(source, l)?;
    let Some(set) = inputs.forms.get(nav_str(&subject, "id")?) else {
        return Ok(None);
    };
    l.fixed(5)?;
    if set.value["schema_version"] != "tos_human_form_set_v1" || set.value["subject"] != subject {
        return Err(ReferenceError::InvalidBinding);
    }
    let forms = nav_array(&set.value, "forms")?;
    if forms.len() > l.max_rows {
        return Err(ReferenceError::Budget);
    }
    l.bytes(forms.len().checked_mul(128).ok_or(ReferenceError::Budget)?)?;
    let mut output = Vec::with_capacity(forms.len());
    let mut seen = std::collections::BTreeSet::new();
    for form in forms {
        l.fixed(25)?;
        l.value_bound(form, 0)?;
        let id = nav_str(form, "form_id")?;
        if !seen.insert(id) {
            return Err(ReferenceError::DuplicateIdentity);
        }
        if form["schema_version"] != "tos_human_form_v1"
            || form["subject"] != subject
            || form["content"]["kind"] != "source-copy"
        {
            return Err(ReferenceError::UnsupportedSourceProfile);
        }
        if let Some(revises) = form.get("revises").filter(|v| !v.is_null()) {
            let mut found = false;
            for prior in nav_array(&set.value, "prior_forms")? {
                l.step(4)?;
                if prior["form_id"] == revises["id"] && prior["form_version"] == revises["version"]
                {
                    if found || revises["digest"] != format!("sha256:{}", l.digest(prior)?) {
                        return Err(ReferenceError::InvalidBinding);
                    }
                    found = true;
                }
            }
            if !found {
                return Err(ReferenceError::InvalidBinding);
            }
        }
        let bindings = form["bindings"]
            .as_object()
            .ok_or(ReferenceError::InvalidBinding)?;
        let slot = nav_str(&form["content"], "slot")?;
        let wording = bindings.get(slot).ok_or(ReferenceError::InvalidBinding)?;
        if wording["record"] != subject {
            return Err(ReferenceError::InvalidBinding);
        }
        let pointer = wording["pointer"]
            .as_str()
            .ok_or(ReferenceError::InvalidBinding)?;
        let display = nav_pointer(&source.value, pointer, l)?
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or(ReferenceError::InvalidBinding)?;
        let role = nav_str(form, "role")?;
        // SourceCopy semantic binding profile is derived from SOURCE fields.
        let mut contexts: Vec<String> = Vec::new();
        let (language, script) = if source.value.get("claim_id").is_some() {
            contexts.push(String::new());
            if role == "statement" && pointer == "/qualifiers/statement" {
                (
                    &source.value["qualifiers"]["statement_language"],
                    &source.value["qualifiers"]["statement_script"],
                )
            } else if matches!(role, "name" | "caption" | "hover")
                && pointer == format!("/qualifiers/display_fields/{role}/text")
            {
                (
                    &source.value["qualifiers"]["display_fields"][role]["language"],
                    &source.value["qualifiers"]["display_fields"][role]["script"],
                )
            } else {
                return Err(ReferenceError::UnsupportedSourceProfile);
            }
        } else {
            for key in [
                "identity_status",
                "same_as_posture",
                "semantic_scope",
                "semantic_content",
                "form_identity",
                "native_text_binding",
            ] {
                l.step(1)?;
                if source.value.get(key).is_some() {
                    contexts.push(format!("/{key}"));
                }
            }
            let field = match (role, pointer) {
                ("name", "/preferred_label") => "preferred_label",
                ("hover", "/notes") => "notes",
                ("name", p) if p.starts_with("/variant_labels/") && p.ends_with("/value") => {
                    "variant_labels"
                }
                _ => return Err(ReferenceError::UnsupportedSourceProfile),
            };
            if field == "variant_labels" {
                let parent = pointer
                    .strip_suffix("/value")
                    .ok_or(ReferenceError::InvalidBinding)?;
                let variant = nav_pointer(&source.value, parent, l)?
                    .as_object()
                    .ok_or(ReferenceError::InvalidBinding)?;
                for key in variant.keys().filter(|k| k.as_str() != "value") {
                    l.step(1)?;
                    contexts.push(format!(
                        "{parent}/{}",
                        key.replace('~', "~0").replace('/', "~1")
                    ));
                }
                (
                    source
                        .value
                        .pointer(&format!("{parent}/language"))
                        .unwrap_or(&Value::Null),
                    source
                        .value
                        .pointer(&format!("{parent}/script"))
                        .unwrap_or(&Value::Null),
                )
            } else {
                if source.value["field_languages"].get(field).is_some() {
                    contexts.push(format!("/field_languages/{field}"));
                }
                (
                    &source.value["field_languages"][field]["language"],
                    &source.value["field_languages"][field]["script"],
                )
            }
        };
        if form["language"] != *language
            || form["script"] != *script
            || bindings.len() != contexts.len() + 1
        {
            return Err(ReferenceError::InvalidBinding);
        }
        let mut context = Vec::new();
        for (i, pointer) in contexts.iter().enumerate() {
            l.fixed(3)?;
            let key = format!("context-{i}");
            let binding = bindings.get(&key).ok_or(ReferenceError::InvalidBinding)?;
            if binding["record"] != subject || binding["pointer"] != *pointer {
                return Err(ReferenceError::InvalidBinding);
            }
            let selected = nav_pointer(&source.value, pointer, l)?;
            let value = l.copy(selected)?;
            let binding = l.copy(binding)?;
            context.push(nav_object(
                [
                    ("slot", Value::String(key)),
                    ("binding", binding),
                    ("value", value),
                ],
                l,
            )?);
        }
        let version = form["form_version"]
            .as_u64()
            .filter(|v| *v > 0)
            .ok_or(ReferenceError::InvalidBinding)?;
        let digest = l.digest(form)?;
        l.bytes(display.len())?;
        let subject_value = l.copy(&subject)?;
        let dependency = l.copy(&subject)?;
        let language = l.copy(language)?;
        let script = l.copy(script)?;
        output.push(nav_object(
            [
                ("schema_version", json!("tos_human_form_materialization_v1")),
                (
                    "form",
                    json!({"id":id,"version":version,"digest":format!("sha256:{digest}")}),
                ),
                ("subject", subject_value),
                ("state", json!("ready")),
                ("display_text", json!(display)),
                ("context", Value::Array(context)),
                ("issues", json!([])),
                ("admission", Value::Null),
                ("performs_semantic_assessment", json!(false)),
                ("role", json!(role)),
                ("language", language),
                ("script", script),
                ("derivation", json!("source-copy")),
                ("dependencies", Value::Array(vec![dependency])),
                ("standalone_reading", json!(false)),
            ],
            l,
        )?);
    }
    l.bytes(set.path.len())?;
    Ok(Some((set.path.clone(), Value::Array(output))))
}

fn nav_type_matches(
    kind: &str,
    domain: &[serde_json::Value],
    reg: &NavigationRegistries,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<bool> {
    let types = reference_source_array(&reg.entities, "types", l)?;
    let mut start = None;
    for (i, t) in types.iter().enumerate() {
        l.step(1)?;
        for mapping in reference_source_array(t, "source_mappings", l)? {
            l.step(2)?;
            let graph = reference_source_member(mapping, "source_graph", l)?
                .and_then(tos_foundation::JsonValue::as_str);
            let mapped_kind = reference_source_member(mapping, "source_kind_id", l)?
                .and_then(tos_foundation::JsonValue::as_str);
            if graph == Some("source-claims") && mapped_kind == Some(kind) {
                if start.replace(i).is_some() {
                    return Err(ReferenceError::DuplicateIdentity);
                }
            }
        }
    }
    let start = start.ok_or(ReferenceError::UnsupportedSourceProfile)?;
    if reference_source_member(&types[start], "abstract", l)?
        .and_then(tos_foundation::JsonValue::as_bool)
        != Some(false)
    {
        return Err(ReferenceError::UnsupportedSourceProfile);
    }
    let mut todo = Vec::new();
    let mut active = std::collections::BTreeSet::new();
    let mut done = std::collections::BTreeSet::new();
    l.bytes(128)?;
    todo.push((start, false));
    let mut matches = false;
    while let Some((i, exit)) = todo.pop() {
        l.step(1)?;
        if exit {
            active.remove(&i);
            done.insert(i);
            continue;
        }
        if active.contains(&i) {
            return Err(ReferenceError::InvalidBinding);
        }
        if done.contains(&i) {
            continue;
        }
        l.bytes(256)?;
        active.insert(i);
        todo.push((i, true));
        for allowed in domain {
            l.step(1)?;
            if allowed.as_str().ok_or(ReferenceError::InvalidBinding)?
                == reference_source_str(&types[i], "type_id", l)?
            {
                matches = true;
            }
        }
        for parent in reference_source_array(&types[i], "parent_type_ids", l)? {
            l.step(1)?;
            let id = parent.as_str().ok_or(ReferenceError::InvalidBinding)?;
            let index = *reg
                .type_index
                .get(id)
                .ok_or(ReferenceError::InvalidBinding)?;
            l.bytes(32)?;
            todo.push((index, false));
        }
    }
    Ok(matches)
}
fn nav_descriptor(
    claim: &NavigationSource,
    subject: &NavigationSource,
    object: &NavigationSource,
    inputs: &DeclaredNavigationInputs<'_>,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<serde_json::Value> {
    use serde_json::{Value, json};
    l.fixed(48)?;
    let c = &claim.value;
    if c["subject_ref"] != subject.value["record_id"] || c["object"] != object.value["record_id"] {
        return Err(ReferenceError::InvalidBinding);
    }
    let reg = inputs.registries;
    let (ri, mi) = *reg
        .predicate_index
        .get(nav_str(c, "predicate")?)
        .ok_or(ReferenceError::UnsupportedSourceProfile)?;
    let relation_view = reg.relation_view(ri, l)?;
    let relation = &relation_view.value;
    let mapping = &relation["source_mappings"][mi];
    let profile = &relation["source_claim_profile"];
    if relation["abstract"] != false
        || relation["assertion_mode"] != "reified-claim"
        || profile["reader"] != "identity-relation-v1"
        || profile["profile_version"] != 1
    {
        return Err(ReferenceError::UnsupportedSourceProfile);
    }
    let mut layer = false;
    for declared in nav_array(profile, "assertion_layers")? {
        l.step(1)?;
        if declared == &c["assertion_layer"] {
            layer = true;
        }
    }
    let mut schema = false;
    for declared in nav_array(profile, "schemas")? {
        l.step(2)?;
        if declared["schema_version"] == c["schema_version"] {
            if schema {
                return Err(ReferenceError::DuplicateIdentity);
            }
            schema = true;
            for path in std::iter::once(&declared["schema_ref"])
                .chain(nav_array(declared, "schema_dependencies")?.iter())
            {
                l.step(1)?;
                let path = path.as_str().ok_or(ReferenceError::InvalidBinding)?;
                if !inputs.files.contains_key(path) {
                    return Err(ReferenceError::InvalidBinding);
                }
            }
        }
    }
    if !schema
        || !layer
        || !nav_type_matches(
            nav_str(&subject.value, "record_type")?,
            nav_array(relation, "domain_type_ids")?,
            reg,
            l,
        )?
        || !nav_type_matches(
            nav_str(&object.value, "record_type")?,
            nav_array(relation, "range_type_ids")?,
            reg,
            l,
        )?
    {
        return Err(ReferenceError::UnsupportedSourceProfile);
    }
    let template_view = reg.template_view(l)?;
    let template = &template_view.value;
    if template["reader"] != "claim-navigation-v1" || template["purpose"] != "claim-navigation-only"
    {
        return Err(ReferenceError::UnsupportedSourceProfile);
    }
    let cap = template["max_output_bytes"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(ReferenceError::InvalidBinding)?
        .min(l.max_row_bytes);
    let labels = if mapping.get("labels").is_some() {
        &mapping["labels"]
    } else {
        let mut count = 0usize;
        for m in nav_array(relation, "source_mappings")? {
            l.step(2)?;
            if m["source_graph"] == "source-claims" && m["scope"] == "claim-predicate" {
                count += 1;
            }
        }
        if count != 1 {
            return Err(ReferenceError::UnsupportedSourceProfile);
        }
        &relation["labels"]
    };
    let renderings = template["renderings"]
        .as_object()
        .ok_or(ReferenceError::InvalidBinding)?;
    let mut title = serde_json::Map::new();
    let mut aggregate = 0usize;
    for (language, parts) in renderings {
        l.step(1)?;
        let mut text = String::new();
        for part in parts.as_array().ok_or(ReferenceError::InvalidBinding)? {
            l.step(3)?;
            let part_map = part.as_object().ok_or(ReferenceError::InvalidBinding)?;
            if part_map.len() != 1 {
                return Err(ReferenceError::UnsupportedSourceProfile);
            }
            let chunk = if let Some(literal) = part.get("literal") {
                literal.as_str().ok_or(ReferenceError::InvalidBinding)?
            } else {
                match nav_str(part, "slot")? {
                    "claim-marker" => template["marker"][language].as_str(),
                    "subject-label" => subject.value["preferred_label"].as_str(),
                    "object-label" => object.value["preferred_label"].as_str(),
                    "predicate-label" => labels[language]
                        .as_str()
                        .or_else(|| labels["default"].as_str()),
                    "declared-epistemic-status" => template["status_labels"]["epistemic_status"]
                        [nav_str(c, "epistemic_status")?][language]
                        .as_str(),
                    "declared-review-status" => template["status_labels"]["review_status"]
                        [nav_str(c, "review_status")?][language]
                        .as_str(),
                    _ => return Err(ReferenceError::UnsupportedSourceProfile),
                }
                .filter(|s| !s.is_empty())
                .ok_or(ReferenceError::UnsupportedSourceProfile)?
            };
            let len = text
                .len()
                .checked_add(chunk.len())
                .ok_or(ReferenceError::Budget)?;
            if len > cap {
                return Err(ReferenceError::Budget);
            }
            l.bytes(chunk.len())?;
            text.push_str(chunk);
        }
        aggregate = aggregate
            .checked_add(text.len())
            .ok_or(ReferenceError::Budget)?;
        if aggregate > cap {
            return Err(ReferenceError::Budget);
        }
        l.fixed(1)?;
        title.insert(language.clone(), Value::String(text));
    }
    let default = nav_str(template, "default_language")?;
    let default_value = l.copy(
        title
            .get(default)
            .ok_or(ReferenceError::UnsupportedSourceProfile)?,
    )?;
    title.insert("default".into(), default_value);
    let endpoint = |source: &NavigationSource, l: &mut ReferenceLimits<'_>| -> NavResult<Value> {
        l.fixed(7)?;
        let id = nav_str(&source.value, "record_id")?;
        let label = nav_str(&source.value, "preferred_label")?;
        if label.trim().is_empty() || label == id || label == source.path {
            return Err(ReferenceError::UnsupportedSourceProfile);
        }
        l.bytes(
            id.len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(label.len()))
                .and_then(|n| n.checked_add(source.path.len()))
                .and_then(|n| n.checked_add(128))
                .ok_or(ReferenceError::Budget)?,
        )?;
        Ok(
            json!({"identity_ref":id,"node_id":format!("identity:{id}"),"source_ref":source.path,
            "record_version":source.value["record_version"],"sha256":source.sha,"label_pointer":"/preferred_label","label":label}),
        )
    };
    let sub = endpoint(subject, l)?;
    let obj = endpoint(object, l)?;
    let template_id = nav_str(template, "template_id")?;
    let template_sha = template_view.digest(&reg.projection_profile, l)?;
    l.bytes(template_sha.len())?; // independent output String, not a free hit copy
    let template_value =
        json!({"id":template_id,"version":template["template_version"],"sha256":template_sha});
    let claim_value =
        json!({"id":nav_str(c,"claim_id")?,"version":c["claim_version"],"sha256":claim.sha});
    let predicate_id = nav_str(c, "predicate")?;
    let relation_type_id = nav_str(relation, "relation_type_id")?;
    let relation_sha = relation_view.digest(&reg.projection_profile, l)?;
    l.bytes(relation_sha.len())?; // output owns its own digest String
    let predicate = json!({"id":predicate_id,"relation_type_id":relation_type_id,"sha256":relation_sha,"mapping_sha256":l.digest(mapping)?});
    let statuses = json!({"epistemic_status":{"present":true,"value":c["epistemic_status"]},"review_status":{"present":true,"value":c["review_status"]}});
    nav_object(
        [
            (
                "schema_version",
                json!("tos_claim_navigation_descriptor_v1"),
            ),
            ("purpose", json!("claim-navigation-only")),
            ("standalone", json!(false)),
            ("state", json!("ready")),
            ("reason", Value::Null),
            ("template", template_value),
            ("claim", claim_value),
            ("predicate", predicate),
            ("subject", sub),
            ("object", obj),
            ("statuses", statuses),
            ("title", Value::Object(title)),
        ],
        l,
    )
}
// Move owned children into the expected object. json! on an already-owned Value
// would serialize/clone that child again, which is not this ownership law.
fn nav_object<const N: usize>(
    entries: [(&str, serde_json::Value); N],
    l: &mut ReferenceLimits<'_>,
) -> NavResult<serde_json::Value> {
    l.fixed(N)?;
    let mut map = serde_json::Map::new();
    for (key, value) in entries {
        l.bytes(key.len())?;
        if map.insert(key.to_owned(), value).is_some() {
            return Err(ReferenceError::Conflict);
        }
    }
    Ok(serde_json::Value::Object(map))
}
fn nav_node(
    id: String,
    kind: &str,
    source: &str,
    status: &str,
    properties: serde_json::Value,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<serde_json::Value> {
    let label = nav_label(
        properties
            .pointer("/bibliographic_carrier/display/title/default")
            .and_then(serde_json::Value::as_str),
        properties
            .get("preferred_label")
            .and_then(serde_json::Value::as_str),
        &id,
    );
    l.bytes(
        id.len()
            .checked_add(label.len())
            .and_then(|n| n.checked_add(source.len()))
            .and_then(|n| n.checked_add(kind.len()))
            .and_then(|n| n.checked_add(status.len()))
            .ok_or(ReferenceError::Budget)?,
    )?;
    let label = serde_json::Value::String(label.to_owned());
    nav_object(
        [
            ("node_id", serde_json::Value::String(id)),
            ("node_kind", serde_json::json!(kind)),
            ("label", label),
            ("source_ref", serde_json::json!(source)),
            ("identity_status", serde_json::json!(status)),
            ("properties", properties),
        ],
        l,
    )
}
fn nav_merge<Row: std::borrow::Borrow<serde_json::Value>>(
    rows: &mut BTreeMap<String, Row>,
    row: Row,
    key: &str,
    output_bytes: &mut usize,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<()> {
    l.step(2)?;
    let row_value: &serde_json::Value = row.borrow();
    let id = nav_str(row_value, key)?;
    let bound = merge_read_bound(row_value, l)?;
    if bound > l.max_row_bytes {
        return Err(ReferenceError::Budget);
    }
    *output_bytes = output_bytes
        .checked_add(bound)
        .ok_or(ReferenceError::Budget)?;
    if *output_bytes > l.max_output_bytes || rows.len() >= l.max_rows {
        return Err(ReferenceError::Budget);
    }
    if let Some(prior) = rows.get(id) {
        // A full comparison is structural work, not a free final-len check.
        let prior_value: &serde_json::Value = prior.borrow();
        l.compare_work(prior_value, 0)?;
        l.compare_work(row_value, 0)?;
        if prior_value != row_value {
            return Err(ReferenceError::Conflict);
        }
    } else {
        l.bytes(id.len().checked_add(128).ok_or(ReferenceError::Budget)?)?;
        rows.insert(id.to_owned(), row);
    }
    Ok(())
}
fn nav_evidence(
    path: &str,
    inputs: &DeclaredNavigationInputs<'_>,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<serde_json::Value> {
    l.fixed(15)?;
    l.step(path.len())?;
    l.bytes(path.len().checked_add(128).ok_or(ReferenceError::Budget)?)?;
    tos_foundation::RelativePath::parse(path).map_err(|_| ReferenceError::InvalidBinding)?;
    let provenance = if path.starts_with("ToS/research-packets/") {
        "catalogued-research-lead-h1"
    } else if path.starts_with("ToS/review-ledger/") {
        "catalogued-review-note-h1"
    } else {
        return Err(ReferenceError::UnsupportedSourceProfile);
    };
    if !path.ends_with(".md") {
        return Err(ReferenceError::UnsupportedSourceProfile);
    }
    let raw = inputs
        .files
        .get(path)
        .ok_or(ReferenceError::InvalidBinding)?;
    l.bytes(raw.len())?;
    let text = std::str::from_utf8(raw).map_err(|_| ReferenceError::InvalidBinding)?;
    let mut title = None;
    for line in text.lines() {
        l.step(1)?;
        if let Some(h1) = line.strip_prefix("# ") {
            title = Some(h1.trim());
            break;
        }
    }
    let title = title
        .filter(|s| !s.is_empty())
        .ok_or(ReferenceError::UnsupportedSourceProfile)?;
    l.step(title.len())?;
    if title.chars().count() > 240
        || title.chars().any(|c| {
            !tos_foundation::python_printable_unicode16_v1(c)
                && !matches!(
                    c,
                    '\u{a0}' | '\u{1680}' | '\u{2000}'
                        ..='\u{200a}'
                            | '\u{2028}'
                            | '\u{2029}'
                            | '\u{202f}'
                            | '\u{205f}'
                            | '\u{3000}'
                )
        })
    {
        return Err(ReferenceError::UnsupportedSourceProfile);
    }
    l.bytes(
        path.len()
            .checked_mul(4)
            .and_then(|n| n.checked_add(title.len()))
            .ok_or(ReferenceError::Budget)?,
    )?;
    let id = format!(
        "evidence:sha256:{}",
        tos_foundation::Digest256::of_bytes(path.as_bytes()).to_hex()
    );
    let props = serde_json::json!({"evidence_ref":path,"evidence_kind":"repo_path","resolved":true,
        "bibliographic_carrier":{"source_sha256":tos_foundation::Digest256::of_bytes(raw).to_hex(),"display":{
            "title":{"default":title},"summary":{"default":format!("Repo path evidence reference: {path}. Return to {path}.")},
            "summary_state":"metadata-synthesis","provenance":{"title":provenance,"summary":"evidence-reference-navigation",
            "source_title_available":true,"source_summary_available":false,"human_form_authority":"none","source_ref":path}}}});
    nav_node(id, "evidence", path, "not_applicable", props, l)
}
fn expected_source_navigation(
    historical: &serde_json::Value,
    inputs: &DeclaredNavigationInputs<'_>,
    mut limits: ReferenceLimits<'_>,
) -> NavResult<serde_json::Value> {
    use serde_json::{Value, json};
    let l = &mut limits;
    l.fixed(8)?;
    let mut nodes = BTreeMap::new();
    let mut edges = BTreeMap::new();
    let mut output_bytes = 0usize;
    for (collection, key, rows) in [
        ("nodes", "node_id", &mut nodes),
        ("edges", "edge_id", &mut edges),
    ] {
        let old_rows = nav_array(historical, collection)?;
        if historical["counts"][collection].as_u64() != u64::try_from(old_rows.len()).ok() {
            return Err(ReferenceError::InvalidBinding);
        }
        let mut prior_id: Option<&str> = None;
        for row in old_rows {
            l.step(1)?;
            let id = nav_str(row, key)?;
            if prior_id.is_some_and(|p| p >= id) {
                return Err(ReferenceError::InvalidBinding);
            }
            prior_id = Some(id);
            let row = l.copy(row)?;
            if rows.contains_key(nav_str(&row, key)?) {
                return Err(ReferenceError::DuplicateIdentity);
            }
            nav_merge(rows, row, key, &mut output_bytes, l)?;
        }
    }
    let mut claims_seen = std::collections::BTreeSet::new();
    for claim in &inputs.claims {
        l.fixed(24)?;
        // Added source-length-admission locals: event source borrow plus one
        // target borrow, checked length sums/results and admission arguments.
        l.bytes(
            std::mem::size_of::<&str>()
                .checked_mul(2)
                .and_then(|n| n.checked_add(std::mem::size_of::<Option<usize>>().checked_mul(4)?))
                .and_then(|n| n.checked_add(std::mem::size_of::<usize>().checked_mul(4)?))
                .and_then(|n| n.checked_add(std::mem::size_of::<NavResult<()>>().checked_mul(2)?))
                .ok_or(ReferenceError::Budget)?,
        )?;
        let c = &claim.value;
        let id = nav_str(c, "claim_id")?;
        if !claims_seen.insert(id) {
            return Err(ReferenceError::DuplicateIdentity);
        }
        l.bytes(id.len().checked_add(256).ok_or(ReferenceError::Budget)?)?;
        let claim_id = format!("claim:{id}");
        let subject = inputs
            .identities
            .get(nav_str(c, "subject_ref")?)
            .ok_or(ReferenceError::InvalidBinding)?;
        let object = inputs
            .identities
            .get(
                c["object"]
                    .as_str()
                    .ok_or(ReferenceError::UnsupportedSourceProfile)?,
            )
            .ok_or(ReferenceError::InvalidBinding)?;
        for source in [subject, object] {
            l.fixed(8)?;
            nav_no_reserved(
                &source.value,
                &[
                    "identity_ref",
                    "identity_kind",
                    "source_record",
                    "human_forms",
                    "human_forms_source_ref",
                    "bibliographic_carrier",
                ],
                l,
            )?;
            let rid = nav_str(&source.value, "record_id")?;
            l.bytes(rid.len().checked_mul(2).ok_or(ReferenceError::Budget)?)?;
            let mut props = l.copy(&source.value)?;
            let map = props
                .as_object_mut()
                .ok_or(ReferenceError::InvalidBinding)?;
            map.insert("identity_ref".into(), json!(rid));
            map.insert(
                "identity_kind".into(),
                l.copy(&source.value["record_type"])?,
            );
            map.insert("source_record".into(), l.copy(&source.value)?);
            map.insert(
                "bibliographic_carrier".into(),
                json!({"source_sha256":source.sha}),
            );
            if let Some((path, forms)) = nav_forms(source, inputs, l)? {
                map.insert("human_forms".into(), forms);
                map.insert("human_forms_source_ref".into(), json!(path));
            }
            let row = nav_node(
                format!("identity:{rid}"),
                nav_str(&source.value, "record_type")?,
                &source.path,
                nav_str(&source.value, "identity_status")?,
                props,
                l,
            )?;
            nav_merge(&mut nodes, row, "node_id", &mut output_bytes, l)?;
        }
        let event = inputs
            .events
            .get(nav_str(c, "provenance_event_ref")?)
            .ok_or(ReferenceError::InvalidBinding)?;
        nav_no_reserved(
            &event.value,
            &[
                "event_ref",
                "source_event",
                "human_forms",
                "bibliographic_carrier",
            ],
            l,
        )?;
        let event_source_id = nav_str(&event.value, "event_id")?;
        l.bytes(
            "provenance_event:"
                .len()
                .checked_add(event_source_id.len())
                .ok_or(ReferenceError::Budget)?,
        )?;
        let event_id = format!("provenance_event:{event_source_id}");
        l.fixed(10)?;
        let mut props = l.copy(&event.value)?;
        let map = props
            .as_object_mut()
            .ok_or(ReferenceError::InvalidBinding)?;
        map.remove("inputs");
        map.remove("outputs");
        map.insert("event_ref".into(), l.copy(&event.value["event_id"])?);
        map.insert("source_event".into(), l.copy(&event.value)?);
        let mut agents = Vec::new();
        if event.value["schema_version"] == "tos_provenance_event_v2" {
            for responsibility in nav_array(&event.value, "responsibility")? {
                l.step(2)?;
                let agent = nav_str(responsibility, "agent_ref")?;
                l.step(agents.len())?;
                if !agents.iter().any(|v: &Value| v.as_str() == Some(agent)) {
                    l.bytes(agent.len().checked_add(128).ok_or(ReferenceError::Budget)?)?;
                    agents.push(json!(agent));
                }
            }
            for key in ["event_type", "started_at", "ended_at", "status"] {
                l.step(1)?;
                map.insert(key.into(), l.copy(&event.value["activity"][key])?);
            }
        } else if event.value["schema_version"] == "tos_provenance_event_v1" {
            agents = match l.copy(&event.value["agent_refs"])? {
                Value::Array(values) => values,
                _ => return Err(ReferenceError::InvalidBinding),
            };
        } else {
            return Err(ReferenceError::UnsupportedSourceProfile);
        }
        map.insert("agent_refs".into(), Value::Array(agents));
        map.insert("bibliographic_carrier".into(),json!({"source_line":event.line.ok_or(ReferenceError::InvalidBinding)?,"source_sha256":event.sha}));
        l.bytes(event_id.len())?; // argument clone occurs BEFORE callee admission
        let row = nav_node(
            event_id.clone(),
            "provenance_event",
            &event.path,
            "not_applicable",
            props,
            l,
        )?;
        nav_merge(&mut nodes, row, "node_id", &mut output_bytes, l)?;
        let maker = &c["maker"];
        let maker_ref = nav_str(maker, "agent_ref")?;
        l.fixed(2)?;
        l.bytes(
            id.len()
                .checked_add(maker_ref.len())
                .ok_or(ReferenceError::Budget)?,
        )?;
        let maker_id = format!("maker:claim:sha256:{}", l.digest(&json!([id, maker_ref]))?);
        let resolved_maker = inputs.identities.get(maker_ref);
        let (maker_path, maker_sha) = resolved_maker
            .map(|r| (r.path.as_str(), r.sha.as_str()))
            .unwrap_or((&claim.path, &claim.sha));
        l.fixed(8)?;
        if resolved_maker.is_some() && !nodes.contains_key(&format!("identity:{maker_ref}")) {
            return Err(ReferenceError::UnsupportedSourceProfile);
        }
        l.bytes(
            maker_ref
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(maker_path.len()))
                .and_then(|n| n.checked_add(nav_str(maker, "maker_type").ok()?.len()))
                .ok_or(ReferenceError::Budget)?,
        )?;
        let maker_source = l.copy(maker)?;
        let props = nav_object(
            [
                ("agent_ref", json!(maker_ref)),
                ("maker_type", json!(nav_str(maker, "maker_type")?)),
                ("source_maker", maker_source),
                (
                    "identity_node_id",
                    json!(resolved_maker.map(|_| format!("identity:{maker_ref}"))),
                ),
                ("bibliographic_carrier", json!({"source_sha256":maker_sha})),
            ],
            l,
        )?;
        l.bytes(maker_id.len())?;
        let row = nav_node(
            maker_id.clone(),
            "maker",
            maker_path,
            "not_applicable",
            props,
            l,
        )?;
        nav_merge(&mut nodes, row, "node_id", &mut output_bytes, l)?;
        let mut evidence_ids = Vec::new();
        for evidence in nav_array(c, "evidence_refs")? {
            l.step(1)?;
            let row = nav_evidence(
                evidence
                    .as_str()
                    .ok_or(ReferenceError::UnsupportedSourceProfile)?,
                inputs,
                l,
            )?;
            evidence_ids.push(l.copy(&row["node_id"])?);
            nav_merge(&mut nodes, row, "node_id", &mut output_bytes, l)?;
        }
        nav_no_reserved(
            c,
            &[
                "claim_ref",
                "source_claim",
                "navigation_descriptor",
                "human_forms",
                "human_forms_source_ref",
                "bibliographic_carrier",
            ],
            l,
        )?;
        let descriptor = nav_descriptor(claim, subject, object, inputs, l)?;
        l.fixed(15)?;
        let mut props = l.copy(c)?;
        let map = props
            .as_object_mut()
            .ok_or(ReferenceError::InvalidBinding)?;
        map.insert("claim_ref".into(), json!(id));
        map.insert("confidence".into(), l.copy(&c["confidence"])?);
        map.insert("source_claim".into(), l.copy(c)?);
        map.insert("navigation_descriptor".into(), descriptor);
        if let Some((path, forms)) = nav_forms(claim, inputs, l)? {
            map.insert("human_forms".into(), forms);
            map.insert("human_forms_source_ref".into(), json!(path));
        }
        map.insert("bibliographic_carrier".into(),json!({"source_line":claim.line.ok_or(ReferenceError::InvalidBinding)?,"source_sha256":claim.sha}));
        l.bytes(claim_id.len())?;
        let row = nav_node(
            claim_id.clone(),
            "claim",
            &claim.path,
            "not_applicable",
            props,
            l,
        )?;
        nav_merge(&mut nodes, row, "node_id", &mut output_bytes, l)?;
        l.bytes(
            evidence_ids
                .len()
                .checked_add(4)
                .and_then(|n| n.checked_mul(192))
                .ok_or(ReferenceError::Budget)?,
        )?;
        let mut targets = vec![
            ("has_subject", {
                let subject = nav_str(c, "subject_ref")?;
                l.bytes(
                    "identity:"
                        .len()
                        .checked_add(subject.len())
                        .ok_or(ReferenceError::Budget)?,
                )?;
                format!("identity:{subject}")
            }),
            ("has_object", {
                let object = c["object"].as_str().ok_or(ReferenceError::InvalidBinding)?;
                l.bytes(
                    "identity:"
                        .len()
                        .checked_add(object.len())
                        .ok_or(ReferenceError::Budget)?,
                )?;
                format!("identity:{object}")
            }),
            ("made_by", {
                l.bytes(maker_id.len())?;
                maker_id.clone()
            }),
            ("generated_by", {
                l.bytes(event_id.len())?;
                event_id.clone()
            }),
        ];
        for id in &evidence_ids {
            l.step(1)?;
            targets.push((
                "supported_by",
                id.as_str()
                    .ok_or(ReferenceError::InvalidBinding)?
                    .to_owned(),
            ));
        }
        for (ordinal, (predicate, target)) in targets.into_iter().enumerate() {
            l.bytes(
                id.len()
                    .checked_add(predicate.len())
                    .and_then(|n| n.checked_add("edge:::".len()))
                    .and_then(|n| n.checked_add(20))
                    .ok_or(ReferenceError::Budget)?,
            )?;
            let row = nav_edge(
                claim,
                &claim_id,
                &target,
                predicate,
                predicate,
                &format!("edge:{id}:{predicate}:{:03}", ordinal + 1),
                &evidence_ids,
                &maker_id,
                &event_id,
                l,
            )?;
            nav_merge(&mut edges, row, "edge_id", &mut output_bytes, l)?;
        }
        l.bytes(
            "source-navigation:claim-subject:"
                .len()
                .checked_add(id.len())
                .ok_or(ReferenceError::Budget)?,
        )?;
        let inverse = nav_edge(
            claim,
            nav_str(c, "subject_ref")?,
            &claim_id,
            "claim_subject_navigation",
            "has_source_claim",
            &format!("source-navigation:claim-subject:{id}"),
            &evidence_ids,
            &maker_id,
            &event_id,
            l,
        )?;
        nav_merge(&mut edges, inverse, "edge_id", &mut output_bytes, l)?;
    }
    // Require every endpoint, including inverse RAW metadata subject, to exist.
    for row in edges.values() {
        l.step(3)?;
        if !nodes.contains_key(nav_str(row, "from_id")?)
            || !nodes.contains_key(nav_str(row, "to_id")?)
        {
            return Err(ReferenceError::InvalidBinding);
        }
    }
    let counts = json!({"nodes":nodes.len(),"edges":edges.len()});
    l.bytes(
        nodes
            .len()
            .checked_add(edges.len())
            .and_then(|n| n.checked_mul(128))
            .ok_or(ReferenceError::Budget)?,
    )?;
    let schema = l.copy(&historical["schema_version"])?;
    let authority = l.copy(&historical["authority_boundary"])?;
    let rights = l.copy(&historical["rights"])?;
    nav_object(
        [
            ("schema_version", schema),
            ("authority_boundary", authority),
            ("rights", rights),
            ("nodes", Value::Array(nodes.into_values().collect())),
            ("edges", Value::Array(edges.into_values().collect())),
            ("counts", counts),
        ],
        l,
    )
}

fn nav_edge(
    claim: &NavigationSource,
    from: &str,
    to: &str,
    kind: &str,
    predicate: &str,
    id: &str,
    evidence: &[serde_json::Value],
    maker: &str,
    event: &str,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<serde_json::Value> {
    l.fixed(15)?;
    l.bytes(
        evidence
            .len()
            .checked_mul(128)
            .ok_or(ReferenceError::Budget)?,
    )?;
    let mut refs = Vec::with_capacity(evidence.len());
    for value in evidence {
        refs.push(l.copy(value)?);
    }
    l.bytes(
        id.len()
            .checked_add(from.len())
            .and_then(|n| n.checked_add(to.len()))
            .and_then(|n| n.checked_add(claim.path.len().checked_mul(3)?))
            .and_then(|n| n.checked_add(kind.len()))
            .and_then(|n| n.checked_add(predicate.len()))
            .and_then(|n| n.checked_add(claim.sha.len()))
            .and_then(|n| n.checked_add(maker.len()))
            .and_then(|n| n.checked_add(event.len()))
            .ok_or(ReferenceError::Budget)?,
    )?;
    nav_object(
        [
            ("edge_id", serde_json::json!(id)),
            ("edge_kind", serde_json::json!(kind)),
            ("from_id", serde_json::json!(from)),
            ("to_id", serde_json::json!(to)),
            ("claim_ref", l.copy(&claim.value["claim_id"])?),
            ("claim_sha256", serde_json::json!(claim.sha)),
            ("evidence_node_ids", serde_json::Value::Array(refs)),
            ("maker_node_id", serde_json::json!(maker)),
            ("provenance_event_node_id", serde_json::json!(event)),
            ("review_status", l.copy(&claim.value["review_status"])?),
            ("source_claim_file_ref", serde_json::json!(claim.path)),
            (
                "source_claim_line",
                serde_json::json!(claim.line.ok_or(ReferenceError::InvalidBinding)?),
            ),
            ("predicate_id", serde_json::json!(predicate)),
            ("source_ref", serde_json::json!(claim.path)),
            ("source_refs", serde_json::json!([claim.path])),
        ],
        l,
    )
}

fn nav_rewrite_strings(
    value: &mut serde_json::Value,
    replacements: &BTreeMap<String, String>,
    l: &mut ReferenceLimits<'_>,
    depth: usize,
) -> NavResult<()> {
    use serde_json::Value;
    l.step(1)?;
    if depth > 96 {
        return Err(ReferenceError::Budget);
    }
    match value {
        Value::String(s) => {
            if let Some(new) = replacements.get(s) {
                l.bytes(new.len())?;
                *s = new.clone();
            }
        }
        Value::Array(a) => {
            for v in a {
                nav_rewrite_strings(v, replacements, l, depth + 1)?;
            }
        }
        Value::Object(o) => {
            for v in o.values_mut() {
                nav_rewrite_strings(v, replacements, l, depth + 1)?;
            }
        }
        _ => (),
    }
    Ok(())
}
// Explicit logical simultaneous holders of the pinned compact serializer.
// This is not a compiler-stack/RSS or arbitrary user-Serialize certificate.
fn variant_wire_holders(depth: usize) -> NavResult<usize> {
    use serde_json::ser::{CompactFormatter, Compound};
    use std::mem::size_of;
    type Ser = serde_json::Serializer<&'static mut Vec<u8>, CompactFormatter>;
    type Part = Compound<'static, &'static mut Vec<u8>, CompactFormatter>;
    let container = size_of::<Part>()
        .checked_add(
            size_of::<std::slice::Iter<'_, serde_json::Value>>()
                .max(size_of::<serde_json::map::Iter<'_>>()),
        )
        .and_then(|n| n.checked_add(size_of::<Option<(&String, &serde_json::Value)>>()))
        .and_then(|n| n.checked_add(size_of::<&serde_json::Value>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&String>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&mut Ser>().checked_mul(6)?))
        .and_then(|n| n.checked_add(size_of::<&mut Part>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<Result<(), serde_json::Error>>().checked_mul(4)?))
        .and_then(|n| n.checked_add(size_of::<Option<usize>>().checked_mul(4)?))
        .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(8)?))
        .ok_or(ReferenceError::Budget)?;
    let root = size_of::<Ser>()
        .checked_add(
            size_of::<Vec<u8>>()
                .checked_mul(2)
                .ok_or(ReferenceError::Budget)?,
        )
        .and_then(|n| n.checked_add(size_of::<CanonicalPlan<VariantWireCounts>>()))
        .and_then(|n| n.checked_add(size_of::<NavResult<CanonicalPlan<VariantWireCounts>>>()))
        .and_then(|n| n.checked_add(size_of::<Result<(), serde_json::Error>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<NavResult<Vec<u8>>>()))
        .and_then(|n| n.checked_add(size_of::<&serde_json::Value>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&mut Vec<u8>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&mut ReferenceLimits<'_>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<Option<usize>>().checked_mul(8)?))
        .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(8)?))
        .ok_or(ReferenceError::Budget)?;
    let leaf = size_of::<Part>()
        .checked_add(
            size_of::<&str>()
                .checked_mul(8)
                .ok_or(ReferenceError::Budget)?,
        )
        .and_then(|n| n.checked_add(size_of::<&[u8]>().checked_mul(8)?))
        .and_then(|n| n.checked_add(size_of::<Option<(&u8, &[u8])>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<(&[u8], &[u8])>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&mut Ser>().checked_mul(4)?))
        .and_then(|n| n.checked_add(size_of::<&mut Vec<u8>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<std::io::Result<()>>().checked_mul(4)?))
        .and_then(|n| n.checked_add(size_of::<Result<(), serde_json::Error>>().checked_mul(4)?))
        .and_then(|n| n.checked_add(size_of::<serde_json::ser::CharEscape>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<Option<u8>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<u8>().checked_mul(4)?))
        .and_then(|n| n.checked_add(6))
        .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(8)?))
        .ok_or(ReferenceError::Budget)?;
    root.checked_add(depth.checked_mul(container).ok_or(ReferenceError::Budget)?)
        .and_then(|n| n.checked_add(leaf))
        .ok_or(ReferenceError::Budget)
}
fn nav_variant_raw(v: &serde_json::Value, l: &mut ReferenceLimits<'_>) -> NavResult<Vec<u8>> {
    use std::mem::size_of;
    l.step(1)?;
    // BEFORE purpose/Plan/result and checked payment/holder-calculation locals.
    // Serializer holders below are an additional, separately named debit.
    l.bytes(
        size_of::<CanonicalPlan<VariantWireCounts>>()
            .checked_add(size_of::<NavResult<CanonicalPlan<VariantWireCounts>>>())
            .and_then(|n| n.checked_add(size_of::<PreflightPurpose>()))
            .and_then(|n| n.checked_add(size_of::<&serde_json::Value>().checked_mul(2)?))
            .and_then(|n| n.checked_add(size_of::<&mut ReferenceLimits<'_>>().checked_mul(2)?))
            .and_then(|n| n.checked_add(size_of::<Option<usize>>().checked_mul(8)?))
            .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(8)?))
            .and_then(|n| n.checked_add(size_of::<NavResult<usize>>().checked_mul(2)?))
            .ok_or(ReferenceError::Budget)?,
    )?;
    let plan = canonical_preflight_for::<VariantWireCounts>(v, l, PreflightPurpose::VariantWire)?;
    let bound = plan.meta.wire;
    if bound > l.max_record_bytes {
        return Err(ReferenceError::Budget);
    }
    l.bytes(bound)?; // original output reservation retained WITHOUT credit
    l.step(4)?;
    let capacity = plan.meta.raw.checked_add(1).ok_or(ReferenceError::Budget)?;
    l.bytes(
        capacity
            .checked_mul(2) // requested buffer + actual emission (one spare newline)
            .and_then(|n| n.checked_add(plan.meta.extra.reread)) // separate serializer scan
            .ok_or(ReferenceError::Budget)?,
    )?;
    l.bytes(variant_wire_holders(plan.max_frames)?)?;
    l.step(
        plan.meta
            .values
            .checked_add(plan.meta.extra.keys)
            .ok_or(ReferenceError::Budget)?,
    )?;
    // SAME compact serializer as to_vec, with prospective exact capacity.
    // All final sources are immutably borrowed after their last mutation.
    let mut output = Vec::with_capacity(capacity);
    serde_json::to_writer(&mut output, v).map_err(|_| ReferenceError::InvalidBinding)?;
    l.step(1)?;
    if output.len() != plan.meta.raw {
        return Err(ReferenceError::InvalidBinding);
    }
    Ok(output)
}
fn canonical_depth_input(
    depth: usize,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<serde_json::Value> {
    use serde_json::Value;
    use std::mem::size_of;
    l.bytes(
        depth
            .checked_mul(2)
            .and_then(|n| n.checked_add(3))
            .and_then(|n| n.checked_mul(size_of::<Value>()))
            .and_then(|n| n.checked_add(size_of::<Vec<Value>>().checked_mul(2)?))
            .ok_or(ReferenceError::Budget)?,
    )?;
    l.step(
        depth
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or(ReferenceError::Budget)?,
    )?;
    let mut value = Value::Null;
    for _ in 0..depth {
        let mut parent = Vec::with_capacity(1);
        parent.push(value);
        value = Value::Array(parent);
    }
    Ok(value)
}
fn reference_move_controls(l: &mut ReferenceLimits<'_>) -> NavResult<()> {
    // Borrowed authored literals, not SUT output or a second source oracle. The
    // tiny old-serde comparison is ONLY pinned representation compatibility.
    // All controls use the containing case's SAME remainder and cancellation.
    l.step(1)?;
    reference_move_layout()?; // the comparison reservation uses the SAME pinned layout
    // Authored typed inputs for canonical CONSTRUCTION compatibility, not SUT
    // outputs or new semantic goldens. Every allocation/old+new route is paid.
    use serde_json::{Value as CanonicalValue, json as canonical_json};
    let text = "quote\" slash\\ line\n\0 é😀";
    // Five literal cases: 18 Values + 7 keys. Numeric literal formatting uses
    // the existing bounded scalar envelope; case-array/move headers distinct.
    l.fixed(25)?;
    l.bytes(
        text.len()
            .checked_add(3 + 4 + 28 + 4 + 1 + 6 + 30 + 28 + 7 + 7 + 7)
            .and_then(|n| n.checked_add(3 * 64))
            .and_then(|n| n.checked_add(std::mem::size_of::<CanonicalValue>().checked_mul(12)?))
            .ok_or(ReferenceError::Budget)?,
    )?;
    let depth96 = canonical_depth_input(96, l)?;
    let depth97 = canonical_depth_input(97, l)?;
    let canonical_cases = [
        CanonicalValue::String(text.to_owned()),
        canonical_json!({"$serde_json::private::Number":"123","tail":[true,null]}),
        canonical_json!({"a":0,"$serde_json::private::Number":"literal","nested":{"é😀\0":"control","$serde_json::private::RawValue":"null"}}),
        canonical_json!([{}, [], -0.0, 0.001]),
        CanonicalValue::Number(reference_number_from_lexeme("1e100", l)?),
        depth96,
    ];
    for value in &canonical_cases {
        let fresh = l.canonical_value(value)?;
        let legacy = l.canonical_value_legacy(value)?;
        l.bytes(
            fresh
                .len()
                .checked_add(legacy.len())
                .ok_or(ReferenceError::Budget)?,
        )?;
        l.step(1)?;
        assert_eq!(
            fresh, legacy,
            "complete canonical constructor compatibility"
        );
    }
    // Prepare all negative inputs BEFORE any expected Budget below. No case
    // uses a new work pool, and no diagnostic candidate is cleared here.
    l.fixed(11)?; // ten Values plus one key; depth97 was paid independently
    l.bytes(6 + 8)?; // three "aa" payloads and the exact "long-key" bytes
    let short = CanonicalValue::String("aa".to_owned());
    // Positive EXACT conservative-W boundary precedes all expected failures.
    let saved_ceiling = l.max_output_bytes;
    l.max_output_bytes = saved_ceiling.min(14);
    let at_boundary = l.canonical_value(&short)?;
    let old_boundary = l.canonical_value_legacy(&short)?;
    l.bytes(
        at_boundary
            .len()
            .checked_add(old_boundary.len())
            .ok_or(ReferenceError::Budget)?,
    )?;
    l.step(1)?;
    assert_eq!(at_boundary, old_boundary);
    l.max_output_bytes = saved_ceiling;
    let early_w = CanonicalValue::Array(vec![
        CanonicalValue::String("aa".to_owned()),
        CanonicalValue::Number(reference_number_from_lexeme("1e9999", l)?),
    ]);
    let early_number = CanonicalValue::Array(vec![
        CanonicalValue::Number(reference_number_from_lexeme("1e9999", l)?),
        CanonicalValue::String("aa".to_owned()),
    ]);
    let mut key_map = serde_json::Map::new();
    key_map.insert(
        "long-key".to_owned(),
        CanonicalValue::Number(reference_number_from_lexeme("1e9999", l)?),
    );
    let key_before_number = CanonicalValue::Object(key_map);
    // W(number)=64, but exact serialized token is100 bytes; the two guards
    // must remain separate. Old comparison wire capacity/work is prepaid too.
    let numeric_wire = CanonicalValue::Number(reference_number_from_lexeme(
        "1000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
        l,
    )?);
    l.bytes(200)?;
    l.bytes(
        std::mem::size_of::<[(&CanonicalValue, usize, ReferenceError); 5]>()
            .checked_mul(2)
            .ok_or(ReferenceError::Budget)?,
    )?;
    l.bytes(
        std::mem::size_of::<[&[u8]; 16]>()
            .checked_add(std::mem::size_of::<[&[u8]; 6]>())
            .and_then(|n| n.checked_add(std::mem::size_of::<[&[u8]; 11]>()))
            .and_then(|n| n.checked_add(std::mem::size_of::<serde_json::Value>().checked_mul(2)?))
            .and_then(|n| n.checked_add(std::mem::size_of::<&str>().checked_mul(8)?))
            .ok_or(ReferenceError::Budget)?,
    )?; // BEFORE constructing borrowed literal arrays / comparison holders
    for (index, raw) in [
        &b"null"[..],
        &b"true"[..],
        &b"false"[..],
        &b"-0"[..],
        &b"1.0"[..],
        &b"18446744073709551616"[..],
        &b"-9223372036854775809"[..],
        &b"-1e-9999"[..],
        &br#""\uD834\uDD1E\n\u0000""#[..],
        &br#"[]"#[..],
        &br#"{}"#[..],
        &br#"{"a":[null,true,false,-0,1.0,"x"],"b":{"c":[]},"d":{}}"#[..],
        &br#"{"ordinary":0,"$serde_json::private::Number":"123","last":true}"#[..],
        &br#"{"ordinary":0,"$serde_json::private::RawValue":"[1]","last":true}"#[..],
        &br#"{"outer":{"ordinary":0,"$serde_json::private::Number":"123","$serde_json::private::RawValue":"null"}}"#[..],
        &br#"{"\uD834\uDD1E":"paired key","controls":"\b\f\r\t"}"#[..],
    ].into_iter().enumerate() {
        l.work.diagnostic_stage("compat-supported", index);
        l.step(1)?;
        // Each literal has <=32 values/keys. Cover the old scalar/control
        // decoder's tree/text plus whole comparison BEFORE either action.
        // This is a control-only reservation, never the new bridge's input law.
        l.bytes(
            raw.len()
                .checked_mul(16)
                .and_then(|n| n.checked_add(32 * 128 + 4096))
                .ok_or(ReferenceError::Budget)?,
        )?;
        l.step(96)?;
        let expected: serde_json::Value =
            serde_json::from_slice(raw).map_err(|_| ReferenceError::InvalidBinding)?;
        let actual = l.parse(raw, l.max_record_bytes)?;
        assert_eq!(actual, expected);
        let retained = l.retained_root(raw, l.max_record_bytes)?;
        let requested = reference_requested_value(&retained, l)?;
        l.compare_work(&requested, 0)?;
        l.compare_work(&expected, 0)?;
        assert_eq!(requested, expected);
        // Value equality alone does not certify IndexMap insertion order. The
        // bounded root/child objects in these authored controls check it too.
        if let (Some(actual), Some(expected)) = (actual.as_object(), expected.as_object()) {
            assert!(actual.keys().eq(expected.keys()));
            for (actual, expected) in actual.values().zip(expected.values()) {
                if let (Some(actual), Some(expected)) = (actual.as_object(), expected.as_object()) {
                    assert!(actual.keys().eq(expected.keys()));
                }
            }
        }
    }
    for (index, raw) in [
        &br#"{"$serde_json::private::Number":"123"}"#[..],
        &br#"{"$serde_json::private::RawValue":"null"}"#[..],
        &br#"[{"$serde_json::private::Number":"123"}]"#[..],
        &br#"{"outer":{"$serde_json::private::RawValue":"null"}}"#[..],
        &br#"{"\u0024serde_json::private::Number":"123"}"#[..],
        &br#"{"\u0024serde_json::private::RawValue":"null"}"#[..],
    ]
    .into_iter()
    .enumerate()
    {
        l.work.diagnostic_stage("compat-reserved", index);
        l.step(1)?;
        assert_eq!(
            l.parse(raw, l.max_record_bytes),
            Err(ReferenceError::UnsupportedSourceProfile)
        );
        assert_eq!(
            l.retained_root(raw, l.max_record_bytes),
            Err(ReferenceError::UnsupportedSourceProfile)
        );
    }
    for (index, raw) in [
        &br#"{"$serde_json::private::Number":"123","tail":]}"#[..],
        &br#"{"$serde_json::private::RawValue":"null","a":1,"\u0061":2}"#[..],
        &br#"{"a":1,"\u0061":2}"#[..],
        &br#""\uD800""#[..],
        &br#"{"\uD800":0}"#[..],
        &b"\"\xff\""[..],
        &b"01"[..],
        &b"1e"[..],
        &b"NaN"[..],
        &b"Infinity"[..],
        // serde arbitrary_precision accepts this token, but PublishedStrict
        // rejects its nonfinite f64 interpretation BEFORE either bridge.
        &b"1e9999"[..],
    ]
    .into_iter()
    .enumerate()
    {
        l.work.diagnostic_stage("compat-invalid", index);
        l.step(1)?;
        assert_eq!(
            l.parse(raw, l.max_record_bytes),
            Err(ReferenceError::InvalidBinding)
        );
        assert_eq!(
            l.retained_root(raw, l.max_record_bytes),
            Err(ReferenceError::InvalidBinding)
        );
    }
    l.work.diagnostic_stage("compat-unselected", 0);
    // Even an UNSELECTED nested reserved-first object must refuse before a
    // caller can extract the innocuous requested sibling.
    assert_eq!(
        l.retained_root(
            br#"{"requested":1,"unused":{"$serde_json::private::Number":"1"}}"#,
            l.max_record_bytes
        ),
        Err(ReferenceError::UnsupportedSourceProfile)
    );
    // A malformed tail still wins over the earlier reserved-key profile.
    assert_eq!(
        l.retained_root(
            br#"{"requested":1,"unused":{"$serde_json::private::Number":"1"},"tail":]}"#,
            l.max_record_bytes
        ),
        Err(ReferenceError::InvalidBinding)
    );
    l.work.diagnostic_stage("compat-depth", 0);
    // One borrowed over-depth literal, with no generated runtime input buffer.
    // The strict parser rejects it BEFORE the move workspace/profile is entered.
    let too_deep = b"[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[0]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]";
    l.step(1)?;
    assert_eq!(
        l.parse(too_deep, l.max_record_bytes),
        Err(ReferenceError::Budget)
    );
    let original_ceiling = l.max_output_bytes;
    for (value, ceiling, error) in [
        (&short, original_ceiling.min(13), ReferenceError::Budget),
        (&early_w, original_ceiling.min(13), ReferenceError::Budget),
        (
            &early_number,
            original_ceiling.min(8),
            ReferenceError::UnsupportedSourceProfile,
        ),
        (
            &key_before_number,
            original_ceiling.min(8),
            ReferenceError::UnsupportedSourceProfile,
        ),
        (
            &numeric_wire,
            original_ceiling.min(80),
            ReferenceError::Budget,
        ),
    ] {
        l.max_output_bytes = ceiling; // narrower POLICY fixture, not a counter reset
        assert_eq!(l.canonical_value(value), Err(error));
        assert_eq!(l.canonical_value_legacy(value), Err(error));
    }
    l.max_output_bytes = original_ceiling;
    assert_eq!(l.canonical_value(&depth97), Err(ReferenceError::Budget));
    assert_eq!(
        l.canonical_value_legacy(&depth97),
        Err(ReferenceError::Budget)
    );
    Ok(())
}
// Prospective payment BEFORE the caller passes the parsed owner by value.
// Outer raw files/registries have independent immutable owners and are borrowed.
fn navigation_variant_handoff_payment(l: &mut ReferenceLimits<'_>) -> NavResult<()> {
    use std::mem::size_of;
    l.step(1)?;
    l.bytes(
        size_of::<DeclaredNavigationInputs<'_>>()
            .checked_mul(3) // caller -> diagnostic wrapper -> body move overlap
            .and_then(|n| n.checked_add(size_of::<&serde_json::Value>().checked_mul(4)?))
            .and_then(|n| n.checked_add(size_of::<&mut ReferenceLimits<'_>>().checked_mul(2)?))
            .and_then(|n| n.checked_add(size_of::<NavResult<()>>().checked_mul(4)?))
            .and_then(|n| n.checked_add(size_of::<Option<usize>>().checked_mul(4)?))
            .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(4)?))
            .ok_or(ReferenceError::Budget)?,
    )
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum VariantSourceRole {
    Identity,
    Claim,
    Event,
    Form,
}
type OwnedVariantSource = (VariantSourceRole, NavigationSource);
type VariantMapIntoValues = std::collections::btree_map::IntoValues<String, NavigationSource>;
type VariantClaimIntoIter = std::vec::IntoIter<NavigationSource>;
type VariantMapAdapter =
    std::iter::Map<VariantMapIntoValues, fn(NavigationSource) -> OwnedVariantSource>;
type VariantClaimAdapter =
    std::iter::Map<VariantClaimIntoIter, fn(NavigationSource) -> OwnedVariantSource>;
type VariantFirstChain = std::iter::Chain<VariantMapAdapter, VariantClaimAdapter>;
type VariantSecondChain = std::iter::Chain<VariantFirstChain, VariantMapAdapter>;
type VariantOwnedChain = std::iter::Chain<VariantSecondChain, VariantMapAdapter>;
type VariantAuthoredSource = (String, Option<usize>, serde_json::Value);
struct VariantClaimBinding {
    path: String,
    line: Option<usize>,
    value: serde_json::Value,
}
fn variant_identity_source(source: NavigationSource) -> OwnedVariantSource {
    (VariantSourceRole::Identity, source)
}
fn variant_claim_source(source: NavigationSource) -> OwnedVariantSource {
    (VariantSourceRole::Claim, source)
}
fn variant_event_source(source: NavigationSource) -> OwnedVariantSource {
    (VariantSourceRole::Event, source)
}
fn variant_form_source(source: NavigationSource) -> OwnedVariantSource {
    (VariantSourceRole::Form, source)
}
fn variant_source_move_workspace() -> NavResult<usize> {
    use std::mem::size_of;
    size_of::<VariantOwnedChain>()
        .checked_mul(2) // completed chain plus for/IntoIterator move header
        .and_then(|n| {
            n.checked_add(
                size_of::<VariantFirstChain>()
                    .max(size_of::<VariantSecondChain>())
                    .checked_mul(2)?,
            )
        })
        .and_then(|n| n.checked_add(size_of::<VariantMapAdapter>().checked_mul(3)?))
        .and_then(|n| n.checked_add(size_of::<VariantClaimAdapter>()))
        .and_then(|n| n.checked_add(size_of::<VariantMapIntoValues>().checked_mul(3)?))
        .and_then(|n| n.checked_add(size_of::<VariantClaimIntoIter>()))
        .and_then(|n| {
            n.checked_add(size_of::<fn(NavigationSource) -> OwnedVariantSource>().checked_mul(2)?)
        })
        .and_then(|n| n.checked_add(size_of::<&NavigationSource>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&serde_json::Value>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&mut ReferenceLimits<'_>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<NavResult<VariantClaimBinding>>()))
        .and_then(|n| n.checked_add(size_of::<VariantClaimBinding>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<OwnedVariantSource>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<Option<OwnedVariantSource>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<NavigationSource>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<VariantAuthoredSource>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<VariantSourceRole>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&BTreeMap<String, Vec<u8>>>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<&NavigationRegistries>().checked_mul(2)?))
        .and_then(|n| n.checked_add(size_of::<Option<usize>>().checked_mul(8)?))
        .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(8)?))
        .and_then(|n| n.checked_add(size_of::<NavResult<usize>>().checked_mul(2)?))
        .ok_or(ReferenceError::Budget)
}
// This is ONLY original-input selection metadata. Preserve absent versus null;
// do not validate types early or use the already-mutated source to fill it.
fn variant_claim_binding(
    source: &NavigationSource,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<VariantClaimBinding> {
    use std::mem::size_of;
    l.fixed(4)?; // one small object and at most three map entries
    l.bytes(
        source
            .path
            .len()
            .checked_add(
                size_of::<VariantClaimBinding>()
                    .checked_mul(2)
                    .ok_or(ReferenceError::Budget)?,
            )
            .and_then(|n| n.checked_add(size_of::<NavResult<VariantClaimBinding>>()))
            .and_then(|n| {
                n.checked_add(
                    size_of::<serde_json::Map<String, serde_json::Value>>().checked_mul(2)?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    size_of::<serde_json::Value>()
                        .max(size_of::<Option<serde_json::Value>>())
                        .max(size_of::<NavResult<serde_json::Value>>())
                        .checked_mul(2)?,
                )
            })
            .and_then(|n| n.checked_add(size_of::<String>().checked_mul(2)?))
            .and_then(|n| n.checked_add(size_of::<Option<&serde_json::Value>>().checked_mul(2)?))
            .and_then(|n| n.checked_add(size_of::<&NavigationSource>().checked_mul(2)?))
            .and_then(|n| n.checked_add(size_of::<&mut ReferenceLimits<'_>>().checked_mul(2)?))
            .and_then(|n| n.checked_add(size_of::<[&str; 3]>()))
            .and_then(|n| n.checked_add(size_of::<std::array::IntoIter<&str, 3>>()))
            .and_then(|n| n.checked_add(size_of::<&str>().checked_mul(2)?))
            .and_then(|n| n.checked_add(size_of::<Option<usize>>().checked_mul(4)?))
            .and_then(|n| n.checked_add(size_of::<usize>().checked_mul(4)?))
            .ok_or(ReferenceError::Budget)?,
    )?;
    let mut fields = serde_json::Map::with_capacity(3);
    for key in ["evidence_refs", "predicate", "schema_version"] {
        l.step(1)?;
        if let Some(value) = source.value.get(key) {
            l.bytes(
                key.len()
                    .checked_mul(6)
                    .and_then(|n| n.checked_add(4 + 96))
                    .ok_or(ReferenceError::Budget)?,
            )?;
            let value = l.copy(value)?;
            fields.insert(key.to_owned(), value);
        }
    }
    Ok(VariantClaimBinding {
        path: source.path.clone(),
        line: source.line,
        value: serde_json::Value::Object(fields),
    })
}
fn navigation_reference_variants(
    historical: &serde_json::Value,
    original: &serde_json::Value,
    inputs: DeclaredNavigationInputs<'_>,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<()> {
    l.work
        .begin_variant_diagnostic()
        .map_err(|_| ReferenceError::Budget)?;
    let result = navigation_reference_variants_body(historical, original, inputs, l);
    if let Err(error) = result {
        let name = match error {
            ReferenceError::Budget => "Budget",
            ReferenceError::InvalidBinding => "InvalidBinding",
            ReferenceError::UnsupportedSourceProfile => "UnsupportedSourceProfile",
            ReferenceError::DuplicateIdentity => "DuplicateIdentity",
            ReferenceError::Conflict => "Conflict",
        };
        l.work.report_refusal(name);
    } else {
        // Observation is operation-local. Dropping its owner is not a refund;
        // every diagnostic prepayment remains consumed on the same account.
        l.work.diagnostic = None;
    }
    result
}
fn navigation_reference_variants_body(
    historical: &serde_json::Value,
    original: &serde_json::Value,
    inputs: DeclaredNavigationInputs<'_>,
    l: &mut ReferenceLimits<'_>,
) -> NavResult<()> {
    use serde_json::{Value, json};
    l.fixed(8)?;
    l.work.diagnostic_stage("compat-controls", 0);
    reference_move_controls(l)?;
    l.work.diagnostic_expected_controls_consumed();
    assert_eq!(nav_label(Some(""), Some("preferred"), "node"), "node");
    assert_eq!(nav_label(None, Some("preferred"), "node"), "preferred");
    l.work.diagnostic_stage("duplicate-and-pointer-controls", 0);
    let sample = inputs
        .identities
        .values()
        .next()
        .ok_or(ReferenceError::InvalidBinding)?;
    let raw = inputs
        .files
        .get(&sample.path)
        .ok_or(ReferenceError::InvalidBinding)?;
    let kind = nav_str(&sample.value, "record_type")?;
    l.bytes(
        raw.len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(512))
            .ok_or(ReferenceError::Budget)?,
    )?;
    let duplicate = BTreeMap::from([
        (
            format!("ToS/source-witnesses/reference-duplicate/first/{kind}.json"),
            raw.clone(),
        ),
        (
            format!("ToS/source-witnesses/reference-duplicate/second/{kind}.json"),
            raw.clone(),
        ),
    ]);
    assert!(matches!(
        DeclaredNavigationInputs::from_files(&duplicate, inputs.registries, l),
        Err(ReferenceError::DuplicateIdentity)
    ));
    assert!(matches!(
        nav_pointer(&sample.value, "/invalid~2escape", l),
        Err(ReferenceError::InvalidBinding)
    ));
    assert!(matches!(
        nav_evidence(
            "ToS/research-packets/reference-input-variant/MISSING.md",
            &inputs,
            l
        ),
        Err(ReferenceError::InvalidBinding)
    ));
    l.work
        .diagnostic_stage("unknown-claim-descriptor-controls", 0);
    let source_claim = inputs
        .claims
        .first()
        .ok_or(ReferenceError::InvalidBinding)?;
    let source_subject = inputs
        .identities
        .get(nav_str(&source_claim.value, "subject_ref")?)
        .ok_or(ReferenceError::InvalidBinding)?;
    let source_object = inputs
        .identities
        .get(
            source_claim.value["object"]
                .as_str()
                .ok_or(ReferenceError::InvalidBinding)?,
        )
        .ok_or(ReferenceError::InvalidBinding)?;
    l.bytes(
        source_claim
            .path
            .len()
            .checked_add(64)
            .ok_or(ReferenceError::Budget)?,
    )?;
    let mut unknown_claim = NavigationSource {
        path: source_claim.path.clone(),
        line: source_claim.line,
        value: l.copy(&source_claim.value)?,
        sha: source_claim.sha.clone(),
    };
    unknown_claim.value["predicate"] = json!("reference_unsupported_predicate");
    unknown_claim.sha = l.digest(&unknown_claim.value)?;
    assert!(matches!(
        nav_descriptor(&unknown_claim, source_subject, source_object, &inputs, l),
        Err(ReferenceError::UnsupportedSourceProfile)
    ));
    unknown_claim.value["predicate"] = l.copy(&source_claim.value["predicate"])?;
    unknown_claim.value["object"] = l.copy(&source_claim.value["subject_ref"])?;
    unknown_claim.sha = l.digest(&unknown_claim.value)?;
    assert!(matches!(
        nav_descriptor(&unknown_claim, source_subject, source_subject, &inputs, l),
        Err(ReferenceError::UnsupportedSourceProfile)
    ));
    l.work.diagnostic_stage("collision-control", 0);
    let original_row = nav_array(original, "nodes")?
        .first()
        .ok_or(ReferenceError::InvalidBinding)?;
    // Construct the SAME collision row without cloning a properties subtree
    // that is immediately discarded. Keep every other complete field. This
    // changes ownership/construction only, not the Conflict input or assertion.
    let original_fields = original_row
        .as_object()
        .ok_or(ReferenceError::InvalidBinding)?;
    l.step(1)?;
    l.bytes(128)?; // same logical object-header admission as copy/value_bound
    let mut colliding_fields = serde_json::Map::new();
    for (key, value) in original_fields {
        l.step(3)?; // field traversal, replacement decision and map insertion
        l.bytes(
            key.len()
                .checked_mul(6)
                .and_then(|n| n.checked_add(4 + 96))
                .ok_or(ReferenceError::Budget)?,
        )?;
        let value = if key == "properties" {
            l.step(1)?;
            l.bytes(128)?; // admit the constructed Null, not the discarded tree
            Value::Null
        } else {
            l.copy(value)?
        };
        colliding_fields.insert(key.clone(), value);
    }
    // The independently constructed navigation node profile has this required
    // field; do not silently broaden this constructor to other row profiles.
    l.step(1)?;
    l.bytes("properties".len())?;
    if !colliding_fields.contains_key("properties") {
        return Err(ReferenceError::InvalidBinding);
    }
    let colliding = Value::Object(colliding_fields);
    // The immutable complete baseline already has an owner in `original`,
    // which outlives this local map. Store a borrow, not another deep clone.
    // Prepay both Cow wrappers plus construction/move and reference holders;
    // this never replaces nav_merge's full bound/comparison/insertion work.
    l.bytes(
        std::mem::size_of::<std::borrow::Cow<'_, Value>>()
            .checked_mul(4)
            .and_then(|n| n.checked_add(std::mem::size_of::<&Value>().checked_mul(4)?))
            .ok_or(ReferenceError::Budget)?,
    )?;
    l.step(2)?;
    let mut collision_rows = BTreeMap::new();
    let mut collision_bytes = 0usize;
    nav_merge(
        &mut collision_rows,
        std::borrow::Cow::Borrowed(original_row),
        "node_id",
        &mut collision_bytes,
        l,
    )?;
    assert!(matches!(
        nav_merge(
            &mut collision_rows,
            std::borrow::Cow::Owned(colliding),
            "node_id",
            &mut collision_bytes,
            l
        ),
        Err(ReferenceError::Conflict)
    ));
    // This is a second authored INPUT, not a second expected packet derived
    // from SUT. Reuse immutable registry roots and the SAME work remainder.
    l.work.diagnostic_stage("copy-authored-variant-sources", 0);
    l.bytes(variant_source_move_workspace()?)?; // BEFORE owning iterators/count holders
    l.step(8)?; // four collection lengths, checked sums and Claim capacity read
    let source_count = inputs
        .identities
        .len()
        .checked_add(inputs.claims.len())
        .and_then(|n| n.checked_add(inputs.events.len()))
        .and_then(|n| n.checked_add(inputs.forms.len()))
        .ok_or(ReferenceError::Budget)?;
    let claim_count = inputs.claims.len();
    // Counts come from the already-admitted private collections. Exact initial
    // capacities avoid any new sources/shadow Vec growth during the move loop.
    l.bytes(
        source_count
            .checked_mul(std::mem::size_of::<VariantAuthoredSource>())
            .and_then(|n| {
                n.checked_add(claim_count.checked_mul(std::mem::size_of::<VariantClaimBinding>())?)
            })
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<Vec<VariantAuthoredSource>>().checked_mul(2)?)
            })
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<Vec<VariantClaimBinding>>().checked_mul(2)?)
            })
            .ok_or(ReferenceError::Budget)?,
    )?;
    l.step(source_count.checked_mul(4).ok_or(ReferenceError::Budget)?)?;
    let input_files = inputs.files;
    let input_registries = inputs.registries;
    let mut replacements = BTreeMap::new();
    let mut sources = Vec::with_capacity(source_count);
    let mut original_claims = Vec::with_capacity(claim_count);
    for (role, source) in inputs
        .identities
        .into_values()
        .map(variant_identity_source as fn(NavigationSource) -> OwnedVariantSource)
        .chain(
            inputs
                .claims
                .into_iter()
                .map(variant_claim_source as fn(NavigationSource) -> OwnedVariantSource),
        )
        .chain(
            inputs
                .events
                .into_values()
                .map(variant_event_source as fn(NavigationSource) -> OwnedVariantSource),
        )
        .chain(
            inputs
                .forms
                .into_values()
                .map(variant_form_source as fn(NavigationSource) -> OwnedVariantSource),
        )
    {
        l.step(1)?;
        l.bytes(
            source
                .path
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(256))
                .ok_or(ReferenceError::Budget)?,
        )?;
        replacements.insert(
            source.path.clone(),
            source.path.replacen(
                "ToS/source-witnesses/",
                "ToS/source-witnesses/reference-variant/",
                1,
            ),
        );
        // SAME complete old W/error admission at the former clone position.
        // The parsed owner is no longer needed after these original controls.
        merge_read_bound(&source.value, l)?;
        if role == VariantSourceRole::Claim {
            original_claims.push(variant_claim_binding(&source, l)?);
        }
        let mut value = source.value; // whole immutable input -> authored variant owner
        for key in ["record_id", "claim_id", "event_id"] {
            l.step(1)?;
            if let Some(id) = value.get(key).and_then(Value::as_str) {
                l.bytes(
                    id.len()
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(128))
                        .ok_or(ReferenceError::Budget)?,
                )?;
                replacements.insert(id.to_owned(), format!("{id}.reference-variant"));
            }
        }
        if let Some(maker) = value.pointer("/maker/agent_ref").and_then(Value::as_str) {
            l.bytes(
                maker
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(128))
                    .ok_or(ReferenceError::Budget)?,
            )?;
            replacements.insert(maker.to_owned(), format!("{maker}.reference-variant"));
        }
        if value.get("forms").is_some() {
            for collection in ["forms", "prior_forms"] {
                for form in nav_array(&value, collection)? {
                    l.step(1)?;
                    let id = nav_str(form, "form_id")?;
                    l.bytes(
                        id.len()
                            .checked_mul(2)
                            .and_then(|n| n.checked_add(128))
                            .ok_or(ReferenceError::Budget)?,
                    )?;
                    replacements.insert(id.to_owned(), format!("{id}.reference-variant"));
                }
            }
        }
        if value.get("record_id").is_some() {
            value["preferred_label"] = json!(format!(
                "Independent {} label",
                nav_str(&value, "record_type")?
            ));
        }
        if value.get("claim_id").is_some() {
            l.fixed(2)?;
            value["qualifiers"]["statement"] = json!("Independent source statement");
            value["qualifiers"]["reference_input_extra"] = json!({"retained":"independent"});
            for role in ["name", "caption", "hover"] {
                l.fixed(2)?;
                value["qualifiers"]["display_fields"][role]["text"] =
                    json!(format!("Independent {role} wording"));
            }
        }
        sources.push((source.path, source.line, value));
    }
    l.work
        .diagnostic_stage("rewrite-authored-variant-sources", 0);
    for (_, _, value) in &mut sources {
        nav_rewrite_strings(value, &replacements, l, 0)?;
    }
    // Finish the owned authored mutation BEFORE its first wire/parse. This is
    // the same final source as the former serialize/parse/mutate/serialize path.
    // Final nine members still receive complete strict admission below.
    l.work.diagnostic_stage("append-variant-evidence", 0);
    let second = "ToS/research-packets/reference-input-variant/SECOND.md";
    let mut added_evidence = false;
    for (path, line, value) in &mut sources {
        let mut is_claim = false;
        for declared in &original_claims {
            l.step(1)?;
            l.bytes(
                path.len()
                    .checked_add(declared.path.len())
                    .ok_or(ReferenceError::Budget)?,
            )?;
            if path.as_str() == declared.path.as_str() && *line == declared.line {
                is_claim = true;
            }
        }
        if is_claim {
            l.fixed(1)?;
            let evidence = value["evidence_refs"]
                .as_array_mut()
                .ok_or(ReferenceError::InvalidBinding)?;
            l.bytes(
                evidence
                    .len()
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(128))
                    .and_then(|n| n.checked_add(second.len()))
                    .ok_or(ReferenceError::Budget)?,
            )?;
            evidence.reserve_exact(1);
            evidence.push(json!(second));
            added_evidence = true;
        }
    }
    if !added_evidence {
        return Err(ReferenceError::UnsupportedSourceProfile);
    }
    // Exact new subject references derive from the NEW source records, not from
    // an old digest with a renamed label. Raw file hashes remain a separate domain.
    l.work.diagnostic_stage("variant-subject-digests", 0);
    let mut refs = BTreeMap::new();
    for (_, _, v) in &sources {
        l.step(1)?;
        let (id, version) = if v.get("record_id").is_some() {
            ("record_id", "record_version")
        } else if v.get("claim_id").is_some() {
            ("claim_id", "claim_version")
        } else {
            continue;
        };
        let rid = nav_str(v, id)?;
        l.fixed(3)?;
        refs.insert(
            rid.to_owned(),
            json!({"id":rid,"version":v[version],"digest":format!("sha256:{}",l.digest(v)?)}),
        );
    }
    l.work.diagnostic_stage("variant-form-exact-refs", 0);
    for (_, _, v) in &mut sources {
        l.step(1)?;
        if v.get("forms").is_none() {
            continue;
        }
        let id = nav_str(&v["subject"], "id")?;
        let subject = refs.get(id).ok_or(ReferenceError::InvalidBinding)?;
        v["subject"] = l.copy(subject)?;
        // Prior forms remain explicit prior source assertions. Recompute their
        // canonical exact refs after the synthetic ID rename; do not claim a
        // second schema execution or historical owner mutation.
        let mut prior_refs = BTreeMap::new();
        for prior in nav_array(v, "prior_forms")? {
            l.step(1)?;
            let key = (
                nav_str(prior, "form_id")?.to_owned(),
                prior["form_version"]
                    .as_u64()
                    .ok_or(ReferenceError::InvalidBinding)?,
            );
            prior_refs.insert(key,json!({"id":prior["form_id"],"version":prior["form_version"],"digest":format!("sha256:{}",l.digest(prior)?)}));
        }
        for form in v["forms"]
            .as_array_mut()
            .ok_or(ReferenceError::InvalidBinding)?
        {
            l.step(1)?;
            form["subject"] = l.copy(subject)?;
            for binding in form["bindings"]
                .as_object_mut()
                .ok_or(ReferenceError::InvalidBinding)?
                .values_mut()
            {
                l.step(1)?;
                binding["record"] = l.copy(subject)?;
            }
            if !form["revises"].is_null() {
                let key = (
                    nav_str(&form["revises"], "id")?.to_owned(),
                    form["revises"]["version"]
                        .as_u64()
                        .ok_or(ReferenceError::InvalidBinding)?,
                );
                form["revises"] =
                    l.copy(prior_refs.get(&key).ok_or(ReferenceError::InvalidBinding)?)?;
            }
        }
    }
    l.work
        .diagnostic_stage("serialize-final-variant-members", 0);
    let mut files = BTreeMap::new();
    for (path, line, v) in &sources {
        let name = replacements
            .get(path.as_str())
            .ok_or(ReferenceError::InvalidBinding)?;
        let mut raw = nav_variant_raw(v, l)?;
        if line.is_some() {
            l.bytes(1)?;
            raw.push(b'\n');
        }
        // The containing positive recipe has one nonempty source row per JSONL
        // member. Refuse broader synthetic recipes rather than lose row order.
        if files.insert(name.clone(), raw).is_some() {
            return Err(ReferenceError::UnsupportedSourceProfile);
        }
    }
    l.bytes(second.len() + 256 + 35)?;
    files.insert(
        second.to_owned(),
        b"# Independent second evidence\n".to_vec(),
    );
    l.work
        .diagnostic_stage("variant-evidence-and-schema-inputs", 0);
    // Original evidence remains a separately bound raw input. Subject/form refs
    // above already refer to the final mutated Claim, not an intermediate wire.
    for source in &original_claims {
        for evidence in nav_array(&source.value, "evidence_refs")? {
            l.step(1)?;
            let path = evidence.as_str().ok_or(ReferenceError::InvalidBinding)?;
            let raw = input_files
                .get(path)
                .ok_or(ReferenceError::InvalidBinding)?;
            l.bytes(
                path.len()
                    .checked_add(raw.len())
                    .and_then(|n| n.checked_add(256))
                    .ok_or(ReferenceError::Budget)?,
            )?;
            files.insert(path.to_owned(), raw.clone());
        }
    }
    // Schema dependencies are declared RAW inputs, not inferred from a product
    // schema list. Borrowing registry roots does not remove their input fixity.
    for claim in &original_claims {
        l.step(1)?;
        let (index, _) = input_registries
            .predicate_index
            .get(nav_str(&claim.value, "predicate")?)
            .ok_or(ReferenceError::UnsupportedSourceProfile)?;
        let relation = input_registries.relation(*index, l)?;
        for schema in nav_array(&relation["source_claim_profile"], "schemas")? {
            l.step(1)?;
            if schema["schema_version"] != claim.value["schema_version"] {
                continue;
            }
            for p in std::iter::once(&schema["schema_ref"])
                .chain(nav_array(schema, "schema_dependencies")?.iter())
            {
                l.step(1)?;
                let path = p.as_str().ok_or(ReferenceError::InvalidBinding)?;
                if files.contains_key(path) {
                    continue;
                }
                let raw = input_files
                    .get(path)
                    .ok_or(ReferenceError::InvalidBinding)?;
                l.bytes(
                    path.len()
                        .checked_add(raw.len())
                        .and_then(|n| n.checked_add(256))
                        .ok_or(ReferenceError::Budget)?,
                )?;
                files.insert(path.to_owned(), raw.clone());
            }
        }
    }
    l.work
        .diagnostic_stage("copy-rewrite-historical-navigation", 0);
    let mut old = l.copy(historical)?;
    nav_rewrite_strings(&mut old, &replacements, l, 0)?;
    l.work.diagnostic_stage("parse-final-variant-members", 0);
    let mut variant_inputs = DeclaredNavigationInputs::from_files(&files, input_registries, l)?;
    l.work.diagnostic_stage("build-second-navigation", 0);
    let variant = expected_source_navigation(&old, &variant_inputs, l.reborrow())?;
    assert_eq!(
        variant["counts"]["nodes"].as_u64(),
        original["counts"]["nodes"].as_u64().map(|n| n + 1)
    );
    assert_eq!(
        variant["counts"]["edges"].as_u64(),
        original["counts"]["edges"].as_u64().map(|n| n + 1)
    );
    l.work.diagnostic_stage("second-input-full-assertions", 0);
    for claim in &variant_inputs.claims {
        let id = format!("claim:{}", nav_str(&claim.value, "claim_id")?);
        let rows = nav_array(&variant, "nodes")?;
        l.step(rows.len())?;
        let row = rows
            .iter()
            .find(|row| row["node_id"] == id)
            .ok_or(ReferenceError::InvalidBinding)?;
        assert_eq!(row["source_ref"], claim.path);
        assert_eq!(row["properties"]["source_claim"], claim.value);
        assert_eq!(
            row["properties"]["qualifiers"]["reference_input_extra"],
            json!({"retained":"independent"})
        );
        assert_eq!(
            row["properties"]["navigation_descriptor"]["subject"]["label"],
            "Independent letter label"
        );
        assert_eq!(
            row["properties"]["navigation_descriptor"]["object"]["label"],
            "Independent agent label"
        );
        assert_eq!(
            row["properties"]["human_forms"][0]["display_text"],
            "Independent source statement"
        );
    }
    for old_row in nav_array(historical, "nodes")? {
        l.step(1)?;
        let mut renamed = l.copy(old_row)?;
        nav_rewrite_strings(&mut renamed, &replacements, l, 0)?;
        l.value_bound(&renamed, 0)?;
        assert!(
            nav_array(&variant, "nodes")?
                .iter()
                .any(|row| row == &renamed)
        );
    }
    l.work
        .diagnostic_stage("wrong-pointer-and-profile-controls", 0);
    // Negative source bindings: a valid pointer to the wrong source component
    // and an unsupported derivation MUST refuse, not copy actual materialization.
    let claim = variant_inputs
        .claims
        .first()
        .ok_or(ReferenceError::InvalidBinding)?;
    let id = nav_str(&claim.value, "claim_id")?.to_owned();
    let set = variant_inputs
        .forms
        .get_mut(&id)
        .ok_or(ReferenceError::InvalidBinding)?;
    let pointer = l.copy(&set.value["forms"][0]["bindings"]["wording"]["pointer"])?;
    set.value["forms"][0]["bindings"]["wording"]["pointer"] = json!("/claim_id");
    assert!(matches!(
        nav_forms(claim, &variant_inputs, l),
        Err(ReferenceError::UnsupportedSourceProfile)
    ));
    variant_inputs
        .forms
        .get_mut(&id)
        .ok_or(ReferenceError::InvalidBinding)?
        .value["forms"][0]["bindings"]["wording"]["pointer"] = pointer;
    variant_inputs
        .forms
        .get_mut(&id)
        .ok_or(ReferenceError::InvalidBinding)?
        .value["forms"][0]["content"]["kind"] = json!("unsupported-reference-profile");
    assert!(matches!(
        nav_forms(claim, &variant_inputs, l),
        Err(ReferenceError::UnsupportedSourceProfile)
    ));
    l.work
        .diagnostic_stage("full-field-nested-endpoint-controls", 0);
    // Whole field removal plus nested semantic mutations. A single owned row
    // is reused by MOVE/restore, not six clones of its complete properties.
    let mut descriptor_controls = 0usize;
    let mut form_controls = 0usize;
    for collection in ["nodes", "edges"] {
        for row in nav_array(original, collection)? {
            let fields = row.as_object().ok_or(ReferenceError::InvalidBinding)?;
            let mut changed = l.copy(row)?;
            for key in fields.keys() {
                l.step(1)?;
                let saved = changed
                    .as_object_mut()
                    .ok_or(ReferenceError::InvalidBinding)?
                    .remove(key)
                    .ok_or(ReferenceError::InvalidBinding)?;
                l.compare_work(row, 0)?;
                l.compare_work(&changed, 0)?;
                assert_ne!(&changed, row, "complete {collection} field {key}");
                l.fixed(1)?;
                l.bytes(key.len())?;
                changed
                    .as_object_mut()
                    .ok_or(ReferenceError::InvalidBinding)?
                    .insert(key.clone(), saved);
            }
            for pointer in [
                "/properties/navigation_descriptor/title/default",
                "/properties/navigation_descriptor/statuses/review_status/value",
                "/properties/human_forms/0/context/0/value",
                "/properties/human_forms/0/form/digest",
                "/properties/source_claim/qualifiers/attribution",
                "/properties/bibliographic_carrier/source_sha256",
                "/properties/bibliographic_carrier/source_line",
            ] {
                l.step(1)?;
                if changed.pointer(pointer).is_none() {
                    continue;
                }
                let _ = nav_pointer(&changed, pointer, l)?;
                let saved = std::mem::replace(
                    changed
                        .pointer_mut(pointer)
                        .ok_or(ReferenceError::InvalidBinding)?,
                    Value::Null,
                );
                if saved.is_null() {
                    return Err(ReferenceError::InvalidBinding);
                }
                l.compare_work(row, 0)?;
                l.compare_work(&changed, 0)?;
                assert_ne!(&changed, row, "complete nested field {pointer}");
                *changed
                    .pointer_mut(pointer)
                    .ok_or(ReferenceError::InvalidBinding)? = saved;
                descriptor_controls += usize::from(pointer.contains("navigation_descriptor"));
                form_controls += usize::from(pointer.contains("human_forms"));
            }
            if collection == "edges" && row["from_id"] != row["to_id"] {
                let from = l.copy(&row["from_id"])?;
                changed["from_id"] = l.copy(&row["to_id"])?;
                changed["to_id"] = from;
                l.compare_work(row, 0)?;
                l.compare_work(&changed, 0)?;
                assert_ne!(&changed, row, "existing endpoints swapped");
            }
        }
    }
    assert!(descriptor_controls >= 2 && form_controls >= 2);
    l.work
        .diagnostic_stage("whole-navigation-order-count-controls", 0);
    let mut changed = l.copy(original)?;
    for collection in ["nodes", "edges"] {
        l.step(1)?;
        let rows = changed[collection]
            .as_array_mut()
            .ok_or(ReferenceError::InvalidBinding)?;
        let saved = rows.pop().ok_or(ReferenceError::InvalidBinding)?;
        l.compare_work(original, 0)?;
        l.compare_work(&changed, 0)?;
        assert_ne!(&changed, original, "removed {collection} row");
        changed[collection]
            .as_array_mut()
            .ok_or(ReferenceError::InvalidBinding)?
            .push(saved);
        changed[collection]
            .as_array_mut()
            .ok_or(ReferenceError::InvalidBinding)?
            .swap(0, 1);
        l.compare_work(original, 0)?;
        l.compare_work(&changed, 0)?;
        assert_ne!(&changed, original, "ordered {collection}");
        changed[collection]
            .as_array_mut()
            .ok_or(ReferenceError::InvalidBinding)?
            .swap(0, 1);
    }
    let saved = std::mem::replace(&mut changed["counts"]["nodes"], json!(0));
    l.compare_work(original, 0)?;
    l.compare_work(&changed, 0)?;
    assert_ne!(&changed, original, "independent count");
    changed["counts"]["nodes"] = saved;
    Ok(())
}

fn assert_native_corpus_query_packets(
    selected: &tos_compiler::knowledge_full_fixture::FullKnowledgeFixture,
    projection: &tos_compiler::source_corpus::NativeCorpusProjection,
    expected: &serde_json::Value,
    oracle: &serde_json::Value,
    oracle_root: &std::path::Path,
    output_path: &str,
    budget: tos_query::corpus_read::CorpusReadBudget,
) -> Vec<(
    tos_query::corpus_read::CorpusReadRequest,
    tos_foundation::JsonValue,
)> {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use tos_foundation::{
        CanonicalProfile, Digest256, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
    };
    use tos_query::corpus_read::{
        CORPUS_INTENDED_USE, CorpusReadContext, CorpusReadRequest as R, execute_selected_corpus,
    };
    use tos_query::search_v2::{CurrentPolicyBinding, SearchV2Error, SearchV2ErrorCode};
    use tos_query::{
        IndexedDisclosureScope, InspectCurrentAuthority, InspectDisclosureLease, InspectedCarrier,
        ObservedInspectCarrier, bind_verified_knowledge,
    };
    fn withdrawn() -> SearchV2Error {
        SearchV2Error {
            code: SearchV2ErrorCode::StalePolicy,
            message: "finite projection hold withdrawn",
        }
    }
    struct Hold(Arc<AtomicBool>);
    impl InspectDisclosureLease for Hold {
        fn recheck(&mut self) -> Result<(), SearchV2Error> {
            if self.0.load(Ordering::SeqCst) {
                Err(withdrawn())
            } else {
                Ok(())
            }
        }
    }
    // The same selected original/current-hold law used by the existing finite
    // corpus oracle. Its explicit synthetic policy does not activate main.
    struct ProjectionAuthority {
        policy: CurrentPolicyBinding,
        scope: IndexedDisclosureScope,
        receipt: tos_compiler::CorpusOriginalReceipt,
        rows: Vec<(tos_compiler::CorpusOriginalCollection, u64)>,
        deny: bool,
        withdrawn: Arc<AtomicBool>,
    }
    impl<'hold> InspectCurrentAuthority<'hold> for ProjectionAuthority {
        fn policy_binding(&self) -> CurrentPolicyBinding {
            self.policy.clone()
        }
        fn disclosure_scope(&self) -> IndexedDisclosureScope {
            self.scope.clone()
        }
        fn check_selected(&mut self) -> Result<(), SearchV2Error> {
            if self.withdrawn.load(Ordering::SeqCst) {
                Err(withdrawn())
            } else {
                Ok(())
            }
        }
        fn authorize_current(&mut self, _: &InspectedCarrier) -> Result<(), SearchV2Error> {
            Err(SearchV2Error {
                code: SearchV2ErrorCode::Unavailable,
                message: "corpus oracle grants originals only",
            })
        }
        fn authorize_corpus_original_current(
            &mut self,
            receipt: &tos_compiler::CorpusOriginalReceipt,
            collection: tos_compiler::CorpusOriginalCollection,
            ordinal: u64,
            raw: &[u8],
            sha: Digest256,
        ) -> Result<(), SearchV2Error> {
            self.check_selected()?;
            if self.deny {
                return Err(SearchV2Error {
                    code: SearchV2ErrorCode::Unavailable,
                    message: "finite original grant denied",
                });
            }
            assert_eq!(
                receipt.component_root_sha256,
                self.receipt.component_root_sha256
            );
            assert_eq!(receipt.descriptor_sha256, self.receipt.descriptor_sha256);
            assert_eq!(receipt.source_cut, self.receipt.source_cut);
            assert_eq!(receipt.membership_root, self.receipt.membership_root);
            assert_eq!(Digest256::of_bytes(raw), sha);
            self.rows.push((collection, ordinal));
            Ok(())
        }
        fn acquire_disclosure(
            &mut self,
            scope: &IndexedDisclosureScope,
            consulted: &[ObservedInspectCarrier],
        ) -> Result<Box<dyn InspectDisclosureLease + 'hold>, SearchV2Error> {
            self.check_selected()?;
            assert_eq!(scope.operation_id, self.scope.operation_id);
            assert_eq!(scope.intended_use, CORPUS_INTENDED_USE);
            assert_eq!(
                self.rows.first(),
                Some(&(tos_compiler::CorpusOriginalCollection::Header, 0))
            );
            assert!(consulted.is_empty());
            Ok(Box::new(Hold(self.withdrawn.clone())))
        }
    }
    let mut cold = selected.open().unwrap();
    let receipt = cold.corpus_original_receipt().unwrap().clone();
    assert_eq!(
        receipt.profile,
        tos_compiler::NATIVE_CORPUS_ORIGINAL_PROFILE
    );
    assert_eq!(receipt.origin.profile, "native-corpus-producer-v1");
    assert_eq!(receipt.origin.source_path, output_path);
    assert_eq!(
        receipt.origin.source_sha256,
        Digest256::of_bytes(projection.output_bytes()).to_hex()
    );
    assert_eq!(
        receipt.origin.source_size_bytes,
        u64::try_from(projection.output_bytes().len()).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(receipt.origin.native_producer.as_ref().unwrap()).unwrap(),
        serde_json::to_vec(projection.receipt()).unwrap()
    );
    assert_eq!(projection.value(), expected);
    assert!(
        receipt.origin.source_git_commit.is_none()
            && receipt.origin.source_git_tree.is_none()
            && receipt.origin.capture_manifest_sha256.is_none()
    );
    let bound =
        bind_verified_knowledge(&cold, &selected.vocabulary, &selected.descriptor_bytes).unwrap();
    let captured_root = oracle["tos_root"].as_str().unwrap();
    let captured_index = oracle["index_path"].as_str().unwrap();
    let current_root = oracle_root.to_string_lossy().into_owned();
    let current_index = oracle_root.join(output_path).to_string_lossy().into_owned();
    let mut cases = oracle["cases"].clone();
    fn rebase(
        value: &mut serde_json::Value,
        old_index: &str,
        new_index: &str,
        old_root: &str,
        new_root: &str,
    ) {
        match value {
            serde_json::Value::String(text) => {
                *text = text
                    .replace(old_index, new_index)
                    .replace(old_root, new_root);
            }
            serde_json::Value::Array(values) => {
                for child in values {
                    rebase(child, old_index, new_index, old_root, new_root);
                }
            }
            serde_json::Value::Object(values) => {
                for child in values.values_mut() {
                    rebase(child, old_index, new_index, old_root, new_root);
                }
            }
            _ => {}
        }
    }
    rebase(
        &mut cases,
        captured_index,
        &current_index,
        captured_root,
        &current_root,
    );
    let field = |name| &oracle[name];
    let string = |name| field(name).as_str().unwrap().to_owned();
    let optional = |name| match field(name) {
        serde_json::Value::Null => None,
        value => Some(value.as_str().unwrap().to_owned()),
    };
    let context = CorpusReadContext {
        tos_root: current_root,
        index_path: current_index,
    };
    let authority = |request: &R| {
        let policy = CurrentPolicyBinding {
            scope: "finite-native-corpus-oracle".into(),
            issuer_ref: "synthetic-fixture".into(),
            authorization_receipt_id: "synthetic-fixture".into(),
            policy_epoch: "fixture-epoch".into(),
            withdrawal_generation: "fixture-generation".into(),
        };
        ProjectionAuthority {
            scope: IndexedDisclosureScope {
                operation_id: request.operation_id().into(),
                carrier_layer: "tos_knowledge_public_graph_projection_v1".into(),
                intended_use: CORPUS_INTENDED_USE.into(),
                selected_model_receipt_id: bound.owner_receipt_id().into(),
                source_cut: bound.selection().source_cut.clone(),
                through_commit_seq: bound.selection().through_commit_seq,
                source_membership_root: bound.selection().source_membership_root,
                descriptor_sha256: bound.selection().vocabulary.descriptor_sha256,
                selected_index_sha256: bound.selection().index_root_sha256,
                policy_issuer_ref: policy.issuer_ref.clone(),
                policy_receipt_id: policy.authorization_receipt_id.clone(),
                policy_scope: policy.scope.clone(),
                policy_epoch: policy.policy_epoch.clone(),
                withdrawal_generation: policy.withdrawal_generation.clone(),
            },
            policy,
            receipt: receipt.clone(),
            rows: vec![],
            deny: false,
            withdrawn: Arc::new(AtomicBool::new(false)),
        }
    };
    let view = |view_id: &str| R::GraphView {
        view_id: view_id.into(),
        limit: 1,
    };
    let requests = [
        ("status", R::Status),
        ("summary", R::Summary),
        (
            "search",
            R::Search {
                query: string("query"),
                limit: 2,
                resource_kind: None,
            },
        ),
        (
            "search-filtered",
            R::Search {
                query: String::new(),
                limit: 2,
                resource_kind: optional("resource_kind"),
            },
        ),
        (
            "resources",
            R::Resources {
                resource_kind: optional("resource_kind"),
                owner_branch: optional("owner_branch"),
                limit: 1,
            },
        ),
        (
            "node",
            R::Node {
                node_id: string("node_id"),
            },
        ),
        (
            "endpoint",
            R::Node {
                node_id: string("endpoint_id"),
            },
        ),
        (
            "pack",
            R::RelationPack {
                pack_id: string("pack_id"),
            },
        ),
        ("topology", view("corpus-topology")),
        ("route", view("route-graph")),
        ("promotion", view("promotion-flow")),
        (
            "packet",
            R::Packet {
                query: " ".into(),
                view_id: Some("route-graph".into()),
                limit: 1,
            },
        ),
        (
            "packet-empty",
            R::Packet {
                query: String::new(),
                view_id: Some(String::new()),
                limit: 1,
            },
        ),
    ];
    for (name, request) in &requests {
        let mut current = authority(request);
        let mut packet =
            execute_selected_corpus(&mut cold, &bound, &mut current, &context, request, budget)
                .unwrap();
        let raw = serde_json::to_vec(&cases[name]).unwrap();
        let expected_value = parse_json(&raw, JsonMode::PublishedStrict, budget.inspect.json)
            .unwrap()
            .into_root();
        let expected = canonical_bytes_v1(
            &expected_value,
            CanonicalProfile::SourceRecordDigestV1,
            budget.inspect.json,
        )
        .unwrap();
        assert_eq!(&*packet, expected.as_slice(), "native corpus {name}");
        packet.recheck().unwrap();
    }
    let request = &requests[5].1;
    let mut current = authority(request);
    let mut held =
        execute_selected_corpus(&mut cold, &bound, &mut current, &context, request, budget)
            .unwrap();
    let addressed_rows = u64::try_from(current.rows.len()).unwrap();
    let total_rows = receipt.collections.iter().map(|c| c.rows).sum::<u64>() + 1;
    assert!(
        addressed_rows < total_rows,
        "addressed node must not materialize the whole native corpus"
    );
    let mut addressed = budget;
    addressed.inspect.max_rows = addressed_rows;
    execute_selected_corpus(
        &mut cold,
        &bound,
        &mut authority(request),
        &context,
        request,
        addressed,
    )
    .unwrap()
    .recheck()
    .unwrap();
    addressed.inspect.max_rows = addressed_rows.checked_sub(1).unwrap();
    assert_eq!(
        execute_selected_corpus(
            &mut cold,
            &bound,
            &mut authority(request),
            &context,
            request,
            addressed
        )
        .err()
        .unwrap()
        .code,
        SearchV2ErrorCode::BudgetExceeded
    );
    current.withdrawn.store(true, Ordering::SeqCst);
    assert_eq!(
        held.recheck().unwrap_err().code,
        SearchV2ErrorCode::StalePolicy
    );
    let mut denied = authority(request);
    denied.deny = true;
    assert_eq!(
        execute_selected_corpus(&mut cold, &bound, &mut denied, &context, request, budget)
            .err()
            .unwrap()
            .code,
        SearchV2ErrorCode::Unavailable
    );
    requests
        .into_iter()
        .map(|(name, request)| {
            let raw = serde_json::to_vec(&cases[name]).unwrap();
            let expected = parse_json(&raw, JsonMode::PublishedStrict, budget.inspect.json)
                .unwrap()
                .into_root();
            (request, expected)
        })
        .collect()
}
