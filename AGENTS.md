# Folix — PDF/EPUB Reader

**Status**: Working prototype. Runnable binary with egui window, mode switching, and document loading.

## Stack
- **Language**: Rust (edition 2021)
- **GUI**: egui + eframe (0.31), wgpu (24.0) backend
- **PDF**: Text + image rendering via `mupdf` (0.8, base14-fonts)
- **EPUB**: Parsing via `epub` crate (2.1.5)
- **TXT**: std `read_to_string` + `encoding_rs` for GBK/Big5/Shift_JIS
- **Text Layout**: `cosmic-text` (0.19) — HarfBuzz shaping, fontdb discovery, Swash rasterization
- **Storage**: SQLite via `rusqlite` (0.33, bundled)
- **Other**: `serde`, `uuid`, `chrono`, `image`, `rfd` (file dialogs)

## Quick start
```bash
cargo run            # launch the GUI app
cargo test           # run all tests (pdf, txt, epub)
cargo build          # compile (no warnings)
```

## Architecture
Core model: **Mode state machine** — `Reading` / `Auto` / `Annotate`.

```
Input → Mode System → Mode Handler → per-mode UI + scoped features
```

Directory structure follows `plan.md`:
- `src/app/core/` — `AppState`, `Mode` enum, `DocumentManager`, `FeatureSystem`
- `src/app/engines/` — `Document` trait + `PdfDocument` / `ReflowDocument`
- `src/app/ui/` — egui shell (`FolixApp`), mode-specific UI
- `src/app/render/` — `TextRenderer` (cosmic-text wrapper), wgpu stubs
- `src/app/storage/` — SQLite `Database` with schema for books/progress/annotations/etc.
- `src/app/interaction/`, `auto_reading/`, `annotation/`, `layout/`, `services/`, `platform/` — stubs

## Text rendering (cosmic-text)
Reading mode's Text view uses `TextRenderer` (`render/text_renderer.rs`):
- Creates `FontSystem` once at app startup (fontdb scans system dirs)
- Each frame: `render(text, max_width, font_size) → (w, h, RGBA)`
- Result cached in `ReadingState.text_cache`, invalidated on page/scale/width change
- Displayed as `egui::ColorImage` → `ui.image()` in a `ScrollArea`

Benefits over egui's native text: HarfBuzz shaping for all scripts, per-char font fallback via fontdb, color emoji via Swash.

## Key conventions
- UI never operates on documents directly; it routes through modes (`ModeController` trait).
- Document is wrapped in `Arc<Mutex<Box<dyn Document>>>` — cheaply cloneable for UI.
- `render_reading`/`render_auto`/`render_annotate` take `document: &Option<Arc<...>>` separately to avoid borrow conflicts with mode state.

## What's implemented (working)
- egui window with menu bar (File → Open/Close/Quit, Mode switch, Help → About)
- Mode toolbar: switches between Reading / Auto / Annotate
- File open dialog (rfd) for PDF, EPUB, TXT
- Reading mode: page nav, zoom slider, Text view (cosmic-text) / Image view (MuPDF Pixmap) toggle
- Auto mode: play/pause, speed control, auto-play mode selector (PageFlow/GlyphReveal/SentenceStream)
- Annotate mode: tool selector (Highlight/Pen/Note/Eraser/Select), undo/clear
- SQLite schema creation (6 tables)
- CJK text rendering (cosmic-text font fallback via fontdb)
- Multi-encoding TXT (UTF-8, GBK, Big5, Shift_JIS)
- EPUB text extraction (HTML stripping, encoding detection)

## What's stubbed (module structure exists, no logic)
- `auto_reading/`, `annotation/`, `layout/`, `services/`, `platform/`
- `storage/feature_store.rs`, `storage/library_index.rs`
- `interaction/input_router.rs`, `interaction/mode_handlers.rs`
- `render/wgpu_renderer.rs`, `render/tile_cache.rs`, `render/overlay_compositor.rs`

## Development notes
- Rust 1.96.0, edition 2021
- eframe 0.31 API: `Frame::NONE`, `Margin::symmetric(i8, i8)`, `Label::wrap()` (no arg), `ComboBox::from_id_salt`
- `epub::doc::EpubDoc::mdata()` returns `Option<&MetadataItem>` (access `.value` field), `get_resource(&mut self, id)` returns `Option<(Vec<u8>, String)>`
- `mupdf::Document` is !Send + !Sync; PdfDocument drops handle in `open()` and `render_page()` to stay Send+Sync
- `cosmic_text::FontSystem::new()` scans all system fonts (slow first call, ok at startup)
- `cosmic_text::Buffer::draw()` takes `(&mut FontSystem, &mut SwashCache, Color, FnMut)` — the `FontSystem` must NOT be borrowed by `borrow_with` at call time
- `TextRenderer::render()` creates a new `Buffer` per call; OK for document text, not for realtime UI
- Read `plan.md` for full design doc.
