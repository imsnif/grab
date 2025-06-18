use zellij_tile::prelude::*;

use std::collections::BTreeMap;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

#[derive(Default)]
struct State {
    pane_metadata: Vec<PaneMetadata>,
    selected_index: Option<usize>,
    scroll_offset: usize,
    last_rows: usize,
    search_term: String,
    search_results: Vec<SearchResult>,
}

#[derive(Debug, Clone)]
struct SearchResult {
    pane: PaneMetadata,
    score: i64,
    indices: Vec<usize>,
}

impl SearchResult {
    fn new(pane: PaneMetadata, score: i64, indices: Vec<usize>) -> Self {
        SearchResult { pane, score, indices }
    }
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);
        subscribe(&[EventType::PaneUpdate, EventType::Key, EventType::PermissionRequestResult]);
//         // Mock data loop - paste this into the load() function for testing
//         // Comment out the subscribe() call when using this
//         for i in 1..=50 {
//             let mock_pane = PaneMetadata {
//                 id: PaneId::Terminal(i),
//                 title: match i % 10 {
//                     1 => format!("nvim src/main.rs - Project {}", i),
//                     2 => format!("vim ~/.config/zellij/config.kdl - Config {}", i),
//                     3 => format!("helix components/table.rs - Component {}", i),
//                     4 => format!("code workspace/frontend/app.tsx - Frontend {}", i),
//                     5 => format!("emacs ~/.bashrc - Shell Config {}", i),
//                     6 => format!("nano README.md - Documentation {}", i),
//                     7 => format!("micro scripts/deploy.sh - Deployment {}", i),
//                     8 => format!("nvim tests/integration_test.rs - Tests {}", i),
//                     9 => format!("vim Cargo.toml - Dependencies {}", i),
//                     0 => format!("hx very/long/path/to/some/deeply/nested/file/with/a/really/long/filename.rs - Deep File {}", i),
//                     _ => unreachable!(),
//                 },
//             };
//             self.pane_metadata.push(mock_pane);
//         }
        self.update_search_results();
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::PermissionRequestResult(_) => {
                let own_plugin_id = get_plugin_ids().plugin_id;
                rename_plugin_pane(own_plugin_id, "Editor panes");
            }
            Event::PaneUpdate(pane_manifest) => {
                self.pane_metadata = extract_editor_pane_metadata(&pane_manifest);
                self.adjust_selection_after_pane_update();
                self.update_search_results();
                should_render = true;
            }
            Event::Key(key) => {
                match key.bare_key {
                    BareKey::Down if key.has_no_modifiers() => {
                        self.move_selection_down();
                        should_render = true;
                    }
                    BareKey::Up if key.has_no_modifiers() => {
                        self.move_selection_up();
                        should_render = true;
                    }
                    BareKey::Enter if key.has_no_modifiers() => {
                        self.focus_selected_pane();
                    }
                    BareKey::Char(character) if key.has_no_modifiers() => {
                        self.search_term.push(character);
                        self.update_search_results();
                        should_render = true;
                    }
                    BareKey::Backspace if key.has_no_modifiers() => {
                        self.search_term.pop();
                        self.update_search_results();
                        should_render = true;
                    }
                    BareKey::Char('c') if key.has_modifiers(&[KeyModifier::Ctrl]) => {
                        if self.search_term.is_empty() {
                            close_self();
                        } else {
                            self.search_term.clear();
                            should_render = true;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        should_render
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        false
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.last_rows = rows;
        
        // Prepare search line with truncation
        let search_display = format!("{}_", self.search_term);
        let max_search_width = cols.saturating_sub(4);
        let truncated_search = truncate_middle(&search_display, max_search_width);
        let search_text = Text::new(&truncated_search).color_all(3);
        let search_width = search_text.len();

        // Early return for no panes at all
        if self.pane_metadata.is_empty() {
            let empty_text = Text::new("No editor panes found");
            let text_width = empty_text.content().chars().count();
            let search_term_width = search_text.content().chars().count();
            let ui_width = std::cmp::max(text_width, search_term_width);
            let x = (cols.saturating_sub(ui_width)) / 2;
            let y = rows / 2;
            print_text_with_coordinates(
                search_text,
                x,
                y,
                None,
                None
            );
            print_text_with_coordinates(
                empty_text,
                x,
                y + 2,
                None,
                None
            );
            return;
        }

        // Always calculate based on total panes for consistent positioning
        let total_panes = self.pane_metadata.len();
        let available_rows = rows.saturating_sub(6);
        let max_visible_items = available_rows.min(total_panes);
        
        // Determine which items to show and how many (for actual rendering)
        let (display_count, is_searching) = if self.search_term.is_empty() {
            (self.pane_metadata.len(), false)
        } else {
            (self.search_results.len(), true)
        };
        let visible_items = available_rows.min(display_count);

        let has_scroll_up = self.scroll_offset > 0;
        let remaining_items = display_count.saturating_sub(self.scroll_offset + visible_items);
        let has_scroll_down = remaining_items > 0;

        let scroll_indication_space = if has_scroll_up || has_scroll_down { 10 } else { 0 };

        // Calculate the maximum possible UI width based on ALL panes (not just visible ones)
        let available_title_width = cols.saturating_sub(scroll_indication_space);
        let mut max_ui_width = search_width;
        
        for pane in &self.pane_metadata {
            let truncated_title = truncate_middle(&pane.title, available_title_width);
            let line_width = truncated_title.chars().count() + 1 + scroll_indication_space;
            max_ui_width = max_ui_width.max(line_width);
        }
        
        let ui_width = max_ui_width.min(cols);
        let ui_x = (cols.saturating_sub(ui_width)) / 2;
        
        // Calculate Y positioning based on TOTAL panes (for consistency)
        let table_rows = 1; // Title row
        let content_rows = if total_panes == 0 { 1 } else { max_visible_items };
        let total_ui_height = 2 + table_rows + content_rows; // search + padding + table header + content
        let search_y = (rows.saturating_sub(total_ui_height)) / 2 + 1;
        let table_y = search_y + 1;
        
        // Always render search line at consistent position
        print_text_with_coordinates(search_text, ui_x, search_y, None, None);
        
        // Handle empty search results case with consistent positioning
        if !self.search_term.is_empty() && self.search_results.is_empty() {
            let mut table = Table::new().add_row(vec![" ", " "]);
            let empty_text = Text::new("No matching panes found");
            table = table.add_styled_row(vec![empty_text, Text::new(" ")]);
            print_table_with_coordinates(table, ui_x, table_y, None, None);
            return;
        }
        
        self.adjust_scroll_for_selection(visible_items, is_searching);
        
        // Start with empty title row (2 columns: filename, empty)
        let mut table = Table::new().add_row(vec![" ", " "]);
        
        // Add visible pane rows
        let end_index = (self.scroll_offset + visible_items).min(display_count);
        for (row_index, item_index) in (self.scroll_offset..end_index).enumerate() {
            let (pane, highlight_indices) = if is_searching {
                let search_result = &self.search_results[item_index];
                (&search_result.pane, Some(&search_result.indices))
            } else {
                (&self.pane_metadata[item_index], None)
            };
            
            let is_selected = self.selected_index == Some(item_index);
            
            let truncated_title = truncate_middle(&pane.title, available_title_width);
            
            let mut filename_cell = if is_selected {
                Text::new(&truncated_title).selected()
            } else {
                Text::new(&truncated_title)
            };
            
            // Apply fuzzy match highlighting if we have indices
            if let Some(indices) = highlight_indices {
                let valid_indices: Vec<usize> = indices.iter()
                    .filter(|&&i| i < truncated_title.chars().count())
                    .copied()
                    .collect();
                if !valid_indices.is_empty() {
                    filename_cell = filename_cell.color_indices(3, valid_indices);
                }
            }
            
            // Determine what goes in the second column
            let second_column = if row_index == 0 && has_scroll_up {
                let indicator_text = format!("↑ {} more", self.scroll_offset);
                Text::new(&indicator_text).color_all(1)
            } else if row_index == (end_index.saturating_sub(self.scroll_offset)).saturating_sub(1) && has_scroll_down {
                let indicator_text = format!("↓ {} more", remaining_items);
                Text::new(&indicator_text).color_all(1)
            } else {
                Text::new(" ")
            };
            
            table = table.add_styled_row(vec![
                filename_cell,
                second_column.clone(),
            ]);
        }
        
        // Render table at consistent position
        print_table_with_coordinates(table, ui_x, table_y, None, None);
    }
}

impl State {
    fn update_search_results(&mut self) {
        if self.search_term.is_empty() {
            self.search_results.clear();
            self.selected_index = None;
            return;
        }
        
        let mut matches = vec![];
        let matcher = SkimMatcherV2::default().use_cache(true);
        
        for pane in &self.pane_metadata {
            if let Some((score, indices)) = matcher.fuzzy_indices(&pane.title, &self.search_term) {
                matches.push(SearchResult::new(pane.clone(), score, indices));
            }
        }
        
        matches.sort_by(|a, b| b.score.cmp(&a.score));
        self.search_results = matches;
        
        if !self.search_results.is_empty() {
            self.selected_index = Some(0);
        } else {
            self.selected_index = None;
        }
    }
    
    fn move_selection_down(&mut self) {
        let items_count = if self.search_term.is_empty() {
            self.pane_metadata.len()
        } else {
            self.search_results.len()
        };
        
        if items_count == 0 {
            return;
        }
        
        match self.selected_index {
            None => {
                self.selected_index = Some(0);
            }
            Some(current) => {
                if current + 1 < items_count {
                    self.selected_index = Some(current + 1);
                } else {
                    self.selected_index = None;
                }
            }
        }
    }
    
    fn move_selection_up(&mut self) {
        let items_count = if self.search_term.is_empty() {
            self.pane_metadata.len()
        } else {
            self.search_results.len()
        };
        
        if items_count == 0 {
            return;
        }
        
        match self.selected_index {
            None => {
                self.selected_index = Some(items_count.saturating_sub(1));
            }
            Some(current) => {
                if current > 0 {
                    self.selected_index = Some(current.saturating_sub(1));
                } else {
                    self.selected_index = None;
                }
            }
        }
    }
    
    fn focus_selected_pane(&mut self) {
        if let Some(selected_index) = self.selected_index {
            let pane = if self.search_term.is_empty() {
                self.pane_metadata.get(selected_index)
            } else {
                self.search_results.get(selected_index).map(|sr| &sr.pane)
            };
            
            if let Some(pane) = pane {
                match pane.id {
                    PaneId::Terminal(terminal_id) => {
                        focus_terminal_pane(terminal_id, true);
                        close_self();
                    }
                    PaneId::Plugin(plugin_id) => {
                        focus_plugin_pane(plugin_id, true);
                        close_self();
                    }
                }
            }
        }
    }
    
    fn adjust_selection_after_pane_update(&mut self) {
        let items_count = if self.search_term.is_empty() {
            self.pane_metadata.len()
        } else {
            self.search_results.len()
        };
        
        if let Some(selected) = self.selected_index {
            if selected >= items_count {
                self.selected_index = if items_count == 0 {
                    None
                } else {
                    Some(items_count.saturating_sub(1))
                };
            }
        }
    }
    
    fn adjust_scroll_for_selection(&mut self, visible_items: usize, is_searching: bool) {
        let items_count = if is_searching {
            self.search_results.len()
        } else {
            self.pane_metadata.len()
        };
        
        if let Some(selected) = self.selected_index {
            let center_position = visible_items / 2;
            let ideal_scroll_offset = selected.saturating_sub(center_position);
            let max_scroll = items_count.saturating_sub(visible_items);
            self.scroll_offset = ideal_scroll_offset.min(max_scroll);
        } else {
            let max_scroll = items_count.saturating_sub(visible_items);
            if self.scroll_offset > max_scroll {
                self.scroll_offset = max_scroll;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PaneMetadata {
    pub id: PaneId,
    pub title: String,
}

pub fn extract_editor_pane_metadata(manifest: &PaneManifest) -> Vec<PaneMetadata> {
    let mut result = Vec::new();
    
    for (_, panes) in &manifest.panes {
        for pane_info in panes {
            if is_editor_pane(pane_info) {
                let pane_id = if pane_info.is_plugin {
                    PaneId::Plugin(pane_info.id)
                } else {
                    PaneId::Terminal(pane_info.id)
                };
                
                result.push(PaneMetadata {
                    id: pane_id,
                    title: pane_info.title.clone(),
                });
            }
        }
    }
    
    result.sort_by(|a, b| a.title.cmp(&b.title));
    result
}

fn is_editor_pane(pane_info: &PaneInfo) -> bool {
    let common_editors = [
        "vim", "nvim", "neovim", "vi",
        "emacs", "nano", "micro", "helix", "hx",
        "code", "subl", "atom", "notepad",
        "kak", "kakoune", "joe", "mcedit",
        "ed", "ex", "pico"
    ];
    
    if let Some(ref command) = pane_info.terminal_command {
        let command_lower = command.to_lowercase();
        if common_editors.iter().any(|&editor| {
            command_lower.contains(editor) || 
            command_lower.starts_with(&format!("{} ", editor)) ||
            command_lower.ends_with(&format!("/{}", editor))
        }) {
            return true;
        }
    }
    
    let title_lower = pane_info.title.to_lowercase();
    common_editors.iter().any(|&editor| {
        title_lower.contains(editor) ||
        title_lower.starts_with(&format!("{} ", editor)) ||
        title_lower.contains(&format!(" {} ", editor)) ||
        title_lower.ends_with(&format!(" {}", editor))
    })
}

fn truncate_middle(text: &str, max_width: usize) -> String {
    if text.chars().count() <= max_width {
        return text.to_string();
    }
    
    if max_width < 3 {
        return "...".chars().take(max_width).collect();
    }
    
    let ellipsis = "...";
    let available_chars = max_width.saturating_sub(ellipsis.len());
    let left_chars = available_chars / 2;
    let right_chars = available_chars.saturating_sub(left_chars);
    
    let chars: Vec<char> = text.chars().collect();
    let total_chars = chars.len();
    
    let left_part: String = chars.iter().take(left_chars).collect();
    let right_part: String = chars.iter()
        .skip(total_chars.saturating_sub(right_chars))
        .collect();
    
    format!("{}{}{}", left_part, ellipsis, right_part)
}
