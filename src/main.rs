#[cfg(not(test))]
use zellij_tile::prelude::*;
#[cfg(test)]
use crate::test_zellij::prelude::*;

use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;

// Mock Zellij for the tests
#[cfg(test)]
pub mod test_zellij;

#[cfg(test)]
mod fixtures;

mod app_state;
mod ui_state;
mod search_state;
mod search;
mod ui;
mod pane;
mod files;
mod read_shell_histories;

register_plugin!(State);

use crate::app_state::AppState;
use crate::ui_state::UIState;
use crate::search_state::SearchState;
use crate::search::{SearchEngine, SearchItem};
use crate::ui::UIRenderer;
use crate::pane::extract_editor_pane_metadata;
use crate::files::get_all_files;
use crate::read_shell_histories::{DeduplicatedCommand, read_shell_histories};

// Git repository detection functions
fn is_current_directory_git_repository() -> bool {
    // Check if the current host folder has a .git directory or file
    let git_dir = PathBuf::from("/host/.git");
    git_dir.exists() && (git_dir.is_dir() || git_dir.is_file())
}

// Rust asset search detection
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
                
                // Start searching for git repository from initial directory
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
                    self.populate_shell_histories();
                    change_host_folder(initial_cwd);
                } else if self.searching_for_git_repo {
                    // Continue git repository search
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
            &[], // No shell commands
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
            self.app_state.get_shell_histories(),
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

    fn populate_shell_histories(&mut self) {
        let shell_histories = read_shell_histories();
        let btree_histories: BTreeMap<String, Vec<DeduplicatedCommand>> = shell_histories.into_iter().collect();
        self.app_state.update_shell_histories(btree_histories);
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
                    "Select base folder for the picker...",
                )
                .new_plugin_instance_should_replace_pane(PaneId::Plugin(get_plugin_ids().plugin_id))
                .with_args(args),
        );
    }
}

// ** NOTE: To run the tests, run "cargo test --target x86_64-unknown-linux-gnu" **

#[cfg(test)]
mod tests {

    use super::*;
    use crate::test_zellij;

    fn setup() -> State {
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/test/project"),
        });
        State::default()
    }

    #[test]
    fn test_load_requests_permissions() {
        let mut state = setup();
        state.load(BTreeMap::new());

        let calls = test_zellij::mock_get_calls();
        assert!(calls.iter().any(|c| matches!(c, test_zellij::ZellijCall::RequestPermission(_))));
    }

    #[test]
    fn test_permission_result_renames_pane() {
        let mut state = setup();
        state.load(BTreeMap::new());
        test_zellij::mock_clear_calls();

        state.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        let calls = test_zellij::mock_get_calls();
        assert!(calls.iter().any(|c| matches!(
            c,
            test_zellij::ZellijCall::RenamePluginPane { id: 42, name }
                if name == "Grab..."
        )));
    }

    #[test]
    fn test_down_key_triggers_render() {
        let mut state = setup();
        state.load(BTreeMap::new());

        let should_render = state.update(Event::Key(Key {
            bare_key: BareKey::Down,
            modifiers: vec![],
        }));

        assert!(should_render);
    }

    #[test]
    fn test_typing_triggers_render() {
        let mut state = setup();
        state.load(BTreeMap::new());

        let should_render = state.update(Event::Key(Key {
            bare_key: BareKey::Char('x'),
            modifiers: vec![],
        }));

        assert!(should_render);
    }

    #[test]
    fn test_ctrl_c_on_empty_search_closes_plugin() {
        let mut state = setup();
        state.load(BTreeMap::new());
        test_zellij::mock_clear_calls();

        state.update(Event::Key(Key {
            bare_key: BareKey::Char('c'),
            modifiers: vec![KeyModifier::Ctrl],
        }));

        let calls = test_zellij::mock_get_calls();
        assert!(calls.iter().any(|c| matches!(c, test_zellij::ZellijCall::CloseSelf)));
    }

    #[test]
    fn test_ctrl_c_with_text_does_not_close() {
        let mut state = setup();
        state.load(BTreeMap::new());

        // Type something
        state.update(Event::Key(Key {
            bare_key: BareKey::Char('x'),
            modifiers: vec![],
        }));

        test_zellij::mock_clear_calls();

        state.update(Event::Key(Key {
            bare_key: BareKey::Char('c'),
            modifiers: vec![KeyModifier::Ctrl],
        }));

        let calls = test_zellij::mock_get_calls();
        assert!(!calls.iter().any(|c| matches!(c, test_zellij::ZellijCall::CloseSelf)));
    }

    #[test]
    fn test_render_completes_without_panic() {
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/test/project"),
        });
        test_zellij::mock_init_frame(80, 24);

        let mut state = State::default();
        state.load(BTreeMap::new());
        state.update(Event::PermissionRequestResult(PermissionStatus::Granted));
        state.render(24, 80);

        // Assert against snapshot to verify rendering output
        test_zellij::assert_frame_snapshot("render_default_state");
    }

    #[test]
    fn test_render_empty_state() {
        // Setup
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/home/user/project"),
        });
        test_zellij::mock_init_frame(80, 24);

        let mut plugin = State::default();
        plugin.load(BTreeMap::new());

        // Simulate permission granted
        plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        // Render with no search term
        plugin.render(24, 80);

        // Assert snapshot
        test_zellij::assert_frame_snapshot("render_empty_state");
    }

    #[test]
    fn test_render_with_sample_data() {
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/home/user/project"),
        });
        test_zellij::mock_init_frame(80, 24);

        let mut plugin = State::default();
        plugin.app_state.update_panes(fixtures::sample_panes());
        plugin.app_state.update_files(fixtures::sample_files());
        plugin.app_state.update_rust_assets(fixtures::sample_rust_assets());

        plugin.load(BTreeMap::new());
        plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        plugin.render(24, 80);

        test_zellij::assert_frame_snapshot("render_with_sample_data");
    }

    #[test]
    fn test_render_with_search_term() {
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/home/user/project"),
        });
        test_zellij::mock_init_frame(80, 24);

        let mut plugin = State::default();
        plugin.app_state.update_panes(fixtures::sample_panes());
        plugin.app_state.update_files(fixtures::sample_files());
        plugin.app_state.update_rust_assets(fixtures::sample_rust_assets());

        plugin.load(BTreeMap::new());
        plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        // Type search term "main"
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Char('m'),
            modifiers: vec![],
        }));
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Char('a'),
            modifiers: vec![],
        }));
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Char('i'),
            modifiers: vec![],
        }));
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Char('n'),
            modifiers: vec![],
        }));

        // Render
        plugin.render(24, 80);

        test_zellij::assert_frame_snapshot("render_with_search_main");
    }

    #[test]
    fn test_render_with_selection() {
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/home/user/project"),
        });
        test_zellij::mock_init_frame(80, 24);

        let mut plugin = State::default();
        plugin.app_state.update_panes(fixtures::sample_panes());

        plugin.load(BTreeMap::new());
        plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        // Press down to select first result
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Down,
            modifiers: vec![],
        }));

        plugin.render(24, 80);

        test_zellij::assert_frame_snapshot("render_with_selection");
    }

    #[test]
    fn test_typing_string_searches_and_displays_results() {
        // Setup
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/home/user/project"),
        });
        test_zellij::mock_init_frame(80, 24);

        let mut plugin = State::default();
        plugin.app_state.update_panes(fixtures::sample_panes());
        plugin.app_state.update_files(fixtures::sample_files());
        plugin.app_state.update_rust_assets(fixtures::sample_rust_assets());

        plugin.load(BTreeMap::new());
        plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        test_zellij::mock_clear_calls();

        // Type "cargo" to search for Cargo.toml
        for ch in "cargo".chars() {
            plugin.update(Event::Key(Key {
                bare_key: BareKey::Char(ch),
                modifiers: vec![],
            }));
        }

        // Verify search results were updated
        let results = plugin.search_state.get_current_display_results();
        assert!(!results.is_empty(), "Should have search results for 'cargo'");

        // Check that Cargo.toml is in the results
        let has_cargo_toml = results.iter().any(|r| {
            r.display_text().contains("Cargo.toml")
        });
        assert!(has_cargo_toml, "Cargo.toml should be in search results");

        // Render and verify output
        plugin.render(24, 80);
        test_zellij::assert_frame_snapshot("search_results_cargo");
    }

    #[test]
    fn test_enter_on_pane_opens_pane() {
        // Setup
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/home/user/project"),
        });

        let mut plugin = State::default();
        plugin.app_state.update_panes(fixtures::sample_panes());

        plugin.load(BTreeMap::new());
        plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        // Type "vim" to search for vim panes
        for ch in "vim".chars() {
            plugin.update(Event::Key(Key {
                bare_key: BareKey::Char(ch),
                modifiers: vec![],
            }));
        }

        // Verify we have results
        let results = plugin.search_state.get_current_display_results();
        assert!(!results.is_empty(), "Should have search results for 'vim'");

        // First result should be a pane (from "vim ~/project/src/main.rs")
        assert!(results[0].is_pane(), "First result should be a pane");

        test_zellij::mock_clear_calls();

        // Press ENTER to open the pane
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Enter,
            modifiers: vec![],
        }));

        // Verify that replace_pane_with_existing_pane was called
        let calls = test_zellij::mock_get_calls();
        let replaced = calls.iter().any(|c| {
            matches!(c, test_zellij::ZellijCall::ReplacePaneWithExistingPane {
                plugin_pane: PaneId::Plugin(42),
                target_pane: PaneId::Terminal(1)
            })
        });
        assert!(replaced, "Should call replace_pane_with_existing_pane for pane");
    }

    #[test]
    fn test_enter_on_file_opens_file() {
        // Setup
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/home/user/project"),
        });

        let mut plugin = State::default();
        plugin.app_state.set_cwd(PathBuf::from("/home/user/project"));
        plugin.app_state.update_files(fixtures::sample_files());

        plugin.load(BTreeMap::new());
        plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        // Type "README" to search for README.md
        for ch in "README".chars() {
            plugin.update(Event::Key(Key {
                bare_key: BareKey::Char(ch),
                modifiers: vec![],
            }));
        }

        // Verify we have results
        let results = plugin.search_state.get_current_display_results();
        assert!(!results.is_empty(), "Should have search results for 'README'");

        // Result should be a file
        assert!(results[0].is_file(), "Result should be a file");

        test_zellij::mock_clear_calls();

        // Press ENTER to open the file
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Enter,
            modifiers: vec![],
        }));

        // Verify that open_file_in_place_of_plugin was called
        let calls = test_zellij::mock_get_calls();
        let opened = calls.iter().any(|c| {
            matches!(c, test_zellij::ZellijCall::OpenFileInPlaceOfPlugin {
                path,
                line_number: None,
                close_plugin: true
            } if path.ends_with("README.md"))
        });
        assert!(opened, "Should call open_file_in_place_of_plugin for file");
    }

    #[test]
    fn test_struct_search_and_enter_opens_file_at_line() {
        use crate::files::TypeKind;

        // Setup
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/home/user/project"),
        });
        test_zellij::mock_init_frame(80, 24);

        let mut plugin = State::default();
        plugin.app_state.set_cwd(PathBuf::from("/home/user/project"));
        plugin.app_state.update_rust_assets(fixtures::struct_search_rust_assets());

        plugin.load(BTreeMap::new());
        plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        // Type "struct mystruct" to search for MyStruct (fuzzy match)
        for ch in "struct mystruct".chars() {
            plugin.update(Event::Key(Key {
                bare_key: BareKey::Char(ch),
                modifiers: vec![],
            }));
        }

        // Verify we have results and they're rust assets
        let results = plugin.search_state.get_current_display_results();
        assert!(!results.is_empty(), "Should have search results for 'struct mystruct'");

        // All results should only be structs (not functions)
        for result in &results {
            assert!(result.is_rust_asset(), "All results should be rust assets");
            if let crate::search::SearchItem::RustAsset(asset) = &result.item {
                assert!(matches!(asset.type_kind, TypeKind::Struct), "Should only show structs");
            }
        }

        // Should fuzzy match MyStruct and MyStructHelper
        let result_names: Vec<String> = results.iter()
            .filter_map(|r| {
                if let crate::search::SearchItem::RustAsset(asset) = &r.item {
                    Some(asset.name.clone())
                } else {
                    None
                }
            })
            .collect();
        assert!(result_names.contains(&"MyStruct".to_string()), "Should find MyStruct");
        assert!(result_names.contains(&"MyStructHelper".to_string()), "Should find MyStructHelper");

        // Render and verify output
        plugin.render(24, 80);
        test_zellij::assert_frame_snapshot("struct_search_mystruct");

        test_zellij::mock_clear_calls();

        // Press ENTER to open the file at the line (should open first result)
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Enter,
            modifiers: vec![],
        }));

        // Verify that open_file_in_place_of_plugin was called with line number
        let calls = test_zellij::mock_get_calls();
        let opened = calls.iter().any(|c| {
            matches!(c, test_zellij::ZellijCall::OpenFileInPlaceOfPlugin {
                path: _,
                line_number: Some(_),
                close_plugin: true
            })
        });
        assert!(opened, "Should call open_file_in_place_of_plugin with line number");
    }

    #[test]
    fn test_enum_search_and_enter_opens_file_at_line() {
        use crate::files::TypeKind;

        // Setup
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/home/user/project"),
        });
        test_zellij::mock_init_frame(80, 24);

        let mut plugin = State::default();
        plugin.app_state.set_cwd(PathBuf::from("/home/user/project"));
        plugin.app_state.update_rust_assets(fixtures::enum_search_rust_assets());

        plugin.load(BTreeMap::new());
        plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        // Type "enum search" to fuzzy search for search-related enums
        for ch in "enum search".chars() {
            plugin.update(Event::Key(Key {
                bare_key: BareKey::Char(ch),
                modifiers: vec![],
            }));
        }

        // Verify we have results and they're rust assets
        let results = plugin.search_state.get_current_display_results();
        assert!(!results.is_empty(), "Should have search results for 'enum search'");

        // All results should only be enums (not structs)
        for result in &results {
            assert!(result.is_rust_asset(), "All results should be rust assets");
            if let crate::search::SearchItem::RustAsset(asset) = &result.item {
                assert!(matches!(asset.type_kind, TypeKind::Enum), "Should only show enums");
            }
        }

        // Should fuzzy match SearchMode, SearchType, and SearchItem
        let result_names: Vec<String> = results.iter()
            .filter_map(|r| {
                if let crate::search::SearchItem::RustAsset(asset) = &r.item {
                    Some(asset.name.clone())
                } else {
                    None
                }
            })
            .collect();
        assert!(result_names.contains(&"SearchMode".to_string()), "Should find SearchMode");
        assert!(result_names.contains(&"SearchType".to_string()), "Should find SearchType");
        assert!(result_names.contains(&"SearchItem".to_string()), "Should find SearchItem");

        // Render and verify output
        plugin.render(24, 80);
        test_zellij::assert_frame_snapshot("enum_search_search");

        test_zellij::mock_clear_calls();

        // Press ENTER to open the file at the line (should open first result)
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Enter,
            modifiers: vec![],
        }));

        // Verify that open_file_in_place_of_plugin was called with line number
        let calls = test_zellij::mock_get_calls();
        let opened = calls.iter().any(|c| {
            matches!(c, test_zellij::ZellijCall::OpenFileInPlaceOfPlugin {
                path: _,
                line_number: Some(_),
                close_plugin: true
            })
        });
        assert!(opened, "Should call open_file_in_place_of_plugin with line number");
    }

    #[test]
    fn test_fn_search_and_enter_opens_file_at_line() {
        use crate::files::TypeKind;

        // Setup
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/home/user/project"),
        });
        test_zellij::mock_init_frame(80, 24);

        let mut plugin = State::default();
        plugin.app_state.set_cwd(PathBuf::from("/home/user/project"));
        plugin.app_state.update_rust_assets(fixtures::function_search_rust_assets());

        plugin.load(BTreeMap::new());
        plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        // Type "fn render" to fuzzy search for render-related functions
        for ch in "fn render".chars() {
            plugin.update(Event::Key(Key {
                bare_key: BareKey::Char(ch),
                modifiers: vec![],
            }));
        }

        // Verify we have results and they're rust assets
        let results = plugin.search_state.get_current_display_results();
        assert!(!results.is_empty(), "Should have search results for 'fn render'");

        // All results should only be functions (not structs)
        for result in &results {
            assert!(result.is_rust_asset(), "All results should be rust assets");
            if let crate::search::SearchItem::RustAsset(asset) = &result.item {
                assert!(matches!(asset.type_kind, TypeKind::Function), "Should only show functions");
            }
        }

        // Should fuzzy match render-related functions
        let result_names: Vec<String> = results.iter()
            .filter_map(|r| {
                if let crate::search::SearchItem::RustAsset(asset) = &r.item {
                    Some(asset.name.clone())
                } else {
                    None
                }
            })
            .collect();
        assert!(result_names.contains(&"render".to_string()), "Should find render");
        assert!(result_names.contains(&"render_ui".to_string()), "Should find render_ui");
        assert!(result_names.contains(&"render_table".to_string()), "Should find render_table");
        assert!(result_names.contains(&"render_text".to_string()), "Should find render_text");

        // Render and verify output
        plugin.render(24, 80);
        test_zellij::assert_frame_snapshot("fn_search_render");

        test_zellij::mock_clear_calls();

        // Press ENTER to open the file at the line (should open first result)
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Enter,
            modifiers: vec![],
        }));

        // Verify that open_file_in_place_of_plugin was called with line number
        let calls = test_zellij::mock_get_calls();
        let opened = calls.iter().any(|c| {
            matches!(c, test_zellij::ZellijCall::OpenFileInPlaceOfPlugin {
                path: _,
                line_number: Some(_),
                close_plugin: true
            })
        });
        assert!(opened, "Should call open_file_in_place_of_plugin with line number");
    }

    #[test]
    fn test_ctrl_f_calls_filepicker() {
        // Setup
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/home/user/project"),
        });

        let mut plugin = State::default();
        plugin.app_state.set_cwd(PathBuf::from("/home/user/project"));
        plugin.load(BTreeMap::new());
        plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        test_zellij::mock_clear_calls();

        // Press Ctrl+F
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Char('f'),
            modifiers: vec![KeyModifier::Ctrl],
        }));

        // Verify that pipe_message_to_plugin was called with filepicker
        let calls = test_zellij::mock_get_calls();
        let called_filepicker = calls.iter().any(|c| {
            matches!(c, test_zellij::ZellijCall::PipeMessageToPlugin {
                plugin_url,
                args
            } if plugin_url == "filepicker" && args.contains_key("request_id"))
        });
        assert!(called_filepicker, "Should call pipe_message_to_plugin with filepicker");

        // Verify request_id was stored
        assert!(!plugin.request_ids.is_empty(), "Should store request_id");
    }

    #[test]
    fn test_receiving_pipe_from_filepicker_changes_folder() {
        // Setup
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/home/user/project"),
        });
        test_zellij::mock_init_frame(80, 24);

        let mut plugin = State::default();
        plugin.app_state.set_cwd(PathBuf::from("/home/user/project"));
        plugin.load(BTreeMap::new());
        plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        // Render initial state to show original folder
        plugin.render(24, 80);
        test_zellij::assert_frame_snapshot("filepicker_before_folder_change");

        // Simulate pressing Ctrl+F to get a request_id
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Char('f'),
            modifiers: vec![KeyModifier::Ctrl],
        }));

        let request_id = plugin.request_ids[0].clone();
        test_zellij::mock_clear_calls();

        // Simulate receiving a pipe message from filepicker
        let mut args = BTreeMap::new();
        args.insert("request_id".to_string(), request_id.clone());

        let pipe_message = PipeMessage {
            source: test_zellij::PipeSource::Plugin(1),
            name: "filepicker_result".to_string(),
            payload: Some("/new/folder/path".to_string()),
            args,
            is_private: false,
        };

        plugin.pipe(pipe_message);

        // Verify that change_host_folder was called with the new path
        let calls = test_zellij::mock_get_calls();
        let changed_folder = calls.iter().any(|c| {
            matches!(c, test_zellij::ZellijCall::ChangeHostFolder {
                path
            } if path == &PathBuf::from("/new/folder/path"))
        });
        assert!(changed_folder, "Should call change_host_folder with new path");

        // Verify request_id was removed
        assert!(plugin.request_ids.is_empty(), "Should remove request_id after processing");

        // Verify user_selected_directory flag was set
        assert!(plugin.app_state.is_user_selected_directory(), "Should mark as user selected directory");

        // Simulate the HostFolderChanged event that would be triggered by Zellij
        plugin.update(Event::HostFolderChanged(PathBuf::from("/new/folder/path")));

        // Verify the folder was updated in app state
        assert_eq!(plugin.app_state.get_cwd(), &PathBuf::from("/new/folder/path"),
            "App state should reflect new folder");

        // Render and verify the new folder is displayed
        test_zellij::mock_clear_frame();
        plugin.render(24, 80);
        test_zellij::assert_frame_snapshot("filepicker_after_folder_change");

        // Verify the frame contains the new folder path
        let frame = test_zellij::mock_get_frame().expect("Frame should be initialized");
        let frame_str = frame.to_string();
        assert!(frame_str.contains("/new/folder/path"),
            "Rendered output should display the new folder path");
    }

    #[test]
    fn test_struct_keyword_filters_only_structs() {
        // Setup
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/home/user/project"),
        });

        let mut plugin = State::default();
        plugin.app_state.update_rust_assets(fixtures::sample_rust_assets());

        plugin.load(BTreeMap::new());
        plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        // Type "struct " (with space) to search for all structs
        for ch in "struct ".chars() {
            plugin.update(Event::Key(Key {
                bare_key: BareKey::Char(ch),
                modifiers: vec![],
            }));
        }

        // Verify we have results and they're all structs
        let results = plugin.search_state.get_current_display_results();
        assert!(!results.is_empty(), "Should have search results for 'struct '");

        // All results should be structs
        for result in results {
            assert!(result.is_rust_asset(), "All results should be rust assets");
            if let crate::search::SearchItem::RustAsset(asset) = &result.item {
                assert!(matches!(asset.type_kind, crate::files::TypeKind::Struct),
                    "All results should be structs, found: {:?}", asset.type_kind);
            }
        }
    }

    #[test]
    fn test_search_rendering_shows_correct_results() {
        // Setup
        test_zellij::mock_init();
        test_zellij::mock_set_plugin_ids(PluginIds {
            plugin_id: 42,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/home/user/project"),
        });
        test_zellij::mock_init_frame(80, 24);

        let mut plugin = State::default();
        plugin.app_state.update_panes(fixtures::sample_panes());
        plugin.app_state.update_files(fixtures::sample_files());
        plugin.app_state.update_rust_assets(fixtures::sample_rust_assets());

        plugin.load(BTreeMap::new());
        plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

        // Type "ui" to search
        for ch in "ui".chars() {
            plugin.update(Event::Key(Key {
                bare_key: BareKey::Char(ch),
                modifiers: vec![],
            }));
        }

        // Render
        plugin.render(24, 80);

        // Verify frame contains expected results
        let frame = test_zellij::mock_get_frame().expect("Frame should be initialized");
        let frame_str = frame.to_string();

        // Should contain "ui" somewhere in the search results
        assert!(frame_str.contains("ui") || frame_str.contains("UI"),
            "Rendered output should contain search results for 'ui'");

        test_zellij::assert_frame_snapshot("search_results_ui");
    }
}
