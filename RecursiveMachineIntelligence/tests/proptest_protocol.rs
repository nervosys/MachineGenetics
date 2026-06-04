//! Fuzz-style property-based tests for protocol parsing
//!
//! Ensures the protocol layer never panics on arbitrary input and
//! correctly validates well-formed messages via roundtrip properties.

use proptest::prelude::*;
use rmi::core::protocol::{
    Message, MessageFlags, MessageHeader, MessageType, Protocol, TensorAttachment, HEADER_SIZE,
    PROTOCOL_MAGIC,
};
use uuid::Uuid;

// ============================================================================
// Strategy: arbitrary bytes
// ============================================================================

fn arb_bytes(max_len: usize) -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..max_len)
}

fn arb_message_type() -> impl Strategy<Value = MessageType> {
    prop_oneof![
        Just(MessageType::Handshake),
        Just(MessageType::HandshakeAck),
        Just(MessageType::Heartbeat),
        Just(MessageType::Disconnect),
        Just(MessageType::CapabilityQuery),
        Just(MessageType::CapabilityResponse),
        Just(MessageType::AgentDiscovery),
        Just(MessageType::AgentAnnounce),
        Just(MessageType::TaskRequest),
        Just(MessageType::TaskAccept),
        Just(MessageType::TaskReject),
        Just(MessageType::TaskProgress),
        Just(MessageType::TaskComplete),
        Just(MessageType::TaskCancel),
        Just(MessageType::TensorTransfer),
        Just(MessageType::GradientTransfer),
        Just(MessageType::ModelTransfer),
        Just(MessageType::OntologyTransfer),
        Just(MessageType::Query),
        Just(MessageType::QueryResponse),
        Just(MessageType::InferenceRequest),
        Just(MessageType::InferenceResponse),
        Just(MessageType::Proposal),
        Just(MessageType::Vote),
        Just(MessageType::Commit),
        Just(MessageType::Abort),
        Just(MessageType::StreamStart),
        Just(MessageType::StreamData),
        Just(MessageType::StreamEnd),
    ]
}

fn arb_flags() -> impl Strategy<Value = MessageFlags> {
    any::<u32>().prop_map(MessageFlags::from_bits_truncate)
}

// ============================================================================
// Fuzz: MessageHeader::from_bytes must never panic
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn header_from_bytes_never_panics(data in arb_bytes(128)) {
        // Should return Ok or Err, but never panic
        let _ = MessageHeader::from_bytes(&data);
    }
}

// ============================================================================
// Fuzz: Message::from_binary must never panic
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn message_from_binary_never_panics(data in arb_bytes(512)) {
        // Should return Ok or Err, but never panic
        let _ = Message::from_binary(&data);
    }
}

// ============================================================================
// Fuzz: TensorAttachment::from_binary must never panic
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn tensor_from_binary_never_panics(data in arb_bytes(256)) {
        let _ = TensorAttachment::from_binary(&data);
    }
}

// ============================================================================
// Property: Header roundtrip is lossless for all valid message types
// ============================================================================

proptest! {
    #[test]
    fn header_roundtrip_all_types(
        msg_type in arb_message_type(),
        flags in arb_flags(),
        payload_len in 0u64..1_000_000,
    ) {
        let header = MessageHeader::new(msg_type, flags, payload_len);
        let bytes = header.to_bytes();

        prop_assert_eq!(bytes.len(), HEADER_SIZE);
        prop_assert_eq!(&bytes[0..4], &PROTOCOL_MAGIC);

        let restored = MessageHeader::from_bytes(&bytes).unwrap();
        prop_assert_eq!(header.version, restored.version);
        prop_assert_eq!(header.message_type, restored.message_type);
        prop_assert_eq!(header.flags, restored.flags);
        prop_assert_eq!(header.payload_length, restored.payload_length);
    }
}

// ============================================================================
// Property: Message roundtrip with arbitrary payloads
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn message_roundtrip_arbitrary_payload(payload in arb_bytes(256)) {
        let sender = Uuid::new_v4();
        let recipient = Uuid::new_v4();
        let msg = Message::new(sender, recipient, MessageType::Query, payload.clone());

        let binary = msg.to_binary();
        let restored = Message::from_binary(&binary);

        prop_assert!(restored.is_ok(), "Valid message should deserialize: {:?}", restored.err());
        let restored = restored.unwrap();
        prop_assert_eq!(msg.sender_id, restored.sender_id);
        prop_assert_eq!(msg.recipient_id, restored.recipient_id);
        prop_assert_eq!(msg.payload, restored.payload);
    }
}

// ============================================================================
// Property: Message priority clamped to [0, 10]
// ============================================================================

proptest! {
    #[test]
    fn message_priority_clamped(priority in 0u8..=255) {
        let msg = Message::new(Uuid::new_v4(), Uuid::new_v4(), MessageType::Query, vec![])
            .with_priority(priority);
        prop_assert!(msg.priority <= 10, "Priority should be clamped to max 10");
    }
}

// ============================================================================
// Fuzz: Corrupted magic bytes are always rejected
// ============================================================================

proptest! {
    #[test]
    fn corrupted_magic_rejected(
        b0 in any::<u8>(),
        b1 in any::<u8>(),
        b2 in any::<u8>(),
        b3 in any::<u8>(),
    ) {
        let magic = [b0, b1, b2, b3];
        if magic != PROTOCOL_MAGIC {
            let mut data = [0u8; HEADER_SIZE];
            data[0..4].copy_from_slice(&magic);
            let result = MessageHeader::from_bytes(&data);
            prop_assert!(result.is_err(), "Non-FWRX magic should be rejected");
        }
    }
}

// ============================================================================
// Fuzz: Truncated headers are rejected
// ============================================================================

proptest! {
    #[test]
    fn truncated_header_rejected(len in 0usize..HEADER_SIZE) {
        let data = vec![0u8; len];
        let result = MessageHeader::from_bytes(&data);
        prop_assert!(result.is_err(),
            "Header with only {} bytes (need {}) should be rejected", len, HEADER_SIZE);
    }
}

// ============================================================================
// Property: get_message_type maps all valid discriminants
// ============================================================================

proptest! {
    #[test]
    fn get_message_type_covers_all(msg_type in arb_message_type()) {
        let header = MessageHeader::new(msg_type, MessageFlags::NONE, 0);
        let decoded = header.get_message_type();
        prop_assert!(decoded.is_some(),
            "Valid message type {:?} should decode from header", msg_type);
        prop_assert_eq!(decoded.unwrap(), msg_type);
    }
}

// ============================================================================
// Property: Unknown message types return None
// ============================================================================

proptest! {
    #[test]
    fn unknown_message_type_returns_none(raw in 0x0100u16..0xFFFF) {
        // Skip known values
        let known: Vec<u16> = vec![
            0x0001, 0x0002, 0x0003, 0x0004,
            0x0010, 0x0011, 0x0012, 0x0013,
            0x0020, 0x0021, 0x0022, 0x0023, 0x0024, 0x0025,
            0x0030, 0x0031, 0x0032, 0x0033,
            0x0040, 0x0041, 0x0042, 0x0043,
            0x0050, 0x0051, 0x0052, 0x0053,
            0x0060, 0x0061, 0x0062,
        ];
        if !known.contains(&raw) {
            let mut header = MessageHeader::new(MessageType::Query, MessageFlags::NONE, 0);
            header.message_type = raw;
            prop_assert!(header.get_message_type().is_none(),
                "Unknown message type 0x{:04x} should return None", raw);
        }
    }
}

// ============================================================================
// Property: Checksum mismatch detected on corruption
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn checksum_detects_corruption(
        payload in arb_bytes(64),
        corrupt_pos in 0usize..32,
        corrupt_val in 1u8..255,
    ) {
        let msg = Message::new(Uuid::new_v4(), Uuid::new_v4(), MessageType::Query, payload);
        let mut binary = msg.to_binary();
        if binary.len() > HEADER_SIZE + corrupt_pos {
            let pos = HEADER_SIZE + corrupt_pos;
            binary[pos] = binary[pos].wrapping_add(corrupt_val);
            let result = Message::from_binary(&binary);
            // Corruption should cause either checksum failure or decompression failure
            prop_assert!(result.is_err(),
                "Corrupted payload should fail deserialization");
        }
    }
}

// ============================================================================
// Property: Protocol default configuration is sane
// ============================================================================

#[test]
fn protocol_defaults_valid() {
    let proto = Protocol::new();
    assert_eq!(proto.compression, "lz4");
    assert!(proto.max_message_size > 0);
    assert!(proto.stream_chunk_size > 0);
    assert!(proto.encryption.is_none());
}

// ============================================================================
// Property: Schema validation rejects missing required fields
// ============================================================================

#[test]
fn schema_rejects_missing_required() {
    let schema = rmi::core::protocol::schemas::handshake();
    let empty = std::collections::HashMap::new();
    assert!(!schema.validate(&empty));
}

// ============================================================================
// Property: Schema roundtrip through binary
// ============================================================================

#[test]
fn schema_binary_roundtrip() {
    use rmi::core::protocol::schemas;
    let schema = schemas::task_request();
    let binary = schema.to_binary();
    let restored = rmi::core::protocol::MessageSchema::from_binary(&binary).unwrap();
    assert_eq!(schema.version(), restored.version());
}

// ============================================================================
// Property: Tensor attachment roundtrip with various shapes
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn tensor_roundtrip_shapes(
        rows in 1usize..8,
        cols in 1usize..8,
    ) {
        let size = rows * cols;
        let data: Vec<f32> = (0..size).map(|i| i as f32 * 0.1).collect();
        let array = ndarray::ArrayD::from_shape_vec(ndarray::IxDyn(&[rows, cols]), data).unwrap();
        let attachment = TensorAttachment::from_array_f32("prop_test", &array);
        let binary = attachment.to_binary();
        let (restored, consumed) = TensorAttachment::from_binary(&binary).unwrap();

        prop_assert_eq!(&attachment.name, &restored.name);
        prop_assert_eq!(&attachment.shape, &restored.shape);
        prop_assert_eq!(&attachment.dtype, &restored.dtype);
        prop_assert_eq!(&attachment.data, &restored.data);
        prop_assert_eq!(consumed, binary.len());
    }
}
