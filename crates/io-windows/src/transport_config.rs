use std::path::{Path, PathBuf};

use crate::StorageError;

const PIPE_SUFFIX_LEN: usize = 32;
const MAX_SID_TEXT_LEN: usize = 184;
const BROKER_FILE_NAME: &str = "undelete-master-broker.exe";
const DESKTOP_FILE_NAME: &str = "undelete-master-desktop.exe";

pub(crate) fn build_pipe_name(pipe_suffix: &str) -> Result<String, StorageError> {
    validate_pipe_suffix(pipe_suffix)?;
    Ok(format!(r"\\.\pipe\UndeleteMaster-v1-{pipe_suffix}"))
}

pub(crate) fn build_broker_arguments(
    pipe_suffix: &str,
    parent_pid: u32,
) -> Result<String, StorageError> {
    validate_pipe_suffix(pipe_suffix)?;
    if parent_pid == 0 {
        return Err(StorageError::InvalidParentProcess);
    }
    Ok(format!("--pipe {pipe_suffix} --parent-pid {parent_pid}"))
}

pub(crate) fn build_pipe_sddl(user_sid: &str) -> Result<String, StorageError> {
    if user_sid.len() > MAX_SID_TEXT_LEN
        || !user_sid.starts_with("S-")
        || user_sid[2..].split('-').count() < 2
        || user_sid[2..]
            .split('-')
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(StorageError::InvalidUserIdentity);
    }
    Ok(format!("D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;{user_sid})"))
}

pub(crate) fn broker_executable_from_current(
    current_executable: &Path,
) -> Result<PathBuf, StorageError> {
    if !current_executable.is_absolute() {
        return Err(StorageError::BrokerExecutableUnavailable);
    }
    let directory = current_executable
        .parent()
        .ok_or(StorageError::BrokerExecutableUnavailable)?;
    Ok(directory.join(BROKER_FILE_NAME))
}

pub(crate) fn desktop_executable_from_broker(
    broker_executable: &Path,
) -> Result<PathBuf, StorageError> {
    if !broker_executable.is_absolute()
        || !broker_executable
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(BROKER_FILE_NAME))
    {
        return Err(StorageError::BrokerExecutableUnavailable);
    }
    let directory = broker_executable
        .parent()
        .ok_or(StorageError::BrokerExecutableUnavailable)?;
    Ok(directory.join(DESKTOP_FILE_NAME))
}

fn validate_pipe_suffix(pipe_suffix: &str) -> Result<(), StorageError> {
    if pipe_suffix.len() != PIPE_SUFFIX_LEN
        || !pipe_suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(StorageError::InvalidPipeSuffix);
    }
    Ok(())
}
