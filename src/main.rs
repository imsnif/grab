#[cfg(not(test))]
use zellij_tile::prelude::*;
#[cfg(test)]
use crate::unit::test_zellij::prelude::*;

use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;

#[cfg(test)]
mod unit;

mod app_state;
mod ui_state;
mod search_state;
mod search;
mod ui;
mod pane;
mod files;

register_plugin!(State);

use crate::app_state::AppState;
use crate::ui_state::UIState;
use crate::search_state::SearchState;
use crate::search::{SearchEngine, SearchItem};
use crate::ui::UIRenderer;
use crate::pane::extract_editor_pane_metadata;
use crate::files::get_all_files;

fn is_current_directory_git_repository() -> bool {
    // Check if the current host folder has a .git directory or file
    let git_dir = PathBuf::from("/host/.git");
    git_dir.exists() && (git_dir.is_dir() || git_dir.is_file())
}

#[derive(Debug, Clone)]
pub enum RustAssetSearchMode {
    Struct(String),    // Search term after "struct"
    Enum(String),      // Search term after "enum"
    Function(String),  // Search term after "fn"
}

fn parse_rust_asset_search(search_term: &str) -> Option<RustAssetSearchMode> {
    // Don't trim initially - we need to preserve trailing spaces

    if let Some(rest) = search_term.strip_prefix("struct ") {
        Some(RustAssetSearchMode::Struct(rest.to_string()))
    } else if let Some(rest) = search_term.strip_prefix("enum ") {
        Some(RustAssetSearchMode::Enum(rest.to_string()))
    } else if let Some(rest) = search_term.strip_prefix("fn ") {
        Some(RustAssetSearchMode::Function(rest.to_string()))
    } else {
        // Case insensitive check
        let lower = search_term.to_lowercase();
        if let Some(_rest) = lower.strip_prefix("struct ") {
            // Find the original casing for the search term after "struct "
            let original_rest = &search_term[7..]; // Skip "struct " (7 chars)
            Some(RustAssetSearchMode::Struct(original_rest.to_string()))
        } else if let Some(_rest) = lower.strip_prefix("enum ") {
            // Find the original casing for the search term after "enum "
            let original_rest = &search_term[5..]; // Skip "enum " (5 chars)
            Some(RustAssetSearchMode::Enum(original_rest.to_string()))
        } else if let Some(_rest) = lower.strip_prefix("fn ") {
            // Find the original casing for the search term after "fn "
            let original_rest = &search_term[3..]; // Skip "fn " (3 chars)
            Some(RustAssetSearchMode::Function(original_rest.to_string()))
        } else {
            None
        }
    }
}

#[derive(Default)]
pub struct State {
    app_state: AppState,
    ui_state: UIState,
    search_state: SearchState,
    search_engine: SearchEngine,
    ui_renderer: UIRenderer,
    tabs: Vec<TabInfo>,
    request_ids: Vec<String>,
    initial_cwd: Option<PathBuf>,
    searching_for_git_repo: bool,
}

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::OpenFiles,
            PermissionType::FullHdAccess,
            PermissionType::MessageAndLaunchOtherPlugins,
            PermissionType::RunCommands,
        ]);
        subscribe(&[
            EventType::PaneUpdate,
            EventType::Key,
            EventType::PermissionRequestResult,
            EventType::TabUpdate,
            EventType::HostFolderChanged,
        ]);

        self.initial_cwd = Some(get_plugin_ids().initial_cwd);
        self.update_host_folder(None);
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::PermissionRequestResult(_) => {
                let own_plugin_id = get_plugin_ids().plugin_id;
                rename_plugin_pane(own_plugin_id, "Grab...");
                
                self.searching_for_git_repo = true;
                self.start_git_repository_search();
            }
            Event::TabUpdate(tab_info) => {
                self.tabs = tab_info;
            }
            Event::PaneUpdate(pane_manifest) => {
                let panes = extract_editor_pane_metadata(&pane_manifest);
                self.app_state.update_panes(panes);
                self.adjust_selection_after_pane_update();
                self.update_search_results();
                should_render = true;
            }
            Event::HostFolderChanged(new_host_folder) => {
                if let Some(initial_cwd) = self.initial_cwd.take() {
                    change_host_folder(initial_cwd);
                } else if self.searching_for_git_repo {
                    self.continue_git_repository_search(new_host_folder);
                } else {
                    let user_selected = self.app_state.is_user_selected_directory();
                    self.app_state.set_user_selected_directory(false); // Reset flag after use
                    self.update_host_folder_with_scan_control(Some(new_host_folder), user_selected);
                }
                should_render = true;
            }
            Event::Key(key) => match key.bare_key {
                BareKey::Down if key.has_no_modifiers() => {
                    self.move_selection_down();
                    should_render = true;
                }
                BareKey::Up if key.has_no_modifiers() => {
                    self.move_selection_up();
                    should_render = true;
                }
                BareKey::Tab | BareKey::Enter if key.has_no_modifiers() => {
                    self.focus_selected_item();
                }
                BareKey::Char(character) if key.has_no_modifiers() => {
                    self.search_state.add_char(character);
                    self.update_search_results();
                    should_render = true;
                }
                BareKey::Backspace if key.has_no_modifiers() => {
                    self.search_state.remove_char();
                    self.update_search_results();
                    should_render = true;
                }
                BareKey::Char('c') if key.has_modifiers(&[KeyModifier::Ctrl]) => {
                    if self.search_state.is_empty() {
                        close_self();
                    } else {
                        self.search_state.clear();
                        self.update_search_results();
                        should_render = true;
                    }
                }
                BareKey::Char('f') if key.has_modifiers(&[KeyModifier::Ctrl]) => {
                    let request_id = Uuid::new_v4();
                    let mut config = BTreeMap::new();
                    let mut args = BTreeMap::new();
                    self.request_ids.push(request_id.to_string());
                    config.insert("request_id".to_owned(), request_id.to_string());
                    config.insert("caller_cwd".to_owned(), self.app_state.get_cwd().display().to_string());
                    args.insert("request_id".to_owned(), request_id.to_string());
                    pipe_message_to_plugin(
                        MessageToPlugin::new("filepicker")
                            .with_plugin_url("filepicker")
                            .with_plugin_config(config)
                            .new_plugin_instance_should_have_pane_title(
                                "Select new base folder for the picker...",
                            )
                            .new_plugin_instance_should_replace_pane(PaneId::Plugin(get_plugin_ids().plugin_id))
                            .with_args(args),
                    );
                    should_render = true;
                },
                _ => {}
            },
            _ => {}
        }
        should_render
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if pipe_message.name == "filepicker_result" {
            match (pipe_message.payload, pipe_message.args.get("request_id")) {
                (Some(payload), Some(request_id)) => {
                    match self.request_ids.iter().position(|p| p == request_id) {
                        Some(request_id_position) => {
                            self.request_ids.remove(request_id_position);
                            let new_folder = std::path::PathBuf::from(payload);
                            // Mark that this is a user-selected directory, so scanning should proceed
                            self.app_state.set_user_selected_directory(true);
                            change_host_folder(new_folder);
                        },
                        None => {},
                    }
                },
                _ => {},
            }
            true
        } else {
            false
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.ui_state.update_last_rows(rows);

        let table_count = self.search_state.get_current_display_count();

        let available_rows = rows.saturating_sub(8);
        let visible_items = available_rows.min(table_count);

        self.ui_state.adjust_scroll_for_selection(visible_items, table_count);

        let (displayed_files, remaining_files) = self.search_engine.get_displayed_files(
            self.search_state.get_term(),
            self.app_state.get_files(),
        );

        self.ui_renderer.render_plugin(
            rows,
            cols,
            self.search_state.get_term(),
            self.app_state.get_panes(),
            self.search_state.get_files_panes_results(),
            self.ui_state.selected_index,
            self.ui_state.scroll_offset,
            &displayed_files,
            remaining_files,
            self.app_state.get_cwd(),
        );
    }
}

impl State {
    fn update_search_results(&mut self) {
        let rust_assets = self.app_state.get_rust_assets();
        let results = self.search_engine.search(
            self.search_state.get_term(),
            self.app_state.get_panes(),
            self.app_state.get_files(),
            &rust_assets,
            self.app_state.get_cwd(),
        );

        self.search_state.update_results(results);

        let table_count = self.search_state.get_current_display_count();

        if table_count > 0 {
            self.ui_state.set_selected_index(Some(0));
        } else {
            self.ui_state.set_selected_index(None);
        }
    }

    fn move_selection_down(&mut self) {
        let table_count = self.search_state.get_current_display_count();
        
        if table_count > 0 {
            self.ui_state.move_selection_down(table_count);
        }
    }

    fn move_selection_up(&mut self) {
        let table_count = self.search_state.get_current_display_count();
        
        if table_count > 0 {
            self.ui_state.move_selection_up(table_count);
        }
    }

    fn focus_selected_item(&mut self) {
        if let Some(selected_index) = self.ui_state.get_selected_index() {
            let display_results = self.search_state.get_current_display_results();
            if let Some(search_result) = display_results.get(selected_index).cloned() {
                self.execute_search_result_action(&search_result);
            }
        }
    }

    fn execute_search_result_action(&mut self, search_result: &crate::search::SearchResult) {
        match &search_result.item {
            SearchItem::Pane(pane) => {
                let own_plugin_id = get_plugin_ids().plugin_id;
                replace_pane_with_existing_pane(PaneId::Plugin(own_plugin_id), pane.id);
            },
            SearchItem::File(file) => {
                let should_close_plugin = true;
                open_file_in_place_of_plugin(
                    FileToOpen::new(self.app_state.get_cwd().join(file)),
                    should_close_plugin,
                    Default::default(),
                );
            },
            SearchItem::RustAsset(rust_asset) => {
                let should_close_plugin = true;
                let mut file_to_open = FileToOpen::new(self.app_state.get_cwd().join(rust_asset.file_path.as_ref()));
                file_to_open.line_number = Some(rust_asset.line_number);
                open_file_in_place_of_plugin(
                    file_to_open,
                    should_close_plugin,
                    Default::default(),
                );
            },
        }
    }

    fn adjust_selection_after_pane_update(&mut self) {
        let table_count = self.search_state.get_current_display_count();

        self.ui_state.adjust_selection_after_update(table_count);
    }

    fn update_host_folder(&mut self, new_host_folder: Option<PathBuf>) {
        self.update_host_folder_with_scan_control(new_host_folder, false);
    }

    fn update_host_folder_with_scan_control(&mut self, new_host_folder: Option<PathBuf>, user_selected: bool) {
        let new_host_folder = new_host_folder.unwrap_or_else(|| get_plugin_ids().initial_cwd);
        self.app_state.set_cwd(new_host_folder);
        
        // Only scan if conditions are met
        let should_scan = self.app_state.get_files().is_empty() && 
                         (is_current_directory_git_repository() || user_selected);
        
        if should_scan {
            if let Ok(files_and_rust_assets) = get_all_files("/host") {
                let files: Vec<PathBuf> = files_and_rust_assets.keys().cloned().collect();
                self.app_state.update_files(files);
                self.app_state.update_rust_assets(files_and_rust_assets)
            }
        }
        self.update_search_results();
    }

    fn start_git_repository_search(&mut self) {
        let initial_cwd = get_plugin_ids().initial_cwd;
        change_host_folder(initial_cwd);
    }

    fn continue_git_repository_search(&mut self, current_folder: PathBuf) {
        if is_current_directory_git_repository() {
            self.searching_for_git_repo = false;
            // This is a git repo, so we can scan
            self.update_host_folder_with_scan_control(Some(current_folder), false);
        } else {
            // Try to go to parent directory
            match current_folder.parent() {
                Some(parent) if parent != current_folder => {
                    change_host_folder(parent.to_path_buf());
                }
                _ => {
                    // Reached root or can't go further up
                    self.searching_for_git_repo = false;
                    self.request_folder_selection();
                }
            }
        }
    }

    fn request_folder_selection(&mut self) {
        let request_id = Uuid::new_v4();
        let mut config = BTreeMap::new();
        let mut args = BTreeMap::new();
        self.request_ids.push(request_id.to_string());
        config.insert("request_id".to_owned(), request_id.to_string());
        config.insert("caller_cwd".to_owned(), self.app_state.get_cwd().display().to_string());
        args.insert("request_id".to_owned(), request_id.to_string());
        pipe_message_to_plugin(
            MessageToPlugin::new("filepicker")
                .with_plugin_url("filepicker")
                .with_plugin_config(config)
                .new_plugin_instance_should_have_pane_title(
                    "Git folder not found, please select a base folder for the picker...",
                )
                .new_plugin_instance_should_replace_pane(PaneId::Plugin(get_plugin_ids().plugin_id))
                .with_args(args),
        );
    }
}

