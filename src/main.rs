use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Redirect,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

type Liens = Arc<Mutex<HashMap<String, String>>>;

#[derive(Deserialize)]
struct NouveauLien {
    url: String,
}

#[derive(Serialize)]
struct LienCourt {
    code: String,
    url: String,
}

fn lien_valide(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://")) && url.len() <= 2048
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let liens: Liens = Arc::new(Mutex::new(HashMap::new()));

    let app = Router::new()
        .route("/shorten", post(raccourcir))
        .route("/{code}", get(rediriger))
        .with_state(liens);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("impossible d'ouvrir le port");
    tracing::info!("CrabeLink en écoute sur le port {port}");
    axum::serve(listener, app).await.unwrap();
}

async fn raccourcir(
    State(liens): State<Liens>,
    Json(body): Json<NouveauLien>,
) -> Result<(StatusCode, Json<LienCourt>), StatusCode> {
    let url = body.url.trim().to_string();
    if !lien_valide(&url) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let code = nanoid::nanoid!(6);
    liens.lock().unwrap().insert(code.clone(), url.clone());
    tracing::info!("nouveau lien court : {code}");
    Ok((StatusCode::CREATED, Json(LienCourt { code, url })))
}

async fn rediriger(
    State(liens): State<Liens>,
    Path(code): Path<String>,
) -> Result<Redirect, StatusCode> {
    liens
        .lock()
        .unwrap()
        .get(&code)
        .map(|url| Redirect::temporary(url))
        .ok_or(StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepte_un_lien_https() {
        assert!(lien_valide("https://www.clever.cloud"));
    }

    #[test]
    fn refuse_un_lien_sans_http() {
        assert!(!lien_valide("clever.cloud"));
    }

    #[test]
    fn refuse_un_lien_trop_long() {
        assert!(!lien_valide(&format!("https://{}", "a".repeat(2048))));
    }
}
