# TuxSplit

A clean, opinionated, GTK4-based speedrun timer for Linux. Load your LiveSplit splits, start your run, and keep an eye on your times with a streamlined, distraction-free interface.

![Screenshot](https://raw.githubusercontent.com/AntonioRodriguezRuiz/tuxsplit/main/assets/screenshot.png)
> Obviously fake splits.

TuxSplit is currently in a very early development phase. The checklist below tracks what's ready today and what’s planned next, so you can quickly see the state of the app and what will be added down the line.

---

## Compile

TuxSplit isn’t bundled yet. You can build it from source:

1) Install Rust (stable) and Cargo
   - https://rustup.rs

2) Install GTK4 and libadwaita development packages
   - Debian/Ubuntu:
   ```bash
   $ sudo apt install libgtk-4-dev libadwaita-1-dev
   ```
   - Fedora:
   ```bash
   $ sudo dnf install gtk4-devel libadwaita-devel
   ```
   - Arch:
   ```bash
   $ sudo pacman -S gtk4 libadwaita
   ```

3) Build and run
   ```bash
   $ cargo run --release
   ```
   or build the binary:
   ```bash
   $ cargo build --release && ./target/release/tuxsplit
   ```

## Quick start

1) Launch TuxSplit
2) Click the menu button (top-left) → Load Splits → pick your .lss file
3) Press the Start/Split key (see defaults below) and run!

Tip: Use the same menu to Save Splits when you're done. There are currently no automatic saves nor prompts when closing the app.

---

## Default hotkeys

- Start / Split: Numpad 1
- Skip Split: Numpad 2
- Reset: Numpad 3
- Previous Comparison: Numpad 4
- Pause: Numpad 5
- Next Comparison: Numpad 6
- Undo: Numpad 8

Hotkeys are handled by the app even when the window isn’t focused (global hotkeys). On Linux, this currently relies on the X11 backend.

---

## Features

- [x] Splits
  - [x] Load existing LiveSplit splits (.lss)
  - [x] Save splits back to the same file
  - [x] Splits list with current segment highlighting
  - [ ] Subsplits
  - [ ] Scrollable list of splits
  - [ ] Multi-column splits view (name, time, delta, best segment, etc)
  - [ ] Split editor/creator
  - [ ] Drag-and-drop to open splits
- [x] Timer
  - [x] Start / Split
  - [x] Pause / Resume
  - [x] Reset
  - [x] Undo
- [x] Comparisons
  - [x] Switch comparisons via hotkeys (previous/next)
- [x] Hotkeys
  - [x] Global hotkeys on X11/XWayland
  - [x] In-app Keybindings overview dialog
  - [ ] Editable keybindings (rebind keys from the UI)
  - [ ] Wayland global hotkeys support (through xdg portals)
- [x] UI
  - [x] Run info display (Game and Category)
  - [ ] “Always on top” toggle
  - [ ] Layout customization (rows, columns, sizes, themes)
  - [ ] Translations (multi-language)
  - [ ] Enable/Disable color smooth color transitions (segment highlight)
- [ ] Settings
  - [ ] Settings screen (in-app)
  - [ ] Export/import settings
- [ ] Auto-splitters
  - [ ] Auto-splitter loading and management from the UI
- [ ] Integrations
  - [ ] Twitch/stream overlays and integrations
- [ ] Distribution
  - [ ] App packaging and distribution improvements
- [ ] Customizations
  - [ ] Custom split colors and styles
  - [ ] Custom comparisons
  - [ ] Flexible time display formats (hours/minutes/seconds/decimals, dynamic).

---


## Notes and limitations

- Global hotkeys: TuxSplit currently targets X11. On Wayland sessions, it runs through XWayland; if XWayland isn’t available, global hotkeys may not register.
- Splits format: TuxSplit reads and writes LiveSplit’s .lss files.
- Some menu items (like Settings) are visible but not implemented yet—see the roadmap above.

---

## Feedback

Have an idea or found an issue? Feel free to open an issue in the repository!!

Thanks for trying TuxSplit!
