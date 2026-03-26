pub mod error;
pub mod routes;
pub mod settings;
pub mod state;
pub mod utils;

use std::sync::Arc;

use crate::state::AppState;
use aide::{
    axum::{ApiRouter, IntoApiResponse},
    openapi::{Info, OpenApi, Tag},
    transform::TransformOpenApi,
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
        .merge(routes::routes())
        .merge(routes::docs::routes())
        .nest("/extensions", routes::extensions::routes())
        .layer(TraceLayer::new_for_http().make_span_with(RequestSpan))
        .with_state(Arc::new(state));

    tracing::info!("Starting server at {host}:{port}");

    let mut api = OpenApi {
        info: Info {
            title: "Quelle API".to_string(),
            description: Some(
                "API for managing and executing extensions in the Quelle system".to_string(),
            ),
            version: "0.1.0".to_string(),
            ..Info::default()
        },
        ..OpenApi::default()
    };

    let server = axum::serve(
        listener,
        app.finish_api_with(&mut api, |api: TransformOpenApi| {
            api.tag(Tag {
                name: "System".to_string(),
                description: Some("System-level operations such as health checks.".to_string()),
                ..Tag::default()
            })
            .tag(Tag {
                name: "Extensions".to_string(),
                description: Some("Operations for managing and querying extensions.".to_string()),
                ..Tag::default()
            })
        })
        .layer(Extension(api))
        .into_make_service(),
    );

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
