use std::time::Instant;
use folix::app::engines::{Document, ContentBlock, reflow_engine::ReflowDocument};

#[test]
fn test_open_epub_books() {
    let mut total_imgs = 0;
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
        println!("  pages: {}", doc.page_count());
        println!("  toc entries: {}", doc.toc_entries().len());

        // Sum blocks and images across all pages
        let mut total_blocks = 0;
        let mut img_count = 0;
        let mut total_text_len = 0;
        for p in 0..doc.page_count() {
            let blocks = doc.content_blocks(p);
            total_blocks += blocks.len();
            for b in &blocks {
                match b {
                    ContentBlock::Text(t) => total_text_len += t.len(),
                    ContentBlock::Image(_) => img_count += 1,
                }
            }
        }
        println!("  total blocks: {}", total_blocks);
        println!("  total text len: {}", total_text_len);
        println!("  total images: {}", img_count);
        total_imgs += img_count;
        assert!(img_count > 0, "Expected at least 1 image block in {}", path);
    }
    assert!(total_imgs >= 10, "Expected >=10 total image blocks across both books, got {}", total_imgs);
}

#[test]
fn test_open_chinese_epub_fast() {
    // Must open quickly (< 2s with lazy loading)
    let path = "testsdoc/如何学习 (本尼迪克特·凯里,Benedict Carey,玉冰) (z-library.sk, 1lib.sk, z-lib.sk).epub";
    let start = Instant::now();
    let result = ReflowDocument::open(path);
    let elapsed = start.elapsed();
    assert!(result.is_some(), "Failed to open");
    println!("Opened in {:?}", elapsed);
    assert!(elapsed.as_secs() < 2, "Open took too long: {:?}", elapsed);

    let doc = result.unwrap();
    assert!(doc.page_count() > 0, "No pages");

    // Sum images across all pages
    let mut total_images = 0;
    for p in 0..doc.page_count() {
        let blocks = doc.content_blocks(p);
        total_images += blocks.iter().filter(|b| matches!(b, ContentBlock::Image(_))).count();
    }
    assert!(total_images >= 10, "Expected >=10 image blocks, got {}", total_images);
    println!("Images found: {}", total_images);
}

#[test]
fn test_image_dimensions_valid() {
    let path = "testsdoc/如何学习 (本尼迪克特·凯里,Benedict Carey,玉冰) (z-library.sk, 1lib.sk, z-lib.sk).epub";
    let doc = ReflowDocument::open(path).expect("Failed to open");
    for p in 0..doc.page_count() {
        for (i, block) in doc.content_blocks(p).iter().enumerate() {
            if let ContentBlock::Image(img) = block {
                assert!(img.width > 0 && img.height > 0,
                    "Image block {} in page {} has invalid dimensions: {}x{}", i, p, img.width, img.height);
                assert!(!img.raw_bytes.is_empty(), "Image block {} in page {} has no raw bytes", i, p);
            }
        }
    }
}
