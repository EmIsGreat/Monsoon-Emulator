//! Cross-platform storage abstraction for native and WASM environments.
//!
//! This module provides a unified interface for persistent storage that works
//! across:
//! - **Native**: Uses the file system via the `directories` crate for
//!   OS-appropriate paths
//! - **WASM**: Uses `IndexedDB` via `rexie` for structured data storage
//!
//! # Architecture
//!
//! The storage system is built around three main concepts:
//!
//! 1. **`StorageKey`**: Identifies what data is being stored (config, saves,
//!    palettes, etc.)
//! 2. **Storage trait**: Async interface for get/set/delete/list operations
//! 3. **Platform-specific implementations**: `NativeStorage` and `WasmStorage`
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
//! - **`IndexedDB`**: Larger storage (~50MB+), async, supports binary data
//!   (recommended)
//!
//! This module uses `IndexedDB` for WASM to support save states and other
//! binary data.

use std::clone::Clone;
use std::fmt::{Display, Formatter};
use std::ops::{Add, AddAssign};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::string::ToString;

use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};
use thiserror::Error;

/// Type alias for async storage results
pub type StorageResult<T> = Result<T, StorageError>;

/// Errors that can occur during storage operations
#[derive(Debug, Clone, Error)]
pub enum StorageError {
    #[error("The requested key was not found")]
    NotFound,
    #[error("Failed to read data: {0}")]
    ReadError(String),
    #[error("Failed to write data: {0}")]
    WriteError(String),
    #[error("Failed to delete data: {0}")]
    DeleteError(String),
    #[error("Storage is not available")]
    NotAvailable,
    #[error("Serialization failed: {0}")]
    SerializationError(String),
    #[cfg(target_arch = "wasm32")]
    #[error("IndexedDB error: {0}")]
    IndexedDbError(String),
}

/// Storage categories for organizing data
///
/// These categories help organize data and may map to different directories
/// on native platforms or different `IndexedDB` object stores on WASM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, EnumIter)]
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

#[derive(Debug, Clone, Error)]
pub enum StoragePathParseError {
    #[error("Storage Path was empty")]
    NoRoot,
    #[error("Storage Path contains an empty path segment")]
    EmptySegment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct StorageKey {
    pub category: StorageCategory,
    pub(crate) path: PathBuf,
}

impl Display for StorageKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.category.prefix(), self.path.display())
    }
}

#[derive(Debug, Error)]
pub enum StorageKeyParseError {
    #[error("Storage Key is not prefixed by a known Storage Category")]
    InvalidCategory,
    #[error("Storage Key did not include a path")]
    NoPath,
    #[error("Error parsing Storage Path: `{0}`")]
    InvalidPath(StoragePathParseError),
}

impl TryFrom<String> for StorageKey {
    type Error = StorageKeyParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if let Some(category) = StorageCategory::iter().find(|c| c.match_prefix().is_match(&s)) {
            let stripped = if category == StorageCategory::Root {
                Some(s.as_str())
            } else {
                s.strip_prefix(category.prefix())
            };

            #[allow(clippy::panic)]
            let Some(s) = stripped else {
                panic!(
                    "String {s} matched prefix {}, but stripping {} failed.",
                    category.match_prefix(),
                    category.prefix()
                );
            };

            if s.is_empty() {
                Err(StorageKeyParseError::NoPath)
            } else {
                let path = PathBuf::from(s);

                Ok(StorageKey {
                    category,
                    path,
                })
            }
        } else {
            Err(StorageKeyParseError::InvalidCategory)
        }
    }
}

impl FromStr for StorageKey {
    type Err = StorageKeyParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> { StorageKey::try_from(s.to_string()) }
}

impl From<&Path> for StorageKey {
    #[allow(clippy::expect_used)]
    fn from(value: &Path) -> Self {
        Self::try_from(value.to_string_lossy().to_string())
            .expect("All valid Paths are also valid StorageKeys")
    }
}

impl StorageKey {
    #[must_use]
    pub fn is_file(&self) -> bool { self.path.is_file() }

    #[must_use]
    pub fn parent(&self) -> Option<StorageKey> {
        let mut parent = self.clone();
        let parent_path = parent.path.parent();

        if let Some(parent_path) = parent_path {
            parent.path = parent_path.to_path_buf();
            Some(parent)
        } else {
            None
        }
    }

    #[must_use]
    #[allow(clippy::expect_used, clippy::missing_panics_doc)]
    pub fn get_leaf_name(&self) -> String {
        self.path
            .file_name()
            .expect("Only fails if path ends in \"..\", which should be impossible.")
            .to_string_lossy()
            .to_string()
    }

    #[must_use]
    pub fn new(storage_category: StorageCategory, path: PathBuf) -> Option<StorageKey> {
        Some(Self {
            category: storage_category,
            path,
        })
    }
}

impl<P: AsRef<Path>> Add<P> for StorageKey {
    type Output = StorageKey;

    fn add(mut self, rhs: P) -> Self::Output {
        self.path.push(rhs);
        self
    }
}

impl<P: AsRef<Path>> AddAssign<P> for StorageKey {
    fn add_assign(&mut self, rhs: P) { self.path.push(rhs) }
}

impl StorageCategory {
    /// Get the string prefix for this category
    #[must_use]
    pub fn prefix(&self) -> &'static str {
        match self {
            StorageCategory::Config => "config/",
            StorageCategory::Data => "data/",
            StorageCategory::Cache => "cache/",
            StorageCategory::Root => "/",
        }
    }

    #[must_use]
    #[allow(clippy::unwrap_used, clippy::missing_panics_doc)]
    pub fn match_prefix(&self) -> Regex {
        match self {
            StorageCategory::Config => Regex::new(r"^config/").unwrap(),
            StorageCategory::Data => Regex::new(r"^data/").unwrap(),
            StorageCategory::Cache => Regex::new(r"^cache/").unwrap(),
            StorageCategory::Root => Regex::new(r"^([A-Z]:|/)").unwrap(),
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
#[allow(clippy::double_must_use)]
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
    async fn list(&self, prefix: &StorageKey) -> StorageResult<Vec<StorageKey>>;

    /// Get the full path/URL for a key (for display purposes)
    fn get_display_path(&self, key: &StorageKey) -> String;
    fn key_to_path(&self, key: Option<&StorageKey>) -> Option<PathBuf>;
}

// ============================================================================
// Native Implementation
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::io::{Read, Write};
    use std::path::PathBuf;

    use async_trait::async_trait;

    use crate::frontend::persistence::get_project_dirs;
    use crate::frontend::storage::{
        Storage, StorageCategory, StorageError, StorageKey, StorageResult,
    };

    /// Native file system storage implementation
    pub struct NativeStorage;

    impl Default for NativeStorage {
        fn default() -> Self { Self::new() }
    }

    impl NativeStorage {
        #[must_use]
        pub fn new() -> Self { NativeStorage }

        #[allow(clippy::expect_used)]
        fn get_base_dir(category: StorageCategory) -> Option<PathBuf> {
            let dirs = get_project_dirs().expect("Unable to retrieve project dirs.");
            match category {
                StorageCategory::Config => Some(dirs.config_dir().to_path_buf()),
                StorageCategory::Data => Some(dirs.data_dir().to_path_buf()),
                StorageCategory::Cache => Some(dirs.cache_dir().to_path_buf()),
                StorageCategory::Root => None,
            }
        }
    }

    #[async_trait]
    impl Storage for NativeStorage {
        async fn get(&self, key: &StorageKey) -> StorageResult<Vec<u8>> {
            let path = self
                .key_to_path(Some(key))
                .ok_or(StorageError::NotAvailable)?;

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
            let path = self
                .key_to_path(Some(key))
                .ok_or(StorageError::NotAvailable)?;

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
            let path = self
                .key_to_path(Some(key))
                .ok_or(StorageError::NotAvailable)?;

            if path.exists() {
                std::fs::remove_file(&path)
                    .map_err(|e| StorageError::DeleteError(e.to_string()))?;
            }

            Ok(())
        }

        async fn exists(&self, key: &StorageKey) -> StorageResult<bool> {
            let path = self
                .key_to_path(Some(key))
                .ok_or(StorageError::NotAvailable)?;

            Ok(path.exists())
        }

        async fn list(&self, prefix: &StorageKey) -> StorageResult<Vec<StorageKey>> {
            let mut results = Vec::new();

            if prefix.path.is_file() {
                results.push(prefix.clone());
            } else {
                self.collect_files(prefix, &mut results)?;
            }

            Ok(results)
        }

        fn get_display_path(&self, key: &StorageKey) -> String {
            self.key_to_path(Some(key)).map_or_else(
                || key.path.to_string_lossy().to_string(),
                |p| p.display().to_string(),
            )
        }

        fn key_to_path(&self, key: Option<&StorageKey>) -> Option<PathBuf> {
            if let Some(key) = key {
                let base = NativeStorage::get_base_dir(key.category);
                if let Some(base) = base {
                    Some(base.join(key.path.clone()))
                } else {
                    Some(key.path.clone())
                }
            } else {
                None
            }
        }
    }

    impl NativeStorage {
        pub fn collect_files(
            &self,
            prefix: &StorageKey,
            results: &mut Vec<StorageKey>,
        ) -> StorageResult<()> {
            let dir = self
                .key_to_path(Some(prefix))
                .ok_or(StorageError::NotAvailable)?;

            if !dir.exists() {
                return Err(StorageError::NotFound);
            }

            let entries =
                std::fs::read_dir(dir).map_err(|e| StorageError::ReadError(e.to_string()))?;

            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();

                let found = prefix.clone() + name;

                if found.is_file() {
                    results.push(found);
                } else {
                    self.collect_files(&found, results)?;
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
    use std::path::PathBuf;

    use js_sys::Uint8Array;
    use rexie::{KeyRange, Rexie, TransactionMode};
    use wasm_bindgen::JsValue;

    use super::{Storage, StorageError, StorageKey, StorageResult, async_trait};

    const DB_NAME: &str = "monsoon_emulator";
    const DB_VERSION: u32 = 1;
    const STORE_NAME: &str = "storage";

    /// WASM storage implementation using `IndexedDB` via rexie.
    ///
    /// Provides persistent storage in the browser using `IndexedDB`,
    /// which supports larger storage limits and binary data.
    ///
    /// # Database Structure
    ///
    /// - Database name: `monsoon_emulator`
    /// - Object store: "storage" (key-value pairs where key is the `StorageKey`
    ///   path string)
    /// - Values are stored as `Uint8Array` (raw bytes)
    /// - Prefix queries use `KeyRange::bound()` on the primary key for
    ///   efficient listing
    pub struct WasmStorage;

    impl WasmStorage {
        #[must_use]
        pub fn new() -> Self { WasmStorage }

        /// Convert `StorageKey` to the string key used in `IndexedDB`
        fn key_string(key: &StorageKey) -> String {
            format!("{}{}", key.category.prefix(), key.path)
        }
    }

    impl Default for WasmStorage {
        fn default() -> Self { Self::new() }
    }

    /// Open the `IndexedDB` database, creating the object store if needed.
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

        async fn list(&self, prefix: &StorageKey) -> StorageResult<Vec<StorageKey>> {
            let db = open_db().await?;
            let tx = db
                .transaction(&[STORE_NAME], TransactionMode::ReadOnly)
                .map_err(|e| StorageError::ReadError(e.to_string()))?;
            let store = tx
                .store(STORE_NAME)
                .map_err(|e| StorageError::ReadError(e.to_string()))?;

            let prefix_str = Self::key_string(prefix);
            let lower = JsValue::from_str(&prefix_str);
            let upper = JsValue::from_str(&format!("{prefix_str}\u{ffff}"));
            let range = KeyRange::bound(&lower, &upper, Some(false), Some(false))
                .map_err(|e| StorageError::ReadError(format!("{e:?}")))?;

            let keys = store
                .get_all_keys(Some(range), None)
                .await
                .map_err(|e| StorageError::ReadError(e.to_string()))?;

            Ok(keys
                .into_iter()
                .filter_map(|k| k.as_string().map(StorageKey::try_from))
                .flatten()
                .collect())
        }

        fn get_display_path(&self, key: &StorageKey) -> String {
            format!("indexeddb://monsoon_emulator/{}", Self::key_string(key))
        }

        fn key_to_path(&self, _: Option<&StorageKey>) -> Option<PathBuf> { None }
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
#[must_use]
pub fn get_storage() -> impl Storage { NativeStorage::new() }

/// Get the platform-appropriate storage implementation
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn get_storage() -> impl Storage { WasmStorage::new() }

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a storage key for a quicksave
#[must_use]
pub fn quicksave_key(game_name: &str, timestamp: &str) -> StorageKey {
    quicksave_prefix(game_name) + format!("quicksave_{timestamp}.sav")
}

/// Generate a storage key for an autosave
#[must_use]
pub fn autosave_key(game_name: &str, timestamp: &str) -> StorageKey {
    autosave_prefix(game_name) + format!("autosave_{timestamp}.sav")
}

/// Generate a storage key for a cached uploaded savestate
#[must_use]
pub fn uploaded_savestate_key(filename: &str) -> StorageKey {
    uploaded_savestate_prefix() + filename.to_string()
}

#[must_use]
pub fn palette_cache_key(file_name: String) -> StorageKey { palette_cache_dir() + file_name }

/// Generate the prefix for listing autosaves for a game
#[must_use]
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn autosave_prefix(game_name: &str) -> StorageKey {
    #[allow(clippy::expect_used)]
    StorageKey::try_from(format!("data/saves/{game_name}/autosaves/"))
        .expect("Default autosave prefix is not a valid StorageKey")
}

/// Generate the prefix for listing quicksaves for a game
#[must_use]
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn quicksave_prefix(game_name: &str) -> StorageKey {
    #[allow(clippy::expect_used)]
    StorageKey::try_from(format!("data/saves/{game_name}/quicksaves/"))
        .expect("Default quicksave prefix is not a valid StorageKey")
}

/// Generate the prefix for uploaded savestates
#[must_use]
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn uploaded_savestate_prefix() -> StorageKey {
    #[allow(clippy::expect_used)]
    StorageKey::from_str("cache/saves/")
        .expect("Default uploaded savestate directory is not a valid StorageKey")
}

/// Generate a storage key for the application config
#[must_use]
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn config_key() -> StorageKey {
    #[allow(clippy::expect_used)]
    StorageKey::from_str("config/config.toml")
        .expect("Default config directory is not a valid StorageKey")
}

/// Generate a storage key for egui state
#[must_use]
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn egui_state_key() -> StorageKey {
    #[allow(clippy::expect_used)]
    StorageKey::from_str("config/egui_state/")
        .expect("Default egui state directory is not a valid StorageKey")
}

/// Generate a storage key for a cached ROM file
#[must_use]
pub fn rom_cache_key(filename: &str) -> StorageKey { rom_cache_dir() + filename.to_string() }

/// Generate the prefix for listing all cached ROMs
#[must_use]
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn rom_cache_dir() -> StorageKey {
    #[allow(clippy::expect_used)]
    StorageKey::from_str("cache/roms/")
        .expect("Default rom cache directory is not a valid StorageKey")
}

/// Generate a storage key for the cached ROM-info database binary
#[must_use]
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn db_cache_key() -> StorageKey {
    #[allow(clippy::expect_used)]
    StorageKey::from_str("cache/rom-info-db.bin")
        .expect("Default RomDB cache key is not a valid StorageKey")
}

#[must_use]
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn palette_cache_dir() -> StorageKey {
    #[allow(clippy::expect_used)]
    StorageKey::try_from("cache/palettes/".to_string())
        .expect("Default palette cache directory is not a valid StorageKey")
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
        NativeStorage, Storage, StorageError, StorageKey, StorageResult,
    };

    /// Global storage instance for synchronous access
    static STORAGE: std::sync::OnceLock<NativeStorage> = std::sync::OnceLock::new();

    fn get_storage_instance() -> &'static NativeStorage { STORAGE.get_or_init(NativeStorage::new) }

    /// Get the full filesystem path for a storage key (native only)
    #[must_use]
    pub fn get_path_for_key(key: &StorageKey) -> Option<std::path::PathBuf> {
        get_storage_instance().key_to_path(Some(key))
    }

    /// Read data synchronously from storage
    pub fn read_sync(key: &StorageKey) -> StorageResult<Vec<u8>> {
        let storage = get_storage_instance();
        let path = storage
            .key_to_path(Some(key))
            .ok_or(StorageError::NotAvailable)?;

        if !path.exists() {
            return Err(StorageError::NotFound);
        }

        std::fs::read(&path).map_err(|e| StorageError::ReadError(e.to_string()))
    }

    /// Write data synchronously to storage
    pub fn write_sync(key: &StorageKey, data: &[u8]) -> StorageResult<()> {
        let storage = get_storage_instance();
        let path = storage
            .key_to_path(Some(key))
            .ok_or(StorageError::NotAvailable)?;

        // Create parent directories
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| StorageError::WriteError(e.to_string()))?;
        }

        std::fs::write(&path, data).map_err(|e| StorageError::WriteError(e.to_string()))
    }

    /// Delete data synchronously from storage
    pub fn delete_sync(key: &StorageKey) -> StorageResult<()> {
        let storage = get_storage_instance();
        let path = storage
            .key_to_path(Some(key))
            .ok_or(StorageError::NotAvailable)?;

        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| StorageError::DeleteError(e.to_string()))?;
        }

        Ok(())
    }

    /// Check if a key exists synchronously
    pub fn exists_sync(key: &StorageKey) -> StorageResult<bool> {
        let storage = get_storage_instance();
        let path = storage
            .key_to_path(Some(key))
            .ok_or(StorageError::NotAvailable)?;

        Ok(path.exists())
    }

    /// List all keys with a given prefix synchronously
    pub fn list_sync(prefix: &StorageKey) -> StorageResult<Vec<StorageKey>> {
        let storage = get_storage_instance();
        let mut results = Vec::new();

        if prefix.path.is_file() {
            results.push(prefix.clone());
        } else {
            storage.collect_files(prefix, &mut results)?;
        }

        Ok(results)
    }

    /// Get the display path for a key
    #[must_use]
    pub fn get_display_path(key: &StorageKey) -> String {
        get_storage_instance().get_display_path(key)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use sync_wrappers::*;
