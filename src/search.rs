use std::path::PathBuf;
use std::collections::BTreeMap;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use crate::pane::PaneMetadata;
use crate::files::TypeDefinition;
use crate::read_shell_histories::DeduplicatedCommand;

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
    ShellCommand { shell: String, command: String, folders: Vec<String> },
}

#[derive(Debug, Clone, Default)]
pub struct DualSearchResults {
    pub files_panes_results: Vec<SearchResult>,
    pub shell_commands_results: Vec<SearchResult>,
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

    pub fn new_shell_command(shell: String, command: String, folders: Vec<String>, score: i64, indices: Vec<usize>) -> Self {
        SearchResult {
            item: SearchItem::ShellCommand { shell, command, folders },
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
            SearchItem::ShellCommand { command, .. } => command.clone(),
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

    pub fn is_shell_command(&self) -> bool {
        matches!(self.item, SearchItem::ShellCommand { .. })
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

    pub fn search_dual(
        &self,
        search_term: &str,
        panes: &[PaneMetadata],
        files: &[PathBuf],
        rust_assets: &[TypeDefinition],
        shell_histories: &BTreeMap<String, Vec<DeduplicatedCommand>>,
        current_cwd: &PathBuf,
    ) -> DualSearchResults {
        let mut results = DualSearchResults::default();

        if search_term.is_empty() {
            // Return all items when no search term
            results.files_panes_results = self.get_all_files_panes_rust(panes, files, rust_assets);
            results.shell_commands_results = self.get_all_shell_commands(shell_histories, current_cwd);
            return results;
        }

        // Search files, panes, and rust assets
        results.files_panes_results = self.search_files_panes_rust(search_term, panes, files, rust_assets);
        
        // Search shell commands
        results.shell_commands_results = self.search_shell_commands_prioritized(
            search_term,
            shell_histories,
            current_cwd,
        );

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

        // Add limited files (top 50)
        for (i, file) in files.iter().enumerate() {
            if i >= 50 {
                break;
            }
            results.push(SearchResult::new_file(file.clone(), 100, vec![]));
        }

        results
    }

    fn get_all_shell_commands(
        &self,
        shell_histories: &BTreeMap<String, Vec<DeduplicatedCommand>>,
        current_cwd: &PathBuf,
    ) -> Vec<SearchResult> {
        let mut results = Vec::new();
        let current_cwd_str = current_cwd.to_string_lossy().to_string();

        for (shell_name, deduplicated_commands) in shell_histories {
            for cmd in deduplicated_commands {
                let score = if cmd.folders.contains(&current_cwd_str) { 1000 } else { 500 };
                results.push(SearchResult::new_shell_command(
                    shell_name.clone(),
                    cmd.command.clone(),
                    cmd.folders.clone(),
                    score,
                    vec![],
                ));
            }
        }

        // Sort by score and recency
        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(100);
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

    fn search_shell_commands_prioritized(
        &self,
        search_term: &str,
        shell_histories: &BTreeMap<String, Vec<DeduplicatedCommand>>,
        current_cwd: &PathBuf,
    ) -> Vec<SearchResult> {
        let mut current_dir_matches = Vec::new();
        let mut other_dir_matches = Vec::new();
        
        let current_cwd_str = current_cwd.to_string_lossy().to_string();

        for (shell_name, deduplicated_commands) in shell_histories {
            for cmd in deduplicated_commands {
                if let Some((score, indices)) = self.matcher.fuzzy_indices(&cmd.command, search_term) {
                    let search_result = SearchResult::new_shell_command(
                        shell_name.clone(),
                        cmd.command.clone(),
                        cmd.folders.clone(),
                        score,
                        indices,
                    );

                    let is_current_dir = cmd.folders.contains(&current_cwd_str);

                    if is_current_dir {
                        current_dir_matches.push((search_result, cmd.latest_timestamp));
                    } else {
                        other_dir_matches.push((search_result, cmd.latest_timestamp));
                    }
                }
            }
        }

        Self::sort_shell_matches_by_score_and_recency(&mut current_dir_matches);
        Self::sort_shell_matches_by_score_and_recency(&mut other_dir_matches);

        let mut final_matches = Vec::new();
        final_matches.extend(current_dir_matches.into_iter().map(|(result, _)| result));
        final_matches.extend(other_dir_matches.into_iter().map(|(result, _)| result));

        final_matches.truncate(50);
        final_matches
    }

    fn sort_shell_matches_by_score_and_recency(
        matches: &mut Vec<(SearchResult, Option<u64>)>,
    ) {
        matches.sort_by(|a, b| {
            let score_cmp = b.0.score.cmp(&a.0.score);
            if score_cmp != std::cmp::Ordering::Equal {
                return score_cmp;
            }

            match (&b.1, &a.1) {
                (Some(timestamp_b), Some(timestamp_a)) => timestamp_b.cmp(timestamp_a),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });
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