use crate::novel::NovelStatus;

impl NovelStatus {
    pub fn from_str(status: &str) -> Self {
        match status.to_ascii_lowercase().as_str() {
            "ongoing" => NovelStatus::Ongoing,
            "completed" => NovelStatus::Completed,
            "hiatus" => NovelStatus::Hiatus,
            "dropped" => NovelStatus::Dropped,
            "stub" => NovelStatus::Stub,
            _ => NovelStatus::Unknown,
        }
    }
}
