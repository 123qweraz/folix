use super::{Document, RenderedPage, TocEntry};
use mupdf::{Document as MuDocument, MetadataName, TextExtractOptions, Colorspace, Matrix};

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
    pages: Vec<String>,
    doc_title: String,
}

impl PdfDocument {
    pub fn open(path: &str) -> Option<Self> {
        let doc = MuDocument::open(path).ok()?;
        let count = doc.page_count().ok()? as usize;

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

        let mut pages = Vec::with_capacity(count);
        for i in 0..count {
            let text = match doc.load_page(i as i32) {
                Ok(page) => page.text(TextExtractOptions::default()).unwrap_or_default(),
                Err(_) => String::new(),
            };
            pages.push(text);
        }

        Some(Self {
            path: path.to_string(),
            pages,
            doc_title,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Document for PdfDocument {
    fn supports_image(&self) -> bool { true }

    fn page_count(&self) -> usize {
        self.pages.len()
    }

    fn page_text(&self, page: usize) -> String {
        if page < self.pages.len() {
            self.pages[page].clone()
        } else {
            String::new()
        }
    }

    fn title(&self) -> String {
        self.doc_title.clone()
    }

    fn metadata(&self, _key: &str) -> Option<String> {
        None
    }

    fn toc_entries(&self) -> Vec<TocEntry> {
        let doc = match MuDocument::open(&self.path) {
            Ok(d) => d,
            Err(_) => return vec![],
        };
        let outlines = match doc.outlines() {
            Ok(o) => o,
            Err(_) => return vec![],
        };
        let mut toc = Vec::new();
        flatten_outline(&outlines, 0, &mut toc);
        toc
    }

    fn render_page(&self, page: usize, scale: f32) -> Option<RenderedPage> {
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

        Some(RenderedPage { width: w, height: h, rgba })
    }
}
