use axum::{extract, http::StatusCode, routing::post, Extension, Router};
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::add_extension::AddExtensionLayer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();

    let listen_addr = config.listen_addr.clone();

    let app =
        Router::new()
            .route("/gerrit", post(hook_handler))
            .layer(
                ServiceBuilder::new().layer(AddExtensionLayer::new(ApiContext {
                    config: Arc::new(config),
                })),
            );

    println!("Hosting at {}", listen_addr);

    axum::Server::bind(&listen_addr.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

async fn hook_handler(
    extract::Json(payload): extract::Json<serde_json::Value>,
    ctx: Extension<ApiContext>,
) -> StatusCode {
    let payload_type = match &payload["type"] {
        Value::String(val) => Some(val),
        _ => None,
    };
    if payload_type != Some(&String::from("comment-added")) {
        return StatusCode::BAD_REQUEST;
    }

    let change_id = match &payload["change"]["id"] {
        Value::String(val) => Some(val),
        _ => None,
    };
    let project = match &payload["change"]["project"] {
        Value::String(val) => Some(val),
        _ => None,
    };
    let commit = match &payload["patchSet"]["revision"] {
        Value::String(val) => Some(val),
        _ => None,
    };
    let comment = match &payload["comment"] {
        Value::String(val) => Some(val),
        _ => None,
    };

    let mut lines = comment.unwrap().lines();
    let first_line = lines.nth(0).unwrap();
    if !first_line.contains(r"\check") {
        return StatusCode::OK;
    }

    let request_payload = TektonTrigger {
        commit: commit.unwrap().to_string(),
        change_id: change_id.unwrap().to_string(),
        project: project.unwrap().to_string(),
    };

    let post = reqwest::Client::new()
        .post(&ctx.config.service_addr)
        .json(&request_payload)
        .send()
        .await;

    match post {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// This is the thing we're going to send to Tekton.
#[derive(Serialize, Deserialize)]
struct TektonTrigger {
    commit: String,
    change_id: String,
    project: String,
}

/// Config for this node.
#[derive(clap::Parser, Clone)]
pub struct Config {
    #[clap(long, env)]
    pub service_addr: String,
    #[clap(long, env)]
    pub listen_addr: String,
}

/// Shared context for the routes.
#[derive(Clone)]
struct ApiContext {
    config: Arc<Config>,
}
