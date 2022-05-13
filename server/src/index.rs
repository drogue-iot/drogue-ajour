use drogue_client::{
    core::v1::{ConditionStatus, Conditions},
    dialect,
    openid::AccessTokenProvider,
    Section, Translator,
};

use crate::metadata::Metadata;
use drogue_ajour_protocol::Status;
use serde::{Deserialize, Serialize};

pub type DrogueClient = drogue_client::registry::v1::Client<AccessTokenProvider>;

#[derive(Clone)]
pub struct Index {
    client: DrogueClient,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum ImagePullPolicy {
    Always,
    IfNotPresent,
}

impl Default for ImagePullPolicy {
    fn default() -> Self {
        Self::IfNotPresent
    }
}

dialect!(FirmwareSpec [Section::Spec => "firmware"]);

#[derive(Serialize, Deserialize, Debug)]
pub enum FirmwareSpec {
    #[serde(rename = "oci")]
    OCI {
        image: String,
        #[serde(rename = "imagePullPolicy", default = "Default::default")]
        image_pull_policy: ImagePullPolicy,
    },
    #[serde(rename = "hawkbit")]
    HAWKBIT { controller: String },
}

dialect!(FirmwareStatus [Section::Status => "firmware"]);

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct FirmwareStatus {
    conditions: Conditions,
    current: String,
    target: String,
}

impl FirmwareStatus {
    pub fn update(&mut self, status: &Status, data: Result<&Metadata, String>) {
        match data {
            Ok(metadata) => {
                self.current = status.version.clone();
                self.target = metadata.version.clone();
                if status.version == metadata.version {
                    self.conditions.clear();
                    self.conditions.update("InSync", true);
                } else {
                    self.conditions.update("InSync", false);

                    if let Some(update) = &status.update {
                        let progress = 100.0 * (update.offset as f32 / metadata.size as f32);
                        self.conditions.update(
                            "UpdateProgress",
                            ConditionStatus {
                                message: Some(format!("{:.2}", progress)),
                                ..Default::default()
                            },
                        );
                    }
                }
            }
            Err(error) => {
                self.conditions.clear();
                self.current = status.version.clone();
                self.target = "Unknown".to_string();
                self.conditions.update(
                    "InSync",
                    ConditionStatus {
                        status: Some(false),
                        message: Some("Error retrieving firmware metadata".to_string()),
                        reason: Some(error),
                        ..Default::default()
                    },
                );
            }
        }
    }
}

impl Index {
    pub fn new(client: DrogueClient) -> Self {
        Self { client }
    }
    pub async fn latest_version(
        &self,
        application: &str,
        device: &str,
    ) -> Result<Option<FirmwareSpec>, anyhow::Error> {
        // Check if we got a device on the device first
        if let Some(device) = self.client.get_device(application, device).await? {
            if let Some(spec) = device.section::<FirmwareSpec>() {
                return Ok(Some(spec?));
            }
        }

        let app = self.client.get_app(application).await?;
        if let Some(app) = app {
            // Check if we've got a device spec first;
            if let Some(spec) = app.section::<FirmwareSpec>() {
                return Ok(Some(spec?));
            }
        }
        Ok(None)
    }

    pub async fn update_status(
        &self,
        application: &str,
        device: &str,
        status: &Status,
        data: Result<&Metadata, String>,
    ) -> Result<(), anyhow::Error> {
        if let Some(mut device) = self.client.get_device(application, device).await? {
            let mut s: FirmwareStatus = device
                .section::<FirmwareStatus>()
                .unwrap_or(Ok(Default::default()))?;

            //     .unwrap_or(Ok(Default::default()))?;
            s.update(status, data);
            device.set_section::<FirmwareStatus>(s)?;
            self.client.update_device(&device).await?;
        }
        Ok(())
    }
}
