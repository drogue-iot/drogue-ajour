use cloudevents::{event::AttributeValue, Data, Event};
use embedded_update::Status;

use futures::stream::StreamExt;
use paho_mqtt as mqtt;

use crate::updater::Updater;

pub struct Server {
    client: mqtt::AsyncClient,
    group_id: Option<String>,
    applications: Vec<String>,
    updater: Updater,
}

impl Server {
    pub fn new(
        client: mqtt::AsyncClient,
        group_id: Option<String>,
        applications: Vec<String>,
        updater: Updater,
    ) -> Self {
        Self {
            client,
            group_id,
            applications,
            updater,
        }
    }

    pub async fn run(&mut self) -> Result<(), anyhow::Error> {
        let mut stream = self.client.get_stream(100);
        for application in self.applications.iter() {
            if let Some(group_id) = &self.group_id {
                self.client
                    .subscribe(format!("$shared/{}/app/{}", &group_id, &application), 1);
            } else {
                self.client.subscribe(format!("app/{}", &application), 1);
            }
        }
        loop {
            if let Some(m) = stream.next().await {
                if let Some(m) = m {
                    match serde_json::from_slice::<Event>(m.payload()) {
                        Ok(e) => {
                            let mut application = String::new();
                            let mut device = String::new();
                            let mut sender = String::new();
                            let mut subject = String::new();
                            for a in e.iter() {
                                log::trace!("Attribute {:?}", a);
                                if a.0 == "subject" {
                                    if let AttributeValue::String(s) = a.1 {
                                        subject = s.to_string();
                                    }
                                } else if a.0 == "device" {
                                    if let AttributeValue::String(d) = a.1 {
                                        device = d.to_string();
                                    }
                                } else if a.0 == "application" {
                                    if let AttributeValue::String(d) = a.1 {
                                        application = d.to_string();
                                    }
                                } else if a.0 == "sender" {
                                    if let AttributeValue::String(d) = a.1 {
                                        sender = d.to_string();
                                    }
                                }
                            }

                            let is_dfu = if sender == "ttn-gateway" {
                                if subject == "223" {
                                    subject = format!("port:{}", subject);
                                    true
                                } else {
                                    false
                                }
                            } else {
                                subject == "dfu"
                            };

                            log::trace!(
                                "Event from app {}, device {}, is dfu: {}",
                                application,
                                device,
                                is_dfu
                            );

                            if is_dfu {
                                let mut temporary = Vec::new();
                                let status: Option<Result<Status, anyhow::Error>> = if let Some(d) =
                                    e.data()
                                {
                                    match d {
                                        Data::Binary(b) => Some(
                                            serde_cbor::from_slice(&b[..]).map_err(|e| e.into()),
                                        ),
                                        Data::String(s) => {
                                            Some(serde_json::from_str(&s).map_err(|e| e.into()))
                                        }
                                        Data::Json(v) => {
                                            // Extract lorawan payload
                                            if sender == "ttn-gateway" {
                                                // TODO: Refactor/make it functional
                                                if let Some(uplink) = v.get("uplink_message") {
                                                    if let Some(frm) = uplink.get("frm_payload") {
                                                        if let Some(s) = frm.as_str() {
                                                            if let Ok(b) = base64::decode(s) {
                                                                temporary.extend_from_slice(&b[..]);
                                                                let s: Option<Status> =
                                                                    serde_cbor::from_slice(
                                                                        &temporary[..],
                                                                    )
                                                                    .unwrap_or(None);
                                                                s.map(|s| Ok(s))
                                                            } else {
                                                                None
                                                            }
                                                        } else {
                                                            None
                                                        }
                                                    } else {
                                                        None
                                                    }
                                                } else {
                                                    None
                                                }
                                            } else {
                                                Some(
                                                    serde_json::from_str(v.as_str().unwrap())
                                                        .map_err(|e| e.into()),
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    None
                                };

                                log::trace!("Status decode: {:?}", status);

                                if let Some(Ok(status)) = status {
                                    log::info!(
                                        "Device {}/{} running version {:?}",
                                        application,
                                        device,
                                        status.version
                                    );
                                    log::debug!("Received status from {}: {:?}", device, status);
                                    if let Ok(command) =
                                        self.updater.process(&application, &device, &status).await
                                    {
                                        //log::trace!("Sending command to {}: {:?}", device, command);

                                        let topic = format!(
                                            "command/{}/{}/{}",
                                            application, device, subject
                                        );
                                        let message =
                                            mqtt::Message::new(topic, command.as_bytes(), 1);
                                        if let Err(e) = self.client.publish(message).await {
                                            log::warn!(
                                                "Error publishing command back to device: {:?}",
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Error parsing event: {:?}", e);
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
