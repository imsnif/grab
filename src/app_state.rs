use std::path::PathBuf;
use std::collections::BTreeMap;
use crate::pane::PaneMetadata;
use crate::search::SearchResult;
use crate::files::TypeDefinition;
use crate::read_shell_histories::HistoryEntry;

#[derive(Default)]
pub struct AppState {
    pub pane_metadata: Vec<PaneMetadata>,
    pub files: Vec<PathBuf>,
    pub rust_assets: BTreeMap<PathBuf, Vec<TypeDefinition>>,
    pub cwd: PathBuf,
    pub shell_histories: BTreeMap<String, Vec<HistoryEntry>>, // <shell -> history entries>
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_panes(&mut self, panes: Vec<PaneMetadata>) {
        self.pane_metadata = panes;
    }

    pub fn update_files(&mut self, files: Vec<PathBuf>) {
        self.files = files;
    }

    pub fn update_rust_assets(&mut self, rust_assets: BTreeMap<PathBuf, Vec<TypeDefinition>>) {
        self.rust_assets = rust_assets;
    }

    pub fn update_shell_histories(&mut self, shell_histories: BTreeMap<String, Vec<HistoryEntry>>) {
        self.shell_histories = shell_histories;
    }

    pub fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
    }

    pub fn get_panes(&self) -> &[PaneMetadata] {
        &self.pane_metadata
    }

    pub fn get_files(&self) -> &[PathBuf] {
        &self.files
    }

    pub fn get_rust_assets(&self) -> Vec<TypeDefinition> {
        let mut all_assets = Vec::new();
        for definitions in self.rust_assets.values() {
            all_assets.extend(definitions.clone());
        }
        all_assets
    }

    pub fn get_shell_histories(&self) -> &BTreeMap<String, Vec<HistoryEntry>> {
        &self.shell_histories
    }

    pub fn get_cwd(&self) -> &PathBuf {
        &self.cwd
    }

    pub fn pane_count(&self) -> usize {
        self.pane_metadata.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pane_metadata.is_empty()
    }
}
