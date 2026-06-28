use mupdf::{pdf::PdfDocument, Size};

pub fn rotate_page(path: &str, page: usize, degrees: i32) -> Result<(), String> {
    let doc = PdfDocument::open(path).map_err(|e| format!("open: {e}"))?;
    let mut page_obj = doc
        .load_pdf_page(page as i32)
        .map_err(|e| format!("load page {page}: {e}"))?;
    page_obj
        .set_rotation(degrees)
        .map_err(|e| format!("rotate: {e}"))?;
    doc.save(path).map_err(|e| format!("save: {e}"))?;
    Ok(())
}

pub fn delete_page(path: &str, page: usize) -> Result<(), String> {
    let mut doc = PdfDocument::open(path).map_err(|e| format!("open: {e}"))?;
    let page_obj = doc
        .load_pdf_page(page as i32)
        .map_err(|e| format!("load page {page}: {e}"))?;
    drop(page_obj);
    doc.delete_page(page as i32)
        .map_err(|e| format!("delete page {page}: {e}"))?;
    doc.save(path).map_err(|e| format!("save: {e}"))?;
    Ok(())
}

pub fn insert_blank_page(path: &str, after_page: usize) -> Result<(), String> {
    let mut doc = PdfDocument::open(path).map_err(|e| format!("open: {e}"))?;
    let (w, h) = {
        let page = doc
            .load_pdf_page(after_page as i32)
            .map_err(|e| format!("load page {after_page}: {e}"))?;
        let bounds = page.bounds().map_err(|e| format!("bounds: {e}"))?;
        (bounds.width(), bounds.height())
    };
    doc.new_page_at((after_page + 1) as i32, Size::new(w, h))
        .map_err(|e| format!("insert page: {e}"))?;
    doc.save(path).map_err(|e| format!("save: {e}"))?;
    Ok(())
}
