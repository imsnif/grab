use crate::unit::{fixtures, test_zellij};
use crate::State;
use std::collections::BTreeMap;
use std::path::PathBuf;
use test_zellij::{
    BareKey, Event, Key, KeyModifier, PaneId, PermissionStatus, PipeMessage, PluginIds,
    ZellijPlugin,
};

fn setup() -> State {
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/test/project"),
    });
    State::default()
}

#[test]
fn test_load_requests_permissions() {
    let mut state = setup();
    state.load(BTreeMap::new());

    let calls = test_zellij::mock_get_calls();
    assert!(calls
        .iter()
        .any(|c| matches!(c, test_zellij::ZellijCall::RequestPermission(_))));
}

#[test]
fn test_permission_result_renames_pane() {
    let mut state = setup();
    state.load(BTreeMap::new());
    test_zellij::mock_clear_calls();

    state.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    let calls = test_zellij::mock_get_calls();
    assert!(calls.iter().any(|c| matches!(
        c,
        test_zellij::ZellijCall::RenamePluginPane { id: 42, name }
            if name == "Grab..."
    )));
}

#[test]
fn test_down_key_triggers_render() {
    let mut state = setup();
    state.load(BTreeMap::new());

    let should_render = state.update(Event::Key(Key {
        bare_key: BareKey::Down,
        modifiers: vec![],
    }));

    assert!(should_render);
}

#[test]
fn test_typing_triggers_render() {
    let mut state = setup();
    state.load(BTreeMap::new());

    let should_render = state.update(Event::Key(Key {
        bare_key: BareKey::Char('x'),
        modifiers: vec![],
    }));

    assert!(should_render);
}

#[test]
fn test_ctrl_c_on_empty_search_closes_plugin() {
    let mut state = setup();
    state.load(BTreeMap::new());
    test_zellij::mock_clear_calls();

    state.update(Event::Key(Key {
        bare_key: BareKey::Char('c'),
        modifiers: vec![KeyModifier::Ctrl],
    }));

    let calls = test_zellij::mock_get_calls();
    assert!(calls
        .iter()
        .any(|c| matches!(c, test_zellij::ZellijCall::CloseSelf)));
}

#[test]
fn test_ctrl_c_with_text_does_not_close() {
    let mut state = setup();
    state.load(BTreeMap::new());

    // Type something
    state.update(Event::Key(Key {
        bare_key: BareKey::Char('x'),
        modifiers: vec![],
    }));

    test_zellij::mock_clear_calls();

    state.update(Event::Key(Key {
        bare_key: BareKey::Char('c'),
        modifiers: vec![KeyModifier::Ctrl],
    }));

    let calls = test_zellij::mock_get_calls();
    assert!(!calls
        .iter()
        .any(|c| matches!(c, test_zellij::ZellijCall::CloseSelf)));
}

#[test]
fn test_render_completes_without_panic() {
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/test/project"),
    });
    test_zellij::mock_init_frame(80, 24);

    let mut state = State::default();
    state.load(BTreeMap::new());
    state.update(Event::PermissionRequestResult(PermissionStatus::Granted));
    state.render(24, 80);

    // Assert against snapshot to verify rendering output
    test_zellij::assert_frame_snapshot("render_default_state");
}

#[test]
fn test_render_empty_state() {
    // Setup
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/home/user/project"),
    });
    test_zellij::mock_init_frame(80, 24);

    let mut plugin = State::default();
    plugin.load(BTreeMap::new());

    // Simulate permission granted
    plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    // Render with no search term
    plugin.render(24, 80);

    // Assert snapshot
    test_zellij::assert_frame_snapshot("render_empty_state");
}

#[test]
fn test_render_with_sample_data() {
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/home/user/project"),
    });
    test_zellij::mock_init_frame(80, 24);

    let mut plugin = State::default();
    plugin.app_state.update_panes(fixtures::sample_panes());
    plugin.app_state.update_files(fixtures::sample_files());
    plugin
        .app_state
        .update_rust_assets(fixtures::sample_rust_assets());

    plugin.load(BTreeMap::new());
    plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    plugin.render(24, 80);

    test_zellij::assert_frame_snapshot("render_with_sample_data");
}

#[test]
fn test_render_with_search_term() {
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/home/user/project"),
    });
    test_zellij::mock_init_frame(80, 24);

    let mut plugin = State::default();
    plugin.app_state.update_panes(fixtures::sample_panes());
    plugin.app_state.update_files(fixtures::sample_files());
    plugin
        .app_state
        .update_rust_assets(fixtures::sample_rust_assets());

    plugin.load(BTreeMap::new());
    plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    // Type search term "main"
    plugin.update(Event::Key(Key {
        bare_key: BareKey::Char('m'),
        modifiers: vec![],
    }));
    plugin.update(Event::Key(Key {
        bare_key: BareKey::Char('a'),
        modifiers: vec![],
    }));
    plugin.update(Event::Key(Key {
        bare_key: BareKey::Char('i'),
        modifiers: vec![],
    }));
    plugin.update(Event::Key(Key {
        bare_key: BareKey::Char('n'),
        modifiers: vec![],
    }));

    // Render
    plugin.render(24, 80);

    test_zellij::assert_frame_snapshot("render_with_search_main");
}

#[test]
fn test_render_with_selection() {
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/home/user/project"),
    });
    test_zellij::mock_init_frame(80, 24);

    let mut plugin = State::default();
    plugin.app_state.update_panes(fixtures::sample_panes());

    plugin.load(BTreeMap::new());
    plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    // Press down to select first result
    plugin.update(Event::Key(Key {
        bare_key: BareKey::Down,
        modifiers: vec![],
    }));

    plugin.render(24, 80);

    test_zellij::assert_frame_snapshot("render_with_selection");
}

#[test]
fn test_typing_string_searches_and_displays_results() {
    // Setup
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/home/user/project"),
    });
    test_zellij::mock_init_frame(80, 24);

    let mut plugin = State::default();
    plugin.app_state.update_panes(fixtures::sample_panes());
    plugin.app_state.update_files(fixtures::sample_files());
    plugin
        .app_state
        .update_rust_assets(fixtures::sample_rust_assets());

    plugin.load(BTreeMap::new());
    plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    test_zellij::mock_clear_calls();

    // Type "cargo" to search for Cargo.toml
    for ch in "cargo".chars() {
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Char(ch),
            modifiers: vec![],
        }));
    }

    // Verify search results were updated
    let results = plugin.search_state.get_current_display_results();
    assert!(
        !results.is_empty(),
        "Should have search results for 'cargo'"
    );

    // Check that Cargo.toml is in the results
    let has_cargo_toml = results
        .iter()
        .any(|r| r.display_text().contains("Cargo.toml"));
    assert!(has_cargo_toml, "Cargo.toml should be in search results");

    // Render and verify output
    plugin.render(24, 80);
    test_zellij::assert_frame_snapshot("search_results_cargo");
}

#[test]
fn test_enter_on_pane_opens_pane() {
    // Setup
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/home/user/project"),
    });

    let mut plugin = State::default();
    plugin.app_state.update_panes(fixtures::sample_panes());

    plugin.load(BTreeMap::new());
    plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    // Type "vim" to search for vim panes
    for ch in "vim".chars() {
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Char(ch),
            modifiers: vec![],
        }));
    }

    // Verify we have results
    let results = plugin.search_state.get_current_display_results();
    assert!(!results.is_empty(), "Should have search results for 'vim'");

    // First result should be a pane (from "vim ~/project/src/main.rs")
    assert!(results[0].is_pane(), "First result should be a pane");

    test_zellij::mock_clear_calls();

    // Press ENTER to open the pane
    plugin.update(Event::Key(Key {
        bare_key: BareKey::Enter,
        modifiers: vec![],
    }));

    // Verify that replace_pane_with_existing_pane was called
    let calls = test_zellij::mock_get_calls();
    let replaced = calls.iter().any(|c| {
        matches!(
            c,
            test_zellij::ZellijCall::ReplacePaneWithExistingPane {
                plugin_pane: PaneId::Plugin(42),
                target_pane: PaneId::Terminal(1)
            }
        )
    });
    assert!(
        replaced,
        "Should call replace_pane_with_existing_pane for pane"
    );
}

#[test]
fn test_enter_on_file_opens_file() {
    // Setup
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/home/user/project"),
    });

    let mut plugin = State::default();
    plugin
        .app_state
        .set_cwd(PathBuf::from("/home/user/project"));
    plugin.app_state.update_files(fixtures::sample_files());

    plugin.load(BTreeMap::new());
    plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    // Type "README" to search for README.md
    for ch in "README".chars() {
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Char(ch),
            modifiers: vec![],
        }));
    }

    // Verify we have results
    let results = plugin.search_state.get_current_display_results();
    assert!(
        !results.is_empty(),
        "Should have search results for 'README'"
    );

    // Result should be a file
    assert!(results[0].is_file(), "Result should be a file");

    test_zellij::mock_clear_calls();

    // Press ENTER to open the file
    plugin.update(Event::Key(Key {
        bare_key: BareKey::Enter,
        modifiers: vec![],
    }));

    // Verify that open_file_in_place_of_plugin was called
    let calls = test_zellij::mock_get_calls();
    let opened = calls.iter().any(|c| {
        matches!(c, test_zellij::ZellijCall::OpenFileInPlaceOfPlugin {
            path,
            line_number: None,
            close_plugin: true
        } if path.ends_with("README.md"))
    });
    assert!(opened, "Should call open_file_in_place_of_plugin for file");
}

#[test]
fn test_struct_search_and_enter_opens_file_at_line() {
    use crate::files::TypeKind;

    // Setup
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/home/user/project"),
    });
    test_zellij::mock_init_frame(80, 24);

    let mut plugin = State::default();
    plugin
        .app_state
        .set_cwd(PathBuf::from("/home/user/project"));
    plugin
        .app_state
        .update_rust_assets(fixtures::struct_search_rust_assets());

    plugin.load(BTreeMap::new());
    plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    // Type "struct mystruct" to search for MyStruct (fuzzy match)
    for ch in "struct mystruct".chars() {
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Char(ch),
            modifiers: vec![],
        }));
    }

    // Verify we have results and they're rust assets
    let results = plugin.search_state.get_current_display_results();
    assert!(
        !results.is_empty(),
        "Should have search results for 'struct mystruct'"
    );

    // All results should only be structs (not functions)
    for result in &results {
        assert!(result.is_rust_asset(), "All results should be rust assets");
        if let crate::search::SearchItem::RustAsset(asset) = &result.item {
            assert!(
                matches!(asset.type_kind, TypeKind::Struct),
                "Should only show structs"
            );
        }
    }

    // Should fuzzy match MyStruct and MyStructHelper
    let result_names: Vec<String> = results
        .iter()
        .filter_map(|r| {
            if let crate::search::SearchItem::RustAsset(asset) = &r.item {
                Some(asset.name.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(
        result_names.contains(&"MyStruct".to_string()),
        "Should find MyStruct"
    );
    assert!(
        result_names.contains(&"MyStructHelper".to_string()),
        "Should find MyStructHelper"
    );

    // Render and verify output
    plugin.render(24, 80);
    test_zellij::assert_frame_snapshot("struct_search_mystruct");

    test_zellij::mock_clear_calls();

    // Press ENTER to open the file at the line (should open first result)
    plugin.update(Event::Key(Key {
        bare_key: BareKey::Enter,
        modifiers: vec![],
    }));

    // Verify that open_file_in_place_of_plugin was called with line number
    let calls = test_zellij::mock_get_calls();
    let opened = calls.iter().any(|c| {
        matches!(
            c,
            test_zellij::ZellijCall::OpenFileInPlaceOfPlugin {
                path: _,
                line_number: Some(_),
                close_plugin: true
            }
        )
    });
    assert!(
        opened,
        "Should call open_file_in_place_of_plugin with line number"
    );
}

#[test]
fn test_enum_search_and_enter_opens_file_at_line() {
    use crate::files::TypeKind;

    // Setup
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/home/user/project"),
    });
    test_zellij::mock_init_frame(80, 24);

    let mut plugin = State::default();
    plugin
        .app_state
        .set_cwd(PathBuf::from("/home/user/project"));
    plugin
        .app_state
        .update_rust_assets(fixtures::enum_search_rust_assets());

    plugin.load(BTreeMap::new());
    plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    // Type "enum search" to fuzzy search for search-related enums
    for ch in "enum search".chars() {
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Char(ch),
            modifiers: vec![],
        }));
    }

    // Verify we have results and they're rust assets
    let results = plugin.search_state.get_current_display_results();
    assert!(
        !results.is_empty(),
        "Should have search results for 'enum search'"
    );

    // All results should only be enums (not structs)
    for result in &results {
        assert!(result.is_rust_asset(), "All results should be rust assets");
        if let crate::search::SearchItem::RustAsset(asset) = &result.item {
            assert!(
                matches!(asset.type_kind, TypeKind::Enum),
                "Should only show enums"
            );
        }
    }

    // Should fuzzy match SearchMode, SearchType, and SearchItem
    let result_names: Vec<String> = results
        .iter()
        .filter_map(|r| {
            if let crate::search::SearchItem::RustAsset(asset) = &r.item {
                Some(asset.name.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(
        result_names.contains(&"SearchMode".to_string()),
        "Should find SearchMode"
    );
    assert!(
        result_names.contains(&"SearchType".to_string()),
        "Should find SearchType"
    );
    assert!(
        result_names.contains(&"SearchItem".to_string()),
        "Should find SearchItem"
    );

    // Render and verify output
    plugin.render(24, 80);
    test_zellij::assert_frame_snapshot("enum_search_search");

    test_zellij::mock_clear_calls();

    // Press ENTER to open the file at the line (should open first result)
    plugin.update(Event::Key(Key {
        bare_key: BareKey::Enter,
        modifiers: vec![],
    }));

    // Verify that open_file_in_place_of_plugin was called with line number
    let calls = test_zellij::mock_get_calls();
    let opened = calls.iter().any(|c| {
        matches!(
            c,
            test_zellij::ZellijCall::OpenFileInPlaceOfPlugin {
                path: _,
                line_number: Some(_),
                close_plugin: true
            }
        )
    });
    assert!(
        opened,
        "Should call open_file_in_place_of_plugin with line number"
    );
}

#[test]
fn test_fn_search_and_enter_opens_file_at_line() {
    use crate::files::TypeKind;

    // Setup
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/home/user/project"),
    });
    test_zellij::mock_init_frame(80, 24);

    let mut plugin = State::default();
    plugin
        .app_state
        .set_cwd(PathBuf::from("/home/user/project"));
    plugin
        .app_state
        .update_rust_assets(fixtures::function_search_rust_assets());

    plugin.load(BTreeMap::new());
    plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    // Type "fn render" to fuzzy search for render-related functions
    for ch in "fn render".chars() {
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Char(ch),
            modifiers: vec![],
        }));
    }

    // Verify we have results and they're rust assets
    let results = plugin.search_state.get_current_display_results();
    assert!(
        !results.is_empty(),
        "Should have search results for 'fn render'"
    );

    // All results should only be functions (not structs)
    for result in &results {
        assert!(result.is_rust_asset(), "All results should be rust assets");
        if let crate::search::SearchItem::RustAsset(asset) = &result.item {
            assert!(
                matches!(asset.type_kind, TypeKind::Function),
                "Should only show functions"
            );
        }
    }

    // Should fuzzy match render-related functions
    let result_names: Vec<String> = results
        .iter()
        .filter_map(|r| {
            if let crate::search::SearchItem::RustAsset(asset) = &r.item {
                Some(asset.name.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(
        result_names.contains(&"render".to_string()),
        "Should find render"
    );
    assert!(
        result_names.contains(&"render_ui".to_string()),
        "Should find render_ui"
    );
    assert!(
        result_names.contains(&"render_table".to_string()),
        "Should find render_table"
    );
    assert!(
        result_names.contains(&"render_text".to_string()),
        "Should find render_text"
    );

    // Render and verify output
    plugin.render(24, 80);
    test_zellij::assert_frame_snapshot("fn_search_render");

    test_zellij::mock_clear_calls();

    // Press ENTER to open the file at the line (should open first result)
    plugin.update(Event::Key(Key {
        bare_key: BareKey::Enter,
        modifiers: vec![],
    }));

    // Verify that open_file_in_place_of_plugin was called with line number
    let calls = test_zellij::mock_get_calls();
    let opened = calls.iter().any(|c| {
        matches!(
            c,
            test_zellij::ZellijCall::OpenFileInPlaceOfPlugin {
                path: _,
                line_number: Some(_),
                close_plugin: true
            }
        )
    });
    assert!(
        opened,
        "Should call open_file_in_place_of_plugin with line number"
    );
}

#[test]
fn test_ctrl_f_calls_filepicker() {
    // Setup
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/home/user/project"),
    });

    let mut plugin = State::default();
    plugin
        .app_state
        .set_cwd(PathBuf::from("/home/user/project"));
    plugin.load(BTreeMap::new());
    plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    test_zellij::mock_clear_calls();

    // Press Ctrl+F
    plugin.update(Event::Key(Key {
        bare_key: BareKey::Char('f'),
        modifiers: vec![KeyModifier::Ctrl],
    }));

    // Verify that pipe_message_to_plugin was called with filepicker
    let calls = test_zellij::mock_get_calls();
    let called_filepicker = calls.iter().any(|c| {
        matches!(c, test_zellij::ZellijCall::PipeMessageToPlugin {
            plugin_url,
            args
        } if plugin_url == "filepicker" && args.contains_key("request_id"))
    });
    assert!(
        called_filepicker,
        "Should call pipe_message_to_plugin with filepicker"
    );

    // Verify request_id was stored
    assert!(!plugin.request_ids.is_empty(), "Should store request_id");
}

#[test]
fn test_receiving_pipe_from_filepicker_changes_folder() {
    // Setup
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/home/user/project"),
    });
    test_zellij::mock_init_frame(80, 24);

    let mut plugin = State::default();
    plugin
        .app_state
        .set_cwd(PathBuf::from("/home/user/project"));
    plugin.load(BTreeMap::new());
    plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    // Clear initial_cwd by triggering the first HostFolderChanged event
    // This simulates the normal initialization flow
    plugin.update(Event::HostFolderChanged(PathBuf::from(
        "/home/user/project",
    )));

    // Disable git repo search since we're testing folder change behavior
    plugin.searching_for_git_repo = false;

    // Render initial state to show original folder
    plugin.render(24, 80);
    test_zellij::assert_frame_snapshot("filepicker_before_folder_change");

    // Simulate pressing Ctrl+F to get a request_id
    plugin.update(Event::Key(Key {
        bare_key: BareKey::Char('f'),
        modifiers: vec![KeyModifier::Ctrl],
    }));

    let request_id = plugin.request_ids[0].clone();
    test_zellij::mock_clear_calls();

    // Simulate receiving a pipe message from filepicker
    let mut args = BTreeMap::new();
    args.insert("request_id".to_string(), request_id.clone());

    let pipe_message = PipeMessage {
        source: test_zellij::PipeSource::Plugin(1),
        name: "filepicker_result".to_string(),
        payload: Some("/new/folder/path".to_string()),
        args,
        is_private: false,
    };

    plugin.pipe(pipe_message);

    // Verify that change_host_folder was called with the new path
    let calls = test_zellij::mock_get_calls();
    let changed_folder = calls.iter().any(|c| {
        matches!(c, test_zellij::ZellijCall::ChangeHostFolder {
            path
        } if path == &PathBuf::from("/new/folder/path"))
    });
    assert!(
        changed_folder,
        "Should call change_host_folder with new path"
    );

    // Verify request_id was removed
    assert!(
        plugin.request_ids.is_empty(),
        "Should remove request_id after processing"
    );

    // Verify user_selected_directory flag was set
    assert!(
        plugin.app_state.is_user_selected_directory(),
        "Should mark as user selected directory"
    );

    // Simulate the HostFolderChanged event that would be triggered by Zellij
    plugin.update(Event::HostFolderChanged(PathBuf::from("/new/folder/path")));

    // Verify the folder was updated in app state
    assert_eq!(
        plugin.app_state.get_cwd(),
        &PathBuf::from("/new/folder/path"),
        "App state should reflect new folder"
    );

    // Render and verify the new folder is displayed
    test_zellij::mock_clear_frame();
    plugin.render(24, 80);
    test_zellij::assert_frame_snapshot("filepicker_after_folder_change");

    // Verify the frame contains the new folder path
    let frame = test_zellij::mock_get_frame().expect("Frame should be initialized");
    let frame_str = frame.to_string();
    assert!(
        frame_str.contains("/new/folder/path"),
        "Rendered output should display the new folder path"
    );
}

#[test]
fn test_struct_keyword_filters_only_structs() {
    // Setup
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/home/user/project"),
    });

    let mut plugin = State::default();
    plugin
        .app_state
        .update_rust_assets(fixtures::sample_rust_assets());

    plugin.load(BTreeMap::new());
    plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    // Type "struct " (with space) to search for all structs
    for ch in "struct ".chars() {
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Char(ch),
            modifiers: vec![],
        }));
    }

    // Verify we have results and they're all structs
    let results = plugin.search_state.get_current_display_results();
    assert!(
        !results.is_empty(),
        "Should have search results for 'struct '"
    );

    // All results should be structs
    for result in results {
        assert!(result.is_rust_asset(), "All results should be rust assets");
        if let crate::search::SearchItem::RustAsset(asset) = &result.item {
            assert!(
                matches!(asset.type_kind, crate::files::TypeKind::Struct),
                "All results should be structs, found: {:?}",
                asset.type_kind
            );
        }
    }
}

#[test]
fn test_search_rendering_shows_correct_results() {
    // Setup
    test_zellij::mock_init();
    test_zellij::mock_set_plugin_ids(PluginIds {
        plugin_id: 42,
        zellij_pid: 1234,
        initial_cwd: PathBuf::from("/home/user/project"),
    });
    test_zellij::mock_init_frame(80, 24);

    let mut plugin = State::default();
    plugin.app_state.update_panes(fixtures::sample_panes());
    plugin.app_state.update_files(fixtures::sample_files());
    plugin
        .app_state
        .update_rust_assets(fixtures::sample_rust_assets());

    plugin.load(BTreeMap::new());
    plugin.update(Event::PermissionRequestResult(PermissionStatus::Granted));

    // Type "ui" to search
    for ch in "ui".chars() {
        plugin.update(Event::Key(Key {
            bare_key: BareKey::Char(ch),
            modifiers: vec![],
        }));
    }

    // Render
    plugin.render(24, 80);

    // Verify frame contains expected results
    let frame = test_zellij::mock_get_frame().expect("Frame should be initialized");
    let frame_str = frame.to_string();

    // Should contain "ui" somewhere in the search results
    assert!(
        frame_str.contains("ui") || frame_str.contains("UI"),
        "Rendered output should contain search results for 'ui'"
    );

    test_zellij::assert_frame_snapshot("search_results_ui");
}
