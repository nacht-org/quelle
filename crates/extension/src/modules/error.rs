pub use crate::wit::quelle::extension::error as wit_error;

impl From<eyre::Report> for wit_error::Error {
    fn from(err: eyre::Report) -> Self {
        wit_error::Error {
            message: err.to_string(),
            location: None,
        }
    }
}

pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        let location = panic_info
            .location()
            .map_or("unknown location".to_string(), |loc| {
                format!("{} {}:{}", loc.file(), loc.line(), loc.column())
            });

        let error = wit_error::Error {
            message,
            location: Some(location),
        };

        wit_error::report_panic(&error);
    }));
}
