use std::sync::Arc;

use monsoon_core::rom_db::{DbParseError, RomDb};

#[cfg(feature = "online")]
use crate::manifest::Manifest;
use crate::manifest::is_newer;

#[derive(Clone, Debug)]
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

/// Builder for [`DbProvider`].
///
/// Cache I/O is now the caller's responsibility:
/// - Supply previously-cached bytes via [`with_cache_bytes`].
/// - After a successful [`build`], check the returned `Option<Vec<u8>>`.  If
///   `Some`, those are freshly-fetched remote bytes that the caller should
///   persist for next time.
#[derive(Debug, Default)]
pub struct DbProviderBuilder {
    #[cfg(feature = "online")]
    update_url: Option<String>,
    /// Bytes read from the cache by the caller before invoking `build`.
    cache_bytes: Option<Vec<u8>>,
    fallback: Option<Arc<RomDb>>,
}

impl DbProviderBuilder {
    #[cfg(feature = "online")]
    pub fn with_update_url(mut self, url: &str) -> Self {
        self.update_url = Some(url.to_string());
        self
    }

    /// Supply previously-cached database bytes.  The bytes will be parsed and,
    /// if valid, treated as a `Local` candidate during selection.
    pub fn with_cache_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.cache_bytes = Some(bytes);
        self
    }

    pub fn with_fallback(mut self, data: Arc<RomDb>) -> Self {
        self.fallback = Some(data);
        self
    }

    /// Build the provider.
    ///
    /// Returns `(provider, bytes_to_cache)`.  When `bytes_to_cache` is
    /// `Some(bytes)`, the caller should persist those bytes so they are
    /// available as cache input on the next run.
    pub async fn build(self) -> Result<(DbProvider, Option<Vec<u8>>), DbParseError> {
        let DbProviderBuilder {
            #[cfg(feature = "online")]
            update_url,
            cache_bytes,
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

        if let Some(bytes) = cache_bytes
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
            match try_load_source(candidate.source.clone()).await {
                Ok((db, bytes_to_cache)) => {
                    return Ok((
                        DbProvider {
                            db,
                        },
                        bytes_to_cache,
                    ));
                }
                Err(err) => last_error = err,
            }
        }

        Err(last_error)
    }
}

/// Returns `(db, bytes_to_cache)`.  `bytes_to_cache` is `Some` only when the
/// data was fetched from a remote source and should be persisted by the caller.
async fn try_load_source(source: Source) -> Result<(Arc<RomDb>, Option<Vec<u8>>), DbParseError> {
    match source {
        Source::BuiltIn(db) => Ok((db, None)),
        Source::Local {
            bytes,
        } => RomDb::deserialize(&bytes).map(|db| (Arc::new(db), None)),
        #[cfg(feature = "online")]
        Source::Remote {
            url,
        } => {
            let response = reqwest::get(url).await.map_err(|_| DbParseError::IOError)?;
            let bytes = response.bytes().await.map_err(|_| DbParseError::IOError)?;
            let bytes_vec = bytes.to_vec();
            let db = RomDb::deserialize(&bytes_vec)?;
            Ok((Arc::new(db), Some(bytes_vec)))
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
