pub mod routes;
pub mod settings;
pub mod state;
pub mod utils;

use std::sync::Arc;

use crate::state::AppState;
use axum::{Router, http, routing::get};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

pub type Server = axum::serve::Serve<TcpListener, axum::Router, axum::Router>;

pub async fn run(listener: TcpListener, state: AppState) -> Result<Server, std::io::Error> {
    let host = state.settings.server.host.clone();
    let port = state.settings.server.port;

    let app = Router::new()
        .route("/health", get(routes::health))
        .layer(TraceLayer::new_for_http().make_span_with(RequestSpan))
        .with_state(Arc::new(state));

    tracing::info!("Starting server at {host}:{port}");

    let server = axum::serve(listener, app);

    Ok(server)
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
