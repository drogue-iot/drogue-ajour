use crate::metadata::Metadata;
use crate::updater::FirmwareStore;
use ajour_schema::*;
use anyhow::anyhow;
pub use client::{ClientConfig, ClientProtocol};
use lru::LruCache;
use oci_distribution::{client, secrets::RegistryAuth, Reference};
use tokio::time::{Duration, Instant};

pub struct OciClient {
    prefix: String,
    auth: RegistryAuth,
    client: client::Client,

    // Cache of metadata
    metadata_cache: LruCache<String, (Instant, Metadata)>,
    metadata_cache_expiry: Option<Duration>,

    // Cached by checksum
    firmware_cache: LruCache<String, Vec<u8>>,
}

impl OciClient {
    pub fn new(
        config: ClientConfig,
        prefix: String,
        user: Option<String>,
        token: Option<String>,
        cache_size: usize,
        metadata_cache_expiry: Option<Duration>,
    ) -> Self {
        Self {
            client: client::Client::new(config),
            prefix,
            auth: token
                .map(|t| RegistryAuth::Basic(user.unwrap_or("".to_string()), t))
                .unwrap_or(RegistryAuth::Anonymous),
            metadata_cache: LruCache::new(cache_size),
            firmware_cache: LruCache::new(cache_size),
            metadata_cache_expiry,
        }
    }

    pub async fn fetch_metadata(
        &mut self,
        image: &str,
        image_pull_policy: ImagePullPolicy,
    ) -> Result<Option<Metadata>, anyhow::Error> {
        if let ImagePullPolicy::IfNotPresent = image_pull_policy {
            // Attempt cache lookup
            if let Some((inserted, entry)) = self.metadata_cache.get(image) {
                // Discard outdated items, let the LRU logic clean them out eventually
                if let Some(expiry) = self.metadata_cache_expiry {
                    let oldest = Instant::now() - expiry;
                    if inserted > &oldest {
                        log::debug!("Found metadata cache entry for {}", image);
                        return Ok(Some(entry.clone()));
                    } else {
                        log::debug!("Found expired entry for {}, fetching new", image);
                    }
                } else {
                    log::debug!("Found metadata cache entry for {}", image);
                    return Ok(Some(entry.clone()));
                }
            }
        }
        let imageref = format!("{}{}", self.prefix, image).parse()?;
        let manifest = self.client.pull_image_manifest(&imageref, &self.auth).await;
        match manifest {
            Ok((manifest, _)) => {
                for layer in manifest.layers.iter() {
                    if layer.media_type == "application/octet-stream" {
                        let metadata: Metadata = Metadata {
                            version: imageref.tag().unwrap_or("").as_bytes().to_vec(),
                            checksum: layer.digest.clone(),
                            size: layer.size as u32,
                        };
                        self.metadata_cache
                            .put(image.to_string(), (Instant::now(), metadata.clone()));
                        return Ok(Some(metadata));
                    }
                }
                Err(anyhow!("Unable to locate metadata in image config"))
            }
            Err(e) => {
                log::info!("Error pulling manifest: {:?}", e);
                Err(e.into())
            }
        }
    }

    pub async fn fetch_firmware(
        &mut self,
        image: &str,
        metadata: &Metadata,
        image_pull_policy: ImagePullPolicy,
    ) -> Result<Vec<u8>, anyhow::Error> {
        if let ImagePullPolicy::IfNotPresent = image_pull_policy {
            if let Some(firmware) = self.firmware_cache.get(&metadata.checksum) {
                log::debug!("Found firmware cache entry for {}", image);
                return Ok(firmware.clone());
            }
        }

        let imageref: Reference = format!("{}{}", self.prefix, image).parse()?;
        let mut payload = Vec::new();
        let manifest = self
            .client
            .pull_blob(&imageref, &metadata.checksum, &mut payload)
            .await;
        match manifest {
            Ok(()) => {
                self.firmware_cache
                    .put(metadata.checksum.clone(), payload.clone());
                return Ok(payload);
            }
            Err(e) => Err(e.into()),
        }
    }
}

#[async_trait::async_trait]
impl FirmwareStore for OciClient {
    type Params = (String, ImagePullPolicy);
    async fn fetch_metadata(
        &mut self,
        params: &Self::Params,
    ) -> Result<(Self::Context, Option<Metadata>), anyhow::Error> {
        let m = OciClient::fetch_metadata(self, &params.0, params.1).await?;
        Ok(((), m))
    }

    async fn update_progress(
        &mut self,
        _: &Self::Params,
        _: &Self::Context,
        _: u32,
        _: u32,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    type Context = ();
    async fn fetch_firmware(
        &mut self,
        params: &Self::Params,
        _: &Self::Context,
        metadata: &Metadata,
    ) -> Result<Vec<u8>, anyhow::Error> {
        OciClient::fetch_firmware(self, &params.0, metadata, params.1).await
    }

    async fn mark_synced(
        &mut self,
        _: &Self::Params,
        _: &Self::Context,
        _: bool,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }
}
