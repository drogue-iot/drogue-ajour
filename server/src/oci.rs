use crate::index::ImagePullPolicy;
use crate::metadata::Metadata;
use crate::updater::FirmwareStore;
use anyhow::anyhow;
pub use client::{ClientConfig, ClientProtocol};
use lru::LruCache;
use oci_distribution::{client, secrets::RegistryAuth};
use serde_json::Value;
use std::io::Read;
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
        let manifest = self
            .client
            .pull_manifest_and_config(&format!("{}{}", self.prefix, image).parse()?, &self.auth)
            .await;
        match manifest {
            Ok((_, _, config)) => {
                let val: Value = serde_json::from_str(&config)?;
                if let Some(annotation) = val["config"]["Labels"]["io.drogue.metadata"].as_str() {
                    let metadata: Metadata = serde_json::from_str(&annotation)?;
                    self.metadata_cache
                        .put(image.to_string(), (Instant::now(), metadata.clone()));
                    Ok(Some(metadata))
                } else {
                    Err(anyhow!("Unable to locate metadata in image config"))
                }
            }
            Err(e) => Err(e),
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

        let manifest = self
            .client
            .pull(
                &format!("{}{}", self.prefix, image).parse()?,
                &self.auth,
                vec!["application/vnd.oci.image.layer.v1.tar+gzip"],
            )
            .await;
        match manifest {
            Ok(image) => {
                let layer = &image.layers[0];
                let mut decompressed = Vec::new();
                let mut d = flate2::read::GzDecoder::new(&layer.data[..]);
                d.read_to_end(&mut decompressed)?;

                let mut archive = tar::Archive::new(&decompressed[..]);
                let mut entries = archive.entries()?;
                loop {
                    if let Some(entry) = entries.next() {
                        let mut entry = entry?;
                        let path = entry.path()?;
                        if let Some(p) = path.to_str() {
                            if p == "firmware" {
                                let mut payload = Vec::new();
                                entry.read_to_end(&mut payload)?;
                                self.firmware_cache
                                    .put(metadata.checksum.clone(), payload.clone());
                                return Ok(payload);
                            }
                        }
                    } else {
                        break;
                    }
                }
                Err(anyhow!("Error locating firmware"))
            }
            Err(e) => Err(e),
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
