use crate::simulator::generators::{self, sine, SingleTarget};
use gloo_storage::{LocalStorage, Storage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::time::Duration;

pub const DEFAULT_CONFIG_KEY: &str = "drogue.io/device-simulator/defaultConfiguration";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub auto_connect: bool,
    pub target: Target,
    pub application: String,
    pub device: String,

    #[serde(default)]
    pub simulations: BTreeMap<String, Simulation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Simulation {
    Sine(generators::sine::Properties),
    Sawtooth(generators::sawtooth::Properties),
    Wave(generators::wave::Properties),
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_connect: false,
            target: Target::Mqtt {
                url: "wss://mqtt-endpoint-ws-browser-drogue-dev.apps.wonderful.iot-playground.org/mqtt".into(),
                credentials: Credentials::Password("my-password".into()),
            },
            application: "my-application".into(),
            device: "my-device".into(),
            simulations: {
                let mut s = BTreeMap::new();
                s.insert("sine1".to_string(), Simulation::Sine(sine::Properties{
                    amplitude: 100f64.into(),
                    length: Duration::from_secs(60),
                    period: Duration::from_secs(1),
                    target: SingleTarget{
                        channel: "state".to_string(),
                        feature: None,
                        property: "value".to_string()
                    }
                }));
                s
            }
        }
    }
}

impl Settings {
    pub fn load() -> Option<anyhow::Result<Self>> {
        let json: Option<String> = LocalStorage::get(DEFAULT_CONFIG_KEY).ok();
        json.map(|json| serde_json::from_str(&json).map_err(|err| anyhow::Error::new(err)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    Mqtt {
        url: String,
        credentials: Credentials,
    },
    Http {
        url: String,
        credentials: Credentials,
    },
}

impl Target {
    pub fn as_protocol(&self) -> Protocol {
        match self {
            Self::Mqtt { .. } => Protocol::Mqtt,
            Self::Http { .. } => Protocol::Http,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Credentials {
    None,
    Password(String),
    UsernamePassword { username: String, password: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Protocol {
    Http,
    Mqtt,
}

impl Display for Protocol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http => f.write_str("HTTP"),
            Self::Mqtt => f.write_str("MQTT"),
        }
    }
}
