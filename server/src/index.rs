use drogue_client::{dialect, openid::AccessTokenProvider, Section, Translator};

use crate::oci::Metadata;
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
    HAWKBIT,
}

dialect!(FirmwareStatus [Section::Status => "firmware"]);

#[derive(Serialize, Deserialize, Debug)]
pub struct FirmwareStatus {
    state: FirmwareStatusState,
    current: String,
    target: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum FirmwareStatusState {
    /// Device is up to date with latest version
    InSync,
    /// Device update is in progress
    InProgress {
        /// Number between 0 and 100 indicating how far in the update process
        progress: f32,
    },
    /// Device is out of date
    OutOfDate,
    /// Error during update
    Error { description: String },
}

impl FirmwareStatus {
    pub fn error(status: &Status, error: String) -> Self {
        Self {
            state: FirmwareStatusState::Error { description: error },
            current: status.version.clone(),
            target: String::new(),
        }
    }

    pub fn new(status: &Status, metadata: &Metadata) -> Self {
        if status.version == metadata.version {
            Self {
                state: FirmwareStatusState::InSync,
                current: status.version.clone(),
                target: metadata.version.clone(),
            }
        } else {
            if let Some(update) = &status.update {
                let progress = 100.0 * (update.offset as f32 / metadata.size as f32);
                Self {
                    state: FirmwareStatusState::InProgress { progress },
                    current: status.version.clone(),
                    target: metadata.version.clone(),
                }
            } else {
                Self {
                    state: FirmwareStatusState::OutOfDate,
                    current: status.version.clone(),
                    target: metadata.version.clone(),
                }
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

    pub async fn update_state(
        &self,
        application: &str,
        device: &str,
        status: FirmwareStatus,
    ) -> Result<(), anyhow::Error> {
        if let Some(mut device) = self.client.get_device(application, device).await? {
            device.set_section::<FirmwareStatus>(status)?;
            self.client.update_device(&device).await?;
        }
        Ok(())
    }
}
