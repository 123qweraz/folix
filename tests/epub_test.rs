use folix::app::engines::Document;

#[test]
fn test_open_chinese_epub() {
    let path = concat!(
        "tests/如何学习 ",
        "(本尼迪克特·凯里,Benedict Carey,玉冰)",
        " (z-library.sk, 1lib.sk, z-lib.sk).epub"
    );
    let doc = folix::app::engines::reflow_engine::ReflowDocument::open(path)
        .expect("Failed to open Chinese EPUB");
    assert!(doc.page_count() > 0, "Should have at least one chapter");
    let text = doc.page_text(0);
    assert!(!text.is_empty(), "First chapter should have text");
    let toc = doc.toc_entries();
    println!(
        "Chinese EPUB: {} chapters, {} toc entries, first chapter {} chars",
        doc.page_count(),
        toc.len(),
        text.chars().count()
    );
    for entry in &toc {
        println!("  ToC: {} (page {})", entry.label, entry.page_index);
    }
}

#[test]
fn test_open_english_epub() {
    let path = concat!(
        "tests/Building AI Agent Platforms ",
        "(Ben OMahony and Fabian Nonnenmacher)",
        " (z-library.sk, 1lib.sk, z-lib.sk).epub"
    );
    let doc = folix::app::engines::reflow_engine::ReflowDocument::open(path)
        .expect("Failed to open English EPUB");
    assert!(doc.page_count() > 0, "Should have at least one chapter");
    let text = doc.page_text(0);
    assert!(!text.is_empty(), "First chapter should have text");
    let toc = doc.toc_entries();
    println!(
        "English EPUB: {} chapters, {} toc entries, first chapter {} chars",
        doc.page_count(),
        toc.len(),
        text.chars().count()
    );
    for entry in &toc {
        println!("  ToC: {} (page {})", entry.label, entry.page_index);
    }
}
