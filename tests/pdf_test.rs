use folix::app::engines::{Document, FixedLayout};

#[test]
fn test_open_math_pdf() {
    let path = "testsdoc/Math.pdf";
    let doc = folix::app::engines::pdf_engine::PdfDocument::open(path)
        .expect("Failed to open Math.pdf");
    assert!(doc.page_count() > 0);
    let text = doc.page_text(0);
    assert!(!text.is_empty(), "Page 0 should have text");
    println!("Math.pdf: {} pages, title: {:?}", doc.page_count(), doc.title());

    let toc = doc.toc_entries();
    for entry in &toc {
        println!("  ToC: {} (page {})", entry.label, entry.page_index);
    }
}
