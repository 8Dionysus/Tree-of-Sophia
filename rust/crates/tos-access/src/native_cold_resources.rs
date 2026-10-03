//! Actual Linux cgroup-v2 RAM custody for the selected native cold opener.
//! Stage tickets and nominal process limits do not replace these kernel facts.
use std::{
    fs::File,
    io::Read,
    os::fd::{AsRawFd, FromRawFd},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use tos_compiler::native_snapshot::NativeColdOpenResourceHold;
use tos_compiler::{ColdOpenLimits, Error, NativeProcessLimits, Result};

const PATH_BYTES: usize = 8193;
const COPY_BUFFER_BYTES: u64 = 64 * 1024;

pub struct LinuxCgroupColdOpenResourceHold {
    directory: File,
    membership: Vec<u8>,
    device: libc::dev_t,
    inode: libc::ino_t,
    memory_max: u64,
    working_ram_bytes: u64,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}

impl LinuxCgroupColdOpenResourceHold {
    /// Acquire from the current kernel membership, never a caller path or JSON proof.
    pub fn acquire(
        working_ram_bytes: u64,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self> {
        active(deadline, cancelled.as_ref())?;
        if working_ram_bytes == 0 {
            return Err(Error::Budget("native cold RAM allowance"));
        }
        let membership = membership(deadline, cancelled.as_ref())?;
        let directory = open_cgroup(&membership, deadline, cancelled.as_ref())?;
        let identity = identity(&directory)?;
        let memory_max = scalar(&directory, b"memory.max\0", deadline, cancelled.as_ref())?;
        if memory_max == 0
            || memory_max > working_ram_bytes
            || scalar(
                &directory,
                b"memory.swap.max\0",
                deadline,
                cancelled.as_ref(),
            )? != 0
        {
            return Err(Error::Budget("native cold kernel RAM or swap limit"));
        }
        let hold = Self {
            directory,
            membership,
            device: identity.st_dev,
            inode: identity.st_ino,
            memory_max,
            working_ram_bytes,
            deadline,
            cancelled,
        };
        hold.check_current(deadline, hold.cancelled.as_ref())?;
        Ok(hold)
    }

    pub(crate) fn check_current(&self, deadline: Instant, cancelled: &AtomicBool) -> Result<u64> {
        if deadline != self.deadline || !std::ptr::eq(cancelled, self.cancelled.as_ref()) {
            return Err(Error::Invalid("native cold original request"));
        }
        active(deadline, cancelled)?;
        let current_membership = membership(deadline, cancelled)?;
        if current_membership != self.membership {
            return Err(Error::Invalid("native cold cgroup membership changed"));
        }
        let named = open_cgroup(&current_membership, deadline, cancelled)?;
        for directory in [&self.directory, &named] {
            let current = identity(directory)?;
            if current.st_dev != self.device || current.st_ino != self.inode {
                return Err(Error::Invalid("native cold cgroup custody changed"));
            }
            if scalar(directory, b"memory.max\0", deadline, cancelled)? != self.memory_max
                || scalar(directory, b"memory.swap.max\0", deadline, cancelled)? != 0
            {
                return Err(Error::Invalid("native cold kernel limits changed"));
            }
        }
        let used = scalar(&self.directory, b"memory.current\0", deadline, cancelled)?;
        if used > self.memory_max {
            return Err(Error::Budget("native cold kernel RAM usage"));
        }
        active(deadline, cancelled)?;
        Ok(used)
    }
}

impl NativeColdOpenResourceHold for LinuxCgroupColdOpenResourceHold {
    fn verify_cold_open(
        &self,
        _cold: ColdOpenLimits,
        _process: NativeProcessLimits,
        working_ram_bytes: u64,
        stage_model_bytes: u64,
        sealed_model_bytes: u64,
        additional_copy_bytes: u64,
        copy_buffer_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        if working_ram_bytes != self.working_ram_bytes
            || stage_model_bytes == 0
            || sealed_model_bytes == 0
            || additional_copy_bytes > sealed_model_bytes
            || !matches!(copy_buffer_bytes, 0 | COPY_BUFFER_BYTES)
            || (additional_copy_bytes > 0 && copy_buffer_bytes != COPY_BUFFER_BYTES)
        {
            return Err(Error::Invalid("native cold original RAM envelope"));
        }
        // The compiler already validates the exact cold/process tuple and real
        // RLIMIT_AS/FSIZE through its existing owner kernel before this callback.
        let used = self.check_current(deadline, cancelled)?;
        // Actual current usage already includes allocated stage and copied pages.
        // Only producer-owned unallocated copy bytes and its live stack buffer
        // remain prospective. Cold/runtime heap remains under the SAME hard max.
        headroom(
            used,
            self.memory_max,
            additional_copy_bytes,
            copy_buffer_bytes,
        )?;
        let final_used = self.check_current(deadline, cancelled)?;
        headroom(
            final_used,
            self.memory_max,
            additional_copy_bytes,
            copy_buffer_bytes,
        )?;
        Ok(())
    }
}

fn active(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Acquire) || Instant::now() >= deadline {
        return Err(Error::Budget("native cold original request stopped"));
    }
    Ok(())
}

fn membership(deadline: Instant, cancelled: &AtomicBool) -> Result<Vec<u8>> {
    active(deadline, cancelled)?;
    let mut file = File::open("/proc/self/cgroup")?;
    let mut fs = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::fstatfs(file.as_raw_fd(), fs.as_mut_ptr()) } != 0 {
        return Err(Error::Io(std::io::Error::last_os_error()));
    }
    if unsafe { fs.assume_init() }.f_type != libc::PROC_SUPER_MAGIC {
        return Err(Error::Invalid("native cold kernel proc filesystem"));
    }
    let mut raw = [0u8; PATH_BYTES + 1];
    let mut count = 0;
    loop {
        active(deadline, cancelled)?;
        let n = file.read(&mut raw[count..])?;
        if n == 0 {
            break;
        }
        count += n;
        if count > PATH_BYTES {
            return Err(Error::Budget("native cold membership bytes"));
        }
    }
    let mut selected = None;
    for line in raw[..count].split(|b| *b == b'\n') {
        if let Some(path) = line.strip_prefix(b"0::") {
            if selected.is_some() || !path.starts_with(b"/") || path.contains(&0) {
                return Err(Error::Invalid("native cold unified membership"));
            }
            selected = Some(path.to_vec());
        }
    }
    active(deadline, cancelled)?;
    selected.ok_or(Error::Invalid("native cold cgroup-v2 membership absent"))
}

fn open_cgroup(path: &[u8], deadline: Instant, cancelled: &AtomicBool) -> Result<File> {
    active(deadline, cancelled)?;
    let fd = unsafe {
        libc::open(
            c"/sys/fs/cgroup".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(Error::Io(std::io::Error::last_os_error()));
    }
    let mut directory = unsafe { File::from_raw_fd(fd) };
    let mut name = [0u8; PATH_BYTES + 1];
    for component in path.split(|b| *b == b'/').filter(|c| !c.is_empty()) {
        if component == b"."
            || component == b".."
            || component.len() > PATH_BYTES
            || component.contains(&0)
        {
            return Err(Error::Invalid("native cold cgroup path"));
        }
        active(deadline, cancelled)?;
        name[..component.len()].copy_from_slice(component);
        name[component.len()] = 0;
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr().cast(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(Error::Io(std::io::Error::last_os_error()));
        }
        directory = unsafe { File::from_raw_fd(fd) };
    }
    let mut fs = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::fstatfs(directory.as_raw_fd(), fs.as_mut_ptr()) } != 0 {
        return Err(Error::Io(std::io::Error::last_os_error()));
    }
    if unsafe { fs.assume_init() }.f_type != libc::CGROUP2_SUPER_MAGIC {
        return Err(Error::Invalid("native cold kernel cgroup filesystem"));
    }
    active(deadline, cancelled)?;
    Ok(directory)
}

fn identity(directory: &File) -> Result<libc::stat> {
    let mut value = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(directory.as_raw_fd(), value.as_mut_ptr()) } != 0 {
        return Err(Error::Io(std::io::Error::last_os_error()));
    }
    Ok(unsafe { value.assume_init() })
}

fn scalar(directory: &File, name: &[u8], deadline: Instant, cancelled: &AtomicBool) -> Result<u64> {
    active(deadline, cancelled)?;
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr().cast(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(Error::Io(std::io::Error::last_os_error()));
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    let mut raw = [0u8; 32];
    let mut count = 0;
    loop {
        active(deadline, cancelled)?;
        let n = file.read(&mut raw[count..])?;
        if n == 0 {
            break;
        }
        count += n;
        if count == raw.len() {
            return Err(Error::Budget("native cold kernel scalar bytes"));
        }
    }
    let number = raw[..count].strip_suffix(b"\n").unwrap_or(&raw[..count]);
    if number.is_empty() {
        return Err(Error::Invalid("native cold kernel scalar empty"));
    }
    number.iter().try_fold(0u64, |v, b| {
        if !b.is_ascii_digit() {
            return Err(Error::Invalid("native cold finite kernel scalar"));
        }
        v.checked_mul(10)
            .and_then(|v| v.checked_add((b - b'0') as u64))
            .ok_or(Error::Budget("native cold kernel scalar overflow"))
    })
}

fn headroom(used: u64, maximum: u64, additional: u64, buffer: u64) -> Result<()> {
    let prospective = used
        .checked_add(additional)
        .and_then(|v| v.checked_add(buffer))
        .ok_or(Error::Budget("native cold kernel headroom overflow"))?;
    if prospective > maximum {
        return Err(Error::Budget("native cold kernel RAM headroom"));
    }
    Ok(())
}
