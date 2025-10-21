# Pipe-Based Launch Implementation Plan

## Overview

Transform the Zellij picker plugin from normal launch to pipe-based launch with a management/worker instance architecture. The management instance is launched via pipe message with special config, stays hidden, and spawns worker instances that replace the user's focused pane when receiving subsequent pipe messages.

## Current Plugin Analysis

The plugin currently:
- Launches normally as a single instance
- Has file/pane/Rust asset search functionality
- Uses permissions: ReadApplicationState, ChangeApplicationState, OpenFiles, FullHdAccess, MessageAndLaunchOtherPlugins, RunCommands
- Subscribes to: PaneUpdate, Key, PermissionRequestResult, TabUpdate, HostFolderChanged
- Handles pipe messages from filepicker plugin
- Maintains state for search, UI, and application data

## Architecture Design

### Instance Types
- **Management Instance**: Hidden background process launched via pipe with `instance_type=management` config
- **Worker Instance**: Functional picker instance that replaces focused pane, launched by management instance

### Launch Flow
```
1. Launch management: zellij pipe "picker_launch" --plugin-configuration "instance_type=management"
   → Plugin loads with management config, calls hide_self(), subscribes to pipes

2. Spawn worker: zellij pipe "picker_launch" --plugin-url <same-url>
   → Management receives pipe, spawns worker with focused pane CWD
   → Worker loads with worker config, replaces focused pane
```

## Required Imports

Ensure these imports are available in `src/main.rs`:

```rust
use zellij_tile::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;
// ... existing imports remain unchanged
```

The key functions needed from `zellij_tile::prelude::*`:
- `hide_self()` - for hiding management instance
- `pipe_message_to_plugin()` - for spawning worker instances
- `MessageToPlugin` - for constructing pipe messages
- `PaneContents`, `PaneId` - for pane detection logic

## Detailed Implementation Steps

### Step 1: Create Instance Type Enum

**File**: `src/main.rs`
**Location**: After imports, before `State` struct

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceType {
    Management,
    Worker,
}

impl Default for InstanceType {
    fn default() -> Self {
        InstanceType::Worker
    }
}

impl InstanceType {
    fn from_config(config: &BTreeMap<String, String>) -> Self {
        match config.get("instance_type").map(|s| s.as_str()) {
            Some("management") => InstanceType::Management,
            _ => InstanceType::Worker, // Default for backward compatibility
        }
    }
}
```

### Step 2: Modify State Struct

**File**: `src/main.rs`
**Location**: Add to `State` struct fields

```rust
#[derive(Default)]
pub struct State {
    instance_type: InstanceType,
    // ... existing fields remain unchanged
}
```

### Step 3: Update load() Method

**File**: `src/main.rs`
**Location**: Replace existing `load()` method

```rust
fn load(&mut self, configuration: BTreeMap<String, String>) {
    self.instance_type = InstanceType::from_config(&configuration);

    match self.instance_type {
        InstanceType::Management => {
            self.load_management_instance(&configuration);
        }
        InstanceType::Worker => {
            self.load_worker_instance(&configuration);
        }
    }
}
```

### Step 4: Implement Management Instance Load

**File**: `src/main.rs`
**Location**: Add new method to `State` impl block

```rust
fn load_management_instance(&mut self, _configuration: &BTreeMap<String, String>) {
    // Request minimal permissions for management
    request_permission(&[
        PermissionType::ReadApplicationState,
        PermissionType::MessageAndLaunchOtherPlugins,
    ]);

    // Subscribe only to necessary events
    subscribe(&[
        EventType::PermissionRequestResult,
        EventType::TabUpdate,
        EventType::PaneUpdate,
    ]);

    // Hide immediately when permissions are granted
    // (will be done in PermissionRequestResult event)
}
```

### Step 5: Implement Worker Instance Load

**File**: `src/main.rs`
**Location**: Add new method to `State` impl block

```rust
fn load_worker_instance(&mut self, configuration: &BTreeMap<String, String>) {
    // Use provided CWD if available
    if let Some(cwd_str) = configuration.get("cwd") {
        let cwd_path = PathBuf::from(cwd_str);
        if cwd_path.exists() {
            self.initial_cwd = Some(cwd_path);
        }
    }

    // Original load logic (unchanged)
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

    if self.initial_cwd.is_none() {
        self.initial_cwd = Some(get_plugin_ids().initial_cwd);
    }
    self.update_host_folder(None, false);
}
```

### Step 6: Update update() Method

**File**: `src/main.rs`
**Location**: Replace existing `update()` method

```rust
fn update(&mut self, event: Event) -> bool {
    match self.instance_type {
        InstanceType::Management => {
            self.handle_management_event(event)
        }
        InstanceType::Worker => {
            self.handle_worker_event(event)
        }
    }
}
```

### Step 7: Implement Management Event Handler

**File**: `src/main.rs`
**Location**: Add new method to `State` impl block

```rust
fn handle_management_event(&mut self, event: Event) -> bool {
    match event {
        Event::PermissionRequestResult(_) => {
            // Hide the management instance as soon as permissions are granted
            hide_self();
            false
        }
        Event::TabUpdate(tab_info) => {
            self.tabs = tab_info;
            false
        }
        Event::PaneUpdate(pane_manifest) => {
            let panes = extract_editor_pane_metadata(&pane_manifest);
            self.app_state.update_panes(panes);
            false
        }
        _ => false
    }
}
```

### Step 8: Extract Worker Event Handler

**File**: `src/main.rs`
**Location**: Add new method to `State` impl block

```rust
fn handle_worker_event(&mut self, event: Event) -> bool {
    // This is the existing update() logic unchanged
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
                self.update_host_folder_with_scan_control(Some(new_host_folder), true, user_selected);
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
```

### Step 9: Add Focused Pane Detection

**File**: `src/main.rs`
**Location**: Add new method to `State` impl block

```rust
fn get_focused_pane_info(&self) -> Option<(PaneId, PathBuf)> {
    for tab in &self.tabs {
        if tab.active {
            for pane_info in &tab.panes {
                if pane_info.is_focused {
                    let cwd = match &pane_info.pane_contents {
                        PaneContents::Terminal(terminal_pane) => {
                            terminal_pane.cwd.clone()
                        }
                        _ => get_plugin_ids().initial_cwd,
                    };
                    return Some((pane_info.id, cwd));
                }
            }
        }
    }
    None
}
```

### Step 10: Add Plugin Pane Detection

**File**: `src/main.rs`
**Location**: Add new method to `State` impl block

```rust
fn is_focused_pane_plugin(&self) -> bool {
    if let Some((focused_pane_id, _)) = self.get_focused_pane_info() {
        matches!(focused_pane_id, PaneId::Plugin(_))
    } else {
        false
    }
}
```

### Step 11: Update pipe() Method

**File**: `src/main.rs`
**Location**: Replace existing `pipe()` method

```rust
fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
    match self.instance_type {
        InstanceType::Management => {
            self.handle_management_pipe_message(pipe_message)
        }
        InstanceType::Worker => {
            self.handle_worker_pipe_message(pipe_message)
        }
    }
}
```

### Step 12: Implement Management Pipe Handler

**File**: `src/main.rs`
**Location**: Add new method to `State` impl block

```rust
fn handle_management_pipe_message(&mut self, pipe_message: PipeMessage) -> bool {
    match pipe_message.name.as_str() {
        "picker_launch" => {
            self.spawn_worker_instance();
            true
        }
        _ => false
    }
}

fn spawn_worker_instance(&mut self) {
    // Check if focused pane is a plugin
    if self.is_focused_pane_plugin() {
        eprintln!("Focused pane is a plugin, skipping spawn");
        return;
    }

    // Get focused pane info
    let (focused_pane_id, focused_cwd) = match self.get_focused_pane_info() {
        Some(info) => info,
        None => {
            eprintln!("Could not determine focused pane, cannot spawn worker");
            return;
        }
    };

    // Create worker configuration
    let mut config = BTreeMap::new();
    config.insert("instance_type".to_owned(), "worker".to_owned());
    config.insert("cwd".to_owned(), focused_cwd.display().to_string());

    // Spawn worker instance
    let message = MessageToPlugin::new("picker_worker_spawn")
        .with_plugin_url("file:target/wasm32-wasip1/release/picker.wasm") // Same URL as management instance
        .with_plugin_config(config)
        .new_plugin_instance_should_replace_pane(focused_pane_id);

    pipe_message_to_plugin(message);
}
```

### Step 13: Implement Worker Pipe Handler

**File**: `src/main.rs`
**Location**: Add new method to `State` impl block

```rust
fn handle_worker_pipe_message(&mut self, pipe_message: PipeMessage) -> bool {
    // Existing pipe message handling (filepicker_result)
    if pipe_message.name == "filepicker_result" {
        match (pipe_message.payload, pipe_message.args.get("request_id")) {
            (Some(payload), Some(request_id)) => {
                match self.request_ids.iter().position(|p| p == request_id) {
                    Some(request_id_position) => {
                        self.request_ids.remove(request_id_position);
                        let new_folder = std::path::PathBuf::from(payload);
                        self.app_state.set_user_selected_directory(true);
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
```

### Step 14: Update render() Method

**File**: `src/main.rs`
**Location**: Replace existing `render()` method

```rust
fn render(&mut self, rows: usize, cols: usize) {
    match self.instance_type {
        InstanceType::Management => {
            // Management instance should be hidden, no rendering needed
        }
        InstanceType::Worker => {
            // Existing render logic unchanged
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
}
```

## Testing

Build and test the implementation with:
```bash
cargo build
```

## Usage Instructions

### Setup
1. Build plugin: `cargo build --release --target wasm32-wasip1`
2. Start management instance: `zellij pipe "picker_launch" --plugin-url "file:target/wasm32-wasip1/release/picker.wasm" --plugin-configuration "instance_type=management"`

### Daily Usage
- Launch picker: `zellij pipe "picker_launch" --plugin-url "file:target/wasm32-wasip1/release/picker.wasm"`
- Or create a Zellij alias/keybind for easier access

### Backward Compatibility
- Direct launches still work: `zellij action new-pane --plugin "file:target/wasm32-wasip1/release/picker.wasm"`

## Migration Notes

### Configuration Changes
- New config key: `instance_type` ("management" | "worker")
- New config key for workers: `cwd` (path string)
- Existing configurations continue to work (default to worker mode)

### Pipe Message Format
- Management launch: uses config `instance_type=management`
- Worker spawn trigger: message name `"picker_launch"` to management instance
- Existing filepicker messages unchanged

## Implementation Order

1. Add InstanceType enum and State modifications
2. Update load() method with branching logic
3. Implement management instance load and hide logic
4. Extract worker instance logic (existing code)
5. Add focused pane detection utilities
6. Implement management pipe handler with worker spawning
7. Update render method with instance type branching

This implementation maintains full backward compatibility while adding the pipe-based management architecture with proper load timing.
