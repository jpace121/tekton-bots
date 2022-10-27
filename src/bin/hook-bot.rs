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
use axum::{extract, http::header::HeaderMap, http::StatusCode, routing::post, Extension, Router};
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

    let app = Router::new()
        .route("/gerrit", post(gerrit_handler))
        .route("/gitea", post(gitea_handler))
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
    println!("Got a request from gerrit!");

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

    let url = format!("{}/{}", &ctx.config.gerrit_clone_url, project.unwrap());
    let request_payload = TektonTrigger {
        commit: commit.unwrap().to_string(),
        clone_url: url,
        feedback_url: ctx.config.gerrit_feedback_url.clone(),
        feedback_port: ctx.config.gerrit_feedback_port.clone(),
    };

    println!("Sending request to Tekton from gerrit!",);

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

async fn gitea_handler(
    extract::Json(payload): extract::Json<serde_json::Value>,
    headers: HeaderMap,
    ctx: Extension<ApiContext>,
) -> StatusCode {
    //println!("Got a request! : {}", &payload);
    //println!("With headers!");
    //for (key, value) in headers.iter() {
    //println!("{:?}: {:?}", key, value);
    //}

    println!("Got a request from gitea!");

    // Make sure we have the right type of event.
    let request_type = headers.get("x-gitea-event-type");
    if request_type.is_none() || request_type.unwrap() != "pull_request_comment" {
        println!("Not the right type of event!");
        return StatusCode::BAD_REQUEST;
    }

    // Pull the things I need from request body:
    // 1) comment.body
    // 2) issue.id
    // 3) issue.repository.owner
    // 4) issue.repository.name
    // 5) repository.ssh_url
    let comment = match &payload["comment"]["body"] {
        Value::String(val) => Some(val),
        _ => None,
    };
    let issue_id = match &payload["issue"]["id"] {
        Value::String(val) => Some(val),
        _ => None,
    };
    let repo_owner = match &payload["issue"]["repository"]["owner"] {
        Value::String(val) => Some(val),
        _ => None,
    };
    let repo_name = match &payload["issue"]["repository"]["name"] {
        Value::String(val) => Some(val),
        _ => None,
    };
    let ssh_url = match &payload["repository"]["ssh_url"] {
        Value::String(val) => Some(val),
        _ => None,
    };

    // Should we check this PR?
    // Check the arguments.
    if comment.is_none() || issue_id.is_none() {
        println!("Couldn't find the comment body or the issue number?");
        return StatusCode::BAD_REQUEST;
    }
    // Check the content.
    let lines = comment.unwrap().lines();
    let last_line = lines.last().unwrap();
    if !last_line.contains(r"\check") {
        println!(r"Last line does not contain \check.");
        return StatusCode::OK;
    }

    // Get the commit from the gitea API.
    let commit_request = format!(
        "{}/repos/{}/pulls/{}/commits",
        &ctx.config.gitea_api_url,
        repo_owner.unwrap(),
        repo_name.unwrap(),
        index.unwrap()
    );
    let raw_response = reqwest::Client::new()
        .send(commit_request)
        .send()
        .await;

    StatusCode::OK

    /*
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
    */
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
    pub gerrit_clone_url: String,
    #[clap(long, env)]
    pub gerrit_feedback_url: String,
    #[clap(long, env)]
    pub gerrit_feedback_port: String,
    #[clap(long, env)]
    pub gitea_api_url: String,
}

/// Shared context for the routes.
#[derive(Clone)]
struct ApiContext {
    config: Arc<Config>,
}
