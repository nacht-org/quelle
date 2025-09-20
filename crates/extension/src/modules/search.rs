use crate::novel::SimpleSearchQuery;

impl SimpleSearchQuery {
    pub fn page(&self) -> u32 {
        self.page.unwrap_or(1)
    }
}
