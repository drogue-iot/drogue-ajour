use super::Connector;
use crate::connector::mqtt::{MqttClient, MqttConnectOptions, MqttMessage, QoS};
use crate::settings::Credentials;
use crate::simulator::{Command, ConnectOptions, ConnectorOptions, SubscribeOptions};
use std::time::Duration;

pub struct MqttConnector {
    client: MqttClient,
    username: Option<String>,
    password: Option<String>,
}

impl MqttConnector {
    pub fn new(opts: ConnectorOptions) -> Self {
        let mut client = MqttClient::new(&opts.url, None);
        client.set_on_connection_lost(opts.on_connection_lost);
        client.set_on_message_arrived(opts.on_command.reform(|msg: MqttMessage| Command {
            name: msg.topic,
            payload: Some(msg.payload),
        }));

        let (username, password) = match opts.credentials {
            Credentials::None => (None, None),
            Credentials::Password(password) => (
                Some(format!(
                    "{}@{}",
                    opts.settings.device, opts.settings.application
                )),
                Some(password.clone()),
            ),
            Credentials::UsernamePassword { username, password } => {
                (Some(username.clone()), Some(password.clone()))
            }
        };

        Self {
            client,
            username,
            password,
        }
    }
}

impl Connector for MqttConnector {
    fn connect(&mut self, opts: ConnectOptions) -> anyhow::Result<()> {
        self.client.connect(
            MqttConnectOptions {
                username: self.username.clone(),
                password: self.password.clone(),
                clean_session: true,
                reconnect: true,
                keep_alive_interval: Some(Duration::from_secs(2)),
                timeout: Some(Duration::from_secs(5)),
            },
            opts.on_success,
            opts.on_failure,
        )
    }

    fn subscribe(&mut self, opts: SubscribeOptions) -> anyhow::Result<()> {
        self.client.subscribe(
            "command/inbox/#",
            QoS::QoS0,
            Duration::from_secs(5),
            opts.on_success,
            opts.on_failure,
        )
    }

    fn publish(&mut self, channel: &str, payload: Vec<u8>, qos: QoS) {
        if let Err(err) = self.client.publish(channel, payload, qos, false) {
            log::info!("Failed to publish: {err}");
        }
    }
}
