# RecursiveMachineIntelligence Binary Protocol Specification

**Version:** 1.0  
**Status:** Draft

---

## Overview

The RecursiveMachineIntelligence protocol is a binary message format designed for efficient communication between AI agents. It supports structured messages with tensor attachments, using MessagePack serialization with LZ4 compression.

---

## Design Principles

1. **Machine-First**: Designed for agent communication, not human readability
2. **Efficiency**: Compact binary format with optional compression
3. **Extensibility**: Version field and payload flexibility for evolution
4. **Self-Describing**: Headers contain enough information to parse messages

---

## Message Format

All header fields are **big-endian**. Offsets are from the start of the frame.

```
┌────────────────────────────────────────────────────────────────┐
│                       HEADER (32 bytes)                        │
├─────────────┬──────────────┬───────────────────────────────────┤
│ 0..4        │ magic        │ "FWRX" (0x46 0x57 0x52 0x58)      │
│ 4..6        │ version      │ u16, currently 1                  │
│ 6..8        │ message_type │ u16                               │
│ 8..12       │ flags        │ u32 (see below)                   │
│ 12..20      │ payload_len  │ u64 — 8 + payload + tensor bytes  │
│ 20..28      │ checksum     │ u64, XXH64 (see Checksum)         │
│ 28..32      │ reserved     │ u32, zero                         │
├─────────────┴──────────────┴───────────────────────────────────┤
│                        BODY (variable)                         │
├─────────────┬──────────────┬───────────────────────────────────┤
│ 32..40      │ payload_len  │ u64 **little-endian**             │
│ 40..        │ payload      │ MessagePack, LZ4 when COMPRESSED  │
│ …           │ tensors      │ concatenated tensor attachments   │
└─────────────┴──────────────┴───────────────────────────────────┘
```

> **Corrected 2026-08-18.** The previous diagram matched the implementation in
> almost no respect, and a client written from it fails at byte 4:
>
> - **The magic was `FRWX`; it is `FWRX`.** The W and R are transposed, and the
>   hex given alongside spelled out the same wrong order, so the two agreed
>   with each other and not with the code.
> - `flags` and `message_type` were the wrong widths *and* the wrong way round.
> - **`MsgId` and `Timestamp` are not in the header.** Those 16 bytes are
>   `payload_length` and `checksum`.
> - **The checksum is inside the header, not a 4-byte trailer.** There is no
>   trailer.
> - There is **no sender-ID section** and no attachment count; the body is a
>   little-endian length followed by the payload and then tensor bytes.
>
> Every one of those is a silent interop failure rather than a compile error,
> which is what makes a wrong wire-format diagram worse than a missing one.

---

## Header Flags

| Bit  | Name         | Description                    |
| ---- | ------------ | ------------------------------ |
| 0    | COMPRESSED   | Payload is LZ4 compressed      |
| 1    | ENCRYPTED    | Payload is encrypted (future)  |
| 2    | FRAGMENTED   | Message is fragmented (future) |
| 3    | REQUIRES_ACK | Sender expects acknowledgment  |
| 4    | PRIORITY     | High-priority message          |
| 5-15 | Reserved     | Reserved for future use        |

---

## Message Types

| Code   | Name       | Description                           |
| ------ | ---------- | ------------------------------------- |
| 0x0001 | QUERY      | Request for information or inference  |
| 0x0002 | RESULT     | Response to a query                   |
| 0x0003 | GOAL       | Goal assignment message               |
| 0x0004 | TENSOR     | Tensor data transfer                  |
| 0x0005 | CAPABILITY | Capability advertisement              |
| 0x0006 | ACK        | Acknowledgment                        |
| 0x0007 | NACK       | Negative acknowledgment               |
| 0x0008 | HEARTBEAT  | Keep-alive signal                     |
| 0x0009 | STATE_SYNC | State synchronization                 |
| 0x000A | GRADIENT   | Gradient sharing (federated learning) |
| 0x000B | CHECKPOINT | Model checkpoint                      |

---

## Payload Schemas

### QUERY (0x0001)

```msgpack
{
  "predicate": {
    "name": string,
    "args": [Term]
  },
  "timeout_ms": ?uint,
  "max_results": ?uint,
  "mode": ?string  // "neural", "symbolic", "hybrid"
}
```

### RESULT (0x0002)

```msgpack
{
  "query_id": uint64,
  "success": bool,
  "results": [Substitution],
  "confidence": ?float,
  "explanations": ?[string]
}
```

### GOAL (0x0003)

```msgpack
{
  "goal_id": string,
  "goal_type": string,  // "minimize", "maximize", "satisfy", "achieve"
  "target": string,
  "constraints": {string: float},
  "priority": float,
  "deadline_ms": ?uint
}
```

### TENSOR (0x0004)

```msgpack
{
  "name": string,
  "description": ?string,
  "attachment_index": uint  // Index into attachments array
}
```

### CAPABILITY (0x0005)

```msgpack
{
  "capabilities": [
    {
      "name": string,
      "version": string,
      "parameters": {string: any}
    }
  ],
  "resources": {
    "compute_flops": ?float,
    "memory_bytes": ?uint,
    "gpu_available": ?bool
  }
}
```

---

## Tensor Attachment Format

```
┌─────────────────────────────────────────────────────────────┐
│                    TENSOR HEADER (24+ bytes)                 │
├──────────────┬──────────────────────────────────────────────┤
│ DType (1)    │ Data type code (see below)                   │
│ NDim (1)     │ Number of dimensions                         │
│ Flags (2)    │ Tensor flags                                 │
│ Shape (var)  │ NDim × 4 bytes, each dimension as uint32     │
│ Data Length  │ 4 bytes, length of raw data                  │
├──────────────┴──────────────────────────────────────────────┤
│                       TENSOR DATA                            │
├─────────────────────────────────────────────────────────────┤
│ Raw bytes in row-major (C) order                            │
└─────────────────────────────────────────────────────────────┘
```

### DType Codes

| Code | Type | Size    |
| ---- | ---- | ------- |
| 0x01 | F32  | 4 bytes |
| 0x02 | F64  | 8 bytes |
| 0x03 | F16  | 2 bytes |
| 0x04 | BF16 | 2 bytes |
| 0x05 | I32  | 4 bytes |
| 0x06 | I64  | 8 bytes |
| 0x07 | U8   | 1 byte  |
| 0x08 | Bool | 1 byte  |

### Tensor Flags

| Bit  | Name          | Description                       |
| ---- | ------------- | --------------------------------- |
| 0    | REQUIRES_GRAD | Tensor requires gradient tracking |
| 1    | COMPRESSED    | Data is LZ4 compressed            |
| 2    | SPARSE        | Sparse tensor format (future)     |
| 3-15 | Reserved      | Reserved                          |

---

## Term Encoding

Logical terms use tagged encoding:

```msgpack
// Variable
{"type": "var", "name": string}

// Symbol
{"type": "sym", "name": string}

// Function
{"type": "fn", "name": string, "args": [Term]}

// List
{"type": "list", "items": [Term]}
```

---

## Substitution Encoding

```msgpack
{
  "bindings": {
    "VarName1": Term,
    "VarName2": Term
  }
}
```

---

## Example Messages

### Query Example

```
Header:
  Magic: FRWX
  Version: 1.0
  Flags: 0x0001 (COMPRESSED)
  Type: 0x0001 (QUERY)
  Length: 156
  MsgId: 123456789
  Timestamp: 1699574400000000

Sender: "agent-reasoning-01"

Payload (MessagePack, compressed):
{
  "predicate": {
    "name": "similar_to",
    "args": [
      {"type": "sym", "name": "concept_attention"},
      {"type": "var", "name": "X"}
    ]
  },
  "timeout_ms": 5000,
  "max_results": 10,
  "mode": "hybrid"
}

Attachments: []
```

### Result with Tensor

```
Header:
  Magic: FRWX
  Version: 1.0
  Flags: 0x0001 (COMPRESSED)
  Type: 0x0002 (RESULT)
  Length: 2048
  MsgId: 123456790
  Timestamp: 1699574400500000

Sender: "agent-embedding-01"

Payload:
{
  "query_id": 123456789,
  "success": true,
  "results": [
    {"bindings": {"X": {"type": "sym", "name": "self_attention"}}},
    {"bindings": {"X": {"type": "sym", "name": "cross_attention"}}}
  ],
  "confidence": 0.95
}

Attachments: [
  {
    DType: 0x01 (F32)
    NDim: 2
    Shape: [2, 512]
    Data: <4096 bytes of embedding vectors>
  }
]
```

---

## Error Handling

### NACK Payload

```msgpack
{
  "original_id": uint64,
  "error_code": uint32,
  "error_message": string
}
```

### Error Codes

| Code | Name               | Description              |
| ---- | ------------------ | ------------------------ |
| 1001 | MALFORMED          | Malformed message        |
| 1002 | UNKNOWN_TYPE       | Unknown message type     |
| 1003 | DECODE_ERROR       | Failed to decode payload |
| 1004 | TIMEOUT            | Operation timed out      |
| 1005 | UNSUPPORTED        | Unsupported operation    |
| 1006 | RESOURCE_EXHAUSTED | Out of resources         |
| 1007 | INFERENCE_FAILED   | Inference failed         |

---

## Compression

When the COMPRESSED flag is set:

1. Payload is compressed using LZ4
2. Tensor attachments may be individually compressed (per tensor flag)
3. Header and checksums are never compressed

**LZ4 Frame Format:**

```
┌─────────────────────────────────────────┐
│ Uncompressed size (4 bytes, little-end) │
│ Compressed data                         │
└─────────────────────────────────────────┘
```

---

## Implementation Notes

### Rust Encoding

```rust
use rmp_serde::{encode, decode};
use lz4_flex::{compress_prepend_size, decompress_size_prepended};

pub fn encode_payload<T: Serialize>(payload: &T, compress: bool) -> Result<Vec<u8>> {
    let bytes = encode::to_vec_named(payload)?;
    if compress {
        Ok(compress_prepend_size(&bytes))
    } else {
        Ok(bytes)
    }
}

pub fn decode_payload<T: DeserializeOwned>(data: &[u8], compressed: bool) -> Result<T> {
    let bytes = if compressed {
        decompress_size_prepended(data)?
    } else {
        data.to_vec()
    };
    Ok(decode::from_slice(&bytes)?)
}
```

### Checksum

**XXH64** (seed 0), stored as a big-endian `u64` at header offset 20..28.
Computed over, in this order:

1. the compressed payload length, as **little-endian `u64`**
2. the compressed payload bytes
3. the serialized tensor bytes, if any

> **Corrected 2026-08-18.** This said "CRC32 (IEEE polynomial)" over the
> header, sender, payload and attachments. Every part was wrong, and the error
> is not cosmetic: an interoperable client written from the old text computes a
> **32-bit** digest over the **wrong bytes in the wrong order**, and the wire
> format carries a 64-bit field. Every message would fail validation, and the
> implementer would have no way to tell which of the four discrepancies was
> responsible. Note the mixed endianness — the header is big-endian throughout,
> while the length fed to the hasher is little-endian — because that is what
> `MessageEnvelope::to_binary` does and a client has to match it.

---

## Versioning

- Major version: Breaking changes to header format
- Minor version: Backward-compatible additions

Current: **v1.0**

---

*RecursiveMachineIntelligence Protocol Specification v1.0*
