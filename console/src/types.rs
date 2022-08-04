use crate::progress::{ChartColor, Gauge};
pub use ajour_schema::*;
use chrono::{DateTime, Utc};
use drogue_client::{
    core::v1::Conditions,
    registry::v1::{Application, Device},
    Translator,
};
use patternfly_yew::*;
use std::fmt::Display;
use yew::prelude::*;

pub type Data = Vec<(Application, Vec<Device>)>;

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct DeviceModel {
    pub app: String,
    pub name: String,
    pub update_type: String,
    pub conditions: Conditions,
    pub current: String,
    pub target: String,
    pub has_build: bool,
}

impl From<&Device> for DeviceModel {
    fn from(device: &Device) -> Self {
        let (update_type, has_build) = if let Some(Ok(spec)) = device.section::<FirmwareSpec>() {
            match spec {
                FirmwareSpec::OCI {
                    image: _,
                    image_pull_policy: _,
                    build,
                } => ("OCI".to_string(), build.is_some()),
                FirmwareSpec::HAWKBIT { .. } => ("Hawkbit".to_string(), false),
                FirmwareSpec::FILE { .. } => ("File".to_string(), false),
            }
        } else {
            ("Unspecified".to_string(), false)
        };

        if let Some(Ok(status)) = device.section::<FirmwareStatus>() {
            Self {
                name: device.metadata.name.clone(),
                app: device.metadata.application.clone(),
                update_type,
                conditions: status.conditions.clone(),
                current: status.current.clone(),
                target: status.target.clone(),
                has_build,
            }
        } else {
            Self {
                name: device.metadata.name.clone(),
                app: device.metadata.application.clone(),
                update_type,
                conditions: Default::default(),
                current: "Unknown".to_string(),
                target: "Unknown".to_string(),
                has_build,
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
            1 => {
                if self.update_type != "Unspecified" {
                    html! {<div class="middle">{&self.update_type}</div>}
                } else {
                    html! {{"Not Enabled"}}
                }
            }
            2 => {
                if self.update_type != "Unspecified" {
                    match self.state() {
                        DeviceState::Updating(_) => {
                            html! {<div class="middle"><Label outline={outline} label={format!("{}", self.state())} color={Color::Blue} icon={Icon::Pending} /></div>}
                        }
                        DeviceState::Synced => {
                            html! {<div class="middle"><Label outline={outline} label={format!("{}", self.state())} color={Color::Green} icon={Icon::Check} /></div>}
                        }
                        DeviceState::Unknown => {
                            html! {<div class="middle"><Label outline={outline} label={format!("{}", self.state())} color={Color::Orange} icon={Icon::QuestionCircle} /></div>}
                        }
                    }
                } else {
                    html! {<></>}
                }
            }
            3 => {
                if self.update_type != "Unspecified" {
                    match self.state() {
                        DeviceState::Updating(value) => {
                            html! {<div class="middle"><Gauge id={format!("{}-{}", self.app.clone(), self.name.clone())} values={vec![(value, ChartColor::DarkBlue.code().to_string(), None), (100 as f32 - value, ChartColor::LightBlue.code().to_string(), None)]} class={"progress"} label={format!("{:.1}%", value)} /></div>}
                        }
                        DeviceState::Synced => {
                            html! {<></>}
                        }
                        DeviceState::Unknown => {
                            html! {<></>}
                        }
                    }
                } else {
                    html! {<></>}
                }
            }
            4 => {
                if self.update_type != "Unspecified" {
                    html! {<div class="middle">{&self.current}</div>}
                } else {
                    html! {<></>}
                }
            }

            5 => {
                if self.update_type != "Unspecified" {
                    html! {<div class="middle">{&self.target}</div>}
                } else {
                    html! {<></>}
                }
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

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct ApplicationModel {
    pub name: String,
    pub update_type: String,
    pub total: usize,
    pub synced: usize,
    pub updating: usize,
    pub unknown: usize,
    pub has_build: bool,
}

impl From<&(Application, Vec<Device>)> for ApplicationModel {
    fn from(entry: &(Application, Vec<Device>)) -> Self {
        let app = &entry.0;

        let mut total = 0;
        let mut synced = 0;
        let mut updating = 0;
        let mut unknown = 0;
        for device in entry.1.iter() {
            let model: DeviceModel = device.into();
            if model.update_type != "Unspecified" {
                total += 1;
                match model.state() {
                    DeviceState::Synced => synced += 1,
                    DeviceState::Updating(_) => updating += 1,
                    DeviceState::Unknown => unknown += 1,
                }
            }
        }

        let (update_type, has_build) = if let Some(Ok(spec)) = app.section::<FirmwareSpec>() {
            match spec {
                FirmwareSpec::OCI {
                    image: _,
                    image_pull_policy: _,
                    build,
                } => ("OCI".to_string(), build.is_some()),
                FirmwareSpec::HAWKBIT { .. } => ("Hawkbit".to_string(), false),
                FirmwareSpec::FILE { .. } => ("File".to_string(), false),
            }
        } else {
            ("Unspecified".to_string(), false)
        };

        Self {
            name: app.metadata.name.clone(),
            update_type,
            total,
            synced,
            updating,
            unknown,
            has_build,
        }
    }
}

impl TableRenderer for ApplicationModel {
    fn render(&self, column: ColumnIndex) -> Html {
        let outline = false;
        match column.index {
            0 => html! {{&self.name}},
            1 => {
                if self.total > 0 {
                    html! {{&self.update_type}}
                } else {
                    html! {{"Not Enabled"}}
                }
            }
            2 => {
                if self.total > 0 {
                    let color = if self.synced == self.total {
                        Color::Green
                    } else if self.synced == 0 {
                        Color::Red
                    } else {
                        Color::Orange
                    };
                    html! {<Label outline={outline} label={format!("{}/{}", self.synced, self.total)} color={color} />}
                } else {
                    html! {<></> }
                }
            }
            3 => {
                if self.total > 0 {
                    let color = if self.updating > 0 {
                        Color::Orange
                    } else {
                        Color::Green
                    };
                    html! {<Label outline={outline} label={format!("{}/{}", self.updating, self.total)} color={color} />}
                } else {
                    html! {<></> }
                }
            }
            4 => {
                if self.total > 0 {
                    let color = if self.unknown == self.total {
                        Color::Red
                    } else if self.unknown > 0 {
                        Color::Orange
                    } else {
                        Color::Green
                    };
                    html! {<Label outline={outline} label={format!("{}/{}", self.unknown, self.total)} color={color} />}
                } else {
                    html! {<></> }
                }
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

#[derive(PartialEq, Clone, Debug)]
pub struct BuildModel {
    pub app: String,
    pub device: Option<String>,
    pub started: Option<DateTime<Utc>>,
    pub completed: Option<DateTime<Utc>>,
    pub status: Option<String>,
    pub on_select: Option<Callback<(String, Option<String>)>>,
}

impl From<&BuildInfo> for BuildModel {
    fn from(info: &BuildInfo) -> Self {
        Self {
            app: info.app.clone(),
            device: info.device.clone(),
            started: info.started.clone(),
            completed: info.completed.clone(),
            status: info.status.clone(),
            on_select: None,
        }
    }
}

impl TableRenderer for BuildModel {
    fn render(&self, column: ColumnIndex) -> Html {
        let _outline = false;
        match column.index {
            0 => {
                if let Some(on_select) = &self.on_select {
                    let app = self.app.clone();
                    let device = self.device.clone();
                    let on_select = on_select.clone();
                    let cb = move |_| {
                        on_select.emit((app.clone(), device.clone()));
                    };
                    html! {
                        <>
                            <input type="checkbox" onchange={cb} />
                        </>
                    }
                } else {
                    html! {}
                }
            }
            1 => html! {{&self.app}},
            2 => {
                if let Some(device) = &self.device {
                    html! {{device}}
                } else {
                    html! {{"N/A"}}
                }
            }
            3 => {
                if let Some(started) = &self.started {
                    html! {{started}}
                } else {
                    html! {{"N/A"}}
                }
            }
            4 => {
                if let Some(completed) = &self.completed {
                    html! {{completed}}
                } else {
                    html! {{"N/A"}}
                }
            }
            5 => {
                if let Some(status) = &self.status {
                    html! {{status}}
                } else {
                    html! {{"N/A"}}
                }
            }
            _ => html! {},
        }
    }

    fn render_details(&self) -> Vec<Span> {
        vec![Span::max(html! {
            <>
            </>
        })]
    }
}
