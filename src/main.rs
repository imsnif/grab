use zellij_tile::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;

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

#[derive(Default)]
pub struct State {
    app_state: AppState,
    ui_state: UIState,
    search_state: SearchState,
    search_engine: SearchEngine,
    ui_renderer: UIRenderer,
    tabs: Vec<TabInfo>,
    request_ids: Vec<String>,
}

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::OpenFiles,
            PermissionType::FullHdAccess,
            PermissionType::MessageAndLaunchOtherPlugins,
        ]);
        subscribe(&[
            EventType::PaneUpdate,
            EventType::Key,
            EventType::PermissionRequestResult,
            EventType::TabUpdate,
            EventType::HostFolderChanged,
        ]);

        self.update_host_folder(None, false);
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::PermissionRequestResult(_) => {
                let own_plugin_id = get_plugin_ids().plugin_id;
                rename_plugin_pane(own_plugin_id, "Grab...");
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
                eprintln!("HostFolderChanged");
                self.update_host_folder(Some(new_host_folder), true);
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
                BareKey::Enter if key.has_no_modifiers() => {
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
                    // we insert this into the config so that a new plugin will be opened (the plugin's
                    // uniqueness is determined by its name/url as well as its config)
                    config.insert("request_id".to_owned(), request_id.to_string());
                    config.insert("caller_cwd".to_owned(), self.app_state.get_cwd().display().to_string());
                    // we also insert this into the args so that the plugin will have an easier access to
                    // it
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
                            change_host_folder(new_folder);
                        },
                        None => {
                            eprintln!("request id not found");
                        },
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

        let (display_count, is_searching) = if self.search_state.is_empty() {
            (self.app_state.pane_count(), false)
        } else {
            (self.search_state.results_count(), true)
        };

        let available_rows = rows.saturating_sub(8); // Increased to account for cwd line
        let visible_items = available_rows.min(display_count);

        self.ui_state.adjust_scroll_for_selection(visible_items, display_count);

        let (displayed_files, remaining_files) = self.search_engine.get_displayed_files(
            self.search_state.get_term(),
            self.app_state.get_files(),
        );

        self.ui_renderer.render_plugin(
            rows,
            cols,
            self.search_state.get_term(),
            self.app_state.get_panes(),
            self.search_state.get_results(),
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
        if self.search_state.is_empty() {
            self.search_state.update_results(vec![]);
            self.ui_state.set_selected_index(None);
            return;
        }

        let rust_assets = self.app_state.get_rust_assets();
        let results = self.search_engine.search_panes_and_files(
            self.search_state.get_term(),
            self.app_state.get_panes(),
            self.app_state.get_files(),
            &rust_assets,
        );

        self.search_state.update_results(results);

        if self.search_state.has_results() {
            self.ui_state.set_selected_index(Some(0));
        } else {
            self.ui_state.set_selected_index(None);
        }
    }

    fn move_selection_down(&mut self) {
        let items_count = if self.search_state.is_empty() {
            self.app_state.pane_count()
        } else {
            self.search_state.results_count()
        };

        // Only allow selection if there are items to select
        if items_count > 0 {
            self.ui_state.move_selection_down(items_count);
        }
    }

    fn move_selection_up(&mut self) {
        let items_count = if self.search_state.is_empty() {
            self.app_state.pane_count()
        } else {
            self.search_state.results_count()
        };

        // Only allow selection if there are items to select
        if items_count > 0 {
            self.ui_state.move_selection_up(items_count);
        }
    }

    fn focus_selected_item(&mut self) {
        if let Some(selected_index) = self.ui_state.selected_index {
            if self.search_state.is_empty() {
                // Only try to focus panes if we have panes and are not searching
                if let Some(pane) = self.app_state.get_panes().get(selected_index) {
                    let own_plugin_id = get_plugin_ids().plugin_id;
                    replace_pane_with_existing_pane(PaneId::Plugin(own_plugin_id), pane.id);
                }
           } else {
                // When searching, focus the selected search result
                if let Some(search_result) = self.search_state.get_results().get(selected_index) {
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
                            let mut file_to_open = FileToOpen::new(self.app_state.get_cwd().join(&rust_asset.file_path));
                            file_to_open.line_number = Some(rust_asset.line_number);
                            open_file_in_place_of_plugin(
                                file_to_open,
                                should_close_plugin,
                                Default::default(),
                            );
                        }
                    }
                }
            }
        }
    }

    fn adjust_selection_after_pane_update(&mut self) {
        let items_count = if self.search_state.is_empty() {
            self.app_state.pane_count()
        } else {
            self.search_state.results_count()
        };

        self.ui_state.adjust_selection_after_update(items_count);
    }

    fn update_host_folder(&mut self, new_host_folder: Option<PathBuf>, force_update: bool) {
        let new_host_folder = new_host_folder.unwrap_or_else(|| get_plugin_ids().initial_cwd);
        self.app_state.set_cwd(new_host_folder);
        if self.app_state.get_files().is_empty() || force_update {
            if let Ok(files_and_rust_assets) = get_all_files("/host") {
                let files: Vec<PathBuf> = files_and_rust_assets.keys().cloned().collect();
                self.app_state.update_files(files);
                self.app_state.update_rust_assets(files_and_rust_assets)
            }
        }
        self.update_search_results();
    }
}
