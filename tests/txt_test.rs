use folix::app::engines::{ReflowLayout, ContentBlock};

#[test]
fn test_open_txt_utf8() {
    let dir = std::env::temp_dir().join("folix_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test_utf8.txt");
    std::fs::write(&path, b"Hello World\nLine 2").unwrap();

    let doc = folix::app::engines::reflow_engine::ReflowDocument::open(
        path.to_str().unwrap()
    ).expect("Failed to open UTF-8 TXT");
    assert_eq!(doc.chapter_count(), 1);
    let ch = doc.load_chapter(0);
    let text: String = ch.blocks.iter()
        .map(|b| match b { ContentBlock::Text(t) => t.as_str(), _ => "" })
        .collect::<Vec<&str>>()
        .join("");
    assert!(text.contains("Hello World"));
}

#[test]
fn test_open_txt_gbk_small() {
    let dir = std::env::temp_dir().join("folix_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test_gbk.txt");
    let gbk_data = b"\xd6\xd0\xce\xc4\xb2\xe2\xca\xd4\nLine2";
    std::fs::write(&path, gbk_data).unwrap();

    let doc = folix::app::engines::reflow_engine::ReflowDocument::open(
        path.to_str().unwrap()
    ).expect("Failed to open GBK TXT");
    let ch = doc.load_chapter(0);
    let text: String = ch.blocks.iter()
        .map(|b| match b { ContentBlock::Text(t) => t.as_str(), _ => "" })
        .collect::<Vec<&str>>()
        .join("");
    assert!(text.contains("中文测试"), "GBK text should decode as '中文测试', got: {:?}", text);
}

#[test]
fn test_open_chinese_novel() {
    let path = concat!(
        "testsdoc/《我家老婆来自一千年前》",
        "（校对版全本）作者：花还没开.txt"
    );
    let doc = folix::app::engines::reflow_engine::ReflowDocument::open(path)
        .expect("Failed to open Chinese novel TXT");
    assert!(doc.chapter_count() >= 3, "Should be split into chapters, got {}", doc.chapter_count());
    // Concatenate all chapters' text
    let mut full_text = String::new();
    for i in 0..doc.chapter_count() {
        let ch = doc.load_chapter(i);
        for b in ch.blocks {
            if let ContentBlock::Text(t) = b {
                full_text.push_str(&t);
            }
        }
    }
    assert!(!full_text.is_empty(), "Should have text content");
    assert!(full_text.chars().count() > 100000, "Full text should be long");
    assert!(
        full_text.contains("更多") || full_text.contains("精校") || full_text.contains("下载"),
        "Text should contain expected Chinese: got prefix {:?}",
        &full_text[..full_text.len().min(100)]
    );
    println!("Chinese TXT: {} chars, {} chapters", full_text.chars().count(), doc.chapter_count());
}
