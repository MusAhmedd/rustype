# Rustype

A note-taking app for Windows that looks and sounds like a typewriter, written in Rust with eframe/egui.

## Features

- A typewriter you type on: the paper moves as you write, the carriage returns, typebars swing, plus key sounds (four sound packs) and a small focus light
- Proper editing: cursor movement, selection, copy/cut/paste, undo/redo, soft word wrap, overtype with the Insert key
- Tabs, plus a home screen with search
- Colour tags, grid / list / stack views, and sorting
- Export to TXT, Markdown, PNG, JPEG and PDF (PNG, JPEG and PDF match the look of your sheet)
- Six typewriter typefaces and paper colours you can choose

Notes are plain `.txt` files in `Documents\Rustype Notes`.

## Build

```
cargo build --release
```

The binary is `target\release\rustype.exe`. It is statically linked and runs on its own.

## Fonts

Bundled fonts: Special Elite (Apache 2.0), plus Courier Prime, Cutive Mono, Space Mono and IBM Plex Mono (SIL Open Font License).
