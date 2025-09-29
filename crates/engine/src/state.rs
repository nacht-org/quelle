use std::sync::Arc;

use crate::bindings::quelle::extension::tracing::LogEvent;
use crate::bindings::quelle::extension::{
    error as wit_error, novel, source, time as wit_time, tracing as wit_tracing,
};
use crate::http::{Http, HttpExecutor};
use chrono::Local;
use tracing::event;
use wasmtime::component::HasData;

pub struct State {
    pub http: Http,
    pub panic_error: Option<wit_error::Error>,
}

impl State {
    pub fn new(executor: Arc<dyn HttpExecutor>) -> Self {
        Self {
            http: Http::new(executor),
            panic_error: None,
        }
    }
}

impl HasData for State {
    type Data<'a> = &'a mut Self;
}

impl novel::Host for State {}

impl source::Host for State {}

impl wit_error::Host for State {
    fn report_panic(&mut self, error: wit_error::Error) {
        self.panic_error = Some(error);
    }
}

impl wit_tracing::Host for State {
    fn on_event(&mut self, event: LogEvent) {
        macro_rules! log_event {
            ($level:expr) => {
                event!(
                    target: "quelle_extension",
                    $level,
                    message = event.message,
                    wasm_target = event.target,
                    wasm_file = event.file.as_deref(),
                    wasm_line = event.line,
                    wasm_attributes = ?event.attributes,
                )
            };
        }

        match event.level {
            wit_tracing::LogLevel::Debug => log_event!(tracing::Level::DEBUG),
            wit_tracing::LogLevel::Error => log_event!(tracing::Level::ERROR),
            wit_tracing::LogLevel::Info => log_event!(tracing::Level::INFO),
            wit_tracing::LogLevel::Trace => log_event!(tracing::Level::TRACE),
            wit_tracing::LogLevel::Warn => log_event!(tracing::Level::WARN),
        }
    }
}

impl wit_time::Host for State {
    fn local_now_millis(&mut self) -> i64 {
        Local::now().timestamp_millis()
    }
}
