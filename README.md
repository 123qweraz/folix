# Folix

A cross-platform document reader and PDF toolbox, written in Rust.

Folix is a GPU-accelerated desktop application that reads PDF, EPUB, TXT, Markdown, DOCX, and image files. It combines a modern reader with annotation tools, auto-reading, full-text search, vocabulary management, and batch PDF operations—all in a tabbed, keyboard-friendly interface.

## Features

### Reading
- **Multi-format support**: PDF (via MuPDF), EPUB (via rbook), TXT/MD/DOCX (custom parsing), images
- **Two reading layouts**: Paged mode (traditional page-by-page) and Scroll mode (continuous vertical)
- **Table of Contents**: Navigate by chapter for all formats
- **Auto-reading**: Timed page advance or continuous scroll, 0.5x–5.0x speed
- **Bookmarks**: Per-document, persisted to SQLite
- **Reading position**: Auto-saved every 5s, restored on reopen

### Annotation (Deep Reading Mode)
- **Highlight**: Select text and apply one of 8 colors
- **Pen**: Freehand drawing on PDF pages
- **Note**: Attach text notes to highlighted regions
- **Eraser**: Remove annotations by clicking
- All annotations persisted to SQLite

### Text & Search
- **Text selection**: Click/drag, Shift-click, Ctrl+drag additive selection
- **Copy to clipboard**: Right-click context menu or Ctrl+C
- **Full-text search**: Sidebar panel with per-page match highlighting and result navigation
- **Vocabulary**: Right-click → "Add to Vocabulary" with context sentence
- **Sentence collection**: Save sentences with page references

### PDF Toolbox
- **Merge PDFs**: Combine multiple files in order
- **Split PDF**: By page range, every N pages, or by TOC chapters
- **Extract images**: Export each page as PNG
- **Extract text**: Export all text to .txt
- **Images to PDF**: Convert images to a single PDF
- **Page editing**: Rotate, delete, insert blank pages

### Other
- **Tabbed interface**: Multi-document browsing with middle-click close
- **MoYu (摸鱼) Mode**: Floating mini-reader for discreet reading
- **Keyboard shortcuts**: 20+ fully customizable shortcuts
- **i18n**: Chinese and English UI
- **Dark/Light mode**
- **Recent files**: Pinned and unpinned, with "Show in folder"
- **Drag-and-drop**: Open files by dragging into the window

## Supported Formats

| Format | Engine |
|--------|--------|
| PDF | MuPDF |
| EPUB | rbook + custom reflow |
| TXT | UTF-8 / GBK / Big5 / Shift_JIS auto-detection |
| Markdown (.md) | Custom parser (strips markdown syntax) |
| DOCX | ZIP + quick-xml |
| PNG, JPG, JPEG, GIF, BMP, WEBP, TIFF | image crate |

## Build

```bash
# Requires Rust 2021 edition
git clone https://github.com/your-username/folix.git
cd folix
cargo build --release
```

Run with:

```bash
cargo run --release
```

## Tech Stack

| Crate | Use |
|-------|-----|
| eframe / egui 0.31 | GUI framework (immediate mode, OpenGL) |
| mupdf 0.8 | PDF rendering and manipulation |
| rbook 0.7 | EPUB parsing |
| rusqlite 0.33 | Persistence (progress, annotations, bookmarks, vocabulary) |
| image 0.25 | Image decoding |
| tikv-jemallocator | Memory allocator |
| serde / serde_json | Config serialization |
| encoding_rs 0.8 | CJK text encoding detection |
| quick-xml 0.37 | DOCX XML parsing |

## Project Status

Folix is in active development (v0.1.0). EPUB image support and non-CJK font fallback are works in progress.


