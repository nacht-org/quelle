pub use crate::wit::quelle::extension::error as wit_error;

impl From<eyre::Report> for wit_error::Error {
    fn from(err: eyre::Report) -> Self {
        // Build a frame for every layer in the eyre chain.
        // Index 0 is the outermost context (e.g. "failed to fetch chapter");
        // the last entry is the root cause.
        let frames = err
            .chain()
            .map(|e| wit_error::ErrorFrame {
                message: e.to_string(),
                location: None,
            })
            .collect();
        wit_error::Error { frames }
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

        // A panic is always a single-frame error, but with the source location set.
        let error = wit_error::Error {
            frames: vec![wit_error::ErrorFrame {
                message,
                location: Some(location),
            }],
        };

        wit_error::report_panic(&error);
    }));
}
