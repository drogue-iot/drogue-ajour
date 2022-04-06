use anyhow::{anyhow, Context};
use clap::Parser;

use drogue_client::openid::AccessTokenProvider;
use paho_mqtt as mqtt;

use std::time::Duration;

mod hawkbit;
mod health;
mod index;
mod metadata;
mod oci;
mod server;
mod updater;

#[derive(Parser, Debug)]
struct Args {
    /// Prefix to use for container registry storing images
    #[clap(long)]
    oci_registry_prefix: Option<String>,

    /// Prefix to use for container registry storing images
    #[clap(long)]
    oci_registry_enable: bool,

    /// Prefix to use for container registry storing images
    #[clap(long)]
    oci_registry_tls: bool,

    /// Token to use for authenticating to registry
    #[clap(long)]
    oci_registry_token: Option<String>,

    /// User to use for authenticating to registry
    #[clap(long)]
    oci_registry_user: Option<String>,

    /// Do not require registry to be valid cert and host
    #[clap(long)]
    oci_registry_insecure: bool,

    // Max number of OCI firmware cache entries
    #[clap(long, default_value_t = 50)]
    oci_cache_entries_max: usize,

    #[clap(long)]
    oci_cache_expiry: Option<u64>,

    #[clap(long)]
    hawkbit_enable: bool,

    #[clap(long)]
    hawkbit_url: Option<String>,

    #[clap(long)]
    hawkbit_tenant: Option<String>,

    #[clap(long)]
    hawkbit_gateway_token: Option<String>,

    /// Mqtt server uri (tcp://host:port)
    #[clap(long)]
    mqtt_uri: String,

    /// Mqtt group id for shared subscription (for horizontal scaling)
    #[clap(long)]
    mqtt_group_id: Option<String>,

    /// Device registry URL
    /// Mqtt server uri (tcp://host:port)
    #[clap(long)]
    device_registry: String,

    /// Name of specific application to manage firmware updates for (will use all accessible from service account by default)
    #[clap(long)]
    application: Option<String>,

    /// Exclude a comma-separated list of applications from ajour processing
    #[clap(long)]
    exclude_applications: Option<String>,

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init();

    let oci_client = if args.oci_registry_enable {
        log::info!("Enabling OCI registry");
        Some(oci::OciClient::new(
            oci::ClientConfig {
                protocol: if args.oci_registry_tls {
                    oci::ClientProtocol::Https
                } else {
                    oci::ClientProtocol::Http
                },
                accept_invalid_hostnames: args.oci_registry_insecure,
                accept_invalid_certificates: args.oci_registry_insecure,
                extra_root_certificates: Vec::new(),
            },
            args.oci_registry_prefix.unwrap().clone(),
            args.oci_registry_user.clone(),
            args.oci_registry_token.clone(),
            args.oci_cache_entries_max,
            args.oci_cache_expiry.map(|s| Duration::from_secs(s)),
        ))
    } else {
        None
    };

    let hawkbit_client = if args.hawkbit_enable {
        log::info!("Enabling Hawkbit Registry");
        Some(hawkbit::HawkbitClient::new(
            &args.hawkbit_url.unwrap(),
            &args.hawkbit_tenant.unwrap(),
            &args.hawkbit_gateway_token.unwrap(),
        ))
    } else {
        None
    };

    let mqtt_uri = args.mqtt_uri;
    let token = args.token;

    let mqtt_opts = mqtt::CreateOptionsBuilder::new()
        .server_uri(mqtt_uri)
        .client_id("drogue-ajour")
        .persistence(mqtt::PersistenceType::None)
        .finalize();
    let mut mqtt_client = mqtt::AsyncClient::new(mqtt_opts)?;

    let tp = AccessTokenProvider {
        user: args.user.to_string(),
        token: token.to_string(),
    };

    let url = reqwest::Url::parse(&args.device_registry)?;
    let drg = index::DrogueClient::new(reqwest::Client::new(), url, tp);

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

    mqtt_client.set_disconnected_callback(|c, _, _| {
        log::info!("Disconnected");
        let t = c.reconnect();
        if let Err(e) = t.wait_for(Duration::from_secs(10)) {
            log::warn!("Error reconnecting to broker ({:?}), exiting", e);
            std::process::exit(1);
        }
    });

    mqtt_client.set_connection_lost_callback(|c| {
        log::info!("Connection lost");
        let t = c.reconnect();
        if let Err(e) = t.wait_for(Duration::from_secs(10)) {
            log::warn!("Error reconnecting to broker ({:?}), exiting", e);
            std::process::exit(1);
        }
    });

    mqtt_client
        .connect(conn_opts)
        .await
        .context("Failed to connect to MQTT endpoint")?;

    let healthz = if !args.disable_health {
        Some(health::HealthServer::new(args.health_port))
    } else {
        None
    };

    let excluded: Vec<String> = if let Some(excluded) = args.exclude_applications {
        excluded.split(",").map(|s| s.to_string()).collect()
    } else {
        Vec::new()
    };
    let mut applications = Vec::new();
    if let Some(app) = args.application {
        applications.push(app);
    } else {
        let apps: Option<Vec<drogue_client::registry::v1::Application>> = drg.list_apps().await?;
        if let Some(apps) = apps {
            for app in apps {
                if !excluded.contains(&app.metadata.name) {
                    applications.push(app.metadata.name);
                }
            }
        } else {
            return Err(anyhow!("no applications available"));
        }
    }

    log::info!(
        "Starting server subscribing to applications: {:?}",
        applications
    );

    let index = index::Index::new(drg);
    let updater = updater::Updater::new(index, oci_client, hawkbit_client);

    let mut app = server::Server::new(mqtt_client, args.mqtt_group_id, applications, updater);

    if let Some(mut h) = healthz {
        futures::try_join!(app.run(), h.run())?;
    } else {
        app.run().await?;
    }
    Ok(())
}
