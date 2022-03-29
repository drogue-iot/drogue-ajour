pub mod generators;
mod mqtt;
mod publish;

use crate::connector::mqtt::QoS;
use crate::data::{self, SharedDataBridge};
use crate::settings::{Credentials, Settings, Simulation, Target};
use crate::simulator::publish::SimulatorStateUpdate;
use crate::simulator::{
    generators::{
        sawtooth::SawtoothGenerator, sine::SineGenerator, tick::TickedGenerator,
        wave::WaveGenerator, Generator, SimulationDescription, SimulationState,
    },
    mqtt::MqttConnector,
    publish::{ChannelState, PublishEvent, Publisher},
};
use chrono::{DateTime, Utc};
use std::{
    collections::{btree_map::Entry, BTreeMap, HashMap, HashSet},
    fmt::{Debug, Display, Formatter},
    ops::{Deref, DerefMut},
    rc::Rc,
};
use yew::{html::Scope, Callback, Component};
use yew_agent::*;

pub struct ConnectorOptions<'a> {
    pub url: &'a str,
    pub credentials: &'a Credentials,
    pub settings: &'a Settings,

    pub on_command: Callback<Command>,
    pub on_connection_lost: Callback<String>,
}

pub struct ConnectOptions {
    pub on_success: Callback<()>,
    pub on_failure: Callback<String>,
}

pub struct SubscribeOptions {
    pub on_success: Callback<()>,
    pub on_failure: Callback<String>,
}

pub trait Connector {
    fn connect(&mut self, opts: ConnectOptions) -> anyhow::Result<()>;
    fn subscribe(&mut self, opts: SubscribeOptions) -> anyhow::Result<()>;
    fn publish(&mut self, channel: &str, payload: Vec<u8>, qos: QoS);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub name: String,
    pub payload: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub timestamp: DateTime<Utc>,
    pub channel: String,
    pub payload: Vec<u8>,
}

pub type GeneratorId = String;

pub struct Simulator {
    link: AgentLink<Self>,
    subscribers: HashSet<HandlerId>,

    state: SimulatorState,

    _settings_agent: SharedDataBridge<Settings>,
    settings: Settings,

    connector: Option<Box<dyn Connector>>,
    commands: Vec<Command>,
    events: Vec<Event>,

    simulations: HashMap<GeneratorId, Box<dyn GeneratorHandler>>,
    data: InternalState,

    sim_subs: BTreeMap<GeneratorId, Vec<HandlerId>>,
    sim_states: BTreeMap<GeneratorId, SimulationState>,

    internal_subs: Vec<HandlerId>,
}

trait GeneratorHandler {
    fn start(&mut self, ctx: generators::Context);
    fn stop(&mut self);
}

impl<G> GeneratorHandler for G
where
    G: Generator,
{
    fn start(&mut self, ctx: generators::Context) {
        Generator::start(self, ctx)
    }

    fn stop(&mut self) {
        Generator::stop(self)
    }
}

#[derive(Debug)]
pub enum Msg {
    Settings(Settings),
    Connected,
    Subscribed,
    Disconnected(String),
    Command(Command),
    PublishEvent(PublishEvent),
    SimulationState(GeneratorId, SimulationState),
}

pub enum Request {
    Start,
    Stop,
    Publish { channel: String, payload: Vec<u8> },
    FetchCommandHistory,
    FetchEventHistory,
    SubscribeSimulation(String),
    UnsubscribeSimulation(String),
    SubscribeInternalState,
    UnsubscribeInternalState,
}

pub enum Response {
    State(SimulatorState),
    SimulationState(SimulationState),
    Command(Rc<Command>),
    CommandHistory(Vec<Command>),
    Event(Rc<Event>),
    EventHistory(Vec<Event>),
    InternalState(InternalState),
}

#[derive(Clone, Debug)]
pub struct SimulatorState {
    pub running: bool,
    pub state: State,
    pub simulations: BTreeMap<String, SimulationDescription>,
}

impl Default for SimulatorState {
    fn default() -> Self {
        Self {
            running: false,
            state: State::Disconnected,
            simulations: Default::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum State {
    Connecting,
    Subscribing,
    Connected,
    Disconnected,
    Failed(String),
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connecting => f.write_str("Connecting"),
            Self::Subscribing => f.write_str("Subscribing"),
            Self::Connected => f.write_str("Connected"),
            Self::Disconnected => f.write_str("Disconnected"),
            Self::Failed(err) => write!(f, "Failed ({})", err),
        }
    }
}

impl State {
    #[allow(unused)]
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }
}

#[derive(Clone, Debug, Default)]
pub struct InternalState(pub BTreeMap<String, ChannelState>);

impl Agent for Simulator {
    type Reach = Context<Self>;
    type Message = Msg;
    type Input = Request;
    type Output = Response;

    fn create(link: AgentLink<Self>) -> Self {
        log::info!("Created new simulator");

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
            Msg::Connected => {
                self.state.state = State::Subscribing;
                self.send_state();
                if let Some(connector) = &mut self.connector {
                    if let Err(err) = connector.subscribe(SubscribeOptions {
                        on_success: self.link.callback(|_| Msg::Subscribed),
                        on_failure: self.link.callback(|err| Msg::Disconnected(err)),
                    }) {
                        log::warn!("Failed to subscribe: {err}");
                    };
                }
            }
            Msg::Subscribed => {
                self.state.state = State::Connected;
                self.send_state();
            }
            Msg::Disconnected(err) => {
                self.state.state = State::Failed(err);
                self.send_state();
            }
            Msg::Command(command) => {
                // record in history

                self.commands.push(command.clone());
                let command = Rc::new(command);

                // broadcast

                for id in &self.subscribers {
                    self.link
                        .respond(id.clone(), Response::Command(command.clone()));
                }
            }
            Msg::PublishEvent(event) => {
                self.publish(event);
            }
            Msg::SimulationState(id, state) => {
                // update global description list

                let changed = match self.state.simulations.entry(id.clone()) {
                    Entry::Vacant(e) => {
                        e.insert(state.description.clone());
                        true
                    }
                    Entry::Occupied(mut e) => {
                        if e.get() != &state.description {
                            e.insert(state.description.clone());
                            true
                        } else {
                            false
                        }
                    }
                };

                if changed {
                    self.send_state();
                }

                // update subscriptions

                self.sim_states.insert(id.clone(), state.clone());
                if let Some(subs) = self.sim_subs.get(&id) {
                    for sub in subs {
                        self.link
                            .respond(sub.clone(), Response::SimulationState(state.clone()));
                    }
                }
            }
        }
    }

    fn connected(&mut self, id: HandlerId) {
        if id.is_respondable() {
            self.subscribers.insert(id);
            self.link.respond(id, Response::State(self.state.clone()));
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
            Request::Publish { channel, payload } => {
                self.publish_raw(&channel, payload);
            }
            Request::FetchCommandHistory => {
                if id.is_respondable() {
                    self.link
                        .respond(id, Response::CommandHistory(self.commands.clone()));
                }
            }
            Request::FetchEventHistory => {
                if id.is_respondable() {
                    self.link
                        .respond(id, Response::EventHistory(self.events.clone()));
                }
            }
            Request::SubscribeSimulation(sim_id) if id.is_respondable() => {
                if let Some(state) = self.sim_states.get(&sim_id) {
                    self.link
                        .respond(id, Response::SimulationState(state.clone()));
                }

                match self.sim_subs.entry(sim_id) {
                    Entry::Occupied(mut e) => {
                        e.get_mut().push(id);
                    }
                    Entry::Vacant(e) => {
                        e.insert(vec![id]);
                    }
                }
            }
            Request::SubscribeSimulation(_) => {}
            Request::UnsubscribeSimulation(sim_id) => match self.sim_subs.entry(sim_id) {
                Entry::Occupied(mut e) => {
                    e.get_mut().retain(|i| i != &id);
                    if e.get().is_empty() {
                        e.remove();
                    }
                }
                Entry::Vacant(_) => {}
            },
            Request::SubscribeInternalState if id.is_respondable() => {
                self.internal_subs.push(id.clone());
                self.link
                    .respond(id, Response::InternalState(self.data.clone()));
            }
            Request::SubscribeInternalState => {}
            Request::UnsubscribeInternalState => {
                self.internal_subs.retain(|i| i != &id);
            }
        }
    }

    fn disconnected(&mut self, id: HandlerId) {
        if id.is_respondable() {
            self.subscribers.remove(&id);
        }
    }
}

impl Simulator {
    fn send_state(&self) {
        log::debug!("Broadcast state: {:?}", self.state);
        for id in &self.subscribers {
            self.link
                .respond(id.clone(), Response::State(self.state.clone()));
        }
    }

    fn send_internal_state(&self) {
        for id in &self.internal_subs {
            self.link
                .respond(id.clone(), Response::InternalState(self.data.clone()));
        }
    }

    fn add_generator(
        &mut self,
        id: String,
        mut generator: Box<dyn GeneratorHandler>,
    ) -> GeneratorId {
        // start

        let sim_id = id.clone();
        let ctx = generators::Context::new(
            id.clone(),
            self.link.callback(Msg::PublishEvent),
            self.link
                .callback(move |state| Msg::SimulationState(sim_id.clone(), state)),
        );
        generator.start(ctx);

        // insert

        self.simulations.insert(id.clone(), generator);
        self.state
            .simulations
            .insert(id.clone(), SimulationDescription { label: id.clone() });
        self.send_state();

        // return handle

        id
    }

    fn remove_generator(&mut self, id: &GeneratorId) {
        if let Some(mut generator) = self.simulations.remove(id) {
            generator.stop()
        }
    }

    fn publish_raw(&mut self, channel: &str, payload: Vec<u8>) {
        if let Some(connector) = &mut self.connector {
            connector.publish(channel, payload.clone(), QoS::QoS0);

            let event = Event {
                timestamp: Utc::now(),
                channel: channel.to_string(),
                payload,
            };

            self.events.push(event.clone());

            let event = Rc::new(event);
            for id in &self.subscribers {
                self.link
                    .respond(id.clone(), Response::Event(event.clone()));
            }
        }
    }

    fn publish(&mut self, event: PublishEvent) {
        match event {
            PublishEvent::Full { channel, state } => {
                if let Ok(payload) = serde_json::to_vec(&state) {
                    self.publish_raw(&channel, payload);
                }
                self.data.0.insert(channel, state);
            }
            PublishEvent::Single { channel, state } => {
                let entry = self.data.0.entry(channel.clone());
                let state = match entry {
                    Entry::Vacant(e) => {
                        let mut features = HashMap::new();
                        features.insert(state.name, state.state);
                        let state = ChannelState { features };
                        e.insert(state.clone());
                        state
                    }
                    Entry::Occupied(mut e) => {
                        let e = e.get_mut();
                        e.features.insert(state.name, state.state);
                        e.clone()
                    }
                };

                self.publish_channel_state(&channel, &state);
            }
        }
        self.send_internal_state();
    }

    fn publish_channel_state(&mut self, channel: &str, state: &ChannelState) {
        if let Ok(payload) = serde_json::to_vec(&state) {
            self.publish_raw(&channel, payload);
        }
    }

    fn start(&mut self) {
        self.state.running = true;
        self.send_state();

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
        self.apply_settings(settings);
        if self.state.running {
            // disconnect to trigger reconnect
            self.stop();
            self.start();
        } else if self.settings.auto_connect {
            // auto-connect on, but not started yet
            self.start();
        }
    }

    /// Apply the new settings
    fn apply_settings(&mut self, settings: Settings) {
        let mut current_sims: HashSet<_> = self.simulations.keys().cloned().collect();

        for (id, sim) in &settings.simulations {
            self.add_generator(id.clone(), Self::create_sim(sim));
            current_sims.remove(id);
        }

        for removed_id in current_sims {
            self.remove_generator(&removed_id);
        }

        self.settings = settings;
    }

    fn create_sim(sim: &Simulation) -> Box<dyn GeneratorHandler> {
        match sim {
            Simulation::Sine(props) => Box::new(SineGenerator::new(props.clone())),
            Simulation::Sawtooth(props) => Box::new(SawtoothGenerator::new(props.clone())),
            Simulation::Wave(props) => Box::new(WaveGenerator::new(props.clone())),
        }
    }
}

impl Publisher for Callback<PublishEvent> {
    fn publish(&mut self, event: PublishEvent) {
        self.emit(event);
    }
}

impl SimulatorStateUpdate for Callback<SimulationState> {
    fn state(&mut self, state: SimulationState) {
        self.emit(state)
    }
}

pub struct SimulatorBridge(Box<dyn Bridge<Simulator>>);

impl SimulatorBridge {
    pub fn new(callback: Callback<Response>) -> SimulatorBridge {
        Self(Simulator::bridge(callback))
    }

    pub fn from<C, F>(link: &Scope<C>, f: F) -> Self
    where
        C: Component,
        F: Fn(SimulatorState) -> C::Message + 'static,
    {
        let callback = link.batch_callback(move |msg| match msg {
            Response::State(data) => vec![f(data)],
            _ => vec![],
        });
        Self::new(callback)
    }

    pub fn subscribe_simulation(&mut self, id: String) {
        self.send(Request::SubscribeSimulation(id));
    }

    pub fn unsubscribe_simulation(&mut self, id: String) {
        self.send(Request::UnsubscribeSimulation(id));
    }

    pub fn start(&mut self) {
        self.send(Request::Start);
    }

    pub fn stop(&mut self) {
        self.send(Request::Stop);
    }

    pub fn publish<C, P>(&mut self, channel: C, payload: P)
    where
        C: Into<String>,
        P: Into<Vec<u8>>,
    {
        self.send(Request::Publish {
            channel: channel.into(),
            payload: payload.into(),
        })
    }
}

impl Deref for SimulatorBridge {
    type Target = Box<dyn Bridge<Simulator>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SimulatorBridge {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
