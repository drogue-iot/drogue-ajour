use actix_cors::Cors;
use actix_web::{
    http::header::Header, middleware, web, App, HttpRequest, HttpResponse, HttpResponseBuilder,
    HttpServer, Responder,
};
use ajour_schema::BuildInfo;
use ajour_schema::*;
use anyhow::anyhow;
use clap::Parser;
use drogue_client::core::v1::Conditions;
use drogue_client::{
    registry::v1::{Application, Client as DrogueClient, Device},
    Translator,
};
use kube::api::ListParams;
use kube::{
    api::{ApiResource, DynamicObject, ObjectList},
    discovery, Api, Resource,
};
use kube::{Client as KubeClient, ResourceExt};
use reqwest::Url;
use serde_json::json;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use actix_web_httpauth::headers::authorization::{Authorization, Bearer};

#[derive(Parser, Debug)]
struct Args {
    /// Device registry URL
    /// Mqtt server uri (tcp://host:port)
    #[clap(long)]
    device_registry: String,

    /// Kubernetes namespace
    #[clap(long)]
    namespace: String,

    /// Port for health endpoint
    #[clap(long, default_value_t = 8080)]
    port: u16,

    /// A comma-separated list of applications that can have builds triggered
    #[clap(long)]
    allowed_applications: Option<String>,
}

pub struct ApiConfig {
    apps: HashSet<String>,
    namespace: String,
    pipeline: String,
    volume_size: String,
    service_account: String,
}

#[actix_web::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    let args = Args::parse();
    let namespace = args.namespace;
    let registry_url = reqwest::Url::parse(&args.device_registry).unwrap();
    let apps: HashSet<String> = if let Some(apps) = args.allowed_applications {
        apps.split(",").map(|s| s.to_string()).collect()
    } else {
        HashSet::new()
    };

    const GROUP_TEKTON_DEV: &str = "tekton.dev";
    const KIND_PIPELINE_RUN: &str = "PipelineRun";

    let kube = KubeClient::try_default().await?;
    let group = discovery::group(&kube, GROUP_TEKTON_DEV).await?;
    let (pipeline_run_resource, _caps) = group
        .recommended_kind(KIND_PIPELINE_RUN)
        .ok_or_else(|| anyhow!("Unable to discover '{}'", KIND_PIPELINE_RUN))?;
    let pipeline_runs =
        Api::<DynamicObject>::namespaced_with(kube.clone(), &namespace, &pipeline_run_resource);

    let config = ApiConfig {
        apps,
        service_account: "pipeline".to_string(),
        pipeline: "oci-firmware".to_string(),
        volume_size: "10Gi".to_string(),
        namespace,
    };

    log::info!("API starting allowing {:?}", config.apps);
    let state = Arc::new(State::new(
        config,
        registry_url,
        pipeline_runs,
        pipeline_run_resource,
    ));
    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(Cors::permissive())
            .app_data(web::JsonConfig::default().limit(4096))
            .app_data(web::Data::new(state.clone()))
            .service(web::resource("/healthz").route(web::get().to(healthz)))
            .service(web::resource("/api/build/v1alpha1").route(web::get().to(get_builds)))
            .service(
                web::scope("/api/build/v1alpha1/apps/{appId}")
                    .service(web::resource("/trigger").route(web::post().to(trigger_app_build)))
                    .service(
                        web::resource("/devices/{deviceId}/trigger")
                            .route(web::post().to(trigger_device_build)),
                    ),
            )
    })
    .bind(("0.0.0.0", args.port))?
    .run()
    .await?;
    Ok(())
}

fn build_name(app: &str, dev: Option<&str>) -> String {
    if let Some(dev) = dev {
        format!("dev-{}-{}", app, dev)
    } else {
        format!("app-{}", app)
    }
}

async fn healthz(_request: HttpRequest) -> impl Responder {
    HttpResponse::Ok().json(json!({
        "status": "OK"
    }))
}

fn extract_token(request: HttpRequest) -> Option<String> {
    if let Ok(auth) = Authorization::<Bearer>::parse(&request) {
        Some(auth.into_scheme().token().to_string())
    } else {
        None
    }
}

async fn get_builds(state: web::Data<Arc<State>>, request: HttpRequest) -> impl Responder {
    if let Some(token) = extract_token(request) {
        if let Ok(builds) = state.get_builds(&token).await {
            HttpResponse::Ok().json(builds)
        } else {
            HttpResponse::Ok().finish()
        }
    } else {
        HttpResponse::BadRequest().finish()
    }
}

async fn trigger_app_build(
    state: web::Data<Arc<State>>,
    request: HttpRequest,
    app_id: web::Path<String>,
) -> impl Responder {
    if let Some(token) = extract_token(request) {
        match state.get_app(&token, &app_id).await {
            Ok(None) => HttpResponse::NotFound(),
            Ok(Some(app)) => {
                if let Some(Ok(spec)) = app.section::<FirmwareSpec>() {
                    trigger_build(state, &app_id, None, spec).await
                } else {
                    HttpResponse::NotFound()
                }
            }
            Err(e) => {
                log::warn!("Unable to trigger build: {:?}", e);
                HttpResponse::InternalServerError()
            }
        }
    } else {
        HttpResponse::BadRequest()
    }
}

async fn trigger_build(
    state: web::Data<Arc<State>>,
    app: &str,
    dev: Option<&str>,
    spec: FirmwareSpec,
) -> HttpResponseBuilder {
    match spec {
        FirmwareSpec::OCI {
            image,
            image_pull_policy: _,
            build,
        } => {
            if state.is_allowed(app) {
                if let Some(build) = build {
                    if let Err(e) = state.trigger(app, dev, &image, build).await {
                        log::warn!("Error triggering build: {:?}", e);
                        HttpResponse::InternalServerError()
                    } else {
                        HttpResponse::Ok()
                    }
                } else {
                    HttpResponse::NotFound()
                }
            } else {
                HttpResponse::Forbidden()
            }
        }
        _ => {
            log::info!("Firmware registry not yet supported for builds");
            HttpResponse::NotImplemented()
        }
    }
}

async fn trigger_device_build(
    state: web::Data<Arc<State>>,
    request: HttpRequest,
    ids: web::Path<(String, String)>,
) -> impl Responder {
    if let Some(token) = extract_token(request) {
        if state.is_allowed(&ids.0) {
            match state.get_device(&token, &ids.0, &ids.1).await {
                Ok(None) => HttpResponse::NotFound(),
                Ok(Some(dev)) => {
                    if let Some(Ok(spec)) = dev.section::<FirmwareSpec>() {
                        trigger_build(state, &ids.0, Some(&ids.1), spec).await
                    } else {
                        HttpResponse::NotFound()
                    }
                }
                Err(e) => {
                    log::warn!("Unable to trigger build: {:?}", e);
                    HttpResponse::InternalServerError()
                }
            }
        } else {
            HttpResponse::Forbidden()
        }
    } else {
        HttpResponse::BadRequest()
    }
}

fn update_build_status(info: &mut BuildInfo, object: &DynamicObject) -> Result<(), anyhow::Error> {
    info.started = object.meta().creation_timestamp.clone().map(|s| s.0);

    if let Some(s) = object.data.get("status") {
        if let Some(conditions) = s.get("conditions") {
            let conditions: Conditions = serde_json::from_value(conditions.clone())?;
            for condition in conditions.iter() {
                if condition.r#type == "Succeeded" {
                    info.status = condition.reason.clone();
                    info.completed = Some(condition.last_transition_time);
                    break;
                }
            }
        }
    }
    Ok(())
}

pub struct State {
    config: ApiConfig,
    device_registry: Url,
    kube: Api<DynamicObject>,
    pipeline_run_resource: ApiResource,
}

impl State {
    pub fn new(
        config: ApiConfig,
        device_registry: Url,
        kube: Api<DynamicObject>,
        pipeline_run_resource: ApiResource,
    ) -> Self {
        Self {
            config,
            device_registry,
            kube,
            pipeline_run_resource,
        }
    }

    //    let drogue = DrogueClient::new(reqwest::Client::new(), registry_url, tp);
    async fn get_builds(&self, token: &str) -> Result<Vec<BuildInfo>, anyhow::Error> {
        let drogue = DrogueClient::new(
            reqwest::Client::new(),
            self.device_registry.clone(),
            token.to_string(),
        );
        let apps = drogue.list_apps(None).await?;
        let apps = apps.ok_or(anyhow!("Unable to find any apps"))?;

        let mut all_builds: Vec<BuildInfo> = Vec::new();

        for app in apps {
            // Find all devices for this app
            let devs: Vec<Device> = drogue
                .list_devices(&app.metadata.name, None)
                .await?
                .unwrap_or(Vec::new());

            let mut app_build = if has_build_spec(&app.section::<FirmwareSpec>()) {
                Some(BuildInfo {
                    app: app.metadata.name.clone(),
                    device: None,
                    started: None,
                    status: None,
                    completed: None,
                })
            } else {
                None
            };

            let mut devs: HashMap<String, BuildInfo> = HashMap::from_iter(
                devs.iter()
                    .map(|dev| (dev.metadata.name.clone(), dev.section::<FirmwareSpec>()))
                    .filter(|spec| has_build_spec(&spec.1))
                    .map(|spec| {
                        let name = spec.0.clone();
                        let info = BuildInfo {
                            app: app.metadata.name.clone(),
                            device: Some(name.clone()),
                            started: None,
                            status: None,
                            completed: None,
                        };
                        (name, info)
                    }),
            );

            // Neither app nor devices have any build spec, skip
            if app_build.is_none() && devs.is_empty() {
                continue;
            }

            // Retrieve all build statuses for this app and its devs
            let builds: ObjectList<DynamicObject> = self
                .kube
                .list(&ListParams::default().labels(&format!("application={}", app.metadata.name)))
                .await?;

            for build in builds {
                if let Some(labels) = &build.metadata.labels {
                    if let Some(device) = labels.get("device") {
                        if let Some(info) = devs.get_mut(device) {
                            update_build_status(info, &build)?;
                        }
                    } else {
                        // This is the app build
                        if let Some(info) = &mut app_build {
                            update_build_status(info, &build)?;
                        }
                    }
                }
            }

            if let Some(info) = app_build {
                all_builds.push(info);
            }

            all_builds.extend(devs.values().cloned().collect::<Vec<BuildInfo>>());
        }

        Ok(all_builds)
    }

    async fn get_app(&self, token: &str, app: &str) -> Result<Option<Application>, anyhow::Error> {
        let drogue = DrogueClient::new(
            reqwest::Client::new(),
            self.device_registry.clone(),
            token.to_string(),
        );
        let app = drogue.get_app(app).await?;
        Ok(app)
    }

    async fn get_device(
        &self,
        token: &str,
        app: &str,
        device: &str,
    ) -> Result<Option<Device>, anyhow::Error> {
        let drogue = DrogueClient::new(
            reqwest::Client::new(),
            self.device_registry.clone(),
            token.to_string(),
        );
        let dev = drogue.get_device(app, device).await?;
        Ok(dev)
    }

    fn is_allowed(&self, app: &str) -> bool {
        self.config.apps.contains(app)
    }

    async fn trigger(
        &self,
        app: &str,
        dev: Option<&str>,
        image: &str,
        spec: FirmwareBuildSpec,
    ) -> Result<(), anyhow::Error> {
        let name = build_name(app, dev);
        let (git_repo, git_rev, project_path) = match spec.source {
            FirmwareBuildSource::GIT { uri, rev, project } => (uri, rev, project),
        };

        let mut run =
            DynamicObject::new(&name, &self.pipeline_run_resource).within(&self.config.namespace);

        run.labels_mut()
            .insert("application".to_string(), app.to_string());
        if let Some(dev) = dev {
            run.labels_mut()
                .insert("device".to_string(), dev.to_string());
        }

        // Delete existing if exists
        if let Ok(_) = self.kube.delete(&name, &Default::default()).await {
            log::info!("Deleted previous pipeline run {}", name);
        }

        let mut params = vec![
            FirmwareBuildEnv {
                name: "GIT_REPO".to_string(),
                value: git_repo,
            },
            FirmwareBuildEnv {
                name: "GIT_REVISION".to_string(),
                value: git_rev,
            },
            FirmwareBuildEnv {
                name: "PROJECT_PATH".to_string(),
                value: project_path,
            },
            FirmwareBuildEnv {
                name: "IMAGE".to_string(),
                value: image.to_string(),
            },
        ];

        if let Some(args) = spec.args {
            params.push(FirmwareBuildEnv {
                name: "CARGO_BUILD_ARGS".to_string(),
                value: args.join(" "),
            });
        }

        if let Some(image) = spec.image {
            params.push(FirmwareBuildEnv {
                name: "BUILDER_IMAGE".to_string(),
                value: image.to_string(),
            });
        }

        run.data["spec"] = json!({
            "params": params,
            "pipelineRef": {
                "name": &self.config.pipeline,
            },
            "serviceAccountName": &self.config.service_account,
            "timeout": spec.timeout.unwrap_or("1h0m0s".to_string()),
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
        });

        self.kube.create(&Default::default(), &run).await?;
        Ok(())
    }
}

fn has_build_spec(spec: &Option<Result<FirmwareSpec, serde_json::Error>>) -> bool {
    if let Some(Ok(FirmwareSpec::OCI {
        image: _,
        image_pull_policy: _,
        build,
    })) = spec
    {
        return build.is_some();
    }
    return false;
}
