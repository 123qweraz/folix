use mupdf::{
    pdf::{PdfDocument, PageRange, PageSelection, InsertPosition, InsertPdfOptions, PageImageSource, InsertImageOptions},
    Image, Colorspace, Matrix, ImageFormat, Size, Rect,
};

fn open_pdf(path: &str) -> Result<PdfDocument, String> {
    PdfDocument::open(path).map_err(|e| format!("open {path}: {e}"))
}

/// Get the number of pages in a PDF.
pub fn page_count(path: &str) -> Result<usize, String> {
    let doc = PdfDocument::open(path).map_err(|e| format!("open {path}: {e}"))?;
    let n = doc.page_count().map_err(|e| format!("page count {path}: {e}"))?;
    Ok(n as usize)
}

/// Merge multiple PDFs into one (append in order).
pub fn merge_pdfs(inputs: &[String], output: &str) -> Result<(), String> {
    if inputs.len() < 2 {
        return Err("Need at least 2 PDF files".into());
    }

    let mut doc = open_pdf(&inputs[0])?;
    for input in &inputs[1..] {
        let src = open_pdf(input)?;
        let opts = InsertPdfOptions {
            source_pages: PageSelection::All,
            target: InsertPosition::Append,
            rotate: None,
            copy_links: true,
            copy_annotations: true,
            copy_widgets: true,
        };
        doc.insert_pdf(&src, opts)
            .map_err(|e| format!("insert from {input}: {e}"))?;
    }
    doc.save(output).map_err(|e| format!("save: {e}"))?;
    Ok(())
}

/// Split PDF by page range (start..end, 1-indexed).
/// Uses PdfDocument + insert_pdf instead of convert_to_pdf to avoid mupdf-rs 0.8 memory corruption.
pub fn split_pdf_by_range(input: &str, output: &str, start: usize, end: usize) -> Result<(), String> {
    let src = open_pdf(input)?;
    let total = src.page_count().map_err(|e| format!("page count: {e}"))? as usize;
    let s = (start.max(1) - 1).min(total.saturating_sub(1));
    let e = end.max(start).min(total);
    if s >= e {
        return Err(format!("Invalid page range {start}..{end} (total pages: {total})"));
    }
    let mut out = PdfDocument::new();
    let opts = InsertPdfOptions {
        source_pages: PageSelection::Range(PageRange::new(s, e)),
        target: InsertPosition::Append,
        rotate: None,
        copy_links: true,
        copy_annotations: true,
        copy_widgets: true,
    };
    out.insert_pdf(&src, opts)
        .map_err(|e| format!("insert pages {start}..{end}: {e}"))?;
    out.save(output).map_err(|e| format!("save: {e}"))?;
    Ok(())
}

/// Split PDF into chunks of N pages each.
pub fn split_pdf_every_n(input: &str, output_dir: &str, n: usize) -> Result<Vec<String>, String> {
    let src = open_pdf(input)?;
    let total = src.page_count().map_err(|e| format!("page count: {e}"))? as usize;
    let n = n.max(1);
    let stem = std::path::Path::new(input)
        .file_stem().and_then(|s| s.to_str()).unwrap_or("split");
    let mut outputs = Vec::new();

    for chunk in (0..total).step_by(n) {
        let start = chunk;
        let end = (chunk + n).min(total);
        let out_path = format!("{output_dir}/{stem}_p{}-p{}.pdf", start + 1, end);
        let mut out = PdfDocument::new();
        let opts = InsertPdfOptions {
            source_pages: PageSelection::Range(PageRange::new(start, end)),
            target: InsertPosition::Append,
            rotate: None,
            copy_links: true,
            copy_annotations: true,
            copy_widgets: true,
        };
        out.insert_pdf(&src, opts)
            .map_err(|e| format!("insert pages {}-{}: {e}", start + 1, end))?;
        out.save(&out_path).map_err(|e| format!("save {out_path}: {e}"))?;
        outputs.push(out_path);
    }
    Ok(outputs)
}

/// Split PDF by TOC chapters.
pub fn split_pdf_by_toc(input: &str, output_dir: &str) -> Result<Vec<String>, String> {
    let src = open_pdf(input)?;
    let total = src.page_count().map_err(|e| format!("page count: {e}"))? as usize;
    let stem = std::path::Path::new(input)
        .file_stem().and_then(|s| s.to_str()).unwrap_or("split");
    // Read outlines
    let outlines = src.outlines().map_err(|e| format!("outlines: {e}"))?;
    let mut outputs = Vec::new();

    if outlines.is_empty() {
        return Err("No TOC entries found in this PDF".into());
    }

    let mut toc_pages: Vec<(String, usize)> = Vec::new();
    collect_outlines(&outlines, &mut toc_pages);
    toc_pages.sort_by_key(|(_, p)| *p);
    toc_pages.dedup_by_key(|(_, p)| *p);

    for (i, (title, start_page)) in toc_pages.iter().enumerate() {
        let end_page = toc_pages.get(i + 1).map(|(_, p)| *p).unwrap_or(total);
        if *start_page >= end_page {
            continue;
        }
        let safe_title: String = title.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let safe_title = if safe_title.is_empty() { format!("chapter_{}", i + 1) } else { safe_title };
        let out_path = format!("{output_dir}/{stem}_{safe_title}.pdf");
        let mut chapter_doc = PdfDocument::new();
        let opts = InsertPdfOptions {
            source_pages: PageSelection::Range(PageRange::new(*start_page, end_page)),
            target: InsertPosition::Append,
            rotate: None,
            copy_links: true,
            copy_annotations: true,
            copy_widgets: true,
        };
        chapter_doc.insert_pdf(&src, opts)
            .map_err(|e| format!("insert chapter {title}: {e}"))?;
        chapter_doc.save(&out_path).map_err(|e| format!("save {out_path}: {e}"))?;
        outputs.push(out_path);
    }
    Ok(outputs)
}

fn collect_outlines(outlines: &[mupdf::Outline], result: &mut Vec<(String, usize)>) {
    for o in outlines {
        let page = o.dest.map(|d| d.loc.page_number as usize).unwrap_or(0);
        result.push((o.title.clone(), page));
        collect_outlines(&o.down, result);
    }
}

/// Load TOC chapters from a PDF (returns flat list of title + page).
pub fn load_toc_chapters(path: &str) -> Result<Vec<(String, usize)>, String> {
    let doc = PdfDocument::open(path).map_err(|e| format!("open {path}: {e}"))?;
    let outlines = doc.outlines().map_err(|e| format!("outlines: {e}"))?;
    let mut result = Vec::new();
    collect_outlines(&outlines, &mut result);
    result.sort_by_key(|(_, p)| *p);
    result.dedup_by_key(|(_, p)| *p);
    Ok(result)
}

/// Extract PDF pages as PNG images.
pub fn extract_pages_as_images(input: &str, output_dir: &str, pages: &[usize]) -> Result<Vec<String>, String> {
    let doc = PdfDocument::open(input).map_err(|e| format!("open {input}: {e}"))?;
    let stem = std::path::Path::new(input)
        .file_stem().and_then(|s| s.to_str()).unwrap_or("page");
    let cs = Colorspace::device_rgb();
    let mut outputs = Vec::new();

    for &p in pages {
        let page_obj = doc.load_page(p as i32)
            .map_err(|e| format!("load page {}: {e}", p + 1))?;
        let pixmap = page_obj.to_pixmap(&Matrix::new_scale(2.0, 2.0), &cs, false, true)
            .map_err(|e| format!("render page {}: {e}", p + 1))?;
        let out_path = format!("{output_dir}/{stem}_p{}.png", p + 1);
        pixmap.save_as(&out_path, ImageFormat::PNG)
            .map_err(|e| format!("save page {}: {e}", p + 1))?;
        outputs.push(out_path);
    }
    Ok(outputs)
}

/// Extract entire PDF text to a .txt file.
pub fn extract_pdf_text(input: &str, output: &str) -> Result<(), String> {
    let doc = PdfDocument::open(input).map_err(|e| format!("open {input}: {e}"))?;
    let total = doc.page_count().map_err(|e| format!("page count: {e}"))? as usize;
    let mut text = String::new();

    for p in 0..total {
        if p > 0 {
            text.push_str("\n\n--- Page ");
            text.push_str(&(p + 1).to_string());
            text.push_str(" ---\n\n");
        }
        let page_obj = doc.load_page(p as i32)
            .map_err(|e| format!("load page {}: {e}", p + 1))?;
        let page_text = page_obj.text(Default::default())
            .map_err(|e| format!("extract text page {}: {e}", p + 1))?;
        text.push_str(&page_text);
    }

    std::fs::write(output, &text)
        .map_err(|e| format!("write {output}: {e}"))?;
    Ok(())
}

/// Convert one or more images to a single PDF.
pub fn images_to_pdf(inputs: &[String], output: &str) -> Result<(), String> {
    if inputs.is_empty() {
        return Err("No input images".into());
    }

    let mut doc = PdfDocument::new();

    for input in inputs {
        // Load image via mupdf
        let mupdf_img = Image::from_file(input)
            .map_err(|e| format!("load image {input}: {e}"))?;
        let w = mupdf_img.width() as f32;
        let h = mupdf_img.height() as f32;

        // Scale down to fit Letter size if too large
        let max_w = 612.0;
        let max_h = 792.0;
        let scale = (max_w / w).min(max_h / h).min(1.0);
        let pw = w * scale;
        let ph = h * scale;

        // Create page sized to image
        let mut page = doc.new_page_at(-1, Size::new(pw, ph))
            .map_err(|e| format!("create page for {input}: {e}"))?;

        // Insert image covering the full page
        page.insert_image(
            &mut doc,
            Rect::new(0.0, 0.0, pw, ph),
            PageImageSource::Image(&mupdf_img),
            InsertImageOptions::default(),
        ).map_err(|e| format!("insert image {input}: {e}"))?;
    }

    doc.save(output).map_err(|e| format!("save {output}: {e}"))?;
    Ok(())
}
