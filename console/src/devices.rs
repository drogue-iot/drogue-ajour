use crate::data::SharedDataBridge;
use crate::types::{ApplicationModel, Data, DeviceModel};
use drogue_client::registry::v1::{Application, Device};
use patternfly_yew::*;
use yew::prelude::*;

pub struct DeviceOverview {
    apps: Vec<(Application, Vec<Device>)>,
    selected_app: Option<String>,
    _bridge: SharedDataBridge<Data>,
}

pub enum Msg {
    DataUpdated(Vec<(Application, Vec<Device>)>),
    SelectionUpdated(String),
}

impl Component for DeviceOverview {
    type Message = Msg;
    type Properties = ();
    fn create(ctx: &Context<Self>) -> Self {
        let bridge = SharedDataBridge::from(ctx.link(), Msg::DataUpdated);
        Self {
            apps: Vec::new(),
            selected_app: None,
            _bridge: bridge,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::DataUpdated(apps) => {
                self.apps = apps;
                true
            }
            Msg::SelectionUpdated(app) => {
                self.selected_app.replace(app);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let header = html_nested! {
            <TableHeader>
                <TableColumn label="Device" />
                <TableColumn label="Update Type" />
                <TableColumn label="State" />
                <TableColumn label="Update Progress" />
                <TableColumn label="Current Version" />
                <TableColumn label="Target Version" />
            </TableHeader>
        };
        let (app_model, devices): (Option<ApplicationModel>, Vec<Device>) =
            if let Some(selected_app) = &self.selected_app {
                let mut found = None;
                for app in self.apps.iter() {
                    if app.0.metadata.name == *selected_app {
                        found.replace((app.0.clone(), app.1.clone()));
                        break;
                    }
                }
                if let Some(found) = found {
                    (Some((&(found.0, found.1.clone())).into()), found.1)
                } else {
                    (None, Vec::new())
                }
            } else {
                let mut app = None;
                // Select first;
                for found in self.apps.iter() {
                    app.replace((
                        Some((&(found.0.clone(), found.1.clone())).into()),
                        found.1.clone(),
                    ));
                    break;
                }
                if let Some(app) = app {
                    app
                } else {
                    (None, Vec::new())
                }
            };
        let models: Vec<DeviceModel> = devices
            .iter()
            .map(|device| {
                let mut model: DeviceModel = device.into();
                if model.update_type == "Unknown" {
                    if let Some(app_model) = &app_model {
                        model.update_type = app_model.update_type.clone();
                    }
                }
                model
            })
            .collect();
        let model: SharedTableModel<DeviceModel> = models.into();

        let selected = if let Some(app_model) = &app_model {
            app_model.name.clone()
        } else {
            "".to_string()
        };

        html! {
            <>
                <PageSection variant={PageSectionVariant::Light}>
                <ContextSelector
                    selected={selected}>
                    {
                        for self.apps.iter().map(|app| {
                            let a = app.clone();
                            let name = a.0.metadata.name.clone();
                            let onclick = ctx.link().callback(move |_| Msg::SelectionUpdated(name.clone()));
                            html_nested!{<ContextSelectorItem label={a.0.metadata.name.clone()} onclick={onclick} /> }
                        })
                    }
                </ContextSelector>
                </PageSection>
                <PageSection variant={PageSectionVariant::Light} limit_width=true>
                    <Title level={Level::H1} size={Size::XXXXLarge}>{ "Devices" }</Title>
                </PageSection>
                <PageSection>
                    <Table<SharedTableModel<DeviceModel>>
                        header={header}
                        entries={model}
                    >

                    </Table<SharedTableModel<DeviceModel>>>
                </PageSection>
            </>
        }
    }
}
