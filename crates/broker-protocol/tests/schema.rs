use um_broker_protocol::{
    encode_frame, BrokerErrorCode, Message, Opcode, ProtocolError, ALL_OPCODES, MAX_SOURCE_ID_LEN,
    MESSAGE_SCHEMA,
};

#[test]
fn broker_protocol_arch_001_forbids_unsafe_code_at_crate_boundary() {
    let crate_root = include_str!("../src/lib.rs");
    assert!(crate_root.contains("#![forbid(unsafe_code)]"));
}

#[test]
fn broker_protocol_arch_002_schema_is_the_exact_operation_allowlist() {
    let expected = [
        (Opcode::Hello, "Hello", &["challenge"][..]),
        (Opcode::HelloAck, "HelloAck", &["challenge"][..]),
        (Opcode::OpenSource, "OpenSource", &["source_id"][..]),
        (
            Opcode::Opened,
            "Opened",
            &[
                "handle_id",
                "size",
                "logical_sector",
                "physical_sector",
                "physical_disk_number",
            ][..],
        ),
        (
            Opcode::ReadAt,
            "ReadAt",
            &["handle_id", "offset", "length"][..],
        ),
        (Opcode::ReadData, "ReadData", &["bytes"][..]),
        (Opcode::CloseSource, "CloseSource", &["handle_id"][..]),
        (Opcode::Closed, "Closed", &["handle_id"][..]),
        (Opcode::Shutdown, "Shutdown", &[][..]),
        (Opcode::Error, "Error", &["code"][..]),
    ];

    assert_eq!(MESSAGE_SCHEMA.len(), expected.len());
    for (actual, (opcode, name, fields)) in MESSAGE_SCHEMA.iter().zip(expected) {
        assert_eq!(actual.opcode, opcode);
        assert_eq!(actual.name, name);
        assert_eq!(actual.fields, fields);
    }
    assert_eq!(MESSAGE_SCHEMA.len(), ALL_OPCODES.len());
    for (schema, opcode) in MESSAGE_SCHEMA.iter().zip(ALL_OPCODES) {
        assert_eq!(schema.opcode, opcode);
        assert_eq!(
            Opcode::try_from(opcode as u16).expect("known opcode"),
            opcode
        );
    }
    assert!(matches!(
        Opcode::try_from(0),
        Err(ProtocolError::UnknownOpcode { actual: 0 })
    ));
    assert!(matches!(
        Opcode::try_from(11),
        Err(ProtocolError::UnknownOpcode { actual: 11 })
    ));
}

#[test]
fn broker_protocol_arch_003_schema_has_no_privileged_generic_or_sink_fields() {
    const FORBIDDEN_TOKENS: [&str; 13] = [
        "path",
        "destination",
        "access_mask",
        "ioctl",
        "device_control",
        "command",
        "execute",
        "format",
        "trim",
        "delete",
        "lock",
        "dismount",
        "write",
    ];

    for entry in MESSAGE_SCHEMA {
        let name = entry.name.to_ascii_lowercase();
        for forbidden in FORBIDDEN_TOKENS {
            assert!(
                !name.contains(forbidden),
                "message {} contains forbidden token {forbidden}",
                entry.name
            );
            for field in entry.fields {
                assert!(
                    !field.to_ascii_lowercase().contains(forbidden),
                    "field {field} contains forbidden token {forbidden}"
                );
            }
        }
    }
}

#[test]
fn broker_protocol_arch_004_opaque_identifiers_cannot_transport_device_names() {
    let forbidden_ids = [
        [r"\\.\", "PhysicalDrive0"].concat(),
        [r"\\?\", "Volume{1234}"].concat(),
        ["/dev", "/sda"].concat(),
        "C:".to_owned(),
        "source id".to_owned(),
    ];
    for source_id in forbidden_ids {
        let message = Message::OpenSource { source_id };
        assert!(matches!(
            encode_frame(1, &message),
            Err(ProtocolError::InvalidIdentifier {
                field: "source_id",
                ..
            })
        ));
    }
}

#[test]
fn broker_protocol_schema_001_enforces_all_identifier_bounds() {
    let mut message = Message::OpenSource {
        source_id: "s".repeat(MAX_SOURCE_ID_LEN),
    };
    encode_frame(1, &message).expect("exact identifier limits are valid");

    if let Message::OpenSource { source_id, .. } = &mut message {
        source_id.push('s');
    }
    assert!(matches!(
        encode_frame(1, &message),
        Err(ProtocolError::InvalidFieldLength {
            field: "source_id",
            ..
        })
    ));
}

#[test]
fn broker_protocol_schema_002_error_codes_have_stable_wire_numbers() {
    let expected = [
        (BrokerErrorCode::InvalidRequest, 1),
        (BrokerErrorCode::AuthenticationFailed, 2),
        (BrokerErrorCode::SourceNotFound, 3),
        (BrokerErrorCode::SourceIdentityMismatch, 4),
        (BrokerErrorCode::SourceUnavailable, 5),
        (BrokerErrorCode::InvalidHandle, 6),
        (BrokerErrorCode::ReadOutOfRange, 7),
        (BrokerErrorCode::ReadFailed, 8),
        (BrokerErrorCode::SourceChanged, 9),
        (BrokerErrorCode::InternalFailure, 10),
    ];
    for (code, wire) in expected {
        assert_eq!(code as u16, wire);
        assert_eq!(BrokerErrorCode::try_from(wire).expect("stable code"), code);
    }
    assert!(matches!(
        BrokerErrorCode::try_from(0),
        Err(ProtocolError::UnknownErrorCode { actual: 0 })
    ));
}
