use std::collections::HashMap;
use std::env;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct RomDb {
    pub version: String,
    data: HashMap<[u8; 32], RomDbEntry>,
}

impl RomDb {
    pub fn get_entry(&self, full_hash: &[u8; 32]) -> Option<&RomDbEntry> {
        self.data.get(full_hash)
    }

    pub fn get_entry_by_headerless(&self, headerless_hash: &[u8; 32]) -> Option<&RomDbEntry> {
        self.data
            .iter()
            .find(|(_, i)| {
                if let Some(hash) = i.unheadered_sha256 {
                    hash == *headerless_hash
                } else {
                    false
                }
            })
            .map(|(_, r)| r)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct RomDbEntry {
    pub name: String,
    pub orig_name: Option<String>,
    pub headered_sha256: Option<[u8; 32]>,
    pub unheadered_sha256: Option<[u8; 32]>,
    pub header: Option<Vec<u8>>,
}

impl Hash for RomDb {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.version.hash(state);

        let mut keys: Vec<_> = self.data.keys().collect();
        keys.sort_unstable();
        for key in keys {
            key.hash(state);
            if let Some(entry) = self.data.get(key) {
                entry.hash(state);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum DbParseError {
    Invalid,
    IOError,
    AllOptionsFailed,
    NotSet,
    DeserializationError(postcard::Error),
}

impl From<postcard::Error> for DbParseError {
    fn from(value: postcard::Error) -> Self { DbParseError::DeserializationError(value) }
}

impl RomDb {
    pub fn deserialize(data: &[u8]) -> Result<Self, DbParseError> {
        postcard::from_bytes::<RomDb>(data).map_err(DbParseError::from)
    }
}

impl Default for RomDb {
    fn default() -> Self {
        postcard::from_bytes::<RomDb>(include_bytes!(concat!(env!("OUT_DIR"), "/rom-info-db.bin")))
            .expect("Error deserializing built-in rom db")
    }
}
