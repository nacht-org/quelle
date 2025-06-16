use crate::QuelleExtension;

static mut EXTENSION: Option<Box<dyn QuelleExtension>> = None;

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
    unsafe { EXTENSION = Some((build_extension)()) }
}

pub fn register_tracing() {
    use crate::modules::tracing::HostLayer;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    tracing_subscriber::registry().with(HostLayer).init();
}

pub fn extension() -> &'static mut dyn QuelleExtension {
    #[expect(static_mut_refs)]
    unsafe {
        EXTENSION.as_deref_mut().unwrap()
    }
}
