use actix_web::{get, middleware, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use ajour_schema::*;
use chrono::{offset::Utc, DateTime};
use drogue_client::registry::v1::Client as DrogueClient;
use kube::Client as KubeClient;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

pub struct State {
    config: ApiConfig,
    drogue: DrogueClient,
    kube: KubeClient,
}

pub struct ApiConfig {
    namespace: String,
    pipeline: String,
    volume_size: String,
    service_account: String,
}

impl State {
    pub fn new(config: ApiConfig, drogue: DrogueClient, kube: KubeClient) -> Self {
        Self {
            config,
            drogue,
            kube,
        }
    }

    async fn trigger(&self, app: &str, device: &str, spec: FirmwareBuildSpec) {
        let name = format!("run-{}-{}", app, device);
        let (git_repo, git_rev, project_path) = match spec.source {
            FirmwareBuildSource::GIT { uri, rev, project } => (uri, rev, project),
        };
        let build_args = spec.args.join(" ");
        let run = json! ({
            "apiVersion": "tekton.dev/v1beta1",
            "kind": "PipelineRun",
            "metadata": {
                "name": name,
                "namespace": &self.config.namespace,
            },
            "spec": {
                "params": [
                    {
                        "name": "GIT_REPO",
                        "value": git_repo,
                    },
                    {
                        "name": "GIT_REVISION",
                        "value": git_rev,
                    },
                    {
                        "name": "PROJECT_PATH",
                        "value": project_path,
                    },
                    {
                        "name": "IMAGE",
                        "value": spec.image,
                    },
                    {
                        "name": "CARGO_BUILD_ARGS",
                        "value": build_args,
                    },
                ],
                "pipelineRef": {
                    "name": &self.config.pipeline,
                },
                "serviceAccountName": &self.config.service_account,
                "timeout": spec.timeout, //"1h0m0s", // TODO
                "workspaces": [
                    {
                        "name": "build",
                        "volumeClaimTemplate": {
                            "spec": {
                                "accessModes": [
                                    "ReadWriteOnce"
                                ],
                                "resources": {
                                    "requests": {
                                        "storage": &self.config.volume_size,
                                    }
                                }
                            }
                        }
                    }
                ]
            }
        });
    }
}

#[actix_web::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    let token = std::env::var("ACCESS_TOKEN").unwrap();
    let namespace = std::env::var("NAMESPACE").unwrap();
    let registry_url = reqwest::Url::parse(&std::env::var("REGISTRY_URL").unwrap()).unwrap();
    let drogue = DrogueClient::new(reqwest::Client::new(), registry_url, token);
    let client = KubeClient::try_default().await?;
    let config = ApiConfig {
        service_account: "pipeline".to_string(),
        pipeline: "oci-firmware".to_string(),
        volume_size: "10Gi".to_string(),
        namespace,
    };

    let state = Arc::new(State::new(config, drogue, client));
    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .app_data(web::JsonConfig::default().limit(4096))
            .app_data(web::Data::new(state.clone()))
            .service(
                web::scope("/api/build/v1alpha1/apps/{appId}")
                    .service(web::resource("/trigger").route(web::post().to(trigger_app_build)))
                    .service(
                        web::resource("/devices/{deviceId}/trigger")
                            .route(web::post().to(trigger_device_build)),
                    ),
            )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await?;
    Ok(())
}

async fn trigger_app_build(
    state: web::Data<Arc<State>>,
    request: HttpRequest,
    appId: web::Path<String>,
) -> impl Responder {
    log::info!("APP: {}", appId);
    HttpResponse::Ok()
}

async fn trigger_device_build(
    state: web::Data<Arc<State>>,
    request: HttpRequest,
    ids: web::Path<(String, String)>,
) -> impl Responder {
    log::info!("APP: {}", ids.0);
    log::info!("DEVICE: {}", ids.1);
    HttpResponse::Ok()
}
