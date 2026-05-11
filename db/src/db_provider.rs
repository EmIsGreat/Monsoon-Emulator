use std::path::{Path, PathBuf};
use std::sync::Arc;

use monsoon_core::rom_db::{DbParseError, RomDb};

#[cfg(feature = "online")]
use crate::manifest::Manifest;
use crate::manifest::is_newer;

#[derive(Clone)]
enum Source {
    BuiltIn(Arc<RomDb>),
    Local {
        bytes: Vec<u8>,
    },
    #[cfg(feature = "online")]
    Remote {
        url: String,
    },
}

#[derive(Clone)]
struct Candidate {
    version: String,
    preference: u8,
    source: Source,
}

#[derive(Default)]
pub struct DbProvider {
    db: Arc<RomDb>,
}

impl DbProvider {
    pub fn builder() -> DbProviderBuilder { DbProviderBuilder::default() }

    pub fn database(&self) -> Arc<RomDb> { Arc::clone(&self.db) }
}

#[derive(Debug, Default)]
pub struct DbProviderBuilder {
    #[cfg(feature = "online")]
    update_url: Option<String>,
    cache_path: Option<PathBuf>,
    fallback: Option<Arc<RomDb>>,
}

impl DbProviderBuilder {
    #[cfg(feature = "online")]
    pub fn with_update_url(mut self, url: &str) -> Self {
        self.update_url = Some(url.to_string());
        self
    }

    pub fn with_cache_path(mut self, path: &Path) -> Self {
        self.cache_path = Some(path.to_path_buf());
        self
    }

    pub fn with_fallback(mut self, data: Arc<RomDb>) -> Self {
        self.fallback = Some(data);
        self
    }

    pub async fn build(self) -> Result<DbProvider, DbParseError> {
        let DbProviderBuilder {
            #[cfg(feature = "online")]
            update_url,
            cache_path,
            fallback,
        } = self;

        let mut candidates = Vec::new();

        if let Some(fallback) = fallback {
            candidates.push(Candidate {
                version: fallback.version.clone(),
                preference: 0,
                source: Source::BuiltIn(fallback),
            });
        }

        if let Some(path) = cache_path.as_ref()
            && let Ok(bytes) = std::fs::read(path)
            && let Ok(db) = RomDb::deserialize(&bytes)
        {
            candidates.push(Candidate {
                version: db.version,
                preference: 1,
                source: Source::Local {
                    bytes,
                },
            });
        }

        #[cfg(feature = "online")]
        if let Some(update_url) = update_url.as_ref()
            && let Some((version, db_url)) = fetch_manifest_info(update_url).await
        {
            candidates.push(Candidate {
                version,
                preference: 2,
                source: Source::Remote {
                    url: db_url,
                },
            });
        }

        if candidates.is_empty() {
            return Err(DbParseError::AllOptionsFailed);
        }

        candidates.sort_by(|a, b| {
            if is_newer(&a.version, &b.version) {
                std::cmp::Ordering::Less
            } else if is_newer(&b.version, &a.version) {
                std::cmp::Ordering::Greater
            } else {
                a.preference.cmp(&b.preference)
            }
        });

        let mut last_error = DbParseError::AllOptionsFailed;

        for candidate in candidates {
            match try_load_source(candidate.source, cache_path.as_deref()).await {
                Ok(db) => {
                    return Ok(DbProvider {
                        db,
                    });
                }
                Err(err) => last_error = err,
            }
        }

        Err(last_error)
    }
}

async fn try_load_source(
    source: Source,
    cache_path: Option<&Path>,
) -> Result<Arc<RomDb>, DbParseError> {
    match source {
        Source::BuiltIn(db) => Ok(db),
        Source::Local {
            bytes,
        } => RomDb::deserialize(&bytes).map(Arc::new),
        #[cfg(feature = "online")]
        Source::Remote {
            url,
        } => {
            let response = reqwest::get(url).await.map_err(|_| DbParseError::IOError)?;
            let bytes = response.bytes().await.map_err(|_| DbParseError::IOError)?;
            let db = RomDb::deserialize(&bytes)?;

            if let Some(path) = cache_path {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(path, &bytes);
            }

            Ok(Arc::new(db))
        }
    }
}

#[cfg(feature = "online")]
async fn fetch_manifest_info(update_url: &str) -> Option<(String, String)> {
    let manifest_url = reqwest::Url::parse(update_url).ok()?;
    let response = reqwest::get(update_url).await.ok()?;
    let text = response.text().await.ok()?;
    let manifest: Manifest = serde_json::from_str(&text).ok()?;
    let db_url = manifest_url
        .join(&manifest.rom_info_db.url)
        .ok()?
        .to_string();

    Some((manifest.rom_info_db.version, db_url))
}
