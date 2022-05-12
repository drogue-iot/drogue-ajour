use serde::{Deserialize, Serialize};
use yew::prelude::*;
use yew::Properties;

use wasm_bindgen::JsValue;

pub struct Gauge;

#[derive(PartialEq, Serialize, Deserialize, Clone)]
pub enum ChartColor {
    LightGreen,
    DarkGreen,
    LightBlue,
    DarkBlue,
    LightRed,
    DarkRed,
    LightYellow,
    DarkYellow,
}

impl ChartColor {
    pub fn code(&self) -> &str {
        match self {
            Self::LightGreen => "#BDE2B9",
            Self::DarkGreen => "#38812F",
            Self::LightBlue => "#8BC1F7",
            Self::DarkBlue => "#004B95",
            Self::LightYellow => "#F9E0A2",
            Self::DarkYellow => " #F0AB00",
            Self::LightRed => "#C9190B",
            Self::DarkRed => "#470000",
        }
    }
}

#[derive(Properties, PartialEq, Serialize, Deserialize, Clone)]
pub struct Props {
    pub id: String,
    pub values: Vec<(f32, String, Option<String>)>,
    pub title: Option<String>,
    pub label: Option<String>,
    pub class: String,
}

pub enum Msg {
    Update,
}

impl Component for Gauge {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        log::info!("Gauge create");
        ctx.link().send_message(Msg::Update);
        Self {}
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        log::info!("Gauge update");
        match msg {
            Msg::Update => {
                if let Ok(props) = JsValue::from_serde(&ctx.props()) {
                    crate::bindings::gauge_chart(props, JsValue::from_bool(false));
                }
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        log::info!("Gauge render");
        html! {
            <div class={ctx.props().class.clone()}>
                <canvas id={ctx.props().id.clone()} />
            </div>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        log::info!("Rendered, first: {}", first_render);
        if let Ok(props) = JsValue::from_serde(&ctx.props()) {
            crate::bindings::gauge_chart(props, JsValue::from_bool(false));
        }
    }
}
