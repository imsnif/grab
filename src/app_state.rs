use crate::files::TypeDefinition;
use crate::pane::PaneMetadata;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Default)]
pub struct AppState {
    pub pane_metadata: Vec<PaneMetadata>,
    pub files: Vec<PathBuf>,
    pub rust_assets: BTreeMap<PathBuf, Vec<TypeDefinition>>,
    pub cwd: PathBuf,
    pub user_selected_directory: bool, // Flag to track if directory was selected by user
}

impl AppState {
    pub fn update_panes(&mut self, panes: Vec<PaneMetadata>) {
        self.pane_metadata = panes;
    }

    pub fn update_files(&mut self, files: Vec<PathBuf>) {
        self.files = files;
    }

    pub fn update_rust_assets(&mut self, rust_assets: BTreeMap<PathBuf, Vec<TypeDefinition>>) {
        self.rust_assets = rust_assets;
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

    pub fn get_cwd(&self) -> &PathBuf {
        &self.cwd
    }

    pub fn set_user_selected_directory(&mut self, user_selected: bool) {
        self.user_selected_directory = user_selected;
    }

    pub fn is_user_selected_directory(&self) -> bool {
        self.user_selected_directory
    }
}
