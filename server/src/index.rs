use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use anyhow::{anyhow, Context};
use clap::Parser;
use cloudevents::{event::AttributeValue, Data, Event};
use drogue_ajour_protocol::{Command, Status};
use drogue_client::{dialect, openid::AccessTokenProvider, Section, Translator};
use futures::{stream::StreamExt, TryFutureExt};
use paho_mqtt as mqtt;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub type DrogueClient = drogue_client::registry::v1::Client<AccessTokenProvider>;

#[derive(Clone)]
pub struct Index {
    client: DrogueClient,
}

dialect!(FirmwareSpec [Section::Spec => "firmware"]);

#[derive(Serialize, Deserialize, Debug)]
pub enum FirmwareSpec {
    #[serde(rename = "oci")]
    OCI { image: String },
    #[serde(rename = "hawkbit")]
    HAWKBIT,
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
            log::info!("WE GOT DEVICE {:?}", device);
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
}
