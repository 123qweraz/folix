use folix::app::engines::{FixedLayout, Document};

fn make_test_png(width: u32, height: u32) -> Vec<u8> {
    use std::io::BufWriter;
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;
    let mut png_data = Vec::new();
    {
        let writer = BufWriter::new(&mut png_data);
        let encoder = PngEncoder::new(writer);
        let rgba: Vec<u8> = (0..width * height)
            .flat_map(|i| {
                let r = (i % 256) as u8;
                let g = ((i / 256) % 256) as u8;
                let b = ((i / 65536) % 256) as u8;
                [r, g, b, 255u8]
            })
            .collect();
        encoder
            .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)
            .unwrap();
    }
    png_data
}

#[test]
fn test_image_open_png() {
    let dir = std::env::temp_dir().join("folix_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.png");
    let png = make_test_png(100, 80);
    std::fs::write(&path, &png).unwrap();

    let doc = folix::app::engines::image_engine::ImageDocument::open(
        path.to_str().unwrap()
    ).expect("Failed to open PNG");
    assert_eq!(doc.page_count(), 1);
    assert!(doc.title().contains("test"));
}

#[test]
fn test_image_render_page() {
    let dir = std::env::temp_dir().join("folix_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("render_test.png");
    let png = make_test_png(50, 30);
    std::fs::write(&path, &png).unwrap();

    let doc = folix::app::engines::image_engine::ImageDocument::open(
        path.to_str().unwrap()
    ).expect("Failed to open PNG");

    // Page 0 should render
    let page = doc.render_page(0, 1.0).expect("Page 0 should render");
    assert_eq!(page.width, 50);
    assert_eq!(page.height, 30);

    // Page 1 should be None (single page)
    assert!(doc.render_page(1, 1.0).is_none());

    // Scaled render
    let scaled = doc.render_page(0, 2.0).expect("Scaled page should render");
    assert_eq!(scaled.width, 100);
    assert_eq!(scaled.height, 60);
}

#[test]
fn test_image_page_size() {
    let dir = std::env::temp_dir().join("folix_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("size_test.png");
    let png = make_test_png(200, 150);
    std::fs::write(&path, &png).unwrap();

    let doc = folix::app::engines::image_engine::ImageDocument::open(
        path.to_str().unwrap()
    ).expect("Failed to open PNG");

    let size = doc.page_size(0, 1.0).expect("Should have size");
    assert_eq!(size, (200.0, 150.0));

    let scaled = doc.page_size(0, 0.5).expect("Should have scaled size");
    assert_eq!(scaled, (100.0, 75.0));
}

#[test]
fn test_image_no_text() {
    let dir = std::env::temp_dir().join("folix_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("text_test.png");
    let png = make_test_png(10, 10);
    std::fs::write(&path, &png).unwrap();

    let doc = folix::app::engines::image_engine::ImageDocument::open(
        path.to_str().unwrap()
    ).expect("Failed to open PNG");

    assert!(doc.page_text(0).is_empty());
    assert!(doc.page_text_positions(0).is_empty());
    assert!(doc.toc_entries().is_empty());
}

#[test]
fn test_image_invalid() {
    let dir = std::env::temp_dir().join("folix_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("invalid.png");
    std::fs::write(&path, b"not a real image").unwrap();

    let doc = folix::app::engines::image_engine::ImageDocument::open(
        path.to_str().unwrap()
    );
    assert!(doc.is_none(), "Invalid image should return None");
}
