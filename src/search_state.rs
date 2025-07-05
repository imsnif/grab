use crate::search::{SearchResult, DualSearchResults};

#[derive(Default)]
pub struct SearchState {
    pub search_term: String,
    pub files_panes_results: Vec<SearchResult>,
    pub shell_commands_results: Vec<SearchResult>,
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

    pub fn update_results(&mut self, results: DualSearchResults) {
        self.files_panes_results = results.files_panes_results;
        self.shell_commands_results = results.shell_commands_results;
    }

    pub fn get_files_panes_results(&self) -> &[SearchResult] {
        &self.files_panes_results
    }

    pub fn get_shell_commands_results(&self) -> &[SearchResult] {
        &self.shell_commands_results
    }

    pub fn files_panes_count(&self) -> usize {
        self.files_panes_results.len()
    }

    pub fn shell_commands_count(&self) -> usize {
        self.shell_commands_results.len()
    }

    pub fn get_term(&self) -> &str {
        &self.search_term
    }

    pub fn has_files_panes_results(&self) -> bool {
        !self.files_panes_results.is_empty()
    }

    pub fn has_shell_commands_results(&self) -> bool {
        !self.shell_commands_results.is_empty()
    }

    pub fn has_any_results(&self) -> bool {
        self.has_files_panes_results() || self.has_shell_commands_results()
    }
}