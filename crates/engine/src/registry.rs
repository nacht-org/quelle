use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{ExtensionEngine, ExtensionRunner};

/// A session for running extension calls, which may reuse the same runner across multiple calls
/// to avoid reinitialization overhead. The session holds the extension bytes and creates a runner
/// on demand, caching it for subsequent calls until an error occurs.
pub struct ExtensionSession {
    engine: Arc<ExtensionEngine>,
    bytes: Vec<u8>,
    runners: Mutex<Option<ExtensionRunner>>,
}

impl ExtensionSession {
    pub fn new(engine: Arc<ExtensionEngine>, bytes: Vec<u8>) -> Self {
        Self {
            engine,
            bytes,
            runners: Mutex::new(None),
        }
    }

    /// Call a function on the extension runner, reusing the same runner if possible. If the runner
    /// encounters an error, it will be dropped and a new runner will be created on the next call.
    pub async fn call<F, T>(&self, func: F) -> eyre::Result<T>
    where
        F: AsyncFnOnce(ExtensionRunner) -> eyre::Result<(ExtensionRunner, T)> + Send,
    {
        let mut guard = self.runners.lock().await;

        let runner = match guard.take() {
            Some(r) => r,
            None => self
                .engine
                .new_runner_from_bytes(&self.bytes)
                .await
                .map_err(|e| eyre::eyre!("{}", e))?,
        };

        match func(runner).await {
            Ok((runner, result)) => {
                *guard = Some(runner);
                Ok(result)
            }
            Err(e) => Err(e),
        }
    }
}
