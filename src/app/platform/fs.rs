pub struct FileSystem;

impl FileSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn read_to_string(&self, path: &str) -> Result<String, std::io::Error> {
        std::fs::read_to_string(path)
    }
}
