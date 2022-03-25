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

use crate::index::{FirmwareSpec, Index};
use crate::oci::OciClient;

pub struct Updater {
    index: Index,
    oci: OciClient,
}

impl Updater {
    pub fn new(index: Index, oci: OciClient) -> Self {
        Self { oci, index }
    }
    pub async fn process(
        &mut self,
        application: &str,
        device: &str,
        status: Status,
    ) -> Result<Command, anyhow::Error> {
        if let Some(spec) = self.index.latest_version(application, device).await? {
            match spec {
                FirmwareSpec::OCI { image } => match self.oci.fetch_metadata(&image).await {
                    Ok(metadata) => {
                        if status.version == metadata.version {
                            Ok(Command::new_sync(&status.version, None))
                        } else {
                            let mut offset = 0;
                            let mut mtu = 512;
                            if let Some(m) = status.mtu {
                                mtu = m as usize;
                            }
                            if let Some(update) = status.update {
                                if update.version == metadata.version {
                                    offset = update.offset as usize;
                                }
                            }

                            if offset < metadata.size.parse::<usize>().unwrap() {
                                let firmware = self.oci.fetch_firmware(&image).await?;

                                let to_copy = core::cmp::min(firmware.len() - offset, mtu);
                                let block = &firmware[offset..offset + to_copy];

                                log::trace!(
                                    "Sending firmware block offset {} size {}",
                                    offset,
                                    block.len()
                                );
                                Ok(Command::new_write(&metadata.version, offset as u32, block))
                            } else {
                                let data = hex::decode(&metadata.checksum)?;
                                Ok(Command::new_swap(&metadata.version, &data))
                            }
                        }
                    }
                    Err(e) => Err(e.into()),
                },
                FirmwareSpec::HAWKBIT => {
                    todo!("hawkbit firmware spec no yet supported")
                }
            }
        } else {
            Err(anyhow!("Unable to find latest version for {}", application))
        }
    }
}
