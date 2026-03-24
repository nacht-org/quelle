use crate::bindings::quelle::extension::error::Error as ExtensionError;

pub type Result<T> = std::result::Result<T, Error>;

// ---------------------------------------------------------------------------
// chain_display — inherent helper on the generated ExtensionError type
// ---------------------------------------------------------------------------

impl ExtensionError {
    /// Formats the full error chain as a single human-readable string.
    ///
    /// Frames are joined with `": "` (outermost context first, root cause last).
    /// Panic frames that carry a source location append `" [at <loc>]"` after
    /// their message so the location is visible inline.
    pub fn chain_display(&self) -> String {
        self.frames
            .iter()
            .map(|f| match &f.location {
                Some(loc) => format!("{} [at {}]", f.message, loc),
                None => f.message.clone(),
            })
            .collect::<Vec<_>>()
            .join(": ")
    }

    pub fn into_report(self) -> eyre::Report {
        eyre::eyre!(self.chain_display())
    }
}

// ---------------------------------------------------------------------------
// Engine-level Error enum
// ---------------------------------------------------------------------------

/// This defines the error types used in the engine for handling various errors
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// This wraps the [`wasmtime::Error`] type, which is returned by the Wasmtime runtime.
    ///
    /// This error is returned when there is an issue with the Wasmtime runtime, such as
    /// a failure to instantiate a component, call a function, or any other runtime-related issue.
    #[error(transparent)]
    WasmtimeError(#[from] wasmtime::Error),

    /// Extension returned an application-level error.
    ///
    /// The full error chain is rendered by [`ExtensionError::chain_display`] so callers
    /// see every `.wrap_err()` context layer, not just the outermost message.
    #[error("{}", .0.chain_display())]
    ExtensionError(#[from] ExtensionError),

    /// An error that occurs during the call to an extension method (e.g. a WASM trap).
    ///
    /// `panic_error` carries the last panic reported by the extension via
    /// `report-panic`, if any, and can be used by callers to surface a more
    /// informative message than the raw wasmtime trap.
    #[error("Runtime error: {wasmtime_error}")]
    RuntimeError {
        #[source]
        wasmtime_error: wasmtime::Error,
        panic_error: Option<ExtensionError>,
    },
}
