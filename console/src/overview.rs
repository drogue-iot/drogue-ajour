use crate::data::SharedDataBridge;
use crate::progress::*;
use crate::types::*;
use patternfly_yew::*;
use yew::prelude::*;

pub struct Overview {
    apps: usize,
    devices: usize,
    synced: usize,
    updating: usize,
    unknown: usize,
    firmwares: usize,
    _bridge: SharedDataBridge<Data>,
}

pub enum Msg {
    DataUpdated(Data),
}

impl Component for Overview {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            apps: 0,
            devices: 0,
            synced: 0,
            updating: 0,
            unknown: 0,
            firmwares: 0,
            _bridge: SharedDataBridge::from(ctx.link(), Msg::DataUpdated),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::DataUpdated(data) => {
                let mut devices = 0;
                let mut synced = 0;
                let mut updating = 0;
                let mut unknown = 0;
                let mut firmwares = 0;
                for app in data.iter() {
                    for device in app.1.iter() {
                        let model: DeviceModel = device.into();
                        match model.state() {
                            DeviceState::Synced => synced += 1,
                            DeviceState::Updating(_) => updating += 1,
                            DeviceState::Unknown => unknown += 1,
                        }
                        if model.has_build {
                            log::info!("Device {} has build", model.name);
                            firmwares += 1;
                        }
                    }
                    devices += app.1.len();
                    let model: ApplicationModel = app.into();
                    if model.has_build {
                        log::info!("App {} has build", model.name);
                        firmwares += 1;
                    }
                }
                self.apps = data.len();
                self.devices = devices;
                self.synced = synced;
                self.updating = updating;
                self.unknown = unknown;
                self.firmwares = firmwares;
                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let app_title = if self.apps == 1 {
            "Application"
        } else {
            "Applications"
        };

        let device_title = if self.devices == 1 {
            "Device"
        } else {
            "Devices"
        };

        let build_title = if self.firmwares == 1 {
            "Build"
        } else {
            "Builds"
        };
        html! {
            <>
                <PageSection variant={PageSectionVariant::Light} limit_width=true>
                    <Title level={Level::H1} size={Size::XXXXLarge}>{ "Overview" }</Title>
                </PageSection>
                <PageSection>
                <Bullseye>
                <Gauge id={"apps"} title={format!("{} {}", self.apps, app_title)} values={vec![(100 as f32, ChartColor::DarkBlue.code().to_string(), None)]} class={"large"}/>
                <Gauge id={"devices"} title={format!("{} {}", self.devices, device_title)} values={vec![(self.synced as f32, ChartColor::DarkBlue.code().to_string(), Some("Synced".to_string())), (self.updating as f32, ChartColor::LightBlue.code().to_string(), Some("Updating".to_string())), (self.unknown as f32, ChartColor::DarkYellow.code().to_string(), Some("Unknown".to_string()))]} class={"large"}/>
                <Gauge id={"firmwares"} title={format!("{} {}", self.firmwares, build_title)} values={vec![(100 as f32, ChartColor::DarkBlue.code().to_string(), None)]} class={"large"}/>
                </Bullseye>
                </PageSection>
            </>
        }
    }
}

#[derive(Clone, Debug, Properties, PartialEq)]
pub struct Props {
    pub children: Children,
}

#[function_component(LayoutItem)]
pub fn layout_item(props: &Props) -> Html {
    html! {
        <div style="border: .2rem dashed gray; padding: 1rem; height: 100%;">
            { for props.children.iter() }
        </div>
    }
}
