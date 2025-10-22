// Mock zellij-tile implementation for testing
// TODO: this should really be part of zellij-tile to help any plugin mock Zellij for tests...

pub mod prelude {
    pub use super::*;
}

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

// Thread-local storage for mock state
thread_local! {
    static MOCK_STATE: RefCell<MockState> = RefCell::new(MockState::default());
}

#[derive(Default)]
struct MockState {
    calls: Vec<ZellijCall>,
    plugin_ids: PluginIds,
    rendered_output: Vec<RenderedOutput>,
    current_frame: Option<Frame>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ZellijCall {
    RequestPermission(Vec<PermissionType>),
    Subscribe(Vec<EventType>),
    GetPluginIds,
    RenamePluginPane { id: u32, name: String },
    CloseSelf,
    PipeMessageToPlugin { plugin_url: String, args: BTreeMap<String, String> },
    ChangeHostFolder { path: PathBuf },
    ReplacePaneWithExistingPane { plugin_pane: PaneId, target_pane: PaneId },
    OpenFileInPlaceOfPlugin { path: PathBuf, line_number: Option<usize>, close_plugin: bool },
}

#[derive(Debug, Clone)]
pub enum RenderedOutput {
    Text { text: String, x: usize, y: usize },
    Table { x: usize, y: usize },
}

// =============================================================================
// PUBLIC TEST HARNESS API - Available from tests
// =============================================================================

/// Initialize the mock for a test
pub fn mock_init() {
    MOCK_STATE.with(|state| {
        *state.borrow_mut() = MockState::default();
    });
}

/// Set the plugin IDs that will be returned by get_plugin_ids()
pub fn mock_set_plugin_ids(plugin_ids: PluginIds) {
    MOCK_STATE.with(|state| {
        state.borrow_mut().plugin_ids = plugin_ids;
    });
}

/// Get all recorded calls
pub fn mock_get_calls() -> Vec<ZellijCall> {
    MOCK_STATE.with(|state| {
        state.borrow().calls.clone()
    })
}

/// Clear all recorded calls
pub fn mock_clear_calls() {
    MOCK_STATE.with(|state| {
        state.borrow_mut().calls.clear();
    });
}

/// Get all rendered output
pub fn mock_get_rendered_output() -> Vec<RenderedOutput> {
    MOCK_STATE.with(|state| {
        state.borrow().rendered_output.clone()
    })
}

/// Clear rendered output
pub fn mock_clear_rendered_output() {
    MOCK_STATE.with(|state| {
        state.borrow_mut().rendered_output.clear();
    });
}

/// Count calls matching a predicate
pub fn mock_count_calls<F>(predicate: F) -> usize
where
    F: Fn(&ZellijCall) -> bool
{
    MOCK_STATE.with(|state| {
        state.borrow().calls.iter().filter(|c| predicate(c)).count()
    })
}

/// Initialize a new frame with given dimensions
/// Should be called at the start of each test that checks rendering
pub fn mock_init_frame(width: usize, height: usize) {
    MOCK_STATE.with(|state| {
        state.borrow_mut().current_frame = Some(Frame::new(width, height));
    });
}

/// Get the current frame for assertions
pub fn mock_get_frame() -> Option<Frame> {
    MOCK_STATE.with(|state| state.borrow().current_frame.clone())
}

/// Clear the frame (reset to spaces)
pub fn mock_clear_frame() {
    MOCK_STATE.with(|state| {
        if let Some(frame) = &mut state.borrow_mut().current_frame {
            *frame = Frame::new(frame.width, frame.height);
        }
    });
}

/// Assert the current frame matches a snapshot
/// Uses cargo-insta for snapshot testing
#[cfg(test)]
pub fn assert_frame_snapshot(snapshot_name: &str) {
    let frame = mock_get_frame().expect("Frame not initialized - call mock_init_frame() first");
    insta::assert_snapshot!(snapshot_name, frame.to_trimmed_string());
}

// =============================================================================
// ZELLIJ TYPES
// =============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct PluginIds {
    pub plugin_id: u32,
    pub zellij_pid: u32,
    pub initial_cwd: PathBuf,
}

impl Default for PluginIds {
    fn default() -> Self {
        Self {
            plugin_id: 1,
            zellij_pid: 1234,
            initial_cwd: PathBuf::from("/test/cwd"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionType {
    ReadApplicationState,
    ChangeApplicationState,
    OpenFiles,
    RunCommands,
    OpenTerminalsOrPlugins,
    WriteToStdin,
    WebAccess,
    ReadCliPipes,
    MessageAndLaunchOtherPlugins,
    FullHdAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    ModeUpdate,
    TabUpdate,
    PaneUpdate,
    SessionUpdate,
    Key,
    Mouse,
    Timer,
    CopyToClipboard,
    SystemClipboardFailure,
    InputReceived,
    Visible,
    CommandPaneOpened,
    CommandPaneExited,
    CustomMessage,
    FileSystemCreate,
    FileSystemRead,
    FileSystemUpdate,
    FileSystemDelete,
    PermissionRequestResult,
    HostFolderChanged,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    ModeUpdate(ModeInfo),
    TabUpdate(Vec<TabInfo>),
    PaneUpdate(PaneManifest),
    Key(Key),
    Mouse(Mouse),
    Timer(f64),
    CopyToClipboard(CopyDestination),
    SystemClipboardFailure,
    InputReceived,
    Visible(bool),
    CustomMessage(String, String),
    PermissionRequestResult(PermissionStatus),
    SessionUpdate(Vec<SessionInfo>, Vec<SessionInfo>),
    HostFolderChanged(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModeInfo {}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionInfo {
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionStatus {
    Granted,
    Denied,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TabInfo {
    pub position: usize,
    pub name: String,
    pub active: bool,
    pub panes_to_hide: usize,
    pub is_fullscreen_active: bool,
    pub is_sync_panes_active: bool,
    pub are_floating_panes_visible: bool,
    pub other_focused_clients: Vec<u16>,
    pub active_swap_layout_name: Option<String>,
    pub is_swap_layout_dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PaneManifest {
    pub panes: BTreeMap<usize, Vec<PaneInfo>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaneInfo {
    pub id: u32,
    pub is_plugin: bool,
    pub is_focused: bool,
    pub is_fullscreen: bool,
    pub is_floating: bool,
    pub is_suppressed: bool,
    pub title: String,
    pub exited: bool,
    pub exit_status: Option<i32>,
    pub is_held: bool,
    pub pane_x: usize,
    pub pane_content_x: usize,
    pub pane_y: usize,
    pub pane_content_y: usize,
    pub pane_rows: usize,
    pub pane_content_rows: usize,
    pub pane_columns: usize,
    pub pane_content_columns: usize,
    pub cursor_coordinates_in_pane: Option<(usize, usize)>,
    pub terminal_command: Option<String>,
    pub plugin_url: Option<String>,
    pub is_selectable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneId {
    Terminal(u32),
    Plugin(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    pub bare_key: BareKey,
    pub modifiers: Vec<KeyModifier>,
}

impl Key {
    pub fn has_no_modifiers(&self) -> bool {
        self.modifiers.is_empty()
    }

    pub fn has_modifiers(&self, modifiers: &[KeyModifier]) -> bool {
        self.modifiers == modifiers
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BareKey {
    Enter,
    Tab,
    Backspace,
    Esc,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    BackTab,
    Delete,
    Insert,
    F(u8),
    Char(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyModifier {
    Ctrl,
    Alt,
    Shift,
    Super,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mouse {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyDestination {
    Command,
    Primary,
    System,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileToOpen {
    pub path: PathBuf,
    pub line_number: Option<usize>,
    pub cwd: Option<PathBuf>,
}

impl FileToOpen {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            path: path.into(),
            line_number: None,
            cwd: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FloatingPaneCoordinates {}

#[derive(Debug, Clone)]
pub struct MessageToPlugin {
    pub message_to_plugin: InternalMessageToPlugin,
}

#[derive(Debug, Clone)]
pub struct InternalMessageToPlugin {
    pub plugin_url: Option<String>,
    pub plugin_config: BTreeMap<String, String>,
    pub message_name: String,
    pub message_payload: Option<String>,
    pub message_args: BTreeMap<String, String>,
    pub new_plugin_args: Option<NewPluginArgs>,
}

#[derive(Debug, Clone)]
pub struct NewPluginArgs {
    pub should_float: Option<bool>,
    pub pane_id_to_replace: Option<PaneId>,
    pub pane_title: Option<String>,
    pub cwd: Option<PathBuf>,
    pub skip_cache: bool,
}

impl MessageToPlugin {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            message_to_plugin: InternalMessageToPlugin {
                plugin_url: None,
                plugin_config: BTreeMap::new(),
                message_name: name.into(),
                message_payload: None,
                message_args: BTreeMap::new(),
                new_plugin_args: None,
            },
        }
    }

    pub fn with_plugin_url(mut self, url: impl Into<String>) -> Self {
        self.message_to_plugin.plugin_url = Some(url.into());
        self
    }

    pub fn with_plugin_config(mut self, config: BTreeMap<String, String>) -> Self {
        self.message_to_plugin.plugin_config = config;
        self
    }

    pub fn with_args(mut self, args: BTreeMap<String, String>) -> Self {
        self.message_to_plugin.message_args = args;
        self
    }

    pub fn new_plugin_instance_should_have_pane_title(mut self, title: impl Into<String>) -> Self {
        let args = self.message_to_plugin.new_plugin_args.get_or_insert_with(|| NewPluginArgs {
            should_float: None,
            pane_id_to_replace: None,
            pane_title: None,
            cwd: None,
            skip_cache: false,
        });
        args.pane_title = Some(title.into());
        self
    }

    pub fn new_plugin_instance_should_replace_pane(mut self, pane_id: PaneId) -> Self {
        let args = self.message_to_plugin.new_plugin_args.get_or_insert_with(|| NewPluginArgs {
            should_float: None,
            pane_id_to_replace: None,
            pane_title: None,
            cwd: None,
            skip_cache: false,
        });
        args.pane_id_to_replace = Some(pane_id);
        self
    }
}

#[derive(Debug, Clone)]
pub struct PipeMessage {
    pub source: PipeSource,
    pub name: String,
    pub payload: Option<String>,
    pub args: BTreeMap<String, String>,
    pub is_private: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipeSource {
    Cli(u32),
    Plugin(u32),
    Keybind,
}

// =============================================================================
// FRAME STRUCTURE FOR SNAPSHOT TESTING
// =============================================================================

/// Represents a 2D terminal frame for testing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// 2D grid of characters (row-major: frame[y][x])
    cells: Vec<Vec<char>>,
    /// Height (rows)
    height: usize,
    /// Width (columns)
    width: usize,
}

impl Frame {
    /// Create new frame with given dimensions, filled with spaces
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            cells: vec![vec![' '; width]; height],
            height,
            width,
        }
    }

    /// Write text at specific coordinates
    /// Text that exceeds width is truncated
    pub fn write_text(&mut self, text: &str, x: usize, y: usize) {
        if y >= self.height {
            return;
        }

        for (i, ch) in text.chars().enumerate() {
            let current_x = x + i;
            if current_x >= self.width {
                break;
            }
            self.cells[y][current_x] = ch;
        }
    }

    /// Write multi-line text starting at coordinates
    pub fn write_lines(&mut self, lines: &[&str], x: usize, y: usize) {
        for (line_offset, line) in lines.iter().enumerate() {
            self.write_text(line, x, y + line_offset);
        }
    }

    /// Convert frame to string representation (for snapshots)
    pub fn to_string(&self) -> String {
        self.cells
            .iter()
            .map(|row| row.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Trim trailing spaces from each line and trailing empty lines
    /// This makes snapshots more readable and stable
    pub fn to_trimmed_string(&self) -> String {
        let lines: Vec<String> = self.cells
            .iter()
            .map(|row| row.iter().collect::<String>().trim_end().to_string())
            .collect();

        // Remove trailing empty lines
        let mut last_non_empty = 0;
        for (i, line) in lines.iter().enumerate() {
            if !line.is_empty() {
                last_non_empty = i;
            }
        }

        lines[..=last_non_empty].join("\n")
    }
}

// UI Components
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    /// The actual text content
    text: String,
    /// Lines for rendering (derived from text)
    lines: Vec<String>,
    /// Styling operations applied (tracked but not used in frame rendering)
    #[allow(dead_code)]
    styles: Vec<StyleOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StyleOperation {
    ColorAll(usize),
    ColorSubstring { color: usize, substring: String },
    ColorIndices { color: usize, indices: Vec<usize> },
    Selected,
}

impl Text {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let lines = if text.is_empty() {
            vec![String::new()]
        } else {
            text.split('\n').map(|s| s.to_string()).collect()
        };

        Self {
            text: text.clone(),
            lines,
            styles: Vec::new(),
        }
    }

    /// Get the plain text content (styling stripped)
    pub fn get_text(&self) -> &str {
        &self.text
    }

    /// Get lines for rendering
    pub fn get_lines(&self) -> &[String] {
        &self.lines
    }

    // Chainable styling methods (for API compatibility)

    pub fn color_all(mut self, color_index: usize) -> Self {
        self.styles.push(StyleOperation::ColorAll(color_index));
        self
    }

    pub fn color_substring(mut self, color_index: usize, substring: impl Into<String>) -> Self {
        self.styles.push(StyleOperation::ColorSubstring {
            color: color_index,
            substring: substring.into(),
        });
        self
    }

    pub fn color_indices(mut self, color_index: usize, indices: Vec<usize>) -> Self {
        self.styles.push(StyleOperation::ColorIndices {
            color: color_index,
            indices,
        });
        self
    }

    pub fn selected(mut self) -> Self {
        self.styles.push(StyleOperation::Selected);
        self
    }
}

#[derive(Debug, Clone)]
pub struct Table {
    /// Rows in the table
    rows: Vec<TableRow>,
}

#[derive(Debug, Clone)]
enum TableRow {
    Plain(Vec<String>),
    Styled(Vec<Text>),
}

impl Table {
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn add_row(mut self, row: Vec<String>) -> Self {
        self.rows.push(TableRow::Plain(row));
        self
    }

    pub fn add_styled_row(mut self, row: Vec<Text>) -> Self {
        self.rows.push(TableRow::Styled(row));
        self
    }

    /// Get number of rows
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Convert table to plain text lines (styling stripped)
    /// Each row is formatted as: "col1  col2  col3" with 2-space separation
    pub fn to_text_lines(&self) -> Vec<String> {
        self.rows
            .iter()
            .map(|row| match row {
                TableRow::Plain(cells) => cells.join("  "),
                TableRow::Styled(cells) => cells
                    .iter()
                    .map(|text| text.get_text())
                    .collect::<Vec<_>>()
                    .join("  "),
            })
            .collect()
    }

    /// Get individual row as text
    pub fn get_row_text(&self, index: usize) -> Option<String> {
        self.to_text_lines().get(index).cloned()
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// ZELLIJ API FUNCTIONS
// =============================================================================

pub fn request_permission(permissions: &[PermissionType]) {
    MOCK_STATE.with(|state| {
        state.borrow_mut().calls.push(ZellijCall::RequestPermission(permissions.to_vec()));
    });
}

pub fn subscribe(events: &[EventType]) {
    MOCK_STATE.with(|state| {
        state.borrow_mut().calls.push(ZellijCall::Subscribe(events.to_vec()));
    });
}

pub fn get_plugin_ids() -> PluginIds {
    MOCK_STATE.with(|state| {
        state.borrow_mut().calls.push(ZellijCall::GetPluginIds);
        state.borrow().plugin_ids.clone()
    })
}

pub fn rename_plugin_pane(id: u32, name: &str) {
    MOCK_STATE.with(|state| {
        state.borrow_mut().calls.push(ZellijCall::RenamePluginPane {
            id,
            name: name.to_string(),
        });
    });
}

pub fn close_self() {
    MOCK_STATE.with(|state| {
        state.borrow_mut().calls.push(ZellijCall::CloseSelf);
    });
}

pub fn pipe_message_to_plugin(message: MessageToPlugin) {
    MOCK_STATE.with(|state| {
        state.borrow_mut().calls.push(ZellijCall::PipeMessageToPlugin {
            plugin_url: message.message_to_plugin.plugin_url.clone().unwrap_or_default(),
            args: message.message_to_plugin.message_args.clone(),
        });
    });
}

pub fn change_host_folder(path: PathBuf) {
    MOCK_STATE.with(|state| {
        state.borrow_mut().calls.push(ZellijCall::ChangeHostFolder { path });
    });
}

pub fn replace_pane_with_existing_pane(plugin_pane: PaneId, target_pane: PaneId) {
    MOCK_STATE.with(|state| {
        state.borrow_mut().calls.push(ZellijCall::ReplacePaneWithExistingPane {
            plugin_pane,
            target_pane,
        });
    });
}

pub fn open_file_in_place_of_plugin(file: FileToOpen, close_plugin: bool, _position: FloatingPaneCoordinates) {
    MOCK_STATE.with(|state| {
        state.borrow_mut().calls.push(ZellijCall::OpenFileInPlaceOfPlugin {
            path: file.path,
            line_number: file.line_number,
            close_plugin,
        });
    });
}

pub fn print_text_with_coordinates(text: Text, x: usize, y: usize, _width: Option<usize>, _height: Option<usize>) {
    // Store in rendered_output for backward compatibility
    MOCK_STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.rendered_output.push(RenderedOutput::Text {
            text: text.get_text().to_string(),
            x,
            y,
        });

        if let Some(frame) = &mut state.current_frame {
            // Handle multi-line text
            for (line_offset, line) in text.get_lines().iter().enumerate() {
                frame.write_text(line, x, y + line_offset);
            }
        }
    });
}

pub fn print_table_with_coordinates(table: Table, x: usize, y: usize, _width: Option<usize>, _height: Option<usize>) {
    // Store in rendered_output for backward compatibility
    MOCK_STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.rendered_output.push(RenderedOutput::Table { x, y });

        if let Some(frame) = &mut state.current_frame {
            let lines = table.to_text_lines();
            for (line_offset, line) in lines.iter().enumerate() {
                frame.write_text(line, x, y + line_offset);
            }
        }
    });
}

// =============================================================================
// ZELLIJ PLUGIN TRAIT
// =============================================================================

pub trait ZellijPlugin: Default {
    fn load(&mut self, configuration: BTreeMap<String, String>);
    fn update(&mut self, event: Event) -> bool;
    fn render(&mut self, rows: usize, cols: usize);
    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        let _ = pipe_message;
        false
    }
}

// Plugin registration macro
#[macro_export]
macro_rules! register_plugin {
    ($t:ty) => {
        // In tests, this is a no-op since we'll instantiate directly
    };
}
