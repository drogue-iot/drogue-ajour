use crate::simulator::generators::SimulationState;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

/// The device state.
///
/// ```json
/// {
///   "features": {
///     "temperature": {
///       "value": 1.23
///     }
///   }
/// }
/// ```
#[derive(Clone, Debug, Serialize)]
pub struct ChannelState {
    pub features: HashMap<String, Feature>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Feature {
    pub properties: HashMap<String, Value>,
}

#[derive(Debug)]
pub struct SingleFeature {
    pub name: String,
    pub state: Feature,
}

pub trait SimulatorStateUpdate {
    fn state(&mut self, state: SimulationState);
}

pub trait Publisher {
    fn publish(&mut self, event: PublishEvent);
}

pub trait PublisherExt {
    fn publish_single<C, F, P, V>(self, channel: C, feature: F, property: P, value: V)
    where
        C: Into<String>,
        F: Into<String>,
        P: Into<String>,
        V: Into<Value>;
}

impl PublisherExt for &mut dyn Publisher {
    fn publish_single<C, F, P, V>(self, channel: C, feature: F, property: P, value: V)
    where
        C: Into<String>,
        F: Into<String>,
        P: Into<String>,
        V: Into<Value>,
    {
        self.publish(PublishEvent::Single {
            channel: channel.into(),
            state: SingleFeature {
                name: feature.into(),
                state: Feature {
                    properties: {
                        let mut p = HashMap::with_capacity(1);
                        p.insert(property.into(), value.into());
                        p
                    },
                },
            },
        })
    }
}

#[derive(Debug)]
pub enum PublishEvent {
    Single {
        channel: String,
        state: SingleFeature,
    },
    Full {
        channel: String,
        state: ChannelState,
    },
}
