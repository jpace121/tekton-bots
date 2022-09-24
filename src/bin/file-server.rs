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
use anyhow::{bail, Result};
use axum::{extract::Multipart, routing::post, Extension, Router};
use clap::Parser;
use std::path::Path;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::add_extension::AddExtensionLayer;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::parse();

    let file_path = Path::new(&*config.file_dir);
    if !file_path.exists() {
        bail!("File path '{}' not found.", file_path.display());
    }

    let app = Router::new()
        .route("/upload", post(upload))
        .merge(axum_extra::routing::SpaRouter::new(
            "/download",
            &config.file_dir,
        ))
        .layer(
            ServiceBuilder::new().layer(AddExtensionLayer::new(ApiContext {
                config: Arc::new(config.clone()),
            })),
        );

    println!("Hosting at {}", config.listen_addr);

    axum::Server::bind(&config.listen_addr.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

async fn upload(mut multipart: Multipart, ctx: Extension<ApiContext>) {
    let file_dir = ctx.config.file_dir.as_str();

    let mut data = None;
    let mut path = None;

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        match name.as_str() {
            "data" => data = Some(field.bytes().await.unwrap()),
            "path" => path = Some(field.text().await.unwrap()),
            _ => continue,
        }
    }

    if data.is_some() & path.is_some() {
        let path = path.unwrap().clone();
        let clean_path = Path::new(path.as_str()).file_name().unwrap();
        let joined_path = Path::new(file_dir).join(clean_path);
        tokio::fs::write(&joined_path, data.unwrap()).await.unwrap();
        println!("Uploaded file {}", joined_path.display());
    } else {
        println!("Failed to find data or path in upload request.");
    }
}

/// Config for this node.
#[derive(clap::Parser, Clone)]
pub struct Config {
    #[clap(long, env)]
    pub file_dir: String,
    #[clap(long, env)]
    pub listen_addr: String,
}

/// Shared context for the routes.
#[derive(Clone)]
struct ApiContext {
    config: Arc<Config>,
}
