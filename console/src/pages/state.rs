use crate::pages::ApplicationPage;
use crate::simulator::{InternalState, Request, Response, SimulatorBridge};
use crate::utils::monaco::to_model;
use monaco::{api::*, sys::editor::BuiltinTheme, yew::CodeEditor};
use patternfly_yew::*;
use std::rc::Rc;
use yew::prelude::*;

pub struct State {
    options: Rc<CodeEditorOptions>,
    json: Option<TextModel>,
    _simulator: SimulatorBridge,
}

impl ApplicationPage for State {
    fn title() -> String {
        "State".into()
    }

    fn help() -> Option<Html> {
        Some(html!(
            <Content>
                <p>
                {r#"
This page shows internal state of all simulations as it would be sent to the cloud, formatted as simple JSON. This does not include
manually events sent directly.
                "#}
                </p>
                <p>
                {r#"
NOTE: The root level is the channel. Each channel will be sent to the cloud individually.
                "#}
                </p>
            </Content>
        ))
    }
}

#[derive(Debug)]
pub enum Msg {
    Update(InternalState),
}

impl Component for State {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let mut simulator =
            SimulatorBridge::new(ctx.link().batch_callback(|response| match response {
                Response::InternalState(state) => vec![Msg::Update(state)],
                _ => vec![],
            }));
        simulator.send(Request::SubscribeInternalState);

        let options = Rc::new(
            CodeEditorOptions::default()
                .with_scroll_beyond_last_line(false)
                .with_language("json".to_owned())
                .with_builtin_theme(BuiltinTheme::VsDark),
        );

        Self {
            options,
            json: Default::default(),
            _simulator: simulator,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        log::debug!("Update: {msg:?}");

        match msg {
            Msg::Update(state) => {
                let json = serde_json::to_string_pretty(&state.0).unwrap_or_default();
                if let Some(model) = &self.json {
                    model.set_value(&json);
                } else {
                    self.json = to_model(Some("json"), json).ok();
                }
            }
        }
        true
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html!(
            <PageSection variant={PageSectionVariant::Light} fill={true}>
                <Stack>
                    <StackItem fill=true>
                        <CodeEditor model={self.json.clone()} options={self.options.clone()}/>
                    </StackItem>
                </Stack>
            </PageSection>
        )
    }
}
