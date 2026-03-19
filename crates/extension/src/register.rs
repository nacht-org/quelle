use std::sync::OnceLock;

use crate::QuelleExtension;

static EXTENSION: OnceLock<Box<dyn QuelleExtension>> = OnceLock::new();

/// Registers the provided type as a Quelle extension.
///
/// The type must implement the [`Extension`] trait.
#[macro_export]
macro_rules! register_extension {
    ($extension_type:ty) => {
        #[unsafe(export_name = "register-extension")]
        pub extern "C" fn __register_extension() {
            $crate::install_panic_hook();
            $crate::register_tracing();
            $crate::register_extension_internal(|| {
                Box::new(<$extension_type as $crate::QuelleExtension>::new())
            });
        }
    };
}

#[doc(hidden)]
pub fn register_extension_internal(build_extension: fn() -> Box<dyn QuelleExtension>) {
    let _ = EXTENSION.set((build_extension)());
}

pub fn register_tracing() {
    use crate::modules::tracing::HostLayer;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    tracing_subscriber::registry().with(HostLayer).init();
}

pub fn extension() -> &'static dyn QuelleExtension {
    EXTENSION.get().unwrap().as_ref()
}
