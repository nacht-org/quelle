mod bindings;
mod error;
mod http;
mod state;

use std::sync::Arc;

use wasmtime::component::*;
use wasmtime::{Config, Engine, Store};

use crate::bindings::Extension;
use crate::bindings::quelle::extension::{error as wit_error, novel, source};
use crate::http::{HeadlessChromeExecutor, HttpExecutor};
use crate::state::State;

fn main() -> error::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let engine = ExtensionEngine::new(Arc::new(HeadlessChromeExecutor::new()))?;

    let component = Component::from_file(
        &engine.engine(),
        "target/wasm32-unknown-unknown/release/extension_scribblehub.wasm",
    )?;

    let runner = engine.new_runner(component)?;

    let (runner, extension_meta) = runner.meta()?;
    println!("Extension: {:?}", extension_meta);

    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        eprintln!("Usage: {} <url>", args[0]);
        std::process::exit(1);
    }

    let url = &args[1];
    let (_runner, result) = runner.fetch_novel_info(url)?;

    println!("Novel: {:?}", result);

    Ok(())
}

pub struct ExtensionEngine {
    engine: Engine,
    linker: Linker<State>,
    executor: Arc<dyn HttpExecutor>,
}

impl ExtensionEngine {
    pub fn new(executor: Arc<dyn HttpExecutor>) -> error::Result<Self> {
        let engine = Engine::new(Config::new().wasm_component_model(true))?;
        let mut linker = Linker::<State>::new(&engine);
        crate::bindings::quelle::extension::source::add_to_linker(&mut linker, |state| state)?;
        crate::bindings::quelle::extension::novel::add_to_linker(&mut linker, |state| state)?;
        crate::bindings::quelle::extension::http::add_to_linker(&mut linker, |state| {
            &mut state.http
        })?;
        crate::bindings::quelle::extension::tracing::add_to_linker(&mut linker, |state| state)?;
        crate::bindings::quelle::extension::error::add_to_linker(&mut linker, |state| state)?;

        Ok(Self {
            engine,
            linker,
            executor,
        })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn new_runner(&'_ self, component: Component) -> error::Result<ExtensionRunner<'_>> {
        let mut store = Store::new(&self.engine, State::new(self.executor.clone()));
        let extension =
            crate::bindings::Extension::instantiate(&mut store, &component, &self.linker)?;

        extension
            .call_register_extension(&mut store)
            .map_err(|e| error::Error::RuntimeError {
                wasmtime_error: e,
                panic_error: store.data_mut().panic_error.take(),
            })?;

        extension
            .call_init(&mut store)
            .map_err(|e| error::Error::RuntimeError {
                wasmtime_error: e,
                panic_error: store.data_mut().panic_error.take(),
            })??;

        Ok(ExtensionRunner::new(self, extension, store))
    }
}

pub struct ExtensionRunner<'a> {
    /// Reference to the extension engine. We don't want runner to outlive the engine.
    _engine: &'a ExtensionEngine,
    extension: Extension,
    store: Store<State>,
}

/// A macro to wrap calls to extension methods, handling errors and returning a tuple of the runner and the result.
macro_rules! wrap_extension_method {
    ($self:expr, $name:ident $(, $arg:expr )*) => {{
        let result = $self.extension.$name(&mut $self.store $(, $arg)*);
        match result {
            Ok(value) => Ok(($self, value)),
            Err(e) => Err(error::Error::RuntimeError {
                wasmtime_error: e,
                panic_error: $self.store.data_mut().panic_error.take(),
            }),
        }
    }};
}

impl<'a> ExtensionRunner<'a> {
    pub fn new(engine: &'a ExtensionEngine, extension: Extension, store: Store<State>) -> Self {
        Self {
            _engine: engine,
            extension,
            store,
        }
    }

    /// Safe wrapper around [`Extension::call_meta`].
    pub fn meta(mut self) -> error::Result<(Self, source::SourceMeta)> {
        wrap_extension_method!(self, call_meta)
    }

    /// Safe wrapper around [`Extension::fetch_novel_info`].
    pub fn fetch_novel_info(
        mut self,
        url: &str,
    ) -> error::Result<(Self, Result<novel::Novel, wit_error::Error>)> {
        wrap_extension_method!(self, call_fetch_novel_info, url)
    }

    /// Safe wrapper around [`Extension::fetch_chapter`].
    pub fn fetch_chapter(
        mut self,
        url: &str,
    ) -> error::Result<(Self, Result<novel::ChapterContent, wit_error::Error>)> {
        wrap_extension_method!(self, call_fetch_chapter, url)
    }
}
