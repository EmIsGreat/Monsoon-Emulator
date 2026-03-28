//! Cross-platform storage abstraction for native and WASM environments.
//!
//! This module provides a unified interface for persistent storage that works
//! across:
//! - **Native**: Uses the file system via the `directories` crate for
//!   OS-appropriate paths
//! - **WASM**: Uses IndexedDB via `rexie` for structured data storage
//!
//! # Architecture
//!
//! The storage system is built around three main concepts:
//!
//! 1. **StorageKey**: Identifies what data is being stored (config, saves,
//!    palettes, etc.)
//! 2. **Storage trait**: Async interface for get/set/delete/list operations
//! 3. **Platform-specific implementations**: NativeStorage and WasmStorage
//!
//! # Usage
//!
//! ```ignore
//! // Get the platform-appropriate storage instance
//! let storage = get_storage();
//!
//! // Save some data
//! storage.set("saves/my_game/quicksave.sav", data).await?;
//!
//! // Read it back
//! let data = storage.get("saves/my_game/quicksave.sav").await?;
//!
//! // List all saves for a game
//! let saves = storage.list("saves/my_game/").await?;
//! ```
//!
//! # WASM Considerations
//!
//! On WASM, storage has different characteristics:
//! - **localStorage**: ~5MB limit, synchronous, string-only (not suitable for
//!   binary data)
//! - **IndexedDB**: Larger storage (~50MB+), async, supports binary data
//!   (recommended)
//!
//! This module uses IndexedDB for WASM to support save states and other binary
//! data.

use std::fmt::Display;
use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Type alias for async storage results
pub type StorageResult<T> = Result<T, StorageError>;

/// Errors that can occur during storage operations
#[derive(Debug, Clone)]
pub enum StorageError {
    /// The requested key was not found
    NotFound,
    /// Failed to read data
    ReadError(String),
    /// Failed to write data
    WriteError(String),
    /// Failed to delete data
    DeleteError(String),
    /// Storage is not available on this platform
    NotAvailable,
    /// Serialization/deserialization error
    SerializationError(String),
    /// IndexedDB specific error (WASM only)
    #[cfg(target_arch = "wasm32")]
    IndexedDbError(String),
}

impl Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::NotFound => write!(f, "Key not found"),
            StorageError::ReadError(e) => write!(f, "Read error: {}", e),
            StorageError::WriteError(e) => write!(f, "Write error: {}", e),
            StorageError::DeleteError(e) => write!(f, "Delete error: {}", e),
            StorageError::NotAvailable => write!(f, "Storage not available"),
            StorageError::SerializationError(e) => write!(f, "Serialization error: {}", e),
            #[cfg(target_arch = "wasm32")]
            StorageError::IndexedDbError(e) => write!(f, "IndexedDB error: {}", e),
        }
    }
}

impl std::error::Error for StorageError {}

/// Metadata about a stored item
#[derive(Debug, Clone)]
pub struct StorageMetadata {
    /// The key/path of the item
    pub key: StorageKey,
}

/// Storage categories for organizing data
///
/// These categories help organize data and may map to different directories
/// on native platforms or different IndexedDB object stores on WASM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum StorageCategory {
    /// Application configuration (config.toml, keybindings, etc.)
    Config,
    /// User data (save states, quicksaves, autosaves)
    Data,
    /// Cached data that can be regenerated (thumbnails, compiled shaders)
    Cache,
    /// Data not managed by Monsoon, that still needs to be addressed via
    /// storage keys
    Root,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct StorageKey {
    pub category: StorageCategory,
    pub sub_path: String,
}

impl Display for StorageKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.category.prefix(), self.sub_path)
    }
}

impl From<&String> for StorageKey {
    fn from(value: &String) -> Self {
        let (prefix, mut sub) = value.split_once("/").unzip();
        let valid_prefixes = [
            StorageCategory::Config,
            StorageCategory::Data,
            StorageCategory::Cache,
        ];

        let category = if let Some(prefix) = prefix {
            valid_prefixes
                .iter()
                .find(|p| p.prefix() == prefix)
                .unwrap_or(&StorageCategory::Root)
        } else {
            sub = Some(value.as_str());
            &StorageCategory::Root
        };

        StorageKey {
            category: *category,
            sub_path: sub.unwrap_or("").to_string(),
        }
    }
}

impl StorageCategory {
    /// Get the string prefix for this category
    pub fn prefix(&self) -> &'static str {
        match self {
            StorageCategory::Config => "config",
            StorageCategory::Data => "data",
            StorageCategory::Cache => "cache",
            StorageCategory::Root => "/",
        }
    }
}

/// Async storage interface that works across platforms.
///
/// All operations are async to support both native (thread-based) and
/// WASM (Promise-based) implementations.
///
/// Note: On WASM, futures don't need to be Send since JavaScript is
/// single-threaded.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Storage: Send + Sync {
    /// Get data by key
    async fn get(&self, key: &StorageKey) -> StorageResult<Vec<u8>>;

    /// Set data for a key
    async fn set(&self, key: &StorageKey, data: Vec<u8>) -> StorageResult<()>;

    /// Delete data by key
    async fn delete(&self, key: &StorageKey) -> StorageResult<()>;

    /// Check if a key exists
    async fn exists(&self, key: &StorageKey) -> StorageResult<bool>;

    /// List all keys with a given prefix
    async fn list(&self, prefix: &StorageKey) -> StorageResult<Vec<StorageMetadata>>;

    /// Get the full path/URL for a key (for display purposes)
    fn get_display_path(&self, key: &StorageKey) -> String;
    fn key_to_path_opt(&self, key: Option<&StorageKey>) -> Option<PathBuf>;
    fn key_to_path(&self, key: &StorageKey) -> Option<PathBuf>;
}

// ============================================================================
// Native Implementation
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    use async_trait::async_trait;
    use directories::ProjectDirs;

    use crate::frontend::storage::{
        Storage, StorageCategory, StorageError, StorageKey, StorageMetadata, StorageResult,
    };

    const APP_QUALIFIER: &str = "com";
    const APP_ORGANIZATION: &str = "Lightsong";
    const APP_NAME: &str = "MonsoonEmulator";

    static PROJECT_DIRS: OnceLock<Option<ProjectDirs>> = OnceLock::new();

    fn get_project_dirs() -> Option<&'static ProjectDirs> {
        PROJECT_DIRS
            .get_or_init(|| ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_NAME))
            .as_ref()
    }

    /// Native file system storage implementation
    pub struct NativeStorage;

    impl Default for NativeStorage {
        fn default() -> Self { Self::new() }
    }

    impl NativeStorage {
        pub fn new() -> Self { NativeStorage }

        fn get_base_dir(&self, category: &StorageCategory) -> Option<PathBuf> {
            let dirs = get_project_dirs()?;
            let base = match category {
                StorageCategory::Config => dirs.config_dir(),
                StorageCategory::Data => dirs.data_dir(),
                StorageCategory::Cache => dirs.cache_dir(),
                StorageCategory::Root => Path::new("/"),
            };
            Some(base.to_path_buf())
        }
    }

    #[async_trait]
    impl Storage for NativeStorage {
        async fn get(&self, key: &StorageKey) -> StorageResult<Vec<u8>> {
            let path = self.key_to_path(key).ok_or(StorageError::NotAvailable)?;

            if !path.exists() {
                return Err(StorageError::NotFound);
            }

            let mut file =
                std::fs::File::open(&path).map_err(|e| StorageError::ReadError(e.to_string()))?;

            let mut data = Vec::new();
            file.read_to_end(&mut data)
                .map_err(|e| StorageError::ReadError(e.to_string()))?;

            Ok(data)
        }

        async fn set(&self, key: &StorageKey, data: Vec<u8>) -> StorageResult<()> {
            let path = self.key_to_path(key).ok_or(StorageError::NotAvailable)?;

            // Create parent directories
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| StorageError::WriteError(e.to_string()))?;
            }

            let mut file = std::fs::File::create(&path)
                .map_err(|e| StorageError::WriteError(e.to_string()))?;

            file.write_all(&data)
                .map_err(|e| StorageError::WriteError(e.to_string()))?;

            Ok(())
        }

        async fn delete(&self, key: &StorageKey) -> StorageResult<()> {
            let path = self.key_to_path(key).ok_or(StorageError::NotAvailable)?;

            if path.exists() {
                std::fs::remove_file(&path)
                    .map_err(|e| StorageError::DeleteError(e.to_string()))?;
            }

            Ok(())
        }

        async fn exists(&self, key: &StorageKey) -> StorageResult<bool> {
            let path = self.key_to_path(key).ok_or(StorageError::NotAvailable)?;

            Ok(path.exists())
        }

        async fn list(&self, prefix: &StorageKey) -> StorageResult<Vec<StorageMetadata>> {
            let base_path = self.key_to_path(prefix).ok_or(StorageError::NotAvailable)?;

            if !base_path.exists() {
                return Ok(Vec::new());
            }

            let mut results = Vec::new();

            if base_path.is_dir() {
                Self::collect_files(&base_path, prefix, &mut results)?;
            } else if base_path.is_file() {
                results.push(StorageMetadata {
                    key: prefix.clone(),
                });
            }

            Ok(results)
        }

        fn get_display_path(&self, key: &StorageKey) -> String {
            self.key_to_path(key)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| key.sub_path.to_string())
        }

        fn key_to_path_opt(&self, key: Option<&StorageKey>) -> Option<PathBuf> {
            if let Some(key) = key {
                self.key_to_path(key)
            } else {
                None
            }
        }

        fn key_to_path(&self, key: &StorageKey) -> Option<PathBuf> {
            let base = self.get_base_dir(&key.category)?;
            Some(base.join(key.sub_path.clone()))
        }
    }

    impl NativeStorage {
        fn collect_files(
            dir: &PathBuf,
            prefix: &StorageKey,
            results: &mut Vec<StorageMetadata>,
        ) -> StorageResult<()> {
            let entries =
                std::fs::read_dir(dir).map_err(|e| StorageError::ReadError(e.to_string()))?;

            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                let sub = if prefix.sub_path.ends_with('/') {
                    format!("{}{}", prefix.sub_path, name)
                } else {
                    format!("{}/{}", prefix.sub_path, name)
                };

                let key = StorageKey {
                    category: prefix.category,
                    sub_path: sub,
                };

                if path.is_file() {
                    results.push(StorageMetadata {
                        key,
                    });
                } else if path.is_dir() {
                    Self::collect_files(&path, &key, results)?;
                }
            }

            Ok(())
        }
    }
}

// ============================================================================
// WASM Implementation
// ============================================================================

#[cfg(target_arch = "wasm32")]
mod wasm {
    use js_sys::Uint8Array;
    use rexie::{KeyRange, Rexie, TransactionMode};
    use wasm_bindgen::JsValue;

    use super::*;

    const DB_NAME: &str = "monsoon_emulator";
    const DB_VERSION: u32 = 1;
    const STORE_NAME: &str = "storage";

    /// WASM storage implementation using IndexedDB via rexie.
    ///
    /// Provides persistent storage in the browser using IndexedDB,
    /// which supports larger storage limits and binary data.
    ///
    /// # Database Structure
    ///
    /// - Database name: "monsoon_emulator"
    /// - Object store: "storage" (key-value pairs where key is the StorageKey
    ///   path string)
    /// - Values are stored as Uint8Array (raw bytes)
    /// - Prefix queries use KeyRange::bound() on the primary key for efficient
    ///   listing
    pub struct WasmStorage;

    impl WasmStorage {
        pub fn new() -> Self { WasmStorage }

        /// Convert StorageKey to the string key used in IndexedDB
        fn key_string(key: &StorageKey) -> String {
            format!("{}/{}", key.category.prefix(), key.sub_path)
        }
    }

    impl Default for WasmStorage {
        fn default() -> Self { Self::new() }
    }

    /// Open the IndexedDB database, creating the object store if needed.
    async fn open_db() -> Result<Rexie, StorageError> {
        Rexie::builder(DB_NAME)
            .version(DB_VERSION)
            .add_object_store(rexie::ObjectStore::new(STORE_NAME))
            .build()
            .await
            .map_err(|e| StorageError::IndexedDbError(e.to_string()))
    }

    #[async_trait(?Send)]
    impl Storage for WasmStorage {
        async fn get(&self, key: &StorageKey) -> StorageResult<Vec<u8>> {
            let db = open_db().await?;
            let tx = db
                .transaction(&[STORE_NAME], TransactionMode::ReadOnly)
                .map_err(|e| StorageError::ReadError(e.to_string()))?;
            let store = tx
                .store(STORE_NAME)
                .map_err(|e| StorageError::ReadError(e.to_string()))?;

            let key_js = JsValue::from_str(&Self::key_string(key));
            match store
                .get(key_js)
                .await
                .map_err(|e| StorageError::ReadError(e.to_string()))?
            {
                Some(val) => {
                    let array = Uint8Array::new(&val);
                    Ok(array.to_vec())
                }
                None => Err(StorageError::NotFound),
            }
        }

        async fn set(&self, key: &StorageKey, data: Vec<u8>) -> StorageResult<()> {
            let db = open_db().await?;
            let tx = db
                .transaction(&[STORE_NAME], TransactionMode::ReadWrite)
                .map_err(|e| StorageError::WriteError(e.to_string()))?;
            let store = tx
                .store(STORE_NAME)
                .map_err(|e| StorageError::WriteError(e.to_string()))?;

            let key_js = JsValue::from_str(&Self::key_string(key));
            let value_js: JsValue = Uint8Array::from(data.as_slice()).into();
            store
                .put(&value_js, Some(&key_js))
                .await
                .map_err(|e| StorageError::WriteError(e.to_string()))?;
            tx.done()
                .await
                .map_err(|e| StorageError::WriteError(e.to_string()))?;
            Ok(())
        }

        async fn delete(&self, key: &StorageKey) -> StorageResult<()> {
            let db = open_db().await?;
            let tx = db
                .transaction(&[STORE_NAME], TransactionMode::ReadWrite)
                .map_err(|e| StorageError::DeleteError(e.to_string()))?;
            let store = tx
                .store(STORE_NAME)
                .map_err(|e| StorageError::DeleteError(e.to_string()))?;

            let key_js = JsValue::from_str(&Self::key_string(key));
            store
                .delete(key_js)
                .await
                .map_err(|e| StorageError::DeleteError(e.to_string()))?;
            tx.done()
                .await
                .map_err(|e| StorageError::DeleteError(e.to_string()))?;
            Ok(())
        }

        async fn exists(&self, key: &StorageKey) -> StorageResult<bool> {
            let db = open_db().await?;
            let tx = db
                .transaction(&[STORE_NAME], TransactionMode::ReadOnly)
                .map_err(|e| StorageError::ReadError(e.to_string()))?;
            let store = tx
                .store(STORE_NAME)
                .map_err(|e| StorageError::ReadError(e.to_string()))?;

            let key_js = JsValue::from_str(&Self::key_string(key));
            store
                .key_exists(key_js)
                .await
                .map_err(|e| StorageError::ReadError(e.to_string()))
        }

        async fn list(&self, prefix: &StorageKey) -> StorageResult<Vec<StorageMetadata>> {
            let db = open_db().await?;
            let tx = db
                .transaction(&[STORE_NAME], TransactionMode::ReadOnly)
                .map_err(|e| StorageError::ReadError(e.to_string()))?;
            let store = tx
                .store(STORE_NAME)
                .map_err(|e| StorageError::ReadError(e.to_string()))?;

            let prefix_str = Self::key_string(prefix);
            let lower = JsValue::from_str(&prefix_str);
            let upper = JsValue::from_str(&format!("{}\u{ffff}", prefix_str));
            let range = KeyRange::bound(&lower, &upper, Some(false), Some(false))
                .map_err(|e| StorageError::ReadError(format!("{:?}", e)))?;

            let keys = store
                .get_all_keys(Some(range), None)
                .await
                .map_err(|e| StorageError::ReadError(e.to_string()))?;

            Ok(keys
                .into_iter()
                .filter_map(|k| {
                    k.as_string().map(|s| StorageMetadata {
                        key: StorageKey::from(&s),
                    })
                })
                .collect())
        }

        fn get_display_path(&self, key: &StorageKey) -> String {
            format!("indexeddb://monsoon_emulator/{}", Self::key_string(key))
        }

        fn key_to_path_opt(&self, _key: Option<&StorageKey>) -> Option<PathBuf> {
            // WASM doesn't have filesystem paths
            None
        }

        fn key_to_path(&self, _key: &StorageKey) -> Option<PathBuf> {
            // WASM doesn't have filesystem paths
            None
        }
    }
}

// ============================================================================
// Platform Selection
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
pub use native::NativeStorage;
#[cfg(target_arch = "wasm32")]
pub use wasm::WasmStorage;

/// Get the platform-appropriate storage implementation
#[cfg(not(target_arch = "wasm32"))]
pub fn get_storage() -> impl Storage { NativeStorage::new() }

/// Get the platform-appropriate storage implementation
#[cfg(target_arch = "wasm32")]
pub fn get_storage() -> impl Storage { WasmStorage::new() }

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a storage key for a quicksave
pub fn quicksave_key(game_name: &str, timestamp: &str) -> StorageKey {
    let sub = format!("saves/{}/quicksaves/quicksave_{}.sav", game_name, timestamp);

    StorageKey {
        category: StorageCategory::Data,
        sub_path: sub,
    }
}

/// Generate a storage key for an autosave
pub fn autosave_key(game_name: &str, timestamp: &str) -> StorageKey {
    let sub = format!("saves/{}/autosaves/autosaves_{}.sav", game_name, timestamp);

    StorageKey {
        category: StorageCategory::Data,
        sub_path: sub,
    }
}

/// Generate the prefix for listing autosaves for a game
pub fn autosaves_prefix(game_name: &str) -> StorageKey {
    let sub = format!("saves/{}/autosaves/", game_name);

    StorageKey {
        category: StorageCategory::Data,
        sub_path: sub,
    }
}

/// Generate the prefix for listing quicksaves for a game
pub fn quicksaves_prefix(game_name: &str) -> StorageKey {
    let sub = format!("saves/{}/quicksaves/", game_name);

    StorageKey {
        category: StorageCategory::Data,
        sub_path: sub,
    }
}

/// Generate a storage key for the application config
pub fn config_key() -> StorageKey {
    StorageKey {
        category: StorageCategory::Config,
        sub_path: "config/config.toml".to_string(),
    }
}

/// Generate a storage key for egui state
pub fn egui_state_key() -> StorageKey {
    StorageKey {
        category: StorageCategory::Config,
        sub_path: "egui_state".to_string(),
    }
}

/// Generate a storage key for a cached ROM file
pub fn rom_cache_key(filename: &str) -> StorageKey {
    StorageKey {
        category: StorageCategory::Data,
        sub_path: format!("roms/{}", filename),
    }
}

/// Generate the prefix for listing all cached ROMs
pub fn roms_prefix() -> StorageKey {
    StorageKey {
        category: StorageCategory::Data,
        sub_path: "roms/".to_string(),
    }
}

/// Generate a storage key for a cached uploaded savestate
pub fn uploaded_savestate_key(filename: &str) -> StorageKey {
    StorageKey {
        category: StorageCategory::Data,
        sub_path: format!("uploads/savestates/{}", filename),
    }
}

// ============================================================================
// Synchronous Wrappers (Native Only)
// ============================================================================
//
// These provide synchronous access to storage for code that can't be async,
// such as startup config loading and shutdown config saving.

#[cfg(not(target_arch = "wasm32"))]
mod sync_wrappers {
    use crate::frontend::storage::{
        NativeStorage, Storage, StorageError, StorageKey, StorageMetadata, StorageResult,
    };

    /// Global storage instance for synchronous access
    static STORAGE: std::sync::OnceLock<NativeStorage> = std::sync::OnceLock::new();

    fn get_storage_instance() -> &'static NativeStorage { STORAGE.get_or_init(NativeStorage::new) }

    /// Get the full filesystem path for a storage key (native only)
    pub fn get_path_for_key(key: &StorageKey) -> Option<std::path::PathBuf> {
        get_storage_instance().key_to_path(key)
    }

    /// Read data synchronously from storage
    pub fn read_sync(key: &StorageKey) -> StorageResult<Vec<u8>> {
        let storage = get_storage_instance();
        let path = storage.key_to_path(key).ok_or(StorageError::NotAvailable)?;

        if !path.exists() {
            return Err(StorageError::NotFound);
        }

        std::fs::read(&path).map_err(|e| StorageError::ReadError(e.to_string()))
    }

    /// Write data synchronously to storage
    pub fn write_sync(key: &StorageKey, data: &[u8]) -> StorageResult<()> {
        let storage = get_storage_instance();
        let path = storage.key_to_path(key).ok_or(StorageError::NotAvailable)?;

        // Create parent directories
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| StorageError::WriteError(e.to_string()))?;
        }

        std::fs::write(&path, data).map_err(|e| StorageError::WriteError(e.to_string()))
    }

    /// Delete data synchronously from storage
    pub fn delete_sync(key: &StorageKey) -> StorageResult<()> {
        let storage = get_storage_instance();
        let path = storage.key_to_path(key).ok_or(StorageError::NotAvailable)?;

        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| StorageError::DeleteError(e.to_string()))?;
        }

        Ok(())
    }

    /// Check if a key exists synchronously
    pub fn exists_sync(key: &StorageKey) -> StorageResult<bool> {
        let storage = get_storage_instance();
        let path = storage.key_to_path(key).ok_or(StorageError::NotAvailable)?;

        Ok(path.exists())
    }

    /// List all keys with a given prefix synchronously
    pub fn list_sync(prefix: &StorageKey) -> StorageResult<Vec<StorageMetadata>> {
        let storage = get_storage_instance();
        let base_path = storage
            .key_to_path(prefix)
            .ok_or(StorageError::NotAvailable)?;

        if !base_path.exists() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();

        if base_path.is_dir() {
            collect_files_sync(&base_path, prefix, &mut results)?;
        } else if base_path.is_file() {
            results.push(StorageMetadata {
                key: prefix.clone(),
            });
        }

        Ok(results)
    }

    /// Get the display path for a key
    pub fn get_display_path(key: &StorageKey) -> String {
        get_storage_instance().get_display_path(key)
    }

    fn collect_files_sync(
        dir: &std::path::Path,
        prefix: &StorageKey,
        results: &mut Vec<StorageMetadata>,
    ) -> StorageResult<()> {
        let entries = std::fs::read_dir(dir).map_err(|e| StorageError::ReadError(e.to_string()))?;

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let sub = if prefix.sub_path.ends_with('/') {
                format!("{}{}", prefix.sub_path, name)
            } else {
                format!("{}/{}", prefix.sub_path, name)
            };

            let key = StorageKey {
                category: prefix.category,
                sub_path: sub,
            };

            if path.is_file() {
                results.push(StorageMetadata {
                    key,
                });
            } else if path.is_dir() {
                collect_files_sync(&path, &key, results)?;
            }
        }

        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use sync_wrappers::*;
