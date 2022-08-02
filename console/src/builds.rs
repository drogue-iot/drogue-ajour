use crate::data::SharedDataBridge;
use crate::toast::success;
use crate::types::BuildModel;
use crate::BackendConfig;
use ajour_schema::BuildInfo;
use patternfly_yew::*;
use std::collections::HashSet;
use yew::prelude::*;
use yew_oauth2::prelude::OAuth2Context;

pub struct BuildOverview {
    builds: Vec<BuildInfo>,
    _bridge: SharedDataBridge<Vec<BuildInfo>>,
    selection: HashSet<(String, Option<String>)>,
}

pub enum Msg {
    DataUpdated(Vec<BuildInfo>),
    TriggerBuilds,
    ToggleBuildSelection(String, Option<String>),
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub config: BackendConfig,
}

impl Component for BuildOverview {
    type Message = Msg;
    type Properties = Props;
    fn create(ctx: &Context<Self>) -> Self {
        let bridge = SharedDataBridge::from(ctx.link(), Msg::DataUpdated);
        Self {
            builds: Vec::new(),
            _bridge: bridge,
            selection: HashSet::new(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::DataUpdated(builds) => {
                self.builds = builds;
                true
            }
            Msg::TriggerBuilds => {
                if let Some(auth) = ctx.link().context::<OAuth2Context>(Callback::noop()) {
                    if let Some(token) = auth.0.access_token() {
                        let token = token.to_string();
                        for build in self.builds.iter() {
                            if self
                                .selection
                                .contains(&(build.app.clone(), build.device.clone()))
                            {
                                let url = if let Some(device) = &build.device {
                                    reqwest::Url::parse(&format!(
                                        "{}/api/build/v1alpha1/apps/{}/devices/{}/trigger",
                                        ctx.props().config.ajour_api_url,
                                        build.app,
                                        device
                                    ))
                                } else {
                                    reqwest::Url::parse(&format!(
                                        "{}/api/build/v1alpha1/apps/{}/trigger",
                                        ctx.props().config.ajour_api_url,
                                        build.app
                                    ))
                                }
                                .unwrap();

                                let client = reqwest::Client::new();
                                let token = token.clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    match client.post(url).bearer_auth(token).send().await {
                                        Ok(result) => {
                                            if result.status().is_success() {
                                                log::debug!("Triggered!");
                                            }
                                        }
                                        Err(e) => {
                                            log::warn!("Error triggering builds: {:?}", e);
                                        }
                                    }
                                });
                            }
                        }
                        success("Builds triggered");
                    } else {
                        log::debug!("Not access token acquired, not triggering build");
                    }
                } else {
                    log::debug!("No auth context set, not triggering build");
                }
                true
            }
            Msg::ToggleBuildSelection(app, dev) => {
                if self.selection.contains(&(app.clone(), dev.clone())) {
                    log::debug!("Unselected {:?}/{:?}", app, dev);
                    self.selection.remove(&(app, dev));
                } else {
                    log::debug!("Selected {:?}/{:?}", app, dev);
                    self.selection.insert((app, dev));
                }
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let header = html_nested! {
            <TableHeader>
                <TableColumn label="Select" />
                <TableColumn label="Application" />
                <TableColumn label="Device" />
                <TableColumn label="Started" />
                <TableColumn label="Completed" />
                <TableColumn label="Status" />
            </TableHeader>
        };

        let trigger_builds = ctx.link().callback(|_| Msg::TriggerBuilds);
        let models: Vec<BuildModel> = self
            .builds
            .iter()
            .map(|build| {
                let mut model: BuildModel = build.into();
                let app = model.app.clone();
                let dev = model.device.clone();
                model.on_select.replace(
                    ctx.link()
                        .callback(move |_| Msg::ToggleBuildSelection(app.clone(), dev.clone())),
                );
                model
            })
            .collect();
        let model: SharedTableModel<BuildModel> = models.into();

        html! {
            <>
                <PageSection variant={PageSectionVariant::Light} limit_width=true>
                    <Title level={Level::H1} size={Size::XXXXLarge}>{ "Builds" }</Title>
                </PageSection>
                <PageSection>
                    <Table<SharedTableModel<BuildModel>>
                        header={header}
                        entries={model}
                    >

                    </Table<SharedTableModel<BuildModel>>>
                </PageSection>
                <PageSection>
                    <Button label="Trigger Builds" variant={Variant::Primary} onclick={trigger_builds} />
                </PageSection>
            </>
        }
    }
}
