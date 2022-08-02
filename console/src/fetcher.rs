use crate::data::{SharedDataBridge, SharedDataDispatcher, SharedDataOps};
use crate::types::{BuildInfo, Data};
use crate::BackendConfig;
use drogue_client::registry::v1::{Application, Device};
use yew_agent::*;
use yew_oauth2::prelude::*;

use gloo::timers::callback::Timeout;

pub type DrogueClient = drogue_client::registry::v1::Client;

pub struct DataFetcher {
    //    auth: Option<OAuth2Context>,
    //_handle: Option<ContextHandle<OAuth2Context>>,
    link: AgentLink<Self>,
    _data: SharedDataDispatcher<Data>,
    _builds: SharedDataDispatcher<Vec<BuildInfo>>,
    config: Option<BackendConfig>,
    _config: SharedDataBridge<Option<BackendConfig>>,
    timer: Option<Timeout>,
    oauth: Option<OAuth2Context>,
}

pub enum Msg {
    Fetch,
    UpdateConfig(Option<BackendConfig>),
}

pub enum FetcherInput {
    Oauth2(OAuth2Context),
}

impl Agent for DataFetcher {
    type Reach = Context<Self>;
    type Message = Msg;
    type Input = FetcherInput;
    type Output = ();

    fn create(link: AgentLink<Self>) -> Self {
        let cb = link.callback(|msg| match msg {
            crate::data::Response::State(data) => Msg::UpdateConfig(data),
        });
        let bridge = SharedDataBridge::new(cb);
        Self {
            link,
            timer: None,
            config: None,
            _config: bridge,
            oauth: None,
            _data: SharedDataDispatcher::new(),
            _builds: SharedDataDispatcher::new(),
        }
    }

    fn update(&mut self, msg: Self::Message) {
        match msg {
            Msg::Fetch => {
                if let Some(auth) = &self.oauth {
                    if let Some(config) = &self.config {
                        if let Some(token) = auth.access_token() {
                            // Fetch apps and devices
                            let url = reqwest::Url::parse(&config.drogue_api_url).unwrap();
                            let client =
                                DrogueClient::new(reqwest::Client::new(), url, token.to_string());
                            let client = client.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                let mut data: Vec<(Application, Vec<Device>)> = Vec::new();
                                let applications = client.list_apps(None).await;
                                if let Ok(Some(applications)) = applications {
                                    for app in applications.iter() {
                                        let app_name = &app.metadata.name;
                                        if let Ok(Some(devices)) =
                                            client.list_devices(app_name, None).await
                                        {
                                            data.push((app.clone(), devices));
                                        }
                                    }
                                }
                                SharedDataDispatcher::new().set(data);
                            });
                            let link = self.link.clone();
                            self.timer.replace(Timeout::new(5_000, move || {
                                link.send_message(Msg::Fetch);
                            }));

                            // Fetch builds
                            let url = reqwest::Url::parse(&format!(
                                "{}/api/build/v1alpha1",
                                config.ajour_api_url
                            ))
                            .unwrap();
                            let client = reqwest::Client::new();
                            let token = token.to_string();
                            wasm_bindgen_futures::spawn_local(async move {
                                match client.get(url).bearer_auth(token).send().await {
                                    Ok(result) => {
                                        log::info!("Result: {:?}", result);
                                        let data: Result<Vec<BuildInfo>, reqwest::Error> =
                                            result.json().await;
                                        log::info!("Got data: {:?}", data);
                                        if let Ok(data) = data {
                                            SharedDataDispatcher::new().set(data);
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!("Error retrieving builds: {:?}", e);
                                    }
                                }
                            });
                        }
                    }
                }
            }
            Msg::UpdateConfig(config) => {
                self.config = config;
                self.link.send_message(Msg::Fetch);
            }
        }
    }

    fn connected(&mut self, _id: HandlerId) {}

    fn handle_input(&mut self, msg: Self::Input, _id: HandlerId) {
        match msg {
            FetcherInput::Oauth2(auth) => {
                self.oauth.replace(auth);
                self.link.send_message(Msg::Fetch);
            }
        }
    }

    fn disconnected(&mut self, _id: HandlerId) {}
}
