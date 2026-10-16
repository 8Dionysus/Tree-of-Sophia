//! Exact output-parent custody. Never open or truncate the installed leaf.
use super::{Result, V2, fail, mint, tick};
use fs2::FileExt;
use std::{
    ffi::CString,
    fs::File,
    io::Write,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
    path::{Path, PathBuf},
    time::Instant,
};
pub(super) struct OutputOwner {
    root: PathBuf,
    root_file: File,
    parent: File,
}
fn descend(root: &File) -> Result<File> {
    let mut parent = tos_fd_open::reopen_directory(root).map_err(|e| e.to_string())?;
    for leaf in Path::new(V2).components() {
        parent = tos_fd_open::open_directory_at(&parent, Path::new(leaf.as_os_str()))
            .map_err(|e| e.to_string())?;
    }
    Ok(parent)
}
fn same(a: &File, b: &File) -> Result<bool> {
    let a = a.metadata().map_err(|e| e.to_string())?;
    let b = b.metadata().map_err(|e| e.to_string())?;
    Ok(a.dev() == b.dev() && a.ino() == b.ino())
}
fn fingerprint(st: &libc::stat) -> (u64, u64, i64, i64, i64, i64, i64) {
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
fn stat(parent: &File, leaf: &CString) -> Result<Option<libc::stat>> {
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
        let e = std::io::Error::last_os_error();
        if e.raw_os_error() == Some(libc::ENOENT) {
            return Ok(None);
        }
        return Err(e.to_string());
    }
    let st = unsafe { st.assume_init() };
    if st.st_mode & libc::S_IFMT != libc::S_IFREG {
        return fail("output leaf is not a regular file");
    }
    Ok(Some(st))
}
impl OutputOwner {
    pub(super) fn acquire(root: &Path, deadline: Instant) -> Result<Self> {
        tick(deadline)?;
        let root_file = tos_fd_open::open_absolute_directory(root).map_err(|e| e.to_string())?;
        let parent = descend(&root_file)?;
        parent
            .try_lock_exclusive()
            .map_err(|e| format!("structural output parent busy: {e}"))?;
        let owner = Self {
            root: root.to_owned(),
            root_file,
            parent,
        };
        owner.check(deadline)?;
        Ok(owner)
    }
    fn check(&self, deadline: Instant) -> Result<()> {
        tick(deadline)?;
        let current =
            tos_fd_open::open_absolute_directory(&self.root).map_err(|e| e.to_string())?;
        if !same(&current, &self.root_file)? || !same(&descend(&current)?, &self.parent)? {
            return fail("selected structural output root or parent changed");
        }
        Ok(())
    }
    pub(super) fn write(
        &self,
        reference: &str,
        raw: &[u8],
        new_only: bool,
        deadline: Instant,
    ) -> Result<()> {
        self.check(deadline)?;
        let path = Path::new(reference);
        if path.parent() != Some(Path::new(V2)) {
            return fail("output reference outside structural parent");
        }
        let leaf = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("output leaf")?;
        if new_only {
            if !matches!(
                leaf,
                "identity-issuance.v2.json" | "primary-challenger-input.v2.json"
            ) {
                return fail("new-only structural output not owned");
            }
        } else if !super::OUTPUTS
            .iter()
            .any(|(kind, _)| super::output(kind) == reference)
        {
            return fail("replacement structural output not owned");
        }
        let leaf = CString::new(leaf).map_err(|e| e.to_string())?;
        let initial = stat(&self.parent, &leaf)?;
        if new_only && initial.is_some() {
            return fail("structural new-only output already exists");
        }
        let stage = CString::new(format!(".native-{}.tmp", mint("structural")?))
            .map_err(|e| e.to_string())?;
        let fd = unsafe {
            libc::openat(
                self.parent.as_raw_fd(),
                stage.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        let result = (|| {
            for chunk in raw.chunks(65536) {
                tick(deadline)?;
                file.write_all(chunk).map_err(|e| e.to_string())?;
            }
            let mode = initial
                .as_ref()
                .map(|st| st.st_mode & 0o777)
                .unwrap_or(0o644);
            if unsafe { libc::fchmod(file.as_raw_fd(), mode) } < 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            file.sync_all().map_err(|e| e.to_string())?;
            self.check(deadline)?;
            let current = stat(&self.parent, &leaf)?;
            if initial.as_ref().map(fingerprint) != current.as_ref().map(fingerprint) {
                return fail("structural output leaf changed before commit");
            }
            let result = if new_only || initial.is_none() {
                // linkat installs only an absent leaf atomically. A racing file or
                // symlink fails with EEXIST, preserving both bytes and identity.
                unsafe {
                    libc::linkat(
                        self.parent.as_raw_fd(),
                        stage.as_ptr(),
                        self.parent.as_raw_fd(),
                        leaf.as_ptr(),
                        0,
                    )
                }
            } else {
                unsafe {
                    libc::renameat(
                        self.parent.as_raw_fd(),
                        stage.as_ptr(),
                        self.parent.as_raw_fd(),
                        leaf.as_ptr(),
                    )
                }
            };
            if result < 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            self.parent.sync_all().map_err(|e| e.to_string())?;
            Ok(())
        })();
        // Held-FD cleanup works even if the selected namespace was replaced.
        let cleanup = unsafe { libc::unlinkat(self.parent.as_raw_fd(), stage.as_ptr(), 0) };
        if cleanup < 0
            && std::io::Error::last_os_error().raw_os_error() != Some(libc::ENOENT)
            && result.is_ok()
        {
            return fail("structural staging cleanup failed");
        }
        result?;
        self.check(deadline)?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::super::default_deadline;
    use super::*;
    use std::{fs, os::unix::fs::symlink};
    #[test]
    fn output_fingerprint_detects_same_inode_dirty_write() {
        let temp = tempfile::tempdir().unwrap();
        let directory = tos_fd_open::open_absolute_directory(temp.path()).unwrap();
        let leaf = CString::new("leaf").unwrap();
        fs::write(temp.path().join("leaf"), b"a").unwrap();
        let before = stat(&directory, &leaf).unwrap().unwrap();
        fs::write(temp.path().join("leaf"), b"changed").unwrap();
        let after = stat(&directory, &leaf).unwrap().unwrap();
        assert_eq!((before.st_dev, before.st_ino), (after.st_dev, after.st_ino));
        assert_ne!(fingerprint(&before), fingerprint(&after));
    }
    #[test]
    fn held_output_refuses_symlink_and_preserves_new_only() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join(V2)).unwrap();
        let ref_ = super::super::reference("identity-issuance.v2.json");
        let owner = OutputOwner::acquire(root, default_deadline()).unwrap();
        owner
            .write(&ref_, b"first", true, default_deadline())
            .unwrap();
        assert!(
            owner
                .write(&ref_, b"second", true, default_deadline())
                .is_err()
        );
        assert_eq!(fs::read(root.join(&ref_)).unwrap(), b"first");
        let outside = root.join("outside");
        fs::write(&outside, b"untouched").unwrap();
        let generated = super::super::output("summary");
        symlink(&outside, root.join(&generated)).unwrap();
        assert!(
            owner
                .write(&generated, b"replacement", false, default_deadline())
                .is_err()
        );
        assert_eq!(fs::read(outside).unwrap(), b"untouched");
    }
    #[test]
    fn symlink_root_and_output_parent_refused_before_write() {
        let temp = tempfile::tempdir().unwrap();
        let real = temp.path().join("real");
        fs::create_dir_all(real.join(V2)).unwrap();
        let alias = temp.path().join("alias");
        symlink(&real, &alias).unwrap();
        assert!(OutputOwner::acquire(&alias, default_deadline()).is_err());
        let parent = real.join(V2);
        let moved = real.join("moved");
        fs::rename(&parent, &moved).unwrap();
        symlink(&moved, &parent).unwrap();
        assert!(OutputOwner::acquire(&real, default_deadline()).is_err());
    }
    #[test]
    fn held_parent_replacement_refused_and_regular_replace_exact() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join(V2)).unwrap();
        let owner = OutputOwner::acquire(root, default_deadline()).unwrap();
        let generated = super::super::output("summary");
        owner
            .write(&generated, b"old", false, default_deadline())
            .unwrap();
        owner
            .write(&generated, b"exact", false, default_deadline())
            .unwrap();
        assert_eq!(fs::read(root.join(&generated)).unwrap(), b"exact");
        fs::rename(root.join(V2), root.join("moved")).unwrap();
        fs::create_dir(root.join(V2)).unwrap();
        assert!(
            owner
                .write(&generated, b"wrong", false, default_deadline())
                .is_err()
        );
        assert_eq!(
            fs::read(root.join("moved").join("summary.v2.json")).unwrap(),
            b"exact"
        );
    }
}
