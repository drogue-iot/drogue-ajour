use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use anyhow::{anyhow, Context};
use clap::{Parser, Subcommand};
use oci_distribution::{client, secrets::RegistryAuth};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use tokio_tungstenite::connect_async;
use tungstenite::http::Request;

#[derive(Serialize, Deserialize)]
pub struct PollResponse {
    /// Current expected version
    pub current: Option<Metadata>,

    /// Poll interval
    pub interval: Option<i64>,
}

#[derive(Serialize, Deserialize)]
pub struct Metadata {
    pub version: String,
    pub size: String,
}

#[derive(Serialize, Deserialize)]
pub struct FirmwareResponse {
    pub metadata: Metadata,
    pub payload: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Image(pub String);

#[derive(Serialize, Deserialize, Debug)]
pub struct Version(pub String);

async fn healthz() -> impl Responder {
    HttpResponse::Ok().finish()
}

async fn poll(
    oci: web::Data<OciClient>,
    index: web::Data<Index>,
    image: web::Path<Image>,
) -> impl Responder {
    let data = if let Some(version) = index.latest_version(&image.0) {
        let image_ref = format!("{}:{}", &image.0, &version);
        match oci.fetch_metadata(&image_ref).await {
            Ok(result) => Some(result),
            Err(e) => {
                log::info!("Error during metadata fetch: {:?}", e);
                None
            }
        }
    } else {
        None
    };
    HttpResponse::Ok().json(PollResponse {
        current: data,
        interval: Some(30),
    })
}

async fn fetch(oci: web::Data<OciClient>, path: web::Path<(Image, Version)>) -> impl Responder {
    let (image, version) = path.into_inner();
    let image_ref = format!("{}:{}", &image.0, &version.0);
    let metadata = oci.fetch_metadata(&image_ref).await;
    if let Ok(metadata) = metadata {
        let payload = oci.fetch_firmware(&image_ref).await;
        match payload {
            Ok(payload) => HttpResponse::Ok().body(payload),
            Err(e) => {
                log::info!("Error fetching firmware for {}: {:?}", image_ref, e);
                HttpResponse::NotFound().finish()
            }
        }
    } else {
        log::info!("Error fetching metadata for {}", image_ref);
        HttpResponse::NotFound().finish()
    }
}

#[derive(Clone)]
pub struct OciClient {
    prefix: String,
    token: String,
    client: Arc<Mutex<client::Client>>,
}

impl OciClient {
    pub async fn fetch_metadata(&self, image: &str) -> Result<Metadata, anyhow::Error> {
        let mut client = self.client.lock().unwrap();
        let manifest = client
            .pull_manifest_and_config(
                &format!("{}{}", self.prefix, image).parse()?,
                &RegistryAuth::Basic("".to_string(), self.token.clone()),
            )
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

    pub async fn fetch_firmware(&self, image: &str) -> Result<Vec<u8>, anyhow::Error> {
        let mut client = self.client.lock().unwrap();
        let manifest = client
            .pull(
                &format!("{}{}", self.prefix, image).parse()?,
                &RegistryAuth::Basic("".to_string(), self.token.clone()),
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
    dir: PathBuf,
}

impl Index {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
    pub fn latest_version(&self, image: &str) -> Option<String> {
        let content = std::fs::read_to_string(format!("{}/{}/latest", self.dir.to_str()?, image));
        if let Ok(r) = content {
            Some(r)
        } else {
            None
        }
    }
}

#[derive(Parser, Debug)]
struct Args {
    /// Directory where firmware index is stored
    #[clap(short, long, default_value = "/registry")]
    index_dir: PathBuf,

    /// Prefix to use for container registry storing images
    #[clap(short, long)]
    registry_prefix: String,

    /// URL to websocket endpoint for application
    #[clap(short, long)]
    url: String,

    /// Token for authenticating fleet manager to Drogue IoT
    #[clap(short, long)]
    token: String,

    /// Username for authenticating fleet manager to Drogue IoT
    #[clap(short, long)]
    user: String,
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init();

    let client = Arc::new(Mutex::new(client::Client::new(client::ClientConfig {
        protocol: client::ClientProtocol::Https,
        accept_invalid_hostnames: true,
        accept_invalid_certificates: true,
        extra_root_certificates: Vec::new(),
    })));
    let index_dir = args.index_dir;
    let index = Index::new(index_dir);
    let url = args.url;
    let token = args.token;
    let user = args.user;

    let encoded = base64::encode(&format!("{}:{}", user, token).as_bytes());
    let basic_header = format!("Basic {}", encoded);

    let request = Request::builder()
        .uri(url)
        .header(tungstenite::http::header::AUTHORIZATION, basic_header)
        .body(())
        .context("Error building websocket request")?;

    log::debug!("Connecting to websocket with request : {:?}", request);
    let (mut socket, response) = connect_async(request)
        .await
        .context("Error connecting to the websocket endpoint:")?;
    log::debug!("HTTP response: {}", response.status());

    //let prefix = arstd::env::var("REGISTRY_PREFIX").unwrap();
    //let token = std::env::var("REGISTRY_TOKEN").unwrap();
    HttpServer::new(move || {
        App::new()
            /*     .app_data(web::Data::new(OciClient {
                client: client.clone(),
                token: token.clone(),
                prefix: prefix.clone(),
            }))
            .app_data(web::Data::new(index.clone()))
            .route("/v1/poll/{image}", web::get().to(poll))
            .route("/v1/fetch/{image}/{version}", web::get().to(fetch))*/
            .route("/healthz", web::get().to(healthz))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await?;
    Ok(())
}
