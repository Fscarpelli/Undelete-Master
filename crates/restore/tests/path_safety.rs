use um_restore::{
    PathSafetyError, SafeRelativePath, MAX_PATH_EVIDENCE_BYTES_PER_ITEM, MAX_SAFE_COMPONENT_UTF16,
    MAX_SAFE_PATH_COMPONENTS, MAX_SAFE_PATH_UTF16,
};

fn strict(parts: &[&str]) -> Result<SafeRelativePath, PathSafetyError> {
    SafeRelativePath::try_from_components(parts.iter().copied())
}

#[test]
fn path_strictly_accepts_only_bounded_portable_windows_components() {
    let path = strict(&["deleted", "reports", "quarter-1.txt"]).unwrap();
    assert_eq!(
        path.components().collect::<Vec<_>>(),
        ["deleted", "reports", "quarter-1.txt"]
    );
    assert_eq!(path.to_slash_string(), "deleted/reports/quarter-1.txt");
}

#[test]
fn path_rejects_traversal_absolute_prefix_and_separator_injection() {
    let invalid = [
        vec![""],
        vec!["."],
        vec![".."],
        vec![r"C:\escape"],
        vec!["C:"],
        vec![r"\\server\share"],
        vec![r"\rooted"],
        vec!["/rooted"],
        vec!["safe/escape"],
        vec![r"safe\escape"],
    ];

    for parts in invalid {
        assert!(strict(&parts).is_err(), "accepted {parts:?}");
    }
}

#[test]
fn path_rejects_control_ads_trailing_dot_space_and_windows_devices() {
    let invalid = [
        "nul\0byte",
        "line\nfeed",
        "tab\tname",
        "stream:secret",
        "trailing.",
        "trailing ",
        "CON",
        "con.txt",
        "PRN",
        "AUX.log",
        "NUL.bin",
        "COM1",
        "com9.txt",
        "COM¹",
        "com².txt",
        "LPT³.log",
        "LPT1",
        "lpt9.anything",
        "CONIN$",
        "conout$.txt",
    ];

    for component in invalid {
        assert!(strict(&[component]).is_err(), "accepted {component:?}");
    }
}

#[test]
fn path_rejects_components_depth_and_total_length_over_bounds() {
    let component = "a".repeat(MAX_SAFE_COMPONENT_UTF16 + 1);
    assert!(SafeRelativePath::try_from_components([component.as_str()]).is_err());

    let parts = vec!["a"; MAX_SAFE_PATH_COMPONENTS + 1];
    assert!(strict(&parts).is_err());

    let wide_component = "a".repeat(MAX_SAFE_COMPONENT_UTF16);
    let total_parts =
        vec![wide_component.as_str(); (MAX_SAFE_PATH_UTF16 / MAX_SAFE_COMPONENT_UTF16) + 2];
    assert!(strict(&total_parts).is_err());
}

#[test]
fn path_lossy_derivation_uses_deterministic_safe_fallback_and_json_only_evidence() {
    let first = SafeRelativePath::derive_for_recovery(&["..", "safe"], r"CON.txt:secret").unwrap();
    let second = SafeRelativePath::derive_for_recovery(&["..", "safe"], r"CON.txt:secret").unwrap();

    assert_eq!(first.path(), second.path());
    assert_ne!(
        first.path().components().collect::<Vec<_>>(),
        ["..", "safe", r"CON.txt:secret"]
    );
    assert!(first
        .path()
        .components()
        .all(|component| strict(&[component]).is_ok()));

    let safe_display = first.path().to_slash_string();
    assert!(!safe_display.contains("CON"));
    assert!(!safe_display.contains("secret"));
    assert!(!safe_display.contains(".."));

    let evidence = first.evidence_json();
    assert!(evidence.starts_with(r#"{"version":1,"substitutions":["#));
    assert!(evidence.contains(r#""original":"..""#));
    assert!(evidence.contains(r#""original":"CON.txt:secret""#));
}

#[test]
fn path_lossy_derivation_does_not_relax_path_depth_or_total_bounds() {
    let too_deep = vec!["unsafe:name".to_owned(); MAX_SAFE_PATH_COMPONENTS];
    assert!(
        SafeRelativePath::derive_for_recovery(&too_deep, "final.txt").is_err(),
        "fallback must not bypass the path-depth budget"
    );
}

#[test]
fn path_file_selection_contains_only_its_sanitized_ancestors() {
    let selected =
        SafeRelativePath::derive_for_recovery(&["chosen", "nested"], "only.txt").unwrap();
    assert_eq!(
        selected.path().components().collect::<Vec<_>>(),
        ["chosen", "nested", "only.txt"]
    );
    assert!(!selected
        .path()
        .to_slash_string()
        .contains("historical-sibling"));
}

#[test]
fn path_rejects_untrusted_original_evidence_over_the_explicit_byte_budget() {
    let oversized = format!("{}:", "x".repeat(MAX_PATH_EVIDENCE_BYTES_PER_ITEM));

    assert!(matches!(
        SafeRelativePath::derive_for_recovery(&[] as &[&str], &oversized),
        Err(PathSafetyError::EvidenceBudgetExceeded {
            maximum: MAX_PATH_EVIDENCE_BYTES_PER_ITEM,
            ..
        })
    ));
}
