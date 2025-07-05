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
        files_panes_results: &[SearchResult],
        shell_commands_results: &[SearchResult],
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

        let cwd_display = format!("Current Folder: {} (Ctrl f to change)", cwd.display());
        let max_cwd_width = cols.saturating_sub(4);
        let truncated_cwd = truncate_middle(&cwd_display, max_cwd_width);
        
        let folder_prefix = "Current Folder: ";
        let ctrl_suffix = "Ctrl f";
        
        let mut cwd_text = Text::new(&truncated_cwd);
        cwd_text = cwd_text.color_substring(2, folder_prefix);
        cwd_text = cwd_text.color_substring(3, ctrl_suffix);

        let cwd_y = base_y;
        let search_y = cwd_y + 1;
        let table1_y = search_y + 1;

        print_text_with_coordinates(cwd_text, base_x, cwd_y, None, None);
        print_text_with_coordinates(search_text, base_x, search_y, None, None);

        let available_rows = rows.saturating_sub(table1_y);
        let table1_rows = available_rows / 2;
        let table2_rows = available_rows.saturating_sub(table1_rows + 1); // +1 for separator

        self.render_dual_tables(
            table1_y,
            base_x,
            cols,
            table1_rows,
            table2_rows,
            search_term,
            panes,
            files_panes_results,
            shell_commands_results,
            selected_index,
            scroll_offset,
            remaining_files,
            cwd,
        );
    }

    fn render_dual_tables(
        &self,
        start_y: usize,
        base_x: usize,
        cols: usize,
        table1_rows: usize,
        table2_rows: usize,
        search_term: &str,
        panes: &[PaneMetadata],
        files_panes_results: &[SearchResult],
        shell_commands_results: &[SearchResult],
        selected_index: Option<usize>,
        scroll_offset: usize,
        remaining_files: usize,
        current_cwd: &PathBuf,
    ) {
        let table1_count = files_panes_results.len();
        let table2_count = shell_commands_results.len();
        let total_items = table1_count + table2_count;

        if panes.is_empty() && !search_term.is_empty() && files_panes_results.is_empty() && shell_commands_results.is_empty() {
            self.render_no_results(start_y, base_x);
            return;
        }

        let scroll_indication_space = 10;
        let type_column_width = 7;
        let available_title_width = cols.saturating_sub(scroll_indication_space + type_column_width);

        // Table 1: Files/Panes/Rust
        let table1_y = start_y;

        let table1_content_y = table1_y;
        let table1_content_rows = table1_rows.saturating_sub(1);

        self.render_table(
            table1_content_y,
            base_x,
            table1_content_rows,
            files_panes_results,
            0, // Table 1 starts at index 0
            selected_index,
            scroll_offset,
            available_title_width,
            total_items,
            false, // is_shell_commands
            current_cwd,
        );

        // Table 2: Shell Commands
        let table2_y = table1_y + table1_rows;

        let table2_content_y = table2_y + 1;
        let table2_content_rows = table2_rows.saturating_sub(1);

        self.render_table(
            table2_content_y,
            base_x,
            table2_content_rows,
            shell_commands_results,
            table1_count, // Table 2 starts after table 1
            selected_index,
            scroll_offset,
            available_title_width,
            total_items,
            true, // is_shell_commands
            current_cwd,
        );
    }

    fn render_no_results(&self, start_y: usize, base_x: usize) {
        let empty_text = Text::new("No matching panes, files, definitions, or shell commands found");
        print_text_with_coordinates(empty_text, base_x, start_y + 2, None, None);
    }

    fn render_table(
        &self,
        table_y: usize,
        base_x: usize,
        visible_rows: usize,
        results: &[SearchResult],
        table_start_index: usize,
        selected_index: Option<usize>,
        scroll_offset: usize,
        available_title_width: usize,
        total_items: usize,
        is_shell_commands: bool,
        current_cwd: &PathBuf,
    ) {
        if results.is_empty() {
            let empty_message = if is_shell_commands {
                "No Commands"
            } else {
                "No Panes or Files"
            };
            let empty_text = Text::new(empty_message).color_all(1);
            print_text_with_coordinates(empty_text, base_x, table_y + 1, None, None); // + 1 to
                                                                                      // account
                                                                                      // fot the
                                                                                      // table
                                                                                      // title
            return;
        }

        let mut table = Table::new().add_row(vec![" ".to_owned(), " ".to_owned(), " ".to_owned()]);

        // Calculate visible range considering the global scroll offset
        let global_start = if scroll_offset > table_start_index {
            scroll_offset.saturating_sub(table_start_index)
        } else {
            0
        };
        
        let global_end = if scroll_offset + visible_rows > table_start_index {
            (scroll_offset + visible_rows).saturating_sub(table_start_index).min(results.len())
        } else {
            0
        };

        let items_to_show = global_end.saturating_sub(global_start);
        let actual_visible = items_to_show.min(visible_rows);

        for item_index in global_start..global_start + actual_visible {
            if let Some(search_result) = results.get(item_index) {
                let global_index = table_start_index + item_index;
                let is_selected = selected_index == Some(global_index);

                let (display_text, highlight_indices, item_type) = match &search_result.item {
                    SearchItem::Pane(_) => {
                        let display_text = search_result.display_text();
                        (display_text, Some(&search_result.indices), "PANE")
                    },
                    SearchItem::File(_) => {
                        let display_text = search_result.display_text();
                        (display_text, Some(&search_result.indices), "FILE")
                    },
                    SearchItem::RustAsset(rust_asset) => {
                        let display_text = search_result.display_text();
                        let item_type = match rust_asset.type_kind {
                            TypeKind::Struct => "STRUCT",
                            TypeKind::Enum => "ENUM",
                        };
                        (display_text, Some(&search_result.indices), item_type)
                    },
                    SearchItem::ShellCommand { command, folders, shell } => {
                        let display_text = self.format_shell_command_display(command, folders, current_cwd);
                        let item_type = match shell.to_uppercase().as_str() {
                            "BASH" => "BASH",
                            "ZSH" => "ZSH", 
                            "FISH" => "FISH",
                            "SH" => "SH",
                            "KSH" => "KSH",
                            _ => "SHELL",
                        };
                        (display_text, Some(&search_result.indices), item_type)
                    }
                };

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

                // Show scroll indicators in the third column
                let third_column = if item_index == global_start && scroll_offset > 0 {
                    let indicator_text = format!("↑ {} more", scroll_offset);
                    Text::new(&indicator_text).color_all(1)
                } else if item_index == global_start + actual_visible.saturating_sub(1) && 
                         scroll_offset + visible_rows < total_items {
                    let remaining = total_items.saturating_sub(scroll_offset + visible_rows);
                    let indicator_text = format!("↓ {} more", remaining);
                    Text::new(&indicator_text).color_all(1)
                } else {
                    Text::new(" ")
                };

                table = table.add_styled_row(vec![type_cell, filename_cell, third_column]);
            }
        }

        print_table_with_coordinates(table, base_x, table_y, None, None);
    }

    fn format_shell_command_display(&self, command: &str, folders: &[String], current_cwd: &PathBuf) -> String {
        let current_cwd_str = current_cwd.to_string_lossy().to_string();
        
        if folders.is_empty() {
            return command.to_string();
        }
        
        let has_current_dir = folders.contains(&current_cwd_str);
        
        if folders.len() == 1 {
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
