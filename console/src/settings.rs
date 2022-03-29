use gloo_storage::{LocalStorage, Storage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::time::Duration;

pub const DEFAULT_CONFIG_KEY: &str = "drogue.io/drogue-ajour/defaultConfiguration";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub auto_connect: bool,
    pub target: Target,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_connect: false,
            target: Target {
                url: "https://api.sandbox.drogue.cloud".into(),
                credentials: Credentials::Password("my-password".into()),
            },
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
pub struct Target {
    pub url: String,
    pub credentials: Credentials,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Credentials {
    None,
    Password(String),
    UsernamePassword { username: String, password: String },
}
