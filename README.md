# Ghosttify

Rust based tool for converting `gnome-terminal` shortcuts to `ghostty` keybindings. `ghosttify` can update a `ghostty` config with these converted keybindings by adding a `config-file` named `gnome-shortcuts`. `ghosttify` will look for the main config file at `~/.config/ghostty/config`, add the config-file to the main config if it is not already declared, and create/update the `gnome-shortcuts` file with the found `gnome-terminal` shortcuts.

ghostty keybindings: https://ghostty.org/docs/config/keybind/reference

gnome-terminal shortcuts: https://systemshortcuts.com/gnome-terminal/

## Flags:
- -a, --apply: Creates or updates the "gnome-shortcuts" file with the converted `gnome-terminal` shortcuts.
- -c, --avoid-conflict: When applying the keybindings, only apply those for actions that don't already have keybindings and keybindings that have not been bound to an action.
- --gnome: Print the found `gnome-terminal` shortcuts.
- --ghosttify: Print the found non-default `ghostty` shortcuts. These are the shortcuts that have been found in the default config and declared config files.

## Installation

To install either clone the repository and build from source or use `cargo`.

1.  
    ```bash
    $ git clone https://github.com/natibek/ghosttify.git
    $ cd ghosttify
    $ cargo build  
    ```
    Delete the repository and move the binary from `~/ghottify/target/debug/ghottify` to a directory in your `PATH`.


2. ```$ cargo install ghosttify```

