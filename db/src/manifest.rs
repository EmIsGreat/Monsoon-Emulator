#[cfg(feature = "online")]
#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize)]
pub struct ManifestEntry {
    pub version: String,
    pub url: String,
}

#[cfg(feature = "online")]
#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize)]
pub struct Manifest {
    #[serde(rename = "rom-info-db")]
    pub rom_info_db: ManifestEntry,
}

pub fn is_newer(a: &str, b: &str) -> bool { a > b }
