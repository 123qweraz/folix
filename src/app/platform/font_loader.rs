use std::path::PathBuf;

pub struct FontLoader;

impl FontLoader {
    pub fn new() -> Self {
        Self
    }

    /// Find system CJK font paths. Returns empty vec if none found.
    pub fn load_system_fonts(&self) -> Vec<PathBuf> {
        let candidates = vec![
            PathBuf::from("/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf"),
            PathBuf::from("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
            PathBuf::from("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"),
        ];
        candidates.into_iter().filter(|p| p.exists()).collect()
    }
}
