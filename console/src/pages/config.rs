use crate::data::{SharedDataBridge, SharedDataOps};
use crate::pages::ApplicationPage;
use crate::settings::{Settings, DEFAULT_CONFIG_KEY};
use crate::utils::monaco::to_yaml_model;
use anyhow::anyhow;
use gloo_storage::{LocalStorage, Storage};
use gloo_utils::window;
use monaco::api::{CodeEditorOptions, TextModel};
use monaco::sys::editor::BuiltinTheme;
use monaco::yew::CodeEditor;
use patternfly_yew::*;
use serde_json::Value;
use std::rc::Rc;
use std::time::Duration;
use url::Url;
use yew::prelude::*;

pub struct Configuration {
    // stored settings
    settings: Settings,
    settings_agent: SharedDataBridge<Settings>,

    yaml: Option<TextModel>,
}

impl ApplicationPage for Configuration {
    fn title() -> String {
        "Configuration".into()
    }
}

pub enum Msg {
    Settings(Settings),

    Apply,
    Share,
    Store,
}

impl Component for Configuration {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let mut settings_agent = SharedDataBridge::from(ctx.link(), Msg::Settings);
        settings_agent.request_state();

        Self {
            settings: Default::default(),
            settings_agent,

            yaml: Default::default(),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Settings(settings) => {
                self.settings = settings;
                self.yaml = to_yaml_model(&self.settings).ok();
            }

            Msg::Apply => {
                if let Some(yaml) = &self.yaml {
                    let yaml = yaml.get_value();
                    match serde_yaml::from_str::<Settings>(&yaml) {
                        Ok(settings) => {
                            log::info!("Apply settings");
                            self.settings_agent.set(settings);
                        }
                        Err(err) => toast_err("Failed to parse settings", err),
                    }
                }
            }

            Msg::Share => {
                self.share();
            }

            Msg::Store => {
                self.store();
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let options = CodeEditorOptions::default()
            .with_scroll_beyond_last_line(false)
            .with_language("yaml".to_owned())
            .with_builtin_theme(BuiltinTheme::VsDark);

        let options = Rc::new(options);

        html!(
            <PageSection variant={PageSectionVariant::Light} fill={true}>
                <Stack>
                    <StackItem fill=true>
                        <CodeEditor model={self.yaml.clone()} options={options}/>
                    </StackItem>
                    <StackItem>
                        <Form>
                            <ActionGroup>
                                <Button
                                    label="Apply"
                                    variant={Variant::Primary}
                                    onclick={ctx.link().callback(|_|Msg::Apply)}
                                />
                                <Button
                                    label="Share"
                                    variant={Variant::Secondary}
                                    onclick={ctx.link().callback(|_|Msg::Share)}
                                />
                                <Button
                                    label="Set as default"
                                    variant={Variant::Secondary}
                                    onclick={ctx.link().callback(|_|Msg::Store)}
                                />
                            </ActionGroup>
                        </Form>
                    </StackItem>
                </Stack>
            </PageSection>
        )
    }
}

impl Configuration {
    /// Get the current (editor) configuration as JSON string
    fn as_json_str(&self) -> anyhow::Result<String> {
        let json: Value = serde_yaml::from_str(
            &self
                .yaml
                .as_ref()
                .ok_or_else(|| anyhow!("No content"))?
                .get_value(),
        )?;

        Ok(json.to_string())
    }

    fn share(&self) {
        if let Err(err) = self.do_share() {
            toast_err("Failed to share settings", err);
        }
    }

    fn do_share(&self) -> anyhow::Result<()> {
        let json = self.as_json_str()?;

        log::debug!("Settings: {}", json);

        let loc = window()
            .location()
            .href()
            .map_err(|err| anyhow!(err.as_string().unwrap_or_default()))?;

        let mut url = Url::parse(&loc)?;
        url.set_path("");

        let cfg = base64::encode_config(json, base64::URL_SAFE);
        url.query_pairs_mut().append_pair("c", &cfg);

        log::debug!("Location: {url}");

        ToastDispatcher::new().toast(Toast {
            title: "Share configuration".to_string(),
            r#type: Type::Success,
            timeout: None,
            body: html!(
                <>
                    <Content>
                        <p>
                        {"You can share the configuration of this instance using the following link"}
                        </p>
                        <p>
                            <Clipboard
                                value={url.to_string()}
                                readonly=true
                                name="share-url"
                            />
                        </p>
                    </Content>
                </>
            ),
            actions: vec![],
        });

        Ok(())
    }

    fn store(&self) {
        if let Err(err) = self.do_store() {
            toast_err("Failed to store settings as default", err);
        }
    }

    fn do_store(&self) -> anyhow::Result<()> {
        LocalStorage::set(DEFAULT_CONFIG_KEY, self.as_json_str()?)?;

        ToastDispatcher::new().toast(Toast {
            title: "Stored default".to_string(),
            r#type: Type::Success,
            timeout: Some(Duration::from_secs(5)),
            body: html!(
                <Content>
                    { "Configuration has been stored as the new default." }
                </Content>
            ),
            actions: vec![],
        });

        Ok(())
    }
}

fn toast_err<S, T>(title: S, err: T)
where
    S: Into<String>,
    T: ToString,
{
    ToastDispatcher::new().toast(Toast {
        title: title.into(),
        r#type: Type::Danger,
        timeout: None,
        body: html!(
            <div>
                <code>{err.to_string()}</code>
            </div>
        ),
        actions: vec![],
    });
}
