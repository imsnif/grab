use std::path::PathBuf;
use std::collections::BTreeMap;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use crate::pane::PaneMetadata;
use crate::files::TypeDefinition;
use crate::read_shell_histories::DeduplicatedCommand;
use crate::{RustAssetSearchMode, parse_rust_asset_search};

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub item: SearchItem,
    pub score: i64,
    pub indices: Vec<usize>,
}

#[derive(Debug, Clone)]
pub enum SearchItem {
    Pane(PaneMetadata),
    File(PathBuf),
    RustAsset(TypeDefinition),
}

#[derive(Debug, Clone, Default)]
pub struct SearchResults {
    pub files_panes_results: Vec<SearchResult>,
}

impl SearchResult {
    pub fn new_pane(pane: PaneMetadata, score: i64, indices: Vec<usize>) -> Self {
        SearchResult {
            item: SearchItem::Pane(pane),
            score,
            indices,
        }
    }

    pub fn new_file(file: PathBuf, score: i64, indices: Vec<usize>) -> Self {
        SearchResult {
            item: SearchItem::File(file),
            score,
            indices,
        }
    }

    pub fn new_rust_asset(rust_asset: TypeDefinition, score: i64, indices: Vec<usize>) -> Self {
        SearchResult {
            item: SearchItem::RustAsset(rust_asset),
            score,
            indices,
        }
    }

    pub fn display_text(&self) -> String {
        match &self.item {
            SearchItem::Pane(pane) => pane.title.clone(),
            SearchItem::File(path) => path.to_string_lossy().to_string(),
            SearchItem::RustAsset(rust_asset) => {
                format!("{} ({})", rust_asset.name, rust_asset.file_path.to_string_lossy())
            }
        }
    }

    pub fn is_pane(&self) -> bool {
        matches!(self.item, SearchItem::Pane(_))
    }

    pub fn is_file(&self) -> bool {
        matches!(self.item, SearchItem::File(_))
    }

    pub fn is_rust_asset(&self) -> bool {
        matches!(self.item, SearchItem::RustAsset(_))
    }

}

pub struct SearchEngine {
    matcher: SkimMatcherV2,
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            matcher: SkimMatcherV2::default().use_cache(true),
        }
    }

    fn is_contiguous_match(indices: &[usize]) -> bool {
        if indices.len() <= 1 {
            return true;
        }

        for i in 1..indices.len() {
            if indices[i] != indices[i.saturating_sub(1)] + 1 {
                return false;
            }
        }
        true
    }

    pub fn search(
        &self,
        search_term: &str,
        panes: &[PaneMetadata],
        files: &[PathBuf],
        rust_assets: &[TypeDefinition],
        _shell_histories: &BTreeMap<String, Vec<DeduplicatedCommand>>,
        _current_cwd: &PathBuf,
    ) -> SearchResults {
        let mut results = SearchResults::default();

        if search_term.is_empty() {
            // Return all items when no search term
            results.files_panes_results = self.get_all_files_panes_rust(panes, files, rust_assets);
            return results;
        }

        // Check if this is a Rust asset search (struct/enum/function)
        if let Some(rust_mode) = parse_rust_asset_search(search_term) {
            // For Rust asset searches, only search rust assets with the term after the keyword
            let actual_search_term = match &rust_mode {
                RustAssetSearchMode::Struct(term) => term,
                RustAssetSearchMode::Enum(term) => term,
                RustAssetSearchMode::Function(term) => term,
                RustAssetSearchMode::PubFunction(term) => term,
            };
            results.files_panes_results = self.search_rust_assets_only(actual_search_term, rust_assets, &rust_mode);
        } else {
            // Normal search: files, panes, and rust assets
            results.files_panes_results = self.search_files_panes_rust(search_term, panes, files, rust_assets);
        }

        results
    }

    fn get_all_files_panes_rust(
        &self,
        panes: &[PaneMetadata],
        files: &[PathBuf],
        rust_assets: &[TypeDefinition],
    ) -> Vec<SearchResult> {
        let mut results = Vec::new();

        // Add all panes
        for pane in panes {
            results.push(SearchResult::new_pane(pane.clone(), 1000, vec![]));
        }

        // Add all rust assets
        for rust_asset in rust_assets {
            results.push(SearchResult::new_rust_asset(rust_asset.clone(), 500, vec![]));
        }

        // Add all files
        for file in files {
            results.push(SearchResult::new_file(file.clone(), 100, vec![]));
        }

        results
    }


    fn search_files_panes_rust(
        &self,
        search_term: &str,
        panes: &[PaneMetadata],
        files: &[PathBuf],
        rust_assets: &[TypeDefinition],
    ) -> Vec<SearchResult> {
        let mut matches = vec![];

        // Search panes with contiguous match scoring
        for pane in panes {
            if let Some((score, indices)) = self.matcher.fuzzy_indices(&pane.title, search_term) {
                let boosted_score = if Self::is_contiguous_match(&indices) {
                    score.saturating_mul(10)
                } else {
                    score
                };
                
                matches.push(SearchResult::new_pane(pane.clone(), boosted_score, indices));
            }
        }

        // Search rust assets
        for rust_asset in rust_assets {
            if let Some((score, indices)) = self.matcher.fuzzy_indices(&rust_asset.name, search_term) {
                matches.push(SearchResult::new_rust_asset(rust_asset.clone(), score, indices));
            }
        }

        // Search all files
        for file in files {
            let file_string = file.to_string_lossy();

            if let Some((score, indices)) = self.matcher.fuzzy_indices(&file_string, search_term) {
                matches.push(SearchResult::new_file(file.clone(), score, indices));
            }
        }

        matches.sort_by(|a, b| b.score.cmp(&a.score));

        matches
    }

    fn search_rust_assets_only(
        &self,
        search_term: &str,
        rust_assets: &[TypeDefinition],
        mode: &RustAssetSearchMode,
    ) -> Vec<SearchResult> {
        let mut matches = vec![];

        for rust_asset in rust_assets {
            // Filter by type first
            let type_matches = match mode {
                RustAssetSearchMode::Struct(_) => matches!(rust_asset.type_kind, crate::files::TypeKind::Struct),
                RustAssetSearchMode::Enum(_) => matches!(rust_asset.type_kind, crate::files::TypeKind::Enum),
                RustAssetSearchMode::Function(_) => matches!(rust_asset.type_kind, crate::files::TypeKind::Function | crate::files::TypeKind::PubFunction),
                RustAssetSearchMode::PubFunction(_) => matches!(rust_asset.type_kind, crate::files::TypeKind::PubFunction),
            };

            if type_matches {
                if search_term.is_empty() {
                    // If no search term after the keyword, show all of that type
                    matches.push(SearchResult::new_rust_asset(rust_asset.clone(), 1000, vec![]));
                } else if let Some((score, indices)) = self.matcher.fuzzy_indices(&rust_asset.name, search_term) {
                    // Fuzzy match against the rust asset name
                    matches.push(SearchResult::new_rust_asset(rust_asset.clone(), score, indices));
                }
            }
        }

        matches.sort_by(|a, b| b.score.cmp(&a.score));
        matches
    }


    pub fn get_displayed_files(&self, search_term: &str, files: &[PathBuf]) -> (Vec<PathBuf>, usize) {
        if search_term.is_empty() {
            return (vec![], 0);
        }

        let mut file_matches = vec![];

        for file in files {
            let file_string = file.to_string_lossy();

            if let Some((score, _)) = self.matcher.fuzzy_indices(&file_string, search_term) {
                file_matches.push((file.clone(), score));
            }
        }

        file_matches.sort_by(|a, b| b.1.cmp(&a.1));

        let displayed_count = file_matches.len().min(3);
        let displayed_files: Vec<PathBuf> = file_matches
            .iter()
            .take(displayed_count)
            .map(|(file, _)| file.clone())
            .collect();

        let remaining_count = file_matches.len().saturating_sub(3);

        (displayed_files, remaining_count)
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}
