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
    assert_eq!(doc.page_count(), 1, "Should be single continuous document");
    let text = doc.page_text(0);
    assert!(!text.is_empty(), "Should have text content");
    assert!(text.len() > 1000, "Full text should be long");
    let toc = doc.toc_entries();
    println!(
        "Chinese EPUB: {} total chars, {} toc entries, first 80 chars: {:?}",
        text.chars().count(),
        toc.len(),
        text.chars().take(80).collect::<String>(),
    );
    for entry in &toc {
        println!("  ToC: {} (char offset {})", entry.label, entry.page_index);
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
    assert_eq!(doc.page_count(), 1, "Should be single continuous document");
    let text = doc.page_text(0);
    assert!(!text.is_empty(), "Should have text content");
    assert!(text.len() > 500, "Full text should be substantial");
    let toc = doc.toc_entries();
    println!(
        "English EPUB: {} total chars, {} toc entries, first 60 chars: {:?}",
        text.chars().count(),
        toc.len(),
        text.chars().take(60).collect::<String>(),
    );
    for entry in &toc {
        println!("  ToC: {} (char offset {})", entry.label, entry.page_index);
    }
}
