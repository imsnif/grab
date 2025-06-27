use zellij_tile::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

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
}

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::OpenFiles,
        ]);
        subscribe(&[
            EventType::PaneUpdate,
            EventType::Key,
            EventType::PermissionRequestResult,
            EventType::TabUpdate,
        ]);

        self.app_state.set_cwd(get_plugin_ids().initial_cwd);
        if self.app_state.get_files().is_empty() {
            if let Ok(files_and_rust_assets) = get_all_files("/host") {
                let files: Vec<PathBuf> = files_and_rust_assets.keys().cloned().collect();
                self.app_state.update_files(files);
                self.app_state.update_rust_assets(files_and_rust_assets)
            }
        }
        self.update_search_results();
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
                _ => {}
            },
            _ => {}
        }
        should_render
    }

    fn pipe(&mut self, _pipe_message: PipeMessage) -> bool {
        false
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.ui_state.update_last_rows(rows);

        let (display_count, is_searching) = if self.search_state.is_empty() {
            (self.app_state.pane_count(), false)
        } else {
            (self.search_state.results_count(), true)
        };

        let available_rows = rows.saturating_sub(6);
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
}
