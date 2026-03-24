use tokio::sync::Mutex;

use crate::{ExtensionEngine, ExtensionRunner};

/// A session for running extension calls, which may reuse the same runner across multiple calls
/// to avoid reinitialization overhead. The session holds the extension bytes and creates a runner
/// on demand, caching it for subsequent calls until an error occurs.
pub struct ExtensionSession<'a> {
    engine: &'a ExtensionEngine,
    bytes: Vec<u8>,
    runners: Mutex<Option<ExtensionRunner<'a>>>,
}

impl<'a> ExtensionSession<'a> {
    pub fn new(engine: &'a ExtensionEngine, bytes: Vec<u8>) -> Self {
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
        F: AsyncFnOnce(ExtensionRunner<'a>) -> eyre::Result<(ExtensionRunner<'a>, T)> + Send,
    {
        let mut guard = self.runners.lock().await;

        let runner = match guard.take() {
            Some(r) => r,
            None => self.engine.new_runner_from_bytes(&self.bytes).await?,
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
