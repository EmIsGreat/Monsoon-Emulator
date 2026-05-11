use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use monsoon_core::rom_db::{DbParseError, RomDb};

#[derive(Default)]
pub struct DbProvider {
    db: Arc<RomDb>,
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
    fallback: Option<Arc<RomDb>>,
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

    pub fn with_fallback(mut self, data: Arc<RomDb>) -> Self {
        self.fallback = Some(data);
        self
    }

    pub async fn build(self) -> Result<DbProvider, DbParseError> {
        #[cfg(feature = "online")]
        let url = if let Some(url) = self.update_url {
            let resp = reqwest::get(&url).await;

            if let Ok(resp) = resp {
                let data = resp.bytes().await;

                if let Ok(data) = data {
                    RomDb::deserialize(&data[..])
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
                db: Arc::new(db),
            });
        } else {
            eprintln!("URL deserialization failed: {:?}", url.unwrap_err())
        };

        let local = if let Some(path) = self.local_path {
            let file = File::open(&path);

            if let Ok(mut file) = file {
                let mut buf = Vec::new();
                if file.read(&mut buf).is_ok() {
                    RomDb::deserialize(&buf)
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
                db: Arc::new(db),
            });
        } else {
            println!("{:?}", local.unwrap_err())
        };

        if let Some(fallback) = self.fallback.clone() {
            return Ok(DbProvider {
                db: fallback,
            });
        }

        Err(DbParseError::AllOptionsFailed)
    }
}
