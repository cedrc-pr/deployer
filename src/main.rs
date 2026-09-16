use std::sync::Arc;

use poem::{
    EndpointExt, Request, Route, Server,
    http::StatusCode,
    listener::TcpListener,
    post,
    web::{Data, Json},
};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::RwLock;

use crate::{config::Config, utils::setup_tracing};

pub mod config;
pub mod utils;

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    setup_tracing();
    tracing::debug!("pid: {}", std::process::id());
    let _ = start_server().await;
}

async fn start_server() -> anyhow::Result<()> {
    let bind = std::env::var("BIND_ADDR").map_err(|_| anyhow::anyhow!("no BIND_ADDR defined"))?;
    let server = Server::new(TcpListener::bind(&bind));
    let config = Arc::new(RwLock::new(Config::default()));
    config.write().await.load().await?;

    let config_reload = config.clone();
    tokio::spawn(async move {
        let mut sighup = signal(SignalKind::hangup()).unwrap();
        loop {
            if sighup.recv().await.is_none() {
                break;
            }
            tracing::info!("hangup received");
            let _ = config_reload.write().await.load().await;
        }
    });

    tracing::info!("server listening on: {}", bind);
    match server
        .run(Route::new().at("/", post(deployer)).data(config))
        .await
    {
        Ok(_) => Ok(()),
        Err(err) => Err(anyhow::anyhow!(err)),
    }
}

#[derive(serde::Deserialize)]
struct Body {
    service: String,
    args: Option<Vec<String>>,
}

#[poem::handler]
async fn deployer(
    req: &Request,
    Data(config): Data<&Arc<RwLock<Config>>>,
    Json(body): Json<Body>,
) -> StatusCode {
    if let Some(auth) = req.headers().get("authorization")
        && let Ok(bearer) = auth.to_str()
        && let Some(token) = bearer
            .strip_prefix("Bearer ")
            .or_else(|| bearer.strip_prefix("bearer "))
        && let Some(service) = config.read().await.services.get(&body.service)
        && service.token == token
    {
        let args = body.args.unwrap_or(vec![]);
        tracing::info!(
            "service: {}, running command: {:?} {}",
            &body.service,
            &service.path,
            &args.join(" ")
        );
        let status = match std::process::Command::new(&service.path)
            .args(args)
            .status()
        {
            Ok(status) => status,
            Err(err) => {
                tracing::error!("command error: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
        };
        if status.success() {
            tracing::info!("command succeed");
        } else {
            tracing::warn!("command status error");
        }
        return StatusCode::ACCEPTED;
    }
    StatusCode::BAD_REQUEST
}
