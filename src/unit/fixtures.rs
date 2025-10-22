// Test fixtures for mock data
#![cfg(test)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use crate::pane::PaneMetadata;
use crate::files::{TypeDefinition, TypeKind};
use super::test_zellij::PaneId;

/// Create sample panes for testing
pub fn sample_panes() -> Vec<PaneMetadata> {
    vec![
        PaneMetadata {
            id: PaneId::Terminal(1),
            title: "vim ~/project/src/main.rs".to_string(),
        },
        PaneMetadata {
            id: PaneId::Terminal(2),
            title: "bash".to_string(),
        },
        PaneMetadata {
            id: PaneId::Terminal(3),
            title: "nvim ~/project/Cargo.toml".to_string(),
        },
    ]
}

/// Create sample files for testing
pub fn sample_files() -> Vec<PathBuf> {
    vec![
        PathBuf::from("src/main.rs"),
        PathBuf::from("src/ui.rs"),
        PathBuf::from("src/search.rs"),
        PathBuf::from("src/app_state.rs"),
        PathBuf::from("Cargo.toml"),
        PathBuf::from("README.md"),
    ]
}

/// Create basic sample rust assets (State struct and render function)
pub fn sample_rust_assets() -> BTreeMap<PathBuf, Vec<TypeDefinition>> {
    let main_rs = Rc::new(PathBuf::from("src/main.rs"));
    let ui_rs = Rc::new(PathBuf::from("src/ui.rs"));

    let mut rust_assets = BTreeMap::new();
    rust_assets.insert(
        (*main_rs).clone(),
        vec![
            TypeDefinition {
                type_kind: TypeKind::Struct,
                name: "State".to_string(),
                file_path: Rc::clone(&main_rs),
                line_number: 79,
            },
            TypeDefinition {
                type_kind: TypeKind::Function,
                name: "render".to_string(),
                file_path: Rc::clone(&main_rs),
                line_number: 230,
            },
        ],
    );
    rust_assets.insert(
        (*ui_rs).clone(),
        vec![
            TypeDefinition {
                type_kind: TypeKind::Struct,
                name: "UIRenderer".to_string(),
                file_path: Rc::clone(&ui_rs),
                line_number: 10,
            },
        ],
    );

    rust_assets
}

/// Create rust assets with multiple structs for struct search testing
pub fn struct_search_rust_assets() -> BTreeMap<PathBuf, Vec<TypeDefinition>> {
    let main_rs = Rc::new(PathBuf::from("src/main.rs"));
    let types_rs = Rc::new(PathBuf::from("src/types.rs"));
    let state_rs = Rc::new(PathBuf::from("src/state.rs"));

    let mut rust_assets = BTreeMap::new();
    rust_assets.insert(
        (*main_rs).clone(),
        vec![
            TypeDefinition {
                type_kind: TypeKind::Struct,
                name: "State".to_string(),
                file_path: Rc::clone(&main_rs),
                line_number: 79,
            },
            TypeDefinition {
                type_kind: TypeKind::Function,
                name: "render".to_string(),
                file_path: Rc::clone(&main_rs),
                line_number: 230,
            },
        ],
    );
    rust_assets.insert(
        (*types_rs).clone(),
        vec![
            TypeDefinition {
                type_kind: TypeKind::Struct,
                name: "MyStruct".to_string(),
                file_path: Rc::clone(&types_rs),
                line_number: 10,
            },
            TypeDefinition {
                type_kind: TypeKind::Struct,
                name: "MyStructHelper".to_string(),
                file_path: Rc::clone(&types_rs),
                line_number: 25,
            },
        ],
    );
    rust_assets.insert(
        (*state_rs).clone(),
        vec![
            TypeDefinition {
                type_kind: TypeKind::Struct,
                name: "AppState".to_string(),
                file_path: Rc::clone(&state_rs),
                line_number: 8,
            },
        ],
    );

    rust_assets
}

/// Create rust assets with multiple enums for enum search testing
pub fn enum_search_rust_assets() -> BTreeMap<PathBuf, Vec<TypeDefinition>> {
    let types_rs = Rc::new(PathBuf::from("src/types.rs"));
    let search_rs = Rc::new(PathBuf::from("src/search.rs"));
    let events_rs = Rc::new(PathBuf::from("src/events.rs"));

    let mut rust_assets = BTreeMap::new();
    rust_assets.insert(
        (*types_rs).clone(),
        vec![
            TypeDefinition {
                type_kind: TypeKind::Enum,
                name: "SearchMode".to_string(),
                file_path: Rc::clone(&types_rs),
                line_number: 42,
            },
            TypeDefinition {
                type_kind: TypeKind::Enum,
                name: "SearchType".to_string(),
                file_path: Rc::clone(&types_rs),
                line_number: 58,
            },
            TypeDefinition {
                type_kind: TypeKind::Struct,
                name: "SearchHelper".to_string(),
                file_path: Rc::clone(&types_rs),
                line_number: 100,
            },
        ],
    );
    rust_assets.insert(
        (*search_rs).clone(),
        vec![
            TypeDefinition {
                type_kind: TypeKind::Enum,
                name: "SearchItem".to_string(),
                file_path: Rc::clone(&search_rs),
                line_number: 17,
            },
        ],
    );
    rust_assets.insert(
        (*events_rs).clone(),
        vec![
            TypeDefinition {
                type_kind: TypeKind::Enum,
                name: "EventType".to_string(),
                file_path: Rc::clone(&events_rs),
                line_number: 5,
            },
        ],
    );

    rust_assets
}

/// Create rust assets with multiple functions for function search testing
pub fn function_search_rust_assets() -> BTreeMap<PathBuf, Vec<TypeDefinition>> {
    let main_rs = Rc::new(PathBuf::from("src/main.rs"));
    let ui_rs = Rc::new(PathBuf::from("src/ui.rs"));
    let search_rs = Rc::new(PathBuf::from("src/search.rs"));

    let mut rust_assets = BTreeMap::new();
    rust_assets.insert(
        (*main_rs).clone(),
        vec![
            TypeDefinition {
                type_kind: TypeKind::Function,
                name: "render".to_string(),
                file_path: Rc::clone(&main_rs),
                line_number: 230,
            },
            TypeDefinition {
                type_kind: TypeKind::Function,
                name: "render_ui".to_string(),
                file_path: Rc::clone(&main_rs),
                line_number: 250,
            },
            TypeDefinition {
                type_kind: TypeKind::Struct,
                name: "RenderState".to_string(),
                file_path: Rc::clone(&main_rs),
                line_number: 50,
            },
        ],
    );
    rust_assets.insert(
        (*ui_rs).clone(),
        vec![
            TypeDefinition {
                type_kind: TypeKind::Function,
                name: "render_table".to_string(),
                file_path: Rc::clone(&ui_rs),
                line_number: 100,
            },
            TypeDefinition {
                type_kind: TypeKind::Function,
                name: "render_text".to_string(),
                file_path: Rc::clone(&ui_rs),
                line_number: 120,
            },
        ],
    );
    rust_assets.insert(
        (*search_rs).clone(),
        vec![
            TypeDefinition {
                type_kind: TypeKind::Function,
                name: "search".to_string(),
                file_path: Rc::clone(&search_rs),
                line_number: 42,
            },
        ],
    );

    rust_assets
}
