use crate::data::{self, SharedDataBridge};
use crate::settings::{Credentials, Settings, Target};
use chrono::{DateTime, Utc};
use std::{
    collections::{btree_map::Entry, BTreeMap, HashMap, HashSet},
    fmt::{Debug, Display, Formatter},
    ops::{Deref, DerefMut},
    rc::Rc,
};
use yew::{html::Scope, Callback, Component};
use yew_agent::*;

pub type GeneratorId = String;

pub struct ApplicationFetcher {
    link: AgentLink<Self>,
    subscribers: HashSet<HandlerId>,

    _settings_agent: SharedDataBridge<Settings>,
    settings: Settings,

    status: Vec<Status>,
}

#[derive(Debug)]
pub enum Msg {
    Settings(Settings),
    Update,
}

#[derive(Debug, Clone)]
pub struct FirmwareStatus;

pub enum Request {
    Start,
    Stop,
    Status,
}

pub enum Response {
    Status(Vec<Status>),
}

#[derive(Debug, Clone)]
pub struct Status {
    application: String,
    device: String,
    status: FirmwareStatus,
}

impl Agent for ApplicationFetcher {
    type Reach = Context<Self>;
    type Message = Msg;
    type Input = Request;
    type Output = Response;

    fn create(link: AgentLink<Self>) -> Self {
        log::info!("Created new application fetcher");

        let mut settings_agent = SharedDataBridge::new(link.callback(|response| match response {
            data::Response::State(settings) => Msg::Settings(settings),
        }));
        settings_agent.request_state();

        let result = Self {
            link,
            subscribers: HashSet::new(),
            state: Default::default(),
            _settings_agent: settings_agent,
            settings: Default::default(),
            connector: None,
            commands: vec![],
            events: vec![],
            simulations: Default::default(),
            data: Default::default(),
            sim_subs: Default::default(),
            sim_states: Default::default(),
            internal_subs: Default::default(),
        };

        // done

        result
    }

    fn update(&mut self, msg: Self::Message) {
        log::debug!("update: {msg:?}");
        match msg {
            Msg::Settings(settings) => {
                self.update_settings(settings);
            }
            Msg::Update => {
                // TODO fetch data
                for id in &self.subscribers {
                    self.link
                        .respond(id.clone(), Response::Status(self.status.clone()));
                }
            }
        }
    }

    fn connected(&mut self, id: HandlerId) {
        if id.is_respondable() {
            self.subscribers.insert(id);
            self.link.respond(id, Response::Status(self.status.clone()));
        }
    }

    fn handle_input(&mut self, msg: Self::Input, id: HandlerId) {
        match msg {
            Request::Start => {
                if !self.state.running {
                    self.start();
                }
            }
            Request::Stop => {
                if self.state.running {
                    self.stop();
                }
            }
            Request::Status => {
                self.link
                    .response(id, Response::Status(self.status.clone()));
            }
        }
    }

    fn disconnected(&mut self, id: HandlerId) {
        if id.is_respondable() {
            self.subscribers.remove(&id);
        }
    }
}

impl ApplicationFetcher {
    fn start(&mut self) {
        log::info!("Creating client");

        let connector = match &self.settings.target {
            Target::Mqtt { url, credentials } => {
                let mut connector = MqttConnector::new(ConnectorOptions {
                    credentials,
                    url,
                    settings: &self.settings,
                    on_connection_lost: self.link.callback(|err| Msg::Disconnected(err)),
                    on_command: self.link.callback(|msg| Msg::Command(msg)),
                });

                self.state.state = State::Connecting;
                self.send_state();

                if let Err(err) = connector.connect(ConnectOptions {
                    on_success: self.link.callback(|_| Msg::Connected),
                    on_failure: self.link.callback(|err| Msg::Disconnected(err)),
                }) {
                    log::warn!("Failed to start connecting: {err}");
                }

                Some(Box::new(connector) as Box<dyn Connector>)
            }
            // FIXME: implement HTTP too
            _ => None,
        };

        self.connector = connector;

        // Done

        log::info!("Started");
    }

    fn stop(&mut self) {
        self.connector.take();
        self.state.running = false;
        self.state.state = State::Disconnected;
        self.send_state();
    }

    fn update_settings(&mut self, settings: Settings) {
        if self.running {
            // disconnect to trigger reconnect
            self.stop();
            self.start();
        } else if self.settings.auto_connect {
            // auto-connect on, but not started yet
            self.start();
        }
    }
}
