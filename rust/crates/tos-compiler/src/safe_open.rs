//! Use the one shared Linux anchored opener. Source digest, selection, and
//! current policy remain separate compiler/source-owner checks.

use crate::{Error, Result};
use std::{fs::File, path::Path};
use tos_fd_open::{OpenErrorCode, open_absolute_regular};

pub(crate) fn open_regular(path: &Path, cap: u64) -> Result<File> {
    open_absolute_regular(path, cap).map_err(|error| match error.code {
        OpenErrorCode::BudgetExceeded => Error::Budget("opened carrier bytes"),
        OpenErrorCode::Io => error
            .source
            .map(Error::Io)
            .unwrap_or(Error::Invalid("carrier open failed")),
        OpenErrorCode::InvalidPath
        | OpenErrorCode::UnsafePath
        | OpenErrorCode::UnsupportedPlatform => Error::Invalid("unsafe carrier path"),
    })
}
