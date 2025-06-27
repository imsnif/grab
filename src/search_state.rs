use crate::search::SearchResult;

#[derive(Default)]
pub struct SearchState {
    pub search_term: String,
    pub search_results: Vec<SearchResult>,
}

impl SearchState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_search_term(&mut self, term: String) {
        self.search_term = term;
    }

    pub fn add_char(&mut self, ch: char) {
        self.search_term.push(ch);
    }

    pub fn remove_char(&mut self) {
        self.search_term.pop();
    }

    pub fn clear(&mut self) {
        self.search_term.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.search_term.is_empty()
    }

    pub fn update_results(&mut self, results: Vec<SearchResult>) {
        self.search_results = results;
    }

    pub fn get_results(&self) -> &[SearchResult] {
        &self.search_results
    }

    pub fn results_count(&self) -> usize {
        self.search_results.len()
    }

    pub fn get_term(&self) -> &str {
        &self.search_term
    }

    pub fn has_results(&self) -> bool {
        !self.search_results.is_empty()
    }
}
