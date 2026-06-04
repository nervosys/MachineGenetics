//! Protocol System - Maximally Efficient Inter-Agent Communication
//!
//! The protocol layer provides binary-first communication between agents.
//! Unlike human-oriented APIs (REST, GraphQL), this system optimizes for:
//!
//! 1. Minimal serialization overhead
//! 2. Schema evolution without version negotiation
//! 3. Direct tensor/gradient sharing
//! 4. Streaming for continuous data
//! 5. Cryptographic integrity
//!
//! ## Message Format
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────┐
//! │ Header (fixed 32 bytes)                                    │
//! ├────────────────────────────────────────────────────────────┤
//! │ Magic (4) │ Version (2) │ Type (2) │ Flags (4) │ Len (8)  │
//! │ Checksum (8) │ Reserved (4)                                │
//! ├────────────────────────────────────────────────────────────┤
//! │ Payload (variable, LZ4 compressed msgpack)                 │
//! ├────────────────────────────────────────────────────────────┤
//! │ Optional: Tensor Attachments (raw binary)                  │
//! └────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::hash::Hasher;
use std::time::{SystemTime, UNIX_EPOCH};

use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use xxhash_rust::xxh64::{xxh64, Xxh64};

use crate::error::{RmiError, Result};

/// Protocol magic bytes
pub const PROTOCOL_MAGIC: [u8; 4] = *b"FWRX";
/// Protocol version
pub const PROTOCOL_VERSION: u16 = 1;
/// Header size in bytes
pub const HEADER_SIZE: usize = 32;

/// Types of messages in the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum MessageType {
    // Control
    /// Initial handshake
    Handshake = 0x0001,
    /// Handshake acknowledgment
    HandshakeAck = 0x0002,
    /// Keep-alive
    Heartbeat = 0x0003,
    /// Graceful disconnect
    Disconnect = 0x0004,

    // Discovery
    /// Query for capabilities
    CapabilityQuery = 0x0010,
    /// Capability response
    CapabilityResponse = 0x0011,
    /// Discover agents
    AgentDiscovery = 0x0012,
    /// Announce agent
    AgentAnnounce = 0x0013,

    // Task coordination
    /// Request task execution
    TaskRequest = 0x0020,
    /// Accept task
    TaskAccept = 0x0021,
    /// Reject task
    TaskReject = 0x0022,
    /// Task progress update
    TaskProgress = 0x0023,
    /// Task completion
    TaskComplete = 0x0024,
    /// Cancel task
    TaskCancel = 0x0025,

    // Data transfer
    /// Transfer tensor data
    TensorTransfer = 0x0030,
    /// Transfer gradient data
    GradientTransfer = 0x0031,
    /// Transfer model
    ModelTransfer = 0x0032,
    /// Transfer ontology
    OntologyTransfer = 0x0033,

    // Reasoning
    /// Query request
    Query = 0x0040,
    /// Query response
    QueryResponse = 0x0041,
    /// Inference request
    InferenceRequest = 0x0042,
    /// Inference response
    InferenceResponse = 0x0043,

    // Consensus
    /// Proposal
    Proposal = 0x0050,
    /// Vote
    Vote = 0x0051,
    /// Commit
    Commit = 0x0052,
    /// Abort
    Abort = 0x0053,

    // Streaming
    /// Start stream
    StreamStart = 0x0060,
    /// Stream data chunk
    StreamData = 0x0061,
    /// End stream
    StreamEnd = 0x0062,
}

bitflags! {
    /// Flags that modify message behavior.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct MessageFlags: u32 {
        /// No flags
        const NONE = 0x00;
        /// Payload is LZ4 compressed
        const COMPRESSED = 0x01;
        /// Payload is encrypted
        const ENCRYPTED = 0x02;
        /// Message has signature
        const SIGNED = 0x04;
        /// High priority
        const PRIORITY = 0x08;
        /// Sender expects acknowledgment
        const REQUIRES_ACK = 0x10;
        /// Part of a stream
        const STREAMING = 0x20;
        /// Has tensor attachments
        const HAS_TENSORS = 0x40;
        /// Broadcast to all agents
        const BROADCAST = 0x80;
    }
}

/// Fixed-size message header (32 bytes).
///
/// # Examples
///
/// ```
/// use rmi::core::protocol::{MessageHeader, MessageType, MessageFlags, HEADER_SIZE};
///
/// let hdr = MessageHeader::new(MessageType::Heartbeat, MessageFlags::NONE, 0);
/// let bytes = hdr.to_bytes();
/// assert_eq!(bytes.len(), HEADER_SIZE);
///
/// let decoded = MessageHeader::from_bytes(&bytes).unwrap();
/// assert_eq!(decoded.message_type, MessageType::Heartbeat as u16);
/// ```
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MessageHeader {
    /// Magic bytes
    pub magic: [u8; 4],
    /// Protocol version
    pub version: u16,
    /// Message type
    pub message_type: u16,
    /// Flags
    pub flags: u32,
    /// Payload length
    pub payload_length: u64,
    /// XXH64 checksum
    pub checksum: u64,
    /// Reserved for future use
    pub reserved: u32,
}

impl MessageHeader {
    /// Create a new header.
    pub fn new(message_type: MessageType, flags: MessageFlags, payload_length: u64) -> Self {
        Self {
            magic: PROTOCOL_MAGIC,
            version: PROTOCOL_VERSION,
            message_type: message_type as u16,
            flags: flags.bits(),
            payload_length,
            checksum: 0,
            reserved: 0,
        }
    }

    /// Serialize to bytes (exactly 32 bytes).
    #[inline]
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut bytes = [0u8; HEADER_SIZE];
        bytes[0..4].copy_from_slice(&self.magic);
        bytes[4..6].copy_from_slice(&self.version.to_be_bytes());
        bytes[6..8].copy_from_slice(&self.message_type.to_be_bytes());
        bytes[8..12].copy_from_slice(&self.flags.to_be_bytes());
        bytes[12..20].copy_from_slice(&self.payload_length.to_be_bytes());
        bytes[20..28].copy_from_slice(&self.checksum.to_be_bytes());
        bytes[28..32].copy_from_slice(&self.reserved.to_be_bytes());
        bytes
    }

    /// Deserialize from bytes.
    #[inline]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_SIZE {
            return Err(RmiError::protocol_simple(format!(
                "Header too short: {} < {}",
                bytes.len(),
                HEADER_SIZE
            )));
        }

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[0..4]);

        if magic != PROTOCOL_MAGIC {
            return Err(RmiError::protocol_simple(format!(
                "Invalid magic: {:?}",
                magic
            )));
        }

        Ok(Self {
            magic,
            version: u16::from_be_bytes([bytes[4], bytes[5]]),
            message_type: u16::from_be_bytes([bytes[6], bytes[7]]),
            flags: u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            payload_length: u64::from_be_bytes([
                bytes[12], bytes[13], bytes[14], bytes[15], bytes[16], bytes[17], bytes[18],
                bytes[19],
            ]),
            checksum: u64::from_be_bytes([
                bytes[20], bytes[21], bytes[22], bytes[23], bytes[24], bytes[25], bytes[26],
                bytes[27],
            ]),
            reserved: u32::from_be_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]),
        })
    }

    /// Get message type enum.
    #[inline]
    pub fn get_message_type(&self) -> Option<MessageType> {
        match self.message_type {
            0x0001 => Some(MessageType::Handshake),
            0x0002 => Some(MessageType::HandshakeAck),
            0x0003 => Some(MessageType::Heartbeat),
            0x0004 => Some(MessageType::Disconnect),
            0x0010 => Some(MessageType::CapabilityQuery),
            0x0011 => Some(MessageType::CapabilityResponse),
            0x0012 => Some(MessageType::AgentDiscovery),
            0x0013 => Some(MessageType::AgentAnnounce),
            0x0020 => Some(MessageType::TaskRequest),
            0x0021 => Some(MessageType::TaskAccept),
            0x0022 => Some(MessageType::TaskReject),
            0x0023 => Some(MessageType::TaskProgress),
            0x0024 => Some(MessageType::TaskComplete),
            0x0025 => Some(MessageType::TaskCancel),
            0x0030 => Some(MessageType::TensorTransfer),
            0x0031 => Some(MessageType::GradientTransfer),
            0x0032 => Some(MessageType::ModelTransfer),
            0x0033 => Some(MessageType::OntologyTransfer),
            0x0040 => Some(MessageType::Query),
            0x0041 => Some(MessageType::QueryResponse),
            0x0042 => Some(MessageType::InferenceRequest),
            0x0043 => Some(MessageType::InferenceResponse),
            0x0050 => Some(MessageType::Proposal),
            0x0051 => Some(MessageType::Vote),
            0x0052 => Some(MessageType::Commit),
            0x0053 => Some(MessageType::Abort),
            0x0060 => Some(MessageType::StreamStart),
            0x0061 => Some(MessageType::StreamData),
            0x0062 => Some(MessageType::StreamEnd),
            _ => None,
        }
    }

    /// Get flags.
    #[inline]
    pub fn get_flags(&self) -> MessageFlags {
        MessageFlags::from_bits_truncate(self.flags)
    }
}

/// Tensor attachment for efficient tensor transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorAttachment {
    /// Name/identifier
    pub name: String,
    /// Shape
    pub shape: Vec<usize>,
    /// Data type (e.g., "f32", "f64", "i32")
    pub dtype: String,
    /// Raw binary data
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

impl TensorAttachment {
    /// Create from ndarray.
    pub fn from_array_f32(name: &str, array: &ndarray::ArrayD<f32>) -> Self {
        let shape = array.shape().to_vec();
        let data: Vec<u8> = array.iter().flat_map(|f| f.to_le_bytes()).collect();

        Self {
            name: name.to_string(),
            shape,
            dtype: "f32".to_string(),
            data,
        }
    }

    /// Convert to ndarray.
    pub fn to_array_f32(&self) -> Result<ndarray::ArrayD<f32>> {
        if self.dtype != "f32" {
            return Err(RmiError::protocol_simple(format!(
                "Expected f32, got {}",
                self.dtype
            )));
        }

        let values: Vec<f32> = self
            .data
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        ndarray::ArrayD::from_shape_vec(ndarray::IxDyn(&self.shape), values)
            .map_err(|e| RmiError::protocol_simple(e.to_string()))
    }

    /// Serialize to binary.
    pub fn to_binary(&self) -> Vec<u8> {
        let meta = rmp_serde::to_vec(&TensorMeta {
            name: self.name.clone(),
            shape: self.shape.clone(),
            dtype: self.dtype.clone(),
        })
        .unwrap_or_default();

        let mut result = Vec::with_capacity(4 + meta.len() + 8 + self.data.len());
        result.extend_from_slice(&(meta.len() as u32).to_le_bytes());
        result.extend_from_slice(&meta);
        result.extend_from_slice(&(self.data.len() as u64).to_le_bytes());
        result.extend_from_slice(&self.data);
        result
    }

    /// Deserialize from binary.
    pub fn from_binary(data: &[u8]) -> Result<(Self, usize)> {
        if data.len() < 4 {
            return Err(RmiError::protocol_simple("Data too short"));
        }

        let meta_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let meta_start = 4;
        let meta_end = meta_start + meta_len;

        if data.len() < meta_end + 8 {
            return Err(RmiError::protocol_simple("Data too short for meta"));
        }

        let meta: TensorMeta = rmp_serde::from_slice(&data[meta_start..meta_end])
            .map_err(|e| RmiError::Serialization(e.to_string()))?;

        let data_len = u64::from_le_bytes([
            data[meta_end],
            data[meta_end + 1],
            data[meta_end + 2],
            data[meta_end + 3],
            data[meta_end + 4],
            data[meta_end + 5],
            data[meta_end + 6],
            data[meta_end + 7],
        ]) as usize;

        let data_start = meta_end + 8;
        let data_end = data_start + data_len;

        if data.len() < data_end {
            return Err(RmiError::protocol_simple("Data too short for tensor"));
        }

        Ok((
            Self {
                name: meta.name,
                shape: meta.shape,
                dtype: meta.dtype,
                data: data[data_start..data_end].to_vec(),
            },
            data_end,
        ))
    }
}

#[derive(Serialize, Deserialize)]
struct TensorMeta {
    name: String,
    shape: Vec<usize>,
    dtype: String,
}

/// A complete message in the protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Sender agent ID
    pub sender_id: Uuid,
    /// Recipient agent ID
    pub recipient_id: Uuid,
    /// Message type
    pub message_type: MessageType,
    /// Payload bytes
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
    /// Timestamp (Unix epoch seconds)
    pub timestamp: f64,
    /// Correlation ID for request-response pairing
    pub correlation_id: Option<Uuid>,
    /// Priority (0-10)
    pub priority: u8,
    /// Time-to-live in seconds
    pub ttl_seconds: f64,
    /// Tensor attachments
    #[serde(skip)]
    pub tensors: Vec<TensorAttachment>,
}

impl Message {
    /// Create a new message.
    pub fn new(
        sender_id: Uuid,
        recipient_id: Uuid,
        message_type: MessageType,
        payload: Vec<u8>,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        Self {
            sender_id,
            recipient_id,
            message_type,
            payload,
            timestamp,
            correlation_id: None,
            priority: 5,
            ttl_seconds: 60.0,
            tensors: Vec::new(),
        }
    }

    /// Add a tensor attachment.
    pub fn with_tensor(mut self, tensor: TensorAttachment) -> Self {
        self.tensors.push(tensor);
        self
    }

    /// Set correlation ID.
    #[inline]
    pub fn with_correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Set priority.
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority.min(10);
        self
    }

    /// Set TTL.
    pub fn with_ttl(mut self, seconds: f64) -> Self {
        self.ttl_seconds = seconds;
        self
    }

    /// Check if message has expired.
    #[inline]
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        now > self.timestamp + self.ttl_seconds
    }

    /// Serialize to binary format.
    pub fn to_binary(&self) -> Vec<u8> {
        // Serialize payload
        let payload_data = rmp_serde::to_vec(&MessagePayload {
            sender_id: self.sender_id,
            recipient_id: self.recipient_id,
            message_type: self.message_type,
            payload: self.payload.clone(),
            timestamp: self.timestamp,
            correlation_id: self.correlation_id,
            priority: self.priority,
            ttl_seconds: self.ttl_seconds,
        })
        .unwrap_or_default();

        // Compress
        let compressed = lz4_flex::compress_prepend_size(&payload_data);

        // Build tensor attachments with capacity hint
        let tensor_capacity: usize = self.tensors.iter().map(|t| t.data.len() + 64).sum();
        let mut tensor_bytes = Vec::with_capacity(tensor_capacity);
        for tensor in &self.tensors {
            tensor_bytes.extend_from_slice(&tensor.to_binary());
        }

        // Calculate checksum via streaming hash (avoids cloning compressed buffer)
        let compressed_len_for_hash = (compressed.len() as u64).to_le_bytes();
        let checksum = {
            let mut hasher = Xxh64::new(0);
            hasher.write(&compressed_len_for_hash);
            hasher.write(&compressed);
            hasher.write(&tensor_bytes);
            hasher.finish()
        };

        // Build flags
        let mut flags = MessageFlags::COMPRESSED;
        if !self.tensors.is_empty() {
            flags |= MessageFlags::HAS_TENSORS;
        }
        if self.priority > 5 {
            flags |= MessageFlags::PRIORITY;
        }

        // Build header
        let mut header = MessageHeader::new(
            self.message_type,
            flags,
            (8 + compressed.len() + tensor_bytes.len()) as u64,
        );
        header.checksum = checksum;

        // Assemble final message
        // Format: [header][compressed_len: u64 LE][compressed][tensor_bytes]
        let compressed_len_bytes = (compressed.len() as u64).to_le_bytes();
        let mut result = Vec::with_capacity(HEADER_SIZE + 8 + compressed.len() + tensor_bytes.len());
        result.extend_from_slice(&header.to_bytes());
        result.extend_from_slice(&compressed_len_bytes);
        result.extend_from_slice(&compressed);
        result.extend_from_slice(&tensor_bytes);
        result
    }

    /// Deserialize from binary.
    pub fn from_binary(data: &[u8]) -> Result<Self> {
        // Parse header
        let header = MessageHeader::from_bytes(data)?;
        let flags = header.get_flags();

        // Extract payload section: [compressed_len: u64][compressed][tensor_bytes]
        let payload_start = HEADER_SIZE;
        let payload_end = HEADER_SIZE + header.payload_length as usize;

        if data.len() < payload_end {
            return Err(RmiError::protocol_simple("Data too short"));
        }

        let payload_section = &data[payload_start..payload_end];

        // Verify checksum over entire payload section
        let actual_checksum = xxh64(payload_section, 0);
        if actual_checksum != header.checksum {
            return Err(RmiError::protocol_simple("Checksum mismatch"));
        }

        // Read compressed_len prefix (8 bytes LE u64)
        if payload_section.len() < 8 {
            return Err(RmiError::protocol_simple("Payload too short for compressed_len"));
        }
        let compressed_len = u64::from_le_bytes([
            payload_section[0], payload_section[1], payload_section[2], payload_section[3],
            payload_section[4], payload_section[5], payload_section[6], payload_section[7],
        ]) as usize;

        let compressed_start = 8;
        let compressed_end = compressed_start + compressed_len;

        if payload_section.len() < compressed_end {
            return Err(RmiError::protocol_simple("Payload too short for compressed data"));
        }

        let compressed_section = &payload_section[compressed_start..compressed_end];
        let tensor_section = &payload_section[compressed_end..];

        // Decompress the message payload
        let decompressed = if flags.contains(MessageFlags::COMPRESSED) {
            lz4_flex::decompress_size_prepended(compressed_section)
                .map_err(|e| RmiError::Serialization(e.to_string()))?
        } else {
            compressed_section.to_vec()
        };

        // Deserialize message payload
        let msg_payload: MessagePayload = rmp_serde::from_slice(&decompressed)
            .map_err(|e| RmiError::Serialization(e.to_string()))?;

        // Parse tensor attachments if present
        let mut tensors = Vec::new();
        if flags.contains(MessageFlags::HAS_TENSORS) && !tensor_section.is_empty() {
            let mut offset = 0;
            while offset < tensor_section.len() {
                match TensorAttachment::from_binary(&tensor_section[offset..]) {
                    Ok((tensor, consumed)) => {
                        tensors.push(tensor);
                        offset += consumed;
                    }
                    Err(_) => break,
                }
            }
        }

        Ok(Self {
            sender_id: msg_payload.sender_id,
            recipient_id: msg_payload.recipient_id,
            message_type: msg_payload.message_type,
            payload: msg_payload.payload,
            timestamp: msg_payload.timestamp,
            correlation_id: msg_payload.correlation_id,
            priority: msg_payload.priority,
            ttl_seconds: msg_payload.ttl_seconds,
            tensors,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct MessagePayload {
    sender_id: Uuid,
    recipient_id: Uuid,
    message_type: MessageType,
    #[serde(with = "serde_bytes")]
    payload: Vec<u8>,
    timestamp: f64,
    correlation_id: Option<Uuid>,
    priority: u8,
    ttl_seconds: f64,
}

/// Schema for message payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSchema {
    /// Schema name
    pub name: String,
    /// Field definitions
    pub fields: HashMap<String, String>,
    /// Required fields
    pub required: Vec<String>,
    /// Schema version (content-addressable)
    version: String,
}

impl MessageSchema {
    /// Create a new schema.
    pub fn new(name: &str, fields: HashMap<String, String>, required: Vec<String>) -> Self {
        let mut schema = Self {
            name: name.to_string(),
            fields,
            required,
            version: String::new(),
        };
        schema.version = schema.compute_version();
        schema
    }

    fn compute_version(&self) -> String {
        let data =
            rmp_serde::to_vec(&(&self.name, &self.fields, &self.required)).unwrap_or_default();
        let hash = xxh64(&data, 0);
        format!("{:016x}", hash)[..8].to_string()
    }

    /// Get schema version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Validate data against schema.
    pub fn validate(&self, data: &HashMap<String, serde_json::Value>) -> bool {
        // Check required fields
        for field in &self.required {
            if !data.contains_key(field) {
                return false;
            }
        }
        true
    }

    /// Serialize to binary.
    pub fn to_binary(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).unwrap_or_default()
    }

    /// Deserialize from binary.
    pub fn from_binary(data: &[u8]) -> Result<Self> {
        rmp_serde::from_slice(data).map_err(|e| RmiError::Serialization(e.to_string()))
    }
}

/// Protocol configuration.
#[derive(Debug, Clone)]
pub struct Protocol {
    /// Compression algorithm
    pub compression: String,
    /// Encryption algorithm (optional)
    pub encryption: Option<String>,
    /// Schema format
    pub schema_format: String,
    /// Maximum message size
    pub max_message_size: usize,
    /// Stream chunk size
    pub stream_chunk_size: usize,
    /// Registered schemas
    schemas: HashMap<String, MessageSchema>,
}

impl Protocol {
    /// Create a new protocol with default settings.
    pub fn new() -> Self {
        Self {
            compression: "lz4".to_string(),
            encryption: None,
            schema_format: "msgpack".to_string(),
            max_message_size: 100 * 1024 * 1024, // 100MB
            stream_chunk_size: 64 * 1024,        // 64KB
            schemas: HashMap::new(),
        }
    }

    /// Create a binary protocol.
    pub fn binary() -> Self {
        Self::new()
    }

    /// Create a secure protocol with encryption.
    pub fn secure(encryption: &str) -> Self {
        let mut protocol = Self::new();
        protocol.encryption = Some(encryption.to_string());
        protocol
    }

    /// Register a schema.
    pub fn register_schema(&mut self, schema: MessageSchema) {
        self.schemas.insert(schema.name.clone(), schema);
    }

    /// Get a schema.
    pub fn get_schema(&self, name: &str) -> Option<&MessageSchema> {
        self.schemas.get(name)
    }

    /// Create a message.
    pub fn create_message(
        &self,
        sender_id: Uuid,
        recipient_id: Uuid,
        message_type: MessageType,
        payload: HashMap<String, serde_json::Value>,
    ) -> Result<Message> {
        let payload_bytes = rmp_serde::to_vec(&payload)
            .map_err(|e| RmiError::Serialization(e.to_string()))?;

        Ok(Message::new(
            sender_id,
            recipient_id,
            message_type,
            payload_bytes,
        ))
    }
}

impl Default for Protocol {
    fn default() -> Self {
        Self::new()
    }
}

/// Standard schemas
pub mod schemas {
    use super::*;

    /// Handshake schema.
    pub fn handshake() -> MessageSchema {
        let mut fields = HashMap::new();
        fields.insert("agent_id".to_string(), "uuid".to_string());
        fields.insert("capabilities".to_string(), "list[u16]".to_string());
        fields.insert("protocol_version".to_string(), "u16".to_string());
        fields.insert("public_key".to_string(), "bytes?".to_string());

        MessageSchema::new(
            "handshake",
            fields,
            vec![
                "agent_id".to_string(),
                "capabilities".to_string(),
                "protocol_version".to_string(),
            ],
        )
    }

    /// Task request schema.
    pub fn task_request() -> MessageSchema {
        let mut fields = HashMap::new();
        fields.insert("task_id".to_string(), "string".to_string());
        fields.insert("goal_type".to_string(), "string".to_string());
        fields.insert("goal_spec".to_string(), "bytes".to_string());
        fields.insert("constraints".to_string(), "map".to_string());
        fields.insert("deadline".to_string(), "f64?".to_string());

        MessageSchema::new(
            "task_request",
            fields,
            vec![
                "task_id".to_string(),
                "goal_type".to_string(),
                "goal_spec".to_string(),
            ],
        )
    }

    /// Tensor transfer schema.
    pub fn tensor_transfer() -> MessageSchema {
        let mut fields = HashMap::new();
        fields.insert("tensor_id".to_string(), "string".to_string());
        fields.insert("shape".to_string(), "list[usize]".to_string());
        fields.insert("dtype".to_string(), "string".to_string());
        fields.insert("compression".to_string(), "string?".to_string());
        fields.insert("checksum".to_string(), "string".to_string());

        MessageSchema::new(
            "tensor_transfer",
            fields,
            vec![
                "tensor_id".to_string(),
                "shape".to_string(),
                "dtype".to_string(),
                "checksum".to_string(),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_roundtrip() {
        let header = MessageHeader::new(MessageType::Query, MessageFlags::COMPRESSED, 1024);
        let bytes = header.to_bytes();
        let restored = MessageHeader::from_bytes(&bytes).unwrap();

        assert_eq!(header.version, restored.version);
        assert_eq!(header.message_type, restored.message_type);
        assert_eq!(header.payload_length, restored.payload_length);
    }

    #[test]
    fn test_message_roundtrip() {
        let sender = Uuid::new_v4();
        let recipient = Uuid::new_v4();

        let msg = Message::new(sender, recipient, MessageType::Query, vec![1, 2, 3, 4])
            .with_priority(8)
            .with_ttl(120.0);

        let binary = msg.to_binary();
        let restored = Message::from_binary(&binary).unwrap();

        assert_eq!(msg.sender_id, restored.sender_id);
        assert_eq!(msg.recipient_id, restored.recipient_id);
        assert_eq!(msg.payload, restored.payload);
    }

    #[test]
    fn test_tensor_attachment() {
        let data: ndarray::ArrayD<f32> = ndarray::ArrayD::from_shape_vec(
            ndarray::IxDyn(&[2, 3]),
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        )
        .unwrap();

        let attachment = TensorAttachment::from_array_f32("test", &data);
        let binary = attachment.to_binary();
        let (restored, _) = TensorAttachment::from_binary(&binary).unwrap();

        assert_eq!(attachment.name, restored.name);
        assert_eq!(attachment.shape, restored.shape);
        assert_eq!(attachment.data, restored.data);
    }

    #[test]
    fn test_message_with_tensor_roundtrip() {
        let sender = Uuid::new_v4();
        let recipient = Uuid::new_v4();

        let tensor_data = ndarray::ArrayD::from_shape_vec(
            ndarray::IxDyn(&[2, 3]),
            vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0],
        )
        .unwrap();

        let tensor = TensorAttachment::from_array_f32("weights", &tensor_data);

        let msg = Message::new(sender, recipient, MessageType::TensorTransfer, vec![42])
            .with_tensor(tensor)
            .with_priority(8);

        let binary = msg.to_binary();
        let restored = Message::from_binary(&binary).unwrap();

        assert_eq!(restored.sender_id, sender);
        assert_eq!(restored.recipient_id, recipient);
        assert_eq!(restored.message_type, MessageType::TensorTransfer);
        assert_eq!(restored.payload, vec![42]);
        assert_eq!(restored.priority, 8);
        assert_eq!(restored.tensors.len(), 1);

        let t = &restored.tensors[0];
        assert_eq!(t.name, "weights");
        assert_eq!(t.shape, vec![2, 3]);
        assert_eq!(t.dtype, "f32");

        let arr = t.to_array_f32().unwrap();
        assert_eq!(arr.shape(), &[2, 3]);
        assert_eq!(arr.as_slice().unwrap(), &[1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_message_with_multiple_tensors_roundtrip() {
        let sender = Uuid::new_v4();
        let recipient = Uuid::new_v4();

        let t1 = TensorAttachment::from_array_f32(
            "layer1",
            &ndarray::ArrayD::from_shape_vec(ndarray::IxDyn(&[3]), vec![1.0f32, 2.0, 3.0]).unwrap(),
        );
        let t2 = TensorAttachment::from_array_f32(
            "layer2",
            &ndarray::ArrayD::from_shape_vec(ndarray::IxDyn(&[2, 2]), vec![10.0f32, 20.0, 30.0, 40.0]).unwrap(),
        );

        let msg = Message::new(sender, recipient, MessageType::GradientTransfer, vec![])
            .with_tensor(t1)
            .with_tensor(t2);

        let binary = msg.to_binary();
        let restored = Message::from_binary(&binary).unwrap();

        assert_eq!(restored.tensors.len(), 2);
        assert_eq!(restored.tensors[0].name, "layer1");
        assert_eq!(restored.tensors[0].shape, vec![3]);
        assert_eq!(restored.tensors[1].name, "layer2");
        assert_eq!(restored.tensors[1].shape, vec![2, 2]);

        let arr1 = restored.tensors[0].to_array_f32().unwrap();
        assert_eq!(arr1.as_slice().unwrap(), &[1.0f32, 2.0, 3.0]);
        let arr2 = restored.tensors[1].to_array_f32().unwrap();
        assert_eq!(arr2.as_slice().unwrap(), &[10.0f32, 20.0, 30.0, 40.0]);
    }


    #[test]
    fn test_header_magic_and_version() {
        let header = MessageHeader::new(MessageType::Handshake, MessageFlags::NONE, 0);
        let bytes = header.to_bytes();
        assert_eq!(&bytes[..4], b"FWRX");
        assert_eq!(u16::from_be_bytes([bytes[4], bytes[5]]), PROTOCOL_VERSION);
    }

    #[test]
    fn test_header_invalid_magic() {
        let mut bytes = [0u8; HEADER_SIZE];
        bytes[..4].copy_from_slice(b"XXXX");
        let result = MessageHeader::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_header_too_short() {
        let bytes = [0u8; 10];
        let result = MessageHeader::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_flags_combinations() {
        let flags = MessageFlags::COMPRESSED | MessageFlags::ENCRYPTED;
        assert!(flags.contains(MessageFlags::COMPRESSED));
        assert!(flags.contains(MessageFlags::ENCRYPTED));
        assert!(!flags.contains(MessageFlags::REQUIRES_ACK));
    }

    #[test]
    fn test_message_priority() {
        let msg = Message::new(Uuid::new_v4(), Uuid::new_v4(), MessageType::Query, vec![])
            .with_priority(5);
        assert_eq!(msg.priority, 5);
    }

    #[test]
    fn test_message_correlation_id() {
        let corr = Uuid::new_v4();
        let msg = Message::new(Uuid::new_v4(), Uuid::new_v4(), MessageType::Query, vec![])
            .with_correlation_id(corr);
        assert_eq!(msg.correlation_id, Some(corr));
    }

    #[test]
    fn test_message_ttl_not_expired() {
        let msg = Message::new(Uuid::new_v4(), Uuid::new_v4(), MessageType::Heartbeat, vec![])
            .with_ttl(3600.0);
        assert!(!msg.is_expired());
    }

    #[test]
    fn test_message_no_ttl_never_expires() {
        let msg = Message::new(Uuid::new_v4(), Uuid::new_v4(), MessageType::Heartbeat, vec![]);
        assert!(!msg.is_expired());
    }

    #[test]
    fn test_protocol_new_defaults() {
        let p = Protocol::new();
        assert_eq!(p.compression, "lz4");
        assert!(p.encryption.is_none());
        assert_eq!(p.max_message_size, 100 * 1024 * 1024);
    }

    #[test]
    fn test_protocol_secure() {
        let p = Protocol::secure("aes-256-gcm");
        assert_eq!(p.encryption.as_deref(), Some("aes-256-gcm"));
    }

    #[test]
    fn test_protocol_register_and_get_schema() {
        let mut p = Protocol::new();
        let schema = MessageSchema::new(
            "test",
            HashMap::from([("field1".into(), "string".into())]),
            vec!["field1".into()],
        );
        p.register_schema(schema);
        assert!(p.get_schema("test").is_some());
        assert!(p.get_schema("nonexistent").is_none());
    }

    #[test]
    fn test_schema_validate_required() {
        let schema = MessageSchema::new(
            "test",
            HashMap::from([("name".into(), "string".into()), ("age".into(), "int".into())]),
            vec!["name".into()],
        );
        let valid = HashMap::from([("name".into(), serde_json::json!("alice"))]);
        assert!(schema.validate(&valid));
        let invalid: HashMap<String, serde_json::Value> = HashMap::from([("age".into(), serde_json::json!(30))]);
        assert!(!schema.validate(&invalid));
    }

    #[test]
    fn test_schema_version_deterministic() {
        let s1 = MessageSchema::new("x", HashMap::from([("a".into(), "b".into())]), vec![]);
        let s2 = MessageSchema::new("x", HashMap::from([("a".into(), "b".into())]), vec![]);
        assert_eq!(s1.version(), s2.version());
        assert!(!s1.version().is_empty());
    }

    #[test]
    fn test_schema_binary_roundtrip() {
        let schema = MessageSchema::new(
            "round",
            HashMap::from([("f".into(), "float".into())]),
            vec!["f".into()],
        );
        let binary = schema.to_binary();
        let restored = MessageSchema::from_binary(&binary).unwrap();
        assert_eq!(schema.name, restored.name);
        assert_eq!(schema.version(), restored.version());
    }

    #[test]
    fn test_message_empty_payload_roundtrip() {
        let s = Uuid::new_v4();
        let r = Uuid::new_v4();
        let msg = Message::new(s, r, MessageType::Heartbeat, vec![]);
        let binary = msg.to_binary();
        let restored = Message::from_binary(&binary).unwrap();
        assert_eq!(restored.sender_id, s);
        assert_eq!(restored.payload, Vec::<u8>::new());
        assert!(restored.tensors.is_empty());
    }

    #[test]
    fn test_header_get_message_type() {
        let header = MessageHeader::new(MessageType::TaskRequest, MessageFlags::NONE, 0);
        assert_eq!(header.get_message_type(), Some(MessageType::TaskRequest));
    }

    #[test]
    fn test_header_get_flags() {
        let flags = MessageFlags::COMPRESSED | MessageFlags::REQUIRES_ACK;
        let header = MessageHeader::new(MessageType::Query, flags, 100);
        assert_eq!(header.get_flags(), flags);
    }
}
