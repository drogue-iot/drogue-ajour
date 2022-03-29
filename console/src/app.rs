use crate::data::{SharedDataBridge, SharedDataOps};
use anyhow::anyhow;
use gloo_utils::window;
use patternfly_yew::*;
use url::Url;
use yew::prelude::*;
use yew::virtual_dom::VChild;
use yew_router::prelude::*;

use crate::pages;
use crate::settings::Settings;

#[derive(Switch, Debug, Clone, PartialEq, Eq)]
pub enum AppRoute {
    #[to = "/connection"]
    Connection,
    #[to = "/status"]
    Status,
    #[to = "/!"]
    Overview,
}

pub struct Application {}

impl Component for Application {
    type Message = ();
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html!(
            <>
                <BackdropViewer/>
                <ToastViewer/>

                <ApplicationView/>
            </>
        )
    }
}

pub enum Msg {
    InitError(Toast),
    Settings(Settings),
    Start,
    Stop,
}

pub struct ApplicationView {
    settings: Settings,
    _settings_agent: SharedDataBridge<Settings>,

    fetcher: ApplicationFetcher,
}

impl Component for ApplicationView {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let cfg = find_config();

        let mut _settings_agent = SharedDataBridge::from(ctx.link(), Msg::Settings);

        match cfg {
            Ok(Some(cfg)) => {
                _settings_agent.set(cfg);
            }
            Ok(None) => {
                _settings_agent.request_state();
            }
            Err(toast) => {
                _settings_agent.request_state();
                ctx.link().send_message(Msg::InitError(toast));
            }
        }

        Self {
            settings: Default::default(),
            _settings_agent,
            fetcher: Default::default(),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::InitError(toast) => ToastDispatcher::new().toast(toast),
            Msg::Settings(settings) => {
                self.settings = settings;
            }
            Msg::Start => {
                self.fetcher.start();
            }
            Msg::Stop => {
                self.fetcher.stop();
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let sidebar = html_nested! {
            <PageSidebar>
                <Nav>
                    <NavList>
                        <NavRouterExpandable<AppRoute> title="Home" expanded=true>
                            <NavRouterItem<AppRoute> to={AppRoute::Overview}>{"Overview"}</NavRouterItem<AppRoute>>
                            <NavRouterItem<AppRoute> to={AppRoute::Connection}>{"Connection"}</NavRouterItem<AppRoute>>
                        </NavRouterExpandable<AppRoute>>
                        <NavRouterExpandable<AppRoute> title="Firmware" expanded=true>
                            <NavRouterItem<AppRoute> to={AppRoute::Status}>{"Status"}</NavRouterItem<AppRoute>>
                        </NavRouterExpandable<AppRoute>>
                    </NavList>
                </Nav>
            </PageSidebar>
        };

        let logo = html_nested! {
            <Logo src="images/logo.png" alt="Drogue IoT" />
        };

        html! (
            <Page
                logo={logo}
                sidebar={sidebar}
                >
                    <Router<AppRoute, ()>
                            redirect = {Router::redirect(|_|AppRoute::Overview)}
                            render = {Router::render(move |switch: AppRoute| {
                                match switch {
                                    AppRoute::Overview => html!{<pages::AppPage<pages::Overview>/>},
                                    AppRoute::Connection => html!{<pages::AppPage<pages::Connection>/>},
                                    AppRoute::Status => html!{<pages::AppPage<pages::Status>/>},
                                }
                            })}
                        />
            </Page>
        )
    }
}

fn find_config() -> Result<Option<Settings>, Toast> {
    if let Some(cfg) = find_config_str() {
        log::info!("Found provided settings");
        match base64::decode_config(&cfg, base64::URL_SAFE)
            .map_err(|err| anyhow!("Failed to decode base64 encoding: {err} was: {cfg}"))
            .and_then(|cfg| {
                serde_json::from_slice(&cfg).map_err(|err| {
                    anyhow!(
                        "Failed to parse provided configuration: {err} was: {:?}",
                        String::from_utf8(cfg)
                    )
                })
            }) {
            Ok(settings) => Ok(Some(settings)),
            Err(err) => Err(Toast {
                title: "Failed to load configuration".to_string(),
                r#type: Type::Danger,
                timeout: None,
                body: html!(
                    <Content>
                        <p>
                            {"The page was opened with a provided configuration. However, that configuration could not be loaded due to the following error: "}
                        </p>
                        <p>{err}</p>
                    </Content>
                ),
                actions: vec![],
            }),
        }
    } else if let Some(settings) = Settings::load() {
        log::info!("Found default settings");
        match settings {
            Ok(settings) => Ok(Some(settings)),
            Err(err) => Err(Toast {
                title: "Failed to load configuration".to_string(),
                r#type: Type::Danger,
                timeout: None,
                body: html!(
                    <Content>
                        <p>
                            {"The page tried to load the default configuration. However, that configuration could not parsed due the following error: "}
                        </p>
                        <p>{err}</p>
                    </Content>
                ),
                actions: vec![],
            }),
        }
    } else {
        log::info!("Not settings found");
        Ok(None)
    }
}

fn find_config_str() -> Option<String> {
    if let Ok(href) = window().location().href() {
        if let Ok(url) = Url::parse(&href) {
            for q in url.query_pairs() {
                if q.0 == "c" {
                    return Some(q.1.to_string());
                }
            }
        }
    }
    None
}
