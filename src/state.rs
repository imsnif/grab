use zellij_tile::prelude::*;
use std::collections::BTreeMap;

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
    pending_pane_moves: Vec<PaneId>,
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
                self.app_state.update_rust_assets(files_and_rust_assets);
            }
        }
        println!("load");
        self.update_search_results();
    }

    fn update(&mut self, event: Event) -> bool {
        eprintln!("update");
        let mut should_render = false;
        match event {
            Event::PermissionRequestResult(_) => {
                let own_plugin_id = get_plugin_ids().plugin_id;
                rename_plugin_pane(own_plugin_id, "Focus or open...");
                eprintln!("can");
                eprintln!("has");
            }
            Event::TabUpdate(tab_info) => {
                self.tabs = tab_info;
            }
            Event::PaneUpdate(pane_manifest) => {
                eprintln!("checking pending pane moves");
                self.check_pending_pane_moves(&pane_manifest);
                eprintln!("done checking");
                
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
            self.app_state.get_shell_histories(),
            self.app_state.get_cwd(), // Pass current working directory
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

        self.ui_state.move_selection_down(items_count);
    }

    fn move_selection_up(&mut self) {
        let items_count = if self.search_state.is_empty() {
            self.app_state.pane_count()
        } else {
            self.search_state.results_count()
        };

        self.ui_state.move_selection_up(items_count);
    }

    fn get_picker_tab_index(&self) -> Option<usize> {
        self.tabs.iter().find(|tab| tab.name == "picker 1").map(|tab| tab.position)
    }

    fn check_pending_pane_moves(&mut self, pane_manifest: &PaneManifest) {
        if self.pending_pane_moves.is_empty() {
            return;
        }

        let picker_tab_index = match self.get_picker_tab_index() {
            Some(index) => index,
            None => return,
        };

        let picker_tab_panes = match pane_manifest.panes.get(&picker_tab_index) {
            Some(panes) => panes,
            None => return,
        };

        let mut completed_moves = Vec::new();
        
        for (i, pending_pane_id) in self.pending_pane_moves.iter().enumerate() {
            let pane_found_in_picker_tab = picker_tab_panes.iter().any(|pane_info| {
                let pane_id = if pane_info.is_plugin {
                    PaneId::Plugin(pane_info.id)
                } else {
                    PaneId::Terminal(pane_info.id)
                };
                pane_id == *pending_pane_id
            });

            if pane_found_in_picker_tab {
                completed_moves.push(i);
            }
        }

        for &index in completed_moves.iter().rev() {
            self.pending_pane_moves.remove(index);
        }

        if !completed_moves.is_empty() && self.pending_pane_moves.is_empty() {
            eprintln!("closing self");
            close_self();
        }
    }

    fn focus_selected_item(&mut self) {
        if let Some(selected_index) = self.ui_state.selected_index {
            if self.search_state.is_empty() {
                if let Some(pane) = self.app_state.get_panes().get(selected_index) {
                    if let Some(tab_index) = self.get_picker_tab_index() {
                        self.pending_pane_moves.push(pane.id);
                        break_panes_to_tab_with_index(&[pane.id], tab_index, true);
                    }
                }
            } else {
                if let Some(search_result) = self.search_state.get_results().get(selected_index) {
                    match &search_result.item {
                        SearchItem::Pane(pane) => {
                            if let Some(tab_index) = self.get_picker_tab_index() {
                                self.pending_pane_moves.push(pane.id);
                                eprintln!("breaking pane");
                                break_panes_to_tab_with_index(&[pane.id], tab_index, true);
                            }
                        },
                        SearchItem::File(file) => {
                            open_file(
                                FileToOpen::new(self.app_state.get_cwd().join(file)),
                                Default::default(),
                            );
                            close_self();
                        },
                        SearchItem::RustAsset(rust_asset) => {
                            let mut file_to_open = FileToOpen::new(self.app_state.get_cwd().join(&rust_asset.file_path));
                            file_to_open.line_number = Some(rust_asset.line_number);
                            open_file(
                                file_to_open,
                                Default::default(),
                            );
                            close_self();
                        },
                        SearchItem::ShellCommand { command, shell, .. } => {
                            // Execute the shell command in a new terminal pane
                            let command_to_run = CommandToRun {
                                path: PathBuf::from(shell),
                                args: vec!["-ic".to_owned(), command.to_string()],
                                cwd: Some(self.app_state.get_cwd().clone()),
                            };
                            open_command_pane(
                                command_to_run,
                                Default::default(),
                            );
                            close_self();
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
