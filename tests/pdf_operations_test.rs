use std::process::Command;

fn pdf_page_count(path: &str) -> usize {
    if let Ok(output) = Command::new("pdfinfo").arg(path).output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(count) = line.strip_prefix("Pages:") {
                return count.trim().parse().expect("parse page count");
            }
        }
    }
    // Avoid MuDocument for page counting due to mupdf-rs 0.8.0 memory corruption bug
    let doc = mupdf::pdf::PdfDocument::open(path).expect("open pdf");
    doc.page_count().expect("page count") as usize
}

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(name);
    let _ = std::fs::create_dir_all(&d);
    d
}

#[test]
fn t01_page_count() {
    assert_eq!(
        folix::app::engines::pdf_operations::page_count("testsdoc/Math.pdf").unwrap(),
        99
    );
}

#[test]
fn t02_split_range() {
    let out = tmp_dir("t02").join("a.pdf");
    let s = out.to_string_lossy().to_string();
    let r = folix::app::engines::pdf_operations::split_pdf_by_range("testsdoc/Math.pdf", &s, 1, 10);
    assert!(r.is_ok(), "split 1-10: {:?}", r);
    assert_eq!(pdf_page_count(&s), 10);
    let _ = std::fs::remove_file(&s);

    // single page
    let s2 = tmp_dir("t02").join("b.pdf").to_string_lossy().to_string();
    let r2 = folix::app::engines::pdf_operations::split_pdf_by_range("testsdoc/Math.pdf", &s2, 5, 5);
    assert!(r2.is_ok(), "split 5-5: {:?}", r2);
    assert_eq!(pdf_page_count(&s2), 1);
    let _ = std::fs::remove_file(&s2);

    // full doc range
    let s3 = tmp_dir("t02").join("c.pdf").to_string_lossy().to_string();
    let r3 = folix::app::engines::pdf_operations::split_pdf_by_range("testsdoc/Math.pdf", &s3, 1, 99);
    assert!(r3.is_ok(), "split 1-99: {:?}", r3);
    assert_eq!(pdf_page_count(&s3), 99);
    let _ = std::fs::remove_file(&s3);
}

#[test]
fn t03_split_every_n() {
    let d = tmp_dir("t03");
    let ds = d.to_string_lossy().to_string();
    // every 10 pages → 10 chunks (first 9 have 10, last has 9)
    let r = folix::app::engines::pdf_operations::split_pdf_every_n("testsdoc/Math.pdf", &ds, 10);
    assert!(r.is_ok(), "every 10: {:?}", r);
    let paths = r.unwrap();
    assert_eq!(paths.len(), 10);
    for (i, p) in paths.iter().enumerate() {
        let expected = if i < 9 { 10 } else { 9 };
        assert_eq!(pdf_page_count(p), expected, "chunk {i}: expected {expected}");
        let _ = std::fs::remove_file(p);
    }

    // every 50 pages → 2 chunks (50 and 49)
    let r2 = folix::app::engines::pdf_operations::split_pdf_every_n("testsdoc/Math.pdf", &ds, 50);
    assert!(r2.is_ok());
    let paths2 = r2.unwrap();
    assert_eq!(paths2.len(), 2);
    assert_eq!(pdf_page_count(&paths2[0]), 50);
    assert_eq!(pdf_page_count(&paths2[1]), 49);
    for p in &paths2 { let _ = std::fs::remove_file(p); }
}

#[test]
fn t04_extract_images() {
    let d = tmp_dir("t04");
    let ds = d.to_string_lossy().to_string();
    let r = folix::app::engines::pdf_operations::extract_pages_as_images("testsdoc/Math.pdf", &ds, &[0, 1, 2]);
    assert!(r.is_ok(), "extract: {:?}", r);
    let paths = r.unwrap();
    assert_eq!(paths.len(), 3);
    for p in &paths {
        assert!(std::path::Path::new(p).exists());
        assert!(p.ends_with(".png"));
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn t05_extract_text() {
    let out = tmp_dir("t05").join("a.txt");
    let s = out.to_string_lossy().to_string();
    let r = folix::app::engines::pdf_operations::extract_pdf_text("testsdoc/Math.pdf", &s);
    assert!(r.is_ok(), "text: {:?}", r);
    let text = std::fs::read_to_string(&s).unwrap();
    assert!(text.len() > 1000);
    assert!(text.contains("Page 1"));
    let _ = std::fs::remove_file(&s);
}

#[test]
fn t06_split_toc() {
    let d = tmp_dir("t06");
    let ds = d.to_string_lossy().to_string();
    let r = folix::app::engines::pdf_operations::split_pdf_by_toc("testsdoc/Math.pdf", &ds);
    assert!(r.is_ok(), "toc split: {:?}", r);
    let paths = r.unwrap();
    assert!(!paths.is_empty(), "should have chapters");
    for p in &paths { let _ = std::fs::remove_file(p); }
}

#[test]
fn t07_merge_pdfs() {
    // Create two small split PDFs to merge back
    let d = tmp_dir("t07");
    let ds = d.to_string_lossy().to_string();

    // Split first 10 pages into two 5-page PDFs
    let r = folix::app::engines::pdf_operations::split_pdf_by_range("testsdoc/Math.pdf", &format!("{ds}/a.pdf"), 1, 5);
    assert!(r.is_ok());
    let r = folix::app::engines::pdf_operations::split_pdf_by_range("testsdoc/Math.pdf", &format!("{ds}/b.pdf"), 6, 10);
    assert!(r.is_ok());

    // Merge them back
    let merged = format!("{ds}/merged.pdf");
    let r = folix::app::engines::pdf_operations::merge_pdfs(&[format!("{ds}/a.pdf"), format!("{ds}/b.pdf")], &merged);
    assert!(r.is_ok(), "merge: {:?}", r);
    assert_eq!(pdf_page_count(&merged), 10);

    for p in &[format!("{ds}/a.pdf"), format!("{ds}/b.pdf"), merged] {
        let _ = std::fs::remove_file(p);
    }
}
