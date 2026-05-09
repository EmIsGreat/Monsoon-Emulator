#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RomDb {
    pub data: String,
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
            data: xml.to_string(),
        })
    }
}

impl Default for RomDb {
    fn default() -> Self {
        RomDb {
            data: include_str!("../../../assets/no-intro-db.xml").to_string(),
        }
    }
}
