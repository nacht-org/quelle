//! WebAssembly extension engine for Quelle.
//!
//! This crate provides the runtime environment for executing WebAssembly extensions
//! that handle novel fetching, searching, and content extraction from various sources.

pub mod bindings;
pub mod error;
pub mod executor;
pub mod http;
pub mod registry;
pub mod scraper;
mod state;

use std::sync::Arc;

use registry::ExtensionSession;

use wasmtime::component::*;
use wasmtime::{Config, Engine, Store};

use crate::bindings::quelle::extension::{error as wit_error, source};
use crate::bindings::{ComplexSearchQuery, Extension, SimpleSearchQuery};
use crate::http::HttpExecutor;
use crate::state::State;

pub use executor::{Executor, create_engine};

pub struct ExtensionEngine {
    engine: Engine,
    linker: Linker<State>,
    executor: Arc<dyn HttpExecutor>,
}

impl ExtensionEngine {
    pub fn new(executor: Arc<dyn HttpExecutor>) -> error::Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);

        let engine = Engine::new(&config)?;
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
        crate::bindings::quelle::extension::scraper::add_to_linker::<_, HasSelf<_>>(
            &mut linker,
            |state| &mut state.scraper,
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

    pub async fn new_runner_from_file(
        self: &Arc<Self>,
        path: &str,
    ) -> error::Result<ExtensionRunner> {
        let component = Component::from_file(&self.engine, path)?;
        self.new_runner(component).await
    }

    pub async fn new_runner_from_bytes(
        self: &Arc<Self>,
        bytes: &[u8],
    ) -> error::Result<ExtensionRunner> {
        let component = Component::from_binary(&self.engine, bytes)?;
        self.new_runner(component).await
    }

    pub async fn new_runner(
        self: &Arc<Self>,
        component: Component,
    ) -> error::Result<ExtensionRunner> {
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

        Ok(ExtensionRunner::new(Arc::clone(self), extension, store))
    }

    pub fn new_session(self: &Arc<Self>, bytes: Vec<u8>) -> ExtensionSession {
        ExtensionSession::new(Arc::clone(self), bytes)
    }
}

pub struct ExtensionRunner {
    /// Arc to the extension engine, keeping it alive as long as the runner exists.
    _engine: Arc<ExtensionEngine>,
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

impl ExtensionRunner {
    pub fn new(engine: Arc<ExtensionEngine>, extension: Extension, store: Store<State>) -> Self {
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
    ) -> error::Result<(Self, Result<quelle_types::Novel, wit_error::Error>)> {
        let (runner, result) = wrap_extension_method!(self, call_fetch_novel_info, url)?;
        Ok((runner, result.map(Into::into)))
    }

    /// Safe wrapper around [`Extension::fetch_chapter`].
    pub async fn fetch_chapter(
        mut self,
        url: &str,
    ) -> error::Result<(Self, Result<quelle_types::ChapterContent, wit_error::Error>)> {
        let (runner, result) = wrap_extension_method!(self, call_fetch_chapter, url)?;
        Ok((runner, result.map(Into::into)))
    }

    /// Safe wrapper around [`Extension::simple_search`].
    pub async fn simple_search(
        mut self,
        query: &SimpleSearchQuery,
    ) -> error::Result<(Self, Result<quelle_types::SearchResult, wit_error::Error>)> {
        let (runner, result) = wrap_extension_method!(self, call_simple_search, query)?;
        Ok((runner, result.map(Into::into)))
    }

    /// Safe wrapper around [`Extension::complex_search`].
    pub async fn complex_search(
        mut self,
        query: &ComplexSearchQuery,
    ) -> error::Result<(Self, Result<quelle_types::SearchResult, wit_error::Error>)> {
        let (runner, result) = wrap_extension_method!(self, call_complex_search, query)?;
        Ok((runner, result.map(Into::into)))
    }
}

// ---------------------------------------------------------------------------
// From trait implementations: WIT → quelle_types
// ---------------------------------------------------------------------------

impl From<bindings::quelle::extension::novel::NovelStatus> for quelle_types::NovelStatus {
    fn from(w: bindings::quelle::extension::novel::NovelStatus) -> Self {
        use bindings::quelle::extension::novel::NovelStatus as WitStatus;
        match w {
            WitStatus::Ongoing => quelle_types::NovelStatus::Ongoing,
            WitStatus::Hiatus => quelle_types::NovelStatus::Hiatus,
            WitStatus::Completed => quelle_types::NovelStatus::Completed,
            WitStatus::Stub => quelle_types::NovelStatus::Stub,
            WitStatus::Dropped => quelle_types::NovelStatus::Dropped,
            WitStatus::Unknown => quelle_types::NovelStatus::Unknown,
        }
    }
}

impl From<bindings::quelle::extension::novel::Namespace> for quelle_types::Namespace {
    fn from(w: bindings::quelle::extension::novel::Namespace) -> Self {
        use bindings::quelle::extension::novel::Namespace as WitNs;
        match w {
            WitNs::Dc => quelle_types::Namespace::Dc,
            WitNs::Opf => quelle_types::Namespace::Opf,
        }
    }
}

impl From<bindings::quelle::extension::novel::Chapter> for quelle_types::Chapter {
    fn from(w: bindings::quelle::extension::novel::Chapter) -> Self {
        quelle_types::Chapter {
            title: w.title,
            index: w.index,
            url: w.url,
            updated_at: w.updated_at,
        }
    }
}

impl From<bindings::quelle::extension::novel::Metadata> for quelle_types::Metadata {
    fn from(w: bindings::quelle::extension::novel::Metadata) -> Self {
        quelle_types::Metadata {
            name: w.name,
            value: w.value,
            ns: w.ns.into(),
            others: w.others,
        }
    }
}

impl From<bindings::quelle::extension::novel::Volume> for quelle_types::Volume {
    fn from(w: bindings::quelle::extension::novel::Volume) -> Self {
        quelle_types::Volume {
            name: w.name,
            index: w.index,
            chapters: w.chapters.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<bindings::quelle::extension::novel::Novel> for quelle_types::Novel {
    fn from(w: bindings::quelle::extension::novel::Novel) -> Self {
        quelle_types::Novel {
            url: w.url,
            authors: w.authors,
            title: w.title,
            cover: w.cover,
            description: w.description,
            volumes: w.volumes.into_iter().map(Into::into).collect(),
            metadata: w.metadata.into_iter().map(Into::into).collect(),
            status: w.status.into(),
            langs: w.langs,
        }
    }
}

impl From<bindings::quelle::extension::novel::ChapterContent> for quelle_types::ChapterContent {
    fn from(w: bindings::quelle::extension::novel::ChapterContent) -> Self {
        quelle_types::ChapterContent { data: w.data }
    }
}

impl From<bindings::quelle::extension::novel::BasicNovel> for quelle_types::BasicNovel {
    fn from(w: bindings::quelle::extension::novel::BasicNovel) -> Self {
        quelle_types::BasicNovel {
            title: w.title,
            cover: w.cover,
            url: w.url,
        }
    }
}

impl From<bindings::quelle::extension::novel::SearchResult> for quelle_types::SearchResult {
    fn from(w: bindings::quelle::extension::novel::SearchResult) -> Self {
        quelle_types::SearchResult {
            novels: w.novels.into_iter().map(Into::into).collect(),
            total_count: w.total_count,
            current_page: w.current_page,
            total_pages: w.total_pages,
            has_next_page: w.has_next_page,
            has_previous_page: w.has_previous_page,
        }
    }
}
