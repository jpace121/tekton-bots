use axum::{extract, http::StatusCode, routing::post, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listen_addr = env::var("LISTEN_ADDR").unwrap_or("0.0.0.0:3000".to_owned());

    let app = Router::new().route("/gerrit", post(hook_handler));

    println!("Hosting at {}", listen_addr);

    axum::Server::bind(&listen_addr.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

async fn hook_handler(extract::Json(payload): extract::Json<serde_json::Value>) -> StatusCode {
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
        .post("http://todo.todo.todo/todo")
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
