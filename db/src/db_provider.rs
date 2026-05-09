use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use monsoon_core::emulation::rom::rom_db::{DbParseError, RomDb};

#[derive(Default)]
pub struct DbProvider {
    db: RomDb,
}

impl DbProvider {
    pub fn builder() -> DbProviderBuilder { DbProviderBuilder::default() }

    pub fn database(&self) -> &RomDb { &self.db }
}

#[derive(Debug, Default)]
pub struct DbProviderBuilder {
    #[cfg(feature = "online")]
    update_url: Option<String>,
    local_path: Option<PathBuf>,
    direct_string: Option<String>,
}

impl DbProviderBuilder {
    #[cfg(feature = "online")]
    pub fn with_update_url(mut self, url: &str) -> Self {
        self.update_url = Some(url.to_string());
        self
    }

    pub fn with_local_path(mut self, path: &Path) -> Self {
        self.local_path = Some(path.to_path_buf());
        self
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn from_string(mut self, string: &str) -> Self {
        self.direct_string = Some(string.to_string());
        self
    }

    pub async fn build(self) -> Result<DbProvider, DbParseError> {
        #[cfg(feature = "online")]
        let url = if let Some(url) = self.update_url {
            let resp = reqwest::get(url).await;

            if let Ok(resp) = resp {
                let data = resp.text().await;

                if let Ok(data) = data {
                    RomDb::from_xml(data.as_str())
                } else {
                    Err(DbParseError::IOError)
                }
            } else {
                Err(DbParseError::IOError)
            }
        } else {
            Err(DbParseError::NotSet)
        };

        #[cfg(feature = "online")]
        if let Ok(db) = url {
            return Ok(DbProvider {
                db,
            });
        };

        let local = if let Some(path) = self.local_path {
            let file = File::open(path);

            if let Ok(mut file) = file {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    RomDb::from_xml(content.as_str())
                } else {
                    Err(DbParseError::IOError)
                }
            } else {
                Err(DbParseError::IOError)
            }
        } else {
            Err(DbParseError::NotSet)
        };

        if let Ok(db) = local {
            return Ok(DbProvider {
                db,
            });
        };

        let direct = if let Some(direct_string) = self.direct_string {
            RomDb::from_xml(direct_string.as_str())
        } else {
            Err(DbParseError::NotSet)
        };

        if let Ok(db) = direct {
            return Ok(DbProvider {
                db,
            });
        };

        Err(DbParseError::AllOptionsFailed)
    }
}
