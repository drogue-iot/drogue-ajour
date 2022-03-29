use super::default_period;
use crate::simulator::generators::{
    tick::{TickState, TickedGenerator},
    Context, SimulationState, SingleTarget,
};
use crate::simulator::publish::PublisherExt;
use crate::utils::{
    float::{ApproxF64, Zero},
    ui::details,
};
use js_sys::Math::sin;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::{f64::consts::TAU, time::Duration};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Properties {
    pub amplitude: ApproxF64<Zero, 2>,

    #[serde(with = "humantime_serde")]
    pub length: Duration,

    #[serde(default = "default_period", with = "humantime_serde")]
    pub period: Duration,

    #[serde(default)]
    pub target: SingleTarget,
}

pub struct State {
    pub amplitude: f64,
    pub length: f64,
    pub period: Duration,
    pub target: SingleTarget,
}

impl TickState for State {
    fn period(&self) -> Duration {
        self.period
    }
}

pub struct SineGenerator;

const DEFAULT_FEATURE: &str = "sine";

impl TickedGenerator for SineGenerator {
    type Properties = Properties;
    type State = State;

    fn make_state(
        properties: &Self::Properties,
        _current_state: Option<Self::State>,
    ) -> Self::State {
        let length = properties.length.as_millis().to_f64().unwrap_or(f64::MAX);
        let amplitude = properties.amplitude.0;
        Self::State {
            length,
            amplitude,
            period: properties.period,
            target: properties.target.clone(),
        }
    }

    fn tick(now: f64, state: &mut Self::State, ctx: &mut Context) {
        let value = sin(now * (TAU / state.length)) * state.amplitude;

        ctx.update(SimulationState {
            description: state.target.describe("Sine", DEFAULT_FEATURE),
            html: details([&("Timestamp", now), &("Value", value)]),
        });

        ctx.publisher().publish_single(
            &state.target.channel,
            state.target.feature.as_deref().unwrap_or(DEFAULT_FEATURE),
            &state.target.property,
            value,
        );
    }
}
