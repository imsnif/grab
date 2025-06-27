use std::path::PathBuf;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use crate::pane::PaneMetadata;
use crate::files::TypeDefinition;

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

    pub fn search_panes_and_files(
        &self,
        search_term: &str,
        panes: &[PaneMetadata],
        files: &[PathBuf],
        rust_assets: &[TypeDefinition],
    ) -> Vec<SearchResult> {
        if search_term.is_empty() {
            return vec![];
        }

        let mut matches = vec![];

        // Search panes
        for pane in panes {
            if let Some((score, indices)) = self.matcher.fuzzy_indices(&pane.title, search_term) {
                matches.push(SearchResult::new_pane(pane.clone(), score, indices));
            }
        }

        // Search rust assets
        for rust_asset in rust_assets {
            if let Some((score, indices)) = self.matcher.fuzzy_indices(&rust_asset.name, search_term) {
                matches.push(SearchResult::new_rust_asset(rust_asset.clone(), score, indices));
            }
        }

        // Search files (limited to top 3)
        let mut file_matches = vec![];
        for file in files {
            let file_string = file.to_string_lossy();

            if let Some((score, indices)) = self.matcher.fuzzy_indices(&file_string, search_term) {
                file_matches.push(SearchResult::new_file(file.clone(), score, indices));
            }
        }

        file_matches.sort_by(|a, b| b.score.cmp(&a.score));
        file_matches.truncate(3);

        matches.extend(file_matches);
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
