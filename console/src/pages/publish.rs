use crate::pages::ApplicationPage;
use crate::simulator::SimulatorBridge;
use patternfly_yew::*;
use web_sys::HtmlInputElement;
use yew::prelude::*;

pub struct Publish {
    simulator: SimulatorBridge,
    refs: Refs,
}

#[derive(Default)]
struct Refs {
    channel: NodeRef,
    payload: NodeRef,
}

impl ApplicationPage for Publish {
    fn title() -> String {
        "Publish".into()
    }
}

pub enum Msg {
    Send,
}

impl Component for Publish {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let simulator = SimulatorBridge::new(ctx.link().batch_callback(|_response| vec![]));

        Self {
            simulator,
            refs: Default::default(),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Send => {
                self.gather_and_send();
                return false;
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html!(
            <>
                <PageSection variant={PageSectionVariant::Light}>
                    <Content>
                        { "Using this page you can send arbitrary data. This directly triggers an event, without using the internal device state management." }
                    </Content>
                </PageSection>
                <PageSection variant={PageSectionVariant::Light} fill={true}>
                    <Flex>
                        <FlexItem modifiers={[FlexModifier::Grow]}>
                            <Form horizontal={[FormHorizontal]} >
                                <FormGroup
                                    required=true
                                    label="Channel"
                                    >
                                    <TextInput
                                        value="state"
                                        ref={self.refs.channel.clone()}
                                    />
                                </FormGroup>

                                <FormGroup
                                    label="Payload"
                                    >
                                    <TextArea
                                        value=""
                                        resize={ResizeOrientation::Vertical}
                                        spellcheck=false
                                        wrap={Wrap::Off}
                                        rows=15
                                        ref={self.refs.payload.clone()}
                                    />
                                </FormGroup>

                                <ActionGroup>
                                    <Button label={"Send"} variant={Variant::Primary} onclick={ctx.link().callback(|_|Msg::Send)}/>
                                </ActionGroup>
                            </Form>
                        </FlexItem>
                        <FlexItem modifiers={[FlexModifier::Grow]}></FlexItem>
                    </Flex>
                </PageSection>
            </>
        )
    }
}

impl Publish {
    fn gather_and_send(&mut self) {
        match (
            self.refs.channel.cast::<HtmlInputElement>(),
            self.refs.payload.cast::<HtmlInputElement>(),
        ) {
            (Some(channel), Some(payload)) => {
                let channel = channel.value();
                let payload = payload.value();
                self.simulator.publish(channel, payload);
            }
            _ => {}
        }
    }
}
