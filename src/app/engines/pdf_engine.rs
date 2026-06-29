use super::{Document, RenderedPage, TocEntry, TextWordPosition};
use mupdf::{Document as MuDocument, MetadataName, TextExtractOptions, Colorspace, Matrix};
use parking_lot::Mutex;
use std::collections::HashMap;
use egui::{TextureId, TextureHandle};

fn flatten_outline(entries: &[mupdf::outline::Outline], depth: usize, out: &mut Vec<TocEntry>) {
    for entry in entries {
        let page = entry
            .dest
            .as_ref()
            .map(|d| d.loc.page_number as usize)
            .unwrap_or(0);
        out.push(TocEntry {
            label: format!("{}{}", "  ".repeat(depth), entry.title),
            page_index: page,
        });
        flatten_outline(&entry.down, depth + 1, out);
    }
}

pub struct PdfDocument {
    path: String,
    doc_title: String,
    page_count: usize,
    toc: Vec<TocEntry>,
    render_cache: Mutex<HashMap<usize, (f32, RenderedPage)>>,
    page_sizes_cache: Mutex<Option<Vec<(f32, f32)>>>,
    text_cache: Mutex<HashMap<usize, String>>,
    text_positions_cache: Mutex<HashMap<usize, Vec<TextWordPosition>>>,
    texture_handles: Mutex<HashMap<usize, (u32, TextureHandle)>>,
}

impl PdfDocument {
    pub fn open(path: &str) -> Option<Self> {
        let doc = MuDocument::open(path).ok()?;
        let page_count = doc.page_count().ok()? as usize;

        let doc_title = doc
            .metadata(MetadataName::Title)
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                std::path::Path::new(path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "Untitled".to_string());

        let toc = doc
            .outlines()
            .ok()
            .map(|outlines| {
                let mut toc = Vec::new();
                flatten_outline(&outlines, 0, &mut toc);
                toc
            })
            .unwrap_or_default();

        Some(Self {
            path: path.to_string(),
            doc_title,
            page_count,
            toc,
            render_cache: Mutex::new(HashMap::new()),
            page_sizes_cache: Mutex::new(None),
            text_cache: Mutex::new(HashMap::new()),
            text_positions_cache: Mutex::new(HashMap::new()),
            texture_handles: Mutex::new(HashMap::new()),
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Document for PdfDocument {
    fn supports_image(&self) -> bool {
        true
    }

    fn page_count(&self) -> usize {
        self.page_count
    }

    fn page_text(&self, page: usize) -> String {
        {
            let cache = self.text_cache.lock();
            if let Some(text) = cache.get(&page) {
                return text.clone();
            }
        }

        let text = match MuDocument::open(&self.path) {
            Ok(doc) => match doc.load_page(page as i32) {
                Ok(p) => p.text(TextExtractOptions::default()).unwrap_or_default(),
                Err(_) => String::new(),
            },
            Err(_) => String::new(),
        };

        {
            let mut cache = self.text_cache.lock();
            cache.insert(page, text.clone());
            if cache.len() > 5 {
                let oldest = *cache.keys().min().unwrap();
                cache.remove(&oldest);
            }
        }

        text
    }

    fn title(&self) -> String {
        self.doc_title.clone()
    }

    fn metadata(&self, _key: &str) -> Option<String> {
        None
    }

    fn toc_entries(&self) -> Vec<TocEntry> {
        self.toc.clone()
    }

    fn page_text_positions(&self, page: usize) -> Vec<TextWordPosition> {
        {
            let cache = self.text_positions_cache.lock();
            if let Some(positions) = cache.get(&page) {
                return positions.clone();
            }
        }

        let doc = match MuDocument::open(&self.path) {
            Ok(d) => d,
            Err(_) => return vec![],
        };
        let page_obj = match doc.load_page(page as i32) {
            Ok(p) => p,
            Err(_) => return vec![],
        };
        let words = match page_obj.words(TextExtractOptions::default()) {
            Ok(w) => w,
            Err(_) => return vec![],
        };
        let positions: Vec<TextWordPosition> = words
            .into_iter()
            .map(|w| TextWordPosition {
                text: w.text,
                x0: w.bounds.x0,
                y0: w.bounds.y0,
                x1: w.bounds.x1,
                y1: w.bounds.y1,
            })
            .collect();

        {
            let mut cache = self.text_positions_cache.lock();
            cache.insert(page, positions.clone());
        }

        positions
    }

    fn render_page(&self, page: usize, scale: f32) -> Option<RenderedPage> {
        {
            let cache = self.render_cache.lock();
            if let Some((cached_scale, cached)) = cache.get(&page) {
                if (*cached_scale - scale).abs() < 0.001 {
                    return Some(RenderedPage {
                        width: cached.width,
                        height: cached.height,
                        rgba: cached.rgba.clone(),
                    });
                }
            }
        }

        let doc = MuDocument::open(&self.path).ok()?;
        let page_obj = doc.load_page(page as i32).ok()?;
        let cs = Colorspace::device_rgb();
        let ctm = Matrix::new_scale(scale, scale);
        let pixmap = page_obj.to_pixmap(&ctm, &cs, false, true).ok()?;

        let w = pixmap.width();
        let h = pixmap.height();
        let samples = pixmap.samples();
        let n = pixmap.n() as usize;

        let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
        if n == 4 {
            for chunk in samples.chunks(4) {
                rgba.extend_from_slice(&chunk[..4]);
            }
        } else {
            for chunk in samples.chunks(3) {
                rgba.extend_from_slice(&chunk[..3]);
                rgba.push(255);
            }
        }

        let rendered = RenderedPage {
            width: w,
            height: h,
            rgba,
        };
        {
            let mut cache = self.render_cache.lock();
            cache.insert(page, (scale, rendered.clone()));
            if cache.len() > 2 {
                let oldest = *cache.keys().min().unwrap();
                cache.remove(&oldest);
            }
        }
        Some(rendered)
    }

    fn page_size(&self, page: usize, scale: f32) -> Option<(f32, f32)> {
        {
            let cache = self.page_sizes_cache.lock();
            if let Some(sizes) = cache.as_ref() {
                return sizes.get(page).map(|&(w, h)| (w * scale, h * scale));
            }
        }

        let sizes: Vec<(f32, f32)> = match MuDocument::open(&self.path) {
            Ok(doc) => {
                let count = self.page_count;
                let mut sizes = Vec::with_capacity(count);
                for i in 0..count {
                    match doc.load_page(i as i32) {
                        Ok(p) => {
                            let bounds = p.bounds().ok();
                            sizes.push(
                                bounds
                                    .map(|b| (b.width(), b.height()))
                                    .unwrap_or((612.0, 792.0)),
                            );
                        }
                        Err(_) => {
                            sizes.push((612.0, 792.0));
                        }
                    }
                }
                sizes
            }
            Err(_) => return None,
        };

        {
            let mut cache = self.page_sizes_cache.lock();
            *cache = Some(sizes.clone());
        }

        sizes.get(page).map(|&(w, h)| (w * scale, h * scale))
    }

    fn get_texture_handle(&self, page: usize, scale: f32) -> Option<(TextureId, [usize; 2])> {
        let cache = self.texture_handles.lock();
        cache.get(&page)
            .filter(|(s, _)| *s == scale.to_bits())
            .map(|(_, h)| (h.id(), h.size()))
    }

    fn set_texture_handle(&self, page: usize, scale: f32, handle: TextureHandle) {
        let mut cache = self.texture_handles.lock();
        cache.insert(page, (scale.to_bits(), handle));
        if cache.len() > 10 {
            let oldest = *cache.keys().min().unwrap();
            cache.remove(&oldest);
        }
    }
}
