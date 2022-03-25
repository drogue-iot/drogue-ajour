use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use anyhow::{anyhow, Context};
use clap::Parser;
use cloudevents::{event::AttributeValue, Data, Event};
use drogue_ajour_protocol::{Command, Status};
use drogue_client::{dialect, openid::AccessTokenProvider, Section, Translator};
use futures::{stream::StreamExt, TryFutureExt};
use oci_distribution::{client, secrets::RegistryAuth};
use paho_mqtt as mqtt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;
use std::time::Duration;

type DrogueClient = drogue_client::registry::v1::Client<AccessTokenProvider>;

#[derive(Serialize, Deserialize, Debug)]
pub struct Metadata {
    pub version: String,
    pub checksum: String,
    pub size: String,
}

async fn healthz() -> impl Responder {
    HttpResponse::Ok().finish()
}

pub struct Updater {
    index: Index,
    oci: OciClient,
}

impl Updater {
    pub fn new(index: Index, oci: OciClient) -> Self {
        Self { oci, index }
    }
    pub async fn process(
        &mut self,
        application: &str,
        device: &str,
        status: Status,
    ) -> Result<Command, anyhow::Error> {
        if let Some(spec) = self.index.latest_version(application, device).await? {
            match spec {
                FirmwareSpec::OCI { image } => match self.oci.fetch_metadata(&image).await {
                    Ok(metadata) => {
                        if status.version == metadata.version {
                            Ok(Command::new_sync(&status.version, None))
                        } else {
                            let mut offset = 0;
                            let mut mtu = 512;
                            if let Some(m) = status.mtu {
                                mtu = m as usize;
                            }
                            if let Some(update) = status.update {
                                if update.version == metadata.version {
                                    offset = update.offset as usize;
                                }
                            }

                            if offset < metadata.size.parse::<usize>().unwrap() {
                                let firmware = self.oci.fetch_firmware(&image).await?;

                                let to_copy = core::cmp::min(firmware.len() - offset, mtu);
                                let block = &firmware[offset..offset + to_copy];

                                log::trace!(
                                    "Sending firmware block offset {} size {}",
                                    offset,
                                    block.len()
                                );
                                Ok(Command::new_write(&metadata.version, offset as u32, block))
                            } else {
                                let data = hex::decode(&metadata.checksum)?;
                                Ok(Command::new_swap(&metadata.version, &data))
                            }
                        }
                    }
                    Err(e) => Err(e.into()),
                },
                FirmwareSpec::HAWKBIT => {
                    todo!("hawkbit firmware spec no yet supported")
                }
            }
        } else {
            Err(anyhow!("Unable to find latest version for {}", application))
        }
    }
}

pub struct OciClient {
    prefix: String,
    auth: RegistryAuth,
    client: client::Client,
}

impl OciClient {
    pub fn new(
        client: client::Client,
        prefix: String,
        user: Option<String>,
        token: Option<String>,
    ) -> Self {
        Self {
            client,
            prefix,
            auth: token
                .map(|t| RegistryAuth::Basic(user.unwrap_or("".to_string()), t))
                .unwrap_or(RegistryAuth::Anonymous),
        }
    }

    pub async fn fetch_metadata(&mut self, image: &str) -> Result<Metadata, anyhow::Error> {
        let manifest = self
            .client
            .pull_manifest_and_config(&format!("{}{}", self.prefix, image).parse()?, &self.auth)
            .await;
        match manifest {
            Ok((_, _, config)) => {
                let val: Value = serde_json::from_str(&config)?;
                if let Some(annotation) = val["config"]["Labels"]["io.drogue.metadata"].as_str() {
                    let metadata: Metadata = serde_json::from_str(&annotation)?;
                    Ok(metadata)
                } else {
                    Err(anyhow!("Unable to locate metadata in image config"))
                }
            }
            Err(e) => Err(e),
        }
    }

    pub async fn fetch_firmware(&mut self, image: &str) -> Result<Vec<u8>, anyhow::Error> {
        let manifest = self
            .client
            .pull(
                &format!("{}{}", self.prefix, image).parse()?,
                &self.auth,
                vec!["application/vnd.oci.image.layer.v1.tar+gzip"],
            )
            .await;
        match manifest {
            Ok(image) => {
                let layer = &image.layers[0];
                let mut decompressed = Vec::new();
                let mut d = flate2::read::GzDecoder::new(&layer.data[..]);
                d.read_to_end(&mut decompressed)?;

                let mut archive = tar::Archive::new(&decompressed[..]);
                let mut entries = archive.entries()?;
                loop {
                    if let Some(entry) = entries.next() {
                        let mut entry = entry?;
                        let path = entry.path()?;
                        if let Some(p) = path.to_str() {
                            if p == "firmware" {
                                let mut payload = Vec::new();
                                entry.read_to_end(&mut payload)?;
                                return Ok(payload);
                            }
                        }
                    } else {
                        break;
                    }
                }
                Err(anyhow!("Error locating firmware"))
            }
            Err(e) => Err(e),
        }
    }
}
#[derive(Clone)]
pub struct Index {
    client: DrogueClient,
}

dialect!(FirmwareSpec [Section::Spec => "firmware"]);

#[derive(Serialize, Deserialize, Debug)]
pub enum FirmwareSpec {
    #[serde(rename = "oci")]
    OCI { image: String },
    #[serde(rename = "hawkbit")]
    HAWKBIT,
}

impl Index {
    pub fn new(client: DrogueClient) -> Self {
        Self { client }
    }
    pub async fn latest_version(
        &self,
        application: &str,
        device: &str,
    ) -> Result<Option<FirmwareSpec>, anyhow::Error> {
        // Check if we got a device on the device first
        if let Some(device) = self.client.get_device(application, device).await? {
            log::info!("WE GOT DEVICE {:?}", device);
            if let Some(spec) = device.section::<FirmwareSpec>() {
                return Ok(Some(spec?));
            }
        }

        let app = self.client.get_app(application).await?;
        if let Some(app) = app {
            // Check if we've got a device spec first;
            if let Some(spec) = app.section::<FirmwareSpec>() {
                return Ok(Some(spec?));
            }
        }
        Ok(None)
    }
}

#[derive(Parser, Debug)]
struct Args {
    /// Prefix to use for container registry storing images
    #[clap(long)]
    oci_registry_prefix: String,

    /// Token to use for authenticating to registry
    #[clap(long)]
    oci_registry_token: Option<String>,

    /// User to use for authenticating to registry
    #[clap(long)]
    oci_registry_user: Option<String>,

    /// Token to use for authenticating to registry
    #[clap(long)]
    oci_registry_insecure: bool,

    /// Mqtt server uri (tcp://host:port)
    #[clap(long)]
    mqtt_uri: String,

    /// Device registry URL
    /// Mqtt server uri (tcp://host:port)
    #[clap(long)]
    device_registry: String,

    /// Name of application to manage firmware updates for
    #[clap(long)]
    application: String,

    /// Token for authenticating ajour to Drogue IoT
    #[clap(long)]
    token: String,

    /// User for authenticating ajour to Drogue IoT
    #[clap(long)]
    user: String,

    /// Path to CA
    #[clap(long)]
    ca_path: Option<String>,

    /// Disable TLS
    #[clap(long)]
    disable_tls: bool,

    /// Disable /health endpoint
    #[clap(long)]
    disable_health: bool,

    /// Port for health endpoint
    #[clap(long, default_value_t = 8080)]
    health_port: u16,
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init();

    let oci_client = client::Client::new(client::ClientConfig {
        protocol: client::ClientProtocol::Https,
        accept_invalid_hostnames: args.oci_registry_insecure,
        accept_invalid_certificates: args.oci_registry_insecure,
        extra_root_certificates: Vec::new(),
    });

    let mqtt_uri = args.mqtt_uri;
    let token = args.token;
    let application = args.application;

    let mqtt_opts = mqtt::CreateOptionsBuilder::new()
        .server_uri(mqtt_uri)
        .client_id("drogue-ajour")
        .finalize();
    let mqtt_client = mqtt::AsyncClient::new(mqtt_opts)?;

    let tp = AccessTokenProvider {
        user: args.user.to_string(),
        token: token.to_string(),
    };

    let url = reqwest::Url::parse(&args.device_registry)?;
    let drg = DrogueClient::new(reqwest::Client::new(), url, tp);
    let index = Index::new(drg);

    let mut conn_opts = mqtt::ConnectOptionsBuilder::new();
    conn_opts.user_name(args.user);
    conn_opts.password(token.clone());
    conn_opts.keep_alive_interval(Duration::from_secs(30));
    conn_opts.automatic_reconnect(Duration::from_millis(100), Duration::from_secs(5));

    if !args.disable_tls {
        let ca = args
            .ca_path
            .unwrap_or("/etc/ssl/certs/ca-bundle.crt".to_string());
        let ssl_opts = mqtt::SslOptionsBuilder::new().trust_store(&ca)?.finalize();
        conn_opts.ssl_options(ssl_opts);
    }

    let conn_opts = conn_opts.finalize();

    let oci = OciClient::new(
        oci_client,
        args.oci_registry_prefix.clone(),
        args.oci_registry_user.clone(),
        args.oci_registry_token.clone(),
    );

    mqtt_client
        .connect(conn_opts)
        .await
        .context("Failed to connect to MQTT endpoint")?;

    let updater = Updater::new(index, oci);

    let healthz = if !args.disable_health {
        Some(
            HttpServer::new(move || App::new().route("/healthz", web::get().to(healthz)))
                .bind(("0.0.0.0", args.health_port))?
                .run(),
        )
    } else {
        None
    };

    let mut app = Server::new(mqtt_client, application, updater);

    if let Some(h) = healthz {
        futures::try_join!(app.run(), h.err_into())?;
    } else {
        app.run().await?;
    }
    Ok(())
}

pub struct Server {
    client: mqtt::AsyncClient,
    application: String,
    updater: Updater,
}

impl Server {
    fn new(client: mqtt::AsyncClient, application: String, updater: Updater) -> Self {
        Self {
            client,
            application,
            updater,
        }
    }

    async fn run(&mut self) -> Result<(), anyhow::Error> {
        let mut stream = self.client.get_stream(100);
        self.client
            .subscribe(format!("app/{}", &self.application), 1);
        loop {
            if let Some(m) = stream.next().await {
                if let Some(m) = m {
                    match serde_json::from_slice::<Event>(m.payload()) {
                        Ok(e) => {
                            let mut is_dfu = false;
                            let mut application = String::new();
                            let mut device = String::new();
                            for a in e.iter() {
                                log::trace!("Attribute {:?}", a);
                                if a.0 == "subject" {
                                    if let AttributeValue::String("dfu") = a.1 {
                                        is_dfu = true;
                                    }
                                } else if a.0 == "device" {
                                    if let AttributeValue::String(d) = a.1 {
                                        device = d.to_string();
                                    }
                                } else if a.0 == "application" {
                                    if let AttributeValue::String(d) = a.1 {
                                        application = d.to_string();
                                    }
                                }
                            }

                            log::trace!(
                                "Event from app {}, device {}, is dfu: {}",
                                application,
                                device,
                                is_dfu
                            );

                            if is_dfu {
                                let status: Option<Result<Status, anyhow::Error>> =
                                    e.data().map(|d| match d {
                                        Data::Binary(b) => {
                                            serde_cbor::from_slice(&b[..]).map_err(|e| e.into())
                                        }
                                        Data::String(s) => {
                                            serde_json::from_str(&s).map_err(|e| e.into())
                                        }
                                        Data::Json(v) => serde_json::from_str(v.as_str().unwrap())
                                            .map_err(|e| e.into()),
                                    });

                                log::trace!("Status decode: {:?}", status);

                                if let Some(Ok(status)) = status {
                                    log::info!("Received status from {}: {:?}", device, status);
                                    if let Ok(command) =
                                        self.updater.process(&application, &device, status).await
                                    {
                                        log::info!("Sending command to {}: {:?}", device, command);

                                        let topic =
                                            format!("command/{}/{}/dfu", application, device);
                                        let message = mqtt::Message::new(
                                            topic,
                                            serde_cbor::to_vec(&command)?,
                                            1,
                                        );
                                        if let Err(e) = self.client.publish(message).await {
                                            log::info!(
                                                "Error publishing command back to device: {:?}",
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Error parsing event: {:?}", e);
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
