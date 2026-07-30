use um_broker_protocol::{ALL_OPCODES, MESSAGE_SCHEMA};

#[test]
fn elevated_broker_arch_001_forbids_unsafe_and_privileged_parser_imports() {
    let library = include_str!("../src/lib.rs");
    let binary = include_str!("../src/main.rs");
    let combined = format!("{library}\n{binary}").to_ascii_lowercase();

    assert!(library.contains("#![forbid(unsafe_code)]"));
    for forbidden in [
        "um_fs_",
        "um_partition",
        "um_carving",
        "um_restore",
        "tauri",
        "deviceiocontrol",
        "ioctl",
        "createprocess",
        "shellexecute",
    ] {
        assert!(
            !combined.contains(forbidden),
            "elevated broker imports forbidden boundary {forbidden}"
        );
    }
}

#[test]
fn elevated_broker_arch_002_manifest_has_a_minimal_dependency_boundary() {
    let manifest = include_str!("../Cargo.toml").to_ascii_lowercase();
    for forbidden in [
        "um-core",
        "um-io-common",
        "um-partition",
        "um-fs-",
        "um-carving",
        "um-restore",
        "tauri",
        "serde_json",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "manifest contains forbidden dependency {forbidden}"
        );
    }
    assert!(manifest.contains("um-broker-protocol"));
    assert!(manifest.contains("um-io-windows"));
}

#[test]
fn elevated_broker_arch_003_protocol_schema_has_no_mutation_or_generic_control() {
    assert_eq!(MESSAGE_SCHEMA.len(), ALL_OPCODES.len());
    for entry in MESSAGE_SCHEMA {
        let schema = format!("{} {}", entry.name, entry.fields.join(" ")).to_ascii_lowercase();
        for forbidden in [
            "write",
            "destination",
            "path",
            "access_mask",
            "ioctl",
            "device_control",
            "trim",
            "format",
            "delete",
            "lock",
            "dismount",
            "mount",
            "execute",
        ] {
            assert!(
                !schema.contains(forbidden),
                "protocol schema contains forbidden token {forbidden}"
            );
        }
    }
}

#[test]
fn elevated_broker_arch_004_has_no_free_form_logging_or_source_path_api() {
    let library = include_str!("../src/lib.rs").to_ascii_lowercase();
    let binary = include_str!("../src/main.rs").to_ascii_lowercase();
    let combined = format!("{library}\n{binary}");

    for forbidden in [
        "println!",
        "eprintln!",
        "dbg!",
        "log::",
        "tracing::",
        "source_path",
        "device_path",
        "volume_path",
        "open_path",
        "destination",
    ] {
        assert!(
            !combined.contains(forbidden),
            "elevated broker contains forbidden surface {forbidden}"
        );
    }
}

#[test]
fn elevated_broker_arch_005_binary_embeds_require_administrator_manifest() {
    let manifest = include_str!("../undelete-master-broker.manifest");
    let build_script = include_str!("../build.rs");

    assert!(manifest.contains("requestedExecutionLevel"));
    assert!(manifest.contains("level=\"requireAdministrator\""));
    assert!(manifest.contains("uiAccess=\"false\""));
    assert!(build_script.contains("/MANIFEST:EMBED"));
    assert!(build_script.contains("/MANIFESTINPUT:"));
}

#[test]
fn elevated_broker_arch_006_authenticates_the_packaged_desktop_before_serving() {
    let library = include_str!("../src/lib.rs");
    let peer_check = library
        .find("peer_is_packaged_desktop")
        .expect("broker must authenticate the desktop image");
    let session = library
        .find("serve_session(&mut connection")
        .expect("broker session entrypoint");

    assert!(
        peer_check < session,
        "desktop image authentication must precede the privileged read session"
    );
}
