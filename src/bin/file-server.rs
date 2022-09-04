use anyhow::{bail, Result};
use axum::{extract::Multipart, routing::post, Router};
use std::env;
use std::path::Path;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let file_dir = Arc::new(env::var("FILE_DIR").unwrap_or("files".to_owned()));
    let listen_addr = env::var("LISTEN_ADDR").unwrap_or("0.0.0.0:3000".to_owned());

    let file_path = Path::new(&*file_dir);
    if !file_path.exists() {
        bail!("File path '{}' not found.", file_path.display());
    }

    let app = Router::new()
        .route(
            "/upload",
            post({
                let file_dir = Arc::clone(&file_dir);
                move |body| upload(body, Arc::clone(&file_dir))
            }),
        )
        .merge(axum_extra::routing::SpaRouter::new("/download", &*file_dir));

    println!("Hosting at {}", listen_addr);

    axum::Server::bind(&listen_addr.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

async fn upload(mut multipart: Multipart, file_dir: Arc<String>) {
    let file_dir = (*file_dir).as_str();

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
