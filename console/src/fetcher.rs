use crate::data::{SharedDataDispatcher, SharedDataOps};
use crate::types::Data;
use drogue_client::registry::v1::{Application, Device};
use yew_agent::*;
use yew_oauth2::prelude::*;

use gloo::timers::callback::Timeout;

pub type DrogueClient = drogue_client::registry::v1::Client;

pub struct DataFetcher {
    //    auth: Option<OAuth2Context>,
    //_handle: Option<ContextHandle<OAuth2Context>>,
    link: AgentLink<Self>,
    client: Option<DrogueClient>,
    _data: SharedDataDispatcher<Data>,
    timer: Option<Timeout>,
}

pub enum Msg {
    Fetch,
}

impl Agent for DataFetcher {
    type Reach = Context<Self>;
    type Message = Msg;
    type Input = OAuth2Context;
    type Output = ();

    fn create(link: AgentLink<Self>) -> Self {
        /*        let (auth, handle) = match ctx
            .link()
            .context::<OAuth2Context>(ctx.link().callback(Msg::UpdateAuth))
        {
            Some((auth, handle)) => (Some(auth), Some(handle)),
            None => (None, None),
        };

        let tp = if let Some(auth) = auth.clone() {
            if let Some(token) = auth.access_token() {
                //                log::info!("Got token: {}", token);

            } else {
                None
            }
        } else {
            None
        };*/

        Self {
            link,
            client: None,
            timer: None,
            _data: SharedDataDispatcher::new(),
        }
    }

    fn update(&mut self, msg: Self::Message) {
        match msg {
            Msg::Fetch => {
                if let Some(client) = &self.client {
                    let client = client.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let mut data: Vec<(Application, Vec<Device>)> = Vec::new();
                        let applications = client.list_apps(None).await;
                        if let Ok(Some(applications)) = applications {
                            for app in applications.iter() {
                                let app_name = &app.metadata.name;
                                if let Ok(Some(devices)) = client.list_devices(app_name, None).await
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
                }
            }
        }
    }

    fn connected(&mut self, _id: HandlerId) {}

    fn handle_input(&mut self, msg: Self::Input, _id: HandlerId) {
        let auth = msg;
        if let Some(token) = auth.access_token() {
            let link = self.link.clone();
            let url = reqwest::Url::parse("https://api.sandbox.drogue.cloud").unwrap();
            self.client.replace(DrogueClient::new(
                reqwest::Client::new(),
                url,
                token.to_string(),
            ));
            link.send_message(Msg::Fetch);
        } else {
            log::info!("No auth info received, not fetching data");
        }
    }

    fn disconnected(&mut self, _id: HandlerId) {}
}
