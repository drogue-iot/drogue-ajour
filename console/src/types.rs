use crate::progress::{ChartColor, Gauge};
use drogue_client::{
    core::v1::Conditions,
    dialect,
    registry::v1::{Application, Device},
    Section, Translator,
};
use patternfly_yew::*;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use yew::prelude::*;

pub type Data = Vec<(Application, Vec<Device>)>;

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

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct DeviceModel {
    pub app: String,
    pub name: String,
    pub update_type: String,
    pub conditions: Conditions,
    pub current: String,
    pub target: String,
}

impl From<&Device> for DeviceModel {
    fn from(device: &Device) -> Self {
        let update_type = if let Some(Ok(spec)) = device.section::<FirmwareSpec>() {
            match spec {
                FirmwareSpec::OCI {
                    image: _,
                    image_pull_policy: _,
                } => "OCI".to_string(),
                FirmwareSpec::HAWKBIT { controller: _ } => "Hawkbit".to_string(),
            }
        } else {
            "Unspecified".to_string()
        };

        if let Some(Ok(status)) = device.section::<FirmwareStatus>() {
            Self {
                name: device.metadata.name.clone(),
                app: device.metadata.application.clone(),
                update_type,
                conditions: status.conditions.clone(),
                current: status.current.clone(),
                target: status.target.clone(),
            }
        } else {
            Self {
                name: device.metadata.name.clone(),
                app: device.metadata.application.clone(),
                update_type,
                conditions: Default::default(),
                current: "Unknown".to_string(),
                target: "Unknown".to_string(),
            }
        }
    }
}

pub enum DeviceState {
    Synced,
    Updating(f32),
    Unknown,
}

impl Display for DeviceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::Synced => write!(f, "Synced"),
            Self::Updating(_) => write!(f, "Updating"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

impl DeviceModel {
    pub fn state(&self) -> DeviceState {
        let mut in_sync = false;
        let mut progress = None;
        for condition in self.conditions.0.iter() {
            if condition.r#type == "InSync" && condition.status == "True" {
                in_sync = true;
            } else if condition.r#type == "UpdateProgress" {
                progress = condition.message.clone();
            }
        }
        match (in_sync, progress) {
            (true, _) => DeviceState::Synced,
            (false, Some(p)) => {
                let mut s = p.split(" ");
                if let Some(v) = s.next() {
                    let value: f32 = v.parse().unwrap();
                    DeviceState::Updating(value)
                } else {
                    DeviceState::Unknown
                }
            }
            (false, _) => DeviceState::Unknown,
        }
    }
}

impl TableRenderer for DeviceModel {
    fn render(&self, column: ColumnIndex) -> Html {
        let outline = false;
        match column.index {
            0 => html! {<div class="middle">{&self.name}</div>},
            1 => html! {<div class="middle">{&self.update_type}</div>},
            2 => match self.state() {
                DeviceState::Updating(_) => {
                    html! {<div class="middle"><Label outline={outline} label={format!("{}", self.state())} color={Color::Blue} icon={Icon::Pending} /></div>}
                }
                DeviceState::Synced => {
                    html! {<div class="middle"><Label outline={outline} label={format!("{}", self.state())} color={Color::Green} icon={Icon::Check} /></div>}
                }
                DeviceState::Unknown => {
                    html! {<div class="middle"><Label outline={outline} label={format!("{}", self.state())} color={Color::Orange} icon={Icon::QuestionCircle} /></div>}
                }
            },
            3 => match self.state() {
                DeviceState::Updating(value) => {
                    html! {<div class="middle"><Gauge id={format!("{}-{}", self.app.clone(), self.name.clone())} values={vec![(value, ChartColor::DarkBlue.code().to_string(), None), (100 as f32 - value, ChartColor::LightBlue.code().to_string(), None)]} class={"progress"} label={format!("{:.1}%", value)} /></div>}
                }
                DeviceState::Synced => {
                    html! {<></>}
                }
                DeviceState::Unknown => {
                    html! {<></>}
                }
            },
            4 => html! {<div class="middle">{&self.current}</div>},
            5 => html! {<div class="middle">{&self.target}</div>},
            _ => html! {},
        }
    }

    fn render_details(&self) -> Vec<Span> {
        vec![Span::max(html! {
            <>
                { "So many details for " }{ &self.name}
            </>
        })]
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct ApplicationModel {
    pub name: String,
    pub update_type: String,
    pub total: usize,
    pub synced: usize,
    pub updating: usize,
    pub unknown: usize,
}

impl From<&(Application, Vec<Device>)> for ApplicationModel {
    fn from(entry: &(Application, Vec<Device>)) -> Self {
        let app = &entry.0;

        let total = entry.1.len();
        let mut synced = 0;
        let mut updating = 0;
        let mut unknown = 0;
        for device in entry.1.iter() {
            let model: DeviceModel = device.into();
            match model.state() {
                DeviceState::Synced => synced += 1,
                DeviceState::Updating(_) => updating += 1,
                DeviceState::Unknown => unknown += 1,
            }
        }

        let update_type = if let Some(Ok(spec)) = app.section::<FirmwareSpec>() {
            match spec {
                FirmwareSpec::OCI {
                    image: _,
                    image_pull_policy: _,
                } => "OCI".to_string(),
                FirmwareSpec::HAWKBIT { controller: _ } => "Hawkbit".to_string(),
            }
        } else {
            "Unspecified".to_string()
        };

        Self {
            name: app.metadata.name.clone(),
            update_type,
            total,
            synced,
            updating,
            unknown,
        }
    }
}

impl TableRenderer for ApplicationModel {
    fn render(&self, column: ColumnIndex) -> Html {
        let outline = false;
        match column.index {
            0 => html! {{&self.name}},
            1 => html! {{&self.update_type}},
            2 => {
                let color = if self.synced == self.total {
                    Color::Green
                } else if self.synced == 0 {
                    Color::Red
                } else {
                    Color::Orange
                };
                html! {<Label outline={outline} label={format!("{}/{}", self.synced, self.total)} color={color} />}
            }
            3 => {
                let color = if self.updating > 0 {
                    Color::Orange
                } else {
                    Color::Green
                };
                html! {<Label outline={outline} label={format!("{}/{}", self.updating, self.total)} color={color} />}
            }
            4 => {
                let color = if self.unknown == self.total {
                    Color::Red
                } else if self.unknown > 0 {
                    Color::Orange
                } else {
                    Color::Green
                };
                html! {<Label outline={outline} label={format!("{}/{}", self.unknown, self.total)} color={color} />}
            }
            _ => html! {},
        }
    }

    fn render_details(&self) -> Vec<Span> {
        vec![Span::max(html! {
            <>
                { "So many details for " }{ &self.name}
            </>
        })]
    }
}
