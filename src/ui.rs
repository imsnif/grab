use zellij_tile::prelude::*;
use std::path::PathBuf;
use crate::search::{SearchResult, SearchItem};
use crate::pane::PaneMetadata;
use crate::{RustAssetSearchMode, parse_rust_asset_search};
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
        _shell_commands_results: &[SearchResult],
        selected_index: Option<usize>,
        scroll_offset: usize,
        _displayed_files: &[PathBuf],
        _remaining_files: usize,
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
        let table_y = search_y + 1;

        print_text_with_coordinates(cwd_text, base_x, cwd_y, None, None);
        print_text_with_coordinates(search_text, base_x, search_y, None, None);

        let available_rows = rows.saturating_sub(table_y + 3); // Reserve space for hint line

        self.render_single_table(
            table_y,
            base_x,
            cols,
            available_rows,
            search_term,
            panes,
            files_panes_results,
            selected_index,
            scroll_offset,
            _remaining_files,
            cwd,
        );

        // Render hint line at the bottom
        let hint_y = rows.saturating_sub(1);
        let hint_text = "Hint: start your search with 'struct', 'fn' or 'enum' to look for rust assets";
        let max_hint_width = cols.saturating_sub(2);
        let truncated_hint = truncate_middle(hint_text, max_hint_width);
        let hint_display = Text::new(&truncated_hint).color_substring(3, "Hint:");
        print_text_with_coordinates(hint_display, base_x, hint_y, None, None);
    }

    fn render_single_table(
        &self,
        start_y: usize,
        base_x: usize,
        cols: usize,
        available_rows: usize,
        search_term: &str,
        _panes: &[PaneMetadata],
        files_panes_results: &[SearchResult],
        selected_index: Option<usize>,
        scroll_offset: usize,
        _remaining_files: usize,
        _current_cwd: &PathBuf,
    ) {
        // Check if we're in Rust asset search mode
        let filtered_results: Vec<SearchResult> = if let Some(rust_mode) = parse_rust_asset_search(search_term) {
            // Show only matching Rust assets
            files_panes_results
                .iter()
                .filter(|result| {
                    if let SearchItem::RustAsset(rust_asset) = &result.item {
                        match &rust_mode {
                            RustAssetSearchMode::Struct(_) => matches!(rust_asset.type_kind, TypeKind::Struct),
                            RustAssetSearchMode::Enum(_) => matches!(rust_asset.type_kind, TypeKind::Enum),
                            RustAssetSearchMode::Function(_) => matches!(rust_asset.type_kind, TypeKind::Function | TypeKind::PubFunction),
                            RustAssetSearchMode::PubFunction(_) => matches!(rust_asset.type_kind, TypeKind::PubFunction),
                        }
                    } else {
                        false
                    }
                })
                .cloned()
                .collect()
        } else {
            // Normal mode: filter out Rust assets - only show panes and files
            files_panes_results
                .iter()
                .filter(|result| matches!(result.item, SearchItem::Pane(_) | SearchItem::File(_)))
                .cloned()
                .collect()
        };

        let total_items = filtered_results.len();

        if !search_term.is_empty() && filtered_results.is_empty() {
            self.render_no_results(start_y, base_x, search_term);
            return;
        }

        let scroll_indication_space = 10;
        let type_column_width = 7;
        let available_title_width = cols.saturating_sub(scroll_indication_space + type_column_width);

        self.render_table(
            start_y,
            base_x,
            available_rows,
            &filtered_results,
            0, // Table starts at index 0
            selected_index,
            scroll_offset,
            available_title_width,
            total_items,
            false, // is_shell_commands
            _current_cwd,
        );
    }

    fn render_no_results(&self, start_y: usize, base_x: usize, search_term: &str) {
        let message = if let Some(mode) = parse_rust_asset_search(search_term) {
            match mode {
                RustAssetSearchMode::Struct(_) => "No matching structs found",
                RustAssetSearchMode::Enum(_) => "No matching enums found",
                RustAssetSearchMode::Function(_) => "No matching functions found",
                RustAssetSearchMode::PubFunction(_) => "No matching public functions found",
            }
        } else {
            "No matching panes or files found"
        };
        let empty_text = Text::new(message);
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
        _current_cwd: &PathBuf,
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
                            TypeKind::Function => "FN",
                            TypeKind::PubFunction => "PUB FN",
                        };
                        (display_text, Some(&search_result.indices), item_type)
                    },
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
                    "STRUCT" | "ENUM" | "FN" | "PUB FN" => 2,
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

                // Show scroll indicators or shortcut in the third column
                let third_column = if item_index == global_start && scroll_offset > 0 {
                    let indicator_text = format!("↑ {} more", scroll_offset);
                    Text::new(&indicator_text).color_all(1)
                } else if item_index == global_start + actual_visible.saturating_sub(1) && 
                         scroll_offset + visible_rows < total_items {
                    let remaining = total_items.saturating_sub(scroll_offset + visible_rows);
                    let indicator_text = format!("↓ {} more", remaining);
                    Text::new(&indicator_text).color_all(1)
                } else if is_selected {
                    Text::new(" <Enter>").color_all(3)
                } else {
                    Text::new(" ")
                };

                table = table.add_styled_row(vec![type_cell, filename_cell, third_column]);
            }
        }

        print_table_with_coordinates(table, base_x, table_y, None, None);
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
