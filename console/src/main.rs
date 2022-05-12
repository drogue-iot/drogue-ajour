use log::Level;
use patternfly_yew::*;
use yew::context::ContextHandle;
use yew::prelude::*;
use yew::virtual_dom::VNode;
use yew_agent::{Dispatched, Dispatcher};
use yew_oauth2::prelude::*;
use yew_router::prelude::*;
use yew_router::router::Render;

use yew_oauth2::openid::*;

use yew_oauth2::openid::Client;

mod applications;
mod bindings;
mod data;
mod devices;
mod fetcher;
mod overview;
mod progress;
mod types;

use applications::ApplicationOverview;
use devices::DeviceOverview;
use fetcher::DataFetcher;
use overview::Overview;

pub struct App {}

#[derive(Clone, Switch, PartialEq, Debug)]
enum AppRoute {
    #[to = "/applications"]
    ApplicationOverview,
    #[to = "/devices"]
    DeviceOverview,
    #[to = "/"]
    Overview,
}

impl Component for App {
    type Message = ();
    type Properties = ();
    fn create(_: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <>
            <OAuth2
                config={
                    Config {
                        client_id: "drogue".into(),
                        issuer_url: "https://sso.sandbox.drogue.cloud/auth/realms/drogue".into(),
                        additional: Default::default(),
                    }
                }
                scopes={vec!["openid".to_string()]}
                >
                <Failure><FailureMessage/></Failure>
                <Authenticated>
                    <BackdropViewer/>
                    <ToastViewer/>
                    <AuthenticatedApp />
                </Authenticated>
                <NotAuthenticated>
                    <BackdropViewer/>
                    <ToastViewer/>
                    <NotAuthenticatedApp />
                </NotAuthenticated>
            </OAuth2>
            </>
        }
    }
}

pub struct AuthenticatedApp {
    fetcher: Dispatcher<DataFetcher>,
    auth: Option<OAuth2Context>,
    _handle: Option<ContextHandle<OAuth2Context>>,
}

pub enum Msg {
    Context(OAuth2Context),
    LoggedOut,
}

impl Component for AuthenticatedApp {
    type Message = Msg;
    type Properties = ();
    fn create(ctx: &Context<Self>) -> Self {
        let mut fetcher = DataFetcher::dispatcher();
        let (auth, handle) = match ctx
            .link()
            .context::<OAuth2Context>(ctx.link().callback(Msg::Context))
        {
            Some((auth, handle)) => (Some(auth), Some(handle)),
            None => (None, None),
        };

        if let Some(auth) = &auth {
            fetcher.send(auth.clone());
        }
        Self {
            fetcher,
            auth,
            _handle: handle,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Context(auth) => {
                self.auth.replace(auth.clone());
                log::info!("GOT NEW CONTEXT");
                self.fetcher.send(auth);
            }
            Msg::LoggedOut => {}
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let logout = ctx.link().callback_once(|()| {
            OAuth2Dispatcher::<Client>::new().logout();
            Msg::LoggedOut
        });
        let tools = vec![{
            let src = "/assets/images/img_avatar.svg"; //.into();

            // gather items
            let mut items = Vec::<DropdownChildVariant>::new();

            // links
            items.push({
                let mut items = Vec::new();
                items.push(
                    html_nested! {<DropdownItem onclick={logout}>{"Sign Out"}</DropdownItem>},
                );
                (html_nested! {<DropdownItemGroup>{items}</DropdownItemGroup>}).into()
            });

            // render
            let full_name =
                if let Some(auth) = ctx.link().context::<OAuth2Context>(Callback::noop()) {
                    if let (
                        OAuth2Context::Authenticated(Authentication {
                            claims: Some(claims),
                            ..
                        }),
                        _,
                    ) = auth
                    {
                        claims
                            .name()
                            .map(|s| s.get(None).map(|e| e.as_str()).unwrap_or("Unknown"))
                            .unwrap_or("Unknown")
                            .to_string()
                    } else {
                        "Unknown".to_string()
                    }
                } else {
                    "Unknown".to_string()
                };

            let user_toggle = html! {<UserToggle name={full_name} src={src} />};
            html! {
                <>
                <Dropdown
                    id="user-dropdown"
                    plain=true
                    position={Position::Right}
                    toggle_style="display: flex;"
                    toggle={user_toggle}
                    >
                {items}
                </Dropdown>
                </>
            }
        }];

        let render = Router::render(move |switch: AppRoute| match switch {
            AppRoute::Overview => page(tools.clone(), html! {<Overview/>}),
            AppRoute::ApplicationOverview => page(tools.clone(), html! {<ApplicationOverview/>}),
            AppRoute::DeviceOverview => page(tools.clone(), html! {<DeviceOverview/>}),
        });

        html! {
            <Router<AppRoute, ()>
                render = {render}
            />
        }
    }
}

pub struct NotAuthenticatedApp;

impl Component for NotAuthenticatedApp {
    type Message = ();
    type Properties = ();
    fn create(_: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let login = ctx.link().callback_once(|_| {
            OAuth2Dispatcher::<Client>::new().start_login();
        });
        let login_action = Action::new(
            "Log In",
            ctx.link().callback(|_| {
                OAuth2Dispatcher::<Client>::new().start_login();
            }),
        );
        let tools = vec![{
            html! {
                <>
                    <div style="padding-right: 8px">
                    <Button label="Log In" variant={Variant::Secondary} onclick={login} />
                    </div>
                    <div>
                    <Button label="Sign Up" variant={Variant::Primary}/>
                    </div>
                </>
            }
        }];

        let logo = html_nested! {
            <Logo src="images/logo.png" alt="Drogue IoT" />
        };
        let render = Router::render(move |_: AppRoute| {
            html! {
            <Page
                logo={logo.clone()}
                tools={Children::new(tools.clone())}
                >
                    <EmptyState
                        title="Login Required"
                        icon={Icon::InfoCircle}
                        primary={login_action.clone()}
                     />

            </Page>
            }
        });

        html! {
            <Router<AppRoute, ()>
                render = {render}
            />
        }
    }
}

fn page(tools: Vec<VNode>, html: Html) -> Html {
    let sidebar = html_nested! {
        <PageSidebar>
            <Nav>
                <NavRouterItem<AppRoute> to={AppRoute::Overview}>{"Overview"}</NavRouterItem<AppRoute>>
                <NavRouterExpandable<AppRoute> title="Firmware" expanded=true>
                    <NavRouterItem<AppRoute> to={AppRoute::ApplicationOverview}>{"Applications"}</NavRouterItem<AppRoute>>
                    <NavRouterItem<AppRoute> to={AppRoute::DeviceOverview}>{"Devices"}</NavRouterItem<AppRoute>>
                </NavRouterExpandable<AppRoute>>
            </Nav>
        </PageSidebar>
    };

    let logo = html_nested! {
        <Logo src="images/logo.png" alt="Drogue IoT" />
    };

    html! {
        <Page
            logo={logo}
            sidebar={sidebar}
            tools={Children::new(tools)}
            >
            { html }
        </Page>
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::new(Level::Info));
    bindings::register_plugin();
    yew::start_app::<App>();
}
