use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::Redirect,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing_subscriber::EnvFilter;

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
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("POSTGRESQL_ADDON_URI"))
        .expect("DATABASE_URL ou POSTGRESQL_ADDON_URI doit être défini");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("connexion à PostgreSQL impossible");

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("échec des migrations");

    let app = Router::new()
        .route(
            "/",
            get(|| async { "Bienvenue sur CrabeLink ! Raccourcis un lien avec POST /shorten" }),
        )
        .route("/shorten", post(raccourcir))
        .route("/{code}", get(rediriger))
        .with_state(pool);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("impossible d'ouvrir le port");
    tracing::info!("CrabeLink en écoute sur le port {port}");
    axum::serve(listener, app).await.unwrap();
}

async fn raccourcir(
    State(pool): State<PgPool>,
    Json(body): Json<NouveauLien>,
) -> Result<(StatusCode, Json<LienCourt>), StatusCode> {
    let url = body.url.trim().to_string();
    if !lien_valide(&url) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let code = nanoid::nanoid!(6);
    sqlx::query("INSERT INTO liens (code, url) VALUES ($1, $2)")
        .bind(&code)
        .bind(&url)
        .execute(&pool)
        .await
        .map_err(erreur_interne)?;
    tracing::info!("nouveau lien court : {code}");
    Ok((StatusCode::CREATED, Json(LienCourt { code, url })))
}

async fn rediriger(
    State(pool): State<PgPool>,
    Path(code): Path<String>,
) -> Result<Redirect, StatusCode> {
    let url = sqlx::query_scalar::<_, String>("SELECT url FROM liens WHERE code = $1")
        .bind(&code)
        .fetch_optional(&pool)
        .await
        .map_err(erreur_interne)?;
    url.map(|url| Redirect::temporary(&url))
        .ok_or(StatusCode::NOT_FOUND)
}

fn erreur_interne(e: sqlx::Error) -> StatusCode {
    tracing::error!("erreur base de données : {e}");
    StatusCode::INTERNAL_SERVER_ERROR
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
