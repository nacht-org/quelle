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
            quelle_extension::register_extension_internal(|| {
                Box::new(<$extension_type as $crate::QuelleExtension>::new())
            });
        }
    };
}

#[doc(hidden)]
pub fn register_extension_internal(build_extension: fn() -> Box<dyn QuelleExtension>) {
    unsafe { EXTENSION = Some((build_extension)()) }
}

pub fn extension() -> &'static mut dyn QuelleExtension {
    #[expect(static_mut_refs)]
    unsafe {
        EXTENSION.as_deref_mut().unwrap()
    }
}
