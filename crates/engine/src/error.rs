use crate::bindings::quelle::extension::error::Error as ExtensionError;

pub type Result<T> = std::result::Result<T, Error>;

/// This defines the error types used in the engine for handling various errors
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// This wraps the [`wasmtime::Error`] type, which is returned by the Wasmtime runtime.
    ///
    /// This error is returned when there is an issue with the Wasmtime runtime, such as
    /// a failure to instantiate a component, call a function, or any other runtime-related issue.
    #[error(transparent)]
    WasmtimeError(#[from] wasmtime::Error),

    /// Extension returned an error.
    ///
    /// This error is returned when extension methods return an error.
    /// It wraps the [`ExtensionError`] that was returned by the extension.
    #[error(transparent)]
    ExtensionError(#[from] ExtensionError),

    /// An error that occurs during the call to extensions methods.
    ///
    /// This error is returned when an extension method fails to execute properly.
    /// It wraps the `wasmtime::Error` that occurred during the call, and optionally includes a panic error
    /// if the extension encountered a panic during execution.
    #[error("Runtime error: {wasmtime_error}")]
    RuntimeError {
        #[source]
        wasmtime_error: wasmtime::Error,
        panic_error: Option<ExtensionError>,
    },
}
