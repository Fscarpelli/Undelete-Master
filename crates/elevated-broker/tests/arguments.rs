use std::ffi::OsString;

use um_elevated_broker::{BrokerArgs, BrokerArgumentError};

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn elevated_broker_args_001_accepts_only_pipe_suffix_and_parent_pid() {
    let parsed = BrokerArgs::parse(args(&[
        "--pipe",
        "0123456789abcdef0123456789ABCDEF",
        "--parent-pid",
        "9123",
    ]))
    .expect("valid broker arguments");

    assert_eq!(parsed.pipe_suffix(), "0123456789abcdef0123456789ABCDEF");
    assert_eq!(parsed.parent_pid(), 9123);
}

#[test]
fn elevated_broker_args_002_rejects_missing_reordered_or_extra_arguments() {
    for values in [
        &[][..],
        &["--pipe", "0123456789abcdef0123456789abcdef"][..],
        &[
            "--parent-pid",
            "9",
            "--pipe",
            "0123456789abcdef0123456789abcdef",
        ][..],
        &[
            "--pipe",
            "0123456789abcdef0123456789abcdef",
            "--parent-pid",
            "9",
            "--extra",
        ][..],
        &["--source", "volume-1", "--parent-pid", "9"][..],
    ] {
        assert_eq!(
            BrokerArgs::parse(args(values)),
            Err(BrokerArgumentError::InvalidShape)
        );
    }
}

#[test]
fn elevated_broker_args_003_rejects_path_like_or_invalid_pipe_suffixes() {
    let too_long = "a".repeat(33);
    for suffix in [
        "",
        "short",
        r"\\.\pipe\undelete-master",
        "../pipe",
        "pipe/name",
        "pipe.name",
        "pipe name",
        "0123456789abcdef0123456789abcdeg",
        too_long.as_str(),
    ] {
        assert_eq!(
            BrokerArgs::parse(args(&["--pipe", suffix, "--parent-pid", "9"])),
            Err(BrokerArgumentError::InvalidPipeSuffix)
        );
    }
}

#[test]
fn elevated_broker_args_004_rejects_zero_or_non_numeric_parent_pid() {
    for pid in ["0", "-1", "not-a-pid", "4294967296"] {
        assert_eq!(
            BrokerArgs::parse(args(&[
                "--pipe",
                "0123456789abcdef0123456789abcdef",
                "--parent-pid",
                pid,
            ])),
            Err(BrokerArgumentError::InvalidParentPid)
        );
    }
}

#[test]
fn elevated_broker_args_005_errors_never_echo_argument_content() {
    let marker = r"\\?\Volume{PRIVATE-MARKER}";
    let error = BrokerArgs::parse(args(&["--pipe", marker, "--parent-pid", "9"]))
        .expect_err("path-like suffix");
    assert_eq!(error.to_string(), "invalid broker pipe suffix");
    assert!(!error.to_string().contains(marker));
}
