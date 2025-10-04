//! WebAssembly extension engine for Quelle.
//!
//! This crate provides the runtime environment for executing WebAssembly extensions
//! that handle novel fetching, searching, and content extraction from various sources.

pub mod bindings;
pub mod error;
pub mod http;
mod state;

use std::sync::Arc;

use wasmtime::component::*;
use wasmtime::{Config, Engine, Store};

use crate::bindings::quelle::extension::{error as wit_error, novel, source};
use crate::bindings::{ComplexSearchQuery, Extension, SearchResult, SimpleSearchQuery};
use crate::http::HttpExecutor;
use crate::state::State;

pub struct ExtensionEngine {
    engine: Engine,
    linker: Linker<State>,
    executor: Arc<dyn HttpExecutor>,
}

impl ExtensionEngine {
    pub fn new(executor: Arc<dyn HttpExecutor>) -> error::Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);

        let engine = Engine::new(&mut config)?;
        let mut linker = Linker::<State>::new(&engine);
        crate::bindings::quelle::extension::source::add_to_linker::<_, HasSelf<_>>(
            &mut linker,
            |state| state,
        )?;
        crate::bindings::quelle::extension::novel::add_to_linker::<_, HasSelf<_>>(
            &mut linker,
            |state| state,
        )?;
        crate::bindings::quelle::extension::http::add_to_linker::<_, HasSelf<_>>(
            &mut linker,
            |state| &mut state.http,
        )?;
        crate::bindings::quelle::extension::tracing::add_to_linker::<_, HasSelf<_>>(
            &mut linker,
            |state| state,
        )?;
        crate::bindings::quelle::extension::error::add_to_linker::<_, HasSelf<_>>(
            &mut linker,
            |state| state,
        )?;
        crate::bindings::quelle::extension::time::add_to_linker::<_, HasSelf<_>>(
            &mut linker,
            |state| state,
        )?;

        Ok(Self {
            engine,
            linker,
            executor,
        })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub async fn new_runner_from_file(&'_ self, path: &str) -> error::Result<ExtensionRunner<'_>> {
        let component = Component::from_file(&self.engine, path)?;
        self.new_runner(component).await
    }

    pub async fn new_runner_from_bytes(
        &'_ self,
        bytes: &[u8],
    ) -> error::Result<ExtensionRunner<'_>> {
        let component = Component::from_binary(&self.engine, bytes)?;
        self.new_runner(component).await
    }

    pub async fn new_runner(&'_ self, component: Component) -> error::Result<ExtensionRunner<'_>> {
        let mut store = Store::new(&self.engine, State::new(self.executor.clone()));
        let extension =
            crate::bindings::Extension::instantiate_async(&mut store, &component, &self.linker)
                .await?;

        extension
            .call_register_extension(&mut store)
            .await
            .map_err(|e| error::Error::RuntimeError {
                wasmtime_error: e,
                panic_error: store.data_mut().panic_error.take(),
            })?;

        extension
            .call_init(&mut store)
            .await
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

/// Wraps calls to extension methods, handling errors and returning a tuple of the runner and the result.
macro_rules! wrap_extension_method {
    ($self:expr, $name:ident $(, $arg:expr )*) => {{
        let result = $self.extension.$name(&mut $self.store $(, $arg)*).await;
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
    pub async fn meta(mut self) -> error::Result<(Self, source::SourceMeta)> {
        wrap_extension_method!(self, call_meta)
    }

    /// Safe wrapper around [`Extension::fetch_novel_info`].
    pub async fn fetch_novel_info(
        mut self,
        url: &str,
    ) -> error::Result<(Self, Result<novel::Novel, wit_error::Error>)> {
        wrap_extension_method!(self, call_fetch_novel_info, url)
    }

    /// Safe wrapper around [`Extension::fetch_chapter`].
    pub async fn fetch_chapter(
        mut self,
        url: &str,
    ) -> error::Result<(Self, Result<novel::ChapterContent, wit_error::Error>)> {
        wrap_extension_method!(self, call_fetch_chapter, url)
    }

    /// Safe wrapper around [`Extension::simple_search`].
    pub async fn simple_search(
        mut self,
        query: &SimpleSearchQuery,
    ) -> error::Result<(Self, Result<SearchResult, wit_error::Error>)> {
        wrap_extension_method!(self, call_simple_search, query)
    }

    /// Safe wrapper around [`Extension::complex_search`].
    pub async fn complex_search(
        mut self,
        query: &ComplexSearchQuery,
    ) -> error::Result<(Self, Result<SearchResult, wit_error::Error>)> {
        wrap_extension_method!(self, call_complex_search, query)
    }
}
