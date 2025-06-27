use zellij_tile::prelude::*;

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
        "vim", "nvim", "neovim", "vi", "emacs", "nano", "micro", "helix", "hx", "code", "subl",
        "atom", "notepad", "kak", "kakoune", "joe", "mcedit", "ed", "ex", "pico",
    ];

    if let Some(ref command) = pane_info.terminal_command {
        let command_lower = command.to_lowercase();
        if common_editors.iter().any(|&editor| {
            command_lower.contains(editor)
                || command_lower.starts_with(&format!("{} ", editor))
                || command_lower.ends_with(&format!("/{}", editor))
        }) {
            return true;
        }
    }

    let title_lower = pane_info.title.to_lowercase();
    common_editors.iter().any(|&editor| {
        title_lower.contains(editor)
            || title_lower.starts_with(&format!("{} ", editor))
            || title_lower.contains(&format!(" {} ", editor))
            || title_lower.ends_with(&format!(" {}", editor))
    })
}
