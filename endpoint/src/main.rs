use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use anyhow::anyhow;
use oci_distribution::{client, secrets::RegistryAuth};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

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
            Err(e) => None,
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
        if let Ok(payload) = payload {
            HttpResponse::Ok().json(FirmwareResponse { metadata, payload })
        } else {
            log::info!("Error fetching firmware for {}", image_ref);
            HttpResponse::NotFound().finish()
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
                vec!["application/vnd.oci.image.layer.v1.tar"],
            )
            .await;
        match manifest {
            Ok(image) => {
                log::info!("Received image with {} layers", image.layers.len());
                Ok(Vec::new())
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let client = Arc::new(Mutex::new(client::Client::new(client::ClientConfig {
        protocol: client::ClientProtocol::Https,
        accept_invalid_hostnames: true,
        accept_invalid_certificates: true,
        extra_root_certificates: Vec::new(),
    })));
    let index_dir = std::env::var("INDEX_DIR").unwrap_or("/registry/".to_string());
    let index = Index::new(PathBuf::from_str(&index_dir).unwrap());
    let prefix = std::env::var("REGISTRY_PREFIX").unwrap();
    let token = std::env::var("REGISTRY_TOKEN").unwrap();
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(OciClient {
                client: client.clone(),
                token: token.clone(),
                prefix: prefix.clone(),
            }))
            .app_data(web::Data::new(index.clone()))
            .route("/v1/poll/{image}", web::get().to(poll))
            .route("/v1/fetch/{image}/{version}", web::get().to(fetch))
            .route("/healthz", web::get().to(healthz))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
