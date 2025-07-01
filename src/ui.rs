use zellij_tile::prelude::*;
use std::path::PathBuf;
use crate::search::{SearchResult, SearchItem};
use crate::pane::PaneMetadata;
use crate::files::TypeKind;

#[derive(Default)]
pub struct UIRenderer;

impl UIRenderer {
    pub fn render_plugin(
        &self,
        rows: usize,
        cols: usize,
        search_term: &str,
        panes: &[PaneMetadata],
        search_results: &[SearchResult],
        selected_index: Option<usize>,
        scroll_offset: usize,
        displayed_files: &[PathBuf],
        remaining_files: usize,
        cwd: &PathBuf,
    ) {
        let base_x = 1;
        let base_y = 0;

        let search_display = format!("{}_", search_term);
        let max_search_width = cols.saturating_sub(4);
        let truncated_search = truncate_middle(&search_display, max_search_width);
        let search_text = Text::new(&truncated_search).color_all(3);

        // Create current directory display
        let cwd_display = format!("Current Folder: {} (Ctrl f to change)", cwd.display());
        let max_cwd_width = cols.saturating_sub(4);
        let truncated_cwd = truncate_middle(&cwd_display, max_cwd_width);
        
        // Apply colors to different parts of the text
        let folder_prefix = "Current Folder: ";
        let ctrl_suffix = "Ctrl f";
        
        let mut cwd_text = Text::new(&truncated_cwd);
        
        // Color "Current Folder: " with color index 2
        cwd_text = cwd_text.color_substring(2, folder_prefix);
        
        // Color "(Ctrl f to change)" with color index 3
        cwd_text = cwd_text.color_substring(3, ctrl_suffix);

        let (display_count, is_searching) = if search_term.is_empty() {
            (panes.len(), false)
        } else {
            (search_results.len(), true)
        };

        // Only show empty state if no panes AND not searching
        if panes.is_empty() && !is_searching {
            self.render_empty_state(rows, cols, search_text, cwd_text, base_x, base_y);
            return;
        }

        // Show no results only when searching but found nothing
        if is_searching && search_results.is_empty() {
            self.render_no_results(rows, cols, search_text, cwd_text, base_x, base_y);
            return;
        }

        let available_rows = rows.saturating_sub(8); // Increased to account for cwd line
        let visible_items = available_rows.min(display_count);

        let has_scroll_up = scroll_offset > 0;
        let remaining_items = display_count.saturating_sub(scroll_offset + visible_items);
        let has_scroll_down = remaining_items > 0;

        let scroll_indication_space = if has_scroll_up || has_scroll_down {
            10
        } else {
            0
        };
        let type_column_width = 7;

        let available_title_width = cols.saturating_sub(scroll_indication_space + type_column_width);

        let table_rows = 1;
        let content_rows = if display_count == 0 { 1 } else { visible_items };
        let cwd_y = base_y + 1;
        let search_y = cwd_y + 1;
        let table_y = search_y + 1;

        print_text_with_coordinates(cwd_text, base_x, cwd_y, None, None);
        print_text_with_coordinates(search_text, base_x, search_y, None, None);

        self.render_table(
            table_y,
            base_x,
            visible_items,
            display_count,
            scroll_offset,
            is_searching,
            panes,
            search_results,
            selected_index,
            available_title_width,
            has_scroll_up,
            has_scroll_down,
            remaining_items,
            remaining_files,
            cwd,
        );
    }

    fn render_empty_state(&self, rows: usize, cols: usize, search_text: Text, cwd_text: Text, base_x: usize, base_y: usize) {
        let empty_text = Text::new("No editor panes found - start typing to search files, definitions, and shell history");
        let y = base_y;
        print_text_with_coordinates(cwd_text, base_x, y, None, None);
        print_text_with_coordinates(search_text, base_x, y + 1, None, None);
        print_text_with_coordinates(empty_text, base_x, y + 3, None, None);
    }

    fn render_no_results(&self, rows: usize, cols: usize, search_text: Text, cwd_text: Text, base_x: usize, base_y: usize) {
        let cwd_y = base_y;
        let search_y = cwd_y + 1;
        let table_y = search_y + 1;

        print_text_with_coordinates(cwd_text, base_x, cwd_y, None, None);
        print_text_with_coordinates(search_text, base_x, search_y, None, None);

        let mut table = Table::new().add_row(vec![" ", " ", " "]);
        let empty_text = Text::new("No matching panes, files, definitions, or shell commands found");
        table = table.add_styled_row(vec![Text::new(" "), empty_text, Text::new(" ")]);
        print_table_with_coordinates(table, base_x, table_y, None, None);
    }

    fn render_table(
        &self,
        table_y: usize,
        base_x: usize,
        visible_items: usize,
        display_count: usize,
        scroll_offset: usize,
        is_searching: bool,
        panes: &[PaneMetadata],
        search_results: &[SearchResult],
        selected_index: Option<usize>,
        available_title_width: usize,
        has_scroll_up: bool,
        has_scroll_down: bool,
        remaining_items: usize,
        remaining_files: usize,
        current_cwd: &PathBuf,
    ) {
        let mut table = Table::new().add_row(vec![" ", " ", " "]);

        let end_index = (scroll_offset + visible_items).min(display_count);
        for (row_index, item_index) in (scroll_offset..end_index).enumerate() {
            let (display_text, highlight_indices, item_type) = if is_searching {
                let search_result = &search_results[item_index];
                let item_type = match &search_result.item {
                    SearchItem::Pane(_) => "PANE",
                    SearchItem::File(_) => "FILE",
                    SearchItem::RustAsset(rust_asset) => match rust_asset.type_kind {
                        TypeKind::Struct => "STRUCT",
                        TypeKind::Enum => "ENUM",
                    },
                    SearchItem::ShellCommand { shell, .. } => {
                        match shell.to_uppercase().as_str() {
                            "BASH" => "BASH",
                            "ZSH" => "ZSH", 
                            "FISH" => "FISH",
                            "SH" => "SH",
                            "KSH" => "KSH",
                            _ => "SHELL",
                        }
                    }
                };
                
                let display_text = match &search_result.item {
                    SearchItem::ShellCommand { command, folders, .. } => {
                        // Create display text with folder information
                        self.format_shell_command_display(command, folders, current_cwd)
                    },
                    _ => search_result.display_text(),
                };
                
                (
                    display_text,
                    Some(&search_result.indices),
                    item_type,
                )
            } else {
                // Only access panes if we're not searching and have panes
                if let Some(pane) = panes.get(item_index) {
                    (pane.title.clone(), None, "PANE")
                } else {
                    continue; // Skip if no pane at this index
                }
            };

            let is_selected = selected_index == Some(item_index);

            let truncated_title = truncate_middle(&display_text, available_title_width);

            let mut type_cell = if is_selected {
                Text::new(item_type).selected()
            } else {
                Text::new(item_type)
            };

            let color_index = match item_type {
                "PANE" => 0,
                "FILE" => 1,
                "STRUCT" | "ENUM" => 2,
                "BASH" | "ZSH" | "FISH" | "SH" | "KSH" | "SHELL" => 4,
                _ => 0,
            };
            type_cell = type_cell.color_all(color_index);

            let mut filename_cell = if is_selected {
                Text::new(&truncated_title).selected()
            } else {
                Text::new(&truncated_title)
            };

            if let Some(indices) = highlight_indices {
                let valid_indices: Vec<usize> = indices
                    .iter()
                    .filter(|&&i| i < truncated_title.chars().count())
                    .copied()
                    .collect();
                if !valid_indices.is_empty() {
                    filename_cell = filename_cell.color_indices(3, valid_indices);
                }
            }

            let third_column = if row_index == 0 && has_scroll_up {
                let indicator_text = format!("↑ {} more", scroll_offset);
                Text::new(&indicator_text).color_all(1)
            } else if row_index == (end_index.saturating_sub(scroll_offset)).saturating_sub(1)
                && has_scroll_down
            {
                let indicator_text = format!("↓ {} more", remaining_items);
                Text::new(&indicator_text).color_all(1)
            } else {
                Text::new(" ")
            };

            table = table.add_styled_row(vec![type_cell, filename_cell, third_column.clone()]);
        }

        if is_searching && scroll_offset == 0 && remaining_files > 0 {
            let more_files_text = format!("+{} files", remaining_files);
            let more_files_cell = Text::new(&more_files_text).color_all(1);
            table = table.add_styled_row(vec![Text::new(" "), more_files_cell, Text::new(" ")]);
        }

        print_table_with_coordinates(table, base_x, table_y, None, None);
    }

    /// Format shell command display text with folder information
    fn format_shell_command_display(&self, command: &str, folders: &[String], current_cwd: &PathBuf) -> String {
        let current_cwd_str = current_cwd.to_string_lossy().to_string();
        
        if folders.is_empty() {
            return command.to_string();
        }
        
        // Check if current directory is in the folders list
        let has_current_dir = folders.contains(&current_cwd_str);
        
        if folders.len() == 1 {
            // Single folder case
            let folder = &folders[0];
            if folder == &current_cwd_str {
                format!("{} (here)", command)
            } else if folder == "unknown" {
                command.to_string()
            } else {
                let folder_display = self.truncate_folder_path(folder);
                format!("{} ({})", command, folder_display)
            }
        } else {
            // Multiple folders case
            if has_current_dir {
                let other_count = folders.len().saturating_sub(1);
                if other_count == 1 {
                    format!("{} (here +1 other)", command)
                } else {
                    format!("{} (here +{} others)", command, other_count)
                }
            } else {
                format!("{} ({} folders)", command, folders.len())
            }
        }
    }

    /// Truncate folder path for display (show last 2 components)
    fn truncate_folder_path(&self, path: &str) -> String {
        let path_buf = PathBuf::from(path);
        let components: Vec<_> = path_buf.components().collect();
        
        if components.len() <= 2 {
            path.to_string()
        } else {
            let last_two: Vec<String> = components
                .iter()
                .rev()
                .take(2)
                .rev()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect();
            format!(".../{}", last_two.join("/"))
        }
    }
}

pub fn truncate_middle(text: &str, max_width: usize) -> String {
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
    let right_part: String = chars
        .iter()
        .skip(total_chars.saturating_sub(right_chars))
        .collect();

    format!("{}{}{}", left_part, ellipsis, right_part)
}
