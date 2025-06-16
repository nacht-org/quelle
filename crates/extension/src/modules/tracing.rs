use crate::wit::quelle::extension::tracing as wit_tracing;
use tracing::{Level, Subscriber};
use tracing_subscriber::{layer::Layer, registry::LookupSpan};

pub struct HostLayer;

impl<S> Layer<S> for HostLayer
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();

        let level = if metadata.level() == &Level::ERROR {
            wit_tracing::LogLevel::Error
        } else if metadata.level() == &Level::WARN {
            wit_tracing::LogLevel::Warn
        } else if metadata.level() == &Level::INFO {
            wit_tracing::LogLevel::Info
        } else if metadata.level() == &Level::DEBUG {
            wit_tracing::LogLevel::Debug
        } else if metadata.level() == &Level::TRACE {
            wit_tracing::LogLevel::Trace
        } else {
            unreachable!()
        };

        let attributes = {
            let mut visitor = TracingVisitor::default();
            event.record(&mut visitor);
            visitor.attributes
        };

        let message = attributes
            .iter()
            .find(|(name, _)| name == "message")
            .map(|(_, value)| value.to_string())
            .unwrap_or_else(|| "No message".to_string());

        let attributes = attributes
            .into_iter()
            .filter(|(name, _)| name != "message")
            .collect::<Vec<_>>();

        let wit_event = wit_tracing::LogEvent {
            level,
            target: metadata.target().to_string(),
            message,
            attributes,
            file: metadata.file().map(|s| s.to_string()),
            line: metadata.line(),
        };

        wit_tracing::on_event(&wit_event);
    }
}

#[derive(Debug, Default)]
struct TracingVisitor {
    attributes: Vec<(String, String)>,
}

impl tracing::field::Visit for TracingVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.attributes
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.attributes
            .push((field.name().to_string(), format!("{:?}", value)));
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.attributes
            .push((field.name().to_string(), format!("{}", value)));
    }
}
