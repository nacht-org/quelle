use tokio::task::JoinHandle;
use tracing::{Subscriber, subscriber::set_global_default};
use tracing_subscriber::{
    EnvFilter, Registry,
    fmt::{self, MakeWriter},
    layer::SubscriberExt,
};

/// Builds a tracing subscriber with environment-based filtering and custom log output.
///
/// Use this to set up structured logging and error reporting for your application.
pub fn get_subscriber<Sink>(env_filter: String, sink: Sink) -> impl Subscriber + Sync + Send
where
    Sink: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));
    let formatting_layer = fmt::layer().with_writer(sink);

    Registry::default()
        .with(env_filter)
        .with(formatting_layer)
        .with(tracing_error::ErrorLayer::default())
}

/// Installs the given tracing subscriber as the global default.
///
/// Call this early in your application to enable tracing/logging.
/// Panics if a global subscriber is already set.
pub fn init_subscriber(subscriber: impl Subscriber + Sync + Send) {
    set_global_default(subscriber).expect("Failed to set subscriber");
}

/// Runs a blocking closure on a separate thread, preserving the current tracing span.
///
/// Use this when you need to run CPU-bound or blocking code and want logs/tracing to be attributed correctly.
pub fn spawn_blocking_with_tracing<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let current_span = tracing::Span::current();
    tokio::task::spawn_blocking(move || current_span.in_scope(f))
}
