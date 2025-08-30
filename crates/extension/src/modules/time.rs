use chrono::{DateTime, Local, TimeZone, offset::LocalResult};

use crate::wit::quelle::extension::time as wit_time;

pub fn local_time() -> Result<DateTime<Local>, eyre::Report> {
    let now_millis = wit_time::local_now_millis();
    let dt = Local.timestamp_millis_opt(now_millis);
    match dt {
        LocalResult::Single(dt) => Ok(dt),
        LocalResult::Ambiguous(earliest, latest) => {
            Err(eyre::eyre!("Ambiguous time: {} - {}", earliest, latest))
        }
        LocalResult::None => Err(eyre::eyre!("Failed to convert milliseconds to DateTime")),
    }
}
