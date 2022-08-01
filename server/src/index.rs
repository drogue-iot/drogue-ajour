use ajour_schema::*;
use drogue_client::{core::v1::ConditionStatus, Translator};

use crate::metadata::Metadata;
use embedded_update::Status;

pub type DrogueClient = drogue_client::registry::v1::Client;

#[derive(Clone)]
pub struct Index {
    client: DrogueClient,
}

fn update_status(fwstatus: &mut FirmwareStatus, status: &Status, data: Result<&Metadata, String>) {
    match data {
        Ok(metadata) => {
            fwstatus.current = core::str::from_utf8(&status.version)
                .unwrap_or("Unknown")
                .to_string();
            fwstatus.target = core::str::from_utf8(&metadata.version)
                .unwrap_or("Unknown")
                .to_string();
            if status.version == metadata.version {
                fwstatus.conditions.clear();
                fwstatus.conditions.update("InSync", true);
            } else {
                fwstatus.conditions.update("InSync", false);

                if let Some(update) = &status.update {
                    let progress = 100.0 * (update.offset as f32 / metadata.size as f32);
                    fwstatus.conditions.update(
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
            fwstatus.conditions.clear();
            fwstatus.current = core::str::from_utf8(&status.version)
                .unwrap_or("Unknown")
                .to_string();
            fwstatus.target = "Unknown".to_string();
            fwstatus.conditions.update(
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
        status: &Status<'_>,
        data: Result<&Metadata, String>,
    ) -> Result<(), anyhow::Error> {
        if let Some(mut device) = self.client.get_device(application, device).await? {
            let mut s: FirmwareStatus = device
                .section::<FirmwareStatus>()
                .unwrap_or(Ok(Default::default()))?;

            //     .unwrap_or(Ok(Default::default()))?;
            update_status(&mut s, status, data);
            device.set_section::<FirmwareStatus>(s)?;
            self.client.update_device(&device).await?;
        }
        Ok(())
    }
}
