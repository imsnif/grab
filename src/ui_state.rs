#[derive(Default)]
pub struct UIState {
    pub selected_index: Option<usize>,
    pub scroll_offset: usize,
    pub last_rows: usize,
}

impl UIState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_selected_index(&mut self, index: Option<usize>) {
        self.selected_index = index;
    }

    pub fn move_selection_down(&mut self, table1_count: usize, table2_count: usize) {
        let total_items = table1_count + table2_count;
        
        if total_items == 0 {
            return;
        }

        match self.selected_index {
            None => {
                self.selected_index = Some(0);
            }
            Some(current) => {
                if current + 1 < total_items {
                    self.selected_index = Some(current + 1);
                } else {
                    self.selected_index = None;
                }
            }
        }
    }

    pub fn move_selection_up(&mut self, table1_count: usize, table2_count: usize) {
        let total_items = table1_count + table2_count;
        
        if total_items == 0 {
            return;
        }

        match self.selected_index {
            None => {
                self.selected_index = Some(total_items.saturating_sub(1));
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

    pub fn adjust_selection_after_update(&mut self, table1_count: usize, table2_count: usize) {
        let total_items = table1_count + table2_count;
        
        if let Some(selected) = self.selected_index {
            if selected >= total_items {
                self.selected_index = if total_items == 0 {
                    None
                } else {
                    Some(total_items.saturating_sub(1))
                };
            }
        }
    }

    pub fn adjust_scroll_for_selection(&mut self, visible_items: usize, table1_count: usize, table2_count: usize) {
        let total_items = table1_count + table2_count;
        
        if let Some(selected) = self.selected_index {
            let center_position = visible_items / 2;
            let ideal_scroll_offset = selected.saturating_sub(center_position);
            let max_scroll = total_items.saturating_sub(visible_items);
            self.scroll_offset = ideal_scroll_offset.min(max_scroll);
        } else {
            let max_scroll = total_items.saturating_sub(visible_items);
            if self.scroll_offset > max_scroll {
                self.scroll_offset = max_scroll;
            }
        }
    }

    pub fn get_table1_selected_index(&self, table1_count: usize) -> Option<usize> {
        if let Some(selected) = self.selected_index {
            if selected < table1_count {
                Some(selected)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn get_table2_selected_index(&self, table1_count: usize) -> Option<usize> {
        if let Some(selected) = self.selected_index {
            if selected >= table1_count {
                Some(selected.saturating_sub(table1_count))
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn update_last_rows(&mut self, rows: usize) {
        self.last_rows = rows;
    }
}