<div align="center">
  <img src="https://github.com/user-attachments/assets/a5e65fd8-fff1-4e6e-82e8-934fec7c47a3" alt="preview">
</div>

## About
This [Zellij][zellij] plugin is a fuzzy finder tailored for Rust developers.

When opened inside a `git` folder, it searches through files in the project, as well as editor panes already opened to those files (prioritizing the latter). Pressing `Enter` or `Tab` will replace `Grab` with an `$EDITOR` pane opened to this file.

If a search term begins with `struct`, `enum` or `fn` followed by space, `Grab` will fuzzy find these Rust entities in the project instead of files. When selected with `Enter` or `Tab`, it will be replaced with an `$EDITOR` pane opened to the relevant file (and the relevant line!)

[zellij]: https://github.com/zellij-org/zellij

## Recommended Usage
Grab works best when bound to a certain key (for example `Alt 0`) and then used as necessary instead of opening a new pane with `Alt n`.

### Example

```kdl
shared_except "locked" {
    bind "Alt 0" {
        LaunchPlugin "file:/home/aram/.config/zellij/plugins/grab.wasm"
    }
}
```

## Installation

1. Download `grab.wasm` from the latest release
2. Place it in `~/.config/zellij/plugins`
3. Bind a key to launch it (see example above)
