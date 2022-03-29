use crate::pages::ApplicationPage;
use patternfly_yew::*;
use yew::prelude::*;

pub struct Status {}

impl ApplicationPage for Status {
    fn title() -> String {
        "Status".into()
    }

    fn help() -> Option<Html> {
        Some(html!(
            <Content>
                <p>
                {r#"
This page shows the applications, devices and their current firmware status.
                "#}
                </p>
            </Content>
        ))
    }
}

#[derive(Debug)]
pub enum Msg {}

impl Component for Status {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        true
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html!(
            <div></div>
        )
    }
}
