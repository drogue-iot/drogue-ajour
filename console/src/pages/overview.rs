use crate::pages::ApplicationPage;
use patternfly_yew::*;
use yew::prelude::*;

pub struct Overview {}

impl ApplicationPage for Overview {
    fn title() -> String {
        "Overview".into()
    }
}

impl Component for Overview {
    type Message = ();
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html!(
            <PageSection fill={true}>
                <Content>
                    {"This is the Drogue IoT Ajour Console, which allows you to manage IoT device firmware from your browser."}
                </Content>
            </PageSection>
        )
    }
}
