# Folix — PDF/EPUB/MD/DOCX/Image Reader

**Status**: Working prototype. Runnable binary with egui window, mode switching, and document loading.

## Stack
- **Language**: Rust (edition 2021)
- **GUI**: egui + eframe (0.31), Glow renderer
- **PDF**: Text + image rendering via `mupdf` (0.8, base14-fonts)
- **EPUB**: Parsing via `rbook` (0.7)
- **TXT/Markdown**: std `read_to_string` + `encoding_rs` for GBK/Big5/Shift_JIS; markdown formatting stripped via `strip_markdown()`
- **DOCX**: ZIP + `quick-xml` (0.37) parsing of OOXML `word/document.xml`; heading styles → chapters
- **Images**: `image` crate (0.25) with `ImageDocument` implementing `FixedLayout` (PNG, JPEG, GIF, BMP, WebP, TIFF)
- **Storage**: SQLite via `rusqlite` (0.33, bundled)
- **Other**: `serde`, `uuid`, `chrono`, `image`, `rfd` (file dialogs), `parking_lot`, `zip`, `quick-xml`

## Quick start
```bash
cargo run            # launch the GUI app
cargo test           # run all tests (pdf, txt, epub, markdown, docx, image)
cargo build          # compile (no warnings)
```

## Architecture

### Document trait hierarchy
```
Document (title, toc, metadata)
  ├── FixedLayout   — page-based: PDF (MuPDF), images (PNG/JPG/GIF/BMP/WebP/TIFF via `image` crate)
  └── ReflowLayout  — chapter+block-based: EPUB, TXT, Markdown, DOCX

DocumentHandle enum { Fixed(Box<dyn FixedLayout>), Reflow(Box<dyn ReflowLayout>) }
    → wrapped in Arc<Mutex<DocumentHandle>> for UI
```

### Mode state machine
Core model: `LightReading` / `DeepReading` / `Edit`.

```
TabModes {
    page            // shared page number — single source of truth
    scale           // shared zoom
    reading_layout  // Paged / Scroll
    reading         // ReadingState (view_mode, sidebar, search, bookmarks, scroll)
    auto            // AutoState (playing, speed, auto_mode, progress)
    annotate        // AnnotateState (tool, annotations, selection, stroke_points)
    edit            // EditState (empty)
    active          // ModeKind
}
```

### Paginator chapter-aligned pages
Paginator forces each chapter to start on a new page. Pages are subdivisions of chapters (not the reverse).
`ensure_paginator()` hardcodes 800×1000 viewport; `set_viewport()` exists but is unused.

### LayoutStream continuous scroll
Reflow (EPUB/TXT) content is rendered as a continuous scroll stream. Pages 0..=stream_page_end
are all rendered in a single egui `ScrollArea` with stable `id_salt("reflow_stream")`. When the
user scrolls to within 15px of the bottom, the next page is appended to the stream. Page navigation
(◀▶, TOC, shortcuts) sets `stream_jump_to` + `stream_page_end`; the renderer estimates scroll offset
from average page height if the exact Y offset isn't yet cached.

### Unified rendering
`mode_ui.rs:render_document()` dispatches by doc type:
- **Fixed** → `render_paged()` or `render_scroll()` → `render_image_page()` (texture-cached MuPDF raster)
- **Reflow** → `Paginator` character-based page splitting → `egui::Label::wrap()` via ScrollArea

Images always centered; all interaction (selection, strokes, annotations, search highlights) is drawn as overlays in `render_image_page()`.

### Toolbars
Two-row layout at bottom. Row 1 = mode tabs + basic reading controls (◀ ▶ Zoom Paged/Scroll). Row 2 = mode-specific (Light=Auto-play, Deep=Annotation tools, Edit=Page ops).

```
Input → Mode System → Mode Handler → per-mode UI + scoped features
```

## Directory structure
- `src/app/config.rs` — ConfigData load/save (serde_json)
- `src/app/core/` — AppState, TabModes, DocumentManager, FeatureSystem, shortcuts
- `src/app/engines/` — Document/FixedLayout/ReflowLayout traits + PdfDocument / ImageDocument / ReflowDocument / edit_operations
- `src/app/ui/` — FolixApp (eframe shell), mode_ui (rendering + interaction), feature_ui
- `src/app/paginator/` — Paginator (character-based page splitting for reflow content)
- `src/app/storage/` — Database (SQLite CRUD for books/progress/annotations/bookmarks)
- `src/app/platform/` — font_loader (CJK font path scan), fs wrapper
- `src/app/interaction/` — input_router (minimal), mode_handlers (stub)
- `src/app/render/` — wgpu_renderer / tile_cache / overlay_compositor (all stubs, unused)
- `src/app/auto_reading/` — controller (minimal), page_flow/glyph_reveal/sentence_stream (stubs)
- `src/app/annotation/` — engine / tool_system / overlay_renderer (all stubs — real logic lives in mode_ui.rs)
- `src/app/layout/` — line_breaker / paginator / glyph_cache (all stubs — real paginator is in app/paginator/)
- `src/app/services/` — search / bookmark / usage_tracker / annotation_service (all stubs)

## Key conventions
- `page`/`scale`/`reading_layout` are shared in `TabModes` (not per-mode) — single source of truth.
- Document is wrapped in `Arc<Mutex<Box<dyn Document>>>` — cheaply cloneable for UI.
- **Unified rendering**: ALL modes use the same `render_document()` function. Mode-specific features (auto-play, annotations) pass as optional params.
- **PDF texture cache**: MuPDF pages rendered to RGBA, cached as egui textures (LRU, 2 entries). Invalidated on scale change.
- **Reflow pagination**: `Paginator` puts each chapter on its own page (no character-based splitting). Images get their own page within a chapter. Page count = chapter count (plus image-only pages).

## What's implemented (working)
- egui window with menu bar (File → Open/Close/Quit, Mode switch, Help → About)
- 3 modes: LightReading (basic + auto-play), DeepReading (basic + annotation), Edit (basic + page ops)
- Two-row bottom toolbar: Row 1 = shared controls, Row 2 = mode-specific
- Settings tab (⚙) with toolbar icon size, visibility, background color, keyboard shortcut editor
- File open dialog (rfd) for PDF, EPUB, TXT, MD, DOCX, PNG, JPG, BMP, GIF, WebP, TIFF
- All modes: page nav, zoom slider, Paged/Scroll layout toggle
- LightReading: play/pause, speed control, auto-play mode selector
- DeepReading: tool selector (Highlight/Pen/Note/Eraser/Select), undo/clear, text selection + copy
- Edit mode: page rotate (CW/CCW), delete, insert blank page
- PDF rendering via MuPDF with GPU texture caching
- EPUB text extraction (HTML tag stripping, encoding detection, image embedding)
- Multi-encoding TXT (UTF-8, GBK, Big5, Shift_JIS)
- Markdown (.md) with formatting stripped, headings as chapter boundaries
- DOCX (.docx) via ZIP + quick-xml OOXML parsing; Heading styles → chapters
- Image rendering (PNG, JPEG, GIF, BMP, WebP, TIFF) via `image` crate; single-page FixedLayout
- SQLite schema + CRUD for books, progress, annotations, bookmarks, feature_usage, search_index
- Paginator for reflowable content (character-based page splitting)
- LayoutStream continuous scroll: pages rendered as a single continuous stream, auto-appended on scroll-to-bottom
- CJK font loading (font_loader scans system paths)
- Dark mode toggle
- Keyboard shortcut system (20+ actions, configurable)
- Tab management (add, close, recent files new tab page)
- Search (sidebar, per-page or per-chapter matching with highlight)

## What's stubbed (module structure exists, no logic)
- `auto_reading/` sub-modules (page_flow, glyph_reveal, sentence_stream)
- `annotation/` (all real annotation logic lives inline in mode_ui.rs)
- `layout/` (real paginator is in `app/paginator/`)
- `render/` wgpu_renderer, overlay_compositor (tile_cache is functional but unused)
- `services/` search, bookmark, usage_tracker, annotation_service
- `interaction/mode_handlers.rs`
- `storage/feature_store.rs`, `storage/library_index.rs`

## Development notes
- Rust 1.96.0, edition 2021
- eframe 0.31 API: `Frame::NONE`, `Margin::symmetric(i8, i8)`, `Label::wrap()` (no arg), `ComboBox::from_id_salt`
- `mupdf::Document` is !Send + !Sync; PdfDocument drops handle in `open()` and `render_page()` to stay Send+Sync
- `rbook::Epub::open()` → `epub.metadata().title()`, `epub.reader()` for spine iteration, `epub.read_resource_bytes()` for resource access
- See `CAVEATS.md` for known pitfalls: parking_lot::Mutex deadlock rules and UTF-8 slicing safety.
- **Architecture docs**: after any architectural change (new module, trait refactor, dependency swap), update this file to match.
- **Git discipline**: after every successful `cargo build`, run `git add -A && git commit -m "..."` to save progress.
- Read `plan.md` for full design doc.
