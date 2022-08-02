use actix_web::{
    middleware, web, App, HttpRequest, HttpResponse, HttpResponseBuilder, HttpServer, Responder,
};
use ajour_schema::*;
use anyhow::anyhow;
use chrono::{offset::Utc, DateTime};
use clap::Parser;
use drogue_client::core::v1::Conditions;
use drogue_client::openid::AccessTokenProvider;
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
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;

pub struct State {
    config: ApiConfig,
    drogue: DrogueClient,
    kube: Api<DynamicObject>,
    pipeline_run_resource: ApiResource,
}

pub struct ApiConfig {
    namespace: String,
    pipeline: String,
    volume_size: String,
    service_account: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct BuildInfo {
    app: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
}

impl BuildInfo {
    fn parse(
        app: &str,
        device: Option<&str>,
        object: &DynamicObject,
    ) -> Result<BuildInfo, anyhow::Error> {
        let started = object.meta().creation_timestamp.clone().map(|s| s.0);
        let mut completed = None;
        let mut status = None;
        if let Some(s) = object.data.get("status") {
            if let Some(conditions) = s.get("conditions") {
                let conditions: Conditions = serde_json::from_value(conditions.clone())?;
                for condition in conditions.iter() {
                    if condition.r#type == "Succeeded" {
                        status = condition.reason.clone();
                        completed = Some(condition.last_transition_time);
                        break;
                    }
                }
            }
        }
        Ok(BuildInfo {
            app: app.to_string(),
            device: device.map(|s| s.to_string()),
            started,
            completed,
            status,
        })
    }
}

impl State {
    pub fn new(
        config: ApiConfig,
        drogue: DrogueClient,
        kube: Api<DynamicObject>,
        pipeline_run_resource: ApiResource,
    ) -> Self {
        Self {
            config,
            drogue,
            kube,
            pipeline_run_resource,
        }
    }

    async fn get_builds(&self) -> Result<Vec<BuildInfo>, anyhow::Error> {
        let apps = self.drogue.list_apps(None).await?;
        let apps = apps.ok_or(anyhow!("Unable to find any apps"))?;

        let mut all_builds = Vec::new();

        // First find all builds with app label
        //
        for app in apps {
            let builds: ObjectList<DynamicObject> = self
                .kube
                .list(&ListParams::default().labels(&format!("application={}", app.metadata.name)))
                .await?;

            // Find all devices for this app
            let devs: Vec<Device> = self
                .drogue
                .list_devices(&app.metadata.name, None)
                .await?
                .unwrap_or(Vec::new());

            let devs: HashSet<String> =
                HashSet::from_iter(devs.iter().map(|dev| dev.metadata.name.clone()));

            // Filter builds for devices we have
            for build in builds {
                if let Some(labels) = &build.metadata.labels {
                    if let Some(device) = labels.get("device") {
                        if devs.contains(device) {
                            all_builds.push(BuildInfo::parse(
                                &app.metadata.name,
                                Some(device),
                                &build,
                            )?);
                        }
                    } else if let Some(app_name) = labels.get("application") {
                        if &app.metadata.name == app_name {
                            all_builds.push(BuildInfo::parse(&app_name, None, &build)?);
                        }
                    }
                }
            }
        }
        Ok(all_builds)
    }

    async fn get_app(&self, app: &str) -> Result<Option<Application>, anyhow::Error> {
        let app = self.drogue.get_app(app).await?;
        Ok(app)
    }

    async fn get_device(&self, app: &str, device: &str) -> Result<Option<Device>, anyhow::Error> {
        let dev = self.drogue.get_device(app, device).await?;
        Ok(dev)
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

        let build_args = if let Some(args) = spec.args {
            args.join(" ")
        } else {
            String::new()
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

        run.data["spec"] = json!({
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
                    "value": image,
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

#[derive(Parser, Debug)]
struct Args {
    /// Device registry URL
    /// Mqtt server uri (tcp://host:port)
    #[clap(long)]
    device_registry: String,

    /// Kubernetes namespace
    #[clap(long)]
    namespace: String,

    /// Token for authenticating ajour to Drogue IoT
    #[clap(long)]
    token: String,

    /// User for authenticating ajour to Drogue IoT
    #[clap(long)]
    user: String,

    /// Port for health endpoint
    #[clap(long, default_value_t = 8080)]
    port: u16,
}

#[actix_web::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    let args = Args::parse();
    let token = args.token;
    let user = args.user;
    let namespace = args.namespace;
    let registry_url = reqwest::Url::parse(&args.device_registry).unwrap();

    let tp = AccessTokenProvider {
        user: user.to_string(),
        token: token.to_string(),
    };
    let drogue = DrogueClient::new(reqwest::Client::new(), registry_url, tp);

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
        service_account: "pipeline".to_string(),
        pipeline: "oci-firmware".to_string(),
        volume_size: "10Gi".to_string(),
        namespace,
    };

    let state = Arc::new(State::new(
        config,
        drogue,
        pipeline_runs,
        pipeline_run_resource,
    ));
    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
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
    .bind(("127.0.0.1", args.port))?
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

async fn get_builds(state: web::Data<Arc<State>>, _request: HttpRequest) -> impl Responder {
    if let Ok(builds) = state.get_builds().await {
        HttpResponse::Ok().json(builds)
    } else {
        HttpResponse::Ok().finish()
    }
}

async fn trigger_app_build(
    state: web::Data<Arc<State>>,
    _request: HttpRequest,
    app_id: web::Path<String>,
) -> impl Responder {
    match state.get_app(&app_id).await {
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
        }
        _ => {
            log::info!("Firmware registry not yet supported for builds");
            HttpResponse::NotImplemented()
        }
    }
}

async fn trigger_device_build(
    state: web::Data<Arc<State>>,
    _request: HttpRequest,
    ids: web::Path<(String, String)>,
) -> impl Responder {
    match state.get_device(&ids.0, &ids.1).await {
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
}
