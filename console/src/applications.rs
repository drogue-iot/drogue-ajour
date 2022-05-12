use crate::data::SharedDataBridge;
use crate::types::{ApplicationModel, Data};
use drogue_client::{
    core::v1::{ConditionStatus, Conditions},
    dialect,
    openid::AccessTokenProvider,
    registry::v1::{Application, Device},
    Section, Translator,
};
use patternfly_yew::*;
use serde::{Deserialize, Serialize};
use yew::prelude::*;
use yew_agent::{Bridge, Bridged};

pub struct ApplicationOverview {
    apps: Vec<(Application, Vec<Device>)>,
    _bridge: SharedDataBridge<Data>,
}

pub enum Msg {
    DataUpdated(Vec<(Application, Vec<Device>)>),
}

impl Component for ApplicationOverview {
    type Message = Msg;
    type Properties = ();
    fn create(ctx: &Context<Self>) -> Self {
        let bridge = SharedDataBridge::from(ctx.link(), Msg::DataUpdated);
        Self {
            apps: Vec::new(),
            _bridge: bridge,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::DataUpdated(apps) => {
                self.apps = apps;
                true
            }
        }
    }

    fn view(&self, _: &Context<Self>) -> Html {
        let header = html_nested! {
            <TableHeader>
                <TableColumn label="Application" />
                <TableColumn label="Update Type" />
                <TableColumn label="Synced" />
                <TableColumn label="Updating" />
                <TableColumn label="Unknown" />
            </TableHeader>
        };
        let models: Vec<ApplicationModel> = self.apps.iter().map(|app| app.into()).collect();
        let model: SharedTableModel<ApplicationModel> = models.into();
        html! {
            <>
                <PageSection variant={PageSectionVariant::Light} limit_width=true>
                    <Title level={Level::H1} size={Size::XXXXLarge}>{ "Applications" }</Title>
                </PageSection>
                <PageSection>
                    <Table<SharedTableModel<ApplicationModel>>
                        header={header}
                        entries={model}
                    >

                    </Table<SharedTableModel<ApplicationModel>>>
                </PageSection>
            </>
        }
    }
}
