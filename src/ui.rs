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
    ) {
        let search_display = format!("{}_", search_term);
        let max_search_width = cols.saturating_sub(4);
        let truncated_search = truncate_middle(&search_display, max_search_width);
        let search_text = Text::new(&truncated_search).color_all(3);
        let search_width = search_text.len();

        if panes.is_empty() {
            self.render_empty_state(rows, cols, search_text);
            return;
        }

        let total_panes = panes.len();
        let available_rows = rows.saturating_sub(6);
        let max_visible_items = available_rows.min(total_panes);

        let (display_count, is_searching) = if search_term.is_empty() {
            (panes.len(), false)
        } else {
            (search_results.len(), true)
        };
        let visible_items = available_rows.min(display_count);

        if !search_term.is_empty() && search_results.is_empty() {
            self.render_no_results(rows, cols, search_text);
            return;
        }

        let has_scroll_up = scroll_offset > 0;
        let remaining_items = display_count.saturating_sub(scroll_offset + visible_items);
        let has_scroll_down = remaining_items > 0;

        let scroll_indication_space = if has_scroll_up || has_scroll_down {
            10
        } else {
            0
        };
        let type_column_width = 7; // Increased to accommodate "STRUCT" and "ENUM"

        let available_title_width = cols.saturating_sub(scroll_indication_space + type_column_width);
        let mut max_ui_width = search_width;

        for pane in panes {
            let truncated_title = truncate_middle(&pane.title, available_title_width);
            let line_width = type_column_width + truncated_title.chars().count() + scroll_indication_space;
            max_ui_width = max_ui_width.max(line_width);
        }

        if is_searching {
            for result in search_results {
                let display_text = result.display_text();
                let truncated_text = truncate_middle(&display_text, available_title_width);
                let line_width = type_column_width + truncated_text.chars().count() + scroll_indication_space;
                max_ui_width = max_ui_width.max(line_width);
            }

            for file in displayed_files {
                let file_display = file.to_string_lossy();
                let truncated_file = truncate_middle(&file_display, available_title_width);
                let line_width = type_column_width + truncated_file.chars().count() + scroll_indication_space;
                max_ui_width = max_ui_width.max(line_width);
            }

            if remaining_files > 0 {
                let more_files_text = format!("+{} files", remaining_files);
                let line_width = type_column_width + more_files_text.chars().count() + scroll_indication_space;
                max_ui_width = max_ui_width.max(line_width);
            }
        }

        let ui_width = max_ui_width.min(cols);
        let ui_x = (cols.saturating_sub(ui_width)) / 2;

        let table_rows = 1;
        let content_rows = if total_panes == 0 { 1 } else { max_visible_items };
        let total_ui_height = 2 + table_rows + content_rows;
        let search_y = (rows.saturating_sub(total_ui_height)) / 2 + 1;
        let table_y = search_y + 1;

        print_text_with_coordinates(search_text, ui_x, search_y, None, None);

        self.render_table(
            table_y,
            ui_x,
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
        );
    }

    fn render_empty_state(&self, rows: usize, cols: usize, search_text: Text) {
        let empty_text = Text::new("No editor panes found");
        let text_width = empty_text.content().chars().count();
        let search_term_width = search_text.content().chars().count();
        let ui_width = std::cmp::max(text_width, search_term_width);
        let x = (cols.saturating_sub(ui_width)) / 2;
        let y = rows / 2;
        print_text_with_coordinates(search_text, x, y, None, None);
        print_text_with_coordinates(empty_text, x, y + 2, None, None);
    }

    fn render_no_results(&self, rows: usize, cols: usize, search_text: Text) {
        let ui_x = (cols.saturating_sub(search_text.len())) / 2;
        let search_y = rows / 2;
        let table_y = search_y + 1;

        print_text_with_coordinates(search_text, ui_x, search_y, None, None);

        let mut table = Table::new().add_row(vec![" ", " ", " "]);
        let empty_text = Text::new("No matching panes, files, or definitions found");
        table = table.add_styled_row(vec![Text::new(" "), empty_text, Text::new(" ")]);
        print_table_with_coordinates(table, ui_x, table_y, None, None);
    }

    fn render_table(
        &self,
        table_y: usize,
        ui_x: usize,
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
                    }
                };
                (
                    search_result.display_text(),
                    Some(&search_result.indices),
                    item_type,
                )
            } else {
                (panes[item_index].title.clone(), None, "PANE")
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

        print_table_with_coordinates(table, ui_x, table_y, None, None);
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
