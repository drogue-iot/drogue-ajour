use crate::simulator::generators::{Context, Generator};
use futures::channel::mpsc;
use futures::{select, FutureExt, SinkExt, StreamExt};
use gloo_timers::future::TimeoutFuture;
use js_sys::Date;
use num_traits::ToPrimitive;
use std::time::Duration;
use wasm_bindgen_futures::spawn_local;

pub trait TickedGenerator: Sized {
    type Properties: 'static + Clone + PartialEq;
    type State: TickState;

    fn make_state(properties: &Self::Properties, current_state: Option<Self::State>)
        -> Self::State;
    fn tick(now: f64, state: &mut Self::State, ctx: &mut Context);

    fn new(properties: Self::Properties) -> TickingGenerator<Self> {
        TickingGenerator::new(properties)
    }
}

pub trait TickState: 'static {
    fn period(&self) -> Duration;
}

pub struct TickingGenerator<G>
where
    G: TickedGenerator,
{
    properties: G::Properties,
    tx: Option<mpsc::UnboundedSender<Msg<G::Properties>>>,
}

pub enum Msg<P> {
    Update(P),
}

impl<G> Generator for TickingGenerator<G>
where
    G: TickedGenerator,
{
    type Properties = G::Properties;

    fn new(properties: Self::Properties) -> Self {
        Self {
            properties,
            tx: None,
        }
    }

    fn update(&mut self, properties: Self::Properties) {
        if self.properties == properties {
            // nothing to do
            return;
        }

        // update our state
        self.properties = properties.clone();

        // send to loop
        if let Some(tx) = &self.tx {
            let mut tx = tx.clone();
            spawn_local(async move {
                if let Err(err) = tx.send(Msg::Update(properties)).await {
                    log::info!("Failed to deliver message: {err}");
                };
            });
        }
    }

    fn start(&mut self, mut ctx: Context) {
        let (tx, mut rx) = mpsc::unbounded::<Msg<_>>();

        self.tx = Some(tx);

        let mut state = G::make_state(&self.properties, None);
        let mut period = state.period().as_millis().to_f64().unwrap_or(f64::MAX);

        spawn_local(async move {
            // we start with a zero delay
            let mut tick = TimeoutFuture::new(0).fuse();
            let mut last = Date::now();

            loop {
                select! {
                    msg = rx.next() => match msg {
                        None => {
                            break;
                        }
                        Some(Msg::Update(props)) => {
                            state = G::make_state(&props, Some(state));
                            let new_period = state.period().as_millis().to_f64().unwrap_or(f64::MAX);
                            if period != new_period {
                                period = new_period;
                                tick = TimeoutFuture::new(period.to_u32().unwrap_or(u32::MAX)).fuse();
                            }
                        }
                    },
                    () = tick => {
                        let now = Date::now();
                        let next = last + period;
                        let delay = if next < now {
                            0f64
                        } else {
                            next - now
                        };
                        last = next;
                        G::tick(now, &mut state, &mut ctx);
                        log::trace!("Next delay: {delay}");
                        tick = TimeoutFuture::new(delay.to_u32().unwrap_or(u32::MAX)).fuse();
                    }
                }
            }
        });
    }

    fn stop(&mut self) {
        if let Some(tx) = self.tx.take() {
            tx.close_channel();
        }
    }
}
