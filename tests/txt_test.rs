use folix::app::engines::{ReflowLayout, ContentBlock};
use std::io::Write;

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

#[test]
fn test_open_markdown() {
    let dir = std::env::temp_dir().join("folix_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.md");
    let md_content = r#"# Chapter 1
Hello **world** with *some* `code`.
More text.

# Chapter 2
Another *italic* paragraph.

## Subheading
Some ~~strikethrough~~ text.
"#;
    std::fs::write(&path, md_content).unwrap();

    let doc = folix::app::engines::reflow_engine::ReflowDocument::open(
        path.to_str().unwrap()
    ).expect("Failed to open Markdown");

    // Two chapters from # headings (## is not detected by the TXT splitter, but that's OK)
    assert!(doc.chapter_count() >= 2, "Should have at least 2 chapters from # headings, got {}", doc.chapter_count());
    let ch0 = doc.load_chapter(0);
    let text0: String = ch0.blocks.iter()
        .map(|b| match b { ContentBlock::Text(t) => t.as_str(), _ => "" })
        .collect::<Vec<&str>>()
        .join("");
    // Markdown formatting stripped: **world** → world, *some* → some, `code` → code
    assert!(text0.contains("world"), "Bold text should be kept: {:?}", text0);
    assert!(text0.contains("code"), "Inline code text should be kept: {:?}", text0);
    assert!(!text0.contains("**"), "Bold markers should be stripped: {:?}", text0);
    assert!(!text0.contains('*'), "Italic markers should be stripped: {:?}", text0);

    let ch1 = doc.load_chapter(1);
    let text1: String = ch1.blocks.iter()
        .map(|b| match b { ContentBlock::Text(t) => t.as_str(), _ => "" })
        .collect::<Vec<&str>>()
        .join("");
    assert!(text1.contains("strikethrough"), "Strikethrough text should be kept: {:?}", text1);
    assert!(!text1.contains("~~"), "Strikethrough markers should be stripped: {:?}", text1);
}

#[test]
fn test_open_markdown_links_images() {
    let dir = std::env::temp_dir().join("folix_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("links.md");
    let md = r#"# Links
This is [a link](https://example.com) and an image ![alt](img.png).

More text.
"#;
    std::fs::write(&path, md).unwrap();

    let doc = folix::app::engines::reflow_engine::ReflowDocument::open(
        path.to_str().unwrap()
    ).expect("Failed to open MD with links");

    let ch = doc.load_chapter(0);
    let text: String = ch.blocks.iter()
        .map(|b| match b { ContentBlock::Text(t) => t.as_str(), _ => "" })
        .collect::<Vec<&str>>()
        .join("");
    assert!(text.contains("a link"), "Link text should be kept: {:?}", text);
    assert!(!text.contains("https://"), "URL should be stripped: {:?}", text);
    assert!(!text.contains("[a link]"), "Link brackets should be stripped: {:?}", text);
    assert!(!text.contains("![alt]"), "Image syntax should be stripped: {:?}", text);
}

#[test]
fn test_open_docx_simple() {


    let dir = std::env::temp_dir().join("folix_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.docx");

    // Create a minimal DOCX (ZIP containing [Content_Types].xml, word/document.xml)
    let docx_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r>
        <w:t>Hello World</w:t>
      </w:r>
    </w:p>
    <w:p>
      <w:pPr>
        <w:pStyle w:val="Heading1"/>
      </w:pPr>
      <w:r>
        <w:t>Chapter One</w:t>
      </w:r>
    </w:p>
    <w:p>
      <w:r>
        <w:t>Some body text here.</w:t>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
</Types>"#;

    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(content_types.as_bytes()).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(rels.as_bytes()).unwrap();

    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(docx_xml.as_bytes()).unwrap();

    zip.finish().unwrap();

    // Now test opening
    let doc = folix::app::engines::reflow_engine::ReflowDocument::open(
        path.to_str().unwrap()
    ).expect("Failed to open DOCX");

    assert_eq!(doc.chapter_count(), 2, "Should have 2 chapters (heading + body)");

    let ch0 = doc.load_chapter(0);
    let text0: String = ch0.blocks.iter()
        .map(|b| match b { ContentBlock::Text(t) => t.as_str(), _ => "" })
        .collect::<Vec<&str>>()
        .join("");
    assert!(text0.contains("Hello World"), "Chapter 0 should contain Hello World: {:?}", text0);

    let ch1 = doc.load_chapter(1);
    let text1: String = ch1.blocks.iter()
        .map(|b| match b { ContentBlock::Text(t) => t.as_str(), _ => "" })
        .collect::<Vec<&str>>()
        .join("");
    assert!(text1.contains("Chapter One"), "Chapter 1 should contain heading text: {:?}", text1);
    assert!(text1.contains("Some body text here."), "Chapter 1 should contain body text: {:?}", text1);
}
