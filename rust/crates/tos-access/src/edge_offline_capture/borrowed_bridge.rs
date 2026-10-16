//! Engine transport only. The parent retains its original Python SQLite views.
//! Every SELECT, schema rule and frame byte is owned by the singular encoder.
use super::typed_snapshot::{
    EncodeBudget, ReadStatement, ReadView, Role, encode_view, reserve_schema,
};
use rusqlite::ffi;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    fs::OpenOptions,
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::fs::OpenOptionsExt,
    rc::Rc,
    time::{Duration, Instant},
};
use tos_foundation::{JsonLimits, JsonMode, JsonString, JsonValue, parse_json_with_state_budget};
const WIRE_CAP: usize = 16 * 1024 * 1024;
const CURSORS: usize = 8;
struct DeadlineRead {
    deadline: Instant,
}
impl Read for DeadlineRead {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        loop {
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "borrowed query input deadline",
                ));
            }
            let mut p = libc::pollfd {
                fd: 0,
                events: libc::POLLIN,
                revents: 0,
            };
            let ready =
                unsafe { libc::poll(&mut p, 1, remaining.as_millis().clamp(1, 1000) as i32) };
            if ready < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            if ready == 0 {
                continue;
            }
            let n = unsafe { libc::read(0, output.as_mut_ptr().cast(), output.len()) };
            if n < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            return Ok(n as usize);
        }
    }
}
fn mono_ns() -> Result<u64, String> {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t) } != 0 {
        return Err("borrowed monotonic clock".into());
    }
    (t.tv_sec as u64)
        .checked_mul(1_000_000_000)
        .and_then(|n| n.checked_add(t.tv_nsec as u64))
        .ok_or("borrowed monotonic overflow".into())
}
fn integer<T: std::str::FromStr>(v: &JsonValue) -> Result<T, String> {
    match v {
        JsonValue::Number(n) => n
            .lexeme
            .parse()
            .map_err(|_| "borrowed integer range".into()),
        _ => Err("borrowed integer type".into()),
    }
}
fn unsigned(v: &JsonValue) -> Result<u64, String> {
    integer(v)
}
fn field<'a>(v: &'a JsonValue, k: &str) -> Result<&'a JsonValue, String> {
    v.object_get(k)
        .ok_or_else(|| format!("borrowed missing {k}"))
}
fn text<'a>(v: &'a JsonValue, k: &str) -> Result<&'a str, String> {
    field(v, k)?.as_str().ok_or("borrowed string".into())
}
fn exact(v: &JsonValue, keys: &[&str]) -> Result<(), String> {
    super::super::prepared_publication::exact(v, keys)
}
fn parse(raw: &[u8], state: usize, visits: usize) -> Result<JsonValue, String> {
    let limits = JsonLimits::new(raw.len().max(1), 8, visits.max(32), WIRE_CAP)
        .map_err(|e| e.to_string())?;
    parse_json_with_state_budget(raw, JsonMode::PublishedStrict, limits, state)
        .map(|v| v.into_root())
        .map_err(|e| e.to_string())
}
struct Channel<'a> {
    input: BufReader<DeadlineRead>,
    output: &'a mut dyn Write,
    deadline: Instant,
    meter: Rc<Cell<u64>>,
    io_remaining: u64,
    seq: u64,
    cursors: usize,
    failure: Option<String>,
}
impl Channel<'_> {
    fn io(&mut self, n: usize) -> Result<(), String> {
        self.io_remaining = self
            .io_remaining
            .checked_sub(n as u64)
            .ok_or("borrowed cumulative RPC IO budget")?;
        Ok(())
    }
    fn call(&mut self, request: Value, columns: usize) -> Result<JsonValue, String> {
        if let Some(e) = &self.failure {
            return Err(e.clone());
        }
        if Instant::now() >= self.deadline {
            return Err("borrowed RPC deadline".into());
        }
        self.seq = self
            .seq
            .checked_add(1)
            .ok_or("borrowed sequence overflow")?;
        // Escrow both endpoints before handing any allocation capability to
        // the adapter. A failed/incomplete exchange forfeits the reservation.
        let reply_cap = match request.get("op").and_then(Value::as_str) {
            Some("next") => columns.checked_mul(64).and_then(|n| n.checked_add(4096)),
            Some("blob") => request
                .get("count")
                .and_then(Value::as_u64)
                .and_then(|n| usize::try_from(n).ok())
                .and_then(|n| n.checked_mul(2))
                .and_then(|n| n.checked_add(4096)),
            _ => Some(4096),
        }
        .filter(|n| *n <= WIRE_CAP)
        .ok_or("borrowed reply envelope cap")?;
        let visits = columns
            .checked_mul(3)
            .and_then(|n| n.checked_add(32))
            .ok_or("borrowed reply visit cap")?;
        let slots =
            2 * (std::mem::size_of::<JsonValue>() + std::mem::size_of::<JsonString>() + 32) + 128;
        let fixed = visits
            .checked_mul(slots)
            .and_then(|n| {
                columns
                    .checked_mul(2 * std::mem::size_of::<CellValue>())
                    .and_then(|c| n.checked_add(c))
            })
            .and_then(|n| n.checked_add(8192))
            .ok_or("borrowed reply fixed state")?;
        // Three growth copies, eight wire/tree/string copies; remote slices,
        // fixed ASCII hex and JSON bytes need at most six reply-sized copies.
        let native_grant = reply_cap
            .checked_mul(11)
            .and_then(|n| n.checked_add(fixed))
            .ok_or("borrowed native reply grant")?;
        let adapter_grant = reply_cap
            .checked_mul(6)
            .and_then(|n| columns.checked_mul(512).and_then(|c| n.checked_add(c)))
            .and_then(|n| n.checked_add(8192))
            .ok_or("borrowed adapter reply grant")?;
        let escrow = native_grant
            .checked_add(adapter_grant)
            .ok_or("borrowed RPC escrow overflow")?;
        reserve_schema(&self.meter, escrow as u64)?;
        reserve_schema(&self.meter, 4096)?;
        let packet = json!({"query":request,"seq":self.seq,"adapter_grant":adapter_grant,"reply_cap":reply_cap});
        let raw = serde_json::to_vec(&packet).map_err(|_| "borrowed control encoding")?;
        if raw.len() > WIRE_CAP {
            return Err("borrowed query control cap".into());
        }
        // SELECT strings are prepaid by query(); control slots have a fixed
        // bound. This debit covers retained serialized request copies.
        reserve_schema(&self.meter, (raw.len() * 2 + 256) as u64)?;
        self.io(raw.len() + 1)?;
        self.output
            .write_all(&raw)
            .and_then(|_| self.output.write_all(b"\n"))
            .and_then(|_| self.output.flush())
            .map_err(|_| "borrowed control output")?;
        let mut reply = Vec::new();
        loop {
            let buf = self.input.fill_buf().map_err(|e| e.to_string())?;
            if buf.is_empty() {
                return Err("borrowed incomplete response".into());
            }
            let end = buf.iter().position(|b| *b == b'\n');
            let count = end.unwrap_or(buf.len());
            if count > reply_cap - reply.len() {
                return Err("borrowed response exceeds granted cap".into());
            }
            reply.extend_from_slice(&buf[..count]);
            self.input.consume(count + usize::from(end.is_some()));
            self.io(count + usize::from(end.is_some()))?;
            if end.is_some() {
                break;
            }
        }
        let native_used = reply
            .len()
            .checked_mul(11)
            .and_then(|n| n.checked_add(fixed))
            .ok_or("borrowed reply allocation")?;
        let v = parse(&reply, native_used, visits)?;
        exact(&v, &["seq", "held", "value", "adapter_charge"])?;
        let adapter_used = usize::try_from(unsigned(field(&v, "adapter_charge")?)?)
            .map_err(|_| "borrowed adapter usage width")?;
        if adapter_used > adapter_grant
            || native_used > native_grant
            || unsigned(field(&v, "seq")?)? != self.seq
            || field(&v, "held")? != &JsonValue::Bool(true)
        {
            return Err("borrowed response grant/transaction/sequence differs".into());
        }
        let value = field(&v, "value")?.clone();
        // Return only unused allowance, after the complete valid response;
        // actual logical allocations remain charged cumulatively.
        let unused = escrow - native_used - adapter_used;
        self.meter.set(
            self.meter
                .get()
                .checked_add(unused as u64)
                .ok_or("borrowed unused escrow overflow")?,
        );
        Ok(value)
    }
}
struct View<'a> {
    channel: Rc<RefCell<Channel<'a>>>,
    snapshot: usize,
}
impl ReadView for View<'_> {
    fn paired_row_lengths(&self) -> bool {
        true
    }
    fn transaction_held(&self) -> Result<bool, String> {
        let v = self
            .channel
            .borrow_mut()
            .call(json!({"op":"held","snapshot":self.snapshot}), 0)?;
        Ok(v == JsonValue::Bool(true))
    }
    fn query<'a>(
        &'a self,
        sql: &str,
        _deadline: Instant,
        _cancel: &'a dyn Fn() -> Result<(), String>,
    ) -> Result<Box<dyn ReadStatement + 'a>, String> {
        let mut channel = self.channel.borrow_mut();
        reserve_schema(
            &channel.meter,
            (sql.len() as u64)
                .checked_mul(8)
                .and_then(|n| n.checked_add(512))
                .ok_or("borrowed query allocation overflow")?,
        )?;
        if channel.cursors >= CURSORS {
            return Err("borrowed cursor cap".into());
        }
        let v = channel.call(
            json!({"op":"prepare","snapshot":self.snapshot,"sql":sql}),
            0,
        )?;
        exact(&v, &["cursor", "columns"])?;
        let id = unsigned(field(&v, "cursor")?)?;
        let columns = usize::try_from(unsigned(field(&v, "columns")?)?)
            .map_err(|_| "borrowed columns range")?;
        if columns == 0 || columns > 196606 {
            return Err("borrowed columns cap".into());
        }
        channel.cursors += 1;
        Ok(Box::new(Statement {
            view: self,
            id,
            columns,
            row: Vec::new(),
            raw_cap: 256,
            prepaid: false,
        }))
    }
}
enum CellValue {
    Null,
    Integer(i64),
    Real([u8; 8]),
    Blob(Vec<u8>),
}
struct Statement<'a, 'b> {
    view: &'a View<'b>,
    id: u64,
    columns: usize,
    row: Vec<CellValue>,
    raw_cap: u64,
    prepaid: bool,
}
fn unhex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0
        || !s
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("borrowed cell hex".into());
    }
    let mut v = Vec::with_capacity(s.len() / 2);
    for p in s.as_bytes().chunks_exact(2) {
        let d = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
        v.push((d(p[0]) << 4) | d(p[1]));
    }
    Ok(v)
}
impl ReadStatement for Statement<'_, '_> {
    fn raw_row_cap(&mut self, bytes: u64, prepaid: bool) -> Result<(), String> {
        self.raw_cap = bytes;
        self.prepaid = prepaid;
        Ok(())
    }
    fn step(&mut self) -> Result<bool, String> {
        self.row.clear();
        if !self.prepaid {
            let slots = self
                .columns
                .checked_mul(2 * std::mem::size_of::<CellValue>() + 128)
                .ok_or("borrowed raw row slots")?;
            reserve_schema(
                &self.view.channel.borrow().meter,
                self.raw_cap
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(slots as u64))
                    .ok_or("borrowed raw row preallocation")?,
            )?;
        }
        let v = self.view.channel.borrow_mut().call(
            json!({"op":"next","snapshot":self.view.snapshot,"cursor":self.id,"raw_cap":self.raw_cap}),
            self.columns,
        )?;
        if v == JsonValue::Null {
            return Ok(false);
        }
        let row = v.as_array().ok_or("borrowed row array")?;
        if row.len() != self.columns {
            return Err("borrowed row width changed".into());
        }
        for cell in row {
            let pair = cell.as_array().ok_or("borrowed cell array")?;
            let tag = pair
                .first()
                .and_then(|v| v.as_str())
                .ok_or("borrowed cell tag")?;
            let value = match (tag, pair.len()) {
                ("null", 1) => CellValue::Null,
                ("integer", 2) => CellValue::Integer(integer(&pair[1])?),
                ("real", 2) => CellValue::Real(
                    unhex(pair[1].as_str().ok_or("borrowed real bits")?)?
                        .try_into()
                        .map_err(|_| "borrowed real width")?,
                ),
                ("blob", 2) => {
                    let length =
                        usize::try_from(unsigned(&pair[1])?).map_err(|_| "borrowed blob length")?;
                    if length as u64 > self.raw_cap {
                        return Err("borrowed raw cell exceeds prepaid row cap".into());
                    }
                    let mut bytes = Vec::with_capacity(length);
                    while bytes.len() < length {
                        let count = (length - bytes.len()).min(65536);
                        let chunk=self.view.channel.borrow_mut().call(json!({"op":"blob","snapshot":self.view.snapshot,"cursor":self.id,"column":self.row.len(),"offset":bytes.len(),"count":count}),1)?;
                        let raw = unhex(chunk.as_str().ok_or("borrowed cell chunk")?)?;
                        if raw.len() != count {
                            return Err("borrowed cell chunk length".into());
                        }
                        bytes.extend_from_slice(&raw);
                    }
                    CellValue::Blob(bytes)
                }
                _ => return Err("borrowed storage class transport".into()),
            };
            self.row.push(value);
        }
        Ok(true)
    }
    fn kind(&self, c: i32) -> i32 {
        match self.row.get(c as usize) {
            Some(CellValue::Null) => ffi::SQLITE_NULL,
            Some(CellValue::Integer(_)) => ffi::SQLITE_INTEGER,
            Some(CellValue::Real(_)) => ffi::SQLITE_FLOAT,
            Some(CellValue::Blob(_)) => ffi::SQLITE_BLOB,
            None => -1,
        }
    }
    fn integer(&self, c: i32) -> Result<i64, String> {
        match self.row.get(c as usize) {
            Some(CellValue::Integer(n)) => Ok(*n),
            _ => Err("borrowed integer type".into()),
        }
    }
    fn blob(&self, c: i32) -> Result<Option<&[u8]>, String> {
        match self.row.get(c as usize) {
            Some(CellValue::Null) => Ok(None),
            Some(CellValue::Blob(v)) => Ok(Some(v)),
            _ => Err("borrowed blob type".into()),
        }
    }
    fn real_bits(&self, c: i32) -> Result<[u8; 8], String> {
        match self.row.get(c as usize) {
            Some(CellValue::Real(v)) => Ok(*v),
            _ => Err("borrowed real type".into()),
        }
    }
}
impl Drop for Statement<'_, '_> {
    fn drop(&mut self) {
        let mut c = self.view.channel.borrow_mut();
        if let Err(e) = c.call(
            json!({"op":"close","snapshot":self.view.snapshot,"cursor":self.id}),
            0,
        ) {
            c.failure = Some(e);
        }
        c.cursors = c.cursors.saturating_sub(1);
    }
}
fn body(stdout: &mut dyn Write) -> Result<(), String> {
    // Initial control has a fixed small reserve; remaining clocks come from the
    // parent's original CLOCK_MONOTONIC value, never a newly granted timeout.
    let mut initial = BufReader::with_capacity(
        4096,
        DeadlineRead {
            deadline: Instant::now() + Duration::from_secs(5),
        },
    );
    let raw = super::super::prepared_publication::line(
        &mut initial,
        65536,
        Instant::now() + Duration::from_secs(5),
    )?;
    let v = parse(&raw, 1_048_576, 4096)?;
    exact(
        &v,
        &[
            "schema",
            "snapshots",
            "frame_bytes",
            "schema_bytes",
            "rpc_bytes",
            "work_deadline_ns",
        ],
    )?;
    if text(&v, "schema")? != "tos_edge_borrowed_query_v1" {
        return Err("borrowed bridge profile".into());
    }
    let anchor = Instant::now();
    let remaining = unsigned(field(&v, "work_deadline_ns")?)?
        .checked_sub(mono_ns()?)
        .filter(|n| *n <= 1_200_000_000_000)
        .ok_or("borrowed original work deadline")?;
    let deadline = anchor
        .checked_add(Duration::from_nanos(remaining))
        .ok_or("borrowed deadline overflow")?;
    initial.get_mut().deadline = deadline;
    let schema = unsigned(field(&v, "schema_bytes")?)?;
    let mut budget = EncodeBudget::new(unsigned(field(&v, "frame_bytes")?)?, schema)?;
    let meter = budget.schema_meter();
    reserve_schema(&meter, (1_048_576 + 65536 + 4096 + CURSORS * 256) as u64)?;
    let io_remaining = unsigned(field(&v, "rpc_bytes")?)?
        .checked_sub(raw.len() as u64 + 1)
        .filter(|n| *n > 0)
        .ok_or("borrowed initial IO cap")?;
    let snapshots = field(&v, "snapshots")?
        .as_array()
        .ok_or("borrowed snapshots array")?;
    if snapshots.is_empty() || snapshots.len() > 3 {
        return Err("borrowed snapshot count".into());
    }
    let channel = Rc::new(RefCell::new(Channel {
        input: initial,
        output: stdout,
        deadline,
        meter,
        io_remaining,
        seq: 0,
        cursors: 0,
        failure: None,
    }));
    let mut inventory = Vec::new();
    for (snapshot, spec) in snapshots.iter().enumerate() {
        exact(spec, &["field", "path"])?;
        let input_field = text(spec, "field")?;
        let role = match input_field {
            "d1_database" => Role::D1,
            "before_prepared_database" | "after_prepared_database" => Role::Prepared,
            _ => return Err("borrowed field profile".into()),
        };
        let path = std::path::Path::new(text(spec, "path")?);
        if !path.is_absolute() {
            return Err("borrowed absolute output required".into());
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| "borrowed exclusive frame output")?;
        let view = View {
            channel: channel.clone(),
            snapshot,
        };
        let value = encode_view(
            &view,
            role,
            input_field,
            &mut file,
            &mut budget,
            deadline,
            &|| Ok(()),
        )?;
        inventory.push(value);
    }
    let mut c = channel.borrow_mut();
    if c.failure.is_some() || c.cursors != 0 {
        return Err("borrowed cursor cleanup incomplete".into());
    }
    struct Count {
        bytes: usize,
        deadline: Instant,
    }
    impl Write for Count {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if Instant::now() >= self.deadline || bytes.len() > WIRE_CAP - self.bytes {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "borrowed result cap/deadline",
                ));
            }
            self.bytes += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut result = json!({"result":{"schema":"tos_edge_borrowed_frames_v1","snapshots":inventory,
        "rpc_io_remaining_before_result":c.io_remaining,"schema_remaining":0}});
    let mut count = Count { bytes: 0, deadline };
    serde_json::to_writer(&mut count, &result)
        .map_err(|_| "borrowed result preallocation bound")?;
    // Account encoder serialization plus the parent's result strings/tree,
    // bounded by each inventory entry already priced at 1024 bytes above.
    let result_cap = count.bytes.checked_add(32).ok_or("borrowed result size")?;
    reserve_schema(
        &c.meter,
        (result_cap as u64)
            .checked_mul(8)
            .ok_or("borrowed result allocation")?,
    )?;
    result["result"]["schema_remaining"] = json!(c.meter.get());
    let raw = serde_json::to_vec(&result).map_err(|_| "borrowed result serialization")?;
    if raw.len() > result_cap {
        return Err("borrowed result preallocation changed".into());
    }
    c.io(raw.len() + 1)?;
    c.output
        .write_all(&raw)
        .and_then(|_| c.output.write_all(b"\n"))
        .and_then(|_| c.output.flush())
        .map_err(|_| "borrowed final result")?;
    Ok(())
}
pub(super) fn run(stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match body(stdout) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(stderr, "borrowed Edge query bridge: {e}");
            2
        }
    }
}
