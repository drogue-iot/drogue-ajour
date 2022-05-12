use crate::metadata::Metadata;
use crate::updater::FirmwareStore;
use serde_json::json;
use std::time::Duration;

pub struct HawkbitClient {
    client: reqwest::Client,
    url: String,
    tenant: String,
    token: String,

    // Cache of metadata
    metadata_cache: LruCache<String, (Instant, Metadata)>,
    metadata_cache_expiry: Option<Duration>,

    // Cached by checksum
    firmware_cache: LruCache<String, Vec<u8>>,
}

pub enum PollResult {
    Wait(Duration),
    Deployment(Deployment),
}

pub struct Deployment {
    id: String,
    path: String,
}

impl HawkbitClient {
    pub fn new(url: &str, tenant: &str, token: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.to_string(),
            tenant: tenant.to_string(),
            token: token.to_string(),
        }
    }

    pub async fn register(&self, controller: &str) -> std::io::Result<()> {
        let url = format!(
            "{}/{}/controller/v1/{}",
            &self.url, &self.tenant, controller
        );

        // TODO: Get attributes from somewhere
        let attributes = json! {{
          "mode": "merge",
          "data": {
            "VIN": "JH4TB2H26CC000001",
            "hwRevision": "1"
          },
          "status": {
            "result": {
              "finished": "success"
            },
            "execution": "closed",
            "details": []
          }
        }};

        let res = self
            .client
            .put(&url)
            .header("Authorization", &format!("GatewayToken {}", &self.token))
            .header("Accept", "application/hal+json")
            .json(&attributes)
            .send()
            .await;
        match res {
            Ok(_) => {
                log::debug!("Successfully set attributes");
            }
            Err(e) => {
                log::info!("Error setting attributes: {:?}", e);
            }
        }
        Ok(())
    }

    async fn fetch_firmware(&self, d: &Deployment) -> Result<Vec<u8>, anyhow::Error> {
        let res = self
            .client
            .get(d.path.to_string())
            .header("Authorization", &format!("GatewayToken {}", &self.token))
            .header("Accept", "application/hal+json")
            .send()
            .await?
            .bytes()
            .await?;
        Ok(res.as_ref().into())
    }

    async fn provide_feedback(
        &self,
        controller: &str,
        deployment: &Deployment,
        success: bool,
    ) -> Result<(), anyhow::Error> {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/{}/controller/v1/{}/deploymentBase/{}/feedback",
            &self.url, &self.tenant, controller, deployment.id
        );

        let feedback = json! {
            {
                "id": deployment.id,
                "status": {
                    "result": {
                        "finished": if success { "success" } else { "failed" },
                    },
                    "execution": "closed",
                    "details": ["Update was successfully installed."],
                }

            }
        };

        client
            .post(&url)
            .header("Authorization", &format!("GatewayToken {}", &self.token))
            .header("Accept", "application/hal+json")
            .json(&feedback)
            .send()
            .await?;

        Ok(())
    }

    async fn read_metadata(&self, path: &str) -> Result<(Metadata, Deployment), anyhow::Error> {
        let client = reqwest::Client::new();
        let res: serde_json::Value = client
            .get(path)
            .header("Authorization", &format!("GatewayToken {}", &self.token))
            .header("Accept", "application/hal+json")
            .send()
            .await?
            .json()
            .await?;

        let id = res["id"].as_str().unwrap().to_string();
        let chunks = &res["deployment"]["chunks"];
        let chunk = &chunks[0];
        let version = chunk["version"].as_str().unwrap().to_string();
        let artifact = &chunk["artifacts"][0];
        let size: usize = artifact["size"].as_i64().unwrap() as usize;
        let path = artifact["_links"]["download-http"]["href"]
            .as_str()
            .unwrap();
        let metadata = Metadata {
            checksum: String::new(),
            version,
            size: size as u32,
        };

        let deployment = Deployment {
            id,
            path: path.to_string(),
        };
        Ok((metadata, deployment))
    }

    pub async fn fetch_metadata(
        &self,
        controller: &str,
    ) -> Result<(PollResult, Option<Metadata>), anyhow::Error> {
        let url = format!(
            "{}/{}/controller/v1/{}",
            &self.url, &self.tenant, controller
        );
        let res = self
            .client
            .get(&url)
            .header("Authorization", &format!("GatewayToken {}", &self.token))
            .header("Accept", "application/hal+json")
            .send()
            .await?;

        let j: serde_json::Value = res.json().await.unwrap();

        // If we have a deployment base, return download link
        if let Some(links) = j.get("_links") {
            if let Some(base) = links.get("deploymentBase") {
                if let Some(href) = base.get("href") {
                    if let Some(path) = href.as_str() {
                        let (m, p) = self.read_metadata(path).await?;
                        return Ok((PollResult::Deployment(p), Some(m)));
                    }
                }
            }
        }

        let poll = j["config"]["polling"]["sleep"].as_str().unwrap();
        let mut s = poll.splitn(3, ":");
        let mut dur = chrono::Duration::zero();
        if let Some(d) = s.next() {
            dur = dur + chrono::Duration::days(d.parse::<i64>().unwrap());
        }

        if let Some(h) = s.next() {
            dur = dur + chrono::Duration::hours(h.parse::<i64>().unwrap());
        }

        if let Some(s) = s.next() {
            dur = dur + chrono::Duration::seconds(s.parse::<i64>().unwrap());
        }
        let s = dur.to_std().unwrap();
        Ok((PollResult::Wait(s), None))
    }
}

#[async_trait::async_trait]
impl FirmwareStore for HawkbitClient {
    type Params = String;
    async fn fetch_metadata(
        &mut self,
        params: &Self::Params,
    ) -> Result<(Self::Context, Option<Metadata>), anyhow::Error> {
        HawkbitClient::fetch_metadata(self, &params).await
    }

    fn get_backoff(&self, context: &Self::Context) -> Option<u32> {
        if let PollResult::Wait(w) = context {
            Some(w.as_secs() as u32)
        } else {
            None
        }
    }

    async fn mark_synced(
        &mut self,
        params: &Self::Params,
        context: &Self::Context,
        success: bool,
    ) -> Result<(), anyhow::Error> {
        if let PollResult::Deployment(d) = context {
            self.provide_feedback(params, d, success).await?;
        }
        Ok(())
    }

    type Context = PollResult;
    async fn fetch_firmware(
        &mut self,
        _: &Self::Params,
        context: &Self::Context,
        _: &Metadata,
    ) -> Result<Vec<u8>, anyhow::Error> {
        if let PollResult::Deployment(d) = context {
            HawkbitClient::fetch_firmware(self, d).await
        } else {
            Err(anyhow::anyhow!("Unexpected PollResult"))
        }
    }
}
