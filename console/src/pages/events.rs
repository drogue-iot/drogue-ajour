use crate::pages::ApplicationPage;
use crate::simulator::{Event, Request, Response, SimulatorBridge};
use crate::utils::ui::render_payload;
use chrono::Local;
use patternfly_yew::*;
use std::rc::Rc;
use yew::prelude::*;

const DEFAULT_MAX_SIZE: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry(Event);

impl TableRenderer for Entry {
    fn render(&self, column: ColumnIndex) -> Html {
        match column.index {
            0 => {
                let timestamp = self
                    .0
                    .timestamp
                    .with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M:%S%.3f");

                timestamp.into()
            }
            1 => html!(<code>{&self.0.channel}</code>),
            2 => render_payload(&self.0.payload, false),
            _ => html!(),
        }
    }

    fn render_details(&self) -> Vec<Span> {
        vec![Span::max(render_payload(&self.0.payload, true)).truncate()]
    }
}

pub struct Events {
    events: SharedTableModel<Entry>,
    total_received: usize,
    _simulator: SimulatorBridge,
}

impl ApplicationPage for Events {
    fn title() -> String {
        "Events".to_string()
    }
}

pub enum Msg {
    Add(Rc<Event>),
    Set(Vec<Event>),
    Clear,
}

impl Component for Events {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let mut simulator =
            SimulatorBridge::new(ctx.link().batch_callback(|response| match response {
                Response::Event(event) => {
                    vec![Msg::Add(event)]
                }
                Response::EventHistory(events) => {
                    vec![Msg::Set(events)]
                }
                _ => vec![],
            }));

        simulator.send(Request::FetchEventHistory);

        Self {
            events: Default::default(),
            total_received: 0,
            _simulator: simulator,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Clear => {
                self.events.clear();
            }
            Msg::Set(events) => {
                for event in events.into_iter().rev().take(DEFAULT_MAX_SIZE) {
                    self.events.push(Entry(event));
                }
                self.total_received = self.events.len();
            }
            Msg::Add(event) => {
                self.total_received += 1;
                self.events.insert(0, Entry((*event).clone()));
                while self.events.len() > DEFAULT_MAX_SIZE {
                    self.events.pop();
                }
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let header = html_nested! {
            <TableHeader>
                <TableColumn label="Timestamp"/>
                <TableColumn label="Channel"/>
                <TableColumn label="Payload"/>
            </TableHeader>
        };

        html!(
            <>
                <PageSection variant={PageSectionVariant::Light} limit_width=true>
                    <Flex>
                        <FlexItem modifiers={[FlexModifier::Flex3.all()]}>
                            <Content>
                        { r#"
This page shows the data sent towards the cloud. Event though simulations may be running, this only
shows events when they are actually sent to the cloud. So the device must be connected for events
to show up here.
                        "# }
                            </Content>
                        </FlexItem>
                        <FlexItem modifiers={[FlexModifier::Flex2.on(Breakpoint::XLarge), FlexModifier::Flex3.on(Breakpoint::XXLarge)]}></FlexItem>
                    </Flex>
                </PageSection>
                <PageSection variant={PageSectionVariant::Light} fill={true}>
                    <Toolbar>
                        <ToolbarGroup>

                            <ToolbarItem>
                                <Button
                                    label="Clear"
                                    icon={Icon::Times}
                                    variant={Variant::Secondary}
                                    onclick={ctx.link().callback(|_|Msg::Clear)}
                                    />
                            </ToolbarItem>
                        </ToolbarGroup>
                        <ToolbarItem modifiers={[ToolbarElementModifier::Right.all()]}>
                            <strong>{"Commands received: "}{self.total_received}</strong>
                        </ToolbarItem>
                    </Toolbar>

                    <Table<SharedTableModel<Entry>>
                        entries={self.events.clone()}
                        mode={TableMode::CompactExpandable}
                        header={header}
                        >
                    </Table<SharedTableModel<Entry>>>

                </PageSection>
            </>
        )
    }
}
