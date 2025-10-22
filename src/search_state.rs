use crate::search::{SearchResult, SearchResults, SearchItem};
use crate::{RustAssetSearchMode, parse_rust_asset_search};
use crate::files::TypeKind;

#[derive(Default)]
pub struct SearchState {
    pub search_term: String,
    pub files_panes_results: Vec<SearchResult>,
}

impl SearchState {
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

    pub fn update_results(&mut self, results: SearchResults) {
        self.files_panes_results = results.files_panes_results;
    }

    pub fn get_files_panes_results(&self) -> &[SearchResult] {
        &self.files_panes_results
    }

    // Count only panes and files (excluding Rust assets) for display purposes
    pub fn display_count(&self) -> usize {
        self.files_panes_results
            .iter()
            .filter(|result| matches!(result.item, SearchItem::Pane(_) | SearchItem::File(_)))
            .count()
    }

    // Get filtered results for selection purposes (only panes and files)
    pub fn get_display_results(&self) -> Vec<SearchResult> {
        self.files_panes_results
            .iter()
            .filter(|result| matches!(result.item, SearchItem::Pane(_) | SearchItem::File(_)))
            .cloned()
            .collect()
    }

    pub fn get_term(&self) -> &str {
        &self.search_term
    }

    // Check if current search term is a Rust asset search
    pub fn is_rust_asset_search(&self) -> bool {
        parse_rust_asset_search(&self.search_term).is_some()
    }

    // Get Rust asset search mode if applicable
    pub fn get_rust_asset_search_mode(&self) -> Option<RustAssetSearchMode> {
        parse_rust_asset_search(&self.search_term)
    }

    // Get filtered results for Rust asset search (only matching Rust assets)
    pub fn get_rust_asset_display_results(&self) -> Vec<SearchResult> {
        if let Some(mode) = self.get_rust_asset_search_mode() {
            self.files_panes_results
                .iter()
                .filter(|result| {
                    if let SearchItem::RustAsset(rust_asset) = &result.item {
                        match &mode {
                            RustAssetSearchMode::Struct(_) => matches!(rust_asset.type_kind, TypeKind::Struct),
                            RustAssetSearchMode::Enum(_) => matches!(rust_asset.type_kind, TypeKind::Enum),
                            RustAssetSearchMode::Function(_) => matches!(rust_asset.type_kind, TypeKind::Function),
                        }
                    } else {
                        false
                    }
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    // Count only Rust assets matching the search mode
    pub fn rust_asset_display_count(&self) -> usize {
        self.get_rust_asset_display_results().len()
    }

    // Get the actual display count based on search mode
    pub fn get_current_display_count(&self) -> usize {
        if self.is_rust_asset_search() {
            self.rust_asset_display_count()
        } else {
            self.display_count()
        }
    }

    // Get the appropriate display results based on search mode
    pub fn get_current_display_results(&self) -> Vec<SearchResult> {
        if self.is_rust_asset_search() {
            self.get_rust_asset_display_results()
        } else {
            self.get_display_results()
        }
    }
}
