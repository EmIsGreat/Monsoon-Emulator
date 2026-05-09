use std::collections::HashMap;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RomDb {
    pub version: String,
    pub data: HashMap<[u8; 32], RomDbEntry>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RomDbEntry {
    pub name: String,
    pub hash: [u8; 32],
    pub header: [u8; 16],
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DbParseError {
    Invalid,
    IOError,
    AllOptionsFailed,
    NotSet,
}

impl RomDb {
    pub fn from_xml(xml: &str) -> Result<Self, DbParseError> {
        Ok(RomDb {
            data: HashMap::new(),
            version: "1".to_string(),
        })
    }
}

impl Default for RomDb {
    fn default() -> Self { RomDb::from_xml(include_str!("../assets/no-intro-db.xml")).unwrap() }
}
