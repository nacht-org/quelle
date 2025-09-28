mod chrome;
mod executor;
mod reqwest;

use std::fmt;
use std::sync::Arc;
use wasmtime::component::{HasData, ResourceTable};

use crate::bindings::quelle::extension::http;
use crate::state::State;

pub use self::chrome::HeadlessChromeExecutor;
pub use self::executor::HttpExecutor;
pub use self::reqwest::ReqwestExecutor;

impl fmt::Display for http::Method {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            http::Method::Get => write!(f, "GET"),
            http::Method::Post => write!(f, "POST"),
            http::Method::Put => write!(f, "PUT"),
            http::Method::Delete => write!(f, "DELETE"),
            http::Method::Patch => write!(f, "PATCH"),
            http::Method::Head => write!(f, "HEAD"),
            http::Method::Options => write!(f, "OPTIONS"),
        }
    }
}

pub struct Http {
    table: ResourceTable,
    executor: Arc<dyn HttpExecutor>,
}

impl HasData for Http {
    type Data<'a> = &'a mut State;
}

impl Http {
    pub fn new(executor: Arc<dyn HttpExecutor>) -> Self {
        Self {
            table: ResourceTable::new(),
            executor,
        }
    }
}

impl http::Host for Http {}

impl http::HostClient for Http {
    async fn new(&mut self) -> wasmtime::component::Resource<http::Client> {
        self.table
            .push(http::Client::new(self.executor.clone()))
            .unwrap()
    }

    async fn request(
        &mut self,
        self_: wasmtime::component::Resource<http::Client>,
        request: http::Request,
    ) -> Result<http::Response, http::ResponseError> {
        tracing::info!(
            "Executing HTTP request: method={:?}, url={}",
            request.method,
            request.url
        );

        let client = self.table.get_mut(&self_).unwrap();
        client.request(request).await
    }

    async fn drop(
        &mut self,
        rep: wasmtime::component::Resource<http::Client>,
    ) -> wasmtime::Result<()> {
        let _ = self.table.delete(rep)?;
        Ok(())
    }
}

pub struct HostClient {
    executor: Arc<dyn HttpExecutor>,
}

impl HostClient {
    pub fn new(executor: Arc<dyn HttpExecutor>) -> Self {
        Self { executor }
    }

    pub async fn request(
        &self,
        request: http::Request,
    ) -> Result<http::Response, http::ResponseError> {
        self.executor.execute(request).await
    }
}
