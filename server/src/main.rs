use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use anyhow::Context;
use clap::Parser;

use drogue_client::openid::AccessTokenProvider;
use futures::TryFutureExt;
use paho_mqtt as mqtt;

use std::time::Duration;

mod index;
mod oci;
mod server;
mod updater;

async fn healthz() -> impl Responder {
    HttpResponse::Ok().finish()
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

    /// Ignore cert validation
    #[clap(long)]
    insecure_tls: bool,

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

    let oci_client = oci::OciClient::new(
        oci::ClientConfig {
            protocol: oci::ClientProtocol::Https,
            accept_invalid_hostnames: args.oci_registry_insecure,
            accept_invalid_certificates: args.oci_registry_insecure,
            extra_root_certificates: Vec::new(),
        },
        args.oci_registry_prefix.clone(),
        args.oci_registry_user.clone(),
        args.oci_registry_token.clone(),
    );

    let mqtt_uri = args.mqtt_uri;
    let token = args.token;
    let application = args.application;

    let mqtt_opts = mqtt::CreateOptionsBuilder::new()
        .server_uri(mqtt_uri)
        .client_id("drogue-ajour")
        .persistence(mqtt::PersistenceType::None)
        .finalize();
    let mqtt_client = mqtt::AsyncClient::new(mqtt_opts)?;

    let tp = AccessTokenProvider {
        user: args.user.to_string(),
        token: token.to_string(),
    };

    let url = reqwest::Url::parse(&args.device_registry)?;
    let drg = index::DrogueClient::new(reqwest::Client::new(), url, tp);
    let index = index::Index::new(drg);

    let mut conn_opts = mqtt::ConnectOptionsBuilder::new();
    conn_opts.user_name(args.user);
    conn_opts.password(token.clone());
    conn_opts.keep_alive_interval(Duration::from_secs(30));
    conn_opts.automatic_reconnect(Duration::from_millis(100), Duration::from_secs(5));

    if !args.disable_tls {
        let ca = args
            .ca_path
            .unwrap_or("/etc/ssl/certs/ca-bundle.crt".to_string());
        let ssl_opts = if args.insecure_tls {
            mqtt::SslOptionsBuilder::new()
                .trust_store(&ca)?
                .enable_server_cert_auth(false)
                .verify(false)
                .finalize()
        } else {
            mqtt::SslOptionsBuilder::new().trust_store(&ca)?.finalize()
        };
        conn_opts.ssl_options(ssl_opts);
    }

    let conn_opts = conn_opts.finalize();

    mqtt_client
        .connect(conn_opts)
        .await
        .context("Failed to connect to MQTT endpoint")?;

    let updater = updater::Updater::new(index, oci_client);

    let healthz = if !args.disable_health {
        Some(
            HttpServer::new(move || App::new().route("/healthz", web::get().to(healthz)))
                .bind(("0.0.0.0", args.health_port))?
                .run(),
        )
    } else {
        None
    };

    let mut app = server::Server::new(mqtt_client, application, updater);

    if let Some(h) = healthz {
        futures::try_join!(app.run(), h.err_into())?;
    } else {
        app.run().await?;
    }
    Ok(())
}
