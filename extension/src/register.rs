use crate::Extension;

static mut EXTENSION: Option<Box<dyn Extension>> = None;

/// Registers the provided type as a Quelle extension.
///
/// The type must implement the [`Extension`] trait.
#[macro_export]
macro_rules! register_extension {
    ($extension_type:ty) => {
        #[unsafe(export_name = "register-extension")]
        pub extern "C" fn __register_extension() {
            quelle_extension::register_extension_internal(|| {
                Box::new(<$extension_type as quelle_extension::Extension>::new())
            });
        }
    };
}

#[doc(hidden)]
pub fn register_extension_internal(build_extension: fn() -> Box<dyn Extension>) {
    unsafe { EXTENSION = Some((build_extension)()) }
}

pub fn extension() -> &'static mut dyn Extension {
    #[expect(static_mut_refs)]
    unsafe {
        EXTENSION.as_deref_mut().unwrap()
    }
}
