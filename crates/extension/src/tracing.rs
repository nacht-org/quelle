use crate::wit::quelle::extension::tracing as wit_tracing;
use tracing::{Level, Subscriber};
use tracing_subscriber::layer::Layer;

pub struct HostLayer;

impl<S: Subscriber> Layer<S> for HostLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();

        wit_tracing::Event {
            metadata: wit_tracing::Metadata {
                name: metadata.name().to_string(),
                target: metadata.target().to_string(),
                level: if metadata.level() == &Level::ERROR {
                    wit_tracing::Level::Error
                } else if metadata.level() == &Level::WARN {
                    wit_tracing::Level::Warn
                } else if metadata.level() == &Level::INFO {
                    wit_tracing::Level::Info
                } else if metadata.level() == &Level::DEBUG {
                    wit_tracing::Level::Debug
                } else if metadata.level() == &Level::TRACE {
                    wit_tracing::Level::Trace
                } else {
                    unreachable!()
                },
                module_path: metadata.module_path().map(|s| s.to_string()),
                file: metadata.file().map(|s| s.to_string()),
                line: metadata.line().map(|l| l as u32),
                kind: if metadata.is_span() {
                    wit_tracing::Kind::Span
                } else if metadata.is_event() {
                    wit_tracing::Kind::Event
                } else {
                    wit_tracing::Kind::Other
                },
            },
            parent: if event.is_contextual() {
                wit_tracing::Parent::Current
            } else if event.is_root() {
                wit_tracing::Parent::Root
            } else if let Some(id) = event.parent() {
                wit_tracing::Parent::Explicit(id.into_u64())
            } else {
                unreachable!("Event must have a parent or be root")
            },
        };
    }
}
