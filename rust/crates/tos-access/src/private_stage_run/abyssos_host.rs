//! Optional Abyss SDK host selection. Data roots never select this software
//! profile; its bytes are embedded from the access product's profile owner.
use super::*;

const PROFILE: &[u8] = include_bytes!("../../../../../access/profiles/abyss-sdk-host.v1.json");

pub(super) struct Host {
    pub(super) executable: String,
    scratch_root: PathBuf,
    admission_socket: PathBuf,
    scratch_root_hold: File,
}

fn selected_paths(
    storage: &serde_json::Value,
    resource: &serde_json::Value,
) -> Result<(PathBuf, PathBuf), String> {
    if storage["schema"] != "abyss_machine_storage_paths_v1"
        || resource["schema"] != "abyss_machine_resource_paths_v2"
    {
        return Err("Abyss host path provider ABI differs".into());
    }
    let tmp = storage["large_roots"]["tmp"]
        .as_str()
        .ok_or("Abyss tmp root absent")?;
    let socket = resource["runtime_admission"]["socket"]
        .as_str()
        .ok_or("Abyss admission socket absent")?;
    let (tmp, socket) = (PathBuf::from(tmp), PathBuf::from(socket));
    path_shape(&tmp)?;
    path_shape(&socket)?;
    Ok((tmp, socket))
}

fn paths(executable: &str, section: &str, end: Cutoff) -> Result<serde_json::Value, String> {
    let bytes = host_command(
        executable,
        &[section.into(), "paths".into(), "--json".into()],
        end,
        false,
    )?;
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

impl Host {
    pub(super) fn select(end: Cutoff) -> Result<Self, String> {
        let profile: serde_json::Value =
            serde_json::from_slice(PROFILE).map_err(|error| error.to_string())?;
        if profile["schema_version"] != "tos_access_sdk_host_profile_v1" {
            return Err("Abyss SDK host profile ABI differs".into());
        }
        let environment = profile["executable_environment"]
            .as_str()
            .ok_or("Abyss host selector absent")?;
        let executable = if let Some(selected) = std::env::var_os(environment) {
            selected
                .into_string()
                .map_err(|_| "Abyss host executable UTF8 required")?
        } else {
            profile["executable_candidates"]
                .as_array()
                .ok_or("Abyss host candidates absent")?
                .iter()
                .filter_map(serde_json::Value::as_str)
                .find(|path| Path::new(path).is_file())
                .ok_or("normal Abyss host owner unavailable")?
                .to_owned()
        };
        path_shape(Path::new(&executable))?;
        let storage = paths(&executable, "storage", end)?;
        let resource = paths(&executable, "resource", end)?;
        let (scratch_root, admission_socket) = selected_paths(&storage, &resource)?;
        let scratch_root_hold = directory(&scratch_root)?;
        Ok(Self {
            executable,
            scratch_root,
            admission_socket,
            scratch_root_hold,
        })
    }

    pub(super) fn scratch(&self, scope: &str) -> Result<PathBuf, String> {
        self.recheck_scratch_parent()?;
        let path = self.scratch_root.join(scope);
        path_shape(&path)?;
        Ok(path)
    }

    pub(super) fn recheck_scratch_parent(&self) -> Result<(), String> {
        identical(&self.scratch_root_hold, &directory(&self.scratch_root)?)
    }

    pub(super) fn admission(
        &self,
        payload: serde_json::Value,
        end: Cutoff,
        cleanup_only: bool,
    ) -> Result<serde_json::Value, String> {
        use std::net::Shutdown;
        use std::os::unix::net::UnixStream;
        let path = &self.admission_socket;
        let meta =
            fs::symlink_metadata(path).map_err(|_| "normal host runtime admission unavailable")?;
        if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
            return Err("host admission socket owner differs".into());
        }
        let mut s = UnixStream::connect(path).map_err(|e| e.to_string())?;
        let mut credentials = unsafe { std::mem::zeroed::<libc::ucred>() };
        let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                s.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                &mut credentials as *mut _ as *mut _,
                &mut size,
            )
        } != 0
            || credentials.uid != unsafe { libc::geteuid() }
        {
            return Err("host admission peer differs".into());
        }
        let remaining = (if cleanup_only { end.whole } else { end.work })
            .checked_sub(clock_ns()?)
            .ok_or("original host cutoff")?;
        s.set_read_timeout(Some(Duration::from_nanos(remaining)))
            .map_err(|e| e.to_string())?;
        s.set_write_timeout(Some(Duration::from_nanos(remaining)))
            .map_err(|e| e.to_string())?;
        let mut encoded = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
        if encoded.len() > 65535 {
            return Err("host request bound".into());
        }
        encoded.push(b'\n');
        s.write_all(&encoded).map_err(|e| e.to_string())?;
        s.shutdown(Shutdown::Write).map_err(|e| e.to_string())?;
        let mut response = Vec::new();
        s.take(65537)
            .read_to_end(&mut response)
            .map_err(|e| e.to_string())?;
        end.cleanup_check()?;
        if response.len() > 65536 {
            return Err("host admission response bound".into());
        }
        let j: serde_json::Value = serde_json::from_slice(&response).map_err(|e| e.to_string())?;
        if j["ok"] != true {
            return Err(format!(
                "normal host admission refused: {}",
                host_refusal_diagnostic(&j, &payload, response.len(), cleanup_only)
            ));
        }
        Ok(j)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_paths_support_relocation_and_refuse_wrong_abi_or_relative_roots() {
        let storage = serde_json::json!({"schema":"abyss_machine_storage_paths_v1","large_roots":{"tmp":"/private/host/scratch"}});
        let resource = serde_json::json!({"schema":"abyss_machine_resource_paths_v2","runtime_admission":{"socket":"/private/runtime/admission.sock"}});
        assert_eq!(
            selected_paths(&storage, &resource).unwrap(),
            (
                PathBuf::from("/private/host/scratch"),
                PathBuf::from("/private/runtime/admission.sock")
            )
        );
        let mut invalid = storage.clone();
        invalid["large_roots"]["tmp"] = serde_json::json!("relative/scratch");
        assert!(selected_paths(&invalid, &resource).is_err());
        invalid = storage.clone();
        invalid["schema"] = serde_json::json!("other_provider");
        assert!(selected_paths(&invalid, &resource).is_err());
        let mut invalid = resource.clone();
        invalid["runtime_admission"]["socket"] = serde_json::json!("/private/../admission.sock");
        assert!(selected_paths(&storage, &invalid).is_err());
    }
}
