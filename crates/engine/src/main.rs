mod http;
mod state;

use std::error;

use wasmtime::component::*;
use wasmtime::{Config, Engine, Store};

use crate::bindings::Extension;
use crate::bindings::quelle::extension::{error as wit_error, novel, source};
use crate::state::State;

mod bindings {
    wasmtime::component::bindgen!({
        path: "../../wit",
        tracing: true,
        with: {
            "quelle:extension/http/client": crate::http::HostClient,
        }
    });
}

fn main() -> Result<(), Box<dyn error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let engine = ExtensionEngine::new()?;

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
}

impl ExtensionEngine {
    pub fn new() -> Result<Self, Box<dyn error::Error>> {
        let engine = Engine::new(Config::new().wasm_component_model(true))?;
        let mut linker = Linker::<State>::new(&engine);
        bindings::quelle::extension::source::add_to_linker(&mut linker, |state| state)?;
        bindings::quelle::extension::novel::add_to_linker(&mut linker, |state| state)?;
        bindings::quelle::extension::http::add_to_linker(&mut linker, |state| &mut state.http)?;
        bindings::quelle::extension::tracing::add_to_linker(&mut linker, |state| state)?;
        bindings::quelle::extension::error::add_to_linker(&mut linker, |state| state)?;

        Ok(Self { engine, linker })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn new_runner(
        &self,
        component: Component,
    ) -> Result<ExtensionRunner, Box<dyn error::Error>> {
        let mut store = Store::new(&self.engine, State::new());
        let extension = Extension::instantiate(&mut store, &component, &self.linker)?;
        extension.call_register_extension(&mut store)?;
        extension.call_init(&mut store)??;
        Ok(ExtensionRunner::new(self, extension, store))
    }
}

pub struct ExtensionRunner<'a> {
    /// Reference to the extension engine. We don't want runner to outlive the engine.
    _engine: &'a ExtensionEngine,
    extension: Extension,
    store: Store<State>,
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
    pub fn meta(mut self) -> Result<(Self, source::SourceMeta), Box<dyn error::Error>> {
        let value = self.extension.call_meta(&mut self.store)?;
        Ok((self, value))
    }

    /// Safe wrapper around [`Extension::fetch_novel_info`].
    pub fn fetch_novel_info(
        mut self,
        url: &str,
    ) -> Result<(Self, Result<novel::Novel, wit_error::Error>), Box<dyn error::Error>> {
        let result = self.extension.call_fetch_novel_info(&mut self.store, url)?;
        Ok((self, result))
    }
}
