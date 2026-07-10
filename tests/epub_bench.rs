use std::time::Instant;
use folix::app::engines::{Document, ReflowLayout, ContentBlock, reflow_engine::ReflowDocument};

fn bench(path: &str) {
    let t = Instant::now();
    let doc = ReflowDocument::open(path).unwrap();
    println!("  open():      {:>8?}", t.elapsed());

    let n = doc.chapter_count();
    println!("  chapters: {}", n);

    let t = Instant::now();
    for ci in 0..n {
        let _ch = doc.load_chapter(ci, false);
    }
    println!("  text parse:  {:>8?}  ({} chapters)", t.elapsed(), n);

    let t = Instant::now();
    let mut img_count = 0;
    for ci in 0..n {
        let ch = doc.load_chapter(ci, true);
        img_count += ch.blocks.iter().filter(|b| matches!(b, ContentBlock::Image(_))).count();
    }
    println!("  load images:{:>8?}  ({} images)", t.elapsed(), img_count);
}

#[test]
fn bench_epub_books() {
    let paths: &[&str] = &[
        "testsdoc/Building AI Agent Platforms (Ben OMahony and Fabian Nonnenmacher) (z-library.sk, 1lib.sk, z-lib.sk).epub",
        "testsdoc/如何学习 (本尼迪克特·凯里,Benedict Carey,玉冰) (z-library.sk, 1lib.sk, z-lib.sk).epub",
        "testsdoc/Python机器学习手册从数据预处理到深度学习 (Chris Albon著,韩慧昌,林然,徐江译) (z-library.sk, 1lib.sk, z-lib.sk).epub",
    ];
    for path in paths {
        println!("\n{}", path);
        bench(path);
    }
}
