use crate::app::AppRoute;
use crate::{
    data::{SharedDataDispatcher, SharedDataOps},
    pages::ApplicationPage,
    settings::{Settings, Simulation},
    simulator::{
        generators::{sawtooth, sine, wave, SingleTarget},
        SimulatorBridge, SimulatorState,
    },
    utils::to_yaml,
};
use futures::SinkExt;
use itertools::Itertools;
use patternfly_yew::*;
use serde_json::{json, Value};
use std::fmt::format;
use std::{
    collections::HashSet,
    convert::Infallible,
    fmt::{Display, Formatter},
    num::ParseFloatError,
    rc::Rc,
    time::Duration,
};
use strum::{EnumDiscriminants, EnumIter, EnumMessage, IntoEnumIterator};
use uuid::Uuid;
use yew::prelude::*;
use yew_router::agent::{RouteAgent, RouteAgentBridge, RouteAgentDispatcher, RouteRequest};
use yew_router::prelude::{Route, Router};

#[derive(Clone, Debug, EnumDiscriminants)]
#[strum_discriminants(derive(strum::Display, EnumMessage, EnumIter))]
pub enum SimulationTypes {
    #[strum_discriminants(strum(message = "Wave generator",))]
    Wave(Box<wave::Properties>),
    #[strum_discriminants(strum(message = "Sawtooth generator",))]
    Sawtooth(Box<sawtooth::Properties>),
    #[strum_discriminants(strum(message = "Simple sine wave generator",))]
    Sine(Box<sine::Properties>),
}

impl SimulationTypes {
    pub fn to_json(&self) -> Value {
        match self {
            Self::Wave(props) => serde_json::to_value(props.as_ref()),
            Self::Sawtooth(props) => serde_json::to_value(props.as_ref()),
            Self::Sine(props) => serde_json::to_value(props.as_ref()),
        }
        .unwrap_or_default()
    }

    pub fn to_simulation(&self) -> Simulation {
        match self {
            Self::Sine(props) => Simulation::Sine(props.as_ref().clone()),
            Self::Sawtooth(props) => Simulation::Sawtooth(props.as_ref().clone()),
            Self::Wave(props) => Simulation::Wave(props.as_ref().clone()),
        }
    }
}

const fn default_period() -> Duration {
    Duration::from_secs(1)
}

impl SimulationTypesDiscriminants {
    pub fn make_default(&self) -> SimulationTypes {
        match self {
            Self::Sine => SimulationTypes::Sine(Box::new(sine::Properties {
                amplitude: 1.0f64.into(),
                length: Duration::from_secs(60),
                period: default_period(),
                target: Default::default(),
            })),
            Self::Sawtooth => SimulationTypes::Sawtooth(Box::new(sawtooth::Properties {
                max: 1.0f64.into(),
                length: Duration::from_secs(60),
                period: default_period(),
                target: Default::default(),
            })),
            Self::Wave => SimulationTypes::Wave(Box::new(wave::Properties {
                lengths: vec![],
                amplitudes: vec![],
                offset: 0f64.into(),
                period: default_period(),
                target: Default::default(),
            })),
        }
    }
}

impl Default for SimulationTypes {
    fn default() -> Self {
        SimulationTypesDiscriminants::Sine.make_default()
    }
}

pub enum Msg {
    SimulatorState(SimulatorState),
    Selected(SimulationTypesDiscriminants),
    SetId(String),
    Add,

    Set(Box<dyn FnOnce(&mut SimulationTypes)>),
}

pub struct Add {
    id: String,
    content: SimulationTypes,

    simulator_state: SimulatorState,
    _simulator: SimulatorBridge,
    settings_agent: SharedDataDispatcher<Settings>,
}

impl ApplicationPage for Add {
    fn title() -> String {
        "Add simulation".into()
    }
}

impl Component for Add {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let simulator = SimulatorBridge::from(ctx.link(), Msg::SimulatorState);
        let settings_agent = SharedDataDispatcher::<Settings>::new();

        Self {
            id: Uuid::new_v4().to_string(),
            content: Default::default(),
            simulator_state: Default::default(),
            _simulator: simulator,
            settings_agent,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Selected(sel) => {
                self.content = sel.make_default();
            }
            Msg::SimulatorState(simulator_state) => {
                self.simulator_state = simulator_state;
            }
            Msg::SetId(id) => self.id = id,
            Msg::Set(setter) => setter(&mut self.content),
            Msg::Add => {
                self.add();
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let ids: HashSet<_> = self.simulator_state.simulations.keys().cloned().collect();

        // FIXME: must also evaluate claims

        let is_unique = Validator::Custom(Rc::new(move |ctx: ValidationContext<String>| {
            if ctx.value.is_empty() {
                return ValidationResult::error("Must not be empty");
            }

            match ids.contains(&ctx.value) {
                true => ValidationResult::error("Value is already in use"),
                false => ValidationResult::help("Provide a unique ID"),
            }
        }));

        html!(
            <PageSection variant={PageSectionVariant::Light} fill={true}>

                <Grid gutter=true>
                    <GridItem cols={[6.lg(), 12.all()]} >

                        <Form horizontal={[FormHorizontal.xl()]}>

                            <FormGroupValidated<TextInput>
                                required=true
                                label={"ID"}
                                validator={is_unique}
                                >
                                <TextInput
                                    placeholder="Unique ID for the simulation"
                                    value={self.id.clone()}
                                    onchange={ctx.link().callback(Msg::SetId)}
                                />
                            </FormGroupValidated<TextInput>>

                            { self.render_type(ctx) }

                            { self.render_properties(ctx) }

                            <ActionGroup>
                                <Button
                                    id="add"
                                    label="Add"
                                    variant={Variant::Primary}
                                    onclick={ctx.link().callback(|_|Msg::Add)}
                                    />
                            </ActionGroup>
                        </Form>
                    </GridItem>

                    <GridItem cols={[6.lg(), 12.all()]} >
                        <Clipboard
                            code=true readonly=true variant={ClipboardVariant::Expanded}
                            value={self.make_yaml()}/>
                    </GridItem>
                </Grid>

            </PageSection>
        )
    }
}

impl Add {
    fn add(&mut self) {
        let id = self.id.clone();
        let cfg = self.content.to_simulation();

        self.settings_agent.update(|settings| {
            settings.simulations.insert(id, cfg);
        });

        let route = Route::<()>::from(AppRoute::Simulation(self.id.clone()));
        RouteAgentDispatcher::new().send(RouteRequest::ChangeRoute(route));
    }

    /// Render the type select dropdown
    fn render_type(&self, ctx: &Context<Self>) -> Html {
        let variant = SelectVariant::Single(ctx.link().callback(|sel| Msg::Selected(sel)));

        let current: SimulationTypesDiscriminants = self.content.clone().into();

        html!(
            <FormGroup
                label={"Type"}
                required=true
                >

                <FormSelect<SimulationTypesDiscriminants>
                    variant={variant}
                >
                    { for SimulationTypesDiscriminants::iter()
                        .sorted_by(|a,b|Ord::cmp(&a.to_string(), &b.to_string()))
                        .map(|t| {

                        let selected = current == t;

                        html_nested!(
                            <FormSelectOption<SimulationTypesDiscriminants>
                                value={t}
                                selected={selected}
                                description={t.get_message()}
                                id={t.to_string()}
                            />
                        )}
                    )}
                </FormSelect<SimulationTypesDiscriminants>>

            </FormGroup>
        )
    }

    /// Render the current state as YAML
    fn make_yaml(&self) -> String {
        to_yaml(&json!({
            &self.id: self.content.to_json()
        }))
    }

    /// Render the properties of the selected type
    fn render_properties(&self, ctx: &Context<Self>) -> Html {
        match &self.content {
            SimulationTypes::Sawtooth(props) => {
                html!(<>
                    <FormSection title="Parameters">
                    { Self::edit_field(FloatRequired, "Maximum", props.max.0, Self::setter(ctx, | state, v: f64|if let SimulationTypes::Sawtooth(props) =  state {
                        props.max = v.into();
                    })) }
                    { Self::edit_field(DurationRequired, "Period", props.period, Self::setter(ctx, |state, v: humantime::Duration|if let SimulationTypes::Sawtooth(props) = state {
                        props.period = v.into();
                    })) }
                    { Self::edit_field(DurationRequired, "Length", props.length, Self::setter(ctx, |state, v: humantime::Duration|if let SimulationTypes::Sawtooth(props) = state {
                        props.length = v.into();
                    })) }
                    </FormSection>
                    { Self::edit_target(ctx, &props.target, |state| match state {
                        SimulationTypes::Sawtooth(props) => Some(&mut props.target),
                        _ => None,
                    })}
                </>)
            }
            SimulationTypes::Sine(props) => {
                html!(<>
                    <FormSection title="Parameters">
                    { Self::edit_field(FloatRequired, "Amplitude", props.amplitude.0, Self::setter(ctx, | state, v: f64|if let SimulationTypes::Sine(props) =  state {
                        props.amplitude = v.into();
                    })) }
                    { Self::edit_field(DurationRequired, "Period", props.period, Self::setter(ctx, |state, v: humantime::Duration|if let SimulationTypes::Sine(props) = state {
                        props.period = v.into();
                    })) }
                    { Self::edit_field(DurationRequired, "Length", props.length, Self::setter(ctx, |state, v: humantime::Duration|if let SimulationTypes::Sine(props) = state {
                        props.length = v.into();
                    })) }
                    </FormSection>
                    { Self::edit_target(ctx, &props.target, |state| match state {
                        SimulationTypes::Sine(props) => Some(&mut props.target),
                        _ => None,
                    })}
                </>)
            }
            SimulationTypes::Wave(props) => {
                html!(<>
                    <FormSection title="Parameters">
                    { Self::edit_field(FloatRequired, "Offset", props.offset.0, Self::setter(ctx, | state, v: f64|if let SimulationTypes::Wave(props) =  state {
                        props.offset = v.into();
                    })) }
                    { Self::edit_field(DurationRequired, "Period", props.period, Self::setter(ctx, |state, v: humantime::Duration|if let SimulationTypes::Wave(props) = state {
                        props.period = v.into();
                    })) }
                    </FormSection>
                    { Self::edit_target(ctx, &props.target, |state| match state {
                        SimulationTypes::Wave(props) => Some(&mut props.target),
                        _ => None,
                    })}
                </>)
            }
        }
    }

    fn setter<T, F>(ctx: &Context<Self>, f: F) -> Callback<T>
    where
        F: Fn(&mut SimulationTypes, T) + 'static,
        T: 'static,
    {
        ctx.link()
            .callback_once(move |v| Msg::Set(Box::new(move |state| f(state, v))))
    }

    fn edit_field<F, T>(_: F, label: &str, value: T, setter: Callback<F::Type>) -> Html
    where
        F: FieldType + 'static,
        T: Into<F::Type>,
    {
        let setter = Callback::from(move |s: String| match F::parse(&s) {
            Ok(value) => setter.emit(value),
            Err(_) => {}
        });

        html!(
            <FormGroupValidated<TextInput>
                required={F::required()}
                validator={F::base_validator().unwrap_or_default()}
                label={label.to_string()}
                >
                <TextInput
                    value={value.into().to_string()}
                    onchange={setter}
                    />
            </FormGroupValidated<TextInput>>
        )
    }

    fn edit_target<F>(ctx: &Context<Self>, target: &SingleTarget, f: F) -> Html
    where
        F: Fn(&mut SimulationTypes) -> Option<&mut SingleTarget> + 'static,
    {
        let f = Rc::new(f);

        let set_channel = {
            let f = f.clone();
            Self::setter(ctx, move |state, value: String| {
                if let Some(target) = f(state) {
                    target.channel = value;
                }
            })
        };

        let set_feature = {
            let f = f.clone();
            Self::setter(ctx, move |state, value: Optional<String>| {
                if let Some(target) = f(state) {
                    target.feature = value.0;
                }
            })
        };

        let set_property = Self::setter(ctx, move |state, value| {
            if let Some(target) = f(state) {
                target.property = value;
            }
        });

        html!(<>
            <FormSection title="Target">
                { Self::edit_field(StringRequired, "Channel", target.channel.clone(), set_channel) }
                { Self::edit_field(StringOptional, "Feature", target.feature.clone(), set_feature) }
                { Self::edit_field(StringRequired, "Property", target.property.clone(), set_property) }
            </FormSection>
        </>)
    }
}

pub trait FieldType {
    type Type: ToString + 'static;
    type ParseError;

    fn required() -> bool;

    fn base_validator() -> Option<Validator<String, ValidationResult>>;
    fn parse(value: &str) -> Result<Self::Type, Self::ParseError>;
}

pub struct Optional<T>(pub Option<T>);

impl<T> From<Option<T>> for Optional<T> {
    fn from(value: Option<T>) -> Self {
        Optional(value)
    }
}

impl<T> Display for Optional<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(v) => v.fmt(f),
            None => Ok(()),
        }
    }
}

pub struct StringOptional;

impl FieldType for StringOptional {
    type Type = Optional<String>;
    type ParseError = Infallible;

    fn required() -> bool {
        false
    }

    fn base_validator() -> Option<Validator<String, ValidationResult>> {
        None
    }

    fn parse(value: &str) -> Result<Self::Type, Self::ParseError> {
        if value.is_empty() {
            Ok(None.into())
        } else {
            Ok(Some(value.to_string()).into())
        }
    }
}

pub struct StringRequired;

impl FieldType for StringRequired {
    type Type = String;
    type ParseError = Infallible;

    fn required() -> bool {
        true
    }

    fn base_validator() -> Option<Validator<String, ValidationResult>> {
        Some(Validator::from(|ctx: ValidationContext<String>| {
            if ctx.value.is_empty() {
                ValidationResult::error("Value must not be empty")
            } else {
                ValidationResult::ok()
            }
        }))
    }

    fn parse(value: &str) -> Result<Self::Type, Self::ParseError> {
        Ok(value.to_string())
    }
}

pub struct FloatRequired;

impl FieldType for FloatRequired {
    type Type = f64;
    type ParseError = ParseFloatError;

    fn required() -> bool {
        true
    }

    fn base_validator() -> Option<Validator<String, ValidationResult>> {
        Some(Validator::from(|ctx: ValidationContext<String>| {
            if let Err(err) = Self::parse(&ctx.value) {
                ValidationResult::error(format!("Must be a floating-point number: {}", err))
            } else {
                ValidationResult::ok()
            }
        }))
    }

    fn parse(value: &str) -> Result<Self::Type, Self::ParseError> {
        value.parse::<Self::Type>()
    }
}

pub struct DurationRequired;

impl FieldType for DurationRequired {
    type Type = humantime::Duration;
    type ParseError = humantime::DurationError;

    fn required() -> bool {
        true
    }

    fn base_validator() -> Option<Validator<String, ValidationResult>> {
        Some(Validator::from(|ctx: ValidationContext<String>| {
            if let Err(err) = Self::parse(&ctx.value) {
                ValidationResult::error(format!("Must be a duration: {}", err))
            } else {
                ValidationResult::ok()
            }
        }))
    }

    fn parse(value: &str) -> Result<Self::Type, Self::ParseError> {
        value.parse::<Self::Type>()
    }
}
