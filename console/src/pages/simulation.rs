use crate::simulator::{generators::SimulationState, Response, SimulatorBridge};
use patternfly_yew::*;
use yew::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, Properties)]
pub struct Properties {
    pub id: String,
}

pub struct Simulation {
    simulator: SimulatorBridge,

    simulation_id: String,
    state: SimulationState,
}

pub enum Msg {
    State(SimulationState),
}

impl Component for Simulation {
    type Message = Msg;
    type Properties = Properties;

    fn create(ctx: &Context<Self>) -> Self {
        let mut simulator =
            SimulatorBridge::new(ctx.link().batch_callback(|response| match response {
                Response::SimulationState(state) => vec![Msg::State(state)],
                _ => vec![],
            }));

        simulator.subscribe_simulation(ctx.props().id.clone());

        Self {
            state: SimulationState::default(),
            simulation_id: ctx.props().id.clone(),
            simulator,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::State(state) => {
                self.state = state;
            }
        }
        true
    }

    fn changed(&mut self, ctx: &Context<Self>) -> bool {
        if self.simulation_id != ctx.props().id {
            self.simulator
                .unsubscribe_simulation(self.simulation_id.clone());
            self.simulation_id = ctx.props().id.clone();
            self.simulator
                .subscribe_simulation(self.simulation_id.clone());
        }
        true
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html!(
            <>
                <PageSection variant={PageSectionVariant::Light}>
                    <Title level={Level::H1} size={Size::XXXXLarge}>{ "Simulation" }
                        <small>
                            { format!(" – {}", self.state.description.label) }
                        </small>
                    </Title>
                </PageSection>
                <PageSection variant={PageSectionVariant::Light} fill={true}>
                    { self.state.html.clone() }
                </PageSection>
            </>
        )
    }
}
