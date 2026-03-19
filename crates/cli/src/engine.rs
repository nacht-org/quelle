//! Extension engine construction helpers.
//!
//! Re-exports [`Executor`] and [`create_engine`] from `quelle_engine` for use
//! within this crate, plus a convenience no-argument wrapper for call sites
//! that do not need to select an executor explicitly.

pub use quelle_engine::{Executor, create_engine as create_extension_engine_with_executor};

/// Create an [`quelle_engine::ExtensionEngine`] using the default executor.
pub fn create_extension_engine() -> eyre::Result<quelle_engine::ExtensionEngine> {
    create_extension_engine_with_executor(Executor::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let result = create_extension_engine();
        assert!(result.is_ok(), "Engine creation should succeed");
    }
}
