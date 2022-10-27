// Copyright 2022 James Pace
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
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
            .route("/gerrit", post(gerrit_handler))
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

async fn gerrit_handler(
    extract::Json(payload): extract::Json<serde_json::Value>,
    ctx: Extension<ApiContext>,
) -> StatusCode {
    println!("Got a request! : {}", &payload);

    let payload_type = match &payload["type"] {
        Value::String(val) => Some(val),
        _ => None,
    };
    if payload_type != Some(&String::from("comment-added")) {
        println!("Not the right type of request.");
        return StatusCode::BAD_REQUEST;
    }

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

    let lines = comment.unwrap().lines();
    let last_line = lines.last().unwrap();
    if !last_line.contains(r"\check") {
        println!(r"Last line does not contain \check.");
        return StatusCode::OK;
    }

    let url = format!("{}/{}", &ctx.config.clone_url, project.unwrap());
    let request_payload = TektonTrigger {
        commit: commit.unwrap().to_string(),
        clone_url: url,
        feedback_url: ctx.config.feedback_url.clone(),
        feedback_port: ctx.config.feedback_port.clone(),
    };

    println!(
        "Sending: {}",
        serde_json::to_string(&request_payload).unwrap()
    );

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

/// This is the thing we're going to send to Tekton.
#[derive(Serialize, Deserialize)]
struct TektonTrigger {
    commit: String,
    clone_url: String,
    feedback_url: String,
    feedback_port: String,
}

/// Config for this node.
#[derive(clap::Parser, Clone)]
pub struct Config {
    #[clap(long, env)]
    pub service_addr: String,
    #[clap(long, env)]
    pub listen_addr: String,
    #[clap(long, env)]
    pub clone_url: String,
    #[clap(long, env)]
    pub feedback_url: String,
    #[clap(long, env)]
    pub feedback_port: String,
}

/// Shared context for the routes.
#[derive(Clone)]
struct ApiContext {
    config: Arc<Config>,
}
