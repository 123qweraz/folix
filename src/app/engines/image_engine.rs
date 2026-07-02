use super::{FixedLayout, Document, RenderedPage, TocEntry, TextWordPosition};
use parking_lot::Mutex;
use std::collections::HashMap;
use egui::{TextureId, TextureHandle};

pub struct ImageDocument {
    path: String,
    doc_title: String,
    img_width: u32,
    img_height: u32,
    render_cache: Mutex<HashMap<usize, (f32, RenderedPage)>>,
    texture_handles: Mutex<HashMap<usize, (u32, TextureHandle)>>,
}

impl ImageDocument {
    pub fn open(path: &str) -> Option<Self> {
        let reader = image::ImageReader::open(path).ok()?;
        let (img_width, img_height) = reader.into_dimensions().ok()?;
        let doc_title = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();
        Some(Self {
            path: path.to_string(),
            doc_title,
            img_width,
            img_height,
            render_cache: Mutex::new(HashMap::new()),
            texture_handles: Mutex::new(HashMap::new()),
        })
    }
}

impl Document for ImageDocument {
    fn title(&self) -> String {
        self.doc_title.clone()
    }

    fn toc_entries(&self) -> Vec<TocEntry> {
        vec![]
    }

    fn metadata(&self, _key: &str) -> Option<String> {
        None
    }
}

impl FixedLayout for ImageDocument {
    fn page_count(&self) -> usize {
        1
    }

    fn render_page(&self, page: usize, scale: f32) -> Option<RenderedPage> {
        if page != 0 {
            return None;
        }
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

        let img = image::open(&self.path).ok()?;
        let new_w = (self.img_width as f32 * scale).ceil() as u32;
        let new_h = (self.img_height as f32 * scale).ceil() as u32;
        let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);
        let rgba = resized.into_rgba8().into_raw();

        let rendered = RenderedPage {
            width: new_w,
            height: new_h,
            rgba,
        };

        {
            let mut cache = self.render_cache.lock();
            cache.insert(page, (scale, rendered.clone()));
            if cache.len() > 4 {
                let oldest = *cache.keys().min().unwrap();
                cache.remove(&oldest);
            }
        }

        Some(rendered)
    }

    fn page_size(&self, page: usize, scale: f32) -> Option<(f32, f32)> {
        if page != 0 { return None; }
        Some((self.img_width as f32 * scale, self.img_height as f32 * scale))
    }

    fn page_text(&self, _page: usize) -> String {
        String::new()
    }

    fn page_text_positions(&self, _page: usize) -> Vec<TextWordPosition> {
        vec![]
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
        if cache.len() > 4 {
            let oldest = *cache.keys().min().unwrap();
            cache.remove(&oldest);
        }
    }
}
