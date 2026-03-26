pub mod error;
pub mod routes;
pub mod settings;
pub mod state;
pub mod utils;

use std::sync::Arc;

use crate::state::AppState;
use aide::{
    axum::{ApiRouter, IntoApiResponse, routing::get},
    openapi::{Info, OpenApi},
};
use axum::{Extension, Json, Router, http, routing::IntoMakeService};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

pub type Server = axum::serve::Serve<TcpListener, IntoMakeService<Router>, Router>;

pub async fn run(listener: TcpListener, state: AppState) -> Result<Server, std::io::Error> {
    let host = state.settings.server.host.clone();
    let port = state.settings.server.port;

    let app = ApiRouter::new()
        .api_route("/health", get(routes::health))
        .route("/openapi.json", get(serve_api))
        .nest("/extensions", routes::extensions::router())
        .layer(TraceLayer::new_for_http().make_span_with(RequestSpan))
        .with_state(Arc::new(state));

    tracing::info!("Starting server at {host}:{port}");

    let mut api = OpenApi {
        info: Info {
            title: "Quelle API".to_string(),
            description: Some(
                "API for managing and executing extensions in the Quelle system".to_string(),
            ),
            ..Info::default()
        },
        ..OpenApi::default()
    };

    let server = axum::serve(
        listener,
        app.finish_api(&mut api)
            .layer(Extension(api))
            .into_make_service(),
    );

    Ok(server)
}

async fn serve_api(Extension(api): Extension<OpenApi>) -> impl IntoApiResponse {
    Json(api)
}

#[derive(Clone)]
pub struct RequestSpan;

impl<B> tower_http::trace::MakeSpan<B> for RequestSpan {
    fn make_span(&mut self, request: &http::Request<B>) -> tracing::Span {
        tracing::info_span!(
            "request",
            request_id = %Uuid::new_v4().to_string(),
            method = %request.method(),
            uri = %request.uri(),
            version = ?request.version(),
        )
    }
}
