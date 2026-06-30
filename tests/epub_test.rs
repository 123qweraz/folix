use std::time::Instant;
use folix::app::engines::{Document, ContentBlock, reflow_engine::ReflowDocument};

#[test]
fn test_open_epub_books() {
    for path in &[
        "testsdoc/Building AI Agent Platforms (Ben OMahony and Fabian Nonnenmacher) (z-library.sk, 1lib.sk, z-lib.sk).epub",
        "testsdoc/如何学习 (本尼迪克特·凯里,Benedict Carey,玉冰) (z-library.sk, 1lib.sk, z-lib.sk).epub",
    ] {
        let start = Instant::now();
        let result = ReflowDocument::open(path);
        let elapsed = start.elapsed();
        assert!(result.is_some(), "Failed to open: {}", path);
        let doc = result.unwrap();
        println!("Opened {} in {:?}", path, elapsed);
        println!("  title: {}", doc.title());
        println!("  text len: {}", doc.page_text(0).len());
        println!("  blocks: {}", doc.content_blocks(0).len());
        println!("  toc entries: {}", doc.toc_entries().len());

        let img_count = doc.content_blocks(0).iter()
            .filter(|b| matches!(b, ContentBlock::Image(_)))
            .count();
        println!("  image blocks: {}", img_count);
        assert!(img_count > 0, "Expected at least 1 image block in {}", path);
    }
}

#[test]
fn test_open_chinese_epub_fast() {
    // Must open quickly (< 2s now that image decode is lazy)
    let path = "testsdoc/如何学习 (本尼迪克特·凯里,Benedict Carey,玉冰) (z-library.sk, 1lib.sk, z-lib.sk).epub";
    let start = Instant::now();
    let result = ReflowDocument::open(path);
    let elapsed = start.elapsed();
    assert!(result.is_some(), "Failed to open");
    println!("Opened in {:?}", elapsed);
    assert!(elapsed.as_secs() < 2, "Open took too long: {:?}", elapsed);

    let doc = result.unwrap();
    let blocks = doc.content_blocks(0);
    assert!(blocks.len() > 0, "No content blocks");

    let images: Vec<&ContentBlock> = blocks.iter()
        .filter(|b| matches!(b, ContentBlock::Image(_)))
        .collect();
    assert!(images.len() >= 10, "Expected >=10 image blocks, got {}", images.len());
    println!("Images found: {}", images.len());
}

#[test]
fn test_image_dimensions_valid() {
    // Verify images have valid dimensions (header probe works)
    let path = "testsdoc/如何学习 (本尼迪克特·凯里,Benedict Carey,玉冰) (z-library.sk, 1lib.sk, z-lib.sk).epub";
    let doc = ReflowDocument::open(path).expect("Failed to open");
    for (i, block) in doc.content_blocks(0).iter().enumerate() {
        if let ContentBlock::Image(img) = block {
            assert!(img.width > 0 && img.height > 0,
                "Image block {} has invalid dimensions: {}x{}", i, img.width, img.height);
            assert!(!img.raw_bytes.is_empty(), "Image block {} has no raw bytes", i);
        }
    }
}
