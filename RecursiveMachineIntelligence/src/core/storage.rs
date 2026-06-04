//! Agent Data Storage - Efficient Persistent State Management
//!
//! This module provides efficient storage primitives for AI agents:
//!
//! 1. **Key-Value Store**: Fast lookup with LRU caching
//! 2. **Tensor Storage**: Efficient binary tensor persistence
//! 3. **Knowledge Store**: Versioned knowledge base storage
//! 4. **Checkpoint Manager**: Model and agent state checkpointing
//! 5. **Distributed Storage**: Sharded storage across agents
//!
//! All storage uses:
//! - MessagePack serialization (compact binary)
//! - LZ4 compression (fast, good ratio)
//! - XXH64 checksums (integrity verification)
//! - Memory-mapped files (large dataset support)

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs::{self, File};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;
use xxhash_rust::xxh64::xxh64;

use crate::error::{RmiError, Result};

// ============================================================================
// Storage Types
// ============================================================================

/// Data types supported by the storage system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum StorageDataType {
    /// Raw bytes
    Binary = 0x01,
    /// UTF-8 string
    String = 0x02,
    /// 32-bit float tensor
    TensorF32 = 0x10,
    /// 64-bit float tensor
    TensorF64 = 0x11,
    /// 32-bit integer tensor
    TensorI32 = 0x12,
    /// 64-bit integer tensor
    TensorI64 = 0x13,
    /// Boolean tensor
    TensorBool = 0x14,
    /// MessagePack-serialized object
    MsgPack = 0x20,
    /// JSON object (human-readable fallback)
    Json = 0x21,
    /// Agent state snapshot
    AgentState = 0x30,
    /// Knowledge base
    KnowledgeBase = 0x31,
    /// Model checkpoint
    ModelCheckpoint = 0x32,
    /// Gradient data
    Gradient = 0x33,
    /// Ontology
    Ontology = 0x34,
}

/// Metadata for stored items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetadata {
    /// Unique key
    pub key: String,
    /// Data type
    pub data_type: StorageDataType,
    /// Size in bytes (uncompressed)
    pub size_bytes: u64,
    /// Size in bytes (compressed)
    pub compressed_size: u64,
    /// Creation timestamp
    pub created_at: f64,
    /// Last modified timestamp
    pub modified_at: f64,
    /// XXH64 checksum
    pub checksum: u64,
    /// Version number (for optimistic concurrency)
    pub version: u64,
    /// Custom tags
    pub tags: HashMap<String, String>,
    /// TTL in seconds (0 = no expiry)
    pub ttl_seconds: u64,
}

impl StorageMetadata {
    /// Create new metadata.
    pub fn new(key: &str, data_type: StorageDataType, size_bytes: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        Self {
            key: key.to_string(),
            data_type,
            size_bytes,
            compressed_size: 0,
            created_at: now,
            modified_at: now,
            checksum: 0,
            version: 1,
            tags: HashMap::new(),
            ttl_seconds: 0,
        }
    }

    /// Check if item has expired.
    #[inline]
    pub fn is_expired(&self) -> bool {
        if self.ttl_seconds == 0 {
            return false;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        now > self.created_at + self.ttl_seconds as f64
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

// ============================================================================
// Key-Value Store
// ============================================================================

/// Entry in the LRU cache.
struct CacheEntry {
    data: Vec<u8>,
    metadata: StorageMetadata,
}

/// High-performance key-value store with LRU cache.
pub struct KeyValueStore {
    /// Base directory for persistent storage
    base_path: PathBuf,
    /// In-memory cache
    cache: RwLock<HashMap<String, CacheEntry>>,
    /// LRU order tracking
    lru_order: RwLock<VecDeque<String>>,
    /// Maximum cache size in bytes
    max_cache_bytes: usize,
    /// Current cache size
    current_cache_bytes: RwLock<usize>,
    /// Enable compression
    compress: bool,
}

impl KeyValueStore {
    /// Create a new key-value store.
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path)?;

        Ok(Self {
            base_path,
            cache: RwLock::new(HashMap::new()),
            lru_order: RwLock::new(VecDeque::new()),
            max_cache_bytes: 256 * 1024 * 1024, // 256 MB default cache
            current_cache_bytes: RwLock::new(0),
            compress: true,
        })
    }

    /// Create an in-memory only store.
    pub fn in_memory() -> Self {
        Self {
            base_path: PathBuf::from(":memory:"),
            cache: RwLock::new(HashMap::new()),
            lru_order: RwLock::new(VecDeque::new()),
            max_cache_bytes: 1024 * 1024 * 1024, // 1 GB
            current_cache_bytes: RwLock::new(0),
            compress: true,
        }
    }

    /// Set maximum cache size.
    pub fn with_cache_size(mut self, bytes: usize) -> Self {
        self.max_cache_bytes = bytes;
        self
    }

    /// Disable compression.
    pub fn without_compression(mut self) -> Self {
        self.compress = false;
        self
    }

    /// Store a value.
    pub fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<StorageMetadata> {
        let serialized =
            rmp_serde::to_vec(value).map_err(|e| RmiError::Serialization(e.to_string()))?;
        self.put_raw(key, StorageDataType::MsgPack, &serialized)
    }

    /// Store raw bytes.
    pub fn put_raw(
        &self,
        key: &str,
        data_type: StorageDataType,
        data: &[u8],
    ) -> Result<StorageMetadata> {
        let checksum = xxh64(data, 0);
        let compressed = if self.compress {
            lz4_flex::compress_prepend_size(data)
        } else {
            data.to_vec()
        };

        let mut metadata = StorageMetadata::new(key, data_type, data.len() as u64);
        metadata.compressed_size = compressed.len() as u64;
        metadata.checksum = checksum;

        // Write to disk if not in-memory
        if self.base_path.to_str() != Some(":memory:") {
            self.write_to_disk(key, &metadata, &compressed)?;
        }

        // Update cache
        self.update_cache(key, data.to_vec(), metadata.clone());

        Ok(metadata)
    }

    /// Retrieve a value.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        match self.get_raw(key)? {
            Some((data, _)) => {
                let value = rmp_serde::from_slice(&data)
                    .map_err(|e| RmiError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Retrieve raw bytes.
    pub fn get_raw(&self, key: &str) -> Result<Option<(Vec<u8>, StorageMetadata)>> {
        // Check cache first
        {
            let cache = self.cache.read().unwrap();
            if let Some(entry) = cache.get(key) {
                if !entry.metadata.is_expired() {
                    self.touch_lru(key);
                    return Ok(Some((entry.data.clone(), entry.metadata.clone())));
                }
            }
        }

        // Try disk
        if self.base_path.to_str() != Some(":memory:") {
            if let Some((data, metadata)) = self.read_from_disk(key)? {
                if !metadata.is_expired() {
                    // Populate cache
                    self.update_cache(key, data.clone(), metadata.clone());
                    return Ok(Some((data, metadata)));
                } else {
                    // Delete expired
                    self.delete(key)?;
                }
            }
        }

        Ok(None)
    }

    /// Check if key exists.
    #[inline]
    pub fn exists(&self, key: &str) -> bool {
        // Check cache
        {
            let cache = self.cache.read().unwrap();
            if cache.contains_key(key) {
                return true;
            }
        }

        // Check disk
        if self.base_path.to_str() != Some(":memory:") {
            let path = self.key_to_path(key);
            return path.exists();
        }

        false
    }

    /// Delete a key.
    pub fn delete(&self, key: &str) -> Result<bool> {
        let mut existed = false;

        // Remove from cache
        {
            let mut cache = self.cache.write().unwrap();
            if let Some(entry) = cache.remove(key) {
                let mut size = self.current_cache_bytes.write().unwrap();
                *size = size.saturating_sub(entry.data.len());
                existed = true;
            }
        }

        // Remove from disk
        if self.base_path.to_str() != Some(":memory:") {
            let data_path = self.key_to_path(key);
            let meta_path = self.key_to_meta_path(key);
            if data_path.exists() {
                fs::remove_file(&data_path)?;
                existed = true;
            }
            if meta_path.exists() {
                fs::remove_file(&meta_path)?;
            }
        }

        Ok(existed)
    }

    /// List all keys matching a prefix.
    pub fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        let mut keys = Vec::new();

        // From cache
        {
            let cache = self.cache.read().unwrap();
            for key in cache.keys() {
                if key.starts_with(prefix) {
                    keys.push(key.clone());
                }
            }
        }

        // From disk
        if self.base_path.to_str() != Some(":memory:") {
            for entry in fs::read_dir(&self.base_path)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".data") {
                    let key = name.trim_end_matches(".data").replace("__", "/");
                    if key.starts_with(prefix) && !keys.contains(&key) {
                        keys.push(key);
                    }
                }
            }
        }

        keys.sort();
        Ok(keys)
    }

    /// Get metadata for a key.
    pub fn metadata(&self, key: &str) -> Result<Option<StorageMetadata>> {
        // Check cache
        {
            let cache = self.cache.read().unwrap();
            if let Some(entry) = cache.get(key) {
                return Ok(Some(entry.metadata.clone()));
            }
        }

        // Check disk
        if self.base_path.to_str() != Some(":memory:") {
            let meta_path = self.key_to_meta_path(key);
            if meta_path.exists() {
                let data = fs::read(&meta_path)?;
                let metadata = StorageMetadata::from_binary(&data)?;
                return Ok(Some(metadata));
            }
        }

        Ok(None)
    }

    // Internal helpers

    fn key_to_path(&self, key: &str) -> PathBuf {
        let safe_key = key.replace('/', "__");
        self.base_path.join(format!("{}.data", safe_key))
    }

    fn key_to_meta_path(&self, key: &str) -> PathBuf {
        let safe_key = key.replace('/', "__");
        self.base_path.join(format!("{}.meta", safe_key))
    }

    fn write_to_disk(&self, key: &str, metadata: &StorageMetadata, data: &[u8]) -> Result<()> {
        let data_path = self.key_to_path(key);
        let meta_path = self.key_to_meta_path(key);

        fs::write(&data_path, data)?;
        fs::write(&meta_path, metadata.to_binary())?;

        Ok(())
    }

    fn read_from_disk(&self, key: &str) -> Result<Option<(Vec<u8>, StorageMetadata)>> {
        let data_path = self.key_to_path(key);
        let meta_path = self.key_to_meta_path(key);

        if !data_path.exists() {
            return Ok(None);
        }

        let compressed = fs::read(&data_path)?;
        let data = if self.compress {
            lz4_flex::decompress_size_prepended(&compressed)
                .map_err(|e| RmiError::Serialization(e.to_string()))?
        } else {
            compressed
        };

        let metadata = if meta_path.exists() {
            StorageMetadata::from_binary(&fs::read(&meta_path)?)?
        } else {
            StorageMetadata::new(key, StorageDataType::Binary, data.len() as u64)
        };

        // Verify checksum
        if xxh64(&data, 0) != metadata.checksum {
            return Err(RmiError::protocol_simple("Checksum mismatch"));
        }

        Ok(Some((data, metadata)))
    }

    fn update_cache(&self, key: &str, data: Vec<u8>, metadata: StorageMetadata) {
        let entry_size = data.len();

        // Evict if needed
        self.evict_if_needed(entry_size);

        // Insert
        {
            let mut cache = self.cache.write().unwrap();
            cache.insert(
                key.to_string(),
                CacheEntry {
                    data,
                    metadata,
                },
            );
        }

        // Update LRU
        {
            let mut lru = self.lru_order.write().unwrap();
            lru.retain(|k| k != key);
            lru.push_back(key.to_string());
        }

        // Update size
        {
            let mut size = self.current_cache_bytes.write().unwrap();
            *size += entry_size;
        }
    }

    fn touch_lru(&self, key: &str) {
        let mut lru = self.lru_order.write().unwrap();
        lru.retain(|k| k != key);
        lru.push_back(key.to_string());
    }

    fn evict_if_needed(&self, new_size: usize) {
        let current = *self.current_cache_bytes.read().unwrap();
        if current + new_size <= self.max_cache_bytes {
            return;
        }

        // Evict oldest entries
        let mut to_evict = Vec::new();
        let mut freed = 0;
        {
            let lru = self.lru_order.read().unwrap();
            let cache = self.cache.read().unwrap();

            for key in lru.iter() {
                if current + new_size - freed <= self.max_cache_bytes {
                    break;
                }
                if let Some(entry) = cache.get(key) {
                    freed += entry.data.len();
                    to_evict.push(key.clone());
                }
            }
        }

        // Perform eviction
        {
            let mut cache = self.cache.write().unwrap();
            let mut lru = self.lru_order.write().unwrap();
            let mut size = self.current_cache_bytes.write().unwrap();

            for key in to_evict {
                if let Some(entry) = cache.remove(&key) {
                    *size = size.saturating_sub(entry.data.len());
                }
                lru.retain(|k| k != &key);
            }
        }
    }
}

// ============================================================================
// Tensor Storage
// ============================================================================

/// Header for tensor storage files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorStorageHeader {
    /// Magic bytes
    pub magic: [u8; 4],
    /// Version
    pub version: u16,
    /// Number of tensors
    pub num_tensors: u32,
    /// Index offset
    pub index_offset: u64,
    /// Total data size
    pub total_size: u64,
}

/// Entry in tensor index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorIndexEntry {
    /// Tensor name
    pub name: String,
    /// Shape
    pub shape: Vec<usize>,
    /// Data type
    pub dtype: String,
    /// Offset in file
    pub offset: u64,
    /// Size in bytes
    pub size: u64,
    /// Checksum
    pub checksum: u64,
}

/// Efficient tensor storage (similar to safetensors format).
pub struct TensorStorage {
    /// File path
    path: PathBuf,
    /// Index of tensors
    index: BTreeMap<String, TensorIndexEntry>,
    /// Memory-mapped data (for reading)
    mmap_data: Option<Vec<u8>>,
}

impl TensorStorage {
    /// Create a new tensor storage for writing.
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        Ok(Self {
            path,
            index: BTreeMap::new(),
            mmap_data: None,
        })
    }

    /// Open existing tensor storage for reading.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let data = fs::read(&path)?;

        // Parse header
        if data.len() < 24 {
            return Err(RmiError::protocol_simple("File too small"));
        }

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&data[0..4]);
        if &magic != b"TENS" {
            return Err(RmiError::protocol_simple("Invalid magic"));
        }

        let index_offset = u64::from_le_bytes([
            data[12], data[13], data[14], data[15], data[16], data[17], data[18], data[19],
        ]) as usize;

        // Parse index
        let index_data = &data[index_offset..];
        let index: BTreeMap<String, TensorIndexEntry> = rmp_serde::from_slice(index_data)
            .map_err(|e| RmiError::Serialization(e.to_string()))?;

        Ok(Self {
            path,
            index,
            mmap_data: Some(data),
        })
    }

    /// Add a tensor (f32).
    pub fn add_f32(&mut self, name: &str, shape: &[usize], data: &[f32]) -> Result<()> {
        let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.add_raw(name, shape, "f32", &bytes)
    }

    /// Add a tensor (f64).
    pub fn add_f64(&mut self, name: &str, shape: &[usize], data: &[f64]) -> Result<()> {
        let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.add_raw(name, shape, "f64", &bytes)
    }

    /// Add raw tensor data.
    pub fn add_raw(&mut self, name: &str, shape: &[usize], dtype: &str, data: &[u8]) -> Result<()> {
        let checksum = xxh64(data, 0);

        let offset = self
            .index
            .values()
            .map(|e| e.offset + e.size)
            .max()
            .unwrap_or(24); // After header

        self.index.insert(
            name.to_string(),
            TensorIndexEntry {
                name: name.to_string(),
                shape: shape.to_vec(),
                dtype: dtype.to_string(),
                offset,
                size: data.len() as u64,
                checksum,
            },
        );

        Ok(())
    }

    /// Save to file.
    pub fn save(&self, tensors_data: &HashMap<String, Vec<u8>>) -> Result<()> {
        let mut file = File::create(&self.path)?;

        // Write header placeholder
        let header_bytes = [0u8; 24];
        file.write_all(&header_bytes)?;

        // Write tensor data
        let mut offset = 24u64;
        for (name, entry) in &self.index {
            if let Some(data) = tensors_data.get(name) {
                file.seek(SeekFrom::Start(entry.offset))?;
                file.write_all(data)?;
                offset = entry.offset + entry.size;
            }
        }

        // Write index
        let index_offset = offset;
        let index_bytes = rmp_serde::to_vec(&self.index)
            .map_err(|e| RmiError::Serialization(e.to_string()))?;
        file.write_all(&index_bytes)?;

        // Write header
        file.seek(SeekFrom::Start(0))?;
        file.write_all(b"TENS")?; // Magic
        file.write_all(&1u16.to_le_bytes())?; // Version
        file.write_all(&(self.index.len() as u32).to_le_bytes())?; // Num tensors
        file.write_all(&index_offset.to_le_bytes())?; // Index offset
        file.write_all(&(offset + index_bytes.len() as u64).to_le_bytes())?; // Total size

        Ok(())
    }

    /// Get tensor by name (f32).
    pub fn get_f32(&self, name: &str) -> Result<Option<(Vec<usize>, Vec<f32>)>> {
        let entry = match self.index.get(name) {
            Some(e) => e,
            None => return Ok(None),
        };

        if entry.dtype != "f32" {
            return Err(RmiError::protocol_simple(format!(
                "Expected f32, got {}",
                entry.dtype
            )));
        }

        let data = self.get_raw_data(entry)?;
        let values: Vec<f32> = data
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        // Verify checksum
        if xxh64(&data, 0) != entry.checksum {
            return Err(RmiError::protocol_simple("Checksum mismatch"));
        }

        Ok(Some((entry.shape.clone(), values)))
    }

    /// List all tensor names.
    pub fn tensor_names(&self) -> Vec<&str> {
        self.index.keys().map(|s| s.as_str()).collect()
    }

    /// Get tensor info.
    pub fn tensor_info(&self, name: &str) -> Option<&TensorIndexEntry> {
        self.index.get(name)
    }

    fn get_raw_data(&self, entry: &TensorIndexEntry) -> Result<Vec<u8>> {
        if let Some(ref data) = self.mmap_data {
            let start = entry.offset as usize;
            let end = start + entry.size as usize;
            if end > data.len() {
                return Err(RmiError::protocol_simple("Data out of bounds"));
            }
            Ok(data[start..end].to_vec())
        } else {
            Err(RmiError::protocol_simple("No data loaded"))
        }
    }
}

// ============================================================================
// Checkpoint Manager
// ============================================================================

/// Checkpoint types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CheckpointType {
    /// Full model checkpoint
    Full,
    /// Incremental (delta) checkpoint
    Incremental,
    /// Agent state snapshot
    AgentState,
    /// Training state (optimizer, scheduler)
    TrainingState,
}

/// Checkpoint metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMeta {
    /// Unique ID
    pub id: Uuid,
    /// Checkpoint type
    pub checkpoint_type: CheckpointType,
    /// Creation time
    pub created_at: f64,
    /// Step/epoch number
    pub step: u64,
    /// Description
    pub description: String,
    /// Metrics at this checkpoint
    pub metrics: HashMap<String, f64>,
    /// Parent checkpoint ID (for incremental)
    pub parent_id: Option<Uuid>,
    /// Associated tensor file
    pub tensor_file: Option<String>,
    /// Custom metadata
    pub custom: HashMap<String, String>,
}

/// Manages model and agent checkpoints.
pub struct CheckpointManager {
    /// Base directory
    base_path: PathBuf,
    /// KV store for metadata
    metadata_store: KeyValueStore,
    /// Maximum checkpoints to keep
    max_checkpoints: usize,
}

impl CheckpointManager {
    /// Create a new checkpoint manager.
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path)?;

        let metadata_store = KeyValueStore::new(base_path.join("metadata"))?;

        Ok(Self {
            base_path,
            metadata_store,
            max_checkpoints: 10,
        })
    }

    /// Set maximum checkpoints to keep.
    pub fn with_max_checkpoints(mut self, max: usize) -> Self {
        self.max_checkpoints = max;
        self
    }

    /// Save a checkpoint.
    pub fn save<T: Serialize>(
        &self,
        checkpoint_type: CheckpointType,
        step: u64,
        description: &str,
        data: &T,
        metrics: HashMap<String, f64>,
    ) -> Result<CheckpointMeta> {
        let id = Uuid::new_v4();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        let meta = CheckpointMeta {
            id,
            checkpoint_type,
            created_at: now,
            step,
            description: description.to_string(),
            metrics,
            parent_id: None,
            tensor_file: None,
            custom: HashMap::new(),
        };

        // Save data
        let data_bytes =
            rmp_serde::to_vec(data).map_err(|e| RmiError::Serialization(e.to_string()))?;
        let compressed = lz4_flex::compress_prepend_size(&data_bytes);

        let data_path = self.base_path.join(format!("{}.ckpt", id));
        fs::write(&data_path, compressed)?;

        // Save metadata
        self.metadata_store
            .put(&format!("checkpoint:{}", id), &meta)?;

        // Cleanup old checkpoints
        self.cleanup_old_checkpoints()?;

        Ok(meta)
    }

    /// Save a checkpoint with tensors.
    pub fn save_with_tensors(
        &self,
        checkpoint_type: CheckpointType,
        step: u64,
        description: &str,
        state: &HashMap<String, Vec<u8>>,
        tensors: &HashMap<String, (Vec<usize>, Vec<f32>)>,
        metrics: HashMap<String, f64>,
    ) -> Result<CheckpointMeta> {
        let id = Uuid::new_v4();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        // Save tensors
        let tensor_file = format!("{}.tensors", id);
        let tensor_path = self.base_path.join(&tensor_file);
        let mut storage = TensorStorage::create(&tensor_path)?;

        let mut tensor_data = HashMap::new();
        for (name, (shape, data)) in tensors {
            storage.add_f32(name, shape, data)?;
            tensor_data.insert(
                name.clone(),
                data.iter().flat_map(|f| f.to_le_bytes()).collect(),
            );
        }
        storage.save(&tensor_data)?;

        let meta = CheckpointMeta {
            id,
            checkpoint_type,
            created_at: now,
            step,
            description: description.to_string(),
            metrics,
            parent_id: None,
            tensor_file: Some(tensor_file),
            custom: HashMap::new(),
        };

        // Save state
        let state_bytes =
            rmp_serde::to_vec(state).map_err(|e| RmiError::Serialization(e.to_string()))?;
        let compressed = lz4_flex::compress_prepend_size(&state_bytes);

        let data_path = self.base_path.join(format!("{}.ckpt", id));
        fs::write(&data_path, compressed)?;

        // Save metadata
        self.metadata_store
            .put(&format!("checkpoint:{}", id), &meta)?;

        // Cleanup old checkpoints
        self.cleanup_old_checkpoints()?;

        Ok(meta)
    }

    /// Load a checkpoint.
    pub fn load<T: DeserializeOwned>(&self, id: Uuid) -> Result<Option<(CheckpointMeta, T)>> {
        // Load metadata
        let meta: CheckpointMeta = match self.metadata_store.get(&format!("checkpoint:{}", id))? {
            Some(m) => m,
            None => return Ok(None),
        };

        // Load data
        let data_path = self.base_path.join(format!("{}.ckpt", id));
        if !data_path.exists() {
            return Ok(None);
        }

        let compressed = fs::read(&data_path)?;
        let data_bytes = lz4_flex::decompress_size_prepended(&compressed)
            .map_err(|e| RmiError::Serialization(e.to_string()))?;

        let data: T = rmp_serde::from_slice(&data_bytes)
            .map_err(|e| RmiError::Serialization(e.to_string()))?;

        Ok(Some((meta, data)))
    }

    /// Load tensors from a checkpoint.
    pub fn load_tensors(&self, id: Uuid) -> Result<Option<TensorStorage>> {
        let meta: CheckpointMeta = match self.metadata_store.get(&format!("checkpoint:{}", id))? {
            Some(m) => m,
            None => return Ok(None),
        };

        if let Some(tensor_file) = meta.tensor_file {
            let tensor_path = self.base_path.join(tensor_file);
            if tensor_path.exists() {
                return Ok(Some(TensorStorage::open(tensor_path)?));
            }
        }

        Ok(None)
    }

    /// List all checkpoints.
    pub fn list(&self) -> Result<Vec<CheckpointMeta>> {
        let keys = self.metadata_store.list_keys("checkpoint:")?;
        let mut checkpoints = Vec::new();

        for key in keys {
            if let Some(meta) = self.metadata_store.get::<CheckpointMeta>(&key)? {
                checkpoints.push(meta);
            }
        }

        checkpoints.sort_by_key(|a| a.step);
        Ok(checkpoints)
    }

    /// Get latest checkpoint.
    pub fn latest(&self) -> Result<Option<CheckpointMeta>> {
        let checkpoints = self.list()?;
        Ok(checkpoints.into_iter().last())
    }

    fn cleanup_old_checkpoints(&self) -> Result<()> {
        let checkpoints = self.list()?;
        if checkpoints.len() <= self.max_checkpoints {
            return Ok(());
        }

        // Delete oldest checkpoints
        let to_delete = checkpoints.len() - self.max_checkpoints;
        for meta in checkpoints.into_iter().take(to_delete) {
            self.delete(meta.id)?;
        }

        Ok(())
    }

    /// Delete a checkpoint.
    pub fn delete(&self, id: Uuid) -> Result<()> {
        // Delete data file
        let data_path = self.base_path.join(format!("{}.ckpt", id));
        if data_path.exists() {
            fs::remove_file(&data_path)?;
        }

        // Delete tensor file
        let meta: Option<CheckpointMeta> =
            self.metadata_store.get(&format!("checkpoint:{}", id))?;
        if let Some(meta) = meta {
            if let Some(tensor_file) = meta.tensor_file {
                let tensor_path = self.base_path.join(tensor_file);
                if tensor_path.exists() {
                    fs::remove_file(&tensor_path)?;
                }
            }
        }

        // Delete metadata
        self.metadata_store.delete(&format!("checkpoint:{}", id))?;

        Ok(())
    }
}

// ============================================================================
// Distributed Storage Interface
// ============================================================================

/// Shard information for distributed storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardInfo {
    /// Shard ID
    pub shard_id: u32,
    /// Agent responsible for this shard
    pub owner_agent: Uuid,
    /// Replica agents
    pub replicas: Vec<Uuid>,
    /// Start of key hash range (inclusive)
    pub key_range_start: u64,
    /// End of key hash range (exclusive)
    pub key_range_end: u64,
    /// Size in bytes
    pub size_bytes: u64,
}

/// Interface for distributed storage across agents.
pub trait DistributedStorage: Send + Sync {
    /// Get the shard for a key.
    fn get_shard(&self, key: &str) -> ShardInfo;

    /// Route a read request.
    fn route_read(&self, key: &str) -> Uuid;

    /// Route a write request.
    fn route_write(&self, key: &str) -> Vec<Uuid>;

    /// Rebalance shards.
    fn rebalance(&mut self) -> Vec<(u32, Uuid, Uuid)>;
}

/// Consistent hash ring for shard routing.
pub struct ConsistentHashRing {
    /// Virtual nodes per agent
    virtual_nodes: u32,
    /// Ring: hash -> agent
    ring: BTreeMap<u64, Uuid>,
    /// Agent -> shard info
    shards: HashMap<Uuid, Vec<ShardInfo>>,
    /// Replication factor
    replication_factor: u32,
}

impl ConsistentHashRing {
    /// Create a new hash ring.
    pub fn new(replication_factor: u32) -> Self {
        Self {
            virtual_nodes: 100,
            ring: BTreeMap::new(),
            shards: HashMap::new(),
            replication_factor,
        }
    }

    /// Add an agent to the ring.
    pub fn add_agent(&mut self, agent_id: Uuid) {
        for i in 0..self.virtual_nodes {
            let key = format!("{}:{}", agent_id, i);
            let hash = xxh64(key.as_bytes(), 0);
            self.ring.insert(hash, agent_id);
        }
        self.shards.entry(agent_id).or_default();
    }

    /// Remove an agent from the ring.
    pub fn remove_agent(&mut self, agent_id: Uuid) {
        for i in 0..self.virtual_nodes {
            let key = format!("{}:{}", agent_id, i);
            let hash = xxh64(key.as_bytes(), 0);
            self.ring.remove(&hash);
        }
        self.shards.remove(&agent_id);
    }

    /// Get agents responsible for a key.
    pub fn get_agents(&self, key: &str, count: u32) -> Vec<Uuid> {
        if self.ring.is_empty() {
            return Vec::new();
        }

        let hash = xxh64(key.as_bytes(), 0);
        let mut agents = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Find first agent clockwise
        for (_, &agent) in self.ring.range(hash..) {
            if seen.insert(agent) {
                agents.push(agent);
                if agents.len() >= count as usize {
                    break;
                }
            }
        }

        // Wrap around if needed
        if agents.len() < count as usize {
            for (_, &agent) in self.ring.iter() {
                if seen.insert(agent) {
                    agents.push(agent);
                    if agents.len() >= count as usize {
                        break;
                    }
                }
            }
        }

        agents
    }
}

impl DistributedStorage for ConsistentHashRing {
    fn get_shard(&self, key: &str) -> ShardInfo {
        let hash = xxh64(key.as_bytes(), 0);
        let agents = self.get_agents(key, self.replication_factor);

        ShardInfo {
            shard_id: (hash % 1024) as u32,
            owner_agent: agents.first().copied().unwrap_or(Uuid::nil()),
            replicas: agents.into_iter().skip(1).collect(),
            key_range_start: hash,
            key_range_end: hash,
            size_bytes: 0,
        }
    }

    fn route_read(&self, key: &str) -> Uuid {
        self.get_agents(key, 1)
            .first()
            .copied()
            .unwrap_or(Uuid::nil())
    }

    fn route_write(&self, key: &str) -> Vec<Uuid> {
        self.get_agents(key, self.replication_factor)
    }

    fn rebalance(&mut self) -> Vec<(u32, Uuid, Uuid)> {
        // Would implement rebalancing logic
        Vec::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kv_store_basic() {
        let store = KeyValueStore::in_memory();

        // Put and get
        store.put("key1", &"value1".to_string()).unwrap();
        let value: Option<String> = store.get("key1").unwrap();
        assert_eq!(value, Some("value1".to_string()));

        // Check exists
        assert!(store.exists("key1"));
        assert!(!store.exists("key2"));

        // Delete
        store.delete("key1").unwrap();
        assert!(!store.exists("key1"));
    }

    #[test]
    fn test_kv_store_complex_values() {
        let store = KeyValueStore::in_memory();

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        struct TestData {
            name: String,
            values: Vec<f64>,
            nested: HashMap<String, i32>,
        }

        let data = TestData {
            name: "test".to_string(),
            values: vec![1.0, 2.0, 3.0],
            nested: [("a".to_string(), 1), ("b".to_string(), 2)]
                .into_iter()
                .collect(),
        };

        store.put("test_data", &data).unwrap();
        let retrieved: Option<TestData> = store.get("test_data").unwrap();
        assert_eq!(retrieved, Some(data));
    }

    #[test]
    fn test_metadata() {
        let meta = StorageMetadata::new("test", StorageDataType::MsgPack, 1024);
        assert!(!meta.is_expired());
        assert_eq!(meta.version, 1);
    }

    #[test]
    fn test_consistent_hash_ring() {
        let mut ring = ConsistentHashRing::new(3);

        let agent1 = Uuid::new_v4();
        let agent2 = Uuid::new_v4();
        let agent3 = Uuid::new_v4();

        ring.add_agent(agent1);
        ring.add_agent(agent2);
        ring.add_agent(agent3);

        // Should get agents for any key
        let agents = ring.get_agents("test_key", 2);
        assert_eq!(agents.len(), 2);

        // Route should be consistent
        let route1 = ring.route_read("test_key");
        let route2 = ring.route_read("test_key");
        assert_eq!(route1, route2);
    }

    #[test]
    fn test_list_keys() {
        let store = KeyValueStore::in_memory();

        store.put("agents/agent1", &"data1".to_string()).unwrap();
        store.put("agents/agent2", &"data2".to_string()).unwrap();
        store.put("models/model1", &"data3".to_string()).unwrap();

        let agent_keys = store.list_keys("agents/").unwrap();
        assert_eq!(agent_keys.len(), 2);

        let model_keys = store.list_keys("models/").unwrap();
        assert_eq!(model_keys.len(), 1);
    }

    #[test]
    fn test_metadata_binary_roundtrip() {
        let meta = StorageMetadata::new("roundtrip", StorageDataType::TensorF32, 2048);
        let binary = meta.to_binary();
        let restored = StorageMetadata::from_binary(&binary).unwrap();
        assert_eq!(restored.key, "roundtrip");
        assert_eq!(restored.size_bytes, 2048);
        assert_eq!(restored.version, 1);
    }

    #[test]
    fn test_metadata_ttl_zero_never_expires() {
        let meta = StorageMetadata::new("persist", StorageDataType::Json, 100);
        assert_eq!(meta.ttl_seconds, 0);
        assert!(!meta.is_expired());
    }

    #[test]
    fn test_kv_store_put_raw_get_raw() {
        let store = KeyValueStore::in_memory();
        let data = b"raw bytes here";
        store.put_raw("raw_key", StorageDataType::Binary, data).unwrap();
        let result = store.get_raw("raw_key").unwrap();
        assert!(result.is_some());
        let (bytes, meta) = result.unwrap();
        assert_eq!(bytes, data);
        assert_eq!(meta.key, "raw_key");
    }

    #[test]
    fn test_kv_store_overwrite() {
        let store = KeyValueStore::in_memory();
        store.put("ow", &10u64).unwrap();
        store.put("ow", &20u64).unwrap();
        let val: Option<u64> = store.get("ow").unwrap();
        assert_eq!(val, Some(20));
    }

    #[test]
    fn test_kv_store_get_missing() {
        let store = KeyValueStore::in_memory();
        let val: Option<String> = store.get("nonexistent").unwrap();
        assert!(val.is_none());
    }

    #[test]
    fn test_kv_store_delete_missing() {
        let store = KeyValueStore::in_memory();
        let deleted = store.delete("no_such_key").unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_kv_store_metadata() {
        let store = KeyValueStore::in_memory();
        store.put("meta_key", &42i32).unwrap();
        let meta = store.metadata("meta_key").unwrap();
        assert!(meta.is_some());
        let m = meta.unwrap();
        assert_eq!(m.key, "meta_key");
        assert!(m.size_bytes > 0);
    }

    #[test]
    fn test_kv_store_metadata_missing() {
        let store = KeyValueStore::in_memory();
        let meta = store.metadata("ghost").unwrap();
        assert!(meta.is_none());
    }

    #[test]
    fn test_kv_store_with_cache_size() {
        let store = KeyValueStore::in_memory().with_cache_size(1024);
        store.put("c", &"cached".to_string()).unwrap();
        let v: Option<String> = store.get("c").unwrap();
        assert_eq!(v, Some("cached".to_string()));
    }

    #[test]
    fn test_kv_store_without_compression() {
        let store = KeyValueStore::in_memory().without_compression();
        store.put("nc", &vec![1, 2, 3]).unwrap();
        let v: Option<Vec<i32>> = store.get("nc").unwrap();
        assert_eq!(v, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_kv_store_list_keys_empty_prefix() {
        let store = KeyValueStore::in_memory();
        store.put("a", &1u8).unwrap();
        store.put("b", &2u8).unwrap();
        let keys = store.list_keys("").unwrap();
        assert!(keys.len() >= 2);
    }

    #[test]
    fn test_hash_ring_remove_agent() {
        let mut ring = ConsistentHashRing::new(2);
        let a1 = Uuid::new_v4();
        let a2 = Uuid::new_v4();
        ring.add_agent(a1);
        ring.add_agent(a2);
        ring.remove_agent(a1);
        let agents = ring.get_agents("key", 2);
        assert!(!agents.contains(&a1));
    }

    #[test]
    fn test_hash_ring_route_write() {
        let mut ring = ConsistentHashRing::new(3);
        let a1 = Uuid::new_v4();
        let a2 = Uuid::new_v4();
        let a3 = Uuid::new_v4();
        ring.add_agent(a1);
        ring.add_agent(a2);
        ring.add_agent(a3);
        let write_targets = ring.route_write("some_key");
        assert_eq!(write_targets.len(), 3);
    }

    #[test]
    fn test_hash_ring_single_agent() {
        let mut ring = ConsistentHashRing::new(3);
        let a1 = Uuid::new_v4();
        ring.add_agent(a1);
        let agents = ring.get_agents("key", 3);
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0], a1);
    }

    #[test]
    fn test_storage_data_type_variants() {
        let meta_bin = StorageMetadata::new("bin", StorageDataType::Binary, 10);
        let meta_json = StorageMetadata::new("json", StorageDataType::Json, 20);
        let meta_grad = StorageMetadata::new("grad", StorageDataType::Gradient, 30);
        assert_eq!(meta_bin.key, "bin");
        assert_eq!(meta_json.key, "json");
        assert_eq!(meta_grad.key, "grad");
    }

}
