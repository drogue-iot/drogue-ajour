use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use oci_distribution::{client, secrets::RegistryAuth};
use serde::{Deserialize, Serialize};
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

#[get("/v1/poll/{image}")]
async fn poll(oci: web::Data<OciClient>, image: web::Path<String>) -> impl Responder {
    let data = match oci.fetch_latest_metadata(&image).await {
        Ok(result) => Some(result),
        Err(e) => None,
    };
    HttpResponse::Ok().json(PollResponse {
        current: data,
        interval: Some(30),
    })
}

#[get("/v1/fetch/{image}/{version}")]
async fn fetch(
    oci: web::Data<OciClient>,
    image: web::Path<String>,
    version: web::Path<String>,
) -> impl Responder {
    format!("Return metadata for image {}!", &image);
    let metadata = Metadata {
        version: version.to_string(),
        size: "0".to_string(),
    };
    let payload = Vec::new();
    HttpResponse::Ok().json(FirmwareResponse { metadata, payload })
}

#[derive(Clone)]
pub struct OciClient {
    prefix: String,
    token: String,
    client: Arc<Mutex<client::Client>>,
}

impl OciClient {
    pub async fn fetch_latest_metadata(&self, image: &str) -> Result<Metadata, anyhow::Error> {
        let mut client = self.client.lock().unwrap();
        let manifest = client
            .fetch_manifest_digest(
                &format!("{}{}", self.prefix, image).parse()?,
                &RegistryAuth::Basic("".to_string(), self.token.clone()),
            )
            .await;
        match manifest {
            Ok(result) => {
                log::info!("MANIFEST FETCH: {:?}", result);
            }
            Err(e) => {
                log::info!("MANIFEST FETCH ERR: {:?}", e);
            }
        }
        format!("Return metadata for image {}!", &image);
        let metadata = Metadata {
            version: "1234".to_string(),
            size: "0".to_string(),
        };
        Ok(metadata)
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let client = Arc::new(Mutex::new(client::Client::new(client::ClientConfig {
        protocol: client::ClientProtocol::Http,
        accept_invalid_hostnames: true,
        accept_invalid_certificates: true,
        extra_root_certificates: Vec::new(),
    })));
    let prefix = std::env::var_os("REGISTRY_PREFIX")
        .unwrap()
        .into_string()
        .unwrap();
    let token = std::env::var_os("REGISTRY_TOKEN")
        .unwrap()
        .into_string()
        .unwrap();
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(OciClient {
                client: client.clone(),
                token: token.clone(),
                prefix: prefix.clone(),
            }))
            .service(poll)
        // .service(fetch)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
