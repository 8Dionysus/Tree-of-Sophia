//! Finite operation budget and descriptor custody shared by research producers.
use fs2::FileExt;
use std::{
    cell::{Cell, RefCell},
    ffi::CString,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    ops::Deref,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
    path::{Component, Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
const FILE_CAP: u64 = 256 * 1024 * 1024;
const READ_CAP: u64 = 2 * 1024 * 1024 * 1024;
const WRITE_CAP: u64 = 1024 * 1024 * 1024;
const WORK_CAP: u64 = 100_000_000;
const STRUCTURAL_READ_ALLOWANCE: u64 = 512 * 1024 * 1024;
const DIRECTORY_RESERVATION: u64 = 64 * 1024;
use tos_source_store::{
    PinnedSqliteAuxLimits, PinnedSqliteAuxRequest, PinnedSqliteAuxScope, PinnedSqliteIoBudget,
    PinnedSqliteSpaceBudget, PinnedSqliteSpaceReservation,
};
fn output_stat(parent: &File, leaf: &CString) -> Result<Option<libc::stat>, String> {
    let mut st = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            leaf.as_ptr(),
            st.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } < 0
    {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENOENT) {
            return Ok(None);
        }
        return Err(error.to_string());
    }
    let st = unsafe { st.assume_init() };
    if st.st_mode & libc::S_IFMT != libc::S_IFREG {
        return Err("research output leaf is not regular".into());
    }
    Ok(Some(st))
}
fn output_fingerprint(st: &libc::stat) -> (u64, u64, i64, i64, i64, i64, i64) {
    (
        st.st_dev,
        st.st_ino,
        st.st_size,
        st.st_mtime,
        st.st_mtime_nsec,
        st.st_ctime,
        st.st_ctime_nsec,
    )
}
pub struct ResearchExecution {
    root: PathBuf,
    directory: File,
    deadline: Instant,
    max_seconds: u64,
    file_cap: u64,
    read_cap: u64,
    work_cap: u64,
    io: PinnedSqliteIoBudget,
    space: Option<PinnedSqliteSpaceBudget>,
    retained_space: Rc<RefCell<Vec<PinnedSqliteSpaceReservation>>>,
    cancelled: Arc<AtomicBool>,
    structural_reserved: Rc<Cell<u64>>,
    structural_returned: Rc<Cell<u64>>,
    work: Rc<Cell<u64>>,
    serial: Rc<Cell<u64>>,
}
impl Deref for ResearchExecution {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.root
    }
}
impl ResearchExecution {
    pub fn new(root: &Path, max_seconds: u64) -> Result<Self, String> {
        Self::selected(root, max_seconds, None)
    }
    /// `available_bytes` is an explicit remaining scratch quota, after the
    /// admitted carrier baseline and other users. This does not grant storage.
    pub fn new_with_scratch(
        root: &Path,
        max_seconds: u64,
        available_bytes: u64,
    ) -> Result<Self, String> {
        Self::selected(root, max_seconds, Some(available_bytes))
    }
    fn selected(
        root: &Path,
        max_seconds: u64,
        available_bytes: Option<u64>,
    ) -> Result<Self, String> {
        Self::selected_profile(
            root,
            max_seconds,
            available_bytes,
            600,
            WORK_CAP,
            Arc::new(AtomicBool::new(false)),
        )
    }
    fn selected_profile(
        root: &Path,
        max_seconds: u64,
        available_bytes: Option<u64>,
        max_window_seconds: u64,
        work_cap: u64,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let started = Instant::now();
        if !(1..=max_window_seconds).contains(&max_seconds) {
            return Err(if max_window_seconds == 600 {
                "research max-seconds must be 1..600".into()
            } else {
                format!("operation max-seconds must be 1..{max_window_seconds}")
            });
        }
        if cancelled.load(Ordering::Relaxed) {
            return Err("research operation cancelled".into());
        }
        let directory = tos_fd_open::open_absolute_directory(root).map_err(|e| e.to_string())?;
        if cancelled.load(Ordering::Relaxed) {
            return Err("research operation cancelled".into());
        }
        Ok(Self {
            root: root.to_owned(),
            directory,
            deadline: started + Duration::from_secs(max_seconds),
            max_seconds,
            file_cap: FILE_CAP,
            read_cap: READ_CAP,
            work_cap,
            io: PinnedSqliteIoBudget::new(READ_CAP, WRITE_CAP).map_err(|e| e.to_string())?,
            space: available_bytes
                .map(PinnedSqliteSpaceBudget::new)
                .transpose()
                .map_err(|e| e.to_string())?,
            retained_space: Rc::new(RefCell::new(Vec::new())),
            cancelled,
            structural_reserved: Rc::new(Cell::new(0)),
            structural_returned: Rc::new(Cell::new(0)),
            work: Rc::new(Cell::new(0)),
            serial: Rc::new(Cell::new(0)),
        })
    }
    /// The maintained Reading v1 input includes a 379,699,200-byte analysis
    /// database. Its original/final hashes, quick checks and selected table
    /// walks exceed the generic 256 MiB/2 GiB envelope. This fixed domain
    /// profile changes neither generic producers nor any caller-selected cap.
    pub fn new_reading_v1(
        root: &Path,
        max_seconds: u64,
        available_bytes: Option<u64>,
    ) -> Result<Self, String> {
        let mut selected = Self::selected(root, max_seconds, available_bytes)?;
        selected.file_cap = 512 * 1024 * 1024;
        selected.read_cap = 4 * 1024 * 1024 * 1024;
        selected.io =
            PinnedSqliteIoBudget::new(selected.read_cap, WRITE_CAP).map_err(|e| e.to_string())?;
        Ok(selected)
    }
    /// Explicit philosophy consumer envelope. The earned f650 whole-authored
    /// producer uses PhilosophySourceLimits::default().max_work_bytes (1 GiB)
    /// and one OPS-selected original window up to 3600 seconds. This keeps
    /// generic and Reading producers at their existing 100M work/600s limits;
    /// selecting another directory cannot renew either allowance. It does not
    /// admit memory, storage, capacity, source meaning, or a runtime result.
    pub fn new_philosophy_products(
        root: &Path,
        max_seconds: u64,
        available_bytes: u64,
    ) -> Result<Self, String> {
        Self::selected_profile(
            root,
            max_seconds,
            Some(available_bytes),
            3600,
            crate::source_philosophy::PhilosophySourceLimits::default().max_work_bytes,
            Arc::new(AtomicBool::new(false)),
        )
    }
    /// Bind the standalone caller's original cancellation flag before any
    /// selected-directory or producer work. The profile and ledgers remain
    /// exactly those of `new_philosophy_products`.
    pub fn new_philosophy_products_with_cancellation(
        root: &Path,
        max_seconds: u64,
        available_bytes: u64,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        Self::selected_profile(
            root,
            max_seconds,
            Some(available_bytes),
            3600,
            crate::source_philosophy::PhilosophySourceLimits::default().max_work_bytes,
            cancelled,
        )
    }
    /// Select another data directory within this same operation. The original
    /// deadline, cancellation, IO, work and scratch ledgers remain shared.
    /// Selecting a directory does not grant storage or extend any allowance.
    pub fn select_directory(&self, root: &Path) -> Result<Self, String> {
        self.check()?;
        let directory = tos_fd_open::open_absolute_directory(root).map_err(|e| e.to_string())?;
        self.check()?;
        Ok(Self {
            root: root.to_owned(),
            directory,
            deadline: self.deadline,
            max_seconds: self.max_seconds,
            file_cap: self.file_cap,
            read_cap: self.read_cap,
            work_cap: self.work_cap,
            io: self.io.clone(),
            space: self.space.clone(),
            retained_space: self.retained_space.clone(),
            cancelled: self.cancelled.clone(),
            structural_reserved: self.structural_reserved.clone(),
            structural_returned: self.structural_returned.clone(),
            work: self.work.clone(),
            serial: self.serial.clone(),
        })
    }
    /// Create a separately selected output directory through the existing
    /// descriptor-only parent walker and shared scratch reservation ledger.
    /// Callers must check source/software separation before requesting this.
    pub fn select_output_directory(&self, root: &Path, create: bool) -> Result<Self, String> {
        if !create {
            return self.select_directory(root);
        }
        let relative = root.strip_prefix("/").map_err(|e| e.to_string())?;
        let reference = relative.to_str().ok_or("output root must be UTF-8")?;
        let anchor = self.select_directory(Path::new("/"))?;
        let (parent, _) = anchor.parent(&format!("{reference}/.research-root-selection"), true)?;
        let selected = self.select_directory(root)?;
        let actual = selected.directory.metadata().map_err(|e| e.to_string())?;
        let expected = parent.metadata().map_err(|e| e.to_string())?;
        if actual.dev() != expected.dev() || actual.ino() != expected.ino() {
            return Err("output directory changed during selection".into());
        }
        self.check()?;
        Ok(selected)
    }
    /// Reserve the upstream owner's full finite allowance before its reads.
    /// Every reconstruction reserves a distinct allowance. No refund: actual
    /// returned bytes are recorded separately afterwards.
    pub fn reserve_structural_reads(&self) -> Result<(), String> {
        self.check()?;
        self.io
            .charge_read(STRUCTURAL_READ_ALLOWANCE)
            .map_err(|e| e.to_string())?;
        self.structural_reserved.set(
            self.structural_reserved
                .get()
                .checked_add(STRUCTURAL_READ_ALLOWANCE)
                .ok_or("structural allowance overflow")?,
        );
        Ok(())
    }
    /// Record actual cumulative structural reads within the reserved allowance.
    pub fn charge_structural(
        &self,
        model: &crate::antonovsky_structural::Model,
        charged: &mut u64,
    ) -> Result<(), String> {
        if model.root_identity()? != self.root_identity()? {
            return Err(
                "structural source root identity differs from selected research root".into(),
            );
        }
        let total = model.source_read_bytes();
        let delta = total
            .checked_sub(*charged)
            .ok_or("structural source read counter regressed")?;
        let returned_total = self
            .structural_returned
            .get()
            .checked_add(delta)
            .ok_or("structural returned byte overflow")?;
        if total > STRUCTURAL_READ_ALLOWANCE || returned_total > self.structural_reserved.get() {
            return Err("structural reads lack their reserved finite allowance".into());
        }
        self.io
            .record_read_returned(delta)
            .map_err(|e| e.to_string())?;
        self.structural_returned.set(returned_total);
        *charged = total;
        Ok(())
    }
    pub fn budget_report(&self) -> serde_json::Value {
        let io = self.io.snapshot();
        let physical = self.space.as_ref().map(|space| {
            let s = space.snapshot();
            serde_json::json!({"declared_available_bytes":s.declared_available_bytes,"reserved_current_bytes":s.reserved_current_bytes,"reserved_high_water_bytes":s.reserved_high_water_bytes,"actual_observed_current_bytes":s.actual_observed_current_bytes,"actual_observed_high_water_bytes":s.actual_observed_high_water_bytes,"allocation_anomalies":s.allocation_anomalies,"ledger_consistent":s.ledger_consistent,"is_storage_grant":false})
        });
        serde_json::json!({"whole_operation_seconds":self.max_seconds,"file_bytes_max":self.file_cap,"file_bytes_max_scope":"source and read-only artifact inputs","output_file_bytes_max":FILE_CAP,"logical_source_and_sqlite_read_bytes_max":self.read_cap,"logical_source_and_sqlite_write_bytes_max":WRITE_CAP,"io_counter_scope":"Rust source/hash/import/entropy and SQLite pager requests; reserved upstream Structural Rust reads","io_counter_exclusions":["separately bounded native-child protocol and internal reads","bounded helper control metadata/proc reads","filesystem metadata and host verification"],"work_units_max":self.work_cap,"charged_work_units":self.work.get(),"read_attempted_bytes":io.read_attempted_bytes,"read_permitted_bytes":io.read_permitted_bytes,"read_returned_bytes":io.read_returned_bytes,"write_attempted_bytes":io.write_attempted_bytes,"write_permitted_bytes":io.write_permitted_bytes,"write_returned_bytes":io.write_returned_bytes,"io_failure":io.failure.map(|failure|format!("{failure:?}")),"structural_reserved_read_allowance":self.structural_reserved.get(),"structural_actual_returned_read_bytes":self.structural_returned.get(),"physical_scratch":physical})
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn root_directory(&self) -> &File {
        &self.directory
    }
    pub fn root_identity(&self) -> Result<(u64, u64), String> {
        self.check()?;
        let meta = self.directory.metadata().map_err(|e| e.to_string())?;
        Ok((meta.dev(), meta.ino()))
    }
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
    pub fn cancellation_flag(&self) -> &AtomicBool {
        self.cancelled.as_ref()
    }
    pub fn check(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err("research operation cancelled".into())
        } else if Instant::now() >= self.deadline {
            Err("research operation deadline exceeded".into())
        } else {
            Ok(())
        }
    }
    fn charge(&self, cell: &Cell<u64>, n: u64, cap: u64, label: &str) -> Result<(), String> {
        self.check()?;
        let v = cell
            .get()
            .checked_add(n)
            .ok_or("research budget overflow")?;
        if v > cap {
            return Err(format!("research {label} budget exceeded"));
        }
        cell.set(v);
        Ok(())
    }
    pub fn tick(&self, n: u64) -> Result<(), String> {
        self.charge(&self.work, n, self.work_cap, "work")
    }
    fn reserve_space(&self, bytes: u64) -> Result<PinnedSqliteSpaceReservation, String> {
        self.check()?;
        self.space
            .as_ref()
            .ok_or("research writes require an explicit reserved --scratch-bytes quota")?
            .reserve(bytes)
            .map_err(|e| e.to_string())
    }
    fn observed_space_with_parent(
        reservation: &PinnedSqliteSpaceReservation,
        file: &File,
        parent: Option<(&File, u64)>,
    ) -> Result<(), String> {
        let mut bytes = file
            .metadata()
            .map_err(|e| e.to_string())?
            .blocks()
            .checked_mul(512)
            .ok_or("allocated byte overflow")?;
        if let Some((directory, before_blocks)) = parent {
            let delta = directory
                .metadata()
                .map_err(|e| e.to_string())?
                .blocks()
                .saturating_sub(before_blocks)
                .checked_mul(512)
                .ok_or("directory allocated byte overflow")?;
            bytes = bytes.checked_add(delta).ok_or("allocated byte overflow")?;
        }
        reservation
            .update_actual_allocated(bytes)
            .map_err(|e| e.to_string())
    }
    /// Charge each permitted request before the syscall, retaining partial/error attempts.
    pub fn read_exact(&self, file: &mut File, output: &mut [u8]) -> Result<(), String> {
        let mut done = 0;
        while done < output.len() {
            self.check()?;
            self.io
                .charge_read((output.len() - done) as u64)
                .map_err(|e| e.to_string())?;
            match file.read(&mut output[done..]) {
                Ok(0) => return Err("research exact read reached EOF".into()),
                Ok(n) => {
                    self.io
                        .record_read_returned(n as u64)
                        .map_err(|e| e.to_string())?;
                    done += n;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        self.check()
    }
    pub fn read_file(&self, file: &mut File, max_bytes: u64) -> Result<Vec<u8>, String> {
        let cap = max_bytes.min(self.file_cap);
        let mut output = Vec::new();
        let mut chunk = [0u8; 65536];
        loop {
            self.check()?;
            let request = (cap.saturating_sub(output.len() as u64).saturating_add(1))
                .min(chunk.len() as u64) as usize;
            self.io
                .charge_read(request as u64)
                .map_err(|e| e.to_string())?;
            let count = match file.read(&mut chunk[..request]) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.to_string()),
            };
            self.io
                .record_read_returned(count as u64)
                .map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            if output.len() as u64 + count as u64 > cap {
                return Err("research file grew beyond cap".into());
            }
            output.try_reserve(count).map_err(|e| e.to_string())?;
            output.extend_from_slice(&chunk[..count]);
        }
        self.check()?;
        Ok(output)
    }
    fn parts(reference: &str) -> Result<Vec<&std::ffi::OsStr>, String> {
        let p = Path::new(reference);
        if reference.len() > 4096 || p.is_absolute() {
            return Err("invalid research relative reference".into());
        }
        let mut out = Vec::new();
        for c in p.components() {
            match c {
                Component::Normal(s) => out.push(s),
                _ => return Err("invalid research relative reference".into()),
            }
        }
        if out.is_empty() || out.len() > 64 {
            return Err("invalid research reference depth".into());
        }
        Ok(out)
    }
    fn parent(&self, reference: &str, create: bool) -> Result<(File, CString), String> {
        self.check()?;
        let parts = Self::parts(reference)?;
        let mut parent =
            tos_fd_open::reopen_directory(&self.directory).map_err(|e| e.to_string())?;
        for part in &parts[..parts.len() - 1] {
            self.tick(1)?;
            match tos_fd_open::open_directory_at(&parent, Path::new(part)) {
                Ok(next) => parent = next,
                Err(e) => {
                    if !create
                        || e.source.as_ref().and_then(|x| x.raw_os_error()) != Some(libc::ENOENT)
                    {
                        return Err(e.to_string());
                    }
                    let leaf = CString::new(part.as_encoded_bytes()).map_err(|e| e.to_string())?;
                    let reservation = self.reserve_space(DIRECTORY_RESERVATION)?;
                    let parent_blocks_before =
                        parent.metadata().map_err(|e| e.to_string())?.blocks();
                    let created =
                        unsafe { libc::mkdirat(parent.as_raw_fd(), leaf.as_ptr(), 0o700) } == 0;
                    if !created
                        && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST)
                    {
                        return Err(std::io::Error::last_os_error().to_string());
                    }
                    if created {
                        self.retained_space.borrow_mut().push(reservation);
                    }
                    let next = tos_fd_open::open_directory_at(&parent, Path::new(part))
                        .map_err(|e| e.to_string())?;
                    if created {
                        let leases = self.retained_space.borrow();
                        Self::observed_space_with_parent(
                            leases.last().ok_or("directory lease missing")?,
                            &next,
                            Some((&parent, parent_blocks_before)),
                        )?;
                    }
                    parent = next;
                }
            }
        }
        Ok((
            parent,
            CString::new(parts.last().unwrap().as_encoded_bytes()).map_err(|e| e.to_string())?,
        ))
    }
    pub fn output_parent(&self, reference: &str) -> Result<(File, CString), String> {
        self.parent(reference, true)
    }
    pub fn source_file(&self, reference: &str, max_bytes: u64) -> Result<File, String> {
        let (parent, leaf) = self.parent(reference, false)?;
        let file = tos_fd_open::open_regular_at(
            &parent,
            Path::new(std::ffi::OsStr::from_bytes(leaf.as_bytes())),
        )
        .map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > max_bytes.min(self.file_cap) {
            return Err("research source file cap exceeded".into());
        }
        Ok(file)
    }
    pub fn verify_file_unchanged(
        &self,
        file: &File,
        before: &std::fs::Metadata,
    ) -> Result<(), String> {
        self.check()?;
        let now = file.metadata().map_err(|e| e.to_string())?;
        let fingerprint = |m: &std::fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        };
        if fingerprint(before) != fingerprint(&now) {
            return Err("research selected file changed during use".into());
        }
        Ok(())
    }
    pub fn hash_file(&self, file: &mut File, max_bytes: u64) -> Result<String, String> {
        self.check()?;
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let cap = max_bytes.min(self.file_cap);
        let mut total = 0;
        let mut chunk = [0u8; 65536];
        let mut hash = tos_foundation::Digest256Hasher::new();
        loop {
            self.check()?;
            let request = (cap - total + 1).min(chunk.len() as u64) as usize;
            self.io
                .charge_read(request as u64)
                .map_err(|e| e.to_string())?;
            let count = match file.read(&mut chunk[..request]) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.to_string()),
            };
            self.io
                .record_read_returned(count as u64)
                .map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            total += count as u64;
            if total > cap {
                return Err("research hash file cap exceeded".into());
            }
            hash.update(&chunk[..count]);
        }
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        self.check()?;
        Ok(hash.finalize().to_hex())
    }
    pub fn read(&self, reference: &str) -> Result<Vec<u8>, String> {
        let mut file = self.source_file(reference, self.file_cap)?;
        self.read_file(&mut file, self.file_cap)
    }
    pub fn open_sqlite_readonly(
        &self,
        file: &File,
    ) -> Result<tos_source_store::PinnedSqliteConnection, String> {
        self.check()?;
        tos_source_store::PinnedSqliteConnection::open_readonly_immutable_budgeted(
            file,
            self.io.clone(),
            self.deadline,
            self.cancelled.clone(),
        )
        .map_err(|e| e.to_string())
    }
    /// Ordered immutable scans may need a sorter. The exact-FD VFS refuses
    /// filesystem temporary names; the operation's finite memory envelope
    /// owns the sorter. Keep this an explicit profile of the existing reader.
    pub fn open_sqlite_readonly_for_ordered_scan(
        &self,
        file: &File,
    ) -> Result<tos_source_store::PinnedSqliteConnection, String> {
        let db = self.open_sqlite_readonly(file)?;
        let deadline = self.deadline();
        let cancelled = self.cancelled.clone();
        db.progress_handler(
            1000,
            Some(move || {
                Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed)
            }),
        );
        db.execute_batch("PRAGMA temp_store=MEMORY; PRAGMA mmap_size=0")
            .map_err(|e| format!("immutable ordered scan storage profile: {e}"))?;
        let temp_store: i64 = db
            .query_row("PRAGMA temp_store", [], |row| row.get(0))
            .map_err(|e| format!("immutable sorter profile check: {e}"))?;
        if temp_store != 2 {
            return Err("immutable ordered scan requires memory temp storage".into());
        }
        self.check()?;
        Ok(db)
    }
    pub fn sqlite_scope(
        &self,
        limits: PinnedSqliteAuxLimits,
    ) -> Result<ResearchSqliteScope<'_>, String> {
        self.check()?;
        let space = self
            .space
            .clone()
            .ok_or("SQLite producers require an explicit reserved --scratch-bytes quota")?;
        let reservation = self.reserve_space(DIRECTORY_RESERVATION)?;
        let parent = tos_fd_open::reopen_directory(&self.directory).map_err(|e| e.to_string())?;
        let serial = self.serial.get();
        self.serial
            .set(serial.checked_add(1).ok_or("scratch serial overflow")?);
        let name = CString::new(format!(".research-sqlite-{}-{serial}", std::process::id()))
            .map_err(|e| e.to_string())?;
        let parent_blocks_before = parent.metadata().map_err(|e| e.to_string())?.blocks();
        if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let directory = match tos_fd_open::open_directory_at(
            &parent,
            Path::new(std::ffi::OsStr::from_bytes(name.as_bytes())),
        ) {
            Ok(dir) => dir,
            Err(error) => {
                self.retained_space.borrow_mut().push(reservation);
                return Err(format!(
                    "{error}; created scratch directory requires owner cleanup"
                ));
            }
        };
        let metadata = match directory.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                self.retained_space.borrow_mut().push(reservation);
                return Err(format!(
                    "{error}; created scratch directory requires owner cleanup"
                ));
            }
        };
        let mut guard = ResearchSqliteScope {
            context: self,
            parent,
            name: Some(name),
            identity: (metadata.dev(), metadata.ino()),
            parent_blocks_before,
            scope: None,
            directory_reservation: Some(reservation),
        };
        let setup = (|| {
            if unsafe { libc::fchmod(directory.as_raw_fd(), 0o700) } < 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Self::observed_space_with_parent(
                guard
                    .directory_reservation
                    .as_ref()
                    .ok_or("scratch directory lease missing")?,
                &directory,
                Some((&guard.parent, guard.parent_blocks_before)),
            )?;
            self.check()?;
            PinnedSqliteAuxScope::new(
                directory,
                PinnedSqliteAuxRequest {
                    limits,
                    io_budget: self.io.clone(),
                    space_budget: space,
                    deadline: self.deadline,
                    cancelled: self.cancelled.clone(),
                },
            )
            .map_err(|e| e.to_string())
        })();
        match setup {
            Ok(scope) => {
                guard.scope = Some(scope);
                Ok(guard)
            }
            Err(error) => {
                let cleanup = guard.cleanup();
                Err(combine_failure(error, cleanup))
            }
        }
    }
    pub fn write(
        &self,
        reference: &str,
        payload: &[u8],
        mode: u32,
        exclusive: bool,
    ) -> Result<(), String> {
        self.write_captured(reference, payload, mode, exclusive, None)
    }
    /// Replace a retained journal only if its exact read bytes are still current
    /// under the same parent lock used by every research writer.
    pub fn write_replacing_exact(
        &self,
        reference: &str,
        payload: &[u8],
        mode: u32,
        expected: &[u8],
    ) -> Result<(), String> {
        self.write_captured(reference, payload, mode, false, Some(expected))
    }
    fn write_captured(
        &self,
        reference: &str,
        payload: &[u8],
        mode: u32,
        exclusive: bool,
        expected: Option<&[u8]>,
    ) -> Result<(), String> {
        self.check()?;
        if payload.len() as u64 > FILE_CAP {
            return Err("research output file cap exceeded".into());
        }
        let (parent, leaf) = self.output_parent(reference)?;
        parent
            .try_lock_exclusive()
            .map_err(|e| format!("research output parent busy: {e}"))?;
        let parent_blocks_before = parent.metadata().map_err(|e| e.to_string())?.blocks();
        let initial = output_stat(&parent, &leaf)?;
        let present = initial.is_some();
        if exclusive && present {
            return Err("research output already exists".into());
        }
        if let Some(expected) = expected {
            if !present || self.read(reference)? != expected {
                return Err("research journal changed since its captured read".into());
            }
        }
        let serial = self.serial.get();
        self.serial.set(serial + 1);
        let stage = CString::new(format!(
            ".research-{}-{}-{serial}.tmp",
            std::process::id(),
            self.deadline.elapsed().as_nanos()
        ))
        .map_err(|e| e.to_string())?;
        let mut reservation = Some(
            self.reserve_space(
                (payload.len() as u64)
                    .checked_add(65535)
                    .ok_or("stage size overflow")?
                    / 65536
                    * 65536
                    + 65536,
            )?,
        );
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                stage.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                mode,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        let result = (|| {
            for chunk in payload.chunks(65536) {
                let mut done = 0;
                while done < chunk.len() {
                    self.check()?;
                    self.io
                        .charge_write((chunk.len() - done) as u64)
                        .map_err(|e| e.to_string())?;
                    match file.write(&chunk[done..]) {
                        Ok(0) => return Err("research stage zero write".into()),
                        Ok(n) => {
                            self.io
                                .record_write_returned(n as u64)
                                .map_err(|e| e.to_string())?;
                            done += n;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(e) => return Err(e.to_string()),
                    }
                    Self::observed_space_with_parent(
                        reservation.as_ref().ok_or("stage lease missing")?,
                        &file,
                        Some((&parent, parent_blocks_before)),
                    )?;
                }
            }
            if unsafe { libc::fchmod(file.as_raw_fd(), mode) } < 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            file.sync_all().map_err(|e| e.to_string())?;
            Self::observed_space_with_parent(
                reservation.as_ref().ok_or("stage lease missing")?,
                &file,
                Some((&parent, parent_blocks_before)),
            )?;
            self.check()?;
            let current = output_stat(&parent, &leaf)?;
            if initial.as_ref().map(output_fingerprint) != current.as_ref().map(output_fingerprint)
            {
                return Err("research output changed before commit".into());
            }
            let rc = if exclusive || !present {
                unsafe {
                    libc::linkat(
                        parent.as_raw_fd(),
                        stage.as_ptr(),
                        parent.as_raw_fd(),
                        leaf.as_ptr(),
                        0,
                    )
                }
            } else {
                unsafe {
                    libc::renameat(
                        parent.as_raw_fd(),
                        stage.as_ptr(),
                        parent.as_raw_fd(),
                        leaf.as_ptr(),
                    )
                }
            };
            if rc < 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let observation = Self::observed_space_with_parent(
                reservation.as_ref().ok_or("stage lease missing")?,
                &file,
                Some((&parent, parent_blocks_before)),
            );
            self.retained_space
                .borrow_mut()
                .push(reservation.take().ok_or("stage lease missing")?);
            observation?;
            parent.sync_all().map_err(|e| e.to_string())?;
            let retained = self.retained_space.borrow();
            Self::observed_space_with_parent(
                retained.last().ok_or("published lease missing")?,
                &file,
                Some((&parent, parent_blocks_before)),
            )?;
            Ok(())
        })();
        let rc = unsafe { libc::unlinkat(parent.as_raw_fd(), stage.as_ptr(), 0) };
        if rc < 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::ENOENT) {
            if let Some(lease) = reservation.take() {
                self.retained_space.borrow_mut().push(lease);
            }
            return Err(match result {
                Ok(()) => "research stage cleanup failed".into(),
                Err(error) => format!("{error}; research stage cleanup failed"),
            });
        }
        drop(file);
        drop(reservation);
        if result.is_ok() {
            self.check()?;
        }
        result
    }
}
fn combine_failure(primary: String, cleanup: Result<(), String>) -> String {
    match cleanup {
        Ok(()) => primary,
        Err(error) => format!("{primary}; scratch cleanup: {error}"),
    }
}
/// Owns only one exclusive private scratch child and the shared backend scope.
pub struct ResearchSqliteScope<'a> {
    context: &'a ResearchExecution,
    parent: File,
    name: Option<CString>,
    identity: (u64, u64),
    parent_blocks_before: u64,
    scope: Option<PinnedSqliteAuxScope>,
    directory_reservation: Option<PinnedSqliteSpaceReservation>,
}
impl ResearchSqliteScope<'_> {
    pub fn scope_mut(&mut self) -> &mut PinnedSqliteAuxScope {
        self.scope.as_mut().expect("live owned SQLite scope")
    }
    fn cleanup(&mut self) -> Result<(), String> {
        drop(self.scope.take());
        let Some(name) = self.name.take() else {
            return Ok(());
        };
        let mut st = std::mem::MaybeUninit::<libc::stat>::uninit();
        let mut retained_parent_bytes = 0;
        let result = (|| {
            if unsafe {
                libc::fstatat(
                    self.parent.as_raw_fd(),
                    name.as_ptr(),
                    st.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } < 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let st = unsafe { st.assume_init() };
            if st.st_mode & libc::S_IFMT != libc::S_IFDIR || (st.st_dev, st.st_ino) != self.identity
            {
                return Err("owned scratch directory identity changed".into());
            }
            if unsafe { libc::unlinkat(self.parent.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) }
                < 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            retained_parent_bytes = self
                .parent
                .metadata()
                .map_err(|e| e.to_string())?
                .blocks()
                .saturating_sub(self.parent_blocks_before)
                .checked_mul(512)
                .ok_or("directory allocated byte overflow")?;
            self.directory_reservation
                .as_ref()
                .ok_or("scratch directory lease missing")?
                .update_actual_allocated(retained_parent_bytes)
                .map_err(|e| e.to_string())?;
            self.parent.sync_all().map_err(|e| e.to_string())?;
            // Sync may change allocation; observe it before a successful return.
            retained_parent_bytes = self
                .parent
                .metadata()
                .map_err(|e| e.to_string())?
                .blocks()
                .saturating_sub(self.parent_blocks_before)
                .checked_mul(512)
                .ok_or("directory allocated byte overflow")?;
            self.directory_reservation
                .as_ref()
                .ok_or("scratch directory lease missing")?
                .update_actual_allocated(retained_parent_bytes)
                .map_err(|e| e.to_string())?;
            self.context.check()
        })();
        // Removal releases the child, but retained parent growth remains charged.
        // On refusal retain a conservative reservation through the operation.
        if result.is_err() || retained_parent_bytes > 0 {
            if let Some(lease) = self.directory_reservation.take() {
                self.context.retained_space.borrow_mut().push(lease);
            }
        } else {
            drop(self.directory_reservation.take());
        }
        result
    }
    pub fn complete(
        mut self,
        recipe: Result<(), String>,
        max_bytes: u64,
    ) -> Result<Vec<u8>, String> {
        let result = recipe.and_then(|()| {
            self.scope_mut()
                .read_main_bytes(max_bytes)
                .map_err(|e| e.to_string())
        });
        let cleanup = self.cleanup();
        match result {
            Ok(bytes) => {
                cleanup?;
                Ok(bytes)
            }
            Err(error) => Err(combine_failure(error, cleanup)),
        }
    }
}
impl Drop for ResearchSqliteScope<'_> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
use std::os::unix::ffi::OsStrExt;
#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::symlink};
    #[test]
    fn ordered_immutable_scan_sorts_within_memory_and_preserves_source() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.sqlite3");
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE source(k INTEGER, v TEXT); INSERT INTO source VALUES (3,'third'),(1,'first'),(2,'second');").unwrap();
        db.close().unwrap();
        let before = fs::read(&path).unwrap();
        let ctx = ResearchExecution::new(temp.path(), 10).unwrap();
        let file = ctx.source_file("source.sqlite3", 1024 * 1024).unwrap();
        let read = ctx.open_sqlite_readonly_for_ordered_scan(&file).unwrap();
        let policy: i64 = read
            .query_row("PRAGMA temp_store", [], |r| r.get(0))
            .unwrap();
        assert_eq!(policy, 2);
        let actual: Vec<String> = read
            .prepare("SELECT v FROM source ORDER BY k")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(actual, vec!["first", "second", "third"]);
        assert!(
            read.execute("INSERT INTO source VALUES (4,'no')", [])
                .is_err()
        );
        read.close().unwrap();
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
        assert!(ctx.budget_report()["read_returned_bytes"].as_u64().unwrap() > 0);
    }
    #[test]
    fn journal_replacement_refuses_a_stale_read_without_losing_history() {
        let root = tempfile::tempdir().unwrap();
        let a = ResearchExecution::new_with_scratch(root.path(), 10, 1024 * 1024).unwrap();
        let b = ResearchExecution::new_with_scratch(root.path(), 10, 1024 * 1024).unwrap();
        a.write("journal", b"first\n", 0o644, true).unwrap();
        let captured = a.read("journal").unwrap();
        b.write_replacing_exact("journal", b"first\nsecond\n", 0o644, &captured)
            .unwrap();
        assert!(
            a.write_replacing_exact("journal", b"first\nlost\n", 0o644, &captured)
                .is_err()
        );
        assert_eq!(a.read("journal").unwrap(), b"first\nsecond\n");
        assert!(
            a.write_replacing_exact("absent", b"unexpected", 0o644, b"")
                .is_err()
        );
    }
    #[test]
    fn separate_output_directory_keeps_the_original_operation_ledgers() {
        let source = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let operation =
            ResearchExecution::new_with_scratch(source.path(), 10, 1024 * 1024).unwrap();
        let selected = operation.select_directory(output.path()).unwrap();
        assert_eq!(selected.deadline(), operation.deadline());
        assert!(std::ptr::eq(
            selected.cancellation_flag(),
            operation.cancellation_flag()
        ));
        operation.tick(5).unwrap();
        selected.tick(7).unwrap();
        assert_eq!(operation.budget_report()["charged_work_units"], 12);
        selected.tick(WORK_CAP - 12).unwrap();
        assert!(operation.tick(1).is_err());
        assert_eq!(operation.root(), source.path());
        assert_eq!(selected.root(), output.path());
    }
    #[test]
    fn finite_context_guards_descriptor_reads_and_exclusive_writes() {
        let t = tempfile::tempdir().unwrap();
        let c = ResearchExecution::new_with_scratch(t.path(), 1, 4 * 1024 * 1024).unwrap();
        c.write("private/a", b"exact", 0o600, true).unwrap();
        assert_eq!(c.read("private/a").unwrap(), b"exact");
        assert!(c.write("private/a", b"other", 0o600, true).is_err());
        symlink("a", t.path().join("private/link")).unwrap();
        assert!(c.read("private/link").is_err());
        assert!(c.write("private/link", b"escape", 0o600, false).is_err());
        assert_eq!(c.read("private/a").unwrap(), b"exact");
        assert!(ResearchExecution::new(t.path(), 0).is_err());
        assert!(ResearchExecution::new(t.path(), 601).is_err());
        assert!(c.read("../escape").is_err());
        assert!(c.tick(WORK_CAP + 1).is_err());
        fs::rename(t.path().join("private"), t.path().join("held")).unwrap();
    }
    #[test]
    fn absent_or_exhausted_physical_quota_cannot_create_output() {
        let temp = tempfile::tempdir().unwrap();
        let without_quota = ResearchExecution::new(temp.path(), 180).unwrap();
        assert!(
            without_quota
                .write("output", b"bytes", 0o600, true)
                .is_err()
        );
        let too_small = ResearchExecution::new_with_scratch(temp.path(), 180, 1).unwrap();
        assert!(too_small.write("output", b"bytes", 0o600, true).is_err());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
        assert!(ResearchExecution::new_with_scratch(temp.path(), 180, 0).is_err());
        assert!(ResearchExecution::new_with_scratch(temp.path(), 180, u64::MAX).is_err());
    }
    #[test]
    fn failed_exact_read_keeps_attempts_and_partial_returned_bytes() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("partial"), b"abc").unwrap();
        let ctx = ResearchExecution::new(temp.path(), 180).unwrap();
        let mut file = ctx.source_file("partial", 1024).unwrap();
        let mut output = [0u8; 6];
        assert!(ctx.read_exact(&mut file, &mut output).is_err());
        let io = ctx.io.snapshot();
        assert_eq!(io.read_returned_bytes, 3);
        assert_eq!(io.read_attempted_bytes, 9);
        assert_eq!(io.read_permitted_bytes, 9);
    }
    #[test]
    fn held_hash_binds_open_inode_after_namespace_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new(temp.path(), 180).unwrap();
        fs::write(temp.path().join("database"), b"selected bytes").unwrap();
        let mut held = ctx.source_file("database", 1024).unwrap();
        fs::write(temp.path().join("replacement"), b"different bytes").unwrap();
        fs::rename(
            temp.path().join("replacement"),
            temp.path().join("database"),
        )
        .unwrap();
        assert_eq!(
            ctx.hash_file(&mut held, 1024).unwrap(),
            tos_foundation::Digest256::of_bytes(b"selected bytes").to_hex()
        );
        assert_eq!(ctx.read("database").unwrap(), b"different bytes");
    }
}
