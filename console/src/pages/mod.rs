mod add;
mod commands;
mod config;
mod connection;
mod events;
mod overview;
mod publish;
mod simulation;
mod state;

pub use add::*;
pub use commands::*;
pub use config::*;
pub use connection::*;
pub use events::*;
pub use overview::*;
pub use publish::*;
pub use simulation::*;
pub use state::*;

use patternfly_yew::{
    Flex, FlexItem, Icon, Level, PageSection, PageSectionVariant, Popover, Size, Title,
};
use std::marker::PhantomData;
use yew::prelude::*;

pub trait ApplicationPage: Component {
    /// The title for the page.
    fn title() -> String;

    /// An optional help, rendered inside a popover.
    fn help() -> Option<Html> {
        None
    }
}

pub struct AppPage<P: ApplicationPage> {
    _marker: PhantomData<P>,
    help: Html,
}

impl<P> Component for AppPage<P>
where
    P: ApplicationPage + 'static,
    P::Properties: Clone,
{
    type Message = ();
    type Properties = P::Properties;

    fn create(_ctx: &Context<Self>) -> Self {
        let target = html!(
            <small class="pf-u-font-size-md pf-u-font-weight-light pf-u-color-200" style="cursor: pointer;">
                { Icon::Help }
            </small>
        );

        let help = P::help()
            .map(|help| {
                html!(
                    <Popover
                        toggle_by_onclick=true
                        target={target}
                        >
                        { help }
                    </Popover>
                )
            })
            .unwrap_or_default();

        Self {
            _marker: Default::default(),
            help,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props().clone();

        html!(
            <>
                <PageSection variant={PageSectionVariant::Light}>
                    <Flex>
                        <FlexItem>
                            <Title level={Level::H1} size={Size::XXXXLarge}>{ P::title() } </Title>
                        </FlexItem>
                        <FlexItem>
                            { self.help.clone() }
                        </FlexItem>
                    </Flex>
                </PageSection>

                <P ..props />
            </>
        )
    }
}
