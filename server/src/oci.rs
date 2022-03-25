use anyhow::anyhow;
use oci_distribution::{client, secrets::RegistryAuth};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

pub use client::{ClientConfig, ClientProtocol};

pub struct OciClient {
    prefix: String,
    auth: RegistryAuth,
    client: client::Client,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Metadata {
    pub version: String,
    pub checksum: String,
    pub size: String,
}

impl OciClient {
    pub fn new(
        config: ClientConfig,
        prefix: String,
        user: Option<String>,
        token: Option<String>,
    ) -> Self {
        Self {
            client: client::Client::new(config),
            prefix,
            auth: token
                .map(|t| RegistryAuth::Basic(user.unwrap_or("".to_string()), t))
                .unwrap_or(RegistryAuth::Anonymous),
        }
    }

    pub async fn fetch_metadata(&mut self, image: &str) -> Result<Metadata, anyhow::Error> {
        let manifest = self
            .client
            .pull_manifest_and_config(&format!("{}{}", self.prefix, image).parse()?, &self.auth)
            .await;
        match manifest {
            Ok((_, _, config)) => {
                let val: Value = serde_json::from_str(&config)?;
                if let Some(annotation) = val["config"]["Labels"]["io.drogue.metadata"].as_str() {
                    let metadata: Metadata = serde_json::from_str(&annotation)?;
                    Ok(metadata)
                } else {
                    Err(anyhow!("Unable to locate metadata in image config"))
                }
            }
            Err(e) => Err(e),
        }
    }

    pub async fn fetch_firmware(&mut self, image: &str) -> Result<Vec<u8>, anyhow::Error> {
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
